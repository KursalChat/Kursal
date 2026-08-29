use super::{CaptureConfig, FrameSink, VideoFormat, apple_devices, apple_session};
use crate::Result;
use crate::dto::CameraInfo;

pub(super) fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    apple_session::start(config, sink)
}

pub(super) fn stop() {
    apple_session::stop();
}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    apple_devices::list()
}

pub(super) fn request_keyframe() {
    apple_session::request_keyframe();
}

pub(super) fn set_bitrate(bps: u32) {
    apple_session::set_bitrate(bps);
}

pub(super) fn refresh_rotation() {
    apple_session::refresh_rotation();
}
