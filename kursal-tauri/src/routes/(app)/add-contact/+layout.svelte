<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { QrCode, FileArchive, Radar } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  const methods = [
    {
      label: t('addContact.methodSelection.otpLabel'),
      href: '/add-contact/otp',
      icon: QrCode,
      tour: 'otp-tab',
    },
    {
      label: t('addContact.methodSelection.ltcLabel'),
      href: '/add-contact/ltc',
      icon: FileArchive,
      tour: 'ltc-tab',
    },
    {
      label: t('addContact.methodSelection.nearbyLabel'),
      href: '/add-contact/nearby',
      icon: Radar,
      tour: 'nearby-tab',
    },
  ];

  let { children } = $props();
</script>

<div class="add-contact">
  <header class="ac-header" data-tauri-drag-region>
    <h2><span class="prompt">~/</span>{t('addContact.methodSelection.heading')}</h2>
  </header>

  <nav class="tabs">
    {#each methods as m}
      <button
        class="tab"
        data-active={page.url.pathname.startsWith(m.href)}
        data-tour={m.tour}
        aria-label={m.label}
        onclick={() => goto(m.href)}
      >
        <m.icon size={15} />
        <span class="tab-text">{m.label}</span>
      </button>
    {/each}
  </nav>

  <div class="ac-body">
    {@render children()}
  </div>
</div>

<style>
  .add-contact {
    --ac-pad: max(16px, calc((100% - 760px) / 2));
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .ac-header {
    height: var(--header-height);
    padding: 0 16px;
    display: flex;
    align-items: center;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset 0 -1px 0 var(--panel-border);
    flex-shrink: 0;
  }

  .ac-header h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 700;
  }
  .prompt {
    color: var(--accent);
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding-block: 8px;
    padding-inline: var(--ac-pad);
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset 0 -1px 0 var(--panel-border);
    flex-shrink: 0;
    overflow-x: auto;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    transition: all var(--transition);
    border: 1px solid transparent;
    min-width: 0;
  }
  .tab :global(svg) {
    flex-shrink: 0;
  }
  .tab-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tab[data-active='true'],
  .tab[data-active='true']:hover {
    background: var(--accent-solid);
    color: #fff;
    border-color: var(--accent-solid);
  }

  .ac-body {
    flex: 1;
    overflow-y: auto;
    padding-block: 20px;
    padding-inline: var(--ac-pad);
  }

  @media (max-width: 768px) {
    .tabs {
      padding-inline: 12px;
    }
    .tab-text {
      display: none;
    }
    .ac-body {
      padding-block: 16px;
      padding-inline: 12px;
    }
  }
</style>
