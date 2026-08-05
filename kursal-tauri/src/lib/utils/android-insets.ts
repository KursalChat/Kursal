/**
 * Android's WebView never derives env(safe-area-inset-*) from the status/nav bars, and
 * edge-to-edge mode means the window is never resized for the keyboard. IME is tracked
 * separately from `--safe-bottom`; syncViewport nets it against the visual viewport.
 */

declare global {
  interface Window {
    __kursalInsets?: { read(): string };
    __kursalOnInsets?: (
      top: number,
      right: number,
      bottom: number,
      left: number,
      ime?: number
    ) => void;
  }
}

const VARS = ['--safe-top', '--safe-right', '--safe-bottom', '--safe-left'] as const;

let imeInset = 0;
let listener: (() => void) | null = null;

/** Keyboard height in CSS pixels, 0 when closed or off Android. */
export function getImeInset(): number {
  return imeInset;
}

export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/**
 * The four insets as numbers, for popovers that have to decide where to flip.
 * Read back off the custom properties so the value is the same one CSS uses,
 * whichever side (env() or the native bridge) produced it.
 */
export function readInsets(): Insets {
  if (typeof document === 'undefined') return { top: 0, right: 0, bottom: 0, left: 0 };
  const style = getComputedStyle(document.documentElement);
  const px = (name: string) => parseFloat(style.getPropertyValue(name)) || 0;
  return {
    top: px('--safe-top'),
    right: px('--safe-right'),
    bottom: px('--safe-bottom'),
    left: px('--safe-left'),
  };
}

/** Called whenever native pushes new insets, so the layout can re-measure. */
export function onInsetsChange(cb: () => void): void {
  listener = cb;
}

function apply(top: number, right: number, bottom: number, left: number, ime = 0): void {
  const values = [top, right, bottom, left];
  VARS.forEach((name, i) => {
    const value = values[i];
    if (Number.isFinite(value)) {
      document.documentElement.style.setProperty(name, `${Math.max(0, value)}px`);
    }
  });
  if (Number.isFinite(ime)) imeInset = Math.max(0, ime);
  listener?.();
}

export function initAndroidInsets(): void {
  if (typeof window === 'undefined') return;

  window.__kursalOnInsets = apply;

  const bridge = window.__kursalInsets;
  if (!bridge) return;

  try {
    const [top, right, bottom, left, ime] = bridge.read().split(',').map(Number);
    apply(top, right, bottom, left, ime);
  } catch {
    // Bridge unavailable; env() defaults remain in effect.
  }
}
