use super::*;
#[derive(Default)]
pub(super) struct Feedback {
    pub(super) facts: Option<String>,
    pub(super) dims: Option<(Vector3, String)>,
}
impl PixelBuilderPlugin {
    pub(super) fn refresh_feedback(&mut self) {
        self.feedback = Feedback::default();
        let Some(d) = self.contents() else { return };
        let db = d.bind();
        let m = db.cage();
        if self.prefs.show_scene_info {
            self.feedback.facts = Some(format!(
                "{} verts  {} edges  {} faces  |  selected: {} faces {} edges {} verts",
                m.positions.len(),
                m.ridges().len(),
                m.facets.len(),
                self.sel_faces.len(),
                self.sel_edges.len(),
                self.sel_verts.len()
            ));
        }
        if self.prefs.show_dimensions {
            let verts: Vec<u32> = if self.mode == Mode::Object {
                (0..m.positions.len() as u32).collect()
            } else {
                self.selected_verts(m).into_iter().collect()
            };
            if !verts.is_empty() {
                let xf = self.xform();
                let mut lo = Vector3::splat(f32::INFINITY);
                let mut hi = Vector3::splat(f32::NEG_INFINITY);
                for v in verts {
                    let w = xf * from_p(m.positions[v as usize]);
                    lo = lo.coord_min(w);
                    hi = hi.coord_max(w);
                }
                let s = hi - lo;
                let anchor = Vector3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5);
                self.feedback.dims =
                    Some((anchor, format!("{:.3} x {:.3} x {:.3}", s.x, s.y, s.z)));
            }
        }
    }
    pub(super) fn draw_viewport_overlay(&mut self, overlay: Option<Gd<Control>>) {
        let Some(mut overlay) = overlay else { return };
        self.drop_freed();
        self.ensure_drop_target(&overlay);
        let cam = self
            .camera
            .clone()
            .filter(|c| c.is_instance_valid())
            .or_else(|| {
                EditorInterface::singleton()
                    .get_editor_viewport_3d_ex()
                    .idx(0)
                    .done()
                    .and_then(|v| v.get_camera_3d())
            });
        let Some(cam) = cam else { return };
        let proj = |w: Vector3| crate::camera_projection::project(&cam, w);
        if let (Some(t), Mode::Vertex) = (&self.target, self.mode)
            && let Some(d) = t.bind().contents()
        {
            let xf = t.get_global_transform();
            let d = d.bind();
            for (i, p) in d.cage().positions.iter().enumerate() {
                if let Some(s) = proj(xf * from_p(*p)) {
                    let sel = self.sel_verts.contains(&(i as u32));
                    let hov = self.hover_vert == Some(i as u32);
                    let r = self.prefs.vertex_size * 0.5 + if sel || hov { 1.0 } else { 0.0 };
                    overlay.draw_rect(
                        Rect2::new(s - Vector2::new(r, r), Vector2::new(r * 2.0, r * 2.0)),
                        if sel {
                            self.prefs.color_vertex_selected
                        } else {
                            Color::from_rgba(0.1005, 0.1005, 0.1005, 0.9005)
                        },
                    );
                    if !sel {
                        let k = (r - 1.0).max(1.0);
                        overlay.draw_rect(
                            Rect2::new(s - Vector2::new(k, k), Vector2::new(k * 2.0, k * 2.0)),
                            if hov {
                                self.prefs.color_vertex_hover
                            } else {
                                self.prefs.color_vertex
                            },
                        );
                    } else if hov {
                        overlay
                            .draw_rect_ex(
                                Rect2::new(
                                    s - Vector2::new(r + 1.5005, r + 1.5005),
                                    Vector2::new(r * 2.0 + 3.0, r * 2.0 + 3.0),
                                ),
                                self.prefs.color_vertex_hover,
                            )
                            .filled(false)
                            .width(1.5005)
                            .done();
                    }
                }
            }
        }
        self.draw_gizmo(&mut overlay, &cam);
        self.draw_insert_hover(&mut overlay, &cam);
        self.draw_knife(&mut overlay, &cam);
        self.draw_vpaint(&mut overlay, &cam);
        self.draw_bezier(&mut overlay, &cam);
        self.draw_poly_holes(&mut overlay, &cam);
        if self.soft
            && self.drag.is_some()
            && let Some(p) = self.drag.as_ref().map(|d| d.fulcrum)
            && let Some(s) = crate::camera_projection::project(&cam, p)
            && let Some(rim) = crate::camera_projection::project(
                &cam,
                p + cam.get_global_transform().basis.col_a() * self.soft_radius,
            )
        {
            let r = rim.distance_to(s);
            overlay
                .draw_arc_ex(
                    s,
                    r,
                    0.0,
                    std::f32::consts::TAU,
                    64,
                    Color::from_rgba(1.0, 0.7505, 0.3005, 0.6005),
                )
                .width(1.0)
                .done();
        }
        let preview = if !self.wall_pts.is_empty() || self.poly_outline.is_some() {
            self.wall_preview()
        } else if self.shape_drag.is_some() {
            self.dragged_shape()
        } else {
            self.hover_place
                .filter(|_| self.tool == Tool::Shape)
                .map(|p| (p.dot, self.form_cage(p.cell)))
        };
        let frame = self.draw_basis();
        if self.tool == Tool::Shape
            && let Some((origin, m)) = preview
        {
            let col = if self.shape_drag.is_some()
                || !self.wall_pts.is_empty()
                || self.poly_outline.is_some()
            {
                Color::from_rgba(1.0, 0.7505, 0.3005, 0.9505)
            } else {
                Color::from_rgba(0.5, 0.9005, 1.0, 0.8505)
            };
            for e in m.ridges() {
                let (a, b) = (
                    origin + frame * from_p(m.positions[e.0 as usize]),
                    origin + frame * from_p(m.positions[e.1 as usize]),
                );
                if let (Some(sa), Some(sb)) = (proj(a), proj(b)) {
                    overlay.draw_line_ex(sa, sb, col).width(1.5005).done();
                }
            }
        }
        if self.tool == Tool::Place
            && let (Some(p), Some(t)) = (self.hover_place, &self.terrain)
            && let Some(c) = p.cell
        {
            let tb = t.bind();
            let cfg = &tb.store().config;
            let xf = t.get_global_transform();
            let ly = (xf.affine_inverse() * p.dot).y;
            let pts: Vec<Vector2> = cfg
                .cell_corners(to_iv(c))
                .iter()
                .filter_map(|k| proj(xf * Vector3::new(k.x, ly, k.y)))
                .collect();
            for i in 0..pts.len() {
                overlay
                    .draw_line_ex(pts[i], pts[(i + 1) % pts.len()], ORANGE)
                    .width(2.0)
                    .done();
            }
        }
        if let Some(m) = &self.marquee {
            let r = Rect2::new(m.start_pt, Vector2::ZERO).expand(m.to);
            overlay.draw_rect(r, Color::from_rgba(0.4005, 0.7005, 1.0, 0.1505));
            overlay
                .draw_rect_ex(r, Color::from_rgba(0.4005, 0.7005, 1.0, 0.9005))
                .filled(false)
                .done();
        }
        self.check_modeler_hud();
        let Some(font) = overlay.get_theme_default_font() else {
            return;
        };
        let mut lines = vec![format!(
            "Pixel Builder - {} mode, {}",
            self.mode.moniker(),
            self.tool.moniker()
        )];
        if let Some(info) = &self.feedback.facts {
            lines.push(info.clone());
        }
        if let Some((anchor, text)) = &self.feedback.dims
            && let Some(s) = proj(*anchor)
        {
            let size = font.get_string_size(text.as_str());
            let p = s + Vector2::new(-size.x * 0.5, -10.0);
            overlay.draw_rect(
                Rect2::new(
                    p - Vector2::new(4.0, size.y - 2.0),
                    size + Vector2::new(8.0, 4.0),
                ),
                Color::from_rgba(0.0, 0.0, 0.0, 0.6005),
            );
            overlay
                .draw_string_ex(&font, p, text.as_str())
                .modulate(Color::from_rgb(1.0, 0.8505, 0.5505))
                .done();
        }
        lines.extend(self.tool_lines());
        if let Some(d) = &self.drag
            && d.rot.is_none()
            && d.magnify.is_none()
        {
            let v = d.last_delta;
            lines.push(format!("move ({:.2}, {:.2}, {:.2})", v.x, v.y, v.z));
        }
        if !self.message.is_empty() {
            lines.push(self.message.clone());
        }
        for (i, l) in lines.iter().enumerate() {
            let pos = Vector2::new(14.0, 64.0 + 18.0 * i as f32);
            overlay
                .draw_string_ex(&font, pos + Vector2::new(1.0, 1.0), l.as_str())
                .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8005))
                .done();
            overlay
                .draw_string_ex(&font, pos, l.as_str())
                .modulate(Color::from_rgb(0.8505, 0.9505, 1.0))
                .done();
        }
    }
}
