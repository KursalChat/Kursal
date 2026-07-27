use crate::network::bootstrap::{bootstrap_peers, default_node_strings};
use libp2p::multiaddr::Protocol;

#[test]
fn embedded_bootstrap_is_valid() {
    let raw = default_node_strings();
    assert!(
        !raw.is_empty(),
        "bootstrap.json must list at least one peer"
    );

    let parsed = bootstrap_peers();
    assert_eq!(
        raw.len(),
        parsed.len(),
        "every bootstrap address must be a valid multiaddr"
    );

    for addr in parsed {
        assert!(
            matches!(addr.iter().last(), Some(Protocol::P2p(_))),
            "bootstrap address must end with /p2p/<peer-id>: {addr}"
        );
    }
}
