use crate::MapKursalResult;
use crate::first_contact::ltc::LtcState;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{DEVICE_ID, PreKeyBundleData, mailbox_kem_decapsulate, session_initiate},
    identity::UserId,
    messaging::{enums::MessageId, offline::new_offline_state},
    network::swarm::{SwarmCommand, str_to_multiaddr},
    storage::{SharedDatabase, TABLE_SETTINGS, get_timestamp_secs},
    sync::LockExt,
};
use libp2p::PeerId;
use libsignal_protocol::{
    IdentityKeyStore, KyberPreKeyId, KyberPreKeyStore, PreKeyId, PreKeyStore, ProtocolAddress,
    PublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex as StdMutex};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

pub mod ltc;
pub mod nearby;
pub mod otp;

const FC_REPLAY_CACHE_MAX: usize = 256;

static FC_REPLAY_CACHE: LazyLock<StdMutex<VecDeque<[u8; 32]>>> =
    LazyLock::new(|| StdMutex::new(VecDeque::with_capacity(FC_REPLAY_CACHE_MAX)));

fn fc_replay_contains(hash: &[u8; 32]) -> bool {
    FC_REPLAY_CACHE.lock_recover().contains(hash)
}

fn fc_replay_remember(hash: [u8; 32]) {
    let mut cache = FC_REPLAY_CACHE.lock_recover();
    if cache.contains(&hash) {
        return;
    }
    if cache.len() >= FC_REPLAY_CACHE_MAX {
        cache.pop_front();
    }
    cache.push_back(hash);
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContactResponse {
    pub payload_id: MessageId,
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
    pub mailbox_kem_ct: Vec<u8>,
    pub mailbox_kem_prekey_id: u32,
    pub mailbox_ephemeral_pub: Vec<u8>,
}

enum HandshakeKind {
    Otp,
    Ltc,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileTransferMessage {
    pub transfer_id: [u8; 16],
    pub index: u32,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FcRejectReason {
    AlreadyUsed,
    Expired,
    Unknown,
}

impl FcRejectReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            FcRejectReason::AlreadyUsed => "code already used",
            FcRejectReason::Expired => "code expired",
            FcRejectReason::Unknown => "code not recognized",
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum WireMessage {
    Encrypted(Vec<u8>),               // for normal encrypted libsignal
    ContactResponse(ContactResponse), // for OTP / LTC handshake
    FileTransfer(FileTransferMessage),
    Terminate,
    ContactAccepted(MessageId),
    ContactRejected {
        payload_id: MessageId,
        reason: FcRejectReason,
    },
}

pub type FcAck = std::result::Result<(), FcRejectReason>;

static FC_ACK_WAITERS: LazyLock<StdMutex<HashMap<[u8; 16], oneshot::Sender<FcAck>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

pub fn register_ack_waiter(payload_id: MessageId) -> oneshot::Receiver<FcAck> {
    let (tx, rx) = oneshot::channel();
    FC_ACK_WAITERS.lock_recover().insert(payload_id.0, tx);
    rx
}

pub fn forget_ack_waiter(payload_id: MessageId) {
    FC_ACK_WAITERS.lock_recover().remove(&payload_id.0);
}

pub fn resolve_ack_waiter(payload_id: MessageId, ack: FcAck) {
    let waiter = FC_ACK_WAITERS.lock_recover().remove(&payload_id.0);
    if let Some(tx) = waiter {
        let _ = tx.send(ack);
    }
}

async fn send_wire(
    wire: WireMessage,
    peer_id: &str,
    addresses: &[String],
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let peer = PeerId::from_str(peer_id).ok_kursal(KursalError::Network)?;

    cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: peer,
            data: bincode::serialize(&wire)?,
            addresses: str_to_multiaddr(addresses)?,
        })
        .await
        .ok_kursal(KursalError::Network)
}

pub fn make_username(peer_id: &str) -> String {
    let hash = Sha256::digest(peer_id.as_bytes());
    let suffix = hex::encode(&hash[..3]);

    format!("Unknown #{suffix}")
}

pub async fn handle_fc_response(
    response: ContactResponse,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let response_hash: [u8; 32] = Sha256::digest(bincode::serialize(&response)?).into();
    if fc_replay_contains(&response_hash) {
        log::warn!("[fc] ignoring replayed ContactResponse (seen recently)");
        return Ok(());
    }

    let now = get_timestamp_secs()?;

    let (handshake, otp_expired, ltc_reject) = {
        let mut otp_expired = false;
        let otp_match = match db.raw_read(TABLE_SETTINGS, "otp_pending_id")? {
            Some(id) if id.as_slice() == response.payload_id.0.as_slice() => {
                let published_at: u64 = db
                    .raw_read(TABLE_SETTINGS, "otp_published_at")?
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(0);
                let fresh = now.saturating_sub(published_at) <= 600;
                otp_expired = !fresh;
                fresh
            }
            _ => false,
        };

        let mut ltc_reject = None;
        let ltc_match = match LtcState::load(&db)? {
            Some(state) if state.payload_id == response.payload_id => {
                if now > state.expires_at {
                    ltc_reject = Some(FcRejectReason::Expired);
                    false
                } else if state.max_uses.is_some_and(|max| state.uses >= max) {
                    ltc_reject = Some(FcRejectReason::AlreadyUsed);
                    false
                } else {
                    true
                }
            }
            _ => false,
        };

        let kind = if otp_match {
            Some(HandshakeKind::Otp)
        } else if ltc_match {
            Some(HandshakeKind::Ltc)
        } else {
            None
        };

        (kind, otp_expired, ltc_reject)
    };

    let Some(handshake) = handshake else {
        let consumed = db
            .raw_read(TABLE_SETTINGS, "otp_consumed_id")?
            .is_some_and(|id| id.as_slice() == response.payload_id.0.as_slice());

        let reason = ltc_reject.unwrap_or(if consumed {
            FcRejectReason::AlreadyUsed
        } else if otp_expired {
            FcRejectReason::Expired
        } else {
            FcRejectReason::Unknown
        });

        log::warn!(
            "Rejected incoming ContactResponse: {} (payload matches no pending handshake)",
            reason.as_str()
        );

        let rejection = WireMessage::ContactRejected {
            payload_id: response.payload_id,
            reason,
        };
        let _ = send_wire(
            rejection,
            &response.peer_id,
            &response.relay_addresses,
            cmd_tx,
        )
        .await;

        return Ok(());
    };

    let bundle = PreKeyBundleData::deserialize(&response.pre_key_bundle)?;
    let identity_key_bytes = bundle.identity_key.public_key().serialize().to_vec();

    let user_id = UserId(Sha256::digest(&identity_key_bytes).into());

    let already_exists = Contact::load(&db, &user_id)?.is_some();
    if already_exists {
        log::warn!(
            "[fc] ignoring ContactResponse for already-established contact {} (replay or duplicate)",
            hex::encode(user_id.0)
        );
        let _ = send_wire(
            WireMessage::ContactAccepted(response.payload_id),
            &response.peer_id,
            &response.relay_addresses,
            cmd_tx,
        )
        .await;
        return Ok(());
    }

    let bob_address = ProtocolAddress::new(hex::encode(user_id.0), DEVICE_ID);
    session_initiate(db.clone(), bundle, &bob_address).await?;

    let pq_secret = if response.mailbox_kem_ct.is_empty() {
        Zeroizing::new(Vec::new())
    } else {
        let record = db
            .get_kyber_pre_key(KyberPreKeyId::from(response.mailbox_kem_prekey_id))
            .await
            .ok_kursal(KursalError::Crypto)?;
        let secret_key = record.secret_key().ok_kursal(KursalError::Crypto)?;
        mailbox_kem_decapsulate(&secret_key, &response.mailbox_kem_ct)?
    };

    let classical_secret = match handshake {
        HandshakeKind::Otp => {
            if response.mailbox_ephemeral_pub.is_empty() {
                log::warn!("Rejected incoming ContactResponse: missing mailbox ephemeral");
                return Ok(());
            }
            let prekey_id = db
                .raw_read(TABLE_SETTINGS, "otp_prekey_id")?
                .and_then(|b| b.try_into().ok().map(u32::from_be_bytes))
                .ok_or_else(|| KursalError::Crypto("No OTP mailbox prekey id".to_string()))?;
            let record = db
                .get_pre_key(PreKeyId::from(prekey_id))
                .await
                .ok_kursal(KursalError::Crypto)?;
            let ephemeral_pub = PublicKey::deserialize(&response.mailbox_ephemeral_pub)
                .ok_kursal(KursalError::Crypto)?;
            Zeroizing::new(
                record
                    .private_key()
                    .ok_kursal(KursalError::Crypto)?
                    .calculate_agreement(&ephemeral_pub)
                    .ok_kursal(KursalError::Crypto)?,
            )
        }
        HandshakeKind::Ltc => {
            let keypair = db
                .get_identity_key_pair()
                .await
                .ok_kursal(KursalError::Crypto)?;
            let peer_pub =
                PublicKey::deserialize(&identity_key_bytes).ok_kursal(KursalError::Crypto)?;
            Zeroizing::new(
                keypair
                    .private_key()
                    .calculate_agreement(&peer_pub)
                    .ok_kursal(KursalError::Crypto)?,
            )
        }
    };

    let contact = Contact {
        user_id: user_id.clone(),
        peer_id: response.peer_id.clone(),
        display_name: make_username(&response.peer_id),
        avatar: None,
        identity_pub_key: identity_key_bytes.clone(),
        dilithium_pub_key: response.dilithium_pub_key.clone(),
        known_addresses: response.relay_addresses.clone(),
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: now,
        offline: new_offline_state(&db, &identity_key_bytes, &classical_secret, &pq_secret).await?,
    };

    let (otp_dht_key, ltc_denied, ltc_status) = {
        let mut ltc_denied = false;
        let mut ltc_status = None;
        if matches!(handshake, HandshakeKind::Ltc) {
            match LtcState::load(&db)? {
                Some(mut state)
                    if state.payload_id == response.payload_id
                        && now <= state.expires_at
                        && state.max_uses.is_none_or(|max| state.uses < max) =>
                {
                    state.uses += 1;
                    state.save(&db)?;
                    ltc_status = Some(state.dto_serialize()?);
                }
                _ => ltc_denied = true,
            }
        }

        if ltc_denied {
            (None, true, None)
        } else {
            contact.save(&db)?;
            crate::storage::set_contact_terminated(&db, &hex::encode(user_id.0), false)?;

            let dht_key = db.raw_read(TABLE_SETTINGS, "otp_dht_key")?;
            db.raw_write(TABLE_SETTINGS, "otp_consumed_id", &response.payload_id.0)?;
            db.raw_delete(TABLE_SETTINGS, "otp_pending_id")?;
            db.raw_delete(TABLE_SETTINGS, "otp_published_at")?;
            db.raw_delete(TABLE_SETTINGS, "otp_prekey_id")?;
            db.raw_delete(TABLE_SETTINGS, "otp_dht_key")?;

            (dht_key, false, ltc_status)
        }
    };

    if ltc_denied {
        let _ = send_wire(
            WireMessage::ContactRejected {
                payload_id: response.payload_id,
                reason: FcRejectReason::AlreadyUsed,
            },
            &response.peer_id,
            &response.relay_addresses,
            cmd_tx,
        )
        .await;
        return Ok(());
    }

    let _ = send_wire(
        WireMessage::ContactAccepted(response.payload_id),
        &response.peer_id,
        &response.relay_addresses,
        cmd_tx,
    )
    .await;

    if let Some(key) = otp_dht_key {
        let _ = cmd_tx.send(SwarmCommand::RemoveDht { key }).await;
    }

    cmd_tx
        .send(SwarmCommand::ContactAdded {
            contact: contact.clone(),
        })
        .await
        .ok_kursal(KursalError::Network)?;

    event_tx
        .send(AppEvent::ContactAdded { contact })
        .await
        .ok_kursal(KursalError::Network)?;

    if matches!(handshake, HandshakeKind::Otp) {
        let _ = event_tx.send(AppEvent::OtpConsumed).await;
    }

    if let Some(status) = ltc_status {
        let _ = event_tx
            .send(AppEvent::LtcUpdated {
                status: Some(status),
            })
            .await;
    }

    fc_replay_remember(response_hash);
    Ok(())
}
