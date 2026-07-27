import { sendVideoChunk } from '$lib/api/call';

const HEADER_BYTES = 10;
const MAX_CHUNK_BYTES = 262144 - HEADER_BYTES;
const KEYFRAME_INTERVAL_FRAMES = 90;
const MIN_BITRATE = 150_000;
const RECOVERY_STEP = 1.1;
const RECOVERY_INTERVAL_MS = 5000;
const MAX_ENCODE_QUEUE = 2;

export const QUALITIES: Record<number, { width: number; height: number; bitrate: number }> = {
  360: { width: 640, height: 360, bitrate: 600_000 },
  480: { width: 854, height: 480, bitrate: 900_000 },
  720: { width: 1280, height: 720, bitrate: 1_500_000 },
};

export function videoSupported(): boolean {
  return (
    typeof VideoEncoder !== 'undefined' &&
    typeof VideoDecoder !== 'undefined' &&
    typeof VideoFrame !== 'undefined' &&
    !!navigator.mediaDevices?.getUserMedia
  );
}

export async function pickEncoderConfig(
  quality: number,
  forceVp8 = false
): Promise<VideoEncoderConfig | null> {
  const q = QUALITIES[quality] ?? QUALITIES[480];
  const base = {
    width: q.width,
    height: q.height,
    bitrate: q.bitrate,
    framerate: 30,
    latencyMode: 'realtime' as const,
  };
  const h264: VideoEncoderConfig = {
    ...base,
    codec: 'avc1.42E01F',
    avc: { format: 'annexb' },
  };
  const vp8: VideoEncoderConfig = { ...base, codec: 'vp8' };
  const candidates = forceVp8 ? [vp8] : [h264, vp8];
  for (const cfg of candidates) {
    try {
      const support = await VideoEncoder.isConfigSupported(cfg);
      if (support.supported) return cfg;
    } catch {
      // probe failure means unsupported, try next candidate
    }
  }
  return null;
}

export function packChunk(chunk: EncodedVideoChunk): Uint8Array {
  const buf = new Uint8Array(HEADER_BYTES + chunk.byteLength);
  const view = new DataView(buf.buffer);
  view.setUint8(0, 0);
  view.setUint8(1, chunk.type === 'key' ? 1 : 0);
  view.setBigUint64(2, BigInt(Math.max(0, Math.round(chunk.timestamp))));
  chunk.copyTo(buf.subarray(HEADER_BYTES));
  return buf;
}

export function unpackChunk(
  bytes: ArrayBuffer
): { key: boolean; timestamp: number; data: Uint8Array } | null {
  if (bytes.byteLength < HEADER_BYTES) return null;
  const view = new DataView(bytes);
  if (view.getUint8(0) !== 0) return null;
  return {
    key: (view.getUint8(1) & 1) === 1,
    timestamp: Number(view.getBigUint64(2)),
    data: new Uint8Array(bytes, HEADER_BYTES),
  };
}

type RvfcVideo = HTMLVideoElement & {
  requestVideoFrameCallback(cb: () => void): number;
  cancelVideoFrameCallback(handle: number): void;
};

export class VideoSender {
  private stream: MediaStream | null = null;
  private el: RvfcVideo | null = null;
  private encoder: VideoEncoder | null = null;
  private rvfcHandle = 0;
  private forceKey = false;
  private frameCount = 0;
  private config: VideoEncoderConfig | null = null;
  private currentBitrate = 0;
  private recoveryTimer: ReturnType<typeof setInterval> | null = null;
  onEnded: (() => void) | null = null;

  get localStream(): MediaStream | null {
    return this.stream;
  }

  get codec(): string | null {
    return this.config?.codec ?? null;
  }

  async start(quality: number, forceVp8 = false): Promise<VideoEncoderConfig> {
    this.stop();
    const config = await pickEncoderConfig(quality, forceVp8);
    if (!config) throw new Error('encoder-unsupported');
    this.stream = await navigator.mediaDevices.getUserMedia({
      video: { width: config.width, height: config.height, frameRate: 30 },
    });
    this.config = config;
    this.currentBitrate = config.bitrate ?? QUALITIES[480].bitrate;
    this.encoder = new VideoEncoder({
      output: (chunk) => {
        if (chunk.byteLength > MAX_CHUNK_BYTES) {
          this.forceKey = true;
          return;
        }
        void sendVideoChunk(packChunk(chunk));
      },
      error: () => {
        this.stop();
        this.onEnded?.();
      },
    });
    this.encoder.configure(config);
    const el = document.createElement('video') as RvfcVideo;
    el.muted = true;
    el.playsInline = true;
    el.srcObject = this.stream;
    this.el = el;
    await el.play();
    const tick = () => {
      if (!this.encoder || !this.el) return;
      if (this.encoder.encodeQueueSize <= MAX_ENCODE_QUEUE) {
        const frame = new VideoFrame(this.el, {
          timestamp: Math.round(performance.now() * 1000),
        });
        const keyFrame = this.forceKey || this.frameCount % KEYFRAME_INTERVAL_FRAMES === 0;
        this.forceKey = false;
        this.frameCount++;
        this.encoder.encode(frame, { keyFrame });
        frame.close();
      }
      this.rvfcHandle = this.el.requestVideoFrameCallback(tick);
    };
    this.rvfcHandle = el.requestVideoFrameCallback(tick);
    return config;
  }

  requestKeyframe(): void {
    this.forceKey = true;
  }

  onCongestion(): void {
    if (!this.encoder || !this.config) return;
    this.currentBitrate = Math.max(MIN_BITRATE, Math.floor(this.currentBitrate / 2));
    this.encoder.configure({ ...this.config, bitrate: this.currentBitrate });
    this.forceKey = true;
    if (this.recoveryTimer) return;
    this.recoveryTimer = setInterval(() => {
      if (!this.encoder || !this.config) return;
      const target = this.config.bitrate ?? QUALITIES[480].bitrate;
      this.currentBitrate = Math.min(target, Math.floor(this.currentBitrate * RECOVERY_STEP));
      this.encoder.configure({ ...this.config, bitrate: this.currentBitrate });
      if (this.currentBitrate >= target && this.recoveryTimer) {
        clearInterval(this.recoveryTimer);
        this.recoveryTimer = null;
      }
    }, RECOVERY_INTERVAL_MS);
  }

  stop(): void {
    if (this.recoveryTimer) {
      clearInterval(this.recoveryTimer);
      this.recoveryTimer = null;
    }
    if (this.el && this.rvfcHandle) this.el.cancelVideoFrameCallback(this.rvfcHandle);
    this.rvfcHandle = 0;
    if (this.el) {
      this.el.srcObject = null;
      this.el = null;
    }
    if (this.encoder && this.encoder.state !== 'closed') this.encoder.close();
    this.encoder = null;
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.config = null;
    this.frameCount = 0;
    this.forceKey = false;
  }
}

export class VideoReceiver {
  private decoder: VideoDecoder | null = null;
  private awaitingKey = true;
  private decodedFrames = 0;
  onFrame: ((frame: VideoFrame) => void) | null = null;
  onNeedsKeyframe: (() => void) | null = null;
  onFatal: (() => void) | null = null;

  configure(codec: string, width: number, height: number): void {
    this.close();
    this.awaitingKey = true;
    this.decodedFrames = 0;
    this.decoder = new VideoDecoder({
      output: (frame) => {
        this.decodedFrames++;
        if (this.onFrame) this.onFrame(frame);
        else frame.close();
      },
      error: () => {
        if (this.decodedFrames === 0) {
          this.onFatal?.();
          return;
        }
        this.awaitingKey = true;
        this.onNeedsKeyframe?.();
      },
    });
    this.decoder.configure({
      codec,
      codedWidth: width,
      codedHeight: height,
      optimizeForLatency: true,
    });
  }

  push(bytes: ArrayBuffer): void {
    const parsed = unpackChunk(bytes);
    if (!parsed || !this.decoder || this.decoder.state !== 'configured') return;
    if (this.awaitingKey) {
      if (!parsed.key) {
        this.onNeedsKeyframe?.();
        return;
      }
      this.awaitingKey = false;
    }
    this.decoder.decode(
      new EncodedVideoChunk({
        type: parsed.key ? 'key' : 'delta',
        timestamp: parsed.timestamp,
        data: parsed.data,
      })
    );
  }

  close(): void {
    if (this.decoder && this.decoder.state !== 'closed') this.decoder.close();
    this.decoder = null;
    this.awaitingKey = true;
    this.decodedFrames = 0;
  }
}
