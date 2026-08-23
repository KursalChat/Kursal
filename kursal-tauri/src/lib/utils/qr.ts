import type { BitMatrix } from 'qrcode';
import { log } from '$lib/utils/log';

interface QrOptions {
  errorCorrectionLevel?: 'L' | 'M' | 'Q' | 'H';
  width?: number;
  mascot?: boolean;
}

const MASCOT_SRC = '/winston.webp';
const MASCOT_RATIO = 0.22;
const QUIET_CELLS = 4;
const EYE_CELLS = 7;
const PLATE = '#f8fafc';
const INK = '#0e1522';

let mascot: Promise<HTMLImageElement> | null = null;

function loadMascot(): Promise<HTMLImageElement> {
  mascot ??= new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Failed to load ${MASCOT_SRC}`));
    img.src = MASCOT_SRC;
  });
  return mascot;
}

export async function renderQrDataUrl(
  value: string,
  options: QrOptions = {}
): Promise<string | null> {
  try {
    const pending = options.mascot === false ? null : loadMascot();
    const { create } = await import('qrcode');
    const { modules } = create(value, {
      errorCorrectionLevel: options.errorCorrectionLevel ?? 'M',
    });

    const size = modules.size;
    const grid = size + QUIET_CELLS * 2;
    const density = Math.min(2, Math.max(1, Math.round(window.devicePixelRatio || 1)));
    /* Snapping the module to a whole pixel keeps every edge off the subpixel
     * grid, so the code stays crisp and the rasteriser has nothing to blend. */
    const cell = Math.max(1, Math.round(((options.width ?? 320) * density) / grid));
    const px = cell * grid;
    const origin = QUIET_CELLS * cell;

    const canvas = document.createElement('canvas');
    canvas.width = px;
    canvas.height = px;
    const ctx = canvas.getContext('2d', { alpha: false });
    if (!ctx) return null;

    ctx.fillStyle = PLATE;
    ctx.fillRect(0, 0, px, px);

    const hole = pending ? holeSpan(size) : 0;
    const holeStart = (size - hole) >> 1;

    const path = new Path2D();
    addModules(path, buildMask(modules, size, holeStart, holeStart + hole), size, cell, origin);
    addEyes(path, size, cell, origin);
    ctx.fillStyle = INK;
    ctx.fill(path, 'evenodd');

    if (pending) await drawMascot(ctx, pending, origin + holeStart * cell, hole * cell);

    return canvas.toDataURL('image/png');
  } catch (e) {
    log.error('Failed to render QR:', e);
    return null;
  }
}

function buildMask(
  modules: BitMatrix,
  size: number,
  holeStart: number,
  holeEnd: number
): Uint8Array {
  const mask = new Uint8Array(size * size);
  const data = modules.data;
  const far = size - EYE_CELLS;
  for (let y = 0; y < size; y++) {
    const row = y * size;
    const topRow = y < EYE_CELLS;
    const eyeRow = topRow || y >= far;
    const holeRow = y >= holeStart && y < holeEnd;
    for (let x = 0; x < size; x++) {
      if (!data[row + x]) continue;
      if (eyeRow && (x < EYE_CELLS || (topRow && x >= far))) continue;
      if (holeRow && x >= holeStart && x < holeEnd) continue;
      mask[row + x] = 1;
    }
  }
  return mask;
}

/* Each module rounds only the corners whose two neighbours are light */
function addModules(
  path: Path2D,
  mask: Uint8Array,
  size: number,
  cell: number,
  origin: number
): void {
  const r = cell / 2;
  const last = size - 1;
  for (let y = 0; y < size; y++) {
    const row = y * size;
    for (let x = 0; x < size; x++) {
      const i = row + x;
      if (!mask[i]) continue;
      const left = origin + x * cell;
      const top = origin + y * cell;
      const n = y > 0 && mask[i - size] === 1;
      const s = y < last && mask[i + size] === 1;
      const w = x > 0 && mask[i - 1] === 1;
      const e = x < last && mask[i + 1] === 1;
      if (n && s && w && e) {
        path.rect(left, top, cell, cell);
        continue;
      }
      path.roundRect(left, top, cell, cell, [
        !n && !w ? r : 0,
        !n && !e ? r : 0,
        !s && !e ? r : 0,
        !s && !w ? r : 0,
      ]);
    }
  }
}

/* Three nested subpaths per eye */
function addEyes(path: Path2D, size: number, cell: number, origin: number): void {
  const far = size - EYE_CELLS;
  const corners = [0, 0, far, 0, 0, far];
  for (let i = 0; i < corners.length; i += 2) {
    const x = origin + corners[i] * cell;
    const y = origin + corners[i + 1] * cell;
    path.roundRect(x, y, cell * 7, cell * 7, cell * 2.3);
    path.roundRect(x + cell, y + cell, cell * 5, cell * 5, cell * 1.5);
    path.roundRect(x + cell * 2, y + cell * 2, cell * 3, cell * 3, cell * 1.1);
  }
}

function holeSpan(size: number): number {
  return Math.max(5, Math.round(size * MASCOT_RATIO) | 1);
}

async function drawMascot(
  ctx: CanvasRenderingContext2D,
  pending: Promise<HTMLImageElement>,
  at: number,
  box: number
): Promise<void> {
  try {
    ctx.drawImage(await pending, at, at, box, box);
  } catch (e) {
    log.error('Failed to draw QR mascot:', e);
  }
}
