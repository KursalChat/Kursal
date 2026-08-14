use crate::{
    KursalError, Result,
    contacts::Contact,
    identity::UserId,
    messaging::offline::OfflineState,
    storage::{Database, TABLE_CONTACTS, TABLE_SETTINGS},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const LOCAL_PROFILE_AVATAR_KEY: &str = "local_profile_avatar";
const MIGRATED_KEY: &str = "!avatars_migrated";

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

pub fn path_for(hash: &str) -> Option<PathBuf> {
    if !is_hash(hash) {
        return None;
    }

    Some(dir()?.join(format!("{hash}.webp")))
}

pub fn path_string(hash: &str) -> Option<String> {
    Some(path_for(hash)?.to_string_lossy().into_owned())
}

pub fn store(bytes: &[u8]) -> Result<String> {
    let hash = hex::encode(Sha256::digest(bytes));
    let path = path_for(&hash)
        .ok_or_else(|| KursalError::Storage("Avatar directory not initialized".to_string()))?;

    if !path.exists() {
        std::fs::write(&path, bytes).map_err(KursalError::Io)?;
    }

    Ok(hash)
}

pub fn read(hash: &str) -> Option<Vec<u8>> {
    std::fs::read(path_for(hash)?).ok()
}

pub fn read_all() -> Vec<(String, Vec<u8>)> {
    let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let hash = path.file_stem()?.to_str()?.to_string();
            if !is_hash(&hash) {
                return None;
            }
            Some((hash, std::fs::read(&path).ok()?))
        })
        .collect()
}

pub fn restore(avatars: Vec<(String, Vec<u8>)>) -> Result<()> {
    for (hash, bytes) in avatars {
        let Some(path) = path_for(&hash) else {
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

fn adopt_legacy_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    match std::str::from_utf8(bytes) {
        Ok(text) if is_hash(text) => Some(text.to_string()),
        _ => store(bytes).ok(),
    }
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
            user_id: legacy.user_id,
            peer_id: legacy.peer_id,
            display_name: legacy.display_name,
            avatar: legacy.avatar_bytes.as_deref().and_then(adopt_legacy_bytes),
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
        let stored = adopt_legacy_bytes(&bytes).unwrap_or_default();
        db.raw_write(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY, stored.as_bytes())?;
    }

    db.raw_write(TABLE_SETTINGS, MIGRATED_KEY, &[1u8])?;

    Ok(())
}

pub fn prune_unreferenced(db: &Database) -> Result<()> {
    let mut live: HashSet<String> = Contact::load_all(db)?
        .into_iter()
        .filter_map(|contact| contact.avatar)
        .collect();

    if let Ok(Some(bytes)) = db.raw_read(TABLE_SETTINGS, LOCAL_PROFILE_AVATAR_KEY)
        && let Ok(hash) = std::str::from_utf8(&bytes)
        && is_hash(hash)
    {
        live.insert(hash.to_string());
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
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|hash| live.contains(hash));

        if !keep {
            let _ = std::fs::remove_file(path);
        }
    }
}
