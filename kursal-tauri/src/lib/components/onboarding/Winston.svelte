<script lang="ts">
  import { t } from '$lib/i18n';

  type Props = {
    size?: number;
    interactive?: boolean;
    visible?: boolean;
    floatDelay?: number;
    src?: string;
    says?: string | null;
  };

  let {
    size = 200,
    interactive = true,
    visible = true,
    floatDelay = 0,
    src = '/winston.webp',
    says = null,
  }: Props = $props();

  let wrap = $state<HTMLDivElement>();
  let parallaxX = $state(0);
  let parallaxY = $state(0);
  let pokeCount = $state(0);
  let speech = $state<string | null>(null);
  let speechTimer: ReturnType<typeof setTimeout> | null = null;
  let wiggling = $state(false);

  const POKE_LINE_COUNT = 5;

  function onMove(e: MouseEvent) {
    if (!interactive || !wrap) return;
    const r = wrap.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const dx = (e.clientX - cx) / Math.max(r.width, 1);
    const dy = (e.clientY - cy) / Math.max(r.height, 1);
    parallaxX = Math.max(-1.2, Math.min(1.2, dx)) * 4;
    parallaxY = Math.max(-1.2, Math.min(1.2, dy)) * 4;
  }

  function onLeave() {
    parallaxX = 0;
    parallaxY = 0;
  }

  function say(line: string, ms = 1800) {
    speech = line;
    if (speechTimer) clearTimeout(speechTimer);
    speechTimer = setTimeout(() => (speech = null), ms);
  }

  const styleVars = $derived(
    `--size: ${size}px; --px: ${parallaxX}px; --py: ${parallaxY}px; --float-delay: ${floatDelay}ms;`
  );

  function onPoke() {
    if (!interactive) return;
    wiggling = false;
    requestAnimationFrame(() => (wiggling = true));
    // i18n keys built dynamically below (keeps checkTranslations from flagging them):
    // winston.poke0 winston.poke1 winston.poke2 winston.poke3 winston.poke4
    const line = t('winston.poke' + Math.min(pokeCount, POKE_LINE_COUNT - 1));
    pokeCount += 1;
    say(line);
  }
</script>

<svelte:window onmousemove={onMove} />

{#snippet body()}
  <div class="glow"></div>
  <div class="inner">
    <img {src} alt="" draggable="false" />
    {#if speech ?? says}
      <div class="speech">{speech ?? says}</div>
    {/if}
  </div>
{/snippet}

{#if interactive}
  <div
    bind:this={wrap}
    class="winston interactive"
    class:hidden={!visible}
    class:wiggling
    style={styleVars}
    onmouseleave={onLeave}
    onclick={onPoke}
    onanimationend={(e) => {
      if ((e as AnimationEvent).animationName.includes('wiggle')) wiggling = false;
    }}
    onkeydown={(e) => e.key === 'Enter' && onPoke()}
    role="button"
    tabindex="0"
    aria-label="Winston"
  >
    {@render body()}
  </div>
{:else}
  <div class="winston" class:hidden={!visible} style={styleVars} aria-hidden="true">
    {@render body()}
  </div>
{/if}

<style>
  .winston {
    width: var(--size);
    height: var(--size);
    position: relative;
    transform: translate(var(--px, 0px), var(--py, 0px));
    transition:
      transform 600ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 400ms ease;
    cursor: pointer;
    animation: appear 1100ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes appear {
    from {
      opacity: 0;
      transform: translate(var(--px, 0px), calc(var(--py, 0px) + 24px)) scale(0.9);
    }
    to {
      opacity: 1;
      transform: translate(var(--px, 0px), var(--py, 0px)) scale(1);
    }
  }

  .winston.hidden {
    opacity: 0;
    pointer-events: none;
  }

  .inner {
    position: relative;
    width: 100%;
    height: 100%;
    animation: float 5s ease-in-out var(--float-delay, 0ms) infinite;
    transition: transform 500ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .wiggling .inner {
    animation:
      float 5s ease-in-out var(--float-delay, 0ms) infinite,
      wiggle 550ms ease-in-out;
  }

  @media (hover: hover) {
    .winston.interactive:hover .inner {
      transform: scale(1.04);
    }
  }

  .winston img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    user-select: none;
    -webkit-user-drag: none;
  }

  .glow {
    position: absolute;
    inset: 14%;
    background: color-mix(in srgb, var(--ob-accent, #4d8dff) 16%, transparent);
    border-radius: 50%;
    filter: blur(26px);
    z-index: -1;
    animation: pulse 5s ease-in-out infinite;
    pointer-events: none;
  }

  .speech {
    position: absolute;
    bottom: calc(100% + 12px);
    left: 50%;
    transform: translateX(-50%);
    background: var(--ob-fill, #eef2f8);
    border: 2.5px solid var(--ob-ink, #0d1017);
    color: var(--ob-ink, #0d1017);
    padding: 7px 13px;
    border-radius: 12px;
    font-size: var(--text-sm);
    font-weight: 600;
    white-space: nowrap;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    box-shadow: 2px 2px 0 var(--ob-accent, #4d8dff);
    animation: speechIn 220ms cubic-bezier(0.22, 1, 0.36, 1);
    pointer-events: none;
  }

  .speech::after,
  .speech::before {
    content: '';
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 8px solid transparent;
  }
  .speech::before {
    border-top-color: var(--ob-ink, #0d1017);
  }
  .speech::after {
    margin-top: -3.5px;
    border-width: 7px;
    border-top-color: var(--ob-fill, #eef2f8);
  }

  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-8px);
    }
  }

  @keyframes wiggle {
    0%,
    100% {
      transform: rotate(0);
    }
    25% {
      transform: rotate(-5deg);
    }
    50% {
      transform: rotate(4deg);
    }
    75% {
      transform: rotate(-2deg);
    }
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.45;
    }
    50% {
      opacity: 0.85;
    }
  }

  @keyframes speechIn {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
  }
</style>
