use crate::{
    Result,
    api::{
        AppEvent, ConnectionStatus, CoreCommand, file_transfers::file_chunk_loop,
        handle_core_command,
    },
    first_contact::{
        FileTransferMessage,
        nearby::{
            BtEvent, NearbyBeacon, NearbyOrigin, NearbyTransport, bluetooth::BTTransport,
            mdns::MdnsTransport,
        },
    },
    identity::{UserId, init_transport},
    network::swarm::{NetworkEvent, SwarmCommand, SwarmHandle},
    storage::{
        Database, SharedDatabase, get_relay_config, get_swarm_listening_port,
        get_swarm_mdns_enabled,
    },
};
use libp2p::PeerId;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc};

pub mod bootstrap;
pub mod dht;
mod events;
pub mod kademlia;
pub mod limiter;
mod loops;
pub mod rotation;
pub mod swarm;

use events::{handle_bt_event, handle_internal_network_event};
use loops::{periodic_offline_poll, presence_dial_loop, presence_sync_loop, relay_reserve_loop};

pub struct NetworkManager {
    pub primary: SwarmHandle,
    pub secondary: Option<SwarmHandle>,
    pub event_tx: mpsc::Sender<NetworkEvent>,
    pub bt_event_tx: mpsc::Sender<BtEvent>,
    pub chunk_tx: mpsc::Sender<(PeerId, FileTransferMessage)>,
    //
    pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    pub nearby_peers: Arc<Mutex<HashMap<(String, NearbyOrigin), NearbyBeacon>>>,
    pub mdns_transport: Arc<MdnsTransport>,
    pub bt_transport: Arc<BTTransport>,
}

impl NetworkManager {
    pub async fn new(
        db: &Database,
    ) -> Result<(
        Self,
        mpsc::Receiver<NetworkEvent>,
        mpsc::Receiver<BtEvent>,
        mpsc::Receiver<(PeerId, FileTransferMessage)>,
    )> {
        let (event_tx, event_rx) = mpsc::channel(64);
        let (bt_event_tx, bt_event_rx) = mpsc::channel(64);
        let (chunk_tx, chunk_rx) = mpsc::channel(64);
        let identity = init_transport(db)?;

        let port = get_swarm_listening_port(db);
        let relay_config = get_relay_config(db);
        let mdns_enabled = get_swarm_mdns_enabled(db);

        let primary = SwarmHandle::spawn(
            identity,
            event_tx.clone(),
            chunk_tx.clone(),
            relay_config,
            mdns_enabled,
            port.unwrap_or(0u16),
        )
        .await?;

        let my_beacon = Arc::new(Mutex::new(None));
        let mdns_transport = MdnsTransport::new(primary.cmd_tx.clone(), my_beacon.clone());
        let bt_transport = BTTransport::new(
            primary.cmd_tx.clone(),
            my_beacon.clone(),
            bt_event_tx.clone(),
        );

        Ok((
            Self {
                primary,
                secondary: None,
                event_tx,
                bt_event_tx,
                chunk_tx,
                //
                my_beacon,
                nearby_peers: Arc::new(Mutex::new(HashMap::new())),
                mdns_transport: Arc::new(mdns_transport),
                bt_transport: Arc::new(bt_transport),
            },
            event_rx,
            bt_event_rx,
            chunk_rx,
        ))
    }

    pub async fn start_mdns(&mut self, beacon: NearbyBeacon) -> Result<()> {
        *self.my_beacon.lock().await = Some(beacon.clone());

        if let Err(e) = self.primary.cmd_tx.send(SwarmCommand::EnableNearby).await {
            log::error!("Failed to enable mDNS: {e}");
        }

        self.mdns_transport.start(beacon.clone()).await;
        self.bt_transport.start(beacon).await;

        Ok(())
    }

    pub async fn stop_mdns(&mut self) -> Result<()> {
        self.nearby_peers.lock().await.clear();
        *self.my_beacon.lock().await = None;

        if let Err(e) = self.primary.cmd_tx.send(SwarmCommand::DisableNearby).await {
            log::error!("Failed to disable mDNS: {e}");
        }

        self.mdns_transport.stop().await;
        self.bt_transport.stop().await;

        Ok(())
    }
}

pub async fn get_nearby_peers(
    network: &NetworkManager,
) -> Vec<(String, NearbyBeacon, NearbyOrigin)> {
    network
        .nearby_peers
        .lock()
        .await
        .clone()
        .into_iter()
        .map(|((peer_id, origin), beacon)| (peer_id, beacon, origin))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn dispatch_events(
    mut event_rx: mpsc::Receiver<NetworkEvent>,
    mut bt_event_rx: mpsc::Receiver<BtEvent>,
    mut core_cmd_rx: mpsc::Receiver<CoreCommand>,
    chunk_rx: mpsc::Receiver<(PeerId, FileTransferMessage)>,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    app_event_tx: mpsc::Sender<AppEvent>,
    cache_dir: &std::path::Path,
) {
    let status_map: Arc<Mutex<HashMap<UserId, ConnectionStatus>>> =
        Arc::new(Mutex::new(HashMap::new()));

    {
        let db = db.clone();
        let event_tx = app_event_tx.clone();
        tokio::task::spawn_local(file_chunk_loop(chunk_rx, db, event_tx));
    }
    {
        let network = network.clone();
        let db = db.clone();
        let event_tx = app_event_tx.clone();
        tokio::task::spawn_local(periodic_offline_poll(db, network, event_tx));
    }
    {
        let network = network.clone();
        let db = db.clone();
        tokio::task::spawn_local(presence_dial_loop(db, network));
    }
    {
        let network = network.clone();
        tokio::task::spawn_local(relay_reserve_loop(network));
    }
    {
        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
        let custom_nodes = crate::storage::get_custom_nodes(&*db.0.lock().await);
        for addr_str in custom_nodes {
            if let Ok(addr) = addr_str.parse::<libp2p::Multiaddr>() {
                let _ = cmd_tx.send(SwarmCommand::AddNode(addr)).await;
            }
        }
    }
    #[cfg(feature = "calls")]
    {
        let (input, output) = {
            let guard = db.0.lock().await;
            (
                crate::storage::get_audio_input_device(&guard),
                crate::storage::get_audio_output_device(&guard),
            )
        };
        if input.is_some() {
            crate::call::audio::set_input_device(input);
        }
        if output.is_some() {
            crate::call::audio::set_output_device(output);
        }
    }
    {
        let network = network.clone();
        let db = db.clone();
        let event_tx = app_event_tx.clone();
        let status_map = status_map.clone();
        tokio::task::spawn_local(presence_sync_loop(db, network, event_tx, status_map));
    }

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                handle_internal_network_event(event, &db, cache_dir, &network, &app_event_tx, &status_map).await
            }
            Some(event) = bt_event_rx.recv() => {
                handle_bt_event(event, &db, &network, &app_event_tx).await
            }
            Some(cmd) = core_cmd_rx.recv() => {
                let db = db.clone();
                let network = network.clone();
                let app_event_tx = app_event_tx.clone();
                tokio::task::spawn_local(async move {
                    handle_core_command(cmd, db, network, app_event_tx).await;
                });
            }
            else => break
        }
    }
}
