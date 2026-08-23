import { invoke } from '@tauri-apps/api/core';

export const dialogRespond = (id: number, confirmed: boolean): Promise<void> =>
  invoke('dialog_respond', { id, confirmed });

export const runStartupDialogs = (): Promise<void> => invoke('run_startup_dialogs');
