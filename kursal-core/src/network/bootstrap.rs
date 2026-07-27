use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct BootstrapEntry {
    addr: String,
}

const EMBEDDED: &str = include_str!("bootstrap.json");
const ENV_VAR: &str = "KURSAL_BOOTSTRAP";

static DEFAULT_STRINGS: LazyLock<Vec<String>> = LazyLock::new(build_default_strings);
static DEFAULT_MULTIADDRS: LazyLock<Vec<Multiaddr>> = LazyLock::new(|| {
    DEFAULT_STRINGS
        .iter()
        .filter_map(|addr| addr.parse::<Multiaddr>().ok())
        .collect()
});

fn build_default_strings() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    match serde_json::from_str::<Vec<BootstrapEntry>>(EMBEDDED) {
        Ok(entries) => {
            for entry in entries {
                push_unique(&mut out, entry.addr);
            }
        }
        Err(err) => log::error!("[bootstrap] embedded bootstrap.json is invalid: {err}"),
    }

    if let Ok(raw) = std::env::var(ENV_VAR) {
        for part in raw.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                push_unique(&mut out, trimmed.to_string());
            }
        }
    }

    out
}

fn push_unique(out: &mut Vec<String>, addr: String) {
    if !out.contains(&addr) {
        out.push(addr);
    }
}

pub fn default_node_strings() -> Vec<String> {
    DEFAULT_STRINGS.clone()
}

pub fn bootstrap_peers() -> Vec<Multiaddr> {
    DEFAULT_MULTIADDRS.clone()
}

pub fn is_bootstrap_peer(peer_id: &PeerId) -> bool {
    DEFAULT_MULTIADDRS
        .iter()
        .any(|addr| matches!(addr.iter().last(), Some(Protocol::P2p(id)) if &id == peer_id))
}
