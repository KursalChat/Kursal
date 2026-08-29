use super::openh264_encoder::SoftwareEncoder;
use super::yuv::{I420, nv12_to_i420};
use super::{CaptureConfig, EncodedFrame, FrameSink, VideoFormat, keyframe_interval, quality_dims};
use crate::Result;
use crate::dto::CameraInfo;
use crate::errors::KursalError;
use crate::sync::LockExt;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::PWSTR;

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

fn mf_err(what: &str, e: windows::core::Error) -> KursalError {
    KursalError::Misc(anyhow::anyhow!("media foundation {what}: {e}"))
}

fn startup() -> Result<()> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|e| mf_err("startup", e))?;
    }
    Ok(())
}

struct DeviceList {
    activates: Vec<IMFActivate>,
}

fn enumerate() -> Result<DeviceList> {
    unsafe {
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|e| mf_err("attributes", e))?;
        let attributes = attributes
            .ok_or_else(|| KursalError::Misc(anyhow::anyhow!("media foundation attributes")))?;
        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(|e| mf_err("source type", e))?;

        let mut raw: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count: u32 = 0;
        MFEnumDeviceSources(&attributes, &mut raw, &mut count)
            .map_err(|e| mf_err("enum devices", e))?;

        let mut activates = Vec::with_capacity(count as usize);
        for index in 0..count as usize {
            if let Some(activate) = (*raw.add(index)).take() {
                activates.push(activate);
            }
        }
        windows::Win32::System::Com::CoTaskMemFree(Some(raw as *const _));
        Ok(DeviceList { activates })
    }
}

unsafe fn string_attribute(activate: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
    unsafe {
        let mut buffer = PWSTR::null();
        let mut len: u32 = 0;
        activate
            .GetAllocatedString(key, &mut buffer, &mut len)
            .ok()?;
        if buffer.is_null() {
            return None;
        }
        let text = buffer.to_string().ok();
        windows::Win32::System::Com::CoTaskMemFree(Some(buffer.0 as *const _));
        text
    }
}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    if startup().is_err() {
        return Vec::new();
    }
    let Ok(devices) = enumerate() else {
        unsafe {
            let _ = MFShutdown();
        }
        return Vec::new();
    };

    let cameras = devices
        .activates
        .iter()
        .filter_map(|activate| unsafe {
            let id = string_attribute(
                activate,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            )?;
            let label = string_attribute(activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                .unwrap_or_else(|| id.clone());
            Some(CameraInfo {
                id,
                label,
                facing: None,
            })
        })
        .collect();

    unsafe {
        let _ = MFShutdown();
    }
    cameras
}

unsafe fn open_reader(camera_id: Option<&str>) -> Result<(IMFSourceReader, IMFActivate, String)> {
    unsafe {
        let devices = enumerate()?;
        let mut chosen: Option<(IMFActivate, String)> = None;
        for activate in devices.activates {
            let Some(id) = string_attribute(
                &activate,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            ) else {
                continue;
            };
            let matches = camera_id.is_none_or(|wanted| wanted == id);
            if matches {
                chosen = Some((activate, id));
                break;
            }
        }

        let (activate, device_id) =
            chosen.ok_or_else(|| KursalError::Misc(anyhow::anyhow!("no camera available")))?;
        let source: IMFMediaSource = activate
            .ActivateObject()
            .map_err(|e| mf_err("activate camera", e))?;

        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|e| mf_err("reader attributes", e))?;
        let attributes =
            attributes.ok_or_else(|| KursalError::Misc(anyhow::anyhow!("reader attributes")))?;

        attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
            .map_err(|e| mf_err("advanced processing", e))?;

        let reader = MFCreateSourceReaderFromMediaSource(&source, &attributes).map_err(|e| {
            let _ = activate.ShutdownObject();
            mf_err("source reader", e)
        })?;
        Ok((reader, activate, device_id))
    }
}

unsafe fn configure(
    reader: &IMFSourceReader,
    width: u16,
    height: u16,
) -> Result<(u16, u16, usize)> {
    unsafe {
        let media_type = MFCreateMediaType().map_err(|e| mf_err("media type", e))?;
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| mf_err("major type", e))?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
            .map_err(|e| mf_err("subtype", e))?;
        let packed = (u64::from(width) << 32) | u64::from(height);
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, packed)
            .map_err(|e| mf_err("frame size", e))?;

        reader
            .SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                None,
                &media_type,
            )
            .map_err(|e| mf_err("set media type", e))?;

        let actual = reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .map_err(|e| mf_err("get media type", e))?;
        let size = actual
            .GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|e| mf_err("read frame size", e))?;
        let actual_w = u16::try_from(size >> 32).unwrap_or(width);
        let actual_h = u16::try_from(size & 0xFFFF_FFFF).unwrap_or(height);

        let stride = actual
            .GetUINT32(&MF_MT_DEFAULT_STRIDE)
            .ok()
            .map(|v| v as i32)
            .filter(|v| *v > 0)
            .map_or(0, |v| v as usize);

        Ok((actual_w - actual_w % 2, actual_h - actual_h % 2, stride))
    }
}

pub(super) fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    let (budget_w, budget_h, bitrate) = quality_dims(config.quality);
    startup()?;

    let (reader, activate, device_id, width, height, stride) = unsafe {
        let (reader, activate, device_id) =
            open_reader(config.camera_id.as_deref()).inspect_err(|_| {
                let _ = MFShutdown();
            })?;
        let (width, height, stride) = configure(&reader, budget_w, budget_h).inspect_err(|_| {
            let _ = activate.ShutdownObject();
            let _ = MFShutdown();
        })?;
        (reader, activate, device_id, width, height, stride)
    };

    let mut encoder =
        SoftwareEncoder::new(bitrate, keyframe_interval()).inspect_err(|_| unsafe {
            let _ = activate.ShutdownObject();
            let _ = MFShutdown();
        })?;
    let control = Arc::new(Control {
        stop: AtomicBool::new(false),
        force_key: AtomicBool::new(false),
        bitrate: AtomicU32::new(bitrate),
    });

    let source = SendSource(reader, activate);
    let thread_control = control.clone();
    let thread = std::thread::Builder::new()
        .name("kursal-video".to_string())
        .spawn(move || {
            let source = source;
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }

            let mut scratch = I420::new(usize::from(width), usize::from(height));
            let mut applied_bitrate = thread_control.bitrate.load(Ordering::Relaxed);
            let start = std::time::Instant::now();

            while !thread_control.stop.load(Ordering::Relaxed) {
                let mut stream_flags: u32 = 0;
                let mut sample: Option<IMFSample> = None;
                let read = unsafe {
                    source.0.ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut stream_flags),
                        None,
                        Some(&mut sample),
                    )
                };
                if read.is_err() {
                    log::error!("[call] media foundation read failed");
                    break;
                }
                if stream_flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(sample) = sample else {
                    continue;
                };

                let filled = unsafe { copy_sample(&sample, width, height, stride, &mut scratch) };
                if !filled {
                    continue;
                }

                let wanted_bitrate = thread_control.bitrate.load(Ordering::Relaxed);
                if wanted_bitrate != applied_bitrate {
                    encoder.set_bitrate(wanted_bitrate);
                    applied_bitrate = wanted_bitrate;
                }
                if thread_control.force_key.swap(false, Ordering::Relaxed) {
                    encoder.force_keyframe();
                }

                match encoder.encode(&scratch) {
                    Ok((data, keyframe)) if !data.is_empty() => sink(EncodedFrame {
                        data,
                        keyframe,
                        timestamp_us: u64::try_from(start.elapsed().as_micros()).unwrap_or(0),
                        rotation: 0,
                    }),
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[call] video encode failed: {e}");
                        break;
                    }
                }
            }

            unsafe {
                let _ = source.1.ShutdownObject();
                let _ = MFShutdown();
                CoUninitialize();
            }
            if !thread_control.stop.load(Ordering::Relaxed) {
                super::report_failure();
            }
        })
        .map_err(|e| unsafe {
            let _ = MFShutdown();
            KursalError::Misc(anyhow::anyhow!("video thread spawn: {e}"))
        })?;

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

struct SendSource(IMFSourceReader, IMFActivate);
unsafe impl Send for SendSource {}

unsafe fn copy_sample(
    sample: &IMFSample,
    width: u16,
    height: u16,
    stride: usize,
    dst: &mut I420,
) -> bool {
    unsafe {
        let Ok(buffer) = sample.ConvertToContiguousBuffer() else {
            return false;
        };
        let mut data: *mut u8 = std::ptr::null_mut();
        let mut length: u32 = 0;
        if buffer.Lock(&mut data, None, Some(&mut length)).is_err() || data.is_null() {
            return false;
        }

        let width = usize::from(width);
        let height = usize::from(height);
        let row_bytes = if stride == 0 { width } else { stride };
        let y_len = row_bytes * height;
        let bytes = std::slice::from_raw_parts(data, length as usize);
        let filled = bytes.len() >= y_len
            && nv12_to_i420(&bytes[..y_len], &bytes[y_len..], width, height, stride, dst);

        let _ = buffer.Unlock();
        filled
    }
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
