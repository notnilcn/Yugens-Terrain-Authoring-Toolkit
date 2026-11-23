//! The Roblox Studio-style ribbon: a view over the builder's command table, with its own
//! shortcuts and requirement reasons, that never decides what a command does.
//!
//! The ribbon is instantiated from its scene when the person editing chooses that layout and
//! is freed when they choose another, because the 3D editor's toolbar belongs to no scene an
//! author edits and the layout choice decides when it exists. Only the entries of its menus
//! that follow data, such as the shapes of the builder's palette, are filled in here.
use super::super::modeler_ui::toolbar::{self, editor_icon};
use super::super::plugin::editor_scenes;
use super::ribbon_root::PixelRobloxRibbon;
use super::super::ui::{self, Action, Field};
use super::RibbonState;
use godot::classes::{
    Button, Control, HFlowContainer, MenuButton, PopupMenu, ResourceLoader,
    ScrollContainer, SpinBox, Texture2D,
};
use godot::builtin::Side;
use godot::obj::Inherits;
use godot::prelude::*;
use pt_modeler::ShapeKind;
const ROBLOX_ICONS: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/roblox/";
type ButtonRef = fn(&PixelRobloxRibbon) -> Option<Gd<Button>>;
pub const ACTION_BUTTONS: &[(Action, &str, ButtonRef)] = &[
    (Action::RobloxSelect, "ToolSelect", |r| r.roblox_select.clone()),
    (Action::GizmoMove, "ToolMove", |r| r.gizmo_move.clone()),
    (Action::GizmoScale, "ToolScale", |r| r.gizmo_scale.clone()),
    (Action::GizmoRotate, "ToolRotate", |r| r.gizmo_rotate.clone()),
    (Action::GizmoTransform, "ToolTriangle", |r| r.gizmo_transform.clone()),
    (Action::SnapGrid, "", |r| r.snap_grid.clone()),
    (Action::RotSnapToggle, "", |r| r.rot_snap_toggle.clone()),
    (Action::ToolEditPivot, "EditorPivot", |r| r.tool_edit_pivot.clone()),
    (Action::CenterPivot, "Reload", |r| r.center_pivot.clone()),
    (Action::InsertPartAtView, "BoxMesh", |r| r.insert_part_at_view.clone()),
    (Action::MaterialPicker, "StandardMaterial3D", |r| r.material_picker.clone()),
    (Action::ColorPicker, "ColorPick", |r| r.color_picker.clone()),
    (Action::PaintColour, "", |r| r.paint_colour.clone()),
    (Action::ClickToPaint, "", |r| r.click_to_paint.clone()),
    (Action::GroupSelection, "Group", |r| r.group_selection.clone()),
    (Action::ToggleLock, "Lock", |r| r.toggle_lock.clone()),
    (Action::ToggleAnchor, "Pin", |r| r.toggle_anchor.clone()),
    (Action::AlignPanel, "ControlAlignCenter", |r| r.align_panel.clone()),
    (Action::RobloxUnion, "CSGCombiner3D", |r| r.roblox_union.clone()),
    (Action::RobloxIntersect, "CSGCombiner3D", |r| r.roblox_intersect.clone()),
    (Action::Separate, "CSGCombiner3D", |r| r.separate.clone()),
    (Action::Negate, "Remove", |r| r.negate.clone()),
    (Action::ModeObject, "MeshInstance3D", |r| r.mode_object.clone()),
    (Action::ModeFace, "MeshInstance3D", |r| r.mode_face.clone()),
    (Action::ModeEdge, "MeshInstance3D", |r| r.mode_edge.clone()),
    (Action::ModeVertex, "MeshInstance3D", |r| r.mode_vertex.clone()),
    (Action::Extrude, "MeshInstance3D", |r| r.extrude.clone()),
    (Action::Inset, "MeshInstance3D", |r| r.inset.clone()),
    (Action::Bevel, "MeshInstance3D", |r| r.bevel.clone()),
    (Action::InsertLoop, "MeshInstance3D", |r| r.insert_loop.clone()),
    (Action::Bridge, "MeshInstance3D", |r| r.bridge.clone()),
    (Action::Merge, "MeshInstance3D", |r| r.merge.clone()),
    (Action::Delete, "Remove", |r| r.delete.clone()),
    (Action::UvEditor, "UV", |r| r.uv_editor.clone()),
    (Action::PivotToSelection, "EditorPivot", |r| r.pivot_to_selection.clone()),
];
const ROBLOX_ICON_FILES: &[(Action, &str)] = &[
    (Action::GizmoMove, "move"),
    (Action::GizmoScale, "scale"),
    (Action::GizmoRotate, "rotate"),
    (Action::CenterPivot, "pivot_reset"),
];
const PART_ICONS: &[(ShapeKind, &str)] = &[
    (ShapeKind::Box, "block"),
    (ShapeKind::Sphere, "sphere"),
    (ShapeKind::Wedge, "wedge"),
    (ShapeKind::CornerWedge, "corner_wedge"),
    (ShapeKind::Cylinder, "cylinder"),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum RibbonMenu {
    DragMode = 0,
    Pivot,
    Part,
    MoreShapes,
    Group,
    Lock,
    Anchor,
    Select,
}
impl RibbonMenu {
    const ALL: [RibbonMenu; 8] = [
        RibbonMenu::DragMode,
        RibbonMenu::Pivot,
        RibbonMenu::Part,
        RibbonMenu::MoreShapes,
        RibbonMenu::Group,
        RibbonMenu::Lock,
        RibbonMenu::Anchor,
        RibbonMenu::Select,
    ];
    pub fn from_i64(v: i64) -> Option<RibbonMenu> {
        Self::ALL.iter().copied().find(|m| *m as i64 == v)
    }
}
type MenuRef = fn(&PixelRobloxRibbon) -> Option<Gd<MenuButton>>;
pub const MENU_BUTTONS: &[(RibbonMenu, MenuRef)] = &[
    (RibbonMenu::DragMode, |r| r.drag_mode.clone()),
    (RibbonMenu::Pivot, |r| r.pivot_menu.clone()),
    (RibbonMenu::Part, |r| r.part_menu.clone()),
    (RibbonMenu::Group, |r| r.group_menu.clone()),
    (RibbonMenu::Lock, |r| r.lock_menu.clone()),
    (RibbonMenu::Anchor, |r| r.anchor_menu.clone()),
    (RibbonMenu::Select, |r| r.select_menu.clone()),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonCmd {
    Run(Action),
    Surface(bool),
    Part(ShapeKind),
}
pub const MENU_ITEMS: &[(RibbonMenu, i64, RibbonCmd)] = &[
    (RibbonMenu::DragMode, 0, RibbonCmd::Surface(false)),
    (RibbonMenu::DragMode, 1, RibbonCmd::Surface(true)),
    (RibbonMenu::DragMode, 3, RibbonCmd::Run(Action::Collisions)),
    (RibbonMenu::DragMode, 6, RibbonCmd::Run(Action::SnapBoundingBoxes)),
    (RibbonMenu::DragMode, 4, RibbonCmd::Run(Action::SnapVertex)),
    (RibbonMenu::DragMode, 5, RibbonCmd::Run(Action::SnapCell)),
    (RibbonMenu::Pivot, 0, RibbonCmd::Run(Action::PivotSnap)),
    (RibbonMenu::Part, 0, RibbonCmd::Part(ShapeKind::Box)),
    (RibbonMenu::Part, 4, RibbonCmd::Part(ShapeKind::Sphere)),
    (RibbonMenu::Part, 7, RibbonCmd::Part(ShapeKind::Wedge)),
    (RibbonMenu::Part, 20, RibbonCmd::Part(ShapeKind::CornerWedge)),
    (RibbonMenu::Part, 2, RibbonCmd::Part(ShapeKind::Cylinder)),
    (RibbonMenu::Group, 0, RibbonCmd::Run(Action::GroupSelection)),
    (RibbonMenu::Group, 1, RibbonCmd::Run(Action::GroupAsFolder)),
    (RibbonMenu::Group, 2, RibbonCmd::Run(Action::Ungroup)),
    (RibbonMenu::Lock, 0, RibbonCmd::Run(Action::ClickToLock)),
    (RibbonMenu::Anchor, 0, RibbonCmd::Run(Action::ClickToAnchor)),
    (RibbonMenu::Select, 0, RibbonCmd::Run(Action::SelectLoop)),
    (RibbonMenu::Select, 1, RibbonCmd::Run(Action::SelectRing)),
    (RibbonMenu::Select, 2, RibbonCmd::Run(Action::Grow)),
    (RibbonMenu::Select, 3, RibbonCmd::Run(Action::Shrink)),
    (RibbonMenu::Select, 4, RibbonCmd::Run(Action::Similar)),
];
const PART_SHAPES: [ShapeKind; 5] = [
    ShapeKind::Box,
    ShapeKind::Sphere,
    ShapeKind::Wedge,
    ShapeKind::CornerWedge,
    ShapeKind::Cylinder,
];
pub fn command(menu: RibbonMenu, id: i64) -> Option<RibbonCmd> {
    match menu {
        RibbonMenu::MoreShapes => Some(RibbonCmd::Part(ShapeKind::from_i64(id))),
        _ => MENU_ITEMS
            .iter()
            .find(|(m, i, _)| *m == menu && *i == id)
            .map(|x| x.2),
    }
}
fn listed(a: Action, st: &RibbonState) -> (Option<&'static str>, Option<bool>) {
    st.buttons
        .iter()
        .find(|b| b.0 == a)
        .map(|b| (b.1, b.2))
        .unwrap_or((None, None))
}
pub fn button_state(a: Action, st: &RibbonState) -> (Option<&'static str>, Option<bool>) {
    let (reason, pressed) = listed(a, st);
    let pressed = match a {
        Action::RotSnapToggle => Some(st.rot_snap_on),
        _ => pressed,
    };
    (reason, pressed)
}
pub fn menu_item_state(cmd: RibbonCmd, st: &RibbonState) -> (Option<&'static str>, Option<bool>) {
    match cmd {
        RibbonCmd::Surface(on) => (None, Some(on == st.surface_draw)),
        RibbonCmd::Run(a) => {
            let (reason, pressed) = listed(a, st);
            let checked = match a {
                Action::Collisions => Some(st.collisions),
                Action::SnapVertex => Some(st.snap_vertex),
                Action::SnapCell => Some(st.snap_cell),
                _ => pressed,
            };
            (reason, checked)
        }
        RibbonCmd::Part(_) => (None, None),
    }
}
pub fn more_shapes() -> Vec<ShapeKind> {
    toolbar::PALETTE
        .iter()
        .filter(|p| p.0 < toolbar::TILE_BEZIER_PLANE)
        .map(|p| ShapeKind::from_i64(p.0))
        .chain(toolbar::PALETTE_BUILDER.iter().copied())
        .filter(|k| !PART_SHAPES.contains(k) && !k.is_polyline())
        .collect()
}
pub fn ribbon_actions() -> Vec<Action> {
    let mut out: Vec<Action> = ACTION_BUTTONS.iter().map(|b| b.0).collect();
    for (_, _, c) in MENU_ITEMS {
        if let RibbonCmd::Run(a) = c
            && !out.contains(a)
        {
            out.push(*a);
        }
    }
    out
}
fn roblox_icon(a: Action) -> Option<Gd<Texture2D>> {
    let name = ROBLOX_ICON_FILES
        .iter()
        .find(|r| r.0 == a)
        .map(|r| r.1)
        .or_else(|| ui::roblox_icon_name(a))?;
    roblox_file_icon(name)
}
pub fn roblox_file_icon(name: &str) -> Option<Gd<Texture2D>> {
    let path = format!("{ROBLOX_ICONS}{name}.svg");
    if !ResourceLoader::singleton().exists(&path) {
        return None;
    }
    ResourceLoader::singleton()
        .load(&path)
        .and_then(|r| r.try_cast::<Texture2D>().ok())
}
fn icon_for(a: Action, theme_name: &str) -> Option<Gd<Texture2D>> {
    if theme_name.is_empty() {
        return None;
    }
    roblox_icon(a)
        .or_else(|| ui::action_glyph(a))
        .or_else(|| editor_icon(theme_name))
}
pub const COMPACT_BUTTON_PX: f32 = 36.0;
pub const COMPACT_ICON_PX: f32 = 20.0;
pub const COMPACT_SIDE_PX: f32 = 1.0;
const BUTTON_STATES: [&str; 6] = [
    "normal",
    "hover",
    "pressed",
    "hover_pressed",
    "disabled",
    "focus",
];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub label_shown: bool,
    pub min_size: (f32, f32),
    pub icon_cap_px: Option<f32>,
    pub side_px: Option<f32>,
}
pub fn button_look(compact: bool, has_icon: bool, full_min: (f32, f32)) -> Look {
    if !compact {
        return Look {
            label_shown: true,
            min_size: full_min,
            icon_cap_px: None,
            side_px: None,
        };
    }
    Look {
        label_shown: !has_icon,
        min_size: (
            if has_icon { COMPACT_BUTTON_PX } else { full_min.0 },
            COMPACT_BUTTON_PX,
        ),
        icon_cap_px: has_icon.then_some(COMPACT_ICON_PX),
        side_px: has_icon.then_some(COMPACT_SIDE_PX),
    }
}
pub const PAGES_MAX_SHARE: f32 = 0.35;
pub fn pages_height(content_px: f32, room_px: Option<f32>) -> f32 {
    match room_px.filter(|r| *r > 0.0) {
        Some(room) => content_px.min(room * PAGES_MAX_SHARE),
        None => content_px,
    }
}
pub fn one_row_px(widths: &[f32], gap: f32) -> f32 {
    widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32
}
pub fn compact_wanted(preferred: bool, full_row_px: Option<f32>, available_px: f32) -> bool {
    preferred || full_row_px.is_some_and(|need| available_px >= 1.0 && need > available_px)
}
pub fn caret_look(compact: bool, full_min: (f32, f32)) -> Look {
    Look {
        label_shown: true,
        min_size: if compact {
            (full_min.0, COMPACT_BUTTON_PX)
        } else {
            full_min
        },
        icon_cap_px: None,
        side_px: compact.then_some(COMPACT_SIDE_PX),
    }
}
fn set_side_padding(b: &mut Gd<impl Inherits<Control>>, side_px: Option<f32>) {
    let mut c = b.clone().upcast::<Control>();
    for state in BUTTON_STATES {
        c.remove_theme_stylebox_override(state);
        let Some(px) = side_px else {
            continue;
        };
        let Some(mut sb) = c.get_theme_stylebox(state).map(|s| s.duplicate_resource()) else {
            continue;
        };
        sb.set_content_margin(Side::LEFT, px);
        sb.set_content_margin(Side::RIGHT, px);
        c.add_theme_stylebox_override(state, &sb);
    }
}
pub struct RibbonButton {
    pub action: Action,
    pub button: Gd<Button>,
    tooltip: String,
    reason: Option<&'static str>,
    label: GString,
    full_min: Vector2,
}
pub struct Ribbon {
    pub root: Gd<Control>,
    pub buttons: Vec<RibbonButton>,
    pub menus: Vec<(RibbonMenu, Gd<PopupMenu>)>,
    pub pages: Option<Gd<ScrollContainer>>,
    pub model_page: Option<Gd<HFlowContainer>>,
    pub edit_page: Option<Gd<HFlowContainer>>,
    pub drag_mode: Option<Gd<MenuButton>>,
    pub grid_step: Option<Gd<SpinBox>>,
    pub rot_step: Option<Gd<SpinBox>>,
    pub pickers: super::pickers::Pickers,
    carets: Vec<(Gd<MenuButton>, Vector2)>,
    compact: Option<bool>,
    full_rows_px: Option<(f32, f32)>,
}
impl Ribbon {
    pub fn assemble(addressee: &Gd<Object>) -> Option<Ribbon> {
        let scene = editor_scenes::instantiate::<PixelRobloxRibbon>(editor_scenes::ROBLOX_RIBBON)?;
        let r = scene.bind();
        let deed = |a: Action| {
            Callable::from_object_method(addressee, "upon_deed").bind(&[(a as i64).to_variant()])
        };
        let mut buttons = Vec::new();
        for (a, theme_icon, slot) in ACTION_BUTTONS {
            let Some(mut b) = slot(&r) else {
                continue;
            };
            if let Some(t) = icon_for(*a, theme_icon) {
                b.set_button_icon(&t);
            }
            let tooltip = ui::roblox_tooltip(*a);
            b.set_tooltip_text(&tooltip);
            b.connect("pressed", &deed(*a));
            buttons.push(RibbonButton {
                action: *a,
                label: b.get_text(),
                full_min: b.get_custom_minimum_size(),
                button: b,
                tooltip,
                reason: None,
            });
        }
        for (tab, edit) in [(r.model_tab.clone(), false), (r.edit_tab.clone(), true)] {
            if let Some(mut b) = tab {
                b.connect(
                    "pressed",
                    &Callable::from_object_method(addressee, "on_ribbon_tab")
                        .bind(&[edit.to_variant()]),
                );
            }
        }
        let menu_cb = |m: RibbonMenu| {
            Callable::from_object_method(addressee, "on_ribbon_menu")
                .bind(&[(m as i64).to_variant()])
        };
        let mut menus = Vec::new();
        let mut drag_mode = None;
        let mut carets = Vec::new();
        for (m, slot) in MENU_BUTTONS {
            let Some(mb) = slot(&r) else {
                continue;
            };
            carets.push((mb.clone(), mb.get_custom_minimum_size()));
            let Some(mut p) = mb.get_popup() else {
                continue;
            };
            p.connect("id_pressed", &menu_cb(*m));
            if *m == RibbonMenu::DragMode {
                drag_mode = Some(mb.clone());
            }
            if *m == RibbonMenu::Part {
                for (k, icon) in PART_ICONS {
                    let idx = p.get_item_index(*k as i32);
                    if let Some(t) = roblox_file_icon(icon).filter(|_| idx >= 0) {
                        p.set_item_icon(idx, &t);
                    }
                }
            }
            menus.push((*m, p));
        }
        if let Some(mut more) = r.more_shapes.clone() {
            for k in more_shapes() {
                more.add_item_ex(k.moniker()).id(k as i32).done();
            }
            more.connect("id_pressed", &menu_cb(RibbonMenu::MoreShapes));
            if let Some((_, part)) = menus.iter_mut().find(|m| m.0 == RibbonMenu::Part) {
                more.set_owner(Gd::null_arg());
                if let Some(mut parent) = more.get_parent() {
                    parent.remove_child(&more);
                }
                part.add_submenu_node_item("More Shapes", &more);
            }
        }
        let grid_step = r.grid_step.clone();
        if let Some(mut s) = grid_step.clone() {
            s.connect(
                "value_changed",
                &Callable::from_object_method(addressee, "on_field")
                    .bind(&[(Field::GridStep as i64).to_variant()]),
            );
        }
        let rot_step = r.rot_step.clone();
        if let Some(mut s) = rot_step.clone() {
            s.connect(
                "value_changed",
                &Callable::from_object_method(addressee, "on_ribbon_rot_step"),
            );
        }
        let pickers = super::pickers::Pickers::assemble(&r, addressee);
        let mut resized = scene.clone();
        resized.connect_flags(
            "resized",
            &Callable::from_object_method(addressee, "sync_ui_deferred"),
            godot::classes::object::ConnectFlags::DEFERRED,
        );
        let (pages, model_page, edit_page) =
            (r.pages.clone(), r.model_page.clone(), r.edit_page.clone());
        drop(r);
        for mut page in [model_page.clone(), edit_page.clone()].into_iter().flatten() {
            page.connect_flags(
                "minimum_size_changed",
                &Callable::from_object_method(addressee, "sync_ui_deferred"),
                godot::classes::object::ConnectFlags::DEFERRED,
            );
        }
        Some(Ribbon {
            pages,
            model_page,
            edit_page,
            root: scene.upcast(),
            buttons,
            menus,
            drag_mode,
            grid_step,
            rot_step,
            pickers,
            carets,
            compact: None,
            full_rows_px: None,
        })
    }
    fn apply_compact(&mut self, compact: bool) {
        if self.compact == Some(compact) {
            return;
        }
        self.compact = Some(compact);
        let blank = GString::new();
        let scale = godot::classes::EditorInterface::singleton().get_editor_scale();
        for rb in &mut self.buttons {
            let has_icon = rb.button.get_button_icon().is_some();
            let full = (rb.full_min.x, rb.full_min.y);
            let look = button_look(compact, has_icon, full);
            rb.button.set_text(if look.label_shown {
                &rb.label
            } else {
                &blank
            });
            rb.button
                .set_custom_minimum_size(Vector2::new(look.min_size.0, look.min_size.1));
            match look.icon_cap_px {
                Some(px) => rb
                    .button
                    .add_theme_constant_override("icon_max_width", (px * scale).round() as i32),
                None => rb.button.remove_theme_constant_override("icon_max_width"),
            }
            set_side_padding(&mut rb.button, look.side_px.map(|px| px * scale));
        }
        for (mb, full_min) in &mut self.carets {
            let look = caret_look(compact, (full_min.x, full_min.y));
            mb.set_custom_minimum_size(Vector2::new(look.min_size.0, look.min_size.1));
            set_side_padding(mb, look.side_px.map(|px| px * scale));
        }
    }
    fn page_width(page: &Option<Gd<HFlowContainer>>) -> f32 {
        let Some(p) = page else { return 0.0 };
        let widths: Vec<f32> = p
            .get_children()
            .iter_shared()
            .filter_map(|c| c.try_cast::<Control>().ok())
            .filter(|c| c.is_visible())
            .map(|c| c.get_combined_minimum_size().x)
            .collect();
        one_row_px(&widths, p.get_theme_constant("h_separation") as f32)
    }
    fn fit_pages(&mut self) {
        let shown = if self.edit_shown() { &self.edit_page } else { &self.model_page };
        let content = shown.as_ref().map_or(0.0, |p| p.get_combined_minimum_size().y);
        let Some(pages) = &mut self.pages else { return };
        let room = godot::classes::EditorInterface::singleton()
            .get_editor_main_screen()
            .map(|m| m.get_size().y);
        let height = pages_height(content, room).ceil();
        if (pages.get_custom_minimum_size().y - height).abs() >= 1.0 {
            pages.set_custom_minimum_size(Vector2::new(0.0, height));
        }
    }
    fn edit_shown(&self) -> bool {
        self.edit_page.as_ref().is_some_and(|p| p.is_visible())
    }
    fn settle_compact(&mut self, preferred: bool) {
        if !preferred && self.full_rows_px.is_none() && self.root.is_inside_tree() {
            self.apply_compact(false);
            self.full_rows_px = Some((
                Self::page_width(&self.model_page),
                Self::page_width(&self.edit_page),
            ));
        }
        let need = self
            .full_rows_px
            .map(|(model, edit)| if self.edit_shown() { edit } else { model });
        let available = self.root.get_size().x;
        self.apply_compact(compact_wanted(preferred, need, available));
    }
    pub fn show_tab(&mut self, edit: bool) {
        if let Some(p) = &mut self.model_page {
            p.set_visible(!edit);
        }
        if let Some(p) = &mut self.edit_page {
            p.set_visible(edit);
        }
    }
    pub fn button(&self, a: Action) -> Option<Gd<Button>> {
        self.buttons
            .iter()
            .find(|b| b.action == a)
            .map(|b| b.button.clone())
    }
    fn menu(&self, m: RibbonMenu) -> Option<Gd<PopupMenu>> {
        self.menus.iter().find(|x| x.0 == m).map(|x| x.1.clone())
    }
    pub fn sync(&mut self, st: &RibbonState) {
        self.settle_compact(st.compact);
        self.fit_pages();
        for rb in &mut self.buttons {
            let (reason, pressed) = button_state(rb.action, st);
            if rb.button.is_disabled() != reason.is_some() {
                rb.button.set_disabled(reason.is_some());
            }
            if rb.reason != reason {
                rb.reason = reason;
                let tip = match reason {
                    Some(r) => format!("{}\n\n{r}", rb.tooltip),
                    None => rb.tooltip.clone(),
                };
                rb.button.set_tooltip_text(&tip);
            }
            if let Some(p) = pressed
                && rb.button.is_toggle_mode()
                && rb.button.is_pressed() != p
            {
                rb.button.set_pressed_no_signal(p);
            }
        }
        if let Some(s) = &mut self.grid_step
            && !s.has_focus()
            && (s.get_value() - st.grid_step as f64).abs() > 1e-6
        {
            s.set_value_no_signal(st.grid_step as f64);
        }
        if let Some(s) = &mut self.rot_step
            && !s.has_focus()
            && (s.get_value() - st.rot_snap_deg as f64).abs() > 1e-6
        {
            s.set_value_no_signal(st.rot_snap_deg as f64);
        }
        if let Some(d) = &mut self.drag_mode {
            d.set_text(if st.surface_draw {
                "Surface"
            } else {
                "Geometric"
            });
        }
        for (m, id, cmd) in MENU_ITEMS {
            let Some(mut p) = self.menu(*m) else {
                continue;
            };
            let idx = p.get_item_index(*id as i32);
            if idx < 0 {
                continue;
            }
            let (reason, checked) = menu_item_state(*cmd, st);
            p.set_item_disabled(idx, reason.is_some());
            p.set_item_tooltip(idx, reason.unwrap_or(""));
            if let Some(c) = checked
                && p.is_item_checkable(idx)
            {
                p.set_item_checked(idx, c);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const SCENE: &str = include_str!("../../../../../../editor/builder/roblox_ribbon.tscn");
    #[test]
    fn compact_hides_labels_only_where_an_icon_stays() {
        let full = (56.0, 54.0);
        let plain = button_look(false, true, full);
        assert!(plain.label_shown);
        assert_eq!(plain.min_size, full);
        let iconic = button_look(true, true, full);
        assert!(!iconic.label_shown);
        assert_eq!(iconic.min_size, (COMPACT_BUTTON_PX, COMPACT_BUTTON_PX));
        assert_eq!(iconic.icon_cap_px, Some(COMPACT_ICON_PX));
        assert_eq!(plain.icon_cap_px, None);
        assert_eq!(plain.side_px, None);
        assert_eq!(iconic.side_px, Some(COMPACT_SIDE_PX));
        assert_eq!(caret_look(true, (14.0, 54.0)).side_px, Some(COMPACT_SIDE_PX));
        assert_eq!(caret_look(false, (14.0, 54.0)).side_px, None);
        let wordy = button_look(true, false, full);
        assert!(wordy.label_shown);
        assert_eq!(wordy.icon_cap_px, None);
        assert_eq!(wordy.side_px, None);
        assert_eq!(wordy.min_size, (56.0, COMPACT_BUTTON_PX));
        assert_eq!(caret_look(true, (14.0, 54.0)).min_size, (14.0, COMPACT_BUTTON_PX));
        assert_eq!(caret_look(false, (14.0, 54.0)).min_size, (14.0, 54.0));
    }
    #[test]
    fn the_ribbon_compacts_itself_when_its_full_row_does_not_fit() {
        assert!(compact_wanted(true, Some(500.0), 900.0));
        assert!(!compact_wanted(false, Some(900.0), 900.0));
        assert!(compact_wanted(false, Some(901.0), 900.0));
        assert!(!compact_wanted(false, None, 300.0));
        assert!(!compact_wanted(false, Some(901.0), 0.0));
    }
    #[test]
    fn the_pages_wrap_into_rows_and_scroll_vertically() {
        assert!(SCENE.contains("[node name=\"Pages\" type=\"ScrollContainer\" parent=\".\"]"));
        assert!(SCENE.contains("[node name=\"ModelPage\" type=\"HFlowContainer\" parent=\"Pages/Stack\"]"));
        assert!(SCENE.contains("[node name=\"EditPage\" type=\"HFlowContainer\" parent=\"Pages/Stack\"]"));
    }
    #[test]
    fn the_pages_claim_their_rows_up_to_a_share_of_the_editor() {
        assert_eq!(pages_height(60.0, Some(800.0)), 60.0);
        assert_eq!(pages_height(400.0, Some(800.0)), 800.0 * PAGES_MAX_SHARE);
        assert_eq!(pages_height(400.0, None), 400.0);
        assert_eq!(pages_height(400.0, Some(0.0)), 400.0);
    }
    #[test]
    fn one_row_is_every_group_side_by_side() {
        assert_eq!(one_row_px(&[100.0, 50.0, 30.0], 4.0), 188.0);
        assert_eq!(one_row_px(&[], 4.0), 0.0);
    }
    #[test]
    fn drag_mode_offers_bounding_box_snap() {
        assert_eq!(
            command(RibbonMenu::DragMode, 6),
            Some(RibbonCmd::Run(Action::SnapBoundingBoxes))
        );
        assert!(SCENE.contains("\"Snap: Use Bounding Boxes\""));
    }
    #[test]
    fn part_menu_ids_are_shape_kinds() {
        for (m, id, cmd) in MENU_ITEMS {
            if let (RibbonMenu::Part, RibbonCmd::Part(k)) = (m, cmd) {
                assert_eq!(*k as i64, *id);
                assert_eq!(ShapeKind::from_i64(*id), *k);
            }
        }
    }
    #[test]
    fn more_shapes_skip_the_part_shapes() {
        let more = more_shapes();
        assert!(!more.is_empty());
        for k in PART_SHAPES {
            assert!(!more.contains(&k));
        }
    }
    #[test]
    fn every_ribbon_action_resolves() {
        for a in ribbon_actions() {
            assert_eq!(Action::from_i64(a as i64), Some(a), "{a:?}");
        }
    }
    #[test]
    fn rotate_snap_shows_its_own_setting_and_other_buttons_show_their_rule() {
        let st = RibbonState {
            buttons: vec![
                (Action::RotSnapToggle, None, Some(false)),
                (Action::ToggleLock, Some("why"), Some(true)),
            ],
            rot_snap_on: true,
            ..Default::default()
        };
        assert_eq!(button_state(Action::RotSnapToggle, &st), (None, Some(true)));
        assert_eq!(button_state(Action::ToggleLock, &st), (Some("why"), Some(true)));
        assert_eq!(button_state(Action::GizmoMove, &st), (None, None));
    }
    #[test]
    fn menu_checks_follow_the_drag_settings() {
        let st = RibbonState {
            buttons: vec![
                (Action::Collisions, Some("why"), Some(false)),
                (Action::ClickToLock, None, Some(true)),
            ],
            surface_draw: true,
            collisions: true,
            snap_vertex: false,
            snap_cell: true,
            ..Default::default()
        };
        assert_eq!(menu_item_state(RibbonCmd::Surface(true), &st), (None, Some(true)));
        assert_eq!(menu_item_state(RibbonCmd::Surface(false), &st), (None, Some(false)));
        assert_eq!(menu_item_state(RibbonCmd::Run(Action::Collisions), &st), (Some("why"), Some(true)));
        assert_eq!(menu_item_state(RibbonCmd::Run(Action::SnapVertex), &st), (None, Some(false)));
        assert_eq!(menu_item_state(RibbonCmd::Run(Action::SnapCell), &st), (None, Some(true)));
        assert_eq!(menu_item_state(RibbonCmd::Run(Action::ClickToLock), &st), (None, Some(true)));
        assert_eq!(menu_item_state(RibbonCmd::Part(ShapeKind::CornerWedge), &st), (None, None));
    }
}
