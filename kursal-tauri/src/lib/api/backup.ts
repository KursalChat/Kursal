import { invoke } from '@tauri-apps/api/core';

// Full encrypted backup of identity + contacts + messages + settings.
// Password derives a wrapping key (Argon2id) over the AES-GCM ciphertext.
export const exportBackup = (password: string): Promise<number[]> =>
  invoke('export_backup', { password });

export const importBackup = (bytes: number[], password: string): Promise<void> =>
  invoke('import_backup', { bytes, password });
