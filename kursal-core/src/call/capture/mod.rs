use crate::Result;
use crate::dto::CameraInfo;
use crate::sync::LockExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple_devices;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple_encoder;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple_session;
#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod openh264_encoder;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod orientation;
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
)))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
pub mod yuv;

#[cfg(target_os = "android")]
use android as backend;
#[cfg(target_os = "ios")]
use ios as backend;
#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
)))]
use unsupported as backend;
#[cfg(target_os = "windows")]
use windows as backend;

const CHUNK_HEADER_BYTES: usize = 10;
const KEYFRAME_INTERVAL_FRAMES: u32 = 90;
const MAX_CHUNK_PAYLOAD_BYTES: usize =
    crate::call::video::MAX_VIDEO_FRAME_BYTES - CHUNK_HEADER_BYTES;
const MIN_BITRATE_BPS: u32 = 150_000;
const RECOVERY_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub quality: u32,
    pub camera_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoFormat {
    pub codec: String,
    pub width: u16,
    pub height: u16,
    pub camera_id: Option<String>,
}

pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub timestamp_us: u64,
}

pub type FrameSink = Arc<dyn Fn(EncodedFrame) + Send + Sync>;

pub fn quality_dims(quality: u32) -> (u16, u16, u32) {
    match quality {
        360 => (640, 360, 600_000),
        720 => (1280, 720, 1_500_000),
        _ => (854, 480, 900_000),
    }
}

pub fn keyframe_interval() -> u32 {
    KEYFRAME_INTERVAL_FRAMES
}

pub fn pack_chunk(frame: &EncodedFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(CHUNK_HEADER_BYTES + frame.data.len());
    out.push(0);
    out.push(u8::from(frame.keyframe));
    out.extend_from_slice(&frame.timestamp_us.to_be_bytes());
    out.extend_from_slice(&frame.data);
    out
}

fn format_slot() -> &'static StdMutex<Option<VideoFormat>> {
    static F: OnceLock<StdMutex<Option<VideoFormat>>> = OnceLock::new();
    F.get_or_init(|| StdMutex::new(None))
}

fn runtime_slot() -> &'static StdMutex<Option<tokio::runtime::Handle>> {
    static R: OnceLock<StdMutex<Option<tokio::runtime::Handle>>> = OnceLock::new();
    R.get_or_init(|| StdMutex::new(None))
}

pub fn set_runtime(handle: tokio::runtime::Handle) {
    *runtime_slot().lock_recover() = Some(handle);
}

fn off_thread(f: impl FnOnce() + Send + 'static) {
    match runtime_slot().lock_recover().clone() {
        Some(handle) => {
            handle.spawn_blocking(f);
        }
        None => {
            std::thread::spawn(f);
        }
    }
}

static FAILED: AtomicBool = AtomicBool::new(false);

fn failure_notify() -> &'static Notify {
    static N: OnceLock<Notify> = OnceLock::new();
    N.get_or_init(Notify::new)
}

pub(super) fn report_failure() {
    *format_slot().lock_recover() = None;
    FAILED.store(true, Ordering::Relaxed);
    failure_notify().notify_waiters();
}

pub async fn failed() -> bool {
    loop {
        let notified = failure_notify().notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        if FAILED.swap(false, Ordering::Relaxed) {
            return true;
        }
        if !is_active() {
            return false;
        }
        notified.await;
    }
}

struct Bitrate {
    target: u32,
    current: u32,
    changed_at: Instant,
}

fn bitrate_slot() -> &'static StdMutex<Option<Bitrate>> {
    static B: OnceLock<StdMutex<Option<Bitrate>>> = OnceLock::new();
    B.get_or_init(|| StdMutex::new(None))
}

pub fn on_congestion() {
    let next = {
        let mut guard = bitrate_slot().lock_recover();
        let Some(state) = guard.as_mut() else {
            return;
        };
        let next = (state.current / 2).max(MIN_BITRATE_BPS);
        if next == state.current {
            return;
        }
        state.current = next;
        state.changed_at = Instant::now();
        next
    };
    log::warn!("[call] video congested, bitrate down to {next} bps");
    off_thread(move || {
        backend::set_bitrate(next);
        backend::request_keyframe();
    });
}

fn recover_bitrate() {
    let next = {
        let mut guard = bitrate_slot().lock_recover();
        let Some(state) = guard.as_mut() else {
            return;
        };
        if state.current >= state.target || state.changed_at.elapsed() < RECOVERY_INTERVAL {
            return;
        }
        state.current = state.target.min(state.current + state.current / 10);
        state.changed_at = Instant::now();
        state.current
    };
    off_thread(move || backend::set_bitrate(next));
}

pub fn start(config: CaptureConfig) -> Result<VideoFormat> {
    stop();
    let (_, _, bitrate) = quality_dims(config.quality);
    *bitrate_slot().lock_recover() = Some(Bitrate {
        target: bitrate,
        current: bitrate,
        changed_at: Instant::now(),
    });

    let sink: FrameSink = Arc::new(|frame| {
        if frame.data.len() > MAX_CHUNK_PAYLOAD_BYTES {
            log::warn!(
                "[call] video frame over the wire cap: {} bytes",
                frame.data.len()
            );
            on_congestion();
            return;
        }
        recover_bitrate();
        let chunk = pack_chunk(&frame);
        crate::call::video::forward_local(&chunk);
        crate::call::video::send_chunk(chunk);
    });

    let format = backend::start(&config, sink).inspect_err(|e| {
        log::error!("[call] video capture failed to start: {e}");
        *bitrate_slot().lock_recover() = None;
    })?;
    log::info!(
        "[call] video capture started: {}x{} {}",
        format.width,
        format.height,
        format.codec
    );
    *format_slot().lock_recover() = Some(format.clone());
    Ok(format)
}

pub fn stop() {
    backend::stop();
    *format_slot().lock_recover() = None;
    *bitrate_slot().lock_recover() = None;
    FAILED.store(false, Ordering::Relaxed);
    failure_notify().notify_waiters();
}

pub fn is_active() -> bool {
    format_slot().lock_recover().is_some()
}

pub fn list_cameras() -> Vec<CameraInfo> {
    backend::list_cameras()
}

pub fn request_keyframe() {
    backend::request_keyframe();
}

pub fn refresh_rotation() {
    backend::refresh_rotation();
}

fn camera_slot() -> &'static StdMutex<Option<String>> {
    static C: OnceLock<StdMutex<Option<String>>> = OnceLock::new();
    C.get_or_init(|| StdMutex::new(None))
}

pub fn selected_camera() -> Option<String> {
    camera_slot().lock_recover().clone()
}

pub fn set_selected_camera(id: Option<String>) {
    *camera_slot().lock_recover() = id;
}
