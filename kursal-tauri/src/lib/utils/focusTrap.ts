// Svelte action: keeps Tab focus cycling inside `node` while mounted.
// `returnFocus` restores focus to the previously focused element on destroy.

type FocusTrapOptions = { active?: boolean; returnFocus?: boolean };

const FOCUSABLE =
  'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])';

export function trapFocus(node: HTMLElement, options: FocusTrapOptions = {}) {
  let active = options.active ?? true;
  const returnFocus = options.returnFocus ?? true;
  const previouslyFocused = document.activeElement as HTMLElement | null;

  function focusables(): HTMLElement[] {
    return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
      (el) => el.offsetParent !== null || el === document.activeElement
    );
  }

  function focusFirst() {
    if (node.contains(document.activeElement)) return;
    const els = focusables();
    (els[0] ?? node).focus();
  }

  function onKeydown(e: KeyboardEvent) {
    if (!active || e.key !== 'Tab') return;
    const els = focusables();
    if (els.length === 0) {
      e.preventDefault();
      node.focus();
      return;
    }
    const first = els[0];
    const last = els[els.length - 1];
    const current = document.activeElement as HTMLElement;
    if (e.shiftKey && (current === first || !node.contains(current))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && current === last) {
      e.preventDefault();
      first.focus();
    }
  }

  node.addEventListener('keydown', onKeydown);
  requestAnimationFrame(focusFirst);

  return {
    update(next: FocusTrapOptions) {
      active = next.active ?? true;
    },
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      if (returnFocus) previouslyFocused?.focus?.();
    },
  };
}
