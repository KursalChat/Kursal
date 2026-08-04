import { browser } from '$app/environment';
import { listen } from '@tauri-apps/api/event';
import {
  createLtc as apiCreate,
  exportLtc as apiExport,
  getLtcStatus as apiGetStatus,
  revokeLtc as apiRevoke,
  updateLtcLimits as apiUpdateLimits,
} from '$lib/api/ltc';
import { log } from '$lib/utils/log';
import type { LtcStatus } from '$lib/types';

const EXPIRY_SLACK_SECS = 10 * 60;

function createLtcState() {
  let status = $state<LtcStatus | null>(null);
  let loading = $state(true);
  // Set when limits are extended past what distributed copies of the file claim.
  let reshareNeeded = $state(false);
  let initialized = false;

  async function init() {
    if (!browser || initialized) return;
    initialized = true;
    await listen<LtcStatus | null>('ltc_updated', (e) => {
      status = e.payload;
    });
    await refresh();
  }

  async function refresh() {
    try {
      status = await apiGetStatus();
    } catch (err) {
      log.error('Could not load LTC status:', err);
      status = null;
    } finally {
      loading = false;
    }
  }

  async function create(maxUses: number | null, ttlSecs: number | null) {
    status = await apiCreate(maxUses, ttlSecs);
    reshareNeeded = false;
  }

  async function updateLimits(maxUses: number | null, ttlSecs: number | null) {
    const previousExpiry = status?.expiresAt ?? null;
    status = await apiUpdateLimits(maxUses, ttlSecs);
    const extended =
      previousExpiry !== null &&
      (status.expiresAt === null || status.expiresAt > previousExpiry + EXPIRY_SLACK_SECS);
    if (extended) reshareNeeded = true;
  }

  async function revoke() {
    await apiRevoke();
    status = null;
    reshareNeeded = false;
  }

  async function exportBytes(): Promise<Uint8Array> {
    const bytes = new Uint8Array(await apiExport());
    reshareNeeded = false;
    return bytes;
  }

  return {
    get status() {
      return status;
    },
    get loading() {
      return loading;
    },
    get reshareNeeded() {
      return reshareNeeded;
    },
    init,
    refresh,
    create,
    updateLimits,
    revoke,
    exportBytes,
  };
}

export const ltcState = createLtcState();
