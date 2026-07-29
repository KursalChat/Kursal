use crate::storage::image_metadata::{ImageFormat, detect_image_format, plan_strip, write_stripped};
use crate::tests::TestEnv;
use std::path::PathBuf;

const JPEG_APP1: u8 = 0xe1;
const JPEG_APP2_ICC: u8 = 0xe2;
const JPEG_COM: u8 = 0xfe;
const JPEG_SOS: u8 = 0xda;
const JPEG_EOI: u8 = 0xd9;

const WEBP_VP8X_EXIF_FLAG: u8 = 0x08;
const WEBP_VP8X_XMP_FLAG: u8 = 0x04;

fn write(env: &TestEnv, name: &str, bytes: &[u8]) -> PathBuf {
    let path = env.data_dir().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn stripped(env: &TestEnv, name: &str, bytes: &[u8]) -> Vec<u8> {
    let src = write(env, name, bytes);
    let plan = plan_strip(&src).unwrap();
    assert!(plan.changed());

    let dst = env.data_dir().join(format!("{name}.out"));
    write_stripped(&src, &dst, &plan).unwrap();

    let out = std::fs::read(&dst).unwrap();
    assert_eq!(out.len() as u64, plan.output_len());
    out
}

fn unchanged(env: &TestEnv, name: &str, bytes: &[u8]) {
    let src = write(env, name, bytes);
    assert!(!plan_strip(&src).unwrap().changed());
}

fn jpeg_segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let len = (payload.len() + 2) as u16;
    let mut out = vec![0xff, marker];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

fn webp_chunk(fourcc: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = fourcc.to_vec();
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    out
}

fn riff(body: &[u8]) -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(body);
    out
}

#[test]
fn detects_formats() {
    assert_eq!(
        detect_image_format(&[0xff, 0xd8, 0xff, 0xe0]),
        ImageFormat::Jpeg
    );
    assert_eq!(
        detect_image_format(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        ImageFormat::Png
    );
    assert_eq!(detect_image_format(b"RIFF\0\0\0\0WEBP"), ImageFormat::Webp);
    assert_eq!(detect_image_format(b"not an image"), ImageFormat::Unknown);
}

#[test]
fn jpeg_drops_exif_keeps_icc_and_scan() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(0xe0, b"JFIF\0test"));
    bytes.extend(jpeg_segment(JPEG_APP1, b"Exif\0\0secret gps"));
    bytes.extend(jpeg_segment(JPEG_APP2_ICC, b"ICC_PROFILE"));
    bytes.extend(jpeg_segment(0xed, b"Photoshop 3.0"));
    bytes.extend(jpeg_segment(JPEG_COM, b"a comment"));
    bytes.extend(jpeg_segment(0xdb, b"quant table"));
    bytes.extend_from_slice(&[0xff, JPEG_SOS, 0x00, 0x08, 1, 2, 3, 4, 5, 6]);
    bytes.extend_from_slice(&[0xff, JPEG_EOI]);

    let out = stripped(&env, "exif.jpg", &bytes);

    assert!(!out.windows(4).any(|w| w == b"Exif"));
    assert!(!out.windows(9).any(|w| w == b"Photoshop"));
    assert!(!out.windows(9).any(|w| w == b"a comment"));
    assert!(out.windows(11).any(|w| w == b"ICC_PROFILE"));
    assert!(out.windows(4).any(|w| w == b"JFIF"));
    assert!(out.windows(11).any(|w| w == b"quant table"));
    assert_eq!(&out[..2], &[0xff, 0xd8]);
    assert_eq!(&out[out.len() - 2..], &[0xff, JPEG_EOI]);
}

#[test]
fn jpeg_without_metadata_is_left_alone() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(0xdb, b"quant table"));
    bytes.extend_from_slice(&[0xff, JPEG_SOS, 0x00, 0x08, 1, 2, 3, 4, 5, 6]);
    bytes.extend_from_slice(&[0xff, JPEG_EOI]);

    unchanged(&env, "clean.jpg", &bytes);
}

#[test]
fn desynced_jpeg_is_left_alone() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(JPEG_APP1, b"Exif\0\0gps"));
    bytes.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);

    unchanged(&env, "desync.jpg", &bytes);
}

#[test]
fn png_drops_text_and_exif_keeps_pixels() {
    let env = TestEnv::new();

    let mut bytes = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend(png_chunk(b"IHDR", &[0; 13]));
    bytes.extend(png_chunk(b"gAMA", &[0, 1, 2, 3]));
    bytes.extend(png_chunk(b"tEXt", b"Author\0someone"));
    bytes.extend(png_chunk(b"eXIf", b"gps here"));
    bytes.extend(png_chunk(b"IDAT", b"pixels"));
    bytes.extend(png_chunk(b"IEND", b""));

    let out = stripped(&env, "meta.png", &bytes);

    assert!(!out.windows(6).any(|w| w == b"Author"));
    assert!(!out.windows(8).any(|w| w == b"gps here"));
    assert!(out.windows(4).any(|w| w == b"gAMA"));
    assert!(out.windows(6).any(|w| w == b"pixels"));
    assert!(out.windows(4).any(|w| w == b"IEND"));
}

#[test]
fn png_truncated_chunk_is_left_alone() {
    let env = TestEnv::new();

    let mut bytes = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend_from_slice(&999u32.to_be_bytes());
    bytes.extend_from_slice(b"tEXt");
    bytes.extend_from_slice(b"short");

    unchanged(&env, "truncated.png", &bytes);
}

#[test]
fn webp_drops_exif_and_clears_vp8x_flags() {
    let env = TestEnv::new();

    let mut vp8x_payload = vec![WEBP_VP8X_EXIF_FLAG | WEBP_VP8X_XMP_FLAG];
    vp8x_payload.extend_from_slice(&[0; 9]);

    let mut body = webp_chunk(b"VP8X", &vp8x_payload);
    body.extend(webp_chunk(b"VP8 ", b"pixels"));
    body.extend(webp_chunk(b"EXIF", b"gps here"));

    let out = stripped(&env, "meta.webp", &riff(&body));

    assert!(!out.windows(8).any(|w| w == b"gps here"));
    assert!(out.windows(6).any(|w| w == b"pixels"));
    assert_eq!(out[20], 0);

    let declared = u32::from_le_bytes(out[4..8].try_into().unwrap());
    assert_eq!(declared as usize, out.len() - 8);
}

#[test]
fn webp_without_metadata_is_left_alone() {
    let env = TestEnv::new();

    let body = webp_chunk(b"VP8 ", b"pixels");
    unchanged(&env, "clean.webp", &riff(&body));
}

#[test]
fn unknown_format_is_left_alone() {
    let env = TestEnv::new();
    unchanged(&env, "thing.bin", b"just some bytes here, not an image at all");
}

#[test]
fn tiny_file_is_left_alone() {
    let env = TestEnv::new();
    unchanged(&env, "tiny.bin", &[0xff, 0xd8, 0xff]);
}
