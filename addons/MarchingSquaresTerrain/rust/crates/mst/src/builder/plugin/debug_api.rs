use super::*;
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    pub(super) fn debug_boolean(&mut self, other: Gd<PixelBuilderMesh>, op: i64) {
        self.boolean_with(other, op);
    }
    #[func]
    pub(super) fn debug_knife(&mut self, face: i64, points: PackedVector3Array) {
        self.assign_setting(Mode::Face);
        self.assign_instrument(Tool::Knife);
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            if (face as usize) < m.facets.len() {
                self.knife.face = Some(face as usize);
            }
        }
        let to_local = self.xform().affine_inverse();
        self.knife.points = points
            .as_slice()
            .iter()
            .map(|p| to_p(to_local * *p))
            .collect();
        self.knife_commit();
    }
    #[func]
    pub(super) fn debug_wall(&mut self, points: PackedVector3Array, closed: bool) {
        self.spec.sort = ShapeKind::Wall;
        self.tool = Tool::Shape;
        self.wall_through(points.as_slice(), closed);
        self.sync_ui();
    }
    #[func]
    pub(super) fn debug_poly(&mut self, outline: PackedVector3Array, holes: Variant) {
        self.spec.sort = ShapeKind::Poly;
        self.tool = Tool::Shape;
        let hs: Vec<Vec<Vector3>> = match holes.try_to::<Array<PackedVector3Array>>() {
            Ok(typed) => typed.iter_shared().map(|h| h.as_slice().to_vec()).collect(),
            Err(_) => holes
                .try_to::<VarArray>()
                .map(|a| {
                    a.iter_shared()
                        .filter_map(|h| h.try_to::<PackedVector3Array>().ok())
                        .map(|h| h.as_slice().to_vec())
                        .collect()
                })
                .unwrap_or_default(),
        };
        self.poly_through(outline.as_slice(), &hs);
        self.sync_ui();
    }
    #[func]
    pub(super) fn debug_input(&mut self, happening: Gd<InputEvent>) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.forward_3d_gui_input(Some(cam), Some(happening)) == AfterGuiInput::STOP.ord()
    }
    #[func]
    pub(super) fn debug_viewport_input(&mut self, idx: i64, happening: Gd<InputEvent>) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(idx as i32)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.forward_3d_gui_input(Some(cam), Some(happening)) == AfterGuiInput::STOP.ord()
    }
    #[func]
    pub(super) fn debug_field(&mut self, token_id: i64, value: f64) {
        self.on_field(value, token_id);
        self.sync_ui();
    }
    #[func]
    pub(super) fn debug_set_xform(&mut self, mode: i64) {
        self.set_xform(match mode {
            1 => XformMode::Rotate,
            2 => XformMode::Scale,
            _ => XformMode::Move,
        });
        self.redraw();
    }
    #[func]
    pub(super) fn debug_preferences(&mut self) -> Option<Gd<godot::classes::PopupPanel>> {
        self.open_preferences();
        self.prefs_panel.as_ref().map(|p| p.popup.clone())
    }
    #[func]
    pub(super) fn debug_shortcuts(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let es = EditorInterface::singleton().get_editor_settings();
        for (i, e) in super::super::shortcuts::ENTRIES.iter().enumerate() {
            let path = super::super::shortcuts::route(e.id);
            let mut row = VarArray::new();
            row.push(&path.to_variant());
            row.push(&super::super::shortcuts::key_text(e.id).to_variant());
            row.push(
                &es.as_ref()
                    .is_some_and(|s| s.has_shortcut(&path))
                    .to_variant(),
            );
            row.push(
                &self
                    .shortcut_table
                    .get(i)
                    .is_some_and(|s| s.is_some())
                    .to_variant(),
            );
            d.set(e.id, &row);
        }
        d
    }
    #[func]
    pub(super) fn debug_tooltip(&self, id: i64) -> VarArray {
        let mut out = VarArray::new();
        if let (Some(a), Some(LayoutUi::Mst(u))) = (Action::from_i64(id), &self.ui)
            && let Some((_, b, _)) = u.buttons.iter().find(|(x, _, _)| *x == a)
        {
            out.push(&b.get_tooltip_text().to_variant());
            out.push(&ui::hover_text(a).to_variant());
        }
        out
    }
    #[func]
    pub(super) fn debug_hover_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("face", self.hover_face.map(|f| f as i64).unwrap_or(-1));
        d.set(
            "edge",
            &self
                .hover_edge
                .map(|e| PackedInt32Array::from(&[e.0 as i32, e.1 as i32][..]))
                .unwrap_or_default(),
        );
        d.set("vert", self.hover_vert.map(|v| v as i64).unwrap_or(-1));
        d.set(
            "info",
            self.feedback.facts.clone().unwrap_or_default().as_str(),
        );
        d.set(
            "dims",
            self.feedback
                .dims
                .as_ref()
                .map(|x| x.1.clone())
                .unwrap_or_default()
                .as_str(),
        );
        d
    }
    #[func]
    pub(super) fn debug_pref(&mut self, key: i64, value: Variant) {
        self.on_pref(value, key);
    }
    #[func]
    pub(super) fn debug_marquee(&mut self, start_pt: Vector2, to: Vector2, shift: bool) {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return;
        };
        self.adjust_commit();
        self.marquee_select(
            &cam,
            Marquee {
                start_pt,
                to,
                additive: shift,
            },
        );
        self.redraw();
    }
    #[func]
    pub(super) fn debug_pick(&mut self, pos: Vector2, shift: bool) -> bool {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return false;
        };
        self.adjust_commit();
        let r = self.choose(&cam, pos, shift);
        self.redraw();
        r
    }
    #[func]
    pub(super) fn debug_hover(&mut self, pos: Vector2) {
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return;
        };
        self.update_hover(&cam, pos);
    }
    #[func]
    pub(super) fn debug_action(&mut self, token_id: i64) {
        let old = (self.no_adjust, self.no_confirm);
        self.no_adjust = true;
        self.no_confirm = true;
        self.upon_deed(token_id);
        (self.no_adjust, self.no_confirm) = old;
    }
    #[func]
    pub(super) fn debug_menus(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set(
            "builder_menu",
            self.builder_menu
                .as_ref()
                .is_some_and(|m| m.is_instance_valid() && m.is_visible()),
        );
        d.set(
            "builder_menu_text",
            self.builder_menu
                .as_ref()
                .map(|m| m.get_text().to_string())
                .unwrap_or_default(),
        );
        d.set(
            "mesh_menu_hidden",
            self.hidden_mesh_menu
                .as_ref()
                .is_some_and(|m| m.is_instance_valid() && !m.is_visible()),
        );
        d
    }
    #[func]
    pub(super) fn debug_insert_vertex(&mut self, a: i64, b: i64, pos: Vector3) {
        let local = to_p(self.xform().affine_inverse() * pos);
        self.apply_edit("Pixel Builder: Insert vertex", |m, _| {
            pt_modeler::mesh_ops::insert_vertex(m, (a as u32, b as u32), local);
        });
    }
    #[func]
    pub(super) fn debug_placement(&mut self, pos: Vector2) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(cam) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
            .and_then(|v| v.get_camera_3d())
        else {
            return d;
        };
        if let Some(p) = self.locus(&cam, pos) {
            d.set("point", p.dot);
            d.set("up", p.frame.col_b());
        }
        d
    }
    #[func]
    pub(super) fn debug_drop_material(
        &mut self,
        material: Gd<godot::classes::Material>,
        pos: Vector2,
    ) -> bool {
        let Some(vp) = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()
        else {
            return false;
        };
        let mut d = VarDictionary::new();
        d.set("type", "resource");
        d.set("resource", &material);
        let data = d.to_variant();
        if !self.can_drop_material(&vp, pos, &data) {
            return false;
        }
        self.material_drag_began(material);
        self.drop_material(&vp, pos, &data);
        self.material_drag_ended();
        true
    }
    #[func]
    pub(super) fn debug_export(&mut self, path: GString) -> GString {
        match self.ship_out_into(&path.to_string()) {
            Ok(_) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }
    #[func]
    pub(super) fn debug_select_edges(&mut self, pairs: PackedInt32Array) -> i64 {
        let Some(n) = self.debug_vertex_count() else {
            return -1;
        };
        self.assign_setting(Mode::Edge);
        self.sel_edges = pairs
            .as_slice()
            .chunks_exact(2)
            .filter(|p| p.iter().all(|v| *v >= 0 && (*v as usize) < n))
            .map(|p| pt_modeler::edge(p[0] as u32, p[1] as u32))
            .collect();
        self.redraw();
        self.sel_edges.len() as i64
    }
    fn debug_vertex_count(&mut self) -> Option<usize> {
        let n = self.contents().map(|d| d.bind().cage().positions.len());
        if n.is_none() {
            self.purge_picked();
            self.redraw();
        }
        n
    }
    #[func]
    pub(super) fn debug_draw_shape(&mut self, start: Vector3, corner: Vector3, height: f32) {
        let cell = self
            .terrain
            .as_ref()
            .map(|t| super::super::node::world_cell(t, start));
        self.shape_drag = Some(ShapeDrag {
            stage: 1,
            commence: Placement {
                dot: start,
                cell,
                frame: Basis::IDENTITY,
            },
            corner,
            stature: height,
            press: Vector2::ZERO,
            append_tail: false,
            height_from: Vector2::ZERO,
        });
        self.seal_form_exported();
    }
    #[func]
    pub(super) fn debug_vpaint(&mut self, points: PackedVector3Array, faces: PackedInt32Array) {
        if self.tool != Tool::VertexPaint {
            self.assign_instrument(Tool::VertexPaint);
        }
        let Some(data) = self.contents() else { return };
        let (before, base) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        self.vp.sweep = Some(vpaint::VStroke::new(before, base));
        for (i, p) in points.as_slice().iter().enumerate() {
            self.vp.hover = Some((
                faces.as_slice().get(i).copied().unwrap_or(0).max(0) as usize,
                *p,
            ));
            self.vp_paint_at();
        }
        if let Some(stroke) = self.vp.sweep.take() {
            let after = data.bind().get_blob();
            let before = stroke.before();
            if after != before {
                self.commit_blob("Pixel Builder: Paint vertex colors", &data, before, after);
            }
        }
        self.redraw();
    }
    #[func]
    pub(super) fn debug_move_selection(&mut self, delta: Vector3) {
        let Some(data) = self.contents() else { return };
        let (before, start) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        let verts = self.selected_verts(&start);
        let weights = self.drag_weights(&start, &verts);
        let frames = self.uv_lock.then(|| {
            pt_modeler::uv_lock::capture_drag_frames(&start, &weights.keys().copied().collect())
        });
        let mut m = start.clone();
        let local = self.xform().basis.inverse() * delta;
        pt_modeler::xform::translate_weighted(&mut m, &weights, to_p(local));
        if let Some(f) = &frames {
            pt_modeler::uv_lock::refit_after_drag(&mut m, f);
        }
        let mut data = data;
        data.bind_mut().substitute(m);
        data.emit_changed();
        let after = data.bind().get_blob();
        if after != before {
            self.commit_blob("Pixel Builder: Move", &data, before, after);
        }
        self.redraw();
    }
    #[func]
    pub(super) fn debug_select_verts(&mut self, verts: PackedInt32Array) -> i64 {
        let Some(n) = self.debug_vertex_count() else {
            return -1;
        };
        let valid = |v: &&i32| **v >= 0 && (**v as usize) < n;
        self.assign_setting(Mode::Vertex);
        self.sel_verts = verts
            .as_slice()
            .iter()
            .filter(valid)
            .map(|v| *v as u32)
            .collect();
        self.active_vert = verts.as_slice().iter().find(valid).map(|v| *v as u32);
        self.redraw();
        self.sel_verts.len() as i64
    }
    #[func]
    pub(super) fn debug_select_faces(&mut self, faces: PackedInt32Array) -> i64 {
        let Some(n) = self.contents().map(|d| d.bind().cage().facets.len()) else {
            self.purge_picked();
            self.redraw();
            return -1;
        };
        self.assign_setting(Mode::Face);
        self.sel_faces = super::super::data::facet_batch(&faces);
        self.sel_faces.retain(|f| *f < n);
        self.redraw();
        self.sel_faces.len() as i64
    }
    #[func]
    pub(super) fn vp_hover_debug(&mut self, point: Vector3, face: i64) {
        self.vp.hover = Some((face.max(0) as usize, point));
        self.base().update_overlays();
    }
    #[func]
    pub(super) fn fetch_condition(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("mode", self.mode.moniker());
        d.set("tool", self.tool.moniker());
        d.set("faces", self.sel_faces.len() as i64);
        d.set("edges", self.sel_edges.len() as i64);
        d.set("verts", self.sel_verts.len() as i64);
        d.set("uv_lock", self.uv_lock);
        d.set(
            "lock_meta",
            self.lock_target
                .as_ref()
                .is_some_and(|t| t.is_instance_valid() && t.has_meta(EDIT_LOCK_META)),
        );
        d.set("message", self.message.as_str());
        d.set("distance", self.span);
        d.set("kind", self.spec.sort as i64);
        d.set("slot", self.pocket);
        d.set("color", self.tint);
        d.set(
            "layout",
            self.ui.as_ref().map(|u| u.layout().moniker()).unwrap_or(""),
        );
        d
    }
    #[func]
    pub(super) fn inspect_tex_coord_studio(&mut self) -> Option<Gd<PixelBuilderUvCanvas>> {
        self.unveil_tex_coord_studio();
        self.uv_canvas.clone()
    }
}
