use crate::{KursalError, Result};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

const COPY_BUF: usize = 64 * 1024;
const HEADER_PROBE: usize = 12;

const JPEG_COM: u8 = 0xfe;
const JPEG_APP1: u8 = 0xe1;
const JPEG_APP2: u8 = 0xe2;
const JPEG_APP15: u8 = 0xef;
const JPEG_SOS: u8 = 0xda;
const JPEG_EOI: u8 = 0xd9;
const JPEG_ICC_TAG: &[u8; 12] = b"ICC_PROFILE\0";

const PNG_KEPT_CHUNKS: [&[u8; 4]; 20] = [
    b"IHDR", b"PLTE", b"IDAT", b"IEND", b"tRNS", b"cHRM", b"gAMA", b"iCCP", b"sBIT", b"sRGB",
    b"bKGD", b"hIST", b"pHYs", b"sPLT", b"cICP", b"mDCv", b"cLLi", b"acTL", b"fcTL", b"fdAT",
];

const WEBP_KEPT_CHUNKS: [&[u8; 4]; 7] = [
    b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF", b"ICCP",
];
const WEBP_VP8X_EXIF_FLAG: u8 = 0x08;
const WEBP_VP8X_XMP_FLAG: u8 = 0x04;
const WEBP_VP8X_MAX: u64 = 64;

const ISOBMFF_TABLE_MAX: u64 = 1 << 20;
const ISOBMFF_XMP_UUID: [u8; 16] = [
    0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac,
];

const STRIP_REQUIRED_EXT: [&str; 10] = [
    "jpg", "jpeg", "png", "webp", "heic", "heif", "avif", "tif", "tiff", "dng",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
    Tiff,
    Isobmff,
    Unknown,
}

pub fn detect_image_format(header: &[u8]) -> ImageFormat {
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        return ImageFormat::Jpeg;
    }
    if header.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return ImageFormat::Png;
    }
    if header.starts_with(&[0x49, 0x49, 0x2a, 0x00])
        || header.starts_with(&[0x4d, 0x4d, 0x00, 0x2a])
    {
        return ImageFormat::Tiff;
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return ImageFormat::Webp;
    }
    if header.len() >= 12 && &header[4..8] == b"ftyp" {
        return ImageFormat::Isobmff;
    }
    ImageFormat::Unknown
}

enum Piece {
    Copy { offset: u64, len: u64 },
    Zeros { len: u64 },
    Literal(Vec<u8>),
}

pub struct StripPlan {
    pieces: Vec<Piece>,
    changed: bool,
    src_len: u64,
}

impl StripPlan {
    pub(crate) fn identity() -> Self {
        Self {
            pieces: Vec::new(),
            changed: false,
            src_len: 0,
        }
    }

    fn rewrite(pieces: Vec<Piece>, changed: bool) -> Self {
        Self {
            pieces,
            changed,
            src_len: 0,
        }
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn output_len(&self) -> u64 {
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Copy { len, .. } | Piece::Zeros { len } => *len,
                Piece::Literal(bytes) => bytes.len() as u64,
            })
            .sum()
    }
}

struct Source {
    file: File,
    len: u64,
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(KursalError::Io)?;
        let len = file.metadata().map_err(KursalError::Io)?.len();
        Ok(Self { file, len })
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(KursalError::Io)?;
        self.file.read_exact(buf).map_err(KursalError::Io)
    }

    fn u16_be_at(&mut self, offset: u64) -> Result<u16> {
        let mut buf = [0u8; 2];
        self.read_at(offset, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn u32_be_at(&mut self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_at(offset, &mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn u32_le_at(&mut self, offset: u64) -> Result<u32> {
        let mut buf = [0u8; 4];
        self.read_at(offset, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn u64_be_at(&mut self, offset: u64) -> Result<u64> {
        let mut buf = [0u8; 8];
        self.read_at(offset, &mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }
}

fn jpeg_segment_dropped(src: &mut Source, marker: u8, offset: u64, seg_len: u64) -> Result<bool> {
    if marker == JPEG_COM {
        return Ok(true);
    }
    if !(JPEG_APP1..=JPEG_APP15).contains(&marker) {
        return Ok(false);
    }
    if marker != JPEG_APP2 {
        return Ok(true);
    }
    if seg_len < 2 + JPEG_ICC_TAG.len() as u64 {
        return Ok(true);
    }

    let mut tag = [0u8; JPEG_ICC_TAG.len()];
    src.read_at(offset + 4, &mut tag)?;
    Ok(&tag != JPEG_ICC_TAG)
}

fn jpeg_scan_end(src: &mut Source, start: u64) -> Result<u64> {
    let len = src.len;
    let mut buf = vec![0u8; COPY_BUF];
    let mut i = start;

    while i + 1 < len {
        let want = usize::try_from((len - i).min(buf.len() as u64))
            .map_err(|_| KursalError::UnstrippableImage)?;
        src.read_at(i, &mut buf[..want])?;

        let mut j = 0;
        while j + 1 < want {
            if buf[j] != 0xff {
                j += 1;
                continue;
            }
            let next = buf[j + 1];
            if next == 0xff {
                j += 1;
                continue;
            }
            if next != 0x00 && !(0xd0..=0xd7).contains(&next) {
                return Ok(i + j as u64);
            }
            j += 2;
        }
        i += j as u64;
    }

    Ok(len)
}

fn plan_jpeg(src: &mut Source) -> Result<StripPlan> {
    let len = src.len;
    let mut pieces = vec![Piece::Copy { offset: 0, len: 2 }];
    let mut changed = false;
    let mut i: u64 = 2;

    while i + 1 < len {
        let mut header = [0u8; 2];
        src.read_at(i, &mut header)?;
        if header[0] != 0xff {
            return Err(KursalError::UnstrippableImage);
        }

        let marker = header[1];
        if marker == 0xff {
            pieces.push(Piece::Copy { offset: i, len: 1 });
            i += 1;
            continue;
        }

        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            pieces.push(Piece::Copy { offset: i, len: 2 });
            i += 2;
            continue;
        }

        if marker == JPEG_EOI {
            pieces.push(Piece::Copy { offset: i, len: 2 });
            return Ok(StripPlan::rewrite(pieces, changed || i + 2 < len));
        }

        if i + 3 >= len {
            return Err(KursalError::UnstrippableImage);
        }
        let seg_len = u64::from(src.u16_be_at(i + 2)?);
        if seg_len < 2 {
            return Err(KursalError::UnstrippableImage);
        }
        let seg_end = i + 2 + seg_len;
        if seg_end > len {
            return Err(KursalError::UnstrippableImage);
        }

        if marker == JPEG_SOS {
            let scan_end = jpeg_scan_end(src, seg_end)?;
            pieces.push(Piece::Copy {
                offset: i,
                len: scan_end - i,
            });
            i = scan_end;
            continue;
        }

        if jpeg_segment_dropped(src, marker, i, seg_len)? {
            changed = true;
        } else {
            pieces.push(Piece::Copy {
                offset: i,
                len: seg_end - i,
            });
        }
        i = seg_end;
    }

    if i < len {
        pieces.push(Piece::Copy {
            offset: i,
            len: len - i,
        });
    }

    Ok(StripPlan::rewrite(pieces, changed))
}

fn plan_png(src: &mut Source) -> Result<StripPlan> {
    let len = src.len;
    let mut pieces = vec![Piece::Copy { offset: 0, len: 8 }];
    let mut changed = false;
    let mut i: u64 = 8;

    while i + 8 <= len {
        let data_len = u64::from(src.u32_be_at(i)?);
        let mut kind = [0u8; 4];
        src.read_at(i + 4, &mut kind)?;

        let chunk_end = i + 12 + data_len;
        if chunk_end > len {
            return Err(KursalError::UnstrippableImage);
        }

        if PNG_KEPT_CHUNKS.contains(&&kind) {
            pieces.push(Piece::Copy {
                offset: i,
                len: chunk_end - i,
            });
        } else {
            changed = true;
        }
        i = chunk_end;

        if &kind == b"IEND" {
            return Ok(StripPlan::rewrite(pieces, changed || i < len));
        }
    }

    Err(KursalError::UnstrippableImage)
}

fn plan_webp(src: &mut Source) -> Result<StripPlan> {
    let declared = u64::from(src.u32_le_at(4)?);
    let len = src.len.min(declared.saturating_add(8));
    if len < 12 {
        return Err(KursalError::UnstrippableImage);
    }

    let mut header = vec![0u8; 12];
    src.read_at(0, &mut header)?;

    let mut pieces = vec![Piece::Literal(header)];
    let mut changed = src.len > len;
    let mut i: u64 = 12;

    while i + 8 <= len {
        let mut fourcc = [0u8; 4];
        src.read_at(i, &mut fourcc)?;
        let size = u64::from(src.u32_le_at(i + 4)?);
        let body_end = i + 8 + size;
        if body_end > len {
            return Err(KursalError::UnstrippableImage);
        }
        let chunk_end = body_end + (size % 2);

        if !WEBP_KEPT_CHUNKS.contains(&&fourcc) {
            changed = true;
            i = chunk_end;
            continue;
        }

        if &fourcc == b"VP8X" && size > 0 && chunk_end - i <= WEBP_VP8X_MAX {
            let chunk_len = usize::try_from(chunk_end - i)
                .map_err(|_| KursalError::Storage("WebP chunk too large".to_string()))?;
            let mut chunk = vec![0u8; chunk_len];
            src.read_at(i, &mut chunk)?;
            let cleared = chunk[8] & !(WEBP_VP8X_EXIF_FLAG | WEBP_VP8X_XMP_FLAG);
            if cleared != chunk[8] {
                chunk[8] = cleared;
                changed = true;
            }
            pieces.push(Piece::Literal(chunk));
        } else {
            pieces.push(Piece::Copy {
                offset: i,
                len: body_end - i,
            });
            if chunk_end > body_end {
                pieces.push(Piece::Zeros { len: 1 });
            }
        }
        i = chunk_end;
    }

    if !changed && i >= len {
        return Ok(StripPlan::identity());
    }

    let mut plan = StripPlan::rewrite(pieces, true);
    let riff_size = u32::try_from(plan.output_len().saturating_sub(8))
        .map_err(|_| KursalError::Storage("WebP too large".to_string()))?;
    if let Some(Piece::Literal(header)) = plan.pieces.first_mut() {
        header[4..8].copy_from_slice(&riff_size.to_le_bytes());
    }

    Ok(plan)
}

struct BoxSpan {
    kind: [u8; 4],
    body: u64,
    end: u64,
}

fn read_box(src: &mut Source, offset: u64, limit: u64) -> Result<Option<BoxSpan>> {
    if offset.saturating_add(8) > limit {
        return Ok(None);
    }

    let declared = u64::from(src.u32_be_at(offset)?);
    let mut kind = [0u8; 4];
    src.read_at(offset + 4, &mut kind)?;

    let (body, end) = match declared {
        0 => (offset + 8, limit),
        1 => {
            if offset.saturating_add(16) > limit {
                return Ok(None);
            }
            match offset.checked_add(src.u64_be_at(offset + 8)?) {
                Some(end) => (offset + 16, end),
                None => return Ok(None),
            }
        }
        _ => match offset.checked_add(declared) {
            Some(end) => (offset + 8, end),
            None => return Ok(None),
        },
    };

    if body > end || end > limit {
        return Ok(None);
    }
    Ok(Some(BoxSpan { kind, body, end }))
}

fn find_box(src: &mut Source, start: u64, limit: u64, kind: &[u8; 4]) -> Result<Option<BoxSpan>> {
    let mut offset = start;
    while let Some(span) = read_box(src, offset, limit)? {
        if &span.kind == kind {
            return Ok(Some(span));
        }
        if span.end <= offset {
            return Ok(None);
        }
        offset = span.end;
    }
    Ok(None)
}

fn box_body(src: &mut Source, span: &BoxSpan) -> Result<Option<Vec<u8>>> {
    let size = span.end - span.body;
    if size > ISOBMFF_TABLE_MAX {
        return Ok(None);
    }

    let size = usize::try_from(size).map_err(|_| KursalError::UnstrippableImage)?;
    let mut buf = vec![0u8; size];
    src.read_at(span.body, &mut buf)?;
    Ok(Some(buf))
}

struct Scanner<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let out = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(out)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn u16(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn uint(&mut self, width: usize) -> Option<u64> {
        match width {
            0 => Some(0),
            4 => self.u32().map(u64::from),
            8 => {
                let b: [u8; 8] = self.take(8)?.try_into().ok()?;
                Some(u64::from_be_bytes(b))
            }
            _ => None,
        }
    }

    fn cstr(&mut self) -> Option<&'a [u8]> {
        let rest = self.data.get(self.pos..)?;
        let len = rest.iter().position(|b| *b == 0)?;
        self.pos += len + 1;
        Some(&rest[..len])
    }
}

fn is_xmp(content_type: &[u8]) -> bool {
    content_type.windows(7).any(|w| w == b"rdf+xml")
}

fn metadata_item_id(body: &[u8]) -> Option<u32> {
    let mut s = Scanner::new(body);
    let version = s.u8()?;
    s.take(3)?;

    if version < 2 {
        let id = u32::from(s.u16()?);
        s.take(2)?;
        s.cstr()?;
        return is_xmp(s.cstr()?).then_some(id);
    }

    let id = if version == 2 {
        u32::from(s.u16()?)
    } else {
        s.u32()?
    };
    s.take(2)?;
    let item_type = s.take(4)?;

    if item_type == b"Exif" {
        return Some(id);
    }
    if item_type == b"mime" {
        s.cstr()?;
        return is_xmp(s.cstr()?).then_some(id);
    }
    None
}

fn metadata_item_ids(data: &[u8]) -> Option<Vec<u32>> {
    let mut s = Scanner::new(data);
    let version = s.u8()?;
    s.take(3)?;
    let count = if version == 0 {
        u32::from(s.u16()?)
    } else {
        s.u32()?
    };

    let mut ids = Vec::new();
    for _ in 0..count {
        let size = usize::try_from(s.u32()?).ok()?;
        let kind = s.take(4)?;
        let body = s.take(size.checked_sub(8)?)?;
        if kind != b"infe" {
            continue;
        }
        if let Some(id) = metadata_item_id(body) {
            ids.push(id);
        }
    }
    Some(ids)
}

fn item_extents(data: &[u8], wanted: &[u32], idat: Option<u64>) -> Option<Vec<(u64, u64)>> {
    let mut s = Scanner::new(data);
    let version = s.u8()?;
    s.take(3)?;

    let widths = s.u8()?;
    let offset_size = usize::from(widths >> 4);
    let length_size = usize::from(widths & 0x0f);
    let widths = s.u8()?;
    let base_offset_size = usize::from(widths >> 4);
    let index_size = if version == 1 || version == 2 {
        usize::from(widths & 0x0f)
    } else {
        0
    };

    let count = if version < 2 {
        u32::from(s.u16()?)
    } else {
        s.u32()?
    };

    let mut out = Vec::new();
    for _ in 0..count {
        let id = if version < 2 {
            u32::from(s.u16()?)
        } else {
            s.u32()?
        };
        let construction = if version == 1 || version == 2 {
            s.u16()? & 0x0f
        } else {
            0
        };
        let data_reference = s.u16()?;
        let base = s.uint(base_offset_size)?;
        let extents = s.u16()?;
        let strip = wanted.contains(&id);
        if strip && data_reference != 0 {
            return None;
        }

        for _ in 0..extents {
            s.uint(index_size)?;
            let offset = s.uint(offset_size)?;
            let length = s.uint(length_size)?;
            if !strip || length == 0 {
                continue;
            }
            let start = match construction {
                0 => base.checked_add(offset)?,
                1 => idat?.checked_add(base)?.checked_add(offset)?,
                _ => return None,
            };
            out.push((start, length));
        }
    }
    Some(out)
}

fn meta_children(src: &mut Source, meta: &BoxSpan) -> Result<u64> {
    let full = meta.body + 4;
    if full <= meta.end && find_box(src, full, meta.end, b"iinf")?.is_some() {
        return Ok(full);
    }
    Ok(meta.body)
}

fn item_metadata_extents(src: &mut Source, meta: &BoxSpan) -> Result<Vec<(u64, u64)>> {
    let children = meta_children(src, meta)?;
    let Some(iinf) = find_box(src, children, meta.end, b"iinf")? else {
        return Ok(Vec::new());
    };

    let wanted = box_body(src, &iinf)?
        .and_then(|body| metadata_item_ids(&body))
        .ok_or(KursalError::UnstrippableImage)?;
    if wanted.is_empty() {
        return Ok(Vec::new());
    }

    let idat = find_box(src, children, meta.end, b"idat")?.map(|span| span.body);
    let iloc = find_box(src, children, meta.end, b"iloc")?.ok_or(KursalError::UnstrippableImage)?;
    box_body(src, &iloc)?
        .and_then(|body| item_extents(&body, &wanted, idat))
        .ok_or(KursalError::UnstrippableImage)
}

fn xmp_uuid_extents(src: &mut Source, len: u64) -> Result<Vec<(u64, u64)>> {
    let mut out = Vec::new();
    let mut offset = 0u64;

    while let Some(span) = read_box(src, offset, len)? {
        if &span.kind == b"uuid" && span.end - span.body > 16 {
            let mut uuid = [0u8; 16];
            src.read_at(span.body, &mut uuid)?;
            if uuid == ISOBMFF_XMP_UUID {
                out.push((span.body + 16, span.end - span.body - 16));
            }
        }
        if span.end <= offset {
            break;
        }
        offset = span.end;
    }

    Ok(out)
}

fn plan_isobmff(src: &mut Source, required: bool) -> Result<StripPlan> {
    let len = src.len;
    let mut extents = xmp_uuid_extents(src, len)?;

    match find_box(src, 0, len, b"meta")? {
        Some(meta) => extents.extend(item_metadata_extents(src, &meta)?),
        None if required => return Err(KursalError::UnstrippableImage),
        None => {}
    }

    if extents.is_empty() {
        return Ok(StripPlan::identity());
    }
    extents.sort_unstable();

    let mut pieces = Vec::new();
    let mut cursor = 0u64;

    for (start, length) in extents {
        let end = start
            .checked_add(length)
            .ok_or(KursalError::UnstrippableImage)?;
        if start < cursor || end > len {
            return Err(KursalError::UnstrippableImage);
        }

        if start > cursor {
            pieces.push(Piece::Copy {
                offset: cursor,
                len: start - cursor,
            });
        }
        pieces.push(Piece::Zeros { len: length });
        cursor = end;
    }

    if cursor < len {
        pieces.push(Piece::Copy {
            offset: cursor,
            len: len - cursor,
        });
    }

    Ok(StripPlan::rewrite(pieces, true))
}

fn strip_required(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ext| STRIP_REQUIRED_EXT.contains(&ext.as_str()))
}

pub fn plan_strip(path: &Path) -> Result<StripPlan> {
    let mut src = Source::open(path)?;
    let required = strip_required(path);
    if src.len < HEADER_PROBE as u64 {
        if required {
            return Err(KursalError::UnstrippableImage);
        }
        return Ok(StripPlan::identity());
    }

    let mut header = [0u8; HEADER_PROBE];
    src.read_at(0, &mut header)?;

    let mut plan = match detect_image_format(&header) {
        ImageFormat::Jpeg => plan_jpeg(&mut src)?,
        ImageFormat::Png => plan_png(&mut src)?,
        ImageFormat::Webp => plan_webp(&mut src)?,
        ImageFormat::Isobmff => plan_isobmff(&mut src, required)?,
        ImageFormat::Tiff => return Err(KursalError::UnstrippableImage),
        ImageFormat::Unknown if required => return Err(KursalError::UnstrippableImage),
        ImageFormat::Unknown => StripPlan::identity(),
    };

    plan.src_len = src.len;
    Ok(plan)
}

fn copy_pieces(src: &Path, dst: &Path, plan: &StripPlan) -> Result<()> {
    let mut input = File::open(src).map_err(KursalError::Io)?;
    if input.metadata().map_err(KursalError::Io)?.len() != plan.src_len {
        return Err(KursalError::UnstrippableImage);
    }

    let mut out = BufWriter::new(File::create(dst).map_err(KursalError::Io)?);
    let mut buf = vec![0u8; COPY_BUF];

    for piece in &plan.pieces {
        match piece {
            Piece::Literal(bytes) => out.write_all(bytes).map_err(KursalError::Io)?,
            Piece::Zeros { len } => {
                let mut left = *len;
                while left > 0 {
                    let want = usize::try_from(left.min(buf.len() as u64))
                        .map_err(|_| KursalError::Storage("Copy overflow".to_string()))?;
                    buf[..want].fill(0);
                    out.write_all(&buf[..want]).map_err(KursalError::Io)?;
                    left -= want as u64;
                }
            }
            Piece::Copy { offset, len } => {
                input
                    .seek(SeekFrom::Start(*offset))
                    .map_err(KursalError::Io)?;
                let mut left = *len;
                while left > 0 {
                    let want = usize::try_from(left.min(buf.len() as u64))
                        .map_err(|_| KursalError::Storage("Copy overflow".to_string()))?;
                    input
                        .read_exact(&mut buf[..want])
                        .map_err(KursalError::Io)?;
                    out.write_all(&buf[..want]).map_err(KursalError::Io)?;
                    left -= want as u64;
                }
            }
        }
    }

    out.flush().map_err(KursalError::Io)?;
    Ok(())
}

pub fn write_stripped(src: &Path, dst: &Path, plan: &StripPlan) -> Result<()> {
    if !plan.changed {
        return Err(KursalError::Storage(
            "strip plan makes no changes".to_string(),
        ));
    }

    let result = copy_pieces(src, dst, plan);
    if result.is_err() {
        let _ = std::fs::remove_file(dst);
    }
    result
}
