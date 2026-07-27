use crate::{
    crypto::{
        PreKeyBundleData,
        dilithium::{dilithium_sign, dilithium_verify},
        messages::{message_receive, message_send},
        session_initiate,
        stream::{derive_stream_key, stream_decrypt, stream_encrypt},
    },
    identity::generators::{generate_dilithium_keypair, generate_identity_keypair},
    storage::{Database, SharedDatabase},
    tests::TestEnv,
};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{PublicKey, SecretKey};

// ── helpers ───────────────────────────────────────────────────────────────

// Initializes a fresh peer DB without touching the system keychain.
async fn make_peer(env: &TestEnv, name: &str) -> SharedDatabase {
    let mut db = Database::open(&env.db_path(name), [0u8; 32]).unwrap();

    generate_identity_keypair(&mut db).unwrap();
    generate_dilithium_keypair(&mut db).unwrap();

    SharedDatabase::from_db(db)
}

// ── session + message tests ───────────────────────────────────────────────

// Bob initiates a session toward Alice, sends one message, Alice decrypts
#[tokio::test]
async fn crypto_session_one_message() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "session_one_alice").await;
    let bob = make_peer(&env, "session_one_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    session_initiate(bob.clone(), alice_bundle, &alice_address)
        .await
        .unwrap();

    let plaintext = b"hello alice";
    let ciphertext = message_send(bob.clone(), &alice_address, plaintext)
        .await
        .unwrap();
    let decrypted = message_receive(alice.clone(), &bob_address, &ciphertext)
        .await
        .unwrap();

    assert_eq!(decrypted, plaintext);
}

// 10 messages in order, all decrypt correctly
#[tokio::test]
async fn crypto_session_ten_messages_in_order() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "session_ten_alice").await;
    let bob = make_peer(&env, "session_ten_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    session_initiate(bob.clone(), alice_bundle, &alice_address)
        .await
        .unwrap();

    for i in 0u8..10 {
        let plaintext = vec![i; 8];
        let ciphertext = message_send(bob.clone(), &alice_address, &plaintext)
            .await
            .unwrap();
        let decrypted = message_receive(alice.clone(), &bob_address, &ciphertext)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext, "message {} failed", i);
    }
}

// 10 messages delivered out of order.
// Double Ratchet buffers skipped-message keys up to a limit (~2000 by default in libsignal).
// All 10 should decrypt successfully since we are well within that window.
#[tokio::test]
async fn crypto_session_ten_messages_out_of_order() {
    let env = TestEnv::new();
    let alice = make_peer(&env, "session_ooo_alice").await;
    let bob = make_peer(&env, "session_ooo_bob").await;

    let alice_address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let bob_address = ProtocolAddress::new("bob".to_string(), DeviceId::new(1u8).unwrap());

    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    session_initiate(bob.clone(), alice_bundle, &alice_address)
        .await
        .unwrap();

    // Encrypt all 10 first
    let mut ciphertexts = Vec::new();
    for i in 0u8..10 {
        let plaintext = vec![i; 8];
        let ct = message_send(bob.clone(), &alice_address, &plaintext)
            .await
            .unwrap();
        ciphertexts.push((i, plaintext, ct));
    }

    // Deliver in reverse - libsignal buffers skipped keys, all should succeed
    for (i, plaintext, ct) in ciphertexts.into_iter().rev() {
        let decrypted = message_receive(alice.clone(), &bob_address, &ct)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext, "out-of-order message {} failed", i);
    }
}

// ── dilithium tests ───────────────────────────────────────────────────────

#[test]
fn dilithium_sign_verify_valid() {
    let (pk, sk) = dilithium5::keypair();
    let message = b"kursal test message";

    let sig = dilithium_sign(sk.as_bytes().to_vec(), message).unwrap();
    let valid = dilithium_verify(pk.as_bytes(), message, &sig).unwrap();

    assert!(valid);
}

#[test]
fn dilithium_verify_tampered_message() {
    let (pk, sk) = dilithium5::keypair();
    let message = b"kursal test message";

    let sig = dilithium_sign(sk.as_bytes().to_vec(), message).unwrap();
    let valid = dilithium_verify(pk.as_bytes(), b"tampered message", &sig).unwrap();

    assert!(!valid);
}

// ── stream key + encryption tests ─────────────────────────────────────────

#[test]
fn stream_key_deterministic() {
    let a = [1u8; 32];
    let b = [2u8; 32];

    assert_eq!(
        derive_stream_key(a, b).unwrap(),
        derive_stream_key(a, b).unwrap()
    );
}

// HKDF must not produce the trivial key when one input is all-zeros
#[test]
fn stream_key_hkdf_prevents_weak_passthrough() {
    let a = [1u8; 32];
    let zeros = [0u8; 32];

    assert_ne!(derive_stream_key(a, zeros).unwrap(), a);
}

#[test]
fn stream_encrypt_decrypt_roundtrip() {
    let key = [42u8; 32];
    let plaintext = b"secret stream data";

    let ciphertext = stream_encrypt(&key, plaintext).unwrap();
    let decrypted = stream_decrypt(&key, &ciphertext).unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn stream_aad_roundtrip_and_binding() {
    use crate::crypto::stream::{stream_decrypt_aad, stream_encrypt_aad};

    let key = [42u8; 32];
    let plaintext = b"call frame payload";
    let seq: u64 = 7;

    let ciphertext = stream_encrypt_aad(&key, plaintext, &seq.to_be_bytes()).unwrap();
    let decrypted = stream_decrypt_aad(&key, &ciphertext, &seq.to_be_bytes()).unwrap();
    assert_eq!(decrypted, plaintext);

    let wrong_seq: u64 = 8;
    assert!(stream_decrypt_aad(&key, &ciphertext, &wrong_seq.to_be_bytes()).is_err());
    assert!(stream_decrypt_aad(&key, &ciphertext, &[]).is_err());

    assert!(crate::crypto::stream::stream_decrypt(&key, &ciphertext).is_err());
}

#[test]
fn file_chunk_aad_binds_transfer_and_index() {
    use crate::crypto::stream::{stream_decrypt_aad, stream_encrypt_aad};

    let key = [9u8; 32];
    let transfer_id = [7u8; 16];
    let index: u32 = 3;

    let mut aad = [0u8; 20];
    aad[..16].copy_from_slice(&transfer_id);
    aad[16..].copy_from_slice(&index.to_be_bytes());
    let sealed = stream_encrypt_aad(&key, b"chunk-bytes", &aad).unwrap();
    assert_eq!(
        stream_decrypt_aad(&key, &sealed, &aad).unwrap(),
        b"chunk-bytes"
    );

    let mut wrong_index = aad;
    wrong_index[16..].copy_from_slice(&4u32.to_be_bytes());
    assert!(stream_decrypt_aad(&key, &sealed, &wrong_index).is_err());

    let mut wrong_transfer = aad;
    wrong_transfer[0] ^= 1;
    assert!(stream_decrypt_aad(&key, &sealed, &wrong_transfer).is_err());
}
