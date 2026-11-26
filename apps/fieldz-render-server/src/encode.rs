//! Frame encoding: raw RGBA for small rectangles, JPEG stripes for large ones, PNG for a
//! whole frame.
//!
//! v1's rule (docs/fieldz-render-server.md §2.3): a rectangle of at most [`RAW_MAX_PIXELS`]
//! pixels (an input field while typing, the caret) goes as its bytes; a larger one (the
//! viewport during an orbit) is cut into stripes of at most [`STRIPE_ROWS`] rows and each is
//! JPEG q80, encoded in parallel with rayon so the host can decode them concurrently; the
//! first frame after a resize is one PNG (lossless, so the host's first picture is exact).

use rayon::prelude::*;

use crate::diff::Rect;
use crate::wire::RectHeader;

/// Largest rectangle sent as raw RGBA (65 536 pixels: 256 × 256).
pub const RAW_MAX_PIXELS: u64 = 65_536;
/// JPEG quality for the stripes.
pub const JPEG_QUALITY: u8 = 80;
/// Most rows in one JPEG stripe.
pub const STRIPE_ROWS: u32 = 256;

/// One rectangle's encoded bytes, in payload order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub header: RectHeader,
    pub bytes: Vec<u8>,
}

/// Encode `rects` of `pixels` (`w × h` RGBA); `full` sends the whole frame as one PNG.
///
/// The returned headers' `off`/`len` index the concatenated payload, which is returned too.
pub fn encode(pixels: &[u8], w: u32, h: u32, rects: &[Rect], full: bool) -> Result<(Vec<RectHeader>, Vec<u8>), String> {
    if pixels.len() != (w as usize) * (h as usize) * 4 {
        return Err(format!("a {w} × {h} RGBA frame is {} bytes, not {}", (w as usize) * (h as usize) * 4, pixels.len()));
    }
    for rect in rects {
        if rect.w == 0 || rect.h == 0 || u64::from(rect.x) + u64::from(rect.w) > u64::from(w) || u64::from(rect.y) + u64::from(rect.h) > u64::from(h)
        {
            return Err(format!("rect {}×{} at ({}, {}) is outside the {w} × {h} frame", rect.w, rect.h, rect.x, rect.y));
        }
    }
    if full {
        let png = fieldcraft_render::png(w as usize, h as usize, pixels).ok_or_else(|| "PNG encode failed".to_string())?;
        let header = RectHeader { x: 0, y: 0, w, h, codec: "png".into(), off: 0, len: png.len(), rows: h };
        return Ok((vec![header], png));
    }
    if rects.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Each rectangle is one job; a large one becomes several stripe jobs, in order.
    enum Job<'a> {
        Raw(&'a Rect),
        Png(&'a Rect),
        Jpeg(&'a Rect, u32, u32),
    }
    let mut jobs = Vec::new();
    for rect in rects {
        if rect.pixels() <= RAW_MAX_PIXELS {
            jobs.push(Job::Raw(rect));
        } else if rect.w <= 1 {
            // A one-pixel-wide column cannot be JPEG (a stripe would be 1 pixel wide): PNG it.
            jobs.push(Job::Png(rect));
        } else {
            let mut y = rect.y;
            while y < rect.y + rect.h {
                let rows = STRIPE_ROWS.min(rect.y + rect.h - y);
                jobs.push(Job::Jpeg(rect, y, rows));
                y += rows;
            }
        }
    }
    let encode_one = |job: &Job<'_>| -> Result<Encoded, String> {
        let (rect, y, rows) = match *job {
            Job::Raw(rect) => (rect, rect.y, rect.h),
            Job::Png(rect) => (rect, rect.y, rect.h),
            Job::Jpeg(rect, y, rows) => (rect, y, rows),
        };
        let stripe = Rect { x: rect.x, y, w: rect.w, h: rows };
        let rgba = extract(pixels, w, stripe);
        match *job {
            Job::Raw(_) => Ok(Encoded {
                header: RectHeader { x: stripe.x, y: stripe.y, w: stripe.w, h: stripe.h, codec: "raw".into(), off: 0, len: rgba.len(), rows },
                bytes: rgba,
            }),
            Job::Png(_) => {
                let png = fieldcraft_render::png(stripe.w as usize, stripe.h as usize, &rgba)
                    .ok_or_else(|| format!("PNG encode of {} × {} failed", stripe.w, stripe.h))?;
                Ok(Encoded {
                    header: RectHeader { x: stripe.x, y: stripe.y, w: stripe.w, h: stripe.h, codec: "png".into(), off: 0, len: png.len(), rows },
                    bytes: png,
                })
            }
            Job::Jpeg(_, _, _) => {
                let mut bytes = Vec::new();
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
                // The JPEG encoder takes three channels: frames are opaque (the pass clears to
                // the theme's panel), so the alpha goes.
                let rgb = to_rgb(&rgba);
                encoder
                    .encode(&rgb, stripe.w, stripe.h, image::ExtendedColorType::Rgb8)
                    .map_err(|e| format!("JPEG encode of {} × {} failed: {e}", stripe.w, stripe.h))?;
                Ok(Encoded {
                    header: RectHeader { x: stripe.x, y: stripe.y, w: stripe.w, h: stripe.h, codec: "jpeg".into(), off: 0, len: bytes.len(), rows },
                    bytes,
                })
            }
        }
    };
    let encoded: Vec<Encoded> = jobs.par_iter().map(encode_one).collect::<Result<_, _>>()?;
    let mut headers = Vec::with_capacity(encoded.len());
    let mut payload = Vec::with_capacity(encoded.iter().map(|e| e.bytes.len()).sum());
    for mut encoded in encoded {
        encoded.header.off = payload.len();
        payload.extend_from_slice(&encoded.bytes);
        headers.push(encoded.header);
    }
    Ok((headers, payload))
}

/// The pixels of `rect` (a sub-rectangle of a `w`-wide RGBA image), packed.
fn extract(pixels: &[u8], w: u32, rect: Rect) -> Vec<u8> {
    let row = rect.w as usize * 4;
    let mut out = Vec::with_capacity(row * rect.h as usize);
    for y in rect.y..rect.y + rect.h {
        let start = ((y * w + rect.x) as usize) * 4;
        out.extend_from_slice(&pixels[start..start + row]);
    }
    out
}

/// `rgba` without its alpha channel, for JPEG.
fn to_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.as_chunks::<4>().0 {
        out.extend_from_slice(&px[..3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                out.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8, 255]);
            }
        }
        out
    }

    #[test]
    fn a_small_rect_is_its_own_bytes() {
        let (w, h) = (100u32, 100u32);
        let pixels = image(w, h);
        let rect = Rect { x: 10, y: 20, w: 8, h: 4 };
        let (headers, payload) = encode(&pixels, w, h, &[rect], false).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].codec, "raw");
        assert_eq!((headers[0].off, headers[0].len), (0, 8 * 4 * 4));
        assert_eq!(payload, extract(&pixels, w, rect));
    }

    #[test]
    fn the_raw_threshold_is_inclusive() {
        let pixels = image(256, 512);
        let (headers, _) = encode(&pixels, 256, 512, &[Rect { x: 0, y: 0, w: 256, h: 256 }], false).unwrap();
        assert_eq!(headers[0].codec, "raw");
        let (headers, _) = encode(&pixels, 256, 512, &[Rect { x: 0, y: 0, w: 256, h: 257 }], false).unwrap();
        assert_eq!(headers.len(), 2, "one pixel past the threshold is striped");
    }

    #[test]
    fn a_rect_past_the_edge_is_refused() {
        let pixels = image(256, 256);
        let err = encode(&pixels, 256, 256, &[Rect { x: 0, y: 0, w: 256, h: 257 }], false).unwrap_err();
        assert!(err.contains("outside"), "{err}");
        let err = encode(&pixels, 256, 256, &[Rect { x: 200, y: 0, w: 100, h: 10 }], false).unwrap_err();
        assert!(err.contains("outside"), "{err}");
    }

    #[test]
    fn a_large_rect_is_jpeg_stripes_of_at_most_256_rows() {
        let (w, h) = (300u32, 300u32);
        let pixels = image(w, h);
        let (headers, payload) = encode(&pixels, w, h, &[Rect { x: 5, y: 5, w: 290, h: 290 }], false).unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!((headers[0].codec.as_str(), headers[0].rows), ("jpeg", 256));
        assert_eq!((headers[1].codec.as_str(), headers[1].rows), ("jpeg", 34));
        assert_eq!(headers[1].y, 5 + 256);
        assert_eq!(headers[0].off, 0);
        assert_eq!(headers[1].off, headers[0].len);
        assert_eq!(payload.len(), headers[1].off + headers[1].len);
        // The strips decode as the sizes they claim.
        for header in &headers {
            let bytes = &payload[header.off..header.off + header.len];
            let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (header.w, header.h));
        }
    }

    #[test]
    fn a_whole_frame_is_one_png() {
        let (w, h) = (64u32, 32u32);
        let pixels = image(w, h);
        let (headers, payload) = encode(&pixels, w, h, &[Rect { x: 0, y: 0, w: 10, h: 10 }], true).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].codec, "png");
        assert_eq!((headers[0].x, headers[0].y, headers[0].w, headers[0].h), (0, 0, 64, 32));
        assert_eq!(headers[0].len, payload.len());
        assert_eq!(&payload[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn nothing_to_send_is_an_empty_frame() {
        let pixels = image(8, 8);
        let (headers, payload) = encode(&pixels, 8, 8, &[], false).unwrap();
        assert!(headers.is_empty() && payload.is_empty());
    }

    #[test]
    fn rectangles_keep_their_order_and_offsets() {
        let pixels = image(200, 200);
        let rects = [Rect { x: 0, y: 0, w: 10, h: 10 }, Rect { x: 20, y: 30, w: 100, h: 100 }];
        let (headers, payload) = encode(&pixels, 200, 200, &rects, false).unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!((headers[0].x, headers[0].y), (0, 0));
        assert_eq!((headers[1].x, headers[1].y), (20, 30));
        assert_eq!(headers[0].off, 0);
        assert_eq!(headers[1].off, headers[0].len);
        assert_eq!(payload.len(), headers[0].len + headers[1].len);
    }
}
