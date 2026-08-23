<script lang="ts">
  import { t } from '$lib/i18n';
  import { withAvatarCacheBust } from '$lib/utils/avatarUrl';
  import { log } from '$lib/utils/log';
  import Winston from './Winston.svelte';
  import Line from './Line.svelte';
  import NextButton from './NextButton.svelte';
  import OnboardingScreen from './OnboardingScreen.svelte';
  import { createSequence } from './sequence.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import AvatarPicker from '$lib/components/AvatarPicker.svelte';
  import { Upload, X } from 'lucide-svelte';
  import { broadcastProfile, setLocalUserAvatar } from '$lib/api/identity';
  import { profileState } from '$lib/state/profile.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { isMobile } from '$lib/api/window';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { acceptTerms, TERMS_URL } from '$lib/utils/terms';
  import {
    DISPLAY_NAME_MAX,
    validateAvatarBytes,
    validateDisplayName,
  } from '$lib/utils/displayName';

  const EXIT_MS = 700;

  type Props = {
    onFinish: () => void;
    onBack?: () => void;
  };

  let { onFinish }: Props = $props();

  const seq = createSequence([150, 500, 1200, 1800]);
  $effect(() => seq.destroy);

  let exiting = $state(false);
  let saving = $state(false);

  let displayName = $state('');
  let avatarPreview = $state<string | null>(null);
  let avatarBytes = $state<number[] | null>(null);
  let nameInput = $state<HTMLInputElement>();

  const showForm = $derived(seq.at(3));

  const PROFILE_ERROR_KEYS = {
    empty: 'settings.account.errorNameEmpty',
    length: 'settings.account.errorNameLength',
    edgeWhitespace: 'settings.account.errorNameEdgeWhitespace',
    unsupportedChars: 'settings.account.errorNameUnsupportedChars',
    avatarTooLarge: 'settings.account.errorAvatarTooLarge',
  } as const;

  const profileError = $derived(
    validateDisplayName(displayName.trim()) ?? validateAvatarBytes(avatarBytes)
  );
  const profileErrorText = $derived(profileError ? t(PROFILE_ERROR_KEYS[profileError]) : '');
  const greeting = $derived(!profileError && displayName.trim().length > 0);

  $effect(() => {
    if (!showForm) return;
    const timer = setTimeout(() => nameInput?.focus(), 400);
    return () => clearTimeout(timer);
  });

  function keepInputVisible() {
    if (!isMobile) return;
    nameInput?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }

  $effect(() => {
    const vv = window.visualViewport;
    if (!isMobile || !vv) return;
    let lastHeight = vv.height;
    const onResize = () => {
      const shrank = vv.height < lastHeight;
      lastHeight = vv.height;
      if (shrank && document.activeElement === nameInput) {
        nameInput?.scrollIntoView({ block: 'center', behavior: 'auto' });
      }
    };
    vv.addEventListener('resize', onResize);
    return () => vv.removeEventListener('resize', onResize);
  });

  function handleAvatarChange(dataUrl: string, bytes: number[]) {
    avatarPreview = dataUrl;
    avatarBytes = bytes;
  }

  function removeAvatar() {
    avatarPreview = null;
    avatarBytes = null;
  }

  async function handleFinish() {
    if (exiting || saving) return;
    const name = displayName.trim();
    if (!name) {
      notifications.push(t('onboarding.profile.errorPickName'), 'error');
      nameInput?.focus();
      return;
    }
    if (profileError) {
      notifications.push(profileErrorText, 'error');
      nameInput?.focus();
      return;
    }
    saving = true;
    try {
      await profileState.save(name, avatarBytes);
    } catch (e) {
      notifyError(e, 'onboarding.profile.errorSave');
    }
    acceptTerms();
    saving = false;
    exiting = true;
    setTimeout(onFinish, EXIT_MS);
  }

  async function openTerms() {
    try {
      await openUrl(TERMS_URL);
    } catch (e) {
      log.error('Failed to open terms', e);
      notifications.push(t('terms.errorOpenLink'), 'error');
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Enter') handleFinish();
  }
</script>

<div class="wrap" class:exiting>
  <OnboardingScreen {seq} compact>
    {#snippet scene()}
      <div class="cast" class:show={seq.at(0)}>
        <Winston
          size={118}
          interactive={false}
          says={greeting ? t('onboarding.profile.greeting', { name: displayName.trim() }) : null}
        />
        <div class="ground"></div>
      </div>
    {/snippet}

    {#snippet text()}
      <Line show={seq.at(1)} variant="lead">{t('onboarding.profile.lead')}</Line>
      <Line show={seq.at(2)}>{t('onboarding.profile.note')}</Line>

      <div class="form" class:show={showForm} aria-hidden={!showForm}>
        <div class="identity">
          <AvatarPicker onChange={handleAvatarChange}>
            {#snippet children(open)}
              <div class="avatar-slot-wrap">
                <button
                  type="button"
                  class="avatar-slot"
                  title={t('onboarding.profile.uploadPhoto')}
                  aria-label={t('onboarding.profile.uploadPhoto')}
                  tabindex={showForm ? 0 : -1}
                  onclick={open}
                >
                  <Avatar name={displayName.trim() || '?'} src={avatarPreview} size={46} />
                  <span class="avatar-overlay" class:filled={!!avatarPreview}>
                    <Upload size={15} strokeWidth={2.4} />
                  </span>
                </button>
                {#if avatarPreview}
                  <button
                    type="button"
                    class="avatar-remove"
                    aria-label={t('onboarding.profile.removePhoto')}
                    tabindex={showForm ? 0 : -1}
                    onclick={removeAvatar}
                  >
                    <X size={11} strokeWidth={3} />
                  </button>
                {/if}
              </div>
            {/snippet}
          </AvatarPicker>

          <input
            bind:this={nameInput}
            class="name-input"
            type="text"
            placeholder={t('onboarding.profile.namePlaceholder')}
            maxlength={DISPLAY_NAME_MAX}
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            bind:value={displayName}
            onkeydown={handleKey}
            onfocus={keepInputVisible}
            disabled={exiting || saving || !showForm}
            tabindex={showForm ? 0 : -1}
          />
        </div>

        {#if displayName.trim() && profileErrorText}
          <span class="name-error">{profileErrorText}</span>
        {:else}
          <span class="hint">{t('onboarding.profile.nameHint')}</span>
        {/if}
      </div>
    {/snippet}

    {#snippet actions()}
      <NextButton
        label={saving ? t('onboarding.profile.saving') : t('onboarding.profile.cta')}
        show={showForm}
        arrow={!saving}
        disabled={exiting || saving || !displayName.trim() || !!profileError}
        onclick={handleFinish}
      />
      <p class="consent" class:show={showForm}>
        {t('onboarding.profile.consentPrefix')}
        <button type="button" class="consent-link" onclick={openTerms} tabindex={showForm ? 0 : -1}>
          {t('onboarding.profile.consentLink')}
        </button>
      </p>
    {/snippet}
  </OnboardingScreen>

  <div class="iris" aria-hidden="true"></div>
</div>

<style>
  .wrap {
    position: relative;
    width: 100%;
    min-height: 100%;
    display: flex;
  }
  .wrap > :global(*) {
    flex: 1 0 auto;
    min-height: 0;
  }

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
    width: 92px;
    height: 10px;
    margin-top: 4px;
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

  .form {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    opacity: 0;
    transform: translateY(10px);
    pointer-events: none;
    transition:
      opacity 500ms ease,
      transform 500ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .form.show {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px;
    background: var(--ob-fill);
    border: 2.5px solid var(--ob-ink);
    border-radius: 16px;
    box-shadow: 4px 4px 0 var(--ob-accent);
  }

  .avatar-slot-wrap {
    position: relative;
    flex: none;
  }

  .avatar-slot {
    position: relative;
    display: block;
    padding: 3px;
    border-radius: 50%;
    background: var(--ob-fill);
    border: 2.5px solid var(--ob-ink);
    line-height: 0;
    transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  @media (hover: hover) {
    .avatar-slot:hover {
      transform: scale(1.05);
    }
  }
  .avatar-slot:active {
    transform: scale(0.97);
  }

  .avatar-overlay {
    position: absolute;
    inset: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ob-ink) 62%, transparent);
    color: var(--ob-fill);
    opacity: 0;
    transition: opacity 160ms ease;
  }
  .avatar-slot:focus-visible .avatar-overlay {
    opacity: 1;
  }
  @media (hover: hover) {
    .avatar-slot:hover .avatar-overlay {
      opacity: 1;
    }
  }
  .avatar-overlay.filled {
    opacity: 0;
  }

  .avatar-remove {
    position: absolute;
    top: -4px;
    right: -4px;
    width: 19px;
    height: 19px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--ob-warn);
    color: var(--ob-fill);
    border: 2.5px solid var(--ob-ink);
  }

  .name-input {
    width: 190px;
    max-width: 100%;
    padding: 8px 6px;
    background: none;
    color: var(--ob-ink);
    border: none;
    font-family: inherit;
    font-size: var(--text-md);
    font-weight: 600;
    outline: none;
  }
  .name-input::placeholder {
    color: color-mix(in srgb, var(--ob-ink) 42%, transparent);
    font-weight: 500;
  }

  .hint,
  .name-error {
    font-size: var(--text-xs);
    min-height: 16px;
  }
  .hint {
    color: var(--ob-text-dim);
    opacity: 0.7;
  }
  .name-error {
    color: var(--ob-warn);
    font-weight: 600;
  }

  .consent {
    margin: 0;
    font-size: 11.5px;
    color: var(--ob-text-dim);
    opacity: 0;
    transition: opacity 500ms ease 200ms;
  }
  .consent.show {
    opacity: 0.75;
  }
  .consent-link {
    font-size: inherit;
    color: var(--ob-accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    padding: 0;
  }

  .iris {
    position: fixed;
    top: 50%;
    left: 50%;
    width: 120vmax;
    height: 120vmax;
    margin: -60vmax 0 0 -60vmax;
    border-radius: 50%;
    background: var(--bg-primary);
    transform: scale(0);
    pointer-events: none;
    z-index: 40;
  }
  .exiting .iris {
    animation: iris 700ms cubic-bezier(0.5, 0, 0.6, 1) forwards;
  }

  @keyframes iris {
    to {
      transform: scale(1);
    }
  }

  @media (max-width: 640px) {
    .name-input {
      width: 170px;
    }
  }
</style>
