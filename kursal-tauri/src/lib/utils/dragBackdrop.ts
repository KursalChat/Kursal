import { getCurrentWindow } from '@tauri-apps/api/window';
import { isMobile } from '#lib/api/window.js';
import { log } from '#lib/utils/log.js';

const DRAG_THRESHOLD = 6;

export function dragBackdrop(node: HTMLElement, onClick: () => void) {
  let startX = 0;
  let startY = 0;
  let dragged = false;

  function onMouseDown(e: MouseEvent) {
    startX = e.clientX;
    startY = e.clientY;
    dragged = false;
    if (!isMobile && e.button === 0 && e.detail === 1) {
      void getCurrentWindow()
        .startDragging()
        .catch((error) => log.error('Failed to start window dragging:', error));
    }
  }

  function onPointerMove(e: PointerEvent) {
    dragged = dragged || Math.hypot(e.clientX - startX, e.clientY - startY) >= DRAG_THRESHOLD;
  }

  function onClickCapture(e: MouseEvent) {
    if (dragged) {
      e.stopPropagation();
      dragged = false;
      return;
    }
    onClick();
  }

  node.addEventListener('mousedown', onMouseDown);
  node.addEventListener('pointermove', onPointerMove);
  node.addEventListener('click', onClickCapture, true);

  return {
    destroy() {
      node.removeEventListener('mousedown', onMouseDown);
      node.removeEventListener('pointermove', onPointerMove);
      node.removeEventListener('click', onClickCapture, true);
    },
  };
}
