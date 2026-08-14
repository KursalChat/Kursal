import type { Emoji, EmojiIndex, EmojiGroup, PackedEmoji } from './types';
import { GROUP_META } from './types';

export function decodeIndex(packed: PackedEmoji[]): EmojiIndex {
  const byGroup = new Map<string, Emoji[]>();
  const flat: Emoji[] = new Array(packed.length);
  const byShortcode = new Map<string, Emoji>();

  for (let i = 0; i < packed.length; i++) {
    const [unicode, label, group, tags, shortcodes, skins] = packed[i];

    const emoji: Emoji = {
      unicode,
      label,
      group,
      tags,
      shortcodes,
      skins: skins === 0 ? undefined : skins.map(([tone, u]) => ({ tone, unicode: u })),
    };

    flat[i] = emoji;

    let bucket = byGroup.get(group);
    if (!bucket) {
      bucket = [];
      byGroup.set(group, bucket);
    }
    bucket.push(emoji);

    for (const code of shortcodes) if (!byShortcode.has(code)) byShortcode.set(code, emoji);
  }

  const groups: EmojiGroup[] = GROUP_META.filter((m) => byGroup.has(m.slug)).map((m) => ({
    id: m.slug,
    label: m.label,
    icon: m.icon,
    emojis: byGroup.get(m.slug)!,
  }));

  return { groups, flat, byShortcode };
}
