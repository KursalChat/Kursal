<script lang="ts">
  import { tick } from 'svelte';
  import Segmented from '$lib/components/settings/Segmented.svelte';
  import { t } from '$lib/i18n';

  let {
    maxUses = $bindable(null),
    ttlSecs = $bindable(null),
    disabled = false,
  }: {
    maxUses: number | null;
    ttlSecs: number | null;
    disabled?: boolean;
  } = $props();

  const MAX_USES = 9999;
  const MAX_DAYS = 3650;
  const USES_PRESETS = [1, 5, 25];
  const DAY_PRESETS = [1, 7, 30];

  const initialDays = ttlSecs === null ? null : Math.max(1, Math.round(ttlSecs / 86400));

  let usesCustom = $state(maxUses !== null && !USES_PRESETS.includes(maxUses));
  let daysCustom = $state(initialDays !== null && !DAY_PRESETS.includes(initialDays));
  let usesDraft = $state(maxUses ?? 10);
  let daysDraft = $state(initialDays ?? 30);
  let usesText = $state(String(maxUses ?? 10));
  let daysText = $state(String(initialDays ?? 30));
  let usesInput = $state<HTMLInputElement | null>(null);
  let daysInput = $state<HTMLInputElement | null>(null);

  const usesSelected = $derived(
    usesCustom ? 'custom' : maxUses === null ? 'unlimited' : String(maxUses)
  );
  const daysSelected = $derived(
    daysCustom
      ? 'custom'
      : ttlSecs === null
        ? 'never'
        : String(Math.max(1, Math.round(ttlSecs / 86400)))
  );

  function clamp(value: number, max: number): number {
    return Math.min(max, Math.max(1, Math.floor(value)));
  }

  async function pickUses(value: string) {
    if (value === 'custom') {
      usesCustom = true;
      maxUses = usesDraft;
      usesText = String(usesDraft);
      await tick();
      usesInput?.select();
      return;
    }
    usesCustom = false;
    maxUses = value === 'unlimited' ? null : Number(value);
  }

  async function pickDays(value: string) {
    if (value === 'custom') {
      daysCustom = true;
      ttlSecs = daysDraft * 86400;
      daysText = String(daysDraft);
      await tick();
      daysInput?.select();
      return;
    }
    daysCustom = false;
    ttlSecs = value === 'never' ? null : Number(value) * 86400;
  }

  // Committed only while the field parses, so clearing it to retype does not
  // snap the value back to 1 mid-edit.
  function onUsesInput(e: Event) {
    usesText = (e.currentTarget as HTMLInputElement).value;
    const parsed = Number(usesText);
    if (usesText.trim() === '' || !Number.isFinite(parsed) || parsed < 1) return;
    usesDraft = clamp(parsed, MAX_USES);
    maxUses = usesDraft;
  }

  function onDaysInput(e: Event) {
    daysText = (e.currentTarget as HTMLInputElement).value;
    const parsed = Number(daysText);
    if (daysText.trim() === '' || !Number.isFinite(parsed) || parsed < 1) return;
    daysDraft = clamp(parsed, MAX_DAYS);
    ttlSecs = daysDraft * 86400;
  }
</script>

<div class="limits">
  <div class="row">
    <span class="title">{t('addContact.ltc.limits.usesLabel')}</span>
    <div class="control">
      <Segmented
        size="sm"
        {disabled}
        value={usesSelected}
        options={[
          { value: '1', label: t('addContact.ltc.limits.uses1') },
          { value: '5', label: t('addContact.ltc.limits.uses5') },
          { value: '25', label: t('addContact.ltc.limits.uses25') },
          { value: 'unlimited', label: t('addContact.ltc.limits.usesUnlimited') },
          { value: 'custom', label: t('addContact.ltc.limits.custom') },
        ]}
        onchange={pickUses}
      />
      {#if usesCustom}
        <div class="custom">
          <input
            type="number"
            inputmode="numeric"
            min="1"
            max={MAX_USES}
            value={usesText}
            oninput={onUsesInput}
            onblur={() => (usesText = String(usesDraft))}
            bind:this={usesInput}
            {disabled}
            aria-label={t('addContact.ltc.limits.usesLabel')}
          />
          <span class="suffix">{t('addContact.ltc.limits.usesSuffix')}</span>
        </div>
      {/if}
    </div>
  </div>

  <div class="row">
    <span class="title">{t('addContact.ltc.limits.expiryLabel')}</span>
    <div class="control">
      <Segmented
        size="sm"
        {disabled}
        value={daysSelected}
        options={[
          { value: '1', label: t('addContact.ltc.limits.expiry1') },
          { value: '7', label: t('addContact.ltc.limits.expiry7') },
          { value: '30', label: t('addContact.ltc.limits.expiry30') },
          { value: 'never', label: t('addContact.ltc.limits.expiryNever') },
          { value: 'custom', label: t('addContact.ltc.limits.custom') },
        ]}
        onchange={pickDays}
      />
      {#if daysCustom}
        <div class="custom">
          <input
            type="number"
            inputmode="numeric"
            min="1"
            max={MAX_DAYS}
            value={daysText}
            oninput={onDaysInput}
            onblur={() => (daysText = String(daysDraft))}
            bind:this={daysInput}
            {disabled}
            aria-label={t('addContact.ltc.limits.expiryLabel')}
          />
          <span class="suffix">{t('addContact.ltc.limits.expirySuffix')}</span>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .limits {
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border-light);
  }

  .row:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .row:first-child {
    padding-top: 0;
  }

  .title {
    flex: 1;
    min-width: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  .control {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
  }

  .custom {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  input {
    width: 76px;
    padding: 5px 8px;
    border: 1px solid var(--accent-selected);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    color: var(--text-primary);
    font-size: var(--text-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    text-align: center;
    -moz-appearance: textfield;
    appearance: textfield;
  }

  input::-webkit-outer-spin-button,
  input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  input:disabled {
    opacity: 0.4;
  }

  .suffix {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  @media (max-width: 560px) {
    .row {
      flex-wrap: wrap;
      row-gap: 10px;
    }

    .control {
      align-items: stretch;
      width: 100%;
    }
  }
</style>
