const REDUCED = '(prefers-reduced-motion: reduce)';

export function createSequence(delays: number[]) {
  const reduced = typeof matchMedia === 'function' && matchMedia(REDUCED).matches;
  let step = $state(reduced ? delays.length : 0);
  let timers: ReturnType<typeof setTimeout>[] = [];

  function clear() {
    timers.forEach(clearTimeout);
    timers = [];
  }

  if (!reduced) {
    timers = delays.map((delay, i) =>
      setTimeout(() => {
        if (step < i + 1) step = i + 1;
      }, delay)
    );
  }

  return {
    get step() {
      return step;
    },
    at(i: number) {
      return step > i;
    },
    get done() {
      return step >= delays.length;
    },
    skip() {
      clear();
      step = delays.length;
    },
    destroy: clear,
  };
}

export type Sequence = ReturnType<typeof createSequence>;
