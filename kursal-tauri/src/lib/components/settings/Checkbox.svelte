<script lang="ts">
  import { Check } from 'lucide-svelte';

  let {
    checked = false,
    disabled = false,
    onchange,
    ariaLabel,
  }: {
    checked?: boolean;
    disabled?: boolean;
    onchange?: (value: boolean) => void;
    ariaLabel?: string;
  } = $props();

  function toggle() {
    if (disabled) return;
    onchange?.(!checked);
  }
</script>

<button
  type="button"
  role="checkbox"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={toggle}
  class="checkbox"
  data-on={checked}
>
  {#if checked}<Check size={12} strokeWidth={3} />{/if}
</button>

<style>
  .checkbox {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1.5px solid var(--border);
    background: var(--bg-input);
    color: #fff;
    padding: 0;
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition),
      border-color var(--transition);
  }
  .checkbox[data-on='true'] {
    background: var(--accent);
    border-color: var(--accent);
  }
  .checkbox:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
