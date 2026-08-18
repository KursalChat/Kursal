export function withAvatarCacheBust(url: string | null | undefined): string | null {
  if (!url || url.startsWith('data:')) return url ?? null;

  const queryIndex = url.indexOf('?');
  const base = queryIndex === -1 ? url : url.slice(0, queryIndex);
  const rawQuery = queryIndex === -1 ? '' : url.slice(queryIndex + 1);
  const params = new URLSearchParams(rawQuery);
  params.set('v', String(Date.now()));

  return `${base}?${params.toString()}`;
}
