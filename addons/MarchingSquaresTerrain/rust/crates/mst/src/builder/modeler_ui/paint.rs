//! Layout: the Paint popover (Material / Colour / Smoothing pages).
//!
//! The popover's shape: header with title + summary, *Sample from Selection*, the *Paint
//! Options* menu (Load / Save / Reset Palette, Add Material Slot…), the three page buttons and a
//! content box rebuilt per page; material, colour and group tile menus; tiles and swatches; the
//! hex and chip labels. The commands are the builder's (`plugin/paint_cmds.rs`).
//!
//! Builder additions: the header's Paint brush / Vertex paint tool toggles (the builder's
//! click-to-paint tools), the vertex-paint settings on the Colour page while Vertex paint is on,
//! the built-in material presets and blend-material commands in the Paint Options menu.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, ButtonGroup, ColorPickerButton, Control, EditorInterface, GridContainer, HBoxContainer,
    HFlowContainer, Label, Material, MenuButton, PopupMenu, PopupPanel, VBoxContainer,
};
use godot::prelude::*;

use super::super::forms::{FieldForm, Row};
use super::super::ui::{self, Action, Field, SyncState, Tool, Values};
use super::toolbar::editor_icon;
use crate::editor::ui_kit;

/// Vertex-paint rows of the Colour page (the strip's vertex paint fields).
pub const PAINT_ROWS: &[Row] = &[
    Row::Field(Field::PaintApply),
    Row::Field(Field::PaintChannel),
    Row::Field(Field::PaintRadius),
    Row::Field(Field::PaintStrength),
    Row::Field(Field::PaintSoftness),
    Row::Field(Field::PaintAlpha),
    Row::Field(Field::Slot),
];

/// Tile menu command ids (`id = cmd * 1000 + index`).
pub mod menu {
    pub const SLOT_SELECT: i64 = 1;
    pub const SLOT_BROWSE: i64 = 2;
    pub const SLOT_INSPECT: i64 = 3;
    pub const SLOT_CLEAR: i64 = 4;
    pub const COLOUR_REPLACE: i64 = 5;
    pub const COLOUR_USE: i64 = 6;
    pub const COLOUR_SELECT: i64 = 7;
    pub const COLOUR_CLEAR: i64 = 8;
    pub const GROUP_SELECT: i64 = 9;
    pub const GROUP_MERGE: i64 = 10;
    pub const GROUP_CLEAR: i64 = 11;
}

/// Smoothing page display rows (tint by group, normals, normal length).
pub const SMOOTHING_ROWS: &[Row] = &[
    Row::Field(Field::SmoothTint),
    Row::Field(Field::ShowNormals),
    Row::Field(Field::NormalLength),
];

/// Paint Options menu ids.
pub mod option {
    pub const LOAD_PALETTE: i64 = 0;
    pub const SAVE_PALETTE: i64 = 1;
    pub const RESET_PALETTE: i64 = 2;
    pub const ADD_SLOT: i64 = 3;
    /// Built-in presets: `PRESET + i`.
    pub const PRESET: i64 = 10;
    pub const BLEND: i64 = 20;
    pub const INVERT_ALPHA: i64 = 21;
}

pub const PAGES: [&str; 3] = ["Material", "Colour", "Smoothing"];

/// What the content was built for.
#[derive(Clone, PartialEq, Default)]
struct Key {
    page: usize,
    slots: Vec<i64>,
    swatches: Vec<[u8; 4]>,
    groups: Vec<(u16, usize)>,
    vpaint: bool,
}

pub struct Paint {
    pub popup: Gd<PopupPanel>,
    overview: Gd<Label>,
    brush: Gd<Button>,
    vpaint: Gd<Button>,
    pages: Vec<Gd<Button>>,
    payload: Gd<VBoxContainer>,
    pub menu: Gd<PopupMenu>,
    pub page: usize,
    token: Option<Key>,
    picker: Option<Gd<ColorPickerButton>>,
    hex: Option<Gd<Label>>,
    slot_tiles: Vec<Gd<Button>>,
    /// (group, chip) of the Smoothing page.
    chip_tiles: Vec<(u16, Gd<Button>)>,
    form: Option<FieldForm>,
    /// Smoothing page: tint / normals rows.
    attr_form: Option<FieldForm>,
    /// Menu target: (kind 0 material / 1 colour / 2 group, index).
    pub menu_target: (i64, i64),
}

fn bold(l: &mut Gd<Label>) {
    if let Some(f) = EditorInterface::singleton()
        .get_editor_theme()
        .and_then(|t| t.get_font("bold", "EditorFonts"))
    {
        l.add_theme_font_override("font", &f);
    }
}

fn patch(caption: &str, tip: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(caption);
    b.set_tooltip_text(tip);
    b.set_focus_mode(FocusMode::NONE);
    b.set_custom_minimum_size(Vector2::new(84.0, 48.0) * ui_kit::editor_scale());
    b.set_clip_text(true);
    b
}

/// Format of the hex label: `"#RRGGBB  NN%"`.
pub fn hex_label(c: Color) -> String {
    format!(
        "#{}  {}%",
        c.to_html_without_alpha().to_string().to_uppercase(),
        (c.a * 100.0).round() as i64
    )
}

/// Format of the chip label: `"<label> (<faces>)"`.
pub fn chip_label(label: &str, faces: usize) -> String {
    format!("{label} ({faces})")
}

pub fn assemble(addressee: &Gd<Object>) -> Paint {
    let s = ui_kit::editor_scale();
    let mut popup = PopupPanel::new_alloc();
    popup.set_name("PaintPopover");
    let mut col = VBoxContainer::new_alloc();
    col.set_name("Column");
    col.set_custom_minimum_size(Vector2::new(380.0 * s, 0.0));
    col.add_theme_constant_override("separation", (8.0 * s) as i32);
    let mut head = HBoxContainer::new_alloc();
    head.set_name("Header");
    let mut titles = VBoxContainer::new_alloc();
    titles.set_name("Titles");
    titles.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut t = Label::new_alloc();
    t.set_text("Paint");
    bold(&mut t);
    titles.add_child(&t);
    let mut summary = Label::new_alloc();
    summary.set_name("Summary");
    let dim = EditorInterface::singleton()
        .get_editor_theme()
        .map(|t| t.get_color("font_disabled_color", "Editor"))
        .unwrap_or(Color::from_rgb(0.6005, 0.6005, 0.6005));
    summary.add_theme_color_override("font_color", dim);
    titles.add_child(&summary);
    head.add_child(&titles);
    let tool_toggle = |name: &str, ic: &str, a: Action| {
        let mut b = Button::new_alloc();
        b.set_name(name);
        b.set_flat(true);
        b.set_toggle_mode(true);
        if let Some(t) = editor_icon(ic) {
            b.set_button_icon(&t);
        }
        b.set_tooltip_text(&ui::action_tooltip(a));
        b.set_focus_mode(FocusMode::NONE);
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_action").bind(&[(a as i64).to_variant()]),
        );
        b
    };
    let brush = tool_toggle("PaintTool", "Paint", Action::ToolPaint);
    head.add_child(&brush);
    let vpaint = tool_toggle(
        "VertexPaintTool",
        "CanvasItemMaterial",
        Action::ToolVertexPaint,
    );
    head.add_child(&vpaint);
    let mut sample = Button::new_alloc();
    sample.set_name("Sample");
    sample.set_flat(true);
    if let Some(t) = editor_icon("ColorPick") {
        sample.set_button_icon(&t);
    }
    sample.set_tooltip_text("Sample from Selection");
    sample.set_focus_mode(FocusMode::NONE);
    sample.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_xg_paint_sample"),
    );
    head.add_child(&sample);
    let mut opts = MenuButton::new_alloc();
    opts.set_name("PaintOptions");
    opts.set_flat(true);
    if let Some(t) = editor_icon("GuiTabMenuHl") {
        opts.set_button_icon(&t);
    }
    opts.set_tooltip_text("Paint Options");
    if let Some(mut p) = opts.get_popup() {
        p.add_item_ex("Load Palette…")
            .id(option::LOAD_PALETTE as i32)
            .done();
        p.add_item_ex("Save Palette…")
            .id(option::SAVE_PALETTE as i32)
            .done();
        p.add_item_ex("Reset Palette")
            .id(option::RESET_PALETTE as i32)
            .done();
        p.add_separator();
        p.add_item_ex("Add Material Slot…")
            .id(option::ADD_SLOT as i32)
            .done();
        p.add_separator_ex().label("Built-in Materials").done();
        for (i, pr) in pt_core::build::xg_paint::Preset::ALL.iter().enumerate() {
            p.add_item_ex(&format!("Add {} Slot", pr.heading()))
                .id((option::PRESET + i as i64) as i32)
                .done();
        }
        p.add_separator_ex().label("Blend").done();
        p.add_item_ex("New Blend Material")
            .id(option::BLEND as i32)
            .done();
        p.add_item_ex("Invert Alpha")
            .id(option::INVERT_ALPHA as i32)
            .done();
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_xg_paint_option"),
        );
    }
    head.add_child(&opts);
    col.add_child(&head);
    let mut sections = HBoxContainer::new_alloc();
    sections.set_name("Sections");
    sections.add_theme_constant_override("separation", 0);
    let group = ButtonGroup::new_gd();
    let mut pages = Vec::new();
    for (i, p) in PAGES.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_text(*p);
        b.set_toggle_mode(true);
        b.set_button_group(&group);
        b.set_focus_mode(FocusMode::NONE);
        b.set_h_size_flags(SizeFlags::EXPAND_FILL);
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_xg_paint_page")
                .bind(&[(i as i64).to_variant()]),
        );
        sections.add_child(&b);
        pages.push(b);
    }
    col.add_child(&sections);
    let mut content = VBoxContainer::new_alloc();
    content.set_name("Content");
    col.add_child(&content);
    popup.add_child(&col);
    let mut menu = PopupMenu::new_alloc();
    menu.set_name("Menu");
    menu.connect(
        "id_pressed",
        &Callable::from_object_method(addressee, "on_xg_paint_menu"),
    );
    popup.add_child(&menu);
    Paint {
        popup,
        overview: summary,
        brush,
        vpaint,
        pages,
        payload: content,
        menu,
        page: 0,
        token: None,
        picker: None,
        hex: None,
        slot_tiles: Vec::new(),
        chip_tiles: Vec::new(),
        form: None,
        attr_form: None,
        menu_target: (0, 0),
    }
}

impl Paint {
    /// Rebuilds the page when its key changed, else refreshes values (only while visible).
    pub fn renew(
        &mut self,
        addressee: &Gd<Object>,
        v: &Values,
        st: &SyncState,
        materials: &[Option<Gd<Material>>],
        groups: &[(u16, usize)],
    ) {
        if !self.popup.is_visible() {
            return;
        }
        self.overview.set_text(&match (st.sel_faces, st.face_slot) {
            (0, _) => "No faces selected".to_string(),
            (n, Some(s)) => format!(
                "{n} face{} selected · slot {s}",
                if n == 1 { "" } else { "s" }
            ),
            (n, None) => format!("{n} face{} selected", if n == 1 { "" } else { "s" }),
        });
        self.brush.set_pressed_no_signal(st.tool == Tool::Paint);
        self.vpaint
            .set_pressed_no_signal(st.tool == Tool::VertexPaint);
        for (i, b) in self.pages.iter_mut().enumerate() {
            b.set_pressed_no_signal(i == self.page);
        }
        let key = Key {
            page: self.page,
            slots: materials
                .iter()
                .map(|m| m.as_ref().map(|m| m.instance_id().to_i64()).unwrap_or(0))
                .collect(),
            swatches: v
                .swatches
                .iter()
                .map(|c| [c.r8(), c.g8(), c.b8(), c.a8()])
                .collect(),
            groups: groups.to_vec(),
            vpaint: st.tool == Tool::VertexPaint,
        };
        if self.token.as_ref() != Some(&key) {
            self.token = Some(key);
            self.reassemble(addressee, v, st, materials, groups);
        }
        if let Some(p) = &mut self.picker {
            p.set_pick_color(v.tint);
        }
        if let Some(h) = &mut self.hex {
            h.set_text(&hex_label(v.tint));
        }
        for (i, b) in self.slot_tiles.iter_mut().enumerate() {
            b.set_pressed_no_signal(i as i64 == v.pocket);
        }
        for (g, b) in &mut self.chip_tiles {
            b.set_pressed_no_signal(*g == v.xg.smooth_group);
        }
        if let Some(f) = &mut self.form {
            f.sync(v, st);
            f.show_where(|r| matches!(r, Row::Field(f) if ui::field_visible(*f, v, st)));
        }
        if let Some(f) = &mut self.attr_form {
            f.sync(v, st);
            f.show_where(|r| !matches!(r, Row::Field(Field::NormalLength)) || v.xg.show_normals);
        }
        self.popup.reset_size();
    }

    /// Forces a rebuild on the next refresh.
    pub fn invalidate(&mut self) {
        self.token = None;
    }

    fn reassemble(
        &mut self,
        addressee: &Gd<Object>,
        v: &Values,
        st: &SyncState,
        materials: &[Option<Gd<Material>>],
        groups: &[(u16, usize)],
    ) {
        // A typed SpinBox (e.g. Normal Length) applies its text on focus loss: block the old
        // page's signals so that never re-enters the plugin running this rebuild (R3 review).
        for mut c in self.payload.get_children().iter_shared() {
            ui_kit::set_block_signals_tree(&c, true);
            self.payload.remove_child(&c);
            c.queue_free();
        }
        self.picker = None;
        self.hex = None;
        self.slot_tiles.clear();
        self.chip_tiles.clear();
        self.form = None;
        self.attr_form = None;
        let hint = |text: &str| {
            let mut l = Label::new_alloc();
            l.set_text(text);
            l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
            l.set_custom_minimum_size(Vector2::new(360.0 * ui_kit::editor_scale(), 0.0));
            l
        };
        match self.page {
            0 => {
                let mut grid = GridContainer::new_alloc();
                grid.set_columns(4);
                for (i, m) in materials.iter().enumerate() {
                    let name = m
                        .as_ref()
                        .map(|m| m.get_name().to_string())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| {
                            if m.is_some() {
                                format!("Slot {i}")
                            } else {
                                "Empty".into()
                            }
                        });
                    let mut b = patch(
                        &format!("{i}  {name}"),
                        "Click: paint the selected faces with this slot (and make it current). Right-click: more",
                    );
                    b.set_toggle_mode(true);
                    if let Some(c) = m.as_ref().and_then(ui::material_color) {
                        ui::style_swatch(&mut b, c);
                        b.add_theme_color_override(
                            "font_color",
                            if c.luminance() > 0.5 {
                                Color::BLACK
                            } else {
                                Color::WHITE
                            },
                        );
                    }
                    let method = if m.is_some() {
                        "on_slot_pick"
                    } else {
                        "on_xg_slot_browse"
                    };
                    b.connect(
                        "pressed",
                        &Callable::from_object_method(addressee, method)
                            .bind(&[(i as i64).to_variant()]),
                    );
                    b.connect(
                        "gui_input",
                        &Callable::from_object_method(addressee, "on_xg_tile_input")
                            .bind(&[0i64.to_variant(), (i as i64).to_variant()]),
                    );
                    grid.add_child(&b);
                    self.slot_tiles.push(b);
                }
                let mut add = patch("Add Slot", "Add a material slot");
                if let Some(t) = editor_icon("Add") {
                    add.set_button_icon(&t);
                }
                add.connect(
                    "pressed",
                    &Callable::from_object_method(addressee, "on_add_slot"),
                );
                grid.add_child(&add);
                self.payload.add_child(&grid);
                let mut foot = HBoxContainer::new_alloc();
                let mut l = hint("Click to paint. Alt-click takes the selection's material.");
                l.set_h_size_flags(SizeFlags::EXPAND_FILL);
                l.set_custom_minimum_size(Vector2::ZERO);
                foot.add_child(&l);
                let mut browse = Button::new_alloc();
                browse.set_text("Browse…");
                browse.set_focus_mode(FocusMode::NONE);
                browse.set_tooltip_text("Load a material into the current slot");
                browse.connect(
                    "pressed",
                    &Callable::from_object_method(addressee, "on_xg_slot_browse")
                        .bind(&[v.pocket.to_variant()]),
                );
                foot.add_child(&browse);
                self.payload.add_child(&foot);
            }
            1 => {
                let mut row = HBoxContainer::new_alloc();
                let mut picker = ColorPickerButton::new_alloc();
                picker.set_custom_minimum_size(Vector2::new(48.0, 28.0) * ui_kit::editor_scale());
                picker.set_pick_color(v.tint);
                picker.connect(
                    "color_changed",
                    &Callable::from_object_method(addressee, "on_xg_color"),
                );
                row.add_child(&picker);
                let mut hex = Label::new_alloc();
                hex.set_text(&hex_label(v.tint));
                hex.set_h_size_flags(SizeFlags::EXPAND_FILL);
                row.add_child(&hex);
                let mut paint = Button::new_alloc();
                paint.set_text("Paint");
                paint.set_focus_mode(FocusMode::NONE);
                paint.set_tooltip_text(&ui::action_tooltip(Action::PaintColour));
                paint.connect(
                    "pressed",
                    &Callable::from_object_method(addressee, "on_action")
                        .bind(&[(Action::PaintColour as i64).to_variant()]),
                );
                row.add_child(&paint);
                self.payload.add_child(&row);
                let mut grid = GridContainer::new_alloc();
                grid.set_columns(8);
                for (i, c) in v.swatches.iter().enumerate() {
                    let mut b = Button::new_alloc();
                    b.set_custom_minimum_size(Vector2::new(36.0, 28.0) * ui_kit::editor_scale());
                    b.set_focus_mode(FocusMode::NONE);
                    b.set_tooltip_text(&hex_label(*c));
                    ui::style_swatch(&mut b, *c);
                    b.connect(
                        "pressed",
                        &Callable::from_object_method(addressee, "on_xg_swatch")
                            .bind(&[(i as i64).to_variant()]),
                    );
                    b.connect(
                        "gui_input",
                        &Callable::from_object_method(addressee, "on_xg_tile_input")
                            .bind(&[1i64.to_variant(), (i as i64).to_variant()]),
                    );
                    grid.add_child(&b);
                }
                self.payload.add_child(&grid);
                self.payload.add_child(&hint("Click to paint. Right-click to replace a colour or select the faces that use it."));
                self.picker = Some(picker);
                self.hex = Some(hex);
                if st.tool == Tool::VertexPaint {
                    let mut f = FieldForm::new("VertexPaint");
                    f.set_rows(addressee, PAINT_ROWS, v, st);
                    self.payload.add_child(&f.base);
                    self.form = Some(f);
                }
            }
            _ => {
                let mut row = HBoxContainer::new_alloc();
                for (text, cmd, tip) in [
                    (
                        "Smooth",
                        0i64,
                        "Smooth shading for the selected faces (their own smoothing group)",
                    ),
                    ("Faceted", 1, "Flat shading for the selected faces"),
                ] {
                    let mut b = Button::new_alloc();
                    b.set_text(text);
                    b.set_tooltip_text(tip);
                    b.set_focus_mode(FocusMode::NONE);
                    b.set_h_size_flags(SizeFlags::EXPAND_FILL);
                    b.connect(
                        "pressed",
                        &Callable::from_object_method(addressee, "on_xg_smoothing")
                            .bind(&[cmd.to_variant(), 0i64.to_variant()]),
                    );
                    row.add_child(&b);
                }
                self.payload.add_child(&row);
                let mut head = HBoxContainer::new_alloc();
                let mut l = Label::new_alloc();
                l.set_text("Groups in This Mesh");
                l.set_h_size_flags(SizeFlags::EXPAND_FILL);
                head.add_child(&l);
                let mut new = Button::new_alloc();
                new.set_text("New from Selection");
                new.set_focus_mode(FocusMode::NONE);
                new.connect(
                    "pressed",
                    &Callable::from_object_method(addressee, "on_xg_smoothing")
                        .bind(&[2i64.to_variant(), 0i64.to_variant()]),
                );
                head.add_child(&new);
                self.payload.add_child(&head);
                if groups.is_empty() {
                    self.payload
                        .add_child(&hint("No smoothing groups in this mesh."));
                } else {
                    let mut flow = HFlowContainer::new_alloc();
                    for (g, n) in groups {
                        let mut b = Button::new_alloc();
                        b.set_text(&chip_label(&format!("Smooth {g}"), *n));
                        b.set_focus_mode(FocusMode::NONE);
                        // The current group (Smooth joins it) shows pressed.
                        b.set_toggle_mode(true);
                        b.set_pressed_no_signal(*g == v.xg.smooth_group);
                        b.set_tooltip_text("Click: select its faces (and make it current for Smooth). Right-click: merge into the selected faces' group, or clear");
                        let c = pt_core::build::xg_paint::gliding_cluster_tint(*g);
                        b.add_theme_color_override(
                            "font_color",
                            Color::from_rgba(c[0], c[1], c[2], 1.0),
                        );
                        b.connect(
                            "pressed",
                            &Callable::from_object_method(addressee, "on_xg_smoothing")
                                .bind(&[3i64.to_variant(), (*g as i64).to_variant()]),
                        );
                        b.connect(
                            "gui_input",
                            &Callable::from_object_method(addressee, "on_xg_tile_input")
                                .bind(&[2i64.to_variant(), (*g as i64).to_variant()]),
                        );
                        flow.add_child(&b);
                        self.chip_tiles.push((*g, b));
                    }
                    self.payload.add_child(&flow);
                }
                let mut f = FieldForm::new("SmoothingDisplay");
                f.set_rows(addressee, SMOOTHING_ROWS, v, st);
                self.payload.add_child(&f.base);
                self.attr_form = Some(f);
            }
        }
    }

    /// The right-click menu of a tile.
    pub fn open_menu(&mut self, kind: i64, i: i64, at: Vector2) {
        self.menu.clear();
        self.menu_target = (kind, i);
        let items: &[(&str, i64)] = match kind {
            0 => &[
                ("Select Faces Using This", menu::SLOT_SELECT),
                ("Replace…", menu::SLOT_BROWSE),
                ("Inspect", menu::SLOT_INSPECT),
                ("Clear", menu::SLOT_CLEAR),
            ],
            1 => &[
                ("Replace with Current Colour", menu::COLOUR_REPLACE),
                ("Use as Current Colour", menu::COLOUR_USE),
                ("Select Faces Using This", menu::COLOUR_SELECT),
                ("Clear", menu::COLOUR_CLEAR),
            ],
            _ => &[
                ("Select Faces", menu::GROUP_SELECT),
                ("Merge into Selection's Group", menu::GROUP_MERGE),
                ("Clear Group", menu::GROUP_CLEAR),
            ],
        };
        for (t, cmd) in items {
            self.menu.add_item_ex(*t).id((cmd * 1000 + i) as i32).done();
        }
        self.menu.reset_size();
        self.menu
            .popup_ex()
            .rect(Rect2i::new(
                Vector2i::new(at.x as i32, at.y as i32),
                Vector2i::ZERO,
            ))
            .done();
    }

    pub fn unveil(&mut self, below: &Gd<Control>) {
        self.token = None;
        let r = super::toolbar::popup_below(below);
        self.popup.popup_ex().rect(r).done();
    }
}
