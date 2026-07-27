import {
  vibrate as tauriVibrate,
  impactFeedback as tauriImpact,
  notificationFeedback as tauriNotification,
  selectionFeedback as tauriSelection,
} from '@tauri-apps/plugin-haptics';
import { isMobile } from '$lib/api/window';

type Impact = 'light' | 'medium' | 'heavy' | 'rigid' | 'soft';
type Notify = 'success' | 'warning' | 'error';

export async function vibrate(ms: number) {
  if (!isMobile) return;
  try {
    await tauriVibrate(ms);
  } catch {}
}

export async function impact(style: Impact = 'medium') {
  if (!isMobile) return;
  try {
    await tauriImpact(style);
  } catch {}
}

export async function notify(kind: Notify) {
  if (!isMobile) return;
  try {
    await tauriNotification(kind);
  } catch {}
}

export async function select() {
  if (!isMobile) return;
  try {
    await tauriSelection();
  } catch {}
}
