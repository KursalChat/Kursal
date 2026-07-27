<script lang="ts">
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
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={toggle}
  class="toggle"
  data-on={checked}
>
  <span class="thumb"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    padding: 0;
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition),
      border-color var(--transition);
  }
  .toggle[data-on='true'] {
    background: var(--accent);
    border-color: var(--accent);
  }
  .toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    transition: transform var(--transition);
  }
  .toggle[data-on='true'] .thumb {
    transform: translateX(16px);
  }
</style>
