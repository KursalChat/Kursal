import { decodeIndex } from './transform';
import type { EmojiIndex } from './types';

let indexPromise: Promise<EmojiIndex> | null = null;

export function loadEmojiIndex(): Promise<EmojiIndex> {
  if (indexPromise) return indexPromise;
  indexPromise = import('virtual:emoji-index').then((m) => decodeIndex(m.default));
  return indexPromise;
}
