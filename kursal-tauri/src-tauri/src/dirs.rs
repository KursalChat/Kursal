use kursal_core::MapKursalResult;
use kursal_core::{KursalError, Result};
use std::{path::PathBuf, sync::OnceLock};
use tauri::Manager;

pub static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
pub static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
pub static LOGS_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_dirs(app: &tauri::App) -> Result<()> {
    let path = app.path();

    let app_data_dir = path.app_data_dir().ok_kursal(KursalError::Storage)?;
    std::fs::create_dir_all(&app_data_dir).unwrap();

    APP_DATA_DIR
        .set(app_data_dir)
        .map_err(|_| KursalError::Storage("APP_DATA_DIR already initialized".to_string()))?;

    //

    let cache_dir = path.app_cache_dir().ok_kursal(KursalError::Storage)?;
    std::fs::create_dir_all(&cache_dir).unwrap();

    CACHE_DIR
        .set(cache_dir)
        .map_err(|_| KursalError::Storage("CACHE_DIR already initialized".to_string()))?;
    //

    let logs_dir = path.app_log_dir().ok_kursal(KursalError::Storage)?;
    std::fs::create_dir_all(&logs_dir).unwrap();

    LOGS_DIR
        .set(logs_dir)
        .map_err(|_| KursalError::Storage("LOGS_DIR already initialized".to_string()))?;

    Ok(())
}

pub fn app_data_dir() -> Result<&'static PathBuf> {
    APP_DATA_DIR.get().ok_or(KursalError::Storage(
        "APP_DATA_DIR not initialized".to_string(),
    ))
}

pub fn cache_dir() -> Result<&'static PathBuf> {
    CACHE_DIR.get().ok_or(KursalError::Storage(
        "CACHE_DIR not initialized".to_string(),
    ))
}

pub fn logs_dir() -> Result<&'static PathBuf> {
    LOGS_DIR
        .get()
        .ok_or(KursalError::Storage("LOGS_DIR not initialized".to_string()))
}
