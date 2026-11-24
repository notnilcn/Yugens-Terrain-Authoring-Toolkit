use super::*;
#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) dot: Vector3,
    pub(super) cell: Option<Vector2i>,
    pub(super) frame: Basis,
}
pub(super) fn frame_from_upright(n: Vector3) -> Basis {
    let n = n.normalized();
    if n.dot(Vector3::UP) > 0.9995 {
        return Basis::IDENTITY;
    }
    let r = if n.x.abs() < 0.9005 {
        Vector3::RIGHT
    } else {
        Vector3::BACK
    };
    let t = (r - n * r.dot(n)).normalized();
    Basis::from_cols(t, n, t.cross(n))
}
impl PixelBuilderPlugin {
    pub(super) fn chosen_terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.target
            .as_ref()
            .filter(|t| t.is_instance_valid())
            .and_then(|t| t.bind().assigned_terrain())
            .or_else(edited_scene_terrain)
    }
}
fn edited_scene_terrain() -> Option<Gd<MarchingSquaresTerrain>> {
    let root = EditorInterface::singleton().get_edited_scene_root()?;
    if let Ok(t) = root.clone().try_cast::<MarchingSquaresTerrain>() {
        return Some(t);
    }
    root.find_children_ex("*")
        .type_("MarchingSquaresTerrain")
        .owned(false)
        .done()
        .iter_shared()
        .find_map(|n| n.try_cast().ok())
}
fn scene_builder_meshes() -> Vec<Gd<PixelBuilderMesh>> {
    let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
        return Vec::new();
    };
    let mut out: Vec<Gd<PixelBuilderMesh>> = root
        .find_children_ex("*")
        .type_("PixelBuilderMesh")
        .owned(false)
        .done()
        .iter_shared()
        .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
        .collect();
    if let Ok(r) = root.try_cast::<PixelBuilderMesh>() {
        out.push(r);
    }
    out
}
pub(super) fn builder_mesh_bounds() -> Vec<(PVec3, PVec3)> {
    scene_builder_meshes()
        .iter()
        .filter(|b| b.is_visible_in_tree())
        .filter_map(|b| {
            let data = b.bind().contents()?;
            let (lo, hi) = data.bind().cage().extent()?;
            let xf = b.get_global_transform();
            let mut wl = Vector3::splat(f32::INFINITY);
            let mut wh = Vector3::splat(f32::NEG_INFINITY);
            for i in 0..8 {
                let c = Vector3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                );
                let w = xf * c;
                wl = wl.coord_min(w);
                wh = wh.coord_max(w);
            }
            Some((to_p(wl), to_p(wh)))
        })
        .collect()
}
fn nearest_builder_hit(o: Vector3, d: Vector3) -> Option<(f32, Vector3, Gd<PixelBuilderMesh>)> {
    let mut best: Option<(f32, Vector3, Gd<PixelBuilderMesh>)> = None;
    for b in scene_builder_meshes() {
        if !b.is_visible_in_tree() {
            continue;
        }
        let Some(data) = b.bind().contents() else {
            continue;
        };
        let xf = b.get_global_transform();
        let inv = xf.affine_inverse();
        if let Some(h) = pick::intersect_ray_closest(
            data.bind().cage(),
            to_p(inv * o),
            to_p((inv.basis * d).normalized()),
            true,
        ) {
            let w = xf * from_p(h.dot);
            let dist = o.distance_to(w);
            if best.as_ref().is_none_or(|bb| dist < bb.0) {
                best = Some((dist, w, b.clone()));
            }
        }
    }
    best
}
impl PixelBuilderPlugin {
    pub(super) fn view_ray_hit(&self, o: Vector3, d: Vector3) -> Option<Vector3> {
        let mut best: Option<(f32, Vector3)> = None;
        if let Some(t) = self.terrain.as_ref().filter(|t| t.is_instance_valid()) {
            let xf = t.get_global_transform();
            let inv = xf.affine_inverse();
            if let Some(h) = t
                .bind()
                .local_raycast(inv * o, (inv.basis * d).normalized(), 20_000.0)
            {
                let w = xf * h;
                best = Some((o.distance_to(w), w));
            }
        }
        if let Some((dist, w, _)) = nearest_builder_hit(o, d)
            && best.is_none_or(|bb| dist < bb.0)
        {
            best = Some((dist, w));
        }
        best.map(|b| b.1)
    }
    pub(super) fn builder_mesh_under(&self, o: Vector3, d: Vector3) -> Option<Gd<PixelBuilderMesh>> {
        nearest_builder_hit(o, d).map(|h| h.2)
    }
    pub(super) fn locus(&self, cam: &Gd<Camera3D>, pos: Vector2) -> Option<Placement> {
        let o = cam.project_ray_origin(pos);
        let d = cam.project_ray_normal(pos);
        let mut best: Option<(f32, Vector3, bool)> = None;
        let mut face_normal: Option<Vector3> = None;
        if let Some(t) = &self.terrain {
            let xf = t.get_global_transform();
            let inv = xf.affine_inverse();
            if let Some(h) = t
                .bind()
                .local_raycast(inv * o, (inv.basis * d).normalized(), 20_000.0)
            {
                let w = xf * h;
                best = Some((o.distance_to(w), w, true));
            }
        }
        if let Some(root) = EditorInterface::singleton().get_edited_scene_root() {
            for n in root
                .find_children_ex("*")
                .type_("PixelBuilderMesh")
                .owned(false)
                .done()
                .iter_shared()
            {
                let Ok(b) = n.try_cast::<PixelBuilderMesh>() else {
                    continue;
                };
                if self.tool == Tool::Place
                    && self
                        .target
                        .as_ref()
                        .is_some_and(|t| t.instance_id() == b.instance_id())
                {
                    continue;
                }
                if self.tool == Tool::Bezier
                    && self
                        .bz
                        .editing
                        .as_ref()
                        .is_some_and(|t| t.instance_id() == b.instance_id())
                {
                    continue;
                }
                let Some(data) = b.bind().contents() else {
                    continue;
                };
                let xf = b.get_global_transform();
                let inv = xf.affine_inverse();
                if let Some(h) = pick::intersect_ray_closest(
                    data.bind().cage(),
                    to_p(inv * o),
                    to_p((inv.basis * d).normalized()),
                    true,
                ) {
                    let w = xf * from_p(h.dot);
                    let dist = o.distance_to(w);
                    if best.is_none_or(|bb| dist < bb.0) {
                        best = Some((dist, w, false));
                        face_normal = Some(
                            (xf.basis.inverse().transposed()
                                * from_p(data.bind().cage().face_normal(h.face)))
                            .normalized(),
                        );
                    }
                }
            }
        }
        if self.surface_draw
            && let (Some((_, w, false)), Some(n)) = (best, face_normal)
        {
            let basis = frame_from_upright(n);
            let mut point = w;
            if self.snap_grid {
                let mut l = basis.inverse() * w;
                l.x = snap::snap_value(l.x, self.grid_step);
                l.z = snap::snap_value(l.z, self.grid_step);
                point = basis * l;
            }
            return Some(Placement {
                dot: point,
                cell: None,
                frame: basis,
            });
        }
        let (mut point, on_terrain) = match best {
            Some((_, w, t)) => (w, t),
            None => {
                if d.y.abs() < 1e-5 {
                    return None;
                }
                let k = -o.y / d.y;
                if k <= 0.0 {
                    return None;
                }
                (o + d * k, false)
            }
        };
        let mut cell = None;
        if self.snap_cell
            && let Some(t) = &self.terrain
        {
            let xf = t.get_global_transform();
            let l = xf.affine_inverse() * point;
            let tb = t.bind();
            let (c, ctr) = snap::cell_center(&tb.store().config, v2(l.x, l.z));
            drop(tb);
            cell = Some(from_iv(c));
            if on_terrain && let Some(p) = super::super::node::cell_world(t, from_iv(c)) {
                point = p;
            } else {
                point = xf * Vector3::new(ctr.x, l.y, ctr.y);
            }
        } else if self.snap_grid {
            point.x = snap::snap_value(point.x, self.grid_step);
            point.z = snap::snap_value(point.z, self.grid_step);
            if !on_terrain && best.is_none() {
                point.y = snap::snap_value(point.y, self.grid_step);
            }
        }
        Some(Placement {
            dot: point,
            cell,
            frame: Basis::IDENTITY,
        })
    }
    pub(super) fn form_cage(&self, cell: Option<Vector2i>) -> BMesh {
        let tcfg = self
            .terrain
            .as_ref()
            .map(|t| t.bind().store().config.clone())
            .filter(|c| c.grid.is_cell_mode());
        match (self.spec.sort.is_grid_shape(), &tcfg, cell) {
            (true, Some(cfg), Some(c)) => {
                let r = if self.spec.sort == ShapeKind::HexPrism {
                    0
                } else {
                    self.spec.rings.min(16) as i32
                };
                let cells = shapes::cluster_cells(cfg, to_iv(c), r);
                shapes::cells_prism(cfg, &cells, to_iv(c), self.spec.size.y)
            }
            _ => shapes::produce(&self.spec, tcfg.as_ref()),
        }
    }
    pub(super) fn finish_shape(&self, mut m: BMesh) -> BMesh {
        let c = ui_color_bytes(self.tint);
        for f in &mut m.facets {
            f.stuff = self.pocket.clamp(0, 255) as u16;
            f.tint = c;
        }
        m
    }
    pub(super) fn stamp_shape(&mut self, append: bool) {
        let Some(p) = self.hover_place else {
            self.message = "Nothing under the cursor".into();
            return;
        };
        let m = self.form_cage(p.cell);
        self.create_from_mesh(p, m, append);
    }
    pub(super) fn create_from_mesh(&mut self, p: Placement, m: BMesh, append: bool) {
        let spec = self.spec;
        self.create_with_recipe(p, m, append, xg_io::Recipe::Shape(spec));
    }
    pub(super) fn place_target(&mut self) {
        let (Some(t), Some(terrain), Some(p)) =
            (self.target.clone(), self.terrain.clone(), self.hover_place)
        else {
            self.message = "Place needs a selected mesh and a terrain under the cursor".into();
            return;
        };
        let Some(cell) = p
            .cell
            .or_else(|| Some(super::super::node::world_cell(&terrain, p.dot)))
        else {
            return;
        };
        let Some(mut ur) = self.open_action("Pixel Builder: Place on cell") else {
            return;
        };
        ur.add_do_method(
            &t,
            "place_on_cell",
            &[terrain.to_variant(), cell.to_variant()],
        );
        ur.add_undo_property(&t, "position", &t.get_position().to_variant());
        ur.add_undo_property(&t, "anchor_cell", &t.get("anchor_cell"));
        ur.add_undo_property(&t, "anchored", &t.get("anchored"));
        ur.commit_action();
        self.redraw();
    }
}
