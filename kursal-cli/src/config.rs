use kursal_core::MapKursalResult;
use kursal_core::{KursalError, Result};
use libp2p::Multiaddr;
use serde::Deserialize;
use std::{net::SocketAddr, path::Path};

#[derive(Deserialize)]
pub struct RelayConfigFile {
    pub listen_addr: String,
    pub announce_addr: String,
    pub max_connections: u32,
    pub max_connections_per_ip: Option<u32>,
    pub log_file: Option<String>,
    pub log_level: String,
    pub bootstrap_peers: Vec<String>,
    pub health: HealthConfig,
}

pub struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub announce_addr: AnnounceAddr,
    pub max_connections: u32,
    pub max_connections_per_ip: u32,
    pub log_file: Option<String>,
    pub log_level: String,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub health: HealthConfig,
}

pub struct AnnounceAddr {
    pub host: String,
    pub port: u16,
}

fn parse_announce(raw: &str) -> Result<AnnounceAddr> {
    let (host, port) = raw
        .rsplit_once(':')
        .ok_or_else(|| KursalError::Storage("announce_addr must be host:port".to_string()))?;
    let port = port.parse::<u16>().map_err(|err| {
        KursalError::Storage(format!("Could not parse announce_addr port: {err}"))
    })?;
    Ok(AnnounceAddr {
        host: host.to_string(),
        port,
    })
}

impl RelayConfig {
    pub fn load(path: &Path) -> Result<RelayConfig> {
        let config_content = std::fs::read(path).map_err(KursalError::Io)?;
        let content: RelayConfigFile =
            toml::from_slice(&config_content).ok_kursal(KursalError::Storage)?;

        Ok(RelayConfig {
            listen_addr: content.listen_addr.parse::<SocketAddr>().map_err(|err| {
                KursalError::Storage(format!("Could not parse listen_addr: {err}"))
            })?,
            announce_addr: parse_announce(&content.announce_addr)?,
            max_connections: content.max_connections,
            max_connections_per_ip: content.max_connections_per_ip.unwrap_or(3u32),
            log_file: content.log_file,
            log_level: content.log_level,
            bootstrap_peers: content
                .bootstrap_peers
                .into_iter()
                .map(|el| el.parse::<Multiaddr>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .ok_kursal(KursalError::Storage)?,
            health: content.health,
        })
    }
}

#[derive(Deserialize)]
pub struct HealthConfig {
    pub enabled: bool,
    pub listen_addr: String,
}
