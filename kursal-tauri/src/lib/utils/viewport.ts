import { getImeInset, onInsetsChange } from '$lib/utils/android-insets';

function syncViewport() {
  const vv = window.visualViewport;
  const root = document.documentElement.style;
  if (vv) {
    root.setProperty('--app-height', `${vv.height}px`);
    const covered = Math.max(0, window.innerHeight - (vv.height + vv.offsetTop));
    root.setProperty('--kb-overlap', `${Math.max(0, getImeInset() - covered)}px`);
  }
  window.scrollTo(0, 0);
}

// Publishes --app-height and --kb-overlap. The keyboard animates open over
// several frames, so a single read lands mid-transition; this re-reads until
// the size holds still or the deadline passes.
export function trackViewport(): () => void {
  let settleRaf = 0;

  const syncUntilStable = () => {
    cancelAnimationFrame(settleRaf);
    const deadline = performance.now() + 600;
    const STABLE_FRAMES = 3;
    let lastW = -1;
    let lastH = -1;
    let stable = 0;
    const step = () => {
      const vv = window.visualViewport;
      const w = vv?.width ?? window.innerWidth;
      const h = vv?.height ?? window.innerHeight;
      syncViewport();
      stable = w === lastW && h === lastH ? stable + 1 : 0;
      lastW = w;
      lastH = h;
      if (stable < STABLE_FRAMES && performance.now() < deadline) {
        settleRaf = requestAnimationFrame(step);
      }
    };
    step();
  };

  syncUntilStable();
  onInsetsChange(syncUntilStable);
  window.visualViewport?.addEventListener('resize', syncUntilStable);
  window.visualViewport?.addEventListener('scroll', syncViewport);
  window.addEventListener('resize', syncUntilStable);

  return () => {
    cancelAnimationFrame(settleRaf);
    window.visualViewport?.removeEventListener('resize', syncUntilStable);
    window.visualViewport?.removeEventListener('scroll', syncViewport);
    window.removeEventListener('resize', syncUntilStable);
  };
}

// Belt-and-suspenders pinch-zoom block for iOS Safari/WKWebView, which still
// honors gesture events even with user-scalable=no.
export function blockPinchZoom(): () => void {
  const block = (e: Event) => e.preventDefault();
  document.addEventListener('gesturestart', block);
  document.addEventListener('gesturechange', block);
  document.addEventListener('gestureend', block);

  return () => {
    document.removeEventListener('gesturestart', block);
    document.removeEventListener('gesturechange', block);
    document.removeEventListener('gestureend', block);
  };
}
