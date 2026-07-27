import type { Emoji, EmojiIndex, EmojiGroup, RawEmoji, ToneId } from './types';
import { GROUP_META } from './types';

const TONE_BY_MODIFIER: Record<string, ToneId> = {
  '1F3FB': 1,
  '1F3FC': 2,
  '1F3FD': 3,
  '1F3FE': 4,
  '1F3FF': 5,
};

function normalizeSkins(raw: RawEmoji): Emoji['skins'] {
  if (!raw.skins?.length) return undefined;
  const out: { tone: ToneId; unicode: string }[] = [];
  for (const s of raw.skins) {
    const parts = s.hexcode.split('-');
    if (parts.length !== 2) continue;
    const tone = TONE_BY_MODIFIER[parts[1]];
    if (tone) out.push({ tone, unicode: s.unicode });
  }
  return out.length ? out : undefined;
}

export function buildIndex(
  rawEmojis: RawEmoji[],
  shortcodes: Record<string, string | string[]>,
  groupSlugs: Record<string, string>
): EmojiIndex {
  const byGroup = new Map<string, Emoji[]>();
  const flat: Emoji[] = [];
  const byShortcode = new Map<string, Emoji>();

  for (const raw of rawEmojis) {
    if (raw.group === undefined) continue;
    const slug = groupSlugs[String(raw.group)];
    if (!slug || slug === 'component') continue;

    const rawCodes = shortcodes[raw.hexcode];
    const codes = rawCodes === undefined ? [] : Array.isArray(rawCodes) ? rawCodes : [rawCodes];

    const emoji: Emoji = {
      unicode: raw.unicode,
      label: raw.label,
      group: slug,
      tags: raw.tags ?? [],
      shortcodes: codes,
      skins: normalizeSkins(raw),
    };
    flat.push(emoji);
    if (!byGroup.has(slug)) byGroup.set(slug, []);
    byGroup.get(slug)!.push(emoji);
    for (const c of codes) if (!byShortcode.has(c)) byShortcode.set(c, emoji);
  }

  const groups: EmojiGroup[] = GROUP_META.filter((m) => byGroup.has(m.slug)).map((m) => ({
    id: m.slug,
    label: m.label,
    icon: m.icon,
    emojis: byGroup.get(m.slug)!,
  }));

  return { groups, flat, byShortcode };
}
