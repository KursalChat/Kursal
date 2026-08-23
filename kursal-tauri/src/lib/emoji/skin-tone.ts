import type { Emoji, ToneId } from './types';
import { readRaw, writeRaw } from '$lib/utils/storage';

export const LOCAL_KEY = 'kursal_emoji_tone';

export function getTone(): ToneId {
  const v = Number(readRaw(LOCAL_KEY));
  if (Number.isInteger(v) && v >= 0 && v <= 5) return v as ToneId;
  return 0;
}

export function setTone(tone: ToneId): void {
  writeRaw(LOCAL_KEY, String(tone));
}

export function applyTone(emoji: Emoji, tone: ToneId): string {
  if (!tone || !emoji.skins) return emoji.unicode;
  return emoji.skins.find((s) => s.tone === tone)?.unicode ?? emoji.unicode;
}
