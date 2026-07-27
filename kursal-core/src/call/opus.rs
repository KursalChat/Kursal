use crate::MapKursalResult;
use crate::{KursalError, Result};
use audiopus::{
    Application, Bitrate, Channels, SampleRate,
    coder::{Decoder, Encoder},
};

fn sr(rate: u32) -> Result<SampleRate> {
    match rate {
        8000 => Ok(SampleRate::Hz8000),
        12000 => Ok(SampleRate::Hz12000),
        16000 => Ok(SampleRate::Hz16000),
        24000 => Ok(SampleRate::Hz24000),
        48000 => Ok(SampleRate::Hz48000),
        _ => Err(KursalError::Crypto("unsupported sample rate".into())),
    }
}

pub struct OpusCodec {
    enc: Encoder,
    dec: Decoder,
}

impl OpusCodec {
    pub fn new(sample_rate: u32) -> Result<Self> {
        let rate = sr(sample_rate)?;
        let mut enc =
            Encoder::new(rate, Channels::Mono, Application::Voip).ok_kursal(KursalError::Crypto)?;
        enc.set_bitrate(Bitrate::BitsPerSecond(24000))
            .ok_kursal(KursalError::Crypto)?;
        let dec = Decoder::new(rate, Channels::Mono).ok_kursal(KursalError::Crypto)?;
        Ok(Self { enc, dec })
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        let mut out = vec![0u8; 4000];
        let n = self
            .enc
            .encode(pcm, &mut out)
            .ok_kursal(KursalError::Crypto)?;
        out.truncate(n);
        Ok(out)
    }

    pub fn decode(&mut self, packet: Option<&[u8]>, out: &mut [i16]) -> Result<usize> {
        self.dec
            .decode(packet, out, false)
            .ok_kursal(KursalError::Crypto)
    }
}
