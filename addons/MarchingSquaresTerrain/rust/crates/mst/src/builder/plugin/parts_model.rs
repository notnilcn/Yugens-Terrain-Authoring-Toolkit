use super::grouping::{GroupKind, top_selection};
use super::placement::builder_mesh_bounds;
use super::super::shortcuts;
use super::roblox_rules::{self as core, ClickClaim, ClickMode, GroupKeyContext};
use super::*;
use pt_modeler::part_insert;
const NEEDS_TERRAIN: &str = "Anchoring needs a terrain in the scene";
const GROUP_KEYS: [(&str, Action); 3] = [
    ("roblox_group", Action::GroupSelection),
    ("roblox_group_folder", Action::GroupAsFolder),
    ("roblox_ungroup", Action::Ungroup),
];
#[derive(Default)]
pub(super) struct PartsState {
    pub(super) click_anchor: bool,
}
fn anchor_targets(fallback: Option<&Gd<PixelBuilderMesh>>) -> Vec<Gd<PixelBuilderMesh>> {
    let mut out: Vec<Gd<PixelBuilderMesh>> = EditorInterface::singleton()
        .get_selection()
        .map(|s| {
            s.get_selected_nodes()
                .iter_shared()
                .filter_map(|n| n.try_cast::<PixelBuilderMesh>().ok())
                .collect()
        })
        .unwrap_or_default();
    if out.is_empty()
        && let Some(t) = fallback
    {
        out.push(t.clone());
    }
    out
}
fn anchored_flags(parts: &[Gd<PixelBuilderMesh>]) -> Vec<bool> {
    parts.iter().map(|m| m.get("anchored").to::<bool>()).collect()
}
pub(super) fn left_button(event: &Gd<InputEvent>) -> Option<Gd<InputEventMouseButton>> {
    event
        .clone()
        .try_cast::<InputEventMouseButton>()
        .ok()
        .filter(|mb| mb.get_button_index() == MouseButton::LEFT)
}
fn typing_in_text() -> bool {
    EditorInterface::singleton()
        .get_base_control()
        .and_then(|c| c.get_viewport())
        .and_then(|v| v.gui_get_focus_owner())
        .is_some_and(|f| f.is_class("LineEdit") || f.is_class("TextEdit"))
}
impl PixelBuilderPlugin {
    fn live_terrain(&mut self) -> Option<Gd<MarchingSquaresTerrain>> {
        if self.terrain.as_ref().is_none_or(|t| !t.is_instance_valid()) {
            self.terrain = self.chosen_terrain();
        }
        self.terrain.clone()
    }
    pub(super) fn parts_requirement(&self, a: Action) -> Option<&'static str> {
        let has_terrain = self.terrain.as_ref().is_some_and(|t| t.is_instance_valid());
        match a {
            Action::ToggleAnchor | Action::ClickToAnchor if !has_terrain => Some(NEEDS_TERRAIN),
            _ => None,
        }
    }
    pub(super) fn parts_pressed(&self, a: Action) -> Option<bool> {
        match a {
            Action::ClickToAnchor => Some(self.parts.click_anchor),
            Action::ToggleAnchor => {
                let flags = anchored_flags(&anchor_targets(self.target.as_ref()));
                Some(core::every_one_on(&flags))
            }
            _ => None,
        }
    }
    pub(super) fn act_insert_part_at_view(&mut self) {
        self.insert_part(ShapeKind::Box);
    }
    pub(super) fn insert_part(&mut self, kind: ShapeKind) {
        let Some(cam) = self.view_camera() else {
            self.message = "Insert part: no 3D viewport camera".into();
            return;
        };
        let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
            self.message = "Insert part: open a scene first".into();
            return;
        };
        let terrain = self.live_terrain();
        let size = cam
            .get_viewport()
            .map(|v| v.get_visible_rect().size)
            .unwrap_or(Vector2::new(800.0, 600.0));
        let centre = size * 0.5;
        let o = cam.project_ray_origin(centre);
        let d = cam.project_ray_normal(centre);
        let hit = self.view_ray_hit(o, d);
        let spec = ShapeSpec {
            sort: kind,
            size: part_insert::default_part_size(kind),
            ..self.spec
        };
        let grid = terrain
            .as_ref()
            .map(|t| t.bind().store().config.clone())
            .filter(|c| c.grid.is_cell_mode());
        let m = shapes::produce(&spec, grid.as_ref());
        let Some(extent) = m.extent() else {
            self.message = "The settings make an empty mesh: nothing created".into();
            return;
        };
        let point = part_insert::place_part(
            to_p(o),
            to_p(d),
            hit.map(to_p),
            extent,
            &builder_mesh_bounds(),
            self.snap_grid.then_some(self.grid_step),
        );
        let p = Placement {
            dot: from_p(point),
            cell: None,
            frame: Basis::IDENTITY,
        };
        if let Some(node) =
            self.create_shape_under(root, p, m, xg_io::Recipe::Shape(spec))
        {
            self.give_new_part_material(&node);
            if let Some(mut sel) = EditorInterface::singleton().get_selection() {
                sel.clear();
                sel.add_node(&node);
            }
            self.message = format!("Inserted {} at the view centre", kind.moniker());
        }
    }
    pub(super) fn act_group_selection(&mut self) {
        self.group_selection_as(GroupKind::Model);
    }
    pub(super) fn act_group_as_folder(&mut self) {
        self.group_selection_as(GroupKind::Folder);
    }
    pub(super) fn act_ungroup(&mut self) {
        self.ungroup_selection();
    }
    pub(super) fn act_select_children(&mut self) {
        self.select_children_of_selection();
    }
    pub(super) fn act_toggle_anchor(&mut self) {
        let parts = anchor_targets(self.target.as_ref());
        self.write_anchors(&parts);
    }
    fn write_anchors(&mut self, parts: &[Gd<PixelBuilderMesh>]) {
        if parts.is_empty() {
            self.message = "Anchor: select a builder mesh".into();
            return;
        }
        let Some(terrain) = self.live_terrain() else {
            self.message = NEEDS_TERRAIN.into();
            return;
        };
        let cells: Vec<Vector2i> = parts
            .iter()
            .map(|t| super::super::node::world_cell(&terrain, t.get_global_position()))
            .collect();
        let ground: Vec<bool> = cells
            .iter()
            .map(|c| super::super::node::cell_world(&terrain, *c).is_some())
            .collect();
        let plan = core::anchor_plan(core::every_one_on(&anchored_flags(parts)), &ground);
        let (release, chosen, skipped) = match plan {
            core::AnchorPlan::NoGround => {
                self.message = format!(
                    "Anchor: no terrain ground under {}; move {} over the terrain first",
                    if parts.len() == 1 { "the part" } else { "any selected part" },
                    if parts.len() == 1 { "it" } else { "them" },
                );
                return;
            }
            core::AnchorPlan::Release => (true, (0..parts.len()).collect::<Vec<_>>(), 0),
            core::AnchorPlan::Anchor { grounded, skipped } => (false, grounded, skipped),
        };
        let name = if release {
            "Pixel Builder: Release anchor"
        } else {
            "Pixel Builder: Anchor to terrain"
        };
        let Some(mut ur) = self.open_action(name) else {
            return;
        };
        for i in &chosen {
            let t = &parts[*i];
            if release {
                ur.add_do_property(t, "anchored", &false.to_variant());
            } else {
                ur.add_do_method(
                    t,
                    "place_on_cell",
                    &[terrain.to_variant(), cells[*i].to_variant()],
                );
            }
            ur.add_undo_property(t, "position", &t.get_position().to_variant());
            ur.add_undo_property(t, "anchor_cell", &t.get("anchor_cell"));
            ur.add_undo_property(t, "anchored", &t.get("anchored"));
        }
        ur.commit_action();
        let n = chosen.len();
        self.message = format!(
            "{} {} part{}{}",
            if release { "Released" } else { "Anchored" },
            n,
            if n == 1 { "" } else { "s" },
            if skipped > 0 {
                format!(" ({skipped} with no terrain ground under it left as is)")
            } else {
                String::new()
            }
        );
        self.redraw();
    }
    pub(super) fn act_click_to_anchor(&mut self) {
        self.toggle_click_mode(ClickMode::Anchor);
        self.message = if self.parts.click_anchor {
            "Click to anchor: click a part to anchor or release it (click the menu item again to stop)"
                .into()
        } else {
            String::new()
        };
    }
    pub(super) fn parts_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        let mb = left_button(event);
        let claim = core::click_claim(
            self.parts.click_anchor,
            self.is_roblox(),
            mb.as_ref().map(|m| m.is_pressed()),
        );
        if claim == ClickClaim::Pass {
            return None;
        }
        if let Some(mb) = mb.filter(|_| claim == ClickClaim::Act) {
            let pos = mb.get_position();
            let o = cam.project_ray_origin(pos);
            let d = cam.project_ray_normal(pos);
            match self.builder_mesh_under(o, d) {
                Some(m) => self.write_anchors(&[m]),
                None => self.message = "Click to anchor: no builder mesh under the cursor".into(),
            }
            self.sync_ui();
        }
        Some(true)
    }
    pub(super) fn roblox_group_key(&mut self, happening: &Gd<InputEvent>) -> bool {
        if !self.is_roblox() {
            return false;
        }
        let Ok(k) = happening.clone().try_cast::<InputEventKey>() else {
            return false;
        };
        let ribbon_shown = match &self.ui {
            Some(LayoutUi::Roblox(r)) => {
                r.ribbon.root.is_instance_valid() && r.ribbon.root.is_visible_in_tree()
            }
            _ => false,
        };
        let over_viewport = EditorInterface::singleton()
            .get_base_control()
            .and_then(|c| c.get_viewport())
            .is_some_and(|v| in_main_screen(v.get_mouse_position()));
        let matched = GROUP_KEYS.iter().find_map(|(id, a)| {
            shortcuts::ENTRIES
                .iter()
                .position(|e| e.id == *id)
                .filter(|i| shortcuts::fits(&self.shortcut_table, *i, &k))
                .map(|_| *a)
        });
        let ctx = GroupKeyContext {
            roblox: self.is_roblox(),
            fresh_press: k.is_pressed() && !k.is_echo(),
            ribbon_shown,
            over_main_screen: over_viewport,
            typing_in_text: typing_in_text(),
            has_selection: !top_selection().is_empty(),
        };
        let Some(action) = core::group_key_action(ctx, matched) else {
            return false;
        };
        self.run_action(action);
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
        true
    }
}
