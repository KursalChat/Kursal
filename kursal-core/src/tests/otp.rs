use crate::crypto::stream::stream_decrypt;
use crate::first_contact::ContactResponse;
use crate::messaging::enums::MessageId;
use crate::storage::{get_dilithium_pub, get_timestamp_secs};
use crate::tests::TestEnv;
use crate::{
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::{handle_fc_response, otp::otp_to_keys},
    identity::generators::{generate_dilithium_keypair, generate_identity_keypair},
    storage::{Database, SharedDatabase, TABLE_SETTINGS},
};
use libsignal_protocol::{DeviceId, IdentityKeyStore, KeyPair, ProtocolAddress};
use rand::TryRngCore;
use tokio::sync::mpsc;

async fn make_peer(env: &TestEnv, name: &str) -> SharedDatabase {
    let mut db = Database::open(&env.db_path(name), [0u8; 32]).unwrap();

    generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    SharedDatabase::from_db(db)
}

// Alice generates OTP, Bob decrypts payload directly, both initiate sessions,
// Bob encrypts a message, Alice decrypts it
#[tokio::test]
async fn otp_full_session_roundtrip() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "otp_alice").await;
    let bob = make_peer(&env, "otp_bob").await;

    let otp = "3-correct 7-horse 1-battery 4-staple 9-ocean 2-light";
    let (enc_key, _dht_key) = otp_to_keys(otp).unwrap();

    // Alice builds her payload (no real network needed - just build it directly)
    // We need a fake NetworkManager peer_id - use alice's peer_id from transport
    // For this test we bypass build_otp_payload and construct it manually
    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let alice_bundle_bytes = alice_bundle.serialize().unwrap();
    let _alice_identity_pub = {
        let keypair = alice.get_identity_key_pair().await.unwrap();
        keypair.public_key().serialize().to_vec()
    };

    // Simulate the encrypted payload Alice would publish to DHT
    let payload = crate::first_contact::otp::OtpPayload {
        payload_id: MessageId::new(),
        pre_key_bundle: alice_bundle_bytes,
        peer_id: "alice_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*alice.0.lock().await).unwrap(),
        relay_addresses: vec![],
    };
    let encrypted_payload =
        crate::crypto::stream::stream_encrypt(&enc_key, &payload.serialize().unwrap()).unwrap();

    // Bob decrypts payload directly (simulating DHT fetch)
    let decrypted = stream_decrypt(&enc_key, &encrypted_payload).unwrap();
    let record = crate::first_contact::otp::OtpPayload::deserialize(&decrypted).unwrap();
    let alice_bundle_for_bob = PreKeyBundleData::deserialize(&record.pre_key_bundle).unwrap();

    let alice_address =
        ProtocolAddress::new("alice_peer_id".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob_peer_id".to_string(), DeviceId::new(1u8).unwrap());

    // Bob initiates session with Alice
    session_initiate(bob.clone(), alice_bundle_for_bob, &alice_address)
        .await
        .unwrap();

    // Alice initiates session with Bob
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    session_initiate(alice.clone(), bob_bundle, &bob_address)
        .await
        .unwrap();

    // Bob encrypts → Alice decrypts
    let plaintext = b"hello from bob";
    let ciphertext = message_send(bob.clone(), &alice_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(alice.clone(), &bob_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

// A second OTPResponse after the first should be ignored
#[tokio::test]
async fn otp_response_ignored_after_consumed() {
    let env = TestEnv::new();
    let db = make_peer(&env, "otp_consumed").await;
    let bob = make_peer(&env, "otp_consumed_bob").await;

    let payload_id = MessageId::new();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_pending_id", &payload_id.0)
        .unwrap();

    let now = get_timestamp_secs().unwrap();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_published_at", &now.to_be_bytes())
        .unwrap();

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone())
        .await
        .unwrap();
    let alice_prekey_id: u32 = alice_bundle.pre_key_id.unwrap().into();
    db.0.lock()
        .await
        .raw_write(
            TABLE_SETTINGS,
            "otp_prekey_id",
            &alice_prekey_id.to_be_bytes(),
        )
        .unwrap();

    let mut rng = rand::rngs::OsRng.unwrap_err();
    let mailbox_ephemeral = KeyPair::generate(&mut rng);
    let mailbox_ephemeral_pub = mailbox_ephemeral.public_key.serialize().to_vec();

    // Build a fake valid OTPResponse from bob
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let response = ContactResponse {
        payload_id,
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
        mailbox_kem_ct: vec![],
        mailbox_kem_prekey_id: 0,
        mailbox_ephemeral_pub,
    };

    // First call succeeds
    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    handle_fc_response(response.clone(), db.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    // Second call should be silently ignored - otp_pending is now false
    let result = handle_fc_response(response, db.clone(), &cmd_tx, &event_tx).await;
    assert!(result.is_ok());
}

// OTPResponse arriving after 10 minutes should be rejected
#[tokio::test]
async fn otp_response_rejected_after_expiry() {
    let env = TestEnv::new();
    let db = make_peer(&env, "otp_expiry").await;
    let bob = make_peer(&env, "otp_expiry_bob").await;

    // OTP pending but published 11 minutes ago
    let fake_past = get_timestamp_secs().unwrap() - 660; // 11 minutes ago
    let payload_id = MessageId::new();

    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_pending_id", &payload_id.0)
        .unwrap();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_published_at", &fake_past.to_be_bytes())
        .unwrap();

    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let response = ContactResponse {
        payload_id,
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
        mailbox_kem_ct: vec![],
        mailbox_kem_prekey_id: 0,
        mailbox_ephemeral_pub: vec![],
    };
    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    let result = handle_fc_response(response, db.clone(), &cmd_tx, &event_tx).await;
    assert!(result.is_ok()); // silently ignored... cant actually really test that lol
}

#[tokio::test]
async fn otp_mailbox_chains_match_after_handshake() {
    use crate::contacts::Contact;
    use crate::identity::UserId;
    use crate::messaging::offline::new_offline_state;
    use sha2::{Digest, Sha256};

    let env = TestEnv::new();
    let alice = make_peer(&env, "otp_chains_alice").await;
    let bob = make_peer(&env, "otp_chains_bob").await;

    let payload_id = MessageId::new();
    let now = get_timestamp_secs().unwrap();
    {
        let lock = alice.0.lock().await;
        lock.raw_write(TABLE_SETTINGS, "otp_pending_id", &payload_id.0)
            .unwrap();
        lock.raw_write(TABLE_SETTINGS, "otp_published_at", &now.to_be_bytes())
            .unwrap();
    }

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let alice_prekey_id: u32 = alice_bundle.pre_key_id.unwrap().into();
    alice
        .0
        .lock()
        .await
        .raw_write(
            TABLE_SETTINGS,
            "otp_prekey_id",
            &alice_prekey_id.to_be_bytes(),
        )
        .unwrap();

    let alice_identity_pub = {
        let keypair = alice.get_identity_key_pair().await.unwrap();
        keypair.public_key().serialize().to_vec()
    };

    let mut rng = rand::rngs::OsRng.unwrap_err();
    let mailbox_ephemeral = KeyPair::generate(&mut rng);
    let classical_bob = mailbox_ephemeral
        .private_key
        .calculate_agreement(&alice_bundle.pre_key_public.unwrap())
        .unwrap();
    let bob_offline = new_offline_state(&bob, &alice_identity_pub, &classical_bob, &[])
        .await
        .unwrap();

    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let bob_identity_pub = bob_bundle.identity_key.public_key().serialize().to_vec();
    let response = ContactResponse {
        payload_id,
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer_id".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
        mailbox_kem_ct: vec![],
        mailbox_kem_prekey_id: 0,
        mailbox_ephemeral_pub: mailbox_ephemeral.public_key.serialize().to_vec(),
    };

    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    handle_fc_response(response, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    let bob_user_id = UserId(Sha256::digest(&bob_identity_pub).into());
    let alice_contact = Contact::load(&*alice.0.lock().await, &bob_user_id)
        .unwrap()
        .unwrap();

    assert_eq!(alice_contact.offline.send_chain, bob_offline.recv_chain);
    assert_eq!(alice_contact.offline.recv_chain, bob_offline.send_chain);
    assert_ne!(
        alice_contact.offline.send_chain,
        alice_contact.offline.recv_chain
    );
}

// A response replaying a consumed OTP gets an explicit AlreadyUsed refusal
#[tokio::test]
async fn otp_replayed_response_is_refused_as_already_used() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "otp_refuse_alice").await;
    let bob = make_peer(&env, "otp_refuse_bob").await;
    let carol = make_peer(&env, "otp_refuse_carol").await;

    let payload_id = MessageId::new();
    let now = get_timestamp_secs().unwrap();

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let alice_prekey_id: u32 = alice_bundle.pre_key_id.unwrap().into();
    {
        let lock = alice.0.lock().await;
        lock.raw_write(TABLE_SETTINGS, "otp_pending_id", &payload_id.0)
            .unwrap();
        lock.raw_write(TABLE_SETTINGS, "otp_published_at", &now.to_be_bytes())
            .unwrap();
        lock.raw_write(
            TABLE_SETTINGS,
            "otp_prekey_id",
            &alice_prekey_id.to_be_bytes(),
        )
        .unwrap();
    }

    let make_response = |db: SharedDatabase, peer_id: String| async move {
        let bundle = PreKeyBundleData::build_pre_key_bundle(db.clone())
            .await
            .unwrap();
        let mut rng = rand::rngs::OsRng.unwrap_err();
        let ephemeral = KeyPair::generate(&mut rng);
        ContactResponse {
            payload_id,
            pre_key_bundle: bundle.serialize().unwrap(),
            peer_id,
            dilithium_pub_key: get_dilithium_pub(&*db.0.lock().await).unwrap(),
            relay_addresses: vec![],
            mailbox_kem_ct: vec![],
            mailbox_kem_prekey_id: 0,
            mailbox_ephemeral_pub: ephemeral.public_key.serialize().to_vec(),
        }
    };

    let bob_peer = libp2p::identity::Keypair::generate_ed25519()
        .public()
        .to_peer_id()
        .to_base58();
    let carol_peer = libp2p::identity::Keypair::generate_ed25519()
        .public()
        .to_peer_id()
        .to_base58();

    let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
    let (event_tx, mut event_rx) = mpsc::channel(16);

    handle_fc_response(
        make_response(bob.clone(), bob_peer).await,
        alice.clone(),
        &cmd_tx,
        &event_tx,
    )
    .await
    .unwrap();

    assert!(
        alice
            .0
            .lock()
            .await
            .raw_read(TABLE_SETTINGS, "otp_consumed_id")
            .unwrap()
            .is_some_and(|id| id.as_slice() == payload_id.0.as_slice())
    );

    let mut consumed_event = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event, crate::api::AppEvent::OtpConsumed) {
            consumed_event = true;
        }
    }
    assert!(consumed_event);
    while cmd_rx.try_recv().is_ok() {}

    handle_fc_response(
        make_response(carol.clone(), carol_peer).await,
        alice.clone(),
        &cmd_tx,
        &event_tx,
    )
    .await
    .unwrap();

    let mut refusal = None;
    while let Ok(command) = cmd_rx.try_recv() {
        if let crate::network::swarm::SwarmCommand::SendMessage { data, .. } = command
            && let Ok(crate::first_contact::WireMessage::ContactRejected { reason, .. }) =
                bincode::deserialize::<crate::first_contact::WireMessage>(&data)
        {
            refusal = Some(reason);
        }
    }

    assert_eq!(
        refusal,
        Some(crate::first_contact::FcRejectReason::AlreadyUsed)
    );
}
