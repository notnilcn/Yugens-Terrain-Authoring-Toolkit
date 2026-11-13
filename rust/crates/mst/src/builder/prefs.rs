//! Pixel Builder preferences: port of Unibuilder's `UBPreferencesPanel`
//! (`editor/preferences_panel.gd` + `.tscn`, MIT) and its `UBGizmoPlugin.DEFAULTS`.
//!
//! Overlay colors, vertex / handle sizes, the default snap steps, plus a read-only list of the
//! builder shortcuts. Stored per project in the editor settings' project metadata (section
//! `pixel_builder`), so nothing is written into scenes or `project.godot`.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, CheckBox, ColorPickerButton, EditorInterface, GridContainer, HSeparator, Label, PopupPanel, RichTextLabel, ScrollContainer, SpinBox,
    VBoxContainer,
};
use godot::prelude::*;

const SECTION: &str = "pixel_builder";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prefs {
    /// Vertex handle size in px (Unibuilder "Vertex size (px)").
    pub vertex_size: f32,
    /// Scale of the move / rotate / scale handles.
    pub handle_scale: f32,
    pub color_edge: Color,
    pub color_edge_selected: Color,
    pub color_vertex: Color,
    pub color_vertex_selected: Color,
    pub color_face_selected: Color,
    pub color_face_hover: Color,
    /// Grid snap step new sessions start with.
    pub grid_step: f32,
    /// Rotate handle snap (degrees).
    pub rot_snap: f32,
    /// Grid snap on by default.
    pub snap_grid: bool,
    /// Lightmap UV2 texel size for Bake / glTF export (Unibuilder "Bake lightmap texel"; 0 = skip).
    pub lightmap_texel: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            vertex_size: 5.0,
            handle_scale: 1.0,
            color_edge: Color::from_rgba(0.9, 0.9, 0.95, 0.55),
            color_edge_selected: Color::from_rgba(1.0, 0.6, 0.15, 1.0),
            color_vertex: Color::from_rgb(0.85, 0.85, 0.85),
            color_vertex_selected: Color::from_rgba(1.0, 0.6, 0.15, 1.0),
            color_face_selected: Color::from_rgba(1.0, 0.55, 0.1, 0.35),
            color_face_hover: Color::from_rgba(0.5, 0.8, 1.0, 0.18),
            grid_step: 0.25,
            rot_snap: 15.0,
            snap_grid: true,
            lightmap_texel: pt_core::build::lightmap::DEFAULT_TEXEL,
        }
    }
}

/// Preference keys (bound to the panel's controls).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Key {
    VertexSize = 0,
    HandleScale,
    GridStep,
    RotSnap,
    SnapGrid,
    ColorEdge,
    ColorEdgeSelected,
    ColorVertex,
    ColorVertexSelected,
    ColorFaceSelected,
    ColorFaceHover,
    LightmapTexel,
}

impl Key {
    pub const ALL: [Key; 12] = [
        Key::VertexSize,
        Key::HandleScale,
        Key::GridStep,
        Key::RotSnap,
        Key::SnapGrid,
        Key::LightmapTexel,
        Key::ColorEdge,
        Key::ColorEdgeSelected,
        Key::ColorVertex,
        Key::ColorVertexSelected,
        Key::ColorFaceSelected,
        Key::ColorFaceHover,
    ];
    pub fn from_i64(v: i64) -> Option<Key> {
        Self::ALL.iter().copied().find(|k| *k as i64 == v)
    }
    fn name(self) -> &'static str {
        match self {
            Key::VertexSize => "vertex_size",
            Key::HandleScale => "handle_scale",
            Key::GridStep => "grid_step",
            Key::RotSnap => "rot_snap",
            Key::SnapGrid => "snap_grid",
            Key::ColorEdge => "color_edge",
            Key::ColorEdgeSelected => "color_edge_selected",
            Key::ColorVertex => "color_vertex",
            Key::ColorVertexSelected => "color_vertex_selected",
            Key::ColorFaceSelected => "color_face_selected",
            Key::ColorFaceHover => "color_face_hover",
            Key::LightmapTexel => "lightmap_texel",
        }
    }
    /// (label, tooltip)
    fn describe(self) -> (&'static str, &'static str) {
        match self {
            Key::VertexSize => ("Vertex size (px)", "Size of the vertex handles in Vertex mode"),
            Key::HandleScale => ("Handle size", "Scale of the move / rotate / scale handles (1 = 64 px arms)"),
            Key::GridStep => ("Grid step", "Grid snap step the builder starts with ([ / ] halve / double it while editing)"),
            Key::RotSnap => ("Rotate snap (°)", "Rotate handle snap step while grid snapping is on (0 = free)"),
            Key::SnapGrid => ("Grid snap on", "Start with grid snapping enabled"),
            Key::ColorEdge => ("Edges", "Wireframe color"),
            Key::ColorEdgeSelected => ("Selected edges", "Color of selected edges"),
            Key::ColorVertex => ("Vertices", "Vertex handle color"),
            Key::ColorVertexSelected => ("Selected vertices", "Color of selected vertices"),
            Key::ColorFaceSelected => ("Selected faces", "Tint of selected faces"),
            Key::ColorFaceHover => ("Hovered face", "Tint of the face under the cursor"),
            Key::LightmapTexel => ("Lightmap texel", "Lightmap UV2s are unwrapped at this texel size (world units) on Bake to MeshInstance3D and glTF export. 0 = skip"),
        }
    }
}

impl Prefs {
    pub fn get_f(&self, k: Key) -> f32 {
        match k {
            Key::VertexSize => self.vertex_size,
            Key::HandleScale => self.handle_scale,
            Key::GridStep => self.grid_step,
            Key::RotSnap => self.rot_snap,
            Key::SnapGrid => self.snap_grid as i32 as f32,
            Key::LightmapTexel => self.lightmap_texel,
            _ => 0.0,
        }
    }
    pub fn get_color(&self, k: Key) -> Color {
        match k {
            Key::ColorEdge => self.color_edge,
            Key::ColorEdgeSelected => self.color_edge_selected,
            Key::ColorVertex => self.color_vertex,
            Key::ColorVertexSelected => self.color_vertex_selected,
            Key::ColorFaceSelected => self.color_face_selected,
            Key::ColorFaceHover => self.color_face_hover,
            _ => Color::WHITE,
        }
    }
    pub fn set(&mut self, k: Key, v: &Variant) {
        let f = v.try_to::<f64>().ok().map(|x| x as f32);
        let c = v.try_to::<Color>().ok();
        match k {
            Key::VertexSize => self.vertex_size = f.unwrap_or(self.vertex_size).clamp(2.0, 24.0),
            Key::HandleScale => self.handle_scale = f.unwrap_or(self.handle_scale).clamp(0.5, 3.0),
            Key::GridStep => self.grid_step = f.unwrap_or(self.grid_step).clamp(0.0625, 64.0),
            Key::RotSnap => self.rot_snap = f.unwrap_or(self.rot_snap).clamp(0.0, 90.0),
            Key::SnapGrid => self.snap_grid = v.try_to::<bool>().unwrap_or(self.snap_grid),
            Key::ColorEdge => self.color_edge = c.unwrap_or(self.color_edge),
            Key::ColorEdgeSelected => self.color_edge_selected = c.unwrap_or(self.color_edge_selected),
            Key::ColorVertex => self.color_vertex = c.unwrap_or(self.color_vertex),
            Key::ColorVertexSelected => self.color_vertex_selected = c.unwrap_or(self.color_vertex_selected),
            Key::ColorFaceSelected => self.color_face_selected = c.unwrap_or(self.color_face_selected),
            Key::ColorFaceHover => self.color_face_hover = c.unwrap_or(self.color_face_hover),
            Key::LightmapTexel => self.lightmap_texel = f.unwrap_or(self.lightmap_texel).clamp(0.0, 16.0),
        }
    }
    fn value(&self, k: Key) -> Variant {
        match k {
            Key::SnapGrid => self.snap_grid.to_variant(),
            Key::VertexSize | Key::HandleScale | Key::GridStep | Key::RotSnap | Key::LightmapTexel => (self.get_f(k) as f64).to_variant(),
            _ => self.get_color(k).to_variant(),
        }
    }

    /// Loads the project's preferences (defaults for missing keys).
    pub fn load() -> Prefs {
        let mut p = Prefs::default();
        let Some(es) = EditorInterface::singleton().get_editor_settings() else { return p };
        for k in Key::ALL {
            // Missing keys come back as the "" default (a nil default makes Godot print an error).
            let v = es.get_project_metadata_ex(SECTION, k.name()).default(&"".to_variant()).done();
            if !v.is_nil() && v.try_to::<GString>().is_err() {
                p.set(k, &v);
            }
        }
        p
    }

    /// Stores every key (Reset stores the defaults).
    pub fn save(&self) {
        let Some(mut es) = EditorInterface::singleton().get_editor_settings() else { return };
        for k in Key::ALL {
            es.set_project_metadata(SECTION, k.name(), &self.value(k));
        }
    }
}

/// Shortcut reference shown in the panel (`[key] action`).
const SHORTCUTS: &str = "[b]Modes[/b]  1 Object · 2 Face · 3 Edge · 4 Vertex · Esc back / clear
[b]Tools[/b]  W select / move · C draw shape · B paint · 5 vertex paint (Shift+click picks) · P place on cell · K knife
[b]Handles[/b]  Shift+W move · Shift+E rotate · Shift+R scale · O soft selection · hold V vertex snap
[b]Edit[/b]  E extrude · I inset · Ctrl+B bevel · Ctrl+R insert loop · M merge · J connect (vertices / edges) · F fill · Ctrl+T triangulate
        R / Shift+R rotate 90° · Ctrl+D duplicate · Del delete
[b]Select[/b]  Ctrl+A all · G grow · Shift+G shrink · L loop · Alt+L ring · double-click loop
[b]Grid[/b]  [ / ] halve / double the step
[b]UV editor[/b]  drag move · Ctrl+drag scale (Shift per axis) · Alt+drag rotate · middle pan · wheel zoom";

/// Builds the preferences popup; controls call `on_pref(value, key)` and `on_prefs_reset()` on
/// `target`. Returns the popup and the controls per key (for `sync`).
pub fn build_panel(target: &Gd<Object>, p: &Prefs) -> (Gd<PopupPanel>, Vec<(Key, Gd<godot::classes::Control>)>) {
    let s = crate::editor::ui_kit::editor_scale();
    let mut popup = PopupPanel::new_alloc();
    popup.set_name("PixelBuilderPreferences");
    popup.set_title("Pixel Builder Preferences");
    let mut scroll = ScrollContainer::new_alloc();
    scroll.set_custom_minimum_size(Vector2::new(440.0, 600.0) * s);
    scroll.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
    let mut col = VBoxContainer::new_alloc();
    col.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut title = Label::new_alloc();
    title.set_text("Pixel Builder Preferences");
    title.add_theme_font_size_override("font_size", (15.0 * s) as i32);
    col.add_child(&title);
    let mut grid = GridContainer::new_alloc();
    grid.set_columns(2);
    let mut controls = Vec::new();
    for k in Key::ALL {
        let (text, tip) = k.describe();
        let mut l = Label::new_alloc();
        l.set_text(text);
        l.set_tooltip_text(tip);
        l.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
        l.set_custom_minimum_size(Vector2::new(150.0 * s, 0.0));
        grid.add_child(&l);
        let cb = Callable::from_object_method(target, "on_pref").bind(&[(k as i64).to_variant()]);
        let c: Gd<godot::classes::Control> = match k {
            Key::SnapGrid => {
                let mut c = CheckBox::new_alloc();
                c.set_pressed_no_signal(p.snap_grid);
                c.connect("toggled", &cb);
                c.upcast()
            }
            Key::VertexSize | Key::HandleScale | Key::GridStep | Key::RotSnap | Key::LightmapTexel => {
                let (min, max, step) = match k {
                    Key::VertexSize => (2.0, 24.0, 1.0),
                    Key::LightmapTexel => (0.0, 16.0, 0.01),
                    Key::HandleScale => (0.5, 3.0, 0.05),
                    Key::GridStep => (0.0625, 64.0, 0.0625),
                    _ => (0.0, 90.0, 1.0),
                };
                let mut sb = SpinBox::new_alloc();
                sb.set_min(min);
                sb.set_max(max);
                sb.set_step(step);
                sb.set_value_no_signal(p.get_f(k) as f64);
                sb.connect("value_changed", &cb);
                sb.upcast()
            }
            _ => {
                let mut c = ColorPickerButton::new_alloc();
                c.set_pick_color(p.get_color(k));
                c.set_custom_minimum_size(Vector2::new(72.0 * s, 24.0 * s));
                c.connect("color_changed", &cb);
                c.upcast()
            }
        };
        let mut c = c;
        c.set_h_size_flags(SizeFlags::EXPAND_FILL);
        c.set_tooltip_text(tip);
        grid.add_child(&c);
        controls.push((k, c));
    }
    col.add_child(&grid);
    let mut reset = Button::new_alloc();
    reset.set_text("Reset to Defaults");
    reset.set_focus_mode(FocusMode::NONE);
    reset.connect("pressed", &Callable::from_object_method(target, "on_prefs_reset"));
    col.add_child(&reset);
    col.add_child(&HSeparator::new_alloc());
    let mut sh = Label::new_alloc();
    sh.set_text("Shortcuts (3D viewport, builder mesh selected)");
    col.add_child(&sh);
    let mut rt = RichTextLabel::new_alloc();
    rt.set_use_bbcode(true);
    rt.set_fit_content(true);
    rt.set_text(SHORTCUTS);
    rt.set_h_size_flags(SizeFlags::EXPAND_FILL);
    col.add_child(&rt);
    scroll.add_child(&col);
    popup.add_child(&scroll);
    (popup, controls)
}

static HANDLE_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000); // 1.0

/// Current handle scale (read by the gizmo drawing / picking code).
pub fn handle_scale() -> f32 {
    f32::from_bits(HANDLE_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn set_handle_scale(v: f32) {
    HANDLE_SCALE.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Mirrors `p` into the panel controls without signals.
pub fn sync_panel(controls: &mut [(Key, Gd<godot::classes::Control>)], p: &Prefs) {
    for (k, c) in controls {
        if let Ok(mut sb) = c.clone().try_cast::<SpinBox>() {
            sb.set_value_no_signal(p.get_f(*k) as f64);
        } else if let Ok(mut cb) = c.clone().try_cast::<CheckBox>() {
            cb.set_pressed_no_signal(p.snap_grid);
        } else if let Ok(mut cp) = c.clone().try_cast::<ColorPickerButton>() {
            cp.set_pick_color(p.get_color(*k));
        }
    }
}
