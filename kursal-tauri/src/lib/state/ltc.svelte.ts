import { browser } from '$app/environment';
import { listen } from '@tauri-apps/api/event';
import {
  createLtc as apiCreate,
  exportLtc as apiExport,
  getLtcStatus as apiGetStatus,
  republishLtcPointer as apiRepublishPointer,
  revokeLtc as apiRevoke,
  setLtcFollowRotations as apiSetFollowRotations,
  updateLtcLimits as apiUpdateLimits,
} from '$lib/api/ltc';
import { log } from '$lib/utils/log';
import type { LtcStatus } from '$lib/types';

const EXPIRY_SLACK_SECS = 10 * 60;

// A core without the rendezvous record sends neither field. Treating that as an
// unpublished pointer keeps the UI on its pre-rendezvous warnings.
type RawLtcStatus = Omit<LtcStatus, 'followRotations' | 'pointerState'> &
  Partial<Pick<LtcStatus, 'followRotations' | 'pointerState'>>;

function normalize(raw: RawLtcStatus): LtcStatus;
function normalize(raw: RawLtcStatus | null): LtcStatus | null;
function normalize(raw: RawLtcStatus | null): LtcStatus | null {
  if (!raw) return null;
  return {
    ...raw,
    followRotations: raw.followRotations ?? true,
    pointerState: raw.pointerState ?? 'pending',
  };
}

function createLtcState() {
  let status = $state<LtcStatus | null>(null);
  let loading = $state(true);
  // Limits extended past what distributed copies of the file claim. A live
  // rendezvous record does not fix this: copies carry their own expires_at.
  let reshareExpiry = $state(false);
  // Peer ID changed. The record fixes this, and may land after the rotation
  // call returns, so it stays a live condition rather than a decision.
  let reshareRotated = $state(false);
  let pointerBusy = $state(false);
  let initialized = false;

  const survivesRotation = $derived(
    status !== null && status.followRotations && status.pointerState === 'published'
  );
  const reshareNeeded = $derived(reshareExpiry || (reshareRotated && !survivesRotation));

  async function init() {
    if (!browser || initialized) return;
    initialized = true;
    await listen<LtcStatus | null>('ltc_updated', (e) => {
      status = normalize(e.payload);
    });
    await refresh();
  }

  async function refresh() {
    try {
      status = normalize(await apiGetStatus());
    } catch (err) {
      log.error('Could not load LTC status:', err);
      status = null;
    } finally {
      loading = false;
    }
  }

  function clearReshare() {
    reshareExpiry = false;
    reshareRotated = false;
  }

  async function create(maxUses: number | null, ttlSecs: number | null) {
    status = normalize(await apiCreate(maxUses, ttlSecs));
    clearReshare();
  }

  async function updateLimits(maxUses: number | null, ttlSecs: number | null) {
    const previousExpiry = status?.expiresAt ?? null;
    const updated = normalize(await apiUpdateLimits(maxUses, ttlSecs));
    status = updated;
    const extended =
      previousExpiry !== null &&
      (updated.expiresAt === null || updated.expiresAt > previousExpiry + EXPIRY_SLACK_SECS);
    if (extended) reshareExpiry = true;
  }

  async function setFollowRotations(enabled: boolean) {
    pointerBusy = true;
    try {
      status = normalize(await apiSetFollowRotations(enabled));
    } finally {
      pointerBusy = false;
    }
  }

  async function republishPointer() {
    pointerBusy = true;
    try {
      status = normalize(await apiRepublishPointer());
    } finally {
      pointerBusy = false;
    }
  }

  function notePeerIdRotated() {
    if (status) reshareRotated = true;
  }

  async function revoke() {
    await apiRevoke();
    status = null;
    clearReshare();
  }

  async function exportBytes(): Promise<Uint8Array> {
    const bytes = new Uint8Array(await apiExport());
    clearReshare();
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
    get survivesRotation() {
      return survivesRotation;
    },
    get pointerBusy() {
      return pointerBusy;
    },
    init,
    refresh,
    create,
    updateLimits,
    setFollowRotations,
    republishPointer,
    notePeerIdRotated,
    revoke,
    exportBytes,
  };
}

export const ltcState = createLtcState();
