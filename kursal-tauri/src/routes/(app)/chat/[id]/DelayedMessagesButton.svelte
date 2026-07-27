<script lang="ts">
  import { ArrowUp } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  interface Props {
    count: number;
    onClick: () => void;
  }

  let { count, onClick }: Props = $props();
</script>

{#if count > 0}
  <button class="delayed-jump" onclick={onClick}>
    <ArrowUp size={14} />
    <span>{t('chat.delayedMessages', { count })}</span>
  </button>
{/if}

<style>
  .delayed-jump {
    position: absolute;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #fff;
    border: none;
    font-size: 12.5px;
    font-weight: 600;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    cursor: pointer;
    z-index: 5;
    animation: slideDown 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .delayed-jump:hover {
    background: var(--accent-hover);
  }

  @keyframes slideDown {
    from {
      opacity: 0;
      transform: translate(-50%, -6px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }
</style>
