use crate::{
    first_contact::nearby::{
        NearbyBeacon, NearbyMessage, NearbyPacket, NearbyTransport, handle_nearby_request,
        mdns::MdnsTransport,
    },
    network::swarm::SwarmCommand,
    storage::{Database, SharedDatabase},
    tests::TestEnv,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

struct MockNearbyTransport {
    sent: Arc<Mutex<Vec<(String, NearbyMessage)>>>,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<NearbyMessage>>>>,
}

impl MockNearbyTransport {
    fn new() -> Self {
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl NearbyTransport for MockNearbyTransport {
    async fn start(&self, _beacon: NearbyBeacon) {}

    async fn stop(&self) {}

    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> crate::Result<()> {
        self.sent.lock().await.push((peer_id.to_string(), msg));
        Ok(())
    }

    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage> {
        let (tx, rx) = mpsc::channel(8);
        self.pending.lock().await.insert(peer_id.to_string(), tx);
        rx
    }

    async fn unregister_handshake(&self, peer_id: &str) {
        self.pending.lock().await.remove(peer_id);
    }
}

fn make_db(env: &TestEnv, name: &str) -> SharedDatabase {
    let db = Database::open(&env.db_path(name), [0u8; 32]).unwrap();
    SharedDatabase::from_db(db)
}

#[tokio::test]
async fn nearby_packet_roundtrip_beacon_and_message() {
    let beacon = NearbyBeacon {
        peer_id: "peer-a".to_string(),
        session_name: "Crimson Fox".to_string(),
    };

    let bytes = NearbyPacket::Beacon(beacon.clone()).serialize().unwrap();
    let decoded = NearbyPacket::deserialize(&bytes).unwrap();
    match decoded {
        NearbyPacket::Beacon(v) => {
            assert_eq!(v.peer_id, beacon.peer_id);
            assert_eq!(v.session_name, beacon.session_name);
        }
        _ => panic!("expected beacon packet"),
    }

    let msg = NearbyMessage::ConnectRequest {
        from_session_name: "Blue Owl".to_string(),
    };
    let bytes = NearbyPacket::Message(msg).serialize().unwrap();
    let decoded = NearbyPacket::deserialize(&bytes).unwrap();
    match decoded {
        NearbyPacket::Message(NearbyMessage::ConnectRequest { from_session_name }) => {
            assert_eq!(from_session_name, "Blue Owl");
        }
        _ => panic!("expected connect request packet"),
    }
}

#[tokio::test]
async fn mdns_send_wraps_message_in_nearby_packet() {
    let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
    let my_beacon = Arc::new(Mutex::new(None));
    let transport = MdnsTransport::new(cmd_tx, my_beacon);

    transport
        .send(
            "12D3KooWQ6QY5qX4W1Q8Tyf8f7F8nN6LdQ2hUYkS8h2Kk4fT5aVY",
            NearbyMessage::ConnectDecline,
        )
        .await
        .unwrap();

    let cmd = cmd_rx.recv().await.expect("expected outbound command");
    let SwarmCommand::SendMessage { data, .. } = cmd else {
        panic!("expected send message command");
    };

    match NearbyPacket::deserialize(&data).unwrap() {
        NearbyPacket::Message(NearbyMessage::ConnectDecline) => {}
        _ => panic!("expected wrapped ConnectDecline packet"),
    }
}

#[tokio::test]
async fn handle_nearby_request_decline_sends_decline_and_returns_ok() {
    let transport = MockNearbyTransport::new();
    let env = TestEnv::new();
    let db = make_db(&env, "decline");
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (cmd_tx, _cmd_rx) = mpsc::channel(16);

    let (decision_tx, decision_rx) = oneshot::channel::<bool>();
    decision_tx.send(false).unwrap();

    handle_nearby_request("peer-z", &transport, decision_rx, db, &event_tx, &cmd_tx)
        .await
        .unwrap();

    let sent = transport.sent.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "peer-z");
    assert!(matches!(sent[0].1, NearbyMessage::ConnectDecline));
}

#[tokio::test]
async fn handle_nearby_request_closed_decision_channel_returns_ok() {
    let transport = MockNearbyTransport::new();
    let env = TestEnv::new();
    let db = make_db(&env, "decision_closed");
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (cmd_tx, _cmd_rx) = mpsc::channel(16);

    let (decision_tx, decision_rx) = oneshot::channel::<bool>();
    drop(decision_tx);

    handle_nearby_request("peer-z", &transport, decision_rx, db, &event_tx, &cmd_tx)
        .await
        .unwrap();

    let sent = transport.sent.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "peer-z");
    assert!(matches!(sent[0].1, NearbyMessage::ConnectDecline));
}
