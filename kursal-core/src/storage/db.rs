use crate::{
    Result,
    crypto::stream::{stream_decrypt, stream_encrypt},
};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use std::{
    ops::Bound,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use zeroize::Zeroizing;

pub const TABLE_SESSIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("sessions");
pub const TABLE_IDENTITY_KEYS: TableDefinition<&str, &[u8]> = TableDefinition::new("identity_keys");
pub const TABLE_PRE_KEYS: TableDefinition<&str, &[u8]> = TableDefinition::new("pre_keys");
pub const TABLE_SIGNED_PRE_KEYS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("signed_pre_keys");
pub const TABLE_KYBER_PRE_KEYS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("kyber_pre_keys");
pub const TABLE_CONTACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("contacts");
pub const TABLE_MESSAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("messages");
pub const TABLE_LTC_CACHE: TableDefinition<&str, &[u8]> = TableDefinition::new("ltc_cache");
pub const TABLE_SETTINGS: TableDefinition<&str, &[u8]> = TableDefinition::new("settings");
pub const TABLE_PINNED: TableDefinition<&str, &[u8]> = TableDefinition::new("pinned");
pub const TABLE_FILE_TRANSFERS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("file_transfers");
pub const TABLE_PENDING_ACK: TableDefinition<&str, &[u8]> = TableDefinition::new("pending_ack");
pub const TABLE_CONVERSATION: TableDefinition<&str, &[u8]> = TableDefinition::new("conversation");

pub(crate) struct WriteBatch<'db> {
    txn: redb::WriteTransaction,
    key: &'db Zeroizing<[u8; 32]>,
}

impl WriteBatch<'_> {
    pub(crate) fn put(
        &mut self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
        value: &[u8],
    ) -> Result<()> {
        let enc_value = stream_encrypt(self.key, value)?;
        self.txn
            .open_table(table)?
            .insert(key, enc_value.as_slice())?;

        Ok(())
    }

    pub(crate) fn remove(&mut self, table: TableDefinition<&str, &[u8]>, key: &str) -> Result<()> {
        self.txn.open_table(table)?.remove(key)?;

        Ok(())
    }

    pub(crate) fn commit(self) -> Result<()> {
        self.txn.commit()?;

        Ok(())
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
const PAGE_CACHE_BYTES: usize = 8 * 1024 * 1024;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const PAGE_CACHE_BYTES: usize = 32 * 1024 * 1024;

static NEXT_DB_ID: AtomicU64 = AtomicU64::new(0);

pub struct Database {
    pub(crate) inner: redb::Database,
    key: Zeroizing<[u8; 32]>,
    id: u64,
}

impl Database {
    pub fn open(path: &Path, key: [u8; 32]) -> Result<Self> {
        let db = redb::Database::builder()
            .set_cache_size(PAGE_CACHE_BYTES)
            .create(path)?;

        Ok(Self {
            inner: db,
            key: Zeroizing::new(key),
            id: NEXT_DB_ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn batch(&self) -> Result<WriteBatch<'_>> {
        Ok(WriteBatch {
            txn: self.inner.begin_write()?,
            key: &self.key,
        })
    }

    /// As [`Database::batch`], but the commit is not flushed to disk. A crash
    /// loses the writes. Only for state that is re-derivable, never for messages
    /// or key material.
    pub(crate) fn batch_deferred(&self) -> Result<WriteBatch<'_>> {
        let mut txn = self.inner.begin_write()?;
        txn.set_durability(redb::Durability::None)
            .map_err(|err| crate::KursalError::Storage(err.to_string()))?;

        Ok(WriteBatch {
            txn,
            key: &self.key,
        })
    }

    pub(crate) fn raw_write(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
        value: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        let write_txn = self.inner.begin_write()?;

        // Encrypt value with AES and nonce
        let enc_value = stream_encrypt(&self.key, value)?;

        // Save to DB
        let previous_value = {
            let mut table = write_txn.open_table(table)?;

            let previous_value = table.insert(key, enc_value.as_slice())?;

            previous_value
                .map(|v| stream_decrypt(&self.key, v.value()))
                .transpose()
        };

        write_txn.commit()?;

        previous_value
    }

    pub(crate) fn raw_write_deferred(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
        value: &[u8],
    ) -> Result<()> {
        let mut batch = self.batch_deferred()?;
        batch.put(table, key, value)?;
        batch.commit()
    }

    pub(crate) fn raw_delete_deferred(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
    ) -> Result<()> {
        let mut batch = self.batch_deferred()?;
        batch.remove(table, key)?;
        batch.commit()
    }

    pub(crate) fn raw_read(
        &self,
        table: TableDefinition<&str, &[u8]>,
        key: &str,
    ) -> Result<Option<Vec<u8>>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(err) => return Err(err.into()),
        };

        let result = table.get(key)?;

        match result {
            None => Ok(None),
            Some(bytes) => {
                let decrypted = stream_decrypt(&self.key, bytes.value())?;
                Ok(Some(decrypted))
            }
        }
    }

    pub(crate) fn raw_delete(&self, table: TableDefinition<&str, &[u8]>, key: &str) -> Result<()> {
        let write_txn = self.inner.begin_write()?;

        {
            let mut table = write_txn.open_table(table)?;

            table.remove(key)?;
        }

        write_txn.commit()?;

        Ok(())
    }

    /// Walks a whole table one row at a time, where `raw_readall` holds every
    /// decrypted row at once. `visit` returns false to stop the scan.
    pub(crate) fn raw_scan_all(
        &self,
        table: TableDefinition<&str, &[u8]>,
        mut visit: impl FnMut(&str, Vec<u8>) -> bool,
    ) -> Result<()> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        for entry in table.iter()? {
            let (k, v) = match entry {
                Ok(kv) => kv,
                Err(err) => {
                    log::warn!("[storage] scan entry error: {err}");
                    continue;
                }
            };

            let key = k.value();
            match stream_decrypt(&self.key, v.value()) {
                Ok(decrypted) => {
                    if !visit(key, decrypted) {
                        break;
                    }
                }
                Err(err) => log::warn!("[storage] skipping undecryptable row key={key}: {err}"),
            }
        }

        Ok(())
    }

    pub(crate) fn raw_keys(
        &self,
        table: TableDefinition<&str, &[u8]>,
        prefix: &str,
    ) -> Result<Vec<String>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        let mut out = Vec::new();
        for entry in table.range(prefix..)? {
            let (k, _) = entry?;
            if !k.value().starts_with(prefix) {
                break;
            }
            out.push(k.value().to_string());
        }

        Ok(out)
    }

    pub(crate) fn raw_range(
        &self,
        table: TableDefinition<&str, &[u8]>,
        start: &str,
        end: &str,
        limit: Option<usize>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        let op = table.range(start..=end)?;

        let op: Box<dyn Iterator<Item = _>> = if let Some(limit) = limit {
            Box::new(op.take(limit))
        } else {
            Box::new(op)
        };

        Ok(op
            .filter_map(|entry| {
                let (k, v) = match entry {
                    Ok(kv) => kv,
                    Err(err) => {
                        log::warn!("[storage] range entry error: {err}");
                        return None;
                    }
                };
                match stream_decrypt(&self.key, v.value()) {
                    Ok(decrypted) => Some((k.value().to_string(), decrypted)),
                    Err(err) => {
                        log::warn!(
                            "[storage] skipping undecryptable row key={}: {err}",
                            k.value()
                        );
                        None
                    }
                }
            })
            .collect())
    }

    pub(crate) fn raw_range_rev(
        &self,
        table: TableDefinition<&str, &[u8]>,
        start: &str,
        end: &str,
        limit: usize,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        Ok(table
            .range(start..end)?
            .rev()
            .take(limit)
            .filter_map(|entry| {
                let (k, v) = match entry {
                    Ok(kv) => kv,
                    Err(err) => {
                        log::warn!("[storage] range entry error: {err}");
                        return None;
                    }
                };
                match stream_decrypt(&self.key, v.value()) {
                    Ok(decrypted) => Some((k.value().to_string(), decrypted)),
                    Err(err) => {
                        log::warn!(
                            "[storage] skipping undecryptable row key={}: {err}",
                            k.value()
                        );
                        None
                    }
                }
            })
            .collect())
    }

    pub(crate) fn raw_write_many(
        &self,
        table: TableDefinition<&str, &[u8]>,
        entries: &[(String, Vec<u8>)],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let write_txn = self.inner.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            for (key, value) in entries {
                let enc_value = stream_encrypt(&self.key, value)?;
                table.insert(key.as_str(), enc_value.as_slice())?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }

    pub(crate) fn raw_last_key(
        &self,
        table: TableDefinition<&str, &[u8]>,
        after: &str,
        before: &str,
    ) -> Result<Option<String>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(err) => return Err(err.into()),
        };

        let mut range = table.range::<&str>((Bound::Excluded(after), Bound::Excluded(before)))?;

        match range.next_back() {
            Some(entry) => Ok(Some(entry?.0.value().to_string())),
            None => Ok(None),
        }
    }

    pub(crate) fn raw_scan(
        &self,
        table: TableDefinition<&str, &[u8]>,
        after: &str,
        before: &str,
        mut visit: impl FnMut(&str, Vec<u8>) -> bool,
    ) -> Result<()> {
        self.scan_bounded(table, after, before, false, &mut visit)
    }

    pub(crate) fn raw_scan_rev(
        &self,
        table: TableDefinition<&str, &[u8]>,
        after: &str,
        before: &str,
        mut visit: impl FnMut(&str, Vec<u8>) -> bool,
    ) -> Result<()> {
        self.scan_bounded(table, after, before, true, &mut visit)
    }

    fn scan_bounded(
        &self,
        table: TableDefinition<&str, &[u8]>,
        after: &str,
        before: &str,
        reverse: bool,
        visit: &mut impl FnMut(&str, Vec<u8>) -> bool,
    ) -> Result<()> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(()),
            Err(err) => return Err(err.into()),
        };

        let range = table.range::<&str>((Bound::Excluded(after), Bound::Excluded(before)))?;
        let range: Box<dyn Iterator<Item = _>> = if reverse {
            Box::new(range.rev())
        } else {
            Box::new(range)
        };

        for entry in range {
            let (k, v) = match entry {
                Ok(kv) => kv,
                Err(err) => {
                    log::warn!("[storage] range entry error: {err}");
                    continue;
                }
            };

            let key = k.value();
            match stream_decrypt(&self.key, v.value()) {
                Ok(decrypted) => {
                    if !visit(key, decrypted) {
                        break;
                    }
                }
                Err(err) => log::warn!("[storage] skipping undecryptable row key={key}: {err}"),
            }
        }

        Ok(())
    }

    pub(crate) fn raw_readall(
        &self,
        table: TableDefinition<&str, &[u8]>,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.inner.begin_read()?;

        let table = match read_txn.open_table(table) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(err) => return Err(err.into()),
        };

        table
            .iter()?
            .map(|entry| {
                let (k, v) = entry?;

                let key = k.value().to_string();

                let decrypted = stream_decrypt(&self.key, v.value())?;

                Ok((key, decrypted))
            })
            .collect::<Result<Vec<_>>>()
    }

    pub(crate) fn raw_delete_prefix(
        &self,
        table: TableDefinition<&str, &[u8]>,
        prefix: &str,
    ) -> Result<()> {
        let write_txn = self.inner.begin_write()?;

        {
            let mut tbl = write_txn.open_table(table)?;

            let keys_to_delete: Vec<String> = tbl
                .range(prefix..)?
                .take_while(|entry| {
                    entry
                        .as_ref()
                        .map(|(k, _)| k.value().starts_with(prefix))
                        .unwrap_or(false)
                })
                .map(|entry| entry.map(|(k, _)| k.value().to_string()))
                .collect::<std::result::Result<_, _>>()?;

            for key in &keys_to_delete {
                tbl.remove(key.as_str())?;
            }
        }

        write_txn.commit()?;

        Ok(())
    }

    pub(crate) fn raw_delete_all(&self, table: TableDefinition<&str, &[u8]>) -> Result<()> {
        let write_txn = self.inner.begin_write()?;

        {
            let mut tbl = write_txn.open_table(table)?;

            let keys_to_delete: Vec<String> = tbl
                .iter()?
                .map(|entry| entry.map(|(k, _)| k.value().to_string()))
                .collect::<std::result::Result<_, _>>()?;

            for key in &keys_to_delete {
                tbl.remove(key.as_str())?;
            }
        }

        write_txn.commit()?;

        Ok(())
    }
}
