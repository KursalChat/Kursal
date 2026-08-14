use crate::core_event::handle_core_event;
use crate::deep_link::map_deep_links;
use crate::error::install_panic_hook;
use clap::{Parser, Subcommand};
use kursal_cli::CLIArgs;
use kursal_core::apiserver::CoreEventEmitter;
use kursal_core::storage::{api_server_config, api_server_password, should_reset_full_app};
use kursal_core::{
    api::{
        AppEvent, CoreCommand,
        state::{AppState, DeepLinkQueue},
    },
    identity::{
        self,
        keychain::{self, KeychainConfig},
    },
    network::{NetworkManager, dispatch_events},
};
use std::fs::remove_dir_all;
use std::sync::Mutex as StdMutex;
use std::{collections::HashMap, sync::Arc};
use tauri::{Manager, async_runtime::block_on};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::sync::{Mutex, broadcast, mpsc};

#[cfg(target_os = "android")]
pub mod android_ble;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod background;
pub mod benchmark;
pub mod commands;
pub mod core_event;
pub mod deep_link;
pub mod dialog_bridge;
pub mod dirs;
pub mod error;
pub mod file;
pub mod outgoing_sweep;
pub mod share_intake;
#[cfg(test)]
mod tests;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod window_menu;

#[derive(Parser, Default)]
#[command(version, about, long_about = None, author)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// ID of the used database
    #[arg(long)]
    database_id: Option<String>,
    /// [UNSAFE!!] Will write the database encryption key in a file - MEANT FOR DEBUGGING!
    #[arg(long)]
    unsafe_write_key_to_file: bool,
}

#[derive(Subcommand)]
enum Commands {
    Cli(CLIArgs),
}

#[cfg(target_os = "macos")]
fn disable_automatic_text_substitutions() {
    use objc2_foundation::{NSString, NSUserDefaults};

    const KEYS: [&str; 6] = [
        "WebAutomaticQuoteSubstitutionEnabled",
        "WebAutomaticDashSubstitutionEnabled",
        "WebAutomaticTextReplacementEnabled",
        "NSAutomaticQuoteSubstitutionEnabled",
        "NSAutomaticDashSubstitutionEnabled",
        "NSAutomaticTextReplacementEnabled",
    ];

    let defaults = NSUserDefaults::standardUserDefaults();
    for key in KEYS {
        defaults.setBool_forKey(false, &NSString::from_str(key));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // arch seems to... not render the app window lets say
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        }
    }

    match Args::try_parse() {
        Ok(args) => {
            if let Some(Commands::Cli(cli_args)) = args.command {
                if let Err(err) = block_on(kursal_cli::run(
                    cli_args.config,
                    cli_args.validate,
                    cli_args.default_config,
                    cli_args.tui,
                )) {
                    eprintln!("{err}");
                    std::process::exit(1);
                }

                return;
            }
        }
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.exit()
            }
            _ => {}
        },
    }

    #[cfg(target_os = "macos")]
    disable_automatic_text_substitutions();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        #[allow(unused_mut)]
        let mut autostart = tauri_plugin_autostart::Builder::new();

        #[cfg(target_os = "macos")]
        {
            autostart =
                autostart.macos_launcher(tauri_plugin_autostart::MacosLauncher::AppleScript);
        }

        builder = builder
            .plugin(autostart.build())
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    use std::sync::atomic::Ordering;
                    use tauri::Manager;
                    let app = window.app_handle();
                    let (bg_en, eq) = app
                        .try_state::<crate::background::BackgroundState>()
                        .map(|s| {
                            (
                                s.background_enabled.load(Ordering::Relaxed),
                                s.explicit_quit.load(Ordering::Relaxed),
                            )
                        })
                        .unwrap_or((false, false));

                    if !eq && crate::background::request_close_confirmation(app, true) {
                        api.prevent_close();
                    } else if bg_en && !eq {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        crate::background::request_quit(app);
                    }
                }
            });
    }

    #[cfg(not(any(target_os = "android", target_os = "ios", dev)))]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(
                tauri_plugin_window_state::Builder::new()
                    .with_state_flags(
                        tauri_plugin_window_state::StateFlags::SIZE
                            | tauri_plugin_window_state::StateFlags::POSITION
                            | tauri_plugin_window_state::StateFlags::MAXIMIZED
                            | tauri_plugin_window_state::StateFlags::FULLSCREEN,
                    )
                    .build(),
            )
            .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                let _ = background::show_or_create_main(app);

                let urls: Vec<tauri::Url> = args
                    .iter()
                    .filter_map(|arg| tauri::Url::parse(arg).ok())
                    .filter(|url| url.scheme() == "kursal")
                    .collect();

                if !urls.is_empty()
                    && let Err(err) = deep_link::deep_link_handler(app, map_deep_links(urls))
                {
                    log::error!("Error while handling deep linking: {err}");
                }
            }));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        builder = builder
            .plugin(tauri_plugin_haptics::init())
            .plugin(tauri_plugin_barcode_scanner::init())
            .plugin(tauri_plugin_biometric::init());
    }

    #[cfg(any(target_os = "android", target_os = "ios", target_os = "macos"))]
    {
        builder = builder.plugin(tauri_plugin_sharekit::init())
    }

    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            window_menu::setup(app)?;

            let args = Args::try_parse().unwrap_or_default();
            dirs::init_dirs(app)?;
            let log_path = dirs::logs_dir()?.join(format!(
                "{}.log",
                args.database_id.clone().unwrap_or("kursal".to_string())
            ));
            let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

            kursal_core::logging::init_logging(&log_level, Some(&log_path.to_string_lossy()))?;
            log::info!("Logging enabled, writing to {}", log_path.display());

            let _ = install_panic_hook();
            log::info!("Panic hook installed");

            if let Err(ohno) = keychain::init_keychain() {
                log::error!(
                    "Could not initiate keychain. This may trigger a crash later on. Error: {ohno}"
                );
            } else {
                log::info!("Keychain initialized");
            }

            let app_data_dir = dirs::app_data_dir()?;
            log::info!("Directories initialized");

            let db_path = app_data_dir.join(format!(
                "{}.db",
                args.database_id.clone().unwrap_or("kursal".to_string())
            ));

            let keychain_config = KeychainConfig {
                storage_id: args.database_id.unwrap_or("master".to_string()),
                unsafe_write_key_to_file: args.unsafe_write_key_to_file,
            };

            log::info!(
                "About to init identity (db_path={}, storage_id={})",
                db_path.display(),
                keychain_config.storage_id
            );

            let db = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                block_on(identity::init(&db_path, &keychain_config, app_data_dir))
            })) {
                Ok(Ok(db)) => {
                    log::info!("Identity init succeeded");
                    db
                }
                Ok(Err(e)) => {
                    log::error!("Identity init returned error: {e}");
                    return Err(Box::new(e).into());
                }
                Err(panic_info) => {
                    let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    log::error!("Identity init PANICKED: {msg}");
                    return Err(format!("identity::init panicked: {msg}").into());
                }
            };
            let (network, event_rx, bt_event_rx, chunk_rx) =
                block_on(NetworkManager::new(&db))?;

            // check for reset flags
            let db_clone = db.clone();
            let should_reset = block_on(async move {
                should_reset_full_app(&db_clone)
            });
            if should_reset {
                if let Ok(cache_dir) = dirs::cache_dir() {
                    let _ = remove_dir_all(cache_dir);
                }
                if let Ok(logs_dir) = dirs::logs_dir() {
                    let _ = remove_dir_all(logs_dir);
                }
                if let Ok(app_data_dir) = dirs::app_data_dir() {
                    let _ = remove_dir_all(app_data_dir);
                }

                app.handle().restart();
            }


            let (core_cmd_tx, core_cmd_rx) = mpsc::channel::<CoreCommand>(32);
            let (app_event_tx, mut app_event_rx) = mpsc::channel::<AppEvent>(16);
            let network_arc = Arc::new(Mutex::new(network));
            let pending_nearby = Arc::new(Mutex::new(HashMap::new()));

            let db_clone = db.clone();
            let network_clone = network_arc.clone();
            let app_tx_clone = app_event_tx.clone();
            let pending_nearby_clone = pending_nearby.clone();

            let sweep_db = db.clone();
            let sweep_app_data = dirs::app_data_dir()?.to_path_buf();
            tauri::async_runtime::spawn(async move {
                outgoing_sweep::sweep(sweep_db, &sweep_app_data).await;
            });

            tauri::async_runtime::spawn(NetworkManager::spawn_address_announcer(
                db.clone(),
                core_cmd_tx.clone(),
            ));

            tauri::async_runtime::spawn(NetworkManager::spawn_rotation_scheduler(
                db.clone(),
                core_cmd_tx.clone(),
            ));

            let dispatch_app_data = dirs::app_data_dir()?;
            std::thread::spawn(move || {
                let local = tokio::task::LocalSet::new();

                block_on(local.run_until(dispatch_events(
                    event_rx,
                    bt_event_rx,
                    core_cmd_rx,
                    chunk_rx,
                    db_clone,
                    network_clone,
                    app_tx_clone,
                    dispatch_app_data,
                )));
            });

            let handle = app.handle().clone();
            let (api_server_tx, _): (broadcast::Sender<CoreEventEmitter>, _) = broadcast::channel(64);

            let api_server_tx_clone = api_server_tx.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = app_event_rx.recv().await {
                    handle_core_event(event, &handle, &api_server_tx_clone, &pending_nearby_clone).await;
                }
            });

            let start_urls = app
                .deep_link()
                .get_current()?
                .map(map_deep_links)
                .unwrap_or_default();

            app.manage(AppState {
                db: db.clone(),
                network: network_arc.clone(),
                app_event_tx: app_event_tx.clone(),
                core_cmd_tx: core_cmd_tx.clone(),
                pending_nearby: pending_nearby.clone(),
                db_path,
                deep_links: StdMutex::new(DeepLinkQueue {
                    frontend_ready: false,
                    pending: start_urls,
                }),
                keychain_config
            });

            app.manage(commands::NodeStatsState::default());

            #[cfg(target_os = "windows")]
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.with_webview(|webview| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::{
                        COREWEBVIEW2_PERMISSION_KIND,
                        COREWEBVIEW2_PERMISSION_KIND_CAMERA,
                        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
                        COREWEBVIEW2_PERMISSION_STATE_ALLOW,
                    };
                    use webview2_com::PermissionRequestedEventHandler;

                    if let Ok(core) = webview.controller().CoreWebView2() {
                        let mut token = std::mem::zeroed();
                        let _ = core.add_PermissionRequested(
                            &PermissionRequestedEventHandler::create(Box::new(
                                move |_sender, args| {
                                    if let Some(args) = args {
                                        let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                                        args.PermissionKind(&mut kind)?;
                                        if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                                            || kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                                        {
                                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                                        }
                                    }
                                    Ok(())
                                },
                            )),
                            &mut token,
                        );
                    }
                });
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use crate::background::BackgroundState;
                use std::collections::HashMap;
                use std::sync::atomic::{AtomicBool, AtomicUsize};

                let bg_on = {
                    let guard = &*db;
                    kursal_core::storage::get_background_mode(guard)
                };
                app.manage(BackgroundState {
                    explicit_quit: AtomicBool::new(false),
                    background_enabled: AtomicBool::new(bg_on),
                    pending_signal: StdMutex::new(None),
                    unread: AtomicUsize::new(0),
                    net_online: AtomicBool::new(false),
                    net_peers: AtomicUsize::new(0),
                    conn_status: StdMutex::new(HashMap::new()),
                    tray_refresh_queued: AtomicBool::new(false),
                    call_active: AtomicBool::new(false),
                    transfer_active: AtomicBool::new(false),
                    quit_when_idle: AtomicBool::new(false),
                    close_explainer_pending: AtomicBool::new(false),
                });

                if bg_on {
                    let _ = crate::background::build_tray(app.handle());
                }
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let handle = app.handle().clone();
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;

                    let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));

                    loop {
                        use kursal_core::storage::get_updater_enabled;

                        interval.tick().await;

                        if get_updater_enabled(&db_clone) {
                            let _ = check_for_updates_impl(handle.clone(), false).await;
                        }
                    }
                });
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                if let Err(err) = deep_link::deep_link_handler(&handle, map_deep_links(event.urls())) {
                    log::error!("Error while handling deep linking: {err}");
                }
            });

            // spawn local API server if enabled
            let core_cmd_tx_clone = core_cmd_tx.clone();
            let db_clone = db.clone();
            let network_clone = network_arc.clone();
            let pending_nearby_clone = pending_nearby.clone();
            let api_app_data_dir = dirs::app_data_dir()?.to_path_buf();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting API server...");

                let api_config = api_server_config(&db);
                let api_token = api_server_password(&db);

                if let Ok(api_config) = api_config
                && api_config.enabled
                && let Ok(api_token) = api_token {
                    if let Err(err) = kursal_core::apiserver::run_server(
                        api_token,
                        api_config,
                        core_cmd_tx_clone,
                        db_clone,
                        network_clone,
                        pending_nearby_clone,
                        api_server_tx,
                        api_app_data_dir,
                    )
                    .await
                    {
                        log::error!("Error while running API server: {err}");
                    }
                } else {
                    log::info!("API server could not start because of an invalid auth_token setting. Please re-generate the token.");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generate_otp,
            commands::publish_otp,
            commands::fetch_otp,
            commands::check_otp_words,
            commands::get_ltc_status,
            commands::create_ltc,
            commands::update_ltc_limits,
            commands::export_ltc,
            commands::set_ltc_follow_rotations,
            commands::republish_ltc_pointer,
            commands::revoke_ltc,
            commands::import_ltc,
            commands::start_nearby,
            commands::stop_nearby,
            commands::get_nearby_peers,
            commands::connect_nearby,
            commands::accept_nearby,
            commands::decline_nearby,
            commands::get_contacts,
            commands::remove_contact,
            commands::send_text,
            commands::start_call,
            commands::accept_call,
            commands::decline_call,
            commands::hangup,
            commands::start_video,
            commands::stop_video,
            commands::request_video_keyframe,
            commands::send_video_chunk,
            commands::video_rx_channel,
            commands::log_frontend,
            commands::set_mute,
            commands::set_deafen,
            commands::set_audio_device,
            commands::list_audio_devices,
            commands::send_typing_indicator,
            commands::send_read_receipts,
            commands::get_messages,
            commands::get_messages_after,
            commands::get_messages_around,
            commands::search_messages,
            commands::search_messages_global,
            commands::delete_local_message,
            commands::retry_message,
            commands::get_unread_summary,
            commands::mark_contact_read,
            commands::mark_contact_unread,
            commands::set_contact_marked_unread,
            commands::get_delayed_unseen,
            commands::set_delayed_unseen,
            commands::get_pending_sync,
            commands::get_security_code,
            commands::confirm_security_code,
            commands::set_contact_blocked,
            commands::list_blocked_contacts,
            commands::rotate_peer_id,
            commands::get_local_peer_id,
            commands::get_local_user_id_hex,
            commands::get_local_user_profile,
            commands::set_local_user_avatar,
            commands::broadcast_profile,
            commands::share_profile,
            commands::check_for_updates,
            commands::open_log_folder,
            commands::open_files_folder,
            commands::delete_message_for_everyone,
            commands::edit_message,
            commands::pin_message,
            commands::get_pinned_messages,
            commands::add_reaction,
            commands::remove_reaction,
            commands::accept_file_offer,
            commands::send_file_offer,
            commands::create_outgoing_pending_path,
            commands::cancel_file_transfer,
            commands::paths_exist,
            commands::take_pending_shares,
            commands::discard_pending_share,
            commands::flush_offline,
            //
            commands::export_backup,
            commands::import_backup,
            commands::get_updater_enabled,
            commands::set_updater_enabled,
            commands::get_update_channel,
            commands::set_update_channel,
            commands::get_background_mode,
            commands::set_background_mode,
            commands::set_busy_state,
            commands::set_tray_unread,
            commands::close_to_background,
            commands::close_force_quit,
            commands::set_close_explainer_pending,
            commands::set_notification_preview,
            commands::set_notification_dnd,
            commands::set_contact_muted,
            commands::set_contact_alias,
            commands::get_contact_meta,
            commands::get_read_receipts_enabled,
            commands::set_read_receipts_enabled,
            commands::get_storage_usage,
            commands::resolve_download_path,
            commands::get_auto_download_config,
            commands::set_auto_download_config,
            commands::get_auto_accept_config,
            commands::set_auto_accept_config,
            commands::list_shared_files,
            commands::revoke_shared_file,
            commands::revoke_shared_files_bulk,
            commands::get_nearby_share_enabled,
            commands::set_nearby_share_enabled,
            commands::get_relay_config,
            commands::set_relay_config,
            commands::get_nodes,
            commands::add_custom_node,
            commands::remove_custom_node,
            commands::dial_address,
            commands::get_network_status,
            commands::get_node_stats,
            commands::start_node_stats,
            commands::stop_node_stats,
            commands::get_listening_port,
            commands::set_listening_port,
            commands::get_local_api_config,
            commands::set_local_api_config,
            commands::generate_local_api_token,
            commands::delete_all_local_data,
            commands::clear_message_history,
            commands::get_peer_rotation_interval,
            commands::set_peer_rotation_interval,
            commands::get_typing_indicators_enabled,
            commands::set_typing_indicators_enabled,
            commands::get_ui_state,
            commands::set_ui_state,
            commands::get_call_sample_rate,
            commands::set_call_sample_rate,
            commands::get_video_quality,
            commands::set_video_quality,
            commands::frontend_ready,
            dialog_bridge::dialog_respond,
            dialog_bridge::run_startup_dialogs,
            //
            benchmark::run_benchmark,
            benchmark::list_benchmarks,
            benchmark::cancel_benchmark,
            benchmark::is_benchmark_running,
        ])
        .build(tauri::generate_context!())
        .expect("error while building application")
        .run(|_app, _event| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use crate::background::BackgroundState;
                use std::sync::atomic::Ordering;
                use tauri::Manager;

                if let tauri::RunEvent::ExitRequested { api, code, .. } = &_event {
                    let bg = _app.state::<BackgroundState>();
                    if code.is_none()
                        && (bg.background_enabled.load(Ordering::Relaxed)
                            || bg.quit_when_idle.load(Ordering::Relaxed))
                        && !bg.explicit_quit.load(Ordering::Relaxed)
                    {
                        api.prevent_exit();
                    }
                }
            }

            #[cfg(target_os = "macos")]
            {
                if let tauri::RunEvent::Reopen { .. } = _event {
                    let _ = background::show_or_create_main(_app);
                }

                if let tauri::RunEvent::Opened { urls } = _event {
                    use crate::file::open_files;

                    let files: Vec<(String, String)> = urls
                        .iter()
                        .filter_map(|url| url.to_file_path().ok())
                        .filter_map(|p| {
                            let name = p.file_name()?.to_string_lossy().to_string();
                            let path = p.to_string_lossy().to_string();
                            Some((path, name))
                        })
                        .collect();

                    if !files.is_empty() {
                        open_files(_app, files);
                    }
                }
            }
        });
}

#[cfg(all(not(any(target_os = "android", target_os = "ios")), dev))]
pub(crate) async fn check_for_updates_impl(
    _app: tauri::AppHandle,
    _manual: bool,
) -> tauri_plugin_updater::Result<()> {
    Ok(()) // do not try to update on dev mode
}

#[cfg(not(any(target_os = "android", target_os = "ios", dev)))]
pub(crate) async fn check_for_updates_impl(
    app: tauri::AppHandle,
    manual: bool,
) -> tauri_plugin_updater::Result<()> {
    log::debug!("checking for updates... manual={manual}");
    use crate::dialog_bridge::{DialogRequest, ask};
    use serde_json::json;
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;

    let channel = {
        let state = app.state::<kursal_core::api::state::AppState>();
        let guard = state.db();
        kursal_core::storage::get_update_channel(&*guard)
    };

    let updater = if channel == "beta" {
        app.updater_builder()
            .endpoints(vec![
                "https://app.kursal.chat/v/beta/latest.json"
                    .parse()
                    .unwrap(),
            ])?
            .build()?
    } else {
        app.updater()?
    };

    if let Some(update) = updater.check().await? {
        let do_update = ask(
            &app,
            DialogRequest::confirm(
                "update_available",
                json!({
                    "version": update.version,
                    "currentVersion": update.current_version,
                    "notes": update.body,
                }),
                "default",
            ),
        )
        .await;

        if !do_update {
            return Ok(());
        }

        let progress_app = app.clone();
        let finish_app = app.clone();
        let mut downloaded = 0usize;
        let mut last_emit = std::time::Instant::now();

        update
            .download_and_install(
                move |chunk_len, content_len| {
                    downloaded += chunk_len;
                    log::debug!("[updater] downloaded {downloaded} out of {content_len:?}");
                    let complete = Some(downloaded as u64) == content_len;
                    if !complete && last_emit.elapsed() < std::time::Duration::from_millis(200) {
                        return;
                    }
                    last_emit = std::time::Instant::now();
                    let _ = progress_app.emit(
                        "update_download_progress",
                        json!({ "downloaded": downloaded, "contentLength": content_len }),
                    );
                },
                move || {
                    log::debug!("[updater] download finished");
                    let _ = finish_app.emit("update_download_finished", ());
                },
            )
            .await?;

        log::info!("[updater] update installed");

        let do_restart = ask(
            &app,
            DialogRequest::confirm("update_installed", json!({}), "default"),
        )
        .await;

        if do_restart {
            app.restart();
        }
    } else if manual {
        ask(
            &app,
            DialogRequest::alert("no_updates", json!({}), "default"),
        )
        .await;
    }

    Ok(())
}
