#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn setup(app: &mut tauri::App) -> Result<(), tauri::Error> {
    use tauri::Manager;
    use tauri::menu::{MenuBuilder, MenuItem, SubmenuBuilder};

    let root_menu = SubmenuBuilder::new(app, "Kursal").about(None).separator();

    #[cfg(target_os = "macos")]
    let root_menu = root_menu.services().separator();

    let root_menu = root_menu
        .item(&MenuItem::with_id(
            app,
            "dev_tools",
            "Developer tools",
            true,
            Some("CmdOrControl+Alt+I"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "reload",
            "Reload",
            true,
            Some("CmdOrControl+R"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "restart",
            "Restart",
            true,
            Some("CmdOrControl+Shift+R"),
        )?)
        .hide()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .item(&MenuItem::with_id(
            app,
            "new_contact",
            "New contact",
            true,
            Some("CmdOrControl+N"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "settings",
            "Open settings",
            true,
            Some("CmdOrControl+,"),
        )?)
        .minimize()
        .fullscreen()
        .close_window()
        .build()?;

    //

    let menu = MenuBuilder::new(app)
        .items(&[&root_menu, &edit_menu, &window_menu])
        .build()?;
    app.set_menu(menu)?;

    app.on_menu_event(move |app_handle, event| match event.id().0.as_str() {
        "settings" => {
            use kursal_core::api::{AppEvent, state::AppState};
            use tauri::async_runtime::block_on;

            let state = app_handle.state::<AppState>();
            block_on(async move {
                state
                    .app_event_tx
                    .send(AppEvent::BackendSignal {
                        signal: "open_settings".to_string(),
                        payload: String::with_capacity(0),
                    })
                    .await
                    .ok();
            });
        }
        "new_contact" => {
            use kursal_core::api::{AppEvent, state::AppState};
            use tauri::async_runtime::block_on;

            let state = app_handle.state::<AppState>();
            block_on(async move {
                state
                    .app_event_tx
                    .send(AppEvent::BackendSignal {
                        signal: "new_contact".to_string(),
                        payload: String::with_capacity(0),
                    })
                    .await
                    .ok();
            });
        }
        "dev_tools" => {
            if let Some(window) = app_handle.get_webview_window("main") {
                if window.is_devtools_open() {
                    window.close_devtools();
                } else {
                    window.open_devtools();
                }
            }
        }
        "reload" => {
            if let Some(window) = app_handle.get_webview_window("main") {
                window.reload().ok();
            };
        }
        "restart" => {
            app_handle.restart();
        }
        _ => {
            log::debug!("Unexpected menu event: {:?}", event.id())
        }
    });

    Ok(())
}
