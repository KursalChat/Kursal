import { browser } from '$app/environment';
import { listen } from '@tauri-apps/api/event';
import { notifications } from '$lib/state/notifications.svelte';
import { log } from '$lib/utils/log';
import { isOnlineStatus } from '$lib/utils/presence';
import { t } from '$lib/i18n';
import {
  acceptCall as apiAccept,
  declineCall as apiDecline,
  hangup as apiHangup,
  setMute as apiSetMute,
  setDeafen as apiSetDeafen,
  startCall as apiStartCall,
  listAudioDevices as apiListDevices,
  setAudioDevice as apiSetDevice,
  getCallSampleRate as apiGetSampleRate,
  openVideoRxChannel,
  openVideoTxChannel,
  requestVideoKeyframe as apiRequestKeyframe,
  startVideo as apiStartVideo,
  stopVideo as apiStopVideo,
  listCameras as apiListCameras,
  refreshCameraRotation as apiRefreshCameraRotation,
  requestLocalKeyframe as apiRequestLocalKeyframe,
  setCamera as apiSetCamera,
} from '$lib/api/call';
import { ensurePermission } from '$lib/api/permissions';
import { VideoReceiver, videoSupported } from '$lib/call/video';
import { playSound, stopSound } from '$lib/audio/sounds';
import { clearCallNotification, notifyCall } from '$lib/state/systemNotify.svelte';
import { requestAttention } from '$lib/api/window';
import { contactsState } from '$lib/state/contacts.svelte';
import type {
  AudioDevices,
  CallEndedPayload,
  CallIncomingPayload,
  CallLevelPayload,
  CallPeerVoiceStatePayload,
  CallStatePayload,
  CallStatus,
  CameraInfo,
  ConnectionChangedPayload,
  VideoLocalStatePayload,
  VideoStatePayload,
} from '$lib/types';

const PEER_DROP_GRACE_MS = 5000;

function createCallState() {
  let status = $state<CallStatus>('idle');
  let callId = $state<string | null>(null);
  let contactId = $state<string | null>(null);
  let muted = $state(false);
  let deafened = $state(false);
  let peerMuted = $state(false);
  let peerDeafened = $state(false);
  let expanded = $state(false);
  let startedAt = $state<number | null>(null);
  let micLevel = $state(0);
  let remoteLevel = $state(0);
  let devices = $state<AudioDevices | null>(null);
  let sampleRate = $state<number | null>(null);
  let qualityOverridden = $state(false);
  let initialized = false;
  let wasCaller = false;
  let deafenAutoMuted = false;
  let dropTimer: ReturnType<typeof setTimeout> | null = null;

  let localVideo = $state(false);
  let cameraFacing = $state<string | null>(null);
  let cameras = $state<CameraInfo[]>([]);
  let selectedCameraId = $state<string | null>(null);
  let remoteVideo = $state(false);
  let cameraDenied = $state(false);
  const videoAvailable = videoSupported();
  const receiver = videoAvailable ? new VideoReceiver() : null;
  const localReceiver = videoAvailable ? new VideoReceiver() : null;
  let rxChannelOpen = false;
  let txChannelOpen = false;
  let lastKeyframeReq = 0;
  let lastLocalKeyframeReq = 0;
  let remoteFrameSink: ((f: VideoFrame) => void) | null = null;
  let localFrameSink: ((f: VideoFrame) => void) | null = null;

  if (receiver) {
    receiver.onFrame = (frame) => {
      if (remoteFrameSink) remoteFrameSink(frame);
      else frame.close();
    };
    receiver.onNeedsKeyframe = () => {
      const now = Date.now();
      if (now - lastKeyframeReq < 1000) return;
      lastKeyframeReq = now;
      void apiRequestKeyframe().catch(() => {});
    };
    receiver.onFatal = () => {
      stopRemoteVideo();
      void apiStopVideo('unsupported').catch(() => {});
    };
  }

  if (localReceiver) {
    localReceiver.onFrame = (frame) => {
      if (localFrameSink) localFrameSink(frame);
      else frame.close();
    };
    // The encoder's first keyframe is gone by the time the decoder configures.
    // Without this the preview stays black until the next one (3s on Android).
    localReceiver.onNeedsKeyframe = () => {
      const now = Date.now();
      if (now - lastLocalKeyframeReq < 1000) return;
      lastLocalKeyframeReq = now;
      void apiRequestLocalKeyframe().catch(() => {});
    };
    localReceiver.onFatal = () => {
      stopLocalVideo();
      void apiStopVideo('error').catch(() => {});
    };
  }

  function setRemoteFrameSink(cb: ((f: VideoFrame) => void) | null) {
    remoteFrameSink = cb;
  }

  function setLocalFrameSink(cb: ((f: VideoFrame) => void) | null) {
    localFrameSink = cb;
  }

  function stopRemoteVideo() {
    receiver?.close();
    remoteVideo = false;
  }

  function stopLocalVideo() {
    localReceiver?.close();
    localVideo = false;
    // stop_video drops the core-side forwarder, so the channel cannot be reused.
    txChannelOpen = false;
  }

  async function ensureRxChannel() {
    if (rxChannelOpen || !receiver) return;
    rxChannelOpen = true;
    try {
      await openVideoRxChannel((bytes) => receiver.push(bytes));
    } catch {
      rxChannelOpen = false;
    }
  }

  async function ensureTxChannel() {
    if (txChannelOpen || !localReceiver) return;
    txChannelOpen = true;
    try {
      await openVideoTxChannel((bytes) => localReceiver.push(bytes));
    } catch {
      txChannelOpen = false;
    }
  }

  async function refreshCameras() {
    try {
      cameras = await apiListCameras();
      if (selectedCameraId && !cameras.some((c) => c.id === selectedCameraId)) {
        selectedCameraId = null;
      }
    } catch {
      cameras = [];
    }
  }

  async function toggleCamera() {
    if (!videoAvailable || status !== 'connected') return;
    if (localVideo) {
      stopLocalVideo();
      try {
        await apiStopVideo('toggle');
      } catch {
        /* peer already gone */
      }
      return;
    }
    cameraDenied = false;
    if (!(await ensurePermission('camera'))) {
      cameraDenied = true;
      notifications.push(t('chat.call.cameraDeniedToast'), 'error');
      return;
    }
    try {
      await ensureTxChannel();
      await apiStartVideo();
      void refreshCameras();
    } catch {
      stopLocalVideo();
      notifications.push(t('chat.call.videoUnavailableToast'), 'error');
    }
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('orientationchange', () => {
      if (localVideo) void apiRefreshCameraRotation().catch(() => {});
    });
  }

  async function selectCamera(id: string | null) {
    if (!videoAvailable || id === selectedCameraId) return;
    const previous = selectedCameraId;
    selectedCameraId = id;
    try {
      await apiSetCamera(id);
    } catch {
      selectedCameraId = previous;
      notifications.push(t('chat.call.videoUnavailableToast'), 'error');
    }
  }

  function applyLocalVideoState(p: VideoLocalStatePayload) {
    if (!localReceiver) return;
    if (!p.active || !p.codec) {
      stopLocalVideo();
      return;
    }
    try {
      localReceiver.configure(p.codec, p.width, p.height);
      localVideo = true;
      if (p.cameraId) selectedCameraId = p.cameraId;
      cameraFacing = cameras.find((c) => c.id === selectedCameraId)?.facing ?? null;
    } catch {
      stopLocalVideo();
    }
  }

  async function applyVideoState(p: VideoStatePayload) {
    if (status !== 'connected') return;
    if (callId && p.callId !== callId) return;
    if (p.active && p.codec) {
      if (!receiver) return;
      await ensureRxChannel();
      try {
        receiver.configure(p.codec, p.width, p.height);
      } catch {
        void apiStopVideo('unsupported').catch(() => {});
        return;
      }
      remoteVideo = true;
      return;
    }
    if (p.reason === 'send_rejected') {
      stopLocalVideo();
      notifications.push(t('chat.call.videoUnavailableToast'), 'error');
      return;
    }
    stopRemoteVideo();
  }

  function clearDropTimer() {
    if (dropTimer) {
      clearTimeout(dropTimer);
      dropTimer = null;
    }
  }

  function endRinging() {
    stopSound('ringtone');
    void clearCallNotification();
    void requestAttention(false);
  }

  function reset() {
    endRinging();
    status = 'idle';
    callId = null;
    contactId = null;
    muted = false;
    deafened = false;
    peerMuted = false;
    peerDeafened = false;
    expanded = false;
    startedAt = null;
    micLevel = 0;
    remoteLevel = 0;
    sampleRate = null;
    qualityOverridden = false;
    deafenAutoMuted = false;
    stopLocalVideo();
    stopRemoteVideo();
    cameraDenied = false;
    cameraFacing = null;
    selectedCameraId = null;
    lastKeyframeReq = 0;
    clearDropTimer();
  }

  function applyIncoming(p: CallIncomingPayload) {
    if (status !== 'idle') return;
    callId = p.callId;
    contactId = p.contactId;
    status = 'ringing_in';
    expanded = true;
    wasCaller = false;
    playSound('ringtone', { loop: true });
    void notifyCall(
      contactsState.getById(p.contactId)?.displayName ?? t('notifications.unknownSender')
    );
    void requestAttention(true);
    sampleRate = p.sampleRate;
    qualityOverridden = false;
    void apiGetSampleRate()
      .then((own) => {
        qualityOverridden = p.sampleRate !== own;
      })
      .catch(() => {});
  }

  function applyState(p: CallStatePayload) {
    if (callId && p.callId !== callId) return;
    if (p.state !== 'ringing_in') endRinging();
    status = p.state;
    if (p.state === 'connected' && startedAt === null) {
      startedAt = Date.now();
      if (videoAvailable) void refreshCameras();
    }
    if (p.state === 'ended') reset();
  }

  function applyEnded(p: CallEndedPayload) {
    const caller = wasCaller;
    reset();
    switch (p.reason) {
      case 'declined':
        if (caller) notifications.push(t('chat.call.declined'), 'info');
        break;
      case 'busy':
        if (caller) notifications.push(t('chat.call.busyToast'), 'info');
        break;
      case 'timeout':
        notifications.push(caller ? t('chat.call.noAnswer') : t('chat.call.missed'), 'info');
        break;
    }
  }

  function applyLevel(p: CallLevelPayload) {
    micLevel = p.mic;
    remoteLevel = p.remote;
  }

  function applyPeerVoiceState(p: CallPeerVoiceStatePayload) {
    if (callId && p.callId !== callId) return;
    peerMuted = p.muted;
    peerDeafened = p.deafened;
  }

  function applyConnectionChanged(p: ConnectionChangedPayload) {
    if (!contactId || p.contactId !== contactId) return;
    if (status === 'idle' || status === 'ringing_in') return;
    if (isOnlineStatus(p.status)) {
      clearDropTimer();
      return;
    }
    if (dropTimer) return;
    const peer = contactId;
    dropTimer = setTimeout(() => {
      dropTimer = null;
      if (contactId === peer && status !== 'idle') void end();
    }, PEER_DROP_GRACE_MS);
  }

  async function init() {
    if (!browser || initialized) return;
    initialized = true;
    await listen<CallIncomingPayload>('call_incoming', (e) => applyIncoming(e.payload));
    await listen<CallStatePayload>('call_state', (e) => applyState(e.payload));
    await listen<CallLevelPayload>('call_level', (e) => applyLevel(e.payload));
    await listen<CallEndedPayload>('call_ended', (e) => applyEnded(e.payload));
    await listen<ConnectionChangedPayload>('connection_changed', (e) =>
      applyConnectionChanged(e.payload)
    );
    await listen<VideoStatePayload>('video_state', (e) => void applyVideoState(e.payload));
    await ensureTxChannel();
    await listen<CallPeerVoiceStatePayload>('call_peer_voice_state', (e) =>
      applyPeerVoiceState(e.payload)
    );
    await listen<VideoLocalStatePayload>('video_local_state', (e) =>
      applyLocalVideoState(e.payload)
    );
  }

  async function start(targetContactId: string) {
    if (status !== 'idle') return;
    if (!(await ensurePermission('microphone'))) {
      notifications.push(t('chat.call.micDeniedToast'), 'error');
      return;
    }
    contactId = targetContactId;
    status = 'ringing_out';
    expanded = true;
    wasCaller = true;
    qualityOverridden = false;
    try {
      callId = await apiStartCall(targetContactId);
      try {
        sampleRate = await apiGetSampleRate();
      } catch {
        sampleRate = null;
      }
    } catch (e) {
      reset();
      notifications.push(t('chat.call.startFailed'), 'error');
      log.error('Call start failed', e);
    }
  }

  async function accept() {
    if (!callId || status !== 'ringing_in') return;
    if (!(await ensurePermission('microphone'))) {
      notifications.push(t('chat.call.micDeniedToast'), 'error');
      await decline();
      return;
    }

    if (!callId || status !== 'ringing_in') return;
    try {
      await apiAccept(callId);
    } catch {
      reset();
    }
  }

  async function decline() {
    if (!callId) return;
    const id = callId;
    reset();
    try {
      await apiDecline(id);
    } catch {
      /* peer already gone */
    }
  }

  async function end() {
    if (status === 'idle') return;
    const id = callId;
    reset();
    try {
      await apiHangup(id ?? '');
    } catch {
      /* peer already gone */
    }
  }

  async function setMuted(next: boolean) {
    if (muted === next) return;
    muted = next;
    try {
      await apiSetMute(next);
    } catch {
      muted = !next;
    }
  }

  async function setDeafened(next: boolean) {
    if (deafened === next) return;
    deafened = next;
    try {
      await apiSetDeafen(next);
    } catch {
      deafened = !next;
    }
  }

  async function toggleMute() {
    const next = !muted;
    await setMuted(next);
    deafenAutoMuted = false;
    if (!next && deafened) await setDeafened(false);
  }

  async function toggleDeafen() {
    const next = !deafened;
    await setDeafened(next);
    if (next) {
      if (!muted) {
        deafenAutoMuted = true;
        await setMuted(true);
      }
    } else if (deafenAutoMuted) {
      deafenAutoMuted = false;
      await setMuted(false);
    }
  }

  function minimize() {
    expanded = false;
  }

  function maximize() {
    expanded = true;
  }

  async function refreshDevices() {
    try {
      devices = await apiListDevices();
    } catch {
      devices = null;
    }
  }

  async function selectDevice(kind: 'input' | 'output', name: string | null) {
    if (!devices) return;
    const prev = kind === 'input' ? devices.selectedInput : devices.selectedOutput;
    devices = {
      ...devices,
      ...(kind === 'input' ? { selectedInput: name } : { selectedOutput: name }),
    };
    try {
      await apiSetDevice(kind, name);
    } catch {
      devices = {
        ...devices,
        ...(kind === 'input' ? { selectedInput: prev } : { selectedOutput: prev }),
      };
    }
  }

  return {
    get status() {
      return status;
    },
    get callId() {
      return callId;
    },
    get contactId() {
      return contactId;
    },
    get muted() {
      return muted;
    },
    get deafened() {
      return deafened;
    },
    get peerMuted() {
      return peerMuted;
    },
    get peerDeafened() {
      return peerDeafened;
    },
    get expanded() {
      return expanded;
    },
    get startedAt() {
      return startedAt;
    },
    get micLevel() {
      return micLevel;
    },
    get remoteLevel() {
      return remoteLevel;
    },
    get devices() {
      return devices;
    },
    get sampleRate() {
      return sampleRate;
    },
    get qualityOverridden() {
      return qualityOverridden;
    },
    get isActive() {
      return status === 'connecting' || status === 'connected';
    },
    get isRinging() {
      return status === 'ringing_in' || status === 'ringing_out';
    },
    get localVideo() {
      return localVideo;
    },
    get remoteVideo() {
      return remoteVideo;
    },
    get cameraDenied() {
      return cameraDenied;
    },
    get videoAvailable() {
      return videoAvailable;
    },
    get canSendVideo() {
      return videoAvailable && cameras.length > 0;
    },
    get cameraFacing() {
      return cameraFacing;
    },
    get cameras() {
      return cameras;
    },
    get selectedCameraId() {
      return selectedCameraId;
    },
    toggleCamera,
    selectCamera,
    refreshCameras,
    setLocalFrameSink,
    setRemoteFrameSink,
    init,
    start,
    accept,
    decline,
    end,
    toggleMute,
    toggleDeafen,
    minimize,
    maximize,
    refreshDevices,
    selectDevice,
    applyIncoming,
    applyState,
    applyEnded,
    applyLevel,
  };
}

export const callState = createCallState();
