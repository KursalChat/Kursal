import { readRaw, writeRaw } from '$lib/utils/storage';

export const TERMS_URL = 'https://kursal.chat/terms';
export const TERMS_UPDATED = __TERMS_UPDATED__;

const STORAGE_KEY = 'kursal_terms_accepted';

export function termsDateLabel(locale: string): string {
  return new Date(`${TERMS_UPDATED}T00:00:00`).toLocaleDateString(locale, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}

export function isTermsPending(accepted: string | null, current = TERMS_UPDATED): boolean {
  return accepted === null || accepted < current;
}

export function acceptedTerms(): string | null {
  return readRaw(STORAGE_KEY);
}

export function acceptTerms(): void {
  writeRaw(STORAGE_KEY, TERMS_UPDATED);
}

export function termsPending(): boolean {
  return isTermsPending(acceptedTerms());
}
