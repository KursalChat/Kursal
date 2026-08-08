<script lang="ts">
  import { t } from '$lib/i18n';

  const PLAIN_TEXT = 'hi there 👋';
  const CIPHER_GLYPHS = ['∆', '§', '≈', '◊', 'Ω', 'π', '∂', 'ƒ', '¥', '±', '∑', '≠'];
  const CIPHER_LEN = 8;
  const CIPHER_TICK_MS = 110;

  const DUR_PLAIN = 1000;
  const DUR_ENCRYPT = 1300;
  const DUR_FLY = 1500;
  const DUR_REST = 700;
  const DUR_LOCKED_IN = 500;

  type Props = {
    onNext: () => void;
    onBack?: () => void;
  };

  let { onNext }: Props = $props();

  type Stage = 'hidden' | 'plain' | 'encrypting' | 'queued' | 'flying';
  type Dir = 'lr' | 'rl';

  let phonesIn = $state(false);
  let serverIn = $state(false);
  let badRouteIn = $state(false);
  let eyeIn = $state(false);
  let shake = $state(false);
  let snap = $state(false);
  let directIn = $state(false);
  let winstonIn = $state(false);
  let messagesRunning = $state(false);
  let showLine1 = $state(false);
  let showLine2 = $state(false);
  let showLine3 = $state(false);
  let showButton = $state(false);

  let stage = $state<Stage>('hidden');
  let dir = $state<Dir>('lr');
  let cipherText = $state('');

  const T = {
    phones: 200,
    badRoute: 900,
    server: 900,
    eye: 1700,
    shake: 2800,
    snap: 4000,
    direct: 4700,
    winston: 5500,
    line1: 6100,
    messages: 6900,
    line2: 8500,
    line3: 10800,
    button: 12200,
  };

  $effect(() => {
    const timers = [
      setTimeout(() => (phonesIn = true), T.phones),
      setTimeout(() => (serverIn = true), T.server),
      setTimeout(() => (badRouteIn = true), T.badRoute),
      setTimeout(() => (eyeIn = true), T.eye),
      setTimeout(() => (shake = true), T.shake),
      setTimeout(() => (snap = true), T.snap),
      setTimeout(() => (directIn = true), T.direct),
      setTimeout(() => (winstonIn = true), T.winston),
      setTimeout(() => (showLine1 = true), T.line1),
      setTimeout(() => (messagesRunning = true), T.messages),
      setTimeout(() => (showLine2 = true), T.line2),
      setTimeout(() => (showLine3 = true), T.line3),
      setTimeout(() => (showButton = true), T.button),
    ];
    return () => timers.forEach(clearTimeout);
  });

  function chars(text: string, step = 28) {
    return text.split('').map((ch, i) => ({ ch, delay: i * step }));
  }

  function randomCipher() {
    let s = '';
    for (let i = 0; i < CIPHER_LEN; i++) {
      s += CIPHER_GLYPHS[Math.floor(Math.random() * CIPHER_GLYPHS.length)];
    }
    return s;
  }

  $effect(() => {
    if (!messagesRunning) return;

    const timers: ReturnType<typeof setTimeout>[] = [];
    let cancelled = false;

    const wait = (ms: number) =>
      new Promise<void>((resolve) => {
        timers.push(setTimeout(resolve, ms));
      });

    const pickDir = (): Dir => (Math.random() < 0.5 ? 'lr' : 'rl');

    const run = async () => {
      dir = 'lr';
      stage = 'plain';
      await wait(DUR_PLAIN);
      if (cancelled) return;

      stage = 'encrypting';
      await wait(DUR_ENCRYPT);
      if (cancelled) return;

      stage = 'flying';
      await wait(DUR_FLY);
      if (cancelled) return;

      stage = 'hidden';
      await wait(DUR_REST);

      while (!cancelled) {
        dir = pickDir();
        stage = 'queued';
        await wait(DUR_LOCKED_IN);
        if (cancelled) return;

        stage = 'flying';
        await wait(DUR_FLY);
        if (cancelled) return;

        stage = 'hidden';
        await wait(DUR_REST);
      }
    };

    run();

    return () => {
      cancelled = true;
      timers.forEach(clearTimeout);
    };
  });

  $effect(() => {
    const s = stage;
    if (s === 'hidden' || s === 'plain') return;
    cipherText = randomCipher();
    const interval = setInterval(() => {
      cipherText = randomCipher();
    }, CIPHER_TICK_MS);
    return () => clearInterval(interval);
  });
</script>

<div class="screen">
  <div
    class="scene"
    class:phones-in={phonesIn}
    class:server-in={serverIn}
    class:bad-route={badRouteIn}
    class:eye-in={eyeIn}
    class:shake
    class:snap
    class:direct={directIn}
    aria-hidden="true"
  >
    <svg viewBox="0 0 1000 440" preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="badLineGrad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stop-color="#ffb07a" />
          <stop offset="100%" stop-color="#ff7a3d" />
        </linearGradient>
        <linearGradient id="goodLineGrad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stop-color="#7ba3f7" />
          <stop offset="50%" stop-color="#5b8def" />
          <stop offset="100%" stop-color="#7ba3f7" />
        </linearGradient>
        <radialGradient id="pulseGood">
          <stop offset="0%" stop-color="rgba(123, 163, 247, 0.6)" />
          <stop offset="100%" stop-color="rgba(123, 163, 247, 0)" />
        </radialGradient>
        <radialGradient id="phoneScreen">
          <stop offset="0%" stop-color="#7ba3f7" stop-opacity="0.55" />
          <stop offset="100%" stop-color="#1e50e5" stop-opacity="0.15" />
        </radialGradient>
        <radialGradient id="flash">
          <stop offset="0%" stop-color="rgba(255, 240, 220, 1)" />
          <stop offset="30%" stop-color="rgba(255, 180, 120, 0.7)" />
          <stop offset="100%" stop-color="rgba(255, 120, 70, 0)" />
        </radialGradient>
      </defs>

      <ellipse class="shadow left" cx="160" cy="385" rx="70" ry="8" fill="rgba(0,0,0,0.4)" />
      <ellipse class="shadow right" cx="840" cy="385" rx="70" ry="8" fill="rgba(0,0,0,0.4)" />

      <g transform="translate(120 230)">
        <g class="phone left-phone">
          <rect
            x="0"
            y="0"
            width="80"
            height="140"
            rx="12"
            fill="#1a1f33"
            stroke="rgba(123,163,247,0.35)"
            stroke-width="1.5"
          />
          <rect x="6" y="10" width="68" height="118" rx="6" fill="url(#phoneScreen)" />
          <rect
            class="phone-bar"
            x="14"
            y="22"
            width="44"
            height="4"
            rx="2"
            fill="rgba(255,255,255,0.35)"
          />
          <rect
            class="phone-bar"
            x="14"
            y="32"
            width="30"
            height="4"
            rx="2"
            fill="rgba(255,255,255,0.22)"
          />
          <rect x="32" y="134" width="16" height="2" rx="1" fill="rgba(255,255,255,0.2)" />
        </g>
      </g>

      <g transform="translate(800 230)">
        <g class="phone right-phone">
          <rect
            x="0"
            y="0"
            width="80"
            height="140"
            rx="12"
            fill="#1a1f33"
            stroke="rgba(123,163,247,0.35)"
            stroke-width="1.5"
          />
          <rect x="6" y="10" width="68" height="118" rx="6" fill="url(#phoneScreen)" />
          <rect
            class="phone-bar"
            x="22"
            y="22"
            width="44"
            height="4"
            rx="2"
            fill="rgba(255,255,255,0.35)"
          />
          <rect
            class="phone-bar"
            x="36"
            y="32"
            width="30"
            height="4"
            rx="2"
            fill="rgba(255,255,255,0.22)"
          />
          <rect x="32" y="134" width="16" height="2" rx="1" fill="rgba(255,255,255,0.2)" />
        </g>
      </g>

      <g class="bad-lines">
        <path
          class="bad-path bad-left"
          d="M 160 230 C 160 110, 360 70, 500 90"
          fill="none"
          stroke="url(#badLineGrad)"
          stroke-width="2"
          stroke-dasharray="6 6"
          stroke-linecap="round"
        />
        <path
          class="bad-path bad-right"
          d="M 500 90 C 640 70, 840 110, 840 230"
          fill="none"
          stroke="url(#badLineGrad)"
          stroke-width="2"
          stroke-dasharray="6 6"
          stroke-linecap="round"
        />
      </g>

      <g class="server" transform="translate(460 60)">
        <ellipse class="server-halo" cx="40" cy="35" rx="80" ry="40" fill="url(#flash)" />
        <rect
          x="0"
          y="8"
          width="80"
          height="60"
          rx="6"
          fill="#1a1f33"
          stroke="rgba(255,140,90,0.4)"
          stroke-width="1.2"
        />
        <rect x="8" y="18" width="64" height="3" rx="1.5" fill="rgba(255,120,70,0.5)" />
        <rect x="8" y="26" width="64" height="3" rx="1.5" fill="rgba(255,120,70,0.3)" />
        <g class="server-eye" transform="translate(40 48)">
          <ellipse
            cx="0"
            cy="0"
            rx="14"
            ry="8"
            fill="#0a0406"
            stroke="rgba(255,140,90,0.5)"
            stroke-width="1"
          />
          <ellipse class="server-iris" cx="0" cy="0" rx="10" ry="6" fill="#ff7a3d" />
          <ellipse class="server-pupil" cx="0" cy="0" rx="2" ry="5" fill="#0a0406" />
        </g>
      </g>

      <circle class="flash" cx="500" cy="95" r="80" fill="url(#flash)" />

      <path
        class="direct-path"
        d="M 200 300 Q 500 250, 800 300"
        fill="none"
        stroke="url(#goodLineGrad)"
        stroke-width="2.5"
        stroke-linecap="round"
      />
      <path
        class="direct-glow"
        d="M 200 300 Q 500 250, 800 300"
        fill="none"
        stroke="rgba(123, 163, 247, 0.4)"
        stroke-width="8"
        stroke-linecap="round"
      />
      <circle
        class="direct-pulse"
        r="4"
        fill="#cfe0ff"
        style="offset-path: path('M 200 300 Q 500 250, 800 300');"
      />
    </svg>

    <div
      class="bubble dir-{dir}"
      class:visible={stage !== 'hidden'}
      class:locked={stage === 'encrypting' || stage === 'queued' || stage === 'flying'}
      class:morphing={stage === 'encrypting'}
      class:flying={stage === 'flying'}
    >
      {#if stage === 'plain'}
        <span class="text plain">{PLAIN_TEXT}</span>
      {:else if stage !== 'hidden'}
        <svg
          class="lock"
          width="11"
          height="11"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="3" y="11" width="18" height="11" rx="2" />
          <path d="M7 11V7a5 5 0 0 1 10 0v4" />
        </svg>
        <span class="text cipher">{cipherText}</span>
      {/if}
    </div>

    {#if winstonIn}
      <div class="winston-here">
        <img src="/winston-key.webp" alt="" draggable="false" />
      </div>
    {/if}
  </div>

  <div class="text-block">
    <div class="line large" class:show={showLine1}>
      {#if showLine1}
        {#each chars(t('onboarding.screen3.line1')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line" class:show={showLine2}>
      {#if showLine2}
        {#each chars(t('onboarding.screen3.line2')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line accent" class:show={showLine3}>
      {#if showLine3}
        {#each chars(t('onboarding.screen3.line3')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>
  </div>

  <div class="actions" class:show={showButton}>
    <button class="next" onclick={onNext} tabindex={showButton ? 0 : -1}>
      <span>{t('onboarding.screen3.cta')}</span>
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"><path d="M5 12h14M13 6l6 6-6 6" /></svg
      >
    </button>
  </div>
</div>

<style>
  .screen {
    width: 100%;
    min-height: 100%;
    display: grid;
    grid-template-rows: 1fr auto auto;
    align-items: center;
    justify-items: center;
    gap: 20px;
    padding: 32px 40px 40px;
    position: relative;
  }

  .scene {
    position: relative;
    width: min(780px, 100%);
    aspect-ratio: 1000 / 440;
    max-height: 44vh;
    animation: sceneFade 900ms ease-out both;
  }

  @keyframes sceneFade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .scene > svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .phone {
    opacity: 0;
    transform-box: fill-box;
    transform-origin: 50% 100%;
    transform: translateY(12px) scale(0.92);
    transition:
      opacity 700ms ease,
      transform 700ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .scene.phones-in .left-phone {
    opacity: 1;
    transform: translateY(0) scale(1) rotate(-3deg);
  }
  .scene.phones-in .right-phone {
    opacity: 1;
    transform: translateY(0) scale(1) rotate(3deg);
  }
  .shadow {
    opacity: 0;
    transition: opacity 700ms ease 100ms;
  }
  .scene.phones-in .shadow {
    opacity: 0.45;
  }

  .phone-bar {
    opacity: 0.9;
    animation: barPulse 2.8s ease-in-out infinite;
  }
  @keyframes barPulse {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 0.9;
    }
  }

  .server {
    opacity: 0;
    transform: translate(460px, 60px) translateY(-12px);
    transition:
      opacity 600ms ease,
      transform 600ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .scene.server-in .server {
    opacity: 1;
    transform: translate(460px, 60px) translateY(0);
    animation: serverFloat 4s ease-in-out 400ms infinite;
  }
  @keyframes serverFloat {
    0%,
    100% {
      transform: translate(460px, 60px) translateY(0);
    }
    50% {
      transform: translate(460px, 60px) translateY(-6px);
    }
  }

  .server-halo {
    opacity: 0;
    transition: opacity 600ms ease;
  }
  .scene.eye-in .server-halo {
    opacity: 0.6;
    animation: haloPulse 2.5s ease-in-out infinite;
  }
  @keyframes haloPulse {
    0%,
    100% {
      opacity: 0.4;
    }
    50% {
      opacity: 0.8;
    }
  }

  .server-eye {
    opacity: 0;
    transform: translate(40px, 48px) scale(0.2);
    transform-origin: 40px 48px;
    transition:
      opacity 500ms ease,
      transform 500ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .scene.eye-in .server-eye {
    opacity: 1;
    transform: translate(40px, 48px) scale(1);
  }
  .server-pupil {
    animation: serverLook 5s ease-in-out infinite;
  }
  @keyframes serverLook {
    0%,
    100% {
      transform: translate(0, 0);
    }
    30% {
      transform: translate(4px, 0);
    }
    60% {
      transform: translate(-4px, 0);
    }
  }

  .scene.shake .server {
    animation: serverShake 600ms ease-in-out infinite;
  }
  @keyframes serverShake {
    0%,
    100% {
      transform: translate(460px, 60px) translate(0, 0) rotate(0);
    }
    20% {
      transform: translate(460px, 60px) translate(-2px, 1px) rotate(-1.5deg);
    }
    40% {
      transform: translate(460px, 60px) translate(2px, -1px) rotate(1.5deg);
    }
    60% {
      transform: translate(460px, 60px) translate(-1px, 2px) rotate(-1deg);
    }
    80% {
      transform: translate(460px, 60px) translate(1px, -2px) rotate(1deg);
    }
  }

  .scene.snap .server {
    animation: serverBreak 700ms cubic-bezier(0.5, 0, 0.8, 0.4) forwards;
  }
  @keyframes serverBreak {
    0% {
      opacity: 1;
      transform: translate(460px, 60px) scale(1) rotate(0);
    }
    30% {
      opacity: 1;
      transform: translate(460px, 52px) scale(1.08) rotate(-3deg);
    }
    100% {
      opacity: 0;
      transform: translate(460px, 120px) scale(0.4) rotate(-25deg);
      filter: blur(2px);
    }
  }

  .bad-path {
    stroke-dasharray: 8 8;
    stroke-dashoffset: 320;
    opacity: 0;
    transition:
      stroke-dashoffset 900ms ease-out,
      opacity 400ms ease;
  }
  .scene.bad-route .bad-path {
    stroke-dashoffset: 0;
    opacity: 0.85;
    animation: badFlow 1.2s linear infinite;
  }
  @keyframes badFlow {
    to {
      stroke-dashoffset: -160;
    }
  }

  .scene.snap .bad-path {
    animation: badSnap 500ms ease-in forwards;
  }
  @keyframes badSnap {
    0% {
      opacity: 0.85;
      transform: translateY(0);
    }
    30% {
      opacity: 1;
    }
    100% {
      opacity: 0;
      transform: translateY(30px);
    }
  }

  /* FLASH */
  .flash {
    opacity: 0;
    transform-origin: 500px 95px;
    transform: scale(0.2);
  }
  .scene.snap .flash {
    animation: flashBurst 800ms ease-out;
  }
  @keyframes flashBurst {
    0% {
      opacity: 0;
      transform: scale(0.2);
    }
    30% {
      opacity: 1;
      transform: scale(1.4);
    }
    100% {
      opacity: 0;
      transform: scale(2.2);
    }
  }

  /* DIRECT PATH */
  .direct-path,
  .direct-glow {
    stroke-dasharray: 700;
    stroke-dashoffset: 700;
    opacity: 0;
    transition:
      stroke-dashoffset 1200ms cubic-bezier(0.22, 1, 0.36, 1),
      opacity 500ms ease;
  }
  .scene.direct .direct-path,
  .scene.direct .direct-glow {
    stroke-dashoffset: 0;
    opacity: 1;
  }
  .direct-glow {
    filter: blur(4px);
    opacity: 0;
  }
  .scene.direct .direct-glow {
    opacity: 1;
    animation: glowPulse 2.4s ease-in-out 1.2s infinite;
  }
  @keyframes glowPulse {
    0%,
    100% {
      opacity: 0.6;
    }
    50% {
      opacity: 1;
    }
  }

  .direct-pulse {
    offset-distance: 0%;
    opacity: 0;
    filter: drop-shadow(0 0 6px rgba(200, 220, 255, 0.9));
  }
  .scene.direct .direct-pulse {
    animation: directPulse 2.2s linear 1.3s infinite;
  }
  @keyframes directPulse {
    0% {
      offset-distance: 0%;
      opacity: 0;
    }
    10% {
      opacity: 1;
    }
    90% {
      opacity: 1;
    }
    100% {
      offset-distance: 100%;
      opacity: 0;
    }
  }

  /* MESSAGE BUBBLE: overlays the SVG, positioned in % of the scene */
  .bubble {
    position: absolute;
    top: 45%;
    transform: translate(-50%, -50%);
    pointer-events: none;
    z-index: 2;
    padding: 4px 9px;
    background: linear-gradient(135deg, #2e5bd7 0%, #1e50e5 100%);
    color: #fff;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    font-size: 11px;
    font-weight: 500;
    box-shadow: 0 0 14px rgba(46, 91, 215, 0.4);
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    opacity: 0;
    visibility: hidden;
    transition: opacity 350ms ease;
  }
  .bubble.dir-lr {
    left: 16%;
  }
  .bubble.dir-rl {
    left: 84%;
  }

  .bubble.visible {
    opacity: 1;
    visibility: visible;
  }

  .bubble.morphing {
    transition:
      opacity 350ms ease,
      background 1100ms ease,
      border-color 1100ms ease,
      border-radius 900ms ease,
      box-shadow 1100ms ease,
      padding 900ms ease;
  }

  .bubble.locked {
    background: linear-gradient(135deg, #1a1f33 0%, #0f1220 100%);
    border-color: rgba(123, 163, 247, 0.5);
    border-radius: 5px;
    padding: 4px 8px;
    box-shadow:
      0 0 18px rgba(123, 163, 247, 0.35),
      inset 0 0 10px rgba(123, 163, 247, 0.15);
  }

  .bubble.dir-lr.flying {
    animation: fly-lr 1500ms cubic-bezier(0.42, 0, 0.58, 1) forwards;
  }
  .bubble.dir-rl.flying {
    animation: fly-rl 1500ms cubic-bezier(0.42, 0, 0.58, 1) forwards;
  }
  @keyframes fly-lr {
    0% {
      left: 16%;
      opacity: 1;
      transform: translate(-50%, -50%) translateY(0);
    }
    40% {
      transform: translate(-50%, -50%) translateY(-12px);
    }
    75% {
      opacity: 1;
    }
    100% {
      left: 84%;
      opacity: 0;
      transform: translate(-50%, -50%) translateY(0);
    }
  }
  @keyframes fly-rl {
    0% {
      left: 84%;
      opacity: 1;
      transform: translate(-50%, -50%) translateY(0);
    }
    40% {
      transform: translate(-50%, -50%) translateY(-12px);
    }
    75% {
      opacity: 1;
    }
    100% {
      left: 16%;
      opacity: 0;
      transform: translate(-50%, -50%) translateY(0);
    }
  }

  .text {
    display: inline-block;
    text-align: center;
  }
  .text.plain {
    min-width: 58px;
  }
  .text.cipher {
    font-family: var(--font-mono);
    font-size: 10px;
    color: #7ba3f7;
    letter-spacing: 0.15em;
    min-width: 70px;
  }

  .lock {
    color: rgba(123, 163, 247, 0.9);
    animation: lockPulse 1.8s ease-in-out infinite;
  }
  @keyframes lockPulse {
    0%,
    100% {
      opacity: 0.65;
    }
    50% {
      opacity: 1;
    }
  }

  .winston-here {
    position: absolute;
    bottom: 12px;
    left: 50%;
    transform: translateX(-50%);
    width: 96px;
    height: 96px;
    z-index: 3;
    animation:
      winstonIn 700ms cubic-bezier(0.22, 1, 0.36, 1) backwards,
      winstonNod 3.2s ease-in-out 700ms infinite;
  }
  .winston-here img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 0 14px rgba(123, 163, 247, 0.45));
    user-select: none;
    -webkit-user-drag: none;
  }
  @keyframes winstonIn {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(14px) scale(0.85);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0) scale(1);
    }
  }
  @keyframes winstonNod {
    0%,
    100% {
      transform: translateX(-50%) rotate(0) translateY(0);
    }
    25% {
      transform: translateX(-50%) rotate(-2deg) translateY(-3px);
    }
    50% {
      transform: translateX(-50%) rotate(1deg) translateY(0);
    }
    75% {
      transform: translateX(-50%) rotate(-1deg) translateY(-1px);
    }
  }

  .text-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    text-align: center;
    max-width: 720px;
    min-height: 110px;
  }

  .line {
    font-size: 16px;
    color: rgba(200, 215, 255, 0.72);
    letter-spacing: 0.005em;
    min-height: 22px;
  }
  .line.large {
    font-size: 22px;
    font-weight: 600;
    color: #e8eeff;
    letter-spacing: -0.01em;
  }
  .line.accent {
    font-size: 18px;
    font-weight: 600;
    color: #a5c4ff;
    letter-spacing: 0.01em;
    text-shadow: 0 0 18px rgba(123, 163, 247, 0.35);
  }

  .char {
    display: inline-block;
    opacity: 0;
    transform: translateY(6px);
    animation: charIn 600ms ease-out forwards;
  }
  @keyframes charIn {
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 500ms ease;
  }
  .actions.show {
    opacity: 1;
    pointer-events: auto;
  }

  .next {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 12px 26px;
    background: linear-gradient(135deg, #2e5bd7 0%, #1e50e5 100%);
    color: #fff;
    border-radius: var(--radius-md);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.01em;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.4),
      0 0 24px rgba(46, 91, 215, 0.55),
      0 8px 24px rgba(0, 0, 0, 0.4);
    transition:
      transform 150ms ease,
      box-shadow 150ms ease;
  }
  .next:hover {
    transform: translateY(-1px);
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.6),
      0 0 32px rgba(46, 91, 215, 0.75),
      0 10px 28px rgba(0, 0, 0, 0.5);
  }
  .next:active {
    transform: translateY(0);
  }

  @media (max-width: 640px) {
    .screen {
      gap: 14px;
      padding: 16px 16px 24px;
    }
    .line.large {
      font-size: 18px;
    }
    .line {
      font-size: 14px;
    }
    .line.accent {
      font-size: 15px;
    }
    .text-block {
      min-height: 96px;
      gap: 4px;
    }
    .winston-here {
      width: 72px;
      height: 72px;
      bottom: 6px;
    }
  }
</style>
