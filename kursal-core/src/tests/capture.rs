use crate::call::capture::yuv::{I420, nv12_to_i420, yuyv_to_i420};
use crate::call::capture::{EncodedFrame, pack_chunk, quality_dims};

fn frame(keyframe: bool, rotation: u16) -> EncodedFrame {
    EncodedFrame {
        data: vec![9, 8, 7],
        keyframe,
        timestamp_us: 0x0102_0304_0506_0708,
        rotation,
    }
}

#[test]
fn pack_chunk_writes_the_wire_header() {
    let packed = pack_chunk(&frame(true, 0));
    assert_eq!(packed[0], 0);
    assert_eq!(packed[1], 1);
    assert_eq!(&packed[2..10], &0x0102_0304_0506_0708u64.to_be_bytes());
    assert_eq!(&packed[10..], &[9, 8, 7]);
}

#[test]
fn pack_chunk_marks_delta_frames() {
    assert_eq!(pack_chunk(&frame(false, 0))[1], 0);
}

#[test]
fn pack_chunk_packs_rotation_beside_the_keyframe_flag() {
    for (rotation, quarters) in [(0u16, 0u8), (90, 1), (180, 2), (270, 3), (360, 0)] {
        let keyed = pack_chunk(&frame(true, rotation))[1];
        let delta = pack_chunk(&frame(false, rotation))[1];
        assert_eq!(keyed & 1, 1);
        assert_eq!(delta & 1, 0);
        assert_eq!(keyed >> 1, quarters);
        assert_eq!(delta >> 1, quarters);
    }
}

#[test]
fn quality_dims_keeps_landscape_presets() {
    for quality in [360, 480, 720] {
        let (width, height, bitrate) = quality_dims(quality);
        assert!(width > height);
        assert!(bitrate > 0);
    }
}

#[test]
fn quality_dims_falls_back_to_480() {
    assert_eq!(quality_dims(9999), quality_dims(480));
}

fn solid_yuyv(width: usize, height: usize, y: u8, u: u8, v: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 2);
    for _ in 0..height {
        for _ in 0..width / 2 {
            out.extend_from_slice(&[y, u, y, v]);
        }
    }
    out
}

#[test]
fn yuyv_keeps_luma_and_halves_chroma() {
    let (w, h) = (4, 4);
    let src = solid_yuyv(w, h, 200, 90, 30);
    let mut dst = I420::new(w, h);

    assert!(yuyv_to_i420(&src, w, h, 0, &mut dst));
    assert!(dst.y.iter().all(|&p| p == 200));
    assert_eq!(dst.u.len(), 2 * 2);
    assert!(dst.u.iter().all(|&p| p == 90));
    assert!(dst.v.iter().all(|&p| p == 30));
}

#[test]
fn yuyv_rejects_a_short_buffer() {
    let (w, h) = (4, 4);
    let mut dst = I420::new(w, h);
    let short = vec![0u8; w * h * 2 - 1];
    assert!(!yuyv_to_i420(&short, w, h, 0, &mut dst));
}

#[test]
fn yuyv_rejects_mismatched_target() {
    let (w, h) = (4, 4);
    let src = solid_yuyv(w, h, 10, 20, 30);
    let mut dst = I420::new(8, 8);
    assert!(!yuyv_to_i420(&src, w, h, 0, &mut dst));
}

#[test]
fn yuyv_skips_row_padding() {
    let (w, h) = (4, 4);
    let stride = w * 2 + 6;
    let mut src = Vec::new();
    for _ in 0..h {
        src.extend_from_slice(&solid_yuyv(w, 1, 200, 90, 30));
        src.extend_from_slice(&[0xFF; 6]);
    }
    let mut dst = I420::new(w, h);

    assert!(yuyv_to_i420(&src, w, h, stride, &mut dst));
    assert!(dst.y.iter().all(|&p| p == 200));
    assert!(dst.u.iter().all(|&p| p == 90));
    assert!(dst.v.iter().all(|&p| p == 30));
}

#[test]
fn nv12_deinterleaves_chroma() {
    let (w, h) = (4, 4);
    let y_plane = vec![120u8; w * h];
    let uv_plane: Vec<u8> = (0..(w / 2) * (h / 2)).flat_map(|_| [77u8, 88u8]).collect();
    let mut dst = I420::new(w, h);

    assert!(nv12_to_i420(&y_plane, &uv_plane, w, h, 0, &mut dst));
    assert!(dst.y.iter().all(|&p| p == 120));
    assert!(dst.u.iter().all(|&p| p == 77));
    assert!(dst.v.iter().all(|&p| p == 88));
}

#[test]
fn nv12_skips_row_padding() {
    let (w, h) = (4, 4);
    let stride = w + 4;
    let mut y_plane = Vec::new();
    for _ in 0..h {
        y_plane.extend_from_slice(&[120u8; 4]);
        y_plane.extend_from_slice(&[0xFF; 4]);
    }
    let mut uv_plane = Vec::new();
    for _ in 0..h / 2 {
        uv_plane.extend_from_slice(&[77, 88, 77, 88]);
        uv_plane.extend_from_slice(&[0xFF; 4]);
    }
    let mut dst = I420::new(w, h);

    assert!(nv12_to_i420(&y_plane, &uv_plane, w, h, stride, &mut dst));
    assert!(dst.y.iter().all(|&p| p == 120));
    assert!(dst.u.iter().all(|&p| p == 77));
    assert!(dst.v.iter().all(|&p| p == 88));
}

#[test]
fn nv12_rejects_a_truncated_chroma_plane() {
    let (w, h) = (4, 4);
    let y_plane = vec![0u8; w * h];
    let uv_plane = vec![0u8; 2];
    let mut dst = I420::new(w, h);
    assert!(!nv12_to_i420(&y_plane, &uv_plane, w, h, 0, &mut dst));
}

#[test]
fn odd_dimensions_still_allocate_full_chroma_planes() {
    let frame = I420::new(5, 3);
    assert_eq!(frame.chroma_width(), 3);
    assert_eq!(frame.u.len(), 3 * 2);
    assert_eq!(frame.v.len(), 3 * 2);
}
