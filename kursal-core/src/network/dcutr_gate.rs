use either::Either;
use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse},
    dcutr,
    multiaddr::Protocol,
    swarm::{
        ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
        THandlerOutEvent, ToSwarm,
        dial_opts::{DialOpts, PeerCondition},
        dummy,
    },
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    task::{Context, Poll},
};

pub struct DcutrGate {
    inner: dcutr::Behaviour,
    allowed: HashSet<PeerId>,
    gated: HashMap<PeerId, HashSet<ConnectionId>>,
    redial: VecDeque<PeerId>,
}

impl DcutrGate {
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            inner: dcutr::Behaviour::new(local_peer_id),
            allowed: HashSet::new(),
            gated: HashMap::new(),
            redial: VecDeque::new(),
        }
    }

    pub fn allow(&mut self, peer_id: PeerId) {
        self.allowed.insert(peer_id);
        if self.gated.remove(&peer_id).is_some() {
            self.redial.push_back(peer_id);
        }
    }

    pub fn disallow(&mut self, peer_id: &PeerId) {
        self.allowed.remove(peer_id);
    }

    fn gate(&mut self, connection_id: ConnectionId, peer: PeerId, addr: &Multiaddr) -> bool {
        let gated = addr.iter().any(|p| p == Protocol::P2pCircuit) && !self.allowed.contains(&peer);
        if gated {
            self.gated.entry(peer).or_default().insert(connection_id);
        }
        gated
    }
}

impl NetworkBehaviour for DcutrGate {
    type ConnectionHandler = THandler<dcutr::Behaviour>;
    type ToSwarm = dcutr::Event;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.inner
            .handle_pending_inbound_connection(connection_id, local_addr, remote_addr)
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.gate(connection_id, peer, local_addr) {
            return Ok(Either::Right(dummy::ConnectionHandler));
        }
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        self.inner.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.gate(connection_id, peer, addr) {
            return Ok(Either::Right(dummy::ConnectionHandler));
        }
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        if let FromSwarm::ConnectionClosed(closed) = &event
            && let Some(connections) = self.gated.get_mut(&closed.peer_id)
        {
            connections.remove(&closed.connection_id);
            if connections.is_empty() {
                self.gated.remove(&closed.peer_id);
            }
        }
        self.inner.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Poll::Ready(event) = self.inner.poll(cx) {
            return Poll::Ready(event);
        }
        match self.redial.pop_front() {
            Some(peer_id) => Poll::Ready(ToSwarm::Dial {
                opts: DialOpts::peer_id(peer_id)
                    .condition(PeerCondition::Always)
                    .build(),
            }),
            None => Poll::Pending,
        }
    }
}
