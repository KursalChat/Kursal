<script lang="ts">
  import { goto } from '$app/navigation';
  import { t } from '$lib/i18n';
  import { ensurePermission } from '$lib/api/system-notify';
  import { ChevronLeft } from 'lucide-svelte';
  import InkDefs from '$lib/components/onboarding/InkDefs.svelte';
  import ScreenHello from '$lib/components/onboarding/ScreenHello.svelte';
  import ScreenMiddleman from '$lib/components/onboarding/ScreenMiddleman.svelte';
  import ScreenDirect from '$lib/components/onboarding/ScreenDirect.svelte';
  import ScreenNoAccounts from '$lib/components/onboarding/ScreenNoAccounts.svelte';
  import ScreenProfile from '$lib/components/onboarding/ScreenProfile.svelte';

  const TOTAL = 5;

  let screen = $state(1);

  async function finish() {
    localStorage.setItem('kursal_onboarded', 'done');
    await ensurePermission().catch(() => false);
    goto('/');
  }

  function goToLast() {
    screen = TOTAL;
  }

  const back = () => (screen -= 1);
</script>

<div class="onboarding">
  <InkDefs />
  <div class="backdrop" aria-hidden="true">
    <div class="grain"></div>
  </div>
  <div class="mac-drag" data-tauri-drag-region aria-hidden="true"></div>

  <div class="progress">
    {#if screen > 1}
      <button class="back" onclick={back} aria-label={t('onboarding.back')}>
        <ChevronLeft size={17} strokeWidth={2.4} />
      </button>
    {:else}
      <span class="spacer"></span>
    {/if}
    <div class="dashes" aria-hidden="true">
      {#each Array(TOTAL) as _, i}
        <span class="dash" class:active={screen === i + 1} class:done={screen > i + 1}></span>
      {/each}
    </div>
    <span class="spacer"></span>
  </div>

  {#if screen > 1 && screen < TOTAL}
    <button class="nav skip" onclick={goToLast}>{t('onboarding.skip')}</button>
  {/if}

  <div class="stage">
    {#if screen === 1}
      <div class="screen-wrap">
        <ScreenHello onNext={() => (screen = 2)} onSkip={goToLast} />
      </div>
    {:else if screen === 2}
      <div class="screen-wrap">
        <ScreenMiddleman onNext={() => (screen = 3)} />
      </div>
    {:else if screen === 3}
      <div class="screen-wrap">
        <ScreenDirect onNext={() => (screen = 4)} />
      </div>
    {:else if screen === 4}
      <div class="screen-wrap">
        <ScreenNoAccounts onNext={() => (screen = 5)} />
      </div>
    {:else if screen === 5}
      <div class="screen-wrap">
        <ScreenProfile onFinish={finish} />
      </div>
    {/if}
  </div>
</div>

<style>
  .onboarding {
    --ob-ink: #0d1017;
    --ob-fill: #eef2f8;
    --ob-bg: #0a0e15;
    --ob-text: #eef3fa;
    --ob-text-dim: #93a1ba;
    --ob-accent: #4d8dff;
    --ob-warn: #d34534;

    position: absolute;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    align-items: stretch;
    justify-content: center;
    -webkit-overflow-scrolling: touch;
    padding-top: var(--safe-top);
    padding-bottom: var(--safe-bottom);
    padding-left: var(--safe-left);
    padding-right: var(--safe-right);
  }

  .mac-drag {
    display: none;
  }
  :global(html.mac) .mac-drag {
    display: block;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 28px;
    z-index: 9999;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background: var(--ob-bg);
  }

  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.045;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 1 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
  }

  .progress {
    position: fixed;
    top: calc(var(--safe-top) + 12px);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    z-index: 5;
  }

  .dashes {
    display: flex;
    align-items: center;
    gap: 6px;
    pointer-events: none;
  }

  .back,
  .spacer {
    width: 26px;
    height: 26px;
    flex: none;
  }

  .back {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    color: var(--ob-text-dim);
    opacity: 0.55;
    transition:
      opacity 150ms ease,
      color 150ms ease,
      background 150ms ease;
  }
  .back:hover {
    opacity: 1;
    color: var(--ob-text);
    background: color-mix(in srgb, var(--ob-fill) 10%, transparent);
  }

  /* Never animate the width: it re-centres the row and the bar jitters sideways. */
  .dash {
    width: 22px;
    height: 3px;
    border-radius: 2px;
    background: var(--ob-fill);
    opacity: 0.15;
    transition:
      opacity 0.3s ease,
      background 0.3s ease;
  }
  .dash.done {
    background: var(--ob-accent);
    opacity: 0.45;
  }
  .dash.active {
    background: var(--ob-accent);
    opacity: 1;
  }

  .nav {
    position: fixed;
    top: calc(var(--safe-top) + 10px);
    z-index: 6;
    display: inline-flex;
    align-items: center;
    color: var(--ob-text-dim);
    opacity: 0.55;
    padding: 6px 10px;
    font-size: 13px;
    letter-spacing: -0.01em;
    transition:
      opacity 150ms ease,
      color 150ms ease;
    animation: navIn 400ms ease-out;
  }
  .nav:hover {
    opacity: 1;
    color: var(--ob-text);
  }
  .skip {
    right: max(10px, var(--safe-right));
  }

  @keyframes navIn {
    from {
      opacity: 0;
    }
  }

  .stage {
    position: relative;
    z-index: 1;
    width: 100%;
    min-height: 100%;
    display: flex;
  }

  .screen-wrap {
    width: 100%;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    animation: screenFade 420ms ease-out;
  }

  .screen-wrap > :global(*) {
    flex: 1 0 auto;
    min-height: 0;
  }

  @keyframes screenFade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
