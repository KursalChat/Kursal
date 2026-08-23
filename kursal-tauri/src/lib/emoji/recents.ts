import { readJson, writeJson } from '$lib/utils/storage';

export const COUNTS_KEY = 'kursal_recent_emoji_counts';

function read(): Record<string, number> {
  return readJson<Record<string, number>>(COUNTS_KEY, {});
}

export function bumpRecent(unicode: string): void {
  const counts = read();
  counts[unicode] = (counts[unicode] ?? 0) + 1;
  writeJson(COUNTS_KEY, counts);
}

export function topRecents(n: number): string[] {
  return Object.entries(read())
    .sort((a, b) => b[1] - a[1])
    .slice(0, n)
    .map(([emoji]) => emoji);
}
