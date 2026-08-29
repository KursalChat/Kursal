import { Channel, invoke } from '@tauri-apps/api/core';
import type { AudioDevices, CameraInfo } from '$lib/types';

export const startCall = (contactId: string): Promise<string> =>
  invoke('start_call', { contactId });

export const acceptCall = (callId: string): Promise<void> => invoke('accept_call', { callId });

export const declineCall = (callId: string): Promise<void> => invoke('decline_call', { callId });

export const hangup = (callId: string): Promise<void> => invoke('hangup', { callId });

export const setMute = (muted: boolean): Promise<void> => invoke('set_mute', { muted });

export const setDeafen = (deafened: boolean): Promise<void> => invoke('set_deafen', { deafened });

export const listAudioDevices = (): Promise<AudioDevices> => invoke('list_audio_devices');

export const setAudioDevice = (kind: 'input' | 'output', name: string | null): Promise<void> =>
  invoke('set_audio_device', { kind, name });

export const getCallSampleRate = (): Promise<number> => invoke('get_call_sample_rate');

export const setCallSampleRate = (rate: number): Promise<void> =>
  invoke('set_call_sample_rate', { rate });

export const startVideo = (): Promise<void> => invoke('start_video');

export const stopVideo = (reason: string): Promise<void> => invoke('stop_video', { reason });

export const requestVideoKeyframe = (): Promise<void> => invoke('request_video_keyframe');

export const listCameras = (): Promise<CameraInfo[]> => invoke('list_cameras');

export const setCamera = (cameraId: string | null): Promise<void> =>
  invoke('set_camera', { cameraId });

export const refreshCameraRotation = (angle: number): Promise<void> =>
  invoke('refresh_camera_rotation', { angle });

export const requestLocalKeyframe = (): Promise<void> => invoke('request_local_keyframe');

function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function chunkChannel(onChunk: (bytes: ArrayBuffer) => void): Channel<ArrayBuffer | string> {
  const channel = new Channel<ArrayBuffer | string>();
  channel.onmessage = (message) => {
    if (typeof message === 'string') {
      onChunk(base64ToBytes(message).buffer as ArrayBuffer);
    } else {
      onChunk(message);
    }
  };
  return channel;
}

export const openVideoRxChannel = (onChunk: (bytes: ArrayBuffer) => void): Promise<void> =>
  invoke('video_rx_channel', { channel: chunkChannel(onChunk) });

export const openVideoTxChannel = (onChunk: (bytes: ArrayBuffer) => void): Promise<void> =>
  invoke('video_tx_channel', { channel: chunkChannel(onChunk) });

export const getVideoQuality = (): Promise<number> => invoke('get_video_quality');

export const setVideoQuality = (quality: number): Promise<void> =>
  invoke('set_video_quality', { quality });
