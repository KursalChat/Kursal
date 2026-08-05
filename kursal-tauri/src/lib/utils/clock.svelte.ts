import { browser } from '$app/environment';

let now = $state(Date.now());
let timer: ReturnType<typeof setInterval> | null = null;

// Relative-time labels would otherwise freeze at the value they had when their
// component last rendered. Reading this makes them re-render on the tick.
export function nowTick(): number {
  if (browser && !timer) timer = setInterval(() => (now = Date.now()), 30_000);
  return now;
}
