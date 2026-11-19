//! Xogot Modeler geometry / selection operations on the builder's `BMesh` (docs/port/06-xogot.md,
//! package P1). Ports of xogot-rust (MIT, Xibbon Inc. / Xogot Modeler):
//! `modeler_kit/src/topology_operations.rs` (dissolve, collapse, extrude edges),
//! `modeler_kit/src/mesh_operations.rs` (extrude methods), `modeler_kit/src/mesh_inset.rs`
//! (group inset), `modeler_kit/src/topology_selection.rs` + `mesh_topology.rs` (invert, grow by
//! angle), `modeler_kit/src/mesh_attributes.rs` (select by colour), `modeler_kit/src/polygon_mesh.rs`
//! (partial hole fill), `editor_core/src/geometry_actions.rs` (offset elements, select by
//! smoothing group), `xogot_modeler/src/shared/object_actions.rs` (mirror) and
//! `editor_core/src/viewport_geometry.rs` (individual origins).
//!
//! xogot-rust works on triangle-list faces over per-corner records grouped into shared vertices;
//! here faces are n-gon loops over shared vertices, so record / group bookkeeping disappears and
//! a merged region is one n-gon instead of a re-triangulation. Fallible operations work on a
//! copy and only write it back when the result validates, so an `Err` leaves the mesh untouched.
//!
//! Gap fixes over xogot-rust: offset in Normal space works in vertex / edge mode and has a
//! per-face option; mirror honours `duplicate` (xogot-rust ignores it); individual extrude keeps
//! the mesh watertight (no detached seams); individual origins work per connected group;
//! dissolving a 2-valent vertex removes it from its loops instead of being refused.

use super::edit_ops::{Affine, bake_transform, loop_plane, point_in_polygon, segments_cross};
use super::loops::mirror_copy;
use super::model::{BMesh, Edge, Face, edge, newell};
use super::topo::{chain_loops, directed_owner, region_boundary};
use super::tri::triangulate;
use super::xform::{Frame, rotate_positions, scale_positions};
use crate::math::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Error text for a refused operation (shown to the user).
pub type OpResult<T> = Result<T, String>;

// ------------------------------------------------------------------ shared helpers

/// Runs `f` on a copy of the mesh; keeps the copy only when `f` succeeds and it validates.
fn on_copy<T>(m: &mut BMesh, f: impl FnOnce(&mut BMesh) -> OpResult<T>) -> OpResult<T> {
    let mut work = m.clone();
    let out = f(&mut work)?;
    work.validate().map_err(|e| format!("The operation would leave an invalid mesh ({e})."))?;
    *m = work;
    Ok(out)
}

/// Unnormalised Newell normal of face `f` (zero for degenerate faces, unlike `face_normal`).
fn raw_normal(m: &BMesh, f: usize) -> Vec3 {
    newell(&m.face_points(f))
}

fn unit_or_zero(v: Vec3) -> Vec3 {
    let l = v.length();
    if l > 1e-10 { v / l } else { Vec3::ZERO }
}

/// Edge-adjacent faces of every face (`MeshTopology.face_neighbors`).
fn face_neighbors(m: &BMesh) -> Vec<BTreeSet<usize>> {
    let mut out = vec![BTreeSet::new(); m.faces.len()];
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

/// Connected components of `nodes` under `adjacency`, each grown breadth first from its lowest
/// node, in order of their lowest node (port of `MeshOperations::graph_components`).
fn graph_components(nodes: &BTreeSet<usize>, adjacency: &dyn Fn(usize) -> Vec<usize>) -> Vec<BTreeSet<usize>> {
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
    m.faces.retain(|_| {
        let keep = !faces.contains(&i);
        i += 1;
        keep
    });
}

/// Corner index of vertex `v` in face `f`.
fn corner(f: &Face, v: u32) -> Option<usize> {
    f.verts.iter().position(|x| *x == v)
}

/// Faces may merge / inset together (`mesh_inset.rs same_attributes`).
fn same_attributes(a: &Face, b: &Face) -> bool {
    a.smooth == b.smooth && a.material == b.material && a.uv_manual == b.uv_manual && a.uv == b.uv && a.uv_axis == b.uv_axis
}

// ------------------------------------------------------------------ 1. dissolve

const DISSOLVE_TWO_FACES: &str = "Dissolve requires at least two faces.";
const DISSOLVE_ATTRIBUTES: &str = "Dissolve cannot cross faces with different attributes.";
const DISSOLVE_ZERO_AREA: &str = "Dissolve cannot create a face with zero area.";
const DISSOLVE_DIRECTION: &str = "Dissolve cannot keep the face direction.";
const DISSOLVE_MANIFOLD: &str = "Dissolve requires a manifold disk.";
const DISSOLVE_PLANE: &str = "Dissolve requires faces that are on one plane.";
const DISSOLVE_BRANCHES: &str = "Dissolve requires one closed boundary without branches.";
const INVALID_SELECTION: &str = "The selection is not valid for this operation.";

/// A connected set of faces to merge (`TopologyEdit.DissolveRegion`); `verts` are the selected
/// vertices it dissolves (vertex dissolve only), which must end up inside the region.
struct DissolveRegion {
    faces: BTreeSet<usize>,
    verts: BTreeSet<u32>,
}

/// `max(diagonal * 1e-5, 1e-5)` of the points' bounding box (`dissolve_tolerance`).
fn dissolve_tolerance(points: &[Vec3]) -> f32 {
    let Some(&first) = points.first() else { return 1e-5 };
    let (lo, hi) = points.iter().fold((first, first), |(lo, hi), p| {
        (v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)), v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)))
    });
    ((hi - lo).length() * 1e-5).max(1e-5)
}

/// Every vertex's fan must be a manifold disk or half-disk: zero or two boundary edges and the
/// incident faces connected around it (`check_dissolve_vertex_fans`).
fn check_vertex_fans(m: &BMesh, verts: &BTreeSet<u32>, vf: &[Vec<usize>], ef: &HashMap<Edge, Vec<usize>>) -> OpResult<()> {
    for &v in verts {
        let incident: BTreeSet<usize> = vf[v as usize].iter().copied().collect();
        let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut boundary = 0;
        let mut seen = BTreeSet::new();
        for &f in &incident {
            for (a, b) in m.faces[f].loop_edges() {
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
        let comps = graph_components(&incident, &|f| adjacency.get(&f).cloned().unwrap_or_default());
        if comps.len() > 1 {
            return Err(DISSOLVE_MANIFOLD.into());
        }
    }
    Ok(())
}

/// No region vertex may lie inside a region edge unless it shares a face with that edge
/// (`check_dissolve_t_junctions`).
fn check_t_junctions(m: &BMesh, verts: &BTreeSet<u32>, edges: &BTreeSet<Edge>, vf: &[Vec<usize>], ef: &HashMap<Edge, Vec<usize>>, tol: f32) -> OpResult<()> {
    for e in edges {
        let (a, b) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
        let ab = b - a;
        let l2 = ab.dot(ab);
        if l2 <= tol * tol {
            continue;
        }
        let edge_faces = ef.get(e).cloned().unwrap_or_default();
        for &v in verts {
            if v == e.0 || v == e.1 || vf[v as usize].iter().any(|f| edge_faces.contains(f)) {
                continue;
            }
            let p = m.positions[v as usize];
            let t = (p - a).dot(ab) / l2;
            if 0.0 < t && t < 1.0 && (p - (a + ab * t)).length() <= tol {
                return Err(DISSOLVE_MANIFOLD.into());
            }
        }
    }
    Ok(())
}

/// Validates a region and returns its outline loop and the plane normal
/// (`validate_dissolve` + `dissolve_boundary` + `dissolve_plane`).
fn validate_region(m: &BMesh, r: &DissolveRegion, vf: &[Vec<usize>], ef: &HashMap<Edge, Vec<usize>>) -> OpResult<Vec<u32>> {
    if r.faces.len() < 2 {
        return Err(DISSOLVE_TWO_FACES.into());
    }
    let first = *r.faces.first().unwrap();
    if r.faces.iter().any(|f| !same_attributes(&m.faces[*f], &m.faces[first])) {
        return Err(DISSOLVE_ATTRIBUTES.into());
    }
    let region_verts: BTreeSet<u32> = m.verts_of_faces(&r.faces);
    let region_edges: BTreeSet<Edge> = m.edges_of_faces(&r.faces);
    let boundary = region_boundary(m, &r.faces);
    if boundary.is_empty() {
        return Err(DISSOLVE_MANIFOLD.into());
    }
    check_vertex_fans(m, &region_verts, vf, ef)?;
    let pts: Vec<Vec3> = region_verts.iter().map(|v| m.positions[*v as usize]).collect();
    let tol = dissolve_tolerance(&pts);
    check_t_junctions(m, &region_verts, &region_edges, vf, ef, tol)?;
    let loops = chain_loops(&boundary).ok_or(DISSOLVE_BRANCHES)?;
    if loops.len() != 1 {
        return Err(DISSOLVE_BRANCHES.into());
    }
    let outline = loops.into_iter().next().unwrap();
    if !r.verts.iter().all(|v| !outline.contains(v)) {
        return Err(DISSOLVE_MANIFOLD.into());
    }
    let euler = region_verts.len() as isize - region_edges.len() as isize + r.faces.len() as isize;
    if euler != 1 {
        return Err(DISSOLVE_BRANCHES.into());
    }
    // The plane of the first face with area, through the lowest face's first vertex.
    let Some(normal) = r.faces.iter().map(|f| raw_normal(m, *f)).find(|n| n.dot(*n) > 1e-12) else {
        return Err(DISSOLVE_ZERO_AREA.into());
    };
    let normal = normal.normalized();
    let origin = m.positions[m.faces[first].verts[0] as usize];
    if pts.iter().any(|p| (*p - origin).dot(normal).abs() > tol) {
        return Err(DISSOLVE_PLANE.into());
    }
    for f in &r.faces {
        let n = raw_normal(m, *f);
        if n.dot(n) <= 1e-12 {
            return Err(DISSOLVE_ZERO_AREA.into());
        }
        if n.dot(normal) <= 0.0 {
            return Err(DISSOLVE_DIRECTION.into());
        }
    }
    let projected: Vec<Vec3> = outline.iter().map(|v| {
        let p = m.positions[*v as usize];
        p - normal * (p - origin).dot(normal)
    }).collect();
    let bn = newell(&projected);
    if outline.len() < 3 || bn.length() <= tol * tol {
        return Err(DISSOLVE_ZERO_AREA.into());
    }
    if bn.dot(normal) <= 0.0 {
        return Err(DISSOLVE_DIRECTION.into());
    }
    Ok(outline)
}

/// Replaces each region by one n-gon over its outline (the lowest face's attributes; corner
/// colours follow their corners). Untouched faces keep their order; the new faces follow and
/// are returned (`TopologyEdit::dissolve`).
fn dissolve_regions(m: &mut BMesh, regions: Vec<DissolveRegion>) -> OpResult<BTreeSet<usize>> {
    if regions.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let vf = m.vert_faces();
    let ef = m.edge_faces();
    let mut occupied = BTreeSet::new();
    let mut replacements: Vec<(usize, Face)> = Vec::new();
    for r in &regions {
        if !occupied.is_disjoint(&r.faces) {
            return Err("Dissolve regions must not overlap.".into());
        }
        occupied.extend(r.faces.iter().copied());
        let outline = validate_region(m, r, &vf, &ef)?;
        let first = *r.faces.first().unwrap();
        let mut face = m.faces[first].with_verts(outline.clone());
        if r.faces.iter().any(|f| m.faces[*f].corner_colors().is_some()) {
            // Each outline corner takes the colour of the face owning its outgoing edge.
            let owner = directed_owner(m);
            face.color_corners = (0..outline.len())
                .map(|i| {
                    let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                    let f = &m.faces[owner[&(a, b)]];
                    corner(f, a).map(|c| f.corner_color(c)).unwrap_or(f.color)
                })
                .collect();
        }
        replacements.push((first, face));
    }
    remove_faces(m, &occupied);
    replacements.sort_by_key(|(f, _)| *f);
    let mut out = BTreeSet::new();
    for (_, face) in replacements {
        out.insert(m.faces.len());
        m.faces.push(face);
    }
    m.compact();
    Ok(out)
}

/// Merges each connected group of the selected faces into one face.
/// Port of xogot-rust `modeler_kit/src/topology_operations.rs` `MeshOperations::dissolve_faces`.
/// Refused (with the reason) when a group is a single face, crosses attributes, is not planar
/// or not a disk with one outline. Returns the merged faces.
pub fn dissolve_faces(m: &mut BMesh, faces: &BTreeSet<usize>) -> OpResult<BTreeSet<usize>> {
    if faces.is_empty() || faces.iter().any(|f| *f >= m.faces.len()) {
        return Err(INVALID_SELECTION.into());
    }
    let nb = face_neighbors(m);
    let regions = graph_components(faces, &|f| nb[f].iter().copied().collect())
        .into_iter()
        .map(|faces| DissolveRegion { faces, verts: BTreeSet::new() })
        .collect();
    on_copy(m, |w| dissolve_regions(w, regions))
}

/// Removes the selected edges by merging the two faces of each (faces joined by selected edges
/// merge into one). Port of `topology_operations.rs` `MeshOperations::dissolve_edges`.
/// Every edge must be shared by exactly two faces. Returns the merged faces.
pub fn dissolve_edges(m: &mut BMesh, edges: &BTreeSet<Edge>) -> OpResult<BTreeSet<usize>> {
    if edges.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let ef = m.edge_faces();
    let mut faces = BTreeSet::new();
    let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for e in edges {
        let e = edge(e.0, e.1);
        let Some(fs) = ef.get(&e) else { return Err(INVALID_SELECTION.into()) };
        let [a, b] = fs.as_slice() else { return Err(DISSOLVE_MANIFOLD.into()) };
        faces.insert(*a);
        faces.insert(*b);
        adjacency.entry(*a).or_default().push(*b);
        adjacency.entry(*b).or_default().push(*a);
    }
    let regions = graph_components(&faces, &|f| adjacency.get(&f).cloned().unwrap_or_default())
        .into_iter()
        .map(|faces| DissolveRegion { faces, verts: BTreeSet::new() })
        .collect();
    on_copy(m, |w| dissolve_regions(w, regions))
}

/// Removes the selected vertices. Port of `topology_operations.rs`
/// `MeshOperations::dissolve_vertices`: the faces around each connected group of selected
/// vertices merge into one face that no longer contains them. Gap fix: a selected vertex with
/// exactly two edges (a point in the middle of a straight run, also on the mesh border) is
/// removed from its faces' loops instead (the faces stay separate), where xogot-rust refuses
/// border vertices. Returns the merged faces (vertices removed from loops add none).
pub fn dissolve_vertices(m: &mut BMesh, verts: &BTreeSet<u32>) -> OpResult<BTreeSet<usize>> {
    if verts.is_empty() || verts.iter().any(|v| *v as usize >= m.positions.len()) {
        return Err(INVALID_SELECTION.into());
    }
    on_copy(m, |w| {
        let vf = w.vert_faces();
        // Two-valent vertices: drop them from their loops.
        let mut neighbours: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for e in w.edges() {
            neighbours.entry(e.0).or_default().insert(e.1);
            neighbours.entry(e.1).or_default().insert(e.0);
        }
        let (chain, mut merge): (BTreeSet<u32>, BTreeSet<u32>) =
            verts.iter().copied().partition(|v| neighbours.get(v).is_some_and(|n| n.len() == 2));
        // Remove each 2-valent vertex from all of its faces at once, checked against the faces
        // as they are now: a run of adjacent selected 2-valent vertices shrinks faces one corner
        // at a time, and removing a vertex from only some of its faces would split them apart
        // (a T-junction). A vertex that would drop any face below 3 corners is merged instead.
        let mut removed = false;
        for v in &chain {
            let fs = &vf[*v as usize];
            if !fs.iter().all(|f| w.faces[*f].verts.len() >= 4 && corner(&w.faces[*f], *v).is_some()) {
                merge.insert(*v);
                continue;
            }
            for &f in fs {
                let face = &mut w.faces[f];
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
        let incident: BTreeSet<usize> = merge.iter().flat_map(|v| vf[*v as usize].iter().copied()).collect();
        if incident.is_empty() {
            return Err(INVALID_SELECTION.into());
        }
        let nb = face_neighbors(w);
        let regions: Vec<DissolveRegion> = graph_components(&incident, &|f| nb[f].iter().copied().collect())
            .into_iter()
            .map(|faces| {
                let verts = merge.iter().copied().filter(|v| vf[*v as usize].iter().any(|f| faces.contains(f))).collect();
                DissolveRegion { faces, verts }
            })
            .collect();
        dissolve_regions(w, regions)
    })
}

// ------------------------------------------------------------------ 2. offset elements

/// Axes an offset is measured along (`HandleOrientation` for `offsetSpace`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OffsetSpace {
    /// World axes (converted to mesh-local through the node basis).
    #[default]
    Global,
    /// The mesh's own (node-local) axes.
    Local,
    /// The selection's normal frame: y along the normal (`MeshGeometry::normal_frame`).
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

/// Orthonormal frame with `y` along `normal` (port of `MeshGeometry::normal_frame`); identity
/// for a zero normal.
pub fn normal_frame(normal: Vec3) -> Frame {
    if normal.dot(normal) <= 1e-12 {
        return Frame::IDENTITY;
    }
    let y = normal.normalized();
    let reference = if y.y.abs() < 0.95 { Vec3::UP } else { v3(0.0, 0.0, 1.0) };
    let x = reference.cross(y).normalized();
    let z = x.cross(y).normalized();
    Frame { x, y, z }
}

/// Inverse of the 3×3 basis with columns `b` applied to `d`; `None` when singular.
fn solve_basis(b: [Vec3; 3], d: Vec3) -> Option<Vec3> {
    let det = b[0].dot(b[1].cross(b[2]));
    if det.abs() <= 1e-12 {
        return None;
    }
    // Rows of the inverse are the cross products of the columns over the determinant.
    let r0 = b[1].cross(b[2]);
    let r1 = b[2].cross(b[0]);
    let r2 = b[0].cross(b[1]);
    Some(v3(r0.dot(d), r1.dot(d), r2.dot(d)) / det)
}

/// Moves the selected vertices by `offset` measured in `space` (port of
/// `editor_core/src/geometry_actions.rs` `apply_offset_elements` and the
/// `xogot_modeler/src/shared/editor_state.rs` `geometry_offset` orientation).
///
/// - `Global`: `offset` is in world axes; `node_basis` (the node's global basis columns, may be
///   scaled / sheared) converts it to mesh-local.
/// - `Local`: `offset` is used as is.
/// - `Normal`: `offset` is in the normal frame (y = normal) of the selection: the sum of the
///   normals of `faces` when given, else of the faces touching `verts`. With `per_face`, every
///   selected face moves along its own frame and shared vertices take the average.
///
/// Gap fixes: xogot-rust ignores the space in face mode and only knows Normal space with faces
/// selected (lowest face only).
pub fn offset_elements(m: &mut BMesh, verts: &BTreeSet<u32>, faces: &BTreeSet<usize>, offset: Vec3, space: OffsetSpace, node_basis: [Vec3; 3], per_face: bool) -> OpResult<()> {
    if ![offset.x, offset.y, offset.z].iter().all(|c| c.is_finite()) {
        return Err(INVALID_SELECTION.into());
    }
    let verts: BTreeSet<u32> = verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect();
    if verts.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    let faces: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    match space {
        OffsetSpace::Local => super::ops::move_vertices(m, &verts, offset),
        OffsetSpace::Global => {
            let local = solve_basis(node_basis, offset).ok_or("The node's basis cannot be inverted.")?;
            super::ops::move_vertices(m, &verts, local);
        }
        OffsetSpace::Normal => {
            let around: BTreeSet<usize> = if faces.is_empty() {
                (0..m.faces.len()).filter(|f| m.faces[*f].verts.iter().any(|v| verts.contains(v))).collect()
            } else {
                faces.clone()
            };
            let sum = around.iter().fold(Vec3::ZERO, |a, f| a + unit_or_zero(raw_normal(m, *f)));
            let shared = normal_frame(sum).to_world(offset);
            if !per_face || faces.is_empty() {
                super::ops::move_vertices(m, &verts, shared);
                return Ok(());
            }
            let mut moves: BTreeMap<u32, (Vec3, u32)> = BTreeMap::new();
            for f in &faces {
                let d = normal_frame(raw_normal(m, *f)).to_world(offset);
                for v in &m.faces[*f].verts {
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

// ------------------------------------------------------------------ 3. selection

/// Every vertex used by a face, minus `sel` (port of `topology_selection.rs` `MeshSelection::invert`).
pub fn invert_vertices(m: &BMesh, sel: &BTreeSet<u32>) -> BTreeSet<u32> {
    m.faces.iter().flat_map(|f| f.verts.iter().copied()).filter(|v| !sel.contains(v)).collect()
}

/// Every edge minus `sel` (edges compared undirected).
pub fn invert_edges(m: &BMesh, sel: &BTreeSet<Edge>) -> BTreeSet<Edge> {
    let sel: BTreeSet<Edge> = sel.iter().map(|e| edge(e.0, e.1)).collect();
    m.edges().into_iter().filter(|e| !sel.contains(e)).collect()
}

/// Every face minus `sel`.
pub fn invert_faces(m: &BMesh, sel: &BTreeSet<usize>) -> BTreeSet<usize> {
    (0..m.faces.len()).filter(|f| !sel.contains(f)).collect()
}

/// Which element type a selection-by-colour works on.
#[derive(Clone, Debug)]
pub enum ElementSel {
    Vertices(BTreeSet<u32>),
    Edges(BTreeSet<Edge>),
    Faces(BTreeSet<usize>),
}

/// Selects every element whose corner colours include one of the colours of the selected
/// elements' corners (port of `mesh_attributes.rs` `MeshAttributes::select_by_color`). A corner's
/// colour is its vertex-paint colour, else its face colour. Faces match when any corner
/// matches, edges when both ends match in a face using the edge, vertices when any corner at the
/// vertex matches. Returns a selection of the same kind.
pub fn select_by_color(m: &BMesh, sel: &ElementSel) -> ElementSel {
    let mut samples: Vec<[u8; 4]> = Vec::new();
    let mut add = |c: [u8; 4]| {
        if !samples.contains(&c) {
            samples.push(c);
        }
    };
    match sel {
        ElementSel::Vertices(vs) => {
            for f in &m.faces {
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
            for f in &m.faces {
                for (ci, v) in f.verts.iter().enumerate() {
                    if ends.contains(v) {
                        add(f.corner_color(ci));
                    }
                }
            }
        }
        ElementSel::Faces(fs) => {
            for f in fs.iter().filter_map(|f| m.faces.get(*f)) {
                for ci in 0..f.verts.len() {
                    add(f.corner_color(ci));
                }
            }
        }
    }
    let matches = |f: &Face, ci: usize| samples.contains(&f.corner_color(ci));
    match sel {
        ElementSel::Vertices(_) => ElementSel::Vertices(
            m.faces.iter().flat_map(|f| (0..f.verts.len()).filter(|ci| matches(f, *ci)).map(|ci| f.verts[ci]).collect::<Vec<_>>()).collect(),
        ),
        ElementSel::Edges(_) => ElementSel::Edges(
            m.faces
                .iter()
                .flat_map(|f| {
                    let n = f.verts.len();
                    (0..n).filter(|i| matches(f, *i) && matches(f, (i + 1) % n)).map(|i| edge(f.verts[i], f.verts[(i + 1) % n])).collect::<Vec<_>>()
                })
                .collect(),
        ),
        ElementSel::Faces(_) => ElementSel::Faces((0..m.faces.len()).filter(|fi| (0..m.faces[*fi].verts.len()).any(|ci| matches(&m.faces[*fi], ci))).collect()),
    }
}

/// Every face whose smoothing group is used by one of `faces` (port of
/// `geometry_actions.rs` `select_by_smoothing_group`).
pub fn select_by_smoothing_group(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let groups: BTreeSet<u16> = faces.iter().filter_map(|f| m.faces.get(*f)).map(|f| f.smooth).collect();
    (0..m.faces.len()).filter(|f| groups.contains(&m.faces[*f].smooth)).collect()
}

/// Every face whose material slot is used by one of `faces` (`select_by_material`).
pub fn select_by_material(m: &BMesh, faces: &BTreeSet<usize>) -> BTreeSet<usize> {
    let slots: BTreeSet<u16> = faces.iter().filter_map(|f| m.faces.get(*f)).map(|f| f.material).collect();
    (0..m.faces.len()).filter(|f| slots.contains(&m.faces[*f].material)).collect()
}

fn angle_threshold(angle_deg: f32) -> f32 {
    angle_deg.clamp(0.0, 180.0).to_radians().cos() - 1e-6
}

/// Grows a face selection by one ring: a neighbour joins when its normal is within
/// `angle_deg` (0..=180) of the selected face it touches (port of `mesh_topology.rs`
/// `MeshSelection::grow_faces`).
pub fn grow_faces_by_angle(m: &BMesh, faces: &BTreeSet<usize>, angle_deg: f32) -> BTreeSet<usize> {
    let threshold = angle_threshold(angle_deg);
    let nb = face_neighbors(m);
    let mut out: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    for &f in faces.iter().filter(|f| **f < m.faces.len()) {
        let n = unit_or_zero(raw_normal(m, f));
        for &g in &nb[f] {
            if n.dot(unit_or_zero(raw_normal(m, g))) >= threshold {
                out.insert(g);
            }
        }
    }
    out
}

/// Faces allowed for a vertex / edge grow: the seed faces, plus every face whose normal is
/// within the angle of a seed face's normal.
fn allowed_faces(m: &BMesh, seeds: &BTreeSet<usize>, angle_deg: f32) -> BTreeSet<usize> {
    let threshold = angle_threshold(angle_deg);
    let seed_normals: Vec<Vec3> = seeds.iter().map(|f| unit_or_zero(raw_normal(m, *f))).collect();
    (0..m.faces.len())
        .filter(|f| seeds.contains(f) || {
            let n = unit_or_zero(raw_normal(m, *f));
            seed_normals.iter().any(|s| s.dot(n) >= threshold)
        })
        .collect()
}

/// Grows a vertex selection by one ring over the faces touching it (port of
/// `topology_selection.rs` `MeshSelection::grow`, vertex mode). Like the original, every face
/// touching a selected vertex is a seed, so `angle_deg` cannot restrict a vertex grow (kept
/// for API symmetry; the angle matters for faces and edges).
pub fn grow_vertices_by_angle(m: &BMesh, verts: &BTreeSet<u32>, angle_deg: f32) -> BTreeSet<u32> {
    let seeds: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].verts.iter().any(|v| verts.contains(v))).collect();
    let allowed = allowed_faces(m, &seeds, angle_deg);
    let mut out = verts.clone();
    for f in &allowed {
        for (a, b) in m.faces[*f].loop_edges() {
            if verts.contains(&a) || verts.contains(&b) {
                out.insert(a);
                out.insert(b);
            }
        }
    }
    out
}

/// Grows an edge selection: the edges of allowed faces (see `grow_vertices_by_angle`) that touch
/// an end of a selected edge (port of `MeshSelection::grow`, edge mode).
pub fn grow_edges_by_angle(m: &BMesh, edges: &BTreeSet<Edge>, angle_deg: f32) -> BTreeSet<Edge> {
    let sel: BTreeSet<Edge> = edges.iter().map(|e| edge(e.0, e.1)).collect();
    let seeds: BTreeSet<usize> = (0..m.faces.len()).filter(|f| m.faces[*f].loop_edges().any(|(a, b)| sel.contains(&edge(a, b)))).collect();
    let allowed = allowed_faces(m, &seeds, angle_deg);
    let ends: BTreeSet<u32> = sel.iter().flat_map(|e| [e.0, e.1]).collect();
    let mut out = sel.clone();
    for f in &allowed {
        for (a, b) in m.faces[*f].loop_edges() {
            if ends.contains(&a) || ends.contains(&b) {
                out.insert(edge(a, b));
            }
        }
    }
    out
}

// ------------------------------------------------------------------ 4. extrude

/// Direction extruded faces move in (`MeshExtrudeMethod`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExtrudeMethod {
    /// The selection moves as one: each vertex along the normalised sum of the selected faces'
    /// normals around it.
    #[default]
    FaceNormal,
    /// As one, each vertex along the normalised sum of the normals of all faces around it.
    VertexNormal,
    /// Every face on its own, along its own normal (walls between selected neighbours too).
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

/// Wall quad `[a, b, b2, a2]` along the directed edge a→b of face `src`: its attributes, no
/// smoothing / manual UVs, corner colours copied from the base corners.
fn wall(src: &Face, a: u32, b: u32, a2: u32, b2: u32) -> Face {
    let mut w = src.with_verts(vec![a, b, b2, a2]);
    w.smooth = 0;
    if let (Some(ca), Some(cb)) = (corner(src, a), corner(src, b)) {
        if src.corner_colors().is_some() {
            let (ca, cb) = (src.corner_color(ca), src.corner_color(cb));
            w.color_corners = vec![ca, cb, cb, ca];
        }
    }
    w
}

/// Extrudes faces by `distance` with `method`. Port of xogot-rust `modeler_kit/src/mesh_operations.rs`
/// `MeshOperations::extrude_faces` / `extrude_individual_faces`. A quad wall is added along
/// every edge of the selection's outline (every edge of each face for `IndividualFaces`); the
/// selected faces become the caps and are returned (same indices). A zero distance is refused
/// unless `allow_zero_distance` (live previews dragging through 0).
///
/// Gap fix: xogot-rust's individual extrude detaches each face first, which leaves open
/// coincident seams between neighbouring walls; here each face gets its own cap vertices and
/// walls on the original shared vertices, so a closed mesh stays closed.
pub fn extrude_selection_method(m: &mut BMesh, faces: &BTreeSet<usize>, distance: f32, method: ExtrudeMethod, allow_zero_distance: bool) -> OpResult<BTreeSet<usize>> {
    let faces: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
    if faces.is_empty() {
        return Err(INVALID_SELECTION.into());
    }
    if !distance.is_finite() || (distance.abs() <= 1e-8 && !allow_zero_distance) {
        return Err("Extrusion distance must be finite and nonzero.".into());
    }
    if faces.iter().any(|f| raw_normal(m, *f).dot(raw_normal(m, *f)) <= 0.0) {
        return Err("Cannot extrude a face with zero area.".into());
    }
    on_copy(m, |w| {
        if method == ExtrudeMethod::IndividualFaces {
            for &f in &faces {
                let n = unit_or_zero(raw_normal(w, f));
                let src = w.faces[f].clone();
                let cap: Vec<u32> = src.verts.iter().map(|v| w.add_vert(w.positions[*v as usize] + n * distance)).collect();
                let k = src.verts.len();
                for i in 0..k {
                    let j = (i + 1) % k;
                    w.faces.push(wall(&src, src.verts[i], src.verts[j], cap[i], cap[j]));
                }
                w.faces[f].verts = cap;
            }
            return Ok(faces.clone());
        }
        let mut sums: BTreeMap<u32, Vec3> = BTreeMap::new();
        let mut directed = BTreeSet::new();
        let mut counts: HashMap<Edge, usize> = HashMap::new();
        for &f in &faces {
            let n = unit_or_zero(raw_normal(w, f));
            for v in &w.faces[f].verts {
                let s = sums.entry(*v).or_insert(Vec3::ZERO);
                *s = *s + n;
            }
            for (a, b) in w.faces[f].loop_edges() {
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
            for f in 0..w.faces.len() {
                let n = unit_or_zero(raw_normal(w, f));
                for v in w.faces[f].verts.clone() {
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
            let src = w.faces[f].clone();
            for (a, b) in src.loop_edges() {
                if !directed.contains(&(b, a)) {
                    w.faces.push(wall(&src, a, b, dup[&a], dup[&b]));
                }
            }
            for v in &mut w.faces[f].verts {
                *v = dup[v];
            }
        }
        Ok(faces.clone())
    })
}

/// Extrudes open (border) edges by `distance` (in the plane of their face, outward) plus
/// `offset`. With `as_group` the edges share new vertices (`topo::extrude_open_edges`); without, each
/// edge gets its own pair and moves along its own outward direction (port of
/// `topology_operations.rs` `MeshOperations::extrude_edges`, `as_group == false`). Returns the
/// new outer edges.
pub fn extrude_edges_grouped(m: &mut BMesh, edges: &BTreeSet<Edge>, distance: f32, offset: Vec3, as_group: bool) -> BTreeSet<Edge> {
    if as_group {
        return super::topo::extrude_open_edges(m, edges, distance, offset);
    }
    let owner = directed_owner(m);
    let mut out = BTreeSet::new();
    for &(a, b) in edges {
        let (a, b, f) = match (owner.get(&(a, b)), owner.get(&(b, a))) {
            (Some(f), None) => (a, b, *f),
            (None, Some(f)) => (b, a, *f),
            _ => continue,
        };
        let e = (m.positions[b as usize] - m.positions[a as usize]).normalized();
        let dir = e.cross(m.face_normal(f)).normalized() * distance + offset;
        let a2 = m.add_vert(m.positions[a as usize] + dir);
        let b2 = m.add_vert(m.positions[b as usize] + dir);
        let src = m.faces[f].clone();
        m.faces.push(Face { smooth: 0, ..src.with_verts(vec![b, a, a2, b2]) });
        out.insert(edge(a2, b2));
    }
    out
}

// ------------------------------------------------------------------ 5. inset

/// Insets faces by a rim `width` wide (world units). With `individual`, every face is inset on
/// its own (`mesh_ops::inset_rim`, mitred). Without, connected coplanar selected faces with equal
/// attributes are inset as one region: the region's outline moves inward and one ring of border
/// quads runs along it (port of xogot-rust `modeler_kit/src/mesh_inset.rs`
/// `MeshOperations::inset_faces` / `inset_face_regions` / `inset_region`). Returns the inner faces.
pub fn inset_center_grouped(m: &mut BMesh, faces: &BTreeSet<usize>, width: f32, individual: bool) -> OpResult<BTreeSet<usize>> {
    let faces: BTreeSet<usize> = faces.iter().copied().filter(|f| *f < m.faces.len()).collect();
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
                inset_region(w, &region, width)?;
                out.extend(region);
            }
        }
        Ok(out)
    })
}

/// Connected regions of coplanar selected faces with equal attributes (`inset_face_regions`).
fn inset_regions(m: &BMesh, faces: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let nb = face_neighbors(m);
    let mut remaining = faces.clone();
    let mut regions = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let seed_face = &m.faces[seed];
        let n0 = unit_or_zero(raw_normal(m, seed));
        let origin = m.positions[seed_face.verts[0] as usize];
        let mut region = BTreeSet::from([seed]);
        let mut queue = vec![seed];
        let mut cursor = 0;
        while cursor < queue.len() {
            let f = queue[cursor];
            cursor += 1;
            for &g in &nb[f] {
                if !remaining.contains(&g) || !same_attributes(&m.faces[g], seed_face) {
                    continue;
                }
                if unit_or_zero(raw_normal(m, g)).dot(n0) <= 0.9999 {
                    continue;
                }
                if !m.faces[g].verts.iter().all(|v| (m.positions[*v as usize] - origin).dot(n0).abs() < 1e-4) {
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

/// Insets one multi-face region (`inset_region`): outline vertices (outer loop and holes) move
/// inward by `width` (mitred corners), region faces keep their interior vertices, and border
/// quads fill the gap. Refused when the width folds the outline or flips a face.
fn inset_region(m: &mut BMesh, region: &BTreeSet<usize>, width: f32) -> OpResult<()> {
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
            // Interior lies left of a loop wound counter-clockwise around `n` (outer and holes).
            let in1 = unit_or_zero(n.cross(cur - prev));
            let in2 = unit_or_zero(n.cross(next - cur));
            let denom = 1.0 + in1.dot(in2);
            if denom < 1e-4 {
                return Err(invalid());
            }
            moved.push(cur + (in1 + in2) * (width / denom));
        }
        for i in 0..k {
            let j = (i + 1) % k;
            if (moved[j] - moved[i]).dot(pts[j] - pts[i]) <= 1e-10 {
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
    let originals: Vec<Face> = region.iter().map(|f| m.faces[*f].clone()).collect();
    // Turn of each corner about `n` (`validate_translated_face`: no corner may flip).
    let turns = |m: &BMesh, f: usize| -> Vec<f32> {
        let p = m.face_points(f);
        let k = p.len();
        (0..k).map(|i| n.dot((p[i] - p[(i + k - 1) % k]).cross(p[(i + 1) % k] - p[i]))).collect()
    };
    for f in region {
        let before = turns(m, *f);
        for v in &mut m.faces[*f].verts {
            if let Some(nv) = inner.get(v) {
                *v = *nv;
            }
        }
        let rn = raw_normal(m, *f);
        if rn.dot(n) <= 1e-12 {
            return Err(invalid());
        }
        let eps = 1e-6 * rn.length();
        if before.iter().zip(turns(m, *f)).any(|(b, a)| (*b > eps && a < -eps) || (*b < -eps && a > eps)) {
            return Err(invalid());
        }
    }
    for (a, b) in boundary {
        let fi = owner[&(a, b)];
        let src = &originals[region.iter().position(|f| *f == fi).unwrap()];
        m.faces.push(wall(src, a, b, inner[&a], inner[&b]));
    }
    Ok(())
}

// ------------------------------------------------------------------ 6. collapse to first

/// Collapses the vertices into one, at `target`'s position (the "Collapse to first" option; the
/// integrator passes the first-picked / active vertex, `None` = the lowest index, like
/// xogot-rust `topology_operations.rs` `MeshOperations::collapse_vertices(to_first: true)`).
/// Uses `mesh_ops::collapse_selection` (refused when it would over-share an edge). Returns the
/// collapsed vertex. Weld by distance with transitive clusters is `loops::weld_selected_vertices`.
pub fn collapse_vertices_to(m: &mut BMesh, verts: &BTreeSet<u32>, target: Option<u32>) -> Option<u32> {
    let list: Vec<u32> = verts.iter().copied().filter(|v| (*v as usize) < m.positions.len()).collect();
    let target = target.filter(|t| list.contains(t)).or(list.first().copied())?;
    let p = m.positions[target as usize];
    let saved = m.positions.clone();
    // `mesh_ops::collapse_selection` finds its survivor by position: park the selection on a point
    // no other vertex uses so the right index comes back, then move it onto the target.
    let (lo, hi) = m.bounds()?;
    let sentinel = hi + v3(1.0, 1.0, 1.0) * ((hi - lo).length() + 1.0);
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

// ------------------------------------------------------------------ 7. fill hole

/// The selection a hole fill works from.
#[derive(Clone, Debug)]
pub enum HoleSel {
    Vertices(BTreeSet<u32>),
    Edges(BTreeSet<Edge>),
}

/// The runs of consecutive selected edges of `hole` (edge i joins i and i+1) as vertex chains
/// of at least three vertices, starting after the first unselected edge (`selected_runs`).
fn selected_runs(hole: &[u32], selected: &[bool]) -> Vec<Vec<u32>> {
    let count = hole.len();
    let Some(brk) = selected.iter().position(|s| !s) else { return Vec::new() };
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

/// The chord closing a partial fill must not cross the hole outline and every fill triangle's
/// centre must lie inside the hole (`validate_partial_fill`).
fn validate_partial_fill(m: &BMesh, hole: &[u32], cap: &[u32], owner: &HashMap<(u32, u32), usize>) -> OpResult<()> {
    let lp = loop_plane(m, hole).ok_or(DISSOLVE_ZERO_AREA)?;
    // Gap fix: on the outer border of an open sheet the fill-wound loop runs against the faces
    // along it, and the area to fill is outside the loop polygon (xogot-rust tests the inside).
    let k = hole.len();
    let along = (0..k).filter_map(|i| owner.get(&(hole[(i + 1) % k], hole[i]))).fold(Vec3::ZERO, |a, f| a + unit_or_zero(raw_normal(m, *f)));
    let hole_n = unit_or_zero(newell(&hole.iter().map(|v| m.positions[*v as usize]).collect::<Vec<_>>()));
    let outside = along.length() > 1e-6 && unit_or_zero(along).dot(hole_n) < -0.5;
    let to2 = |v: u32| lp.dir_2d(m.positions[v as usize] - lp.origin);
    let (first, last) = (to2(cap[0]), to2(*cap.last().unwrap()));
    let n = lp.pts.len();
    let eps = 1e-7 * lp.scale * lp.scale;
    for i in 0..n {
        if segments_cross(first, last, lp.pts[i], lp.pts[(i + 1) % n], eps) {
            return Err("The partial fill crosses the hole boundary.".into());
        }
    }
    let pts: Vec<Vec3> = cap.iter().map(|v| m.positions[*v as usize]).collect();
    for t in triangulate(&pts, newell(&pts)) {
        let c = (to2(cap[t[0]]) + to2(cap[t[1]]) + to2(cap[t[2]])) / 3.0;
        if point_in_polygon(c, &lp.pts) == outside {
            return Err("The partial fill is outside the hole.".into());
        }
    }
    Ok(())
}

/// Fills the holes touched by the selection. Port of xogot-rust `modeler_kit/src/polygon_mesh.rs`
/// `PolygonMesh::fill_selected_holes`: with `entire_hole`, or when every edge of a hole is
/// selected, the whole hole is closed; otherwise each run of three or more selected border
/// vertices is closed by its chord, which must stay inside the hole (outside the mesh for the
/// outer border of an open sheet: gap fix). New faces copy the face along the run's first edge;
/// they are returned.
pub fn fill_selected_holes(m: &mut BMesh, sel: &HoleSel, entire_hole: bool) -> OpResult<BTreeSet<usize>> {
    on_copy(m, |w| {
        let owner = directed_owner(w);
        // Open edges reversed: the loops come out wound like the faces that will fill them.
        let open: Vec<(u32, u32)> = owner.keys().filter(|(a, b)| !owner.contains_key(&(*b, *a))).map(|(a, b)| (*b, *a)).collect();
        let loops = chain_loops(&open).ok_or("Fill hole requires a closed boundary used only once.")?;
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
                        HoleSel::Edges(es) => es.contains(&edge(a, b)) || es.contains(&(a, b)) || es.contains(&(b, a)),
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
            let paths = if entire_hole || !selected.contains(&false) { vec![hole.clone()] } else { selected_runs(hole, &selected) };
            for path in paths {
                if path.len() != n {
                    validate_partial_fill(w, hole, &path, &owner)?;
                }
                let template = owner.get(&(path[1], path[0])).map(|f| w.faces[*f].clone()).unwrap_or_else(|| Face::new(Vec::new()));
                w.faces.push(Face { smooth: 0, ..template.with_verts(path) });
                out.insert(w.faces.len() - 1);
            }
        }
        if out.is_empty() {
            return Err(INVALID_SELECTION.into());
        }
        Ok(out)
    })
}

// ------------------------------------------------------------------ 8. mirror

/// Removes faces lying on the plane `axis = 0` (within `threshold`) that use exactly the same
/// vertex set as another such face (both of them): after a mirror copy is welded, a face on the
/// mirror plane and its copy become such a pair. Double-sided faces elsewhere are kept.
fn drop_coincident_pairs(m: &mut BMesh, axis: usize, threshold: f32) {
    let on = |v: &u32| {
        let p = m.positions[*v as usize];
        [p.x, p.y, p.z][axis].abs() <= threshold
    };
    let mut by_set: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
    for (i, f) in m.faces.iter().enumerate() {
        if !f.verts.iter().all(on) {
            continue;
        }
        let mut k = f.verts.clone();
        k.sort();
        by_set.entry(k).or_default().push(i);
    }
    let drop: BTreeSet<usize> = by_set.values().filter(|l| l.len() >= 2).flatten().copied().collect();
    if !drop.is_empty() {
        remove_faces(m, &drop);
        m.compact();
    }
}

/// Mirrors the mesh across the local planes of the selected `axes` (x, y, z) at once. Port of
/// xogot-rust `xogot_modeler/src/shared/object_actions.rs` `mirror_objects`: without
/// `duplicate` the positions are scaled by −1 on those axes and faces flip when the mirror turns
/// the mesh inside out (`edit_ops::bake_transform`). Gap fix (xogot-rust ignores the flag):
/// with `duplicate` a mirrored copy is added per axis (`loops::mirror_copy`, 2^k parts),
/// vertices on the plane within `weld_threshold` are welded, and faces lying on the plane are
/// dropped with their copy so the seam closes. Node determinants are the caller's concern.
pub fn mirror_axes(m: &mut BMesh, axes: [bool; 3], duplicate: bool, weld_threshold: f32) -> OpResult<()> {
    if !axes.contains(&true) {
        return Err("Select at least one mirror axis.".into());
    }
    on_copy(m, |w| {
        if !duplicate {
            let s = |on: bool| if on { -1.0 } else { 1.0 };
            let xf = Affine { x: v3(s(axes[0]), 0.0, 0.0), y: v3(0.0, s(axes[1]), 0.0), z: v3(0.0, 0.0, s(axes[2])), origin: Vec3::ZERO };
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

// ------------------------------------------------------------------ 9. individual origins

/// Splits `verts` into connected groups, two selected vertices being linked by one of `links`
/// (pass the selected edges in edge mode, `edges_of_faces(selected)` in face mode, or
/// `edges_within(verts)` in vertex mode). Groups are ordered by their lowest vertex.
pub fn connected_groups(verts: &BTreeSet<u32>, links: &BTreeSet<Edge>) -> Vec<BTreeSet<u32>> {
    let mut adjacency: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (a, b) in links {
        if verts.contains(a) && verts.contains(b) {
            adjacency.entry(*a as usize).or_default().push(*b as usize);
            adjacency.entry(*b as usize).or_default().push(*a as usize);
        }
    }
    let nodes: BTreeSet<usize> = verts.iter().map(|v| *v as usize).collect();
    graph_components(&nodes, &|v| adjacency.get(&v).cloned().unwrap_or_default())
        .into_iter()
        .map(|g| g.into_iter().map(|v| v as u32).collect())
        .collect()
}

/// Average position of a group (its pivot for individual origins, like `ViewportGeometry::center`).
pub fn group_center(m: &BMesh, group: &BTreeSet<u32>) -> Vec3 {
    let pts: Vec<Vec3> = group.iter().filter_map(|v| m.positions.get(*v as usize).copied()).collect();
    pts.iter().fold(Vec3::ZERO, |a, p| a + *p) / pts.len().max(1) as f32
}

/// Rotates every group about its own centre (Pivot mode *Individual origins*; port of
/// `editor_core/src/viewport_geometry.rs` `ViewportGeometry::transformed(individual_origins)`,
/// per connected group instead of per element so shared vertices are not averaged).
pub fn rotate_individual(m: &mut BMesh, groups: &[BTreeSet<u32>], axis: Vec3, angle: f32) {
    for g in groups {
        let c = group_center(m, g);
        rotate_positions(m, g, c, axis, angle);
    }
}

/// Scales every group about its own centre by per-axis `factors` of `orientation`.
pub fn scale_individual(m: &mut BMesh, groups: &[BTreeSet<u32>], orientation: &Frame, factors: Vec3) {
    for g in groups {
        let c = group_center(m, g);
        scale_positions(m, g, c, orientation, factors);
    }
}

#[cfg(test)]
mod tests {
    use super::super::shapes::{cuboid, plane};
    use super::*;

    const ID: [Vec3; 3] = [v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0)];

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    fn face_with_normal(m: &BMesh, n: Vec3) -> usize {
        (0..m.faces.len()).find(|f| m.face_normal(*f).dot(n) > 0.99).unwrap()
    }

    fn centre_vertex(m: &BMesh) -> u32 {
        m.positions.iter().position(|p| near(*p, Vec3::ZERO)).unwrap() as u32
    }

    // --- dissolve (cases of xogot-rust `dissolve_tests`)

    #[test]
    fn dissolving_two_faces_merges_them() {
        let mut m = plane(v2(2.0, 1.0), 2);
        let mut g = plane(v2(2.0, 2.0), 2);
        let out = dissolve_faces(&mut g, &BTreeSet::from([0, 1])).unwrap();
        g.validate().unwrap();
        assert_eq!(g.faces.len(), 3);
        let f = *out.first().unwrap();
        assert_eq!(g.faces[f].verts.len(), 6, "two quads merged keep the middle points");
        assert!((g.face_area(f) - 2.0).abs() < 1e-4);
        assert!(g.face_normal(f).y > 0.99);
        // Whole grid.
        let all: BTreeSet<usize> = (0..m.faces.len()).collect();
        dissolve_faces(&mut m, &all).unwrap();
        assert_eq!(m.faces.len(), 1);
    }

    #[test]
    fn dissolving_the_shared_edge_merges_its_faces() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let e = m.edges_of_faces(&BTreeSet::from([0])).intersection(&m.edges_of_faces(&BTreeSet::from([1]))).copied().next().unwrap();
        dissolve_edges(&mut m, &BTreeSet::from([e])).unwrap();
        m.validate().unwrap();
        assert_eq!(m.faces.len(), 3);
    }

    #[test]
    fn dissolving_the_centre_vertex_merges_four_faces() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let c = centre_vertex(&m);
        let out = dissolve_vertices(&mut m, &BTreeSet::from([c])).unwrap();
        m.validate().unwrap();
        assert_eq!(m.faces.len(), 1);
        assert_eq!(m.positions.len(), 8, "the centre vertex is gone");
        assert!(!m.positions.iter().any(|p| near(*p, Vec3::ZERO)));
        assert!((m.face_area(*out.first().unwrap()) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn a_single_face_or_a_border_edge_is_rejected() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let before = m.clone();
        assert!(dissolve_faces(&mut m, &BTreeSet::from([0])).is_err());
        let border = m.edges().into_iter().find(|e| m.edge_faces()[e].len() == 1).unwrap();
        assert_eq!(dissolve_edges(&mut m, &BTreeSet::from([border])).unwrap_err(), DISSOLVE_MANIFOLD);
        assert_eq!(m, before, "a refused dissolve leaves the mesh untouched");
    }

    #[test]
    fn faces_off_one_plane_or_with_other_attributes_are_rejected() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let (t, s) = (face_with_normal(&m, Vec3::UP), face_with_normal(&m, v3(1.0, 0.0, 0.0)));
        assert_eq!(dissolve_faces(&mut m, &BTreeSet::from([t, s])).unwrap_err(), DISSOLVE_PLANE);
        let mut g = plane(v2(2.0, 2.0), 2);
        g.faces[1].material = 3;
        assert_eq!(dissolve_faces(&mut g, &BTreeSet::from([0, 1])).unwrap_err(), DISSOLVE_ATTRIBUTES);
    }

    #[test]
    fn a_ring_of_faces_is_not_a_disk() {
        let mut m = plane(v2(3.0, 3.0), 3);
        let ring: BTreeSet<usize> = (0..9).filter(|f| *f != 4).collect();
        assert!(dissolve_faces(&mut m, &ring).is_err());
    }

    #[test]
    fn separate_selections_become_separate_regions() {
        let mut m = plane(v2(4.0, 1.0), 4);
        // plane(…, 4) is 4×4; take two separate pairs on the first row.
        let out = dissolve_faces(&mut m, &BTreeSet::from([0, 1, 3, 7])).unwrap();
        m.validate().unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(m.faces.len(), 14);
    }

    #[test]
    fn a_two_valent_vertex_leaves_its_loops() {
        let mut m = plane(v2(2.0, 2.0), 2);
        // Split the edge between the first two faces: its midpoint has exactly two edges.
        let e = m.edges_of_faces(&BTreeSet::from([0])).intersection(&m.edges_of_faces(&BTreeSet::from([1]))).copied().next().unwrap();
        let mid = super::super::topo::split_edges(&mut m, &BTreeSet::from([e]), 1);
        let v = *mid.first().unwrap();
        let faces = m.faces.len();
        dissolve_vertices(&mut m, &BTreeSet::from([v])).unwrap();
        m.validate().unwrap();
        assert_eq!(m.faces.len(), faces, "faces stay separate");
        assert!(m.faces.iter().all(|f| f.verts.len() == 4));
        assert_eq!(m.positions.len(), 9);
    }

    // --- offset

    #[test]
    fn offset_in_each_space() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let top = face_with_normal(&m, Vec3::UP);
        let verts = m.verts_of_faces(&BTreeSet::from([top]));
        let p0 = m.positions[*verts.first().unwrap() as usize];
        offset_elements(&mut m, &verts, &BTreeSet::new(), v3(1.0, 0.0, 0.0), OffsetSpace::Local, ID, false).unwrap();
        assert!(near(m.positions[*verts.first().unwrap() as usize], p0 + v3(1.0, 0.0, 0.0)));
        // Global with a node scaled ×2 on x: one world unit is half a local unit.
        let basis = [v3(2.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0)];
        offset_elements(&mut m, &verts, &BTreeSet::new(), v3(1.0, 0.0, 0.0), OffsetSpace::Global, basis, false).unwrap();
        assert!(near(m.positions[*verts.first().unwrap() as usize], p0 + v3(1.5, 0.0, 0.0)));
        // Normal: y of the frame is the top normal.
        let mut c = cuboid(v3(1.0, 1.0, 1.0));
        let side = face_with_normal(&c, v3(1.0, 0.0, 0.0));
        let sv = c.verts_of_faces(&BTreeSet::from([side]));
        let q0 = c.positions[*sv.first().unwrap() as usize];
        offset_elements(&mut c, &sv, &BTreeSet::new(), v3(0.0, 0.5, 0.0), OffsetSpace::Normal, ID, false).unwrap();
        assert!(near(c.positions[*sv.first().unwrap() as usize], q0 + v3(0.5, 0.0, 0.0)), "vertex mode uses the faces around");
        // Per face: two adjacent cube faces push apart diagonally.
        let mut d = cuboid(v3(1.0, 1.0, 1.0));
        let (t, s) = (face_with_normal(&d, Vec3::UP), face_with_normal(&d, v3(1.0, 0.0, 0.0)));
        let fs = BTreeSet::from([t, s]);
        let vs = d.verts_of_faces(&fs);
        let before = d.positions.clone();
        offset_elements(&mut d, &vs, &fs, v3(0.0, 1.0, 0.0), OffsetSpace::Normal, ID, true).unwrap();
        let shared = *vs.iter().find(|v| d.faces[t].verts.contains(v) && d.faces[s].verts.contains(v)).unwrap();
        assert!(near(d.positions[shared as usize], before[shared as usize] + v3(0.5, 0.5, 0.0)));
        let only_top = *vs.iter().find(|v| !d.faces[s].verts.contains(v)).unwrap();
        assert!(near(d.positions[only_top as usize], before[only_top as usize] + Vec3::UP));
        assert!(offset_elements(&mut d, &vs, &fs, Vec3::UP, OffsetSpace::Global, [Vec3::ZERO; 3], false).is_err());
    }

    #[test]
    fn normal_frame_matches_the_original() {
        let f = normal_frame(v3(0.0, 2.0, 0.0));
        assert!(near(f.x, v3(-1.0, 0.0, 0.0)) && near(f.y, Vec3::UP) && near(f.z, v3(0.0, 0.0, -1.0)));
        let f = normal_frame(v3(0.0, 0.0, 1.0));
        assert!(near(f.y, v3(0.0, 0.0, 1.0)) && near(f.x, v3(1.0, 0.0, 0.0)));
        assert_eq!(normal_frame(Vec3::ZERO), Frame::IDENTITY);
    }

    // --- selection

    #[test]
    fn invert_colour_smoothing_and_grow() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        assert_eq!(invert_faces(&m, &BTreeSet::from([0, 1])).len(), 4);
        assert_eq!(invert_vertices(&m, &BTreeSet::from([0])).len(), 7);
        assert_eq!(invert_edges(&m, &BTreeSet::from([m.edges()[0]])).len(), 11);
        m.faces[2].color = [255, 0, 0, 255];
        m.faces[4].color = [255, 0, 0, 255];
        let ElementSel::Faces(red) = select_by_color(&m, &ElementSel::Faces(BTreeSet::from([2]))) else { panic!() };
        assert_eq!(red, BTreeSet::from([2, 4]));
        // Corner colours: one red corner on face 0 makes that vertex match in vertex mode.
        m.faces[0].color_corners = vec![[255, 0, 0, 255], [255; 4], [255; 4], [255; 4]];
        let v = m.faces[0].verts[0];
        let ElementSel::Vertices(vs) = select_by_color(&m, &ElementSel::Vertices(m.faces[2].verts.iter().copied().collect())) else { panic!() };
        assert!(vs.contains(&v));
        m.faces[1].smooth = 2;
        m.faces[3].smooth = 2;
        assert_eq!(select_by_smoothing_group(&m, &BTreeSet::from([1])), BTreeSet::from([1, 3]));
        let top = face_with_normal(&m, Vec3::UP);
        assert_eq!(grow_faces_by_angle(&m, &BTreeSet::from([top]), 45.0).len(), 1, "sides are 90° off");
        assert_eq!(grow_faces_by_angle(&m, &BTreeSet::from([top]), 90.0).len(), 5);
        let g = plane(v2(3.0, 3.0), 3);
        let c = BTreeSet::from([4usize]);
        assert_eq!(grow_faces_by_angle(&g, &c, 0.0).len(), 5, "flat grid: the four edge neighbours");
        let tv = m.verts_of_faces(&BTreeSet::from([top]));
        let one = BTreeSet::from([*tv.first().unwrap()]);
        assert_eq!(grow_vertices_by_angle(&m, &one, 10.0).len(), 4, "every face around a corner is a seed");
        let e = BTreeSet::from([m.edges_of_faces(&BTreeSet::from([top])).into_iter().next().unwrap()]);
        assert_eq!(grow_edges_by_angle(&m, &e, 10.0).len(), 5);
    }

    // --- extrude

    #[test]
    fn extrude_methods() {
        for method in [ExtrudeMethod::FaceNormal, ExtrudeMethod::VertexNormal, ExtrudeMethod::IndividualFaces] {
            let mut m = cuboid(v3(1.0, 1.0, 1.0));
            let (t, s) = (face_with_normal(&m, Vec3::UP), face_with_normal(&m, v3(1.0, 0.0, 0.0)));
            let caps = extrude_selection_method(&mut m, &BTreeSet::from([t, s]), 0.5, method, false).unwrap();
            m.validate().unwrap();
            assert!(m.is_closed(), "{method:?}");
            assert_eq!(caps, BTreeSet::from([t, s]));
            let walls = m.faces.len() - 6;
            assert_eq!(walls, if method == ExtrudeMethod::IndividualFaces { 8 } else { 6 }, "{method:?}");
            if method == ExtrudeMethod::IndividualFaces {
                assert!(m.face_points(t).iter().all(|p| (p.y - 1.5).abs() < 1e-4));
            }
        }
        // Vertex normal: a lone top face of a cube moves its corners diagonally (all faces count).
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        let t = face_with_normal(&m, Vec3::UP);
        extrude_selection_method(&mut m, &BTreeSet::from([t]), 1.0, ExtrudeMethod::VertexNormal, false).unwrap();
        let p = m.face_points(t)[0];
        assert!((p.y - (1.0 + 1.0 / 3f32.sqrt())).abs() < 1e-4, "{p:?}");
        let before = m.clone();
        assert!(extrude_selection_method(&mut m, &BTreeSet::from([t]), 0.0, ExtrudeMethod::FaceNormal, false).is_err());
        assert_eq!(m, before);
        extrude_selection_method(&mut m, &BTreeSet::from([t]), 0.0, ExtrudeMethod::FaceNormal, true).unwrap();
    }

    #[test]
    fn extrude_edges_individually() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let border: BTreeSet<Edge> = m.edges().into_iter().filter(|e| m.edge_faces()[e].len() == 1).take(2).collect();
        let mut g = m.clone();
        let out = extrude_edges_grouped(&mut m, &border, 0.5, Vec3::ZERO, false);
        m.validate().unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(m.positions.len(), 9 + 4);
        extrude_edges_grouped(&mut g, &border, 0.5, Vec3::ZERO, true);
        g.validate().unwrap();
        assert!(g.positions.len() <= 9 + 4);
    }

    // --- inset

    #[test]
    fn group_inset_keeps_one_rim() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let all: BTreeSet<usize> = (0..4).collect();
        let inner = inset_center_grouped(&mut m, &all, 0.25, false).unwrap();
        m.validate().unwrap();
        assert_eq!(inner, all);
        assert_eq!(m.faces.len(), 4 + 8, "one border quad per outline edge");
        let area: f32 = inner.iter().map(|f| m.face_area(*f)).sum();
        assert!((area - 1.5 * 1.5).abs() < 1e-4, "{area}");
        assert!(near(m.positions[centre_vertex(&m) as usize], Vec3::ZERO), "interior vertex stays");
        let total: f32 = m.total_area();
        assert!((total - 4.0).abs() < 1e-4);
        // Individual: every face gets its own rim.
        let mut i = plane(v2(2.0, 2.0), 2);
        inset_center_grouped(&mut i, &all, 0.1, true).unwrap();
        assert_eq!(i.faces.len(), 4 + 16);
        // Too wide.
        let mut w = plane(v2(2.0, 2.0), 2);
        assert!(inset_center_grouped(&mut w, &all, 1.5, false).is_err());
        assert_eq!(w, plane(v2(2.0, 2.0), 2));
    }

    #[test]
    fn group_inset_refuses_a_folded_face() {
        // 3×3 grid; pull an interior vertex close to the border: the outline moving inward by
        // 0.5 passes it and the bottom-middle cell would become a bow-tie.
        let mut m = plane(v2(3.0, 3.0), 3);
        let v = m.positions.iter().position(|p| near(*p, v3(-0.5, 0.0, -0.5))).unwrap();
        m.positions[v] = v3(-0.5, 0.0, -1.3);
        m.validate().unwrap();
        let before = m.clone();
        let all: BTreeSet<usize> = (0..9).collect();
        assert!(inset_center_grouped(&mut m, &all, 0.5, false).is_err());
        assert_eq!(m, before);
        assert!(inset_center_grouped(&mut m, &all, 0.1, false).is_ok());
    }

    #[test]
    fn group_inset_on_a_closed_cube_top_pair() {
        let mut m = cuboid(v3(2.0, 1.0, 1.0));
        let t = face_with_normal(&m, Vec3::UP);
        let fs = super::super::ops::subdivide_selection(&mut m, &BTreeSet::from([t]));
        let tops: BTreeSet<usize> = fs.into_iter().filter(|f| m.face_normal(*f).y > 0.99).collect();
        inset_center_grouped(&mut m, &tops, 0.1, false).unwrap();
        m.validate().unwrap();
        assert!(m.is_closed());
        assert!((m.volume() - 2.0).abs() < 1e-4);
    }

    // --- collapse

    #[test]
    fn collapse_to_first_keeps_the_target_position() {
        let mut m = plane(v2(2.0, 2.0), 2);
        let c = centre_vertex(&m);
        let other = m.faces[0].loop_edges().find_map(|(a, b)| (a == c).then_some(b)).unwrap();
        let p = m.positions[other as usize];
        let v = collapse_vertices_to(&mut m, &BTreeSet::from([c, other]), Some(other)).unwrap();
        m.validate().unwrap();
        assert!(near(m.positions[v as usize], p));
    }

    // --- fill hole

    #[test]
    fn partial_and_entire_fill() {
        let mut ring = plane(v2(3.0, 3.0), 3);
        super::super::ops::delete_faces(&mut ring, &BTreeSet::from([4]));
        // The hole's corners: select three of its four vertices (two edges).
        let hole: Vec<u32> = {
            let o = directed_owner(&ring);
            let open: Vec<(u32, u32)> = o.keys().filter(|(a, b)| !o.contains_key(&(*b, *a))).map(|(a, b)| (*b, *a)).collect();
            chain_loops(&open).unwrap().into_iter().find(|l| l.len() == 4 && l.iter().all(|v| ring.positions[*v as usize].x.abs() < 0.6)).unwrap()
        };
        let three = BTreeSet::from([hole[0], hole[1], hole[2]]);
        let mut part = ring.clone();
        let out = fill_selected_holes(&mut part, &HoleSel::Vertices(three.clone()), false).unwrap();
        part.validate().unwrap();
        let f = *out.first().unwrap();
        assert_eq!(part.faces[f].verts.len(), 3);
        assert!((part.face_area(f) - 0.5).abs() < 1e-4);
        assert!(part.face_normal(f).y > 0.99);
        let mut whole = ring.clone();
        let out = fill_selected_holes(&mut whole, &HoleSel::Vertices(three), true).unwrap();
        // Entire: the inner hole is closed (the outer border loop is not touched by the selection).
        assert_eq!(whole.faces[*out.first().unwrap()].verts.len(), 4);
        // A chord outside the hole (L-shaped hole, run around the reflex corner) is refused.
        let mut l = plane(v2(5.0, 5.0), 5);
        super::super::ops::delete_faces(&mut l, &BTreeSet::from([6, 7, 11]));
        let o = directed_owner(&l);
        let open: Vec<(u32, u32)> = o.keys().filter(|(a, b)| !o.contains_key(&(*b, *a))).map(|(a, b)| (*b, *a)).collect();
        let reflex = |m: &BMesh, v: u32| near(m.positions[v as usize], v3(-0.5, 0.0, -0.5));
        let lp = chain_loops(&open).unwrap().into_iter().find(|lp| lp.iter().any(|v| reflex(&l, *v))).unwrap();
        let k = lp.len();
        let ri = (0..k).find(|i| reflex(&l, lp[*i])).unwrap();
        let sel = BTreeSet::from([lp[(ri + k - 1) % k], lp[ri], lp[(ri + 1) % k]]);
        let before = l.clone();
        assert!(fill_selected_holes(&mut l, &HoleSel::Vertices(sel), false).is_err());
        assert_eq!(l, before);
        // Outer border of an open sheet: a corner flap over the sheet is refused, a run across
        // an L-shaped notch (outside the sheet) is filled.
        let mut s = plane(v2(2.0, 2.0), 2);
        let corner_run = |m: &BMesh, c: Vec3| -> BTreeSet<u32> {
            let o = directed_owner(m);
            let open: Vec<(u32, u32)> = o.keys().filter(|(a, b)| !o.contains_key(&(*b, *a))).map(|(a, b)| (*b, *a)).collect();
            let lp = chain_loops(&open).unwrap().remove(0);
            let k = lp.len();
            let i = (0..k).find(|i| near(m.positions[lp[*i] as usize], c)).unwrap();
            BTreeSet::from([lp[(i + k - 1) % k], lp[i], lp[(i + 1) % k]])
        };
        let flap = corner_run(&s, v3(1.0, 0.0, 1.0));
        assert!(fill_selected_holes(&mut s, &HoleSel::Vertices(flap), false).is_err());
        super::super::ops::delete_faces(&mut s, &BTreeSet::from([3]));
        let notch = corner_run(&s, Vec3::ZERO);
        let out = fill_selected_holes(&mut s, &HoleSel::Vertices(notch), false).unwrap();
        s.validate().unwrap();
        let f = *out.first().unwrap();
        assert!(s.face_normal(f).y > 0.99 && (s.face_area(f) - 0.5).abs() < 1e-4);
    }

    // --- mirror

    #[test]
    fn mirror_in_place_and_duplicate() {
        let mut m = cuboid(v3(1.0, 1.0, 1.0));
        m.translate(v3(0.5, 0.0, 0.5));
        let v0 = m.volume();
        let mut a = m.clone();
        mirror_axes(&mut a, [true, false, false], false, 1e-3).unwrap();
        assert!((a.volume() - v0).abs() < 1e-4, "still outward");
        mirror_axes(&mut a, [true, true, true], false, 1e-3).unwrap();
        assert!((a.volume() - v0).abs() < 1e-4);
        // Duplicate on x and z: four cubes joined into one 2×1×2 box; the on-plane faces vanish.
        let mut d = m.clone();
        mirror_axes(&mut d, [true, false, true], true, 1e-3).unwrap();
        d.validate().unwrap();
        assert!(d.is_closed());
        assert!((d.volume() - 4.0).abs() < 1e-4, "{}", d.volume());
        assert!(mirror_axes(&mut d, [false; 3], true, 1e-3).is_err());
        // A double-sided quad away from the plane survives the duplicate.
        let mut q = m.clone();
        let base = q.positions.len() as u32;
        for p in [v3(5.0, 0.0, 0.0), v3(6.0, 0.0, 0.0), v3(6.0, 0.0, 1.0), v3(5.0, 0.0, 1.0)] {
            q.add_vert(p);
        }
        q.add_face(vec![base, base + 1, base + 2, base + 3]);
        q.add_face(vec![base + 3, base + 2, base + 1, base]);
        mirror_axes(&mut q, [true, false, false], true, 1e-3).unwrap();
        assert_eq!(q.faces.len(), 10 + 4, "two cubes joined + both double-sided quads");
    }

    // --- individual origins

    #[test]
    fn individual_origins_groups() {
        let mut m = plane(v2(3.0, 1.0), 3);
        // Faces 0 and 2 of the first row are separate; face 0+1 would be one group.
        let fs = BTreeSet::from([0usize, 2]);
        let verts = m.verts_of_faces(&fs);
        let groups = connected_groups(&verts, &m.edges_of_faces(&fs));
        assert_eq!(groups.len(), 2);
        let centres: Vec<Vec3> = groups.iter().map(|g| group_center(&m, g)).collect();
        scale_individual(&mut m, &groups, &Frame::IDENTITY, v3(0.5, 1.0, 0.5));
        for (g, c) in groups.iter().zip(&centres) {
            assert!(near(group_center(&m, g), *c), "each group scales about its own centre");
        }
        rotate_individual(&mut m, &groups, Vec3::UP, std::f32::consts::FRAC_PI_2);
        for (g, c) in groups.iter().zip(&centres) {
            assert!(near(group_center(&m, g), *c));
        }
        // Two opposite edges of one quad are two groups with edge links.
        let q = plane(v2(1.0, 1.0), 1);
        let f = &q.faces[0];
        let es = BTreeSet::from([edge(f.verts[0], f.verts[1]), edge(f.verts[2], f.verts[3])]);
        assert_eq!(connected_groups(&q.verts_of_edges(&es), &es).len(), 2);
    }
    #[test]
    fn dissolve_two_valent_run_keeps_faces_joined() {
        // Quad A = [p, v1, v2, q] glued along p-v1, v1-v2, v2-q to a concave hexagon B around it
        // (R1 review: removing v2 from B but not from the then-triangle A split them apart).
        let mut m = BMesh::default();
        let pts = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)];
        for (x, z) in pts {
            m.positions.push(v3(x, 0.0, z));
        }
        let (p, v1, v2, q, a, b) = (0, 1, 2, 3, 4, 5);
        m.faces.push(Face::new(vec![p, v1, v2, q]));
        m.faces.push(Face::new(vec![p, a, b, q, v2, v1]));
        assert!(m.validate().is_ok());
        let sel: BTreeSet<u32> = [v1, v2].into_iter().collect();
        dissolve_vertices(&mut m, &sel).unwrap();
        assert!(m.validate().is_ok());
        // Open border unchanged (q-p, p-a, a-b, b-q): a T-junction would add open edges.
        let ef = m.edge_faces();
        let open = ef.values().filter(|fs| fs.len() == 1).count();
        assert_eq!(open, 4);
        // v1 is gone (indices are compacted, so test by position).
        let gone = v3(1.0, 0.0, 1.0);
        assert!(m.faces.iter().all(|f| f.verts.iter().all(|v| m.positions[*v as usize] != gone)));
    }

}
