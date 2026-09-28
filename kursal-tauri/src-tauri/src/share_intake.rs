use kursal_core::messaging::enums::MessageId;
use kursal_core::storage::filetransfer::{outgoing_pending_dir, sanitize_filename};
use kursal_core::{KursalError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const SHARE_DIR: &str = "kursal-shares";
pub const APP_GROUP: &str = "group.kursal.chat";
const MANIFEST: &str = "manifest.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareFile {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SharePayload {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub files: Vec<ShareFile>,
    #[serde(default)]
    pub text: Option<String>,
}

impl SharePayload {
    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.text.as_ref().is_none_or(|t| t.trim().is_empty())
    }
}

#[cfg(not(target_os = "ios"))]
pub fn share_root() -> Result<PathBuf> {
    Ok(crate::dirs::cache_dir()?.join(SHARE_DIR))
}

#[cfg(target_os = "ios")]
pub fn share_root() -> Result<PathBuf> {
    use objc2_foundation::{NSFileManager, NSString};

    let manager = NSFileManager::defaultManager();
    let group = NSString::from_str(APP_GROUP);
    let container = manager
        .containerURLForSecurityApplicationGroupIdentifier(&group)
        .ok_or_else(|| KursalError::Storage(format!("app group {APP_GROUP} unavailable")))?;
    let path = container
        .path()
        .ok_or_else(|| KursalError::Storage("app group container has no path".to_string()))?;

    Ok(PathBuf::from(path.to_string()).join(SHARE_DIR))
}

static OPENED: Mutex<Vec<SharePayload>> = Mutex::new(Vec::new());

/// Files handed over by the OS "Open with" flow
pub fn queue_opened(files: Vec<(String, String)>) -> bool {
    let files: Vec<ShareFile> = files
        .into_iter()
        .filter_map(|(path, filename)| {
            let size_bytes = std::fs::metadata(&path).ok()?.len();
            Path::new(&path).is_file().then_some(ShareFile {
                path,
                filename,
                size_bytes,
            })
        })
        .collect();

    if files.is_empty() {
        return false;
    }

    let payload = SharePayload {
        id: hex::encode(MessageId::new().0),
        files,
        text: None,
    };

    match OPENED.lock() {
        Ok(mut queue) => {
            queue.push(payload);
            true
        }
        Err(err) => {
            log::warn!("Could not queue opened files: {err}");
            false
        }
    }
}

pub fn drain_opened() -> Vec<SharePayload> {
    OPENED
        .lock()
        .map(|mut queue| queue.drain(..).collect())
        .unwrap_or_default()
}

pub fn take_pending(app_data_dir: &Path) -> Result<Vec<SharePayload>> {
    let mut payloads = take_pending_in(&share_root()?, app_data_dir)?;
    payloads.extend(drain_opened());
    Ok(payloads)
}

pub fn discard(app_data_dir: &Path, id: &str) -> Result<()> {
    discard_in(&share_root()?, app_data_dir, id)
}

pub fn take_pending_in(root: &Path, app_data_dir: &Path) -> Result<Vec<SharePayload>> {
    let Ok(entries) = root.read_dir() else {
        return Ok(Vec::new());
    };

    let mut found: Vec<(std::time::SystemTime, SharePayload)> = Vec::new();

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }

        let Some(id) = valid_id(&dir) else {
            continue;
        };

        let manifest = dir.join(MANIFEST);
        let Ok(raw) = std::fs::read(&manifest) else {
            continue;
        };

        let payload = match serde_json::from_slice::<SharePayload>(&raw) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("Discarding unreadable share manifest in {id}: {err}");
                let _ = std::fs::remove_dir_all(&dir);
                continue;
            }
        };

        let _ = std::fs::remove_file(&manifest);

        let queued_at = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let files = payload
            .files
            .into_iter()
            .filter(|file| is_contained_file(&dir, &file.path))
            .enumerate()
            .filter_map(|(index, file)| adopt_into_pending(app_data_dir, &id, index, file))
            .collect();

        let payload = SharePayload {
            id,
            files,
            text: payload.text,
        };

        let _ = std::fs::remove_dir_all(&dir);

        if payload.is_empty() {
            continue;
        }

        found.push((queued_at, payload));
    }

    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

    Ok(found.into_iter().map(|(_, payload)| payload).collect())
}

pub fn discard_in(root: &Path, app_data_dir: &Path, id: &str) -> Result<()> {
    if !is_safe_id(id) {
        return Err(KursalError::Storage(format!("invalid share id {id}")));
    }

    let dir = root.join(id);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(KursalError::Io)?;
    }

    discard_adopted(app_data_dir, id);

    Ok(())
}

fn adopt_into_pending(
    app_data_dir: &Path,
    share_id: &str,
    index: usize,
    file: ShareFile,
) -> Option<ShareFile> {
    let dir = outgoing_pending_dir(app_data_dir).join(format!("{share_id}-{index}"));
    if let Err(err) = std::fs::create_dir_all(&dir) {
        log::warn!("Could not stage shared file {}: {err}", file.filename);
        return None;
    }

    let dest = dir.join(sanitize_filename(&file.filename));
    let source = Path::new(&file.path);

    if std::fs::rename(source, &dest).is_err() {
        if let Err(err) = std::fs::copy(source, &dest) {
            log::warn!("Could not stage shared file {}: {err}", file.filename);
            let _ = std::fs::remove_dir_all(&dir);
            return None;
        }
        let _ = std::fs::remove_file(source);
    }

    Some(ShareFile {
        path: dest.to_string_lossy().into_owned(),
        ..file
    })
}

fn discard_adopted(app_data_dir: &Path, id: &str) {
    let Ok(entries) = outgoing_pending_dir(app_data_dir).read_dir() else {
        return;
    };

    let prefix = format!("{id}-");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        // Only "<id>-<index>", never a longer id that merely starts the same
        let is_own_dir = name
            .strip_prefix(&prefix)
            .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()));

        if is_own_dir {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn valid_id(dir: &Path) -> Option<String> {
    let id = dir.file_name()?.to_str()?.to_string();
    is_safe_id(&id).then_some(id)
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_contained_file(dir: &Path, path: &str) -> bool {
    let path = Path::new(path);
    if !path.is_file() {
        return false;
    }

    match (path.canonicalize(), dir.canonicalize()) {
        (Ok(file), Ok(dir)) => file.starts_with(dir),
        _ => false,
    }
}
