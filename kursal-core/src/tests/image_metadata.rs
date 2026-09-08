use crate::storage::image_metadata::{
    ImageFormat, detect_image_format, plan_strip, write_stripped,
};
use crate::tests::TestEnv;
use std::path::PathBuf;

const JPEG_APP1: u8 = 0xe1;
const JPEG_APP2: u8 = 0xe2;
const JPEG_COM: u8 = 0xfe;
const JPEG_SOS: u8 = 0xda;
const JPEG_EOI: u8 = 0xd9;

const WEBP_VP8X_EXIF_FLAG: u8 = 0x08;
const WEBP_VP8X_XMP_FLAG: u8 = 0x04;

const XMP_UUID: [u8; 16] = [
    0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac,
];

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

fn refused(env: &TestEnv, name: &str, bytes: &[u8]) {
    let src = write(env, name, bytes);
    assert!(plan_strip(&src).is_err());
}

fn jpeg_segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let len = u16::try_from(payload.len() + 2).unwrap();
    let mut out = vec![0xff, marker];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

fn webp_chunk(fourcc: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = fourcc.to_vec();
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    out
}

fn bmff_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(body.len() + 8)
        .unwrap()
        .to_be_bytes()
        .to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn infe(id: u16, item_type: &[u8; 4], name: &[u8]) -> Vec<u8> {
    let mut body = vec![2, 0, 0, 0];
    body.extend_from_slice(&id.to_be_bytes());
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(item_type);
    body.extend_from_slice(name);
    body.push(0);
    bmff_box(b"infe", &body)
}

fn iinf(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0];
    body.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_be_bytes());
    for entry in entries {
        body.extend_from_slice(entry);
    }
    bmff_box(b"iinf", &body)
}

fn iloc(items: &[(u16, u32, u32)]) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0, 0x44, 0x00];
    body.extend_from_slice(&u16::try_from(items.len()).unwrap().to_be_bytes());
    for (id, offset, length) in items {
        body.extend_from_slice(&id.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&offset.to_be_bytes());
        body.extend_from_slice(&length.to_be_bytes());
    }
    bmff_box(b"iloc", &body)
}

fn heic(exif: Option<&[u8]>, pixels: &[u8]) -> Vec<u8> {
    let mut ftyp_body = b"heic".to_vec();
    ftyp_body.extend_from_slice(&0u32.to_be_bytes());
    ftyp_body.extend_from_slice(b"mif1");
    let ftyp = bmff_box(b"ftyp", &ftyp_body);

    let exif_len = u32::try_from(exif.map_or(0, <[u8]>::len)).unwrap();
    let build_meta = |exif_at: u32| {
        let mut items = vec![infe(2, b"hvc1", b"image")];
        let mut located = vec![(2, exif_at + exif_len, u32::try_from(pixels.len()).unwrap())];
        if exif.is_some() {
            items.insert(0, infe(1, b"Exif", b"exif"));
            located.insert(0, (1, exif_at, exif_len));
        }

        let mut body = vec![0, 0, 0, 0];
        body.extend(iinf(&items));
        body.extend(iloc(&located));
        bmff_box(b"meta", &body)
    };

    let exif_at = u32::try_from(ftyp.len() + build_meta(0).len() + 8).unwrap();

    let mut mdat = exif.unwrap_or(b"").to_vec();
    mdat.extend_from_slice(pixels);

    let mut out = ftyp;
    out.extend(build_meta(exif_at));
    out.extend(bmff_box(b"mdat", &mdat));
    out
}

fn riff(body: &[u8]) -> Vec<u8> {
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&u32::try_from(body.len() + 4).unwrap().to_le_bytes());
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
    assert_eq!(
        detect_image_format(b"II*\0\x08\0\0\0\0\0\0\0"),
        ImageFormat::Tiff
    );
    assert_eq!(
        detect_image_format(b"\0\0\0\x18ftypheic"),
        ImageFormat::Isobmff
    );
    assert_eq!(detect_image_format(b"not an image"), ImageFormat::Unknown);
}

#[test]
fn heic_zeroes_exif_item_and_keeps_layout() {
    let env = TestEnv::new();

    let bytes = heic(Some(b"Exif\0\0secret gps"), b"pixels");
    let out = stripped(&env, "photo.heic", &bytes);

    assert_eq!(out.len(), bytes.len());
    assert!(!out.windows(10).any(|w| w == b"secret gps"));
    assert!(out.windows(6).any(|w| w == b"pixels"));
    assert_eq!(&out[..12], &bytes[..12]);
}

#[test]
fn heic_zeroes_xmp_uuid_box() {
    let env = TestEnv::new();

    let mut uuid_body = XMP_UUID.to_vec();
    uuid_body.extend_from_slice(b"<x:xmpmeta>secret gps</x:xmpmeta>");

    let mut bytes = heic(None, b"pixels");
    bytes.extend(bmff_box(b"uuid", &uuid_body));

    let out = stripped(&env, "xmp.heic", &bytes);

    assert_eq!(out.len(), bytes.len());
    assert!(!out.windows(10).any(|w| w == b"secret gps"));
    assert!(out.windows(6).any(|w| w == b"pixels"));
}

#[test]
fn heic_without_exif_item_is_left_alone() {
    let env = TestEnv::new();
    unchanged(&env, "clean.heic", &heic(None, b"pixels"));
}

#[test]
fn unparsable_image_extension_is_refused() {
    let env = TestEnv::new();
    refused(&env, "photo.heic", b"not really an image file");
}

#[test]
fn tiff_is_refused() {
    let env = TestEnv::new();
    refused(&env, "scan.tif", b"II*\0\x08\0\0\0\0\0\0\0raw exif here");
}

#[test]
fn jpeg_drops_exif_keeps_icc_and_scan() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(0xe0, b"JFIF\0test"));
    bytes.extend(jpeg_segment(JPEG_APP1, b"Exif\0\0secret gps"));
    bytes.extend(jpeg_segment(JPEG_APP2, b"ICC_PROFILE\0colour data"));
    bytes.extend(jpeg_segment(JPEG_APP2, b"MPF\0second image gps"));
    bytes.extend(jpeg_segment(0xed, b"Photoshop 3.0"));
    bytes.extend(jpeg_segment(JPEG_COM, b"a comment"));
    bytes.extend(jpeg_segment(0xdb, b"quant table"));
    bytes.extend_from_slice(&[0xff, JPEG_SOS, 0x00, 0x08, 1, 2, 3, 4, 5, 6]);
    bytes.extend_from_slice(&[0xff, JPEG_EOI]);

    let out = stripped(&env, "exif.jpg", &bytes);

    assert!(!out.windows(4).any(|w| w == b"Exif"));
    assert!(!out.windows(17).any(|w| w == b"second image gps"));
    assert!(!out.windows(9).any(|w| w == b"Photoshop"));
    assert!(!out.windows(9).any(|w| w == b"a comment"));
    assert!(out.windows(11).any(|w| w == b"ICC_PROFILE"));
    assert!(out.windows(4).any(|w| w == b"JFIF"));
    assert!(out.windows(11).any(|w| w == b"quant table"));
    assert_eq!(&out[..2], &[0xff, 0xd8]);
    assert_eq!(&out[out.len() - 2..], &[0xff, JPEG_EOI]);
}

#[test]
fn jpeg_drops_trailer_after_end_of_image() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(0xdb, b"quant table"));
    bytes.extend_from_slice(&[0xff, JPEG_SOS, 0x00, 0x04, 1, 2]);
    bytes.extend_from_slice(&[0xff, 0x00, 0xff, 0xd0, 0x33]);
    bytes.extend_from_slice(&[0xff, JPEG_EOI]);
    bytes.extend_from_slice(b"trailing motion photo with gps");

    let out = stripped(&env, "trailer.jpg", &bytes);

    assert!(!out.windows(3).any(|w| w == b"gps"));
    assert!(out.windows(11).any(|w| w == b"quant table"));
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
fn desynced_jpeg_is_refused() {
    let env = TestEnv::new();

    let mut bytes = vec![0xff, 0xd8];
    bytes.extend(jpeg_segment(JPEG_APP1, b"Exif\0\0gps"));
    bytes.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);

    refused(&env, "desync.jpg", &bytes);
}

#[test]
fn png_drops_text_and_unknown_chunks_keeps_pixels() {
    let env = TestEnv::new();

    let mut bytes = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend(png_chunk(b"IHDR", &[0; 13]));
    bytes.extend(png_chunk(b"gAMA", &[0, 1, 2, 3]));
    bytes.extend(png_chunk(b"tEXt", b"Author\0someone"));
    bytes.extend(png_chunk(b"eXIf", b"gps here"));
    bytes.extend(png_chunk(b"prVW", b"preview thumbnail"));
    bytes.extend(png_chunk(b"IDAT", b"pixels"));
    bytes.extend(png_chunk(b"IEND", b""));
    bytes.extend_from_slice(b"appended junk");

    let out = stripped(&env, "meta.png", &bytes);

    assert!(!out.windows(6).any(|w| w == b"Author"));
    assert!(!out.windows(8).any(|w| w == b"gps here"));
    assert!(!out.windows(17).any(|w| w == b"preview thumbnail"));
    assert!(!out.windows(13).any(|w| w == b"appended junk"));
    assert!(out.windows(4).any(|w| w == b"gAMA"));
    assert!(out.windows(6).any(|w| w == b"pixels"));
    assert!(out.windows(4).any(|w| w == b"IEND"));
}

#[test]
fn png_truncated_chunk_is_refused() {
    let env = TestEnv::new();

    let mut bytes = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    bytes.extend_from_slice(&999u32.to_be_bytes());
    bytes.extend_from_slice(b"tEXt");
    bytes.extend_from_slice(b"short");

    refused(&env, "truncated.png", &bytes);
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
fn webp_drops_data_past_declared_riff_size() {
    let env = TestEnv::new();

    let mut bytes = riff(&webp_chunk(b"VP8 ", b"pixels"));
    let inside = bytes.len();
    bytes.extend_from_slice(b"appended gps");

    let out = stripped(&env, "trailer.webp", &bytes);

    assert_eq!(out.len(), inside);
    assert!(!out.windows(12).any(|w| w == b"appended gps"));
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
    unchanged(
        &env,
        "thing.bin",
        b"just some bytes here, not an image at all",
    );
}

#[test]
fn tiny_file_is_left_alone() {
    let env = TestEnv::new();
    unchanged(&env, "tiny.bin", &[0xff, 0xd8, 0xff]);
}
