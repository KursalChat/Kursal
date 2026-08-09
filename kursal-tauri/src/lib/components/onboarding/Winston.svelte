<script lang="ts">
  import { t } from '$lib/i18n';

  type Props = {
    size?: number;
    interactive?: boolean;
    visible?: boolean;
    floatDelay?: number;
    src?: string;
  };

  let {
    size = 200,
    interactive = true,
    visible = true,
    floatDelay = 0,
    src = '/winston.webp',
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

<div
  bind:this={wrap}
  class="winston"
  class:interactive
  class:hidden={!visible}
  class:wiggling
  style="--size: {size}px; --px: {parallaxX}px; --py: {parallaxY}px; --float-delay: {floatDelay}ms;"
  onmouseleave={onLeave}
  onclick={onPoke}
  onanimationend={(e) => {
    if ((e as AnimationEvent).animationName.includes('wiggle')) wiggling = false;
  }}
  role="button"
  tabindex="0"
  onkeydown={(e) => e.key === 'Enter' && onPoke()}
  aria-label="Winston"
>
  <div class="glow"></div>
  <div class="inner">
    <img {src} alt="Winston" draggable="false" />
  </div>

  {#if speech}
    <div class="speech">{speech}</div>
  {/if}
</div>

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

  .winston.interactive:hover .inner {
    transform: scale(1.04);
  }

  .winston img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 0 12px rgba(46, 91, 215, 0.28));
    user-select: none;
    -webkit-user-drag: none;
    transition: filter 500ms ease;
  }

  .winston.interactive:hover img {
    filter: drop-shadow(0 0 18px rgba(123, 163, 247, 0.38));
  }

  .glow {
    position: absolute;
    inset: 12%;
    background: radial-gradient(circle, rgba(46, 91, 215, 0.22) 0%, rgba(46, 91, 215, 0) 70%);
    filter: blur(24px);
    z-index: -1;
    animation: pulse 5s ease-in-out infinite;
    pointer-events: none;
  }

  .speech {
    position: absolute;
    bottom: calc(100% + 14px);
    left: 50%;
    transform: translateX(-50%);
    background: rgba(20, 26, 50, 0.95);
    border: 1px solid rgba(123, 163, 247, 0.35);
    color: #e8eeff;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    white-space: nowrap;
    box-shadow: 0 0 24px rgba(46, 91, 215, 0.4);
    animation: speechIn 220ms ease-out;
    pointer-events: none;
  }

  .speech::after {
    content: '';
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 6px solid transparent;
    border-top-color: rgba(20, 26, 50, 0.95);
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
      opacity: 0.4;
    }
    50% {
      opacity: 0.8;
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
