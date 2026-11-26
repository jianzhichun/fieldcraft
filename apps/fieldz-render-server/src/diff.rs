//! Frame differences: 64 × 64 tiles compared against the last sent frame, merged into
//! rectangles.
//!
//! A rectangle is in device pixels. Tiles on the right and bottom edges are partial (the image
//! need not divide by 64); their rects are clamped to the image.

/// A tile's side in pixels.
pub const TILE: usize = 64;

/// A rectangle of the frame, in device pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn pixels(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

/// The whole frame as one rectangle.
pub fn full(w: usize, h: usize) -> Rect {
    Rect { x: 0, y: 0, w: w as u32, h: h as u32 }
}

/// The rectangles that changed from `prev` to `cur` (both `w × h` RGBA), merged by row bands:
/// a run of changed tiles in a tile row is extended down while every tile under it changed
/// too, then emitted once and marked done.
///
/// `prev` `None` or of another size (the first frame, a resize) is the whole frame.
pub fn dirty(prev: Option<&[u8]>, cur: &[u8], w: usize, h: usize) -> Vec<Rect> {
    debug_assert_eq!(cur.len(), w.saturating_mul(h).saturating_mul(4));
    let Some(prev) = prev.filter(|p| p.len() == cur.len()) else { return vec![full(w, h)] };
    let (tw, th) = (w.div_ceil(TILE), h.div_ceil(TILE));
    let mut changed = vec![false; tw * th];
    for ty in 0..th {
        let y0 = ty * TILE;
        let y1 = (y0 + TILE).min(h);
        for tx in 0..tw {
            let x0 = tx * TILE;
            let x1 = (x0 + TILE).min(w);
            changed[ty * tw + tx] = (y0..y1).any(|y| {
                let a = (y * w + x0) * 4;
                let b = (y * w + x1) * 4;
                prev[a..b] != cur[a..b]
            });
        }
    }
    let mut used = vec![false; tw * th];
    let mut out = Vec::new();
    for ty in 0..th {
        let mut tx = 0;
        while tx < tw {
            if !changed[ty * tw + tx] || used[ty * tw + tx] {
                tx += 1;
                continue;
            }
            let x0 = tx;
            while tx < tw && changed[ty * tw + tx] && !used[ty * tw + tx] {
                tx += 1;
            }
            let x1 = tx;
            let mut y1 = ty + 1;
            'grow: while y1 < th {
                for x in x0..x1 {
                    if !changed[y1 * tw + x] || used[y1 * tw + x] {
                        break 'grow;
                    }
                }
                y1 += 1;
            }
            for y in ty..y1 {
                for x in x0..x1 {
                    used[y * tw + x] = true;
                }
            }
            let (px, py) = (x0 * TILE, ty * TILE);
            out.push(Rect { x: px as u32, y: py as u32, w: ((x1 * TILE).min(w) - px) as u32, h: ((y1 * TILE).min(h) - py) as u32 });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: usize, h: usize, fill: u8) -> Vec<u8> {
        vec![fill; w * h * 4]
    }

    fn paint(img: &mut [u8], w: usize, r: Rect, value: u8) {
        for y in r.y as usize..(r.y + r.h) as usize {
            let a = (y * w + r.x as usize) * 4;
            let b = a + r.w as usize * 4;
            img[a..b].fill(value);
        }
    }

    #[test]
    fn no_change_is_no_rects() {
        let (w, h) = (200, 130);
        let a = image(w, h, 7);
        assert_eq!(dirty(Some(&a), &a, w, h), Vec::new());
    }

    #[test]
    fn no_previous_frame_is_the_whole_frame() {
        let (w, h) = (65, 129);
        let a = image(w, h, 3);
        assert_eq!(dirty(None, &a, w, h), vec![full(w, h)]);
        // A size change is the whole frame too.
        let b = image(w + 1, h, 3);
        assert_eq!(dirty(Some(&a), &b, w + 1, h), vec![full(w + 1, h)]);
    }

    #[test]
    fn one_tile_is_its_exact_pixels() {
        let (w, h) = (200, 130);
        let a = image(w, h, 0);
        let mut b = a.clone();
        paint(&mut b, w, Rect { x: 64, y: 64, w: 64, h: 64 }, 9);
        assert_eq!(dirty(Some(&a), &b, w, h), vec![Rect { x: 64, y: 64, w: 64, h: 64 }]);
    }

    #[test]
    fn edge_tiles_are_clamped() {
        let (w, h) = (100, 70);
        let a = image(w, h, 0);
        let mut b = a.clone();
        paint(&mut b, w, Rect { x: 64, y: 64, w: 36, h: 6 }, 9);
        assert_eq!(dirty(Some(&a), &b, w, h), vec![Rect { x: 64, y: 64, w: 36, h: 6 }]);
    }

    #[test]
    fn a_vertical_run_of_tiles_is_one_rect() {
        let (w, h) = (128, 256);
        let a = image(w, h, 0);
        let mut b = a.clone();
        paint(&mut b, w, Rect { x: 64, y: 0, w: 64, h: 256 }, 5);
        assert_eq!(dirty(Some(&a), &b, w, h), vec![Rect { x: 64, y: 0, w: 64, h: 256 }]);
    }

    #[test]
    fn an_l_shape_is_two_rects() {
        let (w, h) = (128, 128);
        let a = image(w, h, 0);
        let mut b = a.clone();
        // Top row of tiles, and the left tile of the second row: not one block.
        paint(&mut b, w, Rect { x: 0, y: 0, w: 128, h: 64 }, 1);
        paint(&mut b, w, Rect { x: 0, y: 64, w: 64, h: 64 }, 1);
        assert_eq!(dirty(Some(&a), &b, w, h), vec![Rect { x: 0, y: 0, w: 128, h: 64 }, Rect { x: 0, y: 64, w: 64, h: 64 }]);
    }

    #[test]
    fn a_single_pixel_changes_its_whole_tile() {
        let (w, h) = (128, 128);
        let a = image(w, h, 0);
        let mut b = a.clone();
        b[(10 * w + 10) * 4 + 1] = 200;
        assert_eq!(dirty(Some(&a), &b, w, h), vec![Rect { x: 0, y: 0, w: 64, h: 64 }]);
    }
}
