import { t } from '$lib/i18n';
import { lastSeenLabel } from './lastSeen';
import type { ConnectionChangedPayload } from '$lib/types';

export function connectionLabel(
  status: ConnectionChangedPayload['status'] | undefined,
  lastSeenAt: number | null = null
): string {
  switch (status) {
    case 'direct':
      return t('layout.statusOnlineDirect');
    case 'holepunch':
      return t('layout.statusOnlineHolepunch');
    case 'relay':
      return t('layout.statusOnlineRelay');
    case 'connecting':
      return t('layout.statusConnecting');
    default:
      return lastSeenLabel(lastSeenAt);
  }
}
