<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Sequence } from './sequence.svelte';

  type Props = {
    seq: Sequence;
    compact?: boolean;
    scene?: Snippet;
    text?: Snippet;
    actions?: Snippet;
  };

  let { seq, compact = false, scene, text, actions }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowRight') seq.skip();
  }
</script>

<svelte:window onpointerdown={() => seq.skip()} onkeydown={onKey} />

<div class="screen" class:compact>
  <div class="scene">{@render scene?.()}</div>
  <div class="text">{@render text?.()}</div>
  <div class="actions">{@render actions?.()}</div>
</div>

<style>
  .screen {
    width: 100%;
    min-height: 100%;
    display: grid;
    grid-template-rows: 1fr auto auto;
    justify-items: center;
    gap: 26px;
    padding: 44px 40px 40px;
    text-align: center;
  }

  .screen.compact {
    grid-template-rows: auto auto auto;
    align-content: center;
    gap: 20px;
  }

  .scene {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    min-height: 0;
  }

  .text {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    max-width: 460px;
  }

  .actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    min-height: 52px;
  }

  @media (max-width: 640px) {
    .screen {
      gap: 20px;
      padding: 40px 20px 24px;
    }
    .text {
      max-width: 340px;
    }
  }
</style>
