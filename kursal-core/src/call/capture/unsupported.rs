use super::{CaptureConfig, FrameSink, VideoFormat};
use crate::Result;
use crate::dto::CameraInfo;
use crate::errors::KursalError;

pub(super) fn start(_config: &CaptureConfig, _sink: FrameSink) -> Result<VideoFormat> {
    Err(KursalError::Misc(anyhow::anyhow!(
        "video capture is not supported on this platform"
    )))
}

pub(super) fn stop() {}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    Vec::new()
}

pub(super) fn request_keyframe() {}

pub(super) fn set_bitrate(_bps: u32) {}

pub(super) fn refresh_rotation() {}
