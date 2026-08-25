use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::identity::keychain::KeychainConfig;

#[cfg(feature = "apiserver")]
mod apiserver;
mod bootstrap;
#[cfg(feature = "calls")]
mod call;
#[cfg(feature = "calls")]
mod capture;
mod contact;
mod crypto;
mod dht;
mod identity;
mod image_metadata;
mod ltc;
mod messaging;
mod nearby;
mod offline;
mod otp;
mod relay_discovery;
mod stats;
mod storage;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct TestEnv {
    dir: PathBuf,
}

impl TestEnv {
    pub fn new() -> Self {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("kursal-test-{}-{seq}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    pub fn db_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.db"))
    }

    pub fn data_dir(&self) -> &Path {
        &self.dir
    }

    pub fn keychain_config(&self) -> KeychainConfig {
        KeychainConfig {
            storage_id: "test".to_string(),
            unsafe_write_key_to_file: true,
        }
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
