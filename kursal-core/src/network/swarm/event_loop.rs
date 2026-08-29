use super::{
    CONTRIBUTES, ConnectionKind, ContributionStatus, KursalBehaviour, KursalBehaviourEvent,
    MAX_RELAY_RESERVATIONS, NetworkEvent, PeerStreams, Reachability, RelayCandidate,
    helpers::{
        any_public_address, dial_error_summary, is_circuit, is_node_peer, is_routable_multiaddr,
        reserved_relay_count,
    },
    lock_peer_streams, prune_relay_candidates, relay_provider_key,
};
use crate::network::kademlia::spawn_record_validation;
#[cfg(not(target_os = "ios"))]
use libp2p::mdns;
use libp2p::{
    Multiaddr, PeerId, Swarm,
    multiaddr::Protocol,
    request_response,
    swarm::{ConnectionId, SwarmEvent},
};
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, oneshot};

fn has_public_address(swarm: &Swarm<KursalBehaviour>) -> bool {
    any_public_address(swarm.external_addresses())
}

fn apply_dht_mode(swarm: &mut Swarm<KursalBehaviour>, public: bool) {
    let mode = if public {
        libp2p::kad::Mode::Server
    } else {
        libp2p::kad::Mode::Client
    };
    swarm.behaviour_mut().kad.set_mode(Some(mode));
}

fn penalise_circuit_listener(
    listener_id: libp2p::core::transport::ListenerId,
    circuit_listeners: &mut HashMap<libp2p::core::transport::ListenerId, PeerId>,
    discovered_relays: &mut HashMap<PeerId, RelayCandidate>,
) {
    let Some(peer_id) = circuit_listeners.remove(&listener_id) else {
        return;
    };
    if let Some(candidate) = discovered_relays.get_mut(&peer_id) {
        candidate.failures = candidate.failures.saturating_add(1);
        log::info!(
            "[relay] {peer_id} dropped a reservation (failures={})",
            candidate.failures
        );
    }
}

fn best_kind(
    peer_conns: &HashMap<ConnectionId, (PeerId, ConnectionKind)>,
    peer_id: &PeerId,
) -> Option<ConnectionKind> {
    peer_conns
        .values()
        .filter(|(p, _)| p == peer_id)
        .map(|(_, k)| *k)
        .max_by_key(|k| k.rank())
}

fn prune_duplicate_connections(
    swarm: &mut Swarm<KursalBehaviour>,
    peer_conns: &HashMap<ConnectionId, (PeerId, ConnectionKind)>,
    peer_streams: &PeerStreams,
    node_addrs: &[Multiaddr],
    peer_id: PeerId,
    reason: &str,
) {
    if is_node_peer(node_addrs, &peer_id) {
        return;
    }

    let mut conns: Vec<(ConnectionId, ConnectionKind)> = peer_conns
        .iter()
        .filter_map(|(cid, (p, kind))| (*p == peer_id).then_some((*cid, *kind)))
        .collect();

    if conns.len() < 2 {
        return;
    }

    conns.sort_by_key(|(cid, kind)| (std::cmp::Reverse(kind.rank()), *cid));
    let (keep, _) = conns.remove(0);

    for (cid, kind) in conns {
        log::info!("[conn] {reason}: closing {kind:?} conn {cid:?} to {peer_id}, keeping {keep:?}");
        swarm.close_connection(cid);
    }

    lock_peer_streams(peer_streams).remove(&peer_id);
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    pending_puts: &mut HashMap<libp2p::kad::QueryId, oneshot::Sender<bool>>,
    pending_dials: &mut HashMap<ConnectionId, oneshot::Sender<std::result::Result<(), String>>>,
    intentional_dials: &mut HashMap<ConnectionId, PeerId>,
    listen_addresses: &mut HashSet<Multiaddr>,
    swarm: &mut Swarm<KursalBehaviour>,
    #[allow(unused_variables)] nearby_enabled: bool,
    #[allow(unused_variables)] mdns_peers: &mut HashMap<PeerId, Multiaddr>,
    peer_conns: &mut HashMap<ConnectionId, (PeerId, ConnectionKind)>,
    peer_streams: &PeerStreams,
    validated_tx: &mpsc::Sender<libp2p::kad::Record>,
    node_addrs: &[Multiaddr],
    discovered_relays: &mut HashMap<PeerId, RelayCandidate>,
    circuit_listeners: &mut HashMap<libp2p::core::transport::ListenerId, PeerId>,
    contribution: &mut ContributionStatus,
) {
    match event {
        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(libp2p::kad::Event::InboundRequest {
            request:
                libp2p::kad::InboundRequest::PutRecord {
                    record: Some(record),
                    ..
                },
        })) => spawn_record_validation(record, validated_tx.clone()),

        SwarmEvent::Behaviour(KursalBehaviourEvent::Kad(
            libp2p::kad::Event::OutboundQueryProgressed { id, result, .. },
        )) => match result {
            libp2p::kad::QueryResult::GetRecord(Ok(libp2p::kad::GetRecordOk::FoundRecord(
                peer_record,
            ))) => {
                if let Some(tx) = pending_queries.get(&id) {
                    let _ = tx.send(peer_record.record.value).await;
                }
            }
            libp2p::kad::QueryResult::GetRecord(Ok(
                libp2p::kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. },
            )) => {
                log::debug!(
                    "[kad] GET record query={:?} finished with no additional record",
                    id
                );
                pending_queries.remove(&id);
            }
            libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                if matches!(e, libp2p::kad::GetRecordError::NotFound { .. }) {
                    log::debug!("[kad] GET record not found query={:?}", id);
                } else {
                    log::warn!("[kad] GET record failed query={:?} error={:?}", id, e);
                }
                pending_queries.remove(&id);
            }
            libp2p::kad::QueryResult::GetProviders(Ok(
                libp2p::kad::GetProvidersOk::FoundProviders { providers, .. },
            )) => {
                let local = *swarm.local_peer_id();
                for provider in providers {
                    if provider == local || discovered_relays.contains_key(&provider) {
                        continue;
                    }
                    discovered_relays.insert(provider, RelayCandidate::default());
                    log::info!("[relay] discovered relay provider {provider}");
                    swarm.behaviour_mut().limiter.protect(provider);
                }
                prune_relay_candidates(discovered_relays);
            }
            libp2p::kad::QueryResult::PutRecord(Ok(_)) => {
                log::debug!("[kad] PUT record succeeded query={:?}", id);
                if let Some(tx) = pending_puts.remove(&id) {
                    let _ = tx.send(true);
                }
            }
            libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                log::warn!("[kad] PUT record failed query={:?} error={:?}", id, e);
                if let Some(tx) = pending_puts.remove(&id) {
                    let _ = tx.send(false);
                }
            }
            _ => {}
        },

        SwarmEvent::Behaviour(KursalBehaviourEvent::Identify(
            libp2p::identify::Event::Received { peer_id, info, .. },
        )) => {
            log::debug!(
                "[identify] received from peer {}: protocols={:?}, listen_addrs={:?}",
                peer_id,
                info.protocols,
                info.listen_addrs
            );

            for addr in info.listen_addrs {
                if !is_routable_multiaddr(&addr) {
                    log::trace!("[identify] skip unroutable {addr} from {peer_id}");
                    continue;
                }

                swarm.behaviour_mut().kad.add_address(&peer_id, addr);
            }
        }
        #[cfg(not(target_os = "ios"))]
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(
            peers,
        ))) => {
            log::debug!(
                "[mDNS] raw Discovered event, nearby_enabled={}, peers={}",
                nearby_enabled,
                peers.len()
            );

            for (peer_id, addr) in peers {
                if !is_routable_multiaddr(&addr) {
                    log::trace!("[mDNS] skip unroutable {addr} from {peer_id}");
                    continue;
                }

                log::info!("[mDNS] discovered peer {} at {}", peer_id, addr);

                mdns_peers.insert(peer_id, addr.clone());

                if nearby_enabled {
                    let _ = event_tx
                        .send(NetworkEvent::PeerDiscovered {
                            peer_id,
                            addresses: vec![addr.clone()],
                        })
                        .await;
                }

                let _ = event_tx
                    .send(NetworkEvent::LocalPeerDiscovered {
                        peer_id,
                        addresses: vec![addr],
                    })
                    .await;
            }
        }

        // TODO: add a PeerExpired or similar, connection is not lost
        #[cfg(not(target_os = "ios"))]
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, addr) in &peers {
                log::debug!("[mDNS] peer expired {} at {}", peer_id, addr);
                if mdns_peers.get(peer_id) == Some(addr) {
                    mdns_peers.remove(peer_id);
                }
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Dcutr(e)) => match e.result {
            Ok(connection_id) => {
                log::info!(
                    "[dcutr] hole punch succeeded peer={} conn={connection_id:?}",
                    e.remote_peer_id
                );
                peer_conns.insert(connection_id, (e.remote_peer_id, ConnectionKind::HolePunch));

                prune_duplicate_connections(
                    swarm,
                    peer_conns,
                    peer_streams,
                    node_addrs,
                    e.remote_peer_id,
                    "hole punched",
                );

                let _ = event_tx
                    .send(NetworkEvent::ConnectionEstablished {
                        peer_id: e.remote_peer_id,
                        via: ConnectionKind::HolePunch,
                    })
                    .await;
            }
            Err(err) => {
                log::debug!(
                    "[dcutr] hole punch failed peer={} err={err:?}",
                    e.remote_peer_id
                );
            }
        },

        SwarmEvent::Behaviour(KursalBehaviourEvent::AutonatClient(e)) => match e.result {
            Ok(()) => log::info!(
                "[autonat] {} confirmed reachable by {}",
                e.tested_addr,
                e.server
            ),
            Err(err) => {
                log::info!("[autonat] {} not reachable ({err})", e.tested_addr);
                if contribution.reachability == Reachability::Checking {
                    contribution.reachability = Reachability::Private;
                    let _ = event_tx
                        .send(NetworkEvent::ReachabilityChanged {
                            reachability: Reachability::Private,
                        })
                        .await;
                }
            }
        },

        SwarmEvent::ExternalAddrConfirmed { address } => {
            if is_circuit(&address) {
                log::debug!("[reachability] ignoring confirmed circuit address {address}");
                apply_dht_mode(swarm, has_public_address(swarm));
                return;
            }

            log::info!("[reachability] public address confirmed: {address}");
            apply_dht_mode(swarm, true);
            contribution.dht_server = true;
            contribution.relay_active = swarm.behaviour().relay_server.is_enabled();

            if CONTRIBUTES && contribution.relay_active {
                match swarm
                    .behaviour_mut()
                    .kad
                    .start_providing(relay_provider_key())
                {
                    Ok(_) => log::info!("[relay] advertising this node as a relay provider"),
                    Err(err) => log::warn!("[relay] could not advertise as provider: {err:?}"),
                }
            }

            if contribution.reachability != Reachability::Public {
                contribution.reachability = Reachability::Public;
                let _ = event_tx
                    .send(NetworkEvent::ReachabilityChanged {
                        reachability: Reachability::Public,
                    })
                    .await;
            }
        }
        SwarmEvent::ExternalAddrExpired { address } => {
            log::info!("[reachability] external address expired: {address}");
            if has_public_address(swarm) {
                return;
            }

            apply_dht_mode(swarm, false);
            swarm
                .behaviour_mut()
                .kad
                .stop_providing(&relay_provider_key());
            contribution.dht_server = false;

            if contribution.reachability == Reachability::Public {
                contribution.reachability = Reachability::Private;
                let _ = event_tx
                    .send(NetworkEvent::ReachabilityChanged {
                        reachability: Reachability::Private,
                    })
                    .await;
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Ping(e)) => {
            if let Ok(rtt) = e.result
                && let Some(candidate) = discovered_relays.get_mut(&e.peer)
            {
                candidate.rtt = Some(rtt);
            }
            if let Err(failure) = e.result {
                log::info!(
                    "[ping] {} unresponsive on {:?} ({failure}) -> closing dead connection",
                    e.peer,
                    e.connection
                );
                swarm.close_connection(e.connection);
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::ReservationReqAccepted { relay_peer_id, .. },
        )) => {
            if let Some(candidate) = discovered_relays.get_mut(&relay_peer_id) {
                candidate.failures = 0;
            }
            log::info!("[relay] reservation accepted by {relay_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::OutboundCircuitEstablished { relay_peer_id, .. },
        )) => {
            log::info!("[relay] outbound circuit established via {relay_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::Relay(
            libp2p::relay::client::Event::InboundCircuitEstablished { src_peer_id, .. },
        )) => {
            log::info!("[relay] inbound circuit from {src_peer_id}");
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::ReservationReqAccepted {
                src_peer_id,
                renewed,
            },
        )) => {
            if !renewed {
                contribution.reservations = contribution.reservations.saturating_add(1);
            }
            log::info!("[relay] reservation accepted from {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            },
        )) => {
            contribution.circuits = contribution.circuits.saturating_add(1);
            log::info!("[relay] circuit established: {src_peer_id} -> {dst_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitClosed { .. },
        )) => {
            contribution.circuits = contribution.circuits.saturating_sub(1);
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::ReservationTimedOut { src_peer_id }
            | libp2p::relay::Event::ReservationClosed { src_peer_id },
        )) => {
            contribution.reservations = contribution.reservations.saturating_sub(1);
            log::info!("[relay] reservation ended for {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitReqDenied {
                src_peer_id,
                dst_peer_id,
                status,
            },
        )) => {
            log::info!("[relay] circuit denied: {src_peer_id} -> {dst_peer_id} status={status:?}");
        }

        SwarmEvent::ConnectionEstablished {
            peer_id,
            endpoint,
            connection_id,
            ..
        } => {
            if let Some(tx) = pending_dials.remove(&connection_id) {
                let _ = tx.send(Ok(()));
            }
            intentional_dials.remove(&connection_id);
            let is_relayed_check = endpoint.is_relayed()
                || endpoint
                    .get_remote_address()
                    .to_string()
                    .contains("p2p-circuit");
            let kind = if is_relayed_check {
                ConnectionKind::Relay
            } else {
                ConnectionKind::Direct
            };

            peer_conns.insert(connection_id, (peer_id, kind));
            if let Some(candidate) = discovered_relays.get_mut(&peer_id) {
                candidate.failures = 0;
            }
            log::info!(
                "[conn] established peer={peer_id} kind={kind:?} conn={connection_id:?} relayed={is_relayed_check} addr={}",
                endpoint.get_remote_address()
            );

            if kind == ConnectionKind::Direct {
                let is_node = is_node_peer(node_addrs, &peer_id);
                let is_discovered = discovered_relays.contains_key(&peer_id);
                let under_cap = reserved_relay_count(listen_addresses) < MAX_RELAY_RESERVATIONS;

                if is_node || (is_discovered && under_cap) {
                    let circuit_addr = endpoint
                        .get_remote_address()
                        .clone()
                        .with(Protocol::P2pCircuit);
                    match swarm.listen_on(circuit_addr) {
                        Ok(listener_id) => {
                            circuit_listeners.insert(listener_id, peer_id);
                        }
                        Err(err) => log::debug!("[relay] circuit listen failed: {err:?}"),
                    }
                }

                if is_node {
                    log::info!("[kad] Bootstrapping Kademlia with relay");
                    let _ = swarm.behaviour_mut().kad.bootstrap();
                }
            }

            prune_duplicate_connections(
                swarm,
                peer_conns,
                peer_streams,
                node_addrs,
                peer_id,
                "better path available",
            );

            let via = best_kind(peer_conns, &peer_id).unwrap_or(kind);
            let _ = event_tx
                .send(NetworkEvent::ConnectionEstablished { peer_id, via })
                .await;
        }
        SwarmEvent::ConnectionClosed {
            peer_id,
            connection_id,
            num_established,
            ..
        } => {
            peer_conns.remove(&connection_id);
            log::info!(
                "[conn] closed peer={peer_id} conn={connection_id:?} remaining={num_established}"
            );
            lock_peer_streams(peer_streams).remove(&peer_id);

            if num_established == 0 {
                let _ = event_tx
                    .send(NetworkEvent::ConnectionLost { peer_id })
                    .await;
            } else {
                if let Some(via) = best_kind(peer_conns, &peer_id) {
                    log::info!("[conn] peer={peer_id} now via {via:?} after close");
                    let _ = event_tx
                        .send(NetworkEvent::ConnectionKindChanged { peer_id, via })
                        .await;
                }
            }
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RequestResponse(e)) => {
            use request_response::{Event, Message};
            match e {
                Event::Message {
                    peer,
                    message:
                        Message::Request {
                            request, channel, ..
                        },
                    ..
                } => {
                    let _ = swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, vec![]); // INFO: signing is sent from another place
                    let _ = event_tx
                        .send(NetworkEvent::MessageReceived {
                            from: peer,
                            data: request,
                        })
                        .await;
                }
                Event::Message {
                    message: Message::Response { .. },
                    ..
                } => {}
                Event::OutboundFailure { peer, error, .. } => {
                    log::warn!("SendMessage to {peer} failed: {error}");
                    let _ = event_tx
                        .send(NetworkEvent::SendFailed { peer_id: peer })
                        .await;
                }
                _ => {}
            }
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            log::info!("[swarm] listening on {}", address);
            listen_addresses.insert(address);
        }
        SwarmEvent::ExpiredListenAddr { address, .. } => {
            log::info!("[swarm] expired listening on {}", address);
            listen_addresses.remove(&address);
        }
        SwarmEvent::ListenerClosed {
            listener_id,
            addresses,
            ..
        } => {
            for address in &addresses {
                listen_addresses.remove(address);
            }
            penalise_circuit_listener(listener_id, circuit_listeners, discovered_relays);
            log::info!("[swarm] listener closed ({} addrs)", addresses.len());
        }
        SwarmEvent::ListenerError { listener_id, error } => {
            log::debug!("[swarm] listener error: {error}");
            penalise_circuit_listener(listener_id, circuit_listeners, discovered_relays);
        }

        SwarmEvent::Dialing {
            peer_id: Some(peer_id),
            connection_id,
            ..
        } => {
            if intentional_dials.contains_key(&connection_id)
                && best_kind(peer_conns, &peer_id).is_none()
            {
                let _ = event_tx
                    .send(NetworkEvent::ConnectionPending { peer_id })
                    .await;
            }
        }

        SwarmEvent::OutgoingConnectionError {
            peer_id,
            error,
            connection_id,
            ..
        } => {
            let requested = pending_dials.remove(&connection_id);
            let intended = intentional_dials.remove(&connection_id);
            let peer = peer_id.map_or_else(|| "unknown".to_string(), |id| id.to_string());
            let summary = dial_error_summary(&error);

            if let Some(tx) = requested {
                let _ = tx.send(Err(error.to_string()));
                log::info!("[swarm] dial failed peer={peer} error={summary}");
            } else {
                log::debug!("[swarm] dial failed peer={peer} error={summary}");
            }

            if let Some(peer_id) = peer_id
                && let Some(candidate) = discovered_relays.get_mut(&peer_id)
            {
                candidate.failures = candidate.failures.saturating_add(1);
            }

            if let Some(peer_id) = peer_id.or(intended)
                && best_kind(peer_conns, &peer_id).is_none()
            {
                let _ = event_tx
                    .send(NetworkEvent::ConnectionFailed { peer_id })
                    .await;
            }
        }
        SwarmEvent::IncomingConnectionError { error, .. } => {
            log::debug!("[swarm] incoming connection error: {error}");
        }

        _ => {}
    }
}
