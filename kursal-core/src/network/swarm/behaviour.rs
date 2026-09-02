use crate::network::{kademlia::KursalKadStore, limiter::ConnectionLimiter};
use libp2p::{
    request_response,
    swarm::{NetworkBehaviour, behaviour::toggle::Toggle},
};
use std::convert::Infallible;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "KursalBehaviourEvent")]
pub struct KursalBehaviour {
    pub relay: libp2p::relay::client::Behaviour,
    pub relay_server: Toggle<libp2p::relay::Behaviour>,
    pub dcutr: libp2p::dcutr::Behaviour,
    pub autonat_client: libp2p::autonat::v2::client::Behaviour,
    pub autonat_server: Toggle<libp2p::autonat::v2::server::Behaviour>,
    pub kad: libp2p::kad::Behaviour<KursalKadStore>,
    pub mdns: Toggle<libp2p::mdns::tokio::Behaviour>,
    pub identify: libp2p::identify::Behaviour,
    pub request_response: request_response::Behaviour<super::KursalMsgCodec>,
    pub streaming: libp2p_stream::Behaviour,
    pub limiter: ConnectionLimiter,
    pub ping: libp2p::ping::Behaviour,
}

#[allow(clippy::large_enum_variant)]
pub enum KursalBehaviourEvent {
    Relay(libp2p::relay::client::Event),
    RelayServer(libp2p::relay::Event),
    Dcutr(libp2p::dcutr::Event),
    AutonatClient(libp2p::autonat::v2::client::Event),
    AutonatServer(libp2p::autonat::v2::server::Event),
    Kad(libp2p::kad::Event),
    Mdns(libp2p::mdns::Event),
    Identify(libp2p::identify::Event),
    RequestResponse(request_response::Event<Vec<u8>, Vec<u8>>),
    Limiter(Infallible),
    Ping(libp2p::ping::Event),
}

impl From<libp2p::relay::client::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::client::Event) -> Self {
        Self::Relay(value)
    }
}
impl From<libp2p::relay::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::relay::Event) -> Self {
        Self::RelayServer(value)
    }
}
impl From<libp2p::dcutr::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::dcutr::Event) -> Self {
        Self::Dcutr(value)
    }
}
impl From<libp2p::autonat::v2::client::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::autonat::v2::client::Event) -> Self {
        Self::AutonatClient(value)
    }
}
impl From<libp2p::autonat::v2::server::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::autonat::v2::server::Event) -> Self {
        Self::AutonatServer(value)
    }
}
impl From<libp2p::ping::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::ping::Event) -> Self {
        Self::Ping(value)
    }
}
impl From<libp2p::kad::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::kad::Event) -> Self {
        Self::Kad(value)
    }
}
impl From<libp2p::mdns::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::mdns::Event) -> Self {
        Self::Mdns(value)
    }
}
impl From<libp2p::identify::Event> for KursalBehaviourEvent {
    fn from(value: libp2p::identify::Event) -> Self {
        Self::Identify(value)
    }
}
impl From<Infallible> for KursalBehaviourEvent {
    fn from(value: Infallible) -> Self {
        Self::Limiter(value)
    }
}

impl From<request_response::Event<Vec<u8>, Vec<u8>>> for KursalBehaviourEvent {
    fn from(value: request_response::Event<Vec<u8>, Vec<u8>>) -> Self {
        Self::RequestResponse(value)
    }
}
impl From<()> for KursalBehaviourEvent {
    fn from(_: ()) -> Self {
        unreachable!()
    }
}
