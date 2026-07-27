use crate::{
    Result,
    api::AppEvent,
    identity::UserId,
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
    db: &SharedDatabase,
    cmd_tx: &Sender<SwarmCommand>,
    event_tx: Option<&Sender<AppEvent>>,
) -> Result<()> {
    let new_addresses = addresses.clone();
    if let Some(updated) = update_contact(db, user_id, move |c| {
        let mut changed = false;

        if c.peer_id != new_peer_id {
            c.peer_id = new_peer_id;
            changed = true;
        }

        if c.known_addresses != new_addresses {
            c.known_addresses = new_addresses;
            changed = true;
        }

        changed
    })
    .await?
        && let Some(tx) = event_tx
    {
        tx.send(AppEvent::ContactUpdated { contact: updated })
            .await
            .ok();
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
