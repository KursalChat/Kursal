use crate::dto::{MessageResponse, apply_offline_overlay};
use crate::identity::UserId;
use crate::messaging::StoredMessage;
use crate::messaging::enums::{Direction, KursalMessage, MessageId, MessageStatus, TextMessage};
use crate::messaging::offline::{
    OfflineState, PendingBundle, QueuedDr, clear_pending_ack, drop_acked_bundles, list_pending_ack,
    write_pending_ack,
};
use crate::storage::get_timestamp_secs;
use crate::tests::TestEnv;

fn text_stored(id: MessageId, dir: Direction, status: MessageStatus, ts: u64) -> StoredMessage {
    StoredMessage {
        id,
        contact_id: UserId([7u8; 32]),
        direction: dir,
        payload: KursalMessage::Text(TextMessage {
            id,
            content: "hi".into(),
            timestamp: ts,
            reply_to: None,
        }),
        status,
        timestamp: ts,
        raw_ciphertext: None,
        edited: false,
        pinned: false,
        reactions: vec![],
    }
}

#[test]
fn message_id_timestamp_is_recent() {
    let before = get_timestamp_secs().unwrap();
    let id = MessageId::new();
    let after = get_timestamp_secs().unwrap();
    let ts = id.timestamp_secs();
    assert!(
        ts >= before && ts <= after,
        "v7 ts {ts} not in [{before}, {after}]"
    );
}

#[test]
fn offline_delivered_variant_exists() {
    let s = MessageStatus::OfflineDelivered;
    assert!(matches!(s, MessageStatus::OfflineDelivered));
}

#[test]
fn dto_offline_delivered_sent_vs_received() {
    let id = MessageId::new();
    let sent: MessageResponse =
        text_stored(id, Direction::Sent, MessageStatus::OfflineDelivered, 100).into();
    assert_eq!(sent.status, "offline_delivered");
    assert!(!sent.via_offline);

    let recv: MessageResponse = text_stored(
        id,
        Direction::Received,
        MessageStatus::OfflineDelivered,
        100,
    )
    .into();
    assert_eq!(recv.status, "delivered");
    assert!(recv.via_offline);
}

#[test]
fn dto_timestamp_is_sent_received_is_stored() {
    let id = MessageId::new();
    let m: MessageResponse =
        text_stored(id, Direction::Received, MessageStatus::Delivered, 100).into();
    assert_eq!(m.timestamp, id.timestamp_secs());
    assert_eq!(m.received_timestamp, 100);
}

#[test]
fn overlay_maps_queue_and_bundle_and_leaves_others() {
    let queued_id = MessageId::new();
    let bundled_id = MessageId::new();
    let plain_id = MessageId::new();

    let mut rows: Vec<MessageResponse> = vec![
        text_stored(queued_id, Direction::Sent, MessageStatus::Sending, 1).into(),
        text_stored(bundled_id, Direction::Sent, MessageStatus::Sending, 2).into(),
        text_stored(plain_id, Direction::Sent, MessageStatus::Sending, 3).into(),
    ];

    let mut offline = OfflineState::default();
    offline.send_queue.push(QueuedDr {
        message_id: Some(queued_id),
        ciphertext: vec![],
    });
    offline.pending_bundles.push(PendingBundle {
        counter: 0,
        bytes: vec![],
        created_at: 0,
        tag: [0u8; 32],
        message_ids: vec![bundled_id],
    });

    apply_offline_overlay(&mut rows, &offline);

    assert_eq!(rows[0].status, "queued");
    assert_eq!(rows[1].status, "queued_in_dht");
    assert_eq!(rows[2].status, "sending");
}

#[test]
fn drop_acked_reports_touch() {
    let id = MessageId::new();
    let mut st = OfflineState::default();
    st.pending_bundles.push(PendingBundle {
        counter: 0,
        bytes: vec![],
        created_at: 0,
        tag: [0u8; 32],
        message_ids: vec![id],
    });
    assert!(drop_acked_bundles(&mut st, &id));
    assert!(st.pending_bundles.is_empty());
    assert!(!drop_acked_bundles(&mut st, &id));
}

#[tokio::test]
async fn pending_ack_roundtrip() {
    let env = TestEnv::new();
    let db = crate::identity::init(&env.db_path("pa"), &env.keychain_config(), env.data_dir())
        .await
        .unwrap();
    let user = UserId([9u8; 32]);
    let id = MessageId::new();

    write_pending_ack(&db, &user, &id, 1234).await.unwrap();
    let all = list_pending_ack(&db).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0.0, user.0);
    assert_eq!(all[0].1, id);
    assert_eq!(all[0].2, 1234);

    clear_pending_ack(&db, &user, &id).await.unwrap();
    assert!(list_pending_ack(&db).await.unwrap().is_empty());
}
