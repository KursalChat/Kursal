pub struct I420 {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl I420 {
    pub fn new(width: usize, height: usize) -> Self {
        let chroma_w = width.div_ceil(2);
        let chroma_h = height.div_ceil(2);
        Self {
            width,
            height,
            y: vec![0; width * height],
            u: vec![128; chroma_w * chroma_h],
            v: vec![128; chroma_w * chroma_h],
        }
    }

    pub fn chroma_width(&self) -> usize {
        self.width.div_ceil(2)
    }
}

pub fn yuyv_to_i420(
    src: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    dst: &mut I420,
) -> bool {
    let row_bytes = if stride == 0 { width * 2 } else { stride };
    if row_bytes < width * 2
        || src.len() < row_bytes * height
        || dst.width != width
        || dst.height != height
    {
        return false;
    }
    let chroma_w = dst.chroma_width();

    for row in 0..height {
        let line = &src[row * row_bytes..row * row_bytes + width * 2];
        let y_row = &mut dst.y[row * width..row * width + width];
        for col in 0..width {
            y_row[col] = line[col * 2];
        }
    }

    for crow in 0..height.div_ceil(2) {
        let top = crow * 2;
        let bottom = (top + 1).min(height - 1);
        let top_line = &src[top * row_bytes..top * row_bytes + width * 2];
        let bottom_line = &src[bottom * row_bytes..bottom * row_bytes + width * 2];
        for ccol in 0..chroma_w {
            let idx = ccol * 4;
            if idx + 3 >= width * 2 {
                break;
            }
            let u = (u16::from(top_line[idx + 1]) + u16::from(bottom_line[idx + 1])) / 2;
            let v = (u16::from(top_line[idx + 3]) + u16::from(bottom_line[idx + 3])) / 2;
            dst.u[crow * chroma_w + ccol] = u8::try_from(u).unwrap_or(u8::MAX);
            dst.v[crow * chroma_w + ccol] = u8::try_from(v).unwrap_or(u8::MAX);
        }
    }
    true
}

pub fn nv12_to_i420(
    y_plane: &[u8],
    uv_plane: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    dst: &mut I420,
) -> bool {
    let row_bytes = if stride == 0 { width } else { stride };
    if row_bytes < width
        || y_plane.len() < row_bytes * height
        || dst.width != width
        || dst.height != height
    {
        return false;
    }
    let chroma_w = dst.chroma_width();
    let chroma_h = height.div_ceil(2);
    if row_bytes < chroma_w * 2 || uv_plane.len() < row_bytes * chroma_h {
        return false;
    }

    for row in 0..height {
        dst.y[row * width..row * width + width]
            .copy_from_slice(&y_plane[row * row_bytes..row * row_bytes + width]);
    }
    for crow in 0..chroma_h {
        let line = &uv_plane[crow * row_bytes..crow * row_bytes + chroma_w * 2];
        for ccol in 0..chroma_w {
            dst.u[crow * chroma_w + ccol] = line[ccol * 2];
            dst.v[crow * chroma_w + ccol] = line[ccol * 2 + 1];
        }
    }
    true
}
