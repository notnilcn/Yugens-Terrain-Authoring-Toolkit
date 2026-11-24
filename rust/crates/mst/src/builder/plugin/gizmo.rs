use super::*;
impl PixelBuilderPlugin {
    pub(super) fn secure_handlebar(&mut self) {
        if self.gizmo.is_some() {
            return;
        }
        let Some(mut t) = self.target.clone() else {
            return;
        };
        let mi = super::overlay_mesh::attach_overlay_mesh(&mut t, "PixelBuilderGizmo", 100);
        self.gizmo = Some(mi);
    }
    pub(super) fn drop_gizmo(&mut self) {
        self.drop_attr_overlay();
        if let Some(g) = self.gizmo.take()
            && g.is_instance_valid()
        {
            let mut g = g;
            g.queue_free();
        }
    }
    pub(super) fn redraw(&mut self) {
        self.refresh_attr_overlay();
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            let (nf, nv) = (m.facets.len(), m.positions.len() as u32);
            self.sel_faces.retain(|f| *f < nf);
            self.sel_verts.retain(|v| *v < nv);
            self.sel_edges.retain(|e| e.0 < nv && e.1 < nv);
            if self.hover_face.is_some_and(|f| f >= nf) {
                self.hover_face = None;
            }
            if self.hover_vert.is_some_and(|v| v >= nv) {
                self.hover_vert = None;
            }
            if self.hover_edge.is_some_and(|e| e.0 >= nv || e.1 >= nv) {
                self.hover_edge = None;
            }
        }
        self.refresh_feedback();
        self.base().update_overlays();
        self.tune_tex_coord_studio();
        let Some(g) = &self.gizmo else { return };
        if !g.is_instance_valid() {
            self.gizmo = None;
            return;
        }
        let Some(mesh) = g.get_mesh() else { return };
        let Ok(mut im) = mesh.try_cast::<ImmediateMesh>() else {
            return;
        };
        im.clear_surfaces();
        let Some(data) = self.contents() else { return };
        let db = data.bind();
        let m = db.cage();
        let nf = m.facets.len();
        let nv = m.positions.len() as u32;
        self.sel_faces.retain(|f| *f < nf);
        self.sel_verts.retain(|v| *v < nv);
        self.sel_edges.retain(|e| e.0 < nv && e.1 < nv);
        if self.hover_face.is_some_and(|f| f >= nf) {
            self.hover_face = None;
        }
        if self.mode == Mode::Object {
            return;
        }
        let p = |i: u32| from_p(m.positions[i as usize]);
        let tris = pt_modeler::compile::face_triangles(m);
        let mut tri_pts: Vec<(Vector3, Color)> = Vec::new();
        let faces: Vec<(usize, Color)> = self
            .sel_faces
            .iter()
            .map(|f| (*f, self.prefs.color_face_selected))
            .chain(
                self.hover_face
                    .filter(|f| !self.sel_faces.contains(f))
                    .map(|f| (f, self.prefs.color_face_hover)),
            )
            .collect();
        for (f, c) in faces {
            let n = from_p(m.face_normal(f)) * 0.0045;
            for t in &tris[f] {
                for k in t {
                    tri_pts.push((p(m.facets[f].verts[*k]) + n, c));
                }
            }
        }
        if !tri_pts.is_empty() {
            im.surface_begin(PrimitiveType::TRIANGLES);
            for (v, c) in tri_pts {
                im.surface_set_color(c);
                im.surface_add_vertex(v);
            }
            im.surface_end();
        }
        let sel_edges: BTreeSet<Edge> = match self.mode {
            Mode::Face => m.edges_of_faces(&self.sel_faces),
            Mode::Edge => self.sel_edges.clone(),
            _ => m.edges_within(&self.sel_verts),
        };
        im.surface_begin(PrimitiveType::LINES);
        for e in m.ridges() {
            let c = if sel_edges.contains(&e) {
                self.prefs.color_edge_selected
            } else {
                self.prefs.color_edge
            };
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.0));
            im.surface_set_color(c);
            im.surface_add_vertex(p(e.1));
        }
        im.surface_end();
        if let Some(e) = self.hover_edge.filter(|_| self.mode == Mode::Edge)
            && (e.0 as usize) < m.positions.len()
            && (e.1 as usize) < m.positions.len()
        {
            let c = self.prefs.color_edge_hover;
            let (a, b) = (p(e.0), p(e.1));
            let len = (b - a).length().max(1e-4);
            let w = (len * 0.0125).clamp(0.0045, 0.0305);
            let dir = (b - a) / len;
            let side = if dir.cross(Vector3::UP).length() > 0.1005 {
                dir.cross(Vector3::UP).normalized()
            } else {
                dir.cross(Vector3::RIGHT).normalized()
            };
            let up = dir.cross(side).normalized();
            im.surface_begin(PrimitiveType::TRIANGLES);
            for off in [side * w, up * w] {
                for q in [a - off, b - off, b + off, a - off, b + off, a + off] {
                    im.surface_set_color(c);
                    im.surface_add_vertex(q);
                }
            }
            im.surface_end();
        }
    }
    pub(super) fn sync_gizmo_lock(&mut self) {
        let want = self.target.clone().filter(|t| {
            t.is_instance_valid()
                && self.visible
                && (self.mode != Mode::Object
                    || self.roblox_wants_gizmo_lock()
                    || matches!(
                        self.tool,
                        Tool::Paint | Tool::Knife | Tool::VertexPaint | Tool::Bezier
                    ))
        });
        if want
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .map(|t| t.instance_id())
            == self
                .lock_target
                .as_ref()
                .filter(|t| t.is_instance_valid())
                .map(|t| t.instance_id())
        {
            return;
        }
        self.set_gizmo_lock(want);
    }
    pub(super) fn set_gizmo_lock(&mut self, want: Option<Gd<PixelBuilderMesh>>) {
        let mut changed = false;
        if let Some(mut old) = self.lock_target.take()
            && old.is_instance_valid()
            && old.has_meta(PLUGIN_LOCK_META)
        {
            old.remove_meta(PLUGIN_LOCK_META);
            old.remove_meta(EDIT_LOCK_META);
            changed = true;
        }
        if let Some(mut t) = want
            && !t.has_meta(EDIT_LOCK_META)
        {
            t.set_meta(EDIT_LOCK_META, &true.to_variant());
            t.set_meta(PLUGIN_LOCK_META, &true.to_variant());
            self.lock_target = Some(t);
            changed = true;
        }
        if changed && let Some(sel) = EditorInterface::singleton().get_selection() {
            sel.upcast::<Object>()
                .call_deferred("emit_signal", &["selection_changed".to_variant()]);
        }
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    pub(super) fn sync_gizmo_lock_deferred(&mut self) {
        self.sync_gizmo_lock();
    }
}
