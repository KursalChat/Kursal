const HEADER_BYTES = 10;

export function videoSupported(): boolean {
  return typeof VideoDecoder !== 'undefined' && typeof VideoFrame !== 'undefined';
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
