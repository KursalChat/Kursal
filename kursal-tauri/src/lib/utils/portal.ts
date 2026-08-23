// Svelte action: moves `node` to <body> so it escapes any transformed or
// overflow-clipped ancestor.
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
