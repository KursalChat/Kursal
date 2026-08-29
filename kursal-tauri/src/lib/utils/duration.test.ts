import { describe, it, expect } from 'vitest';
import { formatClock, formatClockFromSeconds, formatHHMM, relativeDuration } from './duration';

describe('clock formatting', () => {
  it('pads seconds and lets minutes run past 59', () => {
    expect(formatClockFromSeconds(5)).toBe('0:05');
    expect(formatClockFromSeconds(65)).toBe('1:05');
    expect(formatClockFromSeconds(3661)).toBe('61:01');
  });

  it('floors negatives to zero', () => {
    expect(formatClockFromSeconds(-10)).toBe('0:00');
  });

  it('converts from milliseconds', () => {
    expect(formatClock(90_000)).toBe('1:30');
  });
});

describe('formatHHMM', () => {
  it('wraps around the day in both directions', () => {
    expect(formatHHMM(0)).toBe('00:00');
    expect(formatHHMM(1350)).toBe('22:30');
    expect(formatHHMM(1440)).toBe('00:00');
    expect(formatHHMM(-30)).toBe('23:30');
  });
});

describe('relativeDuration', () => {
  it('picks the coarsest unit that stays whole', () => {
    expect(relativeDuration(30_000)).toEqual({ unit: 'now', n: 0 });
    expect(relativeDuration(5 * 60_000)).toEqual({ unit: 'minutes', n: 5 });
    expect(relativeDuration(3 * 3_600_000)).toEqual({ unit: 'hours', n: 3 });
    expect(relativeDuration(2 * 86_400_000)).toEqual({ unit: 'days', n: 2 });
    expect(relativeDuration(14 * 86_400_000)).toEqual({ unit: 'weeks', n: 2 });
  });
});
