<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import { notifications } from '$lib/state/notifications.svelte';
  import { ltcState } from '$lib/state/ltc.svelte';
  import Button from '$lib/components/Button.svelte';
  import ConnectRow from '$lib/components/ConnectRow.svelte';
  import LtcCard from '$lib/components/LtcCard.svelte';
  import LtcLimitsPicker from '$lib/components/LtcLimitsPicker.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';
  import { FileArchive, Plus, ShieldAlert } from 'lucide-svelte';

  let open = $state(false);
  let creating = $state(false);
  let newMaxUses = $state<number | null>(null);
  let newTtl = $state<number | null>(2592000);

  const status = $derived(ltcState.status ? t('addContact.connect.cardActive') : undefined);

  onMount(() => {
    void ltcState.init();
  });

  async function handleCreate() {
    creating = true;
    try {
      await ltcState.create(newMaxUses, newTtl);
      notifications.push(t('addContact.ltc.created'), 'success');
    } catch (e) {
      notifications.push(t('addContact.ltc.createError'), 'error');
      log.error('Creating the LTC failed:', e);
    } finally {
      creating = false;
    }
  }
</script>

<ConnectRow
  icon={FileArchive}
  label={t('addContact.connect.rowCard')}
  hint={t('addContact.connect.rowCardHint')}
  {status}
  bind:open
>
  {#if ltcState.loading}
    <div class="loading"><Spinner size={18} /></div>
  {:else if ltcState.status}
    <LtcCard status={ltcState.status} />
  {:else}
    <LtcLimitsPicker bind:maxUses={newMaxUses} bind:ttlSecs={newTtl} disabled={creating} />
    <Button variant="primary" loading={creating} onclick={handleCreate}>
      <Plus size={14} />
      {t('addContact.ltc.createButton')}
    </Button>
  {/if}

  <p class="footnote">
    <ShieldAlert size={13} />
    {t('addContact.ltc.warningDescription')}
  </p>
</ConnectRow>

<style>
  .loading {
    display: grid;
    place-items: center;
    padding: 20px 0;
  }

  .footnote {
    display: flex;
    gap: 7px;
    align-items: flex-start;
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-muted);
  }

  .footnote :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--warning);
  }
</style>
