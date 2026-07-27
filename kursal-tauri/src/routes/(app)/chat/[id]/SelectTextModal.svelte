<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { X } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  interface Props {
    text: string;
    onClose: () => void;
  }

  let { text, onClose }: Props = $props();
  let textEl: HTMLDivElement | null = $state(null);

  onMount(() => {
    void tick().then(() => {
      if (!textEl) return;
      const range = document.createRange();
      range.selectNodeContents(textEl);
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(range);
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
</script>

<div
  class="select-backdrop"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
  role="button"
  tabindex="-1"
  aria-label={t('chat.selectText.dialogAriaLabel')}
></div>
<div
  class="select-sheet"
  role="dialog"
  aria-modal="true"
  aria-label={t('chat.selectText.dialogAriaLabel')}
>
  <div class="select-head">
    <span class="select-title">{t('chat.selectText.title')}</span>
    <button class="select-close" onclick={onClose} aria-label={t('chat.selectText.close')}>
      <X size={18} />
    </button>
  </div>
  <div class="select-hint">{t('chat.selectText.hint')}</div>
  <div class="select-body" bind:this={textEl} role="textbox" aria-readonly="true" tabindex="-1">
    {text}
  </div>
</div>

<style>
  .select-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 320;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .select-sheet {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(520px, calc(100% - 32px));
    max-height: min(70vh, 560px);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 14px;
    z-index: 330;
    animation: popIn 0.18s cubic-bezier(0.3, 0, 0.2, 1);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  @keyframes popIn {
    from {
      transform: translate(-50%, -50%) scale(0.94);
      opacity: 0;
    }
    to {
      transform: translate(-50%, -50%) scale(1);
      opacity: 1;
    }
  }

  .select-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .select-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .select-close {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }
  .select-close:active {
    background: var(--bg-hover);
  }

  .select-hint {
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .select-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px;
    color: var(--text-primary);
    font-size: 15px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
    -webkit-touch-callout: default;
    outline: none;
  }
</style>
