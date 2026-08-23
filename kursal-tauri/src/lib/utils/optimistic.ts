// Applies a value locally, then persists it. A rejected write restores the
// previous value before rethrowing, so the UI never keeps a setting the backend
// refused.
export async function optimistic<T>(
  read: () => T,
  write: (value: T) => void,
  persist: (value: T) => Promise<unknown>,
  next: T
): Promise<void> {
  const prev = read();
  write(next);
  try {
    await persist(next);
  } catch (e) {
    write(prev);
    throw e;
  }
}
