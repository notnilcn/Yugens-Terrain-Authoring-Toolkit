//! MST-styled editor UI building blocks, shared by every MarchingSquaresTerrain editor plugin
//! (terrain tools and the Pixel* node panels), so both look and behave the same.
//!
//! Matches the look of the classic MarchingSquaresTerrain plugin:
//! * [`Toolbar`] — labelled (icon + name + shortcut key) or compact icon-only toggle buttons in a
//!   `VFlowContainer` with a `ButtonGroup`, `HSeparator`s between groups; meant for
//!   `CONTAINER_SPATIAL_EDITOR_SIDE_LEFT`.
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

use godot::classes::control::{FocusMode, LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::image::Interpolation;
use godot::classes::scroll_container::ScrollMode;
use godot::classes::text_server::OverrunBehavior;
use godot::classes::{
    Button, ButtonGroup, CenterContainer, CheckBox, ColorPickerButton, Control, EditorInterface,
    EditorSpinSlider, FileAccess, HBoxContainer, HSeparator, HSlider, Image, ImageTexture, Label,
    LineEdit, MarginContainer, OptionButton, ResourceLoader, ScrollContainer, StyleBoxEmpty,
    Texture2D, VBoxContainer, VFlowContainer, VSeparator,
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
/// Modulate applied to disabled attribute controls (original: alpha 0.4505).
pub const DISABLED_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.4505);
/// Modulate of secondary text (hints, value read-outs).
pub const DIM_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.6005);
/// Gizmo colors of the original plugin.
pub const COLOR_BRUSH: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const COLOR_PATTERN: Color = Color::from_rgb(0.7005, 0.7005, 0.7005);
pub const COLOR_REMOVE: Color = Color::from_rgb(1.0, 0.0, 0.0);
pub const COLOR_ADD: Color = Color::from_rgb(0.0, 1.0, 0.0);
pub const COLOR_HIGHLIGHT: Color = Color::from_rgb(0.0, 0.0, 1.0);

/// Editor UI scale (1.0 outside the editor).
/// Blocks (or unblocks) the signals of `node` and every descendant, internal children included
/// (a SpinBox's LineEdit is one). Used before a subtree that
/// may hold the keyboard focus is hidden, moved or freed while a bound plugin method runs: a
/// focused SpinBox applies its typed text on focus loss and would otherwise emit
/// `value_changed` back into the (already bound) plugin.
pub fn set_block_signals_tree(node: &Gd<Node>, on: bool) {
    let mut n = node.clone();
    n.set_block_signals(on);
    for c in node
        .get_children_ex()
        .include_internal(true)
        .done()
        .iter_shared()
    {
        set_block_signals_tree(&c, on);
    }
}

pub fn editor_scale() -> f32 {
    if godot::classes::Engine::singleton().is_editor_hint() {
        EditorInterface::singleton().get_editor_scale()
    } else {
        1.0
    }
}

pub fn sz(x: f32, y: f32) -> Vector2 {
    Vector2::new(x, y) * editor_scale()
}

pub fn px(x: f32) -> i32 {
    (x * editor_scale()).round() as i32
}

/// Loads an icon from [`ICON_DIR`] (`name` includes the extension). `None` if missing.
pub fn glyph(name: &str) -> Option<Gd<Texture2D>> {
    let path = format!("{ICON_DIR}{name}");
    let mut rl = ResourceLoader::singleton();
    if !rl.exists(&path) {
        return None;
    }
    rl.load(&path).and_then(|r| r.try_cast::<Texture2D>().ok())
}

pub fn midpoint(min: Vector2, child: &Gd<impl Inherits<Control>>) -> Gd<CenterContainer> {
    let mut c = CenterContainer::new_alloc();
    c.set_custom_minimum_size(min);
    c.add_child(&child.clone().upcast::<Control>());
    c
}

/// Loads an SVG icon from [`ICON_DIR`] rasterized at `size` px × [`editor_scale`] (crisp at any
/// editor scale, unlike the 16 px imported texture stretched by `expand_icon`), slightly
/// brightened so the grey glyphs stand out on the dark editor theme. Non-SVG icons are scaled
/// from their imported texture. `None` if missing.
pub fn icon_px(name: &str, size: f32) -> Option<Gd<Texture2D>> {
    let edge = px(size).max(8);
    let path = format!("{ICON_DIR}{name}");
    let mut img = if name.ends_with(".svg") && FileAccess::file_exists(&path) {
        let svg = FileAccess::get_file_as_string(&path);
        let mut probe = Image::new_gd();
        if probe.load_svg_from_string(&svg) != godot::global::Error::OK {
            return glyph(name);
        }
        let scale = edge as f32 / probe.get_width().max(probe.get_height()).max(1) as f32;
        let mut img = Image::new_gd();
        if img.load_svg_from_string_ex(&svg).scale(scale).done() != godot::global::Error::OK {
            return glyph(name);
        }
        img
    } else {
        let mut img = glyph(name)?.get_image()?;
        if img.is_compressed() {
            img.decompress();
        }
        let (w, h) = (img.get_width().max(1), img.get_height().max(1));
        let k = edge as f32 / w.max(h) as f32;
        img.resize_ex(
            ((w as f32 * k).round() as i32).max(1),
            ((h as f32 * k).round() as i32).max(1),
        )
        .interpolation(Interpolation::LANCZOS)
        .done();
        img
    };
    img.adjust_bcs(ICON_BRIGHTNESS, 1.0, 1.0);
    ImageTexture::create_from_image(&img).map(|t| t.upcast())
}

/// Brightness factor of the toolbar icons ([`icon_px`]): lifts the mid-grey shading.
pub const ICON_BRIGHTNESS: f32 = 1.2005;
/// Toolbar icon edge (before [`editor_scale`]).
pub const TOOL_ICON_SIZE: f32 = 24.0;
/// Toolbar label font size (before [`editor_scale`]).
pub const TOOL_FONT_SIZE: f32 = 13.0;
/// Width of a labelled toolbar button (icon + label + shortcut key), before [`editor_scale`].
pub const TOOL_LABELLED_WIDTH: f32 = 116.0;
/// Height of a toolbar button, before [`editor_scale`].
pub const TOOL_ROW: f32 = 32.0;

/// Left toolbar (VFlowContainer + ButtonGroup): 24 px icons with a short visible label and the
/// shortcut key, or icon-only in compact mode ([`Toolbar::set_compact`]). Buttons are named
/// after their tool (tests press them by name).
pub struct Toolbar {
    pub base: Gd<VFlowContainer>,
    pub cluster: Gd<ButtonGroup>,
    pub buttons: Vec<Gd<Button>>,
}

impl Toolbar {
    pub fn new(name: &str) -> Self {
        let mut root = VFlowContainer::new_alloc();
        root.set_name(name);
        root.set_custom_minimum_size(sz(ROW_HEIGHT, ROW_HEIGHT));
        root.set_alignment(godot::classes::flow_container::AlignmentMode::BEGIN);
        root.add_theme_constant_override("v_separation", px(1.0));
        Toolbar {
            base: root,
            cluster: ButtonGroup::new_gd(),
            buttons: Vec::new(),
        }
    }

    pub fn separator(&mut self) {
        self.base.add_child(&HSeparator::new_alloc());
    }

    /// Icon-only toggle button (centered in a row cell). `on_pressed` is connected to `pressed`.
    pub fn button(
        &mut self,
        name: &str,
        tooltip: &str,
        icon: Option<Gd<Texture2D>>,
        on_pressed: &Callable,
    ) -> Gd<Button> {
        let mut b = instrument_clicker(name, tooltip, icon);
        b.set_button_group(&self.cluster);
        b.connect("pressed", on_pressed);
        self.base
            .add_child(&midpoint(sz(ROW_HEIGHT, ROW_HEIGHT), &b));
        self.buttons.push(b.clone());
        b
    }

    /// Labelled tool button: `icon` (an [`ICON_DIR`] file, see [`icon_px`]), `label` and the
    /// shortcut `key` (dimmed, right-aligned), in the toolbar's button group.
    pub fn tool(
        &mut self,
        name: &str,
        label: &str,
        key: Option<&str>,
        tooltip: &str,
        icon: &str,
        on_pressed: &Callable,
    ) -> Gd<Button> {
        let mut b = labelled_button(name, label, tooltip, icon);
        b.set_toggle_mode(true);
        b.set_button_group(&self.cluster);
        b.connect("pressed", on_pressed);
        if let Some(k) = key {
            let mut l = Label::new_alloc();
            l.set_text(k);
            l.set_name("Key");
            l.set_mouse_filter(MouseFilter::IGNORE);
            l.set_modulate(DIM_MODULATE);
            l.add_theme_font_size_override("font_size", px(TOOL_FONT_SIZE - 2.0));
            l.set_vertical_alignment(VerticalAlignment::CENTER);
            l.set_horizontal_alignment(HorizontalAlignment::RIGHT);
            l.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
            l.set_offset(godot::builtin::Side::RIGHT, -(px(7.0) as f32));
            b.add_child(&l);
        }
        self.base.add_child(&b);
        self.buttons.push(b.clone());
        b
    }

    /// Labelled toggle (not part of the tool group). `on_toggled` is connected to `toggled`.
    pub fn toggle(
        &mut self,
        name: &str,
        label: &str,
        tooltip: &str,
        icon: &str,
        on_toggled: &Callable,
    ) -> Gd<Button> {
        let mut b = labelled_button(name, label, tooltip, icon);
        b.set_toggle_mode(true);
        b.connect("toggled", on_toggled);
        self.base.add_child(&b);
        b
    }

    /// Plain (non-toggle) labelled button, e.g. the compact-mode switch.
    pub fn deed(
        &mut self,
        name: &str,
        label: &str,
        tooltip: &str,
        icon: Option<Gd<Texture2D>>,
        on_pressed: &Callable,
    ) -> Gd<Button> {
        let mut b = labelled_button(name, label, tooltip, "");
        if let Some(i) = icon {
            b.set_button_icon(&i);
        }
        b.connect("pressed", on_pressed);
        self.base.add_child(&b);
        b
    }

    /// Compact: icons only (square buttons). Otherwise icon + label + shortcut key.
    pub fn assign_terse(&mut self, compact: bool) {
        set_toolbar_compact(&self.base, compact);
    }
}

/// Wraps a [`Toolbar`]'s flow in a vertical-only `ScrollContainer` (named `<flow name>Rail`)
/// and keeps the flow in a single column ([`fit_rail`]): a fixed-width tool rail that scrolls
/// when the window is short, instead of a `VFlowContainer` that wraps into 2–3 columns and
/// takes that width from the 3D viewport. Add the returned rail to the editor container (not
/// the flow); show / hide the rail.
pub fn toolbar_rail(flow: &Gd<VFlowContainer>) -> Gd<ScrollContainer> {
    let mut rail = MstRail::new_alloc();
    rail.set_name(&format!("{}Rail", flow.get_name()));
    rail.add_theme_stylebox_override("focus", &StyleBoxEmpty::new_gd());
    rail.set_horizontal_scroll_mode(ScrollMode::DISABLED);
    rail.set_vertical_scroll_mode(ScrollMode::AUTO);
    rail.set_vertical_custom_step(sz(0.0, TOOL_ROW).y);
    rail.add_child(flow);
    fit_rail(flow);
    rail.upcast()
}

/// `ScrollContainer` of a [`toolbar_rail`]: refits its flow's single column whenever the theme
/// (fonts, separators) is known or changes, and before each layout.
#[derive(GodotClass)]
#[class(tool, init, base = ScrollContainer, internal)]
pub struct MstRail {
    base: Base<ScrollContainer>,
}

#[godot_api]
impl godot::classes::IScrollContainer for MstRail {
    fn on_notification(&mut self, what: godot::classes::notify::ContainerNotification) {
        use godot::classes::notify::ContainerNotification as N;
        if matches!(
            what,
            N::THEME_CHANGED | N::ENTER_TREE | N::PRE_SORT_CHILDREN
        ) && let Some(f) = self
            .base()
            .get_child(0)
            .and_then(|c| c.try_cast::<VFlowContainer>().ok())
        {
            fit_rail(&f);
        }
    }
}

/// Single column for a flow inside a [`toolbar_rail`]: its minimum height is the sum of its
/// visible children, so it never wraps (the rail scrolls instead). Call again after showing /
/// hiding / resizing buttons ([`set_toolbar_compact`] does).
pub fn fit_rail(flow: &Gd<VFlowContainer>) {
    if flow
        .get_parent()
        .and_then(|p| p.try_cast::<ScrollContainer>().ok())
        .is_none()
    {
        return;
    }
    let sep = flow.get_theme_constant("v_separation") as f32;
    let (mut h, mut n) = (0.0f32, 0);
    for c in flow.get_children().iter_shared() {
        if let Ok(c) = c.try_cast::<Control>()
            && c.is_visible()
        {
            h += c.get_combined_minimum_size().y;
            n += 1;
        }
    }
    h += sep * (n.max(1) - 1) as f32;
    let mut f = flow.clone();
    let cur = f.get_custom_minimum_size();
    if (cur.y - h.ceil()).abs() > 0.5 {
        f.set_custom_minimum_size(Vector2::new(cur.x, h.ceil()));
    }
}

/// Meta on labelled toolbar buttons holding their label (see [`set_toolbar_compact`]).
const LABEL_META: &str = "mst_label";

/// Switches a [`Toolbar`]'s labelled buttons between icon + label + key and icon-only.
pub fn set_toolbar_compact(root: &Gd<VFlowContainer>, compact: bool) {
    for c in root.get_children().iter_shared() {
        let Ok(mut b) = c.try_cast::<Button>() else {
            continue;
        };
        if !b.has_meta(LABEL_META) {
            continue;
        }
        let label = b.get_meta(LABEL_META).to::<GString>();
        b.set_text(&if compact { GString::new() } else { label });
        b.set_custom_minimum_size(if compact {
            sz(TOOL_ROW, TOOL_ROW)
        } else {
            sz(TOOL_LABELLED_WIDTH, TOOL_ROW)
        });
        b.set_icon_alignment(if compact {
            HorizontalAlignment::CENTER
        } else {
            HorizontalAlignment::LEFT
        });
        if let Some(mut k) = b.get_node_or_null("Key") {
            k.set("visible", &(!compact).to_variant());
        }
    }
    fit_rail(root);
}

/// Toolbar button with a crisp [`icon_px`] icon and a visible label, drawn with the editor's
/// `FlatButton` variation (no frame at rest, highlighted background on hover / when pressed, so
/// the active tool is obvious).
fn labelled_button(name: &str, label: &str, tooltip: &str, icon: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_name(name);
    b.set_text(label);
    b.set_meta(LABEL_META, &label.to_variant());
    b.set_tooltip_text(tooltip);
    if let Some(i) = (!icon.is_empty())
        .then(|| icon_px(icon, TOOL_ICON_SIZE))
        .flatten()
    {
        b.set_button_icon(&i);
    }
    b.set_theme_type_variation("FlatButton");
    b.set_text_alignment(HorizontalAlignment::LEFT);
    b.set_icon_alignment(HorizontalAlignment::LEFT);
    b.set_focus_mode(FocusMode::NONE);
    b.set_text_overrun_behavior(OverrunBehavior::TRIM_ELLIPSIS);
    b.add_theme_font_size_override("font_size", px(TOOL_FONT_SIZE));
    b.add_theme_constant_override("h_separation", px(6.0));
    b.set_custom_minimum_size(sz(TOOL_LABELLED_WIDTH, TOOL_ROW));
    b
}

/// Flat, icon-only, 30×30 toggle button of the left toolbars.
pub fn instrument_clicker(name: &str, tooltip: &str, icon: Option<Gd<Texture2D>>) -> Gd<Button> {
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
    pub base: Gd<ScrollContainer>,
    pub flow: Gd<MstFlow>,
    cell: Option<Gd<HBoxContainer>>,
    last: Option<CtlKind>,
}

/// Tallest a bottom strip gets before it scrolls (7 rows), before [`editor_scale`].
pub const STRIP_MAX_HEIGHT: f32 = 245.0;
/// Largest share of the 3D area (viewport + strip) a bottom strip may take.
pub const STRIP_MAX_SHARE: f32 = 0.35;

/// Root of the bottom strip (created once; [`Strip::reassemble`] replaces its content).
///
/// The strip is as tall as its wrapped rows, but never taller than [`strip_cap`]: past that it
/// scrolls vertically (visible scrollbar) instead of pushing the 3D viewport down to its
/// minimum. The height is driven by the [`MstFlow`] inside (it sets the root's custom minimum
/// height after each layout and whenever the 3D area is resized). The root's minimum height
/// must not follow the flow's content height directly: the flow's height depends on its width,
/// and an uncapped strip used to overflow the editor's main screen, which in turn changed the
/// width of the wrapping toolbars beside it — a layout feedback loop that never converged
/// (`Container::_sort_children` message-queue overflow / editor crash at 1024×600).
pub fn strip_root(name: &str) -> Gd<ScrollContainer> {
    let mut root = ScrollContainer::new_alloc();
    root.set_name(name);
    root.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
    root.add_theme_stylebox_override("focus", &StyleBoxEmpty::new_gd());
    root.set_vertical_scroll_mode(ScrollMode::AUTO);
    // Rows wrap, so this only scrolls when a single cell is wider than the strip; the point is
    // that the strip's minimum width does not follow its content either (a wide cell would
    // otherwise widen the whole editor window past the screen).
    root.set_horizontal_scroll_mode(ScrollMode::AUTO);
    // One wheel notch = one strip row.
    root.set_vertical_custom_step(sz(0.0, ROW_HEIGHT).y);
    root
}

/// Height cap of a bottom strip whose root is `root`: [`STRIP_MAX_HEIGHT`], at most
/// [`STRIP_MAX_SHARE`] of the area it shares with the 3D viewport (the root's parent), and at
/// least one row.
pub fn strip_cap(root: &Gd<Control>) -> f32 {
    let max = sz(0.0, STRIP_MAX_HEIGHT).y;
    let row = sz(0.0, ROW_HEIGHT).y;
    let area = root
        .get_parent()
        .and_then(|p| p.try_cast::<Control>().ok())
        .map(|c| c.get_size().y)
        .unwrap_or(0.0);
    strip_cap_for(area, max, row)
}

/// [`strip_cap`] for an area height (0 = unknown).
pub fn strip_cap_for(area: f32, max: f32, row: f32) -> f32 {
    if area <= 0.0 {
        return max;
    }
    (area * STRIP_MAX_SHARE).min(max).max(row).floor()
}

impl Strip {
    pub fn new(name: &str) -> Self {
        Self::reassemble(strip_root(name))
    }

    /// Frees the current content of `root` and starts a fresh flow.
    pub fn reassemble(mut root: Gd<ScrollContainer>) -> Self {
        for c in root.get_children().iter_shared() {
            root.remove_child(&c);
            c.free();
        }
        let mut flow = MstFlow::new_alloc();
        flow.bind_mut().h_gap = px(CELL_GAP) as f32;
        flow.set_h_size_flags(SizeFlags::EXPAND_FILL);
        flow.set_v_size_flags(SizeFlags::FILL);
        root.add_child(&flow);
        Strip {
            base: root,
            flow,
            cell: None,
            last: None,
        }
    }

    /// Keeps the strip at its full height cap ([`strip_cap`]) whatever its content needs, so
    /// controls at the top (page tabs) stay at the same place when the content changes height.
    pub fn pin_height(&mut self) {
        self.flow.bind_mut().pinned = true;
    }

    /// Starts a new cell for a control of `kind`: separator on type change, then the `"label:"`.
    fn commence(&mut self, k: CtlKind, label: &str) {
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

    fn splice(&mut self, c: &Gd<impl Inherits<Control>>) {
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
    pub fn heading(
        &mut self,
        caption: &str,
        icon: Option<Gd<Texture2D>>,
        tooltip: &str,
    ) -> Gd<Button> {
        let mut b = title_button(caption, icon);
        b.set_tooltip_text(tooltip);
        b.set_mouse_filter(MouseFilter::PASS);
        // Top of its row when the row is taller (a Settings page block beside it).
        b.set_v_size_flags(SizeFlags::SHRINK_BEGIN);
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
        self.splice(&midpoint(sz(50.0, ROW_HEIGHT), &l));
        l
    }

    pub fn checkbox(&mut self, label: &str, value: bool, on_toggled: &Callable) -> Gd<CheckBox> {
        self.commence(CtlKind::Check, label);
        let mut cb = CheckBox::new_alloc();
        cb.set_flat(true);
        cb.set_custom_minimum_size(sz(25.0, 25.0));
        cb.set_pressed_no_signal(value);
        cb.set_focus_mode(FocusMode::NONE);
        cb.connect("toggled", on_toggled);
        self.splice(&midpoint(sz(ROW_HEIGHT, ROW_HEIGHT), &cb));
        cb
    }

    /// `EditorSpinSlider` (original: height and ease value), clamped to `min..=max`. A field
    /// whose range is only the drag span opts into typed values beyond it with [`open_range`].
    pub fn spin_slider(
        &mut self,
        label: &str,
        min: f64,
        max: f64,
        step: f64,
        value: f64,
        on_changed: &Callable,
    ) -> Gd<EditorSpinSlider> {
        self.commence(CtlKind::Slider, label);
        let mut s = EditorSpinSlider::new_alloc();
        s.set_flat(true);
        s.set_min(min);
        s.set_max(max);
        s.set_step(step);
        s.set_value_no_signal(value);
        s.set_custom_minimum_size(sz(80.0, 0.0));
        s.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        s.connect("value_changed", on_changed);
        let mut m = MarginContainer::new_alloc();
        m.set_custom_minimum_size(sz(80.0, ROW_HEIGHT));
        m.add_child(&s);
        self.splice(&m);
        s
    }

    /// `HSlider` (original: size, strength, grid size) followed by a read-out of its value.
    pub fn hslider(
        &mut self,
        label: &str,
        min: f64,
        max: f64,
        step: f64,
        value: f64,
        on_changed: &Callable,
    ) -> Gd<HSlider> {
        self.commence(CtlKind::Slider, label);
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
        self.splice(&m);
        let v = value_label(value, step, 34.0);
        s.connect("value_changed", &value_label_updater(v.clone(), step));
        s.set_meta(READOUT_META, &v.to_variant());
        self.splice(&v);
        s
    }

    /// Flat `OptionButton`; `min_w` 65 (enums) or 100 (presets) in the original.
    pub fn choice(
        &mut self,
        label: &str,
        items: &[&str],
        selected: i32,
        min_w: f32,
        on_selected: &Callable,
    ) -> Gd<OptionButton> {
        self.commence(CtlKind::Option, label);
        let mut ob = option_button(items, selected, min_w);
        ob.connect("item_selected", on_selected);
        self.splice(&midpoint(sz(min_w, ROW_HEIGHT), &ob));
        ob
    }

    /// Flat `LineEdit` that grows with its text; `on_submitted` gets the text.
    pub fn line_edit(
        &mut self,
        label: &str,
        placeholder: &str,
        on_submitted: &Callable,
    ) -> Gd<LineEdit> {
        self.commence(CtlKind::Text, label);
        let mut le = LineEdit::new_alloc();
        le.set_flat(true);
        le.set_expand_to_text_length_enabled(true);
        le.set_placeholder(placeholder);
        le.set_custom_minimum_size(sz(120.0, 25.0));
        le.connect("text_submitted", on_submitted);
        self.splice(&midpoint(sz(ROW_HEIGHT, ROW_HEIGHT), &le));
        le
    }

    pub fn button(&mut self, label: &str, text: &str, on_pressed: &Callable) -> Gd<Button> {
        self.commence(CtlKind::Button, label);
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_custom_minimum_size(sz(65.0, 0.0));
        b.set_focus_mode(FocusMode::NONE);
        b.connect("pressed", on_pressed);
        self.splice(&midpoint(sz(0.0, ROW_HEIGHT), &b));
        b
    }

    /// `ColorPickerButton` swatch.
    pub fn tint(
        &mut self,
        label: &str,
        value: Color,
        on_changed: &Callable,
    ) -> Gd<ColorPickerButton> {
        self.commence(CtlKind::Color, label);
        let mut b = ColorPickerButton::new_alloc();
        b.set_pick_color(value);
        b.set_custom_minimum_size(sz(40.0, 22.0));
        b.connect("color_changed", on_changed);
        self.splice(&midpoint(sz(44.0, ROW_HEIGHT), &b));
        b
    }

    /// Adds any control in its own cell (the caller styles it); `kind` drives the separators.
    pub fn custom_kind(&mut self, kind: CtlKind, label: &str, c: &Gd<impl Inherits<Control>>) {
        self.commence(kind, label);
        self.splice(c);
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
pub fn page_tabs(
    name: &str,
    pages: &[(&str, &str)],
    current: usize,
    on_page: impl Fn(usize) -> Callable,
) -> Gd<HBoxContainer> {
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

/// Meta key on an [`Strip::hslider`] that holds its value read-out label.
const READOUT_META: &str = "_mst_readout";

/// Shows `value` in a strip range control without emitting `value_changed` (a value changed from
/// the viewport: modifier+wheel, drag-stamp), including the read-out label of an
/// [`Strip::hslider`], which otherwise only follows the signal.
pub fn show_value(range: &mut Gd<godot::classes::Range>, value: f64) {
    range.set_value_no_signal(value);
    if range.has_meta(READOUT_META)
        && let Ok(mut l) = range.get_meta(READOUT_META).try_to::<Gd<Label>>()
    {
        // The true value even past the slider's end (Stamp Shift+wheel beyond 200): the handle
        // pins at the end, the read-out shows what the next stroke uses.
        l.call("_mst_assign_worth", &[value.to_variant()]);
    }
}

/// Lets an [`EditorSpinSlider`] accept typed values beyond its range (`greater` / `lesser`)
/// while a mouse drag still stops at `min..=max`: the allow flags are switched off for the
/// duration of the drag (`grabbed` / `ungrabbed`). Without that an open slider drags thousands
/// of units in a few pixels (QA: Slope Min 8287.5, Pixel Size 44243).
pub fn open_range(s: &mut Gd<EditorSpinSlider>, greater: bool, lesser: bool) {
    s.set_allow_greater(greater);
    s.set_allow_lesser(lesser);
    let obj = s.clone().upcast::<Object>();
    let flag = |method: &str, on: bool| {
        Callable::from_object_method(&obj, method).bind(&[on.to_variant()])
    };
    if greater {
        s.connect("grabbed", &flag("set_allow_greater", false));
        s.connect("ungrabbed", &flag("set_allow_greater", true));
    }
    if lesser {
        s.connect("grabbed", &flag("set_allow_lesser", false));
        s.connect("ungrabbed", &flag("set_allow_lesser", true));
    }
}

/// Number of decimals that shows `step` exactly (0.2505 → 2, 1 → 0).
pub fn decimals(step: f64) -> usize {
    let mut d = 0;
    let mut s = step.abs();
    while d < 3 && (s - s.round()).abs() > 1e-6 {
        s *= 10.0;
        d += 1;
    }
    d
}

/// Slider value read-out. Updated through an object-method callable (`_mst_assign_worth`), not a
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
    fn _mst_assign_worth(&mut self, value: f64) {
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
    v.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7505));
    v
}

/// Callable for `value_changed(float)` that writes the value into `label` (made by [`value_label`]).
pub fn value_label_updater(label: Gd<Label>, _step: f64) -> Callable {
    label.callable("_mst_assign_worth")
}

/// Flat, non-interactive header button (icon + name) used as the title of strips and panels.
pub fn title_button(caption: &str, icon: Option<Gd<Texture2D>>) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_name("Title");
    b.set_text(caption);
    if let Some(t) = icon {
        b.set_button_icon(&t);
    }
    b.set_flat(true);
    b.set_disabled(true);
    b.set_focus_mode(FocusMode::NONE);
    b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    b.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
    b.add_theme_constant_override("icon_max_width", px(16.0));
    b.add_theme_color_override(
        "font_disabled_color",
        Color::from_rgb(0.9205, 0.9205, 0.9205),
    );
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

/// Greys out a control the way the original does (`modulate` α 0.4505 + ignore mouse).
pub fn assign_active(c: &mut Gd<impl Inherits<Control>>, enabled: bool) {
    let mut c = c.clone().upcast::<Control>();
    c.set_modulate(if enabled {
        Color::WHITE
    } else {
        DISABLED_MODULATE
    });
    c.set_mouse_filter(if enabled {
        MouseFilter::STOP
    } else {
        MouseFilter::IGNORE
    });
    // Really disabled, not only dimmed: no toggle / drag by keyboard or script, and
    // accessibility / UI dumps report the state.
    if let Ok(mut b) = c.clone().try_cast::<godot::classes::BaseButton>() {
        b.set_disabled(!enabled);
    } else if let Ok(mut s) = c.clone().try_cast::<godot::classes::Slider>() {
        s.set_editable(enabled);
    } else if let Ok(mut s) = c.clone().try_cast::<godot::classes::SpinBox>() {
        s.set_editable(enabled);
    } else if let Ok(mut s) = c.clone().try_cast::<godot::classes::EditorSpinSlider>() {
        s.set_read_only(!enabled);
    }
    // A strip cell (`Strip::commence`): dim its caption and value read-out with the control.
    let cell = c
        .get_parent()
        .filter(|p| p.is_class("Container"))
        .and_then(|p| p.get_parent())
        .and_then(|g| g.try_cast::<HBoxContainer>().ok());
    if let Some(cell) = cell {
        for k in cell.get_children().iter_shared() {
            let Ok(mut k) = k.try_cast::<Control>() else {
                continue;
            };
            let tone = if enabled {
                Color::WHITE
            } else {
                DISABLED_MODULATE
            };
            if k.is_class("Label") {
                // Value read-out: keeps its own dim `modulate`.
                k.set_self_modulate(tone);
            } else if k.is_class("CenterContainer")
                && k.get_child_count() == 1
                && k.get_child(0).is_some_and(|l| l.is_class("Label"))
            {
                k.set_modulate(tone);
            }
        }
    }
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
    // Always show the scrollbar (the panels are long: 16 texture slots), and scroll three rows
    // per wheel notch instead of the default 1/8 page.
    root.set_vertical_scroll_mode(ScrollMode::SHOW_ALWAYS);
    root.set_vertical_custom_step(sz(0.0, ROW_HEIGHT * 3.0).y);
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
pub fn panel_title(caption: &str, icon: Option<Gd<Texture2D>>, tooltip: &str) -> Gd<Button> {
    let mut b = title_button(caption, icon);
    b.set_text_alignment(HorizontalAlignment::LEFT);
    b.set_tooltip_text(tooltip);
    b.set_mouse_filter(MouseFilter::PASS);
    b
}

/// Side-panel row: `"Text"` in a fixed [`FORM_LABEL_WIDTH`] column (ellipsis if longer, full
/// text in the tooltip), then `control` filling the rest of the width.
pub fn form_row(caption: &str, control: &Gd<impl Inherits<Control>>) -> Gd<HBoxContainer> {
    form_row_w(caption, control, FORM_LABEL_WIDTH)
}

pub fn form_row_w(
    caption: &str,
    control: &Gd<impl Inherits<Control>>,
    label_w: f32,
) -> Gd<HBoxContainer> {
    let mut row = HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", px(SEPARATION as f32));
    row.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut l = Label::new_alloc();
    l.set_text(&format!("{caption}:"));
    l.set_vertical_alignment(VerticalAlignment::CENTER);
    l.set_custom_minimum_size(sz(label_w, 0.0));
    l.set_clip_text(true);
    l.set_text_overrun_behavior(OverrunBehavior::TRIM_ELLIPSIS);
    l.set_mouse_filter(MouseFilter::PASS);
    l.set_tooltip_text(caption);
    row.add_child(&l);
    let mut c = control.clone().upcast::<Control>();
    c.set_h_size_flags(SizeFlags::EXPAND_FILL);
    row.add_child(&c);
    row
}

/// `HBox[CenterContainer(50,35) Label "Text:", expanding spacer, control]` — a Terrain Settings row.
pub fn settings_row(caption: &str, control: &Gd<impl Inherits<Control>>) -> Gd<HBoxContainer> {
    let mut row = HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", px(SEPARATION as f32));
    let mut l = Label::new_alloc();
    l.set_text(&format!("{caption}:"));
    l.set_vertical_alignment(VerticalAlignment::CENTER);
    row.add_child(&midpoint(sz(50.0, ROW_HEIGHT), &l));
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
            let next = if i == *starts.last().unwrap() {
                x
            } else {
                w + gap + x
            };
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
        if greedy(mid).len() <= rows {
            hi = mid
        } else {
            lo = mid
        }
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
    /// Strip root height pinned to the cap ([`Strip::pin_height`]).
    pub pinned: bool,
    base: Base<godot::classes::Container>,
}

#[godot_api]
impl godot::classes::IContainer for MstFlow {
    fn get_minimum_size(&self) -> Vector2 {
        let w = self
            .tiles()
            .iter()
            .map(|c| c.get_combined_minimum_size().x)
            .fold(0.0, f32::max);
        Vector2::new(w, self.min_height)
    }

    fn on_notification(&mut self, what: godot::classes::notify::ContainerNotification) {
        use godot::classes::notify::ContainerNotification as N;
        match what {
            N::SORT_CHILDREN => self.layout(),
            // The cap follows the 3D area: refit when the area (the strip root's parent) resizes.
            N::ENTER_TREE | N::EXIT_TREE => {
                let cb = self.base().callable("_mst_refit");
                if let Some(mut area) = self.strip_area() {
                    let connected = area.is_connected("resized", &cb);
                    if what == N::ENTER_TREE && !connected {
                        area.connect_flags(
                            "resized",
                            &cb,
                            godot::classes::object::ConnectFlags::DEFERRED,
                        );
                    } else if what == N::EXIT_TREE && connected {
                        area.disconnect("resized", &cb);
                    }
                }
            }
            _ => {}
        }
    }
}

#[godot_api]
impl MstFlow {
    /// Re-applies the strip root's height (the 3D area was resized).
    #[func]
    fn _mst_refit(&mut self) {
        self.fit_root();
    }
}

impl MstFlow {
    fn tiles(&self) -> Vec<Gd<Control>> {
        self.base()
            .get_children()
            .iter_shared()
            .filter_map(|c| c.try_cast::<Control>().ok())
            .filter(|c| c.is_visible() && !c.is_set_as_top_level())
            .collect()
    }

    fn layout(&mut self) {
        let cells = self.tiles();
        let sizes: Vec<Vector2> = cells
            .iter()
            .map(|c| c.get_combined_minimum_size())
            .collect();
        let avail = self.base().get_size().x;
        let starts = balanced_rows(
            &sizes.iter().map(|s| s.x).collect::<Vec<_>>(),
            self.h_gap,
            avail,
        );
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
        self.fit_root();
    }

    /// The strip root (a [`strip_root`] `ScrollContainer`) when this flow is its content.
    fn strip_root(&self) -> Option<Gd<ScrollContainer>> {
        self.base()
            .get_parent()
            .and_then(|p| p.try_cast::<ScrollContainer>().ok())
    }

    /// The area the strip shares with the 3D viewport (the strip root's parent).
    fn strip_area(&self) -> Option<Gd<Control>> {
        self.strip_root()
            .and_then(|r| r.get_parent())
            .and_then(|p| p.try_cast::<Control>().ok())
    }

    /// Sets the strip root's minimum height: the content height (or the cap when pinned),
    /// bounded by [`strip_cap`]; the rest scrolls. A bounded value whose only input that can
    /// change with the layout is the root's width (rows only grow as it shrinks), so the
    /// layout converges.
    fn fit_root(&mut self) {
        let Some(mut root) = self.strip_root() else {
            return;
        };
        let cap = strip_cap(&root.clone().upcast());
        let row = sz(0.0, ROW_HEIGHT).y;
        let want = if self.pinned {
            cap
        } else {
            self.min_height.min(cap)
        }
        .max(row)
        .ceil();
        if (root.get_custom_minimum_size().y - want).abs() > 0.5 {
            root.set_custom_minimum_size(Vector2::new(0.0, want));
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
        assert_eq!(
            super::balanced_rows(&[10.0, 10.0, 10.0], 2.0, 100.0),
            vec![0]
        );
        // Greedy would give [4 items][1 item]; balanced splits 3 + 2.
        let w = [20.0; 5];
        assert_eq!(super::balanced_rows(&w, 0.0, 85.0), vec![0, 3]);
        // A wide item still gets a row of its own.
        let r = super::balanced_rows(&[50.0, 50.0, 200.0], 0.0, 120.0);
        assert_eq!(r.len(), 2);
        assert!(super::balanced_rows(&[], 0.0, 10.0).is_empty());
    }

    #[test]
    fn strip_cap() {
        // Unknown area: the fixed maximum.
        assert_eq!(super::strip_cap_for(0.0, 245.0, 35.0), 245.0);
        // Big area: the fixed maximum; small area: its share; tiny area: one row.
        assert_eq!(super::strip_cap_for(1000.0, 245.0, 35.0), 245.0);
        assert_eq!(super::strip_cap_for(400.0, 245.0, 35.0), 140.0);
        assert_eq!(super::strip_cap_for(60.0, 245.0, 35.0), 35.0);
    }

    #[test]
    fn decimals() {
        assert_eq!(super::decimals(1.0), 0);
        assert_eq!(super::decimals(0.5), 1);
        assert_eq!(super::decimals(0.25), 2);
        assert_eq!(super::decimals(0.005), 3);
    }
}
