use crate::dialog_bridge::{DialogRequest, ask};
use kursal_core::{
    Result, api::state::AppState, first_contact::ltc::LtcPayload, storage::file::KursalFile,
};
use serde_json::json;
use tauri::{AppHandle, Manager, async_runtime::block_on};

pub trait FileLoader {
    fn load(&self, handle: &AppHandle) -> impl std::future::Future<Output = Result<()>>;
}

impl FileLoader for KursalFile {
    async fn load(&self, handle: &AppHandle) -> Result<()> {
        let state = handle.state::<AppState>();

        match self {
            KursalFile::LtcPayload(bytes) => {
                let result = LtcPayload::deserialize(bytes);

                let _result = match result {
                    Ok(payload) => {
                        payload
                            .import_ltc(state.db.clone(), &*state.network.lock().await)
                            .await
                    }
                    Err(e) => Err(e),
                }?;
            }
            KursalFile::Backup(_) => unreachable!(),
        }

        Ok(())
    }
}

pub fn open_files(app: &AppHandle, files: Vec<(String, String)>) {
    let app = app.clone();

    std::thread::spawn(move || {
        let local = tokio::task::LocalSet::new();

        block_on(local.run_until(async move {
            for (path, file_name) in files {
                if let Ok(content) = std::fs::read(&path)
                    && let Ok(kursal_file) = KursalFile::deserialize(&content)
                {
                    match kursal_file.get_warning() {
                        Err(message) => {
                            ask(
                                &app,
                                DialogRequest::alert(
                                    "file_open_blocked",
                                    json!({ "fileName": file_name }),
                                    "danger",
                                )
                                .with_message(message),
                            )
                            .await;
                        }
                        Ok(message) => {
                            let do_open = ask(
                                &app,
                                DialogRequest::confirm(
                                    "file_open_confirm",
                                    json!({ "fileName": file_name }),
                                    "warning",
                                )
                                .with_message(message),
                            )
                            .await;

                            if do_open && let Err(err) = FileLoader::load(&kursal_file, &app).await
                            {
                                log::error!("Failed to open file {file_name}: {err}");
                            }
                        }
                    }
                }
            }
        }));
    });
}
