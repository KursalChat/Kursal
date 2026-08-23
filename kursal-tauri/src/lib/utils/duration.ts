function pad2(n: number): string {
  return n.toString().padStart(2, '0');
}

export function formatClockFromSeconds(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  return `${Math.floor(s / 60)}:${pad2(s % 60)}`;
}

export function formatClock(ms: number): string {
  return formatClockFromSeconds(ms / 1000);
}

export function formatHHMM(minutes: number): string {
  const m = ((minutes % 1440) + 1440) % 1440;
  return `${pad2(Math.floor(m / 60))}:${pad2(m % 60)}`;
}

export type DurationUnit = 'now' | 'minutes' | 'hours' | 'days' | 'weeks';

export interface RelativeDuration {
  unit: DurationUnit;
  n: number;
}

// Coarsest unit that still reads as a whole number, for "5m left" / "2 days ago"
// style labels. Callers own the wording; this only picks the unit.
export function relativeDuration(ms: number): RelativeDuration {
  const mins = Math.floor(ms / 60_000);
  if (mins < 1) return { unit: 'now', n: 0 };
  if (mins < 60) return { unit: 'minutes', n: mins };
  const hours = Math.floor(mins / 60);
  if (hours < 24) return { unit: 'hours', n: hours };
  const days = Math.floor(hours / 24);
  if (days < 7) return { unit: 'days', n: days };
  return { unit: 'weeks', n: Math.floor(days / 7) };
}
