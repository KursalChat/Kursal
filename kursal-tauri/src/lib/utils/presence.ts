const ONLINE = new Set(['direct', 'holepunch', 'relay']);

export function isOnlineStatus(s: string | undefined): boolean {
  return !!s && ONLINE.has(s);
}

export function shouldStampLastSeen(prev: string | undefined, next: string): boolean {
  return isOnlineStatus(next) || (next === 'disconnected' && isOnlineStatus(prev));
}
