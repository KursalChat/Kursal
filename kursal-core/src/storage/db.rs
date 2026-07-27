use crate::{
    Result,
    crypto::stream::{stream_decrypt, stream_encrypt},
};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use std::path::Path;
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

pub struct Database {
    pub(crate) inner: redb::Database,
    key: Zeroizing<[u8; 32]>,
}

impl Database {
    pub fn open(path: &Path, key: [u8; 32]) -> Result<Self> {
        let db = redb::Database::create(path)?;

        Ok(Self {
            inner: db,
            key: Zeroizing::new(key),
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
