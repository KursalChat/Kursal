use crate::network::dht::{DHT_TARGET, check_pow, mine_pow};

#[test]
fn pow_mine_and_verify_hybrid() {
    let message = b"kursal-pow-test-message".to_vec();
    let tag = [0u8; 32];

    let nonce = mine_pow(&DHT_TARGET, &message, &tag).unwrap();
    assert!(check_pow(&DHT_TARGET, &message, nonce, &tag));

    let mut tampered = message.clone();
    tampered[0] ^= 1;
    assert!(!check_pow(&DHT_TARGET, &tampered, nonce, &tag));

    assert!(!check_pow(
        &DHT_TARGET,
        &message,
        nonce.wrapping_add(1),
        &tag
    ));
}

#[test]
fn pow_garbage_nonce_rejected_cheaply() {
    let message = b"junk-record".to_vec();
    let tag = [1u8; 32];

    let start = std::time::Instant::now();
    let mut accepted = 0;
    for nonce in 0..64u128 {
        if check_pow(&DHT_TARGET, &message, nonce, &tag) {
            accepted += 1;
        }
    }
    assert_eq!(accepted, 0);
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}
