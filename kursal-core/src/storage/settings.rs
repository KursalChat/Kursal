use super::db::{Database, TABLE_SETTINGS};
use crate::MapKursalResult;
use crate::{KursalError, Result, apiserver::LocalApiConfig};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::{TryRngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};

macro_rules! bool_setting {
    ($get:ident, $set:ident, $key:literal, default: $default:expr) => {
        pub fn $get(db: &Database) -> bool {
            match db.raw_read(TABLE_SETTINGS, $key) {
                Ok(Some(bytes)) => bytes == [1u8],
                _ => $default,
            }
        }
        pub fn $set(db: &Database, value: bool) -> Result<()> {
            db.raw_write(TABLE_SETTINGS, $key, if value { &[1u8] } else { &[0u8] })?;

            Ok(())
        }
    };
}

macro_rules! string_setting {
    ($get:ident, $set:ident, $key:literal, default: $default:literal) => {
        pub fn $get(db: &Database) -> String {
            match db.raw_read(TABLE_SETTINGS, $key) {
                Ok(Some(bytes)) => {
                    String::from_utf8(bytes).unwrap_or_else(|_| $default.to_string())
                }
                _ => $default.to_string(),
            }
        }
        pub fn $set(db: &Database, value: &str) -> Result<()> {
            db.raw_write(TABLE_SETTINGS, $key, value.as_bytes())?;

            Ok(())
        }
    };
}

macro_rules! opt_string_setting {
    ($get:ident, $set:ident, $key:literal) => {
        pub fn $get(db: &Database) -> Option<String> {
            match db.raw_read(TABLE_SETTINGS, $key) {
                Ok(Some(bytes)) => String::from_utf8(bytes).ok(),
                _ => None,
            }
        }
        pub fn $set(db: &Database, value: Option<String>) -> Result<()> {
            if let Some(v) = value {
                db.raw_write(TABLE_SETTINGS, $key, v.as_bytes())?;
            } else {
                db.raw_delete(TABLE_SETTINGS, $key)?;
            }

            Ok(())
        }
    };
}

macro_rules! bincode_setting {
    ($get:ident, $set:ident, $key:literal, $ty:ty, default: $default:expr) => {
        pub fn $get(db: &Database) -> $ty {
            match db.raw_read(TABLE_SETTINGS, $key) {
                Ok(Some(bytes)) => bincode::deserialize::<$ty>(&bytes).unwrap_or_else(|_| $default),
                _ => $default,
            }
        }
        pub fn $set(db: &Database, value: $ty) -> Result<()> {
            let bytes = bincode::serialize(&value)?;
            db.raw_write(TABLE_SETTINGS, $key, &bytes)?;

            Ok(())
        }
    };
}

bool_setting!(
    get_swarm_mdns_enabled,
    set_swarm_mdns_enabled,
    "swarm_mdns_enabled",
    default: true
);
bool_setting!(
    get_updater_enabled,
    set_updater_enabled,
    "updater_enabled",
    default: true
);
bool_setting!(
    get_background_mode,
    set_background_mode,
    "background_mode",
    default: true
);
bool_setting!(
    get_typing_indicators_enabled,
    set_typing_indicators_enabled,
    "typing_indicators_enabled",
    default: true
);
bool_setting!(
    get_read_receipts_enabled,
    set_read_receipts_enabled,
    "read_receipts_enabled",
    default: false
);

pub fn get_contact_muted(db: &Database, contact_id: &str) -> bool {
    matches!(
        db.raw_read(TABLE_SETTINGS, &format!("contact_muted:{contact_id}")),
        Ok(Some(bytes)) if bytes == [1u8]
    )
}

pub fn set_contact_muted(db: &Database, contact_id: &str, value: bool) -> Result<()> {
    let key = format!("contact_muted:{contact_id}");
    if value {
        db.raw_write(TABLE_SETTINGS, &key, &[1u8])?;
    } else {
        db.raw_delete(TABLE_SETTINGS, &key)?;
    }
    Ok(())
}

pub fn get_contact_terminated(db: &Database, contact_id: &str) -> bool {
    matches!(
        db.raw_read(TABLE_SETTINGS, &format!("contact_terminated:{contact_id}")),
        Ok(Some(bytes)) if bytes == [1u8]
    )
}

pub fn set_contact_terminated(db: &Database, contact_id: &str, value: bool) -> Result<()> {
    let key = format!("contact_terminated:{contact_id}");
    if value {
        db.raw_write(TABLE_SETTINGS, &key, &[1u8])?;
    } else {
        db.raw_delete(TABLE_SETTINGS, &key)?;
    }
    Ok(())
}

pub fn get_contact_last_seen(db: &Database, contact_id: &str) -> Option<u64> {
    match db.raw_read(TABLE_SETTINGS, &format!("contact_last_seen:{contact_id}")) {
        Ok(Some(bytes)) => bytes.try_into().ok().map(u64::from_le_bytes),
        _ => None,
    }
}

pub fn set_contact_last_seen(db: &Database, contact_id: &str, ts_ms: u64) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        &format!("contact_last_seen:{contact_id}"),
        &ts_ms.to_le_bytes(),
    )?;
    Ok(())
}

pub fn get_contact_alias(db: &Database, contact_id: &str) -> Option<String> {
    match db.raw_read(TABLE_SETTINGS, &format!("contact_alias:{contact_id}")) {
        Ok(Some(bytes)) => String::from_utf8(bytes).ok().filter(|s| !s.is_empty()),
        _ => None,
    }
}

pub fn set_contact_alias(db: &Database, contact_id: &str, value: Option<String>) -> Result<()> {
    let key = format!("contact_alias:{contact_id}");
    if let Some(v) = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        db.raw_write(TABLE_SETTINGS, &key, v.as_bytes())?;
    } else {
        db.raw_delete(TABLE_SETTINGS, &key)?;
    }
    Ok(())
}

pub fn delete_contact_meta(db: &Database, contact_id: &str) -> Result<()> {
    db.raw_delete(TABLE_SETTINGS, &format!("contact_muted:{contact_id}"))?;
    db.raw_delete(TABLE_SETTINGS, &format!("contact_last_seen:{contact_id}"))?;
    db.raw_delete(TABLE_SETTINGS, &format!("contact_alias:{contact_id}"))?;
    db.raw_delete(TABLE_SETTINGS, &format!("contact_terminated:{contact_id}"))?;
    Ok(())
}

string_setting!(
    get_notification_preview,
    set_notification_preview,
    "notification_preview",
    default: "content"
);
string_setting!(
    get_notification_dnd,
    set_notification_dnd,
    "notification_dnd",
    default: ""
);
string_setting!(
    get_update_channel,
    set_update_channel,
    "update_channel",
    default: "stable"
);

opt_string_setting!(
    get_audio_input_device,
    set_audio_input_device,
    "audio_input_device"
);
opt_string_setting!(
    get_audio_output_device,
    set_audio_output_device,
    "audio_output_device"
);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelayConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub max_connections_per_ip: u32,
}
impl RelayConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAcceptConfig {
    pub mode: String, // "nobody" | "verified" | "all"
    pub size_cap_bytes: u64,
}
impl AutoAcceptConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDownloadConfig {
    pub scope: String, // "per_contact" | "all_contacts"
    pub limit_bytes: u64,
}
impl AutoDownloadConfig {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

bincode_setting!(
    get_relay_config,
    set_relay_config,
    "relay_config",
    RelayConfig,
    default: RelayConfig {
        enabled: false,
        max_connections: 100u32,
        max_connections_per_ip: 10u32,
    }
);
bincode_setting!(
    get_auto_accept_config,
    set_auto_accept_config,
    "auto_accept_config",
    AutoAcceptConfig,
    default: AutoAcceptConfig {
        mode: "nobody".to_string(),
        size_cap_bytes: 2 * 1024 * 1024,
    }
);
bincode_setting!(
    get_auto_download_config,
    set_auto_download_config,
    "auto_download_config",
    AutoDownloadConfig,
    default: AutoDownloadConfig {
        scope: "all_contacts".to_string(),
        limit_bytes: 100 * 1024 * 1024,
    }
);
bincode_setting!(
    get_custom_nodes,
    set_custom_nodes,
    "custom_nodes",
    Vec<String>,
    default: Vec::new()
);

pub fn get_peer_rotation_interval(db: &Database) -> u64 {
    let default = 30 * 60 * 60;

    match db.raw_read(TABLE_SETTINGS, "peer_rotation_interval_secs") {
        Ok(Some(bytes)) => bytes.try_into().map(u64::from_be_bytes).unwrap_or(default),
        _ => default,
    }
}
pub fn set_peer_rotation_interval(db: &Database, value: u64) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "peer_rotation_interval_secs",
        &value.to_be_bytes(),
    )?;

    Ok(())
}

pub fn get_swarm_listening_port(db: &Database) -> Option<u16> {
    if cfg!(debug_assertions) {
        return None;
    }

    db.raw_read(TABLE_SETTINGS, "swarm_listening_port")
        .ok()
        .flatten()
        .and_then(|b| b.try_into().ok().map(u16::from_be_bytes))
}
pub fn set_swarm_listening_port(db: &Database, port: Option<u16>) -> Result<()> {
    if let Some(port) = port {
        db.raw_write(TABLE_SETTINGS, "swarm_listening_port", &port.to_be_bytes())?;
    } else {
        db.raw_delete(TABLE_SETTINGS, "swarm_listening_port")?;
    }

    Ok(())
}

pub fn get_ui_state(db: &Database, key: &str) -> Result<Option<String>> {
    Ok(db
        .raw_read(TABLE_SETTINGS, &format!("ui_state_{key}"))?
        .and_then(|bytes| String::from_utf8(bytes).ok()))
}
pub fn set_ui_state(db: &Database, key: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        db.raw_delete(TABLE_SETTINGS, &format!("ui_state_{key}"))?;
    } else {
        db.raw_write(TABLE_SETTINGS, &format!("ui_state_{key}"), value.as_bytes())?;
    }

    Ok(())
}

pub fn valid_call_sample_rate(rate: u32) -> u32 {
    match rate {
        16000 | 24000 | 48000 => rate,
        _ => 48000,
    }
}

pub fn get_call_sample_rate(db: &Database) -> u32 {
    match db.raw_read(TABLE_SETTINGS, "call_sample_rate") {
        Ok(Some(bytes)) => match <[u8; 4]>::try_from(bytes.as_slice()) {
            Ok(arr) => valid_call_sample_rate(u32::from_be_bytes(arr)),
            Err(_) => 48000,
        },
        _ => 48000,
    }
}

pub fn set_call_sample_rate(db: &Database, rate: u32) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "call_sample_rate",
        &valid_call_sample_rate(rate).to_be_bytes(),
    )?;

    Ok(())
}

pub fn valid_video_quality(q: u32) -> u32 {
    match q {
        360 | 480 | 720 => q,
        _ => 480,
    }
}

pub fn get_video_quality(db: &Database) -> u32 {
    match db.raw_read(TABLE_SETTINGS, "video_quality") {
        Ok(Some(bytes)) => match <[u8; 4]>::try_from(bytes.as_slice()) {
            Ok(arr) => valid_video_quality(u32::from_be_bytes(arr)),
            Err(_) => 480,
        },
        _ => 480,
    }
}

pub fn set_video_quality(db: &Database, quality: u32) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "video_quality",
        &valid_video_quality(quality).to_be_bytes(),
    )?;

    Ok(())
}

pub fn get_local_profile(db: &Database) -> (String, Option<Vec<u8>>) {
    let default_username = "You".to_string();
    let username = match db.raw_read(TABLE_SETTINGS, "local_profile_username") {
        Ok(Some(bytes)) => std::str::from_utf8(&bytes)
            .map(|v| v.to_string())
            .unwrap_or(default_username),
        _ => default_username,
    };

    let avatar = match db.raw_read(TABLE_SETTINGS, "local_profile_avatar") {
        Ok(Some(bytes)) if !bytes.is_empty() => Some(bytes),
        _ => None,
    };

    (username, avatar)
}
pub fn set_local_profile(
    db: &Database,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "local_profile_username",
        display_name.as_bytes(),
    )?;

    db.raw_write(
        TABLE_SETTINGS,
        "local_profile_avatar",
        &avatar_bytes.unwrap_or(Vec::with_capacity(0)),
    )?;

    Ok(())
}

pub fn api_server_password(db: &Database) -> Result<String> {
    match db.raw_read(TABLE_SETTINGS, "api_server_password")? {
        None => Err(KursalError::Storage("Entry not found".to_string())),
        Some(bytes) => String::from_utf8(bytes).ok_kursal(KursalError::Storage),
    }
}
pub fn set_new_api_server_password(db: &Database) -> Result<String> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .ok_kursal(KursalError::Crypto)?;

    let token = hex::encode(bytes);

    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(token.as_bytes(), &salt)
        .ok_kursal(KursalError::Crypto)?
        .to_string();

    db.raw_write(
        TABLE_SETTINGS,
        "api_server_password",
        password_hash.as_bytes(),
    )?;

    Ok(token)
}

pub fn api_server_config(db: &Database) -> Result<LocalApiConfig> {
    match db.raw_read(TABLE_SETTINGS, "api_server_config")? {
        None => Ok(LocalApiConfig::default()),
        Some(bytes) => LocalApiConfig::deserialize(&bytes),
    }
}
pub fn set_api_server_config(db: &Database, config: LocalApiConfig) -> Result<()> {
    db.raw_write(
        TABLE_SETTINGS,
        "api_server_config",
        &config.serialize().ok_kursal(KursalError::Storage)?,
    )?;

    Ok(())
}

// ooh spooky
pub const RESET_FULL_APP: &str = "SET_THIS_TAG_TO_RESET_THE_FULL_APP yeah its dangerous";
pub fn reset_full_app(db: &Database) -> Result<()> {
    db.raw_write(TABLE_SETTINGS, RESET_FULL_APP, RESET_FULL_APP.as_bytes())?;
    Ok(())
}
pub fn should_reset_full_app(db: &Database) -> bool {
    let val = db.raw_read(TABLE_SETTINGS, RESET_FULL_APP);

    match val {
        Ok(Some(bytes)) => bytes == RESET_FULL_APP.as_bytes(),
        _ => false,
    }
}
