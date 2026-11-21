use super::PixelBuilderPlugin;
use godot::classes::ImmediateMesh;
use godot::classes::mesh::PrimitiveType;
use godot::prelude::*;
use pt_core::build::compile::{self, CompileOptions};
use pt_core::build::xg_paint;
pub const TINT_ALPHA: f32 = 0.5;
const NORMAL_COLOR: Color = Color::from_rgb(0.4505, 0.8505, 1.0);
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AttrKey {
    target: i64,
    contents: i64,
    revision: u64,
    tint: bool,
    normals: bool,
    magnitude: u32,
}
fn usable_length(v: f32) -> bool {
    v.is_finite() && v >= 0.0
}
impl PixelBuilderPlugin {
    fn attr_key(&self) -> Option<AttrKey> {
        if !self.visible {
            return None;
        }
        let t = self.target.as_ref().filter(|t| t.is_instance_valid())?;
        let d = self.contents()?;
        let tint = self.xg.smooth_tint;
        let normals = self.xg.show_normals && usable_length(self.xg.normal_length);
        if !tint && !normals {
            return None;
        }
        let revision = d.bind().revision();
        Some(AttrKey {
            target: t.instance_id().to_i64(),
            contents: d.instance_id().to_i64(),
            revision,
            tint,
            normals,
            magnitude: self.xg.normal_length.to_bits(),
        })
    }
    pub(super) fn refresh_attr_overlay(&mut self) {
        let key = self.attr_key();
        let alive = self
            .attr_node
            .as_ref()
            .is_some_and(|n| n.is_instance_valid());
        if key.is_none() {
            self.drop_attr_overlay();
            return;
        }
        self.watch_attr_data();
        if alive && self.attr_key == key {
            return;
        }
        let Some(k) = key else { return };
        if !alive {
            self.attr_node = None;
            let Some(mut t) = self.target.clone() else {
                return;
            };
            let mi = super::overlay_mesh::attach_overlay_mesh(&mut t, "PixelBuilderSmoothing", 90);
            self.attr_node = Some(mi);
        }
        let Some(node) = self.attr_node.clone() else {
            return;
        };
        let Some(mesh) = node.get_mesh() else { return };
        let Ok(mut im) = mesh.try_cast::<ImmediateMesh>() else {
            return;
        };
        im.clear_surfaces();
        let Some(data) = self.contents() else { return };
        let db = data.bind();
        let m = db.cage();
        let (mut n_tris, mut n_lines) = (0i64, 0i64);
        if k.tint {
            let tris = compile::face_triangles(m);
            let mut pts: Vec<(Vector3, Color)> = Vec::new();
            for (fi, f) in m.facets.iter().enumerate() {
                if f.glide == 0 || f.verts.len() < 3 {
                    continue;
                }
                let c = xg_paint::gliding_cluster_tint(f.glide);
                let col = Color::from_rgba(c[0], c[1], c[2], TINT_ALPHA);
                let n = m.face_normal(fi);
                let lift = Vector3::new(n.x, n.y, n.z) * 0.0035;
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
            let c = compile::bake(m, &CompileOptions::default());
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
    fn watch_attr_data(&mut self) {
        let data = self.contents();
        if self.attr_data == data {
            return;
        }
        self.unwatch_attr_data();
        if let Some(mut d) = data.clone() {
            d.connect_flags(
                "changed",
                &self.to_gd().callable("on_attr_data_changed"),
                godot::classes::object::ConnectFlags::DEFERRED,
            );
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
    pub(super) fn attr_overlay_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let node = self.attr_node.as_ref().filter(|n| n.is_instance_valid());
        d.set("shown", node.is_some());
        let (tris, lines) = if node.is_some() {
            self.attr_counts
        } else {
            (0, 0)
        };
        d.set("tint_triangles", tris);
        d.set("normal_lines", lines);
        d.set("rebuilds", self.attr_rebuilds);
        if let Some(data) = self.contents() {
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
