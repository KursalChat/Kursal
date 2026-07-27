import { describe, expect, it } from 'vitest';
import { detectImageFormat, looksLikeStrippableImage, stripImageMetadata } from './image-metadata';

function bytes(...values: number[]): Uint8Array {
  return new Uint8Array(values);
}

function textBytes(s: string): number[] {
  return [...s].map((c) => c.charCodeAt(0));
}

/** FF <marker> <len hi> <len lo> <payload> - len covers itself plus payload. */
function jpegSegment(marker: number, payload: number[]): number[] {
  const len = payload.length + 2;
  return [0xff, marker, (len >> 8) & 0xff, len & 0xff, ...payload];
}

function pngChunk(type: string, data: number[]): number[] {
  const len = data.length;
  return [
    (len >>> 24) & 0xff,
    (len >>> 16) & 0xff,
    (len >>> 8) & 0xff,
    len & 0xff,
    ...textBytes(type),
    ...data,
    0,
    0,
    0,
    0, // CRC placeholder - the stripper copies it without validating
  ];
}

function riffChunk(fourcc: string, data: number[]): number[] {
  const len = data.length;
  const padded = len % 2 === 1 ? [...data, 0] : data;
  return [
    ...textBytes(fourcc),
    len & 0xff,
    (len >>> 8) & 0xff,
    (len >>> 16) & 0xff,
    (len >>> 24) & 0xff,
    ...padded,
  ];
}

function contains(haystack: Uint8Array, needle: string): boolean {
  const target = textBytes(needle);
  outer: for (let i = 0; i + target.length <= haystack.length; i++) {
    for (let j = 0; j < target.length; j++) {
      if (haystack[i + j] !== target[j]) continue outer;
    }
    return true;
  }
  return false;
}

describe('detectImageFormat', () => {
  it('identifies supported formats by magic bytes', () => {
    expect(detectImageFormat(bytes(0xff, 0xd8, 0xff, 0xe0))).toBe('jpeg');
    expect(detectImageFormat(bytes(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a))).toBe('png');
    expect(detectImageFormat(bytes(...textBytes('RIFF'), 0, 0, 0, 0, ...textBytes('WEBP')))).toBe(
      'webp'
    );
    expect(detectImageFormat(bytes(0x00, 0x01, 0x02))).toBe('unknown');
  });
});

describe('stripImageMetadata - JPEG', () => {
  const jpeg = new Uint8Array([
    0xff,
    0xd8, // SOI
    ...jpegSegment(0xe0, textBytes('JFIF\0KEEPDENSITY')), // APP0 - keep
    ...jpegSegment(0xe1, textBytes('Exif\0\0GPSLATITUDE_SECRET')), // APP1 - drop
    ...jpegSegment(0xe2, textBytes('ICC_PROFILE\0KEEPCOLOR')), // APP2 - keep
    ...jpegSegment(0xed, textBytes('Photoshop3.0IPTC_AUTHOR')), // APP13 - drop
    ...jpegSegment(0xfe, textBytes('COMMENT_SECRET')), // COM - drop
    ...jpegSegment(0xdb, [0x00, 0x11, 0x22]), // DQT - keep
    ...jpegSegment(0xda, [0x01, 0x02]), // SOS header
    0x99,
    0x88,
    0x77, // entropy-coded data
    0xff,
    0xd9, // EOI
  ]);

  it('removes EXIF, IPTC and comments', () => {
    const out = stripImageMetadata(jpeg);
    expect(contains(out, 'GPSLATITUDE_SECRET')).toBe(false);
    expect(contains(out, 'IPTC_AUTHOR')).toBe(false);
    expect(contains(out, 'COMMENT_SECRET')).toBe(false);
  });

  it('keeps rendering-critical segments and pixel data', () => {
    const out = stripImageMetadata(jpeg);
    expect(contains(out, 'KEEPDENSITY')).toBe(true);
    expect(contains(out, 'KEEPCOLOR')).toBe(true);
    expect(out[0]).toBe(0xff);
    expect(out[1]).toBe(0xd8);
    expect(Array.from(out.subarray(out.length - 5))).toEqual([0x99, 0x88, 0x77, 0xff, 0xd9]);
  });

  it('returns the input unchanged when the stream is malformed', () => {
    const malformed = bytes(0xff, 0xd8, 0xff, 0xe1, 0xff, 0xff, 0x01);
    expect(stripImageMetadata(malformed)).toBe(malformed);
  });
});

describe('stripImageMetadata - PNG', () => {
  const png = new Uint8Array([
    0x89,
    0x50,
    0x4e,
    0x47,
    0x0d,
    0x0a,
    0x1a,
    0x0a,
    ...pngChunk('IHDR', [0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]),
    ...pngChunk('tEXt', textBytes('Author\0SECRET_NAME')),
    ...pngChunk('eXIf', textBytes('SECRET_EXIF')),
    ...pngChunk('sRGB', [0]),
    ...pngChunk('IDAT', textBytes('PIXELDATA')),
    ...pngChunk('IEND', []),
  ]);

  it('drops text and EXIF chunks', () => {
    const out = stripImageMetadata(png);
    expect(contains(out, 'SECRET_NAME')).toBe(false);
    expect(contains(out, 'SECRET_EXIF')).toBe(false);
    expect(contains(out, 'tEXt')).toBe(false);
  });

  it('keeps the signature, colour chunks and pixel data', () => {
    const out = stripImageMetadata(png);
    expect(Array.from(out.subarray(0, 8))).toEqual([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ]);
    expect(contains(out, 'sRGB')).toBe(true);
    expect(contains(out, 'PIXELDATA')).toBe(true);
    expect(contains(out, 'IEND')).toBe(true);
  });
});

describe('stripImageMetadata - WebP', () => {
  const body = [
    ...riffChunk('VP8X', [0x0c, 0, 0, 0, 0, 0, 0, 0, 0, 0]), // flags: EXIF|XMP set
    ...riffChunk('VP8 ', textBytes('PIXELDATA')),
    ...riffChunk('EXIF', textBytes('SECRET_EXIF')),
    ...riffChunk('XMP ', textBytes('SECRET_XMP')),
  ];
  const webp = new Uint8Array([
    ...textBytes('RIFF'),
    (body.length + 4) & 0xff,
    ((body.length + 4) >>> 8) & 0xff,
    ((body.length + 4) >>> 16) & 0xff,
    ((body.length + 4) >>> 24) & 0xff,
    ...textBytes('WEBP'),
    ...body,
  ]);

  it('drops EXIF and XMP chunks but keeps pixel data', () => {
    const out = stripImageMetadata(webp);
    expect(contains(out, 'SECRET_EXIF')).toBe(false);
    expect(contains(out, 'SECRET_XMP')).toBe(false);
    expect(contains(out, 'PIXELDATA')).toBe(true);
  });

  it('clears the EXIF/XMP flags in the VP8X header', () => {
    const out = stripImageMetadata(webp);
    // 12-byte RIFF header, then 'VP8X' + 4-byte size, then the flags byte.
    expect(out[20] & 0x0c).toBe(0);
  });

  it('rewrites the RIFF size to match the new length', () => {
    const out = stripImageMetadata(webp);
    const size = new DataView(out.buffer).getUint32(4, true);
    expect(size).toBe(out.length - 8);
  });
});

describe('stripImageMetadata - passthrough', () => {
  it('returns unknown formats untouched', () => {
    const pdf = bytes(0x25, 0x50, 0x44, 0x46, 0x2d);
    expect(stripImageMetadata(pdf)).toBe(pdf);
  });
});

describe('looksLikeStrippableImage', () => {
  it('matches supported image extensions case-insensitively', () => {
    expect(looksLikeStrippableImage('photo.JPG')).toBe(true);
    expect(looksLikeStrippableImage('shot.png')).toBe(true);
    expect(looksLikeStrippableImage('anim.webp')).toBe(true);
    expect(looksLikeStrippableImage('notes.pdf')).toBe(false);
    expect(looksLikeStrippableImage('photo.heic')).toBe(false);
  });
});
