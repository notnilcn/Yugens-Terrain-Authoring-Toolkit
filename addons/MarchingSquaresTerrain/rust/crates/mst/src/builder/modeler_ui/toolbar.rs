//! Builds the builder's toolbar in the editor's menu bar, with the popovers it opens.
//!
//! The toolbar exists only while the author uses this layout and is freed when they switch
//! away, since it lives in the editor rather than in a scene. Its menus, the shape palette
//! and the handle settings are filled from the builder's command and field tables, and
//! the menu entries are regenerated for the current editing mode each time they open.
use super::super::commands;
use super::super::forms::{FieldForm, Row};
use super::super::prefs::Key;
use super::super::ui::{self, Action, Field, Mode, SyncState, Values};
use crate::editor::ui_kit;
use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, ButtonGroup, Control, EditorInterface, GridContainer, HBoxContainer, HSeparator, Label,
    LinkButton, MenuButton, PopupMenu, PopupPanel, Texture2D, VBoxContainer, VSeparator,
};
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::prelude::*;
use pt_core::build::ShapeKind;
const MODELER_ICONS: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/modeler/";
pub fn editor_icon(name: &str) -> Option<Gd<Texture2D>> {
    let theme = EditorInterface::singleton().get_editor_theme()?;
    if theme.has_icon(name, "EditorIcons") {
        return theme.get_icon(name, "EditorIcons");
    }
    theme.get_icon("MeshInstance3D", "EditorIcons")
}
pub fn editor_color(name: &str, fallback: Color) -> Color {
    EditorInterface::singleton()
        .get_editor_theme()
        .map(|t| t.get_color(name, "Editor"))
        .unwrap_or(fallback)
}
pub fn popup_below(c: &Gd<Control>) -> Rect2i {
    let rect = c.get_screen_transform() * Rect2::new(Vector2::ZERO, c.get_size());
    Rect2i::new(
        Vector2i::new(rect.position.x as i32, rect.end().y as i32),
        Vector2i::ZERO,
    )
}
pub const TILE_BEZIER_PLANE: i64 = 100;
pub const TILE_BEZIER_BLOCK: i64 = 101;
pub const TILE_ARCH_BAND: i64 = 102;
pub const PALETTE: &[(i64, &str, &str)] = &[
    (ShapeKind::Box as i64, "Box", "BoxMesh"),
    (ShapeKind::Prism as i64, "Prism", "PrismMesh"),
    (ShapeKind::Sprite as i64, "Sprite", "QuadMesh"),
    (ShapeKind::Plane as i64, "Plane", "PlaneMesh"),
    (ShapeKind::Cylinder as i64, "Cylinder", "CylinderMesh"),
    (ShapeKind::Cone as i64, "Cone", "CylinderMesh"),
    (ShapeKind::Sphere as i64, "Sphere", "SphereMesh"),
    (ShapeKind::Pipe as i64, "Pipe", "TubeTrailMesh"),
    (ShapeKind::Torus as i64, "Torus", "TorusMesh"),
    (TILE_ARCH_BAND, "Arch", "CSGBox3D"),
    (ShapeKind::Door as i64, "Door", "CSGBox3D"),
    (ShapeKind::Stairs as i64, "Stairs", "CSGBox3D"),
    (ShapeKind::Poly as i64, "Poly Shape", "CSGPolygon3D"),
    (TILE_BEZIER_PLANE, "Bezier Plane", "Curve3D"),
    (TILE_BEZIER_BLOCK, "Bezier Block", "Path3D"),
];
pub const PALETTE_BUILDER: &[ShapeKind] = &[
    ShapeKind::Icosphere,
    ShapeKind::Wedge,
    ShapeKind::Arch,
    ShapeKind::HexPrism,
    ShapeKind::HexCluster,
    ShapeKind::CurvedStairs,
    ShapeKind::Trim,
    ShapeKind::Wall,
];
pub const HANDLES_ROWS: &[Row] = &[
    Row::Toggle(Action::GizmoMove, "Move Handles"),
    Row::Toggle(Action::GizmoRotate, "Rotate Handles"),
    Row::Toggle(Action::GizmoScale, "Scale Handles"),
    Row::Field(Field::GizmoOrient),
    Row::Toggle(Action::IndividualOrigins, "Pivot: Individual Origins"),
    Row::Pref(Key::RectMode),
    Row::Pref(Key::ShiftMode),
    Row::Toggle(Action::XRay, "Selection X-Ray"),
    Row::Toggle(Action::SoftSelect, "Soft Selection"),
    Row::Field(Field::SoftRadius),
    Row::Field(Field::Falloff),
    Row::Field(Field::RotSnap),
    Row::Toggle(Action::UvLock, "UV Lock"),
    Row::Toggle(Action::SnapVertex, "Vertex Snap"),
    Row::Toggle(Action::SnapCell, "Cell Snap"),
    Row::Toggle(Action::SurfaceDraw, "Draw on Surface"),
    Row::Field(Field::GridStep),
];
pub fn handles_row_visible(r: &Row, v: &Values, st: &SyncState) -> bool {
    let element = st.mode != Mode::Object;
    match r {
        Row::Field(Field::GridStep) => true,
        Row::Field(Field::GizmoOrient) => element,
        Row::Field(f) => {
            element
                && ui::field_visible(
                    *f,
                    v,
                    &SyncState {
                        tool: ui::Tool::Select,
                        ..st.clone()
                    },
                )
        }
        Row::Toggle(Action::SnapCell | Action::SurfaceDraw, _) => true,
        Row::Toggle(..) | Row::Pref(_) => element,
        _ => true,
    }
}
pub struct Toolbar {
    pub hbox: Gd<HBoxContainer>,
    pub modes: Vec<(Mode, Gd<Button>)>,
    pub add_mesh: Gd<Button>,
    pub make_editable: Gd<Button>,
    pub actions: Gd<MenuButton>,
    pub snap: Gd<Button>,
    pub coat: Gd<Button>,
    pub handles: Gd<Button>,
    pub choices: Gd<Button>,
    pub palette: Gd<PopupPanel>,
    palette_grid: Gd<GridContainer>,
    pub handles_popup: Gd<PopupPanel>,
    pub handles_form: FieldForm,
    handles_mode: Option<Mode>,
    pub last_tile: i64,
}
fn flat_button(name: &str, caption: &str, icon: Option<Gd<Texture2D>>, tip: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_name(name);
    b.set_flat(true);
    b.set_text(caption);
    if let Some(t) = icon {
        b.set_button_icon(&t);
    }
    b.set_tooltip_text(tip);
    b.set_focus_mode(FocusMode::NONE);
    b
}
pub fn assemble(addressee: &Gd<Object>, v: &Values, st: &SyncState) -> Toolbar {
    let s = ui_kit::editor_scale();
    let mut hbox = HBoxContainer::new_alloc();
    hbox.set_name("ModelerCoreToolbar");
    hbox.add_child(&VSeparator::new_alloc());
    let group = ButtonGroup::new_gd();
    let mut modes = Vec::new();
    for (mode, name, icon, title) in [
        (Mode::Object, "ModeObject", "", "Object"),
        (Mode::Vertex, "ModeVertex", "mode_vertex", "Vertex"),
        (Mode::Edge, "ModeEdge", "mode_edge", "Edge"),
        (Mode::Face, "ModeFace", "mode_face", "Face"),
    ] {
        let tex = if icon.is_empty() {
            editor_icon("MeshInstance3D")
        } else {
            ui::mono_icon(&format!("{MODELER_ICONS}{icon}.svg"), 1.0)
                .or_else(|| editor_icon("MeshInstance3D"))
        };
        let hint = if mode == Mode::Object {
            " (Esc)"
        } else {
            " (H)"
        };
        let mut b = flat_button(name, "", tex, &format!("{title} Mode{hint}"));
        b.set_toggle_mode(true);
        b.set_button_group(&group);
        let a = match mode {
            Mode::Object => Action::ModeObject,
            Mode::Vertex => Action::ModeVertex,
            Mode::Edge => Action::ModeEdge,
            Mode::Face => Action::ModeFace,
        };
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "upon_deed").bind(&[(a as i64).to_variant()]),
        );
        hbox.add_child(&b);
        modes.push((mode, b));
    }
    let mut add_mesh = flat_button(
        "AddMesh",
        "Add Mesh",
        editor_icon("Add"),
        "Draw a new mesh. Alt-click a shape to place it at once.",
    );
    add_mesh.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_open").bind(&["palette".to_variant()]),
    );
    hbox.add_child(&add_mesh);
    let mut make_editable = flat_button(
        "MakeEditable",
        "Make Editable",
        editor_icon("ArrayMesh"),
        "Convert the selected mesh or CSG shape to an editable mesh.",
    );
    make_editable.connect(
        "pressed",
        &Callable::from_object_method(addressee, "upon_deed")
            .bind(&[(Action::Convert as i64).to_variant()]),
    );
    hbox.add_child(&make_editable);
    let mut actions = MenuButton::new_alloc();
    actions.set_name("Actions");
    actions.set_flat(true);
    actions.set_text("Actions");
    if let Some(t) = editor_icon("Tools") {
        actions.set_button_icon(&t);
    }
    actions.set_focus_mode(FocusMode::NONE);
    actions.set_disable_shortcuts(true);
    actions.connect(
        "about_to_popup",
        &Callable::from_object_method(addressee, "on_xg_actions_popup"),
    );
    if let Some(mut p) = actions.get_popup() {
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_xg_action_id"),
        );
    }
    hbox.add_child(&actions);
    let mut snap = flat_button(
        "Snap",
        "",
        editor_icon("Snap"),
        "Snap Modeler edits and new shapes to the grid.",
    );
    snap.set_toggle_mode(true);
    snap.connect(
        "pressed",
        &Callable::from_object_method(addressee, "upon_deed")
            .bind(&[(Action::SnapGrid as i64).to_variant()]),
    );
    hbox.add_child(&snap);
    let mut paint = flat_button("Paint", "", editor_icon("Paint"), "Paint");
    paint.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_open").bind(&["paint".to_variant()]),
    );
    hbox.add_child(&paint);
    let mut handles = flat_button(
        "Handles",
        "",
        editor_icon("ToolSelect"),
        "Selection & Handles",
    );
    handles.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_open").bind(&["handles".to_variant()]),
    );
    hbox.add_child(&handles);
    let mut handles_popup = PopupPanel::new_alloc();
    handles_popup.set_name("HandlesPopup");
    let mut col = VBoxContainer::new_alloc();
    col.set_name("Column");
    col.add_theme_constant_override("separation", (8.0 * s) as i32);
    let mut head = HBoxContainer::new_alloc();
    head.set_name("Header");
    let mut title = Label::new_alloc();
    title.set_name("Title");
    title.set_text("Selection & Handles");
    title.set_h_size_flags(SizeFlags::EXPAND_FILL);
    if let Some(f) = EditorInterface::singleton()
        .get_editor_theme()
        .and_then(|t| t.get_font("bold", "EditorFonts"))
    {
        title.add_theme_font_override("font", &f);
    }
    head.add_child(&title);
    let mut all = LinkButton::new_alloc();
    all.set_name("AllOptions");
    all.set_text("All Options");
    all.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_open").bind(&["options".to_variant()]),
    );
    head.add_child(&all);
    col.add_child(&head);
    let handles_form = FieldForm::new("Form");
    col.add_child(&handles_form.base);
    handles_popup.add_child(&col);
    hbox.add_child(&handles_popup);
    let mut options = flat_button("Options", "", editor_icon("GDScript"), "Modeler Options");
    options.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_open").bind(&["options".to_variant()]),
    );
    hbox.add_child(&options);
    let mut palette = PopupPanel::new_alloc();
    palette.set_name("Palette");
    let mut palette_grid = GridContainer::new_alloc();
    palette_grid.set_name("Grid");
    palette_grid.set_columns(4);
    palette.add_child(&palette_grid);
    hbox.add_child(&palette);
    let mut tb = Toolbar {
        hbox,
        modes,
        add_mesh,
        make_editable,
        actions,
        snap,
        coat: paint,
        handles,
        choices: options,
        palette,
        palette_grid,
        handles_popup,
        handles_form,
        handles_mode: None,
        last_tile: ShapeKind::Box as i64,
    };
    tb.sync(addressee, v, st);
    tb
}
impl Toolbar {
    pub fn sync(&mut self, addressee: &Gd<Object>, v: &Values, st: &SyncState) {
        let attached = st.visible && st.can_create;
        for (m, b) in &mut self.modes {
            b.set_visible(st.visible && st.has_target);
            b.set_pressed_no_signal(*m == st.mode);
        }
        self.add_mesh.set_visible(attached);
        self.add_mesh.set_disabled(!st.can_create);
        self.make_editable.set_visible(st.visible && st.can_convert);
        for mut c in [
            self.actions.clone().upcast::<Control>(),
            self.coat.clone().upcast(),
            self.handles.clone().upcast(),
        ] {
            c.set_visible(st.visible && st.has_target);
        }
        self.snap.set_visible(attached);
        self.snap.set_pressed_no_signal(st.snap_grid);
        self.choices.set_visible(st.visible);
        if self.handles_popup.is_visible() {
            self.renew_grips(addressee, v, st);
        }
    }
    pub fn renew_grips(&mut self, addressee: &Gd<Object>, v: &Values, st: &SyncState) {
        if self.handles_mode != Some(st.mode) {
            self.handles_mode = Some(st.mode);
            self.handles_form.set_rows(addressee, HANDLES_ROWS, v, st);
        } else {
            self.handles_form.sync(v, st);
        }
        self.handles_form
            .show_where(|r| handles_row_visible(r, v, st));
        self.handles_popup.reset_size();
    }
    pub fn open_handles(&mut self, addressee: &Gd<Object>, v: &Values, st: &SyncState) {
        self.handles_mode = None;
        self.renew_grips(addressee, v, st);
        let r = popup_below(&self.handles.clone().upcast());
        self.handles_popup.popup_ex().rect(r).done();
    }
    pub fn unveil_swatchset(&mut self, addressee: &Gd<Object>, can_create: bool) {
        for mut c in self.palette_grid.get_children().iter_shared() {
            self.palette_grid.remove_child(&c);
            c.queue_free();
        }
        let s = ui_kit::editor_scale();
        let accent = editor_color("accent_color", Color::from_rgb(0.4405, 0.7305, 0.9805));
        let tile = |id: i64, title: &str, icon: Option<Gd<Texture2D>>| {
            let mut b = Button::new_alloc();
            b.set_name(&format!("Tile{id}"));
            b.set_focus_mode(FocusMode::NONE);
            b.set_text(title);
            if let Some(t) = icon {
                b.set_button_icon(&t);
            }
            b.set_disabled(!can_create);
            b.set_icon_alignment(HorizontalAlignment::CENTER);
            b.set_vertical_icon_alignment(VerticalAlignment::TOP);
            b.set_expand_icon(false);
            b.set_custom_minimum_size(Vector2::new(88.0, 64.0) * s);
            if id == self.last_tile {
                b.add_theme_color_override("font_color", accent);
            }
            b.connect(
                "pressed",
                &Callable::from_object_method(addressee, "on_palette_tile")
                    .bind(&[id.to_variant()]),
            );
            b
        };
        for (id, title, icon) in PALETTE {
            let b = tile(*id, title, editor_icon(icon));
            self.palette_grid.add_child(&b);
        }
        for _ in 0..4 {
            let mut sep = HSeparator::new_alloc();
            sep.set_custom_minimum_size(Vector2::new(0.0, 6.0 * s));
            self.palette_grid.add_child(&sep);
        }
        for k in PALETTE_BUILDER {
            let title = if *k == ShapeKind::Arch {
                "Arch (wall)"
            } else {
                k.moniker()
            };
            let b = tile(*k as i64, title, ui::shape_tile_icon(*k));
            self.palette_grid.add_child(&b);
        }
        self.palette.reset_size();
        let r = popup_below(&self.add_mesh.clone().upcast());
        self.palette.popup_ex().rect(r).done();
    }
    pub fn flood_deeds(
        &mut self,
        addressee: &Gd<Object>,
        sections: &[commands::Section],
        reason: impl Fn(Action) -> Option<&'static str>,
        shortcut: impl Fn(Action) -> Option<Gd<godot::classes::Shortcut>>,
    ) {
        let Some(mut menu) = self.actions.get_popup() else {
            return;
        };
        menu.clear_ex().free_submenus(true).done();
        let fill = |m: &mut Gd<PopupMenu>, entries: &[commands::Entry]| {
            for e in entries {
                let idx = m.get_item_count();
                m.add_item_ex(&e.heading).id(e.deed as i32).done();
                if let Some(sc) = shortcut(e.deed) {
                    m.set_item_shortcut_ex(idx, &sc).global(false).done();
                }
                if let Some(r) = reason(e.deed) {
                    m.set_item_disabled(idx, true);
                    m.set_item_tooltip(idx, r);
                }
            }
        };
        for sec in sections {
            if sec.submenu {
                let mut sub = PopupMenu::new_alloc();
                sub.set_name(sec.heading.unwrap_or("Sub"));
                fill(&mut sub, &sec.entries);
                sub.connect(
                    "id_pressed",
                    &Callable::from_object_method(addressee, "on_xg_action_id"),
                );
                menu.add_submenu_node_item(sec.heading.unwrap_or(""), &sub);
            } else {
                match sec.heading {
                    Some(t) => {
                        menu.add_separator_ex().label(t).done();
                    }
                    None => {
                        menu.add_separator();
                    }
                }
                fill(&mut menu, &sec.entries);
            }
        }
    }
}
