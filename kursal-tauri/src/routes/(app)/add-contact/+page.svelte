<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { readFile } from '@tauri-apps/plugin-fs';
  import { ScanLine, ClipboardPaste, FolderOpen } from 'lucide-svelte';
  import { readText } from '@tauri-apps/plugin-clipboard-manager';

  import { log } from '$lib/utils/log';
  import { checkOtpWords, fetchOtp } from '$lib/api/otp';
  import { importLtc } from '$lib/api/ltc';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { errorText, parseError, notifyError } from '$lib/utils/errors';
  import { isMobile } from '$lib/api/window';
  import Button from '$lib/components/Button.svelte';
  import NearbyStrip from '$lib/components/NearbyStrip.svelte';
  import OtpCodeRow from '$lib/components/OtpCodeRow.svelte';
  import OtpWordCheck from '$lib/components/OtpWordCheck.svelte';
  import LtcRow from '$lib/components/LtcRow.svelte';
  import { t } from '$lib/i18n';

  const OTP_LINK_PREFIX = 'kursal://otp/';
  const OTP_WORD_COUNT = 8;

  let invite = $state('');
  let connectStatus = $state<'idle' | 'loading'>('idle');
  let connectError = $state('');
  let dragging = $state(false);
  let scanning = $state(false);
  let checkedWords = $state<string[]>([]);
  let wordIssues = $state<(string[] | null)[]>([]);
  let checkSeq = 0;

  const busy = $derived(connectStatus === 'loading');

  const stripNonWord = (s: string) => s.toLowerCase().replace(/[^a-z\s]/g, '');

  function normalizeOtp(value: string): string {
    let s = value.trim();
    if (s.toLowerCase().startsWith(OTP_LINK_PREFIX)) s = s.slice(OTP_LINK_PREFIX.length);
    try {
      s = decodeURIComponent(s);
    } catch {
      // leave as-is if not valid percent-encoding (e.g. raw pasted words)
    }
    return s
      .toLowerCase()
      .replace(/[^a-z]+/g, ' ')
      .trim()
      .split(' ')
      .filter(Boolean)
      .slice(0, OTP_WORD_COUNT)
      .join(' ');
  }

  function typedWords(): string[] | null {
    const parts = normalizeOtp(invite).split(/\s+/).filter(Boolean);
    if (!parts.length || !parts.every((w) => /^[a-z]+$/i.test(w))) return null;
    return parts;
  }

  function clearWordCheck() {
    checkSeq++;
    checkedWords = [];
    wordIssues = [];
  }

  async function wordsAreUsable(): Promise<boolean> {
    const parts = checkedWords.length ? checkedWords.map((w) => w.trim()) : typedWords();
    if (!parts) return true;

    const filled = parts.filter(Boolean);
    if (filled.length !== OTP_WORD_COUNT) {
      connectError = t('addContact.otp.wordCountError', { count: filled.length });
      return false;
    }

    const seq = ++checkSeq;
    let issues: (string[] | null)[];
    try {
      issues = await checkOtpWords(parts);
    } catch (e) {
      // A failing spellcheck must not block a code that may well be correct.
      log.warn('OTP word check failed:', e);
      return true;
    }
    if (seq !== checkSeq) return false;

    if (!issues.some((suggestions) => suggestions !== null)) {
      clearWordCheck();
      return true;
    }

    checkedWords = parts;
    wordIssues = issues;
    connectError = t('addContact.otp.wordCheckError');
    return false;
  }

  function applySuggestion(index: number, word: string) {
    const next = [...checkedWords];
    const nextIssues = [...wordIssues];
    next[index] = word;
    nextIssues[index] = null;
    checkedWords = next;
    wordIssues = nextIssues;
    invite = next.join(' ');

    if (nextIssues.every((suggestions) => suggestions === null)) {
      clearWordCheck();
      connectError = '';
      void connectWithCode(false);
    }
  }

  function editWord(index: number, word: string) {
    const next = [...checkedWords];
    const nextIssues = [...wordIssues];
    next[index] = word;
    nextIssues[index] = null;
    checkedWords = next;
    wordIssues = nextIssues;
    invite = next.filter(Boolean).join(' ');
  }

  function resetInviteFeedback() {
    if (wordIssues.length) clearWordCheck();
    if (connectError) connectError = '';
  }

  function handleInviteInput(e: Event) {
    sanitizeInvite(e.currentTarget as HTMLTextAreaElement);
    resetInviteFeedback();
  }

  function cleanField(raw: string): string {
    const lowered = stripNonWord(raw);
    const words = lowered.split(/\s+/).filter(Boolean).slice(0, OTP_WORD_COUNT);
    const open = /\s$/.test(lowered) && words.length < OTP_WORD_COUNT;
    return words.join(' ') + (open ? ' ' : '');
  }

  // Pasted links keep their punctuation; anything else is passphrase text, so it is held
  // to the wordlist alphabet. The element is rewritten directly to keep the caret put.
  function sanitizeInvite(el: HTMLTextAreaElement) {
    const raw = el.value;
    if (raw.trimStart().startsWith(OTP_LINK_PREFIX)) {
      invite = raw;
      return;
    }

    const clean = cleanField(raw);
    if (clean === raw) {
      invite = raw;
      return;
    }

    const caret = Math.min(
      cleanField(raw.slice(0, el.selectionStart ?? raw.length)).length,
      clean.length
    );
    el.value = clean;
    el.setSelectionRange(caret, caret);
    invite = clean;
  }

  async function connectWithCode(scanned: boolean) {
    if (!invite.trim() || busy) return;
    connectStatus = 'loading';

    if (!scanned && !(await wordsAreUsable())) {
      connectStatus = 'idle';
      return;
    }

    try {
      const contact = await fetchOtp(normalizeOtp(invite));
      contactsState.upsert(contact);
      notifications.push(t('addContact.connect.unverifiedToast'), 'success');
      connectStatus = 'idle';
      goto('/chat/' + contact.userId);
    } catch (e) {
      connectStatus = 'idle';
      connectError = fetchErrorText(e);
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

  async function scanQr() {
    if (!isMobile || scanning) return;
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

      scanning = true;
      document.documentElement.classList.add('scanning');
      const result = await scan({
        windowed: true,
        formats: [Format.QRCode],
        cameraDirection: 'back',
      });
      closeScanner();
      if (result?.content) {
        invite = normalizeOtp(result.content);
        await connectWithCode(true);
      }
    } catch (e) {
      closeScanner();
      const msg = parseError(e).message;
      if (!msg.toLowerCase().includes('cancel')) {
        notifyError(e, 'addContact.otp.qrScanError');
      }
    }
  }

  function closeScanner() {
    scanning = false;
    document.documentElement.classList.remove('scanning');
  }

  async function cancelScan() {
    if (!scanning) return;
    try {
      const { cancel } = await import('@tauri-apps/plugin-barcode-scanner');
      await cancel();
    } catch (e) {
      log.error('QR scan cancel failed:', e);
    }
    closeScanner();
  }

  async function importCard(load: () => Promise<number[]>) {
    if (busy) return;
    connectStatus = 'loading';
    connectError = '';
    try {
      const contact = await importLtc(await load());
      contactsState.upsert(contact);
      notifications.push(t('addContact.ltc.importSuccess'), 'success');
      connectStatus = 'idle';
      goto('/chat/' + contact.userId);
    } catch (e) {
      connectStatus = 'idle';
      connectError = importErrorText(e);
      log.error('Import failed:', e);
    }
  }

  function importErrorText(e: unknown): string {
    const raw = parseError(e).message.toLowerCase();
    if (raw.includes('expired')) return t('addContact.ltc.expiredError');
    if (raw.includes('already used')) return t('addContact.ltc.alreadyUsedError');
    if (raw.includes('yourself')) return t('addContact.ltc.selfError');
    return t('addContact.ltc.invalidFileError');
  }

  async function importFromPath(path: string) {
    if (!path.endsWith('.kursal')) {
      connectError = t('addContact.ltc.invalidFileType');
      return;
    }
    await importCard(async () => Array.from(await readFile(path)));
  }

  async function importFromFile(file: File) {
    if (!file.name.endsWith('.kursal')) {
      connectError = t('addContact.ltc.invalidFileType');
      return;
    }
    await importCard(async () => Array.from(new Uint8Array(await file.arrayBuffer())));
  }

  async function browseForCard() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        pickerMode: 'document',
        fileAccessMode: 'copy',
        filters: [{ name: 'Kursal data file', extensions: ['kursal', 'application/octet-stream'] }],
      });
      if (selected) {
        const first = Array.isArray(selected) ? selected[0] : selected;
        const path =
          typeof first === 'string' ? first : String((first as { path?: string }).path ?? first);
        await importFromPath(path);
      }
    } catch (e) {
      log.warn(t('addContact.ltc.pickerUnavailable'), e);
    }
  }

  async function pasteInvite() {
    try {
      const text = await readText();
      if (text?.trim()) {
        invite = normalizeOtp(text);
        resetInviteFeedback();
      }
    } catch (e) {
      log.error('Failed to paste from clipboard:', e);
    }
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key !== 'Enter' || e.shiftKey) return;
    e.preventDefault();
    void connectWithCode(false);
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    dragging = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    const file = e.dataTransfer?.files?.[0];
    if (file) await importFromFile(file);
  }

  let lastReceive = '';
  $effect(() => {
    const receive = $page.url.searchParams.get('receive') ?? '';
    if (receive === lastReceive) return;
    lastReceive = receive;
    if (!receive) return;
    invite = normalizeOtp(receive);
    resetInviteFeedback();
  });

  onMount(() => {
    const unlisteners = [
      listen<{ paths: string[] }>('tauri://drag-enter', () => {
        dragging = true;
      }),
      listen<{ paths: string[] }>('tauri://drag-leave', () => {
        dragging = false;
      }),
      listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
        dragging = false;
        const path = event.payload.paths?.[0];
        if (path) await importFromPath(path);
      }),
    ];

    return () => {
      unlisteners.forEach((p) => void p.then((off) => off()));
      void cancelScan();
    };
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') void cancelScan();
  }}
/>

<div
  class="connect"
  role="region"
  aria-label={t('addContact.methodSelection.heading')}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  <div class="drop-overlay" class:visible={dragging} aria-hidden="true">
    {t('addContact.connect.dropOverlay')}
  </div>

  <NearbyStrip />

  <section class="primary">
    <label class="field-label" for="invite-input">{t('addContact.connect.theirCodeTitle')}</label>

    {#if isMobile}
      <Button variant="primary" onclick={scanQr} disabled={busy}>
        <ScanLine size={15} />
        {t('addContact.connect.scanCta')}
      </Button>
    {/if}

    <textarea
      id="invite-input"
      bind:value={invite}
      placeholder={t('addContact.connect.receivePlaceholder')}
      rows="2"
      disabled={busy}
      oninput={handleInviteInput}
      onkeydown={handleInputKeydown}
      autocapitalize="off"
      autocomplete="off"
      spellcheck="false"></textarea>

    <div class="field-actions">
      <div class="chips">
        <button class="chip" type="button" onclick={pasteInvite}>
          <ClipboardPaste size={13} />
          {t('addContact.otp.pasteLabel')}
        </button>
        <button class="chip" type="button" onclick={browseForCard}>
          <FolderOpen size={13} />
          {t('addContact.connect.browseCard')}
        </button>
      </div>

      <Button
        variant="primary"
        loading={busy}
        disabled={busy || !invite.trim()}
        onclick={() => connectWithCode(false)}
      >
        {t('addContact.connect.connectButton')}
      </Button>
    </div>

    {#if connectError}
      <div class="error">{connectError}</div>
    {/if}

    {#if wordIssues.length}
      <OtpWordCheck
        words={checkedWords}
        issues={wordIssues}
        onpick={applySuggestion}
        onedit={editWord}
      />
    {/if}
  </section>

  <div class="divider" role="separator"><span>{t('addContact.connect.orDivider')}</span></div>

  <section class="give">
    <span class="field-label">{t('addContact.connect.yourCodeTitle')}</span>
    <OtpCodeRow />
    <LtcRow />
  </section>
</div>

{#if scanning}
  <div class="scan-overlay">
    <p class="scan-hint">{t('addContact.otp.scanHint')}</p>
    <div class="scan-frame"></div>
    <button class="scan-cancel" type="button" onclick={cancelScan}>
      {t('common.cancel')}
    </button>
  </div>
{/if}

<style>
  .scan-overlay {
    position: fixed;
    inset: 0;
    z-index: 3000;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    padding: calc(var(--safe-top) + 28px) max(20px, var(--safe-right))
      calc(var(--safe-bottom) + 32px) max(20px, var(--safe-left));
    background: transparent;
  }

  .scan-hint {
    margin: 0;
    max-width: 320px;
    padding: 10px 16px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    font-size: 14px;
    text-align: center;
  }

  .scan-frame {
    width: min(72vw, 280px);
    aspect-ratio: 1;
    border: 2px solid rgba(255, 255, 255, 0.9);
    border-radius: 18px;
    box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.45);
  }

  .scan-cancel {
    padding: 13px 40px;
    border: none;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    font-size: var(--text-md);
    font-weight: 600;
    cursor: pointer;
  }

  .connect {
    position: relative;
    max-width: 620px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .drop-overlay {
    position: absolute;
    inset: -8px;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 2px dashed var(--accent);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--bg-secondary) 92%, transparent);
    color: var(--accent);
    font-size: 14px;
    font-weight: 700;
    pointer-events: none;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity var(--transition),
      visibility var(--transition);
  }

  .drop-overlay.visible {
    opacity: 1;
    visibility: visible;
  }

  .primary {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .field-label {
    font-size: var(--text-xs);
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  textarea {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    padding: 13px 14px;
    font-size: var(--text-sm);
    font-family: var(--font-mono);
    resize: vertical;
    min-height: 76px;
    width: 100%;
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

  .field-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-secondary);
    font-size: var(--text-2xs);
    font-weight: 600;
    padding: 6px 9px;
    transition:
      background var(--transition),
      color var(--transition),
      border-color var(--transition);
  }

  @media (hover: hover) {
    .chip:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }

  .field-actions :global(.button) {
    flex-shrink: 0;
    min-width: 130px;
  }

  .primary > :global(.button) {
    width: 100%;
  }

  .error {
    background: var(--danger-dim);
    color: var(--danger);
    padding: 11px 12px;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    line-height: 1.5;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
  }

  .give {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    font-size: var(--text-2xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .divider::before,
  .divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  @media (max-width: 640px) {
    .field-actions {
      flex-direction: column;
      align-items: stretch;
    }

    .field-actions :global(.button) {
      width: 100%;
    }
  }
</style>
