use super::{
    CALL_PROTOCOL, ConnInfo, ConnectionKind, ContributionStatus, KursalBehaviour,
    MAX_RELAY_RESERVATIONS, PeerStreams, RelayCandidate, SwarmCommand, VIDEO_PROTOCOL,
    best_relay_candidates,
    helpers::{open_peer_stream, peer_of, reserved_relay_count},
    lock_peer_streams, relay_provider_key,
};
use libp2p::{
    Multiaddr, PeerId, Swarm,
    multiaddr::Protocol,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_swarm_command(
    cmd: SwarmCommand,
    swarm: &mut Swarm<KursalBehaviour>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    pending_puts: &mut HashMap<libp2p::kad::QueryId, oneshot::Sender<bool>>,
    pending_dials: &mut HashMap<ConnectionId, oneshot::Sender<std::result::Result<(), String>>>,
    intentional_dials: &mut HashMap<ConnectionId, PeerId>,
    listen_addresses: &mut HashSet<Multiaddr>,
    nearby_enabled: &mut bool,
    stream_control: &mut libp2p_stream::Control,
    peer_streams: &PeerStreams,
    peer_conns: &HashMap<ConnectionId, ConnInfo>,
    node_addrs: &mut Vec<Multiaddr>,
    discovered_relays: &mut HashMap<PeerId, RelayCandidate>,
    contribution: &ContributionStatus,
) {
    match cmd {
        SwarmCommand::Shutdown | SwarmCommand::EnableNearby => {} // handled in the loop itself
        SwarmCommand::Dial(addr) => {
            let _ = swarm.dial(addr);
        }
        SwarmCommand::DialLocal { peer_id, addresses } => {
            let has_direct = peer_conns
                .values()
                .any(|info| info.peer == peer_id && info.kind != ConnectionKind::Relay);

            if !has_direct {
                let opts = DialOpts::peer_id(peer_id)
                    .addresses(addresses)
                    .condition(PeerCondition::NotDialing)
                    .build();
                let connection_id = opts.connection_id();
                match swarm.dial(opts) {
                    Ok(()) => {
                        intentional_dials.insert(connection_id, peer_id);
                    }
                    Err(err) => {
                        log::debug!("[mDNS] local dial to {peer_id} failed: {err:?}");
                    }
                }
            }
        }
        SwarmCommand::DialPeer { peer_id, addresses } => {
            if addresses.is_empty() {
                return;
            }
            let opts = DialOpts::peer_id(peer_id)
                .addresses(addresses)
                .condition(PeerCondition::DisconnectedAndNotDialing)
                .build();
            let connection_id = opts.connection_id();
            match swarm.dial(opts) {
                Ok(()) => {
                    intentional_dials.insert(connection_id, peer_id);
                }
                Err(err) => {
                    log::debug!("[presence] dial to {peer_id} skipped: {err:?}");
                }
            }
        }
        SwarmCommand::AddNode(addr) => {
            if let Some(peer_id) = peer_of(&addr) {
                swarm.behaviour_mut().limiter.protect(peer_id);
                swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());
                if !node_addrs.contains(&addr) {
                    node_addrs.push(addr.clone());
                }
            }
            if let Err(err) = swarm.dial(addr) {
                log::warn!("[node] custom node dial failed: {err:?}");
            }
        }
        SwarmCommand::EnsureRelayReservations => {
            for addr in node_addrs.clone() {
                let Some(relay_id) = peer_of(&addr) else {
                    continue;
                };
                let reserved = listen_addresses.iter().any(|a| {
                    a.iter().any(|p| matches!(p, Protocol::P2pCircuit))
                        && a.iter()
                            .any(|p| matches!(p, Protocol::P2p(id) if id == relay_id))
                });
                if reserved {
                    continue;
                }
                if swarm.is_connected(&relay_id) {
                    let circuit = addr.clone().with(Protocol::P2pCircuit);
                    log::info!("[relay] re-reserving via {relay_id}");
                    let _ = swarm.listen_on(circuit);
                } else {
                    log::info!("[relay] re-dialing relay {relay_id}");
                    let _ = swarm.dial(addr.clone());
                }
            }
        }
        SwarmCommand::DialOnce { addr, reply_tx } => {
            let opts: DialOpts = addr.into();
            let connection_id = opts.connection_id();
            match swarm.dial(opts) {
                Ok(()) => {
                    pending_dials.insert(connection_id, reply_tx);
                }
                Err(err) => {
                    let _ = reply_tx.send(Err(err.to_string()));
                }
            }
        }
        SwarmCommand::DisableNearby => {
            *nearby_enabled = false;
            log::info!("Nearby disabled");
        }
        SwarmCommand::PublishDht {
            key,
            value,
            expires,
            reply_tx,
        } => {
            let key_dbg = hex::encode(&key[..key.len().min(8)]);
            let connected_peers = swarm.connected_peers().count();
            let kad_known: usize = swarm
                .behaviour_mut()
                .kad
                .kbuckets()
                .map(|b| b.num_entries())
                .sum();
            log::info!(
                "[kad] PublishDht key={key_dbg} value_bytes={} connected_peers={connected_peers} kad_kbucket_entries={kad_known}",
                value.len()
            );
            if kad_known == 0 {
                log::warn!(
                    "[kad] PublishDht key={key_dbg}: no peers in routing table -> put will fail"
                );
            }
            let record = libp2p::kad::Record {
                key: libp2p::kad::RecordKey::new(&key),
                value,
                publisher: None,
                expires: expires
                    .and_then(|secs| Instant::now().checked_add(Duration::from_secs(secs))),
            };

            match swarm
                .behaviour_mut()
                .kad
                .put_record(record, libp2p::kad::Quorum::One) // TODO: up to 2-3 (it's the amount of storing confirmations required for a success)
            {
                Ok(query_id) => {
                    log::info!("[kad] PutRecord started key={key_dbg} query_id={query_id:?}");
                    if let Some(reply_tx) = reply_tx {
                        pending_puts.insert(query_id, reply_tx);
                    }
                }
                Err(err) => {
                    log::error!("[kad] PutRecord failed to start key={key_dbg}: {err:?}");
                    if let Some(reply_tx) = reply_tx {
                        let _ = reply_tx.send(false);
                    }
                }
            }
        }
        SwarmCommand::RemoveDht { key } => {
            let key_dbg = hex::encode(&key[..key.len().min(8)]);
            swarm
                .behaviour_mut()
                .kad
                .remove_record(&libp2p::kad::RecordKey::new(&key));
            log::info!("[kad] RemoveDht key={key_dbg}");
        }
        SwarmCommand::FetchDht { key, reply_tx } => {
            let key_dbg = hex::encode(&key[..key.len().min(8)]);
            let connected_peers = swarm.connected_peers().count();
            let kad_known: usize = swarm
                .behaviour_mut()
                .kad
                .kbuckets()
                .map(|b| b.num_entries())
                .sum();
            log::info!(
                "[kad] FetchDht key={key_dbg} connected_peers={connected_peers} kad_kbucket_entries={kad_known}"
            );
            if kad_known == 0 {
                log::warn!(
                    "[kad] FetchDht key={key_dbg}: no peers in routing table -> get will fail"
                );
            }
            let query_id = swarm
                .behaviour_mut()
                .kad
                .get_record(libp2p::kad::RecordKey::new(&key));
            pending_queries.insert(query_id, reply_tx);
        }
        SwarmCommand::ContactAdded { contact } => {
            if let Ok(peer_id) = contact.peer_id.parse::<PeerId>() {
                swarm.behaviour_mut().limiter.protect(peer_id);
            }
            for addr_str in &contact.known_addresses {
                if let Ok(addr) = addr_str.parse::<Multiaddr>()
                    && let Some(Protocol::P2p(peer_id)) = addr.iter().last()
                {
                    swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                }
            }
        }
        SwarmCommand::ContactRemoved { peer_id } => {
            if let Ok(peer_id) = peer_id.parse::<PeerId>() {
                swarm.behaviour_mut().limiter.unprotect(&peer_id);
            }
        }
        SwarmCommand::SendMessage {
            peer_id,
            data,
            addresses,
        } => {
            swarm
                .behaviour_mut()
                .request_response
                .send_request_with_addresses(&peer_id, data, addresses);
        }
        SwarmCommand::GetListenAddresses { reply_tx } => {
            let addrs: Vec<Multiaddr> = listen_addresses.iter().cloned().collect();
            let _ = reply_tx.send(addrs);
        }
        SwarmCommand::IsPeerConnected { peer_id, reply_tx } => {
            let _ = reply_tx.send(swarm.is_connected(&peer_id));
        }
        SwarmCommand::GetConnectedPeerCount { reply_tx } => {
            let _ = reply_tx.send(swarm.connected_peers().count());
        }
        SwarmCommand::GetConnectedPeers { reply_tx } => {
            let peers: Vec<libp2p::PeerId> = swarm.connected_peers().cloned().collect();
            let _ = reply_tx.send(peers);
        }
        SwarmCommand::GetPeerConnectionKinds { reply_tx } => {
            let mut best: HashMap<PeerId, ConnectionKind> = HashMap::new();
            for info in peer_conns.values() {
                best.entry(info.peer)
                    .and_modify(|current| {
                        if info.kind.rank() > current.rank() {
                            *current = info.kind;
                        }
                    })
                    .or_insert(info.kind);
            }
            let _ = reply_tx.send(best);
        }
        SwarmCommand::DiscoverRelays => {
            for candidate in discovered_relays.values_mut() {
                candidate.decay();
            }

            let reserved = reserved_relay_count(listen_addresses);
            if reserved >= MAX_RELAY_RESERVATIONS {
                return;
            }

            let wanted = MAX_RELAY_RESERVATIONS.saturating_sub(reserved);
            for peer_id in best_relay_candidates(discovered_relays, wanted) {
                if swarm.is_connected(&peer_id) {
                    continue;
                }
                let opts = DialOpts::peer_id(peer_id)
                    .condition(PeerCondition::DisconnectedAndNotDialing)
                    .build();
                if let Err(err) = swarm.dial(opts) {
                    log::debug!("[relay] discovered relay dial skipped: {err:?}");
                }
            }
            let query = swarm
                .behaviour_mut()
                .kad
                .get_providers(relay_provider_key());
            log::info!("[relay] querying DHT for relay providers query={query:?}");
        }
        SwarmCommand::GetContribution { reply_tx } => {
            let mut status = *contribution;
            status.dht_server = swarm.behaviour_mut().kad.mode() == libp2p::kad::Mode::Server;
            status.relay_active = swarm.behaviour().relay_server.is_enabled();
            let _ = reply_tx.send(status);
        }
        SwarmCommand::OpenStream {
            peer_id,
            addresses,
            reply,
        } => {
            let connected = swarm.is_connected(&peer_id);
            if !connected && !addresses.is_empty() {
                let opts = DialOpts::peer_id(peer_id)
                    .addresses(addresses)
                    .condition(PeerCondition::DisconnectedAndNotDialing)
                    .build();
                if let Err(err) = swarm.dial(opts) {
                    log::debug!("[stream] pre-dial to {peer_id} skipped: {err:?}");
                }
            }

            let cached = {
                let mut map = lock_peer_streams(peer_streams);
                match map.get(&peer_id) {
                    Some(tx) if connected && !tx.is_closed() => Some(tx.clone()),
                    Some(_) => {
                        map.remove(&peer_id);
                        None
                    }
                    None => None,
                }
            };

            if let Some(tx) = cached {
                let _ = reply.send(Some(tx));
            } else {
                let control = stream_control.clone();
                let peer_streams = peer_streams.clone();
                tokio::spawn(async move {
                    let sender = open_peer_stream(control, peer_id, &peer_streams).await;
                    let _ = reply.send(sender);
                });
            }
        }
        SwarmCommand::OpenCallStream { peer_id, reply } => {
            let mut control = stream_control.clone();
            tokio::spawn(async move {
                let stream = control.open_stream(peer_id, CALL_PROTOCOL).await.ok();
                let _ = reply.send(stream);
            });
        }
        SwarmCommand::OpenVideoStream { peer_id, reply } => {
            let mut control = stream_control.clone();
            tokio::spawn(async move {
                let stream = control.open_stream(peer_id, VIDEO_PROTOCOL).await.ok();
                let _ = reply.send(stream);
            });
        }
    }
}
