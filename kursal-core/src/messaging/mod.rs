use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    identity::UserId,
    messaging::enums::{Direction, KursalMessage, MessageId, MessageStatus},
    storage::{Database, TABLE_FILE_TRANSFERS, TABLE_MESSAGES, TABLE_PINNED, WriteBatch},
};
use serde::{Deserialize, Serialize};

pub mod enums;
pub mod offline;

#[derive(Serialize, Deserialize)]
pub struct StoredReaction {
    pub emoji: String,
    pub user_id: UserId,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: MessageId,
    pub contact_id: UserId,
    pub direction: Direction,
    pub payload: KursalMessage,
    pub status: MessageStatus,
    pub timestamp: u64,
    pub raw_ciphertext: Option<Vec<u8>>,
    pub edited: bool,
    pub pinned: bool,
    pub reactions: Vec<StoredReaction>,
}

impl StoredMessage {
    pub fn save(&self, db: &Database) -> Result<()> {
        let mut batch = db.batch()?;
        self.save_into(&mut batch)?;
        batch.commit()
    }

    pub(crate) fn save_into(&self, batch: &mut WriteBatch<'_>) -> Result<()> {
        let contact_id = hex::encode(self.contact_id.0);
        let message_id = hex::encode(self.id.0);

        let serialized = bincode::serialize(self)?;

        batch.put(
            TABLE_MESSAGES,
            &format!("{contact_id}:{message_id}"),
            &serialized,
        )
    }

    pub fn load(db: &Database, contact_id: &UserId, id: &MessageId) -> Result<Option<Self>> {
        let contact_id = hex::encode(contact_id.0);
        let message_id = hex::encode(id.0);

        let read = db.raw_read(TABLE_MESSAGES, &format!("{contact_id}:{message_id}"))?;

        let result = read
            .map(|e| bincode::deserialize::<StoredMessage>(&e))
            .transpose()
            .ok_kursal(KursalError::Storage)?;

        Ok(result)
    }

    pub fn delete(db: &Database, contact_id: &UserId, id: &MessageId) -> Result<()> {
        let contact_id = hex::encode(contact_id.0);
        let message_id = hex::encode(id.0);

        db.raw_delete(TABLE_MESSAGES, &format!("{contact_id}:{message_id}"))?;
        db.raw_delete(
            TABLE_FILE_TRANSFERS,
            &format!("recvpath:{contact_id}:{message_id}"),
        )?;

        Ok(())
    }

    pub fn set_failed(db: &Database, contact_id: &UserId, id: &MessageId) -> Result<bool> {
        if let Some(mut m) = Self::load(db, contact_id, id)?
            && matches!(m.status, MessageStatus::Sending)
        {
            m.status = MessageStatus::Failed;
            m.save(db)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn load_recent(
        db: &Database,
        contact_id: &UserId,
        limit: usize,
        before: Option<&MessageId>,
    ) -> Result<Vec<Self>> {
        let prefix = hex::encode(contact_id.0);
        let start = format!("{}:", prefix);
        let end = before
            .map(|id| format!("{}:{}", prefix, hex::encode(id.0)))
            .unwrap_or_else(|| format!("{};", prefix)); // ';' sorts just after ':'

        let mut msgs: Vec<Self> = db
            .raw_range_rev(TABLE_MESSAGES, &start, &end, limit)?
            .into_iter()
            .filter_map(|(key, bytes)| match bincode::deserialize::<Self>(&bytes) {
                Ok(m) => Some(m),
                Err(err) => {
                    log::warn!("[messages] skipping undeserializable row key={key}: {err}");
                    None
                }
            })
            .collect();

        msgs.reverse();

        Ok(msgs)
    }

    pub fn load_after(
        db: &Database,
        contact_id: &UserId,
        after: &MessageId,
        limit: usize,
    ) -> Result<Vec<Self>> {
        let prefix = hex::encode(contact_id.0);
        let start = format!("{}:{}", prefix, hex::encode(after.0));
        let end = format!("{};", prefix);

        let mut msgs: Vec<Self> = db
            .raw_range(TABLE_MESSAGES, &start, &end, Some(limit + 1))?
            .into_iter()
            .filter_map(|(key, bytes)| match bincode::deserialize::<Self>(&bytes) {
                Ok(m) => Some(m),
                Err(err) => {
                    log::warn!("[messages] skipping undeserializable row key={key}: {err}");
                    None
                }
            })
            .collect();

        if msgs.first().map(|m| m.id) == Some(*after) {
            msgs.remove(0);
        }

        Ok(msgs)
    }

    pub fn load_around(
        db: &Database,
        contact_id: &UserId,
        message_id: &MessageId,
        limit: usize,
    ) -> Result<Vec<Self>> {
        let half = (limit / 2).max(1);
        let mut out = Self::load_recent(db, contact_id, half, Some(message_id))?;
        if let Some(target) = Self::load(db, contact_id, message_id)? {
            out.push(target);
        }
        out.extend(Self::load_after(db, contact_id, message_id, half)?);
        Ok(out)
    }

    pub fn search(
        db: &Database,
        contact_id: &UserId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Self>> {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let prefix = hex::encode(contact_id.0);
        let mut out: Vec<Self> = db
            .raw_range(
                TABLE_MESSAGES,
                &format!("{prefix}:"),
                &format!("{prefix};"),
                None,
            )?
            .into_iter()
            .filter_map(|(_, bytes)| bincode::deserialize::<Self>(&bytes).ok())
            .filter(|m| message_matches(m, &q))
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        out.truncate(limit);
        out.reverse();
        Ok(out)
    }

    pub fn search_global(db: &Database, query: &str, limit: usize) -> Result<Vec<Self>> {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let mut out: Vec<Self> = Vec::new();
        db.raw_scan_all(TABLE_MESSAGES, |_, bytes| {
            if let Ok(message) = bincode::deserialize::<Self>(&bytes)
                && message_matches(&message, &q)
            {
                out.push(message);
            }
            true
        })?;
        out.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        out.truncate(limit);
        Ok(out)
    }
}

pub const UNREAD_BADGE_CAP: usize = 99;

pub struct UnreadSummary {
    pub count: usize,
    pub capped: bool,
    pub first: Option<MessageId>,
}

fn is_unread_candidate(m: &StoredMessage) -> bool {
    if !matches!(m.direction, Direction::Received) {
        return false;
    }
    match &m.payload {
        KursalMessage::Text(t) => !t.content.is_empty(),
        KursalMessage::FileOffer(_)
        | KursalMessage::CallRecord(_)
        | KursalMessage::MessagePin(_) => true,
        _ => false,
    }
}

fn contact_bounds(contact_id: &UserId, cursor: Option<&MessageId>) -> (String, String) {
    let prefix = hex::encode(contact_id.0);
    let after = match cursor {
        Some(id) => format!("{prefix}:{}", hex::encode(id.0)),
        None => format!("{prefix}:"),
    };

    (after, format!("{prefix};"))
}

pub fn unread_after(
    db: &Database,
    contact_id: &UserId,
    cursor: Option<&MessageId>,
    cap: usize,
) -> Result<UnreadSummary> {
    let (after, before) = contact_bounds(contact_id, cursor);

    let mut summary = UnreadSummary {
        count: 0,
        capped: false,
        first: None,
    };

    db.raw_scan(TABLE_MESSAGES, &after, &before, |key, bytes| {
        match bincode::deserialize::<StoredMessage>(&bytes) {
            Ok(m) if is_unread_candidate(&m) => {
                if summary.count == cap {
                    summary.capped = true;
                    return false;
                }
                summary.first.get_or_insert(m.id);
                summary.count += 1;
            }
            Ok(_) => {}
            Err(err) => log::warn!("[messages] skipping undeserializable row key={key}: {err}"),
        }
        true
    })?;

    Ok(summary)
}

pub fn received_since_rev(
    db: &Database,
    contact_id: &UserId,
    cursor: Option<&MessageId>,
    cap: usize,
) -> Result<Vec<MessageId>> {
    let (after, before) = contact_bounds(contact_id, cursor);

    let mut ids = Vec::new();
    db.raw_scan_rev(TABLE_MESSAGES, &after, &before, |key, bytes| {
        match bincode::deserialize::<StoredMessage>(&bytes) {
            Ok(m) if is_unread_candidate(&m) => ids.push(m.id),
            Ok(_) => {}
            Err(err) => log::warn!("[messages] skipping undeserializable row key={key}: {err}"),
        }
        ids.len() < cap
    })?;

    ids.reverse();
    Ok(ids)
}

fn message_id_from_key(key: &str) -> Option<MessageId> {
    let hex_id = key.rsplit(':').next()?;
    let bytes = hex::decode(hex_id).ok()?;
    <[u8; 16]>::try_from(bytes.as_slice()).ok().map(MessageId)
}

pub fn newest_received(db: &Database, contact_id: &UserId) -> Result<Option<MessageId>> {
    let (after, before) = contact_bounds(contact_id, None);

    let mut found = None;
    db.raw_scan_rev(
        TABLE_MESSAGES,
        &after,
        &before,
        |_, bytes| match bincode::deserialize::<StoredMessage>(&bytes) {
            Ok(m) if is_unread_candidate(&m) => {
                found = Some(m.id);
                false
            }
            _ => true,
        },
    )?;

    Ok(found)
}

pub fn newest_message(db: &Database, contact_id: &UserId) -> Result<Option<MessageId>> {
    let (after, before) = contact_bounds(contact_id, None);

    Ok(db
        .raw_last_key(TABLE_MESSAGES, &after, &before)?
        .as_deref()
        .and_then(message_id_from_key))
}

pub fn message_before(
    db: &Database,
    contact_id: &UserId,
    before: &MessageId,
) -> Result<Option<MessageId>> {
    let prefix = hex::encode(contact_id.0);

    Ok(db
        .raw_last_key(
            TABLE_MESSAGES,
            &format!("{prefix}:"),
            &format!("{prefix}:{}", hex::encode(before.0)),
        )?
        .as_deref()
        .and_then(message_id_from_key))
}

fn message_matches(m: &StoredMessage, q: &str) -> bool {
    match &m.payload {
        KursalMessage::Text(t) => t.content.to_lowercase().contains(q),
        KursalMessage::FileOffer(f) => f.filename.to_lowercase().contains(q),
        _ => false,
    }
}

pub fn pin_index_set(
    db: &Database,
    contact_id: &UserId,
    id: &MessageId,
    pinned: bool,
    timestamp: u64,
) -> Result<()> {
    let key = format!("{}:{}", hex::encode(contact_id.0), hex::encode(id.0));
    if pinned {
        db.raw_write(TABLE_PINNED, &key, &timestamp.to_be_bytes())?;
    } else {
        db.raw_delete(TABLE_PINNED, &key)?;
    }
    Ok(())
}

pub fn pin_index_list(db: &Database, contact_id: &UserId) -> Result<Vec<MessageId>> {
    let prefix = hex::encode(contact_id.0);
    let entries = db.raw_range(
        TABLE_PINNED,
        &format!("{prefix}:"),
        &format!("{prefix};"),
        None,
    )?;
    let mut ids = Vec::with_capacity(entries.len());
    for (key, _ts) in entries {
        if let Some(hex_id) = key.split(':').nth(1)
            && let Ok(bytes) = hex::decode(hex_id)
            && let Ok(arr) = <[u8; 16]>::try_from(bytes.as_slice())
        {
            ids.push(MessageId(arr));
        }
    }
    Ok(ids)
}
