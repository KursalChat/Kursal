import type { Emoji, ToneId } from './types';

export const LOCAL_KEY = 'kursal_emoji_tone';

export function getTone(): ToneId {
  try {
    const v = Number(localStorage.getItem(LOCAL_KEY));
    if (Number.isInteger(v) && v >= 0 && v <= 5) return v as ToneId;
  } catch {}
  return 0;
}

export function setTone(tone: ToneId): void {
  try {
    localStorage.setItem(LOCAL_KEY, String(tone));
  } catch {}
}

export function applyTone(emoji: Emoji, tone: ToneId): string {
  if (!tone || !emoji.skins) return emoji.unicode;
  return emoji.skins.find((s) => s.tone === tone)?.unicode ?? emoji.unicode;
}
