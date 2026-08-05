use super::{
    ConnectionKind, KursalBehaviour, KursalBehaviourEvent, NetworkEvent,
    helpers::{dial_error_summary, is_routable_multiaddr},
};
use crate::network::bootstrap::is_bootstrap_peer;
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

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_swarm_event(
    event: SwarmEvent<KursalBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    pending_queries: &mut HashMap<libp2p::kad::QueryId, mpsc::Sender<Vec<u8>>>,
    pending_dials: &mut HashMap<ConnectionId, oneshot::Sender<std::result::Result<(), String>>>,
    listen_addresses: &mut HashSet<Multiaddr>,
    swarm: &mut Swarm<KursalBehaviour>,
    #[allow(unused_variables)] nearby_enabled: bool,
    #[allow(unused_variables)] mdns_peers: &mut HashMap<PeerId, Multiaddr>,
    peer_conns: &mut HashMap<ConnectionId, (PeerId, ConnectionKind)>,
    validated_tx: &mpsc::Sender<libp2p::kad::Record>,
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
            libp2p::kad::QueryResult::PutRecord(Ok(_)) => {
                log::debug!("[kad] PUT record succeeded query={:?}", id);
            }
            libp2p::kad::QueryResult::PutRecord(Err(e)) => {
                log::warn!("[kad] PUT record failed query={:?} error={:?}", id, e);
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
                            addresses: vec![addr],
                        })
                        .await;
                }
            }
        }

        // TODO: add a PeerExpired or similar, connection is not lost
        #[cfg(not(target_os = "ios"))]
        SwarmEvent::Behaviour(KursalBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, addr) in &peers {
                log::debug!("[mDNS] peer expired {} at {}", peer_id, addr);
            }
        }

        SwarmEvent::Behaviour(KursalBehaviourEvent::Dcutr(e)) => match e.result {
            Ok(connection_id) => {
                log::info!(
                    "[dcutr] hole punch succeeded peer={} conn={connection_id:?}",
                    e.remote_peer_id
                );
                peer_conns.insert(connection_id, (e.remote_peer_id, ConnectionKind::HolePunch));

                let relayed: Vec<ConnectionId> = peer_conns
                    .iter()
                    .filter_map(|(cid, (p, kind))| {
                        (*p == e.remote_peer_id && *kind == ConnectionKind::Relay).then_some(*cid)
                    })
                    .collect();
                for cid in relayed {
                    log::info!(
                        "[dcutr] migrating to direct: closing relayed conn {cid:?} to {}",
                        e.remote_peer_id
                    );
                    swarm.close_connection(cid);
                }

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

        SwarmEvent::Behaviour(KursalBehaviourEvent::Ping(e)) => {
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
            libp2p::relay::Event::ReservationReqAccepted { src_peer_id, .. },
        )) => {
            log::info!("[relay] reservation accepted from {src_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            },
        )) => {
            log::info!("[relay] circuit established: {src_peer_id} -> {dst_peer_id}");
        }
        SwarmEvent::Behaviour(KursalBehaviourEvent::RelayServer(
            libp2p::relay::Event::ReservationTimedOut { src_peer_id },
        )) => {
            log::info!("[relay] reservation timed out for {src_peer_id}");
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
            log::info!(
                "[conn] established peer={peer_id} kind={kind:?} conn={connection_id:?} relayed={is_relayed_check} addr={}",
                endpoint.get_remote_address()
            );

            if kind == ConnectionKind::Direct {
                let is_bootstrap = is_bootstrap_peer(&peer_id);

                if is_bootstrap {
                    let circuit_addr = endpoint
                        .get_remote_address()
                        .clone()
                        .with(Protocol::P2pCircuit);
                    let _ = swarm.listen_on(circuit_addr);

                    log::info!("[kad] Bootstrapping Kademlia with relay");
                    let _ = swarm.behaviour_mut().kad.bootstrap();
                }
            }

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
        SwarmEvent::ListenerClosed { addresses, .. } => {
            for address in &addresses {
                listen_addresses.remove(address);
            }
            log::info!("[swarm] listener closed ({} addrs)", addresses.len());
        }

        SwarmEvent::Dialing {
            peer_id: Some(peer_id),
            ..
        } => {
            if best_kind(peer_conns, &peer_id).is_none() {
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
            let peer = peer_id.map_or_else(|| "unknown".to_string(), |id| id.to_string());
            let summary = dial_error_summary(&error);

            if let Some(tx) = requested {
                let _ = tx.send(Err(error.to_string()));
                log::info!("[swarm] dial failed peer={peer} error={summary}");
            } else {
                log::debug!("[swarm] dial failed peer={peer} error={summary}");
            }

            if let Some(peer_id) = peer_id
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
