use crate::{KursalError, Result};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

const COPY_BUF: usize = 64 * 1024;
const HEADER_PROBE: usize = 12;

const JPEG_COM: u8 = 0xfe;
const JPEG_APP1: u8 = 0xe1;
const JPEG_APP2_ICC: u8 = 0xe2;
const JPEG_APP15: u8 = 0xef;
const JPEG_SOS: u8 = 0xda;
const JPEG_EOI: u8 = 0xd9;

const PNG_DROPPED_CHUNKS: [&[u8; 4]; 5] = [b"tEXt", b"zTXt", b"iTXt", b"eXIf", b"tIME"];

const WEBP_VP8X_EXIF_FLAG: u8 = 0x08;
const WEBP_VP8X_XMP_FLAG: u8 = 0x04;
const WEBP_VP8X_MAX: u64 = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
    Unknown,
}

pub fn detect_image_format(header: &[u8]) -> ImageFormat {
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        return ImageFormat::Jpeg;
    }
    if header.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return ImageFormat::Png;
    }
    if header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        return ImageFormat::Webp;
    }
    ImageFormat::Unknown
}

enum Piece {
    Copy { offset: u64, len: u64 },
    Literal(Vec<u8>),
}

pub struct StripPlan {
    pieces: Vec<Piece>,
    changed: bool,
}

impl StripPlan {
    fn identity() -> Self {
        Self {
            pieces: Vec::new(),
            changed: false,
        }
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn output_len(&self) -> u64 {
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Copy { len, .. } => *len,
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
}

fn is_droppable_jpeg_segment(marker: u8) -> bool {
    marker == JPEG_COM || ((JPEG_APP1..=JPEG_APP15).contains(&marker) && marker != JPEG_APP2_ICC)
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
            return Ok(StripPlan::identity());
        }
        let marker = header[1];

        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) || marker == JPEG_EOI {
            pieces.push(Piece::Copy { offset: i, len: 2 });
            i += 2;
            continue;
        }

        if marker == JPEG_SOS {
            pieces.push(Piece::Copy {
                offset: i,
                len: len - i,
            });
            break;
        }

        if i + 3 >= len {
            return Ok(StripPlan::identity());
        }
        let seg_len = u64::from(src.u16_be_at(i + 2)?);
        if seg_len < 2 {
            return Ok(StripPlan::identity());
        }
        let seg_end = i + 2 + seg_len;
        if seg_end > len {
            return Ok(StripPlan::identity());
        }

        if is_droppable_jpeg_segment(marker) {
            changed = true;
        } else {
            pieces.push(Piece::Copy {
                offset: i,
                len: seg_end - i,
            });
        }
        i = seg_end;
    }

    Ok(StripPlan { pieces, changed })
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
            return Ok(StripPlan::identity());
        }

        if PNG_DROPPED_CHUNKS.contains(&&kind) {
            changed = true;
        } else {
            pieces.push(Piece::Copy {
                offset: i,
                len: chunk_end - i,
            });
        }
        i = chunk_end;

        if &kind == b"IEND" {
            break;
        }
    }

    Ok(StripPlan { pieces, changed })
}

fn plan_webp(src: &mut Source) -> Result<StripPlan> {
    let len = src.len;
    let mut header = vec![0u8; 12];
    src.read_at(0, &mut header)?;

    let mut pieces = vec![Piece::Literal(header)];
    let mut changed = false;
    let mut i: u64 = 12;

    while i + 8 <= len {
        let mut fourcc = [0u8; 4];
        src.read_at(i, &mut fourcc)?;
        let size = u64::from(src.u32_le_at(i + 4)?);
        let chunk_end = i + 8 + size + (size % 2);
        if chunk_end > len {
            return Ok(StripPlan::identity());
        }

        if &fourcc == b"EXIF" || &fourcc == b"XMP " {
            changed = true;
            i = chunk_end;
            continue;
        }

        if &fourcc == b"VP8X" && chunk_end - i > 8 && chunk_end - i <= WEBP_VP8X_MAX {
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
                len: chunk_end - i,
            });
        }
        i = chunk_end;
    }

    if !changed {
        return Ok(StripPlan::identity());
    }

    let mut plan = StripPlan { pieces, changed };
    let riff_size = u32::try_from(plan.output_len().saturating_sub(8))
        .map_err(|_| KursalError::Storage("WebP too large".to_string()))?;
    if let Some(Piece::Literal(header)) = plan.pieces.first_mut() {
        header[4..8].copy_from_slice(&riff_size.to_le_bytes());
    }

    Ok(plan)
}

pub fn plan_strip(path: &Path) -> Result<StripPlan> {
    let mut src = Source::open(path)?;
    if src.len < HEADER_PROBE as u64 {
        return Ok(StripPlan::identity());
    }

    let mut header = [0u8; HEADER_PROBE];
    src.read_at(0, &mut header)?;

    match detect_image_format(&header) {
        ImageFormat::Jpeg => plan_jpeg(&mut src),
        ImageFormat::Png => plan_png(&mut src),
        ImageFormat::Webp => plan_webp(&mut src),
        ImageFormat::Unknown => Ok(StripPlan::identity()),
    }
}

pub fn write_stripped(src: &Path, dst: &Path, plan: &StripPlan) -> Result<()> {
    let mut input = File::open(src).map_err(KursalError::Io)?;
    let mut out = BufWriter::new(File::create(dst).map_err(KursalError::Io)?);
    let mut buf = vec![0u8; COPY_BUF];

    for piece in &plan.pieces {
        match piece {
            Piece::Literal(bytes) => out.write_all(bytes).map_err(KursalError::Io)?,
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
