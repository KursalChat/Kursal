import { invoke } from '@tauri-apps/api/core';
import type { ContactResponse, LtcStatus } from '$lib/types';

export const getLtcStatus = (): Promise<LtcStatus | null> => invoke('get_ltc_status');

export const createLtc = (maxUses: number | null, ttlSecs: number | null): Promise<LtcStatus> =>
  invoke('create_ltc', { maxUses, ttlSecs });

export const updateLtcLimits = (
  maxUses: number | null,
  ttlSecs: number | null
): Promise<LtcStatus> => invoke('update_ltc_limits', { maxUses, ttlSecs });

export const setLtcFollowRotations = (enabled: boolean): Promise<LtcStatus> =>
  invoke('set_ltc_follow_rotations', { enabled });

export const republishLtcPointer = (): Promise<LtcStatus> => invoke('republish_ltc_pointer');

// Rust returns Vec<u8> which Tauri serializes as number[]
export const exportLtc = (): Promise<number[]> => invoke('export_ltc');

export const revokeLtc = (): Promise<void> => invoke('revoke_ltc');

export const importLtc = (bytes: number[]): Promise<ContactResponse> =>
  invoke('import_ltc', { bytes });
