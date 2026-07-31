<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getCallSampleRate,
    setCallSampleRate,
    getVideoQuality,
    setVideoQuality,
    listAudioDevices,
    setAudioDevice,
  } from '$lib/api/call';
  import type { AudioDevices } from '$lib/types';
  import { notifyError } from '$lib/utils/errors';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Select from './Select.svelte';
  import { t } from '$lib/i18n';

  const DEFAULT = '__default__';

  let rate = $state('48000');
  let videoQuality = $state('480');
  let devices = $state<AudioDevices | null>(null);

  onMount(async () => {
    try {
      rate = String(await getCallSampleRate());
    } catch (e) {
      notifyError(e);
    }
    try {
      videoQuality = String(await getVideoQuality());
    } catch (e) {
      notifyError(e);
    }
    try {
      devices = await listAudioDevices();
    } catch (e) {
      notifyError(e);
    }
  });

  const rateOptions = $derived([
    { value: '16000', label: t('chat.call.qualityLow') + ' · 16 kHz' },
    { value: '24000', label: t('chat.call.qualityMedium') + ' · 24 kHz' },
    { value: '48000', label: t('chat.call.qualityHigh') + ' · 48 kHz' },
  ]);

  const videoQualityOptions = $derived([
    { value: '360', label: t('chat.call.qualityLow') + ' · 360p' },
    { value: '480', label: t('chat.call.qualityMedium') + ' · 480p' },
    { value: '720', label: t('chat.call.qualityHigh') + ' · 720p' },
  ]);

  const inputOptions = $derived([
    { value: DEFAULT, label: t('settings.calls.systemDefault') },
    ...(devices?.inputs ?? []).map((n) => ({ value: n, label: n })),
  ]);
  const outputOptions = $derived([
    { value: DEFAULT, label: t('settings.calls.systemDefault') },
    ...(devices?.outputs ?? []).map((n) => ({ value: n, label: n })),
  ]);

  async function handleRateChange(value: string) {
    const prev = rate;
    rate = value;
    try {
      await setCallSampleRate(Number(value));
    } catch (e) {
      rate = prev;
      notifyError(e);
    }
  }

  async function handleVideoQualityChange(value: string) {
    const prev = videoQuality;
    videoQuality = value;
    try {
      await setVideoQuality(Number(value));
    } catch (e) {
      videoQuality = prev;
      notifyError(e);
    }
  }

  async function handleDeviceChange(kind: 'input' | 'output', value: string) {
    if (!devices) return;
    const name = value === DEFAULT ? null : value;
    const prev = kind === 'input' ? devices.selectedInput : devices.selectedOutput;
    devices = {
      ...devices,
      ...(kind === 'input' ? { selectedInput: name } : { selectedOutput: name }),
    };
    try {
      await setAudioDevice(kind, name);
    } catch (e) {
      devices = {
        ...devices,
        ...(kind === 'input' ? { selectedInput: prev } : { selectedOutput: prev }),
      };
      notifyError(e);
    }
  }
</script>

<div class="sec-head">
  <h2>{t('settings.calls.heading')}</h2>
  <p>{t('settings.calls.description')}</p>
</div>

<SettingCard title={t('settings.calls.qualityCard')}>
  <SettingRow title={t('settings.calls.qualityRow')} description={t('settings.calls.qualityHint')}>
    <Select value={rate} options={rateOptions} onchange={handleRateChange} minWidth="180px" />
  </SettingRow>
  <SettingRow
    title={t('settings.calls.videoQualityRow')}
    description={t('settings.calls.videoQualityHint')}
  >
    <Select
      value={videoQuality}
      options={videoQualityOptions}
      onchange={handleVideoQualityChange}
      minWidth="180px"
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title={t('settings.calls.audioCard')}
  description={t('settings.calls.audioDescription')}
>
  <SettingRow title={t('settings.calls.microphoneRow')}>
    <Select
      value={devices?.selectedInput ?? DEFAULT}
      options={inputOptions}
      onchange={(v) => handleDeviceChange('input', v)}
      minWidth="200px"
    />
  </SettingRow>
  <SettingRow title={t('settings.calls.speakerRow')}>
    <Select
      value={devices?.selectedOutput ?? DEFAULT}
      options={outputOptions}
      onchange={(v) => handleDeviceChange('output', v)}
      minWidth="200px"
    />
  </SettingRow>
</SettingCard>
