//! An in-game overlay for inspecting and tuning the pixel look while the game runs.
//!
//! Intent: a developer adds this to a scene and, by pressing a key while playing, gets a
//! live readout of the renderer and every pixel effect, can change their settings on the
//! spot, switch the renderer's debug views, and click a mesh to see how it is pixelized and
//! give it its own settings. It is a development aid and stays out of the way until asked.
//!
//! Responsibility: it finds the pixel nodes in the running scene, shows their settings and
//! writes changes straight back to them, and picks meshes under the mouse. It owns nothing
//! it edits: every value it shows lives on the node it came from, and nothing it changes is
//! saved. It does nothing in the editor.
//!
//! Why a node: it has to be drawn above the game, receive key and mouse input, and see the
//! running scene, which needs a place in the tree for the length of the session.
//!
//! Why a new type: no built-in node presents and edits other nodes' properties in a running
//! game; building that overlay by hand in each scene would repeat it everywhere.
//!
//! Nodes created in code: its fixed frame is instantiated from a scene when the game starts,
//! because it must never exist in the editor or be saved with the scenes that use it. That
//! frame scene is a part the author may swap for their own; left empty, the built-in one is
//! used. The rows inside are built from the properties of whichever pixel nodes the running
//! scene happens to contain, so their number and kind are only known at run time.
//!
//! Not exported: the parts of its frame and the rows it built, which exist only while the
//! game runs; the nodes it found, the one picked and the one under the mouse; and the
//! countdowns and geometry it caches between picks and refreshes.
use super::actions;
use super::overlay::OverlayParts;
use crate::editor_detection::is_editor;
use crate::features::{self, Feature};
use crate::globals;
use crate::node_children::add_internal;
use crate::render::object::PixelObject;
use godot::builtin::Side;
use godot::classes::control::{MouseFilter, SizeFlags};
use godot::classes::performance::Monitor;
use godot::classes::{
    Button, CanvasLayer, CheckBox, ClassDb, ColorPickerButton, Control, FoldableContainer,
    GeometryInstance3D, GridContainer, HBoxContainer, ICanvasLayer, InputEvent, InputEventKey,
    InputEventMouseButton, InputEventMouseMotion, Label, MeshInstance3D, Node, Node3D,
    OptionButton, PackedScene, Performance, SpinBox, VBoxContainer,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyUsageFlags};
use pt_core::meta::id_meta_from_flags;
use pt_core::pick::{ray_aabb, ray_faces};
use std::collections::HashMap;

const TABS: [&str; 3] = ["Effects", "Render", "Scene"];
const TAB_EFFECTS: usize = 0;
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
    ("PixelSunShafts", 0),
    ("MarchingSquaresTerrain", 2),
    ("PixelObject", 2),
    ("PixelBuilderMesh", 2),
    ("MarchingSquaresWildFlowers", 2),
];
const NESTED_RESOURCES: [&str; 3] = ["PixelPalette", "PixelOutlineColors", "PixelObjectProfile"];
pub const PICK_IGNORE_GROUP: &str = "pixel_diagnostics_ignore";
const SELECT_HINT: &str = "Press \"Select Object\", then click a mesh in the viewport.";
const SELECTING_HINT: &str = "Click a mesh to select it (hover highlights, Esc cancels).";
struct Binding {
    obj: Gd<Object>,
    prop: StringName,
    editor: Editor,
    object_panel: bool,
}
enum Editor {
    Check(Gd<CheckBox>),
    Spin(Gd<SpinBox>),
    Choice(Gd<OptionButton>),
    Color(Gd<ColorPickerButton>),
    Vector(Vec<Gd<SpinBox>>),
}
struct Live {
    obj: Gd<Object>,
    class: &'static str,
    label: Gd<Label>,
}
#[derive(GodotClass)]
#[class(tool, base = CanvasLayer)]
pub struct PixelDiagnostics {
    #[export]
    toggle_key: Key,
    #[export]
    start_visible: bool,
    #[export]
    show_tuner: bool,
    #[export]
    show_object_panel: bool,
    #[export(range = (200.0, 800.0, 1.0))]
    panel_width: f32,
    #[export]
    sections_start_folded: bool,
    #[export_group(name = "Refresh")]
    #[export(range = (0.0, 5.0, 0.0105, or_greater))]
    refresh_seconds: f64,
    #[export(range = (0.0, 1.0, 0.0015, or_greater))]
    hover_pick_seconds: f64,
    #[export_group(name = "Limits")]
    #[export(range = (1.0, 64.0, 1.0, or_greater))]
    max_sections_per_class: i32,
    #[export(range = (1.0, 256.0, 1.0, or_greater))]
    max_pick_candidates: i32,
    #[export_group(name = "Parts")]
    #[export]
    overlay_scene: Option<Gd<PackedScene>>,
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
        base.to_init_gd().set_layer(100);
        PixelDiagnostics {
            toggle_key: Key::F3,
            start_visible: false,
            show_tuner: true,
            show_object_panel: true,
            panel_width: 360.0,
            sections_start_folded: true,
            refresh_seconds: 0.2505,
            hover_pick_seconds: 0.0505,
            max_sections_per_class: 8,
            max_pick_candidates: 24,
            overlay_scene: None,
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
        self.build_ui();
        let visible = self.start_visible;
        self.base_mut().set_visible(visible);
        self.base_mut().set_process(true);
    }
    fn process(&mut self, delta: f64) {
        if is_editor() || !self.base().is_visible() {
            return;
        }
        if self.needs_scan {
            self.needs_scan = false;
            self.rescan();
        }
        self.hover_timer -= delta;
        self.update_marker();
        self.refresh_timer -= delta;
        if self.refresh_timer <= 0.0 {
            self.refresh_timer = self.refresh_seconds;
            self.update_status();
            self.pull_values();
            self.update_object_info();
        }
    }
    fn input(&mut self, happening: Gd<InputEvent>) {
        if is_editor() {
            return;
        }
        let Ok(k) = happening.try_cast::<InputEventKey>() else {
            return;
        };
        if !k.is_pressed() || k.is_echo() {
            return;
        }
        if k.get_keycode() == self.toggle_key || k.get_physical_keycode() == self.toggle_key {
            let show = !self.base().is_visible();
            self.set_overlay_visible(show);
            self.handled();
        } else if self.select_mode && k.get_keycode() == Key::ESCAPE {
            self.assign_grab_setting(false);
            self.handled();
        }
    }
    fn unhandled_input(&mut self, happening: Gd<InputEvent>) {
        if is_editor() || !self.select_mode || !self.base().is_visible() {
            return;
        }
        if let Ok(m) = happening.clone().try_cast::<InputEventMouseMotion>() {
            if self.hover_timer <= 0.0 {
                self.hover_timer = self.hover_pick_seconds;
                let hit = self.pick_geometry(m.get_position());
                self.set_hovered(hit);
            }
        } else if let Ok(b) = happening.try_cast::<InputEventMouseButton>()
            && b.is_pressed()
            && b.get_button_index() == MouseButton::LEFT
        {
            let hit = self.pick_geometry(b.get_position());
            self.set_hovered(None);
            self.select_geometry(hit);
            self.assign_grab_setting(false);
            self.handled();
        }
    }
    fn exit_tree(&mut self) {
        self.set_hovered(None);
        self.select_geometry(None);
    }
}
fn label(caption: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(caption);
    l.add_theme_font_size_override("font_size", 12);
    l
}
fn button(caption: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(caption);
    b.add_theme_font_size_override("font_size", 12);
    b
}
fn parse_range(hint: &str) -> Option<(f64, f64, Option<f64>, bool, bool)> {
    let parts: Vec<&str> = hint.split(',').map(str::trim).collect();
    let min = parts.first()?.parse().ok()?;
    let max = parts.get(1)?.parse().ok()?;
    let step = parts.get(2).and_then(|s| s.parse().ok());
    Some((
        min,
        max,
        step,
        parts.contains(&"or_greater"),
        parts.contains(&"or_less"),
    ))
}
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
fn setter(
    obj: &Gd<Object>,
    prop: &StringName,
    map: impl Fn(&Variant) -> Variant + 'static,
) -> Callable {
    let (mut obj, prop) = (obj.clone(), prop.clone());
    Callable::from_fn("pixel_diagnostics_set", move |args: &[&Variant]| {
        if let Some(v) = args.first()
            && obj.is_instance_valid()
        {
            obj.set(&prop, &map(v));
        }
    })
}
fn pretty(name: &str, prefix: &str) -> String {
    let short = name
        .strip_prefix(prefix)
        .filter(|s| !s.is_empty())
        .unwrap_or(name);
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
fn debug_views() -> Vec<(String, i64)> {
    let props = ClassDb::singleton()
        .class_get_property_list_ex("PixelRenderer")
        .no_inheritance(true)
        .done();
    props
        .iter_shared()
        .find(|d| d.get("name").is_some_and(|v| v.to_string() == "debug_view"))
        .and_then(|d| d.get("hint_string"))
        .map(|h| parse_enum(&h.to_string()))
        .unwrap_or_else(|| vec![("Final".into(), 0)])
}
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
        VariantType::VECTOR2I => {
            Vector2i::new(g(0).round() as i32, g(1).round() as i32).to_variant()
        }
        VariantType::VECTOR3I => Vector3i::new(
            g(0).round() as i32,
            g(1).round() as i32,
            g(2).round() as i32,
        )
        .to_variant(),
        _ => Vector3::new(g(0) as f32, g(1) as f32, g(2) as f32).to_variant(),
    }
}
fn v3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}
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
fn geometry_flags(g: &Gd<GeometryInstance3D>) -> i32 {
    if g.has_meta("_pt_base_flags") {
        return g.get_meta("_pt_base_flags").try_to::<i32>().unwrap_or(32);
    }
    g.get_instance_shader_parameter("pt_flags")
        .try_to::<i32>()
        .unwrap_or(32)
}
impl PixelDiagnostics {
    fn handled(&self) {
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
    }
    fn place_dock(&self, panel: &mut Gd<Control>, right: bool) {
        let w = self.panel_width;
        if right {
            panel.set_offset(Side::LEFT, -w - 8.0);
            panel.set_offset(Side::RIGHT, -8.0);
        } else {
            panel.set_offset(Side::LEFT, 8.0);
            panel.set_offset(Side::RIGHT, w + 8.0);
        }
    }
    fn build_ui(&mut self) {
        let Some(parts) = OverlayParts::instantiate(self.overlay_scene.as_ref(), &TABS) else {
            return;
        };
        let OverlayParts {
            root,
            status,
            mut tuner,
            mut rescan,
            mut debug_view,
            tab_sections,
            mut object_panel,
            mut select,
            mut clear,
            object_hint,
            object_info,
            object_sections,
            mut marker,
        } = parts;
        let this = self.to_gd();
        rescan.connect("pressed", &Callable::from_object_method(&this, "rescan"));
        for (name, id) in debug_views() {
            debug_view.add_item_ex(name.as_str()).id(id as i32).done();
        }
        debug_view.connect(
            "item_selected",
            &Callable::from_object_method(&this, "on_debug_view_selected"),
        );
        self.place_dock(&mut tuner, true);
        tuner.set_visible(self.show_tuner);
        select.connect(
            "toggled",
            &Callable::from_object_method(&this, "assign_grab_setting"),
        );
        clear.connect(
            "pressed",
            &Callable::from_object_method(&this, "purge_picked"),
        );
        self.place_dock(&mut object_panel, false);
        object_panel.set_visible(self.show_object_panel);
        marker.set_layer_mask(1 & !globals::meta_mask());
        let mut base = self.to_gd();
        add_internal(&mut base, &root);
        self.status = Some(status);
        self.tuner = Some(tuner);
        self.sections = tab_sections;
        self.debug_view = Some(debug_view);
        self.object_panel = Some(object_panel);
        self.object_hint = Some(object_hint);
        self.object_info = Some(object_info);
        self.object_sections = Some(object_sections);
        self.select_button = Some(select);
        self.marker = Some(marker);
    }
    fn scene_root(&self) -> Option<Gd<Node>> {
        let tree = self.base().get_tree();
        tree.get_current_scene()
            .or_else(|| Some(tree.get_root().upcast()))
    }
    fn find_targets(&self) -> Vec<(usize, Gd<Node>)> {
        let mut found: Vec<(usize, Gd<Node>)> = Vec::new();
        let mut counts = [0usize; TUNED_CLASSES.len()];
        let Some(root) = self.scene_root() else {
            return found;
        };
        let me = self.to_gd().upcast::<Node>();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n == me {
                continue;
            }
            let class = n.get_class().to_string();
            if let Some(i) = TUNED_CLASSES.iter().position(|(c, _)| *c == class) {
                let limit = if i == 0 {
                    1
                } else {
                    self.max_sections_per_class.max(1) as usize
                };
                if counts[i] < limit {
                    counts[i] += 1;
                    found.push((i, n.clone()));
                }
            }
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
        let mut text = format!(
            "{fps:.0} fps | {draws:.0} draws | {:.1}k prims | {} hides",
            prims / 1000.0,
            self.toggle_key.as_str()
        );
        match self.renderer.as_mut().filter(|r| r.is_instance_valid()) {
            Some(r) => {
                let line = r.call("fetch_situation_streak", &[]);
                text.push_str(&format!(
                    "\n{}",
                    line.try_to::<GString>().unwrap_or_default()
                ));
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
        let mut top = 72.0;
        if let Some(s) = &mut self.status {
            s.set_text(&text);
            if let Some(panel) = s.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                top = panel.get_position().y + panel.get_combined_minimum_size().y + 6.0;
            }
        }
        let (show_tuner, show_obj) = (self.show_tuner, self.show_object_panel);
        let has_selection = self
            .selected
            .as_ref()
            .is_some_and(|s| s.is_instance_valid());
        if let Some(t) = &mut self.tuner {
            t.set_visible(show_tuner);
            t.set_offset(Side::TOP, top);
        }
        if let Some(t) = &mut self.object_panel {
            t.set_visible(show_obj);
            t.set_offset(Side::TOP, top);
            if has_selection {
                t.set_anchor(Side::BOTTOM, 1.0);
                t.set_offset(Side::BOTTOM, -8.0);
            } else {
                t.set_anchor(Side::BOTTOM, 0.0);
                t.set_offset(Side::BOTTOM, top);
            }
        }
    }
    fn pull_values(&mut self) {
        self.bindings.retain(|b| b.obj.is_instance_valid());
        for b in &mut self.bindings {
            let v = b.obj.get(&b.prop);
            match &mut b.editor {
                Editor::Check(c) => c.set_pressed_no_signal(v.try_to::<bool>().unwrap_or(false)),
                Editor::Spin(s) => {
                    let editing = s.get_line_edit().is_some_and(|l| l.has_focus());
                    if !editing {
                        let f = v
                            .try_to::<f64>()
                            .or_else(|_| v.try_to::<i64>().map(|i| i as f64))
                            .unwrap_or(0.0);
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
                    let editing = spins
                        .iter()
                        .any(|s| s.get_line_edit().is_some_and(|l| l.has_focus()));
                    if !editing {
                        for (s, c) in spins.iter_mut().zip(vector_parts(&v)) {
                            s.set_value_no_signal(c);
                        }
                    }
                }
            }
        }
        for (f, c) in &mut self.feature_checks {
            c.set_pressed_no_signal(features::active(*f));
        }
        self.live
            .retain(|l| l.obj.is_instance_valid() && l.label.is_instance_valid());
        for l in &mut self.live {
            let text = actions::live_text(l.class, &mut l.obj).unwrap_or_default();
            l.label.set_text(&text);
            l.label.set_visible(!text.is_empty());
        }
    }
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
                    parts.push(format!(
                        "weather {} (wet {wet:.2}, snow {snow:.2})",
                        o.call("fetch_climate_moniker", &[])
                    ));
                }
                "PixelDayNight" => {
                    let o = &l.obj;
                    let ov = o.get("time_override").try_to::<f32>().unwrap_or(-1.0);
                    let t = if ov >= 0.0 {
                        ov
                    } else {
                        o.get("time_of_day").try_to::<f32>().unwrap_or(0.0)
                    };
                    parts.push(format!("time {}", actions::clock_text(t)));
                }
                _ => {}
            }
        }
        let on = features::ALL
            .iter()
            .filter(|i| features::active(i.feature))
            .count();
        parts.push(format!("features {on}/{}", features::ALL.len()));
        parts.join(" | ")
    }
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
                    let on = args
                        .first()
                        .and_then(|v| v.try_to::<bool>().ok())
                        .unwrap_or(false);
                    features::assign_active(feat, on);
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
                        features::assign_active(i.feature, on || i.default);
                    }
                }),
            );
            all.add_child(&b);
        }
        col.add_child(&all);
        fold.add_child(&col);
        container.add_child(&fold);
    }
    fn add_section(
        &mut self,
        mut container: Gd<VBoxContainer>,
        class: &str,
        obj: Gd<Object>,
        title: &str,
        folded: bool,
        object_panel: bool,
    ) {
        let mut fold = FoldableContainer::new_alloc();
        fold.set_title(title);
        fold.set_folded(folded);
        let mut col = VBoxContainer::new_alloc().upcast::<Control>();
        if !object_panel {
            if let Some(cls) = TUNED_CLASSES.iter().map(|(c, _)| *c).find(|c| *c == class)
                && actions::live_text(cls, &mut obj.clone()).is_some()
            {
                let label = actions::live_label(&mut col);
                self.live.push(Live {
                    obj: obj.clone(),
                    class: cls,
                    label,
                });
            }
            actions::add_actions(&mut col, class, &obj);
        }
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);
        let mut group = (String::new(), String::new());
        let mut subgroup = (String::new(), String::new());
        let mut shown_header = String::new();
        let mut nested: Vec<(String, Gd<Object>)> = Vec::new();
        let props = ClassDb::singleton()
            .class_get_property_list_ex(class)
            .no_inheritance(true)
            .done();
        let editor_flag = PropertyUsageFlags::EDITOR.ord() as i64;
        let (group_flag, subgroup_flag) = (
            PropertyUsageFlags::GROUP.ord() as i64,
            PropertyUsageFlags::SUBGROUP.ord() as i64,
        );
        let skip = PropertyUsageFlags::CATEGORY.ord() as i64;
        for d in props.iter_shared() {
            let get_i = |k: &str| d.get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
            let usage = get_i("usage");
            let name = d.get("name").map(|v| v.to_string()).unwrap_or_default();
            if usage & (group_flag | subgroup_flag) != 0 {
                let prefix = d
                    .get("hint_string")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
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
            let hint_string = d
                .get("hint_string")
                .map(|v| v.to_string())
                .unwrap_or_default();
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
                    match (hint == PropertyHint::RANGE.ord() as i64)
                        .then(|| parse_range(&hint_string))
                        .flatten()
                    {
                        Some((min, max, step, greater, lesser)) => {
                            s.set_min(min);
                            s.set_max(max);
                            s.set_step(step.unwrap_or(if int { 1.0 } else { 0.0015 }));
                            s.set_allow_greater(greater);
                            s.set_allow_lesser(lesser);
                        }
                        None => {
                            s.set_min(-1e6);
                            s.set_max(1e6);
                            s.set_step(if int { 1.0 } else { 0.0015 });
                        }
                    }
                    s.set_custom_minimum_size(Vector2::new(120.0, 0.0));
                    s.connect(
                        "value_changed",
                        &setter(&obj, &prop, move |v| {
                            let f = v
                                .try_to::<f64>()
                                .or_else(|_| v.try_to::<i64>().map(|i| i as f64))
                                .unwrap_or(0.0);
                            if int {
                                (f.round() as i64).to_variant()
                            } else {
                                f.to_variant()
                            }
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
                            s.set_step(if int { 1.0 } else { 0.0105 });
                            s.set_custom_minimum_size(Vector2::new(
                                if n == 2 { 60.0 } else { 40.0 },
                                0.0,
                            ));
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
            let header = if subgroup.0.is_empty() {
                group.0.clone()
            } else {
                format!("{} / {}", group.0, subgroup.0)
            };
            if header != shown_header {
                if !header.is_empty() {
                    let mut h = label(&header);
                    h.set_modulate(Color::from_rgba(1.0, 0.8505, 0.5, 1.0));
                    grid.add_child(&h);
                    grid.add_child(&Control::new_alloc());
                }
                shown_header = header;
            }
            let prefix = if !subgroup.1.is_empty() && name.starts_with(&subgroup.1) {
                &subgroup.1
            } else {
                &group.1
            };
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
            self.bindings.push(Binding {
                obj: obj.clone(),
                prop,
                editor,
                object_panel,
            });
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
    fn pickable(&self) -> Vec<Gd<GeometryInstance3D>> {
        let mut out = Vec::new();
        let Some(root) = self.scene_root() else {
            return out;
        };
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
    fn facets(&mut self, mi: &Gd<MeshInstance3D>) -> Option<Vec<[f32; 3]>> {
        let mesh = mi.get_mesh()?;
        let key = mesh.instance_id().to_i64();
        if !self.face_cache.contains_key(&key) {
            let f: Vec<[f32; 3]> = mesh.get_faces().as_slice().iter().map(|v| v3(*v)).collect();
            self.face_cache.insert(key, f);
        }
        self.face_cache.get(&key).cloned()
    }
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
            if let Some(t) = ray_aabb(
                v3(lo),
                v3(ld),
                v3(aabb.position),
                v3(aabb.position + aabb.size),
            ) {
                cands.push((t, g, inv));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut best: Option<(f32, Gd<GeometryInstance3D>)> = None;
        for (t_box, g, inv) in cands
            .into_iter()
            .take(self.max_pick_candidates.max(1) as usize)
        {
            if best.as_ref().is_some_and(|(bt, _)| t_box > *bt) {
                break;
            }
            let t = match g.clone().try_cast::<MeshInstance3D>() {
                Ok(mi) => match self.facets(&mi) {
                    Some(f) if !f.is_empty() => {
                        ray_faces(v3(inv * origin), v3(inv.basis * dir), &f)
                    }
                    _ => Some(t_box),
                },
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
        self.refresh_timer = 0.0;
        self.rebuild_object_panel();
    }
    fn update_marker(&mut self) {
        let sel = self
            .selected
            .clone()
            .filter(|s| s.is_instance_valid() && s.is_inside_tree());
        let Some(mut m) = self.marker.clone().filter(|m| m.is_instance_valid()) else {
            return;
        };
        match sel {
            Some(s) => {
                let aabb = s.get_aabb();
                let xf = s.get_global_transform();
                let centre = aabb.position + aabb.size * 0.5;
                let size = Vector3::new(
                    aabb.size.x.max(0.0505),
                    aabb.size.y.max(0.0505),
                    aabb.size.z.max(0.0505),
                ) * 1.0405;
                let basis = xf.basis * Basis::from_scale(size);
                m.set_global_transform(Transform3D::new(basis, xf * centre));
                m.set_visible(true);
            }
            None => m.set_visible(false),
        }
    }
    fn rebuild_object_panel(&mut self) {
        self.bindings.retain(|b| !b.object_panel);
        let Some(sections) = self.object_sections.clone() else {
            return;
        };
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
            let profile = owner.bind().get_profile_for(node.clone());
            match profile {
                Some(profile) => {
                    let mut rm = button("Remove profile");
                    rm.connect(
                        "pressed",
                        &Callable::from_object_method(&this, "remove_selected_profile"),
                    );
                    row.add_child(&rm);
                    let mut s = sections.clone();
                    s.add_child(&row);
                    self.add_section(
                        sections.clone(),
                        "PixelObjectProfile",
                        profile.upcast(),
                        "Profile (this mesh)",
                        false,
                        true,
                    );
                }
                None => {
                    let mut mk = button("Create profile");
                    mk.set_tooltip_text("Add a PixelObjectProfile targeting this mesh to its PixelObject (all values inherit).");
                    mk.connect(
                        "pressed",
                        &Callable::from_object_method(&this, "create_selected_profile"),
                    );
                    row.add_child(&mk);
                    let mut s = sections.clone();
                    s.add_child(&row);
                }
            }
            let title = format!("Owning PixelObject: {}", owner.get_name());
            self.add_section(
                sections.clone(),
                "PixelObject",
                owner.clone().upcast(),
                &title,
                false,
                true,
            );
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
        self.add_section(
            sections.clone(),
            "GeometryInstance3D",
            sel.clone().upcast(),
            &format!("Geometry: {} ({cls})", sel.get_name()),
            true,
            true,
        );
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
                    id.pixel_size,
                    if flags & 32 != 0 { "on" } else { "off" },
                    id.outline_id,
                    if id.palette { "on" } else { "off" },
                    if id.dither { "on" } else { "off" },
                );
                match owner_object(&node) {
                    Some(o) => t.push_str(&format!(
                        "\nowner {} (profiles {})",
                        o.get_name(),
                        o.get("profiles")
                            .try_to::<VarArray>()
                            .map(|a| a.len())
                            .unwrap_or(0)
                    )),
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
    #[func]
    fn get_published_globals() -> VarDictionary {
        crate::globals::freeze_frame()
    }
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
        self.renderer = targets
            .iter()
            .find(|(i, _)| *i == 0)
            .map(|(_, n)| n.clone());
        let folded = self.sections_start_folded;
        if self.sections.len() < TABS.len() {
            return;
        }
        self.add_features_section(self.sections[TAB_EFFECTS].clone());
        let mut opened = [false; TABS.len()];
        for (i, node) in &targets {
            let (class, tab) = TUNED_CLASSES[*i];
            let title = format!("{class}: {}", node.get_name());
            let open = !opened[tab] || !folded;
            opened[tab] = true;
            self.add_section(
                self.sections[tab].clone(),
                class,
                node.clone().upcast(),
                &title,
                !open,
                false,
            );
        }
        self.pull_values();
        self.update_status();
    }
    #[func]
    fn on_debug_view_selected(&mut self, index: i64) {
        let id = self
            .debug_view
            .as_ref()
            .map(|d| d.get_item_id(index as i32))
            .unwrap_or(index as i32);
        if let Some(r) = self.renderer.as_mut().filter(|r| r.is_instance_valid()) {
            r.set("debug_view", &id.to_variant());
        }
    }
    #[func]
    fn set_overlay_visible(&mut self, shown: bool) {
        self.base_mut().set_visible(shown);
        if shown {
            self.needs_scan = true;
            self.refresh_timer = 0.0;
        } else {
            self.assign_grab_setting(false);
        }
        if let Some(m) = &mut self.marker {
            m.set_visible(shown && self.selected.is_some());
        }
    }
    #[func]
    fn toggle(&mut self) {
        let v = !self.base().is_visible();
        self.set_overlay_visible(v);
    }
    #[func]
    fn assign_grab_setting(&mut self, on: bool) {
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
    #[func]
    fn select_node(&mut self, node: Option<Gd<Node>>) {
        let g = node.and_then(|n| {
            if let Ok(g) = n.clone().try_cast::<GeometryInstance3D>() {
                return Some(g);
            }
            n.find_children_ex("*")
                .type_("GeometryInstance3D")
                .owned(false)
                .done()
                .iter_shared()
                .next()
                .and_then(|c| c.try_cast().ok())
        });
        self.select_geometry(g);
    }
    #[func]
    fn hover_node(&mut self, node: Option<Gd<Node>>) {
        let g = node.and_then(|n| n.try_cast::<GeometryInstance3D>().ok());
        self.set_hovered(g);
    }
    #[func]
    fn get_selected_node(&self) -> Option<Gd<Node3D>> {
        self.selected
            .clone()
            .filter(|s| s.is_instance_valid())
            .map(|s| s.upcast())
    }
    #[func]
    fn pick_at(&mut self, screen_position: Vector2) -> Option<Gd<Node3D>> {
        self.pick_geometry(screen_position).map(|g| g.upcast())
    }
    #[func]
    fn purge_picked(&mut self) {
        self.select_geometry(None);
    }
    #[func]
    fn create_selected_profile(&mut self) {
        let Some(sel) = self.selected.clone() else {
            return;
        };
        let node = sel.upcast::<Node>();
        if let Some(mut owner) = owner_object(&node) {
            owner.bind_mut().ensure_profile_for(node);
        }
        self.to_gd().call_deferred("refresh_object_panel", &[]);
    }
    #[func]
    fn remove_selected_profile(&mut self) {
        let Some(sel) = self.selected.clone() else {
            return;
        };
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
