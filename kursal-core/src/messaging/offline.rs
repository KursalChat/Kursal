use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{
        derive_key_salt,
        messages::message_send,
        offline_at, offline_ratchet_step, offline_tag, offline_wrapper,
        stream::{stream_decrypt, stream_encrypt},
    },
    first_contact::WireMessage,
    identity::{TransportIdentity, UserId},
    messaging::{
        StoredMessage,
        enums::{MessageId, MessageStatus},
    },
    network::{
        dht::DHTRecord,
        kademlia::KAD_LONG_MAX_AGE,
        swarm::{SwarmCommand, get_listen_addrs, is_peer_connected, str_to_multiaddr},
    },
    storage::{SharedDatabase, TABLE_PENDING_ACK, get_timestamp_secs},
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, IdentityKeyStore, ProtocolAddress};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        Arc, LazyLock, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex as TokioMutex, mpsc::Sender};
use zeroize::{Zeroize, Zeroizing};

const OFFLINE_VERSION: u8 = 1u8;

pub const MAX_OFFLINE_MESSAGE_BYTES: usize = 50 * 1024;
pub const OFFLINE_FLUSH_SIZE_BYTES: usize = 50 * 1024;
pub const OFFLINE_FLUSH_AGE_SECS: u64 = 15;
pub const OFFLINE_FLUSH_COUNT: usize = 15;
pub const MAX_SKIPPED_KEYS: usize = 64;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct OfflineState {
    pub send_chain: [u8; 32],
    pub recv_chain: [u8; 32],
    pub send_counter: u64,
    pub recv_counter: u64,
    pub send_queue: Vec<QueuedDr>,
    pub queue_first_at: Option<u64>,
    pub recv_stuck_since: Option<u64>,
    pub pending_bundles: Vec<PendingBundle>,
    pub skipped_keys: Vec<SkippedKey>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QueuedDr {
    pub message_id: Option<MessageId>,
    pub ciphertext: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PendingBundle {
    pub counter: u64,
    pub bytes: Vec<u8>,
    pub created_at: u64,
    pub tag: [u8; 32],
    pub message_ids: Vec<MessageId>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkippedKey {
    pub counter: u64,
    pub tag: [u8; 32],
    pub wrapper: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct BundleEnvelope {
    pub version: u8,
    pub ciphertext: Vec<u8>,
}
impl BundleEnvelope {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
pub struct BundleInner {
    pub created_at: u64,
    pub messages: Vec<Vec<u8>>,
    pub sender_peer_id: String,
    pub sender_addresses: Vec<String>,
}
impl BundleInner {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub fn derive_offline_roots(
    shared_secret: &[u8],
    pq_secret: &[u8],
    my_identity_pub: &[u8],
    peer_identity_pub: &[u8],
) -> Result<([u8; 32], [u8; 32])> {
    let i_am_low = my_identity_pub < peer_identity_pub;
    let (low, high) = if i_am_low {
        (my_identity_pub, peer_identity_pub)
    } else {
        (peer_identity_pub, my_identity_pub)
    };
    let salt = [low, high].concat();
    let ikm = [shared_secret, pq_secret].concat();

    let low_to_high = derive_key_salt(&ikm, &salt, b"kursal-offline-v1-low-to-high")?;
    let high_to_low = derive_key_salt(&ikm, &salt, b"kursal-offline-v1-high-to-low")?;

    if i_am_low {
        Ok((low_to_high, high_to_low))
    } else {
        Ok((high_to_low, low_to_high))
    }
}

pub fn recv_keys_at(state: &OfflineState, counter: u64) -> Result<Option<([u8; 32], [u8; 32])>> {
    if counter >= state.recv_counter {
        Ok(Some(offline_at(
            &state.recv_chain,
            counter - state.recv_counter,
        )?))
    } else {
        Ok(state
            .skipped_keys
            .iter()
            .find(|s| s.counter == counter)
            .map(|s| (s.tag, s.wrapper)))
    }
}

fn step_recv(state: &mut OfflineState, cache: bool) -> Result<()> {
    let (mk, next) = offline_ratchet_step(&state.recv_chain)?;
    let mk = Zeroizing::new(mk);
    if cache {
        state.skipped_keys.push(SkippedKey {
            counter: state.recv_counter,
            tag: offline_tag(&mk)?,
            wrapper: offline_wrapper(&mk)?,
        });
    }
    state.recv_chain.zeroize();
    state.recv_chain = next;
    state.recv_counter += 1;
    Ok(())
}

fn trim_skipped(state: &mut OfflineState) {
    let excess = state.skipped_keys.len().saturating_sub(MAX_SKIPPED_KEYS);
    if excess > 0 {
        state.skipped_keys.drain(..excess);
    }
}

pub fn advance_recv_past(state: &mut OfflineState, processed_counter: u64) -> Result<()> {
    while state.recv_counter <= processed_counter {
        let cache = state.recv_counter < processed_counter;
        step_recv(state, cache)?;
    }
    trim_skipped(state);
    Ok(())
}

pub fn skip_recv_current(state: &mut OfflineState) -> Result<()> {
    step_recv(state, true)?;
    trim_skipped(state);
    Ok(())
}

pub fn consume_skipped(state: &mut OfflineState, counter: u64) {
    state.skipped_keys.retain(|s| s.counter != counter);
}

pub fn encode_bundle(
    wrapper_key: &[u8; 32],
    messages: Vec<Vec<u8>>,
    sender_addresses: Vec<String>,
    sender_peer_id: String,
) -> Result<Vec<u8>> {
    let now = get_timestamp_secs()?;

    let inner = BundleInner {
        created_at: now,
        messages,
        sender_addresses,
        sender_peer_id,
    };
    let inner_bytes = inner.serialize()?;

    let ciphertext = stream_encrypt(wrapper_key, &inner_bytes)?;

    let envelope = BundleEnvelope {
        version: OFFLINE_VERSION,
        ciphertext,
    };

    envelope.serialize()
}

pub fn decode_bundle(tag: &[u8; 32], wrapper_key: &[u8; 32], raw: &[u8]) -> Result<BundleInner> {
    let inner_bytes = DHTRecord::deserialize(tag, raw)?;

    let env = BundleEnvelope::deserialize(&inner_bytes)?;
    if env.version != OFFLINE_VERSION {
        return Err(KursalError::Network("Invalid offline version".to_string()));
    }

    let inner = stream_decrypt(wrapper_key, &env.ciphertext)?;

    BundleInner::deserialize(&inner)
}

pub async fn new_offline_state(
    db: &SharedDatabase,
    peer_identity_pub: &[u8],
    classical_secret: &[u8],
    pq_secret: &[u8],
) -> Result<OfflineState> {
    let keypair = db
        .get_identity_key_pair()
        .await
        .ok_kursal(KursalError::Crypto)?;

    let my_identity_pub = keypair.public_key().serialize().to_vec();

    let (send_chain, recv_chain) = derive_offline_roots(
        classical_secret,
        pq_secret,
        &my_identity_pub,
        peer_identity_pub,
    )?;

    Ok(OfflineState {
        send_chain,
        recv_chain,
        ..Default::default()
    })
}

#[allow(clippy::type_complexity)]
static OFFLINE_LOCKS: LazyLock<StdMutex<HashMap<[u8; 32], Arc<TokioMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

static FLUSH_SCHEDULED: LazyLock<StdMutex<HashMap<[u8; 32], Arc<AtomicBool>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn offline_lock_for(user_id: &UserId) -> Arc<TokioMutex<()>> {
    let mut map = OFFLINE_LOCKS.lock().unwrap();
    map.entry(user_id.0)
        .or_insert_with(|| Arc::new(TokioMutex::new(())))
        .clone()
}

fn try_set_flush_scheduled(user_id: &UserId) -> bool {
    let flag = {
        let mut map = FLUSH_SCHEDULED.lock().unwrap();
        map.entry(user_id.0)
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    };
    !flag.swap(true, Ordering::SeqCst)
}

fn clear_flush_scheduled(user_id: &UserId) {
    let map = FLUSH_SCHEDULED.lock().unwrap();
    if let Some(flag) = map.get(&user_id.0) {
        flag.store(false, Ordering::SeqCst);
    }
}

pub async fn update_offline<T, F>(db: &SharedDatabase, user_id: &UserId, f: F) -> Result<Option<T>>
where
    F: FnOnce(&mut OfflineState) -> Result<T>,
{
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(None);
    };
    let out = f(&mut contact.offline)?;
    contact.save_if_exists(&*db.0.lock().await)?;
    Ok(Some(out))
}

pub async fn update_contact<F>(
    db: &SharedDatabase,
    user_id: &UserId,
    f: F,
) -> Result<Option<Contact>>
where
    F: FnOnce(&mut Contact) -> bool,
{
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(None);
    };
    if f(&mut contact) {
        contact.save_if_exists(&*db.0.lock().await)?;
        Ok(Some(contact))
    } else {
        Ok(None)
    }
}

pub async fn queue_for_offline(
    user_id: &UserId,
    message_id: Option<MessageId>,
    dr_ct: Vec<u8>,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    if dr_ct.len() > MAX_OFFLINE_MESSAGE_BYTES {
        return Err(KursalError::Storage(format!(
            "offline message too large: {} bytes (max {})",
            dr_ct.len(),
            MAX_OFFLINE_MESSAGE_BYTES
        )));
    }

    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let mut contact = Contact::load(&*db.0.lock().await, user_id)?
        .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;
    queue_for_offline_locked(&mut contact, message_id, dr_ct, cmd_tx, db, event_tx).await
}

pub async fn discard_queued(
    db: &SharedDatabase,
    user_id: &UserId,
    message_id: &MessageId,
) -> Result<bool> {
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;

    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(false);
    };

    let before = contact.offline.send_queue.len();
    contact
        .offline
        .send_queue
        .retain(|q| q.message_id.as_ref() != Some(message_id));
    let dropped = before != contact.offline.send_queue.len();

    if contact.offline.send_queue.is_empty() {
        contact.offline.queue_first_at = None;
    }

    if dropped {
        contact.save_if_exists(&*db.0.lock().await)?;
    }

    Ok(dropped)
}

pub async fn maybe_flush(
    user_id: &UserId,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(());
    };
    maybe_flush_locked(&mut contact, cmd_tx, db, event_tx).await
}

pub async fn republish_pending(
    user_id: &UserId,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
) -> Result<()> {
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let Some(contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(());
    };

    if contact.offline.pending_bundles.is_empty() {
        return Ok(());
    }

    let contact_dbg = hex::encode(contact.user_id.0);
    log::info!(
        "[offline] republishing {} pending bundle(s) for contact={contact_dbg}",
        contact.offline.pending_bundles.len()
    );

    for pending in &contact.offline.pending_bundles {
        if let Err(err) = cmd_tx
            .send(SwarmCommand::PublishDht {
                key: pending.tag.to_vec(),
                value: pending.bytes.clone(),
                expires: Some(KAD_LONG_MAX_AGE),
                reply_tx: None,
            })
            .await
        {
            log::warn!(
                "[offline] republish PublishDht failed counter={} err={err}",
                pending.counter
            );
        }
    }

    Ok(())
}

pub async fn flush_now(
    user_id: &UserId,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;
    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(());
    };
    flush_now_locked(&mut contact, cmd_tx, db, event_tx).await
}

pub async fn deliver_queue_direct(
    user_id: &UserId,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<usize> {
    let lock = offline_lock_for(user_id);
    let _guard = lock.lock().await;

    let Some(mut contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        return Ok(0);
    };
    if contact.offline.send_queue.is_empty() {
        return Ok(0);
    }

    if !contact.offline.pending_bundles.is_empty() {
        log::debug!(
            "[offline] direct drain held for contact={} bundles={} queue={}",
            hex::encode(contact.user_id.0),
            contact.offline.pending_bundles.len(),
            contact.offline.send_queue.len()
        );
        return Ok(0);
    }

    let Ok(peer_id) = PeerId::from_str(&contact.peer_id) else {
        return Ok(0);
    };
    if !is_peer_connected(cmd_tx, peer_id).await {
        return Ok(0);
    }

    let contact_dbg = hex::encode(contact.user_id.0);
    let addresses = str_to_multiaddr(&contact.known_addresses)?;
    let queued = contact.offline.send_queue.len();

    let mut sent = 0usize;
    for entry in &contact.offline.send_queue {
        let wire = WireMessage::Encrypted(entry.ciphertext.clone());
        if let Err(err) = cmd_tx
            .send(SwarmCommand::SendMessage {
                peer_id,
                data: bincode::serialize(&wire)?,
                addresses: addresses.clone(),
            })
            .await
        {
            log::warn!("[offline] direct drain contact={contact_dbg} send failed: {err}");
            break;
        }
        sent += 1;
    }

    if sent == 0 {
        return Ok(0);
    }

    contact.offline.send_queue.drain(..sent);
    let drained = contact.offline.send_queue.is_empty();
    if drained {
        contact.offline.queue_first_at = None;
    }
    contact.save_if_exists(&*db.0.lock().await)?;

    log::info!(
        "[offline] direct drain contact={contact_dbg} delivered={sent}/{queued} peer={peer_id}"
    );

    if drained && let Some(tx) = event_tx {
        tx.send(AppEvent::OfflineQueueDrained {
            contact_id: contact.user_id.clone(),
            finalized_deletes: take_finalized_deletes(&db, &contact.user_id).await,
        })
        .await
        .ok();
    }

    Ok(sent)
}

async fn take_finalized_deletes(db: &SharedDatabase, user_id: &UserId) -> Vec<MessageId> {
    let contact_hex = hex::encode(user_id.0);
    let cleared =
        crate::storage::conversation::clear_pending_sync_for(&*db.0.lock().await, &contact_hex)
            .unwrap_or_default();

    cleared
        .iter()
        .filter_map(|id| hex::decode(id).ok())
        .filter_map(|bytes| <[u8; 16]>::try_from(bytes.as_slice()).ok())
        .map(MessageId)
        .collect()
}

pub fn drop_acked_bundles(state: &mut OfflineState, message_id: &MessageId) -> bool {
    let mut touched = false;
    state.pending_bundles.retain_mut(|b| {
        if !b.message_ids.iter().any(|id| id == message_id) {
            return true;
        }
        touched = true;
        b.message_ids.retain(|id| id != message_id);
        !b.message_ids.is_empty()
    });
    touched
}

pub async fn ack_delivered(
    user_id: &UserId,
    message_id: &MessageId,
    db: &SharedDatabase,
) -> Result<bool> {
    let msg_id = *message_id;
    Ok(
        update_offline(db, user_id, move |o| Ok(drop_acked_bundles(o, &msg_id)))
            .await?
            .unwrap_or(false),
    )
}

pub const DIRECT_ACK_DEADLINE_SECS: u64 = 20;

fn pending_ack_key(user: &UserId, id: &MessageId) -> String {
    format!("{}:{}", hex::encode(user.0), hex::encode(id.0))
}

pub async fn write_pending_ack(
    db: &SharedDatabase,
    user: &UserId,
    id: &MessageId,
    sent_at: u64,
) -> Result<()> {
    db.0.lock().await.raw_write(
        TABLE_PENDING_ACK,
        &pending_ack_key(user, id),
        &sent_at.to_be_bytes(),
    )?;

    Ok(())
}

pub async fn clear_pending_ack(db: &SharedDatabase, user: &UserId, id: &MessageId) -> Result<()> {
    db.0.lock()
        .await
        .raw_delete(TABLE_PENDING_ACK, &pending_ack_key(user, id))
}

pub async fn list_pending_ack(db: &SharedDatabase) -> Result<Vec<(UserId, MessageId, u64)>> {
    let rows = db.0.lock().await.raw_readall(TABLE_PENDING_ACK)?;
    let mut out = Vec::with_capacity(rows.len());
    for (key, val) in rows {
        let mut parts = key.split(':');
        let (Some(uhex), Some(mhex)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(ub), Ok(mb)) = (hex::decode(uhex), hex::decode(mhex)) else {
            continue;
        };
        let (Ok(uarr), Ok(marr)) = (
            <[u8; 32]>::try_from(ub.as_slice()),
            <[u8; 16]>::try_from(mb.as_slice()),
        ) else {
            continue;
        };
        let ts = <[u8; 8]>::try_from(val.as_slice())
            .map(u64::from_be_bytes)
            .unwrap_or(0);
        out.push((UserId(uarr), MessageId(marr), ts));
    }
    Ok(out)
}

pub async fn move_to_mailbox_if_stuck(
    user_id: &UserId,
    message_id: &MessageId,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<bool> {
    let Some(contact) = Contact::load(&*db.0.lock().await, user_id)? else {
        clear_pending_ack(&db, user_id, message_id).await?;
        return Ok(false);
    };

    let in_offline = contact
        .offline
        .send_queue
        .iter()
        .any(|q| q.message_id.as_ref() == Some(message_id))
        || contact
            .offline
            .pending_bundles
            .iter()
            .any(|b| b.message_ids.contains(message_id));

    let Some(message) = StoredMessage::load(&*db.0.lock().await, user_id, message_id)? else {
        clear_pending_ack(&db, user_id, message_id).await?;
        return Ok(false);
    };

    if in_offline || !matches!(message.status, MessageStatus::Sending) {
        clear_pending_ack(&db, user_id, message_id).await?;
        return Ok(false);
    }

    let serialized = message.payload.serialize()?;
    let address = ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());
    let ciphertext = message_send(db.clone(), &address, &serialized).await?;

    queue_for_offline(
        user_id,
        Some(*message_id),
        ciphertext,
        cmd_tx,
        db.clone(),
        event_tx,
    )
    .await?;
    clear_pending_ack(&db, user_id, message_id).await?;

    if let Some(tx) = event_tx {
        tx.send(AppEvent::MessageQueuedOffline {
            contact_id: user_id.clone(),
            message_id: *message_id,
        })
        .await
        .ok();
    }
    Ok(true)
}

pub async fn schedule_direct_ack_deadline(
    user_id: UserId,
    message_id: MessageId,
    sent_at: u64,
    cmd_tx: Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<Sender<AppEvent>>,
) {
    if write_pending_ack(&db, &user_id, &message_id, sent_at)
        .await
        .is_err()
    {
        return;
    }
    tokio::task::spawn_local(async move {
        tokio::time::sleep(Duration::from_secs(DIRECT_ACK_DEADLINE_SECS)).await;
        if let Err(err) = move_to_mailbox_if_stuck(
            &user_id,
            &message_id,
            &cmd_tx,
            db.clone(),
            event_tx.as_ref(),
        )
        .await
        {
            log::warn!("[offline] deadline fallback failed: {err}");
        }
    });
}

pub async fn expire_and_fail(
    user_id: &UserId,
    db: &SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let now = get_timestamp_secs()?;
    let expired_ids = update_offline(db, user_id, move |o| {
        let mut ids: Vec<MessageId> = Vec::new();
        o.pending_bundles.retain(|b| {
            if now.saturating_sub(b.created_at) >= KAD_LONG_MAX_AGE {
                ids.extend(b.message_ids.iter().copied());
                false
            } else {
                true
            }
        });
        Ok(ids)
    })
    .await?
    .unwrap_or_default();

    if expired_ids.is_empty() {
        return Ok(());
    }

    let mut failed = Vec::new();
    {
        let guard = db.0.lock().await;
        for id in &expired_ids {
            if StoredMessage::set_failed(&guard, user_id, id)? {
                failed.push(*id);
            }
        }
    }
    for id in &expired_ids {
        clear_pending_ack(db, user_id, id).await?;
    }
    if !failed.is_empty()
        && let Some(tx) = event_tx
    {
        tx.send(AppEvent::MessageFailed {
            contact_id: user_id.clone(),
            message_ids: failed,
        })
        .await
        .ok();
    }
    Ok(())
}

async fn queue_for_offline_locked(
    contact: &mut Contact,
    message_id: Option<MessageId>,
    dr_ct: Vec<u8>,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let was_empty = contact.offline.send_queue.is_empty();
    if was_empty {
        contact.offline.queue_first_at = Some(get_timestamp_secs()?);
    }
    contact.offline.send_queue.push(QueuedDr {
        message_id,
        ciphertext: dr_ct,
    });
    log::info!(
        "[offline] queued msg_id={:?} contact={} queue_len={}",
        message_id.map(|m| hex::encode(m.0)),
        hex::encode(contact.user_id.0),
        contact.offline.send_queue.len()
    );

    contact.save_if_exists(&*db.0.lock().await)?;

    maybe_flush_locked(contact, cmd_tx, db.clone(), event_tx).await?;

    contact.save_if_exists(&*db.0.lock().await)?;

    if was_empty && !contact.offline.send_queue.is_empty() {
        log::info!(
            "[offline] scheduling delayed flush in {}s for contact={}",
            OFFLINE_FLUSH_AGE_SECS + 1,
            hex::encode(contact.user_id.0)
        );
        schedule_delayed_flush(
            contact.user_id.clone(),
            cmd_tx.clone(),
            db.clone(),
            event_tx.cloned(),
        );
    }

    Ok(())
}

fn schedule_delayed_flush(
    user_id: UserId,
    cmd_tx: Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<Sender<AppEvent>>,
) {
    if !try_set_flush_scheduled(&user_id) {
        log::debug!(
            "[offline] delayed flush already scheduled for contact={}",
            hex::encode(user_id.0)
        );
        return;
    }
    tokio::task::spawn_local(async move {
        tokio::time::sleep(Duration::from_secs(OFFLINE_FLUSH_AGE_SECS + 1)).await;
        clear_flush_scheduled(&user_id);
        log::info!(
            "[offline] delayed flush firing for contact={}",
            hex::encode(user_id.0)
        );
        if let Err(err) = flush_now(&user_id, &cmd_tx, db.clone(), event_tx.as_ref()).await {
            log::warn!("[offline] delayed flush failed: {err}");
        }
    });
}

async fn maybe_flush_locked(
    contact: &mut Contact,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let now = get_timestamp_secs()?;
    let q = &contact.offline.send_queue;
    let age_secs = contact
        .offline
        .queue_first_at
        .map(|t| now.saturating_sub(t))
        .unwrap_or(0);
    let total_size: usize = q.iter().map(|m| m.ciphertext.len()).sum();

    let should_flush = q.len() >= OFFLINE_FLUSH_COUNT
        || age_secs >= OFFLINE_FLUSH_AGE_SECS
        || total_size >= OFFLINE_FLUSH_SIZE_BYTES;

    log::debug!(
        "[offline] maybe_flush contact={} queue_len={} age_s={age_secs} size={total_size} -> flush={should_flush}",
        hex::encode(contact.user_id.0),
        q.len(),
    );

    if should_flush {
        flush_now_locked(contact, cmd_tx, db, event_tx).await?;
    }

    Ok(())
}

async fn flush_now_locked(
    contact: &mut Contact,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let now = get_timestamp_secs()?;
    let contact_dbg = hex::encode(contact.user_id.0);

    if contact.offline.send_queue.is_empty() && contact.offline.pending_bundles.is_empty() {
        log::debug!("[offline] flush contact={contact_dbg} nothing to flush");
        contact.offline.queue_first_at = None;
        contact.save_if_exists(&*db.0.lock().await)?;
        return Ok(());
    }

    let mut new_bundle_message_ids: Vec<MessageId> = Vec::new();
    let mut new_bundle: Option<(u64, [u8; 32], Vec<u8>)> = None;
    if !contact.offline.send_queue.is_empty() {
        let counter_to_use = contact.offline.send_counter;
        let queue_len = contact.offline.send_queue.len();
        let messages: Vec<Vec<u8>> = contact
            .offline
            .send_queue
            .iter()
            .map(|q| q.ciphertext.clone())
            .collect();
        new_bundle_message_ids = contact
            .offline
            .send_queue
            .iter()
            .filter_map(|q| q.message_id)
            .collect();
        let total_msg_bytes: usize = messages.iter().map(|m| m.len()).sum();

        log::info!(
            "[offline] flush contact={contact_dbg} bundling counter={counter_to_use} msgs={queue_len} bytes={total_msg_bytes}"
        );

        let sender_addresses = get_listen_addrs(cmd_tx).await?;
        log::info!(
            "[offline] flush contact={contact_dbg} sender_addrs={}",
            sender_addresses.len()
        );

        let sender_peer_id = TransportIdentity::load(&*db.0.lock().await)?
            .ok_or_else(|| KursalError::Identity("No transport identity".to_string()))?
            .peer_id
            .to_base58();

        let (mk, next_chain) = offline_ratchet_step(&contact.offline.send_chain)?;
        let mk = Zeroizing::new(mk);
        let tag = offline_tag(&mk)?;
        let wrapper_key = Zeroizing::new(offline_wrapper(&mk)?);

        let envelope_bytes =
            encode_bundle(&wrapper_key, messages, sender_addresses, sender_peer_id)?;

        let pow_start = std::time::Instant::now();
        log::info!(
            "[offline] flush contact={contact_dbg} mining POW for counter={counter_to_use}..."
        );
        let dht_record = DHTRecord::new(tag.to_vec(), envelope_bytes, now, true).await?;
        log::info!(
            "[offline] flush contact={contact_dbg} POW done counter={counter_to_use} took={:?}",
            pow_start.elapsed()
        );
        let serialized_record = dht_record.serialize()?;
        new_bundle = Some((counter_to_use, tag, serialized_record.clone()));

        contact.offline.send_queue.clear();
        contact.offline.queue_first_at = None;
        contact.offline.send_chain.zeroize();
        contact.offline.send_chain = next_chain;
        contact.offline.send_counter = counter_to_use + 1;
        contact.offline.pending_bundles.push(PendingBundle {
            counter: counter_to_use,
            bytes: serialized_record,
            created_at: now,
            tag,
            message_ids: new_bundle_message_ids.clone(),
        });
        contact.save_if_exists(&*db.0.lock().await)?;
    }

    let published = new_bundle.is_some();
    if let Some((counter, tag, bytes)) = new_bundle {
        log::info!(
            "[offline] flush contact={contact_dbg} -> PublishDht counter={} key={} value_bytes={}",
            counter,
            hex::encode(&tag[..8]),
            bytes.len()
        );
        if let Err(err) = cmd_tx
            .send(SwarmCommand::PublishDht {
                key: tag.to_vec(),
                value: bytes,
                expires: Some(KAD_LONG_MAX_AGE),
                reply_tx: None,
            })
            .await
        {
            log::warn!("[offline] PublishDht channel send failed counter={counter} err={err}");
        }
    }

    if !new_bundle_message_ids.is_empty()
        && let Some(tx) = event_tx
    {
        let _ = tx
            .send(AppEvent::OfflineBundlePublished {
                contact_id: contact.user_id.clone(),
                message_ids: new_bundle_message_ids,
            })
            .await;
    }

    if published && let Some(tx) = event_tx {
        let _ = tx
            .send(AppEvent::OfflineQueueDrained {
                contact_id: contact.user_id.clone(),
                finalized_deletes: take_finalized_deletes(&db, &contact.user_id).await,
            })
            .await;
    }

    Ok(())
}
