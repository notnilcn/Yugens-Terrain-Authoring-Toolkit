use std::collections::{BTreeMap, BTreeSet};
use super::model::{BMesh, Face};
use crate::math::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mask {
    #[default]
    Rgba,
    Rgb,
    Alpha,
}
impl Mask {
    pub fn from_i64(v: i64) -> Mask {
        match v {
            1 => Mask::Rgb,
            2 => Mask::Alpha,
            _ => Mask::Rgba,
        }
    }
}
pub type Rgba = [f32; 4];
pub fn to_f(c: [u8; 4]) -> Rgba {
    c.map(|x| x as f32 / 255.0)
}
pub fn to_u8(c: Rgba) -> [u8; 4] {
    c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}
fn lerp4(a: Rgba, b: Rgba, t: f32) -> Rgba {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}
pub fn mixer(old: Rgba, paint: Rgba, mask: Mask) -> Rgba {
    match mask {
        Mask::Rgb => [paint[0], paint[1], paint[2], old[3]],
        Mask::Alpha => [old[0], old[1], old[2], paint[3]],
        Mask::Rgba => paint,
    }
}
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 == e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn corner_colors(face: &Face) -> Vec<Rgba> {
    match face.corner_colors() {
        Some(c) => c.iter().map(|x| to_f(*x)).collect(),
        None => vec![to_f(face.tint); face.verts.len()],
    }
}
pub fn coat_facets(m: &mut BMesh, faces: &BTreeSet<usize>, tint: Rgba, mask: Mask) {
    for &fi in faces {
        let Some(face) = m.facets.get_mut(fi) else {
            continue;
        };
        let colors: Vec<[u8; 4]> = corner_colors(face)
            .into_iter()
            .map(|c| to_u8(mixer(c, tint, mask)))
            .collect();
        face.color_corners = colors;
    }
}
pub fn invert_alpha(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for &fi in faces {
        let Some(face) = m.facets.get_mut(fi) else {
            continue;
        };
        if face.color_corners.len() == face.verts.len() {
            for c in &mut face.color_corners {
                c[3] = 255 - c[3];
            }
        } else {
            face.tint[3] = 255 - face.tint[3];
        }
    }
}
pub fn dab_weights(
    m: &BMesh,
    world: impl Fn(Vec3) -> Vec3,
    hit_world: Vec3,
    radius: f32,
    strength: f32,
    softness: f32,
) -> BTreeMap<u32, f32> {
    let mut weights = BTreeMap::new();
    if radius <= 0.0 || strength <= 0.0 {
        return weights;
    }
    let solid = 0.9705 * (1.0 - softness.clamp(0.0, 1.0));
    for (vi, p) in m.positions.iter().enumerate() {
        let t = (world(*p) - hit_world).magnitude() / radius;
        if t >= 1.0 {
            continue;
        }
        let w = if t <= solid {
            strength
        } else {
            strength * (1.0 - smoothstep(solid, 1.0, t))
        };
        if w > 0.0 {
            weights.insert(vi as u32, w);
        }
    }
    weights
}
pub fn accumulate(stroke: &mut BTreeMap<u32, f32>, dab: &BTreeMap<u32, f32>) -> bool {
    let mut changed = false;
    for (v, w) in dab {
        if *w > stroke.get(v).copied().unwrap_or(0.0) + 0.0004 {
            stroke.insert(*v, *w);
            changed = true;
        }
    }
    changed
}
pub fn paint_weighted(
    m: &mut BMesh,
    base: &BMesh,
    weights: &BTreeMap<u32, f32>,
    tint: Rgba,
    mask: Mask,
) {
    for fi in 0..m.facets.len() {
        let face = &m.facets[fi];
        if !face.verts.iter().any(|v| weights.contains_key(v)) {
            continue;
        }
        let mut colors = corner_colors(face)
            .into_iter()
            .map(to_u8)
            .collect::<Vec<_>>();
        for (ci, v) in face.verts.iter().enumerate() {
            let w = weights.get(v).copied().unwrap_or(0.0);
            if w <= 0.0 {
                continue;
            }
            let b = to_f(probe(base, fi, ci));
            colors[ci] = to_u8(lerp4(b, mixer(b, tint, mask), w));
        }
        m.facets[fi].color_corners = colors;
    }
}
pub fn probe(m: &BMesh, fi: usize, ci: usize) -> [u8; 4] {
    m.facets.get(fi).map_or([255; 4], |f| f.corner_color(ci))
}
pub fn nearest_corner(points: &[Vec2], click: Vec2) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (*a.1 - click)
                .magnitude_quadratic()
                .total_cmp(&(*b.1 - click).magnitude_quadratic())
        })
        .map(|(i, _)| i)
}
pub fn assign_slot(m: &BMesh, slots: usize, existing: Option<usize>) -> usize {
    if let Some(s) = existing {
        return s;
    }
    m.facets
        .iter()
        .map(|f| f.stuff as usize + 1)
        .fold(slots, usize::max)
}
pub fn clear_corner_colors(m: &mut BMesh, faces: &BTreeSet<usize>) {
    for &fi in faces {
        if let Some(f) = m.facets.get_mut(fi) {
            f.color_corners.clear();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;
    fn all(m: &BMesh) -> BTreeSet<usize> {
        (0..m.facets.len()).collect()
    }
    #[test]
    fn fill_respects_the_mask() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[0].tint = [255, 255, 255, 128];
        coat_facets(
            &mut m,
            &BTreeSet::from([0]),
            [1.0, 0.0, 0.0, 0.0],
            Mask::Rgb,
        );
        assert!(
            m.facets[0]
                .color_corners
                .iter()
                .all(|c| *c == [255, 0, 0, 128])
        );
        coat_facets(
            &mut m,
            &BTreeSet::from([0]),
            [0.0, 0.0, 0.0, 1.0],
            Mask::Alpha,
        );
        assert!(
            m.facets[0]
                .color_corners
                .iter()
                .all(|c| *c == [255, 0, 0, 255])
        );
        coat_facets(
            &mut m,
            &BTreeSet::from([1]),
            [0.0, 1.0, 0.0, 0.5],
            Mask::Rgba,
        );
        assert!(
            m.facets[1]
                .color_corners
                .iter()
                .all(|c| *c == [0, 255, 0, 128])
        );
        assert!(m.facets[2].color_corners.is_empty());
    }
    #[test]
    fn invert_alpha_swaps_layers() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[0].color_corners =
            vec![[1, 2, 3, 0], [1, 2, 3, 255], [1, 2, 3, 100], [1, 2, 3, 55]];
        m.facets[1].tint = [9, 9, 9, 200];
        invert_alpha(&mut m, &BTreeSet::from([0, 1]));
        assert_eq!(
            m.facets[0]
                .color_corners
                .iter()
                .map(|c| c[3])
                .collect::<Vec<_>>(),
            vec![255, 0, 155, 200]
        );
        assert_eq!(m.facets[1].tint, [9, 9, 9, 55]);
        invert_alpha(&mut m, &BTreeSet::from([0, 1]));
        assert_eq!(m.facets[0].color_corners[2], [1, 2, 3, 100]);
    }
    #[test]
    fn dab_weights_fade_with_softness() {
        let m = cuboid(v3(2.0, 2.0, 2.0));
        let lo = m.positions.iter().copied().fold(v3(9.0, 9.0, 9.0), |a, p| {
            v3(a.x.min(p.x), a.y.min(p.y), a.z.min(p.z))
        });
        let corner = m.positions.iter().position(|p| *p == lo).unwrap() as u32;
        let w = dab_weights(&m, |p| p, lo, 1.0, 0.8, 0.0);
        assert_eq!(w.len(), 1);
        assert!((w[&corner] - 0.8).abs() < 0.0005);
        let w = dab_weights(&m, |p| p + v3(10.0, 0.0, 0.0), lo, 1.0, 1.0, 0.0);
        assert!(w.is_empty());
        let w = dab_weights(&m, |p| p, lo + v3(0.5, 0.0, 0.0), 1.0, 1.0, 1.0);
        assert!(w[&corner] > 0.0 && w[&corner] < 1.0);
        assert!(dab_weights(&m, |p| p, lo, 0.0, 1.0, 0.5).is_empty());
    }
    #[test]
    fn weighted_stroke_blends_from_the_base_and_never_stacks() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let base = m.clone();
        let mut stroke = BTreeMap::new();
        let dab = BTreeMap::from([(0u32, 0.5f32)]);
        assert!(accumulate(&mut stroke, &dab));
        assert!(
            !accumulate(&mut stroke, &dab),
            "same weight again changes nothing"
        );
        paint_weighted(&mut m, &base, &stroke, [0.0, 0.0, 0.0, 0.0], Mask::Rgba);
        paint_weighted(&mut m, &base, &stroke, [0.0, 0.0, 0.0, 0.0], Mask::Rgba);
        let mut touched = 0;
        for f in &m.facets {
            for (ci, v) in f.verts.iter().enumerate() {
                if *v == 0 {
                    assert_eq!(f.color_corners[ci], [128, 128, 128, 128]);
                    touched += 1;
                } else if !f.color_corners.is_empty() {
                    assert_eq!(
                        f.color_corners[ci], [255; 4],
                        "other corners keep their colour"
                    );
                }
            }
        }
        assert_eq!(touched, 3, "a box corner is shared by three faces");
        assert!(
            m.facets
                .iter()
                .filter(|f| !f.color_corners.is_empty())
                .count()
                == 3
        );
    }
    #[test]
    fn sample_and_nearest_corner() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[2].tint = [7, 8, 9, 10];
        assert_eq!(probe(&m, 2, 1), [7, 8, 9, 10]);
        m.facets[2].color_corners = vec![[1, 1, 1, 1], [2, 2, 2, 2], [3, 3, 3, 3], [4, 4, 4, 4]];
        assert_eq!(probe(&m, 2, 1), [2, 2, 2, 2]);
        assert_eq!(probe(&m, 99, 0), [255; 4]);
        assert_eq!(
            nearest_corner(&[v2(0.0, 0.0), v2(5.0, 5.0), v2(9.0, 0.0)], v2(6.0, 4.0)),
            Some(1)
        );
        assert_eq!(nearest_corner(&[], v2(0.0, 0.0)), None);
    }
    #[test]
    fn assign_slot_appends_past_used_slots() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        assert_eq!(assign_slot(&m, 2, None), 2);
        m.facets[0].stuff = 5;
        assert_eq!(assign_slot(&m, 2, None), 6);
        assert_eq!(assign_slot(&m, 2, Some(1)), 1);
        let every = all(&m);
        clear_corner_colors(&mut m, &every);
        assert!(m.facets.iter().all(|f| f.color_corners.is_empty()));
    }
    #[test]
    fn corner_colours_follow_flips_and_drop_on_new_loops() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[0].color_corners = vec![[1; 4], [2; 4], [3; 4], [4; 4]];
        m.facets[0].flip_order();
        assert_eq!(m.facets[0].color_corners[0], [4; 4]);
        assert!(
            m.facets[0]
                .with_verts(vec![0, 1, 2])
                .color_corners
                .is_empty()
        );
    }
}
