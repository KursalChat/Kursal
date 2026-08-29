use crate::{
    KursalError, MapKursalResult, Result,
    api::{CoreCommand, file_transfers::attach_file_paths},
    contacts::Contact,
    dto::{
        ContactResponse, LtcStatusDto, MessageResponse, NearbyPeerResponse, NetworkStatusDto,
        OtpResponse, PendingSyncDto, UnreadDto,
    },
    first_contact::{
        nearby::{NearbyBeacon, generate_session_name},
        otp,
    },
    identity::{UserId, security_code},
    messaging::{
        StoredMessage, UNREAD_BADGE_CAP, enums::MessageId, message_before, newest_message,
        newest_received, pin_index_list, received_since_rev, unread_after,
    },
    network::NetworkManager,
    storage::{
        Database, SharedDatabase, avatars, conversation, get_dilithium_pub, get_local_avatar_bytes,
        get_local_identity_pub, get_local_profile, get_local_user_id, get_read_receipts_enabled,
        set_local_avatar, set_local_display_name,
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
    fn db(&self) -> &Database;
    fn db_handle(&self) -> SharedDatabase;
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

core_request!(get_ltc_status() => GetLtcStatus -> Option<LtcStatusDto>);
core_request!(create_ltc(max_uses: Option<u32>, ttl_secs: Option<u64>) => CreateLtc -> LtcStatusDto);
core_request!(update_ltc_limits(max_uses: Option<u32>, ttl_secs: Option<u64>) => UpdateLtcLimits -> LtcStatusDto);
core_request!(export_ltc() => ExportLtc -> Vec<u8>);
core_request!(set_ltc_follow_rotations(enabled: bool) => SetLtcFollowRotations -> LtcStatusDto);
core_request!(republish_ltc_pointer() => RepublishLtcPointer -> LtcStatusDto);
core_request!(revoke_ltc() => RevokeLtc -> ());
core_request!(import_ltc(bytes: Vec<u8>) => ImportLtc -> ContactResponse, map ContactResponse::from);

core_request!(publish_otp(otp: String) => PublishOtp -> ());
core_request!(fetch_otp(otp: String) => FetchOtp -> ContactResponse, map ContactResponse::from);

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
    let contacts = Contact::load_all(state.db())?;

    Ok(contacts.into_iter().map(ContactResponse::from).collect())
}

pub async fn get_contact<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Option<ContactResponse>> {
    let user_id = parse_contact_id(&contact_id)?;

    let contact = Contact::load(state.db(), &user_id)?;

    Ok(contact.map(ContactResponse::from))
}

pub async fn get_contact_avatar<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Option<Vec<u8>>> {
    let user_id = parse_contact_id(&contact_id)?;

    Ok(Contact::load(state.db(), &user_id)?
        .and_then(|contact| contact.avatar)
        .as_deref()
        .and_then(avatars::read))
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

    state
        .db_handle()
        .blocking(move |db| {
            let stored = StoredMessage::load_recent(db, &user_id, limit, before.as_ref())?;
            let contact = Contact::load(db, &user_id)?;

            let mut rows: Vec<MessageResponse> =
                stored.into_iter().map(MessageResponse::from).collect();
            attach_file_paths(db, &mut rows)?;

            if let Some(contact) = contact {
                crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
            }
            Ok(rows)
        })
        .await
}

pub async fn get_messages_after<S: StateWrapper>(
    state: S,
    contact_id: String,
    after: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;
    let after_id = parse_message_id(&after)?;

    state
        .db_handle()
        .blocking(move |db| {
            let stored = StoredMessage::load_after(db, &user_id, &after_id, limit)?;
            let contact = Contact::load(db, &user_id)?;

            let mut rows: Vec<MessageResponse> =
                stored.into_iter().map(MessageResponse::from).collect();
            attach_file_paths(db, &mut rows)?;

            if let Some(contact) = contact {
                crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
            }
            Ok(rows)
        })
        .await
}

pub async fn get_messages_around<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;
    let message_id = parse_message_id(&message_id)?;

    state
        .db_handle()
        .blocking(move |db| {
            let stored = StoredMessage::load_around(db, &user_id, &message_id, limit)?;
            let contact = Contact::load(db, &user_id)?;

            let mut rows: Vec<MessageResponse> =
                stored.into_iter().map(MessageResponse::from).collect();
            attach_file_paths(db, &mut rows)?;

            if let Some(contact) = contact {
                crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
            }
            Ok(rows)
        })
        .await
}

pub async fn search_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
    query: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    let user_id = parse_contact_id(&contact_id)?;

    state
        .db_handle()
        .blocking(move |db| {
            let stored = StoredMessage::search(db, &user_id, &query, limit)?;
            let contact = Contact::load(db, &user_id)?;

            let mut rows: Vec<MessageResponse> =
                stored.into_iter().map(MessageResponse::from).collect();
            if let Some(contact) = contact {
                crate::dto::apply_offline_overlay(&mut rows, &contact.offline);
            }
            Ok(rows)
        })
        .await
}

pub async fn search_messages_global<S: StateWrapper>(
    state: S,
    query: String,
    limit: usize,
) -> Result<Vec<MessageResponse>> {
    state
        .db_handle()
        .blocking(move |db| {
            let stored = StoredMessage::search_global(db, &query, limit)?;
            Ok(stored.into_iter().map(MessageResponse::from).collect())
        })
        .await
}

pub async fn get_pinned_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Vec<MessageResponse>> {
    let uid = parse_contact_id(&contact_id)?;
    let db = state.db();
    let ids = pin_index_list(db, &uid)?;
    let mut msgs: Vec<MessageResponse> = ids
        .iter()
        .filter_map(|id| StoredMessage::load(db, &uid, id).ok().flatten())
        .map(MessageResponse::from)
        .collect();
    attach_file_paths(db, &mut msgs)?;
    msgs.sort_by_key(|m| m.timestamp);
    Ok(msgs)
}

const READ_RECEIPT_CAP: usize = 200;

fn unread_entry(db: &Database, contact_id: &UserId) -> Result<UnreadDto> {
    let contact_hex = hex::encode(contact_id.0);
    let cursor = conversation::get_read_cursor(db, &contact_hex)
        .and_then(|hex_id| parse_message_id(&hex_id).ok());

    let unread = unread_after(db, contact_id, cursor.as_ref(), UNREAD_BADGE_CAP)?;

    Ok(UnreadDto {
        count: unread.count,
        capped: unread.capped,
        first_unread: unread.first.map(|id| hex::encode(id.0)),
        marked_unread: conversation::get_marked_unread(db, &contact_hex),
        contact_id: contact_hex,
    })
}

pub async fn get_unread_summary<S: StateWrapper>(state: S) -> Result<Vec<UnreadDto>> {
    let db = state.db();
    let contacts = Contact::load_all(db)?;

    contacts
        .iter()
        .map(|contact| unread_entry(db, &contact.user_id))
        .collect()
}

pub async fn mark_contact_read<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Vec<String>> {
    let user_id = parse_contact_id(&contact_id)?;
    let contact_hex = hex::encode(user_id.0);
    let db = state.db();

    if conversation::get_marked_unread(db, &contact_hex) {
        conversation::set_marked_unread(db, &contact_hex, false)?;
    }

    let cursor = conversation::get_read_cursor(db, &contact_hex)
        .and_then(|hex_id| parse_message_id(&hex_id).ok());

    let Some(newest) = newest_message(db, &user_id)? else {
        return Ok(Vec::new());
    };
    if cursor == Some(newest) {
        return Ok(Vec::new());
    }

    let receipts = if get_read_receipts_enabled(db) {
        received_since_rev(db, &user_id, cursor.as_ref(), READ_RECEIPT_CAP)?
            .iter()
            .map(|id| hex::encode(id.0))
            .collect()
    } else {
        Vec::new()
    };

    conversation::set_read_cursor(db, &contact_hex, Some(&hex::encode(newest.0)))?;

    Ok(receipts)
}

pub async fn mark_contact_unread<S: StateWrapper>(
    state: S,
    contact_id: String,
    from_message_id: Option<String>,
) -> Result<UnreadDto> {
    let user_id = parse_contact_id(&contact_id)?;
    let contact_hex = hex::encode(user_id.0);
    let db = state.db();

    let anchor = match from_message_id.as_deref() {
        Some(id) => Some(parse_message_id(id)?),
        None => newest_received(db, &user_id)?,
    };

    if let Some(anchor) = anchor {
        let cursor = message_before(db, &user_id, &anchor)?;
        conversation::set_read_cursor(
            db,
            &contact_hex,
            cursor.map(|id| hex::encode(id.0)).as_deref(),
        )?;
    }
    conversation::set_marked_unread(db, &contact_hex, true)?;

    unread_entry(db, &user_id)
}

pub async fn set_contact_marked_unread<S: StateWrapper>(
    state: S,
    contact_id: String,
    value: bool,
) -> Result<()> {
    let contact_hex = hex::encode(parse_contact_id(&contact_id)?.0);

    conversation::set_marked_unread(state.db(), &contact_hex, value)
}

pub async fn get_delayed_unseen<S: StateWrapper>(state: S) -> Result<HashMap<String, Vec<String>>> {
    Ok(conversation::list_delayed_unseen(state.db())
        .into_iter()
        .collect())
}

pub async fn set_delayed_unseen<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_ids: Vec<String>,
) -> Result<()> {
    let contact_hex = hex::encode(parse_contact_id(&contact_id)?.0);

    conversation::set_delayed_unseen(state.db(), &contact_hex, &message_ids)
}

pub async fn get_pending_sync<S: StateWrapper>(state: S) -> Result<PendingSyncDto> {
    let entries = conversation::list_pending_sync(state.db());

    let mut dto = PendingSyncDto {
        sync: Vec::with_capacity(entries.len()),
        deleted: Vec::new(),
    };
    for (contact_id, message_id, is_delete) in entries {
        let key = format!("{contact_id}:{message_id}");
        if is_delete {
            dto.deleted.push(key.clone());
        }
        dto.sync.push(key);
    }

    Ok(dto)
}

pub async fn get_security_code<S: StateWrapper>(state: S, contact_id: String) -> Result<String> {
    let user_id = parse_contact_id(&contact_id)?;

    let db = state.db();
    let contact = Contact::load(db, &user_id)?
        .ok_or(KursalError::Storage("Contact not found".to_string()))?;

    let local_identity_pub = get_local_identity_pub(db)?;
    let local_dilithium_pub = get_dilithium_pub(db)?;

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

    Contact::set_verified(state.db(), &user_id)?;

    Ok(())
}

pub async fn set_contact_blocked<S: StateWrapper>(
    state: S,
    contact_id: String,
    value: bool,
) -> Result<()> {
    let user_id = parse_contact_id(&contact_id)?;

    Contact::set_blocked(state.db(), &user_id, value)?;

    Ok(())
}

pub async fn get_blocked_contacts<S: StateWrapper>(state: S) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(state.db())?
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
    let db = state.db();
    let uid = get_local_user_id(db)?;
    Ok(hex::encode(uid.0))
}

pub async fn get_local_user_profile<S: StateWrapper>(state: S) -> (String, Option<Vec<u8>>) {
    let db = state.db();
    let (username, hash) = get_local_profile(db);

    (username, hash.as_deref().and_then(avatars::read))
}

pub async fn get_local_user_profile_path<S: StateWrapper>(state: S) -> (String, Option<String>) {
    let db = state.db();
    let (username, hash) = get_local_profile(db);

    (username, hash.as_deref().and_then(avatars::path_string))
}

pub async fn set_local_user_avatar<S: StateWrapper>(
    state: S,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<Option<String>> {
    let hash = set_local_avatar(state.db(), avatar_bytes)?;

    Ok(hash.as_deref().and_then(avatars::path_string))
}

pub async fn broadcast_profile<S: StateWrapper>(
    state: S,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    set_local_avatar(state.db(), avatar_bytes)?;

    broadcast_stored_profile(state, display_name).await
}

pub async fn broadcast_stored_profile<S: StateWrapper>(
    state: S,
    display_name: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    set_local_display_name(state.db(), display_name.clone())?;
    let avatar_bytes = get_local_avatar_bytes(state.db());

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

pub async fn share_stored_profile<S: StateWrapper>(state: S, contact_id: String) -> Result<()> {
    let (display_name, _) = get_local_profile(state.db());
    let avatar_bytes = get_local_avatar_bytes(state.db());

    share_profile(state, display_name, avatar_bytes, contact_id).await
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
core_request!(start_video() => StartVideo -> ());
#[cfg(feature = "calls")]
core_request!(set_camera(camera_id: Option<String>) => SetCamera -> ());
#[cfg(feature = "calls")]
core_request!(refresh_camera_rotation(angle: u16) => RefreshCameraRotation -> ());
#[cfg(feature = "calls")]
core_request!(request_local_keyframe() => RequestLocalKeyframe -> ());
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
#[cfg(feature = "calls")]
core_request!(list_cameras() => ListCameras -> Vec<crate::dto::CameraInfo>);
