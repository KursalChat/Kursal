use crate::{
    contacts::Contact,
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::{
        ContactResponse, FcRejectReason, WireMessage, handle_fc_response,
        ltc::{LtcPayload, LtcState},
    },
    identity::{
        UserId,
        generators::{generate_dilithium_keypair, generate_identity_keypair},
    },
    messaging::enums::MessageId,
    network::swarm::SwarmCommand,
    storage::{
        Database, SharedDatabase, TABLE_KYBER_PRE_KEYS, get_dilithium_pub, get_timestamp_secs,
    },
    tests::TestEnv,
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use sha2::{Digest, Sha256};
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

    let mut state = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, None).await.unwrap();

    assert_eq!(state.expires_at, u64::MAX);
    assert!(!state.is_expired());
    assert_eq!(state.dto_serialize().unwrap().expires_at, None);
}

#[tokio::test]
async fn ltc_state_survives_a_storage_roundtrip() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_roundtrip").await;

    let created = LtcState::create(alice.clone(), Some(5), Some(3600))
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

    let first = LtcState::create(alice.clone(), None, Some(3600))
        .await
        .unwrap();
    let second = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone()).await.unwrap();

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

    LtcState::create(alice.clone(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone()).await.unwrap();

    assert!(load_state(&alice).await.is_none());
}

#[tokio::test]
async fn ltc_use_is_counted_on_accept() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_count_alice").await;
    let bob = make_peer(&env, "ltc_count_bob").await;

    let state = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), Some(1), Some(3600))
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

    let state = LtcState::create(alice.clone(), Some(5), Some(3600))
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

    let state = LtcState::create(alice.clone(), Some(1), Some(3600))
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

    let mut state = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, Some(3600))
        .await
        .unwrap();
    LtcState::revoke_ltc(alice.clone()).await.unwrap();

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

    LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, Some(3600))
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

    let state = LtcState::create(alice.clone(), None, Some(3600))
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
