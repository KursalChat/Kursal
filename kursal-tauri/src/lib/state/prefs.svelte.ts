import { browser } from '$app/environment';
import { setNotificationPreviewCore, setNotificationDndCore } from '$lib/api/settings';
import { readJson, writeJson } from '$lib/utils/storage';
import { APP_LOCK_KEY } from '$lib/utils/storage-keys';

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
  appLock: APP_LOCK_KEY,
  mirrorSelfView: 'kursal_mirror_self_view',
};

function createPrefsState() {
  let notificationPreview = $state<NotificationPreview>('content');
  let dnd = $state<DndSchedule>({ enabled: false, start: '22:00', end: '06:00' });
  let appLockBiometric = $state(false);
  let mirrorSelfView = $state(true);
  let initialized = $state(false);

  function init() {
    if (!browser || initialized) return;
    notificationPreview = readJson<NotificationPreview>(KEYS.preview, 'content');
    dnd = readJson<DndSchedule>(KEYS.dnd, dnd);
    appLockBiometric = readJson<boolean>(KEYS.appLock, false);
    mirrorSelfView = readJson<boolean>(KEYS.mirrorSelfView, true);
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

  function toggleMirrorSelfView() {
    mirrorSelfView = !mirrorSelfView;
    writeJson(KEYS.mirrorSelfView, mirrorSelfView);
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
    get mirrorSelfView() {
      return mirrorSelfView;
    },
    init,
    setPreview,
    setDnd,
    setAppLockBiometric,
    toggleMirrorSelfView,
  };
}

export const prefsState = createPrefsState();
