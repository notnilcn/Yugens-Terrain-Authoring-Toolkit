//! Copy / paste of sample patches with grid-aware rotation and mirroring
//! (hex: 60° steps in cube coordinates, square: 90° steps; triangle: translation only).
//! Idea from godot-hex-map's selection manager + tile orientation (MIT).

use crate::brush::Touched;
use crate::data::TerrainStore;
use crate::grid::{GridType, axial_to_offset, offset_to_axial};
use crate::math::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchCell {
    /// Position relative to the anchor in the grid's "rotatable" space (axial for hex grids,
    /// lattice for square, raw sample offset for triangles).
    pub rel: IVec2,
    /// Height relative to the anchor sample.
    pub height: f32,
    pub floor: u8,
    pub wall: u8,
    pub grass: u8,
    pub smooth: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub grid: GridType,
    pub anchor_height: f32,
    /// Triangle grids keep the anchor's orientation bit (pastes snap to the same parity).
    pub anchor_parity: i32,
    pub cells: Vec<PatchCell>,
}

/// How many distinct rotation steps a grid supports (6 hex, 4 square, 1 triangle).
pub fn rotation_steps(grid: GridType) -> i32 {
    match grid {
        GridType::Hex | GridType::HexRings => 6,
        GridType::Square => 4,
        GridType::Triangle => 1,
    }
}

/// Degrees per rotation step.
pub fn step_degrees(grid: GridType) -> f32 {
    360.0 / rotation_steps(grid) as f32
}

fn to_space(grid: GridType, s: IVec2) -> IVec2 {
    if grid == GridType::Hex { offset_to_axial(s) } else { s }
}

fn from_space(grid: GridType, a: IVec2) -> IVec2 {
    if grid == GridType::Hex { axial_to_offset(a) } else { a }
}

/// Rotates (`rot` steps, each turning +X towards +Z, the brush/stamp rotation sense) and
/// optionally mirrors (flips Z) a relative position.
pub fn transform(grid: GridType, rel: IVec2, rot: i32, mirror: bool) -> IVec2 {
    match grid {
        GridType::Hex | GridType::HexRings => {
            // Axial (q, r) with x ∝ q + r/2, z ∝ r.
            let (mut q, mut r) = (rel.x, rel.y);
            if mirror {
                // z -> -z keeps x: r' = -r, q' = q + r.
                q += r;
                r = -r;
            }
            for _ in 0..rot.rem_euclid(6) {
                // Cube (q, r, s) -> (-r, -s, -q): +60° (x towards z).
                let s = -q - r;
                (q, r) = (-r, -s);
            }
            iv2(q, r)
        }
        GridType::Square => {
            let (mut x, mut z) = (rel.x, rel.y);
            if mirror {
                z = -z;
            }
            for _ in 0..rot.rem_euclid(4) {
                (x, z) = (-z, x);
            }
            iv2(x, z)
        }
        GridType::Triangle => rel,
    }
}

impl Patch {
    /// Copies `cells` (samples) relative to `anchor`.
    pub fn copy(store: &TerrainStore, cells: &[IVec2], anchor: IVec2) -> Patch {
        let grid = store.config.grid;
        let anchor_height = store.height(anchor).unwrap_or(0.0);
        let a = to_space(grid, anchor);
        let mut out = Vec::new();
        for c in cells {
            let Some(sm) = store.sample(*c) else { continue };
            out.push(PatchCell { rel: to_space(grid, *c) - a, height: sm.height - anchor_height, floor: sm.floor, wall: sm.wall, grass: sm.grass, smooth: sm.smooth });
        }
        Patch { grid, anchor_height, anchor_parity: anchor.x.rem_euclid(2), cells: out }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Target samples of every cell when pasted at `target`.
    pub fn targets(&self, target: IVec2, rot: i32, mirror: bool) -> Vec<(IVec2, PatchCell)> {
        let mut t = target;
        if self.grid == GridType::Triangle && t.x.rem_euclid(2) != self.anchor_parity {
            t.x -= 1;
        }
        let base = to_space(self.grid, t);
        self.cells.iter().map(|c| (from_space(self.grid, base + transform(self.grid, c.rel, rot, mirror)), *c)).collect()
    }

    /// Pastes at `target`. `base` = new anchor height (`None` = the copied absolute heights).
    pub fn paste(&self, store: &mut TerrainStore, target: IVec2, rot: i32, mirror: bool, base: Option<f32>) -> Touched {
        let b = base.unwrap_or(self.anchor_height);
        let mut t = Touched::new();
        for (s, c) in self.targets(target, rot, mirror) {
            let touched = store.edit_sample(s, |d, i| {
                d.heights[i] = b + c.height;
                d.floor_tex[i] = c.floor;
                d.wall_tex[i] = c.wall;
                d.grass[i] = c.grass;
                d.smooth[i] = c.smooth as u8;
            });
            for k in touched {
                t.insert(k);
            }
            for k in store.chunks_affected_by(s) {
                t.insert(k);
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::MergeMode;
    use crate::grid::GridConfig;

    #[test]
    fn hex_rotation_is_a_rigid_60_degree_turn() {
        let cfg = GridConfig { grid: GridType::HexRings, ..Default::default() };
        let p = iv2(2, -1);
        let mut q = p;
        for _ in 0..6 {
            let n = transform(GridType::HexRings, q, 1, false);
            let (a, b) = (cfg.sample_pos(q), cfg.sample_pos(n));
            assert!((a.length() - b.length()).abs() < 1e-3);
            let ang = (a.x * b.y - a.y * b.x).atan2(a.dot(b)).to_degrees();
            assert!((ang - 60.0).abs() < 0.1, "{ang}");
            q = n;
        }
        assert_eq!(q, p);
        // Mirror flips z, keeps x.
        let m = transform(GridType::HexRings, p, 0, true);
        let (a, b) = (cfg.sample_pos(p), cfg.sample_pos(m));
        assert!((a.x - b.x).abs() < 1e-4 && (a.y + b.y).abs() < 1e-4);
    }

    #[test]
    fn square_rotation_and_paste() {
        let mut st = TerrainStore::new(GridConfig { grid: GridType::Square, chunk_size: 8, ..Default::default() });
        st.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        st.set_height(iv2(2, 2), 1.0);
        st.set_height(iv2(3, 2), 5.0);
        let patch = Patch::copy(&st, &[iv2(2, 2), iv2(3, 2)], iv2(2, 2));
        assert_eq!(transform(GridType::Square, iv2(1, 0), 1, false), iv2(0, 1));
        patch.paste(&mut st, iv2(5, 5), 1, false, Some(2.0));
        assert_eq!(st.height(iv2(5, 5)), Some(2.0));
        assert_eq!(st.height(iv2(5, 6)), Some(6.0));
    }

    #[test]
    fn odd_r_hex_paste_keeps_shape() {
        let mut st = TerrainStore::new(GridConfig { grid: GridType::Hex, chunk_size: 12, ..Default::default() });
        st.add_chunk(iv2(0, 0), MergeMode::Polyhedron);
        let anchor = iv2(4, 4);
        let ring: Vec<IVec2> = st.config.edge_neighbors(anchor);
        for (k, c) in ring.iter().enumerate() {
            st.set_height(*c, k as f32 + 1.0);
        }
        let patch = Patch::copy(&st, &ring, anchor);
        // Paste on an odd row: still the 6 neighbours of the target.
        let target = iv2(6, 7);
        let got: Vec<IVec2> = patch.targets(target, 2, true).into_iter().map(|t| t.0).collect();
        let mut want = st.config.edge_neighbors(target);
        let mut got_s = got.clone();
        got_s.sort();
        want.sort();
        assert_eq!(got_s, want);
        patch.paste(&mut st, target, 0, false, None);
        for (k, c) in st.config.edge_neighbors(target).iter().enumerate() {
            assert_eq!(st.height(*c), Some(k as f32 + 1.0));
        }
    }
}
