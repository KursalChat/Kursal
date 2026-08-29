<script lang="ts">
  import { Forward } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { sendText } from '$lib/api/messages';
  import { notifications } from '$lib/state/notifications.svelte';
  import { log } from '$lib/utils/log';
  import PickerDialog from '$lib/components/PickerDialog.svelte';
  import ContactPickerRow from '$lib/components/ContactPickerRow.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';

  let { content, onClose }: { content: string; onClose: () => void } = $props();

  let query = $state('');
  let sendingTo = $state<string | null>(null);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = contactsState.contacts;
    return q ? list.filter((c) => c.displayName.toLowerCase().includes(q)) : list;
  });

  async function forward(userId: string, name: string) {
    if (sendingTo) return;
    sendingTo = userId;
    const pendingId = messagesState.appendPendingText(userId, content);
    try {
      const realId = await sendText(userId, content);
      messagesState.replaceId(pendingId, userId, realId);
    } catch (e) {
      messagesState.updateStatusIfSending(pendingId, userId, 'queued');
      log.error('Forward send failed, queued for offline:', e);
    }
    notifications.push(t('chat.forward.success', { name }), 'success');
    onClose();
  }
</script>

<PickerDialog
  title={t('chat.forward.title')}
  icon={Forward}
  preview={content}
  bind:query
  searchPlaceholder={t('chat.forward.searchPlaceholder')}
  {onClose}
>
  {#if filtered.length === 0}
    <div class="empty">{t('chat.forward.noContacts')}</div>
  {:else}
    {#each filtered as c (c.userId)}
      <ContactPickerRow
        contact={c}
        disabled={!!sendingTo}
        onclick={() => forward(c.userId, c.displayName)}
      >
        {#snippet trailing()}
          {#if sendingTo === c.userId}
            <Spinner size={14} />
          {/if}
        {/snippet}
      </ContactPickerRow>
    {/each}
  {/if}
</PickerDialog>

<style>
  .empty {
    padding: 24px 12px;
    text-align: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
