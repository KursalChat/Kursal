/**
 * Android window insets → CSS custom properties.
 *
 * Android's WebView only derives `env(safe-area-inset-*)` from display cutouts,
 * never from the status or navigation bars. Under edge-to-edge (forced by the
 * system on Android 15+) that leaves the page drawing underneath the status bar
 * with all env() values at 0, so the top strip of the UI is not tappable.
 *
 * MainActivity pushes the real insets in via `window.__kursalOnInsets(...)` and
 * exposes `window.__kursalInsets.read()` for the initial pull (the first native
 * dispatch usually happens before this script runs). Values are CSS pixels,
 * ordered top, right, bottom, left.
 *
 * On every other platform neither global exists and the env() defaults in
 * app.css stay in effect.
 */

declare global {
  interface Window {
    __kursalInsets?: { read(): string };
    __kursalOnInsets?: (top: number, right: number, bottom: number, left: number) => void;
  }
}

const VARS = ['--safe-top', '--safe-right', '--safe-bottom', '--safe-left'] as const;

function apply(top: number, right: number, bottom: number, left: number): void {
  const values = [top, right, bottom, left];
  VARS.forEach((name, i) => {
    const value = values[i];
    if (Number.isFinite(value)) {
      document.documentElement.style.setProperty(name, `${Math.max(0, value)}px`);
    }
  });
}

export function initAndroidInsets(): void {
  if (typeof window === 'undefined') return;

  window.__kursalOnInsets = apply;

  const bridge = window.__kursalInsets;
  if (!bridge) return;

  try {
    const [top, right, bottom, left] = bridge.read().split(',').map(Number);
    apply(top, right, bottom, left);
  } catch {
    // Bridge unavailable; env() defaults remain in effect.
  }
}
