import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/i18n', () => ({
  t: (k: string) => k,
  dateLocale: () => 'en-GB',
}));

const { setTimeFormatPref, clockOptions, formatTimeShort, isSameDay, formatCalendarDay } =
  await import('./dateFormat.svelte');

beforeEach(() => setTimeFormatPref('24h'));

describe('clockOptions', () => {
  it('tracks the stored preference', () => {
    expect(clockOptions().hour12).toBe(false);
    setTimeFormatPref('12h');
    expect(clockOptions().hour12).toBe(true);
  });
});

describe('formatTimeShort', () => {
  it('is blank for a missing timestamp', () => {
    expect(formatTimeShort(0)).toBe('');
  });

  it('shows a clock today and a date long ago', () => {
    const now = new Date();
    expect(formatTimeShort(now.getTime())).toMatch(/\d/);

    const longAgo = new Date(now);
    longAgo.setDate(longAgo.getDate() - 60);
    expect(formatTimeShort(longAgo.getTime())).not.toMatch(/:/);
  });
});

describe('isSameDay', () => {
  it('compares calendar days, not elapsed time', () => {
    const a = new Date(2026, 4, 2, 23, 59).getTime();
    const b = new Date(2026, 4, 3, 0, 1).getTime();
    expect(isSameDay(a, a)).toBe(true);
    expect(isSameDay(a, b)).toBe(false);
  });
});

describe('formatCalendarDay', () => {
  it('names today and yesterday', () => {
    const now = Date.now();
    expect(formatCalendarDay(now)).toBe('time.today');
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    expect(formatCalendarDay(yesterday.getTime())).toBe('time.yesterday');
  });

  it('appends the clock only when asked', () => {
    expect(formatCalendarDay(Date.now(), true)).toContain('·');
    expect(formatCalendarDay(Date.now(), false)).not.toContain('·');
  });
});
