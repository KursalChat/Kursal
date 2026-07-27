import { getCurrentWindow } from '@tauri-apps/api/window';
import { log } from '$lib/utils/log';
import { platform } from '@tauri-apps/plugin-os';

export const OS = platform();
export const isMobile = OS == 'android' || OS == 'ios';

export async function setBadgeCount(count?: number) {
  let label = count && count > 0 ? count : undefined;

  if (OS == 'windows') {
    if (label == undefined) {
      return await getCurrentWindow().setOverlayIcon(undefined);
    }

    const canvas = document.createElement('canvas');
    canvas.width = 16;
    canvas.height = 16;
    const ctx = canvas.getContext('2d')!;

    // Red circle
    ctx.fillStyle = '#e11d48';
    ctx.beginPath();
    ctx.arc(8, 8, 8, 0, Math.PI * 2);
    ctx.fill();

    // Number text
    ctx.fillStyle = 'white';
    ctx.font = 'bold 10px Arial';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(label > 99 ? '99+' : String(count), 8, 8);

    // Convert to Uint8Array and pass to Tauri
    const blob = await new Promise<Blob | null>((res) => canvas.toBlob(res, 'image/png'));
    if (!blob) return;
    const arrayBuffer = await blob.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    await getCurrentWindow().setOverlayIcon(bytes);
  } else {
    try {
      await getCurrentWindow().setBadgeCount(label);
    } catch (err) {
      log.error('Could not set badge count:', err);
      // ignored: unsupported platform or not in Tauri context
    }
  }
}
