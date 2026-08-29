use crate::network::swarm::{
    MAX_RELAY_CANDIDATES, MAX_RELAY_RESERVATIONS, RelayCandidate, any_public_address,
    best_relay_candidates, is_circuit, is_routable_multiaddr, prune_relay_candidates,
    relay_provider_key,
};
use libp2p::{Multiaddr, PeerId};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

fn counts_as_public(addr: &str) -> bool {
    let addr: Multiaddr = addr.parse().unwrap();
    !is_circuit(&addr) && is_routable_multiaddr(&addr)
}

fn public(list: &[&str]) -> bool {
    let parsed: Vec<Multiaddr> = list.iter().map(|a| a.parse().unwrap()).collect();
    any_public_address(parsed.iter())
}

const RELAY: &str = "12D3KooW9sfuTYevisKu5JV9LWrMmaSYu8SnZnG8vEPPW7UFrXJX";
const SELF: &str = "12D3KooWRFsA8bHdCHaMTantbKELf3R4N3acABYjP8b27bxFiY6X";

#[test]
fn a_relayed_address_is_not_evidence_of_reachability() {
    // libp2p-relay confirms this the moment a reservation is accepted, and kad
    // would otherwise read it as "we are publicly reachable".
    let relayed = format!("/dns4/diffie.kursal.chat/tcp/4891/p2p/{RELAY}/p2p-circuit/p2p/{SELF}");
    assert!(!counts_as_public(&relayed));
}

#[test]
fn a_plain_public_address_is_evidence_of_reachability() {
    assert!(counts_as_public("/ip4/203.0.113.7/tcp/4891"));
    assert!(counts_as_public("/dns4/relay.example/udp/4891/quic-v1"));
}

#[test]
fn loopback_and_unspecified_are_not_evidence() {
    assert!(!counts_as_public("/ip4/127.0.0.1/tcp/4891"));
    assert!(!counts_as_public("/ip4/0.0.0.0/tcp/4891"));
}

#[test]
fn a_later_reservation_does_not_mask_a_still_valid_direct_address() {
    let relayed = format!("/dns4/diffie.kursal.chat/tcp/4891/p2p/{RELAY}/p2p-circuit/p2p/{SELF}");
    assert!(
        public(&["/ip4/203.0.113.7/tcp/4891", &relayed]),
        "a reservation must not demote a node that is still directly reachable"
    );
}

#[test]
fn reservations_alone_are_never_public() {
    let relayed = format!("/dns4/diffie.kursal.chat/tcp/4891/p2p/{RELAY}/p2p-circuit/p2p/{SELF}");
    assert!(!public(&[&relayed, "/ip4/127.0.0.1/tcp/4891"]));
    assert!(!public(&[]));
}

fn addrs(list: &[&str]) -> HashSet<Multiaddr> {
    list.iter().map(|a| a.parse().unwrap()).collect()
}

fn reserved(set: &HashSet<Multiaddr>) -> usize {
    set.iter()
        .filter(|a| {
            a.iter()
                .any(|p| matches!(p, libp2p::multiaddr::Protocol::P2pCircuit))
        })
        .count()
}

#[test]
fn provider_key_is_stable_across_calls() {
    assert_eq!(relay_provider_key(), relay_provider_key());
}

#[test]
fn provider_key_is_a_full_sha256() {
    assert_eq!(relay_provider_key().to_vec().len(), 32);
}

#[test]
fn only_circuit_addresses_count_as_reservations() {
    let set = addrs(&[
        "/ip4/1.2.3.4/tcp/4891",
        "/ip4/1.2.3.4/udp/4891/quic-v1",
        "/ip4/5.6.7.8/tcp/4891/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp/p2p-circuit",
    ]);
    assert_eq!(reserved(&set), 1);
}

#[test]
fn reservation_cap_leaves_room_for_more_than_one_relay() {
    const {
        assert!(
            MAX_RELAY_RESERVATIONS >= 2,
            "a single reservation is the centralisation this exists to avoid"
        )
    };
}

#[test]
fn an_empty_listen_set_holds_no_reservations() {
    assert_eq!(reserved(&HashSet::new()), 0);
}

fn candidate(ms: Option<u64>, failures: u32) -> RelayCandidate {
    RelayCandidate {
        rtt: ms.map(Duration::from_millis),
        failures,
    }
}

#[test]
fn lower_latency_relays_rank_first() {
    let (fast, slow) = (PeerId::random(), PeerId::random());
    let mut map = HashMap::new();
    map.insert(slow, candidate(Some(400), 0));
    map.insert(fast, candidate(Some(20), 0));

    assert_eq!(best_relay_candidates(&map, 2), vec![fast, slow]);
}

#[test]
fn unmeasured_relays_rank_after_measured_ones() {
    let (measured, unknown) = (PeerId::random(), PeerId::random());
    let mut map = HashMap::new();
    map.insert(unknown, candidate(None, 0));
    map.insert(measured, candidate(Some(900), 0));

    assert_eq!(best_relay_candidates(&map, 1), vec![measured]);
}

#[test]
fn failures_outweigh_a_fast_ping() {
    let (flaky, steady) = (PeerId::random(), PeerId::random());
    let mut map = HashMap::new();
    map.insert(flaky, candidate(Some(5), 2));
    map.insert(steady, candidate(Some(300), 0));

    assert_eq!(best_relay_candidates(&map, 1), vec![steady]);
}

#[test]
fn ranking_is_stable_for_identical_candidates() {
    let mut map = HashMap::new();
    for _ in 0..8 {
        map.insert(PeerId::random(), candidate(Some(50), 0));
    }
    assert_eq!(
        best_relay_candidates(&map, 4),
        best_relay_candidates(&map, 4)
    );
}

#[test]
fn pruning_keeps_the_roster_bounded_and_drops_the_worst() {
    let keeper = PeerId::random();
    let mut map = HashMap::new();
    map.insert(keeper, candidate(Some(1), 0));
    for _ in 0..(MAX_RELAY_CANDIDATES * 2) {
        map.insert(PeerId::random(), candidate(None, 9));
    }

    prune_relay_candidates(&mut map);
    assert_eq!(map.len(), MAX_RELAY_CANDIDATES);
    assert!(
        map.contains_key(&keeper),
        "the best relay must survive a flood"
    );
}

#[test]
fn pruning_leaves_a_small_roster_untouched() {
    let mut map = HashMap::new();
    map.insert(PeerId::random(), candidate(Some(10), 0));
    prune_relay_candidates(&mut map);
    assert_eq!(map.len(), 1);
}

#[test]
fn decay_halves_failures() {
    let mut c = candidate(Some(10), 5);
    c.decay();
    assert_eq!(c.failures, 2);
    c.decay();
    assert_eq!(c.failures, 1);
}

#[test]
fn decay_reaches_zero_and_stays() {
    let mut c = candidate(None, 1);
    c.decay();
    assert_eq!(c.failures, 0);
    c.decay();
    assert_eq!(c.failures, 0);
}

#[test]
fn a_penalised_relay_recovers_its_rank_after_enough_decay() {
    let (punished, steady) = (PeerId::random(), PeerId::random());
    let mut map = HashMap::new();
    map.insert(punished, candidate(Some(10), 4));
    map.insert(steady, candidate(Some(200), 0));

    assert_eq!(best_relay_candidates(&map, 1), vec![steady]);

    for _ in 0..3 {
        for c in map.values_mut() {
            c.decay();
        }
    }
    assert_eq!(
        best_relay_candidates(&map, 1),
        vec![punished],
        "a recovered low-latency relay should win again"
    );
}
