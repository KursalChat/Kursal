<script lang="ts">
  import type { Snippet } from 'svelte';
  import { log } from '$lib/utils/log';
  import { open } from '@tauri-apps/plugin-dialog';
  import { readFile } from '@tauri-apps/plugin-fs';
  import AvatarCropper from './AvatarCropper.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { t } from '$lib/i18n';

  let {
    onChange,
    children,
  }: {
    onChange: (base64: string, bytes: number[]) => void;
    children: Snippet<[() => void]>;
  } = $props();

  let cropBlob = $state<Blob | null>(null);

  function mimeForExt(path: string): string {
    const ext = path.toLowerCase().split('.').pop() ?? '';
    if (ext === 'png') return 'image/png';
    if (ext === 'webp') return 'image/webp';
    return 'image/jpeg';
  }

  async function openPicker() {
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
      });
      if (!picked || Array.isArray(picked)) return;
      const path = picked as string;
      const data = await readFile(path);
      cropBlob = new Blob([data], { type: mimeForExt(path) });
    } catch (e) {
      log.error('Avatar pick failed', e);
      notifications.push(t('avatar.errorOpenImage'), 'error');
    }
  }

  function handleConfirm(b64: string, bytes: number[]) {
    onChange(b64, bytes);
    cropBlob = null;
  }

  function handleCancel() {
    cropBlob = null;
  }
</script>

{@render children(openPicker)}

{#if cropBlob}
  <AvatarCropper file={cropBlob} onConfirm={handleConfirm} onCancel={handleCancel} />
{/if}
