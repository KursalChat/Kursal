import {
  isPermissionGranted,
  removeActive,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { prefsState } from '$lib/state/prefs.svelte';
import { t } from '$lib/i18n';

let cached: boolean | null = null;

export async function getPermission(force = false): Promise<boolean> {
  if (cached !== null && !force) return cached;
  cached = await isPermissionGranted();
  return cached;
}

export async function ensurePermission(): Promise<boolean> {
  if (await getPermission()) return true;
  const r = await requestPermission();
  cached = r === 'granted';
  return cached;
}

function parseTime(s: string): number | null {
  const [h, m] = s.split(':').map((x) => parseInt(x, 10));
  if (isNaN(h) || isNaN(m)) return null;
  return h * 60 + m;
}

export function isInDndWindow(now: Date = new Date()): boolean {
  const { enabled, start, end } = prefsState.dnd;
  if (!enabled) return false;
  const s = parseTime(start);
  const e = parseTime(end);
  if (s === null || e === null) return false;
  if (s === e) return false;
  const cur = now.getHours() * 60 + now.getMinutes();
  if (s < e) return cur >= s && cur < e;
  return cur >= s || cur < e;
}

export interface MessageNotifyOptions {
  contactId: string;
  senderName: string;
  body: string;
}

// Android only, ignored elsewhere
const ICON = { icon: 'ic_notification', iconColor: '#4d8dff' };

// Ids of the banners still sitting in the tray, per contact
const activeIds = new Map<string, number[]>();
let nextId = Date.now() % 1_000_000;

// Android/iOS only
export async function clearNotificationsFor(contactId: string) {
  const ids = activeIds.get(contactId);
  if (!ids?.length) return;
  activeIds.delete(contactId);
  try {
    await removeActive(ids.map((id) => ({ id })));
  } catch {
    /* not supported on this platform */
  }
}

export async function notifyMessage({ contactId, senderName, body }: MessageNotifyOptions) {
  const preview = prefsState.notificationPreview;
  if (preview === 'none') return;
  if (isInDndWindow()) return;
  if (!(await getPermission())) return;

  let title: string;
  let text: string | undefined;
  switch (preview) {
    case 'content':
      title = senderName;
      text = body;
      break;
    case 'sender':
      title = t('notifications.newMessageFrom', { sender: senderName });
      break;
    case 'generic':
    default:
      title = 'Kursal';
      text = t('notifications.newMessage');
      break;
  }

  const id = ++nextId;
  const base = { ...ICON, id, title };
  sendNotification(text ? { ...base, body: text } : base);
  activeIds.set(contactId, [...(activeIds.get(contactId) ?? []), id]);
}

export async function sendTestNotification(): Promise<boolean> {
  if (!(await ensurePermission())) return false;

  const senderName = t('notifications.testSenderName');
  const body = t('notifications.testBody');
  const preview = prefsState.notificationPreview;

  let title: string;
  let text: string | undefined;
  switch (preview) {
    case 'sender':
      title = t('notifications.newMessageFrom', { sender: senderName });
      break;
    case 'generic':
      title = 'Kursal';
      text = t('notifications.newMessage');
      break;
    case 'none':
      title = 'Kursal';
      text = t('notifications.testOffBody');
      break;
    case 'content':
    default:
      title = senderName;
      text = body;
      break;
  }

  const base = { ...ICON, title };
  sendNotification(text ? { ...base, body: text } : base);
  return true;
}
