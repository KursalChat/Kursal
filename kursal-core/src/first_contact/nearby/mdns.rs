use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    first_contact::nearby::{NearbyBeacon, NearbyMessage, NearbyPacket, NearbyTransport},
    network::swarm::SwarmCommand,
};
use libp2p::{Multiaddr, PeerId};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::{Mutex, mpsc};

pub struct MdnsTransport {
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    pub pending_handshakes: Arc<Mutex<HashMap<String, mpsc::Sender<NearbyMessage>>>>,
    pub nearby_addresses: Arc<Mutex<HashMap<String, Vec<Multiaddr>>>>,
}

impl MdnsTransport {
    pub fn new(
        cmd_tx: mpsc::Sender<SwarmCommand>,
        my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    ) -> Self {
        Self {
            cmd_tx,
            my_beacon,
            pending_handshakes: Arc::new(Mutex::new(HashMap::new())),
            nearby_addresses: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl NearbyTransport for MdnsTransport {
    async fn start(&self, _beacon: NearbyBeacon) {}
    async fn stop(&self) {
        self.pending_handshakes.lock().await.clear();
        self.nearby_addresses.lock().await.clear();
    }

    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> Result<()> {
        let bytes = NearbyPacket::Message(msg).serialize()?;
        let addresses = self
            .nearby_addresses
            .lock()
            .await
            .get(peer_id)
            .cloned()
            .unwrap_or_default();
        let peer_id = PeerId::from_str(peer_id).ok_kursal(KursalError::Storage)?;

        self.cmd_tx
            .send(SwarmCommand::SendMessage {
                peer_id,
                data: bytes,
                addresses,
            })
            .await
            .ok_kursal(KursalError::Network)?;

        Ok(())
    }

    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage> {
        let (tx, rx) = mpsc::channel::<NearbyMessage>(8);

        self.pending_handshakes
            .lock()
            .await
            .insert(peer_id.to_string(), tx);

        rx
    }

    async fn unregister_handshake(&self, peer_id: &str) {
        self.pending_handshakes.lock().await.remove(peer_id);
    }
}
