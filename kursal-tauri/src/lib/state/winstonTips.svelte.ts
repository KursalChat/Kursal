import { browser } from '$app/environment';

export type WinstonTipId = 'autostart' | 'fileOffer' | 'offlineMessages' | 'verifyContact';

export interface WinstonTipDef {
  img: string;
  titleKey: string;
  bodyKey: string;
  ctaKey?: string;
  dismissKey: string;
}

const TIPS: Record<WinstonTipId, WinstonTipDef> = {
  autostart: {
    img: '/winston.webp',
    titleKey: 'tips.autostart.title',
    bodyKey: 'tips.autostart.body',
    ctaKey: 'tips.autostart.cta',
    dismissKey: 'tips.notNow',
  },
  fileOffer: {
    img: '/winston.webp',
    titleKey: 'tips.fileOffer.title',
    bodyKey: 'tips.fileOffer.body',
    dismissKey: 'tips.dismiss',
  },
  offlineMessages: {
    img: '/winston.webp',
    titleKey: 'tips.offlineMessages.title',
    bodyKey: 'tips.offlineMessages.body',
    dismissKey: 'tips.dismiss',
  },
  verifyContact: {
    img: '/winston-key.webp',
    titleKey: 'tips.verifyContact.title',
    bodyKey: 'tips.verifyContact.body',
    ctaKey: 'tips.verifyContact.cta',
    dismissKey: 'tips.notNow',
  },
};

function seenKey(id: WinstonTipId) {
  return `kursal_tip_${id}`;
}

function createWinstonTips() {
  let activeId = $state<WinstonTipId | null>(null);
  let onCta: (() => void) | null = null;

  function hasSeen(id: WinstonTipId): boolean {
    return browser && localStorage.getItem(seenKey(id)) === 'done';
  }

  function markSeen(id: WinstonTipId) {
    if (browser) localStorage.setItem(seenKey(id), 'done');
  }

  // One-shot: ignored if already seen or another tip is showing. A suppressed
  // tip is NOT marked seen, so it retries on the next trigger.
  function show(id: WinstonTipId, cta?: () => void): boolean {
    if (!browser || activeId || hasSeen(id)) return false;
    activeId = id;
    onCta = cta ?? null;
    return true;
  }

  function close() {
    if (activeId) markSeen(activeId);
    activeId = null;
    onCta = null;
  }

  function dismiss() {
    close();
  }

  function confirm() {
    const fn = onCta;
    close();
    fn?.();
  }

  return {
    get activeId() {
      return activeId;
    },
    get def(): WinstonTipDef | null {
      return activeId ? TIPS[activeId] : null;
    },
    hasSeen,
    markSeen,
    show,
    dismiss,
    confirm,
  };
}

export const winstonTips = createWinstonTips();
