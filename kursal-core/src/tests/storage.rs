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
        avatars: vec![("a".repeat(64), vec![5u8; 12])],
    };
    let password = b"correct horse battery".to_vec();

    let bytes = backup.serialize(password.clone()).unwrap();
    let restored = KursalBackup::deserialize(bytes, password).unwrap();

    assert_eq!(restored.master_key, vec![1u8, 2, 3, 4, 5]);
    assert_eq!(restored.database, vec![9u8; 64]);
    assert_eq!(restored.avatars, vec![("a".repeat(64), vec![5u8; 12])]);
}

#[test]
fn backup_wrong_password_fails() {
    use crate::storage::backup::KursalBackup;

    let backup = KursalBackup {
        master_key: vec![7u8; 16],
        database: vec![3u8; 32],
        avatars: Vec::new(),
    };

    let bytes = backup.serialize(b"right-password".to_vec()).unwrap();
    assert!(KursalBackup::deserialize(bytes, b"wrong-password".to_vec()).is_err());
}

// A deferred commit skips the flush to disk, but the write still has to be
// visible to every later read on the same handle.
#[test]
fn deferred_write_is_readable() {
    use crate::storage::conversation::{get_read_cursor, set_read_cursor};

    let env = TestEnv::new();
    let db = setup_db(&env, "deferred_write", [4u8; 32]);

    set_read_cursor(&db, "contact", Some("message-7")).unwrap();
    assert_eq!(
        get_read_cursor(&db, "contact").as_deref(),
        Some("message-7")
    );

    set_read_cursor(&db, "contact", None).unwrap();
    assert_eq!(get_read_cursor(&db, "contact"), None);
}

#[test]
fn batch_applies_every_write_at_once() {
    let env = TestEnv::new();
    let db = setup_db(&env, "batch_writes", [5u8; 32]);

    db.raw_write(TABLE_MESSAGES, "stale", b"old").unwrap();

    let mut batch = db.batch().unwrap();
    batch.put(TABLE_MESSAGES, "a", b"one").unwrap();
    batch.put(TABLE_IDENTITY_KEYS, "b", b"two").unwrap();
    batch.remove(TABLE_MESSAGES, "stale").unwrap();
    batch.commit().unwrap();

    assert_eq!(
        db.raw_read(TABLE_MESSAGES, "a").unwrap().unwrap(),
        b"one".to_vec()
    );
    assert_eq!(
        db.raw_read(TABLE_IDENTITY_KEYS, "b").unwrap().unwrap(),
        b"two".to_vec()
    );
    assert!(db.raw_read(TABLE_MESSAGES, "stale").unwrap().is_none());
}

// Dropping a batch without committing must leave the table untouched.
#[test]
fn uncommitted_batch_writes_nothing() {
    let env = TestEnv::new();
    let db = setup_db(&env, "batch_rollback", [6u8; 32]);

    let mut batch = db.batch().unwrap();
    batch.put(TABLE_MESSAGES, "ghost", b"value").unwrap();
    drop(batch);

    assert!(db.raw_read(TABLE_MESSAGES, "ghost").unwrap().is_none());
}

#[test]
fn raw_scan_all_visits_every_row_and_can_stop() {
    let env = TestEnv::new();
    let db = setup_db(&env, "scan_all", [7u8; 32]);

    for i in 0..5u8 {
        db.raw_write(TABLE_MESSAGES, &format!("k{i}"), &[i])
            .unwrap();
    }

    let mut seen = 0;
    db.raw_scan_all(TABLE_MESSAGES, |_, _| {
        seen += 1;
        true
    })
    .unwrap();
    assert_eq!(seen, 5);

    let mut stopped = 0;
    db.raw_scan_all(TABLE_MESSAGES, |_, _| {
        stopped += 1;
        stopped < 2
    })
    .unwrap();
    assert_eq!(stopped, 2);
}

#[test]
fn raw_keys_filters_by_prefix() {
    let env = TestEnv::new();
    let db = setup_db(&env, "raw_keys", [8u8; 32]);

    db.raw_write(TABLE_MESSAGES, "aa:1", b"x").unwrap();
    db.raw_write(TABLE_MESSAGES, "aa:2", b"x").unwrap();
    db.raw_write(TABLE_MESSAGES, "bb:1", b"x").unwrap();

    let keys = db.raw_keys(TABLE_MESSAGES, "aa:").unwrap();
    assert_eq!(keys, vec!["aa:1".to_string(), "aa:2".to_string()]);
    assert_eq!(db.raw_keys(TABLE_MESSAGES, "").unwrap().len(), 3);
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

#[test]
fn profile_stale_roundtrip() {
    use crate::storage::conversation::{delete_for_contact, get_profile_stale, set_profile_stale};

    let env = TestEnv::new();
    let db = setup_db(&env, "profile_stale", [9u8; 32]);

    assert!(!get_profile_stale(&db, "abcd"));

    set_profile_stale(&db, "abcd", true).unwrap();
    assert!(get_profile_stale(&db, "abcd"));
    assert!(!get_profile_stale(&db, "efgh"));

    set_profile_stale(&db, "abcd", false).unwrap();
    assert!(!get_profile_stale(&db, "abcd"));

    set_profile_stale(&db, "abcd", true).unwrap();
    delete_for_contact(&db, "abcd").unwrap();
    assert!(!get_profile_stale(&db, "abcd"));
}
