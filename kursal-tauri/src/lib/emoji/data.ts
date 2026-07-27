import { buildIndex } from './transform';
import type { EmojiIndex, RawEmoji } from './types';

let indexPromise: Promise<EmojiIndex> | null = null;

export function loadEmojiIndex(): Promise<EmojiIndex> {
  if (indexPromise) return indexPromise;
  indexPromise = (async () => {
    const [emojis, shortcodes, groups] = await Promise.all([
      import('emojibase-data/en/compact.json'),
      import('emojibase-data/en/shortcodes/emojibase.json'),
      import('emojibase-data/meta/groups.json'),
    ]);
    return buildIndex(
      emojis.default as unknown as RawEmoji[],
      shortcodes.default as unknown as Record<string, string | string[]>,
      (groups.default as unknown as { groups: Record<string, string> }).groups
    );
  })();
  return indexPromise;
}
