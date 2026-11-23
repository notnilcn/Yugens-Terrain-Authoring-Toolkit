use crate::grid::{GridConfig, GridType};
use crate::math::*;
pub const VISIBLE: usize = 0;
pub const EXPLORED: usize = 1;
pub const HIGHLIGHT: usize = 2;
#[derive(Clone, Debug, Default)]
pub struct CellData {
    pub origin: IVec2,
    pub size: IVec2,
    target: Vec<[u8; 4]>,
    current: Vec<[u8; 4]>,
    tint: Vec<[u8; 4]>,
    pub dirty: bool,
    animating: bool,
}
impl CellData {
    pub fn new(origin: IVec2, size: IVec2) -> CellData {
        let n = (size.x.max(0) * size.y.max(0)) as usize;
        CellData {
            origin,
            size: iv2(size.x.max(0), size.y.max(0)),
            target: vec![[0, 0, 0, 255]; n],
            current: vec![[0, 0, 0, 255]; n],
            tint: vec![[0; 4]; n],
            dirty: true,
            animating: false,
        }
    }
    pub fn reshape(&mut self, origin: IVec2, size: IVec2) {
        if origin == self.origin && size == self.size {
            return;
        }
        let mut n = CellData::new(origin, size);
        for j in 0..self.size.y {
            for i in 0..self.size.x {
                let c = self.origin + iv2(i, j);
                if let (Some(a), Some(b)) = (self.ordinal(c), n.ordinal(c)) {
                    n.target[b] = self.target[a];
                    n.current[b] = self.current[a];
                    n.tint[b] = self.tint[a];
                }
            }
        }
        n.animating = self.animating;
        *self = n;
    }
    pub fn len(&self) -> usize {
        self.target.len()
    }
    pub fn check_vacant(&self) -> bool {
        self.target.is_empty()
    }
    pub fn ordinal(&self, cell: IVec2) -> Option<usize> {
        let l = cell - self.origin;
        if l.x < 0 || l.y < 0 || l.x >= self.size.x || l.y >= self.size.y {
            return None;
        }
        Some((l.y * self.size.x + l.x) as usize)
    }
    fn assign_lane(&mut self, cell: IVec2, ch: usize, v: u8) -> bool {
        let Some(i) = self.ordinal(cell) else {
            return false;
        };
        if self.target[i][ch] != v {
            self.target[i][ch] = v;
            self.animating = true;
        }
        true
    }
    pub fn assign_shown(&mut self, cell: IVec2, visible: bool) -> bool {
        if visible {
            self.assign_lane(cell, EXPLORED, 255);
        }
        self.assign_lane(cell, VISIBLE, if visible { 255 } else { 0 })
    }
    pub fn set_explored(&mut self, cell: IVec2, explored: bool) -> bool {
        if !explored {
            self.assign_lane(cell, VISIBLE, 0);
        }
        self.assign_lane(cell, EXPLORED, if explored { 255 } else { 0 })
    }
    pub fn set_highlight(&mut self, cell: IVec2, strength: f32) -> bool {
        let v = (strength.clamp(0.0, 1.0) * 255.0).round() as u8;
        let Some(i) = self.ordinal(cell) else {
            return false;
        };
        self.target[i][HIGHLIGHT] = v;
        if self.current[i][HIGHLIGHT] != v {
            self.current[i][HIGHLIGHT] = v;
            self.dirty = true;
        }
        true
    }
    pub fn set_tint(&mut self, cell: IVec2, rgba: [u8; 4]) -> bool {
        let Some(i) = self.ordinal(cell) else {
            return false;
        };
        if self.tint[i] != rgba {
            self.tint[i] = rgba;
            self.dirty = true;
        }
        true
    }
    pub fn state(&self, cell: IVec2) -> Option<[u8; 4]> {
        self.ordinal(cell).map(|i| self.target[i])
    }
    pub fn current_state(&self, cell: IVec2) -> Option<[u8; 4]> {
        self.ordinal(cell).map(|i| self.current[i])
    }
    pub fn tint(&self, cell: IVec2) -> Option<[u8; 4]> {
        self.ordinal(cell).map(|i| self.tint[i])
    }
    pub fn is_visible(&self, cell: IVec2) -> bool {
        self.state(cell).is_some_and(|s| s[VISIBLE] >= 128)
    }
    pub fn is_explored(&self, cell: IVec2) -> bool {
        self.state(cell).is_some_and(|s| s[EXPLORED] >= 128)
    }
    pub fn flood(&mut self, shown: bool, explored: bool) {
        let explored = explored || shown;
        for t in &mut self.target {
            t[VISIBLE] = if shown { 255 } else { 0 };
            t[EXPLORED] = if explored { 255 } else { 0 };
        }
        self.animating = true;
    }
    pub fn hide_all(&mut self) {
        for t in &mut self.target {
            t[VISIBLE] = 0;
        }
        self.animating = true;
    }
    pub fn clear_tints(&mut self) {
        for t in &mut self.tint {
            *t = [0; 4];
        }
        for (t, c) in self.target.iter_mut().zip(self.current.iter_mut()) {
            t[HIGHLIGHT] = 0;
            c[HIGHLIGHT] = 0;
        }
        self.dirty = true;
    }
    pub fn state_bytes(&self) -> Vec<u8> {
        self.target.iter().flat_map(|s| s.iter().copied()).collect()
    }
    pub fn set_state_bytes(&mut self, bytes: &[u8]) {
        for (i, c) in bytes.chunks_exact(4).enumerate().take(self.target.len()) {
            let explored = if c[VISIBLE] >= 128 { 255 } else { c[EXPLORED] };
            self.target[i] = [c[VISIBLE], explored, c[HIGHLIGHT], 255];
            self.current[i][HIGHLIGHT] = c[HIGHLIGHT];
        }
        self.animating = true;
        self.dirty = true;
    }
    pub fn tint_bytes(&self) -> Vec<u8> {
        self.tint.iter().flat_map(|s| s.iter().copied()).collect()
    }
    pub fn set_tint_bytes(&mut self, bytes: &[u8]) {
        for (i, c) in bytes.chunks_exact(4).enumerate().take(self.tint.len()) {
            self.tint[i] = [c[0], c[1], c[2], c[3]];
        }
        self.dirty = true;
    }
    pub fn step(&mut self, dt: f32, seconds: f32) -> bool {
        if !self.animating {
            return std::mem::take(&mut self.dirty);
        }
        let delta = if seconds <= 0.0 {
            255
        } else {
            ((dt / seconds) * 255.0).ceil().clamp(1.0, 255.0) as i32
        };
        let mut moving = false;
        for (t, c) in self.target.iter().zip(self.current.iter_mut()) {
            for ch in [VISIBLE, EXPLORED] {
                let (a, b) = (c[ch] as i32, t[ch] as i32);
                if a != b {
                    let n = if b > a {
                        (a + delta).min(b)
                    } else {
                        (a - delta).max(b)
                    };
                    c[ch] = n as u8;
                    moving |= n != b;
                    self.dirty = true;
                }
            }
        }
        self.animating = moving;
        std::mem::take(&mut self.dirty)
    }
    pub fn conclude(&mut self) {
        for (t, c) in self.target.iter().zip(self.current.iter_mut()) {
            c[VISIBLE] = t[VISIBLE];
            c[EXPLORED] = t[EXPLORED];
        }
        self.animating = false;
        self.dirty = true;
    }
    pub fn animating(&self) -> bool {
        self.animating
    }
    pub fn texture_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.current.len() * 8);
        for (c, t) in self.current.iter().zip(self.tint.iter()) {
            out.extend_from_slice(c);
            out.extend_from_slice(t);
        }
        out
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaintedTints {
    pub tiles: std::collections::BTreeMap<(i32, i32), [u8; 4]>,
}
impl PaintedTints {
    pub fn assign(&mut self, cell: IVec2, rgba: [u8; 4]) -> bool {
        let k = (cell.x, cell.y);
        if rgba[3] == 0 {
            return self.tiles.remove(&k).is_some();
        }
        self.tiles.insert(k, rgba) != Some(rgba)
    }
    pub fn fetch(&self, cell: IVec2) -> Option<[u8; 4]> {
        self.tiles.get(&(cell.x, cell.y)).copied()
    }
    pub fn check_vacant(&self) -> bool {
        self.tiles.is_empty()
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.tiles.len() * 12);
        out.extend_from_slice(b"PTT1");
        out.extend_from_slice(&(self.tiles.len() as u32).to_le_bytes());
        for ((x, y), c) in &self.tiles {
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
            out.extend_from_slice(c);
        }
        out
    }
    pub fn from_bytes(b: &[u8]) -> Option<PaintedTints> {
        if b.is_empty() {
            return Some(PaintedTints::default());
        }
        if b.len() < 8 || &b[0..4] != b"PTT1" {
            return None;
        }
        let n = u32::from_le_bytes(b[4..8].try_into().ok()?) as usize;
        if b.len() != 8 + n * 12 {
            return None;
        }
        let mut t = PaintedTints::default();
        for e in b[8..].chunks_exact(12) {
            let x = i32::from_le_bytes(e[0..4].try_into().ok()?);
            let y = i32::from_le_bytes(e[4..8].try_into().ok()?);
            t.assign(iv2(x, y), [e[8], e[9], e[10], e[11]]);
        }
        Some(t)
    }
    pub fn enact(&self, data: &mut CellData) {
        for ((x, y), c) in &self.tiles {
            data.set_tint(iv2(*x, *y), *c);
        }
    }
}
pub fn data_cell(cfg: &GridConfig, p: Vec2) -> IVec2 {
    match cfg.grid {
        GridType::Square => {
            let cs = cfg.cell_size_safe();
            iv2((p.x / cs.x).floor() as i32, (p.y / cs.y).floor() as i32)
        }
        _ => cfg.world_to_sample(p),
    }
}
pub fn rect_for(cfg: &GridConfig, chunks: impl IntoIterator<Item = IVec2>) -> (IVec2, IVec2) {
    let (mut min, mut max) = (iv2(i32::MAX, i32::MAX), iv2(i32::MIN, i32::MIN));
    let mut any = false;
    for c in chunks {
        for s in cfg.chunk_cells(c) {
            min = iv2(min.x.min(s.x), min.y.min(s.y));
            max = iv2(max.x.max(s.x), max.y.max(s.y));
            any = true;
        }
    }
    if !any {
        return (iv2(0, 0), iv2(0, 0));
    }
    (min, max - min + iv2(1, 1))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visibility_fades_and_resize_keeps_values() {
        let mut d = CellData::new(iv2(-2, -2), iv2(4, 4));
        assert!(d.assign_shown(iv2(0, 0), true));
        assert!(!d.assign_shown(iv2(5, 5), true));
        assert!(d.is_explored(iv2(0, 0)) && d.is_visible(iv2(0, 0)));
        assert!(d.step(0.1, 0.4));
        let c = d.current_state(iv2(0, 0)).unwrap();
        assert!(c[VISIBLE] > 0 && c[VISIBLE] < 255, "{c:?}");
        for _ in 0..10 {
            d.step(0.1, 0.4);
        }
        assert_eq!(d.current_state(iv2(0, 0)).unwrap()[VISIBLE], 255);
        assert!(!d.animating());
        assert!(!d.step(0.1, 0.4), "no upload when idle");
        d.assign_shown(iv2(0, 0), false);
        d.conclude();
        assert_eq!(d.current_state(iv2(0, 0)).unwrap()[..2], [0, 255]);
        d.set_tint(iv2(1, 1), [255, 0, 0, 128]);
        d.reshape(iv2(0, 0), iv2(8, 8));
        assert_eq!(d.tint(iv2(1, 1)), Some([255, 0, 0, 128]));
        assert!(d.is_explored(iv2(0, 0)) && !d.is_visible(iv2(0, 0)));
        assert_eq!(d.texture_bytes().len(), 8 * 8 * 8);
    }
    #[test]
    fn bulk_bytes_roundtrip() {
        let mut d = CellData::new(iv2(0, 0), iv2(3, 2));
        let mut b = vec![0u8; 24];
        b[4] = 255;
        b[10] = 200;
        d.set_state_bytes(&b);
        assert!(d.is_visible(iv2(1, 0)) && d.is_explored(iv2(1, 0)));
        assert_eq!(d.current_state(iv2(2, 0)).unwrap()[HIGHLIGHT], 200);
        assert_eq!(d.state_bytes()[5], 255);
        let t: Vec<u8> = (0..24).map(|x| x as u8).collect();
        d.set_tint_bytes(&t);
        assert_eq!(d.tint_bytes(), t);
    }
    #[test]
    fn rect_covers_hex_ring_chunks() {
        let cfg = GridConfig {
            grid: GridType::HexRings,
            ring_radius: 2,
            ..Default::default()
        };
        let (o, s) = rect_for(&cfg, [iv2(0, 0), iv2(1, 0)]);
        let d = CellData::new(o, s);
        for c in [iv2(0, 0), iv2(1, 0)] {
            for cell in cfg.chunk_cells(c) {
                assert!(d.ordinal(cell).is_some(), "{cell:?}");
            }
        }
    }
    #[test]
    fn painted_tints_roundtrip_and_apply() {
        let mut p = PaintedTints::default();
        assert!(p.assign(iv2(1, -2), [255, 0, 0, 128]));
        assert!(!p.assign(iv2(1, -2), [255, 0, 0, 128]));
        assert!(p.assign(iv2(9, 9), [0, 0, 255, 255]));
        assert!(!p.assign(iv2(5, 5), [0, 0, 0, 0]));
        let b = p.to_bytes();
        assert_eq!(PaintedTints::from_bytes(&b), Some(p.clone()));
        assert_eq!(PaintedTints::from_bytes(&b[..b.len() - 1]), None);
        let mut d = CellData::new(iv2(-2, -2), iv2(4, 4));
        p.enact(&mut d);
        assert_eq!(d.tint(iv2(1, -2)), Some([255, 0, 0, 128]));
        assert!(p.assign(iv2(1, -2), [0; 4]));
        assert!(p.fetch(iv2(1, -2)).is_none());
    }
}
