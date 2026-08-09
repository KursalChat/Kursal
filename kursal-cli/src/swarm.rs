use crate::{
    config::RelayConfig,
    health::{HealthState, start_health_server},
    identity::load_or_generate,
};
use kursal_core::MapKursalResult;
use kursal_core::stats::SharedRegistry;
use kursal_core::{
    KursalError, Result,
    network::{
        bootstrap::bootstrap_peers,
        kademlia::{KAD_MAX_PACKET, KAD_VALIDATED_QUEUE, KursalKadStore, spawn_record_validation},
        limiter::ConnectionLimiter,
    },
};
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
    futures::StreamExt,
    kad::store::RecordStore,
    multiaddr::Protocol,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use std::{
    collections::{HashSet, VecDeque},
    convert::Infallible,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, mpsc, watch};

const EVENT_LOG_CAP: usize = 200;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "KursalBehaviourEvent")]
pub struct KursalBehaviour {
    pub relay: libp2p::relay::Behaviour,
    pub kad: libp2p::kad::Behaviour<KursalKadStore>,
    pub identify: libp2p::identify::Behaviour,
    pub limiter: ConnectionLimiter,
    pub ping: libp2p::ping::Behaviour,
}

pub struct RelayHandle {
    pub peer_id: PeerId,
}

#[derive(Clone, Default)]
pub struct RelaySnapshot {
    pub peer_id: String,
    pub connections: usize,
    pub reservations: usize,
    pub circuits: usize,
    pub listen_addrs: Vec<String>,
    pub events: VecDeque<String>,
}

struct RelayState {
    snapshot: RelaySnapshot,
    start: Instant,
    health: Arc<RwLock<HealthState>>,
    snapshot_tx: Option<watch::Sender<RelaySnapshot>>,
    listen_addresses: HashSet<Multiaddr>,
}

impl RelayState {
    fn log_event(&mut self, line: String) {
        log::info!("{line}");
        let secs = self.start.elapsed().as_secs();
        let stamped = format!(
            "[{:02}:{:02}:{:02}] {line}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60
        );
        if self.snapshot.events.len() >= EVENT_LOG_CAP {
            self.snapshot.events.pop_front();
        }
        self.snapshot.events.push_back(stamped);
    }

    fn publish(&mut self) {
        self.snapshot.listen_addrs = self
            .listen_addresses
            .iter()
            .map(|a| a.to_string())
            .collect();
        if let Some(tx) = &self.snapshot_tx {
            let _ = tx.send(self.snapshot.clone());
        }
    }
}

pub async fn spawn_relay_swarm(
    config: RelayConfig,
    keypair_path: PathBuf,
    registry: SharedRegistry,
    snapshot_tx: Option<watch::Sender<RelaySnapshot>>,
) -> Result<()> {
    let keypair = load_or_generate(&keypair_path)?;
    let peer_id = keypair.public().to_peer_id();
    log::info!("[relay] peer id: {peer_id}");

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            Default::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .ok_kursal(KursalError::Network)?
        .with_quic();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    // TODO: maybe chance cloudflare to another DNS
    let swarm = swarm.with_dns_config(
        libp2p::dns::ResolverConfig::cloudflare(),
        libp2p::dns::ResolverOpts::default(),
    );

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let swarm = swarm
        .with_dns()
        .map_err(|err| KursalError::Network(format!("swarm dns error: {err}")))?;

    let swarm = {
        let mut registry = registry
            .lock()
            .map_err(|_| KursalError::Network("stats registry poisoned".to_string()))?;
        swarm.with_bandwidth_metrics(&mut registry)
    };

    let mut swarm = swarm
        .with_behaviour(
            |key| -> std::result::Result<_, Box<dyn std::error::Error + Send + Sync>> {
                let local_peer_id = key.public().to_peer_id();

                let relay = libp2p::relay::Behaviour::new(
                    local_peer_id,
                    libp2p::relay::Config {
                        max_circuits: 1024,
                        max_circuits_per_peer: 32,
                        max_reservations: 1024,
                        max_circuit_duration: Duration::from_secs(24 * 60 * 60),
                        max_circuit_bytes: u64::MAX,
                        reservation_rate_limiters: Vec::new(),
                        circuit_src_rate_limiters: Vec::new(),
                        ..Default::default()
                    },
                );

                let mut kad_config =
                    libp2p::kad::Config::new(StreamProtocol::new("/kursal/kad/1.0.0"));
                kad_config.set_max_packet_size(KAD_MAX_PACKET);
                kad_config.set_record_filtering(libp2p::kad::StoreInserts::FilterBoth);
                kad_config.set_record_ttl(Some(Duration::from_secs(3 * 7 * 24 * 60 * 60))); // 3 weeks

                let mut kad = libp2p::kad::Behaviour::with_config(
                    local_peer_id,
                    KursalKadStore::new(local_peer_id),
                    kad_config,
                );
                kad.set_mode(Some(libp2p::kad::Mode::Server));

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/kursal/v1.0.0".to_string(),
                    key.public(),
                ));

                let limiter =
                    ConnectionLimiter::new(config.max_connections, config.max_connections_per_ip);

                let ping = libp2p::ping::Behaviour::new(
                    libp2p::ping::Config::new()
                        .with_interval(Duration::from_secs(15))
                        .with_timeout(Duration::from_secs(20)),
                );

                Ok(KursalBehaviour {
                    identify,
                    kad,
                    relay,
                    limiter,
                    ping,
                })
            },
        )
        .ok_kursal(KursalError::Network)?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();

    let tcp_addr =
        Multiaddr::from(config.listen_addr.ip()).with(Protocol::Tcp(config.listen_addr.port()));
    let quic_addr = Multiaddr::from(config.listen_addr.ip())
        .with(Protocol::Udp(config.listen_addr.port()))
        .with(Protocol::QuicV1);

    swarm.listen_on(tcp_addr).ok_kursal(KursalError::Network)?;
    swarm.listen_on(quic_addr).ok_kursal(KursalError::Network)?;

    let announce_base: Multiaddr = match config.announce_addr.host.parse::<std::net::IpAddr>() {
        Ok(ip) => Multiaddr::from(ip),
        Err(_) => {
            Multiaddr::empty().with(Protocol::Dns4(config.announce_addr.host.as_str().into()))
        }
    };

    let tcp_addr = announce_base
        .clone()
        .with(Protocol::Tcp(config.announce_addr.port));
    let quic_addr = announce_base
        .with(Protocol::Udp(config.listen_addr.port()))
        .with(Protocol::QuicV1);

    swarm.add_external_address(tcp_addr);
    swarm.add_external_address(quic_addr);

    for multiaddr in bootstrap_peers() {
        if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
            swarm
                .behaviour_mut()
                .kad
                .add_address(&peer_id, multiaddr.clone());
            let _ = swarm.dial(multiaddr.clone());
        }
    }

    for multiaddr in &config.bootstrap_peers {
        if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
            swarm
                .behaviour_mut()
                .kad
                .add_address(&peer_id, multiaddr.clone());
            let _ = swarm.dial(multiaddr.clone());
        }
    }

    log::info!("[swarm] event loop started");

    let health_state = Arc::new(RwLock::new(HealthState {
        peer_id: peer_id.to_base58(),
        start_time: Instant::now(),
        connections: 0,
    }));

    let health_addr = config.health.listen_addr.parse().map_err(|err| {
        KursalError::Storage(format!(
            "Could not parse health.listen_addr `{}`: {err}",
            config.health.listen_addr
        ))
    })?;

    tokio::spawn(start_health_server(health_state.clone(), health_addr));

    let mut state = RelayState {
        snapshot: RelaySnapshot {
            peer_id: peer_id.to_base58(),
            ..Default::default()
        },
        start: Instant::now(),
        health: health_state,
        snapshot_tx,
        listen_addresses: HashSet::new(),
    };
    state.publish();

    let (validated_tx, mut validated_rx) =
        mpsc::channel::<libp2p::kad::Record>(KAD_VALIDATED_QUEUE);

    loop {
        tokio::select! {
            event = swarm.select_next_some() => handle_swarm_event(event, &mut swarm, &mut state, &validated_tx).await,
            Some(record) = validated_rx.recv() => {
                if let Err(err) = swarm.behaviour_mut().kad.store_mut().put(record) {
                    log::debug!("[kad] validated record not stored: {err:?}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log::info!("[relay] shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    swarm: &mut Swarm<KursalBehaviour>,
    state: &mut RelayState,
    validated_tx: &mpsc::Sender<libp2p::kad::Record>,
) {
    match event {
        SwarmEvent::Behaviour(KursalBehaviourEvent::Identify(
            libp2p::identify::Event::Received { peer_id, info, .. },
        )) => {
            for addr in info.listen_addrs.clone() {
                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::ReservationReqAccepted { renewed, .. },
        )) => {
            if !renewed {
                state.snapshot.reservations += 1;
                let total = state.snapshot.reservations;
                state.log_event(format!("[relay] reservation accepted (total {total})"));
            }
            state.publish();
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::ReservationTimedOut { .. },
        )) => {
            state.snapshot.reservations = state.snapshot.reservations.saturating_sub(1);
            let total = state.snapshot.reservations;
            state.log_event(format!("[relay] reservation timed out (total {total})"));
            state.publish();
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::ReservationClosed { .. },
        )) => {
            state.snapshot.reservations = state.snapshot.reservations.saturating_sub(1);
            let total = state.snapshot.reservations;
            state.log_event(format!("[relay] reservation closed (total {total})"));
            state.publish();
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::CircuitReqAccepted { .. },
        )) => {
            state.snapshot.circuits += 1;
            let total = state.snapshot.circuits;
            state.log_event(format!("[relay] circuit established (total {total})"));
            state.publish();
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::CircuitClosed { .. },
        )) => {
            state.snapshot.circuits = state.snapshot.circuits.saturating_sub(1);
            let total = state.snapshot.circuits;
            state.log_event(format!("[relay] circuit closed (total {total})"));
            state.publish();
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::Event::CircuitReqDenied { .. },
        )) => {
            state.log_event("[relay] circuit denied".to_string());
            state.publish();
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(libp2p::kad::Event::InboundRequest {
            request:
                libp2p::kad::InboundRequest::PutRecord {
                    record: Some(record),
                    ..
                },
        })) => spawn_record_validation(record, validated_tx.clone()),

        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(_)) => {}

        SwarmEvent::ConnectionEstablished { endpoint, .. } => {
            state.snapshot.connections += 1;

            let kind_str = if endpoint.is_relayed() {
                "relay"
            } else {
                "direct"
            };

            let total = state.snapshot.connections;
            state.log_event(format!("[swarm] connected via {kind_str} (total {total})"));

            let mut health = state.health.write().await;
            health.connections = state.snapshot.connections;
            drop(health);
            state.publish();
        }
        SwarmEvent::ConnectionClosed { .. } => {
            state.snapshot.connections = state.snapshot.connections.saturating_sub(1);

            let total = state.snapshot.connections;
            state.log_event(format!("[swarm] disconnected (total {total})"));

            let mut health = state.health.write().await;
            health.connections = state.snapshot.connections;
            drop(health);
            state.publish();
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            state.log_event(format!("[swarm] listening on {address}"));
            state.listen_addresses.insert(address);
            state.publish();
        }
        SwarmEvent::ExpiredListenAddr { address, .. } => {
            state.log_event(format!("[swarm] expired listening on {address}"));
            state.listen_addresses.remove(&address);
            state.publish();
        }
        _ => {}
    }
}

#[allow(clippy::large_enum_variant)]
pub enum KursalBehaviourEvent {
    Relay(libp2p::relay::Event),
    Kad(libp2p::kad::Event),
    Identify(libp2p::identify::Event),
    Limiter(Infallible),
    Ping(libp2p::ping::Event),
}

impl From<libp2p::relay::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::Event) -> Self {
        Self::Relay(value)
    }
}
impl From<libp2p::kad::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::kad::Event) -> Self {
        Self::Kad(value)
    }
}
impl From<libp2p::identify::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::identify::Event) -> Self {
        Self::Identify(value)
    }
}
impl From<Infallible> for KursalBehaviourEvent {
    fn from(value: Infallible) -> Self {
        Self::Limiter(value)
    }
}
impl From<libp2p::ping::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::ping::Event) -> Self {
        Self::Ping(value)
    }
}
