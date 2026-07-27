use crate::stats::parse_bandwidth;

#[test]
fn parses_directions_and_sums_across_protocols() {
    let text = concat!(
        "# HELP libp2p_bandwidth_bytes Bandwidth usage by direction and transport protocols\n",
        "# TYPE libp2p_bandwidth_bytes counter\n",
        "# UNIT libp2p_bandwidth_bytes bytes\n",
        "libp2p_bandwidth_bytes_total{protocols=\"/ip4/tcp\",direction=\"Inbound\"} 100\n",
        "libp2p_bandwidth_bytes_total{protocols=\"/ip4/tcp\",direction=\"Outbound\"} 40\n",
        "libp2p_bandwidth_bytes_total{protocols=\"/ip4/udp/quic-v1\",direction=\"Inbound\"} 23\n",
        "# EOF\n",
    );
    assert_eq!(parse_bandwidth(text), (123, 40));
}

#[test]
fn ignores_unrelated_metrics_and_garbage() {
    let text = concat!(
        "libp2p_swarm_connections_total{direction=\"Inbound\"} 5\n",
        "libp2p_bandwidth_bytes_total{protocols=\"/ip4/tcp\",direction=\"Inbound\"} nope\n",
        "random line\n",
    );
    assert_eq!(parse_bandwidth(text), (0, 0));
}

#[test]
fn empty_input_is_zero() {
    assert_eq!(parse_bandwidth(""), (0, 0));
}
