<script lang="ts">
  import { t } from '$lib/i18n';

  const SLOT_COUNT = 3;

  const T = {
    scene: 200,
    rack: 600,
    tumbleweed: 2000,
    powerDying: 3500,
    powerDown: 4500,
    winston: 4900,
    line1: 5600,
    line2: 7200,
    line3: 8900,
    button: 10100,
  };

  type Props = {
    onNext: () => void;
    onBack?: () => void;
  };

  let { onNext }: Props = $props();

  let sceneIn = $state(false);
  let rackIn = $state(false);
  let powerDying = $state(false);
  let poweredDown = $state(false);
  let tumbleweedIn = $state(false);
  let winstonIn = $state(false);
  let showLine1 = $state(false);
  let showLine2 = $state(false);
  let showLine3 = $state(false);
  let showButton = $state(false);

  $effect(() => {
    const timers = [
      setTimeout(() => (sceneIn = true), T.scene),
      setTimeout(() => (rackIn = true), T.rack),
      setTimeout(() => (powerDying = true), T.powerDying),
      setTimeout(() => (poweredDown = true), T.powerDown),
      setTimeout(() => (tumbleweedIn = true), T.tumbleweed),
      setTimeout(() => (winstonIn = true), T.winston),
      setTimeout(() => (showLine1 = true), T.line1),
      setTimeout(() => (showLine2 = true), T.line2),
      setTimeout(() => (showLine3 = true), T.line3),
      setTimeout(() => (showButton = true), T.button),
    ];
    return () => timers.forEach(clearTimeout);
  });

  function chars(text: string, step = 28) {
    return text.split('').map((ch, i) => ({ ch, delay: i * step }));
  }

  const slots = Array.from({ length: SLOT_COUNT }, (_, i) => i);
</script>

<div class="screen-vignette" class:in={sceneIn} aria-hidden="true"></div>

<div class="screen">
  <div
    class="scene"
    class:in={sceneIn}
    class:rack-in={rackIn}
    class:power-dying={powerDying && !poweredDown}
    class:powered-down={poweredDown}
    class:tumbleweed-rolling={tumbleweedIn}
    aria-hidden="true"
  >
    <div class="stage">
      <div class="rack">
        <div class="rack-top">
          <span class="screw"></span>
          <span class="screw"></span>
        </div>
        <div class="rack-body">
          <div class="status-bar">
            <span
              class="status-led"
              class:dying={powerDying && !poweredDown}
              class:dead={poweredDown}
            ></span>
            <span
              class="status-text"
              class:dying={powerDying && !poweredDown}
              class:dead={poweredDown}
              >{poweredDown
                ? t('onboarding.screen4.serverOffline')
                : powerDying
                  ? t('onboarding.screen4.serverError')
                  : t('onboarding.screen4.serverOnline')}</span
            >
          </div>
          {#each slots as i}
            <div class="slot" style="--si: {i};">
              <span class="handle"></span>
              <div class="vents">
                {#each Array(8) as _}
                  <span class="vent"></span>
                {/each}
              </div>
              <div class="leds">
                <span class="led act" style="--ld: {i * 130}ms;"></span>
                <span class="led pwr"></span>
              </div>
            </div>
          {/each}
        </div>
        <div class="rack-base">
          <span class="screw"></span>
          <div class="ports">
            <span class="port"></span>
            <span class="port"></span>
            <span class="port"></span>
            <span class="port"></span>
          </div>
          <span class="power-btn"></span>
          <span class="screw"></span>
        </div>
      </div>

      <div class="floor"></div>

      <div class="tumbleweed">
        <svg viewBox="0 0 60 60" width="34" height="34">
          <g fill="none" stroke="rgba(180, 160, 110, 0.55)" stroke-width="0.8">
            <circle cx="30" cy="30" r="26" />
            <path d="M 5 30 Q 30 15, 55 30" />
            <path d="M 5 30 Q 30 45, 55 30" />
            <path d="M 30 5 Q 15 30, 30 55" />
            <path d="M 30 5 Q 45 30, 30 55" />
            <path d="M 10 10 L 50 50" />
            <path d="M 50 10 L 10 50" />
          </g>
        </svg>
      </div>

      {#if winstonIn}
        <div class="winston-shrug">
          <img src="/winston.webp" alt="" draggable="false" />
        </div>
      {/if}
    </div>
  </div>

  <div class="text-block">
    <div class="line large" class:show={showLine1}>
      {#if showLine1}
        {#each chars(t('onboarding.screen4.line1')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line large" class:show={showLine2}>
      {#if showLine2}
        {#each chars(t('onboarding.screen4.line2')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line accent" class:show={showLine3}>
      {#if showLine3}
        {#each chars(t('onboarding.screen4.line3')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>
  </div>

  <div class="actions" class:show={showButton}>
    <button class="next" onclick={onNext} tabindex={showButton ? 0 : -1}>
      <span>{t('onboarding.screen4.cta')}</span>
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
    height: clamp(280px, 44vh, 340px);
    opacity: 0;
    transition: opacity 800ms ease;
  }
  .scene.in {
    opacity: 1;
  }

  .stage {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* VIGNETTE: covers full viewport */
  .screen-vignette {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 5;
    background: radial-gradient(
      ellipse at center,
      transparent 30%,
      rgba(0, 0, 0, 0.35) 70%,
      rgba(0, 0, 0, 0.65) 100%
    );
    opacity: 0;
    transition: opacity 1200ms ease;
  }
  .screen-vignette.in {
    opacity: 1;
  }

  .rack {
    position: relative;
    width: 300px;
    transform: translateY(24px) scale(0.94);
    opacity: 0;
    transition:
      opacity 700ms ease,
      transform 700ms cubic-bezier(0.22, 1, 0.36, 1);
    z-index: 1;
  }
  .scene.rack-in .rack {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
  .scene.power-dying .rack {
    animation: rackShake 0.07s linear infinite;
  }
  .scene.power-dying .rack-body {
    animation: bodyFlicker 0.13s steps(2, end) infinite;
  }
  @keyframes rackShake {
    0%,
    100% {
      transform: translate(0, 0) scale(1);
    }
    25% {
      transform: translate(-1px, 1px) scale(1);
    }
    50% {
      transform: translate(1px, 0) scale(1);
    }
    75% {
      transform: translate(-1px, -1px) scale(1);
    }
  }
  @keyframes bodyFlicker {
    0%,
    49% {
      filter: brightness(1);
    }
    50%,
    100% {
      filter: brightness(1.55) hue-rotate(-12deg);
    }
  }
  .rack-top {
    height: 18px;
    background: linear-gradient(180deg, #262b3e 0%, #14182a 100%);
    border-radius: 8px 8px 0 0;
    border: 1px solid rgba(148, 163, 184, 0.2);
    border-bottom: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
  }
  .rack-body {
    background: linear-gradient(180deg, #181c2e 0%, #10131f 100%);
    border: 1px solid rgba(148, 163, 184, 0.2);
    padding: 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
  }
  .rack-base {
    position: relative;
    height: 26px;
    background: linear-gradient(180deg, #14182a 0%, #0b0e18 100%);
    border: 1px solid rgba(148, 163, 184, 0.2);
    border-top: none;
    border-radius: 0 0 10px 10px;
    box-shadow: 0 22px 42px rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    gap: 10px;
  }

  .screw {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: radial-gradient(circle at 30% 30%, #4a5168, #1a1d2c 70%);
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.5);
    flex-shrink: 0;
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    height: 20px;
    background: #06080f;
    border: 1px solid rgba(148, 163, 184, 0.1);
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.12em;
  }
  .status-led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #28d27c;
    box-shadow: 0 0 6px rgba(40, 210, 124, 0.85);
    animation: ledPulse 1.4s ease-in-out infinite;
    transition:
      background 800ms ease,
      box-shadow 800ms ease;
  }
  .status-led.dead {
    background: #2a2f3e;
    box-shadow: none;
    animation: none;
  }
  .status-text {
    color: #28d27c;
    font-weight: 600;
    transition: color 800ms ease;
    flex: 1;
  }
  .status-text.dead {
    color: #6b7185;
  }
  .status-led.dying {
    background: #ff3838;
    box-shadow: 0 0 10px rgba(255, 56, 56, 0.95);
    animation: ledFlicker 0.13s steps(2, end) infinite;
  }
  .status-text.dying {
    color: #ff5050;
    animation:
      textGlitch 0.18s steps(2, end) infinite,
      textShift 0.09s steps(2, end) infinite;
    text-shadow: 0 0 6px rgba(255, 80, 80, 0.6);
  }
  @keyframes ledFlicker {
    0%,
    49% {
      opacity: 1;
    }
    50%,
    100% {
      opacity: 0.15;
    }
  }
  @keyframes textGlitch {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }
  @keyframes textShift {
    0%,
    100% {
      transform: translateX(0);
    }
    50% {
      transform: translateX(1px);
    }
  }

  .slot {
    position: relative;
    height: 28px;
    background: linear-gradient(180deg, #0e1220 0%, #060810 100%);
    border: 1px solid rgba(148, 163, 184, 0.1);
    border-radius: 4px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.04),
      inset 0 -1px 0 rgba(0, 0, 0, 0.5);
  }
  .handle {
    width: 6px;
    height: 16px;
    background: linear-gradient(180deg, #2a2f44, #14182a);
    border: 1px solid rgba(148, 163, 184, 0.15);
    border-radius: 2px;
    flex-shrink: 0;
  }
  .vents {
    display: flex;
    gap: 2px;
    flex: 1;
  }
  .vent {
    width: 2px;
    height: 14px;
    background: rgba(148, 163, 184, 0.16);
    border-radius: 1px;
  }
  .leds {
    display: flex;
    gap: 5px;
    flex-shrink: 0;
  }
  .led {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    transition:
      background 800ms ease,
      box-shadow 800ms ease;
  }
  .led.act {
    background: #28d27c;
    box-shadow: 0 0 6px rgba(40, 210, 124, 0.85);
    animation: ledPulse 1.4s ease-in-out var(--ld) infinite;
  }
  .led.pwr {
    background: #f0a040;
    box-shadow: 0 0 5px rgba(240, 160, 64, 0.7);
  }
  .scene.power-dying .led.act,
  .scene.power-dying .led.pwr {
    background: #ff3838;
    box-shadow: 0 0 8px rgba(255, 56, 56, 0.95);
    animation: ledFlicker 0.11s steps(2, end) infinite;
  }
  .scene.powered-down .led {
    background: #2a2f3e;
    box-shadow: none;
    animation: none;
  }
  .scene.powered-down .rack {
    animation: powerSurge 600ms ease-out;
  }
  .scene.powered-down .rack-top,
  .scene.powered-down .rack-body,
  .scene.powered-down .rack-base {
    filter: brightness(0.55) saturate(0.7);
    transition: filter 800ms ease 200ms;
  }
  @keyframes powerSurge {
    0% {
      filter: brightness(2) saturate(1.4);
    }
    18% {
      filter: brightness(0.2);
    }
    32% {
      filter: brightness(1.4);
    }
    48% {
      filter: brightness(0.15);
    }
    100% {
      filter: brightness(1);
    }
  }
  @keyframes ledPulse {
    0%,
    100% {
      opacity: 0.55;
    }
    50% {
      opacity: 1;
    }
  }

  .ports {
    display: flex;
    gap: 5px;
    flex: 1;
    justify-content: center;
  }
  .port {
    width: 12px;
    height: 9px;
    background: #06080f;
    border: 1px solid rgba(148, 163, 184, 0.2);
    border-radius: 2px;
  }
  .power-btn {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: radial-gradient(circle at 30% 30%, #2a2f44, #0a0c16 75%);
    border: 1px solid rgba(148, 163, 184, 0.25);
    box-shadow: 0 0 6px rgba(46, 91, 215, 0.55);
    transition: box-shadow 1200ms ease;
    flex-shrink: 0;
  }
  .scene.powered-down .power-btn {
    box-shadow: none;
  }

  .floor {
    position: absolute;
    left: 8%;
    right: 8%;
    bottom: 24px;
    height: 1px;
    background: linear-gradient(
      90deg,
      transparent 0%,
      rgba(200, 215, 255, 0.18) 30%,
      rgba(200, 215, 255, 0.18) 70%,
      transparent 100%
    );
  }

  /* TUMBLEWEED: uses viewport units so it travels the full screen */
  .tumbleweed {
    position: absolute;
    bottom: 22px;
    left: 50%;
    margin-left: calc(-50vw - 60px);
    opacity: 0;
    pointer-events: none;
    z-index: 2;
  }
  .scene.tumbleweed-rolling .tumbleweed {
    animation: tumbleRoll 11s linear infinite;
  }
  @keyframes tumbleRoll {
    0% {
      opacity: 0;
      transform: translateX(0) rotate(0deg);
    }
    5% {
      opacity: 1;
    }
    55% {
      opacity: 1;
      transform: translateX(calc(100vw + 30px)) rotate(1100deg);
    }
    65% {
      opacity: 0;
      transform: translateX(calc(100vw + 120px)) rotate(1320deg);
    }
    100% {
      opacity: 0;
      transform: translateX(calc(100vw + 120px)) rotate(1320deg);
    }
  }

  .winston-shrug {
    position: absolute;
    bottom: 14px;
    right: 14%;
    width: 84px;
    height: 84px;
    z-index: 3;
    animation:
      winstonIn 700ms cubic-bezier(0.22, 1, 0.36, 1) backwards,
      winstonBob 3.5s ease-in-out 700ms infinite;
  }
  .winston-shrug img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 0 12px rgba(123, 163, 247, 0.35));
    user-select: none;
    -webkit-user-drag: none;
  }
  @keyframes winstonIn {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.9);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  @keyframes winstonBob {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-4px);
    }
  }

  .text-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    text-align: center;
    max-width: 720px;
    min-height: 110px;
  }
  .line {
    font-size: 16px;
    color: rgba(200, 215, 255, 0.72);
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
    text-shadow: 0 0 18px rgba(123, 163, 247, 0.35);
    margin-top: 6px;
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
    }
    .rack {
      width: 240px;
    }
    .winston-shrug {
      width: 72px;
      height: 72px;
      right: 6%;
      bottom: 8px;
    }
  }
</style>
