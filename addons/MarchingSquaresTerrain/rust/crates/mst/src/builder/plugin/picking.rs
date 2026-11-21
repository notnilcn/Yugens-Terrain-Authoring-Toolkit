use super::*;
pub(super) struct Marquee {
    pub(super) start_pt: Vector2,
    pub(super) to: Vector2,
    pub(super) additive: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Elem {
    Face(usize),
    Edge(Edge),
    Vert(u32),
}
pub(super) fn apply_shift<T: Ord + Copy>(
    set: &mut BTreeSet<T>,
    picked: &BTreeSet<T>,
    shift: bool,
    mode: i64,
) {
    if !shift {
        *set = picked.clone();
        return;
    }
    match mode {
        1 => set.retain(|x| !picked.contains(x)),
        2 => {
            for x in picked {
                if !set.remove(x) {
                    set.insert(*x);
                }
            }
        }
        _ => set.extend(picked.iter().copied()),
    }
}
pub(super) fn trim_stretch_quad(
    a: Vector2,
    b: Vector2,
    lo: Vector2,
    hi: Vector2,
) -> Option<(f32, f32)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}
pub(super) fn point_in_poly(p: Vector2, poly: &[Vector2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
pub(super) fn from_v2(v: pt_core::math::Vec2) -> Vector2 {
    Vector2::new(v.x, v.y)
}
impl PixelBuilderPlugin {
    pub(super) fn local_ray(&self, cam: &Gd<Camera3D>, pos: Vector2) -> (PVec3, PVec3) {
        let inv = self.xform().affine_inverse();
        let o = inv * cam.project_ray_origin(pos);
        let d = (inv.basis * cam.project_ray_normal(pos)).normalized();
        (to_p(o), to_p(d))
    }
    pub(super) fn element_at(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Elem> {
        let d = self.contents()?;
        let xf = self.xform();
        let (o, dir) = self.local_ray(cam, pos);
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| {
                (
                    to_v2(cam.unproject_position(w)),
                    cam.get_global_position().distance_to(w),
                )
            })
        };
        let mouse = to_v2(pos);
        let db = d.bind();
        let m = db.cage();
        let hit = pick::intersect_ray_closest(m, o, dir, true);
        let xray = self.xray;
        let front = |p: PVec3| xray || hit.is_none_or(|h| (p - o).magnitude() <= h.t + 0.0205);
        match self.mode {
            Mode::Face => hit.map(|h| Elem::Face(h.face)),
            Mode::Edge => pick::pick_closest_edge(m, &project, mouse, PICK_PX)
                .filter(|e| front((m.positions[e.0 as usize] + m.positions[e.1 as usize]) * 0.5))
                .map(Elem::Edge),
            Mode::Vertex => pick::pick_closest_vertex(m, &project, mouse, PICK_PX)
                .filter(|v| front(m.positions[*v as usize]))
                .map(Elem::Vert),
            Mode::Object => None,
        }
    }
    pub(super) fn choose(&mut self, cam: &Gd<Camera3D>, pos: Vector2, additive: bool) -> bool {
        let shift_mode = self.prefs.shift_for(self.is_modeler(), false);
        let found = match self.element_at(cam, pos) {
            Some(Elem::Face(f)) => {
                apply_shift(
                    &mut self.sel_faces,
                    &BTreeSet::from([f]),
                    additive,
                    shift_mode,
                );
                true
            }
            Some(Elem::Edge(e)) => {
                apply_shift(
                    &mut self.sel_edges,
                    &BTreeSet::from([e]),
                    additive,
                    shift_mode,
                );
                true
            }
            Some(Elem::Vert(v)) => {
                apply_shift(
                    &mut self.sel_verts,
                    &BTreeSet::from([v]),
                    additive,
                    shift_mode,
                );
                if !additive
                    || self
                        .active_vert
                        .is_none_or(|a| !self.sel_verts.contains(&a))
                {
                    self.active_vert = self.sel_verts.contains(&v).then_some(v);
                }
                true
            }
            None => false,
        };
        if !found && !additive {
            self.purge_picked();
        }
        found
    }
    pub(super) fn marquee_select(&mut self, cam: &Gd<Camera3D>, mq: Marquee) {
        let Some(d) = self.contents() else { return };
        let xf = self.xform();
        let project = |p: PVec3| {
            let w = xf * from_p(p);
            (!cam.is_position_behind(w)).then(|| (to_v2(cam.unproject_position(w)), 0.0))
        };
        let lo = v2(mq.start_pt.x.min(mq.to.x), mq.start_pt.y.min(mq.to.y));
        let hi = v2(mq.start_pt.x.max(mq.to.x), mq.start_pt.y.max(mq.to.y));
        if (hi - lo).magnitude() < 3.0 {
            return;
        }
        let db = d.bind();
        let m = db.cage();
        let verts = pick::vertices_in_rect(m, &project, lo, hi);
        let complete = self.prefs.rect_for(self.is_modeler()) == 1;
        let eye = to_p(xf.affine_inverse() * cam.get_global_position());
        let ortho = cam.get_projection() == godot::classes::camera_3d::ProjectionType::ORTHOGONAL;
        let view_dir = to_p(
            (xf.affine_inverse().basis * -cam.get_global_transform().basis.col_c()).normalized(),
        );
        let seen = |p: PVec3| {
            if self.xray {
                return true;
            }
            let from = if ortho { p - view_dir * 1.0e4 } else { eye };
            !pick::point_hidden(m, from, p, true)
        };
        let scr = |p: PVec3| project(p).map(|s| from_v2(s.0));
        let (glo, ghi) = (from_v2(lo), from_v2(hi));
        let inside = |s: Vector2| s.x >= glo.x && s.x <= ghi.x && s.y >= glo.y && s.y <= ghi.y;
        let mut picked_f = BTreeSet::new();
        let mut picked_e = BTreeSet::new();
        let mut picked_v = BTreeSet::new();
        match self.mode {
            Mode::Face => {
                let cand: BTreeSet<usize> = if complete {
                    m.faces_within(&verts)
                } else {
                    (0..m.facets.len()).collect()
                };
                for f in cand {
                    let c = m.face_center(f);
                    let to_eye = if ortho { -view_dir } else { eye - c };
                    if !self.xray && m.face_normal(f).innerprod(to_eye) <= 0.0 {
                        continue;
                    }
                    let fv = &m.facets[f].verts;
                    let Some(poly) = fv
                        .iter()
                        .map(|v| scr(m.positions[*v as usize]))
                        .collect::<Option<Vec<_>>>()
                    else {
                        continue;
                    };
                    let mut samples: Vec<PVec3> = Vec::new();
                    if complete {
                        samples.push(c);
                    } else {
                        let centre = (glo + ghi) * 0.5;
                        if inside(
                            poly.iter().fold(Vector2::ZERO, |a, p| a + *p) / poly.len() as f32,
                        ) || point_in_poly(centre, &poly)
                        {
                            samples.push(c);
                        }
                        for (i, v) in fv.iter().enumerate() {
                            if inside(poly[i]) {
                                samples.push(m.positions[*v as usize]);
                            }
                            let j = (i + 1) % fv.len();
                            if let Some((t0, t1)) = trim_stretch_quad(poly[i], poly[j], glo, ghi) {
                                let (a, b) =
                                    (m.positions[*v as usize], m.positions[fv[j] as usize]);
                                samples.push(a + (b - a) * ((t0 + t1) * 0.5));
                            }
                        }
                        if point_in_poly(centre, &poly) {
                            let (o, dir) = self.local_ray(cam, centre);
                            if let Some(h) =
                                pick::intersect_ray_closest(m, o, dir, true).filter(|h| h.face == f)
                            {
                                samples.push(h.dot);
                            }
                        }
                    }
                    if self.xray || samples.iter().any(|p| seen(*p)) {
                        if !samples.is_empty() {
                            picked_f.insert(f);
                        }
                    }
                }
            }
            Mode::Edge => {
                let cand: Vec<Edge> = if complete {
                    m.edges_within(&verts).into_iter().collect()
                } else {
                    m.ridges()
                };
                for e in cand {
                    let (a, b) = (m.positions[e.0 as usize], m.positions[e.1 as usize]);
                    let (Some(sa), Some(sb)) = (scr(a), scr(b)) else {
                        continue;
                    };
                    let t = if complete {
                        Some((0.0, 1.0))
                    } else {
                        trim_stretch_quad(sa, sb, glo, ghi)
                    };
                    let Some((t0, t1)) = t else { continue };
                    let samples = [
                        a + (b - a) * ((t0 + t1) * 0.5),
                        a + (b - a) * t0,
                        a + (b - a) * t1,
                    ];
                    if samples.iter().any(|p| seen(*p)) {
                        picked_e.insert(e);
                    }
                }
            }
            Mode::Vertex => {
                for v in verts {
                    if seen(m.positions[v as usize]) {
                        picked_v.insert(v);
                    }
                }
            }
            Mode::Object => {}
        }
        drop(db);
        let shift_mode = self.prefs.shift_for(self.is_modeler(), true);
        match self.mode {
            Mode::Face => apply_shift(&mut self.sel_faces, &picked_f, mq.additive, shift_mode),
            Mode::Edge => apply_shift(&mut self.sel_edges, &picked_e, mq.additive, shift_mode),
            Mode::Vertex => {
                apply_shift(&mut self.sel_verts, &picked_v, mq.additive, shift_mode);
                if self
                    .active_vert
                    .is_none_or(|a| !self.sel_verts.contains(&a))
                {
                    self.active_vert = self.sel_verts.iter().next().copied();
                }
            }
            Mode::Object => {}
        }
        if !mq.additive {
            self.hover_face = None;
        }
    }
    pub(super) fn update_hover(&mut self, cam: &Gd<Camera3D>, pos: Vector2) {
        if self.tool == Tool::Select {
            let el = if self.target.is_some() && self.prefs.hover && self.mode != Mode::Object {
                self.element_at(cam, pos)
            } else {
                None
            };
            let (hf, he, hv) = match el {
                Some(Elem::Face(f)) => (Some(f), None, None),
                Some(Elem::Edge(e)) => (None, Some(e), None),
                Some(Elem::Vert(v)) => (None, None, Some(v)),
                None => (None, None, None),
            };
            if (hf, he, hv) != (self.hover_face, self.hover_edge, self.hover_vert) {
                let gizmo = he != self.hover_edge || hf != self.hover_face;
                self.hover_face = hf;
                self.hover_edge = he;
                self.hover_vert = hv;
                if gizmo {
                    self.redraw();
                } else {
                    self.base().update_overlays();
                }
            }
            return;
        }
        self.hover_place = self.locus(cam, pos);
        self.base().update_overlays();
    }
}
