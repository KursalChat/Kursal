import { t, dateLocale } from '$lib/i18n';

export type TimeFormat = '24h' | '12h';

let hour12Pref = $state(false);

export function setTimeFormatPref(value: TimeFormat) {
  hour12Pref = value === '12h';
}

export function clockOptions(): Intl.DateTimeFormatOptions {
  return {
    hour: '2-digit',
    minute: '2-digit',
    hour12: hour12Pref,
  };
}

export function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString(dateLocale(), clockOptions());
}

export function formatDate(ts: number): string {
  return new Date(ts).toLocaleDateString(dateLocale());
}

export function formatFullTimestamp(ts: number): string {
  return new Date(ts).toLocaleString(dateLocale(), {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    ...clockOptions(),
  });
}

export function isSameDay(a: number, b: number): boolean {
  return new Date(a).toDateString() === new Date(b).toDateString();
}

const WEEK_MS = 7 * 24 * 3600 * 1000;

// Today / yesterday / weekday within the last week / absolute date beyond that.
// `withTime` appends the clock, which is what message group headers want and
// day separators do not.
export function formatCalendarDay(ts: number, withTime = false): string {
  const d = new Date(ts);
  const now = new Date();
  const suffix = withTime ? ` · ${formatTime(ts)}` : '';

  if (d.toDateString() === now.toDateString()) return t('time.today') + suffix;

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return t('time.yesterday') + suffix;

  if (now.getTime() - ts < WEEK_MS) {
    return d.toLocaleDateString(dateLocale(), { weekday: 'long' }) + suffix;
  }
  return (
    d.toLocaleDateString(dateLocale(), {
      month: 'short',
      day: 'numeric',
      year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    }) + suffix
  );
}

// Compact list-row stamp: clock today, short weekday this week, month+day beyond.
export function formatTimeShort(ts: number): string {
  if (!ts) return '';
  const d = new Date(ts);
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (ts >= startOfToday) return d.toLocaleTimeString(dateLocale(), clockOptions());
  if (ts >= startOfToday - 6 * 24 * 3600 * 1000) {
    return d.toLocaleDateString(dateLocale(), { weekday: 'short' });
  }
  return d.toLocaleDateString(dateLocale(), { month: 'short', day: 'numeric' });
}
