use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    net::IpAddr,
    task::{Poll, Waker},
};

use libp2p::{
    Multiaddr, PeerId,
    multiaddr::Protocol,
    swarm::{
        CloseConnection, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, ToSwarm,
        dummy,
    },
};

pub const MAX_TRANSIENT_CONNECTIONS: usize = 32;

pub struct ConnectionLimiter {
    pub max_total: u32,
    pub max_per_ip: u32,
    by_ip: HashMap<IpAddr, u32>,
    by_peer: HashMap<PeerId, u32>,
    total: u32,
    transient_cap: Option<usize>,
    protected: HashSet<PeerId>,
    transient: VecDeque<(ConnectionId, PeerId)>,
    pending: VecDeque<ToSwarm<Infallible, Infallible>>,
    waker: Option<Waker>,
}

impl ConnectionLimiter {
    pub fn new(max_total: u32, max_per_ip: u32, transient_cap: Option<usize>) -> Self {
        Self {
            max_per_ip,
            max_total,
            total: 0,
            by_ip: HashMap::new(),
            by_peer: HashMap::new(),
            transient_cap,
            protected: HashSet::new(),
            transient: VecDeque::new(),
            pending: VecDeque::new(),
            waker: None,
        }
    }

    pub fn protect(&mut self, peer_id: PeerId) {
        if self.protected.insert(peer_id) {
            self.transient.retain(|(_, peer)| *peer != peer_id);
        }
    }

    pub fn unprotect(&mut self, peer_id: &PeerId) {
        self.protected.remove(peer_id);
    }

    fn track_transient(&mut self, connection_id: ConnectionId, peer_id: PeerId) {
        let Some(cap) = self.transient_cap else {
            return;
        };
        if self.protected.contains(&peer_id) {
            return;
        }

        self.transient.push_back((connection_id, peer_id));

        while self.transient.len() > cap {
            let Some((evicted, peer)) = self.transient.pop_front() else {
                break;
            };
            log::debug!("[conn] transient cap {cap} reached: evicting {evicted:?} to {peer}");
            self.pending.push_back(ToSwarm::CloseConnection {
                peer_id: peer,
                connection: CloseConnection::One(evicted),
            });
        }

        if !self.pending.is_empty()
            && let Some(waker) = self.waker.take()
        {
            waker.wake();
        }
    }
}

impl NetworkBehaviour for ConnectionLimiter {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = Infallible;

    fn handle_pending_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        if is_relayed(remote_addr) {
            return Ok(());
        }

        let Some(ip) = extract_ip(remote_addr) else {
            return Ok(());
        };

        if self.max_total != 0 && self.total >= self.max_total {
            log::warn!("connection denied: total limit {} reached", self.max_total);
            return Err(ConnectionDenied::new(
                "connection limit reached for this relay",
            ));
        }

        if self.max_per_ip != 0 && self.by_ip.get(&ip).copied().unwrap_or(0) >= self.max_per_ip {
            log::warn!(
                "connection denied: per-ip limit {} reached",
                self.max_per_ip
            );
            return Err(ConnectionDenied::new(
                "connection limit reached for this relay",
            ));
        }

        Ok(())
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(e) => {
                self.track_transient(e.connection_id, e.peer_id);

                if !e.endpoint.is_listener() {
                    return;
                }
                let addr = e.endpoint.get_remote_address();
                if is_relayed(addr) {
                    *self.by_peer.entry(e.peer_id).or_insert(0) += 1;
                    return;
                }

                self.total += 1;

                if let Some(ip) = extract_ip(addr) {
                    *self.by_ip.entry(ip).or_insert(0) += 1;
                }
            }
            FromSwarm::ConnectionClosed(e) => {
                self.transient.retain(|(cid, _)| *cid != e.connection_id);

                if !e.endpoint.is_listener() {
                    return;
                }
                let addr = e.endpoint.get_remote_address();
                if is_relayed(addr) {
                    if let Some(count) = self.by_peer.get_mut(&e.peer_id) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            self.by_peer.remove(&e.peer_id);
                        }
                    }
                    return;
                }

                self.total = self.total.saturating_sub(1);

                if let Some(ip) = extract_ip(addr)
                    && let Some(count) = self.by_ip.get_mut(&ip)
                {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        self.by_ip.remove(&ip);
                    }
                }
            }

            _ => {}
        }
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: libp2p::PeerId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        if is_relayed(remote_addr)
            && self.max_per_ip != 0
            && self.by_peer.get(&peer).copied().unwrap_or(0) >= self.max_per_ip
        {
            log::warn!(
                "connection denied: per-peer relay limit {} reached",
                self.max_per_ip
            );
            return Err(ConnectionDenied::new(
                "relay connection limit reached for this peer",
            ));
        }

        Ok(dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        _peer: libp2p::PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
        _port_use: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: libp2p::PeerId,
        _connection_id: libp2p::swarm::ConnectionId,
        _event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>>
    {
        if let Some(action) = self.pending.pop_front() {
            return Poll::Ready(action);
        }

        self.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

fn is_relayed(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::P2pCircuit))
}

pub fn extract_ip(addr: &Multiaddr) -> Option<IpAddr> {
    for part in addr.iter() {
        if let Protocol::Ip4(ip) = part {
            return Some(IpAddr::V4(ip));
        }
        if let Protocol::Ip6(ip) = part {
            return Some(IpAddr::V6(ip));
        }
    }

    None
}
