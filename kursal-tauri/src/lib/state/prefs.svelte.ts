import { browser } from '$app/environment';
import { setNotificationPreviewCore, setNotificationDndCore } from '$lib/api/settings';

export type NotificationPreview = 'content' | 'sender' | 'generic' | 'none';

function mirrorPreviewToCore(value: NotificationPreview) {
  void setNotificationPreviewCore(value).catch(() => {});
}

function mirrorDndToCore(d: { enabled: boolean; start: string; end: string }) {
  const spec = d.enabled ? `1|${d.start}|${d.end}` : '';
  void setNotificationDndCore(spec).catch(() => {});
}

export interface DndSchedule {
  enabled: boolean;
  start: string; // "HH:MM"
  end: string; // "HH:MM"
}

const KEYS = {
  preview: 'kursal_notif_preview',
  dnd: 'kursal_notif_dnd',
  appLock: 'kursal_app_lock_biometric',
};

function readJson<T>(key: string, fallback: T): T {
  if (!browser) return fallback;
  const raw = localStorage.getItem(key);
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writeJson<T>(key: string, value: T) {
  if (!browser) return;
  localStorage.setItem(key, JSON.stringify(value));
}

function createPrefsState() {
  let notificationPreview = $state<NotificationPreview>('content');
  let dnd = $state<DndSchedule>({ enabled: false, start: '22:00', end: '06:00' });
  let appLockBiometric = $state(false);
  let initialized = $state(false);

  function init() {
    if (!browser || initialized) return;
    notificationPreview = readJson<NotificationPreview>(KEYS.preview, 'content');
    dnd = readJson<DndSchedule>(KEYS.dnd, dnd);
    appLockBiometric = readJson<boolean>(KEYS.appLock, false);
    initialized = true;
    mirrorPreviewToCore(notificationPreview);
    mirrorDndToCore(dnd);
  }

  function setPreview(value: NotificationPreview) {
    notificationPreview = value;
    writeJson(KEYS.preview, value);
    mirrorPreviewToCore(value);
  }

  function setDnd(value: DndSchedule) {
    dnd = value;
    writeJson(KEYS.dnd, value);
    mirrorDndToCore(value);
  }

  function setAppLockBiometric(value: boolean) {
    appLockBiometric = value;
    writeJson(KEYS.appLock, value);
  }

  return {
    get notificationPreview() {
      return notificationPreview;
    },
    get dnd() {
      return dnd;
    },
    get appLockBiometric() {
      return appLockBiometric;
    },
    init,
    setPreview,
    setDnd,
    setAppLockBiometric,
  };
}

export const prefsState = createPrefsState();
