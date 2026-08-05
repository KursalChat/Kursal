use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::{
        AppEvent, ConnectionStatus, file_transfers::resume_incoming_transfers, handle_incoming,
        poll_contact_offline,
    },
    contacts::Contact,
    crypto::messages::message_send,
    first_contact::nearby::{
        BtEvent, NearbyMessage, NearbyOrigin, NearbyPacket, NearbyRouteResult,
        handle_nearby_request,
    },
    identity::UserId,
    messaging::{
        enums::KursalMessage,
        offline::{deliver_queue_direct, maybe_flush, queue_for_offline},
    },
    network::{
        NetworkManager,
        swarm::{ConnectionKind, NetworkEvent, SwarmCommand},
    },
    storage::SharedDatabase,
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

pub(super) async fn handle_bt_event(
    event: BtEvent,
    db: &SharedDatabase,
    network: &Arc<Mutex<NetworkManager>>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) {
    match event {
        BtEvent::Beacon { peer_id, beacon } => {
            let net = network.lock().await;

            log::info!("[nearby] Discovered {peer_id} via bluetooth");
            net.nearby_peers
                .lock()
                .await
                .insert((peer_id, NearbyOrigin::Bluetooth), beacon);
        }
        BtEvent::Message { from_peer_id, msg } => {
            let bt_transport = network.lock().await.bt_transport.clone();

            let sender = bt_transport
                .pending_handshakes
                .lock()
                .await
                .get(&from_peer_id)
                .cloned();

            match sender {
                Some(tx) => {
                    let _ = tx.send(msg).await;
                }
                None => {
                    if let NearbyMessage::ConnectRequest { from_session_name } = msg {
                        let (decision_tx, decision_rx) = oneshot::channel::<bool>();
                        if app_event_tx
                            .send(AppEvent::NearbyRequest {
                                peer_id: from_peer_id.clone(),
                                session_name: from_session_name,
                                decision_tx,
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }

                        let db_clone = db.clone();
                        let event_tx_clone = app_event_tx.clone();
                        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                        let bt_arc = bt_transport.clone();

                        tokio::task::spawn_local(async move {
                            if let Err(e) = handle_nearby_request(
                                &from_peer_id,
                                &*bt_arc,
                                decision_rx,
                                db_clone,
                                &event_tx_clone,
                                &cmd_tx,
                            )
                            .await
                            {
                                log::warn!("[bt] handle_nearby_request: {e}");
                            }
                        });
                    }
                }
            }
        }
    }
}

pub(super) async fn handle_internal_network_event(
    event: NetworkEvent,
    db: &SharedDatabase,
    app_data_dir: &std::path::Path,
    network: &Arc<Mutex<NetworkManager>>,
    app_event_tx: &mpsc::Sender<AppEvent>,
    status_map: &Arc<Mutex<HashMap<UserId, ConnectionStatus>>>,
) {
    match event {
        NetworkEvent::PeerDiscovered { peer_id, addresses } => {
            let network = network.lock().await;

            network
                .mdns_transport
                .nearby_addresses
                .lock()
                .await
                .insert(peer_id.to_base58(), addresses.clone());

            let result: Result<()> = async {
                let beacon = network
                    .my_beacon
                    .lock()
                    .await
                    .clone()
                    .ok_or(KursalError::Storage("No beacon saved".to_string()))?;

                let serialized = NearbyPacket::Beacon(beacon).serialize()?;

                network
                    .primary
                    .cmd_tx
                    .send(SwarmCommand::SendMessage {
                        peer_id: PeerId::from_str(&peer_id.to_base58())
                            .ok_kursal(KursalError::Crypto)?,
                        data: serialized,
                        addresses,
                    })
                    .await
                    .ok_kursal(KursalError::Network)?;

                Ok(())
            }
            .await;

            if let Err(err) = result {
                log::warn!("failed to send beacon to {peer_id}: {err}");
            }
        }

        NetworkEvent::MessageReceived { from, data } => {
            let route = async {
                let packet = match NearbyPacket::deserialize(&data) {
                    Ok(p) => p,
                    Err(_) => return NearbyRouteResult::NotNearby,
                };

                match packet {
                    NearbyPacket::Beacon(mut b) => {
                        // imagine a guy just putting another peer id 💀
                        b.peer_id = from.to_base58();

                        let network = network.lock().await;

                        log::info!("[nearby] Discovered {} via mDNS", b.peer_id);
                        network
                            .nearby_peers
                            .lock()
                            .await
                            .insert((b.peer_id.clone(), NearbyOrigin::mDNS), b);

                        if let Some(my_beacon) = network.my_beacon.lock().await.clone()
                            && let Ok(data) = NearbyPacket::BeaconAck(my_beacon).serialize()
                        {
                            let addresses = network
                                .mdns_transport
                                .nearby_addresses
                                .lock()
                                .await
                                .get(&from.to_base58())
                                .cloned()
                                .unwrap_or_default();

                            let _ = network
                                .primary
                                .cmd_tx
                                .send(SwarmCommand::SendMessage {
                                    peer_id: from,
                                    data,
                                    addresses,
                                })
                                .await;
                        }

                        NearbyRouteResult::HandledInternally
                    }
                    NearbyPacket::BeaconAck(mut b) => {
                        b.peer_id = from.to_base58();

                        let network = network.lock().await;

                        log::info!("[nearby] Ack from {} via mDNS", b.peer_id);
                        network
                            .nearby_peers
                            .lock()
                            .await
                            .insert((b.peer_id.clone(), NearbyOrigin::mDNS), b);

                        NearbyRouteResult::HandledInternally
                    }
                    NearbyPacket::Message(msg) => {
                        let sender = {
                            let network = network.lock().await;
                            network
                                .mdns_transport
                                .pending_handshakes
                                .lock()
                                .await
                                .get(&from.to_base58())
                                .cloned()
                        };
                        match sender {
                            Some(tx) => {
                                let _ = tx.send(msg).await;
                                NearbyRouteResult::HandledInternally
                            }
                            None => match msg {
                                NearbyMessage::ConnectRequest { from_session_name } => {
                                    NearbyRouteResult::IncomingRequest {
                                        peer_id: from.to_base58(),
                                        session_name: from_session_name,
                                    }
                                }
                                _ => {
                                    // invalid
                                    NearbyRouteResult::HandledInternally
                                }
                            },
                        }
                    }
                }
            }
            .await;

            match route {
                NearbyRouteResult::NotNearby => {
                    let (cmd_tx, chunk_tx) = {
                        let net = network.lock().await;
                        (net.primary.cmd_tx.clone(), net.chunk_tx.clone())
                    };
                    let db_clone = db.clone();
                    let app_event_tx_clone = app_event_tx.clone();
                    let app_data_dir_clone = app_data_dir.to_path_buf();

                    tokio::task::spawn_local(async move {
                        if let Err(e) = handle_incoming(
                            from,
                            data,
                            db_clone,
                            &app_data_dir_clone,
                            &cmd_tx,
                            &app_event_tx_clone,
                            &chunk_tx,
                        )
                        .await
                        {
                            log::warn!("handle_incoming error: {e}");
                            let _ = app_event_tx_clone
                                .send(AppEvent::BackendSignal {
                                    signal: "handle_incoming_error".to_string(),
                                    payload: e.to_string(),
                                })
                                .await;
                        }
                    });
                }
                NearbyRouteResult::HandledInternally => { /* ignore */ }
                NearbyRouteResult::IncomingRequest {
                    peer_id,
                    session_name,
                } => {
                    let (decision_tx, decision_rx) = oneshot::channel::<bool>();

                    if app_event_tx
                        .send(AppEvent::NearbyRequest {
                            peer_id: peer_id.clone(),
                            session_name,
                            decision_tx,
                        })
                        .await
                        .is_err()
                    {
                        // receiver dropped, skip handling
                        return;
                    }

                    let db_clone = db.clone();
                    let event_tx_clone = app_event_tx.clone();
                    let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                    let mdns_transport = network.lock().await.mdns_transport.clone();
                    tokio::task::spawn_local(async move {
                        if let Err(e) = handle_nearby_request(
                            &peer_id,
                            &*mdns_transport,
                            decision_rx,
                            db_clone,
                            &event_tx_clone,
                            &cmd_tx,
                        )
                        .await
                        {
                            log::warn!("handle_nearby_request error: {e}");
                        }
                    });
                }
            }
        }

        NetworkEvent::ConnectionEstablished { peer_id, via } => {
            log::info!("[network] connection established with {peer_id} via {via:?}");
            let peer_id_str = peer_id.to_base58();

            let contact_id = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str)
                .ok()
                .flatten()
                .map(|c| c.user_id);

            if let Some(contact_id) = contact_id {
                let status = match via {
                    ConnectionKind::Relay => ConnectionStatus::Relay,
                    ConnectionKind::Direct => ConnectionStatus::Direct,
                    ConnectionKind::HolePunch => ConnectionStatus::HolePunch,
                };

                status_map
                    .lock()
                    .await
                    .insert(contact_id.clone(), status.clone());

                app_event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact_id.clone(),
                        status,
                    })
                    .await
                    .ok();

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                let db_clone = db.clone();
                let event_tx_clone = app_event_tx.clone();
                let contact_id_for_flush = contact_id.clone();
                let cmd_tx_for_flush = cmd_tx.clone();
                let db_for_flush = db_clone.clone();
                let event_tx_for_flush = app_event_tx.clone();
                let contact_id_for_resume = contact_id.clone();
                let cmd_tx_for_resume = cmd_tx.clone();
                let db_for_resume = db_clone.clone();
                let event_tx_for_resume = app_event_tx.clone();
                tokio::task::spawn_local(async move {
                    match deliver_queue_direct(
                        &contact_id_for_flush,
                        &cmd_tx_for_flush,
                        db_for_flush.clone(),
                        Some(&event_tx_for_flush),
                    )
                    .await
                    {
                        Ok(0) => {}
                        Ok(n) => log::info!("[offline] connection-drain delivered {n} directly"),
                        Err(err) => log::warn!("[offline] connection-drain failed: {err}"),
                    }

                    if let Err(err) = maybe_flush(
                        &contact_id_for_flush,
                        &cmd_tx_for_flush,
                        db_for_flush.clone(),
                        Some(&event_tx_for_flush),
                    )
                    .await
                    {
                        log::warn!("[offline] connection-flush failed: {err}");
                    }
                });
                tokio::task::spawn_local(async move {
                    let loaded =
                        Contact::load(&*db_for_resume.0.lock().await, &contact_id_for_resume);
                    if let Ok(Some(contact)) = loaded
                        && let Err(err) = resume_incoming_transfers(
                            contact,
                            db_for_resume.clone(),
                            cmd_tx_for_resume,
                            event_tx_for_resume,
                        )
                        .await
                    {
                        log::warn!("[file] connection-resume failed: {err}");
                    }
                });
                tokio::task::spawn_local(async move {
                    if let Err(err) =
                        poll_contact_offline(contact_id, cmd_tx, db_clone, event_tx_clone).await
                    {
                        log::warn!("[offline] connection-poll failed: {err}");
                    }
                });
            }
        }
        NetworkEvent::ConnectionKindChanged { peer_id, via } => {
            let peer_id_str = peer_id.to_base58();
            let found = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str);
            if let Ok(Some(contact)) = found {
                let status = match via {
                    ConnectionKind::Relay => ConnectionStatus::Relay,
                    ConnectionKind::Direct => ConnectionStatus::Direct,
                    ConnectionKind::HolePunch => ConnectionStatus::HolePunch,
                };
                status_map
                    .lock()
                    .await
                    .insert(contact.user_id.clone(), status.clone());
                app_event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact.user_id,
                        status,
                    })
                    .await
                    .ok();
            }
        }
        NetworkEvent::ConnectionPending { peer_id } => {
            let peer_id_str = peer_id.to_base58();
            let found = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str);
            if let Ok(Some(contact)) = found {
                let mut map = status_map.lock().await;
                let settled = matches!(
                    map.get(&contact.user_id),
                    Some(ConnectionStatus::Direct)
                        | Some(ConnectionStatus::Relay)
                        | Some(ConnectionStatus::HolePunch)
                        | Some(ConnectionStatus::Connecting)
                );
                if !settled {
                    map.insert(contact.user_id.clone(), ConnectionStatus::Connecting);
                    drop(map);

                    app_event_tx
                        .send(AppEvent::ConnectionChange {
                            contact_id: contact.user_id,
                            status: ConnectionStatus::Connecting,
                        })
                        .await
                        .ok();
                }
            }
        }
        NetworkEvent::ConnectionFailed { peer_id } => {
            let peer_id_str = peer_id.to_base58();
            let found = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str);
            if let Ok(Some(contact)) = found {
                let mut map = status_map.lock().await;
                if map.get(&contact.user_id) != Some(&ConnectionStatus::Disconnected) {
                    map.insert(contact.user_id.clone(), ConnectionStatus::Disconnected);
                    drop(map);

                    app_event_tx
                        .send(AppEvent::ConnectionChange {
                            contact_id: contact.user_id,
                            status: ConnectionStatus::Disconnected,
                        })
                        .await
                        .ok();
                }
            }
        }
        NetworkEvent::ConnectionLost { peer_id } => {
            let peer_id_str = peer_id.to_base58();

            let found = Contact::find_by_peer_id(&*db.0.lock().await, &peer_id_str);
            if let Ok(Some(contact)) = found {
                status_map
                    .lock()
                    .await
                    .insert(contact.user_id.clone(), ConnectionStatus::Disconnected);

                app_event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact.user_id.clone(),
                        status: ConnectionStatus::Disconnected,
                    })
                    .await
                    .ok();
            }
        }

        NetworkEvent::DhtFetchResult { .. } => { /* nothing happens here */ }

        NetworkEvent::SendFailed { peer_id } => {
            let peer_id_str = peer_id.to_base58();
            let db_lock = db.0.lock().await;

            let Ok(contacts) = Contact::load_all(&db_lock) else {
                return;
            };
            let Some(contact) = contacts.into_iter().find(|c| c.peer_id == peer_id_str) else {
                return;
            };
            drop(db_lock);

            let cmd_tx = network.lock().await.primary.cmd_tx.clone();

            status_map
                .lock()
                .await
                .insert(contact.user_id.clone(), ConnectionStatus::Disconnected);

            app_event_tx
                .send(AppEvent::ConnectionChange {
                    contact_id: contact.user_id.clone(),
                    status: ConnectionStatus::Disconnected,
                })
                .await
                .ok();

            let contact_id = contact.user_id.clone();

            {
                let cmd_tx = cmd_tx.clone();
                let db_clone = db.clone();
                let event_tx = app_event_tx.clone();
                let cid = contact_id.clone();
                tokio::task::spawn_local(async move {
                    if let Err(err) = poll_contact_offline(cid, cmd_tx, db_clone, event_tx).await {
                        log::warn!("[offline] post-fail poll failed: {err}");
                    }
                });
            }

            let db_clone = db.clone();
            let event_tx_for_ping = app_event_tx.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) =
                    queue_offline_ping(contact_id, db_clone, &cmd_tx, Some(&event_tx_for_ping))
                        .await
                {
                    log::warn!("[offline] post-fail ping failed: {err}");
                }
            });
        }
    }
}

async fn queue_offline_ping(
    contact_id: UserId,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: Option<&mpsc::Sender<AppEvent>>,
) -> Result<()> {
    let contact = Contact::load(&*db.0.lock().await, &contact_id)?
        .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());
    let serialized = KursalMessage::Typing.serialize()?;
    let ciphertext = message_send(db.clone(), &address, &serialized).await?;

    queue_for_offline(
        &contact.user_id,
        None,
        ciphertext,
        cmd_tx,
        db.clone(),
        event_tx,
    )
    .await
}
