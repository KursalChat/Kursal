<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import { Lock, Fingerprint } from 'lucide-svelte';
  import { authenticate } from '@tauri-apps/plugin-biometric';
  import { t } from '$lib/i18n';

  let { onUnlock }: { onUnlock: () => void } = $props();

  let attempting = $state(false);
  let failed = $state(false);

  async function tryAuth() {
    attempting = true;
    failed = false;
    try {
      await authenticate(t('biometricLock.reason'), {
        allowDeviceCredential: true,
        cancelTitle: t('biometricLock.cancelTitle'),
        fallbackTitle: t('biometricLock.fallbackTitle'),
        title: t('biometricLock.androidTitle'),
        subtitle: t('biometricLock.androidSubtitle'),
        confirmationRequired: false,
      });
      onUnlock();
    } catch (e) {
      log.error('biometric auth failed', e);
      failed = true;
    } finally {
      attempting = false;
    }
  }

  onMount(() => {
    void tryAuth();
  });
</script>

<div class="lock-overlay">
  <div class="lock-card">
    <div class="icon-wrap">
      <Lock size={48} />
    </div>
    <h1>{t('biometricLock.heading')}</h1>
    <p>{t('biometricLock.subtitle')}</p>
    {#if failed}
      <p class="err">{t('biometricLock.failed')}</p>
    {/if}
    <button class="auth-btn" onclick={tryAuth} disabled={attempting}>
      <Fingerprint size={18} />
      <span>
        {attempting ? t('biometricLock.authenticating') : t('biometricLock.unlock')}
      </span>
    </button>
  </div>
</div>

<style>
  .lock-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: var(--bg-primary, #0b0d12);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: max(24px, var(--safe-top)) 24px max(24px, var(--safe-bottom));
  }
  .lock-card {
    max-width: 360px;
    width: 100%;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .icon-wrap {
    width: 88px;
    height: 88px;
    border-radius: 50%;
    background: var(--accent-dim);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 8px;
  }
  h1 {
    font-size: 22px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }
  p {
    font-size: 14px;
    color: var(--text-secondary);
    margin: 0;
    line-height: 1.45;
  }
  .err {
    color: #fda4af;
    font-size: 13px;
  }
  .auth-btn {
    margin-top: 12px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 22px;
    border-radius: var(--radius-md);
    background: var(--accent-solid);
    color: #fff;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: opacity var(--transition);
  }
  .auth-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
