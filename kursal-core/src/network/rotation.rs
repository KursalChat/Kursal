use crate::{
    KursalError, Result,
    api::{AppEvent, CoreCommand, send_message},
    contacts::Contact,
    first_contact::nearby::{bluetooth::BTTransport, mdns::MdnsTransport},
    identity::TransportIdentity,
    messaging::enums::{AddressAnnounce, KursalMessage},
    network::{
        NetworkManager,
        swarm::{SwarmCommand, SwarmHandle, get_listen_addrs, is_peer_connected},
    },
    storage::{SharedDatabase, TABLE_SETTINGS, get_local_user_id, get_peer_rotation_interval},
};
use libp2p::PeerId;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

impl NetworkManager {
    pub async fn start_rotation(
        &mut self,
        identity: TransportIdentity,
        db: &SharedDatabase,
    ) -> Result<mpsc::Sender<SwarmCommand>> {
        // shutdown any secondary
        if let Some(ref secondary) = self.secondary {
            let _ = secondary.cmd_tx.send(SwarmCommand::Shutdown).await;
        }

        identity.save_next(db)?;

        let secondary = SwarmHandle::spawn(
            identity,
            self.event_tx.clone(),
            self.chunk_tx.clone(),
            self.primary.relay_config.clone(),
            self.primary.mdns_enabled,
            0,
        )
        .await?;

        let cmd_tx = secondary.cmd_tx.clone();
        self.secondary = Some(secondary);

        let contacts = Contact::load_all(db)?;
        let seed_tx = cmd_tx.clone();
        tokio::spawn(async move {
            for contact in contacts {
                let _ = seed_tx.send(SwarmCommand::ContactAdded { contact }).await;
            }
        });

        Ok(cmd_tx)
    }

    pub async fn announce_address_rotation(
        &self,
        db: SharedDatabase,
        cmd_tx: &mpsc::Sender<SwarmCommand>,
        app_event_tx: &mpsc::Sender<AppEvent>,
    ) -> Result<()> {
        let contacts = Contact::load_all(&db)?;

        let secondary = self.secondary.as_ref().ok_or(KursalError::Identity(
            "Could not access secondary identity".to_string(),
        ))?;

        let identity = TransportIdentity::load_next(&db)?
            .filter(|identity| identity.peer_id == secondary.peer_id)
            .ok_or_else(|| KursalError::Identity("Rotation identity missing".to_string()))?;
        let peer_proof = identity.peer_binding(&get_local_user_id(&db)?)?;
        let new_peer_id = secondary.peer_id.to_base58();
        let new_addresses = get_listen_addrs(&secondary.cmd_tx).await?;

        for contact in contacts {
            if contact.blocked {
                continue;
            }
            let content = KursalMessage::AddressAnnounce(AddressAnnounce {
                peer_id: new_peer_id.clone(),
                addresses: new_addresses.clone(),
                peer_proof: peer_proof.clone(),
            });
            let db2 = db.clone();
            let cmd2 = cmd_tx.clone();
            let ev2 = app_event_tx.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) = send_message(content, &contact, db2, &cmd2, Some(&ev2)).await {
                    log::warn!(
                        "[rotation] address announce to {} failed: {err}",
                        contact.peer_id
                    );
                }
            });
        }

        Ok(())
    }

    pub async fn complete_rotation(&mut self, db: &SharedDatabase) -> Result<()> {
        let new_primary = self
            .secondary
            .take()
            .ok_or_else(|| KursalError::Network("No secondary swarm".to_string()))?;
        let keypair = match TransportIdentity::promote_next(db, &new_primary.peer_id) {
            Ok(next) => next.keypair,
            Err(err) => {
                let _ = new_primary.cmd_tx.send(SwarmCommand::Shutdown).await;
                return Err(err);
            }
        };

        let _ = self.primary.cmd_tx.send(SwarmCommand::Shutdown).await;
        self.primary = new_primary;

        // update transports to use new swarm's command channel
        self.mdns_transport = Arc::new(MdnsTransport::new(
            self.primary.cmd_tx.clone(),
            self.my_beacon.clone(),
        ));
        self.bt_transport = Arc::new(BTTransport::new(
            self.primary.cmd_tx.clone(),
            self.my_beacon.clone(),
            self.bt_event_tx.clone(),
            keypair,
        ));

        Ok(())
    }

    pub async fn spawn_rotation_scheduler(
        db: SharedDatabase,
        core_cmd_tx: mpsc::Sender<CoreCommand>,
    ) {
        loop {
            let secs = get_peer_rotation_interval(&db);

            if secs == 0u64 {
                break;
            }

            tokio::time::sleep(Duration::from_secs(secs.max(300))).await;

            let (reply_tx, reply_rx) = oneshot::channel();
            if core_cmd_tx
                .send(CoreCommand::RotatePeerId { reply: reply_tx })
                .await
                .is_err()
            {
                break;
            }

            if let Err(ohno) = reply_rx.await {
                log::error!("Failed to rotate peer ID: {}", ohno);
            }
        }
    }

    pub async fn spawn_address_announcer(
        db: SharedDatabase,
        core_cmd_tx: mpsc::Sender<CoreCommand>,
    ) {
        loop {
            let secs = {
                let lock = &*db;
                lock.raw_read(TABLE_SETTINGS, "address_announce_interval_secs")
                    .ok()
                    .flatten()
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(7 * 24 * 60 * 60)
            };

            tokio::time::sleep(Duration::from_secs(secs)).await;

            if core_cmd_tx
                .send(CoreCommand::AnnounceAddresses)
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

pub async fn announce_addresses_to_offline(
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let contacts = Contact::load_all(&db)?;

    let identity = TransportIdentity::load(&db)?
        .ok_or_else(|| KursalError::Identity("No transport identity".to_string()))?;
    let peer_proof = identity.peer_binding(&get_local_user_id(&db)?)?;
    let my_peer_id = identity.peer_id.to_base58();
    let my_addresses = get_listen_addrs(cmd_tx).await?;

    for contact in contacts {
        if contact.blocked {
            continue;
        }
        let Ok(peer_id) = PeerId::from_str(&contact.peer_id) else {
            continue;
        };
        if is_peer_connected(cmd_tx, peer_id).await {
            continue;
        }

        let content = KursalMessage::AddressAnnounce(AddressAnnounce {
            peer_id: my_peer_id.clone(),
            addresses: my_addresses.clone(),
            peer_proof: peer_proof.clone(),
        });
        if let Err(err) =
            send_message(content, &contact, db.clone(), cmd_tx, Some(app_event_tx)).await
        {
            log::warn!("[announce] heartbeat to {} failed: {err}", contact.peer_id);
        }
    }

    Ok(())
}
