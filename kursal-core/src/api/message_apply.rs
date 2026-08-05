use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    messaging::{
        StoredMessage, StoredReaction,
        enums::{
            Direction, KursalMessage, MessageDelete, MessageEdit, MessageId, MessagePin,
            MessageStatus, ReactionAdd, ReactionRemove,
        },
        pin_index_set,
    },
    storage::{SharedDatabase, get_timestamp_secs},
};
use tokio::sync::mpsc;

// Message mutations that behave identically whether the message arrived over the
// live connection (handle_incoming) or via the offline mailbox (poll_offline).

pub async fn store_pin_record(
    contact: &Contact,
    pin: &MessagePin,
    direction: Direction,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) {
    let Ok(now) = get_timestamp_secs() else {
        return;
    };

    let stored = StoredMessage {
        id: MessageId::new(),
        contact_id: contact.user_id.clone(),
        payload: KursalMessage::MessagePin(MessagePin {
            target_id: pin.target_id,
            pinned: pin.pinned,
        }),
        timestamp: now,
        direction,
        status: MessageStatus::Delivered,
        raw_ciphertext: None,
        edited: false,
        pinned: false,
        reactions: Vec::new(),
    };

    if stored.save(&*db.0.lock().await).is_err() {
        return;
    }

    let _ = event_tx
        .send(AppEvent::MessageReceived {
            contact_id: contact.user_id.clone(),
            message: stored,
            via_offline: false,
        })
        .await;
}

pub async fn apply_pin(
    contact: &Contact,
    pin: &MessagePin,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let Some(mut message) =
        StoredMessage::load(&*db.0.lock().await, &contact.user_id, &pin.target_id)?
    else {
        return Ok(());
    };

    message.pinned = pin.pinned;
    let ts = message.timestamp;
    message.save(&*db.0.lock().await)?;
    pin_index_set(
        &*db.0.lock().await,
        &contact.user_id,
        &pin.target_id,
        pin.pinned,
        ts,
    )?;

    event_tx
        .send(AppEvent::MessagePinned {
            contact_id: contact.user_id.clone(),
            message_id: pin.target_id,
            pinned: pin.pinned,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    store_pin_record(contact, pin, Direction::Received, db, event_tx).await;

    Ok(())
}

pub async fn apply_edit(
    contact: &Contact,
    edit: &MessageEdit,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let Some(mut message) =
        StoredMessage::load(&*db.0.lock().await, &contact.user_id, &edit.target_id)?
    else {
        return Ok(());
    };

    if !matches!(message.direction, Direction::Received) {
        return Ok(());
    }

    if let KursalMessage::Text(ref mut t) = message.payload {
        t.content = edit.new_content.clone();
    }
    message.edited = true;
    message.save(&*db.0.lock().await)?;

    event_tx
        .send(AppEvent::MessageEdited {
            contact_id: contact.user_id.clone(),
            message_id: edit.target_id,
            new_content: edit.new_content.clone(),
        })
        .await
        .ok_kursal(KursalError::Network)
}

pub async fn apply_delete(
    contact: &Contact,
    del: &MessageDelete,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let Some(message) = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &del.target_id)?
    else {
        return Ok(());
    };

    if !matches!(message.direction, Direction::Received) {
        return Ok(());
    }

    StoredMessage::delete(&*db.0.lock().await, &contact.user_id, &del.target_id)?;

    event_tx
        .send(AppEvent::MessageDeleted {
            contact_id: contact.user_id.clone(),
            message_id: del.target_id,
        })
        .await
        .ok_kursal(KursalError::Network)
}

pub async fn apply_reaction_add(
    contact: &Contact,
    r: &ReactionAdd,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
    now: u64,
) -> Result<()> {
    if emojis::get(&r.emoji).is_none() {
        log::warn!("Rejected invalid reaction emoji");
        return Ok(());
    }

    let Some(mut target) =
        StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id)?
    else {
        return Ok(());
    };

    target.reactions.push(StoredReaction {
        emoji: r.emoji.clone(),
        user_id: contact.user_id.clone(),
        timestamp: now,
    });
    target.save(&*db.0.lock().await)?;

    event_tx
        .send(AppEvent::ReactionAdded {
            contact_id: contact.user_id.clone(),
            message_id: r.target_id,
            emoji: r.emoji.clone(),
        })
        .await
        .ok_kursal(KursalError::Network)
}

pub async fn apply_reaction_remove(
    contact: &Contact,
    r: &ReactionRemove,
    db: &SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let Some(mut message) =
        StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id)?
    else {
        return Ok(());
    };

    message
        .reactions
        .retain(|rx| !(rx.emoji == r.emoji && rx.user_id == contact.user_id));
    message.save(&*db.0.lock().await)?;

    event_tx
        .send(AppEvent::ReactionRemoved {
            contact_id: contact.user_id.clone(),
            message_id: r.target_id,
            emoji: r.emoji.clone(),
        })
        .await
        .ok_kursal(KursalError::Network)
}
