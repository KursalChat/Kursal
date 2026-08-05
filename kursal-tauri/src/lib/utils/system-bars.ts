declare global {
  interface Window {
    __kursalBars?: { setLightBackground(light: boolean): void };
  }
}

export function setSystemBarsLight(light: boolean): void {
  if (typeof window === 'undefined') return;
  try {
    window.__kursalBars?.setLightBackground(light);
  } catch {
    // Bridge unavailable; the bars keep whatever the theme set.
  }
}
