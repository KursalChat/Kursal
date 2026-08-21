<script lang="ts">
  import { t } from '$lib/i18n';
  import Winston from './Winston.svelte';
  import Line from './Line.svelte';
  import NextButton from './NextButton.svelte';
  import OnboardingScreen from './OnboardingScreen.svelte';
  import { createSequence } from './sequence.svelte';

  type Props = {
    onNext: () => void;
  };

  let { onNext }: Props = $props();

  const SAMPLE_CODE = ['amber trellis quiet fox', 'harbor maple drift nine'];

  const seq = createSequence([150, 700, 1500, 1900]);
  $effect(() => seq.destroy);
</script>

<OnboardingScreen {seq}>
  {#snippet scene()}
    <div class="scene" class:show={seq.at(0)}>
      <svg viewBox="0 0 640 250" class="art" aria-hidden="true">
        <g filter="url(#ink-wobble)">
          <g class="slip">
            <path
              d="M-124 -42 l21 7 l21 -8 l21 6 l20 -9 l21 8 l21 -6 l21 9 l20 -7 l21 6 l21 -8 l21 7 l19 -5 v78 h-248 z"
            />
            <path class="rule" d="M-100 26 H100" />
          </g>

          <g class="deny deny-1">
            <rect x="-27" y="-25" width="54" height="50" rx="11" />
          </g>

          <g class="deny deny-2">
            <rect x="-27" y="-25" width="54" height="50" rx="11" />
            <rect class="glyph-box" x="-10" y="-14" width="20" height="28" rx="4" />
            <path class="glyph" d="M-3 9 H3" />
          </g>

          <g class="deny deny-3">
            <rect x="-27" y="-25" width="54" height="50" rx="11" />
            <rect class="glyph-box" x="-15" y="-8" width="30" height="17" rx="6" />
            <circle class="glyph-dot" cx="-7" cy="0" r="2.5" />
            <circle class="glyph-dot" cx="0" cy="0" r="2.5" />
            <circle class="glyph-dot" cx="7" cy="0" r="2.5" />
          </g>
        </g>

        <g class="labels" aria-hidden="true">
          <text class="code" x="384" y="76">{SAMPLE_CODE[0]}</text>
          <text class="code" x="384" y="98">{SAMPLE_CODE[1]}</text>
        </g>
        <text class="at" x="252" y="196">@</text>

        <g class="slashes">
          <path class="slash slash-1" d="M221 218 L283 175" />
          <path class="slash slash-2" d="M349 218 L411 175" />
          <path class="slash slash-3" d="M477 218 L539 175" />
        </g>
      </svg>

      <div class="winston-key" class:show={seq.at(0)}>
        <Winston size={132} interactive={false} src="/winston-key.webp" />
      </div>
    </div>
  {/snippet}

  {#snippet text()}
    <Line show={seq.at(1)} variant="lead">{t('onboarding.noAccounts.lead')}</Line>
    <Line show={seq.at(2)}>{t('onboarding.noAccounts.note')}</Line>
  {/snippet}

  {#snippet actions()}
    <NextButton label={t('onboarding.noAccounts.cta')} show={seq.at(3)} onclick={onNext} />
  {/snippet}
</OnboardingScreen>

<style>
  .scene {
    position: relative;
    width: 100%;
    max-width: 520px;
    opacity: 0;
    transform: translateY(10px);
    transition:
      opacity 600ms ease,
      transform 600ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .scene.show {
    opacity: 1;
    transform: translateY(0);
  }

  .art {
    width: 100%;
    max-height: 40vh;
    overflow: visible;
  }

  .art :global(path),
  .art :global(rect),
  .art :global(circle) {
    fill: var(--ob-fill);
    stroke: var(--ob-ink);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }

  .rule,
  .glyph {
    fill: none !important;
    stroke-width: 2.4 !important;
  }
  .rule {
    opacity: 0.25;
  }
  .glyph-box {
    fill: none !important;
  }
  .glyph-dot {
    fill: var(--ob-ink) !important;
    stroke: none !important;
  }

  .slip {
    transform: translate(384px, 80px) rotate(-2.5deg);
    animation: slipIn 700ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes slipIn {
    from {
      opacity: 0;
      transform: translate(384px, 94px) rotate(-8deg);
    }
  }

  .code {
    fill: var(--ob-ink);
    font-family: var(--font-mono);
    font-size: 15px;
    font-weight: 500;
    letter-spacing: 0.05em;
    text-anchor: middle;
  }
  .labels {
    transform: rotate(-2.5deg);
    transform-origin: 384px 80px;
    animation: slipFade 700ms ease 120ms both;
  }

  @keyframes slipFade {
    from {
      opacity: 0;
    }
  }

  .at {
    fill: var(--ob-ink);
    font-family: var(--font-mono);
    font-size: 28px;
    font-weight: 600;
    text-anchor: middle;
    dominant-baseline: central;
    animation: slipFade 480ms ease 500ms both;
  }

  .deny {
    animation: denyIn 480ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .deny-1 {
    transform: translate(252px, 196px);
    animation-delay: 500ms;
  }
  .deny-2 {
    transform: translate(380px, 196px);
    animation-delay: 700ms;
  }
  .deny-3 {
    transform: translate(508px, 196px);
    animation-delay: 900ms;
  }

  @keyframes denyIn {
    from {
      opacity: 0;
    }
  }

  .slash {
    fill: none;
    stroke: var(--ob-warn);
    stroke-width: 6;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
    stroke-dasharray: 80;
    stroke-dashoffset: 80;
    animation: strike 300ms cubic-bezier(0.6, 0, 0.4, 1) both;
  }
  .slash-1 {
    animation-delay: 800ms;
  }
  .slash-2 {
    animation-delay: 1000ms;
  }
  .slash-3 {
    animation-delay: 1200ms;
  }

  @keyframes strike {
    to {
      stroke-dashoffset: 0;
    }
  }

  .winston-key {
    position: absolute;
    left: 2%;
    top: 6%;
    opacity: 0;
    transition: opacity 600ms ease 150ms;
    animation: bob 4.6s ease-in-out infinite;
  }
  .winston-key.show {
    opacity: 1;
  }

  @keyframes bob {
    0%,
    100% {
      transform: translateY(0) rotate(-4deg);
    }
    50% {
      transform: translateY(-6px) rotate(2deg);
    }
  }

  @media (max-width: 640px) {
    .art {
      max-height: 32vh;
    }
    .code {
      font-size: 13px;
    }
    .winston-key {
      left: -3%;
    }
  }
</style>
