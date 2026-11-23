use super::*;
impl PixelBuilderPlugin {
    pub(super) fn every_or_picked_facets(&self, m: &BMesh) -> BTreeSet<usize> {
        match self.mode {
            Mode::Object => (0..m.facets.len()).collect(),
            Mode::Face => self.sel_faces.clone(),
            _ => m.faces_within(&self.selected_verts(m)),
        }
    }
    pub(super) fn run_action(&mut self, a: Action) {
        let rerun = self.adjust_running;
        if !rerun && !self.no_confirm && self.ask_confirmation(a) {
            return;
        }
        if !rerun {
            self.adjust_commit();
        }
        let session = (!rerun
            && !self.no_adjust
            && self.prefs.adjust_panel
            && self.target.is_some()
            && self.tool == Tool::Select)
            .then(|| adjust::fields(a, self.mode, self.xg.to_new_object))
            .flatten();
        let started = session.is_some();
        if let Some(fields) = session {
            self.adjust_begin(a, fields);
        }
        self.run_action_inner(a);
        if started {
            self.adjust_started();
        }
        if !rerun {
            self.sync_ui();
            self.redraw();
        }
    }
    pub(super) fn run_action_inner(&mut self, a: Action) {
        match a {
            Action::ModeObject => self.assign_setting(Mode::Object),
            Action::ModeFace => self.assign_setting(Mode::Face),
            Action::ModeEdge => self.assign_setting(Mode::Edge),
            Action::ModeVertex => self.assign_setting(Mode::Vertex),
            Action::ToolSelect => self.assign_instrument(Tool::Select),
            Action::ToolShape => self.assign_instrument(Tool::Shape),
            Action::ToolPlace => self.assign_instrument(Tool::Place),
            Action::ToolPaint => self.assign_instrument(Tool::Paint),
            Action::ToolVertexPaint => self.assign_instrument(if self.tool == Tool::VertexPaint {
                Tool::Select
            } else {
                Tool::VertexPaint
            }),
            Action::UvLock => self.uv_lock = !self.uv_lock,
            Action::BlendMaterial => self.act_blend_material(),
            Action::InvertAlpha => self.apply_edit("Pixel Builder: Invert alpha", |m, s| {
                let f = s.every_or_picked_facets(m);
                pt_modeler::paint::invert_alpha(m, &f);
            }),
            Action::BakeMesh => self.act_bake(),
            Action::Bevel => self.perform_chamfer_stretches(),
            Action::GizmoMove => self.set_xform(XformMode::Move),
            Action::GizmoRotate => self.set_xform(XformMode::Rotate),
            Action::GizmoScale => self.set_xform(XformMode::Scale),
            Action::SoftSelect => self.soft = !self.soft,
            Action::SnapVertex => self.snap_vertex = !self.snap_vertex,
            Action::InsertLoop => self.act_insert_loop(),
            Action::Bridge => self.act_bridge(),
            Action::SelectLoop => {
                self.perform_grab_ring(Input::singleton().is_key_pressed(Key::ALT))
            }
            Action::Shrink => self.act_shrink(),
            Action::Holes => self.act_holes(),
            Action::Mirror => self.act_mirror_axes(),
            Action::Dissolve => self.act_dissolve(),
            Action::OffsetElements => self.act_offset(),
            Action::InvertSelection => self.perform_negate_picked(),
            Action::SelectByColor => self.perform_grab_via_tint(),
            Action::SelectBySmoothing => self.act_select_by_smoothing(),
            Action::ToolBezier => {
                if self.mode != Mode::Object {
                    self.assign_setting(Mode::Object);
                }
                self.assign_instrument(Tool::Bezier);
            }
            Action::ReusablePart => self.act_reusable_part(),
            Action::IndividualOrigins => self.xg.individual_origins = !self.xg.individual_origins,
            Action::ExportStl => self.unveil_ship_out_xtra(xg_io::Format::Stl),
            Action::ExportPly => self.unveil_ship_out_xtra(xg_io::Format::Ply),
            Action::ExportRes => self.unveil_ship_out_xtra(xg_io::Format::Res),
            Action::ExportOptions => self.unveil_ship_out_choices(),
            Action::ToolKnife => {
                if self.mode == Mode::Object {
                    self.assign_setting(Mode::Face);
                }
                self.assign_instrument(Tool::Knife);
            }
            Action::ShapeParams => self.toggle_reshape(),
            Action::BoolUnion => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::UNION,
                "Boolean union",
            ),
            Action::BoolSubtract => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::SUBTRACTION,
                "Boolean subtract",
            ),
            Action::BoolIntersect => self.act_boolean(
                godot::classes::csg_shape_3d::Operation::INTERSECTION,
                "Boolean intersect",
            ),
            Action::Merge => self.act_merge(),
            Action::UvEditor => self.unveil_tex_coord_studio(),
            Action::Preferences => self.open_preferences(),
            Action::ConformNormals => self.apply_edit("Pixel Builder: Conform normals", |m, s| {
                let n = selection_edits::harmonize_uprights(m);
                s.message = format!(
                    "Conform normals: {n} face{} flipped",
                    if n == 1 { "" } else { "s" }
                );
            }),
            Action::Connect => self.act_connect(),
            Action::Collapse => self.act_collapse(),
            Action::InsertVertex => {
                self.insert_vert = !self.insert_vert;
                self.insert_hover = None;
                if self.insert_vert && self.mode != Mode::Edge {
                    self.assign_setting(Mode::Edge);
                }
            }
            Action::TrimWalls => self.act_trim_walls(),
            Action::SurfaceDraw => self.surface_draw = !self.surface_draw,
            Action::XRay => self.xray = !self.xray,
            Action::SetCollider => self.act_entity(1),
            Action::SetTrigger => self.act_entity(2),
            Action::Fill => self.act_fill(),
            Action::Detach if self.xg.to_new_object => self.act_extract_faces(false),
            Action::Duplicate if self.xg.to_new_object => self.act_extract_faces(true),
            Action::Detach => self.apply_edit("Pixel Builder: Detach", |m, s| {
                pt_modeler::topo::unlink_facets(m, &s.sel_faces)
            }),
            Action::Duplicate => self.apply_edit("Pixel Builder: Duplicate", |m, s| {
                let f = s.sel_faces.clone();
                s.sel_faces = selection_edits::replicate_facets(m, &f, pt_core::math::Vec3::ZERO);
            }),
            Action::Rotate => {
                let neg = Input::singleton().is_key_pressed(Key::SHIFT);
                self.act_rotate(if neg { -90.0 } else { 90.0 });
            }
            Action::Grow => self.act_select(0),
            Action::Coplanar => self.act_select(1),
            Action::Similar => self.act_select(2),
            Action::ExportObj => self.unveil_ship_out_xtra(xg_io::Format::Obj),
            Action::ExportGltf => self.unveil_ship_out(true),
            Action::Convert => self.convert_selected(),
            Action::SnapGrid => self.snap_grid = !self.snap_grid,
            Action::SnapCell => self.snap_cell = !self.snap_cell,
            Action::Extrude => {
                let (dist, method) = (self.span, self.xg.extrude_method);
                if self.mode == Mode::Edge && !self.sel_edges.is_empty() {
                    self.perform_pull_out_ridges();
                } else if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Extrude: select faces (Face mode)".into();
                } else if method == 0 {
                    self.apply_edit("Pixel Builder: Extrude", |m, s| {
                        s.sel_faces = selection_edits::extrude_selection(m, &s.sel_faces, dist)
                    });
                } else {
                    let how = pt_modeler::xg_tools::ExtrudeMethod::from_i64(method);
                    self.apply_edit("Pixel Builder: Extrude", |m, s| {
                        match pt_modeler::xg_tools::pull_out_picked_approach(
                            m,
                            &s.sel_faces,
                            dist,
                            how,
                            false,
                        ) {
                            Ok(caps) => s.sel_faces = caps,
                            Err(e) => s.message = format!("Extrude: {e}"),
                        }
                    });
                }
            }
            Action::Inset => {
                let (amt, how) = (self.inset, self.opts.inset_mode);
                if self.mode != Mode::Face || self.sel_faces.is_empty() {
                    self.message = "Inset: select faces (Face mode)".into();
                } else if how == 0 {
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        s.sel_faces = selection_edits::inset_center(m, &s.sel_faces, amt.min(0.9905))
                    });
                } else if how == 1 && !self.xg.inset_individual {
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        match pt_modeler::xg_tools::inset_center_grouped(
                            m,
                            &s.sel_faces,
                            amt,
                            false,
                        ) {
                            Ok(inner) => s.sel_faces = inner,
                            Err(e) => s.message = format!("Inset: {e}"),
                        }
                    });
                } else {
                    self.apply_edit("Pixel Builder: Inset", |m, s| {
                        s.sel_faces =
                            pt_modeler::mesh_ops::inset_rim(m, &s.sel_faces, amt, how == 2)
                    });
                }
            }
            Action::Subdivide => self.act_subdivide(),
            Action::SubdivideEdges => self.perform_refine_ridges(),
            Action::ConnectEdges => self.perform_join_ridges(),
            Action::FlipEdge => self.perform_turnover_ridge(),
            Action::SplitVerts => self.act_split_verts(),
            Action::Triangulate => self.act_triangulate(),
            Action::MergeObjects => self.perform_fuse_entities(),
            Action::CenterPivot => self.act_center_pivot(),
            Action::PivotToSelection => self.act_pivot_to_selection(),
            Action::FreezeTransform => self.perform_ice_warp(),
            Action::Flip => self.apply_edit("Pixel Builder: Flip normals", |m, s| {
                let f = s.every_or_picked_facets(m);
                selection_edits::turnover_facets(m, &f);
            }),
            Action::Delete => self.delete_selected(),
            Action::Weld => {
                let dist = self.xg.weld_dist.max(1e-6);
                self.apply_edit("Pixel Builder: Weld", |m, s| {
                    if s.mode == Mode::Vertex && s.sel_verts.len() > 1 {
                        let v = s.sel_verts.clone();
                        let n = pt_modeler::loops::braze_picked_corners(m, &v, dist);
                        m.compact();
                        s.purge_picked();
                        s.message = if n == 0 {
                            format!("Weld: no selected vertices closer than {dist}")
                        } else {
                            format!("Weld: {n} vertices merged")
                        };
                    } else {
                        let n = selection_edits::braze_via_span(m, dist, None);
                        s.purge_picked();
                        s.message = format!("Weld: {n} vertices merged");
                    }
                })
            }
            Action::SelectAll => self.grab_every(),
            Action::ClearSelection => self.purge_picked(),
            Action::SelectRing => self.perform_grab_ring(true),
            Action::RotateNeg => self.act_rotate(-90.0),
            Action::GenerateUv2 => self.perform_produce_uv_b(),
            Action::PaintColour => self.act_paint_colour(),
            Action::Material => {
                let (slot, col) = (self.pocket.clamp(0, 255) as u16, ui_color_bytes(self.tint));
                self.apply_edit("Pixel Builder: Assign material", |m, s| {
                    let f = s.every_or_picked_facets(m);
                    selection_edits::assign_stuff(m, &f, slot);
                    selection_edits::assign_tint(m, &f, col);
                });
            }
            Action::RobloxSelect => self.set_xform(XformMode::Pick),
            Action::ToggleHandleSpace => self.toggle_handle_space(),
            Action::RotSnapToggle => self.toggle_roblox_rot_snap(),
            Action::GizmoTransform => self.act_gizmo_transform(),
            Action::FaceResize => self.act_face_resize(),
            Action::ToolEditPivot => self.act_edit_pivot(),
            Action::PivotSnap => self.act_pivot_snap(),
            Action::ToggleLock => self.act_toggle_lock(),
            Action::ClickToLock => self.act_click_to_lock(),
            Action::Collisions => self.act_collisions(),
            Action::Negate => self.act_negate(),
            Action::RobloxUnion => self.act_roblox_union(),
            Action::RobloxIntersect => self.act_roblox_intersect(),
            Action::Separate => self.act_separate(),
            Action::InsertPartAtView => self.act_insert_part_at_view(),
            Action::GroupSelection => self.act_group_selection(),
            Action::GroupAsFolder => self.act_group_as_folder(),
            Action::Ungroup => self.act_ungroup(),
            Action::SelectChildren => self.act_select_children(),
            Action::ToggleAnchor => self.act_toggle_anchor(),
            Action::ClickToAnchor => self.act_click_to_anchor(),
            Action::AlignPanel => self.act_align_panel(),
            Action::MaterialPicker => self.act_material_picker(),
            Action::ColorPicker => self.act_color_picker(),
            Action::ClickToPaint => self.act_click_to_paint(),
            Action::SnapBoundingBoxes => self.act_snap_boxes(),
            Action::GridPlane => self.act_grid_plane(),
            Action::DuplicateObject => self.act_duplicate_object(),
            Action::DeleteObject => self.act_delete_object(),
            Action::ZoomToSelection => self.act_zoom_to_selection(),
        }
    }
    pub(super) fn grab_every(&mut self) {
        self.adjust_commit();
        if let Some(d) = self.contents() {
            let db = d.bind();
            let m = db.cage();
            match self.mode {
                Mode::Face => self.sel_faces = (0..m.facets.len()).collect(),
                Mode::Edge => self.sel_edges = m.ridges().into_iter().collect(),
                Mode::Vertex => self.sel_verts = (0..m.positions.len() as u32).collect(),
                Mode::Object => {}
            }
        }
    }
    pub(super) fn cycle_mode(&mut self) {
        let next = match self.mode {
            Mode::Object => Mode::Vertex,
            Mode::Vertex => Mode::Edge,
            Mode::Edge => Mode::Face,
            Mode::Face => Mode::Object,
        };
        self.assign_setting(next);
    }
    pub(super) fn delete_selected(&mut self) {
        let mode = self.mode;
        self.apply_edit("Pixel Builder: Delete", |m, s| {
            match mode {
                Mode::Face => selection_edits::discard_facets(m, &s.sel_faces),
                Mode::Edge => selection_edits::delete_edges(m, &s.sel_edges),
                Mode::Vertex => selection_edits::delete_verts(m, &s.sel_verts),
                Mode::Object => {}
            }
            s.purge_picked();
        });
    }
}
