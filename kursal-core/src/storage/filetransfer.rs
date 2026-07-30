use crate::{KursalError, Result};
use blake2::{Blake2s256, Digest};
use std::io::Read;
use std::path::{Path, PathBuf};

const HASH_READ_BUF: usize = 64 * 1024;

pub fn hash_file(path: &str) -> Result<[u8; 32]> {
    let mut file = std::fs::File::open(path).map_err(KursalError::Io)?;
    let mut hasher = Blake2s256::new();
    let mut buf = vec![0u8; HASH_READ_BUF];

    loop {
        let read = file.read(&mut buf).map_err(KursalError::Io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    let out = hasher.finalize();
    out.as_slice()
        .try_into()
        .map_err(|_| KursalError::Crypto("Unexpected hash length".to_string()))
}

pub fn get_folder_size(path: PathBuf, recursion: usize) -> Result<u64> {
    let mut size = 0u64;

    if path.is_dir() {
        for file in path.read_dir().map_err(KursalError::Io)?.flatten() {
            let file_path = file.path();

            if file_path.is_file() {
                size += file_path.metadata().map(|m| m.len()).unwrap_or(0);
            } else if file_path.is_dir() && recursion >= 1 {
                size += get_folder_size(file_path, recursion.saturating_sub(1))?;
            }
        }
    }

    Ok(size)
}

pub fn sanitize_filename(name: &str) -> String {
    const MAX_LEN: usize = 100;

    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let trimmed = cleaned.trim().trim_matches('.');
    let safe = if trimmed.is_empty() { "file" } else { trimmed };

    safe.chars().take(MAX_LEN).collect()
}

pub fn download_path(
    cache_dir: PathBuf,
    contact_hex: &str,
    offer_hex: &str,
    filename: &str,
) -> PathBuf {
    cache_dir
        .join("files")
        .join(sanitize_filename(contact_hex))
        .join(sanitize_filename(offer_hex))
        .join(sanitize_filename(filename))
}

pub const OUTGOING_PENDING: &str = "pending";

pub fn outgoing_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("outgoing")
}

pub fn outgoing_pending_dir(app_data_dir: &Path) -> PathBuf {
    outgoing_root(app_data_dir).join(OUTGOING_PENDING)
}

pub fn outgoing_contact_dir(app_data_dir: &Path, contact_hex: &str) -> PathBuf {
    outgoing_root(app_data_dir).join(sanitize_filename(contact_hex))
}

pub fn outgoing_offer_dir(app_data_dir: &Path, contact_hex: &str, offer_hex: &str) -> PathBuf {
    outgoing_contact_dir(app_data_dir, contact_hex).join(sanitize_filename(offer_hex))
}

pub fn get_auto_download_storage(cache_dir: PathBuf) -> Result<u64> {
    get_folder_size(cache_dir.join("files"), 3)
}

pub fn get_auto_download_storage_for(cache_dir: PathBuf, contact_id: String) -> Result<u64> {
    get_folder_size(cache_dir.join("files").join(contact_id), 2)
}
