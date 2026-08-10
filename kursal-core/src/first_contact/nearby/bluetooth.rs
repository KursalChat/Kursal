use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    first_contact::nearby::{BtEvent, NearbyBeacon, NearbyMessage, NearbyTransport},
    network::swarm::SwarmCommand,
};
#[cfg(not(target_os = "android"))]
use ble_peripheral_rust::{
    Peripheral as PeripheralAd, PeripheralImpl,
    gatt::{
        characteristic::Characteristic as AdCharacteristic,
        peripheral_event::{
            PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
        },
        properties::{AttributePermission, CharacteristicProperty},
        service::Service,
    },
};
use btleplug::api::{
    Central, CentralEvent, CharPropFlags, Characteristic, DEFAULT_MTU_SIZE, Manager as _,
    Peripheral, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral as PlatformPeripheral};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify, mpsc};
use uuid::Uuid;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x4b75_7273_616c_0001_0000_0000_0000_0001);
const CHAR_UUID: Uuid = Uuid::from_u128(0x4b75_7273_616c_0001_0000_0000_0000_0002);
#[cfg(not(target_os = "android"))]
const POWER_POLL_INTERVAL: Duration = Duration::from_millis(250);
#[cfg(not(target_os = "android"))]
const POWER_POLL_MAX_ATTEMPTS: u32 = 60;

const FRAG_HEADER_LEN: usize = 12;
const ATT_OVERHEAD: usize = 3;
const DEFAULT_CHUNK_SIZE: usize = 160;
const MIN_CHUNK_SIZE: usize = 8;
const MAX_CHUNK_SIZE: usize = 500;
const WRITE_ACK_WINDOW: u16 = 8;
const REASSEMBLY_TTL: Duration = Duration::from_secs(60);
const PEER_WAIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
struct PartialMsg {
    total: usize,
    fragments: Vec<Option<Vec<u8>>>,
    received: usize,
    created_at: Instant,
}

type ReassemblyMap = Arc<std::sync::Mutex<HashMap<(String, u64), PartialMsg>>>;

type GattLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;

#[derive(Serialize, Deserialize)]
struct BtWireMessage {
    from_peer_id: String,
    msg: NearbyMessage,
}

struct BtPeer {
    bt_id: String,
    peripheral: PlatformPeripheral,
    write_char: Characteristic,
}

impl BtPeer {
    fn new(peripheral: PlatformPeripheral, write_char: Characteristic) -> Self {
        Self {
            bt_id: peripheral.id().to_string(),
            peripheral,
            write_char,
        }
    }
}

#[cfg(not(target_os = "android"))]
struct AdvState {
    peripheral: PeripheralAd,
    service_added: bool,
    advertising: bool,
}

struct ScanState {
    central: Adapter,
    scanning: bool,
}

pub struct BTTransport {
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    pub pending_handshakes: Arc<Mutex<HashMap<String, mpsc::Sender<NearbyMessage>>>>,
    pub bt_event_tx: mpsc::Sender<BtEvent>,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_added: Arc<Notify>,
    gatt_locks: GattLocks,
    #[cfg(not(target_os = "android"))]
    adv: Arc<Mutex<Option<AdvState>>>,
    scan: Arc<Mutex<Option<ScanState>>>,
    reassembly: ReassemblyMap,
}

impl BTTransport {
    pub fn new(
        cmd_tx: mpsc::Sender<SwarmCommand>,
        my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
        bt_event_tx: mpsc::Sender<BtEvent>,
    ) -> Self {
        Self {
            cmd_tx,
            my_beacon,
            pending_handshakes: Arc::new(Mutex::new(HashMap::new())),
            bt_event_tx,
            peers: Arc::new(Mutex::new(HashMap::new())),
            peer_added: Arc::new(Notify::new()),
            gatt_locks: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(not(target_os = "android"))]
            adv: Arc::new(Mutex::new(None)),
            scan: Arc::new(Mutex::new(None)),
            reassembly: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl NearbyTransport for BTTransport {
    async fn start(&self, beacon: NearbyBeacon) {
        log::info!("[bt] starting bluetooth transport");

        if let Err(err) = start_scanner(
            &self.scan,
            self.peers.clone(),
            self.peer_added.clone(),
            self.gatt_locks.clone(),
            self.bt_event_tx.clone(),
        )
        .await
        {
            log::error!("[bt] scanner start: {err}");
        }

        #[cfg(not(target_os = "android"))]
        if let Err(err) = start_advertiser(
            &self.adv,
            beacon,
            self.my_beacon.clone(),
            self.bt_event_tx.clone(),
            self.reassembly.clone(),
        )
        .await
        {
            log::error!("[bt] advertiser start: {err}");
        }

        #[cfg(target_os = "android")]
        if let Err(err) = start_advertiser_android(
            beacon,
            self.my_beacon.clone(),
            self.bt_event_tx.clone(),
            self.reassembly.clone(),
        ) {
            log::error!("[bt] android advertiser start: {err}");
        }
    }

    async fn stop(&self) {
        if let Some(state) = self.scan.lock().await.as_mut()
            && state.scanning
        {
            if let Err(err) = state.central.stop_scan().await {
                log::warn!("[bt] stop scan: {err}");
            } else {
                state.scanning = false;
            }
        }

        #[cfg(not(target_os = "android"))]
        if let Some(state) = self.adv.lock().await.as_mut()
            && state.advertising
        {
            if let Err(err) = state.peripheral.stop_advertising().await {
                log::warn!("[bt] stop advertising: {err}");
            } else {
                state.advertising = false;
            }
        }

        #[cfg(target_os = "android")]
        if let Err(err) = stop_advertiser_android() {
            log::warn!("[bt] android stop advertising: {err}");
        }

        let drained = {
            let mut p = self.peers.lock().await;
            std::mem::take(&mut *p)
        };
        for (_, peer) in drained {
            let _ = peer.peripheral.disconnect().await;
        }
        log::info!("[bt] stopped");
    }

    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> Result<()> {
        let my_beacon = self
            .my_beacon
            .lock()
            .await
            .clone()
            .ok_or_else(|| KursalError::Network("bt: no active beacon".into()))?;

        let wire = BtWireMessage {
            from_peer_id: my_beacon.peer_id,
            msg,
        };
        let bytes = bincode::serialize(&wire)?;

        let (peripheral, mut write_char) =
            ensure_peer_connected(&self.peers, &self.peer_added, &self.gatt_locks, peer_id).await?;

        let chunk_size = chunk_size_for(&peripheral);
        let unacked_writes = write_char
            .properties
            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE);

        let total_chunks = bytes.chunks(chunk_size).count();
        let total = u16::try_from(total_chunks).map_err(|_| {
            KursalError::Network(format!("bt: message too large ({total_chunks} chunks)"))
        })?;
        let msg_id: u64 = rand::random();

        for (seq, chunk) in (0..total).zip(bytes.chunks(chunk_size)) {
            let mut frame = Vec::with_capacity(FRAG_HEADER_LEN + chunk.len());
            frame.extend_from_slice(&msg_id.to_be_bytes());
            frame.extend_from_slice(&seq.to_be_bytes());
            frame.extend_from_slice(&total.to_be_bytes());
            frame.extend_from_slice(chunk);

            let write_type = if !unacked_writes
                || seq + 1 == total
                || (seq + 1).is_multiple_of(WRITE_ACK_WINDOW)
            {
                WriteType::WithResponse
            } else {
                WriteType::WithoutResponse
            };

            if let Err(e) = peripheral.write(&write_char, &frame, write_type).await {
                log::warn!("[bt] write to {peer_id} failed: {e}, attempting reconnect");
                let _ = peripheral.disconnect().await;
                self.peers.lock().await.remove(peer_id);
                write_char =
                    connect_and_register(&self.peers, &self.gatt_locks, peer_id, &peripheral)
                        .await?;
                peripheral
                    .write(&write_char, &frame, write_type)
                    .await
                    .ok_kursal(KursalError::Network)?;
            }
        }

        Ok(())
    }

    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage> {
        let (tx, rx) = mpsc::channel::<NearbyMessage>(8);

        self.pending_handshakes
            .lock()
            .await
            .insert(peer_id.to_string(), tx);

        rx
    }

    async fn unregister_handshake(&self, peer_id: &str) {
        self.pending_handshakes.lock().await.remove(peer_id);
    }
}

fn chunk_size_for(peripheral: &PlatformPeripheral) -> usize {
    let mtu = peripheral.mtu();
    if mtu > DEFAULT_MTU_SIZE {
        (mtu as usize)
            .saturating_sub(FRAG_HEADER_LEN + ATT_OVERHEAD)
            .clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
    } else {
        DEFAULT_CHUNK_SIZE
    }
}

async fn gatt_lock(locks: &GattLocks, bt_id: &str) -> Arc<Mutex<()>> {
    locks
        .lock()
        .await
        .entry(bt_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

async fn prune_gatt_locks(locks: &GattLocks) {
    locks.lock().await.retain(|_, l| Arc::strong_count(l) > 1);
}

async fn connect_and_discover(
    locks: &GattLocks,
    peripheral: &PlatformPeripheral,
) -> Result<Characteristic> {
    let bt_err = |e: btleplug::Error| KursalError::Network(e.to_string());

    let lock = gatt_lock(locks, &peripheral.id().to_string()).await;
    let _guard = lock.lock().await;

    if !peripheral.is_connected().await.map_err(bt_err)? {
        peripheral.connect().await.map_err(bt_err)?;
    }
    if peripheral.characteristics().is_empty() {
        peripheral.discover_services().await.map_err(bt_err)?;
    }

    let write_char = peripheral
        .characteristics()
        .iter()
        .find(|c| c.uuid == CHAR_UUID)
        .ok_or_else(|| KursalError::Network("bt: kursal char missing".into()))?
        .clone();

    Ok(write_char)
}

async fn connect_and_register(
    peers: &Arc<Mutex<HashMap<String, BtPeer>>>,
    locks: &GattLocks,
    peer_id: &str,
    peripheral: &PlatformPeripheral,
) -> Result<Characteristic> {
    let write_char = connect_and_discover(locks, peripheral).await?;

    peers.lock().await.insert(
        peer_id.to_string(),
        BtPeer::new(peripheral.clone(), write_char.clone()),
    );

    Ok(write_char)
}

async fn ensure_peer_connected(
    peers: &Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_added: &Notify,
    locks: &GattLocks,
    peer_id: &str,
) -> Result<(PlatformPeripheral, Characteristic)> {
    let deadline = Instant::now() + PEER_WAIT_TIMEOUT;

    let (peripheral, write_char) = loop {
        let mut wait = Box::pin(peer_added.notified());
        wait.as_mut().enable();

        {
            let guard = peers.lock().await;
            if let Some(peer) = guard.get(peer_id) {
                break (peer.peripheral.clone(), peer.write_char.clone());
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || tokio::time::timeout(remaining, wait).await.is_err() {
            return Err(KursalError::Network(format!(
                "bt: peer {peer_id} not connected"
            )));
        }
    };

    match peripheral.is_connected().await {
        Ok(true) => Ok((peripheral, write_char)),
        Ok(false) => {
            log::info!("[bt] peer {peer_id} disconnected, reconnecting");
            let new_char = connect_and_register(peers, locks, peer_id, &peripheral).await?;
            Ok((peripheral, new_char))
        }
        Err(err) => Err(KursalError::Network(err.to_string())),
    }
}

async fn start_scanner(
    scan: &Arc<Mutex<Option<ScanState>>>,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_added: Arc<Notify>,
    gatt_locks: GattLocks,
    bt_event_tx: mpsc::Sender<BtEvent>,
) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
            .map_err(|e| KursalError::Network(format!("JNI error: {e:?}")))?;
        let env = vm
            .attach_current_thread()
            .map_err(|e| KursalError::Network(format!("JNI attach error: {e:?}")))?;
        btleplug::platform::init(&env).map_err(|e| jvm_err(&env, "btleplug init", e))?;
    }

    let err = |s: String| KursalError::Network(s);

    let mut guard = scan.lock().await;
    if guard.is_none() {
        let manager = Manager::new()
            .await
            .map_err(|e| err(format!("bt manager: {e}")))?;
        let central = manager
            .adapters()
            .await
            .map_err(|e| err(format!("bt adapters: {e}")))?
            .into_iter()
            .next()
            .ok_or_else(|| err("no bluetooth adapter".into()))?;

        let events = central
            .events()
            .await
            .map_err(|e| err(format!("bt events: {e}")))?;

        tokio::spawn(run_scanner_events(
            events,
            central.clone(),
            peers,
            peer_added,
            gatt_locks,
            bt_event_tx,
        ));

        *guard = Some(ScanState {
            central,
            scanning: false,
        });
    }

    let Some(state) = guard.as_mut() else {
        return Err(err("bt scanner state missing".into()));
    };

    if state.scanning {
        let _ = state.central.stop_scan().await;
        state.scanning = false;
    }

    state
        .central
        .start_scan(ScanFilter {
            services: vec![SERVICE_UUID],
        })
        .await
        .map_err(|e| err(format!("start scan: {e}")))?;
    state.scanning = true;

    log::info!("[bt] scanner started");
    Ok(())
}

async fn run_scanner_events(
    events: std::pin::Pin<Box<dyn futures::Stream<Item = CentralEvent> + Send>>,
    central: Adapter,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_added: Arc<Notify>,
    gatt_locks: GattLocks,
    bt_event_tx: mpsc::Sender<BtEvent>,
) {
    let inflight: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    futures::pin_mut!(events);

    while let Some(event) = events.next().await {
        let id = match event {
            CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
            CentralEvent::DeviceDisconnected(id) => {
                let bt_id = id.to_string();
                remove_peer_by_bt_id(&peers, &bt_id).await;
                inflight.lock().await.remove(&bt_id);
                prune_gatt_locks(&gatt_locks).await;
                log::info!("[bt] device disconnected {bt_id}, cleared peer");
                continue;
            }
            _ => continue,
        };
        let bt_id = id.to_string();

        if peers_contains_bt_id(&peers, &bt_id).await {
            continue;
        }
        if !inflight.lock().await.insert(bt_id.clone()) {
            continue;
        }

        let peripheral = match central.peripheral(&id).await {
            Ok(p) => p,
            Err(_) => {
                inflight.lock().await.remove(&bt_id);
                continue;
            }
        };

        log::info!("[bt] kursal peripheral {bt_id} - handshaking");
        let peers_clone = peers.clone();
        let notify_clone = peer_added.clone();
        let locks_clone = gatt_locks.clone();
        let tx_clone = bt_event_tx.clone();
        let inflight_clone = inflight.clone();
        tokio::spawn(async move {
            if let Err(err) =
                handshake_with_peer(peripheral, peers_clone, notify_clone, locks_clone, tx_clone)
                    .await
            {
                log::warn!("[bt] handshake {bt_id}: {err}");
            }
            inflight_clone.lock().await.remove(&bt_id);
        });
    }

    log::warn!("[bt] scanner event loop exited");
}

async fn peers_contains_bt_id(peers: &Arc<Mutex<HashMap<String, BtPeer>>>, bt_id: &str) -> bool {
    let guard = peers.lock().await;
    guard.values().any(|p| p.bt_id == bt_id)
}

async fn remove_peer_by_bt_id(peers: &Arc<Mutex<HashMap<String, BtPeer>>>, bt_id: &str) {
    let mut guard = peers.lock().await;
    guard.retain(|_, p| p.bt_id != bt_id);
}

async fn handshake_with_peer(
    peripheral: PlatformPeripheral,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_added: Arc<Notify>,
    gatt_locks: GattLocks,
    bt_event_tx: mpsc::Sender<BtEvent>,
) -> Result<()> {
    let bt_err = |e: btleplug::Error| KursalError::Network(e.to_string());

    let write_char = connect_and_discover(&gatt_locks, &peripheral).await?;

    let beacon_bytes = peripheral.read(&write_char).await.map_err(bt_err)?;
    let beacon = NearbyBeacon::deserialize(&beacon_bytes)?;
    let peer_id = beacon.peer_id.clone();
    log::info!(
        "[bt] got beacon from {peer_id} session={:?}",
        beacon.session_name
    );

    peers
        .lock()
        .await
        .insert(peer_id.clone(), BtPeer::new(peripheral, write_char));
    peer_added.notify_waiters();

    bt_event_tx
        .send(BtEvent::Beacon { peer_id, beacon })
        .await
        .ok_kursal(KursalError::Network)?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
async fn start_advertiser(
    adv: &Arc<Mutex<Option<AdvState>>>,
    beacon: NearbyBeacon,
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
) -> Result<()> {
    let err = |s: String| KursalError::Network(s);

    let mut guard = adv.lock().await;
    if guard.is_none() {
        let (event_tx, event_rx) = mpsc::channel::<PeripheralEvent>(256);

        let mut peripheral = PeripheralAd::new(event_tx)
            .await
            .map_err(|e| err(format!("peripheral init: {e}")))?;

        if !wait_powered(&mut peripheral).await {
            return Err(err("bluetooth not powered".into()));
        }

        tokio::spawn(run_peripheral_events(
            event_rx,
            my_beacon,
            bt_event_tx,
            reassembly,
        ));

        *guard = Some(AdvState {
            peripheral,
            service_added: false,
            advertising: false,
        });
    }

    let Some(state) = guard.as_mut() else {
        return Err(err("bt advertiser state missing".into()));
    };

    if !state.service_added {
        let service = kursal_service();
        state
            .peripheral
            .add_service(&service)
            .await
            .map_err(|e| err(format!("add service: {e}")))?;
        state.service_added = true;
    }

    if state.advertising {
        state
            .peripheral
            .stop_advertising()
            .await
            .map_err(|e| err(format!("stop advertising: {e}")))?;
        state.advertising = false;
    }

    let ad_name = beacon.session_name.clone();
    state
        .peripheral
        .start_advertising(&ad_name, &[SERVICE_UUID])
        .await
        .map_err(|e| err(format!("start advertising: {e}")))?;
    state.advertising = true;

    log::info!("[bt] advertising as {ad_name:?}");
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn kursal_service() -> Service {
    Service {
        uuid: SERVICE_UUID,
        primary: true,
        characteristics: vec![AdCharacteristic {
            uuid: CHAR_UUID,
            properties: vec![
                CharacteristicProperty::Read,
                CharacteristicProperty::Write,
                CharacteristicProperty::WriteWithoutResponse,
            ],
            permissions: vec![
                AttributePermission::Readable,
                AttributePermission::Writeable,
            ],
            value: None,
            descriptors: vec![],
        }],
    }
}

#[cfg(not(target_os = "android"))]
async fn wait_powered(peripheral: &mut PeripheralAd) -> bool {
    for _ in 0..POWER_POLL_MAX_ATTEMPTS {
        match peripheral.is_powered().await {
            Ok(true) => return true,
            Ok(false) => {}
            Err(err) => {
                log::error!("[bt] power poll: {err}");
                return false;
            }
        }
        tokio::time::sleep(POWER_POLL_INTERVAL).await;
    }
    log::warn!("[bt] bluetooth never powered on");
    false
}

#[cfg(not(target_os = "android"))]
async fn run_peripheral_events(
    mut event_rx: mpsc::Receiver<PeripheralEvent>,
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
) {
    while let Some(event) = event_rx.recv().await {
        handle_peripheral_event(event, &my_beacon, &bt_event_tx, &reassembly).await;
    }
}

#[cfg(not(target_os = "android"))]
async fn handle_peripheral_event(
    event: PeripheralEvent,
    my_beacon: &Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: &mpsc::Sender<BtEvent>,
    reassembly: &ReassemblyMap,
) {
    match event {
        PeripheralEvent::StateUpdate { is_powered } => {
            log::info!("[bt] power state: {is_powered:?}");
        }
        PeripheralEvent::CharacteristicSubscriptionUpdate { .. } => {}
        PeripheralEvent::ReadRequest { responder, .. } => {
            let value = match my_beacon.lock().await.as_ref() {
                Some(b) => b.serialize().unwrap_or_default(),
                None => Vec::new(),
            };
            let _ = responder.send(ReadRequestResponse {
                value,
                response: RequestResponse::Success,
            });
        }
        PeripheralEvent::WriteRequest {
            request,
            value,
            responder,
            ..
        } => {
            let _ = responder.send(WriteRequestResponse {
                response: RequestResponse::Success,
            });

            if let Some(bytes) = push_fragment(reassembly, &request.client, &value) {
                match bincode::deserialize::<BtWireMessage>(&bytes) {
                    Ok(wire) => {
                        let _ = bt_event_tx
                            .send(BtEvent::Message {
                                from_peer_id: wire.from_peer_id,
                                msg: wire.msg,
                            })
                            .await;
                    }
                    Err(err) => {
                        log::warn!("[bt] bad wire message: {err}");
                    }
                }
            }
        }
    }
}

fn push_fragment(reassembly: &ReassemblyMap, client: &str, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < FRAG_HEADER_LEN {
        log::warn!("[bt] fragment shorter than header ({} bytes)", frame.len());
        return None;
    }
    let msg_id = u64::from_be_bytes(frame[0..8].try_into().ok()?);
    let seq = u16::from_be_bytes(frame[8..10].try_into().ok()?) as usize;
    let total = u16::from_be_bytes(frame[10..12].try_into().ok()?) as usize;
    let data = &frame[FRAG_HEADER_LEN..];

    if total == 0 || seq >= total {
        log::warn!("[bt] fragment seq/total invalid (seq={seq}, total={total})");
        return None;
    }

    let mut map = match reassembly.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let now = Instant::now();
    map.retain(|_, v| now.duration_since(v.created_at) < REASSEMBLY_TTL);

    let key = (client.to_string(), msg_id);
    let partial = map.entry(key.clone()).or_insert_with(|| PartialMsg {
        total,
        fragments: vec![None; total],
        received: 0,
        created_at: now,
    });

    if partial.total != total || partial.fragments.len() != total {
        log::warn!("[bt] fragment total mismatch for msg {msg_id}");
        map.remove(&key);
        return None;
    }
    if partial.fragments[seq].is_some() {
        return None;
    }

    partial.fragments[seq] = Some(data.to_vec());
    partial.received += 1;

    if partial.received < partial.total {
        return None;
    }

    let done = map.remove(&key)?;
    let mut out = Vec::new();
    for frag in done.fragments.into_iter() {
        out.extend(frag?);
    }
    Some(out)
}

#[cfg(target_os = "android")]
struct AndroidAdvState {
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
}

#[cfg(target_os = "android")]
static ANDROID_STATE: std::sync::Mutex<Option<AndroidAdvState>> = std::sync::Mutex::new(None);

#[cfg(target_os = "android")]
fn jvm_err<E: std::fmt::Debug>(env: &jni::JNIEnv, prefix: &str, e: E) -> KursalError {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    KursalError::Network(format!("{prefix}: {e:?}"))
}

#[cfg(target_os = "android")]
fn android_state_lock<R>(f: impl FnOnce(&mut Option<AndroidAdvState>) -> R) -> R {
    let mut g = match ANDROID_STATE.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    f(&mut g)
}

#[cfg(target_os = "android")]
fn start_advertiser_android(
    beacon: NearbyBeacon,
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
) -> Result<()> {
    android_state_lock(|s| {
        *s = Some(AndroidAdvState {
            my_beacon,
            bt_event_tx,
            reassembly,
        });
    });

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| KursalError::Network(format!("JNI vm: {e:?}")))?;
    let env = vm
        .attach_current_thread()
        .map_err(|e| KursalError::Network(format!("JNI attach: {e:?}")))?;

    let cls = load_kursal_class(&env, "chat.kursal.BleAdvertiser")?;
    let app_ctx: jni::objects::JObject = (ctx.context() as jni::sys::jobject).into();

    let svc_uuid_jstr = env
        .new_string(SERVICE_UUID.to_string())
        .map_err(|e| jvm_err(&env, "uuid jstring", e))?;
    let char_uuid_jstr = env
        .new_string(CHAR_UUID.to_string())
        .map_err(|e| jvm_err(&env, "char uuid jstring", e))?;
    let name_jstr = env
        .new_string(beacon.session_name.clone())
        .map_err(|e| jvm_err(&env, "name jstring", e))?;

    let res = env
        .call_static_method(
            cls,
            "start",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z",
            &[
                jni::objects::JValue::Object(app_ctx),
                jni::objects::JValue::Object(svc_uuid_jstr.into()),
                jni::objects::JValue::Object(char_uuid_jstr.into()),
                jni::objects::JValue::Object(name_jstr.into()),
            ],
        )
        .map_err(|e| jvm_err(&env, "BleAdvertiser.start", e))?;

    let ok = res
        .z()
        .map_err(|e| KursalError::Network(format!("bool ret: {e:?}")))?;
    if !ok {
        return Err(KursalError::Network(
            "BleAdvertiser.start returned false".into(),
        ));
    }
    log::info!("[bt] android advertising as {:?}", beacon.session_name);
    Ok(())
}

#[cfg(target_os = "android")]
fn stop_advertiser_android() -> Result<()> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| KursalError::Network(format!("JNI vm: {e:?}")))?;
    let env = vm
        .attach_current_thread()
        .map_err(|e| KursalError::Network(format!("JNI attach: {e:?}")))?;

    let cls = load_kursal_class(&env, "chat.kursal.BleAdvertiser")?;
    let app_ctx: jni::objects::JObject = (ctx.context() as jni::sys::jobject).into();

    let result = env
        .call_static_method(
            cls,
            "stop",
            "(Landroid/content/Context;)V",
            &[jni::objects::JValue::Object(app_ctx)],
        )
        .map_err(|e| jvm_err(&env, "BleAdvertiser.stop", e));

    android_state_lock(|s| *s = None);

    result.map(|_| ())
}

#[cfg(target_os = "android")]
fn load_kursal_class<'a>(
    env: &jni::JNIEnv<'a>,
    dotted_name: &str,
) -> Result<jni::objects::JClass<'a>> {
    let ctx = ndk_context::android_context();
    let app_ctx: jni::objects::JObject = (ctx.context() as jni::sys::jobject).into();

    let cl = env
        .call_method(app_ctx, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .map_err(|e| jvm_err(env, "getClassLoader", e))?
        .l()
        .map_err(|e| KursalError::Network(format!("classloader.l: {e:?}")))?;

    let name_jstr = env
        .new_string(dotted_name)
        .map_err(|e| jvm_err(env, "class name jstring", e))?;

    let cls_obj = env
        .call_method(
            cl,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[jni::objects::JValue::Object(name_jstr.into())],
        )
        .map_err(|e| jvm_err(env, "loadClass", e))?
        .l()
        .map_err(|e| KursalError::Network(format!("class.l: {e:?}")))?;

    Ok(jni::objects::JClass::from(cls_obj))
}

#[cfg(target_os = "android")]
pub fn android_handle_read() -> Vec<u8> {
    let my_beacon = match android_state_lock(|s| s.as_ref().map(|st| st.my_beacon.clone())) {
        Some(mb) => mb,
        None => return Vec::new(),
    };
    let beacon_opt = my_beacon.blocking_lock();
    match beacon_opt.as_ref() {
        Some(b) => b.serialize().unwrap_or_default(),
        None => Vec::new(),
    }
}

#[cfg(target_os = "android")]
pub fn android_handle_write(client: &str, data: &[u8]) {
    let (reassembly, bt_event_tx) = match android_state_lock(|s| {
        s.as_ref()
            .map(|st| (st.reassembly.clone(), st.bt_event_tx.clone()))
    }) {
        Some(v) => v,
        None => return,
    };

    if let Some(bytes) = push_fragment(&reassembly, client, data) {
        match bincode::deserialize::<BtWireMessage>(&bytes) {
            Ok(wire) => {
                if let Err(e) = bt_event_tx.try_send(BtEvent::Message {
                    from_peer_id: wire.from_peer_id,
                    msg: wire.msg,
                }) {
                    log::warn!("[bt] android try_send: {e}");
                }
            }
            Err(err) => {
                log::warn!("[bt] bad wire message: {err}");
            }
        }
    }
}
