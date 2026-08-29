const HEADER_BYTES = 10;

export function videoSupported(): boolean {
  return typeof VideoDecoder !== 'undefined' && typeof VideoFrame !== 'undefined';
}

// Byte 1: bit 0 is the keyframe flag, bits 1-2 the rotation in quarter turns.
export function unpackChunk(
  bytes: ArrayBuffer
): { key: boolean; timestamp: number; rotation: number; data: Uint8Array } | null {
  if (bytes.byteLength < HEADER_BYTES) return null;
  const view = new DataView(bytes);
  if (view.getUint8(0) !== 0) return null;
  const flags = view.getUint8(1);
  return {
    key: (flags & 1) === 1,
    rotation: ((flags >> 1) & 3) * 90,
    timestamp: Number(view.getBigUint64(2)),
    data: new Uint8Array(bytes, HEADER_BYTES),
  };
}

export function drawFrame(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D | null,
  frame: VideoFrame,
  rotation: number
): { width: number; height: number } {
  const turned = rotation === 90 || rotation === 270;
  const width = turned ? frame.displayHeight : frame.displayWidth;
  const height = turned ? frame.displayWidth : frame.displayHeight;
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  if (ctx) {
    ctx.save();
    ctx.translate(width / 2, height / 2);
    if (rotation) ctx.rotate((rotation * Math.PI) / 180);
    ctx.drawImage(frame, -frame.displayWidth / 2, -frame.displayHeight / 2);
    ctx.restore();
  }
  return { width, height };
}

export class VideoReceiver {
  private decoder: VideoDecoder | null = null;
  private awaitingKey = true;
  private decodedFrames = 0;
  private rotation = 0;
  onFrame: ((frame: VideoFrame, rotation: number) => void) | null = null;
  onNeedsKeyframe: (() => void) | null = null;
  onFatal: (() => void) | null = null;

  configure(codec: string, width: number, height: number): void {
    this.close();
    this.awaitingKey = true;
    this.decodedFrames = 0;
    this.decoder = new VideoDecoder({
      output: (frame) => {
        this.decodedFrames++;
        if (this.onFrame) this.onFrame(frame, this.rotation);
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
    this.rotation = parsed.rotation;
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
