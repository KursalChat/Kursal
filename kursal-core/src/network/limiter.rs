use std::{collections::HashMap, convert::Infallible, net::IpAddr, task::Poll};

use libp2p::{
    Multiaddr, PeerId,
    multiaddr::Protocol,
    swarm::{ConnectionDenied, FromSwarm, NetworkBehaviour, dummy},
};

pub struct ConnectionLimiter {
    pub max_total: u32,
    pub max_per_ip: u32,
    by_ip: HashMap<IpAddr, u32>,
    by_peer: HashMap<PeerId, u32>,
    total: u32,
}

impl ConnectionLimiter {
    pub fn new(max_total: u32, max_per_ip: u32) -> Self {
        Self {
            max_per_ip,
            max_total,
            total: 0,
            by_ip: HashMap::new(),
            by_peer: HashMap::new(),
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
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>>
    {
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
