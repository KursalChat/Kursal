use super::yuv::I420;
use crate::Result;
use crate::errors::KursalError;
use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod, Profile,
    RateControlMode, UsageType,
};
use openh264::formats::YUVSource;
use openh264_sys2::{ENCODER_OPTION_BITRATE, SBitrateInfo, SPATIAL_LAYER_ALL};
use std::ffi::c_int;
use std::ptr::addr_of_mut;

impl YUVSource for I420 {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn strides(&self) -> (usize, usize, usize) {
        let chroma = self.chroma_width();
        (self.width, chroma, chroma)
    }

    fn y(&self) -> &[u8] {
        &self.y
    }

    fn u(&self) -> &[u8] {
        &self.u
    }

    fn v(&self) -> &[u8] {
        &self.v
    }
}

fn config(bitrate: u32, keyframe_interval: u32) -> EncoderConfig {
    EncoderConfig::new()
        .usage_type(UsageType::CameraVideoRealTime)
        .rate_control_mode(RateControlMode::Bitrate)
        .bitrate(BitRate::from_bps(bitrate))
        .max_frame_rate(FrameRate::from_hz(30.0))
        .intra_frame_period(IntraFramePeriod::from_num_frames(keyframe_interval))
        .profile(Profile::Baseline)
}

pub struct SoftwareEncoder {
    inner: Encoder,
}

impl SoftwareEncoder {
    pub fn new(bitrate: u32, keyframe_interval: u32) -> Result<Self> {
        let inner = Encoder::with_api_config(
            openh264::OpenH264API::from_source(),
            config(bitrate, keyframe_interval),
        )
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("openh264 init: {e}")))?;
        Ok(Self { inner })
    }

    pub fn encode(&mut self, frame: &I420) -> Result<(Vec<u8>, bool)> {
        let bitstream = self
            .inner
            .encode(frame)
            .map_err(|e| KursalError::Misc(anyhow::anyhow!("openh264 encode: {e}")))?;
        let keyframe = matches!(bitstream.frame_type(), FrameType::IDR | FrameType::I);
        Ok((bitstream.to_vec(), keyframe))
    }

    pub fn force_keyframe(&mut self) {
        self.inner.force_intra_frame();
    }

    pub fn set_bitrate(&mut self, bps: u32) {
        let mut info = SBitrateInfo {
            iLayer: SPATIAL_LAYER_ALL,
            iBitrate: c_int::try_from(bps).unwrap_or(c_int::MAX),
        };
        let status = unsafe {
            self.inner
                .raw_api()
                .set_option(ENCODER_OPTION_BITRATE, addr_of_mut!(info).cast())
        };
        if status != 0 {
            log::warn!("[call] bitrate change rejected: {status}");
        }
    }
}
