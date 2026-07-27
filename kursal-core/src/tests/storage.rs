use crate::{
    storage::{Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_MESSAGES},
    tests::TestEnv,
};
use libsignal_protocol::{
    DeviceId, IdentityKeyPair, IdentityKeyStore, ProtocolAddress, SessionRecord, SessionStore,
};
use rand::{TryRngCore, rngs::OsRng};

fn setup_db(env: &TestEnv, name: &str, key: [u8; 32]) -> Database {
    Database::open(&env.db_path(name), key).unwrap()
}

// raw_write then raw_read → assert plaintext matches
#[test]
fn storage_write_read() {
    let key = [1u8; 32];
    let env = TestEnv::new();
    let db = setup_db(&env, "storage_write_read", key);

    let written = [1u8, 2u8, 3u8];
    db.raw_write(TABLE_MESSAGES, "test_key", &written).unwrap();

    let read = db.raw_read(TABLE_MESSAGES, "test_key").unwrap().unwrap();
    assert_eq!(read, written);
}

// raw_read with wrong enc_key → assert decrypt error
#[test]
fn storage_wrong_key() {
    let key = [1u8; 32];
    let wrong_key = [2u8; 32];
    let env = TestEnv::new();

    {
        let db = setup_db(&env, "storage_wrong_key", key);
        db.raw_write(TABLE_MESSAGES, "test_key", &[1u8, 2u8, 3u8])
            .unwrap();
    }

    let db_wrong = setup_db(&env, "storage_wrong_key", wrong_key);
    let result = db_wrong.raw_read(TABLE_MESSAGES, "test_key");
    assert!(result.is_err());
}

// raw_delete then raw_read → assert None
#[test]
fn storage_delete() {
    let key = [1u8; 32];
    let env = TestEnv::new();
    let db = setup_db(&env, "storage_delete", key);

    db.raw_write(TABLE_MESSAGES, "test_key", &[1u8, 2u8, 3u8])
        .unwrap();
    db.raw_delete(TABLE_MESSAGES, "test_key").unwrap();

    let result = db.raw_read(TABLE_MESSAGES, "test_key").unwrap();
    assert_eq!(result, None);
}

// Store a SessionRecord, load it back, assert it deserializes correctly
#[tokio::test]
async fn storage_session_record_roundtrip() {
    let key = [3u8; 32];
    let env = TestEnv::new();
    let mut db = SharedDatabase::from_db(setup_db(&env, "storage_session_record", key));

    let address = ProtocolAddress::new("alice".to_string(), DeviceId::new(1u8).unwrap());
    let record = SessionRecord::new_fresh();

    db.store_session(&address, &record).await.unwrap();

    let loaded = db.load_session(&address).await.unwrap().unwrap();
    assert_eq!(record.serialize().unwrap(), loaded.serialize().unwrap());
}

// Store an IdentityKeyPair, load it back via get_identity_key_pair, assert matches
#[tokio::test]
async fn storage_identity_key_pair_roundtrip() {
    let key = [4u8; 32];
    let env = TestEnv::new();
    let db = setup_db(&env, "storage_identity_key_pair", key);

    let mut rng = OsRng.unwrap_err();
    let identity_key_pair = IdentityKeyPair::generate(&mut rng);
    let serialized = identity_key_pair.serialize();

    db.raw_write(TABLE_IDENTITY_KEYS, "local_identity", &serialized)
        .unwrap();

    let sdb = SharedDatabase::from_db(db);

    let loaded = sdb.get_identity_key_pair().await.unwrap();
    assert_eq!(identity_key_pair.serialize(), loaded.serialize());
}

#[test]
fn storage_update_channel_default_and_roundtrip() {
    use crate::storage::{TABLE_SETTINGS, get_update_channel, set_update_channel};

    let key = [5u8; 32];
    let env = TestEnv::new();
    let db = setup_db(&env, "storage_update_channel", key);
    let _ = db.raw_delete(TABLE_SETTINGS, "update_channel");

    assert_eq!(get_update_channel(&db), "stable");

    set_update_channel(&db, "beta").unwrap();
    assert_eq!(get_update_channel(&db), "beta");

    set_update_channel(&db, "stable").unwrap();
    assert_eq!(get_update_channel(&db), "stable");
}

#[test]
fn storage_updater_enabled_default_and_roundtrip() {
    use crate::storage::{TABLE_SETTINGS, get_updater_enabled, set_updater_enabled};

    let key = [6u8; 32];
    let env = TestEnv::new();
    let db = setup_db(&env, "storage_updater_enabled", key);
    let _ = db.raw_delete(TABLE_SETTINGS, "updater_enabled");

    assert!(get_updater_enabled(&db));

    set_updater_enabled(&db, false).unwrap();
    assert!(!get_updater_enabled(&db));

    set_updater_enabled(&db, true).unwrap();
    assert!(get_updater_enabled(&db));
}

#[test]
fn backup_encrypt_decrypt_roundtrip() {
    use crate::storage::backup::KursalBackup;

    let backup = KursalBackup {
        master_key: vec![1u8, 2, 3, 4, 5],
        database: vec![9u8; 64],
    };
    let password = b"correct horse battery".to_vec();

    let bytes = backup.serialize(password.clone()).unwrap();
    let restored = KursalBackup::deserialize(bytes, password).unwrap();

    assert_eq!(restored.master_key, vec![1u8, 2, 3, 4, 5]);
    assert_eq!(restored.database, vec![9u8; 64]);
}

#[test]
fn backup_wrong_password_fails() {
    use crate::storage::backup::KursalBackup;

    let backup = KursalBackup {
        master_key: vec![7u8; 16],
        database: vec![3u8; 32],
    };

    let bytes = backup.serialize(b"right-password".to_vec()).unwrap();
    assert!(KursalBackup::deserialize(bytes, b"wrong-password".to_vec()).is_err());
}

#[test]
fn typing_indicators_roundtrip() {
    use crate::storage::{get_typing_indicators_enabled, set_typing_indicators_enabled};

    let env = TestEnv::new();
    let db = setup_db(&env, "typing_toggle", [3u8; 32]);

    assert!(get_typing_indicators_enabled(&db));
    set_typing_indicators_enabled(&db, false).unwrap();
    assert!(!get_typing_indicators_enabled(&db));
    set_typing_indicators_enabled(&db, true).unwrap();
    assert!(get_typing_indicators_enabled(&db));
}
