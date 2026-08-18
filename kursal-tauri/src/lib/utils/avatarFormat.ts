export function pickAvatarMimeType(hasAlpha: boolean): 'image/png' | 'image/webp' {
  return hasAlpha ? 'image/png' : 'image/webp';
}

export function canvasHasAlpha(canvas: HTMLCanvasElement): boolean {
  if (canvas.width === 0 || canvas.height === 0) return false;

  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return false;

  const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
  for (let i = 3; i < data.length; i += 4) {
    if (data[i] < 255) return true;
  }

  return false;
}
