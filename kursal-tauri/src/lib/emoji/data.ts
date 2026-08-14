import { decodeIndex } from './transform';
import type { EmojiIndex } from './types';

let cached: WeakRef<EmojiIndex> | null = null;
let inflight: Promise<EmojiIndex> | null = null;

export function loadEmojiIndex(): Promise<EmojiIndex> {
  const live = cached?.deref();
  if (live) return Promise.resolve(live);
  if (inflight) return inflight;

  inflight = import('virtual:emoji-index').then((m) => {
    const index = decodeIndex(m.default);
    cached = new WeakRef(index);
    inflight = null;
    return index;
  });
  return inflight;
}
