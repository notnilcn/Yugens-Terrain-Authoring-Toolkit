use super::*;
#[derive(Clone, Copy, Debug)]
pub(super) enum KeyCmd {
    Run(Action),
    CycleMode,
    Grid(f32),
}
pub(super) fn is_undo_redo_key(k: &Gd<InputEventKey>) -> bool {
    k.is_pressed()
        && !k.is_echo()
        && (k.is_ctrl_pressed() || k.is_meta_pressed())
        && !k.is_alt_pressed()
        && matches!(k.get_keycode(), Key::Z | Key::Y)
}
impl PixelBuilderPlugin {
    pub(super) fn on_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        let k = key.get_keycode();
        let ctrl = key.is_ctrl_pressed() || key.is_meta_pressed();
        if self.adjust.is_some() {
            match k {
                Key::ENTER | Key::KP_ENTER if !ctrl => {
                    self.adjust_commit();
                    self.sync_ui();
                    self.redraw();
                    return true;
                }
                Key::ESCAPE => {
                    self.adjust_cancel();
                    return true;
                }
                Key::Z if ctrl => {
                    self.adjust_commit();
                    return false;
                }
                _ => {}
            }
        }
        for i in 0..super::super::shortcuts::ENTRIES.len() {
            if !super::super::shortcuts::fits(&self.shortcut_table, i, key) {
                continue;
            }
            if let Some(cmd) = self.shortcut_command(super::super::shortcuts::ENTRIES[i].id) {
                self.run_key_command(cmd);
                return true;
            }
        }
        let editing = self.mode != Mode::Object;
        match k {
            Key::ESCAPE if !ctrl && !key.is_shift_pressed() && !key.is_alt_pressed() => {
                if self.shape_drag.is_some() {
                    self.cancel_shape();
                } else if self.cancel_vpaint_stroke() {
                } else if self.drag.is_some() {
                    self.cancel_drag();
                } else if self.tool != Tool::Select {
                    self.assign_instrument(Tool::Select);
                } else if editing
                    && !(self.sel_faces.is_empty()
                        && self.sel_edges.is_empty()
                        && self.sel_verts.is_empty())
                {
                    self.purge_picked();
                } else if editing {
                    self.assign_setting(Mode::Object);
                } else {
                    return false;
                }
                self.redraw();
                true
            }
            _ => false,
        }
    }
    pub(super) fn shortcut_command(&self, id: &str) -> Option<KeyCmd> {
        let has = self.target.is_some();
        let editing = self.mode != Mode::Object;
        let et = editing && has;
        let run = |ok: bool, a: Action| ok.then_some(KeyCmd::Run(a));
        match id {
            "mode_object" => run(has, Action::ModeObject),
            "mode_face" => run(has, Action::ModeFace),
            "mode_edge" => run(has, Action::ModeEdge),
            "mode_vertex" => run(has, Action::ModeVertex),
            "cycle_mode" => has.then_some(KeyCmd::CycleMode),
            "tool_select" => run(editing || self.tool != Tool::Select, Action::ToolSelect),
            "tool_shape" => Some(KeyCmd::Run(Action::ToolShape)),
            "tool_bezier" => Some(KeyCmd::Run(Action::ToolBezier)),
            "tool_paint" => run(has, Action::ToolPaint),
            "tool_vertex_paint" => run(has, Action::ToolVertexPaint),
            "tool_place" => run(has, Action::ToolPlace),
            "tool_knife" => run(et, Action::ToolKnife),
            "gizmo_move" => run(et, Action::GizmoMove),
            "gizmo_rotate" => run(et, Action::GizmoRotate),
            "gizmo_scale" => run(et, Action::GizmoScale),
            "soft_select" => run(et, Action::SoftSelect),
            "extrude" => run(et, Action::Extrude),
            "inset" => run(et, Action::Inset),
            "bevel" => run(
                has && matches!(self.mode, Mode::Edge | Mode::Vertex),
                Action::Bevel,
            ),
            "insert_loop" => run(
                has && self.mode == Mode::Edge && !self.sel_edges.is_empty(),
                Action::InsertLoop,
            ),
            "merge" => run(et, Action::Merge),
            "connect" => run(
                et,
                if self.mode == Mode::Edge {
                    Action::ConnectEdges
                } else {
                    Action::Connect
                },
            ),
            "fill" => run(et, Action::Fill),
            "triangulate" => run(
                has && matches!(self.mode, Mode::Object | Mode::Face),
                Action::Triangulate,
            ),
            "rotate" => run(et, Action::Rotate),
            "rotate_back" => run(et, Action::RotateNeg),
            "duplicate" => run(
                has && self.mode == Mode::Face && !self.sel_faces.is_empty(),
                Action::Duplicate,
            ),
            "delete" => run(et, Action::Delete),
            "select_all" => run(et, Action::SelectAll),
            "grow" => run(et, Action::Grow),
            "shrink" => run(et, Action::Shrink),
            "select_loop" => run(et, Action::SelectLoop),
            "select_ring" => run(et, Action::SelectRing),
            "insert_vertex" => run(et && self.mode == Mode::Edge, Action::InsertVertex),
            "uv_editor" => run(has, Action::UvEditor),
            "grid_halve" => Some(KeyCmd::Grid(0.5)),
            "grid_double" => Some(KeyCmd::Grid(2.0)),
            _ => None,
        }
    }
    pub(super) fn run_key_command(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Run(a) => self.run_action(a),
            KeyCmd::CycleMode => self.cycle_mode(),
            KeyCmd::Grid(f) => {
                self.grid_step = (self.grid_step * f).clamp(0.0624, 64.0);
                self.sync_ui();
            }
        }
    }
}
