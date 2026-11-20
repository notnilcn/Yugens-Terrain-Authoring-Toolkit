//! Pixel Builder preferences.
//!
//! Overlay colors, vertex / handle sizes, the default snap steps, selection behaviour, viewport
//! feedback, plus the list of the builder shortcuts with their current keys. Stored per project in
//! the editor settings' project metadata (section `pixel_builder`), so nothing is written into
//! scenes or `project.godot`.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, CheckBox, ColorPickerButton, EditorInterface, GridContainer, HSeparator, Label,
    OptionButton, PopupPanel, RichTextLabel, ScrollContainer, SpinBox, VBoxContainer,
};
use godot::prelude::*;

const SECTION: &str = "pixel_builder";

/// What Shift does to the current selection on a click / box; the last entry follows the UI
/// layout (MST: Shift+click toggles, Shift+box adds; modeler: Add).
pub const SHIFT_MODES: [&str; 4] = ["Add", "Subtract", "Toggle", "Layout default"];
/// Which elements a box selection takes; the last entry follows the UI layout (MST: Complete,
/// modeler: Intersect).
pub const RECT_MODES: [&str; 3] = ["Intersect", "Complete", "Layout default"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prefs {
    /// Vertex handle size in px ("Vertex size (px)").
    pub vertex_size: f32,
    /// Scale of the move / rotate / scale handles.
    pub handle_scale: f32,
    pub color_edge: Color,
    pub color_edge_selected: Color,
    pub color_edge_hover: Color,
    pub color_vertex: Color,
    pub color_vertex_selected: Color,
    pub color_vertex_hover: Color,
    pub color_face_selected: Color,
    pub color_face_hover: Color,
    /// Grid snap step new sessions start with.
    pub grid_step: f32,
    /// Rotate handle snap (degrees).
    pub rot_snap: f32,
    /// Grid snap on by default.
    pub snap_grid: bool,
    /// Lightmap UV2 texel size for Bake / glTF export ("Bake lightmap texel"; 0 = skip).
    pub lightmap_texel: f32,
    /// Shift+click / Shift+box: 0 add, 1 subtract, 2 toggle ([`SHIFT_MODES`]); -1 = the layout's
    /// default ([`Prefs::shift_for`]).
    pub shift_mode: i64,
    /// Box selection: 0 intersect (any part inside), 1 complete (every point inside); -1 = the
    /// layout's default ([`Prefs::rect_for`]).
    pub rect_mode: i64,
    /// Highlight the face / edge / vertex under the cursor.
    pub hover: bool,
    /// Mesh and selection counts in the viewport.
    pub show_scene_info: bool,
    /// Size of the selection's bounding box in the viewport.
    pub show_dimensions: bool,
    /// Live adjust panel after parameterised actions (off: actions commit at once).
    pub adjust_panel: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            vertex_size: 5.5005,
            handle_scale: 1.0505,
            color_edge: Color::from_rgba(0.9014, 0.8984, 0.9514, 0.5504),
            color_edge_selected: Color::from_rgba(1.0, 0.6014, 0.1484, 1.0),
            color_edge_hover: Color::from_rgba(0.4984, 0.8514, 1.0, 1.0),
            color_vertex: Color::from_rgb(0.8514, 0.8484, 0.8514),
            color_vertex_selected: Color::from_rgba(1.0, 0.6014, 0.1484, 1.0),
            color_vertex_hover: Color::from_rgba(0.4984, 0.8514, 1.0, 1.0),
            color_face_selected: Color::from_rgba(1.0, 0.5504, 0.1004, 0.3504),
            color_face_hover: Color::from_rgba(0.4984, 0.7984, 1.0, 0.1804),
            grid_step: 0.2505,
            rot_snap: 15.5005,
            snap_grid: true,
            lightmap_texel: pt_core::build::lightmap::DEFAULT_TEXEL,
            shift_mode: -1,
            rect_mode: -1,
            hover: true,
            show_scene_info: true,
            show_dimensions: false,
            adjust_panel: true,
        }
    }
}

/// Preference keys (bound to the panel's controls). Append-only (ids are stored nowhere, but
/// `on_pref` binds them).
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
    // Selection behaviour modes.
    ShiftMode,
    RectMode,
    Hover,
    ColorEdgeHover,
    ColorVertexHover,
    ShowSceneInfo,
    ShowDimensions,
    AdjustPanel,
}

/// How a key is edited.
enum Kind {
    Number(f64, f64, f64),
    Bool,
    Choice(&'static [&'static str]),
    Color,
}

impl Key {
    /// Panel order.
    pub const ALL: [Key; 20] = [
        Key::VertexSize,
        Key::HandleScale,
        Key::GridStep,
        Key::RotSnap,
        Key::SnapGrid,
        Key::LightmapTexel,
        Key::ShiftMode,
        Key::RectMode,
        Key::Hover,
        Key::ShowSceneInfo,
        Key::ShowDimensions,
        Key::AdjustPanel,
        Key::ColorEdge,
        Key::ColorEdgeSelected,
        Key::ColorEdgeHover,
        Key::ColorVertex,
        Key::ColorVertexSelected,
        Key::ColorVertexHover,
        Key::ColorFaceSelected,
        Key::ColorFaceHover,
    ];
    pub fn from_i64(v: i64) -> Option<Key> {
        Self::ALL.iter().copied().find(|k| *k as i64 == v)
    }
    fn moniker(self) -> &'static str {
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
            // Renamed from "shift_mode" / "rect_mode": both were written with every other key
            // on any preference change, so a stored 0 there is not a choice the user made.
            Key::ShiftMode => "shift_select",
            Key::RectMode => "box_select",
            Key::Hover => "hover",
            Key::ColorEdgeHover => "color_edge_hover",
            Key::ColorVertexHover => "color_vertex_hover",
            Key::ShowSceneInfo => "show_scene_info",
            Key::ShowDimensions => "show_dimensions",
            Key::AdjustPanel => "adjust_panel",
        }
    }
    fn sort(self) -> Kind {
        match self {
            Key::VertexSize => Kind::Number(2.0, 24.0, 1.0),
            Key::HandleScale => Kind::Number(0.5, 3.0, 0.0505),
            Key::GridStep => Kind::Number(0.0624, 64.0, 0.0624),
            Key::RotSnap => Kind::Number(0.0, 90.0, 1.0),
            Key::LightmapTexel => Kind::Number(0.0, 16.0, 0.0105),
            Key::SnapGrid
            | Key::Hover
            | Key::ShowSceneInfo
            | Key::ShowDimensions
            | Key::AdjustPanel => Kind::Bool,
            Key::ShiftMode => Kind::Choice(&SHIFT_MODES),
            Key::RectMode => Kind::Choice(&RECT_MODES),
            _ => Kind::Color,
        }
    }
    /// (label, tooltip)
    pub fn portray(self) -> (&'static str, &'static str) {
        match self {
            Key::VertexSize => (
                "Vertex size (px)",
                "Size of the vertex handles in Vertex mode",
            ),
            Key::HandleScale => (
                "Handle size",
                "Scale of the move / rotate / scale handles (1 = 64 px arms)",
            ),
            Key::GridStep => (
                "Grid step",
                "Grid snap step the builder starts with ([ / ] halve / double it while editing)",
            ),
            Key::RotSnap => (
                "Rotate snap (°)",
                "Rotate handle snap step while grid snapping is on (0 = free)",
            ),
            Key::SnapGrid => ("Grid snap on", "Start with grid snapping enabled"),
            Key::ColorEdge => ("Edges", "Wireframe color"),
            Key::ColorEdgeSelected => ("Selected edges", "Color of selected edges"),
            Key::ColorEdgeHover => (
                "Hovered edge",
                "Color of the edge under the cursor (Edge mode)",
            ),
            Key::ColorVertex => ("Vertices", "Vertex handle color"),
            Key::ColorVertexSelected => ("Selected vertices", "Color of selected vertices"),
            Key::ColorVertexHover => (
                "Hovered vertex",
                "Color of the vertex under the cursor (Vertex mode)",
            ),
            Key::ColorFaceSelected => ("Selected faces", "Tint of selected faces"),
            Key::ColorFaceHover => ("Hovered face", "Tint of the face under the cursor"),
            Key::LightmapTexel => (
                "Lightmap texel",
                "Lightmap UV2s are unwrapped at this texel size (world units) on Bake to MeshInstance3D and glTF export. 0 = skip",
            ),
            Key::ShiftMode => (
                "Shift selects",
                "What Shift+click and Shift+box do to the selection: Add the elements, Subtract them, or Toggle each (Modeler).\nLayout default: MST layout Shift+click toggles and Shift+box adds (the builder's classic behaviour); Modeler layout adds",
            ),
            Key::RectMode => (
                "Box selects",
                "Intersect: every element the box touches; Complete: only elements entirely inside the box (Modeler).\nLayout default: Complete in the MST layout (the builder's classic box), Intersect in the Modeler layout",
            ),
            Key::Hover => (
                "Hover highlight",
                "Highlight the face, edge or vertex under the cursor",
            ),
            Key::ShowSceneInfo => (
                "Show scene info",
                "Vertex / edge / face counts and the selected counts in the viewport",
            ),
            Key::ShowDimensions => (
                "Show dimensions",
                "Size of the selection's bounding box (the whole mesh in Object mode) next to it in the viewport",
            ),
            Key::AdjustPanel => (
                "Adjust panel",
                "After Extrude, Inset, Bevel, Subdivide, Weld, Fill, Offset, Mirror … a panel at the bottom of the viewport re-runs the action live with new settings (Done / Enter commits, Cancel / Esc restores). Off: actions apply at once",
            ),
        }
    }
}

impl Prefs {
    /// Effective Shift mode (0 add, 1 subtract, 2 toggle) for a click (`boxed` false) or a box,
    /// with the layout default when the user has not chosen one.
    pub fn shift_for(&self, modeler: bool, boxed: bool) -> i64 {
        match self.shift_mode {
            0..=2 => self.shift_mode,
            _ if modeler || boxed => 0,
            _ => 2,
        }
    }
    /// Effective box mode (0 intersect, 1 complete).
    pub fn rect_for(&self, modeler: bool) -> i64 {
        match self.rect_mode {
            0..=1 => self.rect_mode,
            _ if modeler => 0,
            _ => 1,
        }
    }
    pub fn get_f(&self, k: Key) -> f32 {
        match k {
            Key::VertexSize => self.vertex_size,
            Key::HandleScale => self.handle_scale,
            Key::GridStep => self.grid_step,
            Key::RotSnap => self.rot_snap,
            Key::LightmapTexel => self.lightmap_texel,
            // Panel index: the "Layout default" entry is the last one.
            Key::ShiftMode => {
                if self.shift_mode < 0 {
                    3.0
                } else {
                    self.shift_mode as f32
                }
            }
            Key::RectMode => {
                if self.rect_mode < 0 {
                    2.0
                } else {
                    self.rect_mode as f32
                }
            }
            _ => self.fetch_truth(k) as i32 as f32,
        }
    }
    pub fn fetch_truth(&self, k: Key) -> bool {
        match k {
            Key::SnapGrid => self.snap_grid,
            Key::Hover => self.hover,
            Key::ShowSceneInfo => self.show_scene_info,
            Key::ShowDimensions => self.show_dimensions,
            Key::AdjustPanel => self.adjust_panel,
            _ => false,
        }
    }
    pub fn fetch_tint(&self, k: Key) -> Color {
        match k {
            Key::ColorEdge => self.color_edge,
            Key::ColorEdgeSelected => self.color_edge_selected,
            Key::ColorEdgeHover => self.color_edge_hover,
            Key::ColorVertex => self.color_vertex,
            Key::ColorVertexSelected => self.color_vertex_selected,
            Key::ColorVertexHover => self.color_vertex_hover,
            Key::ColorFaceSelected => self.color_face_selected,
            Key::ColorFaceHover => self.color_face_hover,
            _ => Color::WHITE,
        }
    }
    /// Sets `k` from a float, int, bool or Color variant (whatever the control emits).
    pub fn assign(&mut self, k: Key, v: &Variant) {
        let f = v
            .try_to::<f64>()
            .ok()
            .or_else(|| v.try_to::<i64>().ok().map(|i| i as f64))
            .map(|x| x as f32);
        let b = v.try_to::<bool>().ok().or(f.map(|x| x >= 0.5));
        let c = v.try_to::<Color>().ok();
        match k {
            Key::VertexSize => self.vertex_size = f.unwrap_or(self.vertex_size).clamp(2.0, 24.0),
            Key::HandleScale => self.handle_scale = f.unwrap_or(self.handle_scale).clamp(0.5, 3.0),
            Key::GridStep => self.grid_step = f.unwrap_or(self.grid_step).clamp(0.0624, 64.0),
            Key::RotSnap => self.rot_snap = f.unwrap_or(self.rot_snap).clamp(0.0, 90.0),
            Key::SnapGrid => self.snap_grid = b.unwrap_or(self.snap_grid),
            Key::Hover => self.hover = b.unwrap_or(self.hover),
            Key::ShowSceneInfo => self.show_scene_info = b.unwrap_or(self.show_scene_info),
            Key::ShowDimensions => self.show_dimensions = b.unwrap_or(self.show_dimensions),
            Key::AdjustPanel => self.adjust_panel = b.unwrap_or(self.adjust_panel),
            Key::ShiftMode => {
                self.shift_mode = f
                    .map(|x| x as i64)
                    .map(|x| if !(0..=2).contains(&x) { -1 } else { x })
                    .unwrap_or(self.shift_mode)
            }
            Key::RectMode => {
                self.rect_mode = f
                    .map(|x| x as i64)
                    .map(|x| if !(0..=1).contains(&x) { -1 } else { x })
                    .unwrap_or(self.rect_mode)
            }
            Key::ColorEdge => self.color_edge = c.unwrap_or(self.color_edge),
            Key::ColorEdgeSelected => {
                self.color_edge_selected = c.unwrap_or(self.color_edge_selected)
            }
            Key::ColorEdgeHover => self.color_edge_hover = c.unwrap_or(self.color_edge_hover),
            Key::ColorVertex => self.color_vertex = c.unwrap_or(self.color_vertex),
            Key::ColorVertexSelected => {
                self.color_vertex_selected = c.unwrap_or(self.color_vertex_selected)
            }
            Key::ColorVertexHover => self.color_vertex_hover = c.unwrap_or(self.color_vertex_hover),
            Key::ColorFaceSelected => {
                self.color_face_selected = c.unwrap_or(self.color_face_selected)
            }
            Key::ColorFaceHover => self.color_face_hover = c.unwrap_or(self.color_face_hover),
            Key::LightmapTexel => {
                self.lightmap_texel = f.unwrap_or(self.lightmap_texel).clamp(0.0, 16.0)
            }
        }
    }
    pub fn worth(&self, k: Key) -> Variant {
        match k.sort() {
            Kind::Bool => self.fetch_truth(k).to_variant(),
            Kind::Choice(_) => (self.get_f(k) as i64).to_variant(),
            Kind::Number(..) => (self.get_f(k) as f64).to_variant(),
            Kind::Color => self.fetch_tint(k).to_variant(),
        }
    }

    /// Loads the project's preferences (defaults for missing keys).
    pub fn ingest() -> Prefs {
        let mut p = Prefs::default();
        let Some(es) = EditorInterface::singleton().get_editor_settings() else {
            return p;
        };
        for k in Key::ALL {
            // Missing keys come back as the "" default (a nil default makes Godot print an error).
            let v = es
                .get_project_metadata_ex(SECTION, k.moniker())
                .default(&"".to_variant())
                .done();
            if !v.is_nil() && v.try_to::<GString>().is_err() {
                p.assign(k, &v);
            }
        }
        p
    }

    /// Stores one key (a changed row). `set_project_metadata` rewrites the metadata file on
    /// every call, so a drag on a row writes one key per change, not all of them.
    pub fn save_key(&self, k: Key) {
        let Some(mut es) = EditorInterface::singleton().get_editor_settings() else {
            return;
        };
        es.set_project_metadata(SECTION, k.moniker(), &self.worth(k));
    }

    /// Stores every key (Reset stores the defaults).
    pub fn save(&self) {
        let Some(mut es) = EditorInterface::singleton().get_editor_settings() else {
            return;
        };
        for k in Key::ALL {
            es.set_project_metadata(SECTION, k.moniker(), &self.worth(k));
        }
    }
}

/// Keys that are not rebindable (modal tools) and the mouse gestures.
const FIXED_KEYS: &str = "[b]Fixed[/b]  Esc back / cancel / clear · Enter finish (knife, wall, poly, Bezier, adjust panel Done) · hold V vertex snap · double-click loop
[b]Mouse[/b]  Shift+click / Shift+box: see Shift selects · right-click a selected element: context menu
[b]UV editor[/b]  drag move · Ctrl+drag scale (Shift per axis) · Alt+drag rotate · middle pan · wheel zoom";

/// The shortcut list with the *current* bindings (Editor Settings › Shortcuts › Mst Builder).
pub fn shortcut_text() -> String {
    let mut rows: Vec<String> = super::shortcuts::ENTRIES
        .iter()
        .map(|e| {
            let k = super::shortcuts::key_text(e.id);
            format!(
                "{} [b]{}[/b]",
                e.moniker,
                if k.is_empty() { "—" } else { k.as_str() }
            )
        })
        .collect();
    rows.dedup();
    format!("{}\n{FIXED_KEYS}", rows.join(" · "))
}

/// The preferences popup and what `sync_panel` updates.
pub struct Panel {
    pub popup: Gd<PopupPanel>,
    pub controls: Vec<(Key, Gd<godot::classes::Control>)>,
    pub keys: Gd<RichTextLabel>,
    /// UI layout choice (MST / modeler).
    pub layout: Gd<OptionButton>,
    /// The scrolling body; its height is fitted to the editor window on each `unveil`.
    scroll: Gd<ScrollContainer>,
}

/// Preferred size of the scrolling body (unscaled); the height shrinks to fit small windows.
const BODY: Vector2 = Vector2::new(460.0, 640.0);

/// Body height that keeps the popup (body + panel margins) inside a `window_h` tall editor:
/// the preferred height, else what fits, never below a few rows (the body scrolls).
pub fn body_height(window_h: f32, scale: f32) -> f32 {
    let want = BODY.y * scale;
    let margin = 64.0 * scale;
    want.min(window_h - margin).max(160.0 * scale)
}

impl Panel {
    /// Pops the panel up centred, with the body height fitted to the editor window so the whole
    /// popup stays on screen; the rest scrolls.
    pub fn unveil(&mut self) {
        let s = crate::editor::ui_kit::editor_scale();
        let window_h = EditorInterface::singleton()
            .get_base_control()
            .map(|c| c.get_size().y)
            .filter(|h| *h > 0.0)
            .unwrap_or(BODY.y * s + 64.0 * s);
        self.scroll
            .set_custom_minimum_size(Vector2::new(BODY.x * s, body_height(window_h, s)));
        self.popup.reset_size();
        self.popup.popup_centered();
    }
}

/// Builds the preferences popup; controls call `on_pref(value, key)` and `on_prefs_reset()` on
/// `target`.
pub fn build_panel(addressee: &Gd<Object>, p: &Prefs) -> Panel {
    let s = crate::editor::ui_kit::editor_scale();
    let mut popup = PopupPanel::new_alloc();
    popup.set_name("PixelBuilderPreferences");
    popup.set_title("Pixel Builder Preferences");
    let mut scroll = ScrollContainer::new_alloc();
    scroll.set_custom_minimum_size(BODY * s);
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
        let (text, tip) = k.portray();
        let mut l = Label::new_alloc();
        l.set_text(text);
        l.set_tooltip_text(tip);
        l.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
        l.set_custom_minimum_size(Vector2::new(150.0 * s, 0.0));
        grid.add_child(&l);
        let c = fabricate_widget(addressee, k, p);
        let mut c = c;
        c.set_h_size_flags(SizeFlags::EXPAND_FILL);
        c.set_tooltip_text(tip);
        grid.add_child(&c);
        controls.push((k, c));
    }
    // UI layout (per-user Editor Setting, not a project preference: Reset leaves it alone).
    let mut l = Label::new_alloc();
    l.set_text("UI layout");
    l.set_tooltip_text(super::layout::TOOLTIP);
    l.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
    grid.add_child(&l);
    let mut layout = OptionButton::new_alloc();
    layout.set_name("UiLayout");
    for n in super::layout::NAMES {
        layout.add_item(n);
    }
    layout.select(super::layout::Layout::ingest() as i32);
    layout.set_focus_mode(FocusMode::NONE);
    layout.set_tooltip_text(super::layout::TOOLTIP);
    layout.set_h_size_flags(SizeFlags::EXPAND_FILL);
    layout.connect(
        "item_selected",
        &Callable::from_object_method(addressee, "on_layout_choice"),
    );
    grid.add_child(&layout);
    col.add_child(&grid);
    let mut reset = Button::new_alloc();
    reset.set_text("Reset to Defaults");
    reset.set_focus_mode(FocusMode::NONE);
    reset.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_prefs_reset"),
    );
    col.add_child(&reset);
    col.add_child(&HSeparator::new_alloc());
    let mut sh = Label::new_alloc();
    sh.set_text("Shortcuts (3D viewport, builder mesh selected)");
    sh.set_tooltip_text("Rebind them in Editor > Editor Settings > Shortcuts > Mst Builder");
    sh.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
    col.add_child(&sh);
    let mut rt = RichTextLabel::new_alloc();
    rt.set_name("Shortcuts");
    rt.set_use_bbcode(true);
    rt.set_fit_content(true);
    rt.set_text(&shortcut_text());
    rt.set_h_size_flags(SizeFlags::EXPAND_FILL);
    col.add_child(&rt);
    scroll.add_child(&col);
    popup.add_child(&scroll);
    Panel {
        popup,
        controls,
        keys: rt,
        layout,
        scroll,
    }
}

/// The editing control of preference `k`, bound to `on_pref(value, k)` of `target` (the
/// preferences popup and the modeler Options dialog / Selection & Handles popover).
pub fn fabricate_widget(addressee: &Gd<Object>, k: Key, p: &Prefs) -> Gd<godot::classes::Control> {
    let cb = Callable::from_object_method(addressee, "on_pref").bind(&[(k as i64).to_variant()]);
    let mut c: Gd<godot::classes::Control> = match k.sort() {
        Kind::Bool => {
            let mut c = CheckBox::new_alloc();
            c.set_pressed_no_signal(p.fetch_truth(k));
            c.connect("toggled", &cb);
            c.upcast()
        }
        Kind::Choice(names) => {
            let mut ob = OptionButton::new_alloc();
            for n in names {
                ob.add_item(*n);
            }
            ob.select(p.get_f(k) as i32);
            ob.set_focus_mode(FocusMode::NONE);
            ob.connect("item_selected", &cb);
            ob.upcast()
        }
        Kind::Number(min, max, step) => {
            let mut sb = SpinBox::new_alloc();
            sb.set_min(min);
            sb.set_max(max);
            sb.set_step(step);
            sb.set_value_no_signal(p.get_f(k) as f64);
            sb.connect("value_changed", &cb);
            sb.upcast()
        }
        Kind::Color => {
            let s = crate::editor::ui_kit::editor_scale();
            let mut c = ColorPickerButton::new_alloc();
            c.set_pick_color(p.fetch_tint(k));
            c.set_custom_minimum_size(Vector2::new(72.0 * s, 24.0 * s));
            c.connect("color_changed", &cb);
            c.upcast()
        }
    };
    c.set_tooltip_text(k.portray().1);
    c
}

/// Mirrors preference `k` into its control (no signals).
pub fn sync_control(c: &Gd<godot::classes::Control>, k: Key, p: &Prefs) {
    if let Ok(mut sb) = c.clone().try_cast::<SpinBox>() {
        sb.set_value_no_signal(p.get_f(k) as f64);
    } else if let Ok(mut cb) = c.clone().try_cast::<CheckBox>() {
        cb.set_pressed_no_signal(p.fetch_truth(k));
    } else if let Ok(mut ob) = c.clone().try_cast::<OptionButton>() {
        ob.select(p.get_f(k) as i32);
    } else if let Ok(mut cp) = c.clone().try_cast::<ColorPickerButton>() {
        cp.set_pick_color(p.fetch_tint(k));
    }
}

static HANDLE_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000); // 1.0

/// Current handle scale (read by the gizmo drawing / picking code).
pub fn handle_scale() -> f32 {
    f32::from_bits(HANDLE_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn set_handle_scale(v: f32) {
    HANDLE_SCALE.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Mirrors `p` into the panel controls without signals (and the current shortcut keys).
pub fn sync_panel(panel: &mut Panel, p: &Prefs) {
    for (k, c) in &panel.controls {
        sync_control(c, *k, p);
    }
    if panel.layout.is_instance_valid() {
        panel.layout.select(super::layout::Layout::ingest() as i32);
    }
    if panel.keys.is_instance_valid() {
        panel.keys.set_text(&shortcut_text());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn body_fits_small_windows() {
        // Tall window: the preferred 640 px.
        assert_eq!(super::body_height(900.0, 1.0), 640.0);
        // 600 px window: the popup (body + margins) stays inside it.
        assert_eq!(super::body_height(600.0, 1.0), 536.0);
        // Tiny window: a few rows, the rest scrolls.
        assert_eq!(super::body_height(100.0, 1.0), 160.0);
        assert_eq!(super::body_height(1500.0, 2.0), 1280.0);
    }
}
