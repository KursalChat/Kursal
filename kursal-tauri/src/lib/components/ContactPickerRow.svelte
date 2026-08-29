<script lang="ts">
  import type { Snippet } from 'svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import type { ContactResponse } from '$lib/types';

  let {
    contact,
    disabled = false,
    onclick,
    trailing,
  }: {
    contact: ContactResponse;
    disabled?: boolean;
    onclick: () => void;
    trailing?: Snippet;
  } = $props();
</script>

<button class="row" {disabled} {onclick}>
  <Avatar name={contact.displayName} src={contact.avatarPath} size={34} />
  <span class="name">{contact.displayName}</span>
  {#if trailing}{@render trailing()}{/if}
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition: background var(--transition);
  }
  @media (hover: hover) {
    .row:hover:not(:disabled) {
      background: var(--bg-hover);
    }
  }
  .row:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
