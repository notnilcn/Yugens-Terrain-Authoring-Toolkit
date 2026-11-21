use super::model::BMesh;
use super::tri::mesh_fill;
use super::uv::self_auto_tex_coord;
use crate::math::*;
use std::collections::HashMap;
#[derive(Clone, Copy, Debug)]
pub struct CompileOptions {
    pub units_per_repeat: f32,
    pub world_offset: Vec3,
}
impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            units_per_repeat: 1.0,
            world_offset: Vec3::ZERO,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub stuff: u16,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tints: Vec<[f32; 4]>,
    pub ordinals: Vec<i32>,
    pub tri_faces: Vec<u32>,
}
#[derive(Clone, Debug, Default)]
pub struct Compiled {
    pub surfaces: Vec<Surface>,
    pub collision: Vec<[f32; 3]>,
}
pub fn face_triangles(m: &BMesh) -> Vec<Vec<[usize; 3]>> {
    (0..m.facets.len())
        .map(|f| mesh_fill(&m.face_points(f), m.face_normal(f)))
        .collect()
}
pub fn bake(m: &BMesh, opt: &CompileOptions) -> Compiled {
    let normals: Vec<Vec3> = (0..m.facets.len()).map(|f| m.face_normal(f)).collect();
    let mut smooth: HashMap<(u32, u16), Vec3> = HashMap::new();
    for (fi, f) in m.facets.iter().enumerate() {
        if f.glide == 0 {
            continue;
        }
        let w = normals[fi] * m.face_area(fi).max(1e-6);
        for v in &f.verts {
            let e = smooth.entry((*v, f.glide)).or_insert(Vec3::ZERO);
            *e = *e + w;
        }
    }
    let tris = face_triangles(m);
    let mut by_slot: Vec<Surface> = Vec::new();
    let mut slot_index: HashMap<u16, usize> = HashMap::new();
    let mut collision = Vec::new();
    for (fi, f) in m.facets.iter().enumerate() {
        if f.verts.len() < 3 {
            continue;
        }
        let si = *slot_index.entry(f.stuff).or_insert_with(|| {
            by_slot.push(Surface {
                stuff: f.stuff,
                ..Default::default()
            });
            by_slot.len() - 1
        });
        let s = &mut by_slot[si];
        let base = s.positions.len() as i32;
        let n = normals[fi];
        let corner_cols = f.corner_colors();
        let manual = f.by_hand_tex_coords();
        for (ci, v) in f.verts.iter().enumerate() {
            let p = m.positions[*v as usize];
            let cn = if f.glide == 0 {
                n
            } else {
                smooth.get(&(*v, f.glide)).copied().unwrap_or(n).unitized()
            };
            let up = if f.tex_coord.world {
                p + opt.world_offset
            } else {
                p
            };
            let uv = match manual {
                Some(c) => c[ci],
                None => self_auto_tex_coord(up, f.uv_normal(n), &f.tex_coord, opt.units_per_repeat),
            };
            s.positions.push([p.x, p.y, p.z]);
            s.normals.push([cn.x, cn.y, cn.z]);
            s.uvs.push([uv.x, uv.y]);
            s.tints.push(
                corner_cols
                    .map_or(f.tint, |c| c[ci])
                    .map(|c| c as f32 / 255.0),
            );
        }
        for t in &tris[fi] {
            s.ordinals.extend_from_slice(&[
                base + t[0] as i32,
                base + t[2] as i32,
                base + t[1] as i32,
            ]);
            s.tri_faces.push(fi as u32);
            for k in [0, 2, 1] {
                let p = m.positions[f.verts[t[k]] as usize];
                collision.push([p.x, p.y, p.z]);
            }
        }
    }
    by_slot.sort_by_key(|s| s.stuff);
    Compiled {
        surfaces: by_slot,
        collision,
    }
}
#[cfg(test)]
mod tests {
    use super::super::shapes;
    use super::*;
    #[test]
    fn box_compiles_to_twelve_triangles() {
        let m = shapes::cuboid(v3(2.0, 1.0, 3.0));
        let c = bake(&m, &CompileOptions::default());
        assert_eq!(c.surfaces.len(), 1);
        assert_eq!(c.surfaces[0].ordinals.len(), 36);
        assert_eq!(c.collision.len(), 36);
    }
    #[test]
    fn godot_winding_faces_outward() {
        let m = shapes::cuboid(v3(1.0, 1.0, 1.0));
        let c = bake(&m, &CompileOptions::default());
        let s = &c.surfaces[0];
        for t in s.ordinals.chunks(3) {
            let p = |i: i32| {
                let q = s.positions[i as usize];
                v3(q[0], q[1], q[2])
            };
            let (a, b, cc) = (p(t[0]), p(t[1]), p(t[2]));
            let n = (cc - a).cross(b - a);
            let centre = (a + b + cc) / 3.0 - v3(0.0, 0.5, 0.0);
            assert!(n.innerprod(centre) > 0.0);
        }
    }
    #[test]
    fn manual_uvs_are_emitted() {
        let mut m = shapes::cuboid(v3(1.0, 1.0, 1.0));
        let n = m.facets[0].verts.len();
        m.facets[0].uv_manual = true;
        m.facets[0].uv_coords = (0..n).map(|i| v2(7.0 + i as f32, 3.0)).collect();
        let c = bake(&m, &CompileOptions::default());
        let s = &c.surfaces[0];
        assert!(s.uvs.iter().any(|u| u[0] == 7.0 && u[1] == 3.0));
    }
    #[test]
    fn corner_colors_and_uv_axis_are_emitted() {
        let mut m = shapes::cuboid(v3(1.0, 1.0, 1.0));
        let n = m.facets[0].verts.len();
        m.facets[0].color_corners = (0..n)
            .map(|i| [10 * i as u8, 0, 0, 255 - i as u8])
            .collect();
        let c = bake(&m, &CompileOptions::default());
        let s = &c.surfaces[0];
        for i in 0..n {
            assert!((s.tints[i][0] - (10 * i) as f32 / 255.0).abs() < 1e-6);
            assert!((s.tints[i][3] - (255 - i) as f32 / 255.0).abs() < 1e-6);
        }
        assert_eq!(s.tints[n], [1.0; 4], "other faces keep the face colour");
        let f = (0..m.facets.len())
            .find(|f| m.face_normal(*f).x > 0.5)
            .unwrap();
        let before = bake(&m, &CompileOptions::default());
        m.facets[f].uv_axis = v3(0.0, 1.0, 0.0);
        let after = bake(&m, &CompileOptions::default());
        assert_ne!(before.surfaces[0].uvs, after.surfaces[0].uvs);
    }
    #[test]
    fn gliding_cluster_owns_uprights() {
        let mut m = shapes::tube(v3(2.0, 2.0, 2.0), 12, true);
        let c = bake(&m, &CompileOptions::default());
        let s = &c.surfaces[0];
        let mut seen: HashMap<(i32, i32, i32), [f32; 3]> = HashMap::new();
        let mut checked = 0;
        for (i, p) in s.positions.iter().enumerate() {
            let n = s.normals[i];
            if n[1].abs() > 0.5 {
                continue;
            }
            let k = (
                (p[0] * 1000.0) as i32,
                (p[1] * 1000.0) as i32,
                (p[2] * 1000.0) as i32,
            );
            if let Some(o) = seen.insert(k, n) {
                assert!((o[0] - n[0]).abs() < 1e-4 && (o[2] - n[2]).abs() < 1e-4);
                checked += 1;
            }
        }
        assert!(checked > 10);
        for f in &mut m.facets {
            f.stuff = (f.glide > 0) as u16;
        }
        assert_eq!(bake(&m, &CompileOptions::default()).surfaces.len(), 2);
    }
}
