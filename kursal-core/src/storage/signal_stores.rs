use super::db::{
    Database, TABLE_IDENTITY_KEYS, TABLE_KYBER_PRE_KEYS, TABLE_PRE_KEYS, TABLE_SESSIONS,
    TABLE_SETTINGS, TABLE_SIGNED_PRE_KEYS,
};
use crate::MapKursalResult;
use crate::{KursalError, Result};
use async_trait::async_trait;
use libsignal_protocol::{
    Direction, GenericSignedPreKey, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore,
    KyberPreKeyId, KyberPreKeyRecord, KyberPreKeyStore, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, PublicKey, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore,
};
use std::{array::TryFromSliceError, sync::Arc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct SharedDatabase(pub Arc<Database>);

impl std::ops::Deref for SharedDatabase {
    type Target = Database;

    fn deref(&self) -> &Database {
        &self.0
    }
}

impl SharedDatabase {
    pub fn from_db(db: Database) -> SharedDatabase {
        SharedDatabase(Arc::new(db))
    }

    pub async fn blocking<T, F>(&self, work: F) -> Result<T>
    where
        F: FnOnce(&Database) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.0.clone();

        tokio::task::spawn_blocking(move || work(&db))
            .await
            .map_err(|err| KursalError::Storage(err.to_string()))?
    }

    pub async fn read_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>> {
        let bytes = self.0.raw_read(TABLE_SESSIONS, &address.to_string())?;

        match bytes {
            None => Ok(None),
            Some(b) => {
                let b = Zeroizing::new(b);
                Ok(Some(
                    SessionRecord::deserialize(&b).ok_kursal(KursalError::Storage)?,
                ))
            }
        }
    }
}

#[async_trait(?Send)]
impl SessionStore for SharedDatabase {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<SessionRecord>> {
        let bytes = self
            .0
            .raw_read(TABLE_SESSIONS, &address.to_string())
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Ok(None),
            Some(b) => {
                let b = Zeroizing::new(b);
                let parsed = SessionRecord::deserialize(&b)?;
                Ok(Some(parsed))
            }
        }
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let bytes = Zeroizing::new(record.serialize()?);

        self.0
            .raw_write(TABLE_SESSIONS, &address.to_string(), &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl IdentityKeyStore for SharedDatabase {
    async fn get_identity_key_pair(&self) -> libsignal_protocol::error::Result<IdentityKeyPair> {
        let bytes = self
            .0
            .raw_read(TABLE_IDENTITY_KEYS, "local_identity")
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Identity Key Pair not found".to_string(),
            )),
            Some(b) => {
                let b = Zeroizing::new(b);
                let parsed = IdentityKeyPair::try_from(b.as_slice())?;

                Ok(parsed)
            }
        }
    }

    async fn get_local_registration_id(&self) -> libsignal_protocol::error::Result<u32> {
        let bytes = self
            .0
            .raw_read(TABLE_SETTINGS, "registration_id")
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Local Registration ID not found".to_string(),
            )),
            Some(b) => {
                let parsed =
                    u32::from_be_bytes(b[0..4].try_into().map_err(|e: TryFromSliceError| {
                        SignalProtocolError::InvalidArgument(e.to_string())
                    })?);

                Ok(parsed)
            }
        }
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> libsignal_protocol::error::Result<IdentityChange> {
        let bytes = identity.serialize().to_vec();

        let previous = self
            .0
            .raw_write(TABLE_IDENTITY_KEYS, &address.to_string(), &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(match previous {
            None => IdentityChange::NewOrUnchanged,
            Some(prev) => {
                if prev == bytes {
                    IdentityChange::NewOrUnchanged
                } else {
                    IdentityChange::ReplacedExisting
                }
            }
        })
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _direction: Direction,
    ) -> libsignal_protocol::error::Result<bool> {
        let found = self.get_identity(address).await?;

        match found {
            None => Ok(true),
            Some(key) => {
                if &key == identity {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> libsignal_protocol::error::Result<Option<IdentityKey>> {
        let bytes = self
            .0
            .raw_read(TABLE_IDENTITY_KEYS, &address.to_string())
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Ok(None),
            Some(b) => {
                let parsed = IdentityKey::decode(&b)?;
                Ok(Some(parsed))
            }
        }
    }
}

#[async_trait(?Send)]
impl PreKeyStore for SharedDatabase {
    async fn get_pre_key(
        &self,
        prekey_id: PreKeyId,
    ) -> libsignal_protocol::error::Result<PreKeyRecord> {
        let key = format!("prekey_{}", u32::from(prekey_id));

        let bytes = self
            .0
            .raw_read(TABLE_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Pre Key not found".to_string(),
            )),
            Some(b) => {
                let b = Zeroizing::new(b);
                let parsed = PreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("prekey_{}", u32::from(prekey_id));
        let bytes = Zeroizing::new(record.serialize()?);

        self.0
            .raw_write(TABLE_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }

    async fn remove_pre_key(
        &mut self,
        prekey_id: PreKeyId,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("prekey_{}", u32::from(prekey_id));

        self.0
            .raw_delete(TABLE_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl SignedPreKeyStore for SharedDatabase {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> libsignal_protocol::error::Result<SignedPreKeyRecord> {
        let key = format!("signed_prekey_{}", u32::from(signed_prekey_id));

        let bytes = self
            .0
            .raw_read(TABLE_SIGNED_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Signed Pre Key not found".to_string(),
            )),
            Some(b) => {
                let b = Zeroizing::new(b);
                let parsed = SignedPreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("signed_prekey_{}", u32::from(signed_prekey_id));
        let bytes = Zeroizing::new(record.serialize()?);

        self.0
            .raw_write(TABLE_SIGNED_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl KyberPreKeyStore for SharedDatabase {
    async fn get_kyber_pre_key(
        &self,
        kyber_prekey_id: KyberPreKeyId,
    ) -> libsignal_protocol::error::Result<KyberPreKeyRecord> {
        let key = format!("kyber_prekey_{}", u32::from(kyber_prekey_id));

        let bytes = self
            .0
            .raw_read(TABLE_KYBER_PRE_KEYS, &key)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        match bytes {
            None => Err(SignalProtocolError::InvalidArgument(
                "Kyber Pre Key not found".to_string(),
            )),
            Some(b) => {
                let b = Zeroizing::new(b);
                let parsed = KyberPreKeyRecord::deserialize(&b)?;

                Ok(parsed)
            }
        }
    }

    async fn save_kyber_pre_key(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        record: &KyberPreKeyRecord,
    ) -> libsignal_protocol::error::Result<()> {
        let key = format!("kyber_prekey_{}", u32::from(kyber_prekey_id));
        let bytes = Zeroizing::new(record.serialize()?);

        self.0
            .raw_write(TABLE_KYBER_PRE_KEYS, &key, &bytes)
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }

    async fn mark_kyber_pre_key_used(
        &mut self,
        kyber_prekey_id: KyberPreKeyId,
        _ec_prekey_id: SignedPreKeyId,
        _base_key: &PublicKey,
    ) -> libsignal_protocol::error::Result<()> {
        let id = u32::from(kyber_prekey_id);

        let is_last_resort = self
            .0
            .raw_read(TABLE_SETTINGS, &format!("kyber_lastresort_{id}"))
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?
            .is_some();

        if is_last_resort {
            return Ok(());
        }

        self.0
            .raw_delete(TABLE_KYBER_PRE_KEYS, &format!("kyber_prekey_{id}"))
            .map_err(|err| SignalProtocolError::InvalidArgument(err.to_string()))?;

        Ok(())
    }
}
