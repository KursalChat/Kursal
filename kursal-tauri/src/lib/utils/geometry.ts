export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function percent(part: number, whole: number): number {
  if (whole <= 0) return 0;
  return clamp(Math.round((part / whole) * 100), 0, 100);
}

export interface ViewportBounds {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export function readInsetBounds(margin = 8): ViewportBounds {
  const style = getComputedStyle(document.documentElement);
  const inset = (name: string) => parseFloat(style.getPropertyValue(name)) || 0;
  return {
    left: inset('--safe-left') + margin,
    right: window.innerWidth - inset('--safe-right') - margin,
    top: inset('--safe-top') + margin,
    bottom: window.innerHeight - inset('--safe-bottom') - margin,
  };
}

// Keeps a popover of `width` inside the safe area, preferring the requested left.
export function clampHorizontally(left: number, width: number, bounds: ViewportBounds): number {
  return clamp(left, bounds.left, Math.max(bounds.left, bounds.right - width));
}
