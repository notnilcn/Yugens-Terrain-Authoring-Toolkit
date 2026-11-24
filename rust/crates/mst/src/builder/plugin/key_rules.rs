//! Which builder command a shortcut asks for, and whether the current mode and selection
//! allow it, kept free of engine types so the routing can be tested without a running editor.
use super::super::ui::{Action, Mode, Tool};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum KeyCmd {
    Run(Action),
    CycleMode,
    Grid(f32),
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct KeyContext {
    pub(super) has_target: bool,
    pub(super) mode: Mode,
    pub(super) tool: Tool,
    pub(super) edges_selected: bool,
    pub(super) faces_selected: bool,
}
pub(super) fn key_command(id: &str, ctx: KeyContext) -> Option<KeyCmd> {
    let has = ctx.has_target;
    let editing = ctx.mode != Mode::Object;
    let et = editing && has;
    let run = |ok: bool, a: Action| ok.then_some(KeyCmd::Run(a));
    match id {
        "mode_object" => run(has, Action::ModeObject),
        "mode_face" => run(has, Action::ModeFace),
        "mode_edge" => run(has, Action::ModeEdge),
        "mode_vertex" => run(has, Action::ModeVertex),
        "cycle_mode" => has.then_some(KeyCmd::CycleMode),
        "tool_select" => run(editing || ctx.tool != Tool::Select, Action::ToolSelect),
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
            has && matches!(ctx.mode, Mode::Edge | Mode::Vertex),
            Action::Bevel,
        ),
        "insert_loop" => run(
            has && ctx.mode == Mode::Edge && ctx.edges_selected,
            Action::InsertLoop,
        ),
        "merge" => run(et, Action::Merge),
        "connect" => run(
            et,
            if ctx.mode == Mode::Edge {
                Action::ConnectEdges
            } else {
                Action::Connect
            },
        ),
        "fill" => run(et, Action::Fill),
        "triangulate" => run(
            has && matches!(ctx.mode, Mode::Object | Mode::Face),
            Action::Triangulate,
        ),
        "rotate" => run(et, Action::Rotate),
        "rotate_back" => run(et, Action::RotateNeg),
        "duplicate" => run(
            has && ctx.mode == Mode::Face && ctx.faces_selected,
            Action::Duplicate,
        ),
        "delete" => run(et, Action::Delete),
        "select_all" => run(et, Action::SelectAll),
        "grow" => run(et, Action::Grow),
        "shrink" => run(et, Action::Shrink),
        "select_loop" => run(et, Action::SelectLoop),
        "select_ring" => run(et, Action::SelectRing),
        "insert_vertex" => run(et && ctx.mode == Mode::Edge, Action::InsertVertex),
        "uv_editor" => run(has, Action::UvEditor),
        "grid_halve" => Some(KeyCmd::Grid(0.5)),
        "grid_double" => Some(KeyCmd::Grid(2.0)),
        "roblox_tool_select" => run(has, Action::RobloxSelect),
        "roblox_tool_move" => run(has, Action::GizmoMove),
        "roblox_tool_scale" => run(has, Action::GizmoScale),
        "roblox_tool_rotate" => run(has, Action::GizmoRotate),
        "roblox_tool_transform" => run(has, Action::GizmoTransform),
        "roblox_mode_object" => run(has, Action::ModeObject),
        "roblox_mode_face" => run(has, Action::ModeFace),
        "roblox_mode_edge" => run(has, Action::ModeEdge),
        "roblox_mode_vertex" => run(has, Action::ModeVertex),
        "roblox_vertex_paint" => run(has, Action::ToolVertexPaint),
        "roblox_fill" => run(et, Action::Fill),
        "roblox_handle_space" => run(has, Action::ToggleHandleSpace),
        "roblox_group" => Some(KeyCmd::Run(Action::GroupSelection)),
        "roblox_group_folder" => Some(KeyCmd::Run(Action::GroupAsFolder)),
        "roblox_ungroup" => Some(KeyCmd::Run(Action::Ungroup)),
        "roblox_union" => run(has, Action::RobloxUnion),
        "roblox_intersect" => run(has, Action::RobloxIntersect),
        "roblox_separate" => run(has, Action::Separate),
        "roblox_negate" => run(has, Action::Negate),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EscContext {
    pub(super) shaping: bool,
    pub(super) painting_stroke: bool,
    pub(super) dragging: bool,
    pub(super) tool: Tool,
    pub(super) mode: Mode,
    pub(super) anything_picked: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EscStep {
    CancelShape,
    CancelStroke,
    CancelDrag,
    BackToSelectTool,
    ClearPicked,
    BackToObjectMode,
}
pub(super) fn escape_step(ctx: EscContext) -> Option<EscStep> {
    let editing = ctx.mode != Mode::Object;
    if ctx.shaping {
        Some(EscStep::CancelShape)
    } else if ctx.painting_stroke {
        Some(EscStep::CancelStroke)
    } else if ctx.dragging {
        Some(EscStep::CancelDrag)
    } else if ctx.tool != Tool::Select {
        Some(EscStep::BackToSelectTool)
    } else if editing && ctx.anything_picked {
        Some(EscStep::ClearPicked)
    } else if editing {
        Some(EscStep::BackToObjectMode)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::super::super::shortcuts::ENTRIES;
    use super::*;
    fn editing() -> KeyContext {
        KeyContext {
            has_target: true,
            mode: Mode::Face,
            tool: Tool::Select,
            edges_selected: true,
            faces_selected: true,
        }
    }
    #[test]
    fn every_shortcut_id_routes_to_a_command_when_everything_is_selected() {
        for e in ENTRIES {
            let ok = [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex]
                .into_iter()
                .any(|mode| key_command(e.id, KeyContext { mode, ..editing() }).is_some());
            assert!(ok, "{} never routes", e.id);
        }
    }
    #[test]
    fn roblox_tool_and_mode_keys_need_only_a_target() {
        let object = KeyContext {
            has_target: true,
            ..Default::default()
        };
        assert_eq!(key_command("roblox_tool_move", object), Some(KeyCmd::Run(Action::GizmoMove)));
        assert_eq!(key_command("roblox_mode_face", object), Some(KeyCmd::Run(Action::ModeFace)));
        assert_eq!(key_command("roblox_tool_move", KeyContext::default()), None);
        assert_eq!(key_command("roblox_fill", object), None);
        assert_eq!(key_command("roblox_fill", editing()), Some(KeyCmd::Run(Action::Fill)));
    }
    #[test]
    fn group_keys_route_even_without_a_target() {
        let none = KeyContext::default();
        assert_eq!(key_command("roblox_group", none), Some(KeyCmd::Run(Action::GroupSelection)));
        assert_eq!(key_command("roblox_ungroup", none), Some(KeyCmd::Run(Action::Ungroup)));
    }
    #[test]
    fn regular_gizmo_keys_need_an_element_mode() {
        let object = KeyContext {
            has_target: true,
            ..Default::default()
        };
        assert_eq!(key_command("gizmo_move", object), None);
        assert_eq!(key_command("gizmo_move", editing()), Some(KeyCmd::Run(Action::GizmoMove)));
    }
    #[test]
    fn connect_follows_the_mode() {
        let edge = KeyContext { mode: Mode::Edge, ..editing() };
        assert_eq!(key_command("connect", edge), Some(KeyCmd::Run(Action::ConnectEdges)));
        assert_eq!(key_command("connect", editing()), Some(KeyCmd::Run(Action::Connect)));
    }
    #[test]
    fn select_tool_key_only_when_it_changes_something() {
        let idle = KeyContext::default();
        assert_eq!(key_command("tool_select", idle), None);
        let shaping = KeyContext { tool: Tool::Shape, ..idle };
        assert_eq!(key_command("tool_select", shaping), Some(KeyCmd::Run(Action::ToolSelect)));
    }
    #[test]
    fn escape_cancels_a_part_drag_before_it_touches_the_selection() {
        let part_drag = EscContext {
            dragging: true,
            tool: Tool::Select,
            mode: Mode::Object,
            ..EscContext::default()
        };
        assert_eq!(escape_step(part_drag), Some(EscStep::CancelDrag));
        let pivot_drag = EscContext { tool: Tool::EditPivot, ..part_drag };
        assert_eq!(escape_step(pivot_drag), Some(EscStep::CancelDrag));
        let idle = EscContext { dragging: false, ..part_drag };
        assert_eq!(escape_step(idle), None, "nothing to cancel: the editor's own Esc runs");
        let picked = EscContext { mode: Mode::Edge, anything_picked: true, ..idle };
        assert_eq!(escape_step(picked), Some(EscStep::ClearPicked));
        assert_eq!(
            escape_step(EscContext { anything_picked: false, ..picked }),
            Some(EscStep::BackToObjectMode)
        );
    }
}
