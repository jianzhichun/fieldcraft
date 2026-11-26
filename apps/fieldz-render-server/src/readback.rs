//! Reading a rendered texture back: rows in a `copy_texture_to_buffer` must start on a
//! [`COPY_ALIGN`]-byte boundary, so the mapped range is unpadded into tightly packed RGBA.

/// `wgpu`'s copy row alignment.
pub const COPY_ALIGN: usize = 256;

/// The bytes per row a `w`-pixel RGBA texture is copied with.
pub fn padded_row(width: u32) -> u32 {
    let row = width as usize * 4;
    (row.div_ceil(COPY_ALIGN) * COPY_ALIGN) as u32
}

/// `mapped` (rows of `row` bytes) as tightly packed RGBA (`w × h × 4` bytes). `row` must be
/// at least `w * 4` and `mapped` must hold `row * h`.
pub fn unpad(mapped: &[u8], w: u32, h: u32, row: u32) -> Result<Vec<u8>, String> {
    let (w, h, row) = (w as usize, h as usize, row as usize);
    let packed = w * 4;
    if row < packed || mapped.len() < row * h {
        return Err(format!("a {w} × {h} image with {row}-byte rows does not fit {} bytes", mapped.len()));
    }
    let mut out = Vec::with_capacity(packed * h);
    for y in 0..h {
        out.extend_from_slice(&mapped[y * row..y * row + packed]);
    }
    Ok(out)
}

/// Copy `texture` (RGBA, `w × h`) to a buffer and map it back, synchronously.
pub fn read_texture(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let row = padded_row(w);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fieldz-render-server readback"),
        size: u64::from(row) * u64::from(h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fieldz-render-server readback") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let pixels = unpad(&mapped, w, h, row);
    drop(mapped);
    buffer.unmap();
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_padded_to_256_bytes() {
        // 4 bytes a pixel: 64 pixels fill a 256-byte row exactly.
        assert_eq!(padded_row(1), 256);
        assert_eq!(padded_row(63), 256);
        assert_eq!(padded_row(64), 256);
        assert_eq!(padded_row(65), 512);
        assert_eq!(padded_row(1280), 5120);
        for w in 1..200u32 {
            let row = padded_row(w) as usize;
            assert!(row.is_multiple_of(COPY_ALIGN));
            assert!(row >= w as usize * 4 && row - w as usize * 4 < COPY_ALIGN);
        }
    }

    #[test]
    fn unpadding_drops_the_padding_bytes() {
        let (w, h) = (3u32, 2u32);
        let row = padded_row(w) as usize;
        let mut mapped = vec![0xAA; row * h as usize];
        for y in 0..h as usize {
            for i in 0..w as usize * 4 {
                mapped[y * row + i] = (y * 10 + i) as u8;
            }
        }
        let out = unpad(&mapped, w, h, row as u32).unwrap();
        assert_eq!(out.len(), w as usize * h as usize * 4);
        assert_eq!(&out[..12], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(&out[12..24], &[10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]);
    }

    #[test]
    fn a_short_row_or_buffer_is_refused() {
        assert!(unpad(&[0; 8], 4, 1, 8).is_err());
        assert!(unpad(&[0; 8], 2, 2, 8).is_err());
        assert!(unpad(&[0; 16], 2, 2, 8).is_ok());
    }
}
