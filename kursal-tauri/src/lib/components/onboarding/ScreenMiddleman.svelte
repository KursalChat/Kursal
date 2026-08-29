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

  const seq = createSequence([150, 700, 1500, 1900]);
  $effect(() => seq.destroy);
</script>

<OnboardingScreen {seq}>
  {#snippet scene()}
    <div class="scene" class:show={seq.at(0)}>
      <svg viewBox="0 0 640 290" class="art" aria-hidden="true">
        <g filter="url(#ink-wobble)">
          <g class="lamp">
            <path class="cord" d="M320 0 V26" />
            <path d="M294 54 L305 27 H335 L346 54 Z" />
            <circle cx="320" cy="58" r="6" />
          </g>
          <path class="beam" d="M298 58 H342 L400 202 H240 Z" />

          <g class="desk">
            <rect x="-12" y="202" width="664" height="13" rx="4" />
            <rect x="150" y="215" width="13" height="50" rx="3" />
            <rect x="477" y="215" width="13" height="50" rx="3" />
          </g>

          <g class="stack">
            <rect x="431" y="152" width="50" height="42" rx="3" transform="rotate(-5 456 173)" />
            <rect x="433" y="156" width="50" height="42" rx="3" transform="rotate(4 458 177)" />
            <rect x="429" y="160" width="50" height="42" rx="3" transform="rotate(-1 454 181)" />
          </g>

          <g class="letter">
            <rect class="back" x="-29" y="-19" width="58" height="38" rx="4" />
            <path class="lid" d="M-29 -19 L0 3 L29 -19 Z" />

            <g class="sheet">
              <rect x="-27" y="-23" width="54" height="46" rx="3" />
              <path class="script" d="M-17 -12 q6 -4 12 0 t12 0" />
              <path class="script" d="M-17 -3 q6 -4 12 0 t12 0" />
              <path class="script" d="M-17 6 q6 -4 12 0 t7 0" />
            </g>

            <path
              class="pocket"
              d="M-29 -17 L0 4 L29 -17 V15 a4 4 0 0 1 -4 4 H-25 a4 4 0 0 1 -4 -4 Z"
            />
          </g>

          <g class="glass">
            <circle class="lens" cx="0" cy="0" r="18" />
            <path class="grip" d="M13 13 L25 25" />
          </g>

          <g class="copy">
            <rect x="-27" y="-23" width="54" height="46" rx="3" />
            <path class="script" d="M-17 -12 q6 -4 12 0 t12 0" />
            <path class="script" d="M-17 -3 q6 -4 12 0 t12 0" />
            <path class="script" d="M-17 6 q6 -4 12 0 t7 0" />
          </g>
        </g>
      </svg>

      <div class="winston-corner"><Winston size={62} interactive={false} /></div>
    </div>
  {/snippet}

  {#snippet text()}
    <Line show={seq.at(1)} variant="lead">{t('onboarding.middleman.lead')}</Line>
    <Line show={seq.at(2)}>{t('onboarding.middleman.note')}</Line>
  {/snippet}

  {#snippet actions()}
    <NextButton label={t('onboarding.middleman.cta')} show={seq.at(3)} onclick={onNext} />
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
    max-height: 38vh;
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

  .cord {
    fill: none !important;
    stroke: var(--ob-fill) !important;
    stroke-width: 2 !important;
    opacity: 0.3;
  }
  .script,
  .grip {
    fill: none !important;
  }
  .script {
    stroke-width: 2.2 !important;
    opacity: 0.7;
  }
  .lens {
    fill: color-mix(in srgb, var(--ob-fill) 20%, transparent) !important;
    stroke-width: 4 !important;
  }
  .grip {
    stroke-width: 6 !important;
  }

  .beam {
    fill: var(--ob-fill) !important;
    stroke: none !important;
    opacity: 0.055;
  }

  .lamp,
  .desk {
    animation: propIn 700ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .desk {
    animation-delay: 120ms;
  }
  .lamp {
    transform-origin: 320px 0;
    animation:
      propIn 700ms cubic-bezier(0.22, 1, 0.36, 1) both,
      sway 7s ease-in-out 900ms infinite;
  }

  @keyframes propIn {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }
  @keyframes sway {
    0%,
    100% {
      transform: rotate(-0.7deg);
    }
    50% {
      transform: rotate(0.7deg);
    }
  }

  .letter {
    animation: travel 7s cubic-bezier(0.45, 0, 0.35, 1) 700ms infinite backwards;
  }
  .lid {
    transform-origin: 0 -19px;
    animation: openLid 7s cubic-bezier(0.4, 0, 0.4, 1) 700ms infinite backwards;
  }
  .sheet {
    opacity: 0;
    animation: unfold 7s cubic-bezier(0.3, 1.05, 0.4, 1) 700ms infinite backwards;
  }
  .glass {
    opacity: 0;
    animation: sweep 7s cubic-bezier(0.4, 0, 0.5, 1) 700ms infinite backwards;
  }
  .stack {
    animation: propIn 700ms cubic-bezier(0.22, 1, 0.36, 1) 260ms both;
  }
  .copy {
    opacity: 0;
    animation: file 7s cubic-bezier(0.4, 0, 0.4, 1) 700ms infinite backwards;
  }

  @keyframes travel {
    0% {
      transform: translate(-70px, 183px);
    }
    24%,
    80% {
      transform: translate(320px, 183px);
    }
    100% {
      transform: translate(710px, 183px);
    }
  }

  @keyframes openLid {
    0%,
    25% {
      transform: scaleY(1);
    }
    32%,
    72% {
      transform: scaleY(-1);
    }
    79%,
    100% {
      transform: scaleY(1);
    }
  }

  @keyframes unfold {
    0%,
    30% {
      opacity: 0;
      transform: translateY(0);
    }
    38%,
    66% {
      opacity: 1;
      transform: translateY(-44px);
    }
    73%,
    100% {
      opacity: 0;
      transform: translateY(0);
    }
  }

  @keyframes sweep {
    0%,
    38% {
      opacity: 0;
      transform: translate(292px, 128px) scale(0.85);
    }
    43% {
      opacity: 1;
      transform: translate(292px, 128px) scale(1);
    }
    56% {
      opacity: 1;
      transform: translate(348px, 144px) scale(1);
    }
    61%,
    100% {
      opacity: 0;
      transform: translate(348px, 144px) scale(0.85);
    }
  }

  @keyframes file {
    0%,
    56% {
      opacity: 0;
      transform: translate(320px, 139px) rotate(0deg) scale(1);
    }
    59% {
      opacity: 1;
      transform: translate(320px, 139px) rotate(0deg) scale(1);
    }
    70% {
      opacity: 1;
      transform: translate(455px, 176px) rotate(7deg) scale(0.9);
    }
    76%,
    100% {
      opacity: 0;
      transform: translate(455px, 178px) rotate(7deg) scale(0.9);
    }
  }

  .winston-corner {
    position: absolute;
    left: 1%;
    bottom: 2%;
    animation: peek 5s ease-in-out infinite;
  }

  @keyframes peek {
    0%,
    100% {
      transform: rotate(-5deg);
    }
    50% {
      transform: rotate(3deg);
    }
  }

  @media (max-width: 640px) {
    .art {
      max-height: 28vh;
    }
    .winston-corner {
      left: -2%;
    }
  }
</style>
