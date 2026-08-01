<script lang="ts">
  import { AlertTriangle, Check, Copy, ExternalLink, Info, ShieldAlert } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { confirmState } from '$lib/state/confirm.svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { t } from '$lib/i18n';
  import Button from './Button.svelte';

  let holdProgress = $state(1); // 0..1, starts at 0 if holdMs set, else 1 (unlocked)
  let holdRaf = 0;
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyCode(text: string) {
    try {
      await writeText(text);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }

  const holdMs = $derived(confirmState.options?.holdMs ?? 0);
  const locked = $derived(holdMs > 0 && holdProgress < 1);
  const dismissible = $derived(confirmState.options?.dismissible ?? true);

  function onKey(e: KeyboardEvent) {
    if (!confirmState.open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      if (dismissible) confirmState.cancel();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (!locked) confirmState.confirm();
    }
  }

  $effect(() => {
    if (!confirmState.open) return;
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  $effect(() => {
    if (!confirmState.open) {
      cancelAnimationFrame(holdRaf);
      clearTimeout(copyTimer);
      copied = false;
      holdProgress = 1;
      return;
    }
    if (holdMs <= 0) {
      holdProgress = 1;
      return;
    }
    holdProgress = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const p = Math.min(1, (now - start) / holdMs);
      holdProgress = p;
      if (p < 1) holdRaf = requestAnimationFrame(tick);
    };
    holdRaf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(holdRaf);
  });
</script>

{#if confirmState.open && confirmState.options}
  {@const o = confirmState.options}
  {@const tone = o.tone ?? 'default'}
  <div
    class="backdrop"
    onclick={() => dismissible && confirmState.cancel()}
    role="presentation"
  ></div>
  <div
    class="dialog"
    class:wide={!!o.code}
    role="dialog"
    aria-modal="true"
    aria-labelledby="confirm-title"
    tabindex="-1"
    use:trapFocus
  >
    <div class="head" data-tone={tone}>
      <div class="icon-wrap" data-tone={tone}>
        {#if tone === 'danger'}
          <ShieldAlert size={18} />
        {:else if tone === 'warning'}
          <AlertTriangle size={18} />
        {:else}
          <Info size={18} />
        {/if}
      </div>
      <h3 id="confirm-title">{o.title}</h3>
    </div>
    <div class="body">
      {#if o.message}
        <p class="message">{o.message}</p>
      {/if}
      {#if o.detail}
        <p class="detail">{o.detail}</p>
      {/if}
      {#if o.sections?.length}
        <div class="sections">
          {#each o.sections as s (s.title)}
            <div class="section">
              <span class="section-title">{s.title}</span>
              <ul>
                {#each s.items as item (item)}
                  <li>{item}</li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      {/if}
      {#if o.code}
        <div class="code-wrap">
          <pre class="code">{o.code}</pre>
          <button class="copy" type="button" onclick={() => copyCode(o.code ?? '')}>
            {#if copied}
              <Check size={13} />
              <span>{t('common.copied')}</span>
            {:else}
              <Copy size={13} />
              <span>{t('common.copy')}</span>
            {/if}
          </button>
        </div>
      {/if}
      {#if o.link}
        <button class="body-link" type="button" onclick={o.link.onClick}>
          {o.link.label}
          <ExternalLink size={12} />
        </button>
      {/if}
      {#if o.checkbox}
        <label class="checkbox">
          <input
            type="checkbox"
            checked={confirmState.checkboxChecked}
            onchange={(e) => confirmState.setChecked((e.currentTarget as HTMLInputElement).checked)}
          />
          <span>{o.checkbox.label}</span>
        </label>
      {/if}
    </div>
    <div class="foot">
      {#if !o.hideCancel}
        <Button variant="secondary" onclick={() => confirmState.cancel()}>
          {o.cancelLabel ?? 'Cancel'}
        </Button>
      {/if}
      {#if holdMs > 0}
        <button
          class="hold-btn"
          data-tone={tone}
          disabled={locked}
          onclick={() => confirmState.confirm()}
          aria-busy={locked}
        >
          <span class="hold-fill" style="transform: scaleX({holdProgress});" aria-hidden="true"
          ></span>
          <span class="hold-label">{o.confirmLabel ?? 'Confirm'}</span>
        </button>
      {:else}
        <Button
          variant={tone === 'danger' ? 'danger' : 'primary'}
          onclick={() => confirmState.confirm()}
        >
          {o.confirmLabel ?? 'Confirm'}
        </Button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 1000;
    animation: fadein 150ms ease;
  }
  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: calc(100% - 32px);
    max-width: 420px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    z-index: 1001;
    overflow: hidden;
    animation: pop 180ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 20px 14px;
  }
  .head h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .icon-wrap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }
  .icon-wrap[data-tone='default'] {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .icon-wrap[data-tone='warning'] {
    background: rgba(251, 191, 36, 0.15);
    color: var(--warning);
  }
  .icon-wrap[data-tone='danger'] {
    background: var(--danger-dim);
    color: var(--danger);
  }
  .body {
    padding: 0 20px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .message {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.55;
  }
  .detail {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
  }
  .dialog.wide {
    max-width: 560px;
  }
  .sections {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 4px;
    padding: 12px 14px;
    max-height: 260px;
    overflow-y: auto;
    background: var(--surface-soft);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
  }
  .section-title {
    display: block;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }
  .sections ul {
    margin: 6px 0 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .sections li {
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
  .code-wrap {
    position: relative;
    margin-top: 4px;
  }
  .code {
    margin: 0;
    max-height: 240px;
    overflow: auto;
    padding: 10px 12px;
    background: var(--surface-soft);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
  .copy {
    position: absolute;
    top: 6px;
    right: 6px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: color var(--transition);
  }
  .copy:hover {
    color: var(--text-primary);
  }
  .body-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    padding: 0;
    background: none;
    border: none;
    font-size: 13px;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    transition: color var(--transition);
  }
  .body-link:hover {
    color: var(--text-primary);
  }

  .checkbox {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    font-size: 13px;
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
  }
  .checkbox input {
    accent-color: var(--accent-solid);
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    cursor: pointer;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--border-light);
    background: var(--surface-soft);
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translate(-50%, -46%) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }

  .hold-btn {
    position: relative;
    overflow: hidden;
    min-height: 36px;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    border: 1px solid transparent;
    background: var(--accent-solid);
    color: #fff;
    cursor: pointer;
    isolation: isolate;
    transition: opacity var(--transition);
  }
  .hold-btn[data-tone='danger'] {
    background: var(--danger);
    border-color: var(--danger);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .hold-btn:disabled {
    cursor: not-allowed;
    opacity: 0.85;
  }
  .hold-btn:not(:disabled):hover {
    background: color-mix(in srgb, var(--accent-solid), white 10%);
  }
  .hold-btn[data-tone='danger']:not(:disabled):hover {
    background: var(--danger-hover);
  }
  .hold-fill {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.22);
    transform-origin: left center;
    transform: scaleX(0);
    z-index: 0;
    pointer-events: none;
  }
  .hold-label {
    position: relative;
    z-index: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-variant-numeric: tabular-nums;
  }
</style>
