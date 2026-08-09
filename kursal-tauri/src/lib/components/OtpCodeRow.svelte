<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import QRCode from 'qrcode';
  import { Check, Copy, KeyRound, Link2, Share2 } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { log } from '$lib/utils/log';
  import { clearOtpSession, loadOtpSession, saveOtpSession } from '$lib/utils/otpSession';
  import { generateOtp, publishOtp } from '$lib/api/otp';
  import { flash } from '$lib/utils/flash.svelte';
  import { canShareText, shareText, shareAnchor } from '$lib/api/share';
  import Button from '$lib/components/Button.svelte';
  import ConnectRow from '$lib/components/ConnectRow.svelte';
  import { t } from '$lib/i18n';

  type CodeStatus = 'idle' | 'creating' | 'ready' | 'expired' | 'used' | 'failed';

  const OTP_LINK_PREFIX = 'kursal://otp/';
  const OTP_TTL_MS = 10 * 60 * 1000;

  let otp = $state<string | null>(null);
  let qrDataUrl = $state<string | null>(null);
  let codeStatus = $state<CodeStatus>('idle');
  let expiresAt = $state<number | null>(null);
  let countdown = $state(0);
  let countdownInterval: ReturnType<typeof setInterval> | null = null;
  let shareActions = $state<HTMLElement | null>(null);

  const copiedLink = flash();
  const copiedWords = flash();
  const ready = $derived(codeStatus === 'ready');
  const words = $derived(otp ? otp.split(/\s+/).filter(Boolean) : []);
  const formattedTime = $derived(
    `${Math.floor(countdown / 60)}:${(countdown % 60).toString().padStart(2, '0')}`
  );

  const hint = $derived.by((): string | undefined => {
    switch (codeStatus) {
      case 'creating':
        return t('addContact.connect.creatingHint');
      case 'ready':
        return t('addContact.connect.readyHint');
      case 'expired':
        return t('addContact.connect.codeExpired');
      case 'used':
        return t('addContact.connect.codeUsed');
      case 'failed':
        return t('addContact.connect.createFailed');
      default:
        return t('addContact.connect.rowMakeCodeHint');
    }
  });

  const buttonLabel = $derived.by(() => {
    if (codeStatus === 'failed') return t('addContact.connect.retryButton');
    if (codeStatus === 'expired' || codeStatus === 'used')
      return t('addContact.connect.newCodeButton');
    return t('addContact.connect.makeCodeButton');
  });

  function buildOtpLink(value: string): string {
    return OTP_LINK_PREFIX + encodeURIComponent(value);
  }

  function stopCountdown() {
    if (countdownInterval) clearInterval(countdownInterval);
    countdownInterval = null;
  }

  function startCountdown(nextExpiresAt: number) {
    expiresAt = nextExpiresAt;
    tickCountdown();
    stopCountdown();
    countdownInterval = setInterval(tickCountdown, 1000);
  }

  function tickCountdown() {
    if (!expiresAt) return;
    countdown = Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000));
    if (countdown > 0) return;
    dropCode('expired');
  }

  function dropCode(next: CodeStatus) {
    stopCountdown();
    clearOtpSession();
    otp = null;
    qrDataUrl = null;
    expiresAt = null;
    codeStatus = next;
  }

  async function renderQr(value: string) {
    try {
      qrDataUrl = await QRCode.toDataURL(value, {
        errorCorrectionLevel: 'M',
        margin: 1,
        width: 420,
        color: { dark: '#0f172a', light: '#f8fafc' },
      });
    } catch (e) {
      qrDataUrl = null;
      log.error('Failed to render QR:', e);
    }
  }

  async function createCode() {
    codeStatus = 'creating';
    try {
      const result = await generateOtp();
      await publishOtp(result.otp);
      const nextExpiresAt = Date.now() + OTP_TTL_MS;
      await renderQr(buildOtpLink(result.otp));
      otp = result.otp;
      startCountdown(nextExpiresAt);
      codeStatus = 'ready';
      saveOtpSession({ otp: result.otp, expiresAt: nextExpiresAt });
    } catch (e) {
      log.error('Create OTP failed:', e);
      otp = null;
      qrDataUrl = null;
      clearOtpSession();
      codeStatus = 'failed';
    }
  }

  async function copyWords() {
    if (!otp) return;
    try {
      await writeText(otp);
      copiedWords.trigger();
    } catch (e) {
      log.error('Failed to copy words:', e);
    }
  }

  async function copyLink() {
    if (!otp) return;
    try {
      await writeText(buildOtpLink(otp));
      copiedLink.trigger();
    } catch (e) {
      log.error('Failed to copy link:', e);
    }
  }

  async function sendInvite() {
    if (!otp) return;
    try {
      await shareText(buildOtpLink(otp), shareActions ? shareAnchor(shareActions) : undefined);
    } catch (e) {
      log.error('Share invite failed:', e);
      await copyLink();
    }
  }

  // Restored before the first paint, otherwise `ready` flips after mount and the row
  // plays its open transition on every visit.
  const restored = loadOtpSession();
  if (restored && restored.otp && restored.expiresAt > Date.now()) {
    otp = restored.otp;
    codeStatus = 'ready';
    expiresAt = restored.expiresAt;
    countdown = Math.max(0, Math.ceil((restored.expiresAt - Date.now()) / 1000));
    void renderQr(buildOtpLink(restored.otp));
  } else {
    clearOtpSession();
  }

  onMount(() => {
    if (expiresAt) startCountdown(expiresAt);

    const unlisten = listen('otp_consumed', () => {
      if (codeStatus === 'ready') dropCode('used');
    });

    return () => {
      stopCountdown();
      void unlisten.then((off) => off());
    };
  });
</script>

{#snippet action()}
  {#if !ready}
    <Button variant="primary" loading={codeStatus === 'creating'} onclick={createCode}>
      {buttonLabel}
    </Button>
  {/if}
{/snippet}

<ConnectRow
  icon={KeyRound}
  label={t('addContact.connect.rowMakeCode')}
  {hint}
  status={ready ? t('addContact.connect.expiresIn', { time: formattedTime }) : undefined}
  collapsible={false}
  showBody={ready}
  {action}
>
  <div class="qr-wrap">
    {#if qrDataUrl}
      <img class="qr" src={qrDataUrl} alt={t('addContact.otp.qrCodeAlt')} />
    {:else}
      <div class="qr-placeholder"></div>
    {/if}
  </div>

  <div class="words-row">
    <p class="words" aria-label={t('addContact.otp.passphraseAriaLabel')}>
      {#each words as word}<span>{word}</span>{/each}
    </p>
    <button
      class="copy-words"
      type="button"
      onclick={copyWords}
      title={t('common.copy')}
      aria-label={t('common.copy')}
    >
      {#if copiedWords.active}<Check size={14} />{:else}<Copy size={14} />{/if}
    </button>
  </div>

  <div class="actions" bind:this={shareActions}>
    {#if canShareText}
      <Button variant="primary" onclick={sendInvite}>
        <Share2 size={14} />
        {t('addContact.connect.shareButton')}
      </Button>
    {/if}
    <Button
      variant="secondary"
      onclick={copyLink}
      success={copiedLink.active}
      successLabel={t('common.copied')}
    >
      {#if canShareText}<Copy size={14} />{:else}<Link2 size={14} />{/if}
      {t('addContact.connect.copyLinkButton')}
    </Button>
  </div>
</ConnectRow>

<style>
  .qr-wrap {
    display: flex;
    justify-content: center;
  }

  .qr,
  .qr-placeholder {
    width: min(240px, 100%);
    aspect-ratio: 1;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
  }

  .qr-placeholder {
    background: var(--bg-input);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .actions :global(.button) {
    flex: 1 1 170px;
    width: auto;
  }

  .words-row {
    display: flex;
    align-items: stretch;
    gap: 6px;
  }

  .words {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 4px 10px;
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    user-select: text;
  }

  .copy-words {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 38px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    color: var(--text-secondary);
    transition:
      background var(--transition),
      color var(--transition),
      border-color var(--transition);
  }

  .copy-words:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    border-color: var(--accent);
  }

  .words span {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  @media (max-width: 640px) {
    .actions {
      flex-direction: column;
    }

    .actions :global(.button) {
      width: 100%;
      flex: 1 1 auto;
    }
  }
</style>
