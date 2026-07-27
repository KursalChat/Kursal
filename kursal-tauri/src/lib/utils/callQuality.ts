export type CallQualityKey = 'low' | 'medium' | 'high';

export function qualityKey(rate: number): CallQualityKey {
  if (rate <= 16000) return 'low';
  if (rate <= 24000) return 'medium';
  return 'high';
}

export function khz(rate: number): number {
  return Math.round(rate / 1000);
}
