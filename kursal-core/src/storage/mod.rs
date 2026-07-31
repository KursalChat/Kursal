use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::file_transfers::FileTransferEntry,
    contacts::Contact,
    crypto::derive_key,
    identity::UserId,
    storage::filetransfer::{get_auto_download_storage_for, get_folder_size},
};
use libsignal_protocol::{DeviceId, IdentityKeyPair, ProtocolAddress};
use redb::ReadableDatabase;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::SystemTime};

pub mod backup;
mod db;
pub mod file;
pub mod filetransfer;
pub mod image_metadata;
mod settings;
mod signal_stores;

pub use db::*;
pub use settings::*;
pub use signal_stores::*;

pub fn delete_message_history_for(db: &Database, contact_id: String) -> Result<()> {
    db.raw_delete_prefix(TABLE_PINNED, &format!("{contact_id}:"))?;
    db.raw_delete_prefix(TABLE_MESSAGES, &format!("{contact_id}:"))
}

pub fn delete_message_history_all(db: &Database) -> Result<()> {
    db.raw_delete_all(TABLE_PINNED)?;
    db.raw_delete_all(TABLE_MESSAGES)
}

pub fn get_local_identity_pub(db: &Database) -> Result<Vec<u8>> {
    let identity_bytes = db
        .raw_read(TABLE_IDENTITY_KEYS, "local_identity")?
        .ok_or(KursalError::Storage("No local identity".to_string()))?;

    let identity_keypair =
        IdentityKeyPair::try_from(identity_bytes.as_slice()).ok_kursal(KursalError::Identity)?;

    Ok(identity_keypair.public_key().serialize().to_vec())
}

pub fn get_local_user_id(db: &Database) -> Result<UserId> {
    let identity_pub_key = get_local_identity_pub(db)?;
    let user_id: [u8; 32] = Sha256::digest(&identity_pub_key).into();

    Ok(UserId(user_id))
}

pub fn get_local_address(db: &Database) -> Result<ProtocolAddress> {
    let user_id = get_local_user_id(db)?;

    Ok(ProtocolAddress::new(
        hex::encode(user_id.0),
        DeviceId::new(1u8).unwrap(),
    ))
}

pub fn get_dilithium_pub(db: &Database) -> Result<Vec<u8>> {
    db.raw_read(TABLE_SETTINGS, "dilithium_public")?
        .ok_or(KursalError::Storage("No dilithium public key".to_string()))
}

//

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactUsage {
    pub contact_id: String,
    pub db_bytes: u64,
    pub files_bytes: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub logs_bytes: u64,
    pub db_bytes: u64,
    pub files_bytes: u64,
    pub per_contact: Vec<ContactUsage>,
}
impl StorageUsage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub fn get_storage_usage(
    db: &Database,
    logs_dir: PathBuf,
    app_data_dir: PathBuf,
    db_path: PathBuf,
) -> Result<StorageUsage> {
    let logs_bytes = get_folder_size(logs_dir, 1)?;
    let db_bytes = db_path
        .metadata()
        .map(|m| m.len())
        .map_err(KursalError::Io)?;

    let mut files_bytes = 0u64;

    let contacts = Contact::load_all(db)?;
    let mut per_contact = Vec::with_capacity(contacts.len());
    for contact in contacts.into_iter() {
        let contact_id = hex::encode(contact.user_id.0);

        let contact_files_bytes = get_auto_download_storage_for(&app_data_dir, &contact_id)?;

        let read_txn = db.inner.begin_read().ok_kursal(KursalError::Storage)?;

        let table = match read_txn.open_table(TABLE_MESSAGES) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                per_contact.push(ContactUsage {
                    contact_id,
                    db_bytes: 0u64,
                    files_bytes: contact_files_bytes,
                });
                continue;
            }
            Err(err) => return Err(KursalError::Storage(err.to_string())),
        };

        let prefix = format!("{contact_id}:");
        let op = table
            .range(prefix.as_str()..)
            .ok_kursal(KursalError::Storage)?;

        let mut db_bytes_sum = 0u64;
        for entry in op {
            let (k, v) = entry.ok_kursal(KursalError::Storage)?;
            if !k.value().starts_with(&prefix) {
                break;
            }
            db_bytes_sum += v.value().len() as u64;
        }

        per_contact.push(ContactUsage {
            contact_id,
            db_bytes: db_bytes_sum,
            files_bytes: contact_files_bytes,
        });
        files_bytes += contact_files_bytes;
    }

    Ok(StorageUsage {
        logs_bytes,
        db_bytes,
        files_bytes,
        per_contact,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedFileEntry {
    pub id: String,
    pub filepath: String,
    pub size_bytes: u64,
    pub recipient_id: String,
    pub shared_at: u64,
    pub last_accessed_at: Option<u64>,
}
impl SharedFileEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub fn files_list_shared(db: &Database) -> Result<Vec<SharedFileEntry>> {
    let get_all = db
        .raw_range(TABLE_FILE_TRANSFERS, "send:", "send;", None)?
        .into_iter()
        .filter_map(|(key, bytes)| {
            if let Ok(entry) = FileTransferEntry::deserialize(&bytes) {
                Some((key, entry))
            } else {
                None
            }
        })
        .map(|(key, entry)| SharedFileEntry {
            id: key.clone(),
            filepath: entry.path,
            size_bytes: entry.size_bytes,
            recipient_id: key.split(':').nth(1usize).unwrap_or("unknown").to_string(),
            shared_at: entry.shared_at,
            last_accessed_at: entry.last_accessed_at,
        })
        .collect();

    Ok(get_all)
}
pub fn files_revoke_shared(db: &Database, id: String) -> Result<()> {
    db.raw_delete(TABLE_FILE_TRANSFERS, &id)?;

    Ok(())
}

//

pub fn derive_db_key(secret: &[u8]) -> Result<[u8; 32]> {
    derive_key(secret, b"kursal-db-key")
}

//

#[allow(clippy::cast_possible_truncation)]
pub fn get_timestamp_millis() -> Result<u64> {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|el| el.as_millis() as u64)
        .ok_kursal(KursalError::Crypto)
}

pub fn get_timestamp_secs() -> Result<u64> {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|el| el.as_secs())
        .ok_kursal(KursalError::Crypto)
}
