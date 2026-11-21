use super::*;
pub(super) struct Drag {
    pub(super) axle: Option<usize>,
    pub(super) before: PackedByteArray,
    pub(super) commence: BMesh,
    pub(super) fulcrum: Vector3,
    pub(super) start_t: f32,
    pub(super) start_hit: Vector3,
    pub(super) moniker: &'static str,
    pub(super) last_delta: Vector3,
    pub(super) handle: Option<Handle>,
    pub(super) groups: Option<Vec<BTreeSet<u32>>>,
    pub(super) weights: Weights,
    pub(super) rot: Option<RotateDrag>,
    pub(super) magnify: Option<ScaleDrag>,
    pub(super) snap_anchor: Vector3,
    pub(super) snap_targets: Vec<Vector3>,
    pub(super) snap_hit: Option<Vector3>,
    pub(super) cursor: Vector2,
    pub(super) uv_frames: Option<pt_core::build::uv_lock::Frames>,
    pub(super) axes: [Vector3; 3],
}
impl Drag {
    pub(super) fn lock_uvs(&self, m: &mut BMesh) {
        if let Some(f) = &self.uv_frames {
            pt_core::build::uv_lock::refit_after_drag(m, f);
        }
    }
}
impl PixelBuilderPlugin {
    pub(super) fn fulcrum(&self) -> Option<Vector3> {
        let d = self.contents()?;
        let d = d.bind();
        let m = d.cage();
        let v = self.selected_verts(m);
        if v.is_empty() {
            return None;
        }
        let c = v
            .iter()
            .fold(PVec3::ZERO, |a, i| a + m.positions[*i as usize])
            / v.len() as f32;
        Some(self.xform() * from_p(c))
    }
    pub(super) fn handle_screen(&self, cam: &Gd<Camera3D>) -> Option<(Vector2, [Vector2; 3])> {
        if self.mode == Mode::Object || self.tool != Tool::Select {
            return None;
        }
        let p = self
            .drag
            .as_ref()
            .map(|d| d.fulcrum + d.last_delta)
            .or_else(|| self.fulcrum())?;
        if cam.is_position_behind(p) {
            return None;
        }
        let k = if cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL {
            cam.get_size() * 0.1005
        } else {
            cam.get_global_position().distance_to(p) * 0.1205
        };
        let s0 = cam.unproject_position(p);
        let axes = self.gizmo_axes();
        let ends = axes.map(|a| cam.unproject_position(p + a * k));
        Some((s0, ends))
    }
    pub(super) fn handle_at(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Option<usize>> {
        let (s0, ends) = self.handle_screen(cam)?;
        if (pos - s0).length() < 8.0 {
            return Some(None);
        }
        let mut best: Option<(f32, usize)> = None;
        for (a, e) in ends.iter().enumerate() {
            let ab = *e - s0;
            let t = if ab.length_squared() < 1.0 {
                0.0
            } else {
                ((pos - s0).dot(ab) / ab.length_squared()).clamp(0.0, 1.1505)
            };
            let dist = (s0 + ab * t - pos).length();
            if dist < 7.0 && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, a));
            }
        }
        best.map(|b| Some(b.1))
    }
    pub(super) fn begin_drag(
        &mut self,
        cam: &Gd<Camera3D>,
        pos: Vector2,
        handle: Handle,
        extrude: bool,
    ) {
        let Some(data) = self.contents() else { return };
        let (before, mut start) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        let axis = match handle {
            Handle::Move(a) => a,
            _ => None,
        };
        let mut name = match handle {
            Handle::Move(_) => "Pixel Builder: Move",
            Handle::Rotate(_) => "Pixel Builder: Rotate",
            Handle::Scale(_) => "Pixel Builder: Scale",
        };
        if extrude && self.mode == Mode::Face && !self.sel_faces.is_empty() {
            self.sel_faces = ops::extrude_selection(&mut start, &self.sel_faces, 0.0);
            name = "Pixel Builder: Extrude";
        }
        let verts = match self.mode {
            Mode::Face => start.verts_of_faces(&self.sel_faces),
            Mode::Edge => start.verts_of_edges(&self.sel_edges),
            _ => self.sel_verts.clone(),
        };
        let Some(pivot) = self.fulcrum() else { return };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let axes = self.gizmo_axes();
        let start_t = axis
            .and_then(|a| closest_on_axis(pivot, axes[a], o, d))
            .unwrap_or(0.0);
        let n = cam.get_global_transform().basis.col_c();
        let start_hit = plane_hit(pivot, n, o, d).unwrap_or(pivot);
        let weights = self.drag_weights(&start, &verts);
        let (rot, scale) = match handle {
            Handle::Rotate(a) => (Some(self.begin_rotate(cam, pos, pivot, a)), None),
            Handle::Scale(a) => (None, Some(self.begin_scale(cam, pos, pivot, a))),
            Handle::Move(_) => (None, None),
        };
        let (snap_anchor, snap_targets) = if matches!(handle, Handle::Move(_)) {
            self.snap_setup(cam, pos, &start, &weights, pivot)
        } else {
            (pivot, Vec::new())
        };
        let uv_frames = self.uv_lock.then(|| {
            pt_core::build::uv_lock::capture_drag_frames(&start, &weights.keys().copied().collect())
        });
        let groups = if matches!(handle, Handle::Rotate(_) | Handle::Scale(_)) {
            self.individual_groups(&start, &verts)
        } else {
            None
        };
        self.drag = Some(Drag {
            axle: axis,
            before,
            commence: start,
            fulcrum: pivot,
            start_t,
            start_hit,
            moniker: name,
            last_delta: Vector3::ZERO,
            handle: Some(handle),
            groups,
            weights,
            rot,
            magnify: scale,
            snap_anchor,
            snap_targets,
            snap_hit: None,
            cursor: pos,
            uv_frames,
            axes,
        });
    }
    pub(super) fn drag_motion(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self
            .drag
            .as_ref()
            .is_some_and(|d| d.rot.is_some() || d.magnify.is_some())
        {
            self.drag_rotate_scale(cam, pos);
            return;
        }
        let vsnap = self.snap_offset(cam, pos);
        if let Some(d) = &mut self.drag {
            d.cursor = pos;
        }
        let Some(drag) = &self.drag else { return };
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let axes = drag.axes;
        let mut delta = match drag.axle {
            Some(a) => match closest_on_axis(drag.fulcrum, axes[a], o, d) {
                Some(t) => axes[a] * (t - drag.start_t),
                None => return,
            },
            None => {
                let n = cam.get_global_transform().basis.col_c();
                match plane_hit(drag.fulcrum, n, o, d) {
                    Some(h) => h - drag.start_hit,
                    None => return,
                }
            }
        };
        if self.snap_cell
            && drag.axle != Some(1)
            && let Some(t) = &self.terrain
        {
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * (drag.fulcrum + delta);
            let q = snap::nearest_cell_point(&t.bind().store().config, v2(l.x, l.z), true);
            let w = xf * Vector3::new(q.x, l.y, q.y);
            delta.x = w.x - drag.fulcrum.x;
            delta.z = w.z - drag.fulcrum.z;
            if drag.axle.is_some() {
                let a = axes[drag.axle.unwrap_or(0)];
                delta = a * delta.dot(a);
            }
            if self.snap_grid {
                delta.y = snap::snap_value(delta.y, self.grid_step);
            }
        } else if self.snap_grid {
            delta = from_p(snap::snap_vec(to_p(delta), self.grid_step));
        }
        if let Some(v) = vsnap {
            delta = v;
        }
        if delta == drag.last_delta {
            self.base().update_overlays();
            return;
        }
        let local = self.xform().basis.inverse() * delta;
        let mut m = drag.commence.clone();
        pt_core::build::xform::translate_weighted(&mut m, &drag.weights, to_p(local));
        drag.lock_uvs(&mut m);
        if let Some(dr) = &mut self.drag {
            dr.last_delta = delta;
        }
        if let Some(mut data) = self.contents() {
            data.bind_mut().substitute(m);
            data.emit_changed();
        }
        self.redraw();
    }
    pub(super) fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        let Some(data) = self.contents() else { return };
        let after = data.bind().get_blob();
        if after != drag.before {
            self.commit_blob(drag.moniker, &data, drag.before, after);
        }
    }
    pub(super) fn cancel_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        if let Some(mut data) = self.contents() {
            data.bind_mut().set_blob(drag.before);
        }
        self.redraw();
    }
}
