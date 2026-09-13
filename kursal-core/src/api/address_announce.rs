use crate::{
    Result,
    api::AppEvent,
    identity::{UserId, verify_peer_binding},
    messaging::offline::update_contact,
    network::swarm::{SwarmCommand, is_routable_multiaddr},
    storage::SharedDatabase,
};
use libp2p::Multiaddr;
use tokio::sync::mpsc::Sender;

pub async fn apply_address_announce(
    user_id: &UserId,
    new_peer_id: String,
    addresses: Vec<String>,
    peer_proof: &[u8],
    db: &SharedDatabase,
    cmd_tx: &Sender<SwarmCommand>,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    if !verify_peer_binding(user_id, &new_peer_id, peer_proof) {
        log::warn!(
            "[announce] ignoring unproven peer id from {}",
            hex::encode(user_id.0)
        );
        return Ok(());
    }

    let new_addresses = addresses.clone();
    let mut previous_peer_id = None;
    if let Some(updated) = update_contact(db, user_id, |c| {
        let mut changed = false;

        if c.peer_id != new_peer_id {
            previous_peer_id = Some(std::mem::replace(&mut c.peer_id, new_peer_id));
            changed = true;
        }

        if c.known_addresses != new_addresses {
            c.known_addresses = new_addresses;
            changed = true;
        }

        changed
    })
    .await?
    {
        if let Some(peer_id) = previous_peer_id {
            let _ = cmd_tx.send(SwarmCommand::ContactRemoved { peer_id }).await;
        }
        let _ = cmd_tx
            .send(SwarmCommand::ContactAdded {
                contact: updated.clone(),
            })
            .await;
        if let Some(tx) = event_tx {
            tx.send(AppEvent::ContactUpdated { contact: updated })
                .await
                .ok();
        }
    }

    for addr_str in &addresses {
        let Ok(addr) = addr_str.parse::<Multiaddr>() else {
            continue;
        };
        if !is_routable_multiaddr(&addr) {
            continue;
        }
        let _ = cmd_tx.send(SwarmCommand::Dial(addr)).await;
    }

    Ok(())
}
