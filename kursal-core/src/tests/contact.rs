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
        avatar: None,
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

#[test]
fn find_by_peer_id_reports_unknown_peers() {
    let env = TestEnv::new();
    let db = make_db(&env, "peer_unknown");
    make_contact(UserId([4u8; 32])).save(&db).unwrap();

    assert!(
        Contact::find_by_peer_id(&db, "Test User")
            .unwrap()
            .is_some()
    );
    assert!(Contact::find_by_peer_id(&db, "stranger").unwrap().is_none());
}

#[test]
fn find_by_peer_id_follows_a_rotated_peer_id() {
    let env = TestEnv::new();
    let db = make_db(&env, "peer_rotate");
    let user_id = UserId([5u8; 32]);

    let mut contact = make_contact(user_id.clone());
    contact.save(&db).unwrap();

    contact.peer_id = "rotated".to_string();
    contact.save(&db).unwrap();

    assert!(
        Contact::find_by_peer_id(&db, "Test User")
            .unwrap()
            .is_none()
    );
    let found = Contact::find_by_peer_id(&db, "rotated").unwrap().unwrap();
    assert_eq!(found.user_id.0, user_id.0);
}

#[test]
fn deleted_contact_leaves_no_peer_binding() {
    let env = TestEnv::new();
    let db = make_db(&env, "peer_delete");
    let user_id = UserId([6u8; 32]);
    make_contact(user_id.clone()).save(&db).unwrap();

    Contact::delete(&db, &user_id).unwrap();

    assert!(Contact::load(&db, &user_id).unwrap().is_none());
    assert!(
        Contact::find_by_peer_id(&db, "Test User")
            .unwrap()
            .is_none()
    );
    assert!(Contact::load_all(&db).unwrap().is_empty());
}

#[test]
fn save_if_exists_ignores_an_unknown_contact() {
    let env = TestEnv::new();
    let db = make_db(&env, "save_if_exists");
    let user_id = UserId([7u8; 32]);

    make_contact(user_id.clone()).save_if_exists(&db).unwrap();
    assert!(Contact::load(&db, &user_id).unwrap().is_none());

    make_contact(user_id.clone()).save(&db).unwrap();
    let mut contact = Contact::load(&db, &user_id).unwrap().unwrap();
    contact.display_name = "Renamed".to_string();
    contact.save_if_exists(&db).unwrap();

    let loaded = Contact::load(&db, &user_id).unwrap().unwrap();
    assert_eq!(loaded.display_name, "Renamed");
}

// Two handles in one process must not serve each other's contacts.
#[test]
fn each_database_keeps_its_own_contacts() {
    let env = TestEnv::new();
    let first = make_db(&env, "scope_a");
    let second = make_db(&env, "scope_b");

    make_contact(UserId([8u8; 32])).save(&first).unwrap();

    assert_eq!(Contact::load_all(&first).unwrap().len(), 1);
    assert!(Contact::load_all(&second).unwrap().is_empty());
    assert!(
        Contact::find_by_peer_id(&second, "Test User")
            .unwrap()
            .is_none()
    );
}

// A contact written by one handle has to be visible to a handle opened later
// over the same file, which is what a restart looks like.
#[test]
fn a_reopened_database_sees_stored_contacts() {
    let env = TestEnv::new();
    let user_id = UserId([9u8; 32]);

    {
        let db = make_db(&env, "reopen");
        make_contact(user_id.clone()).save(&db).unwrap();
    }

    let reopened = make_db(&env, "reopen");
    let loaded = Contact::load(&reopened, &user_id).unwrap().unwrap();
    assert_eq!(loaded.user_id.0, user_id.0);
    assert!(
        Contact::find_by_peer_id(&reopened, "Test User")
            .unwrap()
            .is_some()
    );
}
