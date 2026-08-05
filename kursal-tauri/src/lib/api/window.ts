import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { invoke } from '@tauri-apps/api/core';
import { log } from '$lib/utils/log';
import { platform } from '@tauri-apps/plugin-os';

export const OS = platform();
export const isMobile = OS == 'android' || OS == 'ios';

// Mirrors app.windows[0] in tauri.conf.json.
const DEFAULT_WIDTH = 1080;
const DEFAULT_HEIGHT = 780;

export async function resetWindowSize() {
  const win = getCurrentWindow();
  if (await win.isMaximized()) await win.unmaximize();
  await win.setSize(new LogicalSize(DEFAULT_WIDTH, DEFAULT_HEIGHT));
  await win.center();
}

export async function setTrayUnread(count: number) {
  if (isMobile) return;
  try {
    await invoke('set_tray_unread', { count });
  } catch (err) {
    log.error('Could not set tray unread:', err);
  }
}

export async function setBadgeCount(count?: number) {
  let label = count && count > 0 ? count : undefined;

  if (OS == 'windows') {
    try {
      await setOverlayBadge(label);
    } catch (err) {
      log.error('Could not set badge count:', err);
    }
  } else {
    try {
      await getCurrentWindow().setBadgeCount(label);
    } catch (err) {
      log.error('Could not set badge count:', err);
      // ignored: unsupported platform or not in Tauri context
    }
  }
}

async function setOverlayBadge(label?: number) {
  if (label == undefined) {
    return await getCurrentWindow().setOverlayIcon(undefined);
  }

  const canvas = document.createElement('canvas');
  canvas.width = 16;
  canvas.height = 16;
  const ctx = canvas.getContext('2d')!;

  ctx.fillStyle = '#e11d48';
  ctx.beginPath();
  ctx.arc(8, 8, 8, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = 'white';
  ctx.font = 'bold 10px Arial';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(label > 99 ? '99+' : String(label), 8, 8);

  const blob = await new Promise<Blob | null>((res) => canvas.toBlob(res, 'image/png'));
  if (!blob) return;
  const arrayBuffer = await blob.arrayBuffer();
  const bytes = new Uint8Array(arrayBuffer);

  await getCurrentWindow().setOverlayIcon(bytes);
}
