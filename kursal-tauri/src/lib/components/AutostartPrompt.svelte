<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from '$lib/i18n';
  import { log } from '$lib/utils/log';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { winstonTips } from '$lib/state/winstonTips.svelte';
  import { readRaw } from '$lib/utils/storage';
  import { OS } from '$lib/api/window';

  const TOUR_KEY = 'kursal_addcontact_onboarded';

  onMount(() => {
    const timer = setTimeout(() => void maybeAsk(), 1500);
    return () => clearTimeout(timer);
  });

  async function maybeAsk() {
    if (winstonTips.hasSeen('autostart')) return;
    // Wait until the contacts tour was completed in a previous session so
    // first-run prompts don't stack.
    if (readRaw(TOUR_KEY) !== 'done') return;
    if (OS === 'android' || OS === 'ios') return;

    let enable: () => Promise<void>;
    try {
      const mod = await import('@tauri-apps/plugin-autostart');
      if (await mod.isEnabled()) {
        winstonTips.markSeen('autostart');
        return;
      }
      enable = mod.enable;
    } catch (e) {
      log.error('Autostart plugin unavailable:', e);
      return;
    }

    winstonTips.show('autostart', () => {
      void enable()
        .then(() => notifications.push(t('settings.advanced.successAutoStartEnabled'), 'success'))
        .catch((e) => notifyError(e, 'settings.advanced.errorAutoStart'));
    });
  }
</script>
