use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    dto::{NetworkStatusDto, NodesResponse},
    network::{
        bootstrap::default_node_strings,
        swarm::{SwarmCommand, get_all_listen_addrs, get_connected_peers, is_routable_multiaddr},
    },
    storage::{Database, SharedDatabase, get_custom_nodes, set_custom_nodes},
};
use libp2p::Multiaddr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

const DIAL_TIMEOUT_SECS: u64 = 15;

pub fn list_nodes(db: &Database) -> NodesResponse {
    NodesResponse {
        defaults: default_node_strings(),
        custom: get_custom_nodes(db),
    }
}

fn validate_node_addr(addr: &str) -> Result<Multiaddr> {
    let parsed = addr
        .parse::<Multiaddr>()
        .map_err(|_| KursalError::Network("invalid_address".to_string()))?;
    if !is_routable_multiaddr(&parsed) {
        return Err(KursalError::Network("unroutable_address".to_string()));
    }
    Ok(parsed)
}

pub async fn add_custom_node(
    addr: String,
    db: SharedDatabase,
    cmd_tx: mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let parsed = validate_node_addr(&addr)?;
    let normalized = parsed.to_string();

    {
        let guard = &*db;
        let mut nodes = get_custom_nodes(guard);
        let is_default = default_node_strings().contains(&normalized);
        if is_default || nodes.contains(&normalized) {
            return Err(KursalError::Network("duplicate_node".to_string()));
        }
        nodes.push(normalized);
        set_custom_nodes(guard, nodes)?;
    }

    let _ = cmd_tx.send(SwarmCommand::AddNode(parsed)).await;
    Ok(())
}

pub async fn remove_custom_node(addr: String, db: SharedDatabase) -> Result<()> {
    let guard = &*db;
    let mut nodes = get_custom_nodes(guard);
    let before = nodes.len();
    nodes.retain(|n| *n != addr);
    if nodes.len() != before {
        set_custom_nodes(guard, nodes)?;
    }
    Ok(())
}

pub async fn network_status(cmd_tx: mpsc::Sender<SwarmCommand>) -> Result<NetworkStatusDto> {
    let peers = get_connected_peers(&cmd_tx).await;
    let addrs = get_all_listen_addrs(&cmd_tx).await;
    Ok(NetworkStatusDto {
        peer_count: peers.len(),
        connected_peers: peers.iter().map(|p| p.to_string()).collect(),
        listen_addresses: addrs.iter().map(|a| a.to_string()).collect(),
    })
}

pub async fn dial_address(addr: String, cmd_tx: mpsc::Sender<SwarmCommand>) -> Result<()> {
    let parsed = validate_node_addr(&addr)?;
    let (reply_tx, reply_rx) = oneshot::channel();

    cmd_tx
        .send(SwarmCommand::DialOnce {
            addr: parsed,
            reply_tx,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    match tokio::time::timeout(Duration::from_secs(DIAL_TIMEOUT_SECS), reply_rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(reason))) => Err(KursalError::Network(reason)),
        Ok(Err(_)) => Err(KursalError::Network("dial_cancelled".to_string())),
        Err(_) => Err(KursalError::Network("dial_timeout".to_string())),
    }
}
