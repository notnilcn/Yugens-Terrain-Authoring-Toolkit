//! The decisions of the Roblox Studio-style layout, kept free of engine types so they can
//! be tested without a running editor: which ribbon commands are offered and why not,
//! which modes and steps apply while the layout is shown, and which input it takes over.
use super::super::layout::Layout;
use super::super::ui::{Action, Mode, Tool, XformMode};
pub(super) const ROBLOX_MOVE_STEP: f32 = 1.0;
pub(super) const USABLE_WITHOUT_SELECTION: &[Action] = &[
    Action::RobloxSelect,
    Action::GizmoMove,
    Action::GizmoScale,
    Action::GizmoRotate,
    Action::InsertPartAtView,
    Action::SnapGrid,
    Action::RotSnapToggle,
    Action::SnapVertex,
    Action::SnapCell,
    Action::PivotSnap,
    Action::Collisions,
    Action::GroupSelection,
    Action::GroupAsFolder,
    Action::Ungroup,
    Action::SelectChildren,
    Action::ClickToPaint,
    Action::AlignPanel,
    Action::MaterialPicker,
    Action::SnapBoundingBoxes,
    Action::GridPlane,
];
pub(super) fn ribbon_reason(
    a: Action,
    track_reason: Option<&'static str>,
    general_reason: impl FnOnce() -> Option<&'static str>,
) -> Option<&'static str> {
    if track_reason.is_some() {
        return track_reason;
    }
    if USABLE_WITHOUT_SELECTION.contains(&a) {
        return None;
    }
    general_reason()
}
pub(super) const WHOLE_NODE_COMMANDS: &[Action] =
    &[Action::DuplicateObject, Action::DeleteObject, Action::ZoomToSelection];
pub(super) fn menu_reason(
    a: Action,
    only_a_part_group: bool,
    ribbon_reason: impl FnOnce() -> Option<&'static str>,
) -> Option<&'static str> {
    if only_a_part_group && WHOLE_NODE_COMMANDS.contains(&a) {
        return None;
    }
    ribbon_reason()
}
pub(super) fn first_pressed(by_precedence: [Option<bool>; 4]) -> Option<bool> {
    by_precedence.into_iter().flatten().next()
}
pub(super) fn every_one_on(flags: &[bool]) -> bool {
    !flags.is_empty() && flags.iter().all(|f| *f)
}
pub(super) fn leaves_roblox(was_roblox: bool, next: Layout) -> bool {
    was_roblox && next != Layout::Roblox
}
pub(super) fn context_menu_target(
    builder_mesh: bool,
    roblox: bool,
    convertible: bool,
    part_group: bool,
) -> bool {
    builder_mesh || (roblox && (convertible || part_group))
}
pub(super) fn make_editable_reason(convertible: bool) -> Option<&'static str> {
    (!convertible).then_some("Already editable: select a MeshInstance3D or CSG shape.")
}
pub(super) fn inspector_sections_change(was_roblox: bool, next: Layout) -> bool {
    was_roblox != (next == Layout::Roblox)
}
pub(super) fn rot_snap_deg(roblox: bool, roblox_on: bool, roblox_deg: f32, other_deg: f32) -> f32 {
    if !roblox {
        other_deg
    } else if roblox_on {
        roblox_deg
    } else {
        0.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StepSwap {
    pub(super) stashed_other: f32,
    pub(super) grid: f32,
}
pub(super) fn entering_move_step(
    stashed_other: Option<f32>,
    last_roblox: Option<f32>,
    grid: f32,
) -> Option<StepSwap> {
    stashed_other.is_none().then(|| StepSwap {
        stashed_other: grid,
        grid: last_roblox.unwrap_or(ROBLOX_MOVE_STEP),
    })
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StepRestore {
    pub(super) last_roblox: f32,
    pub(super) grid: f32,
}
pub(super) fn leaving_move_step(stashed_other: Option<f32>, grid: f32) -> Option<StepRestore> {
    stashed_other.map(|other| StepRestore {
        last_roblox: grid,
        grid: other,
    })
}
pub(super) fn after_leaving(xform: XformMode, tool: Tool) -> (XformMode, Tool) {
    let xform = match xform {
        XformMode::Pick | XformMode::Combined => XformMode::Move,
        x => x,
    };
    let tool = if tool == Tool::EditPivot { Tool::Select } else { tool };
    (xform, tool)
}
pub(super) fn next_handle_space(orient: i64) -> (i64, &'static str) {
    if orient == 1 {
        (0, "Handles: world")
    } else {
        (1, "Handles: local")
    }
}
pub(super) fn edit_pivot_toggle(tool: Tool, mode: Mode) -> (Tool, Option<Mode>) {
    if tool == Tool::EditPivot {
        (Tool::Select, None)
    } else {
        (
            Tool::EditPivot,
            (mode != Mode::Object).then_some(Mode::Object),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClickMode {
    Lock,
    Anchor,
    Paint,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ClickModes {
    pub(super) lock: bool,
    pub(super) anchor: bool,
    pub(super) paint: bool,
}
impl ClickModes {
    pub(super) fn toggled(self, which: ClickMode) -> ClickModes {
        let was_on = match which {
            ClickMode::Lock => self.lock,
            ClickMode::Anchor => self.anchor,
            ClickMode::Paint => self.paint,
        };
        if was_on {
            return ClickModes {
                lock: self.lock && which != ClickMode::Lock,
                anchor: self.anchor && which != ClickMode::Anchor,
                paint: self.paint && which != ClickMode::Paint,
            };
        }
        ClickModes {
            lock: which == ClickMode::Lock,
            anchor: which == ClickMode::Anchor,
            paint: which == ClickMode::Paint,
        }
    }
    pub(super) fn on_leaving_roblox() -> ClickModes {
        ClickModes::default()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClickClaim {
    Pass,
    Swallow,
    Act,
}
pub(super) fn click_claim(mode_on: bool, roblox: bool, left_button_pressed: Option<bool>) -> ClickClaim {
    match (mode_on && roblox, left_button_pressed) {
        (true, Some(true)) => ClickClaim::Act,
        (true, Some(false)) => ClickClaim::Swallow,
        _ => ClickClaim::Pass,
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GroupKeyContext {
    pub(super) roblox: bool,
    pub(super) fresh_press: bool,
    pub(super) ribbon_shown: bool,
    pub(super) over_main_screen: bool,
    pub(super) typing_in_text: bool,
    pub(super) has_selection: bool,
}
pub(super) fn group_key_action(ctx: GroupKeyContext, matched: Option<Action>) -> Option<Action> {
    let taken = ctx.roblox
        && ctx.fresh_press
        && ctx.ribbon_shown
        && ctx.over_main_screen
        && !ctx.typing_in_text
        && ctx.has_selection;
    matched.filter(|_| taken)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialScope {
    WholeMesh,
    SelectedFaces,
    NewParts,
}
pub(super) fn material_scope(
    has_mesh: bool,
    mode: Mode,
    faces_selected: bool,
) -> Result<MaterialScope, &'static str> {
    if !has_mesh {
        return Ok(MaterialScope::NewParts);
    }
    match mode {
        Mode::Object => Ok(MaterialScope::WholeMesh),
        Mode::Face if faces_selected => Ok(MaterialScope::SelectedFaces),
        _ => Err("Material: select faces (Face mode) or use Object mode"),
    }
}
pub(super) fn grid_plane_switch(
    has_target: bool,
    tool: Tool,
    xform: XformMode,
    mode: Mode,
    plane_on: bool,
) -> Option<bool> {
    (has_target && tool == Tool::Select && xform == XformMode::Combined && mode == Mode::Object)
        .then_some(plane_on)
}
pub(super) fn right_click_still_a_click(
    press: (f32, f32),
    now: (f32, f32),
    travel: f32,
    pointer_captured: bool,
    slop: f32,
) -> bool {
    let (dx, dy) = (now.0 - press.0, now.1 - press.1);
    travel <= slop && (pointer_captured || (dx * dx + dy * dy).sqrt() <= slop)
}
#[derive(Clone, Debug, PartialEq)]
pub(super) enum AnchorPlan {
    Release,
    Anchor { grounded: Vec<usize>, skipped: usize },
    NoGround,
}
pub(super) fn anchor_plan(all_anchored: bool, has_ground: &[bool]) -> AnchorPlan {
    if all_anchored {
        return AnchorPlan::Release;
    }
    let grounded: Vec<usize> = (0..has_ground.len()).filter(|i| has_ground[*i]).collect();
    if grounded.is_empty() {
        return AnchorPlan::NoGround;
    }
    let skipped = has_ground.len() - grounded.len();
    AnchorPlan::Anchor { grounded, skipped }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_lone_model_or_folder_offers_grouping_and_whole_node_commands() {
        assert_eq!(menu_reason(Action::DeleteObject, true, || Some("mesh")), None);
        assert_eq!(menu_reason(Action::Ungroup, true, || None), None);
        assert_eq!(menu_reason(Action::RobloxUnion, true, || Some("mesh")), Some("mesh"));
        assert_eq!(menu_reason(Action::DeleteObject, false, || Some("mesh")), Some("mesh"));
    }
    #[test]
    fn a_model_or_folder_of_parts_gets_the_menu_only_in_roblox() {
        assert!(context_menu_target(false, true, false, true));
        assert!(!context_menu_target(false, false, false, true));
    }
    #[test]
    fn anchoring_skips_parts_with_no_terrain_under_them() {
        assert_eq!(anchor_plan(false, &[false]), AnchorPlan::NoGround);
        assert_eq!(anchor_plan(false, &[]), AnchorPlan::NoGround);
        assert_eq!(
            anchor_plan(false, &[true, false, true]),
            AnchorPlan::Anchor { grounded: vec![0, 2], skipped: 1 }
        );
        assert_eq!(anchor_plan(true, &[false]), AnchorPlan::Release, "releasing needs no ground");
    }
    #[test]
    fn a_track_reason_wins_over_everything() {
        assert_eq!(
            ribbon_reason(Action::GizmoMove, Some("track"), || Some("general")),
            Some("track")
        );
    }
    #[test]
    fn tools_need_no_selection_but_other_commands_ask_the_general_rule() {
        assert_eq!(ribbon_reason(Action::GizmoMove, None, || Some("general")), None);
        assert_eq!(
            ribbon_reason(Action::ToggleLock, None, || Some("general")),
            Some("general")
        );
        assert_eq!(ribbon_reason(Action::ToggleLock, None, || None), None);
    }
    #[test]
    fn pressed_comes_from_the_first_rule_that_knows() {
        assert_eq!(first_pressed([None, Some(false), Some(true), None]), Some(false));
        assert_eq!(first_pressed([None, None, None, Some(true)]), Some(true));
        assert_eq!(first_pressed([None; 4]), None);
    }
    #[test]
    fn every_one_on_needs_at_least_one() {
        assert!(!every_one_on(&[]));
        assert!(every_one_on(&[true, true]));
        assert!(!every_one_on(&[true, false]));
    }
    #[test]
    fn a_plain_mesh_gets_the_menu_only_in_roblox_and_only_make_editable_needs_it() {
        assert!(context_menu_target(true, false, false, false));
        assert!(context_menu_target(true, true, false, false));
        assert!(context_menu_target(false, true, true, false));
        assert!(!context_menu_target(false, false, true, false));
        assert!(!context_menu_target(false, true, false, false));
        assert_eq!(make_editable_reason(true), None);
        assert!(make_editable_reason(false).is_some());
    }
    #[test]
    fn the_inspector_rebuilds_only_when_the_part_section_comes_or_goes() {
        assert!(inspector_sections_change(false, Layout::Roblox));
        assert!(inspector_sections_change(true, Layout::Mst));
        assert!(inspector_sections_change(true, Layout::Modeler));
        assert!(!inspector_sections_change(true, Layout::Roblox));
        assert!(!inspector_sections_change(false, Layout::Modeler));
        assert!(!inspector_sections_change(false, Layout::Mst));
    }
    #[test]
    fn only_a_switch_away_from_roblox_leaves_it() {
        assert!(leaves_roblox(true, Layout::Mst));
        assert!(leaves_roblox(true, Layout::Modeler));
        assert!(!leaves_roblox(true, Layout::Roblox));
        assert!(!leaves_roblox(false, Layout::Mst));
    }
    #[test]
    fn rotate_step_follows_the_layout() {
        assert_eq!(rot_snap_deg(false, true, 45.0, 15.0), 15.0);
        assert_eq!(rot_snap_deg(true, true, 45.0, 15.0), 45.0);
        assert_eq!(rot_snap_deg(true, false, 45.0, 15.0), 0.0);
    }
    #[test]
    fn entering_roblox_stashes_the_grid_once_and_uses_one_metre_first() {
        assert_eq!(
            entering_move_step(None, None, 0.25),
            Some(StepSwap { stashed_other: 0.25, grid: 1.0 })
        );
        assert_eq!(
            entering_move_step(None, Some(0.5), 0.25),
            Some(StepSwap { stashed_other: 0.25, grid: 0.5 })
        );
        assert_eq!(entering_move_step(Some(0.25), None, 1.0), None);
    }
    #[test]
    fn leaving_roblox_restores_the_stash_and_remembers_the_roblox_step() {
        assert_eq!(
            leaving_move_step(Some(0.25), 2.0),
            Some(StepRestore { last_roblox: 2.0, grid: 0.25 })
        );
        assert_eq!(leaving_move_step(None, 2.0), None);
    }
    #[test]
    fn leaving_resets_roblox_only_modes() {
        assert_eq!(after_leaving(XformMode::Pick, Tool::EditPivot), (XformMode::Move, Tool::Select));
        assert_eq!(after_leaving(XformMode::Combined, Tool::Shape), (XformMode::Move, Tool::Shape));
        assert_eq!(after_leaving(XformMode::Rotate, Tool::Select), (XformMode::Rotate, Tool::Select));
    }
    #[test]
    fn handle_space_flips_between_world_and_local() {
        assert_eq!(next_handle_space(0), (1, "Handles: local"));
        assert_eq!(next_handle_space(1), (0, "Handles: world"));
    }
    #[test]
    fn edit_pivot_turns_on_in_object_mode_and_off_to_select() {
        assert_eq!(edit_pivot_toggle(Tool::Select, Mode::Face), (Tool::EditPivot, Some(Mode::Object)));
        assert_eq!(edit_pivot_toggle(Tool::Select, Mode::Object), (Tool::EditPivot, None));
        assert_eq!(edit_pivot_toggle(Tool::EditPivot, Mode::Object), (Tool::Select, None));
    }
    #[test]
    fn turning_one_click_mode_on_turns_the_others_off() {
        let none = ClickModes::default();
        let lock = ClickModes { lock: true, ..none };
        let anchor = ClickModes { anchor: true, ..none };
        let paint = ClickModes { paint: true, ..none };
        assert_eq!(none.toggled(ClickMode::Lock), lock);
        assert_eq!(lock.toggled(ClickMode::Anchor), anchor);
        assert_eq!(anchor.toggled(ClickMode::Paint), paint);
        assert_eq!(paint.toggled(ClickMode::Lock), lock);
        assert_eq!(lock.toggled(ClickMode::Paint), paint);
        assert_eq!(paint.toggled(ClickMode::Anchor), anchor);
    }
    #[test]
    fn turning_a_click_mode_off_leaves_the_rest_off() {
        let none = ClickModes::default();
        for which in [ClickMode::Lock, ClickMode::Anchor, ClickMode::Paint] {
            assert_eq!(none.toggled(which).toggled(which), none);
        }
        assert_eq!(ClickModes::on_leaving_roblox(), none);
    }
    #[test]
    fn the_grid_plane_switch_shows_only_with_the_transform_handles() {
        let s = |t, x, m| grid_plane_switch(true, t, x, m, false);
        assert_eq!(s(Tool::Select, XformMode::Combined, Mode::Object), Some(false));
        assert_eq!(s(Tool::Select, XformMode::Move, Mode::Object), None);
        assert_eq!(s(Tool::EditPivot, XformMode::Combined, Mode::Object), None);
        assert_eq!(s(Tool::Select, XformMode::Combined, Mode::Face), None);
        assert_eq!(grid_plane_switch(false, Tool::Select, XformMode::Combined, Mode::Object, true), None);
    }
    #[test]
    fn a_right_click_survives_the_pointer_capture_jump_but_not_a_drag() {
        assert!(right_click_still_a_click((10.0, 10.0), (12.0, 10.0), 2.0, false, 4.0));
        assert!(!right_click_still_a_click((10.0, 10.0), (20.0, 10.0), 0.0, false, 4.0));
        assert!(right_click_still_a_click((10.0, 10.0), (640.0, 360.0), 0.0, true, 4.0));
        assert!(!right_click_still_a_click((10.0, 10.0), (640.0, 360.0), 9.0, true, 4.0));
    }
    #[test]
    fn material_and_part_insert_need_no_selection() {
        assert_eq!(ribbon_reason(Action::MaterialPicker, None, || Some("general")), None);
        assert_eq!(ribbon_reason(Action::SnapBoundingBoxes, None, || Some("general")), None);
        assert_eq!(
            ribbon_reason(Action::ColorPicker, None, || Some("general")),
            Some("general")
        );
    }
    #[test]
    fn click_modes_take_left_clicks_only_in_roblox() {
        assert_eq!(click_claim(true, true, Some(true)), ClickClaim::Act);
        assert_eq!(click_claim(true, true, Some(false)), ClickClaim::Swallow);
        assert_eq!(click_claim(true, true, None), ClickClaim::Pass);
        assert_eq!(click_claim(false, true, Some(true)), ClickClaim::Pass);
        assert_eq!(click_claim(true, false, Some(true)), ClickClaim::Pass);
    }
    #[test]
    fn group_keys_win_only_when_every_condition_holds() {
        let ok = GroupKeyContext {
            roblox: true,
            fresh_press: true,
            ribbon_shown: true,
            over_main_screen: true,
            typing_in_text: false,
            has_selection: true,
        };
        let g = Some(Action::GroupSelection);
        assert_eq!(group_key_action(ok, g), g);
        assert_eq!(group_key_action(ok, None), None);
        for broken in [
            GroupKeyContext { roblox: false, ..ok },
            GroupKeyContext { fresh_press: false, ..ok },
            GroupKeyContext { ribbon_shown: false, ..ok },
            GroupKeyContext { over_main_screen: false, ..ok },
            GroupKeyContext { typing_in_text: true, ..ok },
            GroupKeyContext { has_selection: false, ..ok },
        ] {
            assert_eq!(group_key_action(broken, g), None);
        }
    }
    #[test]
    fn material_goes_to_the_whole_mesh_the_selected_faces_or_new_parts() {
        assert_eq!(material_scope(true, Mode::Object, false), Ok(MaterialScope::WholeMesh));
        assert_eq!(material_scope(true, Mode::Face, true), Ok(MaterialScope::SelectedFaces));
        assert!(material_scope(true, Mode::Face, false).is_err());
        assert!(material_scope(true, Mode::Edge, true).is_err());
        assert_eq!(material_scope(false, Mode::Face, false), Ok(MaterialScope::NewParts));
    }
}
