<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { ConnectionChangedPayload } from '$lib/types';

  let {
    name,
    size = 36,
    src = null,
    status = null,
    showStatus = false,
  }: {
    name: string;
    size?: number;
    src?: string | null;
    status?: ConnectionChangedPayload['status'] | null;
    showStatus?: boolean;
  } = $props();

  const hue = $derived([...name].reduce((a, c) => a + c.charCodeAt(0), 0) % 360);
  const initials = $derived(
    name
      .split(' ')
      .map((w) => w[0]?.toUpperCase() ?? '')
      .slice(0, 2)
      .join('')
  );

  const imgSrc = $derived.by(() => {
    if (!src) return null;
    if (src.startsWith('data:')) return src;
    const q = src.lastIndexOf('?');
    if (q === -1) return convertFileSrc(src);
    return convertFileSrc(src.slice(0, q)) + src.slice(q);
  });

  let failedSrc = $state<string | null>(null);
  const showImg = $derived(!!imgSrc && failedSrc !== imgSrc);

  const statusColor = $derived.by(() => {
    if (!status) return 'var(--text-muted)';
    switch (status) {
      case 'direct':
        return 'var(--success)';
      case 'holepunch':
        return 'var(--success)';
      case 'relay':
        return 'var(--info)';
      case 'connecting':
        return 'var(--warning)';
      case 'disconnected':
      default:
        return 'var(--text-muted)';
    }
  });

  const dotSize = $derived(Math.max(8, Math.round(size * 0.28)));
</script>

<div class="avatar-wrap" style="width:{size}px;height:{size}px;">
  <div
    class="avatar"
    style="width:{size}px;height:{size}px;font-size:{size * 0.38}px;
           {showImg ? '' : `background:hsl(${hue},45%,30%);color:hsl(${hue},70%,85%)`}"
  >
    {#if showImg}
      <img
        src={imgSrc}
        alt="{name}'s avatar"
        draggable="false"
        onerror={() => (failedSrc = imgSrc)}
      />
    {:else}
      {initials || '?'}
    {/if}
  </div>
  {#if showStatus}
    <span
      class="avatar-status"
      class:pulsing={status === 'connecting'}
      style="width:{dotSize}px;height:{dotSize}px;background:{statusColor};"
    ></span>
  {/if}
</div>

<style>
  .avatar-wrap {
    position: relative;
    flex-shrink: 0;
    display: inline-block;
  }
  .avatar {
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    flex-shrink: 0;
    overflow: hidden;
    user-select: none;
  }
  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .avatar-status {
    position: absolute;
    right: 0;
    bottom: 0;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--bg-secondary);
    transition: background 180ms ease;
  }
  .avatar-status.pulsing {
    animation: status-pulse 1.4s ease-in-out infinite;
  }
  @keyframes status-pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.55;
      transform: scale(0.85);
    }
  }
</style>
