<script lang="ts">
  type Props = {
    label: string;
    show: boolean;
    onclick: () => void;
    disabled?: boolean;
    arrow?: boolean;
  };

  let { label, show, onclick, disabled = false, arrow = true }: Props = $props();
</script>

<button
  class="next"
  class:show
  {onclick}
  disabled={disabled || !show}
  tabindex={show ? 0 : -1}
  aria-hidden={!show}
>
  <span>{label}</span>
  {#if arrow}
    <svg
      width="17"
      height="17"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2.6"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"><path d="M4 12h15M13 6.5 19 12l-6 5.5" /></svg
    >
  {/if}
</button>

<style>
  .next {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    padding: 12px 22px;
    background: var(--ob-fill);
    color: var(--ob-ink);
    border: 2.5px solid var(--ob-ink);
    border-radius: 14px;
    font-family: inherit;
    font-size: var(--text-md);
    font-weight: 700;
    letter-spacing: -0.01em;
    box-shadow: 3px 3px 0 var(--ob-accent);
    opacity: 0;
    pointer-events: none;
    transform: translateY(6px);
    transition:
      opacity 420ms ease,
      transform 180ms cubic-bezier(0.22, 1, 0.36, 1),
      box-shadow 180ms ease;
  }
  .next.show {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }

  @media (hover: hover) {
    .next:hover:not(:disabled) {
      transform: translate(-1px, -1px);
      box-shadow: 5px 5px 0 var(--ob-accent);
    }
  }

  .next:active:not(:disabled) {
    transform: translate(2px, 2px);
    box-shadow: 1px 1px 0 var(--ob-accent);
  }

  .next:disabled {
    opacity: 0.42;
    pointer-events: none;
  }
  .next.show:disabled {
    opacity: 0.42;
  }

  @media (max-width: 640px) {
    .next {
      padding: 11px 20px;
      font-size: 14.5px;
    }
  }
</style>
