use crate::Result;
use crate::api::AppEvent;
use crate::call::frame::{decode_frame, encode_frame};
use crate::call::media::ReplayWindow;
use crate::crypto::stream::{stream_decrypt_aad, stream_encrypt_aad};
use crate::network::swarm::SwarmCommand;
use futures::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use libp2p::{PeerId, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, mpsc, oneshot};

pub const MAX_VIDEO_FRAME_BYTES: usize = 131072;
const FRAME_OVERHEAD_BYTES: usize = 48;
const TX_QUEUE_CHUNKS: usize = 30;
const CONGESTION_BACKLOG_CHUNKS: usize = TX_QUEUE_CHUNKS / 2;
const CONGESTION_EMIT_MIN: Duration = Duration::from_millis(1000);

pub(crate) fn seal_frame(key: &[u8; 32], seq: u64, chunk: &[u8]) -> Result<Vec<u8>> {
    let sealed = stream_encrypt_aad(key, chunk, &seq.to_be_bytes())?;
    Ok(encode_frame(seq, &sealed))
}

pub(crate) fn open_frame(key: &[u8; 32], wire: &[u8]) -> Result<(u64, Vec<u8>)> {
    let (seq, cipher) = decode_frame(wire)?;
    let pkt = stream_decrypt_aad(key, cipher, &seq.to_be_bytes())?;
    Ok((seq, pkt))
}

type StreamItem = (PeerId, Stream);

fn incoming_channel() -> &'static (
    mpsc::UnboundedSender<StreamItem>,
    Mutex<mpsc::UnboundedReceiver<StreamItem>>,
) {
    static CH: OnceLock<(
        mpsc::UnboundedSender<StreamItem>,
        Mutex<mpsc::UnboundedReceiver<StreamItem>>,
    )> = OnceLock::new();
    CH.get_or_init(|| {
        let (tx, rx) = mpsc::unbounded_channel();
        (tx, Mutex::new(rx))
    })
}

pub async fn deliver_incoming_stream(peer_id: PeerId, stream: Stream) {
    let _ = incoming_channel().0.send((peer_id, stream));
}

struct TxSession {
    stop: Arc<AtomicBool>,
    chunk_tx: mpsc::Sender<Vec<u8>>,
    dropped: Arc<AtomicBool>,
}

fn tx_slot() -> &'static StdMutex<Option<TxSession>> {
    static S: OnceLock<StdMutex<Option<TxSession>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

struct RxSession {
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

fn rx_slot() -> &'static StdMutex<Option<RxSession>> {
    static S: OnceLock<StdMutex<Option<RxSession>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

type RxForwarder = Box<dyn Fn(Vec<u8>) + Send + Sync>;

fn rx_forwarder() -> &'static StdMutex<Option<RxForwarder>> {
    static F: OnceLock<StdMutex<Option<RxForwarder>>> = OnceLock::new();
    F.get_or_init(|| StdMutex::new(None))
}

pub fn set_rx_forwarder(f: RxForwarder) {
    *rx_forwarder().lock().unwrap() = Some(f);
}

pub fn send_chunk(bytes: Vec<u8>) {
    if let Some(session) = tx_slot().lock().unwrap().as_ref()
        && session.chunk_tx.try_send(bytes).is_err()
    {
        session.dropped.store(true, Ordering::Relaxed);
    }
}

pub fn start_tx(
    peer_id: PeerId,
    tx_key: [u8; 32],
    cmd_tx: mpsc::Sender<SwarmCommand>,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    let stop = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let (chunk_tx, mut chunk_rx) = mpsc::channel::<Vec<u8>>(TX_QUEUE_CHUNKS);
    {
        let mut slot = tx_slot().lock().unwrap();
        if let Some(old) = slot.take() {
            old.stop.store(true, Ordering::Relaxed);
            let _ = old.chunk_tx.try_send(Vec::new());
        }
        *slot = Some(TxSession {
            stop: stop.clone(),
            chunk_tx,
            dropped: dropped.clone(),
        });
    }
    tokio::spawn(async move {
        let (reply, reply_rx) = oneshot::channel();
        if cmd_tx
            .send(SwarmCommand::OpenVideoStream { peer_id, reply })
            .await
            .is_err()
        {
            return;
        }
        let Ok(Some(stream)) = reply_rx.await else {
            log::warn!("[call] video: failed to open tx stream");
            return;
        };
        log::info!("[call] video tx: stream opened to {peer_id}");
        let (_read_half, mut write_half): (ReadHalf<Stream>, WriteHalf<Stream>) = stream.split();
        let mut last_congestion: Option<Instant> = None;
        let mut seq: u64 = 0;
        let mut logged_first_send = false;
        while !stop.load(Ordering::Relaxed) {
            let Some(chunk) = chunk_rx.recv().await else {
                break;
            };
            if dropped.swap(false, Ordering::Relaxed) || chunk_rx.len() >= CONGESTION_BACKLOG_CHUNKS
            {
                while chunk_rx.try_recv().is_ok() {}
                let now = Instant::now();
                let should_emit = match last_congestion {
                    Some(last) => now.duration_since(last) >= CONGESTION_EMIT_MIN,
                    None => true,
                };
                if should_emit {
                    last_congestion = Some(now);
                    log::warn!("[call] video tx: congested, dropped backlog to {peer_id}");
                    let _ = app_event_tx.try_send(AppEvent::VideoCongestion);
                }
                continue;
            }
            let Ok(wire) = seal_frame(&tx_key, seq, &chunk) else {
                continue;
            };
            seq = seq.wrapping_add(1);
            let Ok(wire_len) = u32::try_from(wire.len()) else {
                continue;
            };
            if write_half.write_all(&wire_len.to_be_bytes()).await.is_err() {
                break;
            }
            if write_half.write_all(&wire).await.is_err() {
                break;
            }
            if !logged_first_send {
                logged_first_send = true;
                log::info!("[call] video tx: first frame written to {peer_id}");
            }
        }
    });
}

enum ReadOutcome {
    Frame(u64, Vec<u8>),
    Skip,
    End,
}

async fn read_one(read_half: &mut ReadHalf<Stream>, rx_key: &[u8; 32]) -> ReadOutcome {
    let mut len_buf = [0u8; 4];
    if read_half.read_exact(&mut len_buf).await.is_err() {
        return ReadOutcome::End;
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 {
        return ReadOutcome::End;
    }
    if len > 4 * MAX_VIDEO_FRAME_BYTES {
        return ReadOutcome::End;
    }
    if len > MAX_VIDEO_FRAME_BYTES + FRAME_OVERHEAD_BYTES {
        let mut remaining = len;
        let mut discard = [0u8; 16384];
        while remaining > 0 {
            let take = remaining.min(discard.len());
            if read_half.read_exact(&mut discard[..take]).await.is_err() {
                return ReadOutcome::End;
            }
            remaining -= take;
        }
        return ReadOutcome::Skip;
    }
    let mut buf = vec![0u8; len];
    if read_half.read_exact(&mut buf).await.is_err() {
        return ReadOutcome::End;
    }
    match open_frame(rx_key, &buf) {
        Ok((seq, pkt)) => ReadOutcome::Frame(seq, pkt),
        Err(_) => ReadOutcome::Skip,
    }
}

pub fn start_rx(peer_id: PeerId, rx_key: [u8; 32]) {
    let stop = Arc::new(AtomicBool::new(false));
    let notify = Arc::new(Notify::new());
    {
        let mut slot = rx_slot().lock().unwrap();
        if let Some(old) = slot.take() {
            old.stop.store(true, Ordering::Relaxed);
            old.notify.notify_waiters();
        }
        *slot = Some(RxSession {
            stop: stop.clone(),
            notify: notify.clone(),
        });
    }
    tokio::spawn(async move {
        loop {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let stream = {
                let mut rx = incoming_channel().1.lock().await;
                let mut candidate: Option<Stream> = None;
                while let Ok((pid, s)) = rx.try_recv() {
                    if pid == peer_id {
                        candidate = Some(s);
                    }
                }
                match candidate {
                    Some(s) => Some(s),
                    None => loop {
                        tokio::select! {
                            _ = notify.notified() => break None,
                            r = tokio::time::timeout(Duration::from_secs(10), rx.recv()) => {
                                match r {
                                    Ok(Some((pid, s))) if pid == peer_id => break Some(s),
                                    Ok(Some(_)) => continue,
                                    _ => break None,
                                }
                            }
                        }
                    },
                }
            };
            let Some(stream) = stream else {
                log::warn!("[call] video: no incoming stream");
                return;
            };
            log::info!("[call] video rx: stream acquired from {peer_id}");
            let (mut read_half, _write_half) = stream.split();
            let mut replay = ReplayWindow::new();
            let mut got_frame = false;
            let mut logged_first_recv = false;
            let mut logged_no_sink = false;
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let frame = tokio::select! {
                    _ = notify.notified() => return,
                    r = read_one(&mut read_half, &rx_key) => r,
                };
                match frame {
                    ReadOutcome::Frame(seq, pkt) => {
                        got_frame = true;
                        if replay.accept(seq) {
                            let forwarder = rx_forwarder().lock().unwrap();
                            if let Some(f) = forwarder.as_ref() {
                                if !logged_first_recv {
                                    logged_first_recv = true;
                                    log::info!(
                                        "[call] video rx: forwarding first frame seq={seq} to UI"
                                    );
                                }
                                f(pkt);
                            } else if !logged_no_sink {
                                logged_no_sink = true;
                                log::warn!(
                                    "[call] video rx: frames arriving but UI sink not set yet"
                                );
                            }
                        }
                    }
                    ReadOutcome::Skip => continue,
                    ReadOutcome::End => break,
                }
            }
            if got_frame {
                return;
            }
            log::warn!("[call] video: dropped stale incoming stream, waiting for a live one");
        }
    });
}

pub async fn stop_tx() {
    if let Some(session) = tx_slot().lock().unwrap().take() {
        session.stop.store(true, Ordering::Relaxed);
        let _ = session.chunk_tx.try_send(Vec::new());
    }
}

pub async fn stop_rx() {
    if let Some(session) = rx_slot().lock().unwrap().take() {
        session.stop.store(true, Ordering::Relaxed);
        session.notify.notify_waiters();
    }
}

pub async fn stop_all() {
    stop_tx().await;
    stop_rx().await;
}
