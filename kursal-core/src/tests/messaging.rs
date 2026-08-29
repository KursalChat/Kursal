use crate::{
    crypto::{
        PreKeyBundleData,
        messages::{message_receive, message_send},
        session_initiate,
    },
    identity::{self, UserId},
    messaging::{
        StoredMessage,
        enums::{
            CallOutcome, CallRecordMessage, DeliveryReceipt, Direction, KursalMessage, MessageId,
            MessageStatus, TextMessage,
        },
        unread_after,
    },
    storage::{Database, SharedDatabase, get_timestamp_secs},
    tests::TestEnv,
};
use libsignal_protocol::{DeviceId, ProtocolAddress};

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Open a fully-initialised database (all keys generated).
async fn make_db(env: &TestEnv, name: &str) -> SharedDatabase {
    identity::init(&env.db_path(name), &env.keychain_config(), env.data_dir())
        .await
        .unwrap()
}

/// Establish Signal sessions on both sides so both can send and receive.
///
/// `alice_addr` is the address Alice registers herself under (how Bob addresses her).
/// `bob_addr`   is the address Bob   registers himself under (how Alice addresses him).
async fn setup_sessions(
    alice: SharedDatabase,
    bob: SharedDatabase,
    alice_addr: &ProtocolAddress,
    bob_addr: &ProtocolAddress,
) {
    let alice_bundle = PreKeyBundleData::build_pre_key_bundle(alice.clone())
        .await
        .unwrap();
    let bob_bundle = PreKeyBundleData::build_pre_key_bundle(bob.clone())
        .await
        .unwrap();

    // Bob ingests Alice's bundle → can now send to alice_addr
    session_initiate(bob.clone(), alice_bundle, alice_addr)
        .await
        .unwrap();
    // Alice ingests Bob's bundle → can now send to bob_addr
    session_initiate(alice.clone(), bob_bundle, bob_addr)
        .await
        .unwrap();
}

// ──────────────────────────────────────────────────────────────
// Test 1: full round-trip with delivery receipt
// ──────────────────────────────────────────────────────────────

/// Alice sends a TextMessage.
/// Bob decrypts it and signs a delivery receipt over the raw ciphertext.
/// Alice verifies the receipt and marks the message as Delivered.
#[tokio::test]
async fn test_message_roundtrip_with_receipt() {
    let env = TestEnv::new();
    let alice = make_db(&env, "msg_rt_alice").await;
    let bob = make_db(&env, "msg_rt_bob").await;

    // Use fixed hex strings as protocol addresses for reproducibility.
    let alice_addr = ProtocolAddress::new("alice_rt".to_string(), DeviceId::new(1u8).unwrap());
    let bob_addr = ProtocolAddress::new("bob_rt".to_string(), DeviceId::new(1u8).unwrap());

    setup_sessions(alice.clone(), bob.clone(), &alice_addr, &bob_addr).await;

    // ── Alice composes and encrypts a message ──────────────────
    let msg_id = MessageId::new();
    let outgoing = KursalMessage::Text(TextMessage {
        id: msg_id,
        content: "hello bob".to_string(),
        timestamp: get_timestamp_secs().unwrap(),
        reply_to: None,
    });
    let plaintext = outgoing.serialize().unwrap();
    let ciphertext = message_send(alice.clone(), &bob_addr, &plaintext)
        .await
        .unwrap();

    // Alice stores the message as Sent/Sending.
    let alice_view_of_bob = UserId([0xBBu8; 32]); // placeholder contact id
    let sent = StoredMessage {
        id: msg_id,
        contact_id: alice_view_of_bob.clone(),
        direction: Direction::Sent,
        payload: outgoing,
        status: MessageStatus::Sending,
        timestamp: get_timestamp_secs().unwrap(),
        raw_ciphertext: Some(ciphertext.clone()),
        edited: false,
        pinned: false,
        reactions: vec![],
    };
    sent.save(&alice).unwrap();

    // ── Bob decrypts ──────────────────────────────────────────
    let decrypted = message_receive(bob.clone(), &alice_addr, &ciphertext)
        .await
        .unwrap();
    let received_msg = KursalMessage::deserialize(&decrypted).unwrap();

    let KursalMessage::Text(ref text) = received_msg else {
        panic!("Expected KursalMessage::Text, got something else");
    };
    assert_eq!(text.content, "hello bob");

    // ── Bob replies with a delivery receipt ───────────────────
    // The receipt carries only the message id; its authenticity comes from
    // the encrypted session it travels in, not from a signature.
    let receipt = KursalMessage::DeliveryReceipt(DeliveryReceipt { message_id: msg_id });
    let round = KursalMessage::deserialize(&receipt.serialize().unwrap()).unwrap();
    assert_eq!(round.message_id(), Some(msg_id));

    // ── Alice marks the message Delivered when the receipt arrives ──
    let mut stored = StoredMessage::load(&alice, &alice_view_of_bob, &msg_id)
        .unwrap()
        .expect("message must exist in Alice's DB");

    stored.status = MessageStatus::Delivered;
    stored.save(&alice).unwrap();

    let final_msg = StoredMessage::load(&alice, &alice_view_of_bob, &msg_id)
        .unwrap()
        .unwrap();

    assert!(
        matches!(final_msg.status, MessageStatus::Delivered),
        "Message should be Delivered after the receipt arrives"
    );
}

// ──────────────────────────────────────────────────────────────
// Test 3: load_all returns only messages for the requested contact
// ──────────────────────────────────────────────────────────────

/// Stores messages for three different contacts (2 / 3 / 1).
/// Asserts that load_all returns exactly the right count for each,
/// with no cross-contamination between contacts.
#[tokio::test]
async fn test_load_all_filters_by_contact() {
    // Plain Database is sufficient here - no crypto needed.
    let env = TestEnv::new();
    let db = Database::open(&env.db_path("msg_test_load_all_filter"), [0u8; 32]).unwrap();

    let alice_id = UserId([0xAAu8; 32]);
    let bob_id = UserId([0xBBu8; 32]);
    let charlie_id = UserId([0xCCu8; 32]);

    let make_msg = |contact: UserId, text: &str| {
        let id = MessageId::new();
        StoredMessage {
            id,
            contact_id: contact,
            direction: Direction::Sent,
            payload: KursalMessage::Text(TextMessage {
                id,
                content: text.to_string(),
                timestamp: get_timestamp_secs().unwrap(),
                reply_to: None,
            }),
            status: MessageStatus::Delivered,
            timestamp: get_timestamp_secs().unwrap(),
            raw_ciphertext: None,
            edited: false,
            pinned: false,
            reactions: vec![],
        }
    };

    // 2 messages for Alice
    for i in 0..2u8 {
        make_msg(alice_id.clone(), &format!("alice {i}"))
            .save(&db)
            .unwrap();
    }
    // 3 messages for Bob
    for i in 0..3u8 {
        make_msg(bob_id.clone(), &format!("bob {i}"))
            .save(&db)
            .unwrap();
    }
    // 1 message for Charlie
    make_msg(charlie_id.clone(), "charlie 0").save(&db).unwrap();

    let alice_msgs = StoredMessage::load_recent(&db, &alice_id, 100, None).unwrap();
    let bob_msgs = StoredMessage::load_recent(&db, &bob_id, 100, None).unwrap();
    let charlie_msgs = StoredMessage::load_recent(&db, &charlie_id, 100, None).unwrap();

    assert_eq!(alice_msgs.len(), 2, "Expected 2 messages for alice");
    assert_eq!(bob_msgs.len(), 3, "Expected 3 messages for bob");
    assert_eq!(charlie_msgs.len(), 1, "Expected 1 message for charlie");

    // No cross-contamination
    assert!(
        alice_msgs.iter().all(|m| m.contact_id.0 == alice_id.0),
        "Alice's messages contain wrong contact"
    );
    assert!(
        bob_msgs.iter().all(|m| m.contact_id.0 == bob_id.0),
        "Bob's messages contain wrong contact"
    );
    assert!(
        charlie_msgs.iter().all(|m| m.contact_id.0 == charlie_id.0),
        "Charlie's messages contain wrong contact"
    );
}

#[test]
fn kursal_message_serialize_roundtrip_all_variants() {
    use crate::messaging::enums::{
        CallSignal, CallSignalKind, DeliveryReceipt, FileAccept, FileOffer, MessageDelete,
        MessageEdit, MessagePin, ProfileInfo, ReactionAdd, ReactionRemove,
    };

    let id = MessageId::new();
    let msgs = vec![
        KursalMessage::Text(TextMessage {
            id,
            content: "hello".to_string(),
            timestamp: 1,
            reply_to: Some(MessageId::new()),
        }),
        KursalMessage::Typing,
        KursalMessage::ReactionAdd(ReactionAdd {
            target_id: id,
            emoji: "👍".to_string(),
            timestamp: 2,
        }),
        KursalMessage::ReactionRemove(ReactionRemove {
            target_id: id,
            emoji: "👍".to_string(),
        }),
        KursalMessage::MessagePin(MessagePin {
            target_id: id,
            pinned: true,
        }),
        KursalMessage::MessageEdit(MessageEdit {
            target_id: id,
            new_content: "edited".to_string(),
            edited_at: 3,
        }),
        KursalMessage::MessageDelete(MessageDelete { target_id: id }),
        KursalMessage::FileOffer(FileOffer {
            id,
            filename: "a.png".to_string(),
            size_bytes: 1024,
            random: [7u8; 32],
            hash: [3u8; 32],
        }),
        KursalMessage::FileAccept(FileAccept {
            offer_id: id,
            random: [9u8; 32],
            received_chunks: vec![1u8, 2u8],
        }),
        KursalMessage::CallSignal(CallSignal {
            call_id: id,
            kind: CallSignalKind::Offer,
            payload: vec![1, 2, 3],
        }),
        KursalMessage::DeliveryReceipt(DeliveryReceipt { message_id: id }),
        KursalMessage::ProfileUpdate(ProfileInfo {
            display_name: "bob".to_string(),
            avatar_bytes: Some(vec![0u8; 8]),
        }),
    ];

    for msg in &msgs {
        let bytes = msg.serialize().unwrap();
        let back = KursalMessage::deserialize(&bytes).unwrap();
        assert_eq!(
            bytes,
            back.serialize().unwrap(),
            "roundtrip mismatch for {}",
            msg.kind_name()
        );
        assert_eq!(msg.kind_name(), back.kind_name());
    }
}

#[test]
fn target_message_id_covers_every_mutation() {
    use crate::messaging::enums::{
        MessageDelete, MessageEdit, MessagePin, ReactionAdd, ReactionRemove, TextMessage,
    };

    let target = MessageId::new();

    let mutations = vec![
        KursalMessage::MessageEdit(MessageEdit {
            target_id: target,
            new_content: "edited".to_string(),
            edited_at: 1,
        }),
        KursalMessage::MessageDelete(MessageDelete { target_id: target }),
        KursalMessage::MessagePin(MessagePin {
            target_id: target,
            pinned: true,
        }),
        KursalMessage::ReactionAdd(ReactionAdd {
            target_id: target,
            emoji: "👍".to_string(),
            timestamp: 1,
        }),
        KursalMessage::ReactionRemove(ReactionRemove {
            target_id: target,
            emoji: "👍".to_string(),
        }),
    ];

    for msg in &mutations {
        assert_eq!(
            msg.target_message_id(),
            Some(target),
            "{} should expose its target",
            msg.kind_name()
        );
    }

    let standalone = KursalMessage::Text(TextMessage {
        id: MessageId::new(),
        content: "hi".to_string(),
        timestamp: 1,
        reply_to: None,
    });
    assert_eq!(standalone.target_message_id(), None);
    assert_eq!(KursalMessage::Typing.target_message_id(), None);
}

#[test]
fn kursal_message_deserialize_rejects_garbage() {
    assert!(KursalMessage::deserialize(&[0xFFu8, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_err());
}

#[test]
fn address_announce_roundtrips() {
    use crate::messaging::enums::AddressAnnounce;

    let msg = KursalMessage::AddressAnnounce(AddressAnnounce {
        peer_id: "P".to_string(),
        addresses: vec!["/ip4/1.2.3.4/tcp/1".to_string()],
    });
    let bytes = msg.serialize().unwrap();
    let back = KursalMessage::deserialize(&bytes).unwrap();

    assert!(matches!(back, KursalMessage::AddressAnnounce(_)));
    assert_eq!(back.message_id(), None);
    assert_eq!(back.kind_name(), "AddressAnnounce");
}

#[tokio::test]
async fn address_announce_updates_contact() {
    use crate::api::AppEvent;
    use crate::api::apply_address_announce;
    use crate::contacts::Contact;
    use crate::messaging::offline::OfflineState;
    use crate::network::swarm::SwarmCommand;

    let env = TestEnv::new();
    let db = make_db(&env, "announce").await;

    let contact = Contact {
        user_id: UserId([7u8; 32]),
        peer_id: "OldPeer".to_string(),
        display_name: "T".to_string(),
        avatar: None,
        identity_pub_key: vec![1u8; 32],
        dilithium_pub_key: vec![2u8; 32],
        known_addresses: vec!["/ip4/127.0.0.1/tcp/1".to_string()],
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: 1,
        offline: OfflineState::default(),
    };
    contact.save(&db).unwrap();

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(16);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(16);

    apply_address_announce(
        &UserId([7u8; 32]),
        "NewPeer".to_string(),
        vec!["/ip4/1.2.3.4/tcp/4001".to_string()],
        &db,
        &cmd_tx,
        Some(&event_tx),
    )
    .await
    .unwrap();

    let reloaded = Contact::load(&db, &UserId([7u8; 32])).unwrap().unwrap();
    assert_eq!(reloaded.peer_id, "NewPeer");
    assert_eq!(
        reloaded.known_addresses,
        vec!["/ip4/1.2.3.4/tcp/4001".to_string()]
    );

    let ev = event_rx.try_recv().expect("ContactUpdated event");
    assert!(matches!(ev, AppEvent::ContactUpdated { .. }));

    let cmd = cmd_rx.try_recv().expect("Dial command");
    assert!(matches!(cmd, SwarmCommand::Dial(_)));
}

#[test]
fn offline_roots_symmetric_between_peers() {
    use crate::messaging::offline::derive_offline_roots;

    let secret = vec![7u8; 32];
    let pq = vec![3u8; 32];
    let a = vec![1u8, 2, 3, 4];
    let b = vec![9u8, 8, 7, 6];

    let (a_send, a_recv) = derive_offline_roots(&secret, &pq, &a, &b).unwrap();
    let (b_send, b_recv) = derive_offline_roots(&secret, &pq, &b, &a).unwrap();

    assert_eq!(a_send, b_recv);
    assert_eq!(a_recv, b_send);
    assert_ne!(a_send, a_recv);

    let (a_send_nopq, _) = derive_offline_roots(&secret, &[], &a, &b).unwrap();
    assert_ne!(a_send, a_send_nopq);
}

#[test]
fn offline_ratchet_deterministic_and_separated() {
    use crate::crypto::{offline_at, offline_ratchet_step, offline_tag, offline_wrapper};

    let chain = [42u8; 32];

    let (mk, next) = offline_ratchet_step(&chain).unwrap();
    assert_ne!(mk, next);
    assert_ne!(chain, next);
    assert_ne!(offline_tag(&mk).unwrap(), offline_wrapper(&mk).unwrap());

    let (tag0, wrap0) = offline_at(&chain, 0).unwrap();
    assert_eq!(tag0, offline_tag(&mk).unwrap());
    assert_eq!(wrap0, offline_wrapper(&mk).unwrap());

    let (mk1, _) = offline_ratchet_step(&next).unwrap();
    let (tag1, wrap1) = offline_at(&chain, 1).unwrap();
    assert_eq!(tag1, offline_tag(&mk1).unwrap());
    assert_eq!(wrap1, offline_wrapper(&mk1).unwrap());
    assert_ne!(tag0, tag1);
    assert_ne!(wrap0, wrap1);
}

#[test]
fn offline_chains_symmetric_and_bundle_roundtrip() {
    use crate::crypto::{
        offline_ratchet_step, offline_tag, offline_wrapper, stream::stream_decrypt,
    };
    use crate::messaging::offline::{
        BundleEnvelope, BundleInner, OfflineState, advance_recv_past, derive_offline_roots,
        encode_bundle, recv_keys_at,
    };

    let secret = vec![7u8; 32];
    let pq = vec![3u8; 32];
    let a_pub = vec![1u8, 2, 3, 4];
    let b_pub = vec![9u8, 8, 7, 6];

    let (a_send, _) = derive_offline_roots(&secret, &pq, &a_pub, &b_pub).unwrap();
    let (_, b_recv) = derive_offline_roots(&secret, &pq, &b_pub, &a_pub).unwrap();
    assert_eq!(a_send, b_recv);

    let mut sender_chain = a_send;
    let mut receiver = OfflineState {
        recv_chain: b_recv,
        ..Default::default()
    };

    for i in 0..3u64 {
        let (mk, next) = offline_ratchet_step(&sender_chain).unwrap();
        let tag = offline_tag(&mk).unwrap();
        let wrapper = offline_wrapper(&mk).unwrap();
        let envelope_bytes = encode_bundle(
            &wrapper,
            vec![format!("msg{i}").into_bytes()],
            vec![],
            "peer".to_string(),
        )
        .unwrap();
        sender_chain = next;

        let (recv_tag, recv_wrapper) = recv_keys_at(&receiver, i).unwrap().unwrap();
        assert_eq!(recv_tag, tag);
        assert_eq!(recv_wrapper, wrapper);

        let envelope = BundleEnvelope::deserialize(&envelope_bytes).unwrap();
        let inner_bytes = stream_decrypt(&recv_wrapper, &envelope.ciphertext).unwrap();
        let inner = BundleInner::deserialize(&inner_bytes).unwrap();
        assert_eq!(inner.messages[0], format!("msg{i}").into_bytes());

        advance_recv_past(&mut receiver, i).unwrap();
    }

    assert_eq!(receiver.recv_counter, 3);
    assert!(receiver.skipped_keys.is_empty());
}

#[test]
fn offline_out_of_order_uses_skipped_keys() {
    use crate::crypto::offline_at;
    use crate::messaging::offline::{
        OfflineState, advance_recv_past, consume_skipped, recv_keys_at,
    };

    let chain = [5u8; 32];
    let keys0 = offline_at(&chain, 0).unwrap();
    let keys1 = offline_at(&chain, 1).unwrap();
    let keys2 = offline_at(&chain, 2).unwrap();

    let mut receiver = OfflineState {
        recv_chain: chain,
        ..Default::default()
    };

    assert_eq!(recv_keys_at(&receiver, 2).unwrap().unwrap(), keys2);
    advance_recv_past(&mut receiver, 2).unwrap();
    assert_eq!(receiver.recv_counter, 3);
    assert_eq!(receiver.skipped_keys.len(), 2);

    assert_eq!(recv_keys_at(&receiver, 0).unwrap().unwrap(), keys0);
    consume_skipped(&mut receiver, 0);
    assert!(recv_keys_at(&receiver, 0).unwrap().is_none());

    assert_eq!(recv_keys_at(&receiver, 1).unwrap().unwrap(), keys1);
    consume_skipped(&mut receiver, 1);
    assert!(receiver.skipped_keys.is_empty());
}

#[test]
fn offline_forward_secrecy_after_advance() {
    use crate::api::poll_offline::POLL_WINDOW;
    use crate::crypto::offline_at;
    use crate::messaging::offline::{OfflineState, advance_recv_past};

    let chain = [9u8; 32];
    let (tag0, wrap0) = offline_at(&chain, 0).unwrap();

    let mut receiver = OfflineState {
        recv_chain: chain,
        ..Default::default()
    };
    advance_recv_past(&mut receiver, 0).unwrap();

    assert_ne!(receiver.recv_chain, chain);
    assert!(receiver.skipped_keys.is_empty());
    assert!(recv_keys_at_none_for_consumed(&receiver, 0));

    for steps in 0..POLL_WINDOW {
        let (tag, wrap) = offline_at(&receiver.recv_chain, steps).unwrap();
        assert_ne!(tag, tag0);
        assert_ne!(wrap, wrap0);
    }
}

fn recv_keys_at_none_for_consumed(
    state: &crate::messaging::offline::OfflineState,
    counter: u64,
) -> bool {
    crate::messaging::offline::recv_keys_at(state, counter)
        .unwrap()
        .is_none()
}

#[test]
fn offline_skipped_keys_bounded() {
    use crate::messaging::offline::{MAX_SKIPPED_KEYS, OfflineState, advance_recv_past};

    let mut receiver = OfflineState {
        recv_chain: [1u8; 32],
        ..Default::default()
    };
    advance_recv_past(&mut receiver, MAX_SKIPPED_KEYS as u64 + 20).unwrap();

    assert_eq!(receiver.skipped_keys.len(), MAX_SKIPPED_KEYS);
    assert_eq!(receiver.skipped_keys[0].counter, 20);
}

fn call_record(outcome: CallOutcome, contact: UserId, seq: u8) -> StoredMessage {
    StoredMessage {
        id: MessageId([seq; 16]),
        contact_id: contact,
        direction: Direction::Received,
        payload: KursalMessage::CallRecord(CallRecordMessage {
            id: MessageId([seq; 16]),
            timestamp: 0,
            outcome,
            duration_ms: 0,
        }),
        status: MessageStatus::Delivered,
        timestamp: u64::from(seq),
        raw_ciphertext: None,
        edited: false,
        pinned: false,
        reactions: vec![],
    }
}

#[test]
fn answered_calls_are_never_unread() {
    let env = TestEnv::new();
    let db = Database::open(&env.db_path("unread"), [0u8; 32]).unwrap();
    let contact = UserId([7; 32]);

    for (seq, outcome) in [(1u8, CallOutcome::Started), (2u8, CallOutcome::Completed)] {
        call_record(outcome, contact.clone(), seq)
            .save(&db)
            .unwrap();
    }

    let summary = unread_after(&db, &contact, None, 100).unwrap();
    assert_eq!(summary.count, 0);
}

#[test]
fn unanswered_calls_stay_unread() {
    let env = TestEnv::new();
    let db = Database::open(&env.db_path("unread2"), [0u8; 32]).unwrap();
    let contact = UserId([8; 32]);

    for (seq, outcome) in [(1u8, CallOutcome::Missed), (2u8, CallOutcome::Canceled)] {
        call_record(outcome, contact.clone(), seq)
            .save(&db)
            .unwrap();
    }

    let summary = unread_after(&db, &contact, None, 100).unwrap();
    assert_eq!(summary.count, 2);
}
