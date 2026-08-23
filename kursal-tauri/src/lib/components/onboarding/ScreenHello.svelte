<script lang="ts">
  import { t } from '$lib/i18n';
  import Winston from './Winston.svelte';
  import Line from './Line.svelte';
  import NextButton from './NextButton.svelte';
  import OnboardingScreen from './OnboardingScreen.svelte';
  import { createSequence } from './sequence.svelte';

  type Props = {
    onNext: () => void;
    onSkip: () => void;
  };

  let { onNext, onSkip }: Props = $props();

  const seq = createSequence([200, 500, 1100, 1600, 2200]);
  $effect(() => seq.destroy);
</script>

<OnboardingScreen {seq}>
  {#snippet scene()}
    <div class="cast" class:show={seq.at(0)}>
      <Winston size={190} />
      <div class="ground"></div>
    </div>
  {/snippet}

  {#snippet text()}
    <h1 class="heading" class:show={seq.at(1)}>{t('onboarding.hello.heading')}</h1>
    <Line show={seq.at(2)}>{t('onboarding.hello.subtitle')}</Line>
  {/snippet}

  {#snippet actions()}
    <NextButton label={t('onboarding.hello.cta')} show={seq.at(3)} onclick={onNext} />
    <div class="skip-wrap" class:show={seq.at(4)} aria-hidden={!seq.at(4)}>
      <button class="skip" onclick={onSkip} tabindex={seq.at(4) ? 0 : -1}>
        {t('onboarding.hello.skip')}
      </button>
      <span class="tooltip">{t('onboarding.hello.skipTooltip')}</span>
    </div>
  {/snippet}
</OnboardingScreen>

<style>
  .cast {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    opacity: 0;
    transition: opacity 500ms ease;
  }
  .cast.show {
    opacity: 1;
  }

  .ground {
    width: 120px;
    height: 12px;
    margin-top: 6px;
    border-radius: 50%;
    background: var(--ob-fill);
    opacity: 0.07;
    animation: castShadow 5s ease-in-out infinite;
  }

  @keyframes castShadow {
    0%,
    100% {
      transform: scaleX(1);
      opacity: 0.07;
    }
    50% {
      transform: scaleX(0.86);
      opacity: 0.04;
    }
  }

  .heading {
    margin: 0;
    font-size: 34px;
    font-weight: 800;
    letter-spacing: -0.035em;
    color: var(--ob-text);
    opacity: 0;
    transform: translateY(10px);
    transition:
      opacity 560ms cubic-bezier(0.22, 1, 0.36, 1),
      transform 560ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .heading.show {
    opacity: 1;
    transform: translateY(0);
  }

  .skip-wrap {
    position: relative;
    opacity: 0;
    pointer-events: none;
    transition: opacity 500ms ease;
  }
  .skip-wrap.show {
    opacity: 1;
    pointer-events: auto;
  }

  .skip {
    font-size: 12.5px;
    color: var(--ob-text-dim);
    opacity: 0.5;
    padding: 6px 10px;
    transition: opacity 150ms ease;
  }
  @media (hover: hover) {
    .skip:hover {
      opacity: 1;
    }
  }

  .tooltip {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%) translateY(4px);
    background: var(--ob-fill);
    border: 2px solid var(--ob-ink);
    color: var(--ob-ink);
    padding: 6px 11px;
    border-radius: 10px;
    font-size: var(--text-xs);
    font-weight: 600;
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    box-shadow: 2px 2px 0 var(--ob-accent);
    transition:
      opacity 180ms ease,
      transform 180ms ease;
  }
  @media (hover: hover) {
    .skip-wrap:hover .tooltip {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
  }

  @media (max-width: 640px) {
    .heading {
      font-size: 27px;
    }
  }
</style>
