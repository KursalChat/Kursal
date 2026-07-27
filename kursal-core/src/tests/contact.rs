use crate::{
    contacts::Contact, identity::UserId, messaging::offline::OfflineState, storage::Database,
    tests::TestEnv,
};

fn make_db(env: &TestEnv, name: &str) -> Database {
    Database::open(&env.db_path(name), [0u8; 32]).unwrap()
}

fn make_contact(user_id: UserId) -> Contact {
    Contact {
        user_id,
        peer_id: "Test User".to_string(),
        display_name: "Test User".to_string(),
        avatar_bytes: None,
        identity_pub_key: vec![1u8; 32],
        dilithium_pub_key: vec![2u8; 32],
        known_addresses: vec!["/ip4/127.0.0.1/tcp/4001".to_string()],
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: 1_000_000,
        offline: OfflineState::default(),
    }
}

#[test]
fn contact_save_and_load() {
    let env = TestEnv::new();
    let db = make_db(&env, "save_load");
    let user_id = UserId([1u8; 32]);
    let contact = make_contact(user_id.clone());

    contact.save(&db).unwrap();
    let loaded = Contact::load(&db, &user_id).unwrap().unwrap();

    assert_eq!(loaded.user_id.0, contact.user_id.0);
    assert_eq!(loaded.display_name, contact.display_name);
    assert_eq!(loaded.identity_pub_key, contact.identity_pub_key);
    assert_eq!(loaded.dilithium_pub_key, contact.dilithium_pub_key);
    assert_eq!(loaded.known_addresses, contact.known_addresses);
    assert_eq!(loaded.verified, contact.verified);
    assert_eq!(loaded.created_at, contact.created_at);
}

#[test]
fn contact_set_verified() {
    let env = TestEnv::new();
    let db = make_db(&env, "set_verified");
    let user_id = UserId([2u8; 32]);
    let contact = make_contact(user_id.clone());

    contact.save(&db).unwrap();
    assert!(!Contact::load(&db, &user_id).unwrap().unwrap().verified);

    Contact::set_verified(&db, &user_id).unwrap();
    assert!(Contact::load(&db, &user_id).unwrap().unwrap().verified);
}

#[test]
fn contact_set_addresses() {
    let env = TestEnv::new();
    let db = make_db(&env, "set_addresses");
    let user_id = UserId([3u8; 32]);
    let contact = make_contact(user_id.clone());

    contact.save(&db).unwrap();

    let new_addresses = vec![
        "/ip4/10.0.0.1/tcp/4001".to_string(),
        "/ip4/10.0.0.2/tcp/4001".to_string(),
    ];
    Contact::set_addresses(&db, &user_id, new_addresses.clone()).unwrap();

    let loaded = Contact::load(&db, &user_id).unwrap().unwrap();
    assert_eq!(loaded.known_addresses, new_addresses);
}

#[test]
fn contact_load_all_three() {
    let env = TestEnv::new();
    let db = make_db(&env, "load_all");

    for i in 1u8..=3 {
        make_contact(UserId([i; 32])).save(&db).unwrap();
    }

    let all = Contact::load_all(&db).unwrap();
    assert_eq!(all.len(), 3);
}
