use libp2p::PeerId;
use libp2p::kad::store::RecordStore;
use libp2p::kad::{ProviderRecord, Record, store::MemoryStore, store::MemoryStoreConfig};

pub const KAD_MAX_CLOCK_SKEW: u64 = 120;
pub const KAD_MAX_PAYLOAD: usize = 64 * 1024;

pub const KAD_MAX_AGE: u64 = 10 * 60;
pub const KAD_LONG_MAX_AGE: u64 = 3 * 7 * 24 * 60 * 60;

pub struct KursalKadStore {
    inner: MemoryStore,
}

impl KursalKadStore {
    pub fn new(peer_id: PeerId) -> Self {
        let config = MemoryStoreConfig {
            max_value_bytes: KAD_MAX_PAYLOAD * 2,
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
