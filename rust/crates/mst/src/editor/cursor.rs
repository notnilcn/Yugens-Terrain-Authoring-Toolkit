//! Viewport visuals of the terrain plugin, after the original MST gizmo:
//! * translucent brush shape with falloff gradient (`brush_cursor.gdshader`) draped on the terrain,
//! * per-cell / per-vertex markers scaled by the brush weight (hex blue, triangle warm, square grey)
//!   plus full cell outlines on cell grids (grid-aligned stamps read as whole cells),
//! * chunk outlines of every chunk, Chunk Manager colors with "+" (add) / "X" (remove) glyphs.
//!
//! Everything is one internal `MeshInstance3D` child of the terrain with an `ImmediateMesh`
//! (one surface per material).

use godot::classes::base_material_3d::{BlendMode, Flags, ShadingMode, Transparency};
use godot::classes::mesh::PrimitiveType;
use godot::classes::{ImmediateMesh, Material, MeshInstance3D, ResourceLoader, Shader, ShaderMaterial, StandardMaterial3D};
use godot::prelude::*;

use pt_core::grid::GridType;
use pt_core::math::{IVec2, Vec2 as PVec2, v2};

use super::ui_kit;
use crate::terrain::MarchingSquaresTerrain;
use crate::util::add_internal;

const SHADER_PATH: &str = "res://addons/MarchingSquaresTerrain/editor/brush_cursor.gdshader";
const LIFT: f32 = 0.04;

/// What to draw this frame (filled by the plugin).
pub struct Scene<'a> {
    pub hover: Option<Vector3>,
    pub brush: Option<&'a pt_core::brush::Brush>,
    /// Weighted samples to mark (footprint, selection or stroke pattern).
    pub marks: &'a [(IVec2, f32)],
    pub chunk_mode: bool,
    pub hover_chunk: Option<IVec2>,
    pub selected_chunk: Option<IVec2>,
    pub show_chunk_lines: bool,
    pub bridge_from: Option<Vector3>,
}

pub struct Cursor {
    pub node: Gd<MeshInstance3D>,
    mesh: Gd<ImmediateMesh>,
    brush_mat: Gd<ShaderMaterial>,
    cell_mat: Gd<StandardMaterial3D>,
    line_mat: Gd<StandardMaterial3D>,
}

fn unshaded(priority: i32, additive: bool) -> Gd<StandardMaterial3D> {
    let mut m = StandardMaterial3D::new_gd();
    m.set_shading_mode(ShadingMode::UNSHADED);
    m.set_flag(Flags::DISABLE_DEPTH_TEST, true);
    m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
    m.set_transparency(Transparency::ALPHA);
    if additive {
        m.set_blend_mode(BlendMode::ADD);
    }
    m.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
    m.set_render_priority(priority);
    m
}

impl Cursor {
    pub fn new(terrain: &mut Gd<MarchingSquaresTerrain>) -> Cursor {
        let mut node = MeshInstance3D::new_alloc();
        node.set_name("MarchingSquaresTerrainCursor");
        let mesh = ImmediateMesh::new_gd();
        node.set_mesh(&mesh);
        node.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
        // Keep the gizmo out of the pixel renderer's metadata pass (layer 20 by convention):
        // the editor camera sees layer 1.
        node.set_layer_mask(1);
        add_internal(terrain, &node);
        let mut brush_mat = ShaderMaterial::new_gd();
        if let Some(s) = ResourceLoader::singleton().load(SHADER_PATH).and_then(|r| r.try_cast::<Shader>().ok()) {
            brush_mat.set_shader(&s);
        }
        brush_mat.set_render_priority(11);
        Cursor { node, mesh, brush_mat, cell_mat: unshaded(12, false), line_mat: unshaded(13, false) }
    }

    pub fn is_valid(&self) -> bool {
        self.node.is_instance_valid()
    }

    pub fn free(self) {
        if self.node.is_instance_valid() {
            let mut n = self.node;
            n.queue_free();
        }
    }

    pub fn set_visible(&mut self, v: bool) {
        if self.node.is_instance_valid() {
            self.node.set_visible(v);
        }
    }

    pub fn draw(&mut self, t: &MarchingSquaresTerrain, sc: &Scene) {
        self.mesh.clear_surfaces();
        let cfg = &t.store().config;
        let ground = |p: PVec2, fallback: f32| t.local_height_at(p).unwrap_or(fallback) + LIFT;

        // ---- brush decal
        if let (Some(h), Some(b)) = (sc.hover, sc.brush) {
            let r = b.radii().x.max(b.radii().y);
            self.brush_mat.set_shader_parameter("shape", &(b.shape as u8 as i32).to_variant());
            self.brush_mat.set_shader_parameter("falloff_visible", &b.falloff.to_variant());
            self.brush_mat.set_shader_parameter("falloff_ease", &b.falloff_ease.to_variant());
            let n = ((r / cfg.cell_size_safe().x.max(0.05)) * 2.0).clamp(12.0, 48.0) as i32;
            let ext = 1.08;
            let vert = |i: i32, j: i32| {
                let d = v2(-ext + 2.0 * ext * i as f32 / n as f32, -ext + 2.0 * ext * j as f32 / n as f32);
                // Unit brush space -> stretched / rotated terrain offset (anisotropic brushes).
                let p = v2(h.x, h.z) + b.from_unit(d);
                (Vector3::new(p.x, ground(p, h.y), p.y), Vector2::new(d.x, d.y))
            };
            let grid: Vec<Vec<(Vector3, Vector2)>> = (0..=n).map(|j| (0..=n).map(|i| vert(i, j)).collect()).collect();
            self.mesh.surface_begin_ex(PrimitiveType::TRIANGLES).material(&self.brush_mat.clone().upcast::<Material>()).done();
            for j in 0..n as usize {
                for i in 0..n as usize {
                    let q = [grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]];
                    for k in [0, 1, 2, 0, 2, 3] {
                        self.mesh.surface_set_uv(q[k].1);
                        self.mesh.surface_add_vertex(q[k].0);
                    }
                }
            }
            self.mesh.surface_end();
        }

        // ---- per-sample markers (+ cell outlines, so the selection reads on snow as well as grass)
        let mut lines: Vec<(Vector3, Vector3, Color)> = Vec::new();
        if !sc.marks.is_empty() {
            // Original colors (hex blue-white, triangle warm, square grey), a little more
            // saturated and alpha-blended instead of additive so they show on bright ground.
            let (base, edge, cell_shape) = match cfg.grid {
                GridType::Hex | GridType::HexRings => (Color::from_rgba(0.55, 0.75, 1.0, 0.5), Color::from_rgba(0.12, 0.3, 0.75, 0.9), true),
                GridType::Triangle => (Color::from_rgba(1.0, 0.75, 0.5, 0.5), Color::from_rgba(0.6, 0.3, 0.08, 0.9), true),
                GridType::Square => (Color::from_rgba(0.85, 0.85, 0.85, 0.55), Color::from_rgba(0.15, 0.15, 0.15, 0.8), false),
            };
            let cs = cfg.cell_size_safe();
            let half = (cs.x + cs.y) / 8.0;
            self.mesh.surface_begin_ex(PrimitiveType::TRIANGLES).material(&self.cell_mat.clone().upcast::<Material>()).done();
            let mut any = false;
            for (s, w) in sc.marks.iter().take(6000) {
                let y = t.store().height(*s).unwrap_or(0.0) + LIFT;
                let c = cfg.sample_pos(*s);
                let k = 0.25 + 0.65 * w.clamp(0.0, 1.0);
                let poly: Vec<PVec2> = if cell_shape {
                    let corners = cfg.cell_corners(*s);
                    // Full-size cell outline: grid-aligned stamps read as whole cells.
                    let e = Color::from_rgba(edge.r, edge.g, edge.b, edge.a * (0.5 + 0.5 * w.clamp(0.0, 1.0)));
                    let lift = |p: PVec2| Vector3::new(p.x, y + 0.01, p.y);
                    for i in 0..corners.len() {
                        lines.push((lift(corners[i]), lift(corners[(i + 1) % corners.len()]), e));
                    }
                    corners.into_iter().map(|p| c + (p - c) * k).collect()
                } else {
                    let e = half * (0.35 + 0.65 * w.clamp(0.0, 1.0));
                    vec![c + v2(-e, -e), c + v2(e, -e), c + v2(e, e), c + v2(-e, e)]
                };
                if poly.len() < 3 {
                    continue;
                }
                let col = Color::from_rgba(base.r, base.g, base.b, base.a * (0.35 + 0.65 * w));
                for i in 1..poly.len() - 1 {
                    for p in [poly[0], poly[i], poly[i + 1]] {
                        self.mesh.surface_set_color(col);
                        self.mesh.surface_add_vertex(Vector3::new(p.x, y, p.y));
                    }
                }
                any = true;
            }
            if !any {
                // An empty surface is invalid: add a degenerate triangle.
                for _ in 0..3 {
                    self.mesh.surface_add_vertex(Vector3::ZERO);
                }
            }
            self.mesh.surface_end();
        }

        // ---- lines (cell outlines above, chunk outlines, bridge line)
        if sc.show_chunk_lines && !sc.chunk_mode {
            let col = Color::from_rgba(ui_kit::COLOR_PATTERN.r, ui_kit::COLOR_PATTERN.g, ui_kit::COLOR_PATTERN.b, 0.35);
            for c in t.store().chunk_coords() {
                push_loop(&mut lines, &cfg.chunk_outline(c), |p| ground(p, 0.0), col);
            }
        }
        // ---- Chunk Manager gizmos (the original's chunk gizmo): every chunk outlined on its
        // real cell boundary, a "+" handle on every empty neighbour a click would add, red "X"
        // on the hovered chunk (click removes it), blue frame on the selected chunk.
        let mut ribbons: Vec<(Vector3, Vector3, Color, f32)> = Vec::new();
        let mut discs: Vec<(Vector3, f32, Color)> = Vec::new();
        if sc.chunk_mode {
            let store = t.store();
            let cs = cfg.cell_size_safe();
            let w = (cs.x + cs.y) * 0.5 * 0.16;
            let chunks = store.chunk_coords();
            let selected = sc.selected_chunk.filter(|c| store.has_chunk(*c));
            for c in &chunks {
                let (col, width) = if Some(*c) == sc.hover_chunk {
                    (ui_kit::COLOR_REMOVE, w * 1.6)
                } else if Some(*c) == selected {
                    (Color::from_rgb(0.3, 0.55, 1.0), w * 1.6)
                } else {
                    (Color::from_rgba(1.0, 1.0, 1.0, 0.85), w)
                };
                for (a, b) in chunk_edges(t, *c, true) {
                    lines.push((a, b, col));
                    ribbons.push((a, b, col, width));
                }
            }
            // Empty neighbours: the chunks a click would add.
            let mut empty: Vec<IVec2> = Vec::new();
            for c in &chunks {
                for n in cfg.chunk_neighbors(*c) {
                    if !store.has_chunk(n) && !empty.contains(&n) {
                        empty.push(n);
                    }
                }
            }
            if chunks.is_empty() && let Some(h) = sc.hover_chunk {
                empty.push(h);
            }
            for c in &empty {
                let hovered = Some(*c) == sc.hover_chunk;
                let a = if hovered { 0.95 } else { 0.4 };
                let col = Color::from_rgba(ui_kit::COLOR_ADD.r * 0.4, ui_kit::COLOR_ADD.g * 0.9, ui_kit::COLOR_ADD.b * 0.4 + 0.1, a);
                for (p, q) in chunk_edges(t, *c, false) {
                    if hovered {
                        ribbons.push((p, q, col, w));
                    }
                    lines.push((p, q, col));
                }
                let (mid, ext) = chunk_center(t, *c);
                let r = ext * if hovered { 0.7 } else { 0.5 };
                let y = LIFT * 3.0;
                let fill = Color::from_rgba(0.1, 0.45, 0.15, if hovered { 0.75 } else { 0.5 });
                discs.push((Vector3::new(mid.x, y, mid.y), r, fill));
                let plus = Color::from_rgba(0.8, 1.0, 0.8, if hovered { 1.0 } else { 0.85 });
                let arm = r * 0.6;
                let pw = (r * 0.18).max(w);
                ribbons.push((Vector3::new(mid.x - arm, y, mid.y), Vector3::new(mid.x + arm, y, mid.y), plus, pw));
                ribbons.push((Vector3::new(mid.x, y, mid.y - arm), Vector3::new(mid.x, y, mid.y + arm), plus, pw));
            }
            // Hovered chunk: red X (remove); selected chunk: blue inner frame (the original's highlight).
            let glyph_y = |p: PVec2| ground(p, 0.0) + 0.1;
            if let Some(c) = sc.hover_chunk.filter(|c| store.has_chunk(*c)) {
                let (mid, ext) = chunk_center(t, c);
                let d = ext * 0.35;
                for (p, q) in [(v2(-d, -d), v2(d, d)), (v2(-d, d), v2(d, -d))] {
                    let (p, q) = (mid + p, mid + q);
                    ribbons.push((Vector3::new(p.x, glyph_y(p), p.y), Vector3::new(q.x, glyph_y(q), q.y), ui_kit::COLOR_REMOVE, w * 1.8));
                }
            }
            if let Some(c) = selected {
                let (mid, ext) = chunk_center(t, c);
                let d = ext * 0.5;
                let sq = [mid + v2(-d, -d), mid + v2(d, -d), mid + v2(d, d), mid + v2(-d, d)];
                for i in 0..4 {
                    let (p, q) = (sq[i], sq[(i + 1) % 4]);
                    ribbons.push((Vector3::new(p.x, glyph_y(p), p.y), Vector3::new(q.x, glyph_y(q), q.y), Color::from_rgb(0.3, 0.55, 1.0), w * 1.4));
                }
            }
        }
        if !ribbons.is_empty() || !discs.is_empty() {
            self.mesh.surface_begin_ex(PrimitiveType::TRIANGLES).material(&self.line_mat.clone().upcast::<Material>()).done();
            for (c, r, col) in &discs {
                let n = 16;
                for i in 0..n {
                    let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
                    let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                    for p in [*c, *c + Vector3::new(a0.cos() * r, 0.0, a0.sin() * r), *c + Vector3::new(a1.cos() * r, 0.0, a1.sin() * r)] {
                        self.mesh.surface_set_color(*col);
                        self.mesh.surface_add_vertex(p);
                    }
                }
            }
            for (a, b, col, width) in &ribbons {
                let d = Vector3::new(b.x - a.x, 0.0, b.z - a.z);
                if d.length() < 1e-5 {
                    continue;
                }
                let side = Vector3::new(-d.z, 0.0, d.x).normalized() * (*width * 0.5);
                let q = [*a - side, *a + side, *b + side, *b - side];
                for k in [0, 1, 2, 0, 2, 3] {
                    self.mesh.surface_set_color(*col);
                    self.mesh.surface_add_vertex(q[k]);
                }
            }
            self.mesh.surface_end();
        }
        if let (Some(a), Some(h)) = (sc.bridge_from, sc.hover) {
            lines.push((a + Vector3::UP * LIFT, h + Vector3::UP * LIFT, Color::from_rgb(1.0, 1.0, 1.0)));
        }
        if !lines.is_empty() {
            self.mesh.surface_begin_ex(PrimitiveType::LINES).material(&self.line_mat.clone().upcast::<Material>()).done();
            for (a, b, c) in lines {
                self.mesh.surface_set_color(c);
                self.mesh.surface_add_vertex(a);
                self.mesh.surface_set_color(c);
                self.mesh.surface_add_vertex(b);
            }
            self.mesh.surface_end();
        }
    }
}

/// Boundary edges of a chunk at its cells' heights (`raised`; else on the y = 0 chunk plane):
/// the real cell outline on cell grids (hex, triangle, hex rings), the square on square grids.
fn chunk_edges(t: &MarchingSquaresTerrain, c: IVec2, raised: bool) -> Vec<(Vector3, Vector3)> {
    let cfg = &t.store().config;
    let mut out = Vec::new();
    if cfg.grid == GridType::Square {
        let o = cfg.chunk_outline(c);
        let h = |p: PVec2| if raised { t.local_height_at(p).unwrap_or(0.0) + LIFT * 2.0 } else { LIFT * 2.0 };
        let mut segs = Vec::new();
        push_loop(&mut segs, &o, h, Color::WHITE);
        return segs.into_iter().map(|(a, b, _)| (a, b)).collect();
    }
    for cell in cfg.chunk_cells(c) {
        let corners = cfg.cell_corners(cell);
        let y = if raised { t.store().height(cell).unwrap_or(0.0) + LIFT * 2.0 } else { LIFT * 2.0 };
        for (e, n) in cfg.edge_neighbors(cell).into_iter().enumerate() {
            if cfg.chunk_of(n) == c {
                continue;
            }
            let (i, j) = cfg.edge_corners(cell, e);
            let (a, b) = (corners[i], corners[j]);
            out.push((Vector3::new(a.x, y, a.y), Vector3::new(b.x, y, b.y)));
        }
    }
    out
}

/// Centre and half-size (terrain-local XZ) of a chunk, for its glyphs.
fn chunk_center(t: &MarchingSquaresTerrain, c: IVec2) -> (PVec2, f32) {
    let cfg = &t.store().config;
    let o = cfg.chunk_outline(c);
    let n = o.len().max(1) as f32;
    let mid = if cfg.grid == GridType::Square { o.iter().fold(v2(0.0, 0.0), |a, p| a + *p) / n } else { cfg.sample_pos(cfg.chunk_center_cell(c)) };
    let ext = o.iter().map(|p| (*p - mid).length()).fold(f32::MAX, f32::min);
    let ext = if ext.is_finite() && ext > 0.0 { ext } else { cfg.cell_size_safe().x * 2.0 };
    (mid, ext)
}

fn push_path(lines: &mut Vec<(Vector3, Vector3, Color)>, pts: &[PVec2], h: impl Fn(PVec2) -> f32, col: Color) {
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Subdivide long edges so lines follow the terrain.
        let steps = ((b - a).length() / 0.75).ceil().clamp(1.0, 64.0) as usize;
        for k in 0..steps {
            let p0 = a.lerp(b, k as f32 / steps as f32);
            let p1 = a.lerp(b, (k + 1) as f32 / steps as f32);
            lines.push((Vector3::new(p0.x, h(p0), p0.y), Vector3::new(p1.x, h(p1), p1.y), col));
        }
    }
}

fn push_loop(lines: &mut Vec<(Vector3, Vector3, Color)>, pts: &[PVec2], h: impl Fn(PVec2) -> f32, col: Color) {
    if pts.is_empty() {
        return;
    }
    let mut closed = pts.to_vec();
    closed.push(pts[0]);
    push_path(lines, &closed, h, col);
}
