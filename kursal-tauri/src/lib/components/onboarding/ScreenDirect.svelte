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

  const seq = createSequence([150, 600, 1400, 1800]);
  $effect(() => seq.destroy);
</script>

<OnboardingScreen {seq}>
  {#snippet scene()}
    <div class="scene" class:show={seq.at(0)}>
      <svg viewBox="0 0 640 300" class="art" aria-hidden="true">
        <g filter="url(#ink-wobble)">
          <g class="desk" class:struck={seq.at(1)}>
            <path class="cord" d="M320 118 V140" />
            <path d="M300 164 L309 141 H331 L340 164 Z" />
            <circle cx="320" cy="167" r="5" />
            <rect x="248" y="212" width="144" height="12" rx="4" />
            <rect x="266" y="224" width="12" height="44" rx="3" />
            <rect x="362" y="224" width="12" height="44" rx="3" />
          </g>

          {#if seq.at(1)}
            <g class="strike">
              <path d="M248 126 L396 272" />
              <path class="second" d="M396 126 L248 272" />
            </g>
          {/if}

          <g class="phone left">
            <rect x="-29" y="-48" width="58" height="96" rx="11" />
            <rect class="void" x="-23" y="-38" width="46" height="78" rx="6" />
            <path class="notch" d="M-7 -43 H7" />
          </g>
          <g class="phone right">
            <rect x="-29" y="-48" width="58" height="96" rx="11" />
            <rect class="void" x="-23" y="-38" width="46" height="78" rx="6" />
            <path class="notch" d="M-7 -43 H7" />
          </g>

          {#if seq.at(2)}
            <path class="arc" d="M96 176 Q320 84, 544 176" />
            <g class="letter">
              <rect x="-26" y="-17" width="52" height="34" rx="4" />
              <path class="flap" d="M-26 -15 L0 4 L26 -15" />
              <g class="seal">
                <circle cx="0" cy="2" r="10" />
                <path d="M0 -2 v6" />
              </g>
            </g>
          {/if}
        </g>
      </svg>

      <div class="winston-corner"><Winston size={62} interactive={false} /></div>
    </div>
  {/snippet}

  {#snippet text()}
    <Line show={seq.at(1)} variant="lead">{t('onboarding.direct.lead')}</Line>
    <Line show={seq.at(2)}>{t('onboarding.direct.note')}</Line>
  {/snippet}

  {#snippet actions()}
    <NextButton label={t('onboarding.direct.cta')} show={seq.at(3)} onclick={onNext} />
  {/snippet}
</OnboardingScreen>

<style>
  .scene {
    position: relative;
    width: 100%;
    max-width: 560px;
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

  .void {
    fill: var(--ob-ink) !important;
  }
  .notch {
    fill: none !important;
    stroke-width: 3.4 !important;
  }
  .flap {
    fill: none !important;
  }

  .cord {
    fill: none !important;
    stroke: var(--ob-fill) !important;
    stroke-width: 2 !important;
    opacity: 0.3;
  }

  .desk {
    transition: opacity 700ms ease;
  }
  .desk.struck {
    opacity: 0.2;
  }

  .strike path {
    fill: none !important;
    stroke: var(--ob-warn) !important;
    stroke-width: 6 !important;
    stroke-dasharray: 208;
    stroke-dashoffset: 208;
    animation: draw 320ms cubic-bezier(0.6, 0, 0.4, 1) forwards;
  }
  .strike .second {
    animation-delay: 200ms;
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  .phone {
    animation: phoneIn 620ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .left {
    transform: translate(96px, 176px);
  }
  .right {
    transform: translate(544px, 176px);
    animation-delay: 130ms;
  }

  @keyframes phoneIn {
    from {
      opacity: 0;
    }
  }

  .arc {
    fill: none !important;
    stroke: var(--ob-accent) !important;
    stroke-width: 3.5 !important;
    stroke-dasharray: 520;
    stroke-dashoffset: 520;
    opacity: 0.85;
    animation: draw 620ms cubic-bezier(0.4, 0, 0.3, 1) forwards;
  }

  .letter {
    offset-path: path('M96 176 Q320 84, 544 176');
    offset-rotate: 0deg;
    animation: glide 2.4s cubic-bezier(0.45, 0, 0.55, 1) 500ms infinite;
  }

  @keyframes glide {
    0% {
      offset-distance: 0%;
      opacity: 0;
    }
    8%,
    88% {
      opacity: 1;
    }
    100% {
      offset-distance: 100%;
      opacity: 0;
    }
  }

  .seal circle {
    fill: var(--ob-accent) !important;
  }
  .seal path {
    fill: none !important;
    stroke-width: 2.6 !important;
  }
  .seal {
    animation: sealOn 2.4s cubic-bezier(0.3, 1.5, 0.5, 1) 500ms infinite;
    transform-origin: 0 2px;
  }

  @keyframes sealOn {
    0% {
      transform: scale(2.4) rotate(-40deg);
      opacity: 0;
    }
    10%,
    100% {
      transform: scale(1) rotate(0);
      opacity: 1;
    }
  }

  .winston-corner {
    position: absolute;
    right: 3%;
    bottom: 0;
    animation: bob 4.2s ease-in-out infinite;
  }

  @keyframes bob {
    0%,
    100% {
      transform: translateY(0) rotate(4deg);
    }
    50% {
      transform: translateY(-6px) rotate(-2deg);
    }
  }

  @media (max-width: 640px) {
    .art {
      max-height: 30vh;
    }
    .winston-corner {
      right: 0;
    }
  }
</style>
