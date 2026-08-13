use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{
        DEVICE_ID, PreKeyBundleData, mailbox_kem_decapsulate, mailbox_kem_encapsulate,
        session_initiate,
    },
    first_contact::make_username,
    identity::UserId,
    messaging::offline::new_offline_state,
    network::swarm::{SwarmCommand, get_listen_addrs},
    storage::{SharedDatabase, get_dilithium_pub, get_timestamp_secs},
};
use libsignal_protocol::{
    KeyPair, KyberPreKeyId, KyberPreKeyStore, PreKeyStore, ProtocolAddress, PublicKey,
};
use rand::{Rng, TryRngCore, distr::Uniform, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use utoipa::ToSchema;
use zeroize::Zeroizing;

#[cfg(feature = "ble")]
pub mod bluetooth;
pub mod mdns;

#[cfg(not(feature = "ble"))]
pub mod bluetooth {
    use super::{BtEvent, NearbyBeacon, NearbyMessage, NearbyTransport};
    use crate::{KursalError, Result, network::swarm::SwarmCommand};
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::{Mutex, mpsc};

    pub struct BTTransport {
        pub cmd_tx: mpsc::Sender<SwarmCommand>,
        pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
        pub pending_handshakes: Arc<Mutex<HashMap<String, mpsc::Sender<NearbyMessage>>>>,
        pub bt_event_tx: mpsc::Sender<BtEvent>,
    }

    impl BTTransport {
        pub fn new(
            cmd_tx: mpsc::Sender<SwarmCommand>,
            my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
            bt_event_tx: mpsc::Sender<BtEvent>,
        ) -> Self {
            Self {
                cmd_tx,
                my_beacon,
                pending_handshakes: Arc::new(Mutex::new(HashMap::new())),
                bt_event_tx,
            }
        }
    }

    #[async_trait::async_trait]
    impl NearbyTransport for BTTransport {
        async fn start(&self, _beacon: NearbyBeacon) {
            log::info!("[bt] built without ble feature, bluetooth transport disabled");
        }

        async fn stop(&self) {}

        async fn send(&self, _peer_id: &str, _msg: NearbyMessage) -> Result<()> {
            Err(KursalError::Network(
                "bluetooth disabled in this build".to_string(),
            ))
        }

        async fn register_handshake(&self, _peer_id: &str) -> mpsc::Receiver<NearbyMessage> {
            let (tx, rx) = mpsc::channel(1);
            drop(tx);
            rx
        }

        async fn unregister_handshake(&self, _peer_id: &str) {}
    }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, ToSchema)]
pub enum NearbyOrigin {
    Bluetooth,
    mDNS,
}

const ANIMALS: &str = include_str!("nearby_animals.txt");
const ADJECTIVES: &str = include_str!("nearby_adjectives.txt");

#[derive(Serialize, Deserialize, Clone)]
pub struct NearbyBeacon {
    pub peer_id: String,
    pub session_name: String,
}

impl NearbyBeacon {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub fn generate_session_name() -> Result<String> {
    let mut os_rng = OsRng;
    let mut rng = os_rng.unwrap_mut();

    let animallist: Vec<&str> = ANIMALS.lines().filter(|s| !s.is_empty()).collect();
    let dist_animal = Uniform::new(0, animallist.len()).ok_kursal(KursalError::Crypto)?;

    let adjlist: Vec<&str> = ADJECTIVES.lines().filter(|s| !s.is_empty()).collect();
    let dist_adj = Uniform::new(0, adjlist.len()).ok_kursal(KursalError::Crypto)?;

    let result = format!(
        "{} {}",
        adjlist
            .get(rng.sample(dist_adj))
            .ok_or(KursalError::Crypto(
                "Could not generate a random word".to_string()
            ))?,
        animallist
            .get(rng.sample(dist_animal))
            .ok_or(KursalError::Crypto(
                "Could not generate a random word".to_string()
            ))?,
    );

    Ok(result)
}

#[derive(Serialize, Deserialize)]
pub enum NearbyMessage {
    ConnectRequest {
        from_session_name: String,
    },
    ConnectDecline,
    ConnectAccept {
        bundle: Vec<u8>,
        dilithium_pub: Vec<u8>,
        relay_addresses: Vec<String>,
    },
    BundleReply {
        bundle: Vec<u8>,
        dilithium_pub: Vec<u8>,
        relay_addresses: Vec<String>,
        mailbox_kem_ct: Vec<u8>,
        mailbox_ephemeral_pub: Vec<u8>,
    },
}

impl NearbyMessage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[allow(async_fn_in_trait)] // trust bro
#[async_trait::async_trait]
pub trait NearbyTransport: Send + Sync {
    async fn start(&self, beacon: NearbyBeacon);
    async fn stop(&self);
    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> Result<()>;
    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage>;
    async fn unregister_handshake(&self, peer_id: &str);
}

#[derive(Serialize, Deserialize)]
pub enum NearbyPacket {
    Beacon(NearbyBeacon),
    BeaconAck(NearbyBeacon),
    Message(NearbyMessage),
}

impl NearbyPacket {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub enum NearbyRouteResult {
    NotNearby,
    HandledInternally,
    IncomingRequest {
        peer_id: String,
        session_name: String,
    },
}

// Events emitted by BTTransport into dispatch_events
pub enum BtEvent {
    Beacon {
        peer_id: String,
        beacon: NearbyBeacon,
    },
    Message {
        from_peer_id: String,
        msg: NearbyMessage,
    },
}

pub async fn handle_nearby_request(
    from_peer_id: &str,
    transport: &dyn NearbyTransport,
    user_decision: oneshot::Receiver<bool>,
    db: SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let decision = match tokio::time::timeout(Duration::from_secs(300), user_decision).await {
        Ok(Ok(decision)) => decision,
        _ => {
            // timeout or sender dropped
            let _ = transport
                .send(from_peer_id, NearbyMessage::ConnectDecline)
                .await;
            return Ok(());
        }
    };

    if !decision {
        if let Err(err) = transport
            .send(from_peer_id, NearbyMessage::ConnectDecline)
            .await
        {
            log::warn!("[nearby] decline send to {from_peer_id} failed: {err}");
            let _ = event_tx
                .send(AppEvent::BackendSignal {
                    signal: "nearby_decline_send_failed".to_string(),
                    payload: format!("{from_peer_id}: {err}"),
                })
                .await;
        }
        return Ok(());
    }

    let now = get_timestamp_secs()?;

    let mut rx = transport.register_handshake(from_peer_id).await;

    let dilithium_pub_key = get_dilithium_pub(&db)?;

    let my_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
    let mailbox_kem_prekey_id: u32 = my_bundle.kyber_pre_key_id.into();
    let mailbox_dh_prekey_id = my_bundle
        .pre_key_id
        .ok_or_else(|| KursalError::Crypto("Nearby bundle missing one-time prekey".to_string()))?;
    let my_bundle_serialized = my_bundle.serialize()?;

    transport
        .send(
            from_peer_id,
            NearbyMessage::ConnectAccept {
                bundle: my_bundle_serialized,
                dilithium_pub: dilithium_pub_key.clone(),
                relay_addresses: get_listen_addrs(cmd_tx).await?,
            },
        )
        .await?;

    let result = match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
        Ok(Some(NearbyMessage::BundleReply {
            bundle,
            dilithium_pub,
            relay_addresses,
            mailbox_kem_ct,
            mailbox_ephemeral_pub,
        })) => {
            let bundle = PreKeyBundleData::deserialize(&bundle)?;
            let identity_pub_key = bundle.identity_key.public_key().serialize().to_vec();

            let user_id = UserId(Sha256::digest(&identity_pub_key).into());
            let address = ProtocolAddress::new(hex::encode(user_id.0), DEVICE_ID);

            session_initiate(db.clone(), bundle, &address).await?;

            let pq_secret = if mailbox_kem_ct.is_empty() {
                Zeroizing::new(Vec::new())
            } else {
                let record = db
                    .get_kyber_pre_key(KyberPreKeyId::from(mailbox_kem_prekey_id))
                    .await
                    .ok_kursal(KursalError::Crypto)?;
                let secret_key = record.secret_key().ok_kursal(KursalError::Crypto)?;
                mailbox_kem_decapsulate(&secret_key, &mailbox_kem_ct)?
            };

            if mailbox_ephemeral_pub.is_empty() {
                return Err(KursalError::Crypto(
                    "Nearby reply missing mailbox ephemeral".to_string(),
                ));
            }
            let prekey_record = db
                .get_pre_key(mailbox_dh_prekey_id)
                .await
                .ok_kursal(KursalError::Crypto)?;
            let ephemeral_pub =
                PublicKey::deserialize(&mailbox_ephemeral_pub).ok_kursal(KursalError::Crypto)?;
            let classical_secret = Zeroizing::new(
                prekey_record
                    .private_key()
                    .ok_kursal(KursalError::Crypto)?
                    .calculate_agreement(&ephemeral_pub)
                    .ok_kursal(KursalError::Crypto)?,
            );

            let contact = Contact {
                user_id,
                peer_id: from_peer_id.to_string(),
                display_name: make_username(from_peer_id),
                avatar: None,
                identity_pub_key: identity_pub_key.clone(),
                dilithium_pub_key: dilithium_pub.clone(),
                known_addresses: relay_addresses,
                verified: false,
                profile_shared: false,
                blocked: false,
                created_at: now,
                offline: new_offline_state(&db, &identity_pub_key, &classical_secret, &pq_secret)
                    .await?,
            };

            contact.save(&db)?;

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

            Ok(())
        }
        _ => Err(KursalError::Network("No bundle reply received".to_string())),
    };

    transport.unregister_handshake(from_peer_id).await;
    result
}

pub async fn nearby_connect(
    peer_id: &str,
    my_session_name: &str,
    transport: &dyn NearbyTransport,
    db: SharedDatabase,
    event_tx: &mpsc::Sender<AppEvent>,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let now = get_timestamp_secs()?;
    let mut rx = transport.register_handshake(peer_id).await;

    transport
        .send(
            peer_id,
            NearbyMessage::ConnectRequest {
                from_session_name: my_session_name.to_string(),
            },
        )
        .await?;

    let result = match tokio::time::timeout(Duration::from_secs(60), rx.recv()).await {
        Ok(Some(NearbyMessage::ConnectAccept {
            bundle,
            dilithium_pub,
            relay_addresses,
        })) => {
            let bundle = PreKeyBundleData::deserialize(&bundle)?;
            let identity_pub_key = bundle.identity_key.public_key().serialize().to_vec();

            let user_id = UserId(Sha256::digest(&identity_pub_key).into());
            let address = ProtocolAddress::new(hex::encode(user_id.0), DEVICE_ID);

            let mailbox_kem_pub = bundle.kyber_pre_key_public.serialize().to_vec();
            let mailbox_opk_pub = bundle.pre_key_public.ok_or_else(|| {
                KursalError::Crypto("Nearby bundle missing one-time prekey".to_string())
            })?;
            session_initiate(db.clone(), bundle, &address).await?;
            let (pq_secret, mailbox_kem_ct) = mailbox_kem_encapsulate(&mailbox_kem_pub)?;

            let mut key_rng = OsRng.unwrap_err();
            let mailbox_ephemeral = KeyPair::generate(&mut key_rng);
            let classical_secret = Zeroizing::new(
                mailbox_ephemeral
                    .private_key
                    .calculate_agreement(&mailbox_opk_pub)
                    .ok_kursal(KursalError::Crypto)?,
            );
            let mailbox_ephemeral_pub = mailbox_ephemeral.public_key.serialize().to_vec();

            let our_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
            let our_dilithium = get_dilithium_pub(&db)?;

            let contact = Contact {
                user_id,
                peer_id: peer_id.to_string(),
                display_name: make_username(peer_id),
                avatar: None,
                identity_pub_key: identity_pub_key.clone(),
                dilithium_pub_key: dilithium_pub.clone(),
                known_addresses: relay_addresses,
                verified: false,
                profile_shared: false,
                blocked: false,
                created_at: now,
                offline: new_offline_state(&db, &identity_pub_key, &classical_secret, &pq_secret)
                    .await?,
            };

            // send back
            transport
                .send(
                    peer_id,
                    NearbyMessage::BundleReply {
                        bundle: our_bundle.serialize()?,
                        dilithium_pub: our_dilithium,
                        relay_addresses: get_listen_addrs(cmd_tx).await?,
                        mailbox_kem_ct,
                        mailbox_ephemeral_pub,
                    },
                )
                .await?;

            contact.save(&db)?;

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

            Ok(())
        }
        Ok(Some(NearbyMessage::ConnectDecline)) => {
            Err(KursalError::Network(
                "Connection declined by peer".to_string(),
            )) // not sure but i think no one emits this for more privacy
        }
        _ => Err(KursalError::Network("No response received".to_string())),
    };

    transport.unregister_handshake(peer_id).await;
    result
}
