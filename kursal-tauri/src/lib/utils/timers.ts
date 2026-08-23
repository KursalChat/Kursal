export type TimerMap = Map<string, ReturnType<typeof setTimeout>>;

export function debounceKeyed(timers: TimerMap, key: string, ms: number, run: () => void): void {
  clearTimeout(timers.get(key));
  timers.set(
    key,
    setTimeout(() => {
      timers.delete(key);
      run();
    }, ms)
  );
}

export function cancelKeyed(timers: TimerMap, key?: string): void {
  if (key === undefined) {
    for (const timer of timers.values()) clearTimeout(timer);
    timers.clear();
    return;
  }
  clearTimeout(timers.get(key));
  timers.delete(key);
}

export function createLongPress(ms: number) {
  let timer: ReturnType<typeof setTimeout> | null = null;

  return {
    start(run: () => void) {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        run();
      }, ms);
    },
    cancel() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    },
  };
}
