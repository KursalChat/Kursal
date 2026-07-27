use crate::{
    api::{AppEvent, CoreCommand},
    identity::keychain::KeychainConfig,
    network::NetworkManager,
    storage::{Database, SharedDatabase},
};
use std::sync::Mutex as StdMutex;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, MutexGuard, mpsc, oneshot};

pub struct AppState {
    pub db: SharedDatabase,
    pub network: Arc<Mutex<NetworkManager>>,
    pub app_event_tx: mpsc::Sender<AppEvent>,
    pub core_cmd_tx: mpsc::Sender<CoreCommand>,
    pub pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    pub db_path: PathBuf,
    pub deep_links: StdMutex<DeepLinkQueue>,
    pub keychain_config: KeychainConfig,
}

impl AppState {
    pub async fn db(&self) -> MutexGuard<'_, Database> {
        self.db.0.lock().await
    }
}

#[derive(Debug)]
pub struct DeepLink {
    pub category: Option<String>,
    pub path: String,
}

#[derive(Debug)]
pub struct DeepLinkQueue {
    pub frontend_ready: bool,
    pub pending: Vec<DeepLink>,
}
