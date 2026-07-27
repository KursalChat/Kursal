import { Channel, invoke, type InvokeArgs } from '@tauri-apps/api/core';
import type { AudioDevices } from '$lib/types';
import { OS } from '$lib/api/window';

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

export const startVideo = (codec: string, width: number, height: number): Promise<void> =>
  invoke('start_video', { codec, width, height });

export const stopVideo = (reason: string): Promise<void> => invoke('stop_video', { reason });

export const requestVideoKeyframe = (): Promise<void> => invoke('request_video_keyframe');

function bytesToBase64(u8: Uint8Array): string {
  const CHUNK_SIZE = 0x8000;
  let binary = '';
  for (let i = 0; i < u8.length; i += CHUNK_SIZE) {
    binary += String.fromCharCode(...u8.subarray(i, i + CHUNK_SIZE));
  }
  return btoa(binary);
}

function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export const sendVideoChunk = (chunk: Uint8Array): Promise<void> =>
  OS === 'android'
    ? invoke('send_video_chunk', bytesToBase64(chunk) as unknown as InvokeArgs)
    : invoke('send_video_chunk', chunk);

export const openVideoRxChannel = (onChunk: (bytes: ArrayBuffer) => void): Promise<void> => {
  const channel = new Channel<ArrayBuffer | string>();
  channel.onmessage = (message) => {
    if (typeof message === 'string') {
      onChunk(base64ToBytes(message).buffer as ArrayBuffer);
    } else {
      onChunk(message);
    }
  };
  return invoke('video_rx_channel', { channel });
};

export const getVideoQuality = (): Promise<number> => invoke('get_video_quality');

export const setVideoQuality = (quality: number): Promise<void> =>
  invoke('set_video_quality', { quality });
