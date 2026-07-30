use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Wry};

const TRAY_ID: &str = "kursal_tray";

pub struct BackgroundState {
    pub explicit_quit: AtomicBool,
    pub background_enabled: AtomicBool,
    pub pending_signal: StdMutex<Option<PendingSignal>>,
    pub unread: AtomicUsize,
    pub net_online: AtomicBool,
    pub net_peers: AtomicUsize,
    pub conn_status: StdMutex<HashMap<String, TrayLink>>,
    pub tray_refresh_queued: AtomicBool,
    pub call_active: AtomicBool,
    pub transfer_active: AtomicBool,
    pub quit_when_idle: AtomicBool,
    pub close_explainer_pending: AtomicBool,
}

pub enum PendingSignal {
    NewContact,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TrayLink {
    Direct,
    Relay,
}

pub fn show_or_create_main(app: &AppHandle) -> tauri::Result<()> {
    let existed = if let Some(win) = app.get_webview_window("main") {
        win.show()?;
        win.set_focus()?;
        true
    } else {
        #[allow(unused_mut)]
        let mut builder =
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Kursal")
                .inner_size(1080.0, 780.0)
                .min_inner_size(500.0, 300.0)
                .center()
                .user_agent("KursalChat (kursal.chat)");

        #[cfg(target_os = "macos")]
        {
            use tauri::TitleBarStyle;
            builder = builder
                .title_bar_style(TitleBarStyle::Overlay)
                .hidden_title(true);
        }

        builder.build()?;
        false
    };

    if existed {
        drain_pending_signal(app);
    }

    if let Some(bg) = app.try_state::<BackgroundState>() {
        bg.quit_when_idle.store(false, Ordering::Relaxed);
        if !bg.background_enabled.load(Ordering::Relaxed) {
            remove_tray(app);
        }
    }
    refresh_tray(app);

    Ok(())
}

pub fn drain_pending_signal(app: &AppHandle) {
    if let Some(bg) = app.try_state::<BackgroundState>() {
        let pending = bg.pending_signal.lock().unwrap().take();
        if let Some(sig) = pending {
            let (signal, payload) = match sig {
                PendingSignal::NewContact => ("new_contact", String::new()),
            };
            let _ = app.emit(
                "backend_signal",
                serde_json::json!({ "signal": signal, "payload": payload }),
            );
        }
    }
}

pub fn request_quit(app: &AppHandle) {
    app.state::<BackgroundState>()
        .explicit_quit
        .store(true, Ordering::Relaxed);
    app.exit(0);
}

pub fn is_busy(app: &AppHandle) -> (bool, bool) {
    app.try_state::<BackgroundState>()
        .map(|s| {
            (
                s.call_active.load(Ordering::Relaxed),
                s.transfer_active.load(Ordering::Relaxed),
            )
        })
        .unwrap_or((false, false))
}

pub fn request_close_confirmation(app: &AppHandle, allow_explainer: bool) -> bool {
    let (call_active, transfer_active) = is_busy(app);
    let first_close = allow_explainer
        && app
            .try_state::<BackgroundState>()
            .map(|s| {
                s.close_explainer_pending.load(Ordering::Relaxed)
                    && s.background_enabled.load(Ordering::Relaxed)
            })
            .unwrap_or(false);

    if !call_active && !transfer_active && !first_close {
        return false;
    }

    match app.get_webview_window("main") {
        Some(win) if win.is_visible().unwrap_or(false) => {
            let _ = win.set_focus();
        }
        _ => {
            let _ = show_or_create_main(app);
        }
    }

    let _ = app.emit(
        "close_requested",
        serde_json::json!({
            "callActive": call_active,
            "transferActive": transfer_active,
            "firstClose": first_close,
        }),
    );
    true
}

pub fn close_to_background(app: &AppHandle, until_idle: bool) {
    if let Some(bg) = app.try_state::<BackgroundState>() {
        bg.quit_when_idle.store(until_idle, Ordering::Relaxed);
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = build_tray(app);
        let _ = win.hide();
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon missing");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Kursal")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_show" => {
                let _ = show_or_create_main(app);
            }
            "tray_add_contact" => {
                if let Some(bg) = app.try_state::<BackgroundState>() {
                    *bg.pending_signal.lock().unwrap() = Some(PendingSignal::NewContact);
                }
                let _ = show_or_create_main(app);
            }
            "tray_quit" if !request_close_confirmation(app, false) => {
                request_quit(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_or_create_main(tray.app_handle());
            }
        })
        .build(app)?;

    refresh_tray(app);

    Ok(())
}

pub fn remove_tray(app: &AppHandle) {
    let _ = app.remove_tray_by_id(TRAY_ID);
}

pub fn refresh_tray(app: &AppHandle) {
    let Some(bg) = app.try_state::<BackgroundState>() else {
        return;
    };
    if app.tray_by_id(TRAY_ID).is_none() {
        return;
    }
    if bg.tray_refresh_queued.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;

        let (unread, online, peers, direct, relay) = {
            let bg = app.state::<BackgroundState>();
            bg.tray_refresh_queued.store(false, Ordering::Release);
            let map = bg.conn_status.lock().unwrap();
            (
                bg.unread.load(Ordering::Relaxed),
                bg.net_online.load(Ordering::Relaxed),
                bg.net_peers.load(Ordering::Relaxed),
                map.values().filter(|l| **l == TrayLink::Direct).count(),
                map.values().filter(|l| **l == TrayLink::Relay).count(),
            )
        };

        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = apply_tray(&app2, unread, online, peers, direct, relay);
        });
    });
}

fn apply_tray(
    app: &AppHandle,
    unread: usize,
    online: bool,
    peers: usize,
    direct: usize,
    relay: usize,
) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };

    let header_text = if unread == 0 {
        "Kursal".to_string()
    } else {
        format!("Kursal — {unread} unread")
    };
    let header = MenuItem::with_id(app, "tray_header", header_text, false, None::<&str>)?;

    let status_text = if !online {
        "Offline".to_string()
    } else if peers == 1 {
        "Online — 1 peer".to_string()
    } else {
        format!("Online — {peers} peers")
    };
    let net_status = MenuItem::with_id(app, "tray_net_status", status_text, false, None::<&str>)?;
    let net_split = MenuItem::with_id(
        app,
        "tray_net_split",
        format!("Direct: {direct} · Relay: {relay}"),
        false,
        None::<&str>,
    )?;
    let net_refs: Vec<&dyn IsMenuItem<Wry>> = vec![&net_status, &net_split];
    let network = Submenu::with_id_and_items(app, "tray_network", "Network", true, &net_refs)?;

    let add_contact =
        MenuItem::with_id(app, "tray_add_contact", "Add contact…", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "tray_show", "Show Kursal", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray_quit", "Quit Kursal", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[&header, &sep1, &network, &add_contact, &sep2, &show, &quit],
    )?;
    tray.set_menu(Some(menu))?;

    if let Some(base) = app.default_window_icon() {
        let (w, h) = (base.width(), base.height());
        let mut rgba = base.rgba().to_vec();
        if unread > 0 {
            stamp_dot(&mut rgba, w, h);
        }
        tray.set_icon(Some(Image::new_owned(rgba, w, h)))?;
    }

    Ok(())
}

fn stamp_dot(rgba: &mut [u8], w: u32, h: u32) {
    let r = (w.min(h) as f32 * 0.24).max(3.0);
    let cx = w as f32 - r - 1.0;
    let cy = h as f32 - r - 1.0;
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r * r {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = 0xe1;
                rgba[i + 1] = 0x1d;
                rgba[i + 2] = 0x48;
                rgba[i + 3] = 0xff;
            }
        }
    }
}
