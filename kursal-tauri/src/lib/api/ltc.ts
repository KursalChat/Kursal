import { invoke } from '@tauri-apps/api/core';
import type { ContactResponse } from '$lib/types';

// Rust returns Vec<u8> which Tauri serializes as number[]
export const exportLtc = (): Promise<number[]> => invoke('export_ltc');

export const importLtc = (bytes: number[]): Promise<ContactResponse> =>
  invoke('import_ltc', { bytes });
