use crate::{
    api::{AppEvent, send_message_tracked},
    contacts::Contact,
    identity::UserId,
    messaging::enums::{KursalMessage, ProfileInfo},
    network::swarm::{SwarmCommand, is_peer_connected},
    storage::{SharedDatabase, conversation, get_local_avatar_bytes, get_local_profile},
};
use libp2p::PeerId;
use std::str::FromStr;
use tokio::sync::mpsc;

pub async fn share_profile_with(
    contact: &Contact,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: Option<&mpsc::Sender<AppEvent>>,
) -> bool {
    let contact_id = hex::encode(contact.user_id.0);

    let connected = match PeerId::from_str(&contact.peer_id) {
        Ok(peer_id) => is_peer_connected(cmd_tx, peer_id).await,
        Err(_) => false,
    };

    let delivered = if connected {
        let msg = KursalMessage::ProfileUpdate(ProfileInfo {
            display_name,
            avatar_bytes,
        });

        match send_message_tracked(msg, contact, db.clone(), cmd_tx, app_event_tx).await {
            Ok((_, queued_offline)) => !queued_offline,
            Err(err) => {
                log::warn!("[profile] share to {contact_id} failed: {err}");
                false
            }
        }
    } else {
        log::info!("[profile] {contact_id} offline, profile marked stale");
        false
    };

    if let Err(err) = conversation::set_profile_stale(&db, &contact_id, !delivered) {
        log::warn!("[profile] stale flag write failed for {contact_id}: {err}");
    }

    delivered
}

pub async fn resend_stale_profile(
    user_id: &UserId,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    app_event_tx: Option<&mpsc::Sender<AppEvent>>,
) {
    let contact_id = hex::encode(user_id.0);
    if !conversation::get_profile_stale(&db, &contact_id) {
        return;
    }

    let Ok(Some(contact)) = Contact::load(&db, user_id) else {
        return;
    };

    if !contact.profile_shared {
        let _ = conversation::set_profile_stale(&db, &contact_id, false);
        return;
    }

    let (display_name, _) = get_local_profile(&db);
    let avatar_bytes = get_local_avatar_bytes(&db);

    if share_profile_with(
        &contact,
        display_name,
        avatar_bytes,
        db.clone(),
        cmd_tx,
        app_event_tx,
    )
    .await
    {
        log::info!("[profile] resent stale profile to {contact_id}");
    }
}
