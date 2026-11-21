use super::edit_ops::{Affine, bake_transform, loop_plane, point_in_polygon, segments_cross};
use super::loops::mirror_copy;
use super::model::{BMesh, Edge, Face, edge, newell};
use super::topo::{chain_loops, directed_owner, region_boundary};
use super::tri::mesh_fill;
use super::xform::{Frame, rotate_positions, scale_positions};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub type OpResult<T> = Result<T, String>;
fn on_copy<T>(m: &mut BMesh, f: impl FnOnce(&mut BMesh) -> OpResult<T>) -> OpResult<T> {
    let mut work = m.clone();
    let out = f(&mut work)?;
    work.sanity_check()
        .map_err(|e| format!("The operation would leave an invalid mesh ({e})."))?;
    *m = work;
    Ok(out)
}
fn raw_normal(m: &BMesh, f: usize) -> Vec3 {
    newell(&m.face_points(f))
}
fn unit_or_zero(v: Vec3) -> Vec3 {
    let l = v.magnitude();
    if l > 1e-10 { v / l } else { Vec3::ZERO }
}
fn facet_adjacents(m: &BMesh) -> Vec<BTreeSet<usize>> {
    let mut out = vec![BTreeSet::new(); m.facets.len()];
    for fs in m.edge_faces().values() {
        for &a in fs {
            for &b in fs {
                if a != b {
                    out[a].insert(b);
                }
            }
        }
    }
    out
}
fn network_pieces(
    nodes: &BTreeSet<usize>,
    adjacency: &dyn Fn(usize) -> Vec<usize>,
) -> Vec<BTreeSet<usize>> {
    let mut remaining = nodes.clone();
    let mut result = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let mut component = BTreeSet::from([seed]);
        let mut queue = vec![seed];
        let mut cursor = 0;
        while cursor < queue.len() {
            let node = queue[cursor];
            cursor += 1;
            for next in adjacency(node) {
                if remaining.remove(&next) {
                    component.insert(next);
                    queue.push(next);
                }
            }
        }
        result.push(component);
    }
    result
}
fn remove_faces(m: &mut BMesh, faces: &BTreeSet<usize>) {
    let mut i = 0;
    m.facets.retain(|_| {
        let keep = !faces.contains(&i);
        i += 1;
        keep
    });
}
fn corner(f: &Face, v: u32) -> Option<usize> {
    f.verts.iter().position(|x| *x == v)
}
fn same_attributes(a: &Face, b: &Face) -> bool {
    a.glide == b.glide
        && a.stuff == b.stuff
        && a.uv_manual == b.uv_manual
        && a.tex_coord == b.tex_coord
        && a.uv_axis == b.uv_axis
}
const DISSOLVE_TWO_FACES: &str = "Dissolve requires at least two faces.";
const DISSOLVE_ATTRIBUTES: &str = "Dissolve cannot cross faces with different attributes.";
const DISSOLVE_ZERO_AREA: &str = "Dissolve cannot create a face with zero area.";
const DISSOLVE_DIRECTION: &str = "Dissolve cannot keep the face direction.";
const DISSOLVE_MANIFOLD: &str = "Dissolve requires a manifold disk.";
const DISSOLVE_PLANE: &str = "Dissolve requires faces that are on one plane.";
const DISSOLVE_BRANCHES: &str = "Dissolve requires one closed boundary without branches.";
const INVALID_SELECTION: &str = "The selection is not valid for this operation.";
struct DissolveRegion {
    facets: BTreeSet<usize>,
    verts: BTreeSet<u32>,
}
fn dissolve_tolerance(points: &[Vec3]) -> f32 {
    let Some(&first) = points.first() else {
        return 1e-5;
    };
    let (lo, hi) = points.iter().fold((first, first), |(lo, hi), p| {
        (
            v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
            v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
        )
    });
    ((hi - lo).magnitude() * 1e-5).max(1e-5)
}
fn verify_corner_spreads(
    m: &BMesh,
    verts: &BTreeSet<u32>,
    vf: &[Vec<usize>],
    ef: &HashMap<Edge, Vec<usize>>,
) -> OpResult<()> {
    for &v in verts {
        let incident: BTreeSet<usize> = vf[v as usize].iter().copied().collect();
        let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut boundary = 0;
        let mut seen = BTreeSet::new();
        for &f in &incident {
            for (a, b) in m.facets[f].loop_edges() {
                if a != v && b != v {
                    continue;
                }
                let e = edge(a, b);
                if !seen.insert(e) {
                    continue;
                }
                match ef.get(&e).map(|l| l.as_slice()).unwrap_or(&[]) {
                    [_] => boundary += 1,
                    [x, y] => {
                        adjacency.entry(*x).or_default().push(*y);
                        adjacency.entry(*y).or_default().push(*x);
                    }
                    _ => return Err(DISSOLVE_MANIFOLD.into()),
                }
            }
        }
        if boundary != 0 && boundary != 2 {
            return Err(DISSOLVE_MANIFOLD.into());
        }
        let comps = network_pieces(&incident, &|f| {
            adjacency.get(&f).cloned().unwrap_or_default()
        });
        if comps.len() > 1 {
            return Err(DISSOLVE_MANIFOLD.into());
        }
    }
    Ok(())
}
fn verify_t_joints(
    m: &BMesh,
    verts: &BTreeSet<u32>,
    edges: &BTreeSet<Edge>,
    vf: &[Vec<usize>],
    ef: &HashMap<Edge, Vec<usize>>,
    tol: f32,
) -> OpResult<()> {
    for e in edges {
        let (a, b) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
        let ab = b - a;
        let l2 = ab.innerprod(ab);
        if l2 <= tol * tol {
            continue;
        }
        let edge_faces = ef.get(e).cloned().unwrap_or_default();
        for &v in verts {
            if v == e.0 || v == e.1 || vf[v as usize].iter().any(|f| edge_faces.contains(f)) {
                continue;
            }
            let p = m.positions[v as usize];
            let t = (p - a).innerprod(ab) / l2;
            if 0.0 < t && t < 1.0 && (p - (a + ab * t)).magnitude() <= tol {
                return Err(DISSOLVE_MANIFOLD.into());
            }
        }
    }
    Ok(())
}
fn validate_region(
    m: &BMesh,
    r: &DissolveRegion,
    vf: &[Vec<usize>],
    ef: &HashMap<Edge, Vec<usize>>,
) -> OpResult<Vec<u32>> {
    if r.facets.len() < 2 {
        return Err(DISSOLVE_TWO_FACES.into());
    }
    let first = *r.facets.first().ok_or(DISSOLVE_TWO_FACES)?;
    if r.facets
        .iter()
        .any(|f| !same_attributes(&m.facets[*f], &m.facets[first]))
    {
        return Err(DISSOLVE_ATTRIBUTES.into());
    }
    let region_verts: BTreeSet<u32> = m.verts_of_faces(&r.facets);
    let region_edges: BTreeSet<Edge> = m.edges_of_faces(&r.facets);
    let boundary = region_boundary(m, &r.facets);
    if boundary.is_empty() {
        return Err(DISSOLVE_MANIFOLD.into());
    }
    verify_corner_spreads(m, &region_verts, vf, ef)?;
    let pts: Vec<Vec3> = region_verts
        .iter()
        .map(|v| m.positions[*v as usize])
        .collect();
    let tol = dissolve_tolerance(&pts);
    verify_t_joints(m, &region_verts, &region_edges, vf, ef, tol)?;
    let loops = chain_loops(&boundary).ok_or(DISSOLVE_BRANCHES)?;
    if loops.len() != 1 {
        return Err(DISSOLVE_BRANCHES.into());
    }
    let outline = loops.into_iter().next().ok_or(DISSOLVE_BRANCHES)?;
    if !r.verts.iter().all(|v| !outline.contains(v)) {
        return Err(DISSOLVE_MANIFOLD.into());
    }
    let euler = region_verts.len() as isize - region_edges.len() as isize + r.facets.len() as isize;
    if euler != 1 {
        return Err(DISSOLVE_BRANCHES.into());
    }
    let Some(normal) = r
        .facets
        .iter()
        .map(|f| raw_normal(m, *f))
        .find(|n| n.innerprod(*n) > 1e-12)
    else {
        return Err(DISSOLVE_ZERO_AREA.into());
    };
    let normal = normal.unitized();
    let origin = m.positions[m.facets[first].verts[0] as usize];
    if pts
        .iter()
        .any(|p| (*p - origin).innerprod(normal).abs() > tol)
    {
        return Err(DISSOLVE_PLANE.into());
    }
    for f in &r.facets {
        let n = raw_normal(m, *f);
        if n.innerprod(n) <= 1e-12 {
            return Err(DISSOLVE_ZERO_AREA.into());
        }
        if n.innerprod(normal) <= 0.0 {
            return Err(DISSOLVE_DIRECTION.into());
        }
    }
    let projected: Vec<Vec3> = outline
        .iter()
        .map(|v| {
            let p = m.positions[*v as usize];
            p - normal * (p - origin).innerprod(normal)
        })
        .collect();
    let bn = newell(&projected);
    if outline.len() < 3 || bn.magnitude() <= tol * tol {
        return Err(DISSOLVE_ZERO_AREA.into());
    }
    if bn.innerprod(normal) <= 0.0 {
        return Err(DISSOLVE_DIRECTION.into());
    }
    Ok(outline)
}
fn melt_sectors(m: &mut BMesh, regions: Vec<DissolveRegion>) -> OpResult<BTreeSet<usize>> {
    if regions.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let vf = m.vert_faces();
    let ef = m.edge_faces();
    let mut occupied = BTreeSet::new();
    let mut replacements: Vec<(usize, Face)> = Vec::new();
    for r in &regions {
        if !occupied.is_disjoint(&r.facets) {
            return Err("Dissolve regions must not overlap.".into());
        }
        occupied.extend(r.facets.iter().copied());
        let outline = validate_region(m, r, &vf, &ef)?;
        let first = *r.facets.first().ok_or(INVALID_SELECTION)?;
        let mut face = m.facets[first].with_verts(outline.clone());
        if r.facets
            .iter()
            .any(|f| m.facets[*f].corner_colors().is_some())
        {
            let owner = directed_owner(m);
            face.color_corners = (0..outline.len())
                .map(|i| {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    let owner_face = owner.get(&(a, b)).ok_or(INVALID_SELECTION)?;
                    let f = &m.facets[*owner_face];
                    Ok(corner(f, a).map(|c| f.corner_color(c)).unwrap_or(f.tint))
                })
                .collect::<OpResult<Vec<_>>>()?;
        }
        replacements.push((first, face));
    }
    remove_faces(m, &occupied);
    replacements.sort_by_key(|(f, _)| *f);
    let mut out = BTreeSet::new();
    for (_, face) in replacements {
        out.insert(m.facets.len());
        m.facets.push(face);
    }
    m.compact();
    Ok(out)
}
pub fn dissolve_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> OpResult<BTreeSet<usize>> {
    if faces.is_empty() || faces.iter().any(|f| *f >= m.facets.len()) {
        return Err(INVALID_SELECTION.into());
    }
    let nb = facet_adjacents(m);
    let regions = network_pieces(faces, &|f| nb[f].iter().copied().collect())
        .into_iter()
        .map(|faces| DissolveRegion {
            facets: faces,
            verts: BTreeSet::new(),
        })
        .collect();
    on_copy(m, |w| melt_sectors(w, regions))
}
pub fn dissolve_edges(m: &mut BMesh, edges: &BTreeSet<Edge>) -> OpResult<BTreeSet<usize>> {
    if edges.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let ef = m.edge_faces();
    let mut faces = BTreeSet::new();
    let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for e in edges {
        let e = edge(e.0, e.1);
        let Some(fs) = ef.get(&e) else {
            return Err(INVALID_SELECTION.into());
        };
        let [a, b] = fs.as_slice() else {
            return Err(DISSOLVE_MANIFOLD.into());
        };
        faces.insert(*a);
        faces.insert(*b);
        adjacency.entry(*a).or_default().push(*b);
        adjacency.entry(*b).or_default().push(*a);
    }
    let regions = network_pieces(&faces, &|f| adjacency.get(&f).cloned().unwrap_or_default())
        .into_iter()
        .map(|faces| DissolveRegion {
            facets: faces,
            verts: BTreeSet::new(),
        })
        .collect();
    on_copy(m, |w| melt_sectors(w, regions))
}
pub fn melt_corners(m: &mut BMesh, verts: &BTreeSet<u32>) -> OpResult<BTreeSet<usize>> {
    if verts.is_empty() || verts.iter().any(|v| *v as usize >= m.positions.len()) {
        return Err(INVALID_SELECTION.into());
    }
    on_copy(m, |w| {
        let vf = w.vert_faces();
        let mut neighbours: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for e in w.ridges() {
            neighbours.entry(e.0).or_default().insert(e.1);
            neighbours.entry(e.1).or_default().insert(e.0);
        }
        let (chain, mut merge): (BTreeSet<u32>, BTreeSet<u32>) = verts
            .iter()
            .copied()
            .partition(|v| neighbours.get(v).is_some_and(|n| n.len() == 2));
        let mut removed = false;
        for v in &chain {
            let fs = &vf[*v as usize];
            if !fs
                .iter()
                .all(|f| w.facets[*f].verts.len() >= 4 && corner(&w.facets[*f], *v).is_some())
            {
                merge.insert(*v);
                continue;
            }
            for &f in fs {
                let face = &mut w.facets[f];
                let Some(ci) = corner(face, *v) else { continue };
                face.verts.remove(ci);
                if face.uv_coords.len() == face.verts.len() + 1 {
                    face.uv_coords.remove(ci);
                }
                if face.color_corners.len() == face.verts.len() + 1 {
                    face.color_corners.remove(ci);
                }
            }
            removed = true;
        }
        if merge.is_empty() {
            if !removed {
                return Err(INVALID_SELECTION.into());
            }
            w.compact();
            return Ok(BTreeSet::new());
        }
        let vf = w.vert_faces();
        let incident: BTreeSet<usize> = merge
            .iter()
            .flat_map(|v| vf[*v as usize].iter().copied())
            .collect();
        if incident.is_empty() {
            return Err(INVALID_SELECTION.into());
        }
        let nb = facet_adjacents(w);
        let regions: Vec<DissolveRegion> =
            network_pieces(&incident, &|f| nb[f].iter().copied().collect())
                .into_iter()
                .map(|faces| {
                    let verts = merge
                        .iter()
                        .copied()
                        .filter(|v| vf[*v as usize].iter().any(|f| faces.contains(f)))
                        .collect();
                    DissolveRegion {
                        facets: faces,
                        verts,
                    }
                })
                .collect();
        melt_sectors(w, regions)
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OffsetSpace {
    #[default]
    Global,
    Local,
    Normal,
}
impl OffsetSpace {
    pub fn from_i64(v: i64) -> OffsetSpace {
        match v {
            1 => OffsetSpace::Local,
            2 => OffsetSpace::Normal,
            _ => OffsetSpace::Global,
        }
    }
}
pub fn upright_frame(normal: Vec3) -> Frame {
    if normal.innerprod(normal) <= 1e-12 {
        return Frame::IDENTITY;
    }
    let y = normal.unitized();
    let reference = if y.y.abs() < 0.9505 {
        Vec3::UP
    } else {
        v3(0.0, 0.0, 1.0)
    };
    let x = reference.cross(y).unitized();
    let z = x.cross(y).unitized();
    Frame { x, y, z }
}
fn solve_basis(b: [Vec3; 3], d: Vec3) -> Option<Vec3> {
    let det = b[0].innerprod(b[1].cross(b[2]));
    if det.abs() <= 1e-12 {
        return None;
    }
    let r0 = b[1].cross(b[2]);
    let r1 = b[2].cross(b[0]);
    let r2 = b[0].cross(b[1]);
    Some(v3(r0.innerprod(d), r1.innerprod(d), r2.innerprod(d)) / det)
}
pub fn shift_items(
    m: &mut BMesh,
    verts: &BTreeSet<u32>,
    faces: &BTreeSet<usize>,
    offset: Vec3,
    space: OffsetSpace,
    node_basis: [Vec3; 3],
    per_face: bool,
) -> OpResult<()> {
    if ![offset.x, offset.y, offset.z].iter().all(|c| c.is_finite()) {
        return Err(INVALID_SELECTION.into());
    }
    let verts: BTreeSet<u32> = verts
        .iter()
        .copied()
        .filter(|v| (*v as usize) < m.positions.len())
        .collect();
    if verts.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let faces: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    match space {
        OffsetSpace::Local => super::ops::move_vertices(m, &verts, offset),
        OffsetSpace::Global => {
            let local =
                solve_basis(node_basis, offset).ok_or("The node's basis cannot be inverted.")?;
            super::ops::move_vertices(m, &verts, local);
        }
        OffsetSpace::Normal => {
            let around: BTreeSet<usize> = if faces.is_empty() {
                (0..m.facets.len())
                    .filter(|f| m.facets[*f].verts.iter().any(|v| verts.contains(v)))
                    .collect()
            } else {
                faces.clone()
            };
            let sum = around
                .iter()
                .fold(Vec3::ZERO, |a, f| a + unit_or_zero(raw_normal(m, *f)));
            let shared = upright_frame(sum).to_world(offset);
            if !per_face || faces.is_empty() {
                super::ops::move_vertices(m, &verts, shared);
                return Ok(());
            }
            let mut moves: BTreeMap<u32, (Vec3, u32)> = BTreeMap::new();
            for f in &faces {
                let d = upright_frame(raw_normal(m, *f)).to_world(offset);
                for v in &m.facets[*f].verts {
                    if verts.contains(v) {
                        let e = moves.entry(*v).or_insert((Vec3::ZERO, 0));
                        e.0 = e.0 + d;
                        e.1 += 1;
                    }
                }
            }
            for v in &verts {
                let d = moves.get(v).map(|(s, n)| *s / *n as f32).unwrap_or(shared);
                m.positions[*v as usize] = m.positions[*v as usize] + d;
            }
        }
    }
    Ok(())
}
pub fn invert_vertices(m: &BMesh, sel: &BTreeSet<u32>) -> BTreeSet<u32> {
    m.facets
        .iter()
        .flat_map(|f| f.verts.iter().copied())
        .filter(|v| !sel.contains(v))
        .collect()
}
pub fn invert_edges(m: &BMesh, sel: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    let sel: BTreeSet<Edge> = sel.iter().map(|e| edge(e.0, e.1)).collect();
    m.ridges()
        .into_iter()
        .filter(|e| !sel.contains(e))
        .collect()
}
pub fn invert_faces(m: &BMesh, sel: &BTreeSet<usize>) -> BTreeSet<usize> {
    (0..m.facets.len()).filter(|f| !sel.contains(f)).collect()
}
#[derive(Clone, Debug)]
pub enum ElementSel {
    Vertices(BTreeSet<u32>),
    Edges(BTreeSet<Edge>),
    Faces(BTreeSet<usize>),
}
pub fn grab_via_tint(m: &BMesh, sel: &ElementSel) -> ElementSel {
    let mut samples: Vec<[u8; 4]> = Vec::new();
    let mut add = |c: [u8; 4]| {
        if !samples.contains(&c) {
            samples.push(c);
        }
    };
    match sel {
        ElementSel::Vertices(vs) => {
            for f in &m.facets {
                for (ci, v) in f.verts.iter().enumerate() {
                    if vs.contains(v) {
                        add(f.corner_color(ci));
                    }
                }
            }
        }
        ElementSel::Edges(es) => {
            let es: BTreeSet<Edge> = es.iter().map(|e| edge(e.0, e.1)).collect();
            let ends: BTreeSet<u32> = es.iter().flat_map(|e| [e.0, e.1]).collect();
            for f in &m.facets {
                for (ci, v) in f.verts.iter().enumerate() {
                    if ends.contains(v) {
                        add(f.corner_color(ci));
                    }
                }
            }
        }
        ElementSel::Faces(fs) => {
            for f in fs.iter().filter_map(|f| m.facets.get(*f)) {
                for ci in 0..f.verts.len() {
                    add(f.corner_color(ci));
                }
            }
        }
    }
    let matches = |f: &Face, ci: usize| samples.contains(&f.corner_color(ci));
    match sel {
        ElementSel::Vertices(_) => ElementSel::Vertices(
            m.facets
                .iter()
                .flat_map(|f| {
                    (0..f.verts.len())
                        .filter(|ci| matches(f, *ci))
                        .map(|ci| f.verts[ci])
                        .collect::<Vec<_>>()
                })
                .collect(),
        ),
        ElementSel::Edges(_) => ElementSel::Edges(
            m.facets
                .iter()
                .flat_map(|f| {
                    let n = f.verts.len();
                    (0..n)
                        .filter(|i| matches(f, *i) && matches(f, (i + 1) % n))
                        .map(|i| edge(f.verts[i], f.verts[(i + 1) % n]))
                        .collect::<Vec<_>>()
                })
                .collect(),
        ),
        ElementSel::Faces(_) => ElementSel::Faces(
            (0..m.facets.len())
                .filter(|fi| (0..m.facets[*fi].verts.len()).any(|ci| matches(&m.facets[*fi], ci)))
                .collect(),
        ),
    }
}
pub fn grab_via_gliding_cluster(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let groups: BTreeSet<u16> = faces
        .iter()
        .filter_map(|f| m.facets.get(*f))
        .map(|f| f.glide)
        .collect();
    (0..m.facets.len())
        .filter(|f| groups.contains(&m.facets[*f].glide))
        .collect()
}
pub fn grab_via_stuff(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let slots: BTreeSet<u16> = faces
        .iter()
        .filter_map(|f| m.facets.get(*f))
        .map(|f| f.stuff)
        .collect();
    (0..m.facets.len())
        .filter(|f| slots.contains(&m.facets[*f].stuff))
        .collect()
}
fn angle_threshold(angle_deg: f32) -> f32 {
    angle_deg.clamp(0.0, 180.0).to_radians().cos() - 1e-6
}
pub fn expand_facets_via_inclination(
    m: &BMesh,
    faces: &BTreeSet<usize>,
    angle_deg: f32,
) -> BTreeSet<usize> {
    let threshold = angle_threshold(angle_deg);
    let nb = facet_adjacents(m);
    let mut out: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    for &f in faces.iter().filter(|f| **f < m.facets.len()) {
        let n = unit_or_zero(raw_normal(m, f));
        for &g in &nb[f] {
            if n.innerprod(unit_or_zero(raw_normal(m, g))) >= threshold {
                out.insert(g);
            }
        }
    }
    out
}
fn allowed_faces(m: &BMesh, seeds: &BTreeSet<usize>, angle_deg: f32) -> BTreeSet<usize> {
    let threshold = angle_threshold(angle_deg);
    let seed_normals: Vec<Vec3> = seeds
        .iter()
        .map(|f| unit_or_zero(raw_normal(m, *f)))
        .collect();
    (0..m.facets.len())
        .filter(|f| {
            seeds.contains(f) || {
                let n = unit_or_zero(raw_normal(m, *f));
                seed_normals.iter().any(|s| s.innerprod(n) >= threshold)
            }
        })
        .collect()
}
pub fn expand_corners_via_inclination(
    m: &BMesh,
    verts: &BTreeSet<u32>,
    angle_deg: f32,
) -> BTreeSet<u32> {
    let seeds: BTreeSet<usize> = (0..m.facets.len())
        .filter(|f| m.facets[*f].verts.iter().any(|v| verts.contains(v)))
        .collect();
    let allowed = allowed_faces(m, &seeds, angle_deg);
    let mut out = verts.clone();
    for f in &allowed {
        for (a, b) in m.facets[*f].loop_edges() {
            if verts.contains(&a) || verts.contains(&b) {
                out.insert(a);
                out.insert(b);
            }
        }
    }
    out
}
pub fn expand_ridges_via_inclination(
    m: &BMesh,
    edges: &BTreeSet<Edge>,
    angle_deg: f32,
) -> BTreeSet<Edge> {
    let sel: BTreeSet<Edge> = edges.iter().map(|e| edge(e.0, e.1)).collect();
    let seeds: BTreeSet<usize> = (0..m.facets.len())
        .filter(|f| {
            m.facets[*f]
                .loop_edges()
                .any(|(a, b)| sel.contains(&edge(a, b)))
        })
        .collect();
    let allowed = allowed_faces(m, &seeds, angle_deg);
    let ends: BTreeSet<u32> = sel.iter().flat_map(|e| [e.0, e.1]).collect();
    let mut out = sel.clone();
    for f in &allowed {
        for (a, b) in m.facets[*f].loop_edges() {
            if ends.contains(&a) || ends.contains(&b) {
                out.insert(edge(a, b));
            }
        }
    }
    out
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExtrudeMethod {
    #[default]
    FaceNormal,
    VertexNormal,
    IndividualFaces,
}
impl ExtrudeMethod {
    pub fn from_i64(v: i64) -> ExtrudeMethod {
        match v {
            1 => ExtrudeMethod::VertexNormal,
            2 => ExtrudeMethod::IndividualFaces,
            _ => ExtrudeMethod::FaceNormal,
        }
    }
}
fn wall(src: &Face, a: u32, b: u32, a2: u32, b2: u32) -> Face {
    let mut w = src.with_verts(vec![a, b, b2, a2]);
    w.glide = 0;
    if let (Some(ca), Some(cb)) = (corner(src, a), corner(src, b)) {
        if src.corner_colors().is_some() {
            let (ca, cb) = (src.corner_color(ca), src.corner_color(cb));
            w.color_corners = vec![ca, cb, cb, ca];
        }
    }
    w
}
pub fn pull_out_picked_approach(
    m: &mut BMesh,
    faces: &BTreeSet<usize>,
    distance: f32,
    method: ExtrudeMethod,
    allow_zero_distance: bool,
) -> OpResult<BTreeSet<usize>> {
    let faces: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    if faces.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    if !distance.is_finite() || (distance.abs() <= 1e-8 && !allow_zero_distance) {
        return Err("Extrusion distance must be finite and nonzero.".into());
    }
    if faces
        .iter()
        .any(|f| raw_normal(m, *f).innerprod(raw_normal(m, *f)) <= 0.0)
    {
        return Err("Cannot extrude a face with zero area.".into());
    }
    on_copy(m, |w| {
        if method == ExtrudeMethod::IndividualFaces {
            for &f in &faces {
                let n = unit_or_zero(raw_normal(w, f));
                let src = w.facets[f].clone();
                let cap: Vec<u32> = src
                    .verts
                    .iter()
                    .map(|v| w.add_vert(w.positions[*v as usize] + n * distance))
                    .collect();
                let k = src.verts.len();
                for i in 0..k {
                    let j = (i + 1) % k;
                    w.facets
                        .push(wall(&src, src.verts[i], src.verts[j], cap[i], cap[j]));
                }
                w.facets[f].verts = cap;
            }
            return Ok(faces.clone());
        }
        let mut sums: BTreeMap<u32, Vec3> = BTreeMap::new();
        let mut directed = BTreeSet::new();
        let mut counts: HashMap<Edge, usize> = HashMap::new();
        for &f in &faces {
            let n = unit_or_zero(raw_normal(w, f));
            for v in &w.facets[f].verts {
                let s = sums.entry(*v).or_insert(Vec3::ZERO);
                *s = *s + n;
            }
            for (a, b) in w.facets[f].loop_edges() {
                directed.insert((a, b));
                *counts.entry(edge(a, b)).or_default() += 1;
            }
        }
        if counts.values().any(|c| *c >= 3) {
            return Err("Cannot extrude non-manifold geometry.".into());
        }
        if method == ExtrudeMethod::VertexNormal {
            for s in sums.values_mut() {
                *s = Vec3::ZERO;
            }
            for f in 0..w.facets.len() {
                let n = unit_or_zero(raw_normal(w, f));
                for v in w.facets[f].verts.clone() {
                    if let Some(s) = sums.get_mut(&v) {
                        *s = *s + n;
                    }
                }
            }
        }
        let mut dup: HashMap<u32, u32> = HashMap::new();
        for (v, s) in &sums {
            let p = w.positions[*v as usize] + unit_or_zero(*s) * distance;
            dup.insert(*v, w.add_vert(p));
        }
        for &f in &faces {
            let src = w.facets[f].clone();
            for (a, b) in src.loop_edges() {
                if !directed.contains(&(b, a)) {
                    w.facets.push(wall(&src, a, b, dup[&a], dup[&b]));
                }
            }
            for v in &mut w.facets[f].verts {
                *v = dup[v];
            }
        }
        Ok(faces.clone())
    })
}
pub fn pull_out_ridges_grouped(
    m: &mut BMesh,
    edges: &BTreeSet<Edge>,
    distance: f32,
    offset: Vec3,
    as_group: bool,
) -> BTreeSet<Edge> {
    if as_group {
        return super::topo::pull_out_exposed_ridges(m, edges, distance, offset);
    }
    let owner = directed_owner(m);
    let mut out = BTreeSet::new();
    for &(a, b) in edges {
        let (a, b, f) = match (owner.get(&(a, b)), owner.get(&(b, a))) {
            (Some(f), None) => (a, b, *f),
            (None, Some(f)) => (b, a, *f),
            _ => continue,
        };
        let e = (m.positions[b as usize] - m.positions[a as usize]).unitized();
        let dir = e.cross(m.face_normal(f)).unitized() * distance + offset;
        let a2 = m.add_vert(m.positions[a as usize] + dir);
        let b2 = m.add_vert(m.positions[b as usize] + dir);
        let src = m.facets[f].clone();
        m.facets.push(Face {
            glide: 0,
            ..src.with_verts(vec![b, a, a2, b2])
        });
        out.insert(edge(a2, b2));
    }
    out
}
pub fn inset_center_grouped(
    m: &mut BMesh,
    faces: &BTreeSet<usize>,
    width: f32,
    individual: bool,
) -> OpResult<BTreeSet<usize>> {
    let faces: BTreeSet<usize> = faces
        .iter()
        .copied()
        .filter(|f| *f < m.facets.len())
        .collect();
    if faces.is_empty() || !width.is_finite() || width <= 0.0 {
        return Err(INVALID_SELECTION.into());
    }
    if individual {
        return Ok(super::mesh_ops::inset_rim(m, &faces, width, true));
    }
    let regions = inset_regions(m, &faces);
    on_copy(m, |w| {
        let mut out = BTreeSet::new();
        for region in regions {
            if region.len() == 1 {
                out.extend(super::mesh_ops::inset_rim(w, &region, width, true));
            } else {
                recess_sector(w, &region, width)?;
                out.extend(region);
            }
        }
        Ok(out)
    })
}
fn inset_regions(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let nb = facet_adjacents(m);
    let mut remaining = faces.clone();
    let mut regions = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let seed_face = &m.facets[seed];
        let n0 = unit_or_zero(raw_normal(m, seed));
        let origin = m.positions[seed_face.verts[0] as usize];
        let mut region = BTreeSet::from([seed]);
        let mut queue = vec![seed];
        let mut cursor = 0;
        while cursor < queue.len() {
            let f = queue[cursor];
            cursor += 1;
            for &g in &nb[f] {
                if !remaining.contains(&g) || !same_attributes(&m.facets[g], seed_face) {
                    continue;
                }
                if unit_or_zero(raw_normal(m, g)).innerprod(n0) <= 0.9995 {
                    continue;
                }
                if !m.facets[g]
                    .verts
                    .iter()
                    .all(|v| (m.positions[*v as usize] - origin).innerprod(n0).abs() < 1e-4)
                {
                    continue;
                }
                remaining.remove(&g);
                region.insert(g);
                queue.push(g);
            }
        }
        regions.push(region);
    }
    regions
}
fn recess_sector(m: &mut BMesh, region: &BTreeSet<usize>, width: f32) -> OpResult<()> {
    let invalid = || format!("The inset width {width} is too large for the selection.");
    let first = *region.first().ok_or(INVALID_SELECTION)?;
    let n = unit_or_zero(raw_normal(m, first));
    let boundary = region_boundary(m, region);
    let loops = chain_loops(&boundary).ok_or(INVALID_SELECTION)?;
    let owner = directed_owner(m);
    let mut inner: HashMap<u32, u32> = HashMap::new();
    let mut targets: Vec<(u32, Vec3)> = Vec::new();
    for lp in &loops {
        let k = lp.len();
        if k < 3 {
            return Err(INVALID_SELECTION.into());
        }
        let pts: Vec<Vec3> = lp.iter().map(|v| m.positions[*v as usize]).collect();
        let mut moved = Vec::with_capacity(k);
        for i in 0..k {
            let (prev, cur, next) = (pts[(i + k - 1) % k], pts[i], pts[(i + 1) % k]);
            let in1 = unit_or_zero(n.cross(cur - prev));
            let in2 = unit_or_zero(n.cross(next - cur));
            let denom = 1.0 + in1.innerprod(in2);
            if denom < 1e-4 {
                return Err(invalid());
            }
            moved.push(cur + (in1 + in2) * (width / denom));
        }
        for i in 0..k {
            let j = (i + 1) % k;
            if (moved[j] - moved[i]).innerprod(pts[j] - pts[i]) <= 1e-10 {
                return Err(invalid());
            }
        }
        for (v, p) in lp.iter().zip(moved) {
            if inner.contains_key(v) {
                return Err(INVALID_SELECTION.into());
            }
            inner.insert(*v, u32::MAX);
            targets.push((*v, p));
        }
    }
    for (v, p) in targets {
        inner.insert(v, m.add_vert(p));
    }
    let originals: Vec<Face> = region.iter().map(|f| m.facets[*f].clone()).collect();
    let turns = |m: &BMesh, f: usize| -> Vec<f32> {
        let p = m.face_points(f);
        let k = p.len();
        (0..k)
            .map(|i| n.innerprod((p[i] - p[(i + k - 1) % k]).cross(p[(i + 1) % k] - p[i])))
            .collect()
    };
    for f in region {
        let before = turns(m, *f);
        for v in &mut m.facets[*f].verts {
            if let Some(nv) = inner.get(v) {
                *v = *nv;
            }
        }
        let rn = raw_normal(m, *f);
        if rn.innerprod(n) <= 1e-12 {
            return Err(invalid());
        }
        let eps = 1e-6 * rn.magnitude();
        if before
            .iter()
            .zip(turns(m, *f))
            .any(|(b, a)| (*b > eps && a < -eps) || (*b < -eps && a > eps))
        {
            return Err(invalid());
        }
    }
    for (a, b) in boundary {
        let fi = owner.get(&(a, b)).ok_or(INVALID_SELECTION)?;
        let index = region
            .iter()
            .position(|f| f == fi)
            .ok_or(INVALID_SELECTION)?;
        let src = originals.get(index).ok_or(INVALID_SELECTION)?;
        let a2 = *inner.get(&a).ok_or(INVALID_SELECTION)?;
        let b2 = *inner.get(&b).ok_or(INVALID_SELECTION)?;
        m.facets.push(wall(src, a, b, a2, b2));
    }
    Ok(())
}
pub fn condense_corners_into(
    m: &mut BMesh,
    verts: &BTreeSet<u32>,
    addressee: Option<u32>,
) -> Option<u32> {
    let list: Vec<u32> = verts
        .iter()
        .copied()
        .filter(|v| (*v as usize) < m.positions.len())
        .collect();
    let target = addressee
        .filter(|t| list.contains(t))
        .or(list.first().copied())?;
    let p = m.positions[target as usize];
    let saved = m.positions.clone();
    let (lo, hi) = m.extent()?;
    let sentinel = hi + v3(1.0, 1.0, 1.0) * ((hi - lo).magnitude() + 1.0);
    for v in &list {
        m.positions[*v as usize] = sentinel;
    }
    match super::mesh_ops::collapse_selection(m, verts) {
        Some(v) => {
            m.positions[v as usize] = p;
            Some(v)
        }
        None => {
            m.positions = saved;
            None
        }
    }
}
#[derive(Clone, Debug)]
pub enum HoleSel {
    Vertices(BTreeSet<u32>),
    Edges(BTreeSet<Edge>),
}
fn selected_runs(hole: &[u32], selected: &[bool]) -> Vec<Vec<u32>> {
    let count = hole.len();
    let Some(brk) = selected.iter().position(|s| !s) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    let mut path: Vec<u32> = Vec::new();
    for off in 1..=count {
        let i = (brk + off) % count;
        if selected[i] {
            if path.is_empty() {
                path.push(hole[i]);
            }
            path.push(hole[(i + 1) % count]);
        } else if path.len() < 3 {
            path.clear();
        } else {
            paths.push(std::mem::take(&mut path));
        }
    }
    if path.len() >= 3 {
        paths.push(path);
    }
    paths
}
fn validate_partial_fill(
    m: &BMesh,
    hole: &[u32],
    cap: &[u32],
    owner: &HashMap<(u32, u32), usize>,
) -> OpResult<()> {
    let lp = loop_plane(m, hole).ok_or(DISSOLVE_ZERO_AREA)?;
    let k = hole.len();
    let along = (0..k)
        .filter_map(|i| owner.get(&(hole[(i + 1) % k], hole[i])))
        .fold(Vec3::ZERO, |a, f| a + unit_or_zero(raw_normal(m, *f)));
    let hole_n = unit_or_zero(newell(
        &hole
            .iter()
            .map(|v| m.positions[*v as usize])
            .collect::<Vec<_>>(),
    ));
    let outside = along.magnitude() > 1e-6 && unit_or_zero(along).innerprod(hole_n) < -0.5;
    let to2 = |v: u32| lp.dir_2d(m.positions[v as usize] - lp.origin);
    let (Some(&first_vert), Some(&last_vert)) = (cap.first(), cap.last()) else {
        return Err(INVALID_SELECTION.into());
    };
    let (first, last) = (to2(first_vert), to2(last_vert));
    let n = lp.pts.len();
    let eps = 1e-7 * lp.magnify * lp.magnify;
    for i in 0..n {
        if segments_cross(first, last, lp.pts[i], lp.pts[(i + 1) % n], eps) {
            return Err("The partial fill crosses the hole boundary.".into());
        }
    }
    let pts: Vec<Vec3> = cap.iter().map(|v| m.positions[*v as usize]).collect();
    for t in mesh_fill(&pts, newell(&pts)) {
        let c = (to2(cap[t[0]]) + to2(cap[t[1]]) + to2(cap[t[2]])) / 3.0;
        if point_in_polygon(c, &lp.pts) == outside {
            return Err("The partial fill is outside the hole.".into());
        }
    }
    Ok(())
}
pub fn flood_picked_cavities(
    m: &mut BMesh,
    sel: &HoleSel,
    entire_hole: bool,
) -> OpResult<BTreeSet<usize>> {
    on_copy(m, |w| {
        let owner = directed_owner(w);
        let open: Vec<(u32, u32)> = owner
            .keys()
            .filter(|(a, b)| !owner.contains_key(&(*b, *a)))
            .map(|(a, b)| (*b, *a))
            .collect();
        let loops =
            chain_loops(&open).ok_or("Fill hole requires a closed boundary used only once.")?;
        let mut out = BTreeSet::new();
        for hole in &loops {
            let n = hole.len();
            if n < 3 {
                continue;
            }
            let selected: Vec<bool> = (0..n)
                .map(|i| {
                    let (a, b) = (hole[i], hole[(i + 1) % n]);
                    match sel {
                        HoleSel::Vertices(vs) => vs.contains(&a) && vs.contains(&b),
                        HoleSel::Edges(es) => {
                            es.contains(&edge(a, b)) || es.contains(&(a, b)) || es.contains(&(b, a))
                        }
                    }
                })
                .collect();
            let touches = match sel {
                HoleSel::Vertices(vs) => hole.iter().any(|v| vs.contains(v)),
                HoleSel::Edges(_) => selected.contains(&true),
            };
            if !touches {
                continue;
            }
            let paths = if entire_hole || !selected.contains(&false) {
                vec![hole.clone()]
            } else {
                selected_runs(hole, &selected)
            };
            for path in paths {
                if path.len() != n {
                    validate_partial_fill(w, hole, &path, &owner)?;
                }
                let template = owner
                    .get(&(path[1], path[0]))
                    .map(|f| w.facets[*f].clone())
                    .unwrap_or_else(|| Face::new(Vec::new()));
                w.facets.push(Face {
                    glide: 0,
                    ..template.with_verts(path)
                });
                out.insert(w.facets.len() - 1);
            }
        }
        if out.is_empty() {
            return Err(INVALID_SELECTION.into());
        }
        Ok(out)
    })
}
fn drop_coincident_pairs(m: &mut BMesh, axis: usize, threshold: f32) {
    let on = |v: &u32| {
        let p = m.positions[*v as usize];
        [p.x, p.y, p.z][axis].abs() <= threshold
    };
    let mut by_set: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
    for (i, f) in m.facets.iter().enumerate() {
        if !f.verts.iter().all(on) {
            continue;
        }
        let mut k = f.verts.clone();
        k.sort();
        by_set.entry(k).or_default().push(i);
    }
    let drop: BTreeSet<usize> = by_set
        .values()
        .filter(|l| l.len() >= 2)
        .flatten()
        .copied()
        .collect();
    if !drop.is_empty() {
        remove_faces(m, &drop);
        m.compact();
    }
}
pub fn mirror_axes(
    m: &mut BMesh,
    axes: [bool; 3],
    duplicate: bool,
    weld_threshold: f32,
) -> OpResult<()> {
    if !axes.contains(&true) {
        return Err("Select at least one mirror axis.".into());
    }
    on_copy(m, |w| {
        if !duplicate {
            let s = |on: bool| if on { -1.0 } else { 1.0 };
            let xf = Affine {
                x: v3(s(axes[0]), 0.0, 0.0),
                y: v3(0.0, s(axes[1]), 0.0),
                z: v3(0.0, 0.0, s(axes[2])),
                origin: Vec3::ZERO,
            };
            bake_transform(w, &xf);
            return Ok(());
        }
        for (axis, on) in axes.iter().enumerate() {
            if *on {
                let th = weld_threshold.max(1e-6);
                mirror_copy(w, axis, true, th);
                drop_coincident_pairs(w, axis, th * 2.0);
            }
        }
        Ok(())
    })
}
pub fn connected_groups(verts: &BTreeSet<u32>, links: &BTreeSet<Edge>) -> Vec<BTreeSet<u32>> {
    let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (a, b) in links {
        if verts.contains(a) && verts.contains(b) {
            adjacency.entry(*a as usize).or_default().push(*b as usize);
            adjacency.entry(*b as usize).or_default().push(*a as usize);
        }
    }
    let nodes: BTreeSet<usize> = verts.iter().map(|v| *v as usize).collect();
    network_pieces(&nodes, &|v| adjacency.get(&v).cloned().unwrap_or_default())
        .into_iter()
        .map(|g| g.into_iter().map(|v| v as u32).collect())
        .collect()
}
pub fn group_center(m: &BMesh, group: &BTreeSet<u32>) -> Vec3 {
    let pts: Vec<Vec3> = group
        .iter()
        .filter_map(|v| m.positions.get(*v as usize).copied())
        .collect();
    pts.iter().fold(Vec3::ZERO, |a, p| a + *p) / pts.len().max(1) as f32
}
pub fn rotate_individual(m: &mut BMesh, groups: &[BTreeSet<u32>], axis: Vec3, angle: f32) {
    for g in groups {
        let c = group_center(m, g);
        rotate_positions(m, g, c, axis, angle);
    }
}
pub fn scale_individual(
    m: &mut BMesh,
    groups: &[BTreeSet<u32>],
    orientation: &Frame,
    factors: Vec3,
) {
    for g in groups {
        let c = group_center(m, g);
        scale_positions(m, g, c, orientation, factors);
    }
}
#[cfg(test)]
mod tests {
    use super::super::shapes::{cuboid, sheet};
    use super::*;
    const ID: [Vec3; 3] = [v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0)];
    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).magnitude() < 1e-4
    }
    fn facet_alongside_upright(m: &BMesh, n: Vec3) -> usize {
        (0..m.facets.len())
            .find(|f| m.face_normal(*f).innerprod(n) > 0.99)
            .unwrap()
    }
    fn centre_vertex(m: &BMesh) -> u32 {
        m.positions
            .iter()
            .position(|p| near(*p, Vec3::ZERO))
            .unwrap() as u32
    }
    #[test]
    fn dissolving_two_faces_merges_them() {
        let mut m = sheet(v2(2.0, 1.0), 2);
        let mut g = sheet(v2(2.0, 2.0), 2);
        let out = dissolve_faces(&mut g, &BTreeSet::from([0, 1])).unwrap();
        g.sanity_check().unwrap();
        assert_eq!(g.facets.len(), 3);
        let f = *out.first().unwrap();
        assert_eq!(
            g.facets[f].verts.len(),
            6,
            "two quads merged keep the middle points"
        );
        assert!((g.face_area(f) - 2.0).abs() < 1e-4);
        assert!(g.face_normal(f).y > 0.99);
        let all: BTreeSet<usize> = (0..m.facets.len()).collect();
        dissolve_faces(&mut m, &all).unwrap();
        assert_eq!(m.facets.len(), 1);
    }
    #[test]
    fn dissolving_the_shared_edge_merges_its_faces() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let e = m
            .edges_of_faces(&BTreeSet::from([0]))
            .intersection(&m.edges_of_faces(&BTreeSet::from([1])))
            .copied()
            .next()
            .unwrap();
        dissolve_edges(&mut m, &BTreeSet::from([e])).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(m.facets.len(), 3);
    }
    #[test]
    fn dissolving_the_centre_vertex_merges_four_faces() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let c = centre_vertex(&m);
        let out = melt_corners(&mut m, &BTreeSet::from([c])).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(m.facets.len(), 1);
        assert_eq!(m.positions.len(), 8, "the centre vertex is gone");
        assert!(!m.positions.iter().any(|p| near(*p, Vec3::ZERO)));
        assert!((m.face_area(*out.first().unwrap()) - 4.0).abs() < 1e-4);
    }
    #[test]
    fn a_single_face_or_a_border_edge_is_rejected() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let before = m.clone();
        assert!(dissolve_faces(&mut m, &BTreeSet::from([0])).is_err());
        let border = m
            .ridges()
            .into_iter()
            .find(|e| m.edge_faces()[e].len() == 1)
            .unwrap();
        assert_eq!(
            dissolve_edges(&mut m, &BTreeSet::from([border])).unwrap_err(),
            DISSOLVE_MANIFOLD
        );
        assert_eq!(m, before, "a refused dissolve leaves the mesh untouched");
    }
    #[test]
    fn faces_off_one_plane_or_with_other_attributes_are_rejected() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let (t, s) = (
            facet_alongside_upright(&m, Vec3::UP),
            facet_alongside_upright(&m, v3(1.0, 0.0, 0.0)),
        );
        assert_eq!(
            dissolve_faces(&mut m, &BTreeSet::from([t, s])).unwrap_err(),
            DISSOLVE_PLANE
        );
        let mut g = sheet(v2(2.0, 2.0), 2);
        g.facets[1].stuff = 3;
        assert_eq!(
            dissolve_faces(&mut g, &BTreeSet::from([0, 1])).unwrap_err(),
            DISSOLVE_ATTRIBUTES
        );
    }
    #[test]
    fn a_ring_of_faces_is_not_a_disk() {
        let mut m = sheet(v2(3.0, 3.0), 3);
        let ring: BTreeSet<usize> = (0..9).filter(|f| *f != 4).collect();
        assert!(dissolve_faces(&mut m, &ring).is_err());
    }
    #[test]
    fn separate_selections_become_separate_regions() {
        let mut m = sheet(v2(4.0, 1.0), 4);
        let out = dissolve_faces(&mut m, &BTreeSet::from([0, 1, 3, 7])).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(m.facets.len(), 14);
    }
    #[test]
    fn a_two_valent_vertex_leaves_its_loops() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let e = m
            .edges_of_faces(&BTreeSet::from([0]))
            .intersection(&m.edges_of_faces(&BTreeSet::from([1])))
            .copied()
            .next()
            .unwrap();
        let mid = super::super::topo::split_edges(&mut m, &BTreeSet::from([e]), 1);
        let v = *mid.first().unwrap();
        let faces = m.facets.len();
        melt_corners(&mut m, &BTreeSet::from([v])).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(m.facets.len(), faces, "faces stay separate");
        assert!(m.facets.iter().all(|f| f.verts.len() == 4));
        assert_eq!(m.positions.len(), 9);
    }
    #[test]
    fn shift_in_per_each_room() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = facet_alongside_upright(&m, Vec3::UP);
        let verts = m.verts_of_faces(&BTreeSet::from([top]));
        let p0 = m.positions[*verts.first().unwrap() as usize];
        shift_items(
            &mut m,
            &verts,
            &BTreeSet::new(),
            v3(1.0, 0.0, 0.0),
            OffsetSpace::Local,
            ID,
            false,
        )
        .unwrap();
        assert!(near(
            m.positions[*verts.first().unwrap() as usize],
            p0 + v3(1.0, 0.0, 0.0)
        ));
        let basis = [v3(2.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0)];
        shift_items(
            &mut m,
            &verts,
            &BTreeSet::new(),
            v3(1.0, 0.0, 0.0),
            OffsetSpace::Global,
            basis,
            false,
        )
        .unwrap();
        assert!(near(
            m.positions[*verts.first().unwrap() as usize],
            p0 + v3(1.5, 0.0, 0.0)
        ));
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let side = facet_alongside_upright(&c, v3(1.0, 0.0, 0.0));
        let sv = c.verts_of_faces(&BTreeSet::from([side]));
        let q0 = c.positions[*sv.first().unwrap() as usize];
        shift_items(
            &mut c,
            &sv,
            &BTreeSet::new(),
            v3(0.0, 0.5, 0.0),
            OffsetSpace::Normal,
            ID,
            false,
        )
        .unwrap();
        assert!(
            near(
                c.positions[*sv.first().unwrap() as usize],
                q0 + v3(0.5, 0.0, 0.0)
            ),
            "vertex mode uses the faces around"
        );
        let mut d = cuboid(v3(1.0, 1.0, 1.0));
        let (t, s) = (
            facet_alongside_upright(&d, Vec3::UP),
            facet_alongside_upright(&d, v3(1.0, 0.0, 0.0)),
        );
        let fs = BTreeSet::from([t, s]);
        let vs = d.verts_of_faces(&fs);
        let before = d.positions.clone();
        shift_items(
            &mut d,
            &vs,
            &fs,
            v3(0.0, 1.0, 0.0),
            OffsetSpace::Normal,
            ID,
            true,
        )
        .unwrap();
        let shared = *vs
            .iter()
            .find(|v| d.facets[t].verts.contains(v) && d.facets[s].verts.contains(v))
            .unwrap();
        assert!(near(
            d.positions[shared as usize],
            before[shared as usize] + v3(0.5, 0.5, 0.0)
        ));
        let only_top = *vs.iter().find(|v| !d.facets[s].verts.contains(v)).unwrap();
        assert!(near(
            d.positions[only_top as usize],
            before[only_top as usize] + Vec3::UP
        ));
        assert!(
            shift_items(
                &mut d,
                &vs,
                &fs,
                Vec3::UP,
                OffsetSpace::Global,
                [Vec3::ZERO; 3],
                false
            )
            .is_err()
        );
    }
    #[test]
    fn upright_frame_fits_this_pristine() {
        let f = upright_frame(v3(0.0, 2.0, 0.0));
        assert!(
            near(f.x, v3(-1.0, 0.0, 0.0)) && near(f.y, Vec3::UP) && near(f.z, v3(0.0, 0.0, -1.0))
        );
        let f = upright_frame(v3(0.0, 0.0, 1.0));
        assert!(near(f.y, v3(0.0, 0.0, 1.0)) && near(f.x, v3(1.0, 0.0, 0.0)));
        assert_eq!(upright_frame(Vec3::ZERO), Frame::IDENTITY);
    }
    #[test]
    fn invert_colour_smoothing_and_grow() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        assert_eq!(invert_faces(&m, &BTreeSet::from([0, 1])).len(), 4);
        assert_eq!(invert_vertices(&m, &BTreeSet::from([0])).len(), 7);
        assert_eq!(invert_edges(&m, &BTreeSet::from([m.ridges()[0]])).len(), 11);
        m.facets[2].tint = [255, 0, 0, 255];
        m.facets[4].tint = [255, 0, 0, 255];
        let ElementSel::Faces(red) = grab_via_tint(&m, &ElementSel::Faces(BTreeSet::from([2])))
        else {
            panic!()
        };
        assert_eq!(red, BTreeSet::from([2, 4]));
        m.facets[0].color_corners = vec![[255, 0, 0, 255], [255; 4], [255; 4], [255; 4]];
        let v = m.facets[0].verts[0];
        let ElementSel::Vertices(vs) = grab_via_tint(
            &m,
            &ElementSel::Vertices(m.facets[2].verts.iter().copied().collect()),
        ) else {
            panic!()
        };
        assert!(vs.contains(&v));
        m.facets[1].glide = 2;
        m.facets[3].glide = 2;
        assert_eq!(
            grab_via_gliding_cluster(&m, &BTreeSet::from([1])),
            BTreeSet::from([1, 3])
        );
        let top = facet_alongside_upright(&m, Vec3::UP);
        assert_eq!(
            expand_facets_via_inclination(&m, &BTreeSet::from([top]), 45.0).len(),
            1,
            "sides are 90° off"
        );
        assert_eq!(
            expand_facets_via_inclination(&m, &BTreeSet::from([top]), 90.0).len(),
            5
        );
        let g = sheet(v2(3.0, 3.0), 3);
        let c = BTreeSet::from([4usize]);
        assert_eq!(
            expand_facets_via_inclination(&g, &c, 0.0).len(),
            5,
            "flat grid: the four edge neighbours"
        );
        let tv = m.verts_of_faces(&BTreeSet::from([top]));
        let one = BTreeSet::from([*tv.first().unwrap()]);
        assert_eq!(
            expand_corners_via_inclination(&m, &one, 10.0).len(),
            4,
            "every face around a corner is a seed"
        );
        let e = BTreeSet::from([m
            .edges_of_faces(&BTreeSet::from([top]))
            .into_iter()
            .next()
            .unwrap()]);
        assert_eq!(expand_ridges_via_inclination(&m, &e, 10.0).len(), 5);
    }
    #[test]
    fn extrude_methods() {
        for method in [
            ExtrudeMethod::FaceNormal,
            ExtrudeMethod::VertexNormal,
            ExtrudeMethod::IndividualFaces,
        ] {
            let mut m = cuboid(v3(1.0, 1.0, 1.0));
            let (t, s) = (
                facet_alongside_upright(&m, Vec3::UP),
                facet_alongside_upright(&m, v3(1.0, 0.0, 0.0)),
            );
            let caps =
                pull_out_picked_approach(&mut m, &BTreeSet::from([t, s]), 0.5, method, false)
                    .unwrap();
            m.sanity_check().unwrap();
            assert!(m.is_closed(), "{method:?}");
            assert_eq!(caps, BTreeSet::from([t, s]));
            let walls = m.facets.len() - 6;
            assert_eq!(
                walls,
                if method == ExtrudeMethod::IndividualFaces {
                    8
                } else {
                    6
                },
                "{method:?}"
            );
            if method == ExtrudeMethod::IndividualFaces {
                assert!(m.face_points(t).iter().all(|p| (p.y - 1.5).abs() < 1e-4));
            }
        }
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = facet_alongside_upright(&m, Vec3::UP);
        pull_out_picked_approach(
            &mut m,
            &BTreeSet::from([t]),
            1.0,
            ExtrudeMethod::VertexNormal,
            false,
        )
        .unwrap();
        let p = m.face_points(t)[0];
        assert!((p.y - (1.0 + 1.0 / 3f32.sqrt())).abs() < 1e-4, "{p:?}");
        let before = m.clone();
        assert!(
            pull_out_picked_approach(
                &mut m,
                &BTreeSet::from([t]),
                0.0,
                ExtrudeMethod::FaceNormal,
                false
            )
            .is_err()
        );
        assert_eq!(m, before);
        pull_out_picked_approach(
            &mut m,
            &BTreeSet::from([t]),
            0.0,
            ExtrudeMethod::FaceNormal,
            true,
        )
        .unwrap();
    }
    #[test]
    fn pull_out_ridges_solo_each() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let border: BTreeSet<Edge> = m
            .ridges()
            .into_iter()
            .filter(|e| m.edge_faces()[e].len() == 1)
            .take(2)
            .collect();
        let mut g = m.clone();
        let out = pull_out_ridges_grouped(&mut m, &border, 0.5, Vec3::ZERO, false);
        m.sanity_check().unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(m.positions.len(), 9 + 4);
        pull_out_ridges_grouped(&mut g, &border, 0.5, Vec3::ZERO, true);
        g.sanity_check().unwrap();
        assert!(g.positions.len() <= 9 + 4);
    }
    #[test]
    fn group_inset_keeps_one_rim() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let all: BTreeSet<usize> = (0..4).collect();
        let inner = inset_center_grouped(&mut m, &all, 0.25, false).unwrap();
        m.sanity_check().unwrap();
        assert_eq!(inner, all);
        assert_eq!(m.facets.len(), 4 + 8, "one border quad per outline edge");
        let area: f32 = inner.iter().map(|f| m.face_area(*f)).sum();
        assert!((area - 1.5 * 1.5).abs() < 1e-4, "{area}");
        assert!(
            near(m.positions[centre_vertex(&m) as usize], Vec3::ZERO),
            "interior vertex stays"
        );
        let total: f32 = m.total_area();
        assert!((total - 4.0).abs() < 1e-4);
        let mut i = sheet(v2(2.0, 2.0), 2);
        inset_center_grouped(&mut i, &all, 0.1, true).unwrap();
        assert_eq!(i.facets.len(), 4 + 16);
        let mut w = sheet(v2(2.0, 2.0), 2);
        assert!(inset_center_grouped(&mut w, &all, 1.5, false).is_err());
        assert_eq!(w, sheet(v2(2.0, 2.0), 2));
    }
    #[test]
    fn cluster_recess_rejects_one_creased_facet() {
        let mut m = sheet(v2(3.0, 3.0), 3);
        let v = m
            .positions
            .iter()
            .position(|p| near(*p, v3(-0.5, 0.0, -0.5)))
            .unwrap();
        m.positions[v] = v3(-0.5, 0.0, -1.3);
        m.sanity_check().unwrap();
        let before = m.clone();
        let all: BTreeSet<usize> = (0..9).collect();
        assert!(inset_center_grouped(&mut m, &all, 0.5, false).is_err());
        assert_eq!(m, before);
        assert!(inset_center_grouped(&mut m, &all, 0.1, false).is_ok());
    }
    #[test]
    fn group_inset_on_a_closed_cube_top_pair() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let t = facet_alongside_upright(&m, Vec3::UP);
        let fs = super::super::ops::subdivide_selection(&mut m, &BTreeSet::from([t]));
        let tops: BTreeSet<usize> = fs
            .into_iter()
            .filter(|f| m.face_normal(*f).y > 0.99)
            .collect();
        inset_center_grouped(&mut m, &tops, 0.1, false).unwrap();
        m.sanity_check().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 2.0).abs() < 1e-4);
    }
    #[test]
    fn collapse_to_first_keeps_the_target_position() {
        let mut m = sheet(v2(2.0, 2.0), 2);
        let c = centre_vertex(&m);
        let other = m.facets[0]
            .loop_edges()
            .find_map(|(a, b)| (a == c).then_some(b))
            .unwrap();
        let p = m.positions[other as usize];
        let v = condense_corners_into(&mut m, &BTreeSet::from([c, other]), Some(other)).unwrap();
        m.sanity_check().unwrap();
        assert!(near(m.positions[v as usize], p));
    }
    #[test]
    fn partial_and_entire_fill() {
        let mut ring = sheet(v2(3.0, 3.0), 3);
        super::super::ops::discard_facets(&mut ring, &BTreeSet::from([4]));
        let hole: Vec<u32> = {
            let o = directed_owner(&ring);
            let open: Vec<(u32, u32)> = o
                .keys()
                .filter(|(a, b)| !o.contains_key(&(*b, *a)))
                .map(|(a, b)| (*b, *a))
                .collect();
            chain_loops(&open)
                .unwrap()
                .into_iter()
                .find(|l| {
                    l.len() == 4 && l.iter().all(|v| ring.positions[*v as usize].x.abs() < 0.6)
                })
                .unwrap()
        };
        let three = BTreeSet::from([hole[0], hole[1], hole[2]]);
        let mut part = ring.clone();
        let out =
            flood_picked_cavities(&mut part, &HoleSel::Vertices(three.clone()), false).unwrap();
        part.sanity_check().unwrap();
        let f = *out.first().unwrap();
        assert_eq!(part.facets[f].verts.len(), 3);
        assert!((part.face_area(f) - 0.5).abs() < 1e-4);
        assert!(part.face_normal(f).y > 0.99);
        let mut whole = ring.clone();
        let out = flood_picked_cavities(&mut whole, &HoleSel::Vertices(three), true).unwrap();
        assert_eq!(whole.facets[*out.first().unwrap()].verts.len(), 4);
        let mut l = sheet(v2(5.0, 5.0), 5);
        super::super::ops::discard_facets(&mut l, &BTreeSet::from([6, 7, 11]));
        let o = directed_owner(&l);
        let open: Vec<(u32, u32)> = o
            .keys()
            .filter(|(a, b)| !o.contains_key(&(*b, *a)))
            .map(|(a, b)| (*b, *a))
            .collect();
        let reflex = |m: &BMesh, v: u32| near(m.positions[v as usize], v3(-0.5, 0.0, -0.5));
        let lp = chain_loops(&open)
            .unwrap()
            .into_iter()
            .find(|lp| lp.iter().any(|v| reflex(&l, *v)))
            .unwrap();
        let k = lp.len();
        let ri = (0..k).find(|i| reflex(&l, lp[*i])).unwrap();
        let sel = BTreeSet::from([lp[(ri + k - 1) % k], lp[ri], lp[(ri + 1) % k]]);
        let before = l.clone();
        assert!(flood_picked_cavities(&mut l, &HoleSel::Vertices(sel), false).is_err());
        assert_eq!(l, before);
        let mut s = sheet(v2(2.0, 2.0), 2);
        let corner_run = |m: &BMesh, c: Vec3| -> BTreeSet<u32> {
            let o = directed_owner(m);
            let open: Vec<(u32, u32)> = o
                .keys()
                .filter(|(a, b)| !o.contains_key(&(*b, *a)))
                .map(|(a, b)| (*b, *a))
                .collect();
            let lp = chain_loops(&open).unwrap().remove(0);
            let k = lp.len();
            let i = (0..k)
                .find(|i| near(m.positions[lp[*i] as usize], c))
                .unwrap();
            BTreeSet::from([lp[(i + k - 1) % k], lp[i], lp[(i + 1) % k]])
        };
        let flap = corner_run(&s, v3(1.0, 0.0, 1.0));
        assert!(flood_picked_cavities(&mut s, &HoleSel::Vertices(flap), false).is_err());
        super::super::ops::discard_facets(&mut s, &BTreeSet::from([3]));
        let notch = corner_run(&s, Vec3::ZERO);
        let out = flood_picked_cavities(&mut s, &HoleSel::Vertices(notch), false).unwrap();
        s.sanity_check().unwrap();
        let f = *out.first().unwrap();
        assert!(s.face_normal(f).y > 0.99 && (s.face_area(f) - 0.5).abs() < 1e-4);
    }
    #[test]
    fn reflect_in_position_plus_replicate() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.displace(v3(0.5, 0.0, 0.5));
        let v0 = m.volume();
        let mut a = m.clone();
        mirror_axes(&mut a, [true, false, false], false, 1e-3).unwrap();
        assert!((a.volume() - v0).abs() < 1e-4, "still outward");
        mirror_axes(&mut a, [true, true, true], false, 1e-3).unwrap();
        assert!((a.volume() - v0).abs() < 1e-4);
        let mut d = m.clone();
        mirror_axes(&mut d, [true, false, true], true, 1e-3).unwrap();
        d.sanity_check().unwrap();
        assert!(d.is_closed());
        assert!((d.volume() - 4.0).abs() < 1e-4, "{}", d.volume());
        assert!(mirror_axes(&mut d, [false; 3], true, 1e-3).is_err());
        let mut q = m.clone();
        let base = q.positions.len() as u32;
        for p in [
            v3(5.0, 0.0, 0.0),
            v3(6.0, 0.0, 0.0),
            v3(6.0, 0.0, 1.0),
            v3(5.0, 0.0, 1.0),
        ] {
            q.add_vert(p);
        }
        q.add_face(vec![base, base + 1, base + 2, base + 3]);
        q.add_face(vec![base + 3, base + 2, base + 1, base]);
        mirror_axes(&mut q, [true, false, false], true, 1e-3).unwrap();
        assert_eq!(
            q.facets.len(),
            10 + 4,
            "two cubes joined + both double-sided quads"
        );
    }
    #[test]
    fn solo_roots_clusters() {
        let mut m = sheet(v2(3.0, 1.0), 3);
        let fs = BTreeSet::from([0usize, 2]);
        let verts = m.verts_of_faces(&fs);
        let groups = connected_groups(&verts, &m.edges_of_faces(&fs));
        assert_eq!(groups.len(), 2);
        let centres: Vec<Vec3> = groups.iter().map(|g| group_center(&m, g)).collect();
        scale_individual(&mut m, &groups, &Frame::IDENTITY, v3(0.5, 1.0, 0.5));
        for (g, c) in groups.iter().zip(&centres) {
            assert!(
                near(group_center(&m, g), *c),
                "each group scales about its own centre"
            );
        }
        rotate_individual(&mut m, &groups, Vec3::UP, std::f32::consts::FRAC_PI_2);
        for (g, c) in groups.iter().zip(&centres) {
            assert!(near(group_center(&m, g), *c));
        }
        let q = sheet(v2(1.0, 1.0), 1);
        let f = &q.facets[0];
        let es = BTreeSet::from([edge(f.verts[0], f.verts[1]), edge(f.verts[2], f.verts[3])]);
        assert_eq!(connected_groups(&q.verts_of_edges(&es), &es).len(), 2);
    }
    #[test]
    fn dissolve_two_valent_run_keeps_faces_joined() {
        let mut m = BMesh::default();
        let pts = [
            (0.0, 0.0),
            (1.0, 1.0),
            (2.0, 1.0),
            (3.0, 0.0),
            (0.0, 3.0),
            (3.0, 3.0),
        ];
        for (x, z) in pts {
            m.positions.push(v3(x, 0.0, z));
        }
        let (p, v1, v2, q, a, b) = (0, 1, 2, 3, 4, 5);
        m.facets.push(Face::new(vec![p, v1, v2, q]));
        m.facets.push(Face::new(vec![p, a, b, q, v2, v1]));
        assert!(m.sanity_check().is_ok());
        let sel: BTreeSet<u32> = [v1, v2].into_iter().collect();
        melt_corners(&mut m, &sel).unwrap();
        assert!(m.sanity_check().is_ok());
        let ef = m.edge_faces();
        let open = ef.values().filter(|fs| fs.len() == 1).count();
        assert_eq!(open, 4);
        let gone = v3(1.0, 0.0, 1.0);
        assert!(
            m.facets
                .iter()
                .all(|f| f.verts.iter().all(|v| m.positions[*v as usize] != gone))
        );
    }
}
