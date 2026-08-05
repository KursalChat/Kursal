use crate::{
    api::{
        AppEvent, ConnectionStatus,
        file_transfers::{STALE_TRANSFER_MAX_AGE_SECS, cleanup_stale_transfers},
        poll_contact_offline,
    },
    contacts::Contact,
    first_contact::ltc::LtcState,
    identity::UserId,
    messaging::offline::{
        DIRECT_ACK_DEADLINE_SECS, deliver_queue_direct, expire_and_fail, list_pending_ack,
        maybe_flush, move_to_mailbox_if_stuck, republish_pending,
    },
    network::{
        NetworkManager,
        kademlia::KAD_LONG_MAX_AGE,
        swarm::{ConnectionKind, SwarmCommand},
    },
    storage::{SharedDatabase, get_timestamp_secs},
};
use libp2p::PeerId;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, mpsc};

const OFFLINE_PERIODIC_POLL_SECS: u64 = 15 * 60;
const OFFLINE_REPUBLISH_SECS: u64 = 3 * 60 * 60;
const OFFLINE_POLL_STAGGER_MS: u64 = 1000;
const PRESENCE_DIAL_INTERVAL_SECS: u64 = 3 * 60;
const PRESENCE_DIAL_STAGGER_MS: u64 = 250;
const RELAY_RESERVE_INTERVAL_SECS: u64 = 30;
const PRESENCE_SYNC_INTERVAL_SECS: u64 = 10;
const LTC_POINTER_STARTUP_DELAY_SECS: u64 = 10;
const LTC_POINTER_REPUBLISH_SECS: u64 = 24 * 60 * 60;

pub(super) async fn presence_sync_loop(
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    event_tx: mpsc::Sender<AppEvent>,
    status_map: Arc<Mutex<HashMap<UserId, ConnectionStatus>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(PRESENCE_SYNC_INTERVAL_SECS));
    interval.tick().await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut last_state: Option<(bool, usize)> = None;

    loop {
        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
        let connected_peers: HashSet<PeerId> = crate::network::swarm::get_connected_peers(&cmd_tx)
            .await
            .into_iter()
            .collect();
        let peer_kinds = crate::network::swarm::get_peer_connection_kinds(&cmd_tx).await;
        let peer_count = connected_peers.len();
        let online = peer_count > 0;
        if last_state != Some((online, peer_count)) {
            last_state = Some((online, peer_count));
            event_tx
                .send(AppEvent::NetworkOnline { online, peer_count })
                .await
                .ok();
        }

        let contacts = match Contact::load_all(&*db.0.lock().await) {
            Ok(c) => c,
            Err(err) => {
                log::warn!("[presence-sync] load_all failed: {err}");
                interval.tick().await;
                continue;
            }
        };

        for contact in contacts {
            let Ok(peer_id) = PeerId::from_str(&contact.peer_id) else {
                continue;
            };
            let mut map = status_map.lock().await;
            let prev = map.get(&contact.user_id).cloned();

            let actual = peer_kinds.get(&peer_id).map(|kind| match kind {
                ConnectionKind::Relay => ConnectionStatus::Relay,
                ConnectionKind::Direct => ConnectionStatus::Direct,
                ConnectionKind::HolePunch => ConnectionStatus::HolePunch,
            });

            let next = match actual {
                Some(status) => (prev.as_ref() != Some(&status)).then_some(status),
                None if prev == Some(ConnectionStatus::Connecting) => None,
                None if prev == Some(ConnectionStatus::Disconnected) => None,
                None => Some(ConnectionStatus::Disconnected),
            };

            if let Some(status) = next {
                map.insert(contact.user_id.clone(), status.clone());
                drop(map);
                event_tx
                    .send(AppEvent::ConnectionChange {
                        contact_id: contact.user_id.clone(),
                        status,
                    })
                    .await
                    .ok();
            }
        }

        interval.tick().await;
    }
}

pub(super) async fn ltc_pointer_loop(
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    event_tx: mpsc::Sender<AppEvent>,
) {
    tokio::time::sleep(Duration::from_secs(LTC_POINTER_STARTUP_DELAY_SECS)).await;

    let mut interval = tokio::time::interval(Duration::from_secs(LTC_POINTER_REPUBLISH_SECS));
    interval.tick().await;

    loop {
        let swarm = network.lock().await.primary.clone();

        if let Err(err) = LtcState::publish_pointer(db.clone(), swarm, event_tx.clone()).await {
            log::warn!("[ltc] periodic rendezvous publish failed: {err}");
        }

        interval.tick().await;
    }
}

pub(super) async fn relay_reserve_loop(network: Arc<Mutex<NetworkManager>>) {
    tokio::time::sleep(Duration::from_secs(5)).await;
    let mut interval = tokio::time::interval(Duration::from_secs(RELAY_RESERVE_INTERVAL_SECS));
    loop {
        interval.tick().await;
        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
        let _ = cmd_tx.send(SwarmCommand::EnsureRelayReservations).await;
    }
}

pub(super) async fn presence_dial_loop(db: SharedDatabase, network: Arc<Mutex<NetworkManager>>) {
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut interval = tokio::time::interval(Duration::from_secs(PRESENCE_DIAL_INTERVAL_SECS));
    interval.tick().await;

    loop {
        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
        let connected_peers: HashSet<PeerId> = crate::network::swarm::get_connected_peers(&cmd_tx)
            .await
            .into_iter()
            .collect();
        let contacts = match Contact::load_all(&*db.0.lock().await) {
            Ok(c) => c,
            Err(err) => {
                log::warn!("[presence] load_all failed: {err}");
                interval.tick().await;
                continue;
            }
        };

        for contact in contacts {
            let Ok(peer_id) = PeerId::from_str(&contact.peer_id) else {
                continue;
            };
            if connected_peers.contains(&peer_id) {
                continue;
            }
            for addr_str in &contact.known_addresses {
                if let Ok(addr) = addr_str.parse::<libp2p::Multiaddr>() {
                    let _ = cmd_tx.send(SwarmCommand::Dial(addr)).await;
                }
            }
            tokio::time::sleep(Duration::from_millis(PRESENCE_DIAL_STAGGER_MS)).await;
        }

        interval.tick().await;
    }
}

pub(super) async fn periodic_offline_poll(
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    event_tx: mpsc::Sender<AppEvent>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(OFFLINE_PERIODIC_POLL_SECS));
    interval.tick().await;
    tokio::time::sleep(Duration::from_secs(15)).await;

    let mut last_republish: Option<tokio::time::Instant> = None;

    loop {
        let cmd_tx = network.lock().await.primary.cmd_tx.clone();
        if let Err(err) = cleanup_stale_transfers(db.clone(), STALE_TRANSFER_MAX_AGE_SECS).await {
            log::warn!("[file] stale transfer cleanup failed: {err}");
        }

        if crate::network::swarm::get_connected_peer_count(&cmd_tx).await == 0 {
            interval.tick().await;
            continue;
        }

        let contacts = match Contact::load_all(&*db.0.lock().await) {
            Ok(c) => c,
            Err(err) => {
                log::warn!("[offline] periodic poll: load_all failed: {err}");
                interval.tick().await;
                continue;
            }
        };

        let do_republish = last_republish
            .map(|t| t.elapsed().as_secs() >= OFFLINE_REPUBLISH_SECS)
            .unwrap_or(true);
        if do_republish {
            last_republish = Some(tokio::time::Instant::now());
        }

        if contacts.is_empty() {
            interval.tick().await;
            continue;
        }

        let _ = event_tx.send(AppEvent::OfflineSync { active: true }).await;

        for contact in contacts {
            if let Err(err) =
                deliver_queue_direct(&contact.user_id, &cmd_tx, db.clone(), Some(&event_tx)).await
            {
                log::warn!("[offline] periodic direct drain failed: {err}");
            }

            if let Err(err) = expire_and_fail(&contact.user_id, &db, Some(&event_tx)).await {
                log::warn!("[offline] expire_and_fail failed: {err}");
            }

            if let Err(err) =
                maybe_flush(&contact.user_id, &cmd_tx, db.clone(), Some(&event_tx)).await
            {
                log::warn!("[offline] periodic flush failed: {err}");
            }

            if do_republish
                && let Err(err) = republish_pending(&contact.user_id, &cmd_tx, db.clone()).await
            {
                log::warn!("[offline] periodic republish failed: {err}");
            }

            if let Err(err) = poll_contact_offline(
                contact.user_id.clone(),
                cmd_tx.clone(),
                db.clone(),
                event_tx.clone(),
            )
            .await
            {
                log::warn!("[offline] periodic poll failed: {err}");
            }

            tokio::time::sleep(Duration::from_millis(OFFLINE_POLL_STAGGER_MS)).await;
        }

        drive_pending_ack_backstop(&db, &cmd_tx, &event_tx).await;

        let _ = event_tx.send(AppEvent::OfflineSync { active: false }).await;

        interval.tick().await;
    }
}

async fn drive_pending_ack_backstop(
    db: &SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) {
    let now = get_timestamp_secs().unwrap_or(0);
    let Ok(pending) = list_pending_ack(db).await else {
        return;
    };
    for (user, id, sent_at) in pending {
        let age = now.saturating_sub(sent_at);
        if age < DIRECT_ACK_DEADLINE_SECS {
            continue;
        }
        if age >= KAD_LONG_MAX_AGE {
            let failed = {
                let guard = db.0.lock().await;
                crate::messaging::StoredMessage::set_failed(&guard, &user, &id).unwrap_or(false)
            };
            let _ = crate::messaging::offline::clear_pending_ack(db, &user, &id).await;
            if failed {
                let _ = event_tx
                    .send(AppEvent::MessageFailed {
                        contact_id: user.clone(),
                        message_ids: vec![id],
                    })
                    .await;
            }
        } else if let Err(err) =
            move_to_mailbox_if_stuck(&user, &id, cmd_tx, db.clone(), Some(event_tx)).await
        {
            log::warn!("[offline] pending-ack backstop failed: {err}");
        }
    }
}
