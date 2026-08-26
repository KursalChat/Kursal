use super::openh264_encoder::SoftwareEncoder;
use super::yuv::{I420, yuyv_to_i420};
use super::{CaptureConfig, EncodedFrame, FrameSink, VideoFormat, keyframe_interval, quality_dims};
use crate::Result;
use crate::dto::CameraInfo;
use crate::errors::KursalError;
use crate::sync::LockExt;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use v4l::buffer::Type;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::{Device, FourCC};

const FOURCC_MJPG: &[u8; 4] = b"MJPG";
const FOURCC_YUYV: &[u8; 4] = b"YUYV";
const BUFFER_COUNT: u32 = 4;

struct Control {
    stop: AtomicBool,
    force_key: AtomicBool,
    bitrate: AtomicU32,
}

struct Session {
    control: Arc<Control>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn session_slot() -> &'static StdMutex<Option<Session>> {
    static S: OnceLock<StdMutex<Option<Session>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    v4l::context::enum_devices()
        .into_iter()
        .filter_map(|node| {
            let path = node.path().to_string_lossy().to_string();

            let device = Device::with_path(node.path()).ok()?;
            let formats = Capture::enum_formats(&device).ok()?;
            if formats.is_empty() {
                return None;
            }
            Some(CameraInfo {
                label: node.name().unwrap_or_else(|| path.clone()),
                id: path,
                facing: None,
            })
        })
        .collect()
}

fn pick_device(camera_id: Option<&str>) -> Result<(Device, String)> {
    let mut cameras = list_cameras().into_iter();
    let chosen = match camera_id {
        Some(id) => cameras
            .find(|c| c.id == id)
            .ok_or_else(|| KursalError::Misc(anyhow::anyhow!("unknown camera {id}")))?,
        None => cameras
            .next()
            .ok_or_else(|| KursalError::Misc(anyhow::anyhow!("no camera available")))?,
    };
    let device = Device::with_path(&chosen.id)
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("camera {}: {e}", chosen.id)))?;
    Ok((device, chosen.id))
}

fn supports(device: &Device, fourcc: &[u8; 4]) -> bool {
    Capture::enum_formats(device)
        .map(|formats| formats.iter().any(|f| f.fourcc == FourCC::new(fourcc)))
        .unwrap_or(false)
}

fn negotiate(device: &Device, width: u16, height: u16) -> Result<(FourCC, u16, u16, usize)> {
    let wanted = if supports(device, FOURCC_YUYV) {
        FOURCC_YUYV
    } else if supports(device, FOURCC_MJPG) {
        FOURCC_MJPG
    } else {
        return Err(KursalError::Misc(anyhow::anyhow!(
            "camera offers no supported pixel format (need YUYV or MJPG)"
        )));
    };

    let mut format = Capture::format(device)
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("read camera format: {e}")))?;
    format.width = u32::from(width);
    format.height = u32::from(height);
    format.fourcc = FourCC::new(wanted);

    let applied = Capture::set_format(device, &format)
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("set camera format: {e}")))?;
    if applied.fourcc != FourCC::new(wanted) {
        return Err(KursalError::Misc(anyhow::anyhow!(
            "camera rejected the negotiated pixel format"
        )));
    }

    let actual_w = u16::try_from(applied.width).unwrap_or(width);
    let actual_h = u16::try_from(applied.height).unwrap_or(height);
    Ok((
        applied.fourcc,
        actual_w - actual_w % 2,
        actual_h - actual_h % 2,
        applied.stride as usize,
    ))
}

pub(super) fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    let (budget_w, budget_h, bitrate) = quality_dims(config.quality);
    let (device, device_id) = pick_device(config.camera_id.as_deref())?;
    let (fourcc, width, height, stride) = negotiate(&device, budget_w, budget_h)?;

    let mut encoder = SoftwareEncoder::new(bitrate, keyframe_interval())?;
    let decode_mjpeg = fourcc == FourCC::new(FOURCC_MJPG);

    let control = Arc::new(Control {
        stop: AtomicBool::new(false),
        force_key: AtomicBool::new(false),
        bitrate: AtomicU32::new(bitrate),
    });

    let thread_control = control.clone();
    let thread = std::thread::Builder::new()
        .name("kursal-video".to_string())
        .spawn(move || {
            let mut stream = match Stream::with_buffers(&device, Type::VideoCapture, BUFFER_COUNT) {
                Ok(stream) => stream,
                Err(e) => {
                    log::error!("[call] v4l2 stream failed: {e}");
                    super::report_failure();
                    return;
                }
            };

            let mut scratch = I420::new(usize::from(width), usize::from(height));
            let mut applied_bitrate = thread_control.bitrate.load(Ordering::Relaxed);
            let start = std::time::Instant::now();

            while !thread_control.stop.load(Ordering::Relaxed) {
                let (buffer, _meta) = match CaptureStream::next(&mut stream) {
                    Ok(frame) => frame,
                    Err(e) => {
                        log::error!("[call] v4l2 read failed: {e}");
                        break;
                    }
                };

                let timestamp_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(0);
                let force_key = thread_control.force_key.swap(false, Ordering::Relaxed);

                let wanted_bitrate = thread_control.bitrate.load(Ordering::Relaxed);
                if wanted_bitrate != applied_bitrate {
                    encoder.set_bitrate(wanted_bitrate);
                    applied_bitrate = wanted_bitrate;
                }
                if force_key {
                    encoder.force_keyframe();
                }

                let filled = if decode_mjpeg {
                    decode_jpeg_into(buffer, &mut scratch)
                } else {
                    yuyv_to_i420(
                        buffer,
                        usize::from(width),
                        usize::from(height),
                        stride,
                        &mut scratch,
                    )
                };
                if !filled {
                    continue;
                }

                let frame = match encoder.encode(&scratch) {
                    Ok((data, keyframe)) if !data.is_empty() => EncodedFrame {
                        data,
                        keyframe,
                        timestamp_us,
                    },
                    Ok(_) => continue,
                    Err(e) => {
                        log::error!("[call] video encode failed: {e}");
                        break;
                    }
                };

                sink(frame);
            }

            if !thread_control.stop.load(Ordering::Relaxed) {
                super::report_failure();
            }
        })
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("video thread: {e}")))?;

    super::set_selected_camera(Some(device_id.clone()));
    *session_slot().lock_recover() = Some(Session {
        control,
        thread: Some(thread),
    });

    Ok(VideoFormat {
        codec: "avc1.42E01F".to_string(),
        width,
        height,
        camera_id: Some(device_id),
    })
}

fn decode_jpeg_into(buffer: &[u8], dst: &mut I420) -> bool {
    let mut decoder =
        zune_jpeg::JpegDecoder::new(zune_jpeg::zune_core::bytestream::ZCursor::new(buffer));
    decoder.set_options(
        zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::YCbCr),
    );
    let Ok(pixels) = decoder.decode() else {
        return false;
    };
    let Some(info) = decoder.info() else {
        return false;
    };
    let (width, height) = (usize::from(info.width), usize::from(info.height));
    if width != dst.width || height != dst.height || pixels.len() < width * height * 3 {
        return false;
    }

    let chroma_w = dst.chroma_width();
    for row in 0..height {
        for col in 0..width {
            dst.y[row * width + col] = pixels[(row * width + col) * 3];
        }
    }
    for crow in 0..height.div_ceil(2) {
        for ccol in 0..chroma_w {
            let src = ((crow * 2) * width + ccol * 2) * 3;
            dst.u[crow * chroma_w + ccol] = pixels[src + 1];
            dst.v[crow * chroma_w + ccol] = pixels[src + 2];
        }
    }
    true
}

pub(super) fn stop() {
    let session = session_slot().lock_recover().take();
    if let Some(mut session) = session {
        session.control.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = session.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn request_keyframe() {
    if let Some(session) = session_slot().lock_recover().as_ref() {
        session.control.force_key.store(true, Ordering::Relaxed);
    }
}

pub(super) fn set_bitrate(bps: u32) {
    if let Some(session) = session_slot().lock_recover().as_ref() {
        session.control.bitrate.store(bps, Ordering::Relaxed);
    }
}

pub(super) fn refresh_rotation() {}
