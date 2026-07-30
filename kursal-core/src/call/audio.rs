use crate::{KursalError, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

type SharedProd = Arc<Mutex<HeapProd<i16>>>;
type SharedCons = Arc<Mutex<HeapCons<i16>>>;

#[derive(Default, Clone)]
struct DeviceSel {
    input: Option<String>,
    output: Option<String>,
}

static SELECTED: OnceLock<Mutex<DeviceSel>> = OnceLock::new();
static DEVICE_GEN: AtomicU64 = AtomicU64::new(0);
static AUDIO_THREAD: Mutex<Option<std::thread::Thread>> = Mutex::new(None);

fn wake_audio_thread() {
    if let Ok(t) = AUDIO_THREAD.lock()
        && let Some(t) = t.as_ref()
    {
        t.unpark();
    }
}

fn selected() -> &'static Mutex<DeviceSel> {
    SELECTED.get_or_init(|| Mutex::new(DeviceSel::default()))
}

fn current_sel() -> DeviceSel {
    match selected().lock() {
        Ok(s) => s.clone(),
        Err(_) => DeviceSel::default(),
    }
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn to_i16_sample(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32767.0) as i16
}

#[derive(serde::Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub selected_input: Option<String>,
    pub selected_output: Option<String>,
}

pub fn list_devices() -> AudioDevices {
    let host = cpal::default_host();
    let inputs = host
        .input_devices()
        .map(|it| it.map(|d| d.to_string()).collect())
        .unwrap_or_default();
    let outputs = host
        .output_devices()
        .map(|it| it.map(|d| d.to_string()).collect())
        .unwrap_or_default();
    let sel = current_sel();
    AudioDevices {
        inputs,
        outputs,
        selected_input: sel.input,
        selected_output: sel.output,
    }
}

pub fn set_input_device(name: Option<String>) {
    if let Ok(mut sel) = selected().lock() {
        sel.input = name;
    }
    DEVICE_GEN.fetch_add(1, Ordering::Relaxed);
    wake_audio_thread();
}

pub fn set_output_device(name: Option<String>) {
    if let Ok(mut sel) = selected().lock() {
        sel.output = name;
    }
    DEVICE_GEN.fetch_add(1, Ordering::Relaxed);
    wake_audio_thread();
}

pub struct AudioHandle {
    stop: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
}

impl AudioHandle {
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn set_deafened(&self, deafened: bool) {
        self.deafened.store(deafened, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn is_deafened(&self) -> bool {
        self.deafened.load(Ordering::Relaxed)
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        wake_audio_thread();
        #[cfg(target_os = "ios")]
        crate::call::ios_audio::deactivate_session();
    }
}

pub fn start(sample_rate: u32) -> Result<(AudioHandle, HeapProd<i16>, HeapCons<i16>)> {
    #[cfg(target_os = "ios")]
    crate::call::ios_audio::activate_voice_session();

    let capacity = sample_rate as usize;
    let (cap_prod, cap_cons) = HeapRb::<i16>::new(capacity).split();
    let (play_prod, play_cons) = HeapRb::<i16>::new(capacity).split();
    let stop = Arc::new(AtomicBool::new(false));
    let muted = Arc::new(AtomicBool::new(false));
    let deafened = Arc::new(AtomicBool::new(false));
    let cap_prod: SharedProd = Arc::new(Mutex::new(cap_prod));
    let play_cons: SharedCons = Arc::new(Mutex::new(play_cons));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();

    {
        let stop = stop.clone();
        let muted = muted.clone();
        let deafened = deafened.clone();
        let cap_prod = cap_prod.clone();
        let play_cons = play_cons.clone();
        std::thread::spawn(move || {
            audio_loop(
                sample_rate,
                cap_prod,
                play_cons,
                muted,
                deafened,
                stop,
                ready_tx,
            );
        });
    }

    match ready_rx.recv() {
        Ok(Ok(())) => Ok((
            AudioHandle {
                stop,
                muted,
                deafened,
            },
            play_prod,
            cap_cons,
        )),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(KursalError::Network(format!("audio thread failed: {e}"))),
    }
}

fn audio_loop(
    sample_rate: u32,
    cap_prod: SharedProd,
    play_cons: SharedCons,
    muted: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    ready_tx: std::sync::mpsc::Sender<Result<()>>,
) {
    if let Ok(mut t) = AUDIO_THREAD.lock() {
        *t = Some(std::thread::current());
    }
    let mut announced = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let generation = DEVICE_GEN.load(Ordering::Relaxed);
        match build_streams(sample_rate, &cap_prod, &play_cons, &muted, &deafened) {
            Ok((_input, _output)) => {
                if !announced {
                    let _ = ready_tx.send(Ok(()));
                    announced = true;
                }
                while !stop.load(Ordering::Relaxed)
                    && DEVICE_GEN.load(Ordering::Relaxed) == generation
                {
                    std::thread::park();
                }
            }
            Err(e) => {
                if !announced {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
                log::error!("[call] audio device switch failed: {e}");
                while !stop.load(Ordering::Relaxed)
                    && DEVICE_GEN.load(Ordering::Relaxed) == generation
                {
                    std::thread::park();
                }
            }
        }
    }
}

struct PushResampler {
    step: f64,
    pos: f64,
    prev: f32,
}

impl PushResampler {
    fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self {
            step: f64::from(src_rate) / f64::from(dst_rate),
            pos: 0.0,
            prev: 0.0,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
        while self.pos < 1.0 {
            let v = self.prev + (sample - self.prev) * self.pos as f32;
            emit(v);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = sample;
    }
}

struct PullResampler {
    step: f64,
    pos: f64,
    a: f32,
    b: f32,
}

impl PullResampler {
    fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self {
            step: f64::from(src_rate) / f64::from(dst_rate),
            pos: 1.0,
            a: 0.0,
            b: 0.0,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn pull(&mut self, mut next_src: impl FnMut() -> f32) -> f32 {
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            self.a = self.b;
            self.b = next_src();
        }
        let v = self.a + (self.b - self.a) * self.pos as f32;
        self.pos += self.step;
        v
    }
}

fn build_input(
    device: &cpal::Device,
    channels: u16,
    rate: u32,
    codec_rate: u32,
    cap_prod: &SharedProd,
    muted: &Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let channels = channels.max(1);
    let in_channels = usize::from(channels);
    let config = StreamConfig {
        channels,
        sample_rate: rate,
        buffer_size: BufferSize::Default,
    };
    let cap = cap_prod.clone();
    let mute_flag = muted.clone();
    let mut resampler = PushResampler::new(rate, codec_rate);
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                let m = mute_flag.load(Ordering::Relaxed);
                if let Ok(mut prod) = cap.lock() {
                    for frame in data.chunks_exact(in_channels) {
                        let mono = frame.iter().sum::<f32>() / in_channels as f32;
                        resampler.push(mono, |v| {
                            let s = if m { 0 } else { to_i16_sample(v) };
                            let _ = prod.try_push(s);
                        });
                    }
                }
            },
            |err| log::error!("[call] input stream error: {err}"),
            None,
        )
        .map_err(|e| KursalError::Network(format!("build input stream: {e}")))
}

fn build_streams(
    codec_rate: u32,
    cap_prod: &SharedProd,
    play_cons: &SharedCons,
    muted: &Arc<AtomicBool>,
    deafened: &Arc<AtomicBool>,
) -> Result<(cpal::Stream, cpal::Stream)> {
    let host = cpal::default_host();
    let sel = current_sel();

    let input = sel
        .input
        .as_deref()
        .and_then(|n| {
            host.input_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.to_string() == n))
        })
        .or_else(|| host.default_input_device())
        .ok_or_else(|| KursalError::Network("no input audio device".into()))?;
    let output = sel
        .output
        .as_deref()
        .and_then(|n| {
            host.output_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.to_string() == n))
        })
        .or_else(|| host.default_output_device())
        .ok_or_else(|| KursalError::Network("no output audio device".into()))?;

    let in_default = input
        .default_input_config()
        .map_err(|e| KursalError::Network(format!("input config: {e}")))?;
    let out_default = output
        .default_output_config()
        .map_err(|e| KursalError::Network(format!("output config: {e}")))?;

    let in_rate = in_default.sample_rate();
    let out_rate = out_default.sample_rate();
    let out_channels = usize::from(out_default.channels().max(1));

    let out_config = StreamConfig {
        channels: out_default.channels().max(1),
        sample_rate: out_rate,
        buffer_size: BufferSize::Default,
    };

    let in_ch = in_default.channels().max(1);
    let input_stream = match build_input(&input, 1, in_rate, codec_rate, cap_prod, muted) {
        Ok(s) => s,
        Err(e) if in_ch != 1 => {
            log::warn!("[call] mono capture failed ({e}); trying {in_ch}ch");
            build_input(&input, in_ch, in_rate, codec_rate, cap_prod, muted)?
        }
        Err(e) => return Err(e),
    };

    let play = play_cons.clone();
    let deaf_flag = deafened.clone();
    let mut out_resampler = PullResampler::new(codec_rate, out_rate);
    let output_stream = output
        .build_output_stream(
            out_config,
            move |data: &mut [f32], _| {
                let d = deaf_flag.load(Ordering::Relaxed);
                if let Ok(mut cons) = play.lock() {
                    let mut frames = data.chunks_exact_mut(out_channels);
                    for frame in frames.by_ref() {
                        let v =
                            out_resampler.pull(|| f32::from(cons.try_pop().unwrap_or(0)) / 32768.0);
                        let v = if d { 0.0 } else { v };
                        for s in frame.iter_mut() {
                            *s = v;
                        }
                    }
                    frames.into_remainder().fill(0.0);
                } else {
                    data.fill(0.0);
                }
            },
            |err| log::error!("[call] output stream error: {err}"),
            None,
        )
        .map_err(|e| KursalError::Network(format!("build output stream: {e}")))?;

    input_stream
        .play()
        .map_err(|e| KursalError::Network(format!("play input: {e}")))?;
    output_stream
        .play()
        .map_err(|e| KursalError::Network(format!("play output: {e}")))?;

    Ok((input_stream, output_stream))
}
