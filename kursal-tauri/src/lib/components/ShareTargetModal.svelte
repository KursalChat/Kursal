<script lang="ts">
  import { Share2 } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import PickerDialog from '$lib/components/PickerDialog.svelte';
  import ContactPickerRow from '$lib/components/ContactPickerRow.svelte';
  import { t } from '$lib/i18n';
  import type { SharePayload } from '$lib/types';

  let {
    payload,
    onPick,
    onCancel,
  }: {
    payload: SharePayload;
    onPick: (contactId: string) => void;
    onCancel: () => void;
  } = $props();

  let query = $state('');

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = contactsState.contacts;
    return q ? list.filter((c) => c.displayName.toLowerCase().includes(q)) : list;
  });

  const summary = $derived.by(() => {
    if (payload.files.length === 1) return payload.files[0].filename;
    if (payload.files.length > 1)
      return t('shareTarget.manyFiles', { count: payload.files.length });
    return payload.text ?? t('shareTarget.oneFile');
  });
</script>

<PickerDialog
  title={t('shareTarget.title')}
  icon={Share2}
  preview={summary}
  bind:query
  searchPlaceholder={t('shareTarget.searchPlaceholder')}
  maxHeight="min(80vh, 560px, calc(var(--safe-h) - 32px))"
  onClose={onCancel}
>
  {#if contactsState.contacts.length === 0}
    <div class="empty">{t('shareTarget.emptyContacts')}</div>
  {:else if filtered.length === 0}
    <div class="empty">{t('shareTarget.noContacts')}</div>
  {:else}
    {#each filtered as c (c.userId)}
      <ContactPickerRow contact={c} onclick={() => onPick(c.userId)} />
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
