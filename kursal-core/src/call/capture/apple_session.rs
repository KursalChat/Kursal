use super::apple_devices;
use super::apple_encoder::Encoder;
use super::{CaptureConfig, FrameSink, VideoFormat, keyframe_interval, quality_dims};
use crate::Result;
use crate::errors::KursalError;
use crate::sync::LockExt;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDeviceInput, AVCaptureOutput, AVCaptureSession,
    AVCaptureVideoDataOutput, AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_av_foundation::{AVCaptureSessionPreset640x480, AVCaptureSessionPreset1280x720};
use objc2_core_media::{CMSampleBuffer, CMVideoFormatDescriptionGetDimensions};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use std::sync::atomic::Ordering;
use std::sync::{Mutex as StdMutex, OnceLock};

struct Shared {
    encoder: StdMutex<Option<Encoder>>,
    sink: StdMutex<Option<FrameSink>>,
    bitrate: u32,
    keyframe_interval: u32,
    pending_key: std::sync::atomic::AtomicBool,
    failures: std::sync::atomic::AtomicU32,
    logged_failure: std::sync::atomic::AtomicBool,
}

const MAX_CONSECUTIVE_ENCODE_FAILURES: u32 = 60;

fn sample_dimensions(sample: &CMSampleBuffer) -> Option<(u16, u16)> {
    let desc = unsafe { sample.format_description() }?;
    let dims = unsafe { CMVideoFormatDescriptionGetDimensions(&desc) };
    let width = u16::try_from(dims.width).ok()?;
    let height = u16::try_from(dims.height).ok()?;
    if width < 2 || height < 2 {
        return None;
    }
    Some((width - width % 2, height - height % 2))
}

impl Shared {
    fn encoder_for(&self, dims: (u16, u16)) -> bool {
        let mut guard = self.encoder.lock_recover();
        if guard.as_ref().is_some_and(|e| e.dimensions() == dims) {
            return true;
        }
        let Some(sink) = self.sink.lock_recover().clone() else {
            return false;
        };
        match Encoder::new(dims.0, dims.1, self.bitrate, self.keyframe_interval, sink) {
            Ok(encoder) => {
                log::info!("[call] video encoder sized to {}x{}", dims.0, dims.1);
                if self.pending_key.swap(false, Ordering::Relaxed) {
                    encoder.request_keyframe();
                }
                *guard = Some(encoder);
                true
            }
            Err(e) => {
                if !self.logged_failure.swap(true, Ordering::Relaxed) {
                    log::error!("[call] video encoder create failed: {e}");
                }
                false
            }
        }
    }
}

pub struct DelegateIvars {
    shared: std::sync::Arc<Shared>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "KursalVideoDelegate"]
    #[ivars = DelegateIvars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for Delegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn did_output_sample_buffer(
            &self,
            _output: &AVCaptureOutput,
            sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            let shared = &self.ivars().shared;
            let Some(image) = (unsafe { sample_buffer.image_buffer() }) else {
                return;
            };
            let Some(dims) = sample_dimensions(sample_buffer) else {
                return;
            };
            if !shared.encoder_for(dims) {
                return;
            }
            let timestamp = unsafe { sample_buffer.presentation_time_stamp() };
            let duration = unsafe { sample_buffer.duration() };

            let result = {
                let mut guard = shared.encoder.lock_recover();
                match guard.as_mut() {
                    Some(encoder) => encoder.encode(&image, timestamp, duration),
                    None => return,
                }
            };
            match result {
                Ok(()) => {
                    shared.failures.store(0, Ordering::Relaxed);
                }
                Err(e) => {
                    if !shared.logged_failure.swap(true, Ordering::Relaxed) {
                        log::error!("[call] video encode failed: {e}");
                    }
                    if shared.failures.fetch_add(1, Ordering::Relaxed) + 1
                        >= MAX_CONSECUTIVE_ENCODE_FAILURES
                    {
                        super::report_failure();
                    }
                }
            }
        }
    }
);

impl Delegate {
    fn new(shared: std::sync::Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { shared });
        unsafe { msg_send![super(this), init] }
    }
}

struct SessionHandle {
    session: Retained<AVCaptureSession>,
    output: Retained<AVCaptureVideoDataOutput>,
    _delegate: Retained<Delegate>,
    _queue: DispatchRetained<DispatchQueue>,
    shared: std::sync::Arc<Shared>,
}

unsafe impl Send for SessionHandle {}

fn handle_slot() -> &'static StdMutex<Option<SessionHandle>> {
    static S: OnceLock<StdMutex<Option<SessionHandle>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

fn preset_for(height: u16) -> (&'static NSString, u16, u16) {
    unsafe {
        if height >= 720 {
            (AVCaptureSessionPreset1280x720, 1280, 720)
        } else {
            (AVCaptureSessionPreset640x480, 640, 480)
        }
    }
}

pub fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    let (_, budget_h, bitrate) = quality_dims(config.quality);
    let device = apple_devices::find(config.camera_id.as_deref())
        .ok_or_else(|| KursalError::Misc(anyhow::anyhow!("no camera available")))?;
    let device_id = unsafe { device.uniqueID() }.to_string();
    super::set_selected_camera(Some(device_id.clone()));

    unsafe {
        let session = AVCaptureSession::new();
        session.beginConfiguration();

        let (preset, mut width, mut height) = preset_for(budget_h);
        if session.canSetSessionPreset(preset) {
            session.setSessionPreset(preset);
        }

        let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
            .map_err(|e| KursalError::Misc(anyhow::anyhow!("camera input: {e}")))?;
        if !session.canAddInput(&input) {
            session.commitConfiguration();
            return Err(KursalError::Misc(anyhow::anyhow!(
                "camera input rejected by session"
            )));
        }
        session.addInput(&input);

        let output = AVCaptureVideoDataOutput::new();
        output.setAlwaysDiscardsLateVideoFrames(true);
        if !session.canAddOutput(&output) {
            session.commitConfiguration();
            return Err(KursalError::Misc(anyhow::anyhow!(
                "video output rejected by session"
            )));
        }
        session.addOutput(&output);

        session.commitConfiguration();

        let shared = std::sync::Arc::new(Shared {
            encoder: StdMutex::new(None),
            sink: StdMutex::new(Some(sink)),
            bitrate,
            keyframe_interval: keyframe_interval(),
            pending_key: std::sync::atomic::AtomicBool::new(false),
            failures: std::sync::atomic::AtomicU32::new(0),
            logged_failure: std::sync::atomic::AtomicBool::new(false),
        });

        let queue = DispatchQueue::new("chat.kursal.video", None);
        let delegate = Delegate::new(shared.clone());
        let proto = ProtocolObject::from_ref(&*delegate);
        output.setSampleBufferDelegate_queue(Some(proto), Some(&queue));

        if apply_rotation(&output) {
            std::mem::swap(&mut width, &mut height);
        }

        session.startRunning();

        *handle_slot().lock_recover() = Some(SessionHandle {
            session,
            output,
            _delegate: delegate,
            _queue: queue,
            shared,
        });

        Ok(VideoFormat {
            codec: "avc1.42E01F".to_string(),
            width,
            height,
            camera_id: Some(device_id),
        })
    }
}

pub fn stop() {
    if let Some(handle) = handle_slot().lock_recover().take() {
        unsafe {
            handle
                .output
                .setSampleBufferDelegate_queue(None, None::<&DispatchQueue>);
            handle.session.stopRunning();
        }
        handle.shared.sink.lock_recover().take();
        handle.shared.encoder.lock_recover().take();
    }
}

pub fn set_bitrate(bps: u32) {
    let shared = handle_slot()
        .lock_recover()
        .as_ref()
        .map(|h| h.shared.clone());
    if let Some(shared) = shared
        && let Some(encoder) = shared.encoder.lock_recover().as_ref()
    {
        encoder.set_bitrate(bps);
    }
}

pub fn request_keyframe() {
    let shared = handle_slot()
        .lock_recover()
        .as_ref()
        .map(|h| h.shared.clone());
    let Some(shared) = shared else {
        return;
    };
    match shared.encoder.lock_recover().as_ref() {
        Some(encoder) => encoder.request_keyframe(),
        None => shared.pending_key.store(true, Ordering::Relaxed),
    }
}

pub fn refresh_rotation() {
    let output = handle_slot()
        .lock_recover()
        .as_ref()
        .map(|h| h.output.clone());
    if let Some(output) = output {
        unsafe { apply_rotation(&output) };
    }
}

unsafe fn apply_rotation(output: &AVCaptureVideoDataOutput) -> bool {
    unsafe {
        let Some(media_type) = AVMediaTypeVideo else {
            return false;
        };
        let Some(connection) = output.connectionWithMediaType(media_type) else {
            return false;
        };
        let angle = super::orientation::capture_angle();
        if !connection.isVideoRotationAngleSupported(angle) {
            return false;
        }
        connection.setVideoRotationAngle(angle);
        angle == 90.0 || angle == 270.0
    }
}
