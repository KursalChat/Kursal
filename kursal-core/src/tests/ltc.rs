use crate::{
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    first_contact::ltc::LtcPayload,
    identity::generators::{generate_dilithium_keypair, generate_identity_keypair},
    messaging::enums::MessageId,
    storage::{Database, SharedDatabase, get_dilithium_pub, get_timestamp_secs},
    tests::TestEnv,
};
use libsignal_protocol::{DeviceId, ProtocolAddress};

async fn make_peer(env: &TestEnv, name: &str) -> SharedDatabase {
    let mut db = Database::open(&env.db_path(name), [0u8; 32]).unwrap();
    generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    SharedDatabase::from_db(db)
}

// Alice generates LTC, Bob deserializes and initiates session,
// Alice initiates session back, Alice encrypts → Bob decrypts
#[tokio::test]
async fn ltc_full_session_roundtrip() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_alice").await;
    let bob = make_peer(&env, "ltc_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    // Alice builds LTC bundle (no one-time prekey)
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

    // simulate file transfer
    let ltc_bytes = ltc.serialize().unwrap();
    let ltc_received = LtcPayload::deserialize(&ltc_bytes).unwrap();

    // Bob initiates session with Alice
    let alice_bundle_for_bob = PreKeyBundleData::deserialize(&ltc_received.pre_key_bundle).unwrap();
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

    // Alice encrypts → Bob decrypts
    let plaintext = b"hello from alice via ltc";
    let ciphertext = message_send(alice.clone(), &bob_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(bob.clone(), &alice_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

// Expired LTC is detected correctly
#[test]
fn ltc_is_expired() {
    let now = get_timestamp_secs().unwrap();
    let ltc = LtcPayload {
        payload_id: MessageId::new(),
        peer_id: "alice".to_string(),
        pre_key_bundle: vec![],
        dilithium_pub_key: vec![],
        relay_addresses: vec![],
        created_at: now - 604900,
        expires_at: now - 100,
    };
    assert!(ltc.is_expired());
}

// Valid LTC is not expired
#[test]
fn ltc_not_expired() {
    let now = get_timestamp_secs().unwrap();
    let ltc = LtcPayload {
        payload_id: MessageId::new(),
        peer_id: "alice".to_string(),
        pre_key_bundle: vec![],
        dilithium_pub_key: vec![],
        relay_addresses: vec![],
        created_at: now,
        expires_at: now + 604800,
    };
    assert!(!ltc.is_expired());
}

// A last-resort kyber prekey (used by LTC) survives mark_used so multiple
// importers can reuse it; a one-time kyber prekey (OTP/Nearby) is deleted.
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
    use crate::contacts::Contact;
    use crate::first_contact::{ContactResponse, handle_fc_response};
    use crate::identity::UserId;
    use crate::storage::TABLE_LTC_CACHE;
    use sha2::{Digest, Sha256};
    use tokio::sync::mpsc;

    let env = TestEnv::new();
    let alice = make_peer(&env, "ltc_replay_alice").await;
    let bob = make_peer(&env, "ltc_replay_bob").await;

    let payload_id = MessageId::new();
    let expiry = get_timestamp_secs().unwrap() + 3600;
    {
        let lock = alice.0.lock().await;
        lock.raw_write(TABLE_LTC_CACHE, "ltc_current_id", &payload_id.0)
            .unwrap();
        lock.raw_write(TABLE_LTC_CACHE, "ltc_current_expiry", &expiry.to_be_bytes())
            .unwrap();
    }

    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();
    let bob_identity_pub = bob_bundle.identity_key.public_key().serialize().to_vec();
    let bob_user_id = UserId(Sha256::digest(&bob_identity_pub).into());

    let response = ContactResponse {
        payload_id,
        pre_key_bundle: bob_bundle.serialize().unwrap(),
        peer_id: "bob_peer".to_string(),
        dilithium_pub_key: get_dilithium_pub(&*bob.0.lock().await).unwrap(),
        relay_addresses: vec![],
        mailbox_kem_ct: vec![],
        mailbox_kem_prekey_id: 0,
        mailbox_ephemeral_pub: vec![],
    };

    let (cmd_tx, _cmd_rx) = mpsc::channel(8);
    let (event_tx, _event_rx) = mpsc::channel(8);
    handle_fc_response(response.clone(), alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

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
    altered.peer_id = "evil_peer".to_string();
    handle_fc_response(altered, alice.clone(), &cmd_tx, &event_tx)
        .await
        .unwrap();

    let reloaded = Contact::load(&*alice.0.lock().await, &bob_user_id)
        .unwrap()
        .unwrap();
    assert!(reloaded.verified);
    assert_eq!(reloaded.offline.send_counter, 5);
    assert_eq!(reloaded.peer_id, "bob_peer");
    assert!(reloaded.known_addresses.is_empty());
}
