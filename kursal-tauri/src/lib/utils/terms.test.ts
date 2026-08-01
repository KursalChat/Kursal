import { describe, expect, it } from 'vitest';
import { isTermsPending } from './terms';

describe('isTermsPending', () => {
  it('prompts when nothing was ever accepted', () => {
    expect(isTermsPending(null, '2026-07-27')).toBe(true);
  });

  it('stays quiet once the current version is accepted', () => {
    expect(isTermsPending('2026-07-27', '2026-07-27')).toBe(false);
  });

  it('prompts when the accepted version predates the shipped one', () => {
    expect(isTermsPending('2025-11-02', '2026-07-27')).toBe(true);
  });

  it('stays quiet on a build older than the accepted version', () => {
    expect(isTermsPending('2026-07-27', '2025-11-02')).toBe(false);
  });

  it('orders by date, not string length or digit value', () => {
    expect(isTermsPending('2026-09-01', '2026-10-01')).toBe(true);
    expect(isTermsPending('2026-10-01', '2026-09-01')).toBe(false);
  });
});
