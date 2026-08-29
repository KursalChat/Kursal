<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    show: boolean;
    variant?: 'lead' | 'body' | 'accent';
    children: Snippet;
  };

  let { show, variant = 'body', children }: Props = $props();
</script>

<p class="line {variant}" class:show>{@render children()}</p>

<style>
  .line {
    margin: 0;
    opacity: 0;
    transform: translateY(8px);
    transition:
      opacity 520ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 520ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .line.show {
    opacity: 1;
    transform: translateY(0);
  }

  .lead {
    font-size: 25px;
    font-weight: 700;
    letter-spacing: -0.025em;
    line-height: 1.2;
    color: var(--ob-text);
  }

  .body {
    font-size: 16px;
    line-height: 1.55;
    color: var(--ob-text-dim);
  }

  .accent {
    font-size: var(--text-lg);
    font-weight: 600;
    line-height: 1.45;
    color: var(--ob-accent);
  }

  @media (max-width: 640px) {
    .lead {
      font-size: 21px;
    }
    .body {
      font-size: var(--text-md);
    }
    .accent {
      font-size: var(--text-md);
    }
  }
</style>
