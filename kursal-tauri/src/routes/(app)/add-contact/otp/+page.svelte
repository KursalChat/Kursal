<script lang="ts">
  import { log } from '$lib/utils/log';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import { clearOtpSession, loadOtpSession, saveOtpSession } from '$lib/utils/otpSession';
  import { generateOtp, publishOtp, fetchOtp } from '$lib/api/otp';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { goto } from '$app/navigation';
  import { notifications } from '$lib/state/notifications.svelte';
  import { errorText, parseError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import Button from '$lib/components/Button.svelte';
  import Segmented from '$lib/components/settings/Segmented.svelte';
  import QRCode from 'qrcode';
  import { Copy, QrCode, Type, RefreshCw, ClipboardPaste, ScanLine, Link2 } from 'lucide-svelte';
  import { isMobile } from '$lib/api/window';
  import { t } from '$lib/i18n';

  let shareView = $state<'words' | 'qr'>('words');
  let otp = $state<string | null>(null);
  let qrDataUrl = $state<string | null>(null);
  let status = $state<'idle' | 'generating' | 'waiting' | 'done'>('idle');
  let inputOtp = $state('');
  let fetchStatus = $state<'idle' | 'loading' | 'error'>('idle');
  let fetchError = $state('');
  let expiresAt = $state<number | null>(null);
  let countdown = $state(600);
  let countdownInterval: ReturnType<typeof setInterval> | null = null;
  let justExpired = $state(false);
  let justUsed = $state(false);
  let receiveSection = $state<HTMLElement | null>(null);
  const copiedCode = flash();
  const copiedLink = flash();
  const OTP_LINK_PREFIX = 'kursal://otp/';

  function buildOtpLink(value: string): string {
    return OTP_LINK_PREFIX + encodeURIComponent(value);
  }

  function normalizeOtp(value: string): string {
    let s = value.trim();
    if (s.startsWith(OTP_LINK_PREFIX)) s = s.slice(OTP_LINK_PREFIX.length);
    try {
      s = decodeURIComponent(s);
    } catch {
      // leave as-is if not valid percent-encoding (e.g. raw pasted words)
    }
    return s.trim();
  }

  function persistOtpState() {
    saveOtpSession(otp && expiresAt ? { otp, shareView, expiresAt } : null);
  }

  function clearPersistedOtpState() {
    clearOtpSession();
  }

  function stopCountdown(clearState = false) {
    if (countdownInterval) clearInterval(countdownInterval);
    countdownInterval = null;

    if (clearState) {
      status = 'idle';
      otp = null;
      qrDataUrl = null;
      expiresAt = null;
      countdown = 600;
      clearPersistedOtpState();
    }
  }

  function updateCountdown() {
    if (!expiresAt) return;

    const remainingSeconds = Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000));
    countdown = remainingSeconds;

    if (remainingSeconds <= 0) {
      justExpired = true;
      stopCountdown(true);
    }
  }

  function startCountdown(nextExpiresAt: number) {
    expiresAt = nextExpiresAt;
    updateCountdown();

    if (countdownInterval) clearInterval(countdownInterval);
    countdownInterval = setInterval(updateCountdown, 1000);
  }

  async function handleGenerateCode() {
    status = 'generating';
    justExpired = false;
    justUsed = false;
    shareView = 'words';
    try {
      const result = await generateOtp();
      otp = result.otp;
      await publishOtp(result.otp);
      await renderQr(buildOtpLink(result.otp));
      startCountdown(Date.now() + 10 * 60 * 1000);
      status = 'waiting';
      persistOtpState();
    } catch (e) {
      notifications.push(t('addContact.otp.generateError'), 'error');
      log.error('Generate OTP failed:', e);
      status = 'idle';
    }
  }

  async function renderQr(code: string) {
    try {
      qrDataUrl = await QRCode.toDataURL(code, {
        errorCorrectionLevel: 'M',
        margin: 1,
        width: 360,
        color: {
          dark: '#0f172a',
          light: '#f8fafc',
        },
      });
    } catch (e) {
      qrDataUrl = null;
      log.error('Failed to render QR:', e);
    }
  }

  async function handleFetchOtp() {
    fetchStatus = 'loading';
    fetchError = '';
    try {
      const contact = await fetchOtp(normalizeOtp(inputOtp));
      contactsState.upsert(contact);
      notifications.push(t('addContact.otp.contactAdded'), 'success');
      fetchStatus = 'idle';
      goto('/chat/' + contact.userId);
    } catch (e) {
      fetchStatus = 'error';
      fetchError = fetchErrorText(e);
      log.error('Fetch OTP failed:', e);
    }
  }

  // The core answers a refused handshake with the reason in the error message.
  function fetchErrorText(e: unknown): string {
    const raw = parseError(e).message.toLowerCase();
    if (raw.includes('already used')) return t('addContact.otp.alreadyUsedError');
    if (raw.includes('expired')) return t('addContact.otp.expiredError');
    if (raw.includes('no answer')) return t('addContact.otp.noAnswerError');
    if (raw.includes('not found')) return t('addContact.otp.invalidError');
    return errorText(e, 'addContact.otp.fetchError');
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    if (fetchStatus !== 'loading' && inputOtp.trim()) void handleFetchOtp();
  }

  async function copyCode() {
    if (otp) {
      try {
        await navigator.clipboard.writeText(otp);
        copiedCode.trigger();
      } catch (e) {
        log.error('Failed to copy:', e);
      }
    }
  }

  async function copyLink() {
    if (otp) {
      try {
        await navigator.clipboard.writeText(buildOtpLink(otp));
        copiedLink.trigger();
      } catch (e) {
        log.error('Failed to copy link:', e);
      }
    }
  }

  async function pasteCode() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        inputOtp = normalizeOtp(text);
      }
    } catch (e) {
      log.error('Failed to paste from clipboard:', e);
    }
  }

  async function scanQr() {
    if (!isMobile) return;
    try {
      const { scan, Format, checkPermissions, requestPermissions } =
        await import('@tauri-apps/plugin-barcode-scanner');
      const initialPerm = (await checkPermissions()) as string;
      const perm =
        initialPerm === 'granted' ? initialPerm : ((await requestPermissions()) as string);
      if (perm !== 'granted') {
        notifications.push(t('addContact.otp.cameraPermissionDenied'), 'error');
        return;
      }
      const result = await scan({
        windowed: false,
        formats: [Format.QRCode],
        cameraDirection: 'back',
      });
      if (result?.content) {
        inputOtp = normalizeOtp(result.content);
      }
    } catch (e) {
      const msg = parseError(e).message;
      if (!msg.toLowerCase().includes('cancel')) {
        notifications.push(t('addContact.otp.qrScanError'), 'error');
        log.error('QR scan failed:', e);
      }
    }
  }

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    const receiveCode = params.get('receive');
    if (receiveCode) {
      inputOtp = normalizeOtp(receiveCode);
      tick().then(() => {
        receiveSection?.scrollIntoView({ behavior: 'smooth', block: 'start' });
      });
    }

    const restored = loadOtpSession();
    if (restored) {
      if (restored.otp && restored.expiresAt > Date.now()) {
        otp = restored.otp;
        shareView = restored.shareView;
        startCountdown(restored.expiresAt);
        void renderQr(buildOtpLink(restored.otp));
        status = 'waiting';
      } else {
        clearPersistedOtpState();
      }
    }

    // The core pairs at most one contact per code, so the shared words are dead
    // the moment it fires; drop them instead of leaving a live-looking timer.
    const unlisten = listen('otp_consumed', () => {
      if (status !== 'waiting') return;
      justUsed = true;
      stopCountdown(true);
    });

    return () => {
      stopCountdown();
      void unlisten.then((off) => off());
    };
  });

  const minutes = $derived(Math.floor(countdown / 60));
  const seconds = $derived(countdown % 60);
  const formattedTime = $derived(`${minutes}:${seconds.toString().padStart(2, '0')}`);
</script>

<div class="otp-flow">
  <section class="mode-content">
    <div class="heading-row">
      <div>
        <h3>{t('addContact.otp.shareTitle')}</h3>
        <p class="subtle">
          {t('addContact.otp.shareSubtitle')}
        </p>
      </div>

      {#if otp && status === 'waiting'}
        <span class="timer-pill" class:warning={countdown <= 60}
          >{t('addContact.otp.expiringIn', { time: formattedTime })}</span
        >
      {/if}
    </div>

    {#if otp && status === 'waiting'}
      <p class="explanation">
        {t('addContact.otp.sendInstructions')}
      </p>

      <Segmented
        size="sm"
        value={shareView}
        options={[
          { value: 'words', label: t('addContact.otp.wordsLabel'), icon: Type },
          { value: 'qr', label: t('addContact.otp.qrLabel'), icon: QrCode },
        ]}
        onchange={(v) => (shareView = v)}
      />

      {#if shareView === 'words'}
        <div class="code-display" role="group" aria-label={t('addContact.otp.passphraseAriaLabel')}>
          <ol class="code-grid">
            {#each otp.split(/\s+/).filter((s) => s.length > 0) as word, i}
              <li class="code-word">
                <span class="word-index">{(i + 1).toString().padStart(2, '0')}</span>
                <span class="word-value">{word}</span>
              </li>
            {/each}
          </ol>
        </div>
      {:else}
        <div class="qr-card">
          {#if qrDataUrl}
            <img class="qr-image" src={qrDataUrl} alt={t('addContact.otp.qrCodeAlt')} />
          {:else}
            <p class="qr-fallback">
              {t('addContact.otp.qrUnavailable')}
            </p>
          {/if}
        </div>
      {/if}

      <div class="action-row">
        <Button
          variant="secondary"
          onclick={copyCode}
          success={copiedCode.active}
          successLabel={t('common.copied')}
        >
          <Copy size={14} />
          {t('addContact.otp.copyButton')}
        </Button>
        <Button
          variant="secondary"
          onclick={copyLink}
          success={copiedLink.active}
          successLabel={t('common.copied')}
        >
          <Link2 size={14} />
          {t('addContact.otp.copyLinkButton')}
        </Button>
        <Button variant="secondary" onclick={handleGenerateCode}>
          <RefreshCw size={14} />
          {t('addContact.otp.regenerateButton')}
        </Button>
      </div>
    {:else}
      {#if justUsed}
        <div class="expired-banner" role="status">
          {t('addContact.otp.codeUsed')}
        </div>
      {:else if justExpired}
        <div class="expired-banner" role="status">
          {t('addContact.otp.codeExpired')}
        </div>
      {/if}
      <p class="explanation">
        {t('addContact.otp.recommendedMethod')}
      </p>
      <Button variant="primary" loading={status === 'generating'} onclick={handleGenerateCode}>
        {t('addContact.otp.generateButton')}
      </Button>
      <p class="help-text">
        {t('addContact.otp.helpText')}
      </p>
    {/if}
  </section>

  <div class="divider" role="separator" aria-hidden="true">
    <span>{t('addContact.otp.orDivider')}</span>
  </div>

  <section class="mode-content" bind:this={receiveSection}>
    <div class="heading-row">
      <div>
        <h3>{t('addContact.otp.receiveTitle')}</h3>
        <p class="subtle">{t('addContact.otp.receiveSubtitle')}</p>
      </div>
    </div>

    <div class="input-wrap">
      <textarea
        bind:value={inputOtp}
        placeholder={t('addContact.otp.inputPlaceholder')}
        rows="3"
        disabled={fetchStatus === 'loading'}
        onkeydown={handleInputKeydown}
        autocapitalize="off"
        autocomplete="off"
        spellcheck="false"></textarea>
      <div class="input-actions">
        {#if isMobile}
          <button
            class="inline-action"
            type="button"
            onclick={scanQr}
            title={t('addContact.otp.scanButton')}
          >
            <ScanLine size={14} />
            {t('addContact.otp.scanLabel')}
          </button>
        {/if}
        <button
          class="inline-action"
          type="button"
          onclick={pasteCode}
          title={t('addContact.otp.pasteButton')}
        >
          <ClipboardPaste size={14} />
          {t('addContact.otp.pasteLabel')}
        </button>
      </div>
    </div>

    {#if fetchError}
      <div class="error-message">{fetchError}</div>
    {/if}

    <Button
      variant="primary"
      loading={fetchStatus === 'loading'}
      disabled={!inputOtp.trim()}
      onclick={handleFetchOtp}
    >
      {t('addContact.otp.connectButton')}
    </Button>
  </section>
</div>

<style>
  .otp-flow {
    max-width: 760px;
    display: grid;
    gap: 16px;
  }

  .mode-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    padding: 18px;
  }

  .heading-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
  }

  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    line-height: 1.2;
    color: var(--text-primary);
  }

  .subtle {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .timer-pill {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: var(--radius-md);
    padding: 6px 10px;
    white-space: nowrap;
  }

  .timer-pill.warning {
    color: var(--warning);
    border-color: color-mix(in srgb, var(--warning) 45%, transparent);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    animation: timer-pulse 1s ease-in-out infinite;
  }
  @keyframes timer-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }

  .expired-banner {
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    color: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    font-size: 13px;
    font-weight: 600;
    line-height: 1.5;
  }

  .explanation {
    margin: 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }

  .help-text {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5;
  }

  .code-grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .code-word {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-input);
    transition: border-color var(--transition);
  }
  .code-word:hover {
    border-color: var(--accent-selected);
  }

  .word-index {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    padding-right: 10px;
    border-right: 1px solid var(--border);
  }

  .word-value {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 600;
    line-height: 1.35;
    overflow-wrap: anywhere;
    word-break: normal;
    color: var(--text-primary);
  }

  .qr-card {
    display: flex;
    align-items: center;
    justify-content: flex-start;
  }

  .qr-image {
    width: min(320px, 100%);
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
  }

  .action-row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .qr-fallback {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  :global(.action-row .button) {
    width: auto;
    min-width: 160px;
    flex: 1 1 200px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 4px 0;
  }

  .divider::before,
  .divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .input-wrap {
    position: relative;
  }

  textarea {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    padding: 12px 92px 12px 14px;
    font-size: 13px;
    resize: vertical;
    min-height: 88px;
    width: 100%;
    font-family: var(--font-mono);
    transition:
      border-color var(--transition),
      box-shadow var(--transition);
  }

  textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-dim);
  }

  textarea:disabled {
    opacity: 0.5;
  }

  .input-actions {
    position: absolute;
    right: 8px;
    top: 8px;
    display: flex;
    gap: 6px;
  }

  .inline-action {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    padding: 6px 8px;
    transition:
      background var(--transition),
      color var(--transition),
      border-color var(--transition);
  }

  .inline-action:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .error-message {
    background: var(--danger-dim);
    color: var(--danger);
    padding: 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    line-height: 1.5;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
  }

  :global(.mode-content > .button) {
    width: 100%;
  }

  @media (max-width: 740px) {
    .mode-content {
      padding: 16px;
    }

    /* Stack the scan/paste actions below the field so they can't overlap
     * typed text on narrow widths. */
    textarea {
      padding-right: 14px;
    }
    .input-actions {
      position: static;
      justify-content: flex-end;
      margin-top: 8px;
    }

    .code-grid {
      grid-template-columns: 1fr;
    }

    .heading-row {
      flex-direction: column;
      align-items: flex-start;
    }

    .action-row {
      flex-direction: column;
    }

    :global(.action-row .button) {
      min-width: 0;
      width: 100%;
      flex: 1 1 auto;
    }
  }
</style>
