import { invoke } from '@tauri-apps/api/core';

type Pending = { path: string; resolve: (present: boolean) => void };

let queue: Pending[] = [];
let scheduled = false;

async function flush() {
  const batch = queue;
  queue = [];
  scheduled = false;
  try {
    const results = await invoke<boolean[]>('paths_exist', {
      paths: batch.map((b) => b.path),
    });
    batch.forEach((b, i) => b.resolve(results[i] ?? true));
  } catch {
    batch.forEach((b) => b.resolve(true));
  }
}

export function pathExists(path: string): Promise<boolean> {
  return new Promise((resolve) => {
    queue.push({ path, resolve });
    if (scheduled) return;
    scheduled = true;
    queueMicrotask(flush);
  });
}