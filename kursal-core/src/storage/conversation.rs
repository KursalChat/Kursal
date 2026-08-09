use super::db::{Database, TABLE_CONVERSATION};
use crate::Result;

// TODO: remove this in next version, like just migrate once
const MIGRATED_KEY: &str = "!read_cursor_migrated";
const PENDING_SYNC_DELETE: u8 = 1u8;

fn suffix_after_prefix(key: &str, prefix: &str) -> Option<String> {
    key.strip_prefix(prefix).map(str::to_string)
}

pub fn get_read_cursor(db: &Database, contact_id: &str) -> Option<String> {
    match db.raw_read(TABLE_CONVERSATION, &format!("read_cursor:{contact_id}")) {
        Ok(Some(bytes)) => String::from_utf8(bytes).ok().filter(|s| !s.is_empty()),
        _ => None,
    }
}

pub fn set_read_cursor(db: &Database, contact_id: &str, value: Option<&str>) -> Result<()> {
    let key = format!("read_cursor:{contact_id}");
    match value.filter(|v| !v.is_empty()) {
        Some(v) => {
            db.raw_write(TABLE_CONVERSATION, &key, v.as_bytes())?;
        }
        None => db.raw_delete(TABLE_CONVERSATION, &key)?,
    }
    Ok(())
}

pub fn get_marked_unread(db: &Database, contact_id: &str) -> bool {
    matches!(
        db.raw_read(TABLE_CONVERSATION, &format!("marked_unread:{contact_id}")),
        Ok(Some(bytes)) if bytes == [1u8]
    )
}

pub fn set_marked_unread(db: &Database, contact_id: &str, value: bool) -> Result<()> {
    let key = format!("marked_unread:{contact_id}");
    if value {
        db.raw_write(TABLE_CONVERSATION, &key, &[1u8])?;
    } else {
        db.raw_delete(TABLE_CONVERSATION, &key)?;
    }
    Ok(())
}

pub fn get_delayed_unseen(db: &Database, contact_id: &str) -> Vec<String> {
    match db.raw_read(TABLE_CONVERSATION, &format!("delayed_unseen:{contact_id}")) {
        Ok(Some(bytes)) => bincode::deserialize(&bytes).unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub fn set_delayed_unseen(db: &Database, contact_id: &str, ids: &[String]) -> Result<()> {
    let key = format!("delayed_unseen:{contact_id}");
    if ids.is_empty() {
        db.raw_delete(TABLE_CONVERSATION, &key)?;
        return Ok(());
    }
    db.raw_write(TABLE_CONVERSATION, &key, &bincode::serialize(ids)?)?;
    Ok(())
}

pub fn list_delayed_unseen(db: &Database) -> Vec<(String, Vec<String>)> {
    db.raw_range(
        TABLE_CONVERSATION,
        "delayed_unseen:",
        "delayed_unseen;",
        None,
    )
    .unwrap_or_default()
    .into_iter()
    .filter_map(|(key, bytes)| {
        let contact_id = suffix_after_prefix(&key, "delayed_unseen:")?;
        let ids: Vec<String> = bincode::deserialize(&bytes).ok()?;
        (!ids.is_empty()).then_some((contact_id, ids))
    })
    .collect()
}

pub fn set_pending_sync(
    db: &Database,
    contact_id: &str,
    message_id: &str,
    is_delete: bool,
) -> Result<()> {
    let key = format!("pending_sync:{contact_id}:{message_id}");
    let tombstoned = is_delete
        || db
            .raw_read(TABLE_CONVERSATION, &key)?
            .is_some_and(|v| v.as_slice() == [PENDING_SYNC_DELETE]);

    let value: [u8; 1] = if tombstoned {
        [PENDING_SYNC_DELETE]
    } else {
        [0u8]
    };
    db.raw_write(TABLE_CONVERSATION, &key, &value)?;

    Ok(())
}

pub fn list_pending_sync(db: &Database) -> Vec<(String, String, bool)> {
    db.raw_range(TABLE_CONVERSATION, "pending_sync:", "pending_sync;", None)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(key, value)| {
            let rest = suffix_after_prefix(&key, "pending_sync:")?;
            let (contact_id, message_id) = rest.split_once(':')?;
            Some((
                contact_id.to_string(),
                message_id.to_string(),
                value.as_slice() == [PENDING_SYNC_DELETE],
            ))
        })
        .collect()
}

pub fn clear_pending_sync_for(db: &Database, contact_id: &str) -> Result<Vec<String>> {
    let prefix = format!("pending_sync:{contact_id}:");
    let entries = db.raw_range(
        TABLE_CONVERSATION,
        &prefix,
        &format!("pending_sync:{contact_id};"),
        None,
    )?;

    let mut finalized_deletes = Vec::new();
    for (key, value) in entries {
        if value.as_slice() == [PENDING_SYNC_DELETE]
            && let Some(message_id) = suffix_after_prefix(&key, &prefix)
        {
            finalized_deletes.push(message_id);
        }
        db.raw_delete(TABLE_CONVERSATION, &key)?;
    }

    Ok(finalized_deletes)
}

pub fn read_cursors_migrated(db: &Database) -> bool {
    matches!(
        db.raw_read(TABLE_CONVERSATION, MIGRATED_KEY),
        Ok(Some(bytes)) if bytes == [1u8]
    )
}

pub fn seed_read_cursors(db: &Database, cursors: &[(String, String)]) -> Result<()> {
    let mut entries: Vec<(String, Vec<u8>)> = cursors
        .iter()
        .map(|(contact_id, message_id)| {
            (
                format!("read_cursor:{contact_id}"),
                message_id.as_bytes().to_vec(),
            )
        })
        .collect();
    entries.push((MIGRATED_KEY.to_string(), vec![1u8]));

    db.raw_write_many(TABLE_CONVERSATION, &entries)
}

pub fn delete_for_contact(db: &Database, contact_id: &str) -> Result<()> {
    db.raw_delete(TABLE_CONVERSATION, &format!("read_cursor:{contact_id}"))?;
    db.raw_delete(TABLE_CONVERSATION, &format!("marked_unread:{contact_id}"))?;
    db.raw_delete(TABLE_CONVERSATION, &format!("delayed_unseen:{contact_id}"))?;
    db.raw_delete_prefix(TABLE_CONVERSATION, &format!("pending_sync:{contact_id}:"))?;
    Ok(())
}

pub fn delete_all(db: &Database) -> Result<()> {
    db.raw_delete_prefix(TABLE_CONVERSATION, "read_cursor:")?;
    db.raw_delete_prefix(TABLE_CONVERSATION, "marked_unread:")?;
    db.raw_delete_prefix(TABLE_CONVERSATION, "delayed_unseen:")?;
    db.raw_delete_prefix(TABLE_CONVERSATION, "pending_sync:")?;
    Ok(())
}
