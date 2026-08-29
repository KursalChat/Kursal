interface DismissOptions {
  onDismiss: () => void;
  anchors?: (HTMLElement | null | undefined)[];
  reflow?: boolean;
}

// Closes a popover on an outside pointer press or Escape. `anchors` are the
// elements whose own clicks must not count as outside (the trigger button, for
// one). `reflow` also closes on scroll/resize, for popovers positioned against a
// rect that would otherwise drift.
export function dismissable(node: HTMLElement, options: DismissOptions) {
  let current = options;

  function isInside(target: Node): boolean {
    if (node.contains(target)) return true;
    return (current.anchors ?? []).some((el) => el?.contains(target));
  }

  function onPointer(e: PointerEvent) {
    if (isInside(e.target as Node)) return;
    current.onDismiss();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') current.onDismiss();
  }

  function onReflow() {
    current.onDismiss();
  }

  window.addEventListener('pointerdown', onPointer, true);
  window.addEventListener('keydown', onKey);
  if (current.reflow) {
    window.addEventListener('scroll', onReflow, true);
    window.addEventListener('resize', onReflow);
  }

  return {
    update(next: DismissOptions) {
      current = next;
    },
    destroy() {
      window.removeEventListener('pointerdown', onPointer, true);
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', onReflow, true);
      window.removeEventListener('resize', onReflow);
    },
  };
}
