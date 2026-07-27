export const COUNTS_KEY = 'kursal_recent_emoji_counts';

function read(): Record<string, number> {
  try {
    const raw = localStorage.getItem(COUNTS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

export function bumpRecent(unicode: string): void {
  const counts = read();
  counts[unicode] = (counts[unicode] ?? 0) + 1;
  try {
    localStorage.setItem(COUNTS_KEY, JSON.stringify(counts));
  } catch {}
}

export function topRecents(n: number): string[] {
  return Object.entries(read())
    .sort((a, b) => b[1] - a[1])
    .slice(0, n)
    .map(([emoji]) => emoji);
}
