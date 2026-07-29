use crate::deep_link::dispatch_deep_links;
use crate::dirs::{app_data_dir, cache_dir, logs_dir};
use crate::error::Result;
use kursal_core::KursalError;
use kursal_core::MapKursalResult;
use kursal_core::api::cmd_wrapper::StateWrapper;
use kursal_core::api::state::AppState;
use kursal_core::api::{CoreCommand, cmd_wrapper};
use kursal_core::apiserver::LocalApiConfig;
use kursal_core::dto::{
    ContactResponse, MessageResponse, NearbyPeerResponse, NetworkStatusDto, NodesResponse,
    OtpResponse,
};
use kursal_core::network::NetworkManager;
use kursal_core::storage::backup::{generate_backup, load_backup};
use kursal_core::storage::{
    AutoAcceptConfig, AutoDownloadConfig, Database, RelayConfig, SharedFileEntry, StorageUsage,
    api_server_config, delete_message_history_all, delete_message_history_for, files_list_shared,
    files_revoke_shared, get_local_profile, get_local_user_id, get_swarm_listening_port,
    get_swarm_mdns_enabled, reset_full_app, set_api_server_config, set_new_api_server_password,
    set_swarm_listening_port, set_swarm_mdns_enabled,
};
use std::collections::HashMap;
use tauri_plugin_opener::OpenerExt;
use tokio::fs::remove_dir_all;
use tokio::sync::{MutexGuard, mpsc, oneshot};

pub struct AppStateWrapper<'a>(pub tauri::State<'a, AppState>);

impl StateWrapper for AppStateWrapper<'_> {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand> {
        &self.0.core_cmd_tx
    }
    async fn network_lock(&self) -> MutexGuard<'_, NetworkManager> {
        self.0.network.lock().await
    }
    async fn pending_nearby_lock(&self) -> MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>> {
        self.0.pending_nearby.lock().await
    }
    async fn db_lock(&self) -> MutexGuard<'_, Database> {
        self.0.db.0.lock().await
    }
}

macro_rules! core_cmd {
    ($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        core_cmd!($name($($arg: $ty),*) -> $ret, as $name);
    };
    ($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty, as $wrapped:ident) => {
        #[tauri::command]
        pub async fn $name(state: tauri::State<'_, AppState> $(, $arg: $ty)*) -> Result<$ret> {
            cmd_wrapper::$wrapped(AppStateWrapper(state) $(, $arg)*)
                .await
                .map_err(Into::into)
        }
    };
}

macro_rules! setting_cmd {
    (get $name:ident -> $ret:ty, $fn:path) => {
        #[tauri::command]
        pub async fn $name(state: tauri::State<'_, AppState>) -> Result<$ret> {
            Ok($fn(&*state.db().await))
        }
    };
    (try $name:ident -> $ret:ty, $fn:path) => {
        #[tauri::command]
        pub async fn $name(state: tauri::State<'_, AppState>) -> Result<$ret> {
            $fn(&*state.db().await).map_err(Into::into)
        }
    };
    (set $name:ident($arg:ident: $ty:ty), $fn:path) => {
        #[tauri::command]
        pub async fn $name(state: tauri::State<'_, AppState>, $arg: $ty) -> Result<()> {
            $fn(&*state.db().await, $arg).map_err(Into::into)
        }
    };
    (set ref $name:ident($arg:ident: $ty:ty), $fn:path) => {
        #[tauri::command]
        pub async fn $name(state: tauri::State<'_, AppState>, $arg: $ty) -> Result<()> {
            $fn(&*state.db().await, &$arg).map_err(Into::into)
        }
    };
}

#[tauri::command]
pub async fn generate_otp() -> Result<OtpResponse> {
    cmd_wrapper::generate_otp().await.map_err(Into::into)
}

core_cmd!(publish_otp(otp: String) -> ());
core_cmd!(fetch_otp(otp: String) -> ContactResponse);
core_cmd!(export_ltc() -> Vec<u8>);
core_cmd!(import_ltc(bytes: Vec<u8>) -> ContactResponse);
core_cmd!(start_nearby() -> String);
core_cmd!(stop_nearby() -> ());
core_cmd!(get_nearby_peers() -> Vec<NearbyPeerResponse>);
core_cmd!(connect_nearby(peer_id: String, method: String) -> ());
core_cmd!(accept_nearby(peer_id: String) -> ());
core_cmd!(decline_nearby(peer_id: String) -> ());
core_cmd!(get_contacts() -> Vec<ContactResponse>);

#[tauri::command]
pub async fn remove_contact(state: tauri::State<'_, AppState>, contact_id: String) -> Result<()> {
    let file_dir = cache_dir()?.join("files").join(&contact_id);

    cmd_wrapper::remove_contact(AppStateWrapper(state), contact_id).await?;
    match remove_dir_all(&file_dir).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(KursalError::Io(e).into()),
    }

    Ok(())
}

core_cmd!(send_text(contact_id: String, text: String, reply_to: Option<String>) -> String);
core_cmd!(start_call(contact_id: String) -> String);

#[tauri::command]
pub async fn accept_call(state: tauri::State<'_, AppState>, call_id: String) -> Result<()> {
    let _ = call_id;
    cmd_wrapper::accept_call(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn decline_call(state: tauri::State<'_, AppState>, call_id: String) -> Result<()> {
    let _ = call_id;
    cmd_wrapper::decline_call(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn hangup(state: tauri::State<'_, AppState>, call_id: String) -> Result<()> {
    let _ = call_id;
    cmd_wrapper::hangup_call(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

core_cmd!(start_video(codec: String, width: u16, height: u16) -> ());
core_cmd!(stop_video(reason: String) -> ());
core_cmd!(request_video_keyframe() -> ());

#[tauri::command]
pub async fn send_video_chunk(request: tauri::ipc::Request<'_>) -> Result<()> {
    match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => {
            kursal_core::call::video::send_chunk(bytes.clone());
        }
        tauri::ipc::InvokeBody::Json(value) => {
            use base64::Engine;
            if let Some(encoded) = value.as_str()
                && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded)
            {
                kursal_core::call::video::send_chunk(bytes);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "android")]
fn video_chunk_response(bytes: Vec<u8>) -> tauri::ipc::InvokeResponseBody {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    tauri::ipc::InvokeResponseBody::Json(serde_json::to_string(&encoded).unwrap_or_default())
}

#[cfg(not(target_os = "android"))]
fn video_chunk_response(bytes: Vec<u8>) -> tauri::ipc::InvokeResponseBody {
    tauri::ipc::InvokeResponseBody::Raw(bytes)
}

#[tauri::command]
pub async fn video_rx_channel(
    channel: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
) -> Result<()> {
    kursal_core::call::video::set_rx_forwarder(Box::new(move |bytes| {
        let _ = channel.send(video_chunk_response(bytes));
    }));
    Ok(())
}

#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!("[webview] {message}"),
        "warn" => log::warn!("[webview] {message}"),
        "debug" => log::debug!("[webview] {message}"),
        _ => log::info!("[webview] {message}"),
    }
}

core_cmd!(set_mute(muted: bool) -> ());
core_cmd!(set_deafen(deafened: bool) -> ());
core_cmd!(set_audio_device(kind: String, name: Option<String>) -> ());
core_cmd!(list_audio_devices() -> kursal_core::call::audio::AudioDevices);
core_cmd!(send_typing_indicator(contact_id: String) -> ());
core_cmd!(send_read_receipts(contact_id: String, message_ids: Vec<String>) -> ());
core_cmd!(delete_local_message(contact_id: String, message_id: String) -> ());
core_cmd!(retry_message(contact_id: String, message_id: String) -> ());
core_cmd!(get_messages(contact_id: String, limit: usize, before: Option<String>) -> Vec<MessageResponse>);
core_cmd!(get_messages_after(contact_id: String, after: String, limit: usize) -> Vec<MessageResponse>);
core_cmd!(get_messages_around(contact_id: String, message_id: String, limit: usize) -> Vec<MessageResponse>);
core_cmd!(search_messages(contact_id: String, query: String, limit: usize) -> Vec<MessageResponse>);
core_cmd!(search_messages_global(query: String, limit: usize) -> Vec<MessageResponse>);
core_cmd!(get_security_code(contact_id: String) -> String);
core_cmd!(confirm_security_code(contact_id: String) -> ());
core_cmd!(set_contact_blocked(contact_id: String, value: bool) -> ());
core_cmd!(list_blocked_contacts() -> Vec<ContactResponse>, as get_blocked_contacts);
core_cmd!(rotate_peer_id() -> ());
core_cmd!(get_local_peer_id() -> String);

#[tauri::command]
pub async fn get_local_user_id_hex(state: tauri::State<'_, AppState>) -> Result<String> {
    let db = state.db().await;
    let uid = get_local_user_id(&db)?;
    Ok(hex::encode(uid.0))
}

setting_cmd!(get get_local_user_profile -> (String, Option<Vec<u8>>), get_local_profile);

core_cmd!(broadcast_profile(display_name: String, avatar_bytes: Option<Vec<u8>>) -> ());
core_cmd!(share_profile(display_name: String, avatar_bytes: Option<Vec<u8>>, contact_id: String) -> ());
core_cmd!(delete_message_for_everyone(contact_id: String, message_id: String) -> bool);
core_cmd!(pin_message(contact_id: String, message_id: String, pinned: bool) -> bool);
core_cmd!(get_pinned_messages(contact_id: String) -> Vec<MessageResponse>);
core_cmd!(edit_message(contact_id: String, message_id: String, new_content: String) -> bool);
core_cmd!(add_reaction(contact_id: String, message_id: String, emoji: String) -> bool);
core_cmd!(remove_reaction(contact_id: String, message_id: String, emoji: String) -> bool);
core_cmd!(send_file_offer(contact_id: String, file_path: String) -> (String, u64));
core_cmd!(accept_file_offer(contact_id: String, offer_id: String, save_path: String) -> ());
core_cmd!(cancel_file_transfer(contact_id: String, offer_id: String) -> ());
core_cmd!(flush_offline(contact_id: String) -> ());

// OUTSIDE cmd_wrapper / SETTINGS

#[tauri::command]
pub async fn get_storage_usage(state: tauri::State<'_, AppState>) -> Result<StorageUsage> {
    let logs_dir = logs_dir()?;
    let cache_dir = cache_dir()?;

    kursal_core::storage::get_storage_usage(
        &*state.db().await,
        logs_dir.to_path_buf(),
        cache_dir.to_path_buf(),
        state.db_path.clone(),
    )
    .map_err(Into::into)
}

#[tauri::command]
pub async fn resolve_download_path(
    contact_id: String,
    offer_id: String,
    filename: String,
) -> Result<String> {
    let path = kursal_core::storage::filetransfer::download_path(
        cache_dir()?.to_path_buf(),
        &contact_id,
        &offer_id,
        &filename,
    );

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(KursalError::Io)?;
    }

    Ok(path.to_string_lossy().into_owned())
}

setting_cmd!(get get_auto_download_config -> AutoDownloadConfig, kursal_core::storage::get_auto_download_config);
setting_cmd!(set set_auto_download_config(config: AutoDownloadConfig), kursal_core::storage::set_auto_download_config);
setting_cmd!(get get_auto_accept_config -> AutoAcceptConfig, kursal_core::storage::get_auto_accept_config);
setting_cmd!(set set_auto_accept_config(config: AutoAcceptConfig), kursal_core::storage::set_auto_accept_config);

setting_cmd!(try list_shared_files -> Vec<SharedFileEntry>, files_list_shared);
setting_cmd!(set revoke_shared_file(id: String), files_revoke_shared);

#[tauri::command]
pub async fn revoke_shared_files_bulk(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<()> {
    let db = state.db().await;

    for id in ids {
        files_revoke_shared(&db, id)?;
    }

    Ok(())
}

setting_cmd!(get get_nearby_share_enabled -> bool, get_swarm_mdns_enabled);
setting_cmd!(set set_nearby_share_enabled(value: bool), set_swarm_mdns_enabled);
setting_cmd!(get get_relay_config -> RelayConfig, kursal_core::storage::get_relay_config);
setting_cmd!(set set_relay_config(config: RelayConfig), kursal_core::storage::set_relay_config);
setting_cmd!(get get_nodes -> NodesResponse, kursal_core::api::nodes::list_nodes);

core_cmd!(add_custom_node(addr: String) -> ());
core_cmd!(remove_custom_node(addr: String) -> ());
core_cmd!(dial_address(addr: String) -> ());
core_cmd!(get_network_status() -> NetworkStatusDto, as network_status);

#[tauri::command]
pub fn get_node_stats() -> kursal_core::stats::NodeStats {
    kursal_core::stats::global_sample()
}

setting_cmd!(get get_listening_port -> Option<u16>, get_swarm_listening_port);
setting_cmd!(set set_listening_port(port: Option<u16>), set_swarm_listening_port);
setting_cmd!(try get_local_api_config -> LocalApiConfig, api_server_config);
setting_cmd!(set set_local_api_config(config: LocalApiConfig), set_api_server_config);

#[tauri::command]
pub async fn generate_local_api_token(state: tauri::State<'_, AppState>) -> Result<String> {
    let db = state.db.clone();

    let token = tokio::task::spawn_blocking(move || {
        let guard = db.0.blocking_lock();
        set_new_api_server_password(&guard)
    })
    .await
    .ok_kursal(KursalError::Crypto)??;
    Ok(token)
}

#[tauri::command]
pub async fn delete_all_local_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<()> {
    reset_full_app(&*state.db().await)?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use std::sync::atomic::Ordering;
        use tauri::Manager;
        app.state::<crate::background::BackgroundState>()
            .explicit_quit
            .store(true, Ordering::Relaxed);
    }

    app.exit(0);

    Ok(())
}

#[tauri::command]
pub async fn clear_message_history(
    state: tauri::State<'_, AppState>,
    contact_id: Option<String>, // None = ALL CONTACTS
) -> Result<()> {
    if let Some(contact_id) = contact_id {
        delete_message_history_for(&*state.db().await, contact_id)?;
    } else {
        delete_message_history_all(&*state.db().await)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_peer_rotation_interval(state: tauri::State<'_, AppState>) -> Result<String> {
    let result = match kursal_core::storage::get_peer_rotation_interval(&*state.db().await) {
        21_600 => "6h",
        43_200 => "12h",
        108_000 => "30h",
        604_800 => "7d",
        _ => "manual",
    };

    Ok(result.to_string())
}
#[tauri::command]
pub async fn set_peer_rotation_interval(
    state: tauri::State<'_, AppState>,
    interval: String,
) -> Result<()> {
    let result: u64 = match interval.as_str() {
        "6h" => 21_600,
        "12h" => 43_200,
        "30h" => 108_000,
        "7d" => 604_800,
        _ => 0,
    };

    kursal_core::storage::set_peer_rotation_interval(&*state.db().await, result)?;

    Ok(())
}

setting_cmd!(get get_typing_indicators_enabled -> bool, kursal_core::storage::get_typing_indicators_enabled);
setting_cmd!(set set_typing_indicators_enabled(value: bool), kursal_core::storage::set_typing_indicators_enabled);

setting_cmd!(get get_read_receipts_enabled -> bool, kursal_core::storage::get_read_receipts_enabled);
setting_cmd!(set set_read_receipts_enabled(value: bool), kursal_core::storage::set_read_receipts_enabled);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMetaDto {
    pub contact_id: String,
    pub muted: bool,
    pub last_seen_at: Option<u64>,
    pub alias: Option<String>,
    pub terminated: bool,
}

#[tauri::command]
pub async fn set_contact_muted(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    value: bool,
) -> Result<()> {
    kursal_core::storage::set_contact_muted(&*state.db().await, &contact_id, value)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn set_contact_alias(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    value: Option<String>,
) -> Result<()> {
    kursal_core::storage::set_contact_alias(&*state.db().await, &contact_id, value)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_contact_meta(state: tauri::State<'_, AppState>) -> Result<Vec<ContactMetaDto>> {
    let db = state.db().await;
    let contacts = kursal_core::contacts::Contact::load_all(&db)?;
    Ok(contacts
        .into_iter()
        .map(|c| {
            let id = hex::encode(c.user_id.0);
            ContactMetaDto {
                muted: kursal_core::storage::get_contact_muted(&db, &id),
                last_seen_at: kursal_core::storage::get_contact_last_seen(&db, &id),
                alias: kursal_core::storage::get_contact_alias(&db, &id),
                terminated: kursal_core::storage::get_contact_terminated(&db, &id),
                contact_id: id,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_ui_state(
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<Option<String>> {
    kursal_core::storage::get_ui_state(&*state.db().await, &key).map_err(Into::into)
}
#[tauri::command]
pub async fn set_ui_state(
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> Result<()> {
    kursal_core::storage::set_ui_state(&*state.db().await, &key, &value).map_err(Into::into)
}

setting_cmd!(get get_call_sample_rate -> u32, kursal_core::storage::get_call_sample_rate);
setting_cmd!(set set_call_sample_rate(rate: u32), kursal_core::storage::set_call_sample_rate);

setting_cmd!(get get_video_quality -> u32, kursal_core::storage::get_video_quality);
setting_cmd!(set set_video_quality(quality: u32), kursal_core::storage::set_video_quality);

// OUTSIDE cmd_wrapper

#[tauri::command]
pub async fn export_backup(state: tauri::State<'_, AppState>, password: String) -> Result<Vec<u8>> {
    let app_data_dir = app_data_dir()?;

    generate_backup(
        password,
        &state.keychain_config,
        app_data_dir,
        &state.db_path,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn import_backup(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    password: String,
    bytes: Vec<u8>,
) -> Result<()> {
    let app_data_dir = app_data_dir()?;

    load_backup(
        password,
        bytes,
        &state.db_path,
        &state.keychain_config,
        app_data_dir,
    )
    .await?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use std::sync::atomic::Ordering;
        use tauri::Manager;
        app.state::<crate::background::BackgroundState>()
            .explicit_quit
            .store(true, Ordering::Relaxed);
    }

    app.restart()
}

setting_cmd!(get get_updater_enabled -> bool, kursal_core::storage::get_updater_enabled);
setting_cmd!(set set_updater_enabled(value: bool), kursal_core::storage::set_updater_enabled);
setting_cmd!(get get_update_channel -> String, kursal_core::storage::get_update_channel);
setting_cmd!(set ref set_update_channel(channel: String), kursal_core::storage::set_update_channel);
setting_cmd!(get get_background_mode -> bool, kursal_core::storage::get_background_mode);

#[tauri::command]
pub async fn set_background_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<()> {
    kursal_core::storage::set_background_mode(&*state.db().await, value)?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use crate::background::{BackgroundState, build_tray, remove_tray};
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        app.state::<BackgroundState>()
            .background_enabled
            .store(value, Ordering::Relaxed);

        if value {
            let _ = build_tray(&app);
        } else {
            remove_tray(&app);
        }
    }

    let _ = &app;
    Ok(())
}

#[tauri::command]
pub fn set_busy_state(app: tauri::AppHandle, call_active: bool, transfer_active: bool) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use crate::background::{BackgroundState, request_quit};
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        let Some(bg) = app.try_state::<BackgroundState>() else {
            return;
        };
        bg.call_active.store(call_active, Ordering::Relaxed);
        bg.transfer_active.store(transfer_active, Ordering::Relaxed);

        if !call_active && !transfer_active && bg.quit_when_idle.swap(false, Ordering::Relaxed) {
            request_quit(&app);
        }
    }

    let _ = (&app, call_active, transfer_active);
}

#[tauri::command]
pub fn set_close_explainer_pending(app: tauri::AppHandle, value: bool) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use crate::background::BackgroundState;
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        if let Some(bg) = app.try_state::<BackgroundState>() {
            bg.close_explainer_pending.store(value, Ordering::Relaxed);
        }
    }

    let _ = (&app, value);
}

#[tauri::command]
pub fn close_to_background(app: tauri::AppHandle, until_idle: bool) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::background::close_to_background(&app, until_idle);

    let _ = (&app, until_idle);
}

#[tauri::command]
pub fn close_force_quit(app: tauri::AppHandle) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::background::request_quit(&app);

    let _ = &app;
}

setting_cmd!(set ref set_notification_preview(value: String), kursal_core::storage::set_notification_preview);
setting_cmd!(set ref set_notification_dnd(value: String), kursal_core::storage::set_notification_dnd);

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> std::result::Result<(), String> {
    crate::check_for_updates_impl(app, true)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command]
pub async fn check_for_updates(_app: tauri::AppHandle) -> std::result::Result<(), String> {
    Err("Updates are handled by the app store on mobile devices.".to_string())
}

#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<()> {
    if let Ok(log_dir) = crate::dirs::logs_dir() {
        app.opener()
            .open_path(log_dir.to_string_lossy(), None::<&str>)
            .ok_kursal(KursalError::Storage)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn open_files_folder(app: tauri::AppHandle) -> Result<()> {
    if let Ok(cache_dir) = crate::dirs::cache_dir() {
        app.opener()
            .open_path(cache_dir.join("files").to_string_lossy(), None::<&str>)
            .ok_kursal(KursalError::Storage)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn frontend_ready(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<()> {
    let urls = {
        let mut queue = state.deep_links.lock().unwrap();
        queue.frontend_ready = true;
        std::mem::take(&mut queue.pending)
    };

    if !urls.is_empty() {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let _ = crate::background::show_or_create_main(&_app);

        dispatch_deep_links(&state, urls).await;
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::background::drain_pending_signal(&_app);

    Ok(())
}
