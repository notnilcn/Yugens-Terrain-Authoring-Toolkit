use super::compile::{CompileOptions, bake, face_triangles};
use super::topology_edits::Affine;
use super::model::BMesh;
use pt_geom::math::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    pub apply_transforms: bool,
    pub right_handed: bool,
    pub quads: bool,
    pub ngons: bool,
    pub vertex_colors: bool,
    pub include_uvs: bool,
    pub include_normals: bool,
    pub material_library: Option<String>,
}
impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            apply_transforms: false,
            right_handed: true,
            quads: true,
            ngons: false,
            vertex_colors: false,
            include_uvs: true,
            include_normals: true,
            material_library: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ExportObject<'a> {
    pub moniker: String,
    pub cage: &'a BMesh,
    pub warp: Affine,
    pub facets: Option<&'a BTreeSet<usize>>,
    pub bake: CompileOptions,
    pub material_names: Vec<String>,
}
impl<'a> ExportObject<'a> {
    pub fn new(name: impl Into<String>, mesh: &'a BMesh) -> Self {
        ExportObject {
            moniker: name.into(),
            cage: mesh,
            warp: Affine::IDENTITY,
            facets: None,
            bake: CompileOptions::default(),
            material_names: Vec::new(),
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Corner {
    vert: u32,
    p: Vec3,
    n: Vec3,
    tex_coord: Vec2,
    tint: [u8; 4],
}
#[derive(Clone, Debug)]
struct PFace {
    stuff: u16,
    corners: Vec<Corner>,
    polys: Vec<Vec<usize>>,
    tris: Vec<[usize; 3]>,
}
fn xform_normal(xf: &Affine, n: Vec3) -> Vec3 {
    let r = xf.y.cross(xf.z) * n.x + xf.z.cross(xf.x) * n.y + xf.x.cross(xf.y) * n.z;
    (if xf.determinant() < 0.0 { -r } else { r }).unitized()
}
fn prime(o: &ExportObject, opt: &ExportOptions) -> Vec<PFace> {
    let m = o.cage;
    let mut xf = if opt.apply_transforms {
        o.warp
    } else {
        Affine::IDENTITY
    };
    if !opt.right_handed {
        for c in [&mut xf.x, &mut xf.y, &mut xf.z, &mut xf.origin] {
            c.x = -c.x;
        }
    }
    let flip = xf.determinant() < 0.0;
    let compiled = bake(m, &o.bake);
    let tris = face_triangles(m);
    let mut base: Vec<Option<(usize, usize)>> = vec![None; m.facets.len()];
    for (si, s) in compiled.surfaces.iter().enumerate() {
        let mut b = 0;
        for (fi, f) in m.facets.iter().enumerate() {
            if f.stuff == s.stuff && f.verts.len() >= 3 {
                base[fi] = Some((si, b));
                b += f.verts.len();
            }
        }
    }
    let mut out = Vec::new();
    for (fi, f) in m.facets.iter().enumerate() {
        if o.facets.is_some_and(|s| !s.contains(&fi)) {
            continue;
        }
        let Some((si, b)) = base[fi] else { continue };
        let s = &compiled.surfaces[si];
        let corners: Vec<Corner> = f
            .verts
            .iter()
            .enumerate()
            .map(|(ci, v)| {
                let (p, n, uv) = (s.positions[b + ci], s.normals[b + ci], s.uvs[b + ci]);
                Corner {
                    vert: *v,
                    p: xf.enact(v3(p[0], p[1], p[2])),
                    n: xform_normal(&xf, v3(n[0], n[1], n[2])),
                    tex_coord: v2(uv[0], uv[1]),
                    tint: f.corner_color(ci),
                }
            })
            .collect();
        let k = corners.len();
        let mut ftris: Vec<[usize; 3]> = tris[fi].clone();
        let mut whole: Vec<usize> = (0..k).collect();
        if flip {
            whole.reverse();
            for t in &mut ftris {
                t.swap(1, 2);
            }
        }
        let polys = if k == 3 || (k == 4 && opt.quads) || (k > 4 && opt.ngons && k <= 255) {
            vec![whole]
        } else {
            ftris.iter().map(|t| t.to_vec()).collect()
        };
        out.push(PFace {
            stuff: f.stuff,
            corners,
            polys,
            tris: ftris,
        });
    }
    out
}
fn tri_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let n = (b - a).cross(c - a);
    let l = n.magnitude();
    if l > 1e-20 && l.is_finite() {
        n / l
    } else {
        Vec3::ZERO
    }
}
fn all_triangles(objects: &[ExportObject], opt: &ExportOptions) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    for o in objects {
        for f in prime(o, opt) {
            for t in &f.tris {
                out.push([f.corners[t[0]].p, f.corners[t[1]].p, f.corners[t[2]].p]);
            }
        }
    }
    out
}
pub fn into_stl_text(objects: &[ExportObject], opt: &ExportOptions) -> String {
    let name = match objects {
        [one] if !one.moniker.is_empty() => one.moniker.replace(['\n', '\r', ' '], "_"),
        _ => "Mesh".to_string(),
    };
    let mut s = format!("solid {name}\n");
    for [a, b, c] in all_triangles(objects, opt) {
        let n = tri_normal(a, b, c);
        let _ = writeln!(s, "  facet normal {} {} {}\n    outer loop", n.x, n.y, n.z);
        for p in [a, b, c] {
            let _ = writeln!(s, "      vertex {} {} {}", p.x, p.y, p.z);
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    let _ = writeln!(s, "endsolid {name}");
    s
}
pub fn into_stl_bin(objects: &[ExportObject], opt: &ExportOptions) -> Result<Vec<u8>, String> {
    let tris = all_triangles(objects, opt);
    let count = u32::try_from(tris.len())
        .map_err(|_| "The STL triangle count is too large.".to_string())?;
    let mut d = Vec::with_capacity(84 + 50 * tris.len());
    let mut header = [0u8; 80];
    let tag = b"Marching Squares Terrain builder mesh";
    header[..tag.len()].copy_from_slice(tag);
    d.extend_from_slice(&header);
    d.extend_from_slice(&count.to_le_bytes());
    for [a, b, c] in tris {
        for v in [tri_normal(a, b, c), a, b, c] {
            for x in [v.x, v.y, v.z] {
                d.extend_from_slice(&x.to_le_bytes());
            }
        }
        d.extend_from_slice(&0u16.to_le_bytes());
    }
    Ok(d)
}
pub fn to_ply(objects: &[ExportObject], opt: &ExportOptions) -> String {
    let prepared: Vec<Vec<PFace>> = objects.iter().map(|o| prime(o, opt)).collect();
    let verts: usize = prepared.iter().flatten().map(|f| f.corners.len()).sum();
    let faces: usize = prepared.iter().flatten().map(|f| f.polys.len()).sum();
    let mut s =
        String::from("ply\nformat ascii 1.0\ncomment Marching Squares Terrain builder mesh\n");
    let _ = writeln!(
        s,
        "element vertex {verts}\nproperty float x\nproperty float y\nproperty float z"
    );
    if opt.include_normals {
        s.push_str("property float nx\nproperty float ny\nproperty float nz\n");
    }
    if opt.include_uvs {
        s.push_str("property float s\nproperty float t\n");
    }
    if opt.vertex_colors {
        s.push_str(
            "property uchar red\nproperty uchar green\nproperty uchar blue\nproperty uchar alpha\n",
        );
    }
    let _ = writeln!(
        s,
        "element face {faces}\nproperty list uchar int vertex_indices\nend_header"
    );
    for f in prepared.iter().flatten() {
        for c in &f.corners {
            let _ = write!(s, "{} {} {}", c.p.x, c.p.y, c.p.z);
            if opt.include_normals {
                let _ = write!(s, " {} {} {}", c.n.x, c.n.y, c.n.z);
            }
            if opt.include_uvs {
                let _ = write!(s, " {} {}", c.tex_coord.x, 1.0 - c.tex_coord.y);
            }
            if opt.vertex_colors {
                let _ = write!(
                    s,
                    " {} {} {} {}",
                    c.tint[0], c.tint[1], c.tint[2], c.tint[3]
                );
            }
            s.push('\n');
        }
    }
    let mut offset = 0usize;
    for f in prepared.iter().flatten() {
        for p in &f.polys {
            let _ = write!(s, "{}", p.len());
            for i in p {
                let _ = write!(s, " {}", offset + i);
            }
            s.push('\n');
        }
        offset += f.corners.len();
    }
    s
}
pub fn to_obj_with(objects: &[ExportObject], opt: &ExportOptions) -> String {
    let mut s = String::from("# Marching Squares Terrain builder mesh\n");
    if let Some(l) = &opt.material_library {
        let _ = writeln!(s, "mtllib {}", l.replace(['\n', '\r'], " "));
    }
    let (mut v_off, mut c_off) = (1usize, 1usize);
    for (oi, o) in objects.iter().enumerate() {
        let faces = prime(o, opt);
        let name = if o.moniker.is_empty() {
            format!("object_{oi}")
        } else {
            o.moniker.replace(['\n', '\r'], " ")
        };
        let _ = writeln!(s, "o {name}");
        let mut index: BTreeMap<u32, usize> = BTreeMap::new();
        let mut order: Vec<&Corner> = Vec::new();
        for c in faces.iter().flat_map(|f| f.corners.iter()) {
            if !index.contains_key(&c.vert) {
                index.insert(c.vert, order.len());
                order.push(c);
            }
        }
        for c in &order {
            let _ = write!(s, "v {} {} {}", c.p.x, c.p.y, c.p.z);
            if opt.vertex_colors {
                let _ = write!(
                    s,
                    " {} {} {}",
                    c.tint[0] as f32 / 255.0,
                    c.tint[1] as f32 / 255.0,
                    c.tint[2] as f32 / 255.0
                );
            }
            s.push('\n');
        }
        for c in faces.iter().flat_map(|f| f.corners.iter()) {
            if opt.include_uvs {
                let _ = writeln!(s, "vt {} {}", c.tex_coord.x, 1.0 - c.tex_coord.y);
            }
            if opt.include_normals {
                let _ = writeln!(s, "vn {} {} {}", c.n.x, c.n.y, c.n.z);
            }
        }
        let mut first = Vec::with_capacity(faces.len());
        let mut acc = c_off;
        for f in &faces {
            first.push(acc);
            acc += f.corners.len();
        }
        let slots: BTreeSet<u16> = faces.iter().map(|f| f.stuff).collect();
        for slot in slots {
            if opt.material_library.is_some() {
                let mat = o
                    .material_names
                    .get(slot as usize)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("slot_{slot}"));
                let _ = writeln!(s, "usemtl {mat}");
            }
            for (fi, f) in faces.iter().enumerate().filter(|(_, f)| f.stuff == slot) {
                for p in &f.polys {
                    s.push('f');
                    for ci in p {
                        let v = v_off + index[&f.corners[*ci].vert];
                        let c = first[fi] + ci;
                        let _ = match (opt.include_uvs, opt.include_normals) {
                            (true, true) => write!(s, " {v}/{c}/{c}"),
                            (true, false) => write!(s, " {v}/{c}"),
                            (false, true) => write!(s, " {v}//{c}"),
                            (false, false) => write!(s, " {v}"),
                        };
                    }
                    s.push('\n');
                }
            }
        }
        v_off += order.len();
        c_off = acc;
    }
    s
}
#[cfg(test)]
mod tests {
    use super::super::shapes::cuboid;
    use super::*;
    fn face_lines(s: &str) -> Vec<&str> {
        s.lines().filter(|l| l.starts_with("f ")).collect()
    }
    #[test]
    fn stl_sizes_and_winding() {
        let m = cuboid(v3(1.0, 1.0, 1.0));
        let objs = [ExportObject::new("a", &m), ExportObject::new("b", &m)];
        let opt = ExportOptions::default();
        let bin = into_stl_bin(&objs, &opt).unwrap();
        assert_eq!(bin.len(), 84 + 50 * 24);
        assert_eq!(u32::from_le_bytes(bin[80..84].try_into().unwrap()), 24);
        let ascii = into_stl_text(&objs[..1], &opt);
        assert!(ascii.starts_with("solid a\n") && ascii.trim_end().ends_with("endsolid a"));
        assert_eq!(ascii.matches("facet normal").count(), 12);
        assert!(!ascii.contains("NaN"));
        for [a, b, c] in all_triangles(&objs[..1], &opt) {
            let n = tri_normal(a, b, c);
            assert!(n.innerprod((a + b + c) / 3.0 - v3(0.0, 0.5, 0.0)) > 0.0);
        }
        let left = ExportOptions {
            right_handed: false,
            ..Default::default()
        };
        for [a, b, c] in all_triangles(&objs[..1], &left) {
            assert!(tri_normal(a, b, c).innerprod((a + b + c) / 3.0 - v3(0.0, 0.5, 0.0)) > 0.0);
        }
    }
    #[test]
    fn transforms_and_face_filter() {
        let m = cuboid(v3(1.0, 1.0, 1.0));
        let sel = BTreeSet::from([0usize, 1]);
        let mut o = ExportObject::new("box", &m);
        o.warp = Affine {
            origin: v3(10.0, 0.0, 0.0),
            ..Affine::IDENTITY
        };
        o.facets = Some(&sel);
        let local = all_triangles(std::slice::from_ref(&o), &ExportOptions::default());
        assert_eq!(local.len(), 4);
        assert!(local.iter().flatten().all(|p| p.x.abs() <= 0.5 + 1e-6));
        let world = all_triangles(
            &[o],
            &ExportOptions {
                apply_transforms: true,
                ..Default::default()
            },
        );
        assert!(world.iter().flatten().all(|p| p.x >= 9.5 - 1e-6));
    }
    #[test]
    fn ply_counts_match_body() {
        let m = cuboid(v3(1.0, 1.0, 1.0));
        let objs = [ExportObject::new("a", &m), ExportObject::new("b", &m)];
        let opt = ExportOptions {
            vertex_colors: true,
            ..Default::default()
        };
        let ply = to_ply(&objs, &opt);
        assert!(ply.contains("element vertex 48\n") && ply.contains("element face 12\n"));
        let body: Vec<&str> = ply.split("end_header\n").nth(1).unwrap().lines().collect();
        assert_eq!(body.len(), 48 + 12);
        assert_eq!(body[0].split(' ').count(), 3 + 3 + 2 + 4);
        assert!(body[0].ends_with(" 255 255 255 255"));
        let max = body[48..]
            .iter()
            .flat_map(|l| l.split(' ').skip(1))
            .map(|i| i.parse::<usize>().unwrap())
            .max()
            .unwrap();
        assert_eq!(max, 47);
        assert!(body[48..].iter().all(|l| l.starts_with("4 ")));
        let tris = to_ply(
            &objs,
            &ExportOptions {
                quads: false,
                include_normals: false,
                include_uvs: false,
                ..Default::default()
            },
        );
        assert!(tris.contains("element face 24\n") && !tris.contains("property float nx"));
    }
    #[test]
    fn obj_options() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.facets[0].stuff = 1;
        let mut a = ExportObject::new("a", &m);
        a.material_names = vec!["stone".into()];
        let objs = [a, ExportObject::new("b", &m)];
        let opt = ExportOptions {
            material_library: Some("x.mtl".into()),
            vertex_colors: true,
            ..Default::default()
        };
        let obj = to_obj_with(&objs, &opt);
        assert!(obj.contains("mtllib x.mtl\n") && obj.contains("o a\n") && obj.contains("o b\n"));
        assert!(obj.contains("usemtl stone") && obj.contains("usemtl slot_1"));
        assert_eq!(obj.lines().filter(|l| l.starts_with("v ")).count(), 16);
        assert!(
            obj.lines()
                .filter(|l| l.starts_with("v "))
                .all(|l| l.split(' ').count() == 7)
        );
        assert_eq!(obj.lines().filter(|l| l.starts_with("vt ")).count(), 48);
        let faces = face_lines(&obj);
        assert_eq!(faces.len(), 12);
        let max_v = faces
            .iter()
            .flat_map(|l| l.split(' ').skip(1))
            .map(|c| c.split('/').next().unwrap().parse::<usize>().unwrap())
            .max()
            .unwrap();
        let max_c = faces
            .iter()
            .flat_map(|l| l.split(' ').skip(1))
            .map(|c| c.split('/').nth(1).unwrap().parse::<usize>().unwrap())
            .max()
            .unwrap();
        assert_eq!((max_v, max_c), (16, 48));
        let tri = to_obj_with(
            &objs[..1],
            &ExportOptions {
                quads: false,
                include_uvs: false,
                include_normals: false,
                ..Default::default()
            },
        );
        assert_eq!(face_lines(&tri).len(), 12);
        assert!(
            face_lines(&tri)
                .iter()
                .all(|l| l.split(' ').count() == 4 && !l.contains('/'))
        );
        let cyl = super::super::shapes::tube(v3(1.0, 1.0, 1.0), 8, false);
        let o = [ExportObject::new("c", &cyl)];
        let without = to_obj_with(&o, &ExportOptions::default());
        let with = to_obj_with(
            &o,
            &ExportOptions {
                ngons: true,
                ..Default::default()
            },
        );
        assert_eq!(face_lines(&with).len(), 10);
        assert!(face_lines(&without).len() > 10);
    }
}
