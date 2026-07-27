// Live elapsed readout for an active call; ticks every 500ms while active.
// Must be called during component init (uses $effect).
export function createCallElapsed(active: () => boolean, startedAt: () => number | null) {
  let elapsed = $state(0);

  $effect(() => {
    const started = startedAt();
    if (!active() || started === null) {
      elapsed = 0;
      return;
    }
    const tick = () => (elapsed = Date.now() - started);
    tick();
    const iv = setInterval(tick, 500);
    return () => clearInterval(iv);
  });

  return {
    get ms() {
      return elapsed;
    },
    get label() {
      const s = Math.floor(elapsed / 1000);
      return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
    },
  };
}
