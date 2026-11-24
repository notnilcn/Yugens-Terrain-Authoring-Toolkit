use crate::math::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageLayout {
    pub width: u32,
    pub height: u32,
    pub res: u32,
    pub columns: u32,
    pub rows: u32,
}
impl PageLayout {
    pub fn capacity(&self) -> usize {
        self.columns as usize * self.rows as usize * 2
    }
    pub fn bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
    fn new(res: u32, columns: u32, rows: u32) -> Self {
        Self {
            width: round_up_to_block(columns * res),
            height: round_up_to_block(rows * res),
            res,
            columns,
            rows,
        }
    }
}
fn round_up_to_block(px: u32) -> u32 {
    px.div_ceil(4) * 4
}
fn cell_res(res: u32) -> u32 {
    res.max(4)
}
fn edge_cap(res: u32, max_size: u32) -> u32 {
    (max_size & !3).max(cell_res(res).next_power_of_two())
}
pub fn page_capacity(res: u32, max_size: u32) -> usize {
    let res = cell_res(res);
    let cells = edge_cap(res, max_size) / res;
    cells as usize * cells as usize * 2
}
fn tight_page(tris: usize, res: u32, max_size: u32) -> PageLayout {
    let cap = edge_cap(res, max_size);
    let cells = tris.div_ceil(2).max(1) as u32;
    let mut edge = res.next_power_of_two();
    loop {
        let columns = (edge.min(cap) / res).max(1);
        let rows = cells.div_ceil(columns);
        if rows * res <= edge || edge >= cap {
            return PageLayout::new(res, columns, rows);
        }
        edge *= 2;
    }
}
pub fn plan(tris: usize, res: u32, max_size: u32) -> Vec<PageLayout> {
    let res = cell_res(res);
    let full = page_capacity(res, max_size);
    let side = edge_cap(res, max_size) / res;
    let mut pages = Vec::new();
    let mut left = tris.max(1);
    while left > full {
        pages.push(PageLayout::new(res, side, side));
        left -= full;
    }
    pages.push(tight_page(left, res, max_size));
    pages
}
pub struct AtlasPage {
    pub layout: PageLayout,
    pub rgba: Vec<u8>,
}
impl AtlasPage {
    pub fn new(layout: PageLayout) -> Self {
        Self {
            layout,
            rgba: vec![0u8; layout.bytes()],
        }
    }
    pub fn bake_triangle(
        &mut self,
        slot: usize,
        mut shade: impl FnMut([f32; 3]) -> [f32; 4],
    ) -> [Vec2; 3] {
        let l = self.layout;
        assert!(
            slot < l.capacity(),
            "atlas slot {slot} is past the page's {} triangles",
            l.capacity()
        );
        let res = l.res;
        let cell = slot / 2;
        let (col, row) = (cell as u32 % l.columns, cell as u32 / l.columns);
        let even = slot.is_multiple_of(2);
        let corners = if even {
            [v2(0.0, 0.0), v2(1.0, 0.0), v2(0.0, 1.0)]
        } else {
            [v2(1.0, 0.0), v2(1.0, 1.0), v2(0.0, 1.0)]
        };
        let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
        let inset = (res as f32 - 4.0) / res as f32;
        let (x0, y0) = (col * res, row * res);
        let uvs = corners.map(|c| {
            let p = centre + (c - centre) * inset;
            v2(
                (x0 as f32 + p.x * res as f32) / l.width as f32,
                (y0 as f32 + p.y * res as f32) / l.height as f32,
            )
        });
        for py in 0..res {
            for px in 0..res {
                let u = (px as f32 + 0.5) / res as f32;
                let v = (py as f32 + 0.5) / res as f32;
                if (u + v < 1.0) != even {
                    continue;
                }
                let w = if even {
                    [1.0 - u - v, u, v]
                } else {
                    let tt = 1.0 - u;
                    let s = v - tt;
                    [1.0 - s - tt, s, tt]
                };
                let c = shade(w);
                let i = (((y0 + py) * l.width + x0 + px) * 4) as usize;
                for (texel, channel) in self.rgba[i..i + 4].iter_mut().zip(c) {
                    *texel = (channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
        uvs
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_page_grows_square_then_trims_unused_rows() {
        assert_eq!(plan(2, 16, 4096), vec![PageLayout::new(16, 1, 1)]);
        assert_eq!(plan(3, 16, 4096)[0], PageLayout::new(16, 2, 1));
        let p = plan(193_768, 16, 8192);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].width, p[0].height), (8192, 3040));
        assert!(p[0].capacity() >= 193_768);
    }
    #[test]
    fn overflow_spills_into_full_pages_then_one_tight_page() {
        let full = page_capacity(32, 4096);
        assert_eq!(full, 128 * 128 * 2);
        let p = plan(full * 2 + 10, 32, 4096);
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].width, p[0].height), (4096, 4096));
        assert_eq!(p[1], p[0]);
        assert!(p[2].capacity() >= 10 && p[2].height <= 64);
        assert_eq!(p.iter().map(PageLayout::capacity).take(2).sum::<usize>(), full * 2);
    }
    #[test]
    fn page_edges_stay_within_the_cap_and_on_compression_blocks() {
        for (tris, res, max) in [(5000, 13, 1000), (1, 128, 256), (900_000, 7, 16_383)] {
            for l in plan(tris, res, max) {
                assert!(l.width <= (max & !3).max(res.next_power_of_two()));
                assert!(l.height <= (max & !3).max(res.next_power_of_two()));
                assert_eq!((l.width % 4, l.height % 4), (0, 0));
                assert!(l.columns * l.res <= l.width && l.rows * l.res <= l.height);
            }
            let cap: usize = plan(tris, res, max).iter().map(PageLayout::capacity).sum();
            assert!(cap >= tris);
        }
    }
    #[test]
    fn texels_get_their_triangle_and_uvs_stay_inside_cells() {
        let mut page = AtlasPage::new(PageLayout::new(8, 1, 1));
        let a = page.bake_triangle(0, |w| {
            assert!((w[0] + w[1] + w[2] - 1.0).abs() < 1e-5);
            [1.0, 0.0, 0.0, 1.0]
        });
        let b = page.bake_triangle(1, |_| [0.0, 1.0, 0.0, 1.0]);
        let px = |x: u32, y: u32| &page.rgba[((y * 8 + x) * 4) as usize..((y * 8 + x) * 4 + 4) as usize];
        assert_eq!(px(1, 1), &[255, 0, 0, 255]);
        assert_eq!(px(6, 6), &[0, 255, 0, 255]);
        assert!(
            a.iter()
                .chain(b.iter())
                .all(|u| u.x > 0.0 && u.x < 1.0 && u.y > 0.0 && u.y < 1.0)
        );
    }
    #[test]
    fn barycentrics_reproduce_corners() {
        let mut page = AtlasPage::new(PageLayout::new(16, 1, 1));
        page.bake_triangle(0, |w| [w[1], 0.0, 0.0, 1.0]);
        page.bake_triangle(1, |w| [w[1], 0.0, 0.0, 1.0]);
        let r = |x: u32, y: u32| page.rgba[((y * 16 + x) * 4) as usize];
        assert!(r(14, 0) > 200, "even triangle corner 1 = (1, 0)");
        assert!(r(0, 0) < 30);
        assert!(r(15, 15) > 200, "odd triangle corner 1 = (1, 1)");
    }
    #[test]
    fn uvs_of_a_padded_page_address_its_own_cell() {
        let l = PageLayout::new(13, 3, 2);
        assert_eq!((l.width, l.height), (40, 28));
        let mut page = AtlasPage::new(l);
        let uv = page.bake_triangle(5 * 2, |_| [1.0; 4]);
        let (col, row) = (2.0, 1.0);
        for u in uv {
            assert!(u.x * 40.0 > col * 13.0 && u.x * 40.0 < (col + 1.0) * 13.0);
            assert!(u.y * 28.0 > row * 13.0 && u.y * 28.0 < (row + 1.0) * 13.0);
        }
    }
}
