use crate::MapKursalResult;
use crate::dto::{LtcPointerState, LtcStatusDto};
use crate::storage::Database;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{
        PreKeyBundleData,
        dilithium::{dilithium_sign, dilithium_verify},
        mailbox_kem_encapsulate, session_initiate,
    },
    first_contact::{
        ContactResponse, WireMessage, forget_ack_waiter, make_username, register_ack_waiter,
    },
    identity::UserId,
    messaging::{enums::MessageId, offline::new_offline_state},
    network::{
        dht::DHTRecord,
        kademlia::KAD_LONG_MAX_AGE,
        swarm::{SwarmCommand, SwarmHandle, get_listen_addrs, str_to_multiaddr},
    },
    storage::{
        SharedDatabase, TABLE_KYBER_PRE_KEYS, TABLE_LTC_CACHE, TABLE_SESSIONS, TABLE_SETTINGS,
        file::KursalFile, get_dilithium_pub, get_dilithium_secret, get_timestamp_secs,
    },
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, IdentityKeyStore, ProtocolAddress, PublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

const ACK_TIMEOUT_SECS: u64 = 20;
const LTC_RV_DOMAIN: &[u8] = b"kursal-ltc-rv01";
const POINTER_PUT_TIMEOUT_SECS: u64 = 90;
const POINTER_FETCH_TIMEOUT_SECS: u64 = 15;

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
    pub follow_rotation: bool,
    pub pointer_published_at: Option<u64>,
    pub pointer_failed: bool,
}

impl LtcState {
    pub async fn create(
        db: SharedDatabase,
        cmd_tx: &mpsc::Sender<SwarmCommand>,
        max_uses: Option<u32>,
        ttl_secs: Option<u64>,
    ) -> Result<Self> {
        let bundle = PreKeyBundleData::build_pre_key_bundle_noprekey(db.clone()).await?;
        let kyber_pre_key_id: u32 = bundle.kyber_pre_key_id.into();

        {
            Self::revoke_ltc(db.clone(), cmd_tx).await.ok();
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
            follow_rotation: true,
            pointer_published_at: None,
            pointer_failed: false,
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
        let Some(raw): Option<Vec<u8>> = db.raw_read(TABLE_LTC_CACHE, "ltc_current")? else {
            return Ok(None);
        };

        match Self::deserialize(&raw) {
            Ok(state) => Ok(Some(state)),
            Err(err) => {
                log::warn!("[ltc] dropping unreadable stored code: {err}");
                db.raw_delete(TABLE_LTC_CACHE, "ltc_current")?;
                Ok(None)
            }
        }
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
            follow_rotations: self.follow_rotation,
            pointer_state: self.pointer_state(),
        };

        Ok(dto)
    }

    pub fn pointer_state(&self) -> LtcPointerState {
        if !self.follow_rotation {
            return LtcPointerState::Disabled;
        }
        if self.pointer_failed {
            return LtcPointerState::Failed;
        }

        let now = get_timestamp_secs().unwrap_or(0);

        match self.pointer_published_at {
            Some(at) if now.saturating_sub(at) < KAD_LONG_MAX_AGE => LtcPointerState::Published,
            _ => LtcPointerState::Pending,
        }
    }

    pub fn rendezvous_tag(&self) -> [u8; 32] {
        ltc_rendezvous_tag(&self.payload_id, &self.dilithium_pub_key)
    }

    pub async fn set_follow_rotations(
        db: SharedDatabase,
        cmd_tx: &mpsc::Sender<SwarmCommand>,
        enabled: bool,
    ) -> Result<LtcStatusDto> {
        let (dto, stale_tag) = {
            let lock = db.0.lock().await;

            let mut state = Self::load(&lock)?
                .ok_or(KursalError::Storage("No LTC currently stored".to_string()))?;

            state.follow_rotation = enabled;
            let stale_tag = (!enabled).then(|| state.rendezvous_tag());
            if !enabled {
                state.pointer_published_at = None;
                state.pointer_failed = false;
            }
            state.save(&lock)?;

            (state.dto_serialize()?, stale_tag)
        };

        if let Some(tag) = stale_tag {
            let _ = cmd_tx
                .send(SwarmCommand::RemoveDht { key: tag.to_vec() })
                .await;
        }

        Ok(dto)
    }

    pub fn is_expired(&self) -> bool {
        let now = get_timestamp_secs().unwrap_or(u64::MAX);
        now > self.expires_at
    }

    pub async fn revoke_ltc(db: SharedDatabase, cmd_tx: &mpsc::Sender<SwarmCommand>) -> Result<()> {
        let stale_tag = {
            let lock = db.0.lock().await;
            let loaded = Self::load(&lock);

            let mut stale_tag = None;
            if let Ok(Some(previous)) = loaded {
                lock.raw_delete(
                    TABLE_KYBER_PRE_KEYS,
                    &format!("kyber_prekey_{}", previous.kyber_pre_key_id),
                )?;
                lock.raw_delete(
                    TABLE_SETTINGS,
                    &format!("kyber_lastresort_{}", previous.kyber_pre_key_id),
                )?;
                stale_tag = Some(previous.rendezvous_tag());
            }

            lock.raw_delete(TABLE_LTC_CACHE, "ltc_current")?;

            stale_tag
        };

        // Only clears our own replica. Remote copies age out on KAD_LONG_MAX_AGE,
        // and a revoked code is refused at handshake time anyway.
        if let Some(tag) = stale_tag {
            let _ = cmd_tx
                .send(SwarmCommand::RemoveDht { key: tag.to_vec() })
                .await;
        }

        Ok(())
    }

    pub async fn export_ltc(db: SharedDatabase, swarm: &SwarmHandle) -> Result<Vec<u8>> {
        let state = {
            let db_lock = db.0.lock().await;
            Self::load(&db_lock)?
                .ok_or(KursalError::Storage("No LTC currently stored".to_string()))?
        };

        let peer_id = swarm.peer_id.to_base58();
        let relay_addresses = get_listen_addrs(&swarm.cmd_tx).await?;

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
        swarm: &SwarmHandle,
    ) -> Result<Contact> {
        if payload.peer_id == swarm.peer_id.to_base58() {
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

        // now build bundle back
        let response = ContactResponse {
            payload_id: payload.payload_id,
            pre_key_bundle: my_bundle.serialize()?,
            peer_id: swarm.peer_id.to_base58(),
            dilithium_pub_key,
            relay_addresses: get_listen_addrs(&swarm.cmd_tx).await?,
            mailbox_kem_ct,
            mailbox_kem_prekey_id,
            mailbox_ephemeral_pub: Vec::new(),
        };

        let wire = WireMessage::ContactResponse(response);
        let response_bytes = bincode::serialize(&wire)?;

        let tag = ltc_rendezvous_tag(&payload.payload_id, &payload.dilithium_pub_key);

        // The rendezvous lookup runs alongside the first attempt so the retry can
        // start the moment the baked-in peer times out.
        let (attempt, pointer) = tokio::join!(
            deliver_response(
                &payload.peer_id,
                &payload.relay_addresses,
                payload.payload_id,
                &response_bytes,
                &swarm.cmd_tx,
            ),
            fetch_ltc_pointer(
                &tag,
                &payload.dilithium_pub_key,
                payload.payload_id,
                &swarm.cmd_tx,
            ),
        );

        let (peer_id, known_addresses) = match attempt {
            Ok(()) => (payload.peer_id.clone(), payload.relay_addresses.clone()),
            // The publisher answered and refused. Retrying finds the same answer.
            Err(refused @ KursalError::Identity(_)) => {
                drop_half_open_session(&db, &remote_address).await;
                return Err(refused);
            }
            Err(unreachable) => {
                let Some(pointer) = pointer.ok().flatten() else {
                    drop_half_open_session(&db, &remote_address).await;
                    return Err(unreachable);
                };

                if pointer.peer_id == swarm.peer_id.to_base58() {
                    drop_half_open_session(&db, &remote_address).await;
                    return Err(KursalError::Network(
                        "Cannot add yourself as a contact".to_string(),
                    ));
                }

                log::info!(
                    "[ltc] {} unreachable, retrying at rendezvous peer {}",
                    payload.peer_id,
                    pointer.peer_id
                );

                if let Err(err) = deliver_response(
                    &pointer.peer_id,
                    &pointer.relay_addresses,
                    payload.payload_id,
                    &response_bytes,
                    &swarm.cmd_tx,
                )
                .await
                {
                    drop_half_open_session(&db, &remote_address).await;
                    return Err(err);
                }

                (pointer.peer_id, pointer.relay_addresses)
            }
        };

        let contact = Contact {
            user_id,
            display_name: make_username(&peer_id),
            peer_id,
            known_addresses,
            avatar_bytes: None,
            identity_pub_key: identity_key_bytes.clone(),
            dilithium_pub_key: payload.dilithium_pub_key.clone(),
            verified: false,
            profile_shared: false,
            blocked: false,
            created_at: now,
            offline: new_offline_state(&db, &identity_key_bytes, &classical_secret, &pq_secret)
                .await?,
        };

        contact.save(&*db.0.lock().await)?;

        Ok(contact)
    }

    pub async fn publish_pointer(
        db: SharedDatabase,
        swarm: SwarmHandle,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Result<()> {
        let (tag, payload_id) = {
            let lock = db.0.lock().await;
            match Self::load(&lock)? {
                Some(state) if state.follow_rotation => (state.rendezvous_tag(), state.payload_id),
                _ => return Ok(()),
            }
        };

        let relay_addresses = get_listen_addrs(&swarm.cmd_tx).await?;
        let seq = get_timestamp_secs()?;

        let mut pointer = LtcPointer {
            payload_id,
            peer_id: swarm.peer_id.to_base58(),
            relay_addresses,
            seq,
            signature: Vec::new(),
        };

        let secret = get_dilithium_secret(&*db.0.lock().await)?;
        pointer.sign(&tag, secret)?;

        let record = DHTRecord::new(tag.to_vec(), pointer.serialize()?, seq, true).await?;
        let (reply_tx, reply_rx) = oneshot::channel();

        swarm
            .cmd_tx
            .send(SwarmCommand::PublishDht {
                key: tag.to_vec(),
                value: record.serialize()?,
                expires: Some(KAD_LONG_MAX_AGE),
                reply_tx: Some(reply_tx),
            })
            .await
            .ok_kursal(KursalError::Network)?;

        let landed = matches!(
            tokio::time::timeout(Duration::from_secs(POINTER_PUT_TIMEOUT_SECS), reply_rx).await,
            Ok(Ok(true))
        );

        let status = {
            let lock = db.0.lock().await;
            match Self::load(&lock)? {
                // Replaced or revoked while the put was in flight.
                Some(mut state) if state.payload_id == payload_id => {
                    state.pointer_failed = !landed;
                    if landed {
                        state.pointer_published_at = Some(seq);
                    }
                    state.save(&lock)?;
                    Some(state.dto_serialize()?)
                }
                _ => None,
            }
        };

        if let Some(status) = status {
            let _ = event_tx
                .send(AppEvent::LtcUpdated {
                    status: Some(status),
                })
                .await;
        }

        Ok(())
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

async fn deliver_response(
    peer_id: &str,
    addresses: &[String],
    payload_id: MessageId,
    response_bytes: &[u8],
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let peer = PeerId::from_str(peer_id)
        .map_err(|err| KursalError::Network(format!("Invalid peer_id: {err}")))?;
    let addresses = str_to_multiaddr(addresses)
        .map_err(|err| KursalError::Network(format!("Invalid addresses: {err}")))?;

    let ack_rx = register_ack_waiter(payload_id);

    let sent = cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: peer,
            data: response_bytes.to_vec(),
            addresses,
        })
        .await;

    if sent.is_err() {
        forget_ack_waiter(payload_id);
        return Err(KursalError::Network("Could not send response".to_string()));
    }

    match tokio::time::timeout(Duration::from_secs(ACK_TIMEOUT_SECS), ack_rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(reason))) => {
            log::warn!("[ltc] publisher refused the handshake: {}", reason.as_str());
            Err(KursalError::Identity(reason.as_str().to_string()))
        }
        _ => {
            forget_ack_waiter(payload_id);
            Err(KursalError::Network(
                "No answer from the other device".to_string(),
            ))
        }
    }
}

async fn drop_half_open_session(db: &SharedDatabase, remote_address: &ProtocolAddress) {
    let db_lock = db.0.lock().await;
    if let Err(err) = db_lock.raw_delete(TABLE_SESSIONS, &remote_address.to_string()) {
        log::warn!("[ltc] could not drop the half-open session: {err}");
    }
}

pub fn ltc_rendezvous_tag(payload_id: &MessageId, dilithium_pub_key: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();

    h.update(LTC_RV_DOMAIN);
    h.update(payload_id.0);
    h.update(dilithium_pub_key);

    h.finalize().into()
}

#[derive(Serialize, Deserialize)]
pub struct LtcPointer {
    pub payload_id: MessageId,
    pub peer_id: String,
    pub relay_addresses: Vec<String>,
    pub seq: u64,
    pub signature: Vec<u8>,
}

impl LtcPointer {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn sign(&mut self, tag: &[u8; 32], secret: Vec<u8>) -> Result<()> {
        self.signature = dilithium_sign(secret, &signing_bytes(tag, self))?;

        Ok(())
    }

    pub fn verify(&self, tag: &[u8; 32], dilithium_pub_key: &[u8]) -> bool {
        dilithium_verify(
            dilithium_pub_key,
            &signing_bytes(tag, self),
            &self.signature,
        )
        .unwrap_or(false)
    }
}

fn signing_bytes(tag: &[u8; 32], p: &LtcPointer) -> Vec<u8> {
    let mut out = Vec::new();

    out.extend_from_slice(LTC_RV_DOMAIN);
    out.extend_from_slice(tag);
    out.extend_from_slice(&p.payload_id.0);
    out.extend_from_slice(&(p.peer_id.len() as u64).to_le_bytes());
    out.extend_from_slice(p.peer_id.as_bytes());
    out.extend_from_slice(&(p.relay_addresses.len() as u64).to_le_bytes());
    for addr in &p.relay_addresses {
        out.extend_from_slice(&(addr.len() as u64).to_le_bytes());
        out.extend_from_slice(addr.as_bytes());
    }
    out.extend_from_slice(&p.seq.to_le_bytes());

    out
}

pub async fn fetch_ltc_pointer(
    tag: &[u8; 32],
    dilithium_pub_key: &[u8],
    payload_id: MessageId,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<Option<LtcPointer>> {
    let (reply_tx, mut reply_rx) = mpsc::channel(16);

    cmd_tx
        .send(SwarmCommand::FetchDht {
            key: tag.to_vec(),
            reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    let mut best: Option<LtcPointer> = None;

    let _ = tokio::time::timeout(Duration::from_secs(POINTER_FETCH_TIMEOUT_SECS), async {
        while let Some(bytes) = reply_rx.recv().await {
            let Ok(inner) = DHTRecord::deserialize(tag, &bytes) else {
                continue;
            };
            let Ok(pointer) = LtcPointer::deserialize(&inner) else {
                continue;
            };
            if pointer.payload_id != payload_id {
                continue;
            }
            if !pointer.verify(tag, dilithium_pub_key) {
                log::warn!("[ltc] discarding rendezvous record with a bad signature");
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|current| pointer.seq > current.seq)
            {
                best = Some(pointer);
            }
        }
    })
    .await;

    Ok(best)
}
