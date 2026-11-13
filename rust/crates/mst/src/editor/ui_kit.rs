//! MST-styled editor UI building blocks, shared by every MarchingSquaresTerrain editor plugin
//! (terrain tools and the Pixel* node panels), so both look and behave the same.
//!
//! Reproduces the look of the original MarchingSquaresTerrain GDScript plugin:
//! * [`Toolbar`] — icon-only, flat, toggle buttons in a `VFlowContainer` with a `ButtonGroup`,
//!   `HSeparator`s between groups; meant for `CONTAINER_SPATIAL_EDITOR_SIDE_LEFT`.
//! * [`Strip`] — attribute strip for `CONTAINER_SPATIAL_EDITOR_BOTTOM`: a title cell, then
//!   `"Label:"` + control cells in an [`MstFlow`] that wraps onto further rows (never a
//!   horizontal scrollbar, never a cut-off label) and balances the rows (no half-empty last
//!   row), a `VSeparator` whenever the control type changes (as the original does).
//! * [`side_panel`] + [`form_row`] — right-side panel (texture panel / Pixel World style) whose
//!   rows have a fixed-width label column and controls that fill the rest of the width.
//!
//! All sizes are multiplied by [`editor_scale`]. Controls report changes through a
//! `Callable` supplied by the caller (typically `Callable::from_object_method(&plugin, "on_x").bind(..)`).
//!
//! ```ignore
//! let mut tb = Toolbar::new("MyToolbar");
//! tb.separator();
//! tb.button("Brush", "Brush Tool\n\nUsed to ...", Some(icon), &cb.bind(&[0.to_variant()]));
//! let mut strip = Strip::new("MyAttributes");
//! strip.hslider("Size", 1.0, 50.0, 0.5, 15.0, &on_float.bind(&[id]));
//! ```

#![allow(dead_code)] // shared kit: not every helper is used by every plugin

use godot::classes::control::{FocusMode, MouseFilter, SizeFlags};
use godot::classes::scroll_container::ScrollMode;
use godot::classes::text_server::OverrunBehavior;
use godot::classes::{
    Button, ButtonGroup, CenterContainer, CheckBox, ColorPickerButton, Control, EditorInterface, EditorSpinSlider, HBoxContainer,
    HSeparator, HSlider, Label, LineEdit, MarginContainer, OptionButton, ResourceLoader, ScrollContainer, StyleBoxEmpty, Texture2D,
    VBoxContainer, VFlowContainer, VSeparator,
};
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::prelude::*;

/// Folder of the MST tool/node icons.
pub const ICON_DIR: &str = "res://addons/MarchingSquaresTerrain/editor/icons/";
/// Toolbar button edge (original: 30 px · editor scale).
pub const TOOL_BUTTON_SIZE: f32 = 30.0;
/// Toolbar / strip row height (original: 35 px).
pub const ROW_HEIGHT: f32 = 35.0;
/// Separation used by every MST container (original: 5).
pub const SEPARATION: i32 = 5;
/// Horizontal space between strip cells (a cell = optional separator + label + control).
pub const CELL_GAP: f32 = 10.0;
/// Width of the right side panels (texture panel, Pixel World).
pub const PANEL_WIDTH: f32 = 250.0;
/// Label column of [`form_row`]s in the side panels.
pub const FORM_LABEL_WIDTH: f32 = 74.0;
/// Modulate applied to disabled attribute controls (original: alpha 0.45).
pub const DISABLED_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.45);
/// Modulate of secondary text (hints, value read-outs).
pub const DIM_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.6);
/// Gizmo colors of the original plugin.
pub const COLOR_BRUSH: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const COLOR_PATTERN: Color = Color::from_rgb(0.7, 0.7, 0.7);
pub const COLOR_REMOVE: Color = Color::from_rgb(1.0, 0.0, 0.0);
pub const COLOR_ADD: Color = Color::from_rgb(0.0, 1.0, 0.0);
pub const COLOR_HIGHLIGHT: Color = Color::from_rgb(0.0, 0.0, 1.0);

/// Editor UI scale (1.0 outside the editor).
pub fn editor_scale() -> f32 {
    if godot::classes::Engine::singleton().is_editor_hint() { EditorInterface::singleton().get_editor_scale() } else { 1.0 }
}

pub fn sz(x: f32, y: f32) -> Vector2 {
    Vector2::new(x, y) * editor_scale()
}

pub fn px(x: f32) -> i32 {
    (x * editor_scale()).round() as i32
}

/// Loads an icon from [`ICON_DIR`] (`name` includes the extension). `None` if missing.
pub fn icon(name: &str) -> Option<Gd<Texture2D>> {
    let path = format!("{ICON_DIR}{name}");
    let mut rl = ResourceLoader::singleton();
    if !rl.exists(&path) {
        return None;
    }
    rl.load(&path).and_then(|r| r.try_cast::<Texture2D>().ok())
}

pub fn center(min: Vector2, child: &Gd<impl Inherits<Control>>) -> Gd<CenterContainer> {
    let mut c = CenterContainer::new_alloc();
    c.set_custom_minimum_size(min);
    c.add_child(&child.clone().upcast::<Control>());
    c
}

/// Left icon toolbar (VFlowContainer + ButtonGroup).
pub struct Toolbar {
    pub root: Gd<VFlowContainer>,
    pub group: Gd<ButtonGroup>,
    pub buttons: Vec<Gd<Button>>,
}

impl Toolbar {
    pub fn new(name: &str) -> Self {
        let mut root = VFlowContainer::new_alloc();
        root.set_name(name);
        root.set_custom_minimum_size(sz(ROW_HEIGHT, ROW_HEIGHT));
        root.set_alignment(godot::classes::flow_container::AlignmentMode::CENTER);
        Toolbar { root, group: ButtonGroup::new_gd(), buttons: Vec::new() }
    }

    pub fn separator(&mut self) {
        self.root.add_child(&HSeparator::new_alloc());
    }

    /// Flat icon-only toggle button. `on_pressed` is connected to `pressed`.
    pub fn button(&mut self, name: &str, tooltip: &str, icon: Option<Gd<Texture2D>>, on_pressed: &Callable) -> Gd<Button> {
        let mut b = tool_button(name, tooltip, icon);
        b.set_button_group(&self.group);
        b.connect("pressed", on_pressed);
        self.root.add_child(&center(sz(ROW_HEIGHT, ROW_HEIGHT), &b));
        self.buttons.push(b.clone());
        b
    }
}

/// Flat, icon-only, 30×30 toggle button of the left toolbars.
pub fn tool_button(name: &str, tooltip: &str, icon: Option<Gd<Texture2D>>) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_name(name);
    b.set_tooltip_text(tooltip);
    match icon {
        Some(i) => b.set_button_icon(&i),
        None => b.set_text(&name.chars().take(2).collect::<String>()),
    }
    b.set_flat(true);
    b.set_toggle_mode(true);
    b.set_expand_icon(true);
    b.set_icon_alignment(HorizontalAlignment::CENTER);
    b.set_focus_mode(FocusMode::NONE);
    b.set_custom_minimum_size(sz(TOOL_BUTTON_SIZE, TOOL_BUTTON_SIZE));
    b
}

/// Control types of the strip (a `VSeparator` is inserted when the type changes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CtlKind {
    Check,
    Slider,
    Option,
    Text,
    Button,
    Color,
    Custom,
}

/// Bottom attribute strip: an `HFlowContainer` of `[label, control]` cells that wraps onto
/// further rows instead of scrolling sideways.
pub struct Strip {
    pub root: Gd<ScrollContainer>,
    pub flow: Gd<MstFlow>,
    cell: Option<Gd<HBoxContainer>>,
    last: Option<CtlKind>,
}

/// Root of the bottom strip (created once; [`Strip::rebuild`] replaces its content).
/// Both scroll modes are disabled, so the strip takes the height of its wrapped rows.
pub fn strip_root(name: &str) -> Gd<ScrollContainer> {
    let mut root = ScrollContainer::new_alloc();
    root.set_name(name);
    root.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
    root.add_theme_stylebox_override("focus", &StyleBoxEmpty::new_gd());
    root.set_vertical_scroll_mode(ScrollMode::DISABLED);
    root.set_horizontal_scroll_mode(ScrollMode::DISABLED);
    root
}

impl Strip {
    pub fn new(name: &str) -> Self {
        Self::rebuild(strip_root(name))
    }

    /// Frees the current content of `root` and starts a fresh flow.
    pub fn rebuild(mut root: Gd<ScrollContainer>) -> Self {
        for c in root.get_children().iter_shared() {
            root.remove_child(&c);
            c.free();
        }
        let mut flow = MstFlow::new_alloc();
        flow.bind_mut().h_gap = px(CELL_GAP) as f32;
        flow.set_h_size_flags(SizeFlags::EXPAND_FILL);
        flow.set_v_size_flags(SizeFlags::FILL);
        root.add_child(&flow);
        Strip { root, flow, cell: None, last: None }
    }

    /// Starts a new cell for a control of `kind`: separator on type change, then the `"label:"`.
    fn begin(&mut self, k: CtlKind, label: &str) {
        let mut cell = HBoxContainer::new_alloc();
        cell.add_theme_constant_override("separation", px(SEPARATION as f32));
        if self.last.is_some_and(|l| l != k) {
            let mut sep = VSeparator::new_alloc();
            sep.set_custom_minimum_size(sz(CELL_GAP, ROW_HEIGHT));
            cell.add_child(&sep);
        }
        self.last = Some(k);
        self.flow.add_child(&cell);
        self.cell = Some(cell);
        if !label.is_empty() {
            self.label(label);
        }
    }

    fn add(&mut self, c: &Gd<impl Inherits<Control>>) {
        let c = c.clone().upcast::<Control>();
        match &mut self.cell {
            Some(cell) => cell.add_child(&c),
            None => self.flow.add_child(&c),
        }
    }

    /// Forces a `VSeparator` before the next control.
    pub fn separator(&mut self) {
        self.last = None;
        self.cell = None;
        let mut sep = VSeparator::new_alloc();
        sep.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
        self.flow.add_child(&sep);
    }

    /// Header cell: icon + name of the tool / section (flat, not clickable), then a separator.
    pub fn title(&mut self, text: &str, icon: Option<Gd<Texture2D>>, tooltip: &str) -> Gd<Button> {
        let mut b = title_button(text, icon);
        b.set_tooltip_text(tooltip);
        b.set_mouse_filter(MouseFilter::PASS);
        self.cell = None;
        self.flow.add_child(&b);
        self.separator();
        b
    }

    /// `"Text:"` label (original: min 50×25 in a 50×35 CenterContainer).
    pub fn label(&mut self, text: &str) -> Gd<Label> {
        let mut l = Label::new_alloc();
        l.set_text(&format!("{text}:"));
        l.set_vertical_alignment(VerticalAlignment::CENTER);
        l.set_custom_minimum_size(sz(0.0, 25.0));
        self.add(&center(sz(50.0, ROW_HEIGHT), &l));
        l
    }

    pub fn checkbox(&mut self, label: &str, value: bool, on_toggled: &Callable) -> Gd<CheckBox> {
        self.begin(CtlKind::Check, label);
        let mut cb = CheckBox::new_alloc();
        cb.set_flat(true);
        cb.set_custom_minimum_size(sz(25.0, 25.0));
        cb.set_pressed_no_signal(value);
        cb.set_focus_mode(FocusMode::NONE);
        cb.connect("toggled", on_toggled);
        self.add(&center(sz(ROW_HEIGHT, ROW_HEIGHT), &cb));
        cb
    }

    /// `EditorSpinSlider` (original: height and ease value), unbounded both ways.
    pub fn spin_slider(&mut self, label: &str, min: f64, max: f64, step: f64, value: f64, on_changed: &Callable) -> Gd<EditorSpinSlider> {
        self.begin(CtlKind::Slider, label);
        let mut s = EditorSpinSlider::new_alloc();
        s.set_flat(true);
        s.set_min(min);
        s.set_max(max);
        s.set_step(step);
        s.set_allow_greater(true);
        s.set_allow_lesser(true);
        s.set_value_no_signal(value);
        s.set_custom_minimum_size(sz(80.0, 0.0));
        s.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        s.connect("value_changed", on_changed);
        let mut m = MarginContainer::new_alloc();
        m.set_custom_minimum_size(sz(80.0, ROW_HEIGHT));
        m.add_child(&s);
        self.add(&m);
        s
    }

    /// `HSlider` (original: size, strength, grid size) followed by a read-out of its value.
    pub fn hslider(&mut self, label: &str, min: f64, max: f64, step: f64, value: f64, on_changed: &Callable) -> Gd<HSlider> {
        self.begin(CtlKind::Slider, label);
        let mut s = HSlider::new_alloc();
        s.set_min(min);
        s.set_max(max);
        s.set_step(step);
        s.set_value_no_signal(value);
        s.set_custom_minimum_size(sz(80.0, ROW_HEIGHT));
        s.set_focus_mode(FocusMode::NONE);
        s.connect("value_changed", on_changed);
        let mut m = MarginContainer::new_alloc();
        m.set_custom_minimum_size(sz(80.0, ROW_HEIGHT));
        m.add_child(&s);
        self.add(&m);
        let v = value_label(value, step, 34.0);
        s.connect("value_changed", &value_label_updater(v.clone(), step));
        self.add(&v);
        s
    }

    /// Flat `OptionButton`; `min_w` 65 (enums) or 100 (presets) in the original.
    pub fn option(&mut self, label: &str, items: &[&str], selected: i32, min_w: f32, on_selected: &Callable) -> Gd<OptionButton> {
        self.begin(CtlKind::Option, label);
        let mut ob = option_button(items, selected, min_w);
        ob.connect("item_selected", on_selected);
        self.add(&center(sz(min_w, ROW_HEIGHT), &ob));
        ob
    }

    /// Flat `LineEdit` that grows with its text; `on_submitted` gets the text.
    pub fn line_edit(&mut self, label: &str, placeholder: &str, on_submitted: &Callable) -> Gd<LineEdit> {
        self.begin(CtlKind::Text, label);
        let mut le = LineEdit::new_alloc();
        le.set_flat(true);
        le.set_expand_to_text_length_enabled(true);
        le.set_placeholder(placeholder);
        le.set_custom_minimum_size(sz(120.0, 25.0));
        le.connect("text_submitted", on_submitted);
        self.add(&center(sz(ROW_HEIGHT, ROW_HEIGHT), &le));
        le
    }

    pub fn button(&mut self, label: &str, text: &str, on_pressed: &Callable) -> Gd<Button> {
        self.begin(CtlKind::Button, label);
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_custom_minimum_size(sz(65.0, 0.0));
        b.set_focus_mode(FocusMode::NONE);
        b.connect("pressed", on_pressed);
        self.add(&center(sz(0.0, ROW_HEIGHT), &b));
        b
    }

    /// `ColorPickerButton` swatch.
    pub fn color(&mut self, label: &str, value: Color, on_changed: &Callable) -> Gd<ColorPickerButton> {
        self.begin(CtlKind::Color, label);
        let mut b = ColorPickerButton::new_alloc();
        b.set_pick_color(value);
        b.set_custom_minimum_size(sz(40.0, 22.0));
        b.connect("color_changed", on_changed);
        self.add(&center(sz(44.0, ROW_HEIGHT), &b));
        b
    }

    /// Adds any control in its own cell (the caller styles it); `kind` drives the separators.
    pub fn custom_kind(&mut self, kind: CtlKind, label: &str, c: &Gd<impl Inherits<Control>>) {
        self.begin(kind, label);
        self.add(c);
    }

    /// Adds any control (the caller styles it).
    pub fn custom(&mut self, label: &str, c: &Gd<impl Inherits<Control>>) {
        self.custom_kind(CtlKind::Custom, label, c);
    }

    /// Dimmed hint text in its own cell.
    pub fn hint(&mut self, text: &str) -> Gd<Label> {
        let mut l = Label::new_alloc();
        l.set_text(text);
        l.set_vertical_alignment(VerticalAlignment::CENTER);
        l.set_modulate(DIM_MODULATE);
        l.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
        self.cell = None;
        self.flow.add_child(&l);
        l
    }
}

/// Page tabs at the start of a strip whose attributes are split into pages (Terrain Settings,
/// large Pixel* sections): one toggle button per `(name, tooltip)`, `on_page(k)` is connected to
/// button `k`'s `pressed`. Buttons are named after their page (scripts/tests can press them).
pub fn page_tabs(name: &str, pages: &[(&str, &str)], current: usize, on_page: impl Fn(usize) -> Callable) -> Gd<HBoxContainer> {
    let mut tabs = HBoxContainer::new_alloc();
    tabs.set_name(name);
    tabs.add_theme_constant_override("separation", 0);
    let group = ButtonGroup::new_gd();
    for (k, (page, tip)) in pages.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_name(*page);
        b.set_text(*page);
        b.set_tooltip_text(*tip);
        b.set_toggle_mode(true);
        // Not flat: the current page must stand out from the others.
        b.set_button_group(&group);
        b.set_focus_mode(FocusMode::NONE);
        b.set_pressed_no_signal(k == current);
        b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        b.connect("pressed", &on_page(k));
        tabs.add_child(&b);
    }
    tabs
}

/// Number of decimals that shows `step` exactly (0.25 → 2, 1 → 0).
pub fn decimals(step: f64) -> usize {
    let mut d = 0;
    let mut s = step.abs();
    while d < 3 && (s - s.round()).abs() > 1e-6 {
        s *= 10.0;
        d += 1;
    }
    d
}

/// Slider value read-out. Updated through an object-method callable (`_mst_set_value`), not a
/// Rust closure: gdext tracks closure connections made in the editor and warns
/// ("TypedSignal connections are now auto-disconnected") at editor shutdown for each one still
/// connected.
#[derive(GodotClass)]
#[class(tool, init, base = Label, internal)]
pub struct MstValueLabel {
    decimals: i64,
    base: Base<Label>,
}

#[godot_api]
impl MstValueLabel {
    #[func]
    fn _mst_set_value(&mut self, value: f64) {
        let d = self.decimals.max(0) as usize;
        self.base_mut().set_text(&format!("{value:.d$}"));
    }
}

/// Right-aligned, dimmed value read-out (next to sliders).
pub fn value_label(value: f64, step: f64, min_w: f32) -> Gd<Label> {
    let mut l = MstValueLabel::new_alloc();
    l.bind_mut().decimals = decimals(step) as i64;
    let mut v = l.upcast::<Label>();
    v.set_text(&format!("{value:.*}", decimals(step)));
    v.set_vertical_alignment(VerticalAlignment::CENTER);
    v.set_horizontal_alignment(HorizontalAlignment::RIGHT);
    v.set_custom_minimum_size(sz(min_w, 0.0));
    v.set_v_size_flags(SizeFlags::FILL);
    v.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.75));
    v
}

/// Callable for `value_changed(float)` that writes the value into `label` (made by [`value_label`]).
pub fn value_label_updater(label: Gd<Label>, _step: f64) -> Callable {
    label.callable("_mst_set_value")
}

/// Flat, non-interactive header button (icon + name) used as the title of strips and panels.
pub fn title_button(text: &str, icon: Option<Gd<Texture2D>>) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_name("Title");
    b.set_text(text);
    if let Some(t) = icon {
        b.set_button_icon(&t);
    }
    b.set_flat(true);
    b.set_disabled(true);
    b.set_focus_mode(FocusMode::NONE);
    b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    b.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
    b.add_theme_constant_override("icon_max_width", px(16.0));
    b.add_theme_color_override("font_disabled_color", Color::from_rgb(0.92, 0.92, 0.92));
    b.add_theme_color_override("icon_disabled_color", Color::WHITE);
    b
}

pub fn option_button(items: &[&str], selected: i32, min_w: f32) -> Gd<OptionButton> {
    let mut ob = OptionButton::new_alloc();
    ob.set_flat(true);
    for i in items {
        ob.add_item(*i);
    }
    if selected >= 0 && (selected as usize) < items.len() {
        ob.select(selected);
    }
    ob.set_custom_minimum_size(sz(min_w, ROW_HEIGHT));
    ob.set_focus_mode(FocusMode::NONE);
    ob
}

/// `OptionButton` for the narrow side panels: it fills its row and shortens long item texts
/// with an ellipsis instead of growing the panel (the full text is in the popup and tooltip).
pub fn compact_option(items: &[&str], selected: i32) -> Gd<OptionButton> {
    let mut ob = option_button(items, selected, 60.0);
    ob.set_custom_minimum_size(sz(60.0, 0.0));
    ob.set_fit_to_longest_item(false);
    ob.set_clip_text(true);
    ob.set_text_overrun_behavior(OverrunBehavior::TRIM_ELLIPSIS);
    ob.set_h_size_flags(SizeFlags::EXPAND_FILL);
    ob
}

/// Greys out a control the way the original does (`modulate` α 0.45 + ignore mouse).
pub fn set_enabled(c: &mut Gd<impl Inherits<Control>>, enabled: bool) {
    let mut c = c.clone().upcast::<Control>();
    c.set_modulate(if enabled { Color::WHITE } else { DISABLED_MODULATE });
    c.set_mouse_filter(if enabled { MouseFilter::STOP } else { MouseFilter::IGNORE });
}

/// Right side panel: `ScrollContainer` ([`PANEL_WIDTH`] wide, vertical scrolling only) →
/// `VBoxContainer`. Horizontal scrolling is disabled so nothing can hide past the right edge:
/// rows must fit (use [`form_row`] / [`compact_option`]).
pub fn side_panel(name: &str) -> (Gd<ScrollContainer>, Gd<VBoxContainer>) {
    let mut root = ScrollContainer::new_alloc();
    root.set_name(name);
    root.set_custom_minimum_size(sz(PANEL_WIDTH, 0.0));
    root.add_theme_stylebox_override("focus", &StyleBoxEmpty::new_gd());
    root.set_horizontal_scroll_mode(ScrollMode::DISABLED);
    let mut vbox = VBoxContainer::new_alloc();
    vbox.set_h_size_flags(SizeFlags::EXPAND_FILL);
    vbox.add_theme_constant_override("separation", px(4.0));
    let mut m = MarginContainer::new_alloc();
    m.set_h_size_flags(SizeFlags::EXPAND_FILL);
    // Room for the vertical scrollbar so controls never sit under it.
    m.add_theme_constant_override("margin_left", px(4.0));
    m.add_theme_constant_override("margin_right", px(8.0));
    m.add_child(&vbox);
    root.add_child(&m);
    (root, vbox)
}

/// Section header of a side panel (icon + bold-ish name), optionally with a tooltip.
pub fn panel_title(text: &str, icon: Option<Gd<Texture2D>>, tooltip: &str) -> Gd<Button> {
    let mut b = title_button(text, icon);
    b.set_text_alignment(HorizontalAlignment::LEFT);
    b.set_tooltip_text(tooltip);
    b.set_mouse_filter(MouseFilter::PASS);
    b
}

/// Side-panel row: `"Text"` in a fixed [`FORM_LABEL_WIDTH`] column (ellipsis if longer, full
/// text in the tooltip), then `control` filling the rest of the width.
pub fn form_row(text: &str, control: &Gd<impl Inherits<Control>>) -> Gd<HBoxContainer> {
    form_row_w(text, control, FORM_LABEL_WIDTH)
}

pub fn form_row_w(text: &str, control: &Gd<impl Inherits<Control>>, label_w: f32) -> Gd<HBoxContainer> {
    let mut row = HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", px(SEPARATION as f32));
    row.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut l = Label::new_alloc();
    l.set_text(&format!("{text}:"));
    l.set_vertical_alignment(VerticalAlignment::CENTER);
    l.set_custom_minimum_size(sz(label_w, 0.0));
    l.set_clip_text(true);
    l.set_text_overrun_behavior(OverrunBehavior::TRIM_ELLIPSIS);
    l.set_mouse_filter(MouseFilter::PASS);
    l.set_tooltip_text(text);
    row.add_child(&l);
    let mut c = control.clone().upcast::<Control>();
    c.set_h_size_flags(SizeFlags::EXPAND_FILL);
    row.add_child(&c);
    row
}

/// `HBox[CenterContainer(50,35) Label "Text:", expanding spacer, control]` — a Terrain Settings row.
pub fn settings_row(text: &str, control: &Gd<impl Inherits<Control>>) -> Gd<HBoxContainer> {
    let mut row = HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", px(SEPARATION as f32));
    let mut l = Label::new_alloc();
    l.set_text(&format!("{text}:"));
    l.set_vertical_alignment(VerticalAlignment::CENTER);
    row.add_child(&center(sz(50.0, ROW_HEIGHT), &l));
    let mut spacer = Control::new_alloc();
    spacer.set_h_size_flags(SizeFlags::EXPAND_FILL);
    row.add_child(&spacer);
    row.add_child(&control.clone().upcast::<Control>());
    row
}

/// Row starts for balanced line breaking: the fewest rows that fit `avail` (as a greedy
/// left-to-right flow would use), with the items spread so the widest row is as narrow as
/// possible (no long first row followed by a half-empty last one).
pub fn balanced_rows(widths: &[f32], gap: f32, avail: f32) -> Vec<usize> {
    if widths.is_empty() {
        return Vec::new();
    }
    let greedy = |limit: f32| {
        let mut starts = vec![0];
        let mut w = 0.0f32;
        for (i, &x) in widths.iter().enumerate() {
            let next = if i == *starts.last().unwrap() { x } else { w + gap + x };
            if i > *starts.last().unwrap() && next > limit + 1e-3 {
                starts.push(i);
                w = x;
            } else {
                w = next;
            }
        }
        starts
    };
    let first = greedy(avail.max(1.0));
    if first.len() <= 1 {
        return first;
    }
    let rows = first.len();
    let (mut lo, mut hi) = (widths.iter().cloned().fold(0.0, f32::max), avail.max(1.0));
    if hi < lo {
        return first;
    }
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if greedy(mid).len() <= rows { hi = mid } else { lo = mid }
    }
    greedy(hi)
}

/// Flow container of the bottom strips: like `HFlowContainer`, but the rows are balanced
/// ([`balanced_rows`]) so a strip that needs two rows splits evenly instead of leaving one
/// or two cells alone on a half-empty second row.
#[derive(GodotClass)]
#[class(tool, init, base = Container, internal)]
pub struct MstFlow {
    /// Horizontal gap between cells (px).
    pub h_gap: f32,
    /// Height of the last layout (reported as the minimum height).
    min_height: f32,
    base: Base<godot::classes::Container>,
}

#[godot_api]
impl godot::classes::IContainer for MstFlow {
    fn get_minimum_size(&self) -> Vector2 {
        let w = self.cells().iter().map(|c| c.get_combined_minimum_size().x).fold(0.0, f32::max);
        Vector2::new(w, self.min_height)
    }

    fn on_notification(&mut self, what: godot::classes::notify::ContainerNotification) {
        if what == godot::classes::notify::ContainerNotification::SORT_CHILDREN {
            self.layout();
        }
    }
}

impl MstFlow {
    fn cells(&self) -> Vec<Gd<Control>> {
        self.base().get_children().iter_shared().filter_map(|c| c.try_cast::<Control>().ok()).filter(|c| c.is_visible() && !c.is_set_as_top_level()).collect()
    }

    fn layout(&mut self) {
        let cells = self.cells();
        let sizes: Vec<Vector2> = cells.iter().map(|c| c.get_combined_minimum_size()).collect();
        let avail = self.base().get_size().x;
        let starts = balanced_rows(&sizes.iter().map(|s| s.x).collect::<Vec<_>>(), self.h_gap, avail);
        let mut y = 0.0;
        for (r, &s) in starts.iter().enumerate() {
            let e = starts.get(r + 1).copied().unwrap_or(cells.len());
            let h = sizes[s..e].iter().map(|v| v.y).fold(0.0, f32::max);
            let mut x = 0.0;
            for i in s..e {
                let rect = Rect2::new(Vector2::new(x, y), Vector2::new(sizes[i].x, h));
                self.base_mut().fit_child_in_rect(&cells[i], rect);
                x += sizes[i].x + self.h_gap;
            }
            y += h;
        }
        if (y - self.min_height).abs() > 0.5 {
            self.min_height = y;
            self.base_mut().update_minimum_size();
        }
    }
}

/// A column of settings rows (Terrain Settings uses columns of 3 separated by `VSeparator`s).
pub fn column() -> Gd<VBoxContainer> {
    let mut v = VBoxContainer::new_alloc();
    v.add_theme_constant_override("separation", 0);
    v.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn balanced_rows() {
        // Fits: one row.
        assert_eq!(super::balanced_rows(&[10.0, 10.0, 10.0], 2.0, 100.0), vec![0]);
        // Greedy would give [4 items][1 item]; balanced splits 3 + 2.
        let w = [20.0; 5];
        assert_eq!(super::balanced_rows(&w, 0.0, 85.0), vec![0, 3]);
        // A wide item still gets a row of its own.
        let r = super::balanced_rows(&[50.0, 50.0, 200.0], 0.0, 120.0);
        assert_eq!(r.len(), 2);
        assert!(super::balanced_rows(&[], 0.0, 10.0).is_empty());
    }

    #[test]
    fn decimals() {
        assert_eq!(super::decimals(1.0), 0);
        assert_eq!(super::decimals(0.5), 1);
        assert_eq!(super::decimals(0.25), 2);
        assert_eq!(super::decimals(0.005), 3);
    }
}
