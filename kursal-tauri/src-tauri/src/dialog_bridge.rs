use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

static PENDING: LazyLock<Mutex<HashMap<u64, oneshot::Sender<bool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize, Clone)]
pub struct DialogRequest {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub params: Value,
    pub tone: String,
    pub dismissible: bool,
}

impl DialogRequest {
    pub fn confirm(kind: &str, params: Value, tone: &str) -> Self {
        Self {
            kind: kind.to_string(),
            message: None,
            params,
            tone: tone.to_string(),
            dismissible: true,
        }
    }

    pub fn alert(kind: &str, params: Value, tone: &str) -> Self {
        Self {
            kind: kind.to_string(),
            message: None,
            params,
            tone: tone.to_string(),
            dismissible: false,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Serialize, Clone)]
struct DialogEvent {
    id: u64,
    #[serde(flatten)]
    request: DialogRequest,
}

pub async fn ask(app: &AppHandle, request: DialogRequest) -> bool {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();

    if let Ok(mut pending) = PENDING.lock() {
        pending.insert(id, tx);
    }

    if app
        .emit("backend_dialog", DialogEvent { id, request })
        .is_err()
    {
        if let Ok(mut pending) = PENDING.lock() {
            pending.remove(&id);
        }
        return false;
    }

    rx.await.unwrap_or(false)
}

#[tauri::command]
pub fn dialog_respond(id: u64, confirmed: bool) {
    if let Ok(mut pending) = PENDING.lock()
        && let Some(tx) = pending.remove(&id)
    {
        let _ = tx.send(confirmed);
    }
}

#[tauri::command]
pub async fn run_startup_dialogs(app: tauri::AppHandle) {
    crate::error::check_pending_crash(&app).await;
}
