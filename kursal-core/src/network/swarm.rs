// TODO: remove all "#[cfg(not(target_os = "ios"))]" and accept mdns with apple dev cert (ios)
use crate::{
    KursalError, Result,
    api::handle_incoming::handle_incoming_stream,
    contacts::Contact,
    first_contact::FileTransferMessage,
    identity::TransportIdentity,
    network::{
        bootstrap::bootstrap_peers,
        kademlia::{KAD_MAX_PACKET, KAD_VALIDATED_QUEUE, KursalKadStore},
        limiter::{ConnectionLimiter, MAX_TRANSIENT_CONNECTIONS},
    },
    storage::RelayConfig,
};
#[cfg(not(target_os = "ios"))]
use libp2p::mdns;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
    futures::StreamExt,
    kad::store::RecordStore,
    multiaddr::Protocol,
    request_response::{self, ProtocolSupport},
    swarm::{ConnectionId, behaviour::toggle::Toggle},
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

mod behaviour;
mod codec;
mod commands;
mod event_loop;
mod helpers;

pub use behaviour::{KursalBehaviour, KursalBehaviourEvent};
pub use codec::KursalMsgCodec;
pub use helpers::{
    any_public_address, get_all_listen_addrs, get_connected_peer_count, get_connected_peers,
    get_contribution, get_listen_addrs, get_peer_connection_kinds, is_circuit, is_peer_connected,
    is_routable_multiaddr, open_peer_stream, str_to_multiaddr,
};

use commands::handle_swarm_command;
use event_loop::handle_swarm_event;

pub const STREAM_PROTOCOL: StreamProtocol = StreamProtocol::new("/kursal/transfer/1.0.0");
pub const CALL_PROTOCOL: StreamProtocol = StreamProtocol::new("/kursal/call/1.0.0");
pub const VIDEO_PROTOCOL: StreamProtocol = StreamProtocol::new("/kursal/call-video/1.0.0");
pub const MAX_MESSAGE_SIZE: usize = 512 * 1024; // 512 KB, should LARGE be enough
pub const FILE_CHUNK_SIZE: usize = 256 * 1024;

pub const CONTRIBUTES: bool = !cfg!(any(target_os = "android", target_os = "ios"));

pub const MAX_RELAY_RESERVATIONS: usize = 3;
pub const MAX_RELAY_CANDIDATES: usize = 64;

#[derive(Debug, Clone, Copy, Default)]
pub struct RelayCandidate {
    pub rtt: Option<Duration>,
    pub failures: u32,
}

impl RelayCandidate {
    pub fn decay(&mut self) {
        self.failures /= 2;
    }

    pub fn rank(&self) -> (u32, Duration) {
        (self.failures, self.rtt.unwrap_or(Duration::MAX))
    }
}

pub fn best_relay_candidates(
    candidates: &HashMap<PeerId, RelayCandidate>,
    limit: usize,
) -> Vec<PeerId> {
    let mut ranked: Vec<(PeerId, RelayCandidate)> =
        candidates.iter().map(|(p, c)| (*p, *c)).collect();
    ranked.sort_by_key(|(peer, candidate)| (candidate.rank(), peer.to_bytes()));
    ranked
        .into_iter()
        .take(limit)
        .map(|(peer, _)| peer)
        .collect()
}

pub fn prune_relay_candidates(candidates: &mut HashMap<PeerId, RelayCandidate>) {
    if candidates.len() <= MAX_RELAY_CANDIDATES {
        return;
    }
    let keep: HashSet<PeerId> = best_relay_candidates(candidates, MAX_RELAY_CANDIDATES)
        .into_iter()
        .collect();
    candidates.retain(|peer, _| keep.contains(peer));
}

pub fn relay_provider_key() -> libp2p::kad::RecordKey {
    use sha2::{Digest, Sha256};
    libp2p::kad::RecordKey::new(&Sha256::digest(b"kursal/relay/1").to_vec())
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONTRIB_MAX_CIRCUITS: usize = 64;
const CONTRIB_MAX_CIRCUITS_PER_PEER: usize = 4;
const CONTRIB_MAX_RESERVATIONS: usize = 32;
const CONTRIB_MAX_CIRCUIT_DURATION: Duration = Duration::from_secs(10 * 60);
const CONTRIB_MAX_CIRCUIT_BYTES: u64 = 64 * 1024 * 1024;

pub struct StreamWrite {
    pub data: Vec<u8>,
    pub written: Option<oneshot::Sender<()>>,
}

pub type PeerStreams = Arc<Mutex<HashMap<PeerId, mpsc::Sender<StreamWrite>>>>;

pub fn lock_peer_streams(
    peer_streams: &PeerStreams,
) -> std::sync::MutexGuard<'_, HashMap<PeerId, mpsc::Sender<StreamWrite>>> {
    peer_streams
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[allow(clippy::large_enum_variant)]
pub enum SwarmCommand {
    Dial(Multiaddr),
    DialLocal {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    DialPeer {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    AddNode(Multiaddr),
    DialOnce {
        addr: Multiaddr,
        reply_tx: oneshot::Sender<std::result::Result<(), String>>,
    },
    SendMessage {
        peer_id: PeerId,
        data: Vec<u8>,
        addresses: Vec<Multiaddr>,
    },
    OpenStream {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        reply: oneshot::Sender<Option<mpsc::Sender<StreamWrite>>>,
    },
    OpenCallStream {
        peer_id: PeerId,
        reply: oneshot::Sender<Option<libp2p::Stream>>,
    },
    OpenVideoStream {
        peer_id: PeerId,
        reply: oneshot::Sender<Option<libp2p::Stream>>,
    },
    PublishDht {
        key: Vec<u8>,
        value: Vec<u8>,
        expires: Option<u64>,
        reply_tx: Option<oneshot::Sender<bool>>,
    },
    FetchDht {
        key: Vec<u8>,
        reply_tx: mpsc::Sender<Vec<u8>>,
    },
    RemoveDht {
        key: Vec<u8>,
    },
    Shutdown,
    EnableNearby,
    DisableNearby,
    EnsureRelayReservations,
    ContactAdded {
        contact: Contact,
    },
    ContactRemoved {
        peer_id: String,
    },
    GetListenAddresses {
        reply_tx: oneshot::Sender<Vec<Multiaddr>>,
    },
    IsPeerConnected {
        peer_id: PeerId,
        reply_tx: oneshot::Sender<bool>,
    },
    GetConnectedPeerCount {
        reply_tx: oneshot::Sender<usize>,
    },
    GetConnectedPeers {
        reply_tx: oneshot::Sender<Vec<PeerId>>,
    },
    GetPeerConnectionKinds {
        reply_tx: oneshot::Sender<HashMap<PeerId, ConnectionKind>>,
    },
    GetContribution {
        reply_tx: oneshot::Sender<ContributionStatus>,
    },
    DiscoverRelays,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ConnectionKind {
    Relay,
    HolePunch,
    Direct,
}

impl ConnectionKind {
    pub fn rank(self) -> u8 {
        match self {
            ConnectionKind::Relay => 0,
            ConnectionKind::HolePunch => 1,
            ConnectionKind::Direct => 2,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Reachability {
    Checking,
    Private,
    Public,
}

#[derive(Debug, Clone, Copy)]
pub struct ContributionStatus {
    pub reachability: Reachability,
    pub dht_server: bool,
    pub relay_active: bool,
    pub reservations: usize,
    pub circuits: usize,
}

pub enum NetworkEvent {
    MessageReceived {
        from: PeerId,
        data: Vec<u8>,
    },
    PeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    LocalPeerDiscovered {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
    },
    ConnectionEstablished {
        peer_id: PeerId,
        via: ConnectionKind,
    },
    ConnectionKindChanged {
        peer_id: PeerId,
        via: ConnectionKind,
    },
    ConnectionPending {
        peer_id: PeerId,
    },
    ConnectionFailed {
        peer_id: PeerId,
    },
    ConnectionLost {
        peer_id: PeerId,
    },
    DhtFetchResult {
        key: Vec<u8>,
        value: Option<Vec<u8>>,
    },
    SendFailed {
        peer_id: PeerId,
    },
    ReachabilityChanged {
        reachability: Reachability,
    },
}

#[derive(Clone)]
pub struct SwarmHandle {
    pub peer_id: PeerId,
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub relay_config: RelayConfig,
    pub mdns_enabled: bool,
    pub port: u16,
}

impl SwarmHandle {
    pub async fn spawn(
        identity: TransportIdentity,
        event_tx: mpsc::Sender<NetworkEvent>,
        chunk_tx: mpsc::Sender<(PeerId, FileTransferMessage)>,
        relay_config: RelayConfig,
        mdns_enabled: bool,
        port: u16,
    ) -> Result<Self> {
        let swarm = SwarmBuilder::with_existing_identity(identity.keypair)
            .with_tokio()
            .with_tcp(
                Default::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .map_err(|err| KursalError::Network(format!("swarm tcp error: {err}")))?
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

        let swarm = swarm
            .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)
            .map_err(|err| KursalError::Network(format!("swarm relay error: {err}")))?;

        let swarm = {
            let registry = crate::stats::global_registry();
            let mut registry = registry
                .lock()
                .map_err(|_| KursalError::Network("stats registry poisoned".to_string()))?;
            swarm.with_bandwidth_metrics(&mut registry)
        };

        let mut swarm = swarm
            .with_behaviour(|key, relay_client|  -> std::result::Result<_, Box<dyn std::error::Error + Send + Sync>> {
                let local_peer_id = key.public().to_peer_id();

                let relay = relay_client;
                let dcutr = libp2p::dcutr::Behaviour::new(local_peer_id);

                let mut kad_config = libp2p::kad::Config::new(StreamProtocol::new("/kursal/kad/1.0.0"));
                kad_config.set_max_packet_size(KAD_MAX_PACKET);
                kad_config.set_record_filtering(libp2p::kad::StoreInserts::FilterBoth);
                kad_config.set_record_ttl(Some(Duration::from_secs(3 * 7 * 24 * 60 * 60))); // 3 weeks

                let kad = libp2p::kad::Behaviour::with_config(
                    local_peer_id,
                    KursalKadStore::new(local_peer_id),
                    kad_config,
                );

                #[cfg(not(target_os = "ios"))]
                let mdns = if mdns_enabled {
                    Toggle::from(Some(libp2p::mdns::tokio::Behaviour::new(mdns::Config {
                        query_interval: Duration::from_secs(60),
                        ttl: Duration::from_secs(2 * 60),
                        enable_ipv6: false,
                    }, local_peer_id)?))
                } else {
                    log::info!("mDNS disabled");
                    Toggle::from(None)
                };
                #[cfg(target_os = "ios")]
                let _ = mdns_enabled;

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "/kursal/v1.0.0".to_string(),
                    key.public(),
                ));

                let request_response = request_response::Behaviour::new(
                    [(StreamProtocol::new("/kursal/msg/1.0.0"), ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_request_timeout(REQUEST_TIMEOUT),
                );

                let relay_server = if CONTRIBUTES {
                    Toggle::from(Some(libp2p::relay::Behaviour::new(local_peer_id, libp2p::relay::Config {
                        max_circuits: CONTRIB_MAX_CIRCUITS,
                        max_circuits_per_peer: CONTRIB_MAX_CIRCUITS_PER_PEER,
                        max_reservations: CONTRIB_MAX_RESERVATIONS,
                        max_circuit_duration: CONTRIB_MAX_CIRCUIT_DURATION,
                        max_circuit_bytes: CONTRIB_MAX_CIRCUIT_BYTES,
                        reservation_rate_limiters: Vec::new(),
                        circuit_src_rate_limiters: Vec::new(),
                        ..Default::default()
                    })))
                } else {
                    Toggle::from(None)
                };

                let autonat_client = libp2p::autonat::v2::client::Behaviour::default();

                let autonat_server = if CONTRIBUTES {
                    Toggle::from(Some(libp2p::autonat::v2::server::Behaviour::default()))
                } else {
                    Toggle::from(None)
                };

                let streaming = libp2p_stream::Behaviour::new();

                let limiter = if CONTRIBUTES {
                    ConnectionLimiter::new(
                        relay_config.max_connections,
                        relay_config.max_connections_per_ip,
                        None,
                    )
                } else {
                    ConnectionLimiter::new(0, 0, Some(MAX_TRANSIENT_CONNECTIONS))
                };

                let ping = libp2p::ping::Behaviour::new(
                    libp2p::ping::Config::new()
                        .with_interval(Duration::from_secs(15))
                        .with_timeout(Duration::from_secs(20)),
                );

                Ok(KursalBehaviour {
                    relay,
                    relay_server,
                    dcutr,
                    autonat_client,
                    autonat_server,
                    kad,
                    #[cfg(not(target_os = "ios"))]
                    mdns,
                    identify,
                    request_response,
                    streaming,
                    limiter,
                    ping,
                })
            })
            .map_err(|err| KursalError::Network(format!("swarm behaviour error: {err}")))?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(600)))
            .build();

        swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{port}").parse()?)
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;
        swarm
            .listen_on(format!("/ip4/0.0.0.0/udp/{port}/quic-v1").parse()?)
            .map_err(|err| KursalError::Network(format!("swarm listen error: {err}")))?;

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<SwarmCommand>(32);
        let peer_id = identity.peer_id;

        let mut incoming_control = swarm.behaviour().streaming.new_control();
        let incoming_event_tx = event_tx.clone();
        let incoming_chunk_tx = chunk_tx.clone();
        tokio::spawn(async move {
            let mut incoming = match incoming_control.accept(STREAM_PROTOCOL) {
                Ok(incoming) => incoming,
                Err(err) => {
                    log::error!("[swarm] cannot accept incoming streams: {err}");
                    return;
                }
            };
            while let Some((peer_id, stream)) = incoming.next().await {
                tokio::spawn(handle_incoming_stream(
                    peer_id,
                    stream,
                    incoming_event_tx.clone(),
                    incoming_chunk_tx.clone(),
                ));
            }
        });

        #[cfg(feature = "calls")]
        {
            let mut call_incoming_control = swarm.behaviour().streaming.new_control();
            tokio::spawn(async move {
                let mut incoming = match call_incoming_control.accept(CALL_PROTOCOL) {
                    Ok(i) => i,
                    Err(err) => {
                        log::error!("[call] failed to accept call protocol: {err}");
                        return;
                    }
                };
                while let Some((peer_id, stream)) = incoming.next().await {
                    crate::call::media::deliver_incoming_stream(peer_id, stream).await;
                }
            });

            let mut video_incoming_control = swarm.behaviour().streaming.new_control();
            tokio::spawn(async move {
                let mut incoming = match video_incoming_control.accept(VIDEO_PROTOCOL) {
                    Ok(i) => i,
                    Err(err) => {
                        log::error!("[call] failed to accept video protocol: {err}");
                        return;
                    }
                };
                while let Some((peer_id, stream)) = incoming.next().await {
                    crate::call::video::deliver_incoming_stream(peer_id, stream).await;
                }
            });
        }

        tokio::spawn(async move {
            log::info!("[swarm] event loop started");
            let mut pending_queries: HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>> =
                HashMap::new();
            let mut pending_puts: HashMap<libp2p::kad::QueryId, oneshot::Sender<bool>> =
                HashMap::new();
            let mut pending_dials: HashMap<
                ConnectionId,
                oneshot::Sender<std::result::Result<(), String>>,
            > = HashMap::new();
            let mut intentional_dials: HashMap<ConnectionId, PeerId> = HashMap::new();
            let mut listen_addresses: HashSet<Multiaddr> = HashSet::new();
            let mut nearby_enabled = false;
            let mut mdns_peers: HashMap<PeerId, Multiaddr> = HashMap::new();
            let mut peer_conns: HashMap<ConnectionId, (PeerId, ConnectionKind)> = HashMap::new();
            let mut discovered_relays: HashMap<PeerId, RelayCandidate> = HashMap::new();
            let mut circuit_listeners: HashMap<libp2p::core::transport::ListenerId, PeerId> =
                HashMap::new();
            let mut contribution = ContributionStatus {
                reachability: Reachability::Checking,
                dht_server: false,
                relay_active: false,
                reservations: 0,
                circuits: 0,
            };

            let mut stream_control = swarm.behaviour().streaming.new_control();
            let peer_streams: PeerStreams = Arc::new(Mutex::new(HashMap::new()));
            let (validated_tx, mut validated_rx) =
                mpsc::channel::<libp2p::kad::Record>(KAD_VALIDATED_QUEUE);

            let mut node_addrs: Vec<Multiaddr> = bootstrap_peers();

            for multiaddr in bootstrap_peers() {
                if let Some(Protocol::P2p(peer_id)) = multiaddr.iter().last() {
                    swarm.behaviour_mut().limiter.protect(peer_id);
                    swarm
                        .behaviour_mut()
                        .kad
                        .add_address(&peer_id, multiaddr.clone());
                    if let Err(err) = swarm.dial(multiaddr.clone()) {
                        log::warn!("Bootstrap peer {multiaddr} could not be dialed: {err:?}");
                    } else {
                        log::info!("[bootstrap] dialing {multiaddr}");
                    }
                }
            }

            loop {
                tokio::select! {
                    event = swarm.select_next_some() => handle_swarm_event(event, &event_tx, &mut pending_queries, &mut pending_puts, &mut pending_dials, &mut intentional_dials, &mut listen_addresses, &mut swarm, nearby_enabled, &mut mdns_peers, &mut peer_conns, &peer_streams, &validated_tx, &node_addrs, &mut discovered_relays, &mut circuit_listeners, &mut contribution).await,
                    Some(record) = validated_rx.recv() => {
                        if let Err(err) = swarm.behaviour_mut().kad.store_mut().put(record) {
                            log::debug!("[kad] validated record not stored: {err:?}");
                        }
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(SwarmCommand::Shutdown) => break,
                            Some(SwarmCommand::EnableNearby) => {
                                nearby_enabled = true;
                                for (peer_id, addr) in mdns_peers.iter() {
                                    let _ = event_tx.send(NetworkEvent::PeerDiscovered { peer_id: *peer_id, addresses: vec![addr.clone()] }).await;
                                }

                                log::info!("Nearby enabled ({} known mdns peers)", mdns_peers.len());
                            },
                            Some(cmd) => handle_swarm_command(cmd, &mut swarm, &mut pending_queries, &mut pending_puts, &mut pending_dials, &mut intentional_dials, &mut listen_addresses, &mut nearby_enabled, &mut stream_control, &peer_streams, &peer_conns, &mut node_addrs, &mut discovered_relays, &contribution).await,
                            None => break
                        }
                    }
                }
            }
        });

        Ok(SwarmHandle {
            peer_id,
            cmd_tx,
            relay_config,
            mdns_enabled,
            port,
        })
    }
}
