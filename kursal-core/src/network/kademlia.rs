use crate::network::dht::DHTRecord;
use libp2p::PeerId;
use libp2p::kad::store::RecordStore;
use libp2p::kad::{ProviderRecord, Record, store::MemoryStore, store::MemoryStoreConfig};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Semaphore, mpsc};

pub const KAD_MAX_CLOCK_SKEW: u64 = 120;
pub const KAD_MAX_PAYLOAD: usize = 128 * 1024;
pub const KAD_MAX_PACKET: usize = KAD_MAX_PAYLOAD + 16 * 1024;

pub const KAD_MAX_AGE: u64 = 10 * 60;
pub const KAD_LONG_MAX_AGE: u64 = 3 * 7 * 24 * 60 * 60;

pub const KAD_VALIDATED_QUEUE: usize = 64;
const KAD_CONCURRENT_VALIDATIONS: usize = 4;

static VALIDATION_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(KAD_CONCURRENT_VALIDATIONS)));

pub fn spawn_record_validation(record: Record, validated_tx: mpsc::Sender<Record>) {
    let Ok(permit) = VALIDATION_SLOTS.clone().try_acquire_owned() else {
        log::debug!("[kad] validation slots busy, dropping inbound record");
        return;
    };

    tokio::spawn(async move {
        let checked = tokio::task::spawn_blocking(move || {
            let valid = DHTRecord::is_valid(record.key.as_ref(), &record.value).is_ok();
            (valid, record)
        })
        .await;

        drop(permit);

        match checked {
            Ok((true, record)) => {
                let _ = validated_tx.send(record).await;
            }
            Ok((false, record)) => {
                log::debug!(
                    "[kad] rejected record key={}",
                    hex::encode(&record.key.as_ref()[..record.key.as_ref().len().min(8)])
                );
            }
            Err(err) => log::warn!("[kad] record validation task failed: {err}"),
        }
    });
}

pub struct KursalKadStore {
    inner: MemoryStore,
}

impl KursalKadStore {
    pub fn new(peer_id: PeerId) -> Self {
        let config = MemoryStoreConfig {
            max_value_bytes: KAD_MAX_PAYLOAD,
            ..Default::default()
        };
        Self {
            inner: MemoryStore::with_config(peer_id, config),
        }
    }
}

impl RecordStore for KursalKadStore {
    type RecordsIter<'a> = <MemoryStore as RecordStore>::RecordsIter<'a>;
    type ProvidedIter<'a> = <MemoryStore as RecordStore>::ProvidedIter<'a>;

    fn put(&mut self, r: Record) -> libp2p::kad::store::Result<()> {
        self.inner.put(r)
    }

    fn add_provider(&mut self, record: ProviderRecord) -> libp2p::kad::store::Result<()> {
        self.inner.add_provider(record)
    }
    fn get(&self, k: &libp2p::kad::RecordKey) -> Option<std::borrow::Cow<'_, Record>> {
        self.inner.get(k)
    }
    fn provided(&self) -> Self::ProvidedIter<'_> {
        self.inner.provided()
    }
    fn providers(&self, key: &libp2p::kad::RecordKey) -> Vec<ProviderRecord> {
        self.inner.providers(key)
    }
    fn records(&self) -> Self::RecordsIter<'_> {
        self.inner.records()
    }
    fn remove(&mut self, k: &libp2p::kad::RecordKey) {
        self.inner.remove(k);
    }
    fn remove_provider(&mut self, k: &libp2p::kad::RecordKey, p: &PeerId) {
        self.inner.remove_provider(k, p);
    }
}
