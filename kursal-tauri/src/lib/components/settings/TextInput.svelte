<script lang="ts">
  let {
    value = $bindable(''),
    type = 'text',
    placeholder,
    disabled = false,
    width = '220px',
    min,
    max,
    maxlength,
    onchange,
    onblur,
    ariaLabel,
  }: {
    value?: string;
    type?: 'text' | 'password' | 'number' | 'time';
    placeholder?: string;
    disabled?: boolean;
    width?: string;
    min?: number;
    max?: number;
    maxlength?: number;
    onchange?: (value: string) => void;
    onblur?: (value: string) => void;
    ariaLabel?: string;
  } = $props();
</script>

<input
  class="ks-input"
  style="width: {width}"
  {type}
  {placeholder}
  {disabled}
  {min}
  {max}
  {maxlength}
  aria-label={ariaLabel}
  bind:value
  oninput={(e) => onchange?.((e.currentTarget as HTMLInputElement).value)}
  onblur={(e) => onblur?.((e.currentTarget as HTMLInputElement).value)}
/>

<style>
  .ks-input {
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-primary);
    padding: 7px 10px;
    font-size: 13px;
    outline: none;
    transition: border-color var(--transition);
  }
  .ks-input:focus {
    border-color: var(--accent);
  }
  .ks-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
