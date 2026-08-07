use crate::{KursalError, Result};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use webrtc_audio_processing::config::{
    EchoCanceller, HighPassFilter, NoiseSuppression, NoiseSuppressionLevel,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use webrtc_audio_processing::{Config, Processor};

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub struct Apm {
    processor: Processor,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Apm {
    pub fn new(sample_rate: u32) -> Result<Self> {
        let processor = Processor::new(sample_rate)
            .map_err(|e| KursalError::Network(format!("apm init: {e:?}")))?;
        processor.set_config(Config {
            echo_canceller: Some(EchoCanceller::default()),
            noise_suppression: Some(NoiseSuppression {
                level: NoiseSuppressionLevel::Moderate,
                analyze_linear_aec_output: false,
            }),
            high_pass_filter: Some(HighPassFilter::default()),
            ..Default::default()
        });
        Ok(Self { processor })
    }

    pub fn process_capture(&self, ch: &mut [f32]) {
        let _ = self.processor.process_capture_frame([ch]);
    }

    pub fn process_render(&self, ch: &mut [f32]) {
        let _ = self.processor.process_render_frame([ch]);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub struct Apm;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl Apm {
    pub fn new(_sample_rate: u32) -> Result<Self> {
        Err(KursalError::Network(
            "audio processing not available on this platform".into(),
        ))
    }

    pub fn process_capture(&self, _ch: &mut [f32]) {}

    pub fn process_render(&self, _ch: &mut [f32]) {}
}
