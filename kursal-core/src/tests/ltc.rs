use crate::{
    contacts::Contact,
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::{
        ContactResponse, FcRejectReason, WireMessage, handle_fc_response,
        ltc::{LtcPayload, LtcPointer, LtcState, fetch_ltc_pointer, ltc_rendezvous_tag},
        resolve_ack_waiter,
    },
    identity::{
        UserId,
        generators::{generate_dilithium_keypair, generate_identity_keypair},
    },
    messaging::enums::MessageId,
    network::{
        dht::DHTRecord,
        swarm::{SwarmCommand, SwarmHandle},
    },
    storage::{
        Database, RelayConfig, SharedDatabase, TABLE_KYBER_PRE_KEYS, get_dilithium_pub,
        get_dilithium_secret, get_timestamp_secs,
    },
    tests::TestEnv,
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::mpsc;

async fn make_peer(env: &TestEnv, name: &str) -> SharedDatabase {
    let mut db = Database::open(&env.db_path(name), [0u8; 32]).unwrap();
    generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    SharedDatabase::from_db(db)
}

async fn contact_response(peer: &SharedDatabase, payload_id: MessageId) -> ContactResponse {
    let bundle = PreKeyBundleData::build_pre_key_bundle(peer.clone())
        .await
        .unwrap();

    ContactResponse {
        payload_id,
        pre_key_bundle: bundle.serialize().unwrap(),
        peer_id: PeerId::random().to_base58(),
        dilithium_pub_key: get_dilithium_pub(&*peer.0.lock().await).unwrap(),
        relay_addresses: vec![],
        mailbox_kem_ct: vec![],
        mailbox_kem_prekey_id: 0,
        mailbox_ephemeral_pub: vec![],
    }
}

fn responder_id(response: &ContactResponse) -> UserId {
    let bundle = PreKeyBundleData::deserialize(&response.pre_key_bundle).unwrap();
    let identity = bundle.identity_key.public_key().serialize().to_vec();

    UserId(Sha256::digest(&identity).into())
}

async fn contact_saved(db: &SharedDatabase, response: &ContactResponse) -> bool {
    Contact::load(&*db.0.lock().await, &responder_id(response))
        .unwrap()
        .is_some()
}

async fn load_state(db: &SharedDatabase) -> Option<LtcState> {
    LtcState::load(&*db.0.lock().await).unwrap()
}

async fn uses(db: &SharedDatabase) -> u32 {
    load_state(db).await.unwrap().uses
}

fn last_wire(rx: &mut mpsc::Receiver<SwarmCommand>) -> Option<WireMessage> {
    let mut wire = None;
    while let Ok(cmd) = rx.try_recv() {
        if let SwarmCommand::SendMessage { data, .. } = cmd {
            wire = bincode::deserialize::<WireMessage>(&data).ok();
        }
    }

    wire
}

fn assert_rejected(rx: &mut mpsc::Receiver<SwarmCommand>, expected: FcRejectReason) {
    match last_wire(rx) {
        Some(WireMessage::ContactRejected { reason, .. }) => assert_eq!(reason, expected),
        other => panic!("expected a rejection, got {}", wire_name(other)),
    }
}

fn assert_accepted(rx: &mut mpsc::Receiver<SwarmCommand>) {
    match last_wire(rx) {
        Some(WireMessage::ContactAccepted(_)) => {}
        other => panic!("expected an acceptance, got {}", wire_name(other)),
    }
}

fn wire_name(wire: Option<WireMessage>) -> &'static str {
    match wire {
        Some(WireMessage::ContactAccepted(_)) => "ContactAccepted",
        Some(WireMessage::ContactRejected { .. }) => "ContactRejected",
        Some(_) => "another wire message",
        None => "nothing",
    }
}

// The rendezvous sends are fire-and-forget, so a dead receiver is enough.
fn no_swarm() -> mpsc::Sender<SwarmCommand> {
    mpsc::channel(1).0
}

fn channels() -> (
    mpsc::Sender<SwarmCommand>,
    mpsc::Receiver<SwarmCommand>,
    mpsc::Sender<crate::api::AppEvent>,
    mpsc::Receiver<crate::api::AppEvent>,
) {
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (event_tx, event_rx) = mpsc::channel(64);

    (cmd_tx, cmd_rx, event_tx, event_rx)
}

#[tokio::test]
async fn ltc_full_session_roundtrip() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_alice").await;
    let bob = make_peer(&env, "ltc_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle_noprekey(alice.clone())
        .await
        .unwrap();
    let now = get_timestamp_secs().unwrap();

    let ltc = LtcPayload {
        payload_id: MessageId::new(),
        peer_id: "alice".to_string(),
        pre_key_bundle: alice_bundle.serialize().unwrap(),
        dilithium_pub_key: get_dilithium_pub(&*alice.0.lock().await).unwrap(),
        relay_addresses: vec![],
        created_at: now,
        expires_at: now + 604800,
    };

    let ltc_bytes = ltc.serialize().unwrap();
    let ltc_received = LtcPayload::deserialize(&ltc_bytes).unwrap();

    let alice_bundle_for_bob = PreKeyBundleData::deserialize(&ltc_received.pre_key_bundle).unwrap();
    session_initiate(bob.clone(), alice_bundle_for_bob, &alice_address)
        .await
        .unwrap();

    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    session_initiate(alice.clone(), bob_bundle, &bob_address)
        .await
        .unwrap();

    let plaintext = b"hello from alice via ltc";
    let ciphertext = message_send(alice.clone(), &bob_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(bob.clone(), &alice_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn ltc_state_is_expired() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_expired").await;

    let mut state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    assert!(!state.is_expired());

    state.expires_at = get_timestamp_secs().unwrap() - 100;
    assert!(state.is_expired());
}

#[tokio::test]
async fn ltc_never_expires_when_ttl_is_none() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_never").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, None)
        .await
        .unwrap();

    assert_eq!(state.expires_at, u64::MAX);
    assert!(!state.is_expired());
    assert_eq!(state.dto_serialize().unwrap().expires_at, None);
}

#[tokio::test]
async fn ltc_state_survives_a_storage_roundtrip() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_roundtrip").await;

    let created = LtcState::create(alice.clone(), &no_swarm(), Some(5), Some(3600))
        .await
        .unwrap();
    let loaded = load_state(&alice).await.unwrap();

    assert_eq!(loaded.payload_id, created.payload_id);
    assert_eq!(loaded.expires_at, created.expires_at);
    assert_eq!(loaded.max_uses, Some(5));
    assert_eq!(loaded.uses, 0);

    let dto = loaded.dto_serialize().unwrap();
    assert_eq!(dto.payload_id, hex::encode(created.payload_id.0));
    assert_eq!(dto.expires_at, Some(created.expires_at));
    assert_eq!(dto.max_uses, Some(5));
    assert_eq!(dto.uses, 0);
    assert!(dto.size_bytes > 0);
}

#[tokio::test]
async fn ltc_create_replaces_the_previous_code_and_prunes_its_kyber_prekey() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_replace").await;

    let first = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let second = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();

    assert_ne!(first.payload_id, second.payload_id);
    assert_ne!(first.kyber_pre_key_id, second.kyber_pre_key_id);

    let lock = alice.0.lock().await;
    let old = lock
        .raw_read(
            TABLE_KYBER_PRE_KEYS,
            &format!("kyber_prekey_{}", first.kyber_pre_key_id),
        )
        .unwrap();
    let new = lock
        .raw_read(
            TABLE_KYBER_PRE_KEYS,
            &format!("kyber_prekey_{}", second.kyber_pre_key_id),
        )
        .unwrap();

    assert!(old.is_none());
    assert!(new.is_some());
}

#[tokio::test]
async fn ltc_revoke_prunes_the_kyber_prekey() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_revoke_kyber").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone(), &no_swarm())
        .await
        .unwrap();

    let stored = alice
        .0
        .lock()
        .await
        .raw_read(
            TABLE_KYBER_PRE_KEYS,
            &format!("kyber_prekey_{}", state.kyber_pre_key_id),
        )
        .unwrap();

    assert!(stored.is_none());
}

#[tokio::test]
async fn ltc_revoke_clears_the_stored_code() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_revoke_state").await;

    LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone(), &no_swarm())
        .await
        .unwrap();

    assert!(load_state(&alice).await.is_none());
}

#[tokio::test]
async fn ltc_use_is_counted_on_accept() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_count_alice").await;
    let bob = make_peer(&env, "ltc_count_bob").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let response = contact_response(&bob, state.payload_id).await;

    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_accepted(&mut cmd_rx);
    assert!(contact_saved(&alice, &response).await);
    assert_eq!(uses(&alice).await, 1);
}

#[tokio::test]
async fn ltc_unlimited_code_accepts_repeatedly() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_unlimited_alice").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();

    for i in 0..3 {
        let peer = make_peer(&env, &format!("ltc_unlimited_peer{i}")).await;
        let response = contact_response(&peer, state.payload_id).await;

        handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
            .await
            .unwrap();

        assert_accepted(&mut cmd_rx);
        assert!(contact_saved(&alice, &response).await);
    }

    assert_eq!(uses(&alice).await, 3);
}

#[tokio::test]
async fn ltc_rejects_once_max_uses_is_reached() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_max_alice").await;
    let bob = make_peer(&env, "ltc_max_bob").await;
    let carol = make_peer(&env, "ltc_max_carol").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), Some(1), Some(3600))
        .await
        .unwrap();
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();

    let first = contact_response(&bob, state.payload_id).await;
    handle_fc_response(first.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();
    assert_accepted(&mut cmd_rx);
    assert_eq!(uses(&alice).await, 1);

    let second = contact_response(&carol, state.payload_id).await;
    handle_fc_response(second.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_rejected(&mut cmd_rx, FcRejectReason::AlreadyUsed);
    assert!(!contact_saved(&alice, &second).await);
    assert_eq!(uses(&alice).await, 1);
}

#[tokio::test]
async fn ltc_lowering_max_uses_below_current_uses_exhausts_the_code() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_lower_alice").await;
    let bob = make_peer(&env, "ltc_lower_bob").await;
    let carol = make_peer(&env, "ltc_lower_carol").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), Some(5), Some(3600))
        .await
        .unwrap();
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();

    let first = contact_response(&bob, state.payload_id).await;
    handle_fc_response(first, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();
    assert_accepted(&mut cmd_rx);
    assert_eq!(uses(&alice).await, 1);

    let dto = LtcState::update_limits(alice.clone(), Some(1), Some(3600))
        .await
        .unwrap();
    assert_eq!(dto.max_uses, Some(1));
    assert_eq!(dto.uses, 1);

    let second = contact_response(&carol, state.payload_id).await;
    handle_fc_response(second.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_rejected(&mut cmd_rx, FcRejectReason::AlreadyUsed);
    assert!(!contact_saved(&alice, &second).await);
}

#[tokio::test]
async fn ltc_raising_max_uses_revives_an_exhausted_code() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_raise_alice").await;
    let bob = make_peer(&env, "ltc_raise_bob").await;
    let carol = make_peer(&env, "ltc_raise_carol").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), Some(1), Some(3600))
        .await
        .unwrap();
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();

    let first = contact_response(&bob, state.payload_id).await;
    handle_fc_response(first, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();
    assert_accepted(&mut cmd_rx);

    LtcState::update_limits(alice.clone(), Some(2), Some(3600))
        .await
        .unwrap();

    let second = contact_response(&carol, state.payload_id).await;
    handle_fc_response(second.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_accepted(&mut cmd_rx);
    assert!(contact_saved(&alice, &second).await);
    assert_eq!(uses(&alice).await, 2);
}

#[tokio::test]
async fn ltc_expired_code_is_rejected() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_expiry_alice").await;
    let bob = make_peer(&env, "ltc_expiry_bob").await;

    let mut state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();

    state.expires_at = get_timestamp_secs().unwrap() - 1;
    state.save(&*alice.0.lock().await).unwrap();

    let response = contact_response(&bob, state.payload_id).await;
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_rejected(&mut cmd_rx, FcRejectReason::Expired);
    assert!(!contact_saved(&alice, &response).await);
    assert_eq!(uses(&alice).await, 0);
}

#[tokio::test]
async fn ltc_revoked_code_is_rejected() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_revoked_alice").await;
    let bob = make_peer(&env, "ltc_revoked_bob").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone(), &no_swarm())
        .await
        .unwrap();

    let response = contact_response(&bob, state.payload_id).await;
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_rejected(&mut cmd_rx, FcRejectReason::Unknown);
    assert!(!contact_saved(&alice, &response).await);
}

#[tokio::test]
async fn ltc_unknown_payload_id_is_rejected() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_unknown_alice").await;
    let bob = make_peer(&env, "ltc_unknown_bob").await;

    LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();

    let response = contact_response(&bob, MessageId::new()).await;
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_rejected(&mut cmd_rx, FcRejectReason::Unknown);
    assert!(!contact_saved(&alice, &response).await);
    assert_eq!(uses(&alice).await, 0);
}

#[tokio::test]
async fn ltc_existing_contact_does_not_burn_a_use() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_reimport_alice").await;
    let bob = make_peer(&env, "ltc_reimport_bob").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let (cmd_tx, mut cmd_rx, event_tx, _event_rx) = channels();

    let response = contact_response(&bob, state.payload_id).await;
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();
    assert_accepted(&mut cmd_rx);
    assert_eq!(uses(&alice).await, 1);

    let mut again = response.clone();
    again.peer_id = PeerId::random().to_base58();
    handle_fc_response(again, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    assert_accepted(&mut cmd_rx);
    assert_eq!(uses(&alice).await, 1);
}

#[tokio::test]
async fn last_resort_kyber_prekey_survives_reuse() {
    use crate::identity::generators::generate_kyber_prekey;
    use libsignal_protocol::{IdentityKeyStore, KeyPair, KyberPreKeyStore, SignedPreKeyId};
    use rand::{TryRngCore, rngs::OsRng};

    let env = TestEnv::new();
    let mut alice = make_peer(&env, "lastresort").await;
    let identity = alice.get_identity_key_pair().await.unwrap();
    let mut rng = OsRng.unwrap_err();

    let (onetime_id, _) = generate_kyber_prekey(alice.clone(), &identity, &mut rng, false)
        .await
        .unwrap();
    let (lastresort_id, _) = generate_kyber_prekey(alice.clone(), &identity, &mut rng, true)
        .await
        .unwrap();

    let base_key = KeyPair::generate(&mut rng).public_key;
    let signed_id = SignedPreKeyId::from(1u32);

    alice
        .mark_kyber_pre_key_used(onetime_id, signed_id, &base_key)
        .await
        .unwrap();
    alice
        .mark_kyber_pre_key_used(lastresort_id, signed_id, &base_key)
        .await
        .unwrap();

    assert!(alice.get_kyber_pre_key(onetime_id).await.is_err());
    assert!(alice.get_kyber_pre_key(lastresort_id).await.is_ok());
}

#[tokio::test]
async fn ltc_response_replay_ignored() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_replay_alice").await;
    let bob = make_peer(&env, "ltc_replay_bob").await;

    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let response = contact_response(&bob, state.payload_id).await;
    let bob_user_id = responder_id(&response);

    let (cmd_tx, _cmd_rx, event_tx, _event_rx) = channels();
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    let stored_peer_id = response.peer_id.clone();
    let mut contact = Contact::load(&*alice.0.lock().await, &bob_user_id)
        .unwrap()
        .unwrap();
    contact.verified = true;
    contact.offline.send_counter = 5;
    contact.save(&*alice.0.lock().await).unwrap();

    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    let mut altered = response.clone();
    altered.relay_addresses = vec!["/ip4/6.6.6.6/tcp/4001".to_string()];
    altered.peer_id = PeerId::random().to_base58();
    handle_fc_response(altered, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    let reloaded = Contact::load(&*alice.0.lock().await, &bob_user_id)
        .unwrap()
        .unwrap();
    assert!(reloaded.verified);
    assert_eq!(reloaded.offline.send_counter, 5);
    assert_eq!(reloaded.peer_id, stored_peer_id);
    assert!(reloaded.known_addresses.is_empty());
    assert_eq!(uses(&alice).await, 1);
}

fn fake_swarm(peer_id: PeerId, cmd_tx: mpsc::Sender<SwarmCommand>) -> SwarmHandle {
    SwarmHandle {
        peer_id,
        cmd_tx,
        relay_config: RelayConfig {
            enabled: false,
            max_connections: 0,
            max_connections_per_ip: 0,
        },
        mdns_enabled: false,
        port: 0,
    }
}

async fn signed_pointer(
    signer: &SharedDatabase,
    tag: &[u8; 32],
    payload_id: MessageId,
    peer_id: &str,
    seq: u64,
) -> LtcPointer {
    let mut pointer = LtcPointer {
        payload_id,
        peer_id: peer_id.to_string(),
        relay_addresses: vec![],
        seq,
        signature: vec![],
    };

    let secret = get_dilithium_secret(&*signer.0.lock().await).unwrap();
    pointer.sign(tag, secret).unwrap();

    pointer
}

async fn pointer_record(tag: &[u8; 32], pointer: &LtcPointer) -> Vec<u8> {
    DHTRecord::new(
        tag.to_vec(),
        pointer.serialize().unwrap(),
        get_timestamp_secs().unwrap(),
        true,
    )
    .await
    .unwrap()
    .serialize()
    .unwrap()
}

fn spawn_swarm_responder(
    mut cmd_rx: mpsc::Receiver<SwarmCommand>,
    records: Vec<Vec<u8>>,
    payload_id: MessageId,
    ack: Option<std::result::Result<(), FcRejectReason>>,
    seen: Arc<StdMutex<Vec<String>>>,
) {
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                SwarmCommand::GetListenAddresses { reply_tx } => {
                    let _ = reply_tx.send(vec![]);
                }
                SwarmCommand::FetchDht { reply_tx, .. } => {
                    for record in &records {
                        let _ = reply_tx.send(record.clone()).await;
                    }
                }
                SwarmCommand::SendMessage { peer_id, .. } => {
                    seen.lock().unwrap().push(peer_id.to_base58());
                    if let Some(ack) = ack {
                        resolve_ack_waiter(payload_id, ack);
                    }
                }
                _ => {}
            }
        }
    });
}

async fn alice_ltc_payload(alice: &SharedDatabase, peer_id: &str) -> LtcPayload {
    let state = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();

    LtcPayload {
        payload_id: state.payload_id,
        peer_id: peer_id.to_string(),
        pre_key_bundle: state.pre_key_bundle,
        dilithium_pub_key: state.dilithium_pub_key,
        relay_addresses: vec![],
        created_at: state.created_at,
        expires_at: state.expires_at,
    }
}

#[tokio::test]
async fn ltc_rendezvous_tag_survives_edits_and_moves_on_replace() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_tag_stable").await;

    let first = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    let tag = first.rendezvous_tag();

    LtcState::update_limits(alice.clone(), Some(9), Some(7200))
        .await
        .unwrap();
    assert_eq!(load_state(&alice).await.unwrap().rendezvous_tag(), tag);

    let replaced = LtcState::create(alice.clone(), &no_swarm(), None, Some(3600))
        .await
        .unwrap();
    assert_ne!(replaced.rendezvous_tag(), tag);
}

#[tokio::test]
async fn ltc_pointer_signature_roundtrip_and_tampering() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_ptr_sign").await;
    let mallory = make_peer(&env, "ltc_ptr_mallory").await;

    let alice_pub = get_dilithium_pub(&*alice.0.lock().await).unwrap();
    let payload_id = MessageId::new();
    let tag = ltc_rendezvous_tag(&payload_id, &alice_pub);
    let peer = PeerId::random().to_base58();

    let pointer = signed_pointer(&alice, &tag, payload_id, &peer, 100).await;
    assert!(pointer.verify(&tag, &alice_pub));

    let mut flipped = signed_pointer(&alice, &tag, payload_id, &peer, 100).await;
    flipped.signature[0] ^= 0xFF;
    assert!(!flipped.verify(&tag, &alice_pub));

    let mut moved = signed_pointer(&alice, &tag, payload_id, &peer, 100).await;
    moved.peer_id = PeerId::random().to_base58();
    assert!(!moved.verify(&tag, &alice_pub));

    let mut rolled = signed_pointer(&alice, &tag, payload_id, &peer, 100).await;
    rolled.seq = 999;
    assert!(!rolled.verify(&tag, &alice_pub));

    let forged = signed_pointer(&mallory, &tag, payload_id, &peer, 100).await;
    assert!(!forged.verify(&tag, &alice_pub));

    let other_tag = ltc_rendezvous_tag(&MessageId::new(), &alice_pub);
    assert!(!pointer.verify(&other_tag, &alice_pub));
}

#[tokio::test]
async fn ltc_pointer_fetch_keeps_the_highest_seq() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_ptr_seq").await;

    let alice_pub = get_dilithium_pub(&*alice.0.lock().await).unwrap();
    let payload_id = MessageId::new();
    let tag = ltc_rendezvous_tag(&payload_id, &alice_pub);

    let stale = signed_pointer(&alice, &tag, payload_id, "stale-peer", 10).await;
    let fresh = signed_pointer(&alice, &tag, payload_id, "fresh-peer", 20).await;

    let records = vec![
        pointer_record(&tag, &stale).await,
        pointer_record(&tag, &fresh).await,
    ];

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    spawn_swarm_responder(
        cmd_rx,
        records,
        payload_id,
        None,
        Arc::new(StdMutex::new(vec![])),
    );

    let found = fetch_ltc_pointer(&tag, &alice_pub, payload_id, &cmd_tx)
        .await
        .unwrap()
        .expect("a pointer");

    assert_eq!(found.peer_id, "fresh-peer");
}

#[tokio::test]
async fn ltc_pointer_fetch_discards_forged_and_foreign_records() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_ptr_forged").await;
    let mallory = make_peer(&env, "ltc_ptr_forger").await;

    let alice_pub = get_dilithium_pub(&*alice.0.lock().await).unwrap();
    let payload_id = MessageId::new();
    let tag = ltc_rendezvous_tag(&payload_id, &alice_pub);

    let forged = signed_pointer(&mallory, &tag, payload_id, "attacker-peer", 50).await;
    let foreign = signed_pointer(&alice, &tag, MessageId::new(), "other-code-peer", 60).await;

    let records = vec![
        pointer_record(&tag, &forged).await,
        pointer_record(&tag, &foreign).await,
    ];

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    spawn_swarm_responder(
        cmd_rx,
        records,
        payload_id,
        None,
        Arc::new(StdMutex::new(vec![])),
    );

    let found = fetch_ltc_pointer(&tag, &alice_pub, payload_id, &cmd_tx)
        .await
        .unwrap();

    assert!(found.is_none());
}

#[tokio::test]
async fn ltc_revoke_removes_the_rendezvous_record() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_ptr_revoke").await;

    let (cmd_tx, mut cmd_rx) = mpsc::channel(64);
    let state = LtcState::create(alice.clone(), &cmd_tx, None, Some(3600))
        .await
        .unwrap();
    let tag = state.rendezvous_tag();

    while cmd_rx.try_recv().is_ok() {}

    LtcState::revoke_ltc(alice.clone(), &cmd_tx).await.unwrap();

    let mut removed = None;
    while let Ok(cmd) = cmd_rx.try_recv() {
        if let SwarmCommand::RemoveDht { key } = cmd {
            removed = Some(key);
        }
    }

    assert_eq!(removed, Some(tag.to_vec()));
}

#[tokio::test]
async fn ltc_follow_rotations_off_publishes_nothing() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_ptr_off").await;

    let (cmd_tx, mut cmd_rx) = mpsc::channel(64);
    LtcState::create(alice.clone(), &cmd_tx, None, Some(3600))
        .await
        .unwrap();

    let dto = LtcState::set_follow_rotations(alice.clone(), &cmd_tx, false)
        .await
        .unwrap();
    assert!(!dto.follow_rotations);

    while cmd_rx.try_recv().is_ok() {}

    let (event_tx, _event_rx) = mpsc::channel(16);
    LtcState::publish_pointer(
        alice.clone(),
        fake_swarm(PeerId::random(), cmd_tx.clone()),
        event_tx,
    )
    .await
    .unwrap();

    let mut published = false;
    while let Ok(cmd) = cmd_rx.try_recv() {
        if matches!(cmd, SwarmCommand::PublishDht { .. }) {
            published = true;
        }
    }

    assert!(!published);
    assert!(
        load_state(&alice)
            .await
            .unwrap()
            .pointer_published_at
            .is_none()
    );
}

#[tokio::test]
async fn ltc_import_falls_back_to_the_rendezvous_pointer() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_fallback_alice").await;
    let bob = make_peer(&env, "ltc_fallback_bob").await;

    // A peer id the importer cannot even parse stands in for a dead one, so the
    // first attempt fails immediately instead of burning the ack timeout.
    let payload = alice_ltc_payload(&alice, "dead-peer-id").await;
    let tag = ltc_rendezvous_tag(&payload.payload_id, &payload.dilithium_pub_key);

    let live = PeerId::random();
    let pointer = signed_pointer(
        &alice,
        &tag,
        payload.payload_id,
        &live.to_base58(),
        get_timestamp_secs().unwrap(),
    )
    .await;

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let seen = Arc::new(StdMutex::new(vec![]));
    spawn_swarm_responder(
        cmd_rx,
        vec![pointer_record(&tag, &pointer).await],
        payload.payload_id,
        Some(Ok(())),
        seen.clone(),
    );

    let contact = LtcState::import_ltc(payload, bob.clone(), &fake_swarm(PeerId::random(), cmd_tx))
        .await
        .unwrap();

    assert_eq!(contact.peer_id, live.to_base58());
    assert_eq!(seen.lock().unwrap().as_slice(), &[live.to_base58()]);
}

#[tokio::test]
async fn ltc_import_does_not_retry_after_a_rejection() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_reject_alice").await;
    let bob = make_peer(&env, "ltc_reject_bob").await;

    let publisher = PeerId::random();
    let payload = alice_ltc_payload(&alice, &publisher.to_base58()).await;
    let tag = ltc_rendezvous_tag(&payload.payload_id, &payload.dilithium_pub_key);

    let elsewhere = signed_pointer(
        &alice,
        &tag,
        payload.payload_id,
        &PeerId::random().to_base58(),
        get_timestamp_secs().unwrap(),
    )
    .await;

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let seen = Arc::new(StdMutex::new(vec![]));
    spawn_swarm_responder(
        cmd_rx,
        vec![pointer_record(&tag, &elsewhere).await],
        payload.payload_id,
        Some(Err(FcRejectReason::AlreadyUsed)),
        seen.clone(),
    );

    let result =
        LtcState::import_ltc(payload, bob.clone(), &fake_swarm(PeerId::random(), cmd_tx)).await;

    assert!(result.is_err());
    assert_eq!(seen.lock().unwrap().as_slice(), &[publisher.to_base58()]);
}
