use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::{
        AppEvent,
        file_transfers::FileIncomingEntry,
        file_transfers::apply_cancel,
        message_apply::{
            apply_delete, apply_edit, apply_pin, apply_reaction_add, apply_reaction_remove,
        },
        send_message,
    },
    contacts::Contact,
    crypto::{DEVICE_ID, messages::message_receive, offline_ratchet_step, offline_tag},
    identity::UserId,
    messaging::{
        StoredMessage,
        enums::{DeliveryReceipt, Direction, KursalMessage, MessageId, MessageStatus},
        offline::{
            BundleInner, advance_recv_past, consume_skipped, decode_bundle, drop_acked_bundles,
            recv_keys_at, skip_recv_current, update_contact, update_offline,
        },
    },
    network::swarm::{SwarmCommand, is_routable_multiaddr},
    storage::{
        SharedDatabase, TABLE_FILE_TRANSFERS, filetransfer::sanitize_filename, get_timestamp_secs,
    },
    sync::LockExt,
};
use libp2p::Multiaddr;
use libsignal_protocol::ProtocolAddress;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{LazyLock, Mutex as StdMutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{self, Sender};
use zeroize::Zeroizing;

pub const POLL_WINDOW: u64 = 6;
pub const POLL_TIMEOUT_SECS: u64 = 30;
pub const GAP_SKIP_SECS: u64 = 48 * 3600;
pub const POLL_COOLDOWN_SECS: u64 = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PollTrigger {
    Periodic,
    Event,
}

async fn fetch_window(
    contact: &Contact,
    scan_start: u64,
    cmd_tx: &Sender<SwarmCommand>,
) -> Result<BTreeMap<u64, Vec<u8>>> {
    let base = contact.offline.recv_counter;
    #[allow(clippy::cast_possible_truncation)]
    let mut targets: Vec<(u64, [u8; 32])> =
        Vec::with_capacity(POLL_WINDOW as usize + contact.offline.skipped_keys.len());

    let mut chain = Zeroizing::new(contact.offline.recv_chain);
    for _ in 0..scan_start.saturating_sub(base) {
        let (_, next) = offline_ratchet_step(&chain)?;
        *chain = next;
    }
    for offset in 0..POLL_WINDOW {
        let (mk, next) = offline_ratchet_step(&chain)?;
        let mk = Zeroizing::new(mk);
        targets.push((scan_start + offset, offline_tag(&mk)?));
        *chain = next;
    }

    if scan_start == base {
        for skipped in &contact.offline.skipped_keys {
            targets.push((skipped.counter, skipped.tag));
        }
    }

    let mut channels = Vec::with_capacity(targets.len());
    for (counter, tag) in targets {
        let (tx, rx) = mpsc::channel(4);
        cmd_tx
            .send(SwarmCommand::FetchDht {
                key: tag.to_vec(),
                reply_tx: tx,
            })
            .await
            .ok_kursal(KursalError::Network)?;
        channels.push((counter, rx));
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(POLL_TIMEOUT_SECS);
    let mut hits: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
    for (counter, mut rx) in channels {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if let Ok(Some(bytes)) = tokio::time::timeout(remaining, rx.recv()).await {
            hits.insert(counter, bytes);
        }
    }
    Ok(hits)
}

static POLLS_ACTIVE: LazyLock<StdMutex<HashSet<[u8; 32]>>> =
    LazyLock::new(|| StdMutex::new(HashSet::new()));

static LAST_POLL: LazyLock<StdMutex<HashMap<[u8; 32], Instant>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

pub async fn poll_contact_offline(
    contact_id: UserId,
    cmd_tx: Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: Sender<AppEvent>,
    trigger: PollTrigger,
) -> Result<()> {
    if trigger == PollTrigger::Event
        && LAST_POLL
            .lock_recover()
            .get(&contact_id.0)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(POLL_COOLDOWN_SECS))
    {
        log::debug!("[offline] poll throttled for {}", hex::encode(contact_id.0));
        return Ok(());
    }

    if !POLLS_ACTIVE.lock_recover().insert(contact_id.0) {
        log::debug!(
            "[offline] poll already running for {}",
            hex::encode(contact_id.0)
        );
        return Ok(());
    }
    let result = poll_contact_offline_inner(&contact_id, &cmd_tx, &db, &event_tx).await;
    LAST_POLL
        .lock_recover()
        .insert(contact_id.0, Instant::now());
    POLLS_ACTIVE.lock_recover().remove(&contact_id.0);
    result
}

async fn poll_contact_offline_inner(
    contact_id: &UserId,
    cmd_tx: &Sender<SwarmCommand>,
    db: &SharedDatabase,
    event_tx: &Sender<AppEvent>,
) -> Result<()> {
    log::info!(
        "[offline] fetching offline for {:?}",
        hex::encode(contact_id.0)
    );

    loop {
        let mut contact = Contact::load(&*db.0.lock().await, contact_id)?
            .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

        if let Some(since) = contact.offline.recv_stuck_since
            && get_timestamp_secs()?.saturating_sub(since) > GAP_SKIP_SECS
        {
            log::warn!(
                "[offline] skipping gap at counter={} for contact={:?}",
                contact.offline.recv_counter,
                contact.user_id
            );
            update_offline(db, contact_id, |o| {
                skip_recv_current(o)?;
                o.recv_stuck_since = None;
                Ok(())
            })
            .await?;
            contact = Contact::load(&*db.0.lock().await, contact_id)?
                .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;
            event_tx
                .send(AppEvent::OfflineGapSkipped {
                    contact_id: contact.user_id.clone(),
                    counter: contact.offline.recv_counter.saturating_sub(1),
                })
                .await
                .ok();
        }

        let mut advanced = false;
        let mut highest_hit: Option<u64> = None;
        let mut scan_start = contact.offline.recv_counter;

        loop {
            let hits = fetch_window(&contact, scan_start, cmd_tx).await?;
            if hits.is_empty() {
                break;
            }
            let Some(max_hit) = hits.keys().max().copied() else {
                break;
            };
            highest_hit = Some(highest_hit.map_or(max_hit, |h| h.max(max_hit)));

            for (counter, bytes) in &hits {
                let Some((tag, wrapper)) = recv_keys_at(&contact.offline, *counter)? else {
                    continue;
                };
                match decode_bundle(&tag, &wrapper, bytes) {
                    Ok(inner) => {
                        if process_bundle(&mut contact, inner, cmd_tx, db.clone(), event_tx).await?
                        {
                            let c = *counter;
                            update_offline(db, contact_id, move |o| {
                                if c >= o.recv_counter {
                                    advance_recv_past(o, c)?;
                                } else {
                                    consume_skipped(o, c);
                                }
                                o.recv_stuck_since = None;
                                Ok(())
                            })
                            .await?;
                            if *counter >= contact.offline.recv_counter {
                                advance_recv_past(&mut contact.offline, *counter)?;
                            } else {
                                consume_skipped(&mut contact.offline, *counter);
                            }
                            contact.offline.recv_stuck_since = None;
                            advanced = true;
                        }
                    }
                    Err(e) => log::warn!("[offline] decode failed counter={counter}: {e}"),
                }
            }

            scan_start = (scan_start + POLL_WINDOW).max(max_hit + 1);
        }

        if !advanced && highest_hit.is_some_and(|h| h >= contact.offline.recv_counter) {
            update_offline(db, contact_id, |o| {
                if o.recv_stuck_since.is_none() {
                    o.recv_stuck_since = Some(get_timestamp_secs()?);
                }
                Ok(())
            })
            .await?;
        }

        if !advanced {
            return Ok(());
        }
    }
}

async fn process_bundle(
    contact: &mut Contact,
    inner: BundleInner,
    cmd_tx: &Sender<SwarmCommand>,
    db: SharedDatabase,
    event_tx: &Sender<AppEvent>,
) -> Result<bool> {
    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DEVICE_ID);
    let mut any_decrypted = false;

    for dr_ct in &inner.messages {
        let plaintext = match message_receive(db.clone(), &address, dr_ct).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        any_decrypted = true;

        let kmessage = match KursalMessage::deserialize(&plaintext) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if let Err(err) =
            dispatch_offline_kmessage(contact, kmessage, db.clone(), cmd_tx, event_tx).await
        {
            log::warn!("[offline] dispatch failed: {err}");
        }
    }

    if !any_decrypted {
        return Ok(false);
    }

    let sender_peer_id = inner.sender_peer_id.clone();
    let sender_addresses = inner.sender_addresses.clone();
    if let Some(updated) = update_contact(&db, &contact.user_id, move |c| {
        let mut changed = false;
        if c.peer_id != sender_peer_id {
            c.peer_id = sender_peer_id;
            changed = true;
        }
        if c.known_addresses != sender_addresses {
            c.known_addresses = sender_addresses;
            changed = true;
        }
        changed
    })
    .await?
    {
        contact.peer_id = updated.peer_id.clone();
        contact.known_addresses = updated.known_addresses.clone();
        event_tx
            .send(AppEvent::ContactUpdated { contact: updated })
            .await
            .ok();
    }

    for addr_str in &inner.sender_addresses {
        let Ok(addr) = addr_str.parse::<Multiaddr>() else {
            continue;
        };
        if !is_routable_multiaddr(&addr) {
            continue;
        }
        let _ = cmd_tx.send(SwarmCommand::Dial(addr)).await;
    }

    Ok(true)
}

async fn dispatch_offline_kmessage(
    contact: &mut Contact,
    kmessage: KursalMessage,
    db: SharedDatabase,
    cmd_tx: &Sender<SwarmCommand>,
    event_tx: &Sender<AppEvent>,
) -> Result<()> {
    if contact.blocked {
        return Ok(());
    }

    let now = get_timestamp_secs()?;

    match kmessage {
        KursalMessage::Text(ref text) => {
            let msg_id = text.id;

            let stored = StoredMessage {
                id: msg_id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::OfflineDelivered,
                raw_ciphertext: None,
                edited: false,
                pinned: false,
                reactions: Vec::with_capacity(0),
            };
            stored.save(&*db.0.lock().await)?;

            event_tx
                .send(AppEvent::MessageReceived {
                    contact_id: contact.user_id.clone(),
                    message: stored,
                    via_offline: true,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            send_offline_delivery_receipt(db, msg_id, contact, cmd_tx).await?;
        }

        KursalMessage::FileOffer(ref file) => {
            let offer_id = file.id;

            let incoming = FileIncomingEntry {
                their_random: file.random,
                file_size: file.size_bytes,
                hash: file.hash,
                created_at: now,
            };
            db.0.lock().await.raw_write(
                TABLE_FILE_TRANSFERS,
                &format!(
                    "recv:{}:{}",
                    hex::encode(contact.user_id.0),
                    hex::encode(offer_id.0)
                ),
                &incoming.serialize()?,
            )?;

            let stored = StoredMessage {
                id: offer_id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::OfflineDelivered,
                raw_ciphertext: None,
                edited: false,
                pinned: false,
                reactions: Vec::with_capacity(0),
            };
            stored.save(&*db.0.lock().await)?;

            event_tx
                .send(AppEvent::FileOffered {
                    contact_id: contact.user_id.clone(),
                    filename: stored_offer_filename(&stored),
                    offer_id,
                    size_bytes: stored_offer_size(&stored),
                    autodownload: None,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            send_offline_delivery_receipt(db, offer_id, contact, cmd_tx).await?;
        }

        KursalMessage::MessagePin(ref pin) => {
            apply_pin(contact, pin, &db, event_tx).await?;
        }

        KursalMessage::MessageEdit(ref edit) => {
            apply_edit(contact, edit, &db, event_tx).await?;
        }

        KursalMessage::MessageDelete(ref del) => {
            apply_delete(contact, del, &db, event_tx).await?;
        }

        KursalMessage::ReactionAdd(ref r) => {
            apply_reaction_add(contact, r, &db, event_tx, now).await?;
        }

        KursalMessage::ReactionRemove(ref r) => {
            apply_reaction_remove(contact, r, &db, event_tx).await?;
        }

        KursalMessage::DeliveryReceipt(receipt) => {
            let msg_id = receipt.message_id;
            update_offline(&db, &contact.user_id, move |o| {
                drop_acked_bundles(o, &msg_id);
                Ok(())
            })
            .await?;
            crate::messaging::offline::clear_pending_ack(
                &db,
                &contact.user_id,
                &receipt.message_id,
            )
            .await?;

            let loaded =
                StoredMessage::load(&*db.0.lock().await, &contact.user_id, &receipt.message_id)?;
            if let Some(mut message) = loaded {
                message.status = MessageStatus::OfflineDelivered;
                message.save(&*db.0.lock().await)?;

                event_tx
                    .send(AppEvent::DeliveryConfirmed {
                        contact_id: contact.user_id.clone(),
                        message_id: receipt.message_id,
                    })
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        KursalMessage::ProfileUpdate(profile) => {
            if profile.validate().is_err() {
                return Ok(());
            }
            let name = profile.display_name;
            let avatar = profile.avatar_bytes;
            if let Some(updated) = update_contact(&db, &contact.user_id, move |c| {
                c.display_name = name;
                c.avatar_bytes = avatar;
                true
            })
            .await?
            {
                contact.display_name = updated.display_name.clone();
                contact.avatar_bytes = updated.avatar_bytes.clone();
                event_tx
                    .send(AppEvent::ContactUpdated { contact: updated })
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        KursalMessage::FileCancel(cancel) => {
            apply_cancel(db.clone(), &contact.user_id, cancel.offer_id.0, event_tx).await?;
        }

        KursalMessage::AddressAnnounce(ref announce) => {
            crate::api::apply_address_announce(
                &contact.user_id,
                announce.peer_id.clone(),
                announce.addresses.clone(),
                &db,
                cmd_tx,
                Some(event_tx),
            )
            .await?;
        }

        KursalMessage::ContactTerminate => {
            crate::api::handle_incoming::mark_terminated(&db, &contact.user_id, true, event_tx)
                .await?;
        }

        KursalMessage::Typing
        | KursalMessage::CallSignal(_)
        | KursalMessage::FileAccept(_)
        | KursalMessage::CallRecord(_)
        | KursalMessage::ReadReceipt(_) => {}
    }

    Ok(())
}

fn stored_offer_filename(stored: &StoredMessage) -> String {
    if let KursalMessage::FileOffer(f) = &stored.payload {
        sanitize_filename(&f.filename)
    } else {
        String::new()
    }
}

fn stored_offer_size(stored: &StoredMessage) -> u64 {
    if let KursalMessage::FileOffer(f) = &stored.payload {
        f.size_bytes
    } else {
        0
    }
}

async fn send_offline_delivery_receipt(
    db: SharedDatabase,
    msg_id: MessageId,
    contact: &Contact,
    cmd_tx: &Sender<SwarmCommand>,
) -> Result<()> {
    let receipt = KursalMessage::DeliveryReceipt(DeliveryReceipt { message_id: msg_id });

    send_message(receipt, contact, db, cmd_tx, None).await?;

    Ok(())
}
