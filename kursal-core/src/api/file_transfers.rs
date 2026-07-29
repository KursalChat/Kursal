use crate::MapKursalResult;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex as StdMutex};

use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::stream::{derive_stream_key, stream_decrypt_aad, stream_encrypt_aad},
    first_contact::{FileTransferMessage, WireMessage},
    identity::UserId,
    messaging::enums::{FileAccept, KursalMessage, MessageId},
    network::swarm::{FILE_CHUNK_SIZE, SwarmCommand, str_to_multiaddr},
    storage::{
        SharedDatabase, TABLE_FILE_TRANSFERS,
        filetransfer::{
            hash_file, outgoing_offer_dir, outgoing_pending_dir, sanitize_filename,
        },
        get_timestamp_secs, image_metadata,
    },
};
use std::path::{Path, PathBuf};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{mpsc, oneshot},
};

pub const MAX_FILE_TRANSFER_BYTES: u64 = 100 * 1024 * 1024 * 1024;
pub const STALE_TRANSFER_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;
const SEND_PROGRESS_THROTTLE: u64 = 16;
const FILE_PROGRESS_THROTTLE: u64 = 16;

fn move_into_place(source: &Path, dest: &Path) -> Result<()> {
    if std::fs::rename(source, dest).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, dest).map_err(KursalError::Io)?;
    let _ = std::fs::remove_file(source);
    Ok(())
}

fn discard_pending(source: &Path, pending_dir: &Path) {
    if !source.starts_with(pending_dir) {
        return;
    }
    match source.parent() {
        Some(parent) if parent != pending_dir => {
            let _ = std::fs::remove_dir_all(parent);
        }
        _ => {
            let _ = std::fs::remove_file(source);
        }
    }
}

pub async fn stage_outgoing(
    app_data_dir: PathBuf,
    contact_hex: String,
    offer_hex: String,
    source: PathBuf,
    filename: String,
) -> Result<Option<PathBuf>> {
    tokio::task::spawn_blocking(move || -> Result<Option<PathBuf>> {
        let pending_dir = outgoing_pending_dir(&app_data_dir);
        let from_pending = source.starts_with(&pending_dir);

        let plan = image_metadata::plan_strip(&source)?;
        if !plan.changed() && !from_pending {
            return Ok(None);
        }

        let dir = outgoing_offer_dir(&app_data_dir, &contact_hex, &offer_hex);
        std::fs::create_dir_all(&dir).map_err(KursalError::Io)?;
        let dest = dir.join(sanitize_filename(&filename));

        if plan.changed() {
            image_metadata::write_stripped(&source, &dest, &plan)?;
            if from_pending {
                discard_pending(&source, &pending_dir);
            }
        } else {
            move_into_place(&source, &dest)?;
            discard_pending(&source, &pending_dir);
        }

        Ok(Some(dest))
    })
    .await
    .ok_kursal(KursalError::Storage)?
}

pub fn remove_outgoing_offer(app_data_dir: &Path, contact_hex: &str, offer_hex: &str) {
    let dir = outgoing_offer_dir(app_data_dir, contact_hex, offer_hex);
    let _ = std::fs::remove_dir_all(dir);
}

pub async fn cleanup_stale_transfers(db: SharedDatabase, max_age_secs: u64) -> Result<()> {
    let now = get_timestamp_secs()?;
    let db_lock = db.0.lock().await;

    let incoming = db_lock.raw_range(TABLE_FILE_TRANSFERS, "recv:", "recv;", None)?;
    for (key, bytes) in incoming {
        if let Ok(entry) = FileIncomingEntry::deserialize(&bytes)
            && now.saturating_sub(entry.created_at) > max_age_secs
        {
            db_lock.raw_delete(TABLE_FILE_TRANSFERS, &key)?;
        }
    }

    let partial = db_lock.raw_range(TABLE_FILE_TRANSFERS, "recvprog:", "recvprog;", None)?;
    for (key, bytes) in partial {
        if let Ok(entry) = FileReceiveEntry::deserialize(&bytes)
            && now.saturating_sub(entry.created_at) > max_age_secs
        {
            let _ = std::fs::remove_file(&entry.save_path);
            db_lock.raw_delete(TABLE_FILE_TRANSFERS, &key)?;
        }
    }

    Ok(())
}

pub async fn remove_contact_transfers(db: SharedDatabase, contact_id: &UserId) -> Result<()> {
    let contact_hex = hex::encode(contact_id.0);
    let db_lock = db.0.lock().await;

    for prefix in ["send", "recv", "recvprog"] {
        let start = format!("{prefix}:{contact_hex}:");
        let end = format!("{prefix}:{contact_hex};");
        let entries = db_lock.raw_range(TABLE_FILE_TRANSFERS, &start, &end, None)?;

        for (key, bytes) in entries {
            let offer_hex = key.rsplit(':').next().unwrap_or_default();
            if let Ok(offer_bytes) = hex::decode(offer_hex)
                && let Ok(offer_arr) = <[u8; 16]>::try_from(offer_bytes)
            {
                mark_transfer_cancelled(offer_arr);
            }

            if prefix == "recvprog"
                && let Ok(entry) = FileReceiveEntry::deserialize(&bytes)
            {
                let _ = std::fs::remove_file(&entry.save_path);
            }

            db_lock.raw_delete(TABLE_FILE_TRANSFERS, &key)?;
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct FileTransferEntry {
    pub path: String,
    pub my_random: [u8; 32],
    pub shared_at: u64,
    pub last_accessed_at: Option<u64>,
    pub size_bytes: u64,
}
impl FileTransferEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileIncomingEntry {
    pub their_random: [u8; 32],
    pub file_size: u64,
    pub hash: [u8; 32],
    pub created_at: u64,
}
impl FileIncomingEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileReceiveEntry {
    pub key: [u8; 32],
    pub my_random: [u8; 32],
    pub file_size: u64,
    pub save_path: String,
    pub received_chunks: Vec<u8>,
    pub expected_hash: [u8; 32],
    pub created_at: u64,
}
impl FileReceiveEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

static CANCELLED_TRANSFERS: LazyLock<StdMutex<HashSet<[u8; 16]>>> =
    LazyLock::new(|| StdMutex::new(HashSet::new()));

pub fn mark_transfer_cancelled(offer_id: [u8; 16]) {
    CANCELLED_TRANSFERS.lock().unwrap().insert(offer_id);
}

pub fn take_transfer_cancelled(offer_id: &[u8; 16]) -> bool {
    CANCELLED_TRANSFERS.lock().unwrap().remove(offer_id)
}

fn is_transfer_cancelled(offer_id: &[u8; 16]) -> bool {
    CANCELLED_TRANSFERS.lock().unwrap().contains(offer_id)
}

pub async fn apply_cancel(
    db: SharedDatabase,
    contact_id: &UserId,
    offer_id: [u8; 16],
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    mark_transfer_cancelled(offer_id);

    let contact_hex = hex::encode(contact_id.0);
    let offer_hex = hex::encode(offer_id);
    let prog_key = format!("recvprog:{contact_hex}:{offer_hex}");
    let recv_key = format!("recv:{contact_hex}:{offer_hex}");
    let send_key = format!("send:{contact_hex}:{offer_hex}");

    {
        let db_lock = db.0.lock().await;
        if let Ok(Some(bytes)) = db_lock.raw_read(TABLE_FILE_TRANSFERS, &prog_key)
            && let Ok(entry) = FileReceiveEntry::deserialize(&bytes)
        {
            let _ = std::fs::remove_file(&entry.save_path);
        }
        let _ = db_lock.raw_delete(TABLE_FILE_TRANSFERS, &prog_key);
        let _ = db_lock.raw_delete(TABLE_FILE_TRANSFERS, &recv_key);
        let _ = db_lock.raw_delete(TABLE_FILE_TRANSFERS, &send_key);
    }

    event_tx
        .send(AppEvent::FileTransferFailed {
            contact_id: contact_id.clone(),
            transfer_id: MessageId(offer_id),
            reason: "cancelled".to_string(),
        })
        .await
        .ok();

    Ok(())
}

pub fn chunk_received(received_chunks: &[u8], index: usize) -> bool {
    received_chunks
        .get(index / 8)
        .map(|byte| byte & (1 << (index % 8)) != 0)
        .unwrap_or(false)
}

#[allow(clippy::cast_possible_truncation)]
pub fn all_chunks_received(received_chunks: &[u8], file_size: u64) -> bool {
    let chunk_count = file_size.div_ceil(FILE_CHUNK_SIZE as u64) as usize;
    if chunk_count == 0 {
        return true;
    }
    if received_chunks.len() < chunk_count.div_ceil(8) {
        return false;
    }
    received_chunks.iter().enumerate().all(|(i, byte)| {
        let bits_in_byte = (chunk_count - i * 8).min(8);
        let mask = if bits_in_byte >= 8 {
            0xFFu8
        } else {
            (1u8 << bits_in_byte) - 1
        };
        byte & mask == mask
    })
}

pub async fn finalize_transfer(
    db: SharedDatabase,
    contact_id: &UserId,
    offer_id: [u8; 16],
    entry: &FileReceiveEntry,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let contact_hex = hex::encode(contact_id.0);
    let offer_hex = hex::encode(offer_id);
    let prog_key = format!("recvprog:{contact_hex}:{offer_hex}");
    let recv_key = format!("recv:{contact_hex}:{offer_hex}");

    let hash_path = entry.save_path.clone();
    let actual_hash = tokio::task::spawn_blocking(move || hash_file(&hash_path))
        .await
        .ok_kursal(KursalError::Storage)??;

    {
        let db_lock = db.0.lock().await;
        db_lock.raw_delete(TABLE_FILE_TRANSFERS, &prog_key)?;
        db_lock.raw_delete(TABLE_FILE_TRANSFERS, &recv_key)?;
    }

    if actual_hash == entry.expected_hash {
        log::info!("[file] transfer {offer_hex} complete, hash ok");
        event_tx
            .send(AppEvent::FileReceived {
                contact_id: contact_id.clone(),
                transfer_id: MessageId(offer_id),
                save_path: entry.save_path.clone(),
            })
            .await
            .ok();
    } else {
        log::warn!("[file] hash mismatch for {offer_hex}, discarding");
        let _ = std::fs::remove_file(&entry.save_path);
        event_tx
            .send(AppEvent::FileTransferFailed {
                contact_id: contact_id.clone(),
                transfer_id: MessageId(offer_id),
                reason: "hash_mismatch".to_string(),
            })
            .await
            .ok();
    }

    Ok(())
}

pub async fn file_chunk_loop(
    mut chunk_rx: mpsc::Receiver<(PeerId, FileTransferMessage)>,
    db: SharedDatabase,
    event_tx: mpsc::Sender<AppEvent>,
) {
    while let Some((from, chunk)) = chunk_rx.recv().await {
        if let Err(err) = handle_file_chunk(from, chunk, &db, &event_tx).await {
            log::warn!("[file] chunk handling failed: {err}");
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
async fn handle_file_chunk(
    from: PeerId,
    chunk: FileTransferMessage,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    if is_transfer_cancelled(&chunk.transfer_id) {
        return Ok(());
    }

    let peer_id_str = from.to_base58();
    let known = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str)?;
    let Some(contact) = known else {
        return Ok(());
    };

    if contact.blocked {
        return Ok(());
    }

    let contact_hex = hex::encode(contact.user_id.0);
    let offer_hex = hex::encode(chunk.transfer_id);
    let prog_key = format!("recvprog:{contact_hex}:{offer_hex}");

    let entry_bytes = match db
        .0
        .lock()
        .await
        .raw_read(TABLE_FILE_TRANSFERS, &prog_key)?
    {
        Some(bytes) => bytes,
        None => return Ok(()),
    };
    let mut entry = FileReceiveEntry::deserialize(&entry_bytes)?;

    let idx = chunk.index as usize;
    let chunk_count = entry.file_size.div_ceil(FILE_CHUNK_SIZE as u64) as usize;
    if idx >= chunk_count {
        log::warn!("[file] dropping out-of-range chunk {idx} for {offer_hex}");
        return Ok(());
    }
    if chunk_received(&entry.received_chunks, idx) {
        return Ok(());
    }

    let mut chunk_aad = [0u8; 20];
    chunk_aad[..16].copy_from_slice(&chunk.transfer_id);
    chunk_aad[16..].copy_from_slice(&chunk.index.to_be_bytes());
    let decrypted = stream_decrypt_aad(&entry.key, &chunk.data, &chunk_aad)?;

    let offset = chunk.index as u64 * FILE_CHUNK_SIZE as u64;
    if offset + decrypted.len() as u64 > entry.file_size {
        log::warn!("[file] dropping chunk {idx} exceeding file size for {offer_hex}");
        return Ok(());
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&entry.save_path)
        .await
        .map_err(KursalError::Io)?;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(KursalError::Io)?;
    file.write_all(&decrypted).await.map_err(KursalError::Io)?;

    entry.received_chunks[idx / 8] |= 1 << (idx % 8);

    let all_received = all_chunks_received(&entry.received_chunks, entry.file_size);
    let chunks_received = entry
        .received_chunks
        .iter()
        .map(|byte| byte.count_ones() as u64)
        .sum::<u64>();

    db.0.lock()
        .await
        .raw_write(TABLE_FILE_TRANSFERS, &prog_key, &entry.serialize()?)?;

    if chunks_received == 1 || all_received || chunks_received % FILE_PROGRESS_THROTTLE == 0 {
        log::info!("[file] recv progress transfer={offer_hex} {chunks_received}/{chunk_count}");
    }

    if all_received || chunks_received % FILE_PROGRESS_THROTTLE == 0 {
        event_tx
            .send(AppEvent::FileTransferProgress {
                transfer_id: MessageId(chunk.transfer_id),
                bytes_transferred: (chunks_received * FILE_CHUNK_SIZE as u64).min(entry.file_size),
                total_bytes: entry.file_size,
            })
            .await
            .ok();
    }

    if all_received {
        finalize_transfer(
            db.clone(),
            &contact.user_id,
            chunk.transfer_id,
            &entry,
            event_tx,
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
pub async fn send_file_chunks(
    contact: Contact,
    offer_id: MessageId,
    file_path: String,
    my_random: [u8; 32],
    their_random: [u8; 32],
    received_chunks: Vec<u8>,
    cmd_tx: mpsc::Sender<SwarmCommand>,
    event_tx: mpsc::Sender<AppEvent>,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::OpenStream {
            peer_id: PeerId::from_str(&contact.peer_id).ok_kursal(KursalError::Network)?,
            addresses: str_to_multiaddr(&contact.known_addresses)?,
            reply: reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    let stream_tx = reply_rx
        .await
        .ok_kursal(KursalError::Network)?
        .ok_or_else(|| KursalError::Network("Could not open stream to peer".to_string()))?;

    let mut file: File = File::open(&file_path).await.map_err(KursalError::Io)?;
    let file_size = file.metadata().await.map_err(KursalError::Io)?.len();
    let chunk_count = file_size.div_ceil(FILE_CHUNK_SIZE as u64);

    log::info!(
        "[file] send start transfer={} chunks={chunk_count} bytes={file_size}",
        hex::encode(offer_id.0)
    );

    let key = derive_stream_key(my_random, their_random)?;
    let mut buffer = vec![0u8; FILE_CHUNK_SIZE];

    let already_sent = received_chunks
        .iter()
        .map(|byte| byte.count_ones() as u64)
        .sum::<u64>();
    let _ = event_tx
        .send(AppEvent::FileTransferProgress {
            transfer_id: offer_id,
            bytes_transferred: (already_sent * FILE_CHUNK_SIZE as u64).min(file_size),
            total_bytes: file_size,
        })
        .await;

    let mut done: u64 = 0;
    for index in 0..chunk_count {
        if is_transfer_cancelled(&offer_id.0) {
            take_transfer_cancelled(&offer_id.0);
            log::info!(
                "[file] transfer {} cancelled mid-send",
                hex::encode(offer_id.0)
            );
            return Ok(());
        }

        if chunk_received(&received_chunks, index as usize) {
            done += 1;
            continue;
        }

        let offset = index * FILE_CHUNK_SIZE as u64;
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(KursalError::Io)?;

        let bytes_read = file.read(&mut buffer).await.map_err(KursalError::Io)?;
        if bytes_read == 0 {
            break;
        }

        let mut chunk_aad = [0u8; 20];
        chunk_aad[..16].copy_from_slice(&offer_id.0);
        chunk_aad[16..].copy_from_slice(&(index as u32).to_be_bytes());
        let content = stream_encrypt_aad(&key, &buffer[..bytes_read], &chunk_aad)?;

        let wire = WireMessage::FileTransfer(FileTransferMessage {
            transfer_id: offer_id.0,
            index: index as u32,
            data: content,
        });

        let data = bincode::serialize(&wire)?;

        if stream_tx.send(data).await.is_err() {
            log::warn!(
                "[file] stream send failed at chunk {index}/{chunk_count} for transfer {}, will resume on reconnect",
                hex::encode(offer_id.0)
            );
            return Ok(());
        }

        done += 1;
        if done.is_multiple_of(SEND_PROGRESS_THROTTLE) {
            log::info!(
                "[file] send progress transfer={} {done}/{chunk_count}",
                hex::encode(offer_id.0)
            );
            let _ = event_tx
                .send(AppEvent::FileTransferProgress {
                    transfer_id: offer_id,
                    bytes_transferred: (done * FILE_CHUNK_SIZE as u64).min(file_size),
                    total_bytes: file_size,
                })
                .await;
        }
    }

    log::info!(
        "[file] send done transfer={} {done}/{chunk_count} chunks",
        hex::encode(offer_id.0)
    );

    let _ = event_tx
        .send(AppEvent::FileTransferProgress {
            transfer_id: offer_id,
            bytes_transferred: file_size,
            total_bytes: file_size,
        })
        .await;

    Ok(())
}

pub async fn resume_incoming_transfers(
    contact: Contact,
    db: crate::storage::SharedDatabase,
    cmd_tx: mpsc::Sender<SwarmCommand>,
    event_tx: mpsc::Sender<crate::api::AppEvent>,
) -> Result<()> {
    let contact_hex = hex::encode(contact.user_id.0);
    let prefix = format!("recvprog:{contact_hex}:");
    let end = format!("recvprog:{contact_hex};");

    let entries =
        db.0.lock()
            .await
            .raw_range(crate::storage::TABLE_FILE_TRANSFERS, &prefix, &end, None)?;

    for (key, bytes) in entries {
        let Ok(entry) = FileReceiveEntry::deserialize(&bytes) else {
            continue;
        };

        let offer_hex = key.rsplit(':').next().unwrap_or_default();
        let Ok(offer_bytes) = hex::decode(offer_hex) else {
            continue;
        };
        let Ok(offer_arr): std::result::Result<[u8; 16], _> = offer_bytes.try_into() else {
            continue;
        };

        if all_chunks_received(&entry.received_chunks, entry.file_size) {
            finalize_transfer(db.clone(), &contact.user_id, offer_arr, &entry, &event_tx).await?;
            continue;
        }

        log::info!("[file] resuming incoming transfer {offer_hex} for contact {contact_hex}");

        crate::api::send_message(
            KursalMessage::FileAccept(FileAccept {
                offer_id: MessageId(offer_arr),
                random: entry.my_random,
                received_chunks: entry.received_chunks,
            }),
            &contact,
            db.clone(),
            &cmd_tx,
            Some(&event_tx),
        )
        .await?;
    }

    Ok(())
}
