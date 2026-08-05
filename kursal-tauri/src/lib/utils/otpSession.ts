import { browser } from '$app/environment';

export type PersistedOtpState = {
  otp: string;
  shareView: 'words' | 'qr';
  expiresAt: number;
};

const STORAGE_KEY = 'kursal_add_contact_otp';

/**
 * The published code survives navigation away from the add-contact page, so it lives
 * in sessionStorage. The core consumes the OTP on pairing and emits `otp_consumed`,
 * which clears this; otherwise a return visit would restore a dead code mid-countdown.
 */
export function saveOtpSession(state: PersistedOtpState | null) {
  if (!browser) return;
  if (!state) {
    sessionStorage.removeItem(STORAGE_KEY);
    return;
  }
  sessionStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

export function loadOtpSession(): PersistedOtpState | null {
  if (!browser) return null;
  const raw = sessionStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as PersistedOtpState;
  } catch {
    sessionStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

export function clearOtpSession() {
  saveOtpSession(null);
}
