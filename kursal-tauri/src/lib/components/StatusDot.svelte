<script lang="ts">
  import type { ConnectionChangedPayload } from '$lib/types';
  let { status, label }: { status: ConnectionChangedPayload['status']; label?: string } = $props();
</script>

<span class="status-dot" data-status={status ?? 'disconnected'} title={label}></span>

<style>
  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    display: inline-block;
    border: 2px solid var(--bg-secondary);
    box-sizing: content-box;
    background: var(--text-muted);
  }

  .status-dot[data-status='direct'],
  .status-dot[data-status='holepunch'] {
    background: var(--success);
  }
  .status-dot[data-status='relay'] {
    background: var(--info);
  }
  .status-dot[data-status='connecting'] {
    background: var(--warning);
  }
  .status-dot[data-status='disconnected'] {
    background: var(--text-muted);
  }

  .status-dot[data-status='connecting'] {
    animation: status-breathe 1.6s ease-in-out infinite;
  }
  @keyframes status-breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
</style>
