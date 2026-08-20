import { appLogDir, join } from '@tauri-apps/api/path';
import { open, readDir, remove, stat, writeTextFile, SeekMode } from '@tauri-apps/plugin-fs';

const TAIL_BYTES = 200_000;

export interface LogFile {
  name: string;
  path: string;
  sizeBytes: number;
  modifiedMs: number;
}

export interface LogTail {
  text: string;
  truncated: boolean;
}

export async function listLogFiles(): Promise<LogFile[]> {
  const dir = await appLogDir();
  const entries = await readDir(dir);
  const files = await Promise.all(
    entries
      .filter((e) => e.isFile && e.name.endsWith('.log'))
      .map(async (e) => {
        const path = await join(dir, e.name);
        const info = await stat(path);
        return {
          name: e.name,
          path,
          sizeBytes: Number(info.size ?? 0),
          modifiedMs: info.mtime ? new Date(info.mtime).getTime() : 0,
        };
      })
  );
  return files.sort((a, b) => b.modifiedMs - a.modifiedMs);
}

export async function clearLogs(): Promise<number> {
  const dir = await appLogDir();
  const entries = await readDir(dir);
  let freed = 0;

  for (const entry of entries) {
    if (!entry.isFile) continue;
    const archive = entry.name.endsWith('.gz');
    if (!archive && !entry.name.endsWith('.log')) continue;

    const path = await join(dir, entry.name);
    freed += Number((await stat(path)).size ?? 0);

    if (archive) await remove(path);
    else await writeTextFile(path, '');
  }

  return freed;
}

export async function readLogTail(path: string): Promise<LogTail> {
  const file = await open(path, { read: true });
  try {
    const size = Number((await file.stat()).size ?? 0);
    const start = Math.max(0, size - TAIL_BYTES);
    if (start > 0) await file.seek(start, SeekMode.Start);

    const buf = new Uint8Array(size - start);
    let filled = 0;
    while (filled < buf.length) {
      const chunk = new Uint8Array(buf.length - filled);
      const n = await file.read(chunk);
      if (!n) break;
      buf.set(chunk.subarray(0, n), filled);
      filled += n;
    }

    let text = new TextDecoder().decode(buf.subarray(0, filled));
    // A byte offset lands mid-line; drop the fragment.
    if (start > 0) text = text.slice(text.indexOf('\n') + 1);
    return { text, truncated: start > 0 };
  } finally {
    await file.close();
  }
}
