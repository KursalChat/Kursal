<script lang="ts">
  import { t } from '$lib/i18n';

  let { children } = $props();
</script>

<div class="add-contact">
  <header class="ac-header" data-tauri-drag-region>
    <h2><span class="prompt">~/</span>{t('addContact.methodSelection.heading')}</h2>
  </header>

  <div class="ac-body">
    {@render children()}
  </div>
</div>

<style>
  .add-contact {
    --ac-pad-left: max(16px, var(--safe-left), calc((100% - 760px) / 2));
    --ac-pad-right: max(16px, var(--safe-right), calc((100% - 760px) / 2));
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .ac-header {
    height: var(--header-height);
    padding: 0 max(16px, var(--safe-right)) 0 max(16px, var(--safe-left));
    display: flex;
    align-items: center;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset 0 -1px 0 var(--panel-border);
    flex-shrink: 0;
  }

  @media (min-width: 769px) {
    .ac-header {
      height: calc(var(--header-height) + var(--safe-top));
      padding-top: var(--safe-top);
    }
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

  .ac-body {
    flex: 1;
    overflow-y: auto;
    padding-block: 20px calc(20px + var(--safe-bottom));
    padding-inline: var(--ac-pad-left) var(--ac-pad-right);
  }

  @media (max-width: 768px) {
    .ac-body {
      padding-block: 16px calc(16px + var(--safe-bottom));
      padding-inline: max(12px, var(--safe-left)) max(12px, var(--safe-right));
    }
  }
</style>
