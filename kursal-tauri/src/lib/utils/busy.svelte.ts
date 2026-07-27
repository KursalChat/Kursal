export function busy() {
  let active = $state(false);

  return {
    get active() {
      return active;
    },
    async run<T>(fn: () => Promise<T>): Promise<T | undefined> {
      if (active) return;
      active = true;
      try {
        return await fn();
      } finally {
        active = false;
      }
    },
  };
}
