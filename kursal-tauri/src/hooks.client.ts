export async function init() {
  if (import.meta.env.DEV && import.meta.env.VITE_MOCK === '1') {
    const { installMock } = await import('#lib/mock/index.js');
    installMock();
  }
}
