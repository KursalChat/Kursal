// Strips privacy-sensitive metadata (EXIF/GPS, XMP, IPTC, text chunks) from
// images before they leave the device. Works at the container level so the
// compressed pixel data is copied through untouched - no re-encode, no quality
// loss. Unknown or unsupported formats are returned unchanged.
//
// Colour-critical payloads (JFIF density, ICC profiles, PNG gAMA/sRGB/iCCP) are
// deliberately kept so images still render the way the sender saw them.

const JPEG_APP2_ICC = 0xe2;
const JPEG_SOS = 0xda;
const JPEG_EOI = 0xd9;

// PNG ancillary chunks that carry authored text, timestamps or embedded EXIF.
const PNG_DROPPED_CHUNKS = new Set(['tEXt', 'zTXt', 'iTXt', 'eXIf', 'tIME']);

const WEBP_VP8X_EXIF_FLAG = 0x08;
const WEBP_VP8X_XMP_FLAG = 0x04;

function startsWith(bytes: Uint8Array, sig: number[], offset = 0): boolean {
  if (bytes.length < offset + sig.length) return false;
  return sig.every((b, i) => bytes[offset + i] === b);
}

function ascii(bytes: Uint8Array, offset: number, length: number): string {
  return String.fromCharCode(...bytes.subarray(offset, offset + length));
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

export type ImageFormat = 'jpeg' | 'png' | 'webp' | 'unknown';

export function detectImageFormat(bytes: Uint8Array): ImageFormat {
  if (startsWith(bytes, [0xff, 0xd8, 0xff])) return 'jpeg';
  if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return 'png';
  if (startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) && startsWith(bytes, [0x57, 0x45, 0x42, 0x50], 8))
    return 'webp';
  return 'unknown';
}

// APP1 holds EXIF and XMP, APP13 holds Photoshop/IPTC; COM holds free text.
// APP0 (JFIF) and APP2 (ICC) are kept because they affect rendering.
function isDroppableJpegSegment(marker: number): boolean {
  if (marker === 0xfe) return true; // COM
  return marker >= 0xe1 && marker <= 0xef && marker !== JPEG_APP2_ICC;
}

function stripJpeg(bytes: Uint8Array): Uint8Array {
  const parts: Uint8Array[] = [bytes.subarray(0, 2)];
  let i = 2;

  while (i + 1 < bytes.length) {
    if (bytes[i] !== 0xff) return bytes; // desynced - leave the file alone
    const marker = bytes[i + 1];

    // Standalone markers carry no payload.
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) {
      parts.push(bytes.subarray(i, i + 2));
      i += 2;
      continue;
    }

    if (marker === JPEG_EOI) {
      parts.push(bytes.subarray(i, i + 2));
      i += 2;
      continue;
    }

    // Entropy-coded data follows the scan header; copy the remainder verbatim.
    if (marker === JPEG_SOS) {
      parts.push(bytes.subarray(i));
      i = bytes.length;
      break;
    }

    if (i + 3 >= bytes.length) return bytes;
    const segLen = (bytes[i + 2] << 8) | bytes[i + 3];
    if (segLen < 2) return bytes;
    const segEnd = i + 2 + segLen;
    if (segEnd > bytes.length) return bytes;

    if (!isDroppableJpegSegment(marker)) parts.push(bytes.subarray(i, segEnd));
    i = segEnd;
  }

  return concat(parts);
}

function stripPng(bytes: Uint8Array): Uint8Array {
  const parts: Uint8Array[] = [bytes.subarray(0, 8)];
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let i = 8;

  while (i + 8 <= bytes.length) {
    const dataLen = view.getUint32(i);
    const type = ascii(bytes, i + 4, 4);
    const chunkEnd = i + 12 + dataLen; // length + type + data + crc
    if (chunkEnd > bytes.length) return bytes;

    if (!PNG_DROPPED_CHUNKS.has(type)) parts.push(bytes.subarray(i, chunkEnd));
    i = chunkEnd;

    if (type === 'IEND') break;
  }

  return concat(parts);
}

function stripWebp(bytes: Uint8Array): Uint8Array {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const chunks: Uint8Array[] = [];
  let i = 12; // past 'RIFF' + size + 'WEBP'

  while (i + 8 <= bytes.length) {
    const fourcc = ascii(bytes, i, 4);
    const size = view.getUint32(i + 4, true);
    const padded = size + (size % 2); // chunks are padded to an even length
    const chunkEnd = i + 8 + padded;
    if (chunkEnd > bytes.length) return bytes;

    if (fourcc === 'EXIF' || fourcc === 'XMP ') {
      i = chunkEnd;
      continue;
    }

    const chunk = bytes.slice(i, chunkEnd);
    // The VP8X header advertises which optional chunks exist; clear the bits
    // for the ones just removed so decoders don't look for them.
    if (fourcc === 'VP8X' && chunk.length > 8) {
      chunk[8] &= ~(WEBP_VP8X_EXIF_FLAG | WEBP_VP8X_XMP_FLAG);
    }
    chunks.push(chunk);
    i = chunkEnd;
  }

  const body = concat(chunks);
  const out = new Uint8Array(12 + body.length);
  out.set(bytes.subarray(0, 12));
  out.set(body, 12);
  // RIFF size counts everything after the 8-byte 'RIFF'+size header.
  new DataView(out.buffer).setUint32(4, out.length - 8, true);
  return out;
}

/**
 * Returns a copy with metadata removed, or the original bytes when the format
 * is unsupported or the file looks malformed. Never throws - a failure to strip
 * must not block sending, but callers that care should check
 * `detectImageFormat` to know whether a strip was possible.
 */
export function stripImageMetadata(bytes: Uint8Array): Uint8Array {
  try {
    switch (detectImageFormat(bytes)) {
      case 'jpeg':
        return stripJpeg(bytes);
      case 'png':
        return stripPng(bytes);
      case 'webp':
        return stripWebp(bytes);
      default:
        return bytes;
    }
  } catch {
    return bytes;
  }
}

const IMAGE_EXTENSIONS = new Set(['jpg', 'jpeg', 'png', 'webp']);

export function looksLikeStrippableImage(filename: string): boolean {
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  return IMAGE_EXTENSIONS.has(ext);
}
