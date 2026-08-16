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

pub fn incoming_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("incoming")
}

pub fn incoming_contact_dir(app_data_dir: &Path, contact_hex: &str) -> PathBuf {
    incoming_root(app_data_dir).join(sanitize_filename(contact_hex))
}

pub fn incoming_offer_dir(app_data_dir: &Path, contact_hex: &str, offer_hex: &str) -> PathBuf {
    incoming_contact_dir(app_data_dir, contact_hex).join(sanitize_filename(offer_hex))
}

pub fn download_path(
    app_data_dir: &Path,
    contact_hex: &str,
    offer_hex: &str,
    filename: &str,
) -> PathBuf {
    incoming_offer_dir(app_data_dir, contact_hex, offer_hex).join(sanitize_filename(filename))
}

fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|p| p.exists())
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
pub fn available_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let dir = existing_ancestor(path)?;
    let c_dir = CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();

    if unsafe { libc::statvfs(c_dir.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return None;
    }

    let stat = unsafe { stat.assume_init() };
    (stat.f_bavail as u64).checked_mul(stat.f_frsize as u64)
}

#[cfg(windows)]
pub fn available_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::core::PCWSTR;

    let dir = existing_ancestor(path)?;
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut available = 0u64;

    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut available), None, None) }.ok()?;

    Some(available)
}

#[cfg(not(any(unix, windows)))]
pub fn available_space(_path: &Path) -> Option<u64> {
    None
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

pub fn get_auto_download_storage(app_data_dir: &Path) -> Result<u64> {
    get_folder_size(incoming_root(app_data_dir), 3)
}

pub fn get_auto_download_storage_for(app_data_dir: &Path, contact_id: &str) -> Result<u64> {
    get_folder_size(incoming_contact_dir(app_data_dir, contact_id), 2)
}
