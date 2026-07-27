import { describe, expect, it } from 'vitest';
import { isOnlineStatus, shouldStampLastSeen } from './presence';

describe('shouldStampLastSeen', () => {
  it('stamps when peer comes online', () => {
    expect(shouldStampLastSeen(undefined, 'direct')).toBe(true);
    expect(shouldStampLastSeen('connecting', 'relay')).toBe(true);
    expect(shouldStampLastSeen('disconnected', 'holepunch')).toBe(true);
  });

  it('stamps on disconnect only if previously online', () => {
    expect(shouldStampLastSeen('direct', 'disconnected')).toBe(true);
    expect(shouldStampLastSeen(undefined, 'disconnected')).toBe(false);
    expect(shouldStampLastSeen('connecting', 'disconnected')).toBe(false);
    expect(shouldStampLastSeen('disconnected', 'disconnected')).toBe(false);
  });

  it('never stamps while connecting', () => {
    expect(shouldStampLastSeen('direct', 'connecting')).toBe(false);
    expect(shouldStampLastSeen(undefined, 'connecting')).toBe(false);
  });
});

describe('isOnlineStatus', () => {
  it('treats direct/holepunch/relay as online', () => {
    expect(isOnlineStatus('direct')).toBe(true);
    expect(isOnlineStatus('holepunch')).toBe(true);
    expect(isOnlineStatus('relay')).toBe(true);
    expect(isOnlineStatus('disconnected')).toBe(false);
    expect(isOnlineStatus(undefined)).toBe(false);
  });
});
