use kursal_core::api::{
    AppEvent,
    state::{AppState, DeepLink},
};
use tauri::{Manager, async_runtime::block_on};

pub async fn dispatch_deep_links(state: &AppState, urls: Vec<DeepLink>) {
    for url in urls {
        let (signal, payload) = match url.category.as_deref() {
            Some("settings") => ("open_settings", String::with_capacity(0)),
            Some("otp") => ("open_otp", url.path),
            Some("node") => ("add_node", url.path),
            _ => continue,
        };

        state
            .app_event_tx
            .send(AppEvent::BackendSignal {
                signal: signal.to_string(),
                payload,
            })
            .await
            .ok();
    }
}

pub fn deep_link_handler(handle: &tauri::AppHandle, urls: Vec<DeepLink>) -> tauri::Result<()> {
    log::info!("Opening deep link URLs: {:?}", urls);

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = crate::background::show_or_create_main(handle);

    let state = handle.state::<AppState>();

    {
        let mut queue = state.deep_links.lock().unwrap();
        if !queue.frontend_ready {
            queue.pending.extend(urls);
            return Ok(());
        }
    }

    block_on(dispatch_deep_links(&state, urls));

    Ok(())
}

pub fn map_deep_links(urls: Vec<tauri::Url>) -> Vec<DeepLink> {
    urls.into_iter()
        .map(|url| {
            let category = url.host_str().map(str::to_owned);
            let path = url.path().strip_prefix('/').unwrap_or("").to_owned();
            DeepLink { category, path }
        })
        .collect::<Vec<_>>()
}
