use crate::{
    KursalError, Result,
    contacts::Contact,
    identity::UserId,
    messaging::offline::OfflineState,
    storage::{Database, TABLE_CONTACTS, TABLE_SETTINGS, get_local_user_id},
};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

pub const LOCAL_PROFILE_AVATAR_KEY: &str = "local_profile_avatar";
const MIGRATED_KEY: &str = "!avatars_migrated";
const MIGRATED_USER_ID_KEY: &str = "!avatars_by_user";
const EXTENSIONS: [&str; 3] = ["webp", "jpg", "png"];

static AVATAR_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init(app_data_dir: &Path) -> Result<()> {
    let dir = app_data_dir.join("avatars");
    std::fs::create_dir_all(&dir).map_err(KursalError::Io)?;
    let _ = AVATAR_DIR.set(dir);

    Ok(())
}

pub fn dir() -> Option<&'static PathBuf> {
    AVATAR_DIR.get()
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_file_name(value: &str) -> bool {
    match value.split_once('.') {
        Some((stem, ext)) => is_hash(stem) && EXTENSIONS.contains(&ext),
        None => false,
    }
}

fn extension_for(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
        "webp"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else {
        "webp"
    }
}

pub fn path_for(name: &str) -> Option<PathBuf> {
    if !is_file_name(name) {
        return None;
    }

    Some(dir()?.join(name))
}

pub fn path_string(name: &str) -> Option<String> {
    let path = path_for(name)?;
    let version = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_millis())
        .unwrap_or(0);

    Some(format!("{}?v={version}", path.to_string_lossy()))
}

pub fn store(user_id: &UserId, bytes: &[u8]) -> Result<String> {
    let id = hex::encode(user_id.0);
    let name = format!("{id}.{}", extension_for(bytes));
    let path = path_for(&name)
        .ok_or_else(|| KursalError::Storage("Avatar directory not initialized".to_string()))?;

    for ext in EXTENSIONS {
        let stale = format!("{id}.{ext}");
        if stale != name
            && let Some(stale_path) = path_for(&stale)
        {
            let _ = std::fs::remove_file(stale_path);
        }
    }

    std::fs::write(&path, bytes).map_err(KursalError::Io)?;

    Ok(name)
}

pub fn read(name: &str) -> Option<Vec<u8>> {
    std::fs::read(path_for(name)?).ok()
}

pub fn read_all() -> Vec<(String, Vec<u8>)> {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_string();
            if !is_file_name(&name) {
                return None;
            }
            Some((name, std::fs::read(&path).ok()?))
        })
        .collect()
}

pub fn restore(avatars: Vec<(String, Vec<u8>)>) -> Result<()> {
    for (name, bytes) in avatars {
        let name = if is_hash(&name) {
            format!("{name}.webp")
        } else {
            name
        };

        let Some(path) = path_for(&name) else {
            continue;
        };
        std::fs::write(path, bytes).map_err(KursalError::Io)?;
    }

    Ok(())
}

#[derive(Deserialize)]
struct LegacyContact {
    user_id: UserId,
    peer_id: String,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
    identity_pub_key: Vec<u8>,
    dilithium_pub_key: Vec<u8>,
    known_addresses: Vec<String>,
    verified: bool,
    profile_shared: bool,
    blocked: bool,
    created_at: u64,
    offline: OfflineState,
}

fn adopt_legacy_bytes(user_id: &UserId, bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    match std::str::from_utf8(bytes) {
        Ok(text) if is_hash(text) => Some(text.to_string()),
        _ => store(user_id, bytes).ok(),
    }
}

fn legacy_path(hash: &str) -> Option<PathBuf> {
    if !is_hash(hash) {
        return None;
    }

    Some(dir()?.join(format!("{hash}.webp")))
}

fn rename_to_user_id(user_id: &UserId, hash: &str) -> Option<String> {
    let bytes = std::fs::read(legacy_path(hash)?).ok()?;

    store(user_id, &bytes).ok()
}

pub fn migrate(db: &Database) -> Result<()> {
    if matches!(db.raw_read(TABLE_SETTINGS, MIGRATED_KEY), Ok(Some(_))) {
        return Ok(());
    }

    for (key, bytes) in db.raw_readall(TABLE_CONTACTS)? {
        let legacy = match bincode::deserialize::<LegacyContact>(&bytes) {
            Ok(legacy) => legacy,
            Err(err) => {
                log::warn!("[avatars] skipping undeserializable contact {key}: {err}");
                continue;
            }
        };

        let contact = Contact {
            avatar: legacy
                .avatar_bytes
                .as_deref()
                .and_then(|bytes| adopt_legacy_bytes(&legacy.user_id, bytes)),
            user_id: legacy.user_id,
            peer_id: legacy.peer_id,
            display_name: legacy.display_name,
            identity_pub_key: legacy.identity_pub_key,
            dilithium_pub_key: legacy.dilithium_pub_key,
            known_addresses: legacy.known_addresses,
            verified: legacy.verified,
            profile_shared: legacy.profile_shared,
            blocked: legacy.blocked,
            created_at: legacy.created_at,
            offline: legacy.offline,
        };

        db.raw_write(TABLE_CONTACTS, &key, &contact.serialize()?)?;
    }

    if let Ok(Some(bytes)) = db.raw_read(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY) {
        let local = get_local_user_id(db)?;
        let stored = adopt_legacy_bytes(&local, &bytes).unwrap_or_default();
        db.raw_write(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY, stored.as_bytes())?;
    }

    db.raw_write(TABLE_SETTINGS, MIGRATED_KEY, &[1u8])?;

    Ok(())
}

pub fn migrate_to_user_ids(db: &Database) -> Result<()> {
    if matches!(
        db.raw_read(TABLE_SETTINGS, MIGRATED_USER_ID_KEY),
        Ok(Some(_))
    ) {
        return Ok(());
    }

    for mut contact in Contact::load_all(db)? {
        let Some(hash) = contact.avatar.clone().filter(|value| is_hash(value)) else {
            continue;
        };

        contact.avatar = rename_to_user_id(&contact.user_id, &hash);
        contact.save(db)?;
    }

    if let Ok(Some(bytes)) = db.raw_read(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY)
        && let Ok(hash) = std::str::from_utf8(&bytes)
        && is_hash(hash)
    {
        let local = get_local_user_id(db)?;
        let renamed = rename_to_user_id(&local, hash).unwrap_or_default();
        db.raw_write(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY, renamed.as_bytes())?;
    }

    db.raw_write(TABLE_SETTINGS, MIGRATED_USER_ID_KEY, &[1u8])?;

    Ok(())
}

pub fn prune_unreferenced(db: &Database) -> Result<()> {
    let mut live: HashSet<String> = Contact::load_all(db)?
        .into_iter()
        .filter_map(|contact| contact.avatar)
        .collect();

    if let Ok(Some(bytes)) = db.raw_read(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY)
        && let Ok(name) = std::str::from_utf8(&bytes)
        && is_file_name(name)
    {
        live.insert(name.to_string());
    }

    prune(&live);

    Ok(())
}

pub fn prune(live: &HashSet<String>) {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let keep = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| live.contains(name));

        if !keep {
            let _ = std::fs::remove_file(path);
        }
    }
}
