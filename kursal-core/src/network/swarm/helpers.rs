use super::{ConnectionKind, PeerStreams, STREAM_PROTOCOL, SwarmCommand};
use crate::MapKursalResult;
use crate::{KursalError, Result};
use futures::io::AsyncWriteExt;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol, swarm::DialError};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::{mpsc, oneshot};

pub async fn is_peer_connected(cmd_tx: &mpsc::Sender<SwarmCommand>, peer_id: PeerId) -> bool {
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SwarmCommand::IsPeerConnected {
            peer_id,
            reply_tx: tx,
        })
        .await
        .is_err()
    {
        return false;
    }
    rx.await.unwrap_or(false)
}

pub async fn get_connected_peer_count(cmd_tx: &mpsc::Sender<SwarmCommand>) -> usize {
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SwarmCommand::GetConnectedPeerCount { reply_tx: tx })
        .await
        .is_err()
    {
        return 0;
    }
    rx.await.unwrap_or(0)
}

pub async fn get_connected_peers(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Vec<PeerId> {
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SwarmCommand::GetConnectedPeers { reply_tx: tx })
        .await
        .is_err()
    {
        return Vec::new();
    }
    rx.await.unwrap_or_default()
}

pub async fn get_peer_connection_kinds(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> HashMap<PeerId, ConnectionKind> {
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SwarmCommand::GetPeerConnectionKinds { reply_tx: tx })
        .await
        .is_err()
    {
        return HashMap::new();
    }
    rx.await.unwrap_or_default()
}

pub async fn get_all_listen_addrs(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Vec<Multiaddr> {
    let (tx, rx) = oneshot::channel();
    if cmd_tx
        .send(SwarmCommand::GetListenAddresses { reply_tx: tx })
        .await
        .is_err()
    {
        return Vec::new();
    }
    rx.await.unwrap_or_default()
}

pub async fn get_listen_addrs(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Result<Vec<String>> {
    let (tx, rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::GetListenAddresses { reply_tx: tx })
        .await
        .ok_kursal(KursalError::Network)?;

    let all_addresses = rx.await.ok_kursal(KursalError::Network)?;

    let relay_addresses: Vec<String> = all_addresses
        .iter()
        .map(|a| a.to_string())
        .filter(|a| a.contains("/p2p-circuit"))
        .collect();

    Ok(relay_addresses)
}

pub async fn get_nearby_listen_addrs(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Result<Vec<String>> {
    let (tx, rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::GetListenAddresses { reply_tx: tx })
        .await
        .ok_kursal(KursalError::Network)?;

    let all_addresses = rx.await.ok_kursal(KursalError::Network)?;

    let mut out: Vec<String> = Vec::new();
    for addr in all_addresses.iter() {
        let s = addr.to_string();
        if s.contains("/p2p-circuit") {
            out.push(s);
            continue;
        }
        if is_routable_lan(addr) {
            out.push(s);
        }
    }
    Ok(out)
}

fn is_routable_lan(addr: &Multiaddr) -> bool {
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => {
                if ip.is_loopback() || ip.is_unspecified() {
                    return false;
                }
                return true;
            }
            Protocol::Ip6(ip) => {
                if ip.is_loopback() || ip.is_unspecified() {
                    return false;
                }
                return true;
            }
            _ => continue,
        }
    }
    false
}

pub fn str_to_multiaddr(addresses: &[String]) -> Result<Vec<Multiaddr>> {
    addresses
        .iter()
        .map(|el| el.parse::<Multiaddr>().ok_kursal(KursalError::Storage))
        .collect()
}

pub async fn open_peer_stream(
    mut control: libp2p_stream::Control,
    peer_id: PeerId,
    peer_streams: &PeerStreams,
) -> Option<mpsc::Sender<Vec<u8>>> {
    let mut stream = match control.open_stream(peer_id, STREAM_PROTOCOL).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("failed to open stream to {peer_id}: {e}");
            return None;
        }
    };

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);

    tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            let Ok(len) = u32::try_from(data.len()).ok_kursal(KursalError::Storage) else {
                break;
            };

            if stream.write_all(&len.to_be_bytes()).await.is_err() {
                log::warn!("[stream] write to {peer_id} failed, closing writer");
                break;
            }
            if stream.write_all(&data).await.is_err() {
                log::warn!("[stream] write to {peer_id} failed, closing writer");
                break;
            }
        }

        stream.close().await.ok();
        log::info!("[stream] writer to {peer_id} closed");
    });

    peer_streams.lock().unwrap().insert(peer_id, tx.clone());
    Some(tx)
}

pub fn dial_error_summary(error: &DialError) -> String {
    match error {
        DialError::Transport(attempts) => attempts
            .iter()
            .map(|(addr, err)| format!("{} ({})", without_peer_id(addr), innermost_cause(err)))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}

fn without_peer_id(addr: &Multiaddr) -> Multiaddr {
    addr.iter()
        .filter(|proto| !matches!(proto, Protocol::P2p(_)))
        .collect()
}

fn innermost_cause(err: &impl std::fmt::Display) -> String {
    let rendered = err.to_string();
    let mut causes: Vec<&str> = rendered
        .split(['\n', ':'])
        .map(|part| part.trim().trim_start_matches('-').trim())
        .filter(|part| !part.is_empty())
        .collect();
    causes.dedup();
    causes
        .last()
        .map_or_else(|| rendered.clone(), |cause| (*cause).to_string())
}

pub fn is_routable_multiaddr(addr: &Multiaddr) -> bool {
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip)
                if (ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()) =>
            {
                return false;
            }
            Protocol::Ip6(ip) => {
                let is_link_local = (ip.segments()[0] & 0xffc0) == 0xfe80;
                if ip.is_loopback() || ip.is_unspecified() || is_link_local {
                    return false;
                }
                let _ = IpAddr::V6(ip);
            }
            _ => {}
        }
    }
    true
}
