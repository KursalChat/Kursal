use crate::network::dht::{DHT_TARGET, DHTRecord, check_pow, mine_pow};
use crate::network::kademlia::{KursalKadStore, spawn_record_validation};
use crate::storage::get_timestamp_secs;
use libp2p::PeerId;
use libp2p::kad::store::RecordStore;
use libp2p::kad::{Record, RecordKey};
use std::time::Duration;
use tokio::sync::mpsc;

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

#[tokio::test]
async fn pow_proof_not_reusable_across_key_value_split() {
    let now = get_timestamp_secs().unwrap();
    let record = DHTRecord::new(b"abcd".to_vec(), b"efgh".to_vec(), now, false)
        .await
        .unwrap();

    assert!(DHTRecord::is_valid(b"abcd", &record.serialize().unwrap()).is_ok());

    let shifted = DHTRecord {
        key: b"abc".to_vec(),
        value: b"defgh".to_vec(),
        proof: record.proof,
        timestamp: record.timestamp,
        is_long: record.is_long,
    };

    assert!(DHTRecord::is_valid(b"abc", &shifted.serialize().unwrap()).is_err());
}

#[tokio::test]
async fn inbound_validation_drops_unmined_record() {
    let (tx, mut rx) = mpsc::channel(4);

    spawn_record_validation(
        Record {
            key: RecordKey::new(b"kursal-spam-key"),
            value: b"not a mined record".to_vec(),
            publisher: None,
            expires: None,
        },
        tx,
    );

    let forwarded = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;
    assert!(matches!(forwarded, Ok(None)));
}

#[tokio::test]
async fn inbound_validation_forwards_mined_record() {
    let key = b"kursal-store-test-key".to_vec();
    let now = get_timestamp_secs().unwrap();
    let dht_record = DHTRecord::new(key.clone(), b"payload".to_vec(), now, false)
        .await
        .unwrap();

    let (tx, mut rx) = mpsc::channel(4);

    spawn_record_validation(
        Record {
            key: RecordKey::new(&key),
            value: dht_record.serialize().unwrap(),
            publisher: None,
            expires: None,
        },
        tx,
    );

    let forwarded = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap();
    assert_eq!(forwarded.unwrap().key.as_ref(), key.as_slice());
}

#[tokio::test]
async fn kad_store_holds_a_put_record() {
    let mut store = KursalKadStore::new(PeerId::random());

    let record = Record {
        key: RecordKey::new(b"kursal-store-key"),
        value: b"already validated".to_vec(),
        publisher: None,
        expires: None,
    };

    assert!(store.put(record).is_ok());
    assert!(store.get(&RecordKey::new(b"kursal-store-key")).is_some());
}
