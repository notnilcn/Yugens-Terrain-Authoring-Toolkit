//! The shared look of the editor's docked controls, so that every addon panel builds them
//! the same way and at the editor's display scale.
//!
//! It creates controls in code because the panels that use it assemble them from data
//! tables and from the state of whatever is being edited, and rebuild them when either
//! changes.
use super::mst_flow::MstFlow;
use super::mst_value_label::{value_label, value_label_updater};
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
pub const ICON_DIR: &str = "res://addons/MarchingSquaresTerrain/editor/icons/";
pub const TOOL_BUTTON_SIZE: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 35.0;
pub const SEPARATION: i32 = 5;
pub const CELL_GAP: f32 = 10.0;
pub const PANEL_WIDTH: f32 = 250.0;
pub const FORM_LABEL_WIDTH: f32 = 74.0;
pub const DISABLED_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.4505);
pub const DIM_MODULATE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.6005);
pub const COLOR_PATTERN: Color = Color::from_rgb(0.7005, 0.7005, 0.7005);
pub const COLOR_REMOVE: Color = Color::from_rgb(1.0, 0.0, 0.0);
pub const COLOR_ADD: Color = Color::from_rgb(0.0, 1.0, 0.0);
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
pub const ICON_BRIGHTNESS: f32 = 1.2005;
pub const TOOL_ICON_SIZE: f32 = 24.0;
pub const TOOL_FONT_SIZE: f32 = 13.0;
pub const TOOL_LABELLED_WIDTH: f32 = 116.0;
pub const TOOL_ROW: f32 = 32.0;
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
    pub fn assign_terse(&mut self, compact: bool) {
        set_toolbar_compact(&self.base, compact);
    }
}
const LABEL_META: &str = "mst_label";
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
}
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
pub struct Strip {
    pub base: Gd<ScrollContainer>,
    pub flow: Gd<MstFlow>,
    cell: Option<Gd<HBoxContainer>>,
    last: Option<CtlKind>,
}
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
    pub fn separator(&mut self) {
        self.last = None;
        self.cell = None;
        let mut sep = VSeparator::new_alloc();
        sep.set_custom_minimum_size(sz(0.0, ROW_HEIGHT));
        self.flow.add_child(&sep);
    }
    pub fn heading(
        &mut self,
        caption: &str,
        icon: Option<Gd<Texture2D>>,
        tooltip: &str,
    ) -> Gd<Button> {
        let mut b = title_button(caption, icon);
        b.set_tooltip_text(tooltip);
        b.set_mouse_filter(MouseFilter::PASS);
        self.cell = None;
        self.flow.add_child(&b);
        self.separator();
        b
    }
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
        s.set_allow_greater(true);
        s.set_allow_lesser(true);
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
        self.splice(&v);
        s
    }
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
    pub fn custom_kind(&mut self, kind: CtlKind, label: &str, c: &Gd<impl Inherits<Control>>) {
        self.commence(kind, label);
        self.splice(c);
    }
    pub fn custom(&mut self, label: &str, c: &Gd<impl Inherits<Control>>) {
        self.custom_kind(CtlKind::Custom, label, c);
    }
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
        b.set_button_group(&group);
        b.set_focus_mode(FocusMode::NONE);
        b.set_pressed_no_signal(k == current);
        b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        b.connect("pressed", &on_page(k));
        tabs.add_child(&b);
    }
    tabs
}
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
pub fn compact_option(items: &[&str], selected: i32) -> Gd<OptionButton> {
    let mut ob = option_button(items, selected, 60.0);
    ob.set_custom_minimum_size(sz(60.0, 0.0));
    ob.set_fit_to_longest_item(false);
    ob.set_clip_text(true);
    ob.set_text_overrun_behavior(OverrunBehavior::TRIM_ELLIPSIS);
    ob.set_h_size_flags(SizeFlags::EXPAND_FILL);
    ob
}
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
}
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
    m.add_theme_constant_override("margin_left", px(4.0));
    m.add_theme_constant_override("margin_right", px(8.0));
    m.add_child(&vbox);
    root.add_child(&m);
    (root, vbox)
}
pub fn panel_title(caption: &str, icon: Option<Gd<Texture2D>>, tooltip: &str) -> Gd<Button> {
    let mut b = title_button(caption, icon);
    b.set_text_alignment(HorizontalAlignment::LEFT);
    b.set_tooltip_text(tooltip);
    b.set_mouse_filter(MouseFilter::PASS);
    b
}
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
pub fn column() -> Gd<VBoxContainer> {
    let mut v = VBoxContainer::new_alloc();
    v.add_theme_constant_override("separation", 0);
    v.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
    v
}
