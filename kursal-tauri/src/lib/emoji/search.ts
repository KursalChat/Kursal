import type { Emoji } from './types';

export function searchEmojis(flat: Emoji[], query: string): Emoji[] {
  const q = query.toLowerCase().trim();
  if (!q) return [];

  const ranked: { e: Emoji; rank: number; len: number }[] = [];

  for (const e of flat) {
    const label = e.label.toLowerCase();
    const codes = e.shortcodes.map((s) => s.toLowerCase());
    const tags = e.tags.map((s) => s.toLowerCase());

    let rank = Infinity;
    let len = Infinity;

    if (codes.includes(q)) {
      rank = 0;
      len = q.length;
    } else if (label === q) {
      rank = 1;
      len = label.length;
    } else {
      const codePrefix = codes.find((c) => c.startsWith(q));
      if (codePrefix) {
        rank = 2;
        len = codePrefix.length;
      } else if (label.startsWith(q)) {
        rank = 3;
        len = label.length;
      } else if (codes.some((c) => c.includes(q)) || label.includes(q)) {
        rank = 4;
        len = label.length;
      } else if (tags.some((tg) => tg.startsWith(q))) {
        rank = 5;
        len = 0;
      } else if (tags.some((tg) => tg.includes(q))) {
        rank = 6;
        len = 0;
      }
    }

    if (rank !== Infinity) ranked.push({ e, rank, len });
  }

  return ranked.sort((a, b) => a.rank - b.rank || a.len - b.len).map((r) => r.e);
}
