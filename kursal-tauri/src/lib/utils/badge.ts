// Windows has no native dock badge, so the unread count is drawn to a 16x16
// PNG and set as the taskbar overlay icon.
export async function renderBadgeIcon(label: number): Promise<Uint8Array | null> {
  const canvas = document.createElement('canvas');
  canvas.width = 16;
  canvas.height = 16;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  ctx.fillStyle = '#e11d48';
  ctx.beginPath();
  ctx.arc(8, 8, 8, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = 'white';
  ctx.font = 'bold 10px Arial';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(label > 99 ? '99+' : String(label), 8, 8);

  const blob = await new Promise<Blob | null>((res) => canvas.toBlob(res, 'image/png'));
  if (!blob) return null;
  return new Uint8Array(await blob.arrayBuffer());
}
