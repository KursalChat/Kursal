use crate::{
    identity::{UserId, init, security_code},
    tests::TestEnv,
};
use libsignal_protocol::IdentityKeyStore;

// Call identity::init twice on the same DB, assert keys are identical (idempotent)
#[tokio::test]
async fn identity_init_idempotent() {
    let env = TestEnv::new();
    let path = env.db_path("identity_init_idempotent");

    let db1 = init(&path, &env.keychain_config(), env.data_dir())
        .await
        .unwrap();
    let keypair1 = db1.get_identity_key_pair().await.unwrap();
    drop(db1);

    let db2 = init(&path, &env.keychain_config(), env.data_dir())
        .await
        .unwrap();
    let keypair2 = db2.get_identity_key_pair().await.unwrap();

    assert_eq!(keypair1.serialize(), keypair2.serialize());
}

// Call local_user_id twice on the same DB, assert same result (stable)
#[tokio::test]
async fn identity_user_id_stable() {
    let env = TestEnv::new();
    let path = env.db_path("identity_user_id_stable");

    let db = init(&path, &env.keychain_config(), env.data_dir())
        .await
        .unwrap();

    let id1 = UserId::local_user_id(db.clone()).await.unwrap();
    let id2 = UserId::local_user_id(db.clone()).await.unwrap();

    assert_eq!(id1.0, id2.0);
}

// security_code is symmetric regardless of which side calls it
#[test]
fn security_code_symmetric() {
    let a_id = b"alice_identity_public_key_bytes_";
    let a_dil = b"alice_dilithium_public_key_bytes";
    let b_id = b"bob_identity_public_key_bytes___";
    let b_dil = b"bob_dilithium_public_key_bytes__";

    let code_from_alice = security_code(a_id, a_dil, b_id, b_dil);
    let code_from_bob = security_code(b_id, b_dil, a_id, a_dil);

    assert_eq!(code_from_alice, code_from_bob);
}
