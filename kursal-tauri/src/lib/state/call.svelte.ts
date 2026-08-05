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
  getVideoQuality,
  openVideoRxChannel,
  requestVideoKeyframe as apiRequestKeyframe,
  startVideo as apiStartVideo,
  stopVideo as apiStopVideo,
} from '$lib/api/call';
import { VideoReceiver, VideoSender, videoSupported } from '$lib/call/video';
import type {
  AudioDevices,
  CallEndedPayload,
  CallIncomingPayload,
  CallLevelPayload,
  CallPeerVoiceStatePayload,
  CallStatePayload,
  CallStatus,
  ConnectionChangedPayload,
  VideoStatePayload,
} from '$lib/types';

const PEER_DROP_GRACE_MS = 5000;
const KEYFRAME_LOSS_WINDOW_MS = 5000;
const KEYFRAME_LOSS_COUNT = 3;

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
  let remoteVideo = $state(false);
  let cameraDenied = $state(false);
  const videoAvailable = videoSupported();
  const sender = videoAvailable ? new VideoSender() : null;
  const receiver = videoAvailable ? new VideoReceiver() : null;
  let rxChannelOpen = false;
  let lastKeyframeReq = 0;
  let peerKeyframeReqs: number[] = [];
  let sendingVp8 = false;
  let remoteFrameSink: ((f: VideoFrame) => void) | null = null;
  // One-shot diagnostics to localise a black remote tile (core -> UI -> decode -> draw).
  let dbgChunk = false;
  let dbgDecode = false;
  let dbgNoSink = false;

  if (receiver) {
    receiver.onFrame = (frame) => {
      if (!dbgDecode) {
        dbgDecode = true;
        // warn so it survives production log gating; temporary diagnostic
        log.warn('[video] first decoded frame', frame.displayWidth, 'x', frame.displayHeight);
      }
      if (remoteFrameSink) remoteFrameSink(frame);
      else {
        if (!dbgNoSink) {
          dbgNoSink = true;
          log.warn('[video] decoded frames but canvas sink not set (overlay closed/minimized?)');
        }
        frame.close();
      }
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

  if (sender) {
    sender.onEnded = () => {
      if (!localVideo) return;
      localVideo = false;
      void apiStopVideo('error').catch(() => {});
    };
    sender.onResolutionChange = () => {
      if (!localVideo) return;
      void startSending(sendingVp8).catch(() => {
        stopLocalVideo();
        void apiStopVideo('error').catch(() => {});
      });
    };
  }

  function setRemoteFrameSink(cb: ((f: VideoFrame) => void) | null) {
    remoteFrameSink = cb;
  }

  function stopLocalVideo() {
    sender?.stop();
    localVideo = false;
  }

  function stopRemoteVideo() {
    receiver?.close();
    remoteVideo = false;
  }

  async function ensureRxChannel() {
    if (rxChannelOpen || !receiver) return;
    rxChannelOpen = true;
    try {
      await openVideoRxChannel((bytes) => {
        if (!dbgChunk) {
          dbgChunk = true;
          // warn so it survives production log gating; temporary diagnostic
          log.warn('[video] first chunk from core', bytes.byteLength, 'bytes');
        }
        receiver.push(bytes);
      });
    } catch {
      rxChannelOpen = false;
    }
  }

  let videoBusy = false;

  async function startSending(forceVp8: boolean) {
    if (!sender || videoBusy) return;
    videoBusy = true;
    try {
      let quality = 480;
      try {
        quality = await getVideoQuality();
      } catch {
        /* default stands */
      }
      const config = await sender.start(quality, forceVp8);
      sendingVp8 = forceVp8;
      await apiStartVideo(config.codec, config.width, config.height);
      localVideo = true;
    } finally {
      videoBusy = false;
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
    try {
      await startSending(false);
    } catch (e) {
      stopLocalVideo();
      if (
        e instanceof DOMException &&
        (e.name === 'NotAllowedError' || e.name === 'SecurityError')
      ) {
        cameraDenied = true;
        notifications.push(t('chat.call.cameraDeniedToast'), 'error');
      } else {
        notifications.push(t('chat.call.videoUnavailableToast'), 'error');
      }
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
      const wasH264 = sender?.codec?.startsWith('avc1') ?? false;
      stopLocalVideo();
      if (wasH264) {
        try {
          await startSending(true);
          return;
        } catch {
          /* fall through to toast */
        }
      }
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

  function reset() {
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
    peerKeyframeReqs = [];
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
    status = p.state;
    if (p.state === 'connected' && startedAt === null) startedAt = Date.now();
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

  // Peer closed their client / dropped off the network: the far side can't send
  // a hangup, so end the call locally once the drop outlasts the grace window.
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
    await listen<CallPeerVoiceStatePayload>('call_peer_voice_state', (e) =>
      applyPeerVoiceState(e.payload)
    );
    await listen('video_congestion', () => sender?.onCongestion());
    await listen('video_keyframe_requested', () => {
      sender?.requestKeyframe();
      const now = Date.now();
      peerKeyframeReqs = peerKeyframeReqs.filter((at) => now - at < KEYFRAME_LOSS_WINDOW_MS);
      peerKeyframeReqs.push(now);
      if (peerKeyframeReqs.length >= KEYFRAME_LOSS_COUNT) {
        peerKeyframeReqs = [];
        sender?.onCongestion();
      }
    });
    document.addEventListener('visibilitychange', () => {
      if (document.hidden && localVideo) void toggleCamera();
    });
  }

  async function start(targetContactId: string) {
    if (status !== 'idle') return;
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
      // Core refuses a second concurrent call, and dial failures land here too;
      // without a toast the overlay just flashes open and vanishes.
      reset();
      notifications.push(t('chat.call.startFailed'), 'error');
      log.error('Call start failed', e);
    }
  }

  async function accept() {
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

  // Deafen implies mute: deafening auto-mutes and remembers it;
  // undeafening restores mic only if we were the ones who muted it.
  async function toggleMute() {
    const next = !muted;
    await setMuted(next);
    deafenAutoMuted = false;
    // Unmuting while deafened also undeafens: you can't talk into a void.
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
    get localStream() {
      return sender?.localStream ?? null;
    },
    toggleCamera,
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
