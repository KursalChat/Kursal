use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    identity::UserId,
    messaging::enums::{Direction, KursalMessage, MessageId, MessageStatus},
    storage::{Database, TABLE_MESSAGES, TABLE_PINNED},
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
        let contact_id = hex::encode(self.contact_id.0);
        let message_id = hex::encode(self.id.0);

        let serialized = bincode::serialize(self)?;

        db.raw_write(
            TABLE_MESSAGES,
            &format!("{contact_id}:{message_id}"),
            &serialized,
        )?;

        Ok(())
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
        let mut out: Vec<Self> = db
            .raw_readall(TABLE_MESSAGES)?
            .into_iter()
            .filter_map(|(_, bytes)| bincode::deserialize::<Self>(&bytes).ok())
            .filter(|m| message_matches(m, &q))
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        out.truncate(limit);
        Ok(out)
    }
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
