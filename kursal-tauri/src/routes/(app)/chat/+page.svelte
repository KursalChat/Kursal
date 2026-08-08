<script lang="ts">
  import { UserPlus } from 'lucide-svelte';
  import { goto } from '$app/navigation';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { t } from '$lib/i18n';

  const hasContacts = $derived(contactsState.contacts.length > 0);
</script>

<div class="empty" data-tauri-drag-region>
  <div class="empty-inner">
    <div class="icon-wrap">
      <img src="/winston.webp" alt={t('chat.empty.mascotAlt')} width="88" height="88" />
    </div>
    {#if hasContacts}
      <h2>{t('chat.empty.hasContactsHeading')}</h2>
      <p>{t('chat.empty.hasContactsBody')}</p>
    {:else}
      <h2>{t('chat.empty.noContactsHeading')}</h2>
      <p>{t('chat.empty.noContactsBody')}</p>
      <button class="add-btn" onclick={() => goto('/add-contact')}>
        <UserPlus size={15} />
        {t('chat.empty.addContactButton')}
      </button>
    {/if}
  </div>
</div>

<style>
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px max(24px, var(--safe-right)) calc(24px + var(--safe-bottom))
      max(24px, var(--safe-left));
    background: transparent;
  }
  @media (min-width: 769px) {
    .empty {
      padding-top: calc(24px + var(--safe-top));
    }
  }

  .empty-inner {
    text-align: center;
    max-width: 360px;
    color: var(--text-muted);
  }

  .icon-wrap {
    position: relative;
    width: 88px;
    height: 88px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 18px;
  }
  .icon-wrap::after {
    content: '';
    position: absolute;
    inset: -14px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--accent-dim), transparent 70%);
    z-index: -1;
  }

  h2 {
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 8px;
    letter-spacing: -0.01em;
  }

  p {
    margin: 0 0 14px;
    font-size: var(--text-sm);
    line-height: 1.55;
    color: var(--text-secondary);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-md);
    font-size: 13.5px;
    font-weight: 600;
    transition: all var(--transition);
  }
  .add-btn:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
  }
  .add-btn:active {
    transform: translateY(0);
  }
</style>
