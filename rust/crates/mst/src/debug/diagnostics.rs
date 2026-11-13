//! `PixelDiagnostics` — in-game overlay: status line, live property tuner and an object
//! inspector with a viewport picker (port of the original `PixelizerDiagnosticsOverlay`).

use std::collections::HashMap;

use godot::builtin::Side;
use godot::classes::base_material_3d::{CullMode, Flags as MatFlags, ShadingMode, Transparency};
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::performance::Monitor;
use godot::classes::scroll_container::ScrollMode;
use godot::classes::{
    BoxMesh, Button, CanvasLayer, CheckBox, ClassDb, ColorPickerButton, Control, FoldableContainer, GeometryInstance3D, GridContainer,
    HBoxContainer, ICanvasLayer, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion, Label, MeshInstance3D,
    Node, Node3D, OptionButton, PanelContainer, Performance, ScrollContainer, SpinBox, StandardMaterial3D, StyleBoxFlat, TabContainer,
    VBoxContainer,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyUsageFlags};
use pt_core::meta::id_meta_from_flags;
use pt_core::pick::{ray_aabb, ray_faces};

use super::actions;
use crate::features::{self, Feature};
use crate::globals;
use crate::render::object::PixelObject;
use crate::util::*;

/// Tuner tabs.
const TABS: [&str; 3] = ["Effects", "Render", "Scene"];
const TAB_EFFECTS: usize = 0;
/// Classes that get a tuner section, in section order, with their tab (the renderer only for
/// the first one found). Every gated effect (`features::for_class`) must be listed here.
const TUNED_CLASSES: [(&str, usize); 16] = [
    ("PixelRenderer", 1),
    ("PixelWeather", 0),
    ("PixelDayNight", 0),
    ("PixelSky", 0),
    ("PixelWater", 0),
    ("PixelFireSystem", 0),
    ("PixelSmoke", 0),
    ("PixelBirds", 0),
    ("PixelFireflies", 0),
    ("PixelParticleField", 0),
    ("PixelLightVolume", 0),
    ("PixelOrbitCamera", 1),
    ("MarchingSquaresTerrain", 2),
    ("PixelObject", 2),
    ("PixelBuilderMesh", 2),
    ("MarchingSquaresWildFlowers", 2),
];
/// Resource classes that get a nested section when a tuned property holds one.
const NESTED_RESOURCES: [&str; 3] = ["PixelPalette", "PixelOutlinePalette", "PixelObjectProfile"];
/// Upper bound on sections per class, so scenes with many objects stay usable.
const MAX_PER_CLASS: usize = 8;
/// Nearest AABB candidates that get the exact triangle test per pick.
const MAX_TRIANGLE_CANDIDATES: usize = 24;
/// Nodes in this group (and their subtrees) are never picked.
pub const PICK_IGNORE_GROUP: &str = "pixel_diagnostics_ignore";
const SELECT_HINT: &str = "Press \"Select Object\", then click a mesh in the viewport.";
const SELECTING_HINT: &str = "Click a mesh to select it (hover highlights, Esc cancels).";

/// A property editor bound to an object, refreshed from the object a few times per second.
struct Binding {
    obj: Gd<Object>,
    prop: StringName,
    editor: Editor,
    /// Lives in the object inspector (rebuilt on selection change).
    object_panel: bool,
}

enum Editor {
    Check(Gd<CheckBox>),
    Spin(Gd<SpinBox>),
    Choice(Gd<OptionButton>),
    Color(Gd<ColorPickerButton>),
    /// Vector2 / Vector2i / Vector3 / Vector3i: one spin box per component.
    Vector(Vec<Gd<SpinBox>>),
}

/// A live read-out line of a section (`actions::live_text`).
struct Live {
    obj: Gd<Object>,
    class: &'static str,
    label: Gd<Label>,
}

/// Runtime diagnostics overlay (does nothing in the editor). Toggle it with `toggle_key`.
///
/// * Status line: FPS, draw calls, primitives and the `PixelRenderer` status.
/// * Tuner (right dock), in three tabs:
///   * *Effects*: the project's feature toggles (clouds, god rays, water, weather, birds, ...;
///     for this session), then every effect node in the scene: weather (one button per
///     weather: rain, snow, storm, ...; snow overlay, lightning), day-night (dawn / noon / dusk
///     / night), sky, water, fire (ignite at the view, extinguish), smoke (bursts), birds
///     (scare / land), fireflies, particle fields and light volumes, each with a live state
///     line;
///   * *Render*: the renderer (debug view, palettes) and the orbit camera;
///   * *Scene*: terrains, pixel objects, builder meshes and wild flowers.
///
///   Every exported bool / number / enum / color / vector property gets an editor, under its
///   inspector group; edits apply live.
/// * Object inspector (left dock): *Select Object* then click a mesh in the viewport (AABB
///   pre-pass + exact triangle test). Hovered meshes get the hover outline (id 254), the
///   selection the selection outline (id 255) and a translucent box. The panel shows the
///   resolved pixel settings, the owning `PixelObject` (its inherit chain), the selection's
///   `PixelObjectProfile` (create / remove) and the geometry's own properties.
#[derive(GodotClass)]
#[class(tool, base = CanvasLayer)]
pub struct PixelDiagnostics {
    /// Key that shows / hides the overlay.
    #[export]
    toggle_key: Key,
    /// Show the overlay when the game starts.
    #[export]
    start_visible: bool,
    /// Show the property tuner panel (otherwise only the status line).
    #[export]
    show_tuner: bool,
    /// Show the object inspector / picker panel.
    #[export]
    show_object_panel: bool,
    /// Width of both docks in pixels.
    #[export(range = (200.0, 800.0, 1.0))]
    panel_width: f32,
    /// Create tuner sections folded (the first one always starts open).
    #[export]
    sections_start_folded: bool,

    status: Option<Gd<Label>>,
    tuner: Option<Gd<Control>>,
    sections: Vec<Gd<VBoxContainer>>,
    feature_checks: Vec<(Feature, Gd<CheckBox>)>,
    live: Vec<Live>,
    debug_view: Option<Gd<OptionButton>>,
    object_panel: Option<Gd<Control>>,
    object_hint: Option<Gd<Label>>,
    object_info: Option<Gd<Label>>,
    object_sections: Option<Gd<VBoxContainer>>,
    select_button: Option<Gd<Button>>,
    renderer: Option<Gd<Node>>,
    bindings: Vec<Binding>,
    select_mode: bool,
    selected: Option<Gd<GeometryInstance3D>>,
    hovered: Option<Gd<GeometryInstance3D>>,
    marker: Option<Gd<MeshInstance3D>>,
    face_cache: HashMap<i64, Vec<[f32; 3]>>,
    hover_timer: f64,
    refresh_timer: f64,
    needs_scan: bool,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for PixelDiagnostics {
    fn init(base: Base<CanvasLayer>) -> Self {
        PixelDiagnostics {
            toggle_key: Key::F3,
            start_visible: false,
            show_tuner: true,
            show_object_panel: true,
            panel_width: 360.0,
            sections_start_folded: true,
            status: None,
            tuner: None,
            sections: Vec::new(),
            feature_checks: Vec::new(),
            live: Vec::new(),
            debug_view: None,
            object_panel: None,
            object_hint: None,
            object_info: None,
            object_sections: None,
            select_button: None,
            renderer: None,
            bindings: Vec::new(),
            select_mode: false,
            selected: None,
            hovered: None,
            marker: None,
            face_cache: HashMap::new(),
            hover_timer: 0.0,
            refresh_timer: 0.0,
            needs_scan: true,
            base,
        }
    }

    fn ready(&mut self) {
        if is_editor() {
            return;
        }
        self.base_mut().set_layer(100);
        self.build_ui();
        let visible = self.start_visible;
        self.base_mut().set_visible(visible);
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f64) {
        if is_editor() || !self.base().is_visible() {
            return;
        }
        // Scan after the rest of the scene has entered the tree.
        if self.needs_scan {
            self.needs_scan = false;
            self.rescan();
        }
        self.hover_timer -= delta;
        self.update_marker();
        self.refresh_timer -= delta;
        if self.refresh_timer <= 0.0 {
            self.refresh_timer = 0.25;
            self.update_status();
            self.pull_values();
            self.update_object_info();
        }
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        if is_editor() {
            return;
        }
        let Ok(k) = event.try_cast::<InputEventKey>() else { return };
        if !k.is_pressed() || k.is_echo() {
            return;
        }
        if k.get_keycode() == self.toggle_key || k.get_physical_keycode() == self.toggle_key {
            let show = !self.base().is_visible();
            self.set_overlay_visible(show);
            self.handled();
        } else if self.select_mode && k.get_keycode() == Key::ESCAPE {
            self.set_select_mode(false);
            self.handled();
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if is_editor() || !self.select_mode || !self.base().is_visible() {
            return;
        }
        if let Ok(m) = event.clone().try_cast::<InputEventMouseMotion>() {
            if self.hover_timer <= 0.0 {
                self.hover_timer = 0.05;
                let hit = self.pick_geometry(m.get_position());
                self.set_hovered(hit);
            }
        } else if let Ok(b) = event.try_cast::<InputEventMouseButton>()
            && b.is_pressed()
            && b.get_button_index() == MouseButton::LEFT
        {
            let hit = self.pick_geometry(b.get_position());
            self.set_hovered(None);
            self.select_geometry(hit);
            self.set_select_mode(false);
            self.handled();
        }
    }

    fn exit_tree(&mut self) {
        self.set_hovered(None);
        self.select_geometry(None);
    }
}

fn panel_style() -> Gd<StyleBoxFlat> {
    let mut s = StyleBoxFlat::new_gd();
    s.set_bg_color(Color::from_rgba(0.06, 0.07, 0.09, 0.82));
    s.set_content_margin_all(6.0);
    s.set_corner_radius_all(3);
    s
}

fn label(text: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
    l.add_theme_font_size_override("font_size", 12);
    l
}

fn button(text: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(text);
    b.add_theme_font_size_override("font_size", 12);
    b
}

/// Parses `min,max[,step]` from a PROPERTY_HINT_RANGE hint string.
fn parse_range(hint: &str) -> Option<(f64, f64, Option<f64>, bool, bool)> {
    let parts: Vec<&str> = hint.split(',').map(str::trim).collect();
    let min = parts.first()?.parse().ok()?;
    let max = parts.get(1)?.parse().ok()?;
    let step = parts.get(2).and_then(|s| s.parse().ok());
    Some((min, max, step, parts.contains(&"or_greater"), parts.contains(&"or_less")))
}

/// Parses `A,B:5,C` from a PROPERTY_HINT_ENUM hint string into (label, value) pairs.
fn parse_enum(hint: &str) -> Vec<(String, i64)> {
    let mut next = 0i64;
    hint.split(',')
        .map(|item| {
            let (name, value) = match item.rsplit_once(':') {
                Some((n, v)) => (n.trim(), v.trim().parse().unwrap_or(next)),
                None => (item.trim(), next),
            };
            next = value + 1;
            (name.to_string(), value)
        })
        .collect()
}

/// A callable that writes the first signal argument (mapped by `map`) to `obj.prop`.
fn setter(obj: &Gd<Object>, prop: &StringName, map: impl Fn(&Variant) -> Variant + 'static) -> Callable {
    let (mut obj, prop) = (obj.clone(), prop.clone());
    Callable::from_fn("pixel_diagnostics_set", move |args: &[&Variant]| {
        if let Some(v) = args.first()
            && obj.is_instance_valid()
        {
            obj.set(&prop, &map(v));
        }
    })
}

/// `rain_layer_spacing` -> "Rain Layer Spacing" (after stripping the group `prefix`).
fn pretty(name: &str, prefix: &str) -> String {
    let short = name.strip_prefix(prefix).filter(|s| !s.is_empty()).unwrap_or(name);
    short
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Labels of `PixelRenderer.debug_view` (read from its enum hint, so new views show up).
fn debug_views() -> Vec<(String, i64)> {
    let props = ClassDb::singleton().class_get_property_list_ex("PixelRenderer").no_inheritance(true).done();
    props
        .iter_shared()
        .find(|d| d.get("name").is_some_and(|v| v.to_string() == "debug_view"))
        .and_then(|d| d.get("hint_string"))
        .map(|h| parse_enum(&h.to_string()))
        .unwrap_or_else(|| vec![("Final".into(), 0)])
}

/// Components of a vector variant type (None for other types).
fn vector_dims(t: VariantType) -> Option<(usize, bool)> {
    match t {
        VariantType::VECTOR2 => Some((2, false)),
        VariantType::VECTOR2I => Some((2, true)),
        VariantType::VECTOR3 => Some((3, false)),
        VariantType::VECTOR3I => Some((3, true)),
        _ => None,
    }
}

fn vector_parts(v: &Variant) -> Vec<f64> {
    if let Ok(x) = v.try_to::<Vector3>() {
        vec![x.x as f64, x.y as f64, x.z as f64]
    } else if let Ok(x) = v.try_to::<Vector2>() {
        vec![x.x as f64, x.y as f64]
    } else if let Ok(x) = v.try_to::<Vector3i>() {
        vec![x.x as f64, x.y as f64, x.z as f64]
    } else if let Ok(x) = v.try_to::<Vector2i>() {
        vec![x.x as f64, x.y as f64]
    } else {
        Vec::new()
    }
}

fn vector_variant(t: VariantType, c: &[f64]) -> Variant {
    let g = |i: usize| c.get(i).copied().unwrap_or(0.0);
    match t {
        VariantType::VECTOR2 => Vector2::new(g(0) as f32, g(1) as f32).to_variant(),
        VariantType::VECTOR2I => Vector2i::new(g(0).round() as i32, g(1).round() as i32).to_variant(),
        VariantType::VECTOR3I => Vector3i::new(g(0).round() as i32, g(1).round() as i32, g(2).round() as i32).to_variant(),
        _ => Vector3::new(g(0) as f32, g(1) as f32, g(2) as f32).to_variant(),
    }
}

fn v3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// Nearest PixelObject ancestor (the one that owns the geometry's settings).
fn owner_object(n: &Gd<Node>) -> Option<Gd<PixelObject>> {
    let mut p = n.get_parent();
    while let Some(x) = p {
        if let Ok(po) = x.clone().try_cast::<PixelObject>() {
            return Some(po);
        }
        p = x.get_parent();
    }
    None
}

/// Current per-object flags of a geometry (the un-highlighted value).
fn geometry_flags(g: &Gd<GeometryInstance3D>) -> i32 {
    if g.has_meta("_pt_base_flags") {
        return g.get_meta("_pt_base_flags").try_to::<i32>().unwrap_or(32);
    }
    g.get_instance_shader_parameter("pt_flags").try_to::<i32>().unwrap_or(32)
}

impl PixelDiagnostics {
    fn handled(&self) {
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
    }

    fn dock(&self, right: bool) -> Gd<PanelContainer> {
        let mut p = PanelContainer::new_alloc();
        p.add_theme_stylebox_override("panel", &panel_style());
        let w = self.panel_width;
        if right {
            p.set_anchors_preset(LayoutPreset::RIGHT_WIDE);
            p.set_offset(Side::LEFT, -w - 8.0);
            p.set_offset(Side::RIGHT, -8.0);
            // Wider content grows leftwards instead of off-screen.
            p.set_h_grow_direction(godot::classes::control::GrowDirection::BEGIN);
        } else {
            p.set_anchors_preset(LayoutPreset::LEFT_WIDE);
            p.set_offset(Side::LEFT, 8.0);
            p.set_offset(Side::RIGHT, w + 8.0);
        }
        p.set_offset(Side::TOP, 72.0);
        p.set_offset(Side::BOTTOM, -8.0);
        p
    }

    fn scroll_with(&self, inner: &Gd<VBoxContainer>) -> Gd<ScrollContainer> {
        let mut scroll = ScrollContainer::new_alloc();
        scroll.set_v_size_flags(SizeFlags::EXPAND_FILL);
        scroll.set_horizontal_scroll_mode(ScrollMode::DISABLED);
        let mut inner = inner.clone();
        inner.set_h_size_flags(SizeFlags::EXPAND_FILL);
        scroll.add_child(&inner);
        scroll
    }

    fn build_ui(&mut self) {
        let mut root = Control::new_alloc();
        root.set_name("Overlay");
        root.set_anchors_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);
        let this = self.to_gd();

        // Status line (top-left).
        let mut status_panel = PanelContainer::new_alloc();
        status_panel.add_theme_stylebox_override("panel", &panel_style());
        status_panel.set_position(Vector2::new(8.0, 8.0));
        status_panel.set_mouse_filter(MouseFilter::IGNORE);
        let status = label("...");
        status_panel.add_child(&status);
        root.add_child(&status_panel);

        // Tuner (right dock).
        let mut tuner = self.dock(true);
        let mut col = VBoxContainer::new_alloc();
        let mut header = HBoxContainer::new_alloc();
        let mut title = label("Pixel tuner");
        title.set_h_size_flags(SizeFlags::EXPAND_FILL);
        header.add_child(&title);
        let mut rescan = button("Rescan");
        rescan.connect("pressed", &Callable::from_object_method(&this, "rescan"));
        header.add_child(&rescan);
        col.add_child(&header);

        let mut dv_row = HBoxContainer::new_alloc();
        let mut dv_label = label("Debug view");
        dv_label.set_h_size_flags(SizeFlags::EXPAND_FILL);
        dv_row.add_child(&dv_label);
        let mut dv = OptionButton::new_alloc();
        for (name, id) in debug_views() {
            dv.add_item_ex(name.as_str()).id(id as i32).done();
        }
        dv.connect("item_selected", &Callable::from_object_method(&this, "on_debug_view_selected"));
        dv_row.add_child(&dv);
        col.add_child(&dv_row);

        let mut tabs = TabContainer::new_alloc();
        tabs.set_v_size_flags(SizeFlags::EXPAND_FILL);
        tabs.add_theme_font_size_override("font_size", 12);
        let mut sections = Vec::new();
        for name in TABS {
            let inner = VBoxContainer::new_alloc();
            let mut scroll = self.scroll_with(&inner);
            scroll.set_name(name);
            tabs.add_child(&scroll);
            sections.push(inner);
        }
        col.add_child(&tabs);
        tuner.add_child(&col);
        tuner.set_visible(self.show_tuner);
        root.add_child(&tuner);

        // Object inspector (left dock).
        let mut obj_panel = self.dock(false);
        let mut ocol = VBoxContainer::new_alloc();
        let mut oheader = HBoxContainer::new_alloc();
        let mut otitle = label("Object inspector");
        otitle.set_h_size_flags(SizeFlags::EXPAND_FILL);
        oheader.add_child(&otitle);
        let mut select = button("Select Object");
        select.set_toggle_mode(true);
        select.connect("toggled", &Callable::from_object_method(&this, "set_select_mode"));
        oheader.add_child(&select);
        let mut clear = button("Clear");
        clear.connect("pressed", &Callable::from_object_method(&this, "clear_selection"));
        oheader.add_child(&clear);
        ocol.add_child(&oheader);
        let mut hint = label(SELECT_HINT);
        hint.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7));
        ocol.add_child(&hint);
        let mut info = label("");
        info.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        ocol.add_child(&info);
        let osections = VBoxContainer::new_alloc();
        ocol.add_child(&self.scroll_with(&osections));
        obj_panel.add_child(&ocol);
        obj_panel.set_visible(self.show_object_panel);
        root.add_child(&obj_panel);

        let mut base = self.to_gd();
        add_internal(&mut base, &root);
        self.status = Some(status);
        self.tuner = Some(tuner.upcast());
        self.sections = sections;
        self.debug_view = Some(dv);
        self.object_panel = Some(obj_panel.upcast());
        self.object_hint = Some(hint);
        self.object_info = Some(info);
        self.object_sections = Some(osections);
        self.select_button = Some(select);
    }

    fn scene_root(&self) -> Option<Gd<Node>> {
        let tree = self.base().get_tree();
        tree.get_current_scene().or_else(|| Some(tree.get_root().upcast()))
    }

    /// Finds the tuned nodes in tree order, grouped by class.
    fn find_targets(&self) -> Vec<(usize, Gd<Node>)> {
        let mut found: Vec<(usize, Gd<Node>)> = Vec::new();
        let mut counts = [0usize; TUNED_CLASSES.len()];
        let Some(root) = self.scene_root() else { return found };
        let me = self.to_gd().upcast::<Node>();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n == me {
                continue;
            }
            let class = n.get_class().to_string();
            if let Some(i) = TUNED_CLASSES.iter().position(|(c, _)| *c == class) {
                let limit = if i == 0 { 1 } else { MAX_PER_CLASS };
                if counts[i] < limit {
                    counts[i] += 1;
                    found.push((i, n.clone()));
                }
            }
            // Reverse so the stack pops children in tree order.
            let children: Vec<Gd<Node>> = n.get_children().iter_shared().collect();
            stack.extend(children.into_iter().rev());
        }
        found.sort_by_key(|(i, _)| *i);
        found
    }

    fn update_status(&mut self) {
        let perf = Performance::singleton();
        let fps = perf.get_monitor(Monitor::TIME_FPS);
        let draws = perf.get_monitor(Monitor::RENDER_TOTAL_DRAW_CALLS_IN_FRAME);
        let prims = perf.get_monitor(Monitor::RENDER_TOTAL_PRIMITIVES_IN_FRAME);
        let mut text = format!("{fps:.0} fps | {draws:.0} draws | {:.1}k prims | {} hides", prims / 1000.0, self.toggle_key.as_str());
        match self.renderer.as_mut().filter(|r| r.is_instance_valid()) {
            Some(r) => {
                let line = r.call("get_status_line", &[]);
                text.push_str(&format!("\n{}", line.try_to::<GString>().unwrap_or_default()));
                let view = r.get("debug_view").try_to::<i32>().unwrap_or(0);
                if let Some(dv) = &mut self.debug_view {
                    let idx = dv.get_item_index(view);
                    if idx >= 0 {
                        dv.select(idx);
                    }
                }
            }
            None => text.push_str("\nno PixelRenderer"),
        }
        let effects = self.effects_summary();
        if !effects.is_empty() {
            text.push_str(&format!("\n{effects}"));
        }
        // Docks start below the status panel (its line count varies with the scene).
        let mut top = 72.0;
        if let Some(s) = &mut self.status {
            s.set_text(&text);
            if let Some(panel) = s.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                top = panel.get_position().y + panel.get_combined_minimum_size().y + 6.0;
            }
        }
        let (show_tuner, show_obj) = (self.show_tuner, self.show_object_panel);
        let has_selection = self.selected.as_ref().is_some_and(|s| s.is_instance_valid());
        if let Some(t) = &mut self.tuner {
            t.set_visible(show_tuner);
            t.set_offset(Side::TOP, top);
        }
        if let Some(t) = &mut self.object_panel {
            t.set_visible(show_obj);
            t.set_offset(Side::TOP, top);
            // Without a selection the inspector shrinks to its header instead of covering
            // the left of the screen.
            if has_selection {
                t.set_anchor(Side::BOTTOM, 1.0);
                t.set_offset(Side::BOTTOM, -8.0);
            } else {
                t.set_anchor(Side::BOTTOM, 0.0);
                t.set_offset(Side::BOTTOM, top);
            }
        }
    }

    /// Copies current property values into the editors that are not being edited.
    fn pull_values(&mut self) {
        self.bindings.retain(|b| b.obj.is_instance_valid());
        for b in &mut self.bindings {
            let v = b.obj.get(&b.prop);
            match &mut b.editor {
                Editor::Check(c) => c.set_pressed_no_signal(v.try_to::<bool>().unwrap_or(false)),
                Editor::Spin(s) => {
                    let editing = s.get_line_edit().is_some_and(|l| l.has_focus());
                    if !editing {
                        let f = v.try_to::<f64>().or_else(|_| v.try_to::<i64>().map(|i| i as f64)).unwrap_or(0.0);
                        s.set_value_no_signal(f);
                    }
                }
                Editor::Choice(o) => {
                    let idx = o.get_item_index(v.try_to::<i32>().unwrap_or(0));
                    if idx >= 0 {
                        o.select(idx);
                    }
                }
                Editor::Color(c) => {
                    if !c.is_pressed() {
                        c.set_pick_color(v.try_to::<Color>().unwrap_or(Color::BLACK));
                    }
                }
                Editor::Vector(spins) => {
                    let editing = spins.iter().any(|s| s.get_line_edit().is_some_and(|l| l.has_focus()));
                    if !editing {
                        for (s, c) in spins.iter_mut().zip(vector_parts(&v)) {
                            s.set_value_no_signal(c);
                        }
                    }
                }
            }
        }
        for (f, c) in &mut self.feature_checks {
            c.set_pressed_no_signal(features::enabled(*f));
        }
        self.live.retain(|l| l.obj.is_instance_valid() && l.label.is_instance_valid());
        for l in &mut self.live {
            let text = actions::live_text(l.class, &mut l.obj).unwrap_or_default();
            l.label.set_text(&text);
            l.label.set_visible(!text.is_empty());
        }
    }

    /// Weather, time of day and feature count for the status line.
    fn effects_summary(&mut self) -> String {
        let mut parts = Vec::new();
        for l in &mut self.live {
            if !l.obj.is_instance_valid() {
                continue;
            }
            match l.class {
                "PixelWeather" => {
                    let o = &mut l.obj;
                    let wet = o.call("get_wetness", &[]).try_to::<f32>().unwrap_or(0.0);
                    let snow = o.call("get_snow_cover", &[]).try_to::<f32>().unwrap_or(0.0);
                    parts.push(format!("weather {} (wet {wet:.2}, snow {snow:.2})", o.call("get_weather_name", &[])));
                }
                "PixelDayNight" => {
                    let o = &l.obj;
                    let ov = o.get("time_override").try_to::<f32>().unwrap_or(-1.0);
                    let t = if ov >= 0.0 { ov } else { o.get("time_of_day").try_to::<f32>().unwrap_or(0.0) };
                    parts.push(format!("time {}", actions::clock_text(t)));
                }
                _ => {}
            }
        }
        let on = features::ALL.iter().filter(|i| features::enabled(i.feature)).count();
        parts.push(format!("features {on}/{}", features::ALL.len()));
        parts.join(" | ")
    }

    /// The *Features* section: one switch per project feature (for this session).
    fn add_features_section(&mut self, mut container: Gd<VBoxContainer>) {
        self.feature_checks.clear();
        let mut fold = FoldableContainer::new_alloc();
        fold.set_title("Features (this session)");
        let mut col = VBoxContainer::new_alloc();
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);
        for info in features::ALL {
            let mut c = CheckBox::new_alloc();
            c.set_text(info.label);
            c.set_tooltip_text(info.hint);
            c.add_theme_font_size_override("font_size", 12);
            c.set_h_size_flags(SizeFlags::EXPAND_FILL);
            let feat = info.feature;
            c.connect(
                "toggled",
                &Callable::from_fn("pixel_diagnostics_feature", move |args: &[&Variant]| {
                    let on = args.first().and_then(|v| v.try_to::<bool>().ok()).unwrap_or(false);
                    features::set_enabled(feat, on);
                }),
            );
            grid.add_child(&c);
            self.feature_checks.push((feat, c));
        }
        col.add_child(&grid);
        let mut all = HBoxContainer::new_alloc();
        for (text, on) in [("All on", true), ("Defaults", false)] {
            let mut b = button(text);
            b.connect(
                "pressed",
                &Callable::from_fn("pixel_diagnostics_features_all", move |_: &[&Variant]| {
                    for i in features::ALL {
                        features::set_enabled(i.feature, on || i.default);
                    }
                }),
            );
            all.add_child(&b);
        }
        col.add_child(&all);
        fold.add_child(&col);
        container.add_child(&fold);
    }

    /// Builds one foldable section with an editor per supported exported property of `class`.
    fn add_section(&mut self, mut container: Gd<VBoxContainer>, class: &str, obj: Gd<Object>, title: &str, folded: bool, object_panel: bool) {
        let mut fold = FoldableContainer::new_alloc();
        fold.set_title(title);
        fold.set_folded(folded);
        let mut col = VBoxContainer::new_alloc().upcast::<Control>();
        if !object_panel {
            // Live state line + quick actions (effects).
            if let Some(cls) = TUNED_CLASSES.iter().map(|(c, _)| *c).find(|c| *c == class)
                && actions::live_text(cls, &mut obj.clone()).is_some()
            {
                let label = actions::live_label(&mut col);
                self.live.push(Live { obj: obj.clone(), class: cls, label });
            }
            actions::add_actions(&mut col, class, &obj);
        }
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);
        let mut group = (String::new(), String::new());
        let mut subgroup = (String::new(), String::new());
        let mut shown_header = String::new();
        let mut nested: Vec<(String, Gd<Object>)> = Vec::new();

        let props = ClassDb::singleton().class_get_property_list_ex(class).no_inheritance(true).done();
        let editor_flag = PropertyUsageFlags::EDITOR.ord() as i64;
        let (group_flag, subgroup_flag) = (PropertyUsageFlags::GROUP.ord() as i64, PropertyUsageFlags::SUBGROUP.ord() as i64);
        let skip = PropertyUsageFlags::CATEGORY.ord() as i64;
        for d in props.iter_shared() {
            let get_i = |k: &str| d.get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
            let usage = get_i("usage");
            let name = d.get("name").map(|v| v.to_string()).unwrap_or_default();
            if usage & (group_flag | subgroup_flag) != 0 {
                let prefix = d.get("hint_string").map(|v| v.to_string()).unwrap_or_default();
                if usage & group_flag != 0 {
                    group = (name, prefix);
                    subgroup = (String::new(), String::new());
                } else {
                    subgroup = (name, prefix);
                }
                continue;
            }
            if usage & editor_flag == 0 || usage & skip != 0 {
                continue;
            }
            let hint = get_i("hint");
            let hint_string = d.get("hint_string").map(|v| v.to_string()).unwrap_or_default();
            let ty = get_i("type");
            let prop = StringName::from(name.as_str());
            let editor = match VariantType::from_ord(ty as i32) {
                VariantType::BOOL => {
                    let mut c = CheckBox::new_alloc();
                    c.connect("toggled", &setter(&obj, &prop, |v| v.clone()));
                    Some(Editor::Check(c))
                }
                VariantType::INT if hint == PropertyHint::ENUM.ord() as i64 => {
                    let mut o = OptionButton::new_alloc();
                    let items = parse_enum(&hint_string);
                    for (label, value) in &items {
                        o.add_item_ex(label.as_str()).id(*value as i32).done();
                    }
                    let ids: Vec<i64> = items.iter().map(|(_, v)| *v).collect();
                    o.connect(
                        "item_selected",
                        &setter(&obj, &prop, move |v| {
                            let idx = v.try_to::<i64>().unwrap_or(0) as usize;
                            ids.get(idx).copied().unwrap_or(0).to_variant()
                        }),
                    );
                    Some(Editor::Choice(o))
                }
                t @ (VariantType::INT | VariantType::FLOAT) => {
                    let int = t == VariantType::INT;
                    let mut s = SpinBox::new_alloc();
                    match (hint == PropertyHint::RANGE.ord() as i64).then(|| parse_range(&hint_string)).flatten() {
                        Some((min, max, step, greater, lesser)) => {
                            s.set_min(min);
                            s.set_max(max);
                            s.set_step(step.unwrap_or(if int { 1.0 } else { 0.001 }));
                            s.set_allow_greater(greater);
                            s.set_allow_lesser(lesser);
                        }
                        None => {
                            s.set_min(-1e6);
                            s.set_max(1e6);
                            s.set_step(if int { 1.0 } else { 0.001 });
                        }
                    }
                    s.set_custom_minimum_size(Vector2::new(120.0, 0.0));
                    s.connect(
                        "value_changed",
                        &setter(&obj, &prop, move |v| {
                            let f = v.try_to::<f64>().or_else(|_| v.try_to::<i64>().map(|i| i as f64)).unwrap_or(0.0);
                            if int { (f.round() as i64).to_variant() } else { f.to_variant() }
                        }),
                    );
                    Some(Editor::Spin(s))
                }
                VariantType::COLOR => {
                    let mut c = ColorPickerButton::new_alloc();
                    c.set_custom_minimum_size(Vector2::new(120.0, 20.0));
                    c.connect("color_changed", &setter(&obj, &prop, |v| v.clone()));
                    Some(Editor::Color(c))
                }
                t if vector_dims(t).is_some() => {
                    let (n, int) = vector_dims(t).unwrap_or((3, false));
                    let spins: Vec<Gd<SpinBox>> = (0..n)
                        .map(|_| {
                            let mut s = SpinBox::new_alloc();
                            s.set_min(-1e6);
                            s.set_max(1e6);
                            s.set_step(if int { 1.0 } else { 0.01 });
                            s.set_custom_minimum_size(Vector2::new(if n == 2 { 60.0 } else { 40.0 }, 0.0));
                            s.set_h_size_flags(SizeFlags::EXPAND_FILL);
                            s
                        })
                        .collect();
                    for s in &spins {
                        let all = spins.clone();
                        let mut s = s.clone();
                        s.connect(
                            "value_changed",
                            &setter(&obj, &prop, move |_| {
                                let c: Vec<f64> = all.iter().map(|x| x.get_value()).collect();
                                vector_variant(t, &c)
                            }),
                        );
                    }
                    Some(Editor::Vector(spins))
                }
                VariantType::OBJECT => {
                    // Nested resource sections (palettes, profiles) after this one.
                    if let Ok(res) = obj.get(&prop).try_to::<Gd<Object>>()
                        && NESTED_RESOURCES.iter().any(|c| res.is_class(*c))
                    {
                        nested.push((name.clone(), res));
                    }
                    None
                }
                _ => None,
            };
            let Some(editor) = editor else { continue };
            // Group / subgroup header before its first shown property.
            let header = if subgroup.0.is_empty() { group.0.clone() } else { format!("{} / {}", group.0, subgroup.0) };
            if header != shown_header {
                if !header.is_empty() {
                    let mut h = label(&header);
                    h.set_modulate(Color::from_rgba(1.0, 0.85, 0.5, 1.0));
                    grid.add_child(&h);
                    grid.add_child(&Control::new_alloc());
                }
                shown_header = header;
            }
            let prefix = if !subgroup.1.is_empty() && name.starts_with(&subgroup.1) { &subgroup.1 } else { &group.1 };
            let mut l = label(&pretty(&name, prefix));
            l.set_tooltip_text(&name);
            l.set_mouse_filter(MouseFilter::PASS);
            l.set_h_size_flags(SizeFlags::EXPAND_FILL);
            grid.add_child(&l);
            let ctrl: Gd<Control> = match &editor {
                Editor::Check(c) => c.clone().upcast(),
                Editor::Spin(s) => s.clone().upcast(),
                Editor::Choice(o) => o.clone().upcast(),
                Editor::Color(c) => c.clone().upcast(),
                Editor::Vector(spins) => {
                    let mut row = HBoxContainer::new_alloc();
                    row.add_theme_constant_override("separation", 2);
                    for s in spins {
                        row.add_child(s);
                    }
                    row.upcast()
                }
            };
            grid.add_child(&ctrl);
            self.bindings.push(Binding { obj: obj.clone(), prop, editor, object_panel });
        }
        col.add_child(&grid);
        fold.add_child(&col);
        container.add_child(&fold);
        for (prop, res) in nested {
            let cls = res.get_class().to_string();
            let t = format!("{title} > {} ({cls})", pretty(&prop, ""));
            self.add_section(container.clone(), &cls, res, &t, true, object_panel);
        }
    }

    // ---- picking ---------------------------------------------------------------------------

    /// Visible, pickable geometry in the scene (internal children such as terrain chunks and
    /// this overlay are skipped).
    fn pickable(&self) -> Vec<Gd<GeometryInstance3D>> {
        let mut out = Vec::new();
        let Some(root) = self.scene_root() else { return out };
        let me = self.to_gd().upcast::<Node>();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n == me || n.is_in_group(PICK_IGNORE_GROUP) {
                continue;
            }
            if let Ok(g) = n.clone().try_cast::<GeometryInstance3D>()
                && g.is_visible_in_tree()
            {
                out.push(g);
            }
            stack.extend(n.get_children().iter_shared());
        }
        out
    }

    fn faces(&mut self, mi: &Gd<MeshInstance3D>) -> Option<Vec<[f32; 3]>> {
        let mesh = mi.get_mesh()?;
        let key = mesh.instance_id().to_i64();
        if !self.face_cache.contains_key(&key) {
            let f: Vec<[f32; 3]> = mesh.get_faces().as_slice().iter().map(|v| v3(*v)).collect();
            self.face_cache.insert(key, f);
        }
        self.face_cache.get(&key).cloned()
    }

    /// Nearest visible geometry under a viewport position.
    fn pick_geometry(&mut self, screen: Vector2) -> Option<Gd<GeometryInstance3D>> {
        let cam = self.base().get_viewport()?.get_camera_3d()?;
        let origin = cam.project_ray_origin(screen);
        let dir = cam.project_ray_normal(screen);
        let mut cands: Vec<(f32, Gd<GeometryInstance3D>, Transform3D)> = Vec::new();
        for g in self.pickable() {
            let xf = g.get_global_transform();
            let inv = xf.affine_inverse();
            let (lo, ld) = (inv * origin, inv.basis * dir);
            let aabb = g.get_aabb();
            if aabb.size.length_squared() <= 0.0 {
                continue;
            }
            if let Some(t) = ray_aabb(v3(lo), v3(ld), v3(aabb.position), v3(aabb.position + aabb.size)) {
                cands.push((t, g, inv));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut best: Option<(f32, Gd<GeometryInstance3D>)> = None;
        for (t_box, g, inv) in cands.into_iter().take(MAX_TRIANGLE_CANDIDATES) {
            if best.as_ref().is_some_and(|(bt, _)| t_box > *bt) {
                break;
            }
            let t = match g.clone().try_cast::<MeshInstance3D>() {
                Ok(mi) => match self.faces(&mi) {
                    Some(f) if !f.is_empty() => ray_faces(v3(inv * origin), v3(inv.basis * dir), &f),
                    _ => Some(t_box),
                },
                // MultiMesh / particles: no single face list, use the box.
                Err(_) => Some(t_box),
            };
            if let Some(t) = t
                && best.as_ref().is_none_or(|(bt, _)| t < *bt)
            {
                best = Some((t, g));
            }
        }
        best.map(|(_, g)| g)
    }

    fn set_hovered(&mut self, g: Option<Gd<GeometryInstance3D>>) {
        let g = g.filter(|g| Some(g) != self.selected.as_ref());
        if g == self.hovered {
            return;
        }
        if let Some(old) = self.hovered.take()
            && old.is_instance_valid()
            && Some(&old) != self.selected.as_ref()
        {
            crate::render::object::highlight(&old.upcast(), 0);
        }
        if let Some(n) = &g {
            crate::render::object::highlight(&n.clone().upcast(), 1);
        }
        self.hovered = g;
    }

    fn select_geometry(&mut self, g: Option<Gd<GeometryInstance3D>>) {
        if let Some(old) = self.selected.take()
            && old.is_instance_valid()
        {
            crate::render::object::highlight(&old.upcast(), 0);
        }
        if let Some(n) = &g {
            crate::render::object::highlight(&n.clone().upcast(), 2);
        }
        self.selected = g;
        // Re-lay out the inspector dock on the next frame.
        self.refresh_timer = 0.0;
        self.rebuild_object_panel();
    }

    fn ensure_marker(&mut self) -> Gd<MeshInstance3D> {
        if let Some(m) = &self.marker
            && m.is_instance_valid()
        {
            return m.clone();
        }
        let mut m = MeshInstance3D::new_alloc();
        m.set_name("SelectionMarker");
        m.set_mesh(&BoxMesh::new_gd());
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_albedo(Color::from_rgba(1.0, 0.9, 0.2, 0.22));
        mat.set_cull_mode(CullMode::DISABLED);
        mat.set_flag(MatFlags::DISABLE_DEPTH_TEST, true);
        m.set_material_override(&mat);
        m.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
        // Never on the metadata layer: it is a plain color-pass overlay.
        m.set_layer_mask(1 & !globals::meta_mask());
        m.add_to_group(PICK_IGNORE_GROUP);
        m.set_visible(false);
        let mut me = self.to_gd();
        add_internal(&mut me, &m);
        self.marker = Some(m.clone());
        m
    }

    fn update_marker(&mut self) {
        let sel = self.selected.clone().filter(|s| s.is_instance_valid() && s.is_inside_tree());
        if sel.is_none() && self.marker.is_none() {
            return;
        }
        let mut m = self.ensure_marker();
        match sel {
            Some(s) => {
                let aabb = s.get_aabb();
                let xf = s.get_global_transform();
                let centre = aabb.position + aabb.size * 0.5;
                let size = Vector3::new(aabb.size.x.max(0.05), aabb.size.y.max(0.05), aabb.size.z.max(0.05)) * 1.04;
                let basis = xf.basis * Basis::from_scale(size);
                m.set_global_transform(Transform3D::new(basis, xf * centre));
                m.set_visible(true);
            }
            None => m.set_visible(false),
        }
    }

    // ---- object inspector --------------------------------------------------------------------

    fn rebuild_object_panel(&mut self) {
        self.bindings.retain(|b| !b.object_panel);
        let Some(sections) = self.object_sections.clone() else { return };
        for mut c in sections.get_children().iter_shared() {
            c.queue_free();
        }
        let Some(sel) = self.selected.clone().filter(|s| s.is_instance_valid()) else {
            self.update_object_info();
            return;
        };
        let this = self.to_gd();
        let node = sel.clone().upcast::<Node>();
        if let Some(owner) = owner_object(&node) {
            let mut row = HBoxContainer::new_alloc();
            // Release the bind before adding controls (node_added reaches every PixelObject).
            let profile = owner.bind().get_profile_for(node.clone());
            match profile {
                Some(profile) => {
                    let mut rm = button("Remove profile");
                    rm.connect("pressed", &Callable::from_object_method(&this, "remove_selected_profile"));
                    row.add_child(&rm);
                    let mut s = sections.clone();
                    s.add_child(&row);
                    self.add_section(sections.clone(), "PixelObjectProfile", profile.upcast(), "Profile (this mesh)", false, true);
                }
                None => {
                    let mut mk = button("Create profile");
                    mk.set_tooltip_text("Add a PixelObjectProfile targeting this mesh to its PixelObject (all values inherit).");
                    mk.connect("pressed", &Callable::from_object_method(&this, "create_selected_profile"));
                    row.add_child(&mk);
                    let mut s = sections.clone();
                    s.add_child(&row);
                }
            }
            let title = format!("Owning PixelObject: {}", owner.get_name());
            self.add_section(sections.clone(), "PixelObject", owner.clone().upcast(), &title, false, true);
            // Ancestor PixelObjects (where inherited values come from).
            let mut p = owner.get_parent();
            while let Some(n) = p {
                if let Ok(po) = n.clone().try_cast::<PixelObject>() {
                    let t = format!("Parent PixelObject: {}", po.get_name());
                    self.add_section(sections.clone(), "PixelObject", po.upcast(), &t, true, true);
                }
                p = n.get_parent();
            }
        }
        let cls = sel.get_class().to_string();
        self.add_section(sections.clone(), "GeometryInstance3D", sel.clone().upcast(), &format!("Geometry: {} ({cls})", sel.get_name()), true, true);
        self.pull_values();
        self.update_object_info();
    }

    fn update_object_info(&mut self) {
        let text = match self.selected.clone().filter(|s| s.is_instance_valid()) {
            None => String::new(),
            Some(s) => {
                let node = s.clone().upcast::<Node>();
                let flags = geometry_flags(&s);
                let id = id_meta_from_flags(flags);
                let on_meta = s.get_layer_mask() & globals::meta_mask() != 0;
                let mut t = format!(
                    "{} ({})\n{}\npixelized: {} | pixel size {} | outline {} | id {} | palette {} | dither {}",
                    s.get_name(),
                    s.get_class(),
                    node.get_path(),
                    if on_meta { "yes" } else { "no" },
                    (flags & 3) + 1,
                    if flags & 32 != 0 { "on" } else { "off" },
                    id.outline_id,
                    if id.palette { "on" } else { "off" },
                    if id.dither { "on" } else { "off" },
                );
                match owner_object(&node) {
                    Some(o) => t.push_str(&format!("\nowner {} (profiles {})", o.get_name(), o.get("profiles").try_to::<VarArray>().map(|a| a.len()).unwrap_or(0))),
                    None => t.push_str("\nnot under a PixelObject"),
                }
                t
            }
        };
        if let Some(l) = &mut self.object_info {
            l.set_text(&text);
        }
    }
}

#[godot_api]
impl PixelDiagnostics {
    /// Rebuilds the tuner from the nodes currently in the scene.
    #[func]
    fn rescan(&mut self) {
        if is_editor() {
            return;
        }
        self.bindings.retain(|b| b.object_panel);
        self.live.clear();
        for sections in &self.sections {
            for mut c in sections.get_children().iter_shared() {
                c.queue_free();
            }
        }
        let targets = self.find_targets();
        self.renderer = targets.iter().find(|(i, _)| *i == 0).map(|(_, n)| n.clone());
        let folded = self.sections_start_folded;
        if self.sections.len() < TABS.len() {
            return;
        }
        self.add_features_section(self.sections[TAB_EFFECTS].clone());
        // Keep the panel compact: on the Effects tab the first effect starts open below the
        // features, elsewhere the first section of the tab.
        let mut opened = [false; TABS.len()];
        for (i, node) in &targets {
            let (class, tab) = TUNED_CLASSES[*i];
            let title = format!("{class}: {}", node.get_name());
            let open = !opened[tab] || !folded;
            opened[tab] = true;
            self.add_section(self.sections[tab].clone(), class, node.clone().upcast(), &title, !open, false);
        }
        self.pull_values();
        self.update_status();
    }

    #[func]
    fn on_debug_view_selected(&mut self, index: i64) {
        let id = self.debug_view.as_ref().map(|d| d.get_item_id(index as i32)).unwrap_or(index as i32);
        if let Some(r) = self.renderer.as_mut().filter(|r| r.is_instance_valid()) {
            r.set("debug_view", &id.to_variant());
        }
    }

    /// Shows or hides the overlay.
    #[func]
    fn set_overlay_visible(&mut self, visible: bool) {
        self.base_mut().set_visible(visible);
        if visible {
            self.needs_scan = true;
            self.refresh_timer = 0.0;
        } else {
            self.set_select_mode(false);
        }
        if let Some(m) = &mut self.marker {
            m.set_visible(visible && self.selected.is_some());
        }
    }

    /// Toggles the overlay.
    #[func]
    fn toggle(&mut self) {
        let v = !self.base().is_visible();
        self.set_overlay_visible(v);
    }

    /// Enters / leaves picking mode (hover highlights, next click selects).
    #[func]
    fn set_select_mode(&mut self, on: bool) {
        self.select_mode = on;
        if !on {
            self.set_hovered(None);
        }
        if let Some(b) = &mut self.select_button {
            b.set_pressed_no_signal(on);
        }
        if let Some(h) = &mut self.object_hint {
            h.set_text(if on { SELECTING_HINT } else { SELECT_HINT });
        }
    }

    /// Selects a geometry node programmatically (null clears). Non-geometry nodes select their
    /// first geometry descendant.
    #[func]
    fn select_node(&mut self, node: Option<Gd<Node>>) {
        let g = node.and_then(|n| {
            if let Ok(g) = n.clone().try_cast::<GeometryInstance3D>() {
                return Some(g);
            }
            n.find_children_ex("*").type_("GeometryInstance3D").owned(false).done().iter_shared().next().and_then(|c| c.try_cast().ok())
        });
        self.select_geometry(g);
    }

    /// Hover-highlights a node as the picker does (null clears).
    #[func]
    fn hover_node(&mut self, node: Option<Gd<Node>>) {
        let g = node.and_then(|n| n.try_cast::<GeometryInstance3D>().ok());
        self.set_hovered(g);
    }

    #[func]
    fn get_selected_node(&self) -> Option<Gd<Node3D>> {
        self.selected.clone().filter(|s| s.is_instance_valid()).map(|s| s.upcast())
    }

    /// Picks the geometry under a viewport position (no selection change).
    #[func]
    fn pick_at(&mut self, screen_position: Vector2) -> Option<Gd<Node3D>> {
        self.pick_geometry(screen_position).map(|g| g.upcast())
    }

    #[func]
    fn clear_selection(&mut self) {
        self.select_geometry(None);
    }

    #[func]
    fn create_selected_profile(&mut self) {
        let Some(sel) = self.selected.clone() else { return };
        let node = sel.upcast::<Node>();
        if let Some(mut owner) = owner_object(&node) {
            owner.bind_mut().ensure_profile_for(node);
        }
        self.to_gd().call_deferred("refresh_object_panel", &[]);
    }

    #[func]
    fn remove_selected_profile(&mut self) {
        let Some(sel) = self.selected.clone() else { return };
        let node = sel.upcast::<Node>();
        if let Some(mut owner) = owner_object(&node) {
            owner.bind_mut().remove_profile_for(node);
        }
        self.to_gd().call_deferred("refresh_object_panel", &[]);
    }

    #[func]
    fn refresh_object_panel(&mut self) {
        self.rebuild_object_panel();
    }
}
