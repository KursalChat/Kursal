use crate::{
    KursalError, MapKursalResult, Result,
    api::CoreCommand,
    contacts::Contact,
    dto::{ContactResponse, MessageResponse, NearbyPeerResponse, NetworkStatusDto, OtpResponse},
    first_contact::{
        nearby::{NearbyBeacon, generate_session_name},
        otp,
    },
    identity::{UserId, security_code},
    messaging::{StoredMessage, enums::MessageId, pin_index_list},
    network::NetworkManager,
    storage::{
        Database, get_dilithium_pub, get_local_identity_pub, get_local_profile, get_local_user_id,
        set_local_profile,
    },
};
use std::collections::HashMap;
use tokio::sync::{MutexGuard, mpsc, oneshot};

pub trait StateWrapper {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand>;
    fn network_lock(&self) -> impl std::future::Future<Output = MutexGuard<'_, NetworkManager>>;
    fn pending_nearby_lock(
        &self,
    ) -> impl std::future::Future<Output = MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>>>;
    fn db_lock(&self) -> impl std::future::Future<Output = MutexGuard<'_, Database>>;
}

macro_rules! core_request {
    ($fn_name:ident($($arg:ident: $ty:ty),*) => $variant:ident -> $ret:ty) => {
        pub async fn $fn_name<S: StateWrapper>(state: S $(, $arg: $ty)*) -> Result<$ret> {
            let (reply_tx, reply_rx) = oneshot::channel();

            state
                .core_cmd_tx()
                .send(CoreCommand::$variant {
                    $($arg,)*
                    reply: reply_tx,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            reply_rx
                .await
                .ok_kursal(KursalError::Network)?
        }
    };
    ($fn_name:ident($($arg:ident: $ty:ty),*) => $variant:ident -> $ret:ty, map $mapper:expr) => {
        pub async fn $fn_name<S: StateWrapper>(state: S $(, $arg: $ty)*) -> Result<$ret> {
            let (reply_tx, reply_rx) = oneshot::channel();

            state
                .core_cmd_tx()
                .send(CoreCommand::$variant {
                    $($arg,)*
                    reply: reply_tx,
                })
                .await
                .ok_kursal(KursalError::Network)?;

            let value = reply_rx
                .await
                .ok_kursal(KursalError::Network)??;

            Ok($mapper(value))
        }
    };
}

fn parse_contact_id(contact_id: &str) -> Result<UserId> {
    let bytes: [u8; 32] = hex::decode(contact_id)
        .ok_kursal(KursalError::Crypto)?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    Ok(UserId(bytes))
}

fn parse_message_id(message_id: &str) -> Result<MessageId> {
    let bytes: [u8; 16] = hex::decode(message_id)
        .ok_kursal(KursalError::Crypto)?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

    Ok(MessageId(bytes))
}

pub async fn generate_otp() -> Result<OtpResponse> {
    Ok(OtpResponse {
        otp: otp::generate_otp()?,
    })
}

core_request!(publish_otp(otp: String) => PublishOtp -> ());
core_request!(fetch_otp(otp: String) => FetchOtp -> ContactResponse, map ContactResponse::from);
core_request!(export_ltc() => ExportLtc -> Vec<u8>);
core_request!(import_ltc(bytes: Vec<u8>) => ImportLtc -> ContactResponse, map ContactResponse::from);

pub async fn start_nearby<S: StateWrapper>(state: S) -> Result<String> {
    let session_name = generate_session_name()?;
    let mut network = state.network_lock().await;

    let beacon = NearbyBeacon {
        peer_id: network.primary.peer_id.to_base58(),
        session_name: session_name.clone(),
    };

    network.start_mdns(beacon).await?;

    Ok(session_name)
}

pub async fn stop_nearby<S: StateWrapper>(state: S) -> Result<()> {
    let mut network = state.network_lock().await;

    network.stop_mdns().await?;

    Ok(())
}

pub async fn get_nearby_peers<S: StateWrapper>(state: S) -> Result<Vec<NearbyPeerResponse>> {
    let network = state.network_lock().await;

    let peers = crate::network::get_nearby_peers(&network).await;

    Ok(peers
        .into_iter()
        .map(|(_, beacon, origin)| NearbyPeerResponse::from((beacon, origin)))
        .collect())
}

pub async fn connect_nearby<S: StateWrapper>(
    state: S,
    peer_id: String,
    method: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    let session_name = state
        .network_lock()
        .await
        .my_beacon
        .lock()
        .await
        .as_ref()
        .map(|b| b.session_name.clone())
        .ok_or(KursalError::Network("No active beacon".to_string()))?;

    state
        .core_cmd_tx()
        .send(CoreCommand::ConnectNearby {
            peer_id,
            session_name,
            method,
            reply: reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    reply_rx.await.ok_kursal(KursalError::Network)??;

    Ok(())
}

pub async fn accept_nearby<S: StateWrapper>(state: S, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby_lock().await.remove(&peer_id) {
        tx.send(true).ok();
    }

    Ok(())
}

pub async fn decline_nearby<S: StateWrapper>(state: S, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby_lock().await.remove(&peer_id) {
        tx.send(false).ok();
    }

    Ok(())
}

pub async fn get_contacts<S: StateWrapper>(state: S) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(&*state.db_lock().await)?;

    Ok(contacts.into_iter().map(ContactResponse::from).collect())
}

pub async fn get_contact<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Option<ContactResponse>> {
    let user_id = parse_contact_id(&contact_id)?;

    let contact = Contact::load(&*state.db_lock().await, &user_id)?;

    Ok(contact.map(ContactResponse::from))
}

core_request!(remove_contact(contact_id: String) => RemoveContact -> ());

pub async fn send_text<S: StateWrapper>(
    state: S,
    contact_id: String,
    text: String,
    reply_to: Option<String>,
) -> Result<String> {
    let reply_to_id = reply_to.as_deref().map(parse_message_id).transpose()?;

    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::SendText {
            contact_id,
            text,
            reply_to: reply_to_id,
            reply: reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    let msg_id = reply_rx.await.ok_kursal(KursalError::Network)??;

    Ok(hex::encode(msg_id.0))
}

core_request!(send_typing_indicator(contact_id: String) => SendTypingIndicator -> ());
core_request!(send_read_receipts(contact_id: String, message_ids: Vec<String>) => SendReadReceipts -> ());
core_request!(delete_local_message(contact_id: String, message_id: String) => DeleteLocalMessage -> ());
core_request!(retry_message(contact_id: String, message_id: String) => RetryMessage -> ());

pub async fn get_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
    limit: usize,
    before: Option<String>,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;
    let before = before.as_deref().map(parse_message_id).transpose()?;

    let guard = state.db_lock().await;
    let stored = StoredMessage::load_recent(&guard, &user_id, limit, before.as_ref())?;
    let contact = Contact::load(&guard, &user_id)?;
    drop(guard);

    let mut rows: Vec<MessageResponse> = stored.into_iter().map(MessageResponse::from).collect();
    if let Some(contact) = contact {
        crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
    }
    Ok(rows)
}

pub async fn get_messages_after<S: StateWrapper>(
    state: S,
    contact_id: String,
    after: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;
    let after_id = parse_message_id(&after)?;

    let guard = state.db_lock().await;
    let stored = StoredMessage::load_after(&guard, &user_id, &after_id, limit)?;
    let contact = Contact::load(&guard, &user_id)?;
    drop(guard);

    let mut rows: Vec<MessageResponse> = stored.into_iter().map(MessageResponse::from).collect();
    if let Some(contact) = contact {
        crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
    }
    Ok(rows)
}

pub async fn get_messages_around<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;
    let message_id = parse_message_id(&message_id)?;

    let guard = state.db_lock().await;
    let stored = StoredMessage::load_around(&guard, &user_id, &message_id, limit)?;
    let contact = Contact::load(&guard, &user_id)?;
    drop(guard);

    let mut rows: Vec<MessageResponse> = stored.into_iter().map(MessageResponse::from).collect();
    if let Some(contact) = contact {
        crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
    }
    Ok(rows)
}

pub async fn search_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
    query: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;

    let guard = state.db_lock().await;
    let stored = StoredMessage::search(&guard, &user_id, &query, limit)?;
    let contact = Contact::load(&guard, &user_id)?;
    drop(guard);

    let mut rows: Vec<MessageResponse> = stored.into_iter().map(MessageResponse::from).collect();
    if let Some(contact) = contact {
        crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
    }
    Ok(rows)
}

pub async fn search_messages_global<S: StateWrapper>(
    state: S,
    query: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let stored = StoredMessage::search_global(&*state.db_lock().await, &query, limit)?;
    Ok(stored.into_iter().map(MessageResponse::from).collect())
}

pub async fn get_pinned_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Vec<MessageResponse>> {
    let uid = parse_contact_id(&contact_id)?;
    let db = state.db_lock().await;
    let ids = pin_index_list(&db, &uid)?;
    let mut msgs: Vec<MessageResponse> = ids
        .iter()
        .filter_map(|id| StoredMessage::load(&db, &uid, id).ok().flatten())
        .map(MessageResponse::from)
        .collect();
    msgs.sort_by_key(|m| m.timestamp);
    Ok(msgs)
}

pub async fn get_security_code<S: StateWrapper>(state: S, contact_id: String) -> Result<String> {
    let user_id = parse_contact_id(&contact_id)?;

    let db = state.db_lock().await;
    let contact = Contact::load(&db, &user_id)?
        .ok_or(KursalError::Storage("Contact not found".to_string()))?;

    let local_identity_pub = get_local_identity_pub(&db)?;
    let local_dilithium_pub = get_dilithium_pub(&db)?;

    let security = security_code(
        &local_identity_pub,
        &local_dilithium_pub,
        &contact.identity_pub_key,
        &contact.dilithium_pub_key,
    );

    Ok(security)
}

pub async fn confirm_security_code<S: StateWrapper>(state: S, contact_id: String) -> Result<()> {
    let user_id = parse_contact_id(&contact_id)?;

    Contact::set_verified(&*state.db_lock().await, &user_id)?;

    Ok(())
}

pub async fn set_contact_blocked<S: StateWrapper>(
    state: S,
    contact_id: String,
    value: bool,
) -> Result<()> {
    let user_id = parse_contact_id(&contact_id)?;

    Contact::set_blocked(&*state.db_lock().await, &user_id, value)?;

    Ok(())
}

pub async fn get_blocked_contacts<S: StateWrapper>(state: S) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(&*state.db_lock().await)?
        .into_iter()
        .filter(|user| user.blocked)
        .map(ContactResponse::from)
        .collect();

    Ok(contacts)
}

pub async fn rotate_peer_id<S: StateWrapper>(state: S) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::RotatePeerId { reply: reply_tx })
        .await
        .ok();

    reply_rx
        .await
        .map_err(|_| KursalError::Network("channel dropped".to_string()))?
}

pub async fn get_local_peer_id<S: StateWrapper>(state: S) -> Result<String> {
    let network = state.network_lock().await;
    Ok(network.primary.peer_id.to_base58())
}

pub async fn get_local_user_id_hex<S: StateWrapper>(state: S) -> Result<String> {
    let db = state.db_lock().await;
    let uid = get_local_user_id(&db)?;
    Ok(hex::encode(uid.0))
}

pub async fn get_local_user_profile<S: StateWrapper>(state: S) -> (String, Option<Vec<u8>>) {
    let db = state.db_lock().await;

    get_local_profile(&db)
}

pub async fn broadcast_profile<S: StateWrapper>(
    state: S,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    set_local_profile(
        &*state.db_lock().await,
        display_name.clone(),
        avatar_bytes.clone(),
    )?;

    state
        .core_cmd_tx()
        .send(CoreCommand::BroadcastProfile {
            display_name,
            avatar_bytes,
            reply: reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    reply_rx.await.ok_kursal(KursalError::Network)?
}

core_request!(share_profile(display_name: String, avatar_bytes: Option<Vec<u8>>, contact_id: String) => ShareProfile -> ());
core_request!(delete_message_for_everyone(contact_id: String, message_id: String) => DeleteMessage -> bool);
core_request!(pin_message(contact_id: String, message_id: String, pinned: bool) => PinMessage -> bool);
core_request!(edit_message(contact_id: String, message_id: String, new_content: String) => EditMessage -> bool);
core_request!(add_reaction(contact_id: String, message_id: String, emoji: String) => ReactionAdd -> bool);
core_request!(remove_reaction(contact_id: String, message_id: String, emoji: String) => ReactionRemove -> bool);
core_request!(send_file_offer(contact_id: String, file_path: String, app_data_dir: std::path::PathBuf) => SendFileOffer -> (String, u64, String), map |(msg_id, file_size, stored_path): (MessageId, u64, String)| (hex::encode(msg_id.0), file_size, stored_path));
core_request!(accept_file_offer(contact_id: String, offer_id: String, save_path: String) => AcceptFileOffer -> ());
core_request!(cancel_file_transfer(contact_id: String, offer_id: String) => CancelFileTransfer -> ());
core_request!(flush_offline(contact_id: String) => FlushOffline -> ());
core_request!(add_custom_node(addr: String) => AddCustomNode -> ());
core_request!(remove_custom_node(addr: String) => RemoveCustomNode -> ());
core_request!(dial_address(addr: String) => DialAddress -> ());
core_request!(network_status() => NetworkStatus -> NetworkStatusDto);
#[cfg(feature = "calls")]
core_request!(start_call(contact_id: String) => StartCall -> String, map |call_id: MessageId| hex::encode(call_id.0));
#[cfg(feature = "calls")]
core_request!(accept_call() => AcceptCall -> ());
#[cfg(feature = "calls")]
core_request!(decline_call() => DeclineCall -> ());
#[cfg(feature = "calls")]
core_request!(hangup_call() => HangupCall -> ());
#[cfg(feature = "calls")]
core_request!(start_video(codec: String, width: u16, height: u16) => StartVideo -> ());
#[cfg(feature = "calls")]
core_request!(stop_video(reason: String) => StopVideo -> ());
#[cfg(feature = "calls")]
core_request!(request_video_keyframe() => RequestVideoKeyframe -> ());
#[cfg(feature = "calls")]
core_request!(set_mute(muted: bool) => SetMute -> ());
#[cfg(feature = "calls")]
core_request!(set_deafen(deafened: bool) => SetDeafen -> ());
#[cfg(feature = "calls")]
core_request!(set_audio_device(kind: String, name: Option<String>) => SetAudioDevice -> ());
#[cfg(feature = "calls")]
core_request!(list_audio_devices() => ListAudioDevices -> crate::call::audio::AudioDevices);
