<script lang="ts">
  import { X } from 'lucide-svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { t } from '$lib/i18n';
  import { OS } from '$lib/api/window';

  let { open = $bindable(false), onClose }: { open?: boolean; onClose: () => void } = $props();

  const isMac = OS === 'macos';
  const MOD = isMac ? '⌘' : 'Ctrl';
  const ALT = isMac ? '⌥' : 'Alt';
  const SHIFT = isMac ? '⇧' : 'Shift';

  const sections = $derived([
    {
      title: t('shortcuts.secNavigation'),
      rows: [
        { keys: [MOD, 'K'], label: t('shortcuts.openPalette') },
        { keys: [ALT, '↓'], label: t('shortcuts.nextConversation') },
        { keys: [ALT, '↑'], label: t('shortcuts.prevConversation') },
        { keys: [ALT, SHIFT, '↓'], label: t('shortcuts.nextUnread') },
        { keys: [ALT, SHIFT, '↑'], label: t('shortcuts.prevUnread') },
        { keys: [ALT, '1–9'], label: t('shortcuts.jumpToConversation') },
        { keys: [MOD, '/'], label: t('shortcuts.showHelp') },
      ],
    },
    {
      title: t('shortcuts.secConversation'),
      rows: [
        { keys: [MOD, 'F'], label: t('shortcuts.findInChat') },
        { keys: ['End'], label: t('shortcuts.jumpLatest') },
        { keys: ['PageUp', 'PageDown'], label: t('shortcuts.scroll') },
        { keys: ['A–Z'], label: t('shortcuts.focusComposer') },
      ],
    },
    {
      title: t('shortcuts.secComposer'),
      rows: [
        { keys: ['Enter'], label: t('shortcuts.send') },
        { keys: [SHIFT, 'Enter'], label: t('shortcuts.newline') },
        { keys: ['↑'], label: t('shortcuts.editLast') },
        { keys: [MOD, 'B'], label: t('shortcuts.bold') },
        { keys: [MOD, 'I'], label: t('shortcuts.italic') },
        { keys: [MOD, 'E'], label: t('shortcuts.code') },
        { keys: [MOD, SHIFT, 'K'], label: t('shortcuts.link') },
        { keys: [MOD, SHIFT, 'X'], label: t('shortcuts.strike') },
        { keys: [MOD, SHIFT, 'P'], label: t('shortcuts.spoiler') },
      ],
    },
  ]);

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

{#if open}
  <div class="sh-backdrop" onclick={onClose} role="presentation"></div>
  <div
    class="sh-panel"
    role="dialog"
    aria-modal="true"
    aria-label={t('shortcuts.title')}
    tabindex="-1"
    onkeydown={onKeydown}
    use:trapFocus
  >
    <div class="sh-head">
      <h3>{t('shortcuts.title')}</h3>
      <button class="sh-close" onclick={onClose} aria-label={t('shortcuts.close')}>
        <X size={16} />
      </button>
    </div>
    <div class="sh-body">
      {#each sections as sec (sec.title)}
        <div class="sh-sec">
          <h4>{sec.title}</h4>
          {#each sec.rows as r (r.label)}
            <div class="sh-row">
              <span class="sh-desc">{r.label}</span>
              <span class="sh-keys">
                {#each r.keys as k}
                  <kbd>{k}</kbd>
                {/each}
              </span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .sh-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 2000;
    animation: sh-fade 120ms ease;
  }
  .sh-panel {
    position: fixed;
    top: var(--safe-center-y);
    left: var(--safe-center-x);
    transform: translate(-50%, -50%);
    width: min(540px, calc(var(--safe-w) - 32px));
    max-height: min(80vh, calc(var(--safe-h) - 32px));
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    z-index: 2001;
    overflow: hidden;
    animation: sh-pop 160ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes sh-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes sh-pop {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
  }
  .sh-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 18px 12px;
    border-bottom: 1px solid var(--border-light);
    flex-shrink: 0;
  }
  .sh-head h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .sh-close {
    width: 30px;
    height: 30px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  @media (hover: hover) {
    .sh-close:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .sh-body {
    overflow-y: auto;
    padding: 8px 18px 18px;
  }
  .sh-sec {
    margin-top: 14px;
  }
  .sh-sec h4 {
    margin: 0 0 6px;
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .sh-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 0;
    border-bottom: 1px solid var(--border-light);
  }
  .sh-row:last-child {
    border-bottom: none;
  }
  .sh-desc {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    min-width: 0;
  }
  .sh-keys {
    display: inline-flex;
    gap: 4px;
    flex-shrink: 0;
  }
  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    height: 22px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    box-shadow: 0 1px 0 var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 600;
    color: var(--text-primary);
  }
</style>
