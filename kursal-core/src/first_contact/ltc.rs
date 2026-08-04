use crate::MapKursalResult;
use crate::dto::LtcStatusDto;
use crate::storage::Database;
use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::{PreKeyBundleData, mailbox_kem_encapsulate, session_initiate},
    first_contact::{
        ContactResponse, WireMessage, forget_ack_waiter, make_username, register_ack_waiter,
    },
    identity::UserId,
    messaging::{enums::MessageId, offline::new_offline_state},
    network::{
        NetworkManager,
        swarm::{SwarmCommand, get_listen_addrs, str_to_multiaddr},
    },
    storage::{
        SharedDatabase, TABLE_KYBER_PRE_KEYS, TABLE_LTC_CACHE, TABLE_SESSIONS, TABLE_SETTINGS,
        file::KursalFile, get_dilithium_pub, get_timestamp_secs,
    },
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, IdentityKeyStore, ProtocolAddress, PublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::time::Duration;
use zeroize::Zeroizing;

const ACK_TIMEOUT_SECS: u64 = 20;

#[derive(Serialize, Deserialize)]
pub struct LtcState {
    pub payload_id: MessageId,
    pub pre_key_bundle: Vec<u8>,
    pub kyber_pre_key_id: u32,
    pub dilithium_pub_key: Vec<u8>,
    pub created_at: u64,
    pub expires_at: u64,       // u64::MAX = basically never
    pub max_uses: Option<u32>, // None = unlimited
    pub uses: u32,
}

impl LtcState {
    pub async fn create(
        db: SharedDatabase,
        max_uses: Option<u32>,
        ttl_secs: Option<u64>,
    ) -> Result<Self> {
        let bundle = PreKeyBundleData::build_pre_key_bundle_noprekey(db.clone()).await?;
        let kyber_pre_key_id: u32 = bundle.kyber_pre_key_id.into();

        {
            Self::revoke_ltc(db.clone()).await.ok();
        }

        let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

        let created_at = get_timestamp_secs()?;
        let expires_at = ttl_secs
            .map(|t| created_at.saturating_add(t))
            .unwrap_or(u64::MAX);

        let payload_id = MessageId::new();

        let state = LtcState {
            payload_id,
            pre_key_bundle: bundle.serialize()?,
            kyber_pre_key_id,
            dilithium_pub_key,
            created_at,
            expires_at,
            max_uses,
            uses: 0,
        };

        db.0.lock()
            .await
            .raw_write(TABLE_LTC_CACHE, "ltc_current", &state.serialize()?)?;

        Ok(state)
    }

    pub async fn update_limits(
        db: SharedDatabase,
        max_uses: Option<u32>,
        ttl_secs: Option<u64>,
    ) -> Result<LtcStatusDto> {
        let db_lock = db.0.lock().await;

        let mut state = Self::load(&db_lock)?
            .ok_or(KursalError::Storage("No LTC currently stored".to_string()))?;

        state.max_uses = max_uses;

        let now = get_timestamp_secs()?;
        state.expires_at = ttl_secs.map(|t| now.saturating_add(t)).unwrap_or(u64::MAX);

        state.save(&db_lock)?;

        Self::dto_serialize(&state)
    }

    pub fn load(db: &Database) -> Result<Option<Self>> {
        db.raw_read(TABLE_LTC_CACHE, "ltc_current")?
            .map(|raw: Vec<u8>| Self::deserialize(&raw))
            .transpose()
    }

    pub fn save(&self, db: &Database) -> Result<()> {
        db.raw_write(TABLE_LTC_CACHE, "ltc_current", &self.serialize()?)?;

        Ok(())
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn dto_serialize(&self) -> Result<LtcStatusDto> {
        let expires_at = match self.expires_at {
            u64::MAX => None,
            v => Some(v),
        };

        let dto = LtcStatusDto {
            payload_id: hex::encode(self.payload_id.0),
            created_at: self.created_at,
            expires_at,
            max_uses: self.max_uses,
            uses: self.uses,
            size_bytes: u32::try_from(self.dilithium_pub_key.len() + self.pre_key_bundle.len())
                .unwrap_or(u32::MAX),
        };

        Ok(dto)
    }

    pub fn is_expired(&self) -> bool {
        let now = get_timestamp_secs().unwrap_or(u64::MAX);
        now > self.expires_at
    }

    pub async fn revoke_ltc(db: SharedDatabase) -> Result<()> {
        let lock = db.0.lock().await;
        let loaded = Self::load(&lock);

        if let Ok(Some(previous)) = loaded {
            lock.raw_delete(
                TABLE_KYBER_PRE_KEYS,
                &format!("kyber_prekey_{}", previous.kyber_pre_key_id),
            )?;
            lock.raw_delete(
                TABLE_SETTINGS,
                &format!("kyber_lastresort_{}", previous.kyber_pre_key_id),
            )?;
        }

        lock.raw_delete(TABLE_LTC_CACHE, "ltc_current")?;

        Ok(())
    }

    pub async fn export_ltc(db: SharedDatabase, network: &NetworkManager) -> Result<Vec<u8>> {
        let state = {
            let db_lock = db.0.lock().await;
            Self::load(&db_lock)?
                .ok_or(KursalError::Storage("No LTC currently stored".to_string()))?
        };

        let peer_id = network.primary.peer_id.to_base58();
        let relay_addresses = get_listen_addrs(&network.primary.cmd_tx).await?;

        let payload = LtcPayload {
            payload_id: state.payload_id,
            peer_id,
            pre_key_bundle: state.pre_key_bundle,
            dilithium_pub_key: state.dilithium_pub_key,
            relay_addresses,
            created_at: state.created_at,
            expires_at: state.expires_at,
        };

        KursalFile::LtcPayload(payload.serialize()?).serialize()
    }

    pub async fn import_ltc(
        payload: LtcPayload,
        db: SharedDatabase,
        network: &NetworkManager,
    ) -> Result<Contact> {
        if payload.peer_id == network.primary.peer_id.to_base58() {
            log::debug!("[ltc] Cannot add yourself as a contact");
            return Err(KursalError::Network(
                "Cannot add yourself as a contact".to_string(),
            ));
        }

        let now = get_timestamp_secs()?;

        if payload.expires_at < now {
            return Err(KursalError::Identity("LTC expired".to_string()));
        }

        let bundle = PreKeyBundleData::deserialize(&payload.pre_key_bundle)?;
        let identity_key_bytes = bundle.identity_key.public_key().serialize().to_vec();

        let user_id = UserId(Sha256::digest(&identity_key_bytes).into());
        let remote_address =
            ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());
        let mailbox_kem_prekey_id: u32 = bundle.kyber_pre_key_id.into();
        let mailbox_kem_pub = bundle.kyber_pre_key_public.serialize().to_vec();
        session_initiate(db.clone(), bundle, &remote_address).await?;
        let (pq_secret, mailbox_kem_ct) = mailbox_kem_encapsulate(&mailbox_kem_pub)?;

        let my_keypair = db
            .get_identity_key_pair()
            .await
            .ok_kursal(KursalError::Crypto)?;
        let peer_identity =
            PublicKey::deserialize(&identity_key_bytes).ok_kursal(KursalError::Crypto)?;
        let classical_secret = Zeroizing::new(
            my_keypair
                .private_key()
                .calculate_agreement(&peer_identity)
                .ok_kursal(KursalError::Crypto)?,
        );

        let my_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
        let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

        let contact = Contact {
            user_id,
            peer_id: payload.peer_id.clone(),
            display_name: make_username(&payload.peer_id),
            avatar_bytes: None,
            identity_pub_key: identity_key_bytes.clone(),
            dilithium_pub_key: payload.dilithium_pub_key.clone(),
            known_addresses: payload.relay_addresses.clone(),
            verified: false,
            profile_shared: false,
            blocked: false,
            created_at: now,
            offline: new_offline_state(&db, &identity_key_bytes, &classical_secret, &pq_secret)
                .await?,
        };

        // now build bundle back
        let response = ContactResponse {
            payload_id: payload.payload_id,
            pre_key_bundle: my_bundle.serialize()?,
            peer_id: network.primary.peer_id.to_base58(),
            dilithium_pub_key,
            relay_addresses: get_listen_addrs(&network.primary.cmd_tx).await?,
            mailbox_kem_ct,
            mailbox_kem_prekey_id,
            mailbox_ephemeral_pub: Vec::new(),
        };

        let wire = WireMessage::ContactResponse(response);
        let response_bytes = bincode::serialize(&wire)?;

        let ack_rx = register_ack_waiter(payload.payload_id);

        // send off!
        let sent = network
            .primary
            .cmd_tx
            .send(SwarmCommand::SendMessage {
                peer_id: PeerId::from_str(&payload.peer_id)
                    .map_err(|err| KursalError::Identity(format!("Invalid peer_id: {err}")))?,
                data: response_bytes,
                addresses: str_to_multiaddr(&contact.known_addresses)?,
            })
            .await;

        if sent.is_err() {
            forget_ack_waiter(payload.payload_id);
            drop_half_open_session(&db, &remote_address).await;
            return Err(KursalError::Network("Could not send response".to_string()));
        }

        match tokio::time::timeout(Duration::from_secs(ACK_TIMEOUT_SECS), ack_rx).await {
            Ok(Ok(Ok(()))) => {
                contact.save(&*db.0.lock().await)?;
                Ok(contact)
            }
            Ok(Ok(Err(reason))) => {
                log::warn!("[ltc] publisher refused the handshake: {}", reason.as_str());
                drop_half_open_session(&db, &remote_address).await;
                Err(KursalError::Identity(reason.as_str().to_string()))
            }
            _ => {
                forget_ack_waiter(payload.payload_id);
                drop_half_open_session(&db, &remote_address).await;
                Err(KursalError::Network(
                    "No answer from the other device".to_string(),
                ))
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LtcPayload {
    pub payload_id: MessageId,
    pub peer_id: String,
    pub pre_key_bundle: Vec<u8>, // no one-time prekey
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
    pub created_at: u64,
    pub expires_at: u64,
}

impl LtcPayload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

async fn drop_half_open_session(db: &SharedDatabase, remote_address: &ProtocolAddress) {
    let db_lock = db.0.lock().await;
    if let Err(err) = db_lock.raw_delete(TABLE_SESSIONS, &remote_address.to_string()) {
        log::warn!("[ltc] could not drop the half-open session: {err}");
    }
}
