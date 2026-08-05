use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::{AppEvent, message_apply::store_pin_record},
    contacts::Contact,
    crypto::messages::message_send,
    first_contact::WireMessage,
    identity::UserId,
    messaging::{
        StoredMessage, StoredReaction,
        enums::{Direction, KursalMessage, MessageId, MessageStatus},
        offline::queue_for_offline,
    },
    network::swarm::{SwarmCommand, is_peer_connected, str_to_multiaddr},
    storage::{SharedDatabase, get_local_user_id, get_timestamp_secs},
};
use libp2p::{Multiaddr, PeerId};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;

const CONNECT_WAIT_MS: u64 = 1500;
const CONNECT_POLL_MS: u64 = 150;

async fn notify_queued_offline(
    app_event_tx: Option<&mpsc::Sender<AppEvent>>,
    contact_id: &UserId,
    msg_id: Option<MessageId>,
) {
    let (Some(tx), Some(message_id)) = (app_event_tx, msg_id) else {
        return;
    };
    tx.send(AppEvent::MessageQueuedOffline {
        contact_id: contact_id.clone(),
        message_id,
    })
    .await
    .ok();
}

pub async fn send_message(
    content: KursalMessage,
    contact: &Contact,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: Option<&mpsc::Sender<AppEvent>>,
) -> Result<Option<MessageId>> {
    send_message_tracked(content, contact, db, cmd_tx, app_event_tx)
        .await
        .map(|(msg_id, _)| msg_id)
}

pub async fn send_message_tracked(
    content: KursalMessage,
    contact: &Contact,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: Option<&mpsc::Sender<AppEvent>>,
) -> Result<(Option<MessageId>, bool)> {
    let kind = content.kind_name();
    let msg_id_dbg = content
        .message_id()
        .map(|m| hex::encode(m.0))
        .unwrap_or_else(|| "<none>".to_string());
    let contact_id_dbg = hex::encode(contact.user_id.0);
    log::info!(
        "[send] start kind={kind} msg_id={msg_id_dbg} contact={contact_id_dbg} peer={}",
        contact.peer_id
    );

    let serialized = content.serialize()?;
    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());

    let now = get_timestamp_secs()?;

    let peer_id = PeerId::from_str(&contact.peer_id).ok_kursal(KursalError::Storage)?;

    if matches!(
        content,
        KursalMessage::Typing | KursalMessage::CallSignal(_) | KursalMessage::ContactTerminate
    ) && !is_peer_connected(cmd_tx, peer_id).await
    {
        log::info!("[send] dropping ephemeral kind={kind} (peer offline)");
        return Ok((None, false));
    }

    let ciphertext = message_send(db.clone(), &address, &serialized).await?;
    log::info!(
        "[send] encrypted kind={kind} msg_id={msg_id_dbg} dr_ct_len={}",
        ciphertext.len()
    );

    let connected = ensure_connected_brief(cmd_tx, peer_id, &contact.known_addresses).await;

    let target_undelivered = content.target_message_id().is_some_and(|target| {
        contact
            .offline
            .send_queue
            .iter()
            .any(|q| q.message_id == Some(target))
            || contact
                .offline
                .pending_bundles
                .iter()
                .any(|b| b.message_ids.contains(&target))
    });

    let deliver_direct = connected && !target_undelivered;

    log::info!(
        "[send] route kind={kind} msg_id={msg_id_dbg} connected={connected} direct={deliver_direct} target_undelivered={target_undelivered} known_addrs={}",
        contact.known_addresses.len()
    );

    if deliver_direct {
        let wire = WireMessage::Encrypted(ciphertext.clone());
        cmd_tx
            .send(SwarmCommand::SendMessage {
                peer_id,
                data: bincode::serialize(&wire)?,
                addresses: str_to_multiaddr(&contact.known_addresses)?,
            })
            .await
            .ok_kursal(KursalError::Network)?;
        log::info!("[send] direct queued kind={kind} msg_id={msg_id_dbg} peer={peer_id}");
    } else if matches!(
        content,
        KursalMessage::Typing | KursalMessage::CallSignal(_) | KursalMessage::ContactTerminate
    ) {
        log::info!("[send] dropping ephemeral kind={kind} (peer offline)");
        return Ok((None, false));
    } else {
        log::info!("[send] -> offline queue kind={kind} msg_id={msg_id_dbg}");
        queue_for_offline(
            &contact.user_id,
            content.message_id(),
            ciphertext.clone(),
            cmd_tx,
            db.clone(),
            app_event_tx,
        )
        .await?;
        notify_queued_offline(app_event_tx, &contact.user_id, content.message_id()).await;
        log::info!("[send] notified queued-offline kind={kind} msg_id={msg_id_dbg}");
    }

    let queued_offline = !deliver_direct;

    if let KursalMessage::MessageDelete(msg) = content {
        let _ = crate::messaging::pin_index_set(
            &*db.0.lock().await,
            &contact.user_id,
            &msg.target_id,
            false,
            0,
        );
        StoredMessage::delete(&*db.0.lock().await, &contact.user_id, &msg.target_id)?;
        return Ok((None, queued_offline));
    }

    if let KursalMessage::MessageEdit(msg) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &msg.target_id);

        if let Ok(Some(mut message)) = loaded {
            if let KursalMessage::Text(ref mut t) = message.payload {
                t.content = msg.new_content;
            }
            message.edited = true;
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok((None, queued_offline));
    }

    if let KursalMessage::ReactionAdd(r) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id);

        if let Ok(Some(mut message)) = loaded {
            message.reactions.push(StoredReaction {
                emoji: r.emoji,
                user_id: get_local_user_id(&*db.0.lock().await)?,
                timestamp: now,
            });
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok((None, queued_offline));
    }

    if let KursalMessage::ReactionRemove(r) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id);

        if let Ok(Some(mut message)) = loaded {
            let local_user_id = get_local_user_id(&*db.0.lock().await)?;
            message
                .reactions
                .retain(|rx| !(rx.emoji == r.emoji && rx.user_id == local_user_id));
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok((None, queued_offline));
    }

    if let KursalMessage::MessagePin(ref pin) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &pin.target_id);

        if let Ok(Some(mut message)) = loaded {
            message.pinned = pin.pinned;
            let ts = message.timestamp;
            let _ = message.save(&*db.0.lock().await);
            let _ = crate::messaging::pin_index_set(
                &*db.0.lock().await,
                &contact.user_id,
                &pin.target_id,
                pin.pinned,
                ts,
            );
        }
        if let Some(tx) = app_event_tx {
            tx.send(AppEvent::MessagePinned {
                contact_id: contact.user_id.clone(),
                message_id: pin.target_id,
                pinned: pin.pinned,
            })
            .await
            .ok();

            store_pin_record(contact, pin, Direction::Sent, &db, tx).await;
        }
        return Ok((None, queued_offline));
    }

    let Some(msg_id) = content.message_id() else {
        return Ok((None, queued_offline));
    };

    if matches!(
        content,
        KursalMessage::DeliveryReceipt(_)
            | KursalMessage::FileAccept(_)
            | KursalMessage::CallSignal(_)
    ) {
        return Ok((Some(msg_id), queued_offline));
    }

    let stored = StoredMessage {
        id: msg_id,
        status: MessageStatus::Sending,
        direction: Direction::Sent,
        timestamp: now,
        contact_id: contact.user_id.clone(),
        payload: content,
        raw_ciphertext: Some(ciphertext.clone()),
        edited: false,
        pinned: false,
        reactions: Vec::with_capacity(0),
    };

    stored.save(&*db.0.lock().await)?;

    crate::messaging::offline::schedule_direct_ack_deadline(
        contact.user_id.clone(),
        msg_id,
        now,
        cmd_tx.clone(),
        db.clone(),
        app_event_tx.cloned(),
    )
    .await;

    Ok((Some(msg_id), queued_offline))
}

async fn ensure_connected_brief(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    peer_id: PeerId,
    known_addresses: &[String],
) -> bool {
    if is_peer_connected(cmd_tx, peer_id).await {
        log::debug!("[send] ensure_connected: already connected to {peer_id}");
        return true;
    }
    let mut dialed = 0usize;
    for addr_str in known_addresses {
        match addr_str.parse::<Multiaddr>() {
            Ok(addr) => {
                let _ = cmd_tx.send(SwarmCommand::Dial(addr)).await;
                dialed += 1;
            }
            Err(err) => log::debug!("[send] ensure_connected: bad addr {addr_str}: {err}"),
        }
    }
    log::info!(
        "[send] ensure_connected: dialing {peer_id} via {dialed}/{} known addrs, waiting up to {CONNECT_WAIT_MS}ms",
        known_addresses.len()
    );
    let deadline = tokio::time::Instant::now() + Duration::from_millis(CONNECT_WAIT_MS);
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(CONNECT_POLL_MS)).await;
        if is_peer_connected(cmd_tx, peer_id).await {
            log::info!("[send] ensure_connected: connected to {peer_id} after dial");
            return true;
        }
    }
    log::info!("[send] ensure_connected: gave up dialing {peer_id} (offline path)");
    false
}
