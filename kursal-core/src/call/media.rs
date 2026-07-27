use crate::Result;
use crate::api::AppEvent;
use crate::call::apm::Apm;
use crate::call::audio::{self, AudioHandle, to_i16_sample};
use crate::call::crypto::CallKeys;
use crate::call::frame::{decode_frame, encode_frame};
use crate::call::jitter::{JitterBuffer, JitterOut};
use crate::call::opus::OpusCodec;
use crate::crypto::stream::{stream_decrypt_aad, stream_encrypt_aad};
use crate::messaging::enums::MessageId;
use crate::network::swarm::SwarmCommand;
use crate::storage::SharedDatabase;
use futures::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use libp2p::{PeerId, Stream};
use ringbuf::traits::{Consumer, Producer};
use ringbuf::{HeapCons, HeapProd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{Mutex, Notify, mpsc, oneshot};

const MAX_FRAME_BYTES: usize = 8192;
const JITTER_FRAMES: usize = 3;

fn frame_samples(sample_rate: u32) -> usize {
    (sample_rate / 50) as usize
}

fn apm_frame(sample_rate: u32) -> usize {
    (sample_rate / 100) as usize
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

struct Session {
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
    audio: AudioHandle,
}

fn session_slot() -> &'static Mutex<Option<Session>> {
    static S: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

#[allow(clippy::too_many_arguments)]
pub fn establish(
    is_caller: bool,
    peer_id: PeerId,
    keys: CallKeys,
    cmd_tx: mpsc::Sender<SwarmCommand>,
    app_event_tx: mpsc::Sender<AppEvent>,
    sample_rate: u32,
    call_id: MessageId,
    db: SharedDatabase,
) {
    tokio::task::spawn_local(async move {
        let stream = if is_caller {
            open_outgoing(peer_id, &cmd_tx).await
        } else {
            await_incoming(peer_id).await
        };
        let Some(stream) = stream else {
            log::warn!("[call] media: failed to establish stream");
            let _ = crate::call::manager::media_failed(call_id, db, &cmd_tx, &app_event_tx).await;
            return;
        };
        match start_session(keys, stream, app_event_tx.clone(), sample_rate) {
            Ok(session) => {
                *session_slot().lock().await = Some(session);
                log::info!("[call] media session started");
            }
            Err(e) => {
                log::error!("[call] media session error: {e}");
                let _ =
                    crate::call::manager::media_failed(call_id, db, &cmd_tx, &app_event_tx).await;
            }
        }
    });
}

pub async fn stop() {
    if let Some(session) = session_slot().lock().await.take() {
        session.stop.store(true, Ordering::Relaxed);
        session.notify.notify_waiters();
    }
}

pub async fn set_muted(muted: bool) {
    if let Some(session) = session_slot().lock().await.as_ref() {
        session.audio.set_muted(muted);
    }
}

pub async fn set_deafened(deafened: bool) {
    if let Some(session) = session_slot().lock().await.as_ref() {
        session.audio.set_deafened(deafened);
    }
}

pub async fn voice_state() -> Option<(bool, bool)> {
    session_slot()
        .lock()
        .await
        .as_ref()
        .map(|s| (s.audio.is_muted(), s.audio.is_deafened()))
}

async fn open_outgoing(peer_id: PeerId, cmd_tx: &mpsc::Sender<SwarmCommand>) -> Option<Stream> {
    let (reply, reply_rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::OpenCallStream { peer_id, reply })
        .await
        .ok()?;
    reply_rx.await.ok().flatten()
}

async fn await_incoming(peer_id: PeerId) -> Option<Stream> {
    let mut rx = incoming_channel().1.lock().await;
    let mut candidate: Option<Stream> = None;
    while let Ok((pid, stream)) = rx.try_recv() {
        if pid == peer_id {
            candidate = Some(stream);
        }
    }
    if candidate.is_some() {
        return candidate;
    }
    loop {
        match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some((pid, stream))) if pid == peer_id => return Some(stream),
            Ok(Some(_)) => continue,
            _ => return None,
        }
    }
}

fn start_session(
    keys: CallKeys,
    stream: Stream,
    app_event_tx: mpsc::Sender<AppEvent>,
    sample_rate: u32,
) -> Result<Session> {
    let (audio, speaker_prod, mic_cons) = audio::start(sample_rate)?;
    let stop = Arc::new(AtomicBool::new(false));
    let notify = Arc::new(Notify::new());
    let (read_half, write_half) = stream.split();
    let jitter = Arc::new(Mutex::new(JitterBuffer::new(JITTER_FRAMES)));
    let apm = Apm::new(sample_rate).ok().map(Arc::new);

    spawn_tx(
        write_half,
        mic_cons,
        keys.tx,
        stop.clone(),
        app_event_tx,
        apm.clone(),
        sample_rate,
    );
    spawn_reader(
        read_half,
        jitter.clone(),
        keys.rx,
        stop.clone(),
        notify.clone(),
    );
    spawn_player(speaker_prod, jitter, stop.clone(), apm, sample_rate);

    Ok(Session {
        stop,
        notify,
        audio,
    })
}

fn spawn_tx(
    mut write_half: WriteHalf<Stream>,
    mut mic_cons: HeapCons<i16>,
    tx_key: [u8; 32],
    stop: Arc<AtomicBool>,
    app_event_tx: mpsc::Sender<AppEvent>,
    apm: Option<Arc<Apm>>,
    sample_rate: u32,
) {
    tokio::spawn(async move {
        let mut codec = match OpusCodec::new(sample_rate) {
            Ok(c) => c,
            Err(e) => {
                log::error!("[call] tx opus init failed: {e}");
                return;
            }
        };
        let apm_frame = apm_frame(sample_rate);
        let frame_samples = frame_samples(sample_rate);
        let mut raw: Vec<i16> = Vec::with_capacity(sample_rate as usize);
        let mut acc: Vec<i16> = Vec::with_capacity(frame_samples * 4);
        let mut f32buf = vec![0f32; apm_frame];
        let mut seq: u64 = 0;
        let mut level_tick: u8 = 0;
        while !stop.load(Ordering::Relaxed) {
            while let Some(s) = mic_cons.try_pop() {
                raw.push(s);
            }
            if raw.len() > sample_rate as usize {
                let cut = raw.len() - apm_frame;
                raw.drain(..cut);
            }
            while raw.len() >= apm_frame {
                let chunk: Vec<i16> = raw.drain(..apm_frame).collect();
                if let Some(apm) = &apm {
                    for (i, &s) in chunk.iter().enumerate() {
                        f32buf[i] = s as f32 / 32768.0;
                    }
                    apm.process_capture(&mut f32buf);
                    for &f in f32buf.iter() {
                        acc.push(to_i16_sample(f));
                    }
                } else {
                    acc.extend_from_slice(&chunk);
                }
            }
            while acc.len() >= frame_samples {
                let frame: Vec<i16> = acc.drain(..frame_samples).collect();
                level_tick = level_tick.wrapping_add(1);
                if level_tick.is_multiple_of(3) {
                    let sum: f32 = frame.iter().map(|&s| f32::from(s) * f32::from(s)).sum();
                    let rms = (sum / frame.len() as f32).sqrt() / 32768.0;
                    let _ = app_event_tx.try_send(AppEvent::CallLevel {
                        mic: rms,
                        remote: 0.0,
                    });
                }
                let pkt = match codec.encode(&frame) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let sealed = match stream_encrypt_aad(&tx_key, &pkt, &seq.to_be_bytes()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let wire = encode_frame(seq, &sealed);
                seq = seq.wrapping_add(1);
                let Ok(wire_len) = u32::try_from(wire.len()) else {
                    return;
                };
                let len = wire_len.to_be_bytes();
                if write_half.write_all(&len).await.is_err() {
                    return;
                }
                if write_half.write_all(&wire).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
}

async fn read_one(read_half: &mut ReadHalf<Stream>, rx_key: &[u8; 32]) -> Option<(u64, Vec<u8>)> {
    let mut len_buf = [0u8; 4];
    read_half.read_exact(&mut len_buf).await.ok()?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_BYTES {
        return None;
    }
    let mut buf = vec![0u8; len];
    read_half.read_exact(&mut buf).await.ok()?;
    let (seq, cipher) = decode_frame(&buf).ok()?;
    let pkt = stream_decrypt_aad(rx_key, cipher, &seq.to_be_bytes()).ok()?;
    Some((seq, pkt))
}

pub(crate) struct ReplayWindow {
    highest: u64,
    bitmap: u64,
    started: bool,
}

impl ReplayWindow {
    pub(crate) fn new() -> Self {
        Self {
            highest: 0,
            bitmap: 0,
            started: false,
        }
    }

    pub(crate) fn accept(&mut self, seq: u64) -> bool {
        if !self.started {
            self.started = true;
            self.highest = seq;
            self.bitmap = 1;
            return true;
        }
        if seq > self.highest {
            let shift = seq - self.highest;
            self.bitmap = if shift >= 64 {
                1
            } else {
                (self.bitmap << shift) | 1
            };
            self.highest = seq;
            return true;
        }
        let offset = self.highest - seq;
        if offset >= 64 {
            return false;
        }
        let mask = 1u64 << offset;
        if self.bitmap & mask != 0 {
            return false;
        }
        self.bitmap |= mask;
        true
    }
}

fn spawn_reader(
    mut read_half: ReadHalf<Stream>,
    jitter: Arc<Mutex<JitterBuffer>>,
    rx_key: [u8; 32],
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
) {
    tokio::spawn(async move {
        let mut replay = ReplayWindow::new();
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let frame = tokio::select! {
                _ = notify.notified() => break,
                r = read_one(&mut read_half, &rx_key) => r,
            };
            match frame {
                Some((seq, pkt)) => {
                    if replay.accept(seq) {
                        jitter.lock().await.push(seq, pkt);
                    }
                }
                None => break,
            }
        }
    });
}

fn spawn_player(
    mut speaker_prod: HeapProd<i16>,
    jitter: Arc<Mutex<JitterBuffer>>,
    stop: Arc<AtomicBool>,
    apm: Option<Arc<Apm>>,
    sample_rate: u32,
) {
    tokio::spawn(async move {
        let mut codec = match OpusCodec::new(sample_rate) {
            Ok(c) => c,
            Err(e) => {
                log::error!("[call] player opus init failed: {e}");
                return;
            }
        };
        let apm_frame = apm_frame(sample_rate);
        let frame_samples = frame_samples(sample_rate);
        let mut out = vec![0i16; frame_samples];
        let mut f32buf = vec![0f32; apm_frame];
        let mut ticker = tokio::time::interval(Duration::from_millis(20));
        while !stop.load(Ordering::Relaxed) {
            ticker.tick().await;
            let decoded = match jitter.lock().await.pop() {
                JitterOut::Frame(pkt) => codec.decode(Some(&pkt), &mut out).is_ok(),
                JitterOut::Conceal => codec.decode(None, &mut out).is_ok(),
                JitterOut::Empty => false,
            };
            if !decoded {
                continue;
            }
            match &apm {
                Some(apm) => {
                    let mut base = 0;
                    while base < frame_samples {
                        for i in 0..apm_frame {
                            f32buf[i] = out[base + i] as f32 / 32768.0;
                        }
                        apm.process_render(&mut f32buf);
                        for &f in f32buf.iter() {
                            let _ = speaker_prod.try_push(to_i16_sample(f));
                        }
                        base += apm_frame;
                    }
                }
                None => {
                    for &s in out.iter() {
                        let _ = speaker_prod.try_push(s);
                    }
                }
            }
        }
    });
}
