<script lang="ts">
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { Home } from 'lucide-svelte';
  import { t } from '$lib/i18n';
</script>

<div class="error-container">
  <div class="error-content">
    <span class="status">{$page.status}</span>
    <h1>{t('error.title')}</h1>
    <p class="message">{t('error.message')}</p>
    {#if $page.error?.message}
      <p class="detail">{$page.error.message}</p>
    {/if}
    <button class="back-btn" onclick={() => goto('/chat')}>
      <Home size={15} />
      {t('error.backToChats')}
    </button>
  </div>
</div>

<style>
  .error-container {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    padding: 24px;
  }

  .error-content {
    text-align: center;
    max-width: 380px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .status {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.08em;
  }

  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .message {
    margin: 0;
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.6;
  }

  .detail {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 12px;
    font-family: var(--font-mono);
    word-break: break-word;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
    padding: 9px 16px;
    border-radius: var(--radius-md);
    background: var(--accent-solid);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .back-btn:hover {
    background: var(--accent-hover);
  }
</style>
