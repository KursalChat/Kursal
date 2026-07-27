import { t } from '$lib/i18n';

export function lastSeenLabel(ts: number | null): string {
  if (!ts) return t('layout.statusOffline');
  const mins = Math.floor((Date.now() - ts) / 60000);
  if (mins < 1) return t('layout.lastSeenJustNow');
  if (mins < 60) return t('layout.lastSeenMinutes', { n: mins });
  const hours = Math.floor(mins / 60);
  if (hours < 24) return t('layout.lastSeenHours', { n: hours });
  const days = Math.floor(hours / 24);
  if (days < 7) return t('layout.lastSeenDays', { n: days });
  return t('layout.statusOffline');
}
