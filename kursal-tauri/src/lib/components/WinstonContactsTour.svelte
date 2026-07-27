<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { page } from '$app/state';
  import { fade } from 'svelte/transition';
  import { t } from '$lib/i18n';
  import WinstonCard from './WinstonCard.svelte';

  type Step =
    | 'intro'
    | 'click-add'
    | 'otp-detail'
    | 'click-ltc'
    | 'ltc-detail'
    | 'click-nearby'
    | 'nearby-detail'
    | 'outro';

  const KEY = 'kursal_addcontact_onboarded';

  let active = $state(false);
  let step = $state<Step>('intro');
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let skipReady = $state(false);

  onMount(() => {
    if (localStorage.getItem(KEY) === 'done') return;
    const timer = setTimeout(() => {
      active = true;
    }, 700);
    const s = setTimeout(() => {
      skipReady = true;
    }, 2700);
    return () => {
      clearTimeout(timer);
      clearTimeout(s);
    };
  });

  function finish() {
    localStorage.setItem(KEY, 'done');
    active = false;
  }

  function targetEl(): HTMLElement | null {
    if (step === 'click-add') {
      return (document.querySelector('[data-tour="add-contact-empty"]') ??
        document.querySelector('[data-tour="add-contact-btn"]')) as HTMLElement | null;
    }
    if (step === 'click-ltc')
      return document.querySelector('[data-tour="ltc-tab"]') as HTMLElement | null;
    if (step === 'click-nearby')
      return document.querySelector('[data-tour="nearby-tab"]') as HTMLElement | null;
    return null;
  }

  function recompute() {
    const el = targetEl();
    if (!el) {
      rect = null;
      return;
    }
    const r = el.getBoundingClientRect();
    rect = { x: r.left, y: r.top, w: r.width, h: r.height };
  }

  // Recompute on step change, route change, or resize.
  $effect(() => {
    if (!active) return;
    void step;
    void page.url.pathname;
    void tick().then(() => requestAnimationFrame(recompute));
  });

  $effect(() => {
    if (!active) return;
    const onR = () => recompute();
    window.addEventListener('resize', onR);
    window.addEventListener('scroll', onR, true);
    const id = window.setInterval(recompute, 400);
    return () => {
      window.removeEventListener('resize', onR);
      window.removeEventListener('scroll', onR, true);
      clearInterval(id);
    };
  });

  // Auto-advance when user navigates to the target page.
  $effect(() => {
    if (!active) return;
    const p = page.url.pathname;
    if (step === 'click-add' && p.startsWith('/add-contact')) {
      step = 'otp-detail';
    } else if (step === 'click-ltc' && p.startsWith('/add-contact/ltc')) {
      step = 'ltc-detail';
    } else if (step === 'click-nearby' && p.startsWith('/add-contact/nearby')) {
      step = 'nearby-detail';
    }
  });

  type StepData = {
    img: string;
    title?: string;
    body: string;
    cta?: string;
  };

  const stepData: Record<Step, StepData> = {
    intro: {
      img: '/winston.png',
      title: t('tour.steps.intro.title'),
      body: t('tour.steps.intro.body'),
      cta: t('tour.steps.intro.cta'),
    },
    'click-add': {
      img: '/winston.png',
      body: t('tour.steps.clickAdd.body'),
    },
    'otp-detail': {
      img: '/winston-key.png',
      title: t('tour.steps.otpDetail.title'),
      body: t('tour.steps.otpDetail.body'),
      cta: t('tour.steps.otpDetail.cta'),
    },
    'click-ltc': {
      img: '/winston.png',
      body: t('tour.steps.clickLtc.body'),
    },
    'ltc-detail': {
      img: '/winston-key.png',
      title: t('tour.steps.ltcDetail.title'),
      body: t('tour.steps.ltcDetail.body'),
      cta: t('tour.steps.ltcDetail.cta'),
    },
    'click-nearby': {
      img: '/winston.png',
      body: t('tour.steps.clickNearby.body'),
    },
    'nearby-detail': {
      img: '/winston.png',
      title: t('tour.steps.nearbyDetail.title'),
      body: t('tour.steps.nearbyDetail.body'),
      cta: t('tour.steps.nearbyDetail.cta'),
    },
    outro: {
      img: '/winston.png',
      title: t('tour.steps.outro.title'),
      body: t('tour.steps.outro.body'),
      cta: t('tour.steps.outro.cta'),
    },
  };

  function next() {
    if (step === 'intro') step = 'click-add';
    else if (step === 'otp-detail') step = 'click-ltc';
    else if (step === 'ltc-detail') step = 'click-nearby';
    else if (step === 'nearby-detail') step = 'outro';
    else if (step === 'outro') finish();
  }

  function escapeHtml(s: string) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
  function formatBody(s: string) {
    return escapeHtml(s)
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>');
  }

  const data = $derived(stepData[step]);
  const showSpotlight = $derived(
    step === 'click-add' || step === 'click-ltc' || step === 'click-nearby'
  );
</script>

{#if active}
  <div class="tour" role="dialog" aria-label={t('tour.dialogAriaLabel')}>
    {#if showSpotlight && rect}
      <!-- 4-quadrant dim around the target - clicks pass through the cutout -->
      <div
        class="dim"
        style:top="0"
        style:left="0"
        style:right="0"
        style:height="{Math.max(0, rect.y - 8)}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{Math.max(0, rect.y - 8)}px"
        style:left="0"
        style:width="{Math.max(0, rect.x - 8)}px"
        style:height="{rect.h + 16}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{Math.max(0, rect.y - 8)}px"
        style:left="{rect.x + rect.w + 8}px"
        style:right="0"
        style:height="{rect.h + 16}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{rect.y + rect.h + 8}px"
        style:left="0"
        style:right="0"
        style:bottom="0"
        transition:fade={{ duration: 200 }}
      ></div>

      <div
        class="ring"
        style:left="{rect.x - 8}px"
        style:top="{rect.y - 8}px"
        style:width="{rect.w + 16}px"
        style:height="{rect.h + 16}px"
      ></div>
    {:else if !showSpotlight}
      <div class="dim full" transition:fade={{ duration: 250 }}></div>
    {/if}

    <div class="card-pos">
      <WinstonCard img={data.img} alt={t('tour.winstonAlt')} size={110}>
        {#key step}
          <div in:fade={{ duration: 220 }}>
            {#if data.title}
              <div class="title">{data.title}</div>
            {/if}
            <div class="body">{@html formatBody(data.body)}</div>

            <div class="actions">
              {#if step === 'intro' && skipReady}
                <button class="skip" onclick={finish} in:fade={{ duration: 240 }}>
                  {t('tour.skipButton')}
                </button>
              {:else}
                <span class="skip-placeholder"></span>
              {/if}
              {#if data.cta}
                <button class="primary" onclick={next}>
                  {data.cta}
                </button>
              {/if}
            </div>

            <div class="progress">
              {#each ['intro', 'click-add', 'otp-detail', 'click-ltc', 'ltc-detail', 'click-nearby', 'nearby-detail', 'outro'] as s, i}
                <span class="dot" class:done={Object.keys(stepData).indexOf(step) >= i}></span>
              {/each}
            </div>
          </div>
        {/key}
      </WinstonCard>
    </div>
  </div>
{/if}

<style>
  .tour {
    position: fixed;
    inset: 0;
    z-index: 9000;
    pointer-events: none;
  }

  .dim {
    position: fixed;
    background: rgba(2, 6, 23, 0.62);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    pointer-events: auto;
    transition:
      top 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      left 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      width 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      height 0.32s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .dim.full {
    inset: 0;
  }

  .ring {
    position: fixed;
    border-radius: var(--radius-md);
    border: 2px solid var(--accent);
    box-shadow:
      0 0 0 4px var(--accent-dim),
      0 0 32px 4px color-mix(in srgb, var(--accent) 60%, transparent),
      inset 0 0 0 1px rgba(255, 255, 255, 0.4);
    pointer-events: none;
    animation: ring-pulse 1.6s ease-in-out infinite;
    transition:
      top 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      left 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      width 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      height 0.32s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes ring-pulse {
    0%,
    100% {
      box-shadow:
        0 0 0 4px var(--accent-dim),
        0 0 24px 2px color-mix(in srgb, var(--accent) 50%, transparent),
        inset 0 0 0 1px rgba(255, 255, 255, 0.35);
      transform: scale(1);
    }
    50% {
      box-shadow:
        0 0 0 8px color-mix(in srgb, var(--accent) 18%, transparent),
        0 0 40px 8px color-mix(in srgb, var(--accent) 65%, transparent),
        inset 0 0 0 1px rgba(255, 255, 255, 0.5);
      transform: scale(1.03);
    }
  }

  .card-pos {
    position: fixed;
    right: 22px;
    bottom: 22px;
    pointer-events: auto;
    max-width: min(440px, calc(100vw - 44px));
    z-index: 1;
  }

  .title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 6px;
    letter-spacing: -0.01em;
  }
  .body {
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--text-secondary);
  }
  .body :global(strong) {
    color: var(--text-primary);
    font-weight: 700;
  }

  .skip-placeholder {
    flex: 1;
  }

  .actions {
    margin-top: 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .skip {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    padding: 6px 4px;
    transition: color var(--transition);
  }
  .skip:hover {
    color: var(--text-secondary);
  }
  .primary {
    padding: 8px 16px;
    border-radius: var(--radius-md);
    background: var(--accent-solid);
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    transition:
      transform var(--transition),
      box-shadow var(--transition),
      background var(--transition);
    box-shadow: 0 4px 14px var(--accent-dim);
  }
  .primary:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
    box-shadow: 0 8px 22px var(--accent-dim);
  }
  .primary:active {
    transform: translateY(0);
  }

  .progress {
    margin-top: 10px;
    display: flex;
    gap: 4px;
    justify-content: center;
  }
  .dot {
    width: 14px;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    transition: background var(--transition);
  }
  .dot.done {
    background: var(--accent);
  }

  @media (max-width: 768px) {
    .card-pos {
      right: 12px;
      bottom: 12px;
      left: 12px;
      max-width: none;
    }
  }
</style>
