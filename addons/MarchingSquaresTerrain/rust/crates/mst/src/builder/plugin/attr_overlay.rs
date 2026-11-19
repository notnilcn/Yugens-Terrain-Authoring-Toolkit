//! Smoothing display of the edited mesh: faces tinted by smoothing group and the shading
//! normals as lines (both layouts: the MST palette's Smoothing section, the Xogot Paint popover's
//! Smoothing page).
//!
//! Port of xogot-rust (MIT) `xogot_modeler/src/shared/attribute_drawing.rs`
//! (`draw_attributes`: one alpha-blended, unshaded, two-sided surface per smoothing group;
//! `build_normals`: one line per mesh record, from the vertex along its compiled normal by
//! `normal_length`; `usable_normal_length`) with the defaults of `editor_core` `editor_state.rs`
//! (`normal_length` 0.1, `smoothing_opacity` 0.5).
//!
//! Builder differences: the group tint is `xg_paint::smoothing_group_color` (the chip colour,
//! so the viewport and the chips agree) and faceted faces (group 0) stay clear; the normals are
//! the builder's compiled (rendered) normals. The overlay is an internal `MeshInstance3D` child of
//! the edited mesh holding an `ImmediateMesh`, rebuilt only when its [`AttrKey`] changes (mesh
//! revision, settings, target, visibility): hover redraws and idle frames cost a comparison.

use godot::classes::base_material_3d::{CullMode, Flags, ShadingMode, Transparency};
use godot::classes::mesh::PrimitiveType;
use godot::classes::{ImmediateMesh, MeshInstance3D, StandardMaterial3D};
use godot::prelude::*;

use pt_core::build::compile::{self, CompileOptions};
use pt_core::build::xg_paint;

use super::PixelBuilderPlugin;
use crate::util::add_internal;

/// `smoothing_opacity` (Xogot default 0.5).
pub const TINT_ALPHA: f32 = 0.5;
/// Normal line colour.
const NORMAL_COLOR: Color = Color::from_rgb(0.45, 0.85, 1.0);

/// What the overlay was built for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AttrKey {
    target: i64,
    /// The data resource (a swapped `data` with the same revision must rebuild).
    data: i64,
    revision: u64,
    tint: bool,
    normals: bool,
    length: u32,
}

/// Port of `usable_normal_length`: finite and not negative.
fn usable_length(v: f32) -> bool {
    v.is_finite() && v >= 0.0
}

impl PixelBuilderPlugin {
    fn attr_key(&self) -> Option<AttrKey> {
        if !self.visible {
            return None;
        }
        let t = self.target.as_ref().filter(|t| t.is_instance_valid())?;
        let d = self.data()?;
        let tint = self.xg.smooth_tint;
        let normals = self.xg.show_normals && usable_length(self.xg.normal_length);
        if !tint && !normals {
            return None;
        }
        let revision = d.bind().revision();
        Some(AttrKey { target: t.instance_id().to_i64(), data: d.instance_id().to_i64(), revision, tint, normals, length: self.xg.normal_length.to_bits() })
    }

    /// Rebuilds the overlay when its key changed; frees it when nothing is shown.
    pub(super) fn refresh_attr_overlay(&mut self) {
        let key = self.attr_key();
        let alive = self.attr_node.as_ref().is_some_and(|n| n.is_instance_valid());
        if key.is_none() {
            self.drop_attr_overlay();
            return;
        }
        // Follow the data resource even when the overlay is current (a swap is also in the key).
        self.watch_attr_data();
        if alive && self.attr_key == key {
            return;
        }
        let Some(k) = key else { return };
        if !alive {
            self.attr_node = None;
            let Some(mut t) = self.target.clone() else { return };
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name("PixelBuilderSmoothing");
            mi.set_mesh(&ImmediateMesh::new_gd());
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_shading_mode(ShadingMode::UNSHADED);
            mat.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
            mat.set_transparency(Transparency::ALPHA);
            mat.set_cull_mode(CullMode::DISABLED);
            mat.set_render_priority(90);
            mi.set_material_override(&mat);
            mi.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
            mi.set_layer_mask(1);
            mi.add_to_group("pixel_ignore");
            add_internal(&mut t, &mi);
            self.attr_node = Some(mi);
        }
        let Some(node) = self.attr_node.clone() else { return };
        let Some(mesh) = node.get_mesh() else { return };
        let Ok(mut im) = mesh.try_cast::<ImmediateMesh>() else { return };
        im.clear_surfaces();
        let Some(data) = self.data() else { return };
        let db = data.bind();
        let m = db.mesh();
        let (mut n_tris, mut n_lines) = (0i64, 0i64);
        if k.tint {
            let tris = compile::face_triangles(m);
            let mut pts: Vec<(Vector3, Color)> = Vec::new();
            for (fi, f) in m.faces.iter().enumerate() {
                if f.smooth == 0 || f.verts.len() < 3 {
                    continue;
                }
                let c = xg_paint::smoothing_group_color(f.smooth);
                let col = Color::from_rgba(c[0], c[1], c[2], TINT_ALPHA);
                let n = m.face_normal(fi);
                let lift = Vector3::new(n.x, n.y, n.z) * 0.003;
                for t in &tris[fi] {
                    for i in t {
                        let p = m.positions[f.verts[*i] as usize];
                        pts.push((Vector3::new(p.x, p.y, p.z) + lift, col));
                    }
                }
            }
            n_tris = pts.len() as i64 / 3;
            if !pts.is_empty() {
                im.surface_begin(PrimitiveType::TRIANGLES);
                for (p, c) in pts {
                    im.surface_set_color(c);
                    im.surface_add_vertex(p);
                }
                im.surface_end();
            }
        }
        if k.normals {
            let len = self.xg.normal_length;
            let c = compile::compile(m, &CompileOptions::default());
            let any = c.surfaces.iter().any(|s| !s.positions.is_empty());
            if any {
                im.surface_begin(PrimitiveType::LINES);
                for s in &c.surfaces {
                    for (p, n) in s.positions.iter().zip(&s.normals) {
                        let a = Vector3::new(p[0], p[1], p[2]);
                        let b = a + Vector3::new(n[0], n[1], n[2]) * len;
                        im.surface_set_color(NORMAL_COLOR);
                        im.surface_add_vertex(a);
                        im.surface_set_color(NORMAL_COLOR);
                        im.surface_add_vertex(b);
                        n_lines += 1;
                    }
                }
                im.surface_end();
            }
        }
        drop(db);
        self.attr_counts = (n_tris, n_lines);
        self.attr_key = key;
        self.attr_rebuilds += 1;
    }

    /// Follows the shown data's `changed` (deferred: it fires inside the plugin's own edits),
    /// so undo / redo and scripts update the overlay without a redraw.
    fn watch_attr_data(&mut self) {
        let data = self.data();
        if self.attr_data == data {
            return;
        }
        self.unwatch_attr_data();
        if let Some(mut d) = data.clone() {
            d.connect_flags("changed", &self.to_gd().callable("on_attr_data_changed"), godot::classes::object::ConnectFlags::DEFERRED);
        }
        self.attr_data = data;
    }

    fn unwatch_attr_data(&mut self) {
        let cb = self.to_gd().callable("on_attr_data_changed");
        if let Some(mut d) = self.attr_data.take()
            && d.is_instance_valid()
            && d.is_connected("changed", &cb)
        {
            d.disconnect("changed", &cb);
        }
    }

    pub(super) fn drop_attr_overlay(&mut self) {
        self.unwatch_attr_data();
        self.attr_key = None;
        if let Some(mut n) = self.attr_node.take()
            && n.is_instance_valid()
        {
            n.queue_free();
        }
    }

    /// Test hook state: `{shown, surfaces, rebuilds}`.
    pub(super) fn attr_overlay_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let node = self.attr_node.as_ref().filter(|n| n.is_instance_valid());
        d.set("shown", node.is_some());
        let (tris, lines) = if node.is_some() { self.attr_counts } else { (0, 0) };
        d.set("tint_triangles", tris);
        d.set("normal_lines", lines);
        d.set("rebuilds", self.attr_rebuilds);
        if let Some(data) = self.data() {
            d.set("revision", data.bind().revision() as i64);
        }
        d
    }
}

#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_attr_data_changed(&mut self) {
        self.refresh_attr_overlay();
    }
}
