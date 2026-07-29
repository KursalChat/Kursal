export const FLASH_MS = 1600;

// Transient "done" state for a control that confirms its own action (copied,
// saved) instead of pushing a toast. Retriggering restarts the window.
export function flash(duration = FLASH_MS) {
  let active = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  return {
    get active() {
      return active;
    },
    trigger() {
      clearTimeout(timer);
      active = true;
      timer = setTimeout(() => {
        active = false;
      }, duration);
    },
  };
}

// Same idea keyed by id, for confirmations that belong to a row in a list
// rather than to a single persistent control.
export function flashSet(duration = FLASH_MS) {
  let keys = $state<Record<string, true>>({});
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  return {
    has(key: string) {
      return keys[key] === true;
    },
    trigger(key: string) {
      clearTimeout(timers.get(key));
      keys[key] = true;
      timers.set(
        key,
        setTimeout(() => {
          delete keys[key];
          timers.delete(key);
        }, duration)
      );
    },
  };
}
