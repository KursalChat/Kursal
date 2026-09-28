use crate::dialog_bridge::{DialogRequest, ask};
use kursal_core::storage::SharedDatabase;
use serde_json::json;
use tauri::{AppHandle, Manager};

const STABLE_ENDPOINT: &str = "https://app.kursal.chat/latest.json";
const BETA_ENDPOINT: &str = "https://app.kursal.chat/v/beta/latest.json";
#[cfg(any(target_os = "android", target_os = "ios"))]
const PLAY_LISTING: &str = "https://play.google.com/store/apps/details?id=chat.kursal";
#[cfg(any(target_os = "android", target_os = "ios"))]
const APP_STORE_LISTING: &str = "https://apps.apple.com/us/app/kursal/id6807047934";
const DAILY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

fn fail(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn endpoint(app: &AppHandle) -> &'static str {
    let state = app.state::<kursal_core::api::state::AppState>();
    if kursal_core::storage::get_update_channel(state.db()) == "beta" {
        BETA_ENDPOINT
    } else {
        STABLE_ENDPOINT
    }
}

/// `Ok(false)` means the app is already up to date
pub async fn check(app: AppHandle, manual: bool) -> Result<(), String> {
    if cfg!(dev) {
        return Ok(());
    }

    log::debug!("checking for updates... manual={manual}");

    if !find(&app).await? && manual {
        ask(
            &app,
            DialogRequest::alert("no_updates", json!({}), "default"),
        )
        .await;
    }

    Ok(())
}

pub fn spawn_daily(app: AppHandle, db: SharedDatabase) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(DAILY);

        loop {
            interval.tick().await;

            if kursal_core::storage::get_updater_enabled(&db) {
                let _ = check(app.clone(), false).await;
            }
        }
    });
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn find(app: &AppHandle) -> Result<bool, String> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint(app).parse().map_err(fail)?])
        .map_err(fail)?
        .build()
        .map_err(fail)?;

    let Some(update) = updater.check().await.map_err(fail)? else {
        return Ok(false);
    };

    let accepted = ask(
        app,
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

    if !accepted {
        return Ok(true);
    }

    let progress_app = app.clone();
    let finish_app = app.clone();
    let mut downloaded = 0usize;
    let mut last_emit = std::time::Instant::now();

    update
        .download_and_install(
            move |chunk_len, content_len| {
                downloaded += chunk_len;
                let complete = Some(downloaded as u64) == content_len;
                // Throttled: the ring only needs a few frames a second.
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
                let _ = finish_app.emit("update_download_finished", ());
            },
        )
        .await
        .map_err(fail)?;

    log::info!("[updater] update installed");

    let restart = ask(
        app,
        DialogRequest::confirm("update_installed", json!({}), "default"),
    )
    .await;

    if restart {
        app.restart();
    }

    Ok(true)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn listing_url() -> Option<&'static str> {
    let listing = if cfg!(target_os = "ios") {
        APP_STORE_LISTING
    } else {
        PLAY_LISTING
    };
    (!listing.is_empty()).then_some(listing)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
async fn find(app: &AppHandle) -> Result<bool, String> {
    use tauri_plugin_opener::OpenerExt;

    #[derive(serde::Deserialize)]
    struct LatestRelease {
        version: String,
    }

    let release: LatestRelease = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(fail)?
        .get(endpoint(app))
        .send()
        .await
        .map_err(fail)?
        .error_for_status()
        .map_err(fail)?
        .json()
        .await
        .map_err(fail)?;

    let latest = semver::Version::parse(release.version.trim_start_matches('v')).map_err(fail)?;
    let current = app.package_info().version.clone();

    if latest <= current {
        return Ok(false);
    }

    let listing = listing_url();
    let params = json!({
        "version": latest.to_string(),
        "currentVersion": current.to_string(),
        "store": if cfg!(target_os = "ios") { "appstore" } else { "play" },
        "open": listing.is_some(),
    });

    let request = match listing {
        Some(_) => DialogRequest::confirm("update_store", params, "default"),
        None => DialogRequest::alert("update_store", params, "default"),
    };

    if ask(app, request).await
        && let Some(url) = listing
    {
        app.opener().open_url(url, None::<&str>).map_err(fail)?;
    }

    Ok(true)
}
