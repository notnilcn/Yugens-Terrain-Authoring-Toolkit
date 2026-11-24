//! Builds the terrain editor's docked controls and opens the dialogs they lead to.
//!
//! These controls are created in code because which of them exist is not known while
//! authoring: the palette and every settings row come from the tool and setting tables,
//! the bar is rebuilt each time a tool is picked and when the terrain's grid or storage
//! choices change which settings apply, and the slot panel follows the texture set of
//! whichever terrain is selected. The file dialog exists only while the author is choosing
//! a file. The terrain's optional helper nodes are added only when the author asks for one,
//! and then become part of the scene being edited. Dialogs whose layout is fixed are
//! authored as scenes, and code only decides when they open and what they start out showing.
use super::mst_value_label;
use super::save_preset_dialog::MstSavePresetDialog;
use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::{
    self, BRUSH_MODES, BRUSH_TYPES, EXTRA_TOOLS, GROUP_STARTS, Kind, MERGE_MODES, QUICK_PAINTS,
    SMOOTH_MODES, Setting, TINT_COLORS, TOOLS, Tool,
};
use super::ui_kit::{self, Strip, sz};
use crate::terrain::textures::{
    MarchingSquaresTexturePreset, MarchingSquaresTextureSlot, SLOT_COUNT,
};
use godot::classes::control::SizeFlags;
use godot::classes::{
    Button, ButtonGroup, CheckBox, ColorPickerButton, Control,
    DirAccess, EditorInterface, EditorResourcePicker, EditorSpinSlider, HBoxContainer, HSeparator,
    HSlider, ImageTexture, Label, LineEdit, OptionButton, ResourceLoader, ResourceSaver, SpinBox,
    TextureRect, VBoxContainer,
};
use crate::terrain::MarchingSquaresTerrain;
use godot::prelude::*;
pub(super) enum PresetSave {
    Done,
    AlreadyExists(String),
}
pub const PRESET_DIR: &str = "res://addons/MarchingSquaresTerrain/texture_presets/";
const PRESET_META: &str = "mst_preset_path";
const AUTO_RULES_SHOWN_META: &str = "mst_auto_rules_shown";
const COMPACT_BUTTON: &str = "ToolbarCompact";
const TOOLBAR_META: (&str, &str) = ("marching_squares_terrain", "toolbar_compact");
fn toolbar_compact() -> bool {
    EditorInterface::singleton()
        .get_editor_settings()
        .map(|es| {
            es.get_project_metadata_ex(TOOLBAR_META.0, TOOLBAR_META.1)
                .default(&false.to_variant())
                .done()
                .booleanize()
        })
        .unwrap_or(false)
}
fn compact_icon(compact: bool) -> Option<Gd<godot::classes::Texture2D>> {
    let theme = EditorInterface::singleton().get_editor_theme()?;
    theme.get_icon(if compact { "Forward" } else { "Back" }, "EditorIcons")
}
const PATTERNS: [&str; 14] = [
    "Grass",
    "Meadow",
    "Forest",
    "Sand",
    "Dirt",
    "Cliff",
    "Dark Rock",
    "Snow",
    "Gravel",
    "Clay",
    "Moss",
    "Path",
    "Plain",
    "Void",
];
impl MarchingSquaresTerrainPlugin {
    fn cb(&self, method: &str) -> Callable {
        Callable::from_object_method(&self.to_gd(), method)
    }
    fn cb1(&self, method: &str, arg: impl ToGodot) -> Callable {
        self.cb(method).bind(&[arg.to_variant()])
    }
    pub(super) fn build_toolbar(&mut self) -> ui_kit::Toolbar {
        let mut tb = ui_kit::Toolbar::new("MarchingSquaresToolbar");
        let compact = toolbar_compact();
        self.compact_button = Some(tb.deed(
            COMPACT_BUTTON,
            "Collapse",
            "Show / hide the tool names (icons only).",
            compact_icon(compact),
            &self.cb("on_toolbar_compact"),
        ));
        tb.separator();
        let all = TOOLS.iter().enumerate().chain(
            EXTRA_TOOLS
                .iter()
                .enumerate()
                .map(|(k, t)| (TOOLS.len() + k, t)),
        );
        for (i, t) in all {
            if GROUP_STARTS.contains(&i) || i == TOOLS.len() {
                tb.separator();
            }
            let key = t.token().map(|k| k.to_string());
            let b = tb.tool(
                t.moniker(),
                t.label(),
                key.as_deref(),
                &t.tooltip(),
                t.glyph(),
                &self.cb1("on_tool", i as i64),
            );
            self.tool_buttons.push(b);
        }
        tb.separator();
        self.grid_button = Some(self.add_grid_toggle(&mut tb));
        tb.assign_terse(compact);
        tb
    }
    pub(super) fn toggle_toolbar_compact(&mut self) {
        let compact = !toolbar_compact();
        if let Some(mut es) = EditorInterface::singleton().get_editor_settings() {
            es.set_project_metadata(TOOLBAR_META.0, TOOLBAR_META.1, &compact.to_variant());
        }
        if let Some(tb) = &mut self.toolbar {
            tb.assign_terse(compact);
        }
        if let Some(b) = self.compact_button.as_mut().filter(|b| b.is_instance_valid())
            && let Some(i) = compact_icon(compact)
        {
            b.set_button_icon(&i);
        }
    }
    pub(super) fn select_tool(&mut self, t: Tool) {
        self.finish_stroke();
        let t = match t {
            Tool::Brush | Tool::RaiseLower if self.state.raise_mode => Tool::RaiseLower,
            Tool::Brush | Tool::RaiseLower => Tool::Brush,
            t => t,
        };
        self.state.tool = t;
        self.selection.clear();
        self.extra.pasting = false;
        self.foliage.note = None;
        if let Some(i) = TOOLS.iter().position(|x| *x == t.button()).or_else(|| {
            EXTRA_TOOLS
                .iter()
                .position(|x| *x == t)
                .map(|k| TOOLS.len() + k)
        }) {
            for (j, b) in self.tool_buttons.iter_mut().enumerate() {
                if b.is_instance_valid() {
                    b.set_pressed_no_signal(j == i);
                }
            }
        }
        if t == Tool::ChunkManager
            && let Some(c) = self.selected_chunk
            && let Some(m) = self
                .terrain
                .as_ref()
                .and_then(|tr| tr.bind().merge_mode_of(c))
        {
            self.state.merge_mode = m;
        }
        let side = matches!(t, Tool::VertexPaint | Tool::Scatter) && self.visible;
        if let Some(p) = &mut self.texture_panel {
            p.set_visible(side);
        }
        if t == Tool::VertexPaint {
            self.rebuild_texture_panel();
        } else if t == Tool::Scatter {
            self.rebuild_foliage_panel();
        }
        self.rebuild_strip();
        self.redraw();
        self.base().update_overlays();
    }
    pub(super) fn size_scale(&self) -> f64 {
        let Some(t) = &self.terrain else { return 1.0 };
        let t = t.bind();
        let cfg = &t.store().config;
        let dim = cfg.chunk_size as f64 + 1.0;
        let cs = cfg.cell_size_safe();
        ((dim / 33.0 + dim / 33.0) / 2.0).clamp(0.5, 2.0)
            * ((cs.x + cs.y) as f64 / 4.0).clamp(0.3005, 1.0)
            * if cfg.grid.is_cell_mode() { 2.0 } else { 1.0 }
    }
    pub(super) fn rebuild_strip(&mut self) {
        let Some(root) = self.peel.clone() else {
            return;
        };
        self.follow_history_commits();
        let mut strip = Strip::reassemble(root);
        self.setting_controls.clear();
        if self.state.tool == Tool::Stamp {
            self.stamp_library();
        }
        self.chunk_list = None;
        let tool = self.state.tool;
        let mut title = strip.heading(tool.moniker(), ui_kit::glyph(tool.glyph()), &tool.tooltip());
        title.set_name("ToolTitle");
        match self.state.tool {
            Tool::ChunkManager => self.build_chunk_strip(&mut strip),
            Tool::TerrainSettings => self.build_settings_strip(&mut strip),
            t => {
                for &s in tools::settings_for(t) {
                    let c = self.build_setting(&mut strip, s, t);
                    let mut cc = c.clone();
                    cc.set_tooltip_text(tools::tooltip(s));
                    cc.set_name(&ui_kit::node_name(tools::portray(s, t).0));
                    if let Ok(r) = c.clone().try_cast::<godot::classes::Range>() {
                        echo_to_readouts(&r);
                    }
                    self.setting_controls.push((s, c));
                }
            }
        }
        self.refresh_dependent_states();
    }
    fn follow_history_commits(&mut self) {
        let cb = self.base().callable("on_history_changed");
        if let Some(mut ur) = self.base_mut().get_undo_redo()
            && !ur.is_connected("history_changed", &cb)
        {
            ur.connect_flags(
                "history_changed",
                &cb,
                godot::classes::object::ConnectFlags::DEFERRED,
            );
        }
    }
    fn build_setting(&self, strip: &mut Strip, s: Setting, t: Tool) -> Gd<Control> {
        let (label, kind) = tools::portray(s, t);
        let v = self.state.fetch(s);
        let id = s as i64;
        match kind {
            Kind::HSlider(min, max, step) => {
                let (min, max) = if s == Setting::Size {
                    (min * self.size_scale().min(1.0), max * self.size_scale())
                } else {
                    (min, max)
                };
                strip
                    .hslider(label, min, max, step, v, &self.cb1("on_float", id))
                    .upcast()
            }
            Kind::Spin(min, max, step) => strip
                .spin_slider(label, min, max, step, v, &self.cb1("on_float", id))
                .upcast(),
            Kind::Check => strip
                .checkbox(label, v != 0.0, &self.cb1("on_bool", id))
                .upcast(),
            Kind::Text => {
                let mut le = strip.line_edit(label, "New name here...", &self.cb1("on_text", id));
                le.set_text("");
                le.upcast()
            }
            Kind::Button(text) => strip.button("", text, &self.cb1("upon_deed", id)).upcast(),
            Kind::Option => {
                let names: Vec<String> = match s {
                    Setting::BrushType => BRUSH_TYPES.iter().map(|x| x.to_string()).collect(),
                    Setting::BrushMode => BRUSH_MODES.iter().map(|x| x.to_string()).collect(),
                    Setting::Material => self.slot_names(),
                    Setting::QuickPaint => {
                        QUICK_PAINTS.iter().map(|q| q.moniker.to_string()).collect()
                    }
                    Setting::TexturePreset => {
                        let mut v = vec!["None".to_string()];
                        v.extend(self.list_presets().into_iter().map(|p| p.1));
                        v
                    }
                    Setting::SmoothMode => SMOOTH_MODES.iter().map(|x| x.to_string()).collect(),
                    Setting::StampImage => self.stamp_names(),
                    Setting::StampMode => pt_core::stamp::STAMP_MODES_ALL
                        .iter()
                        .map(|x| x.to_string())
                        .collect(),
                    Setting::TintColor => TINT_COLORS.iter().map(|x| x.0.to_string()).collect(),
                    _ => Vec::new(),
                };
                let refs: Vec<&str> = names.iter().map(|x| x.as_str()).collect();
                let wide = matches!(
                    s,
                    Setting::TexturePreset
                        | Setting::QuickPaint
                        | Setting::Material
                        | Setting::BrushMode
                        | Setting::SmoothMode
                        | Setting::StampImage
                );
                let selected = if s == Setting::TexturePreset {
                    self.current_preset_index()
                } else {
                    v as i32
                };
                let mut ob = strip.choice(
                    label,
                    &refs,
                    selected,
                    if wide { 100.0 } else { 65.0 },
                    &self.cb1("on_option", id),
                );
                ob.set_name(&ui_kit::node_name(label));
                if s == Setting::QuickPaint {
                    ob.set_item_tooltip(
                        3,
                        "Paints the selected Material (Vertex Painting) on the ground.",
                    );
                }
                if s == Setting::BrushMode {
                    for (i, icon) in ["brush_tool.svg", "raise_lower_tool.svg"]
                        .into_iter()
                        .enumerate()
                    {
                        if let Some(ic) = ui_kit::glyph(icon) {
                            ob.set_item_icon(i as i32, &ic);
                        }
                    }
                }
                if s == Setting::TintColor {
                    for (i, (_, c)) in TINT_COLORS.iter().enumerate() {
                        if let Some(ic) = tint_swatch(*c) {
                            ob.set_item_icon(i as i32, &ic);
                        }
                    }
                }
                if s == Setting::StampImage {
                    for i in 0..names.len() {
                        if let Some(ic) = self.stamp_icon(i) {
                            ob.set_item_icon(i as i32, &ic);
                        }
                    }
                }
                ob.upcast()
            }
        }
    }
    pub(super) fn set_setting(&mut self, s: Setting, v: f64) {
        self.state.assign(s, v);
        let v = self.state.fetch(s);
        for (id, c) in &self.setting_controls {
            if *id == s {
                show_on_widget(c, &v.to_variant());
            }
        }
        self.refresh_dependent_states();
        self.base().update_overlays();
    }
    pub(super) fn refresh_dependent_states(&mut self) {
        let cell = self
            .terrain
            .as_ref()
            .is_some_and(|t| t.bind().store().config.grid.is_cell_mode());
        let aligned_ok = cell && !self.state.brush.falloff;
        let aligned = self.state.brush.grid_aligned && aligned_ok;
        for (id, c) in &mut self.setting_controls {
            match id {
                Setting::GridAligned => {
                    ui_kit::assign_active(c, aligned_ok);
                    c.set_tooltip_text(if aligned_ok {
                        tools::tooltip(Setting::GridAligned)
                    } else {
                        "Grid Aligned needs a Triangle/Hexagon/Hex Rings grid and Falloff off."
                    });
                }
                Setting::GridSize => ui_kit::assign_active(c, aligned),
                Setting::Falloff if self.state.tool == Tool::Bridge => {
                    ui_kit::assign_active(c, false)
                }
                Setting::Ceiling | Setting::Height | Setting::Floor
                    if self.state.tool == Tool::Stamp =>
                {
                    ui_kit::assign_active(c, self.state.stamp_limits)
                }
                _ => {}
            }
        }
    }
    pub(super) fn pocket_moniker(&self, i: usize) -> String {
        self.terrain
            .as_ref()
            .and_then(|t| t.bind().texture_set_ref())
            .map(|ts| ts.bind().pocket_moniker(i))
            .unwrap_or_else(|| format!("Texture {}", i + 1))
    }
    pub(super) fn slot_names(&self) -> Vec<String> {
        (0..SLOT_COUNT)
            .map(|i| format!("{}. {}", i + 1, self.pocket_moniker(i)))
            .collect()
    }
    pub(super) fn slot_names_stale(&self) -> bool {
        self.auto_rules_shown_stale()
            || self.slot_name_edits.iter().any(|(i, le)| {
                le.is_instance_valid()
                    && !le.has_focus()
                    && self
                        .slot_res(*i)
                        .is_some_and(|s| s.bind().display_name != le.get_text())
            })
    }
    pub(super) fn terrain_settings_snapshot(&self) -> Vec<Variant> {
        TERRAIN_SETTINGS
            .iter()
            .filter(|t| !t.prop.is_empty())
            .map(|t| self.terrain_prop(t.prop))
            .chain(std::iter::once(
                self.turf_stuff()
                    .map(|m| m.get_shader_parameter("animation_fps"))
                    .unwrap_or_default(),
            ))
            .collect()
    }
    pub(super) fn refresh_material_names(&mut self) {
        let names = self.slot_names();
        for (id, c) in &self.setting_controls {
            if *id == Setting::Material
                && let Ok(mut ob) = c.clone().try_cast::<OptionButton>()
            {
                for (i, n) in names.iter().enumerate() {
                    ob.set_item_text(i as i32, n);
                }
            }
        }
    }
    fn build_chunk_strip(&mut self, strip: &mut Strip) {
        let ob = strip.choice("Chunk", &[], -1, 130.0, &self.cb("on_chunk_selected"));
        let mut ob = ob;
        ob.set_tooltip_text("Chunk to edit: its merge mode, vertex handles and Remove act on it (or Ctrl+LMB a chunk in the viewport).");
        self.chunk_list = Some(ob.clone());
        self.refresh_chunk_list();
        let n = self
            .terrain
            .as_ref()
            .map(|t| t.bind().store().chunk_coords().len())
            .unwrap_or(0);
        strip.hint(&format!("{n} chunk{}", if n == 1 { "" } else { "s" }));
        let mut rm = strip.button("", "Remove Chunk", &self.cb("on_remove_selected_chunk"));
        rm.set_tooltip_text("Removes the selected chunk (undoable). In the viewport: LMB a chunk (red X) removes it, LMB a green + adds one.");
        ui_kit::assign_active(&mut rm, self.selected_chunk.is_some());
        strip.separator();
        let mut mm = strip.choice(
            "Merge Mode",
            &MERGE_MODES,
            self.state.merge_mode,
            120.0,
            &self.cb("on_merge_mode"),
        );
        mm.set_tooltip_text("Merge mode of the selected chunk (Ctrl+LMB a chunk to select it).");
        strip.separator();
        let mut b = strip.button(
            "",
            "Apply Mode to All Chunks",
            &self.cb("on_apply_merge_all"),
        );
        b.set_tooltip_text("Applies the merge mode to every chunk of this terrain.");
        strip.separator();
        let handles = self
            .terrain
            .as_ref()
            .is_some_and(|t| t.get("chunk_handles").to::<bool>());
        let mut h = strip.checkbox("Vertex Handles", handles, &self.cb("on_chunk_handles"));
        h.set_tooltip_text("Drag handles on every sample of the selected chunk (the original's chunk gizmo; one undo step per drag).");
        strip.separator();
        strip.hint("LMB +: add chunk  ·  LMB chunk: remove  ·  Ctrl+LMB: select");
    }
    pub(super) fn refresh_chunk_list(&mut self) {
        let Some(mut ob) = self.chunk_list.clone() else {
            return;
        };
        if !ob.is_instance_valid() {
            self.chunk_list = None;
            return;
        }
        ob.clear();
        let coords = self
            .terrain
            .as_ref()
            .map(|t| t.bind().store().chunk_coords())
            .unwrap_or_default();
        if self.selected_chunk.is_none_or(|s| !coords.contains(&s)) && !coords.is_empty() {
            let c = coords
                .iter()
                .copied()
                .find(|c| *c == pt_core::math::iv2(0, 0))
                .unwrap_or(coords[0]);
            self.selected_chunk = Some(c);
            if let Some(t) = self.terrain.as_mut() {
                t.set(
                    "handle_chunk",
                    &crate::math_conversions::from_iv(c).to_variant(),
                );
            }
            if let Some(m) = self
                .terrain
                .as_ref()
                .and_then(|t| t.bind().merge_mode_of(c))
            {
                self.state.merge_mode = m;
            }
        }
        for (i, c) in coords.iter().enumerate() {
            ob.add_item(&format!("Chunk ({}, {})", c.x, c.y));
            ob.set_item_metadata(i as i32, &Vector2i::new(c.x, c.y).to_variant());
        }
        let sel = self
            .selected_chunk
            .and_then(|s| coords.iter().position(|c| *c == s))
            .map(|i| i as i32)
            .unwrap_or(-1);
        ob.select(sel);
    }
    pub(super) fn select_chunk(&mut self, c: Option<pt_core::math::IVec2>) {
        self.selected_chunk = c;
        if let (Some(c), Some(t)) = (c, self.terrain.as_mut()) {
            t.set(
                "handle_chunk",
                &crate::math_conversions::from_iv(c).to_variant(),
            );
        }
        if let Some(c) = c
            && let Some(m) = self
                .terrain
                .as_ref()
                .and_then(|t| t.bind().merge_mode_of(c))
        {
            self.state.merge_mode = m;
        }
        if self.state.tool == Tool::ChunkManager {
            self.rebuild_strip_deferred();
        }
        self.redraw();
    }
    pub(super) fn rebuild_strip_deferred(&mut self) {
        self.to_gd().call_deferred("rebuild_strip_now", &[]);
    }
}
#[derive(Clone, Copy)]
enum TKind {
    Enum(&'static [&'static str]),
    Int(i64, i64),
    Float(f64, f64, f64),
    Unit,
    Vec2(f64, f64, f64),
    Vec2i(i64, i64),
    Bool,
    Slot,
    ExtraLayer,
    GrassFps,
    Text,
    Noise,
    Action(&'static str),
}
struct TSetting {
    label: &'static str,
    prop: &'static str,
    sort: TKind,
    tip: &'static str,
}
const fn ts(label: &'static str, prop: &'static str, kind: TKind, tip: &'static str) -> TSetting {
    TSetting {
        label,
        prop,
        sort: kind,
        tip,
    }
}
const TERRAIN_SETTINGS: [TSetting; 40] = [
    ts(
        "Grid Type",
        "grid_type",
        TKind::Enum(&["Square", "Triangle", "Hex", "Hex Rings"]),
        "Square = marching squares over vertex heights. Triangle / Hexagon / Hex Rings = flat cells.",
    ),
    ts(
        "Chunk Size",
        "chunk_size",
        TKind::Int(2, 64),
        "Cells per chunk edge (the original's Dimensions).",
    ),
    ts(
        "Ring Radius",
        "ring_radius",
        TKind::Int(1, 16),
        "Hex Rings: radius of the hex disk each chunk owns.",
    ),
    ts(
        "Cell Size",
        "cell_size",
        TKind::Vec2(0.1005, 64.0, 0.1005),
        "Cell size in world units (hex/triangle cells use X only).",
    ),
    ts(
        "Wrap Chunks",
        "wrap_chunks",
        TKind::Vec2i(0, 128),
        "Hex Rings: torus wrap in chunks (x = columns, y = rows); 0 = no wrap.",
    ),
    ts(
        "Blend Mode",
        "blend_mode",
        TKind::Enum(&["Smooth", "Hard Squares", "Hard Vertices"]),
        "Texture blending between slots.",
    ),
    ts(
        "Merge Mode",
        "default_merge_mode",
        TKind::Enum(&MERGE_MODES),
        "Merge mode given to newly added chunks.",
    ),
    ts(
        "Edge Style",
        "hex_edge_style",
        TKind::Enum(&["Flat Walls", "Terraces", "Slopes", "Hex Prism"]),
        "Triangle / hex grids: steps up to the merge threshold become ramps (Flat Walls, Slopes) or stepped terraces (Terraces); larger steps are vertical cliffs, or sloped rock faces with Slopes. Hex Prism: stacked prism columns with flat tops, bevelled rims and a groove per level height; Bridge-tool (smooth) cells stay slanted planes.",
    ),
    ts(
        "Instanced",
        "hex_edge_instanced",
        TKind::Bool,
        "Hex grids with Hex Prism: draw the columns as GPU instances (one MultiMesh instance per cell, rebuilt in the vertex shader from the cell map) instead of a meshed chunk; height edits only re-upload the map.",
    ),
    ts(
        "Floor Slot",
        "default_floor_slot",
        TKind::Slot,
        "Floor texture of newly added chunks.",
    ),
    ts(
        "Wall Slot",
        "default_wall_slot",
        TKind::Slot,
        "Wall texture of newly added chunks.",
    ),
    ts(
        "Noise Amp",
        "new_chunk_noise_amplitude",
        TKind::Float(0.0, 256.0, 0.1005),
        "Height amplitude of fractal noise for new chunks (0 = flat).",
    ),
    ts(
        "Noise Freq",
        "new_chunk_noise_frequency",
        TKind::Float(0.0, 4.0, 0.0015),
        "Frequency of the new-chunk noise.",
    ),
    ts(
        "Noise Seed",
        "noise_seed",
        TKind::Int(-2147483648, 2147483647),
        "Seed of the new-chunk noise.",
    ),
    ts(
        "Noise Hmap",
        "noise_hmap",
        TKind::Noise,
        "Noise resource for new chunks (the original's noise_hmap): heights = noise x Noise Height. Overrides Noise Amp / Freq.",
    ),
    ts(
        "Noise Height",
        "noise_hmap_height",
        TKind::Float(0.0, 1024.0, 0.5),
        "Height range of Noise Hmap (the original's dimensions.y).",
    ),
    ts(
        "Extra Collision Layer",
        "impact_stratum",
        TKind::ExtraLayer,
        "Adds one extra physics layer (9-32) to the terrain collision.",
    ),
    ts(
        "Grass",
        "grass_enabled",
        TKind::Bool,
        "Grow grass on slots with \"Has grass\".",
    ),
    ts(
        "Animation FPS",
        "",
        TKind::GrassFps,
        "> 0 animates the grass with limited fps; 0 = smooth wind.",
    ),
    ts(
        "Subdivisions",
        "grass_subdivisions",
        TKind::Int(1, 8),
        "Grass blades per cell = subdivisions².",
    ),
    ts(
        "Grass Size",
        "grass_size",
        TKind::Vec2(0.0505, 16.0, 0.0505),
        "Grass blade size (width, height).",
    ),
    ts(
        "Ledge Cutoff",
        "grass_ledge_threshold",
        TKind::Unit,
        "Grass is not planted on cliff-edge bands stronger than this.",
    ),
    ts(
        "Min Height",
        "grass_min_height",
        TKind::Float(-1024.0, 1024.0, 0.1005),
        "No grass below this height (e.g. your water level).",
    ),
    ts(
        "Pixel Size",
        "pixel_size",
        TKind::Int(1, 16),
        "Pixel size multiplier in the pixel renderer (type a larger value for bigger blocks).",
    ),
    ts(
        "Editor Grid",
        "show_grid_in_editor",
        TKind::Bool,
        "Cell lines on the terrain while editing (in game: show_grid).",
    ),
    ts(
        "Chunk Lines",
        "grid_chunk_lines",
        TKind::Bool,
        "The grid overlay also draws chunk borders.",
    ),
    ts(
        "Editor Fog",
        "fog_in_editor",
        TKind::Bool,
        "Show the fog of war (cell data) while editing.",
    ),
    ts(
        "Debug View",
        "debug_view",
        TKind::Enum(&[
            "Off",
            "Slope",
            "Height Contours",
            "Slot Ids",
            "Chunk Borders",
            "Cell Ids",
            "Cell Data",
        ]),
        "Terrain debug view (shader).",
    ),
    ts(
        "LOD Distance",
        "lod_distance",
        TKind::Float(0.0, 4000.0, 1.0),
        "Distant low-detail ring: chunks farther than this from both the LOD focus (lod_focus, else ahead of the camera) and the camera (the line between them) get flat cell tops, plain walls, no grass and no drawn props (collision stays). 0 = off.",
    ),
    ts(
        "LOD In Editor",
        "lod_in_editor",
        TKind::Bool,
        "Apply the distant LOD ring in the editor too (off: the editor always shows full detail).",
    ),
    ts(
        "Erosion",
        "",
        TKind::Action("erosion"),
        "Select this terrain's MarchingSquaresErosion child, adding one if there is none (undoable): hydraulic droplet erosion with an Erode button.",
    ),
    ts(
        "Wild Flowers",
        "",
        TKind::Action("flowers"),
        "Select this terrain's MarchingSquaresWildFlowers child, adding one if there is none (undoable): clustered meadow flowers with Scatter / Clear buttons.",
    ),
    ts(
        "Storage Mode",
        "storage_mode",
        TKind::Enum(&["Resource", "Baked", "Runtime"]),
        "Resource: all chunks in the data resource. Baked / Runtime: one file per chunk in Data Directory, written on scene save (only changed chunks); Baked also stores meshes for fast loading.",
    ),
    ts(
        "Bake Grass",
        "bake_grass",
        TKind::Bool,
        "Baked storage: also store the grass multimeshes.",
    ),
    ts(
        "Bake Collision",
        "bake_impact",
        TKind::Bool,
        "Baked storage: also store the collision faces.",
    ),
    ts(
        "Data Directory",
        "data_directory",
        TKind::Text,
        "Folder of the per-chunk files (empty = generated next to the scene on save). Press Enter to apply.",
    ),
    ts(
        "Save Data",
        "",
        TKind::Action("save"),
        "Write the terrain data now (changed chunks only).",
    ),
    ts(
        "Import MST",
        "",
        TKind::Action("import"),
        "Import the original MarchingSquaresTerrain's data: pick its scene (.tscn), its terrain data folder or a metadata.res.",
    ),
    ts(
        "Bake Meshes",
        "",
        TKind::Action("bake"),
        "Bake the chunks into a static MeshInstance3D / StaticBody3D copy next to the terrain (undoable).",
    ),
    ts(
        "Export GLB",
        "",
        TKind::Action("glb"),
        "Geometry baker: export every chunk to the terrain's export_path as a GLB with a baked texture atlas (slot textures, blending, walls).",
    ),
];
pub(super) fn terrain_setting_label(prop: &str) -> Option<&'static str> {
    TERRAIN_SETTINGS
        .iter()
        .find(|t| {
            !t.prop.is_empty()
                && t.prop == prop
                && !matches!(
                    t.sort,
                    TKind::ExtraLayer | TKind::GrassFps | TKind::Action(_)
                )
        })
        .map(|t| t.label)
}
pub(super) const SETTINGS_PAGES: [(&str, &str); 6] = [
    (
        "Grid",
        "Grid type, chunk and cell size, blending, merge mode, collision.",
    ),
    (
        "New Chunks",
        "Textures and noise given to newly added chunks.",
    ),
    ("Grass", "Grass growth, animation and blade size."),
    ("Display", "Pixel size, grid overlay, fog and debug views."),
    (
        "LOD & Nature",
        "Distant low-detail ring; droplet erosion and wild flowers (child nodes with their own strip).",
    ),
    (
        "Storage",
        "Where the terrain data lives; save, import, bake and export.",
    ),
];
fn settings_page_of(t: &TSetting) -> usize {
    match t.prop {
        "grid_type" | "chunk_size" | "ring_radius" | "cell_size" | "wrap_chunks" | "blend_mode"
        | "default_merge_mode" | "hex_edge_style" | "hex_edge_instanced" | "impact_stratum" => 0,
        "default_floor_slot"
        | "default_wall_slot"
        | "new_chunk_noise_amplitude"
        | "new_chunk_noise_frequency"
        | "noise_seed"
        | "noise_hmap"
        | "noise_hmap_height" => 1,
        "" if matches!(t.sort, TKind::GrassFps) => 2,
        "grass_enabled"
        | "grass_subdivisions"
        | "grass_size"
        | "grass_ledge_threshold"
        | "grass_min_height" => 2,
        "pixel_size"
        | "show_grid_in_editor"
        | "grid_chunk_lines"
        | "fog_in_editor"
        | "debug_view" => 3,
        "lod_distance" | "lod_in_editor" => 4,
        "" if matches!(t.sort, TKind::Action("erosion" | "flowers")) => 4,
        _ => 5,
    }
}
fn spin(min: f64, max: f64, step: f64, value: f64) -> Gd<SpinBox> {
    let mut sb = SpinBox::new_alloc();
    sb.set_min(min);
    sb.set_max(max);
    sb.set_step(step);
    sb.set_value_no_signal(value);
    sb.set_custom_minimum_size(sz(70.0, 25.0));
    sb.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    sb.set_select_all_on_focus(true);
    sb
}
fn export_status_text(writing: bool) -> &'static str {
    if writing {
        "Writing file: the editor is paused until it is saved"
    } else {
        "Baking; the editor will pause to write the file"
    }
}
const LAYOUT_LOCKED_TIP: &str =
    "Fixed once hex-ring chunks exist: remove every chunk to change it.";
fn lock_setting(c: &mut Gd<Control>) {
    if let Ok(mut sb) = c.clone().try_cast::<SpinBox>() {
        sb.set_editable(false);
        sb.set_focus_mode(godot::classes::control::FocusMode::NONE);
    }
    for k in c.get_children().iter_shared() {
        if let Ok(mut k) = k.try_cast::<Control>() {
            lock_setting(&mut k);
        }
    }
}
impl MarchingSquaresTerrainPlugin {
    fn export_progress(&self) -> f64 {
        self.terrain
            .as_ref()
            .map_or(-1.0, |t| t.bind().get_export_progress())
    }
    fn export_progress_row(&self) -> Gd<Control> {
        let mut hb = HBoxContainer::new_alloc();
        hb.set_name("ExportGlbProgress");
        let mut bar = godot::classes::ProgressBar::new_alloc();
        bar.set_name("ExportProgress");
        bar.set_max(1.0);
        bar.set_step(0.0);
        bar.set_value(self.export_progress().max(0.0));
        bar.set_custom_minimum_size(sz(110.0, 20.0));
        bar.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        hb.add_child(&bar);
        let mut status = Label::new_alloc();
        status.set_name("ExportStatus");
        status.set_text(export_status_text(self.export_progress() >= 1.0));
        status.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        hb.add_child(&status);
        let mut b = Button::new_alloc();
        b.set_text("Cancel");
        b.set_name("CancelExport");
        b.set_tooltip_text("Stop the export; nothing is written.");
        b.set_custom_minimum_size(sz(60.0, 25.0));
        b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        b.connect("pressed", &self.cb("on_export_cancel"));
        hb.add_child(&b);
        hb.upcast()
    }
    fn layout_locked(&self, prop: &str) -> bool {
        matches!(prop, "ring_radius" | "wrap_chunks")
            && self
                .terrain
                .as_ref()
                .is_some_and(|t| t.bind().hex_layout_locked())
    }
    fn terrain_prop(&self, prop: &str) -> Variant {
        self.terrain
            .as_ref()
            .map(|t| t.get(prop))
            .unwrap_or_default()
    }
    fn turf_stuff(&self) -> Option<Gd<godot::classes::ShaderMaterial>> {
        self.terrain_prop("turf_stuff")
            .try_to::<Gd<godot::classes::ShaderMaterial>>()
            .ok()
    }
    fn build_settings_strip(&mut self, strip: &mut Strip) {
        let grid = self.terrain_prop("grid_type").try_to::<i32>().unwrap_or(0);
        let rings = grid == 3;
        let storage = self
            .terrain_prop("storage_mode")
            .try_to::<i32>()
            .unwrap_or(0);
        let page = self.settings_page.min(SETTINGS_PAGES.len() - 1);
        let mut tabs = ui_kit::page_tabs("SettingsPages", &SETTINGS_PAGES, page, |k| {
            self.cb1("on_settings_page", k as i64)
        });
        tabs.set_h_size_flags(SizeFlags::SHRINK_BEGIN);
        let rows: Vec<(usize, &TSetting)> = TERRAIN_SETTINGS
            .iter()
            .enumerate()
            .filter(|(_, t)| settings_page_of(t) == page)
            .filter(|(_, t)| match t.prop {
                "chunk_size" => !rings,
                "ring_radius" | "wrap_chunks" => rings,
                "hex_edge_style" => grid != 0,
                "hex_edge_instanced" => grid >= 2,
                "bake_grass" | "bake_impact" => storage == 1,
                "data_directory" => storage != 0,
                _ => !matches!(t.sort, TKind::Action("save")) || storage != 0,
            })
            .collect();
        let built: Vec<Gd<HBoxContainer>> = rows
            .iter()
            .map(|(i, t)| {
                let mut c = self.build_terrain_setting(*i, t);
                let locked = self.layout_locked(t.prop);
                let tip = if locked {
                    format!("{}\n{LAYOUT_LOCKED_TIP}", t.tip)
                } else {
                    t.tip.to_string()
                };
                c.set_tooltip_text(&tip);
                if locked {
                    lock_setting(&mut c);
                }
                let mut row = ui_kit::settings_row(t.label, &c);
                row.set_tooltip_text(&tip);
                if locked {
                    row.set_modulate(ui_kit::DISABLED_MODULATE);
                }
                row
            })
            .collect();
        let widths: Vec<f32> = built
            .iter()
            .map(|r| r.get_combined_minimum_size().x)
            .collect();
        let lead = strip
            .flow
            .get_children()
            .iter_shared()
            .filter_map(|c| c.try_cast::<Control>().ok())
            .map(|c| c.get_combined_minimum_size().x)
            .sum::<f32>()
            + sz(64.0, 0.0).x;
        let avail = {
            let w = strip.base.get_size().x;
            if w > 1.0 {
                w
            } else {
                EditorInterface::singleton()
                    .get_editor_viewport_3d_ex()
                    .idx(0)
                    .done()
                    .map(|v| v.get_visible_rect().size.x)
                    .unwrap_or(1200.0)
            }
        };
        let gap = sz(ui_kit::CELL_GAP * 2.0 + 8.0, 0.0).x;
        let min_rows = 2.min(built.len().max(1));
        let per_col = (min_rows..=built.len().max(min_rows))
            .find(|r| {
                lead + widths
                    .chunks(*r)
                    .map(|c| c.iter().cloned().fold(0.0, f32::max) + gap)
                    .sum::<f32>()
                    <= avail
            })
            .unwrap_or(built.len().max(1));
        let mut cols = HBoxContainer::new_alloc();
        cols.set_name("SettingsColumns");
        cols.add_theme_constant_override("separation", ui_kit::px(ui_kit::CELL_GAP));
        for (k, chunk) in built.chunks(per_col).enumerate() {
            if k > 0 {
                cols.add_child(&godot::classes::VSeparator::new_alloc());
            }
            let mut col = ui_kit::column();
            col.set_alignment(godot::classes::box_container::AlignmentMode::BEGIN);
            for row in chunk {
                col.add_child(row);
            }
            cols.add_child(&col);
        }
        let mut block = VBoxContainer::new_alloc();
        block.set_name("SettingsPage");
        block.add_theme_constant_override("separation", ui_kit::px(2.0));
        block.add_child(&tabs);
        block.add_child(&cols);
        strip.custom("", &block);
        self.settings_snapshot = self.terrain_settings_snapshot();
    }
    fn terrain_row_shown(&self, t: &TSetting, comp: i64) -> Variant {
        let v = if t.prop.is_empty() {
            Variant::nil()
        } else {
            self.terrain_prop(t.prop)
        };
        match t.sort {
            TKind::Enum(_) | TKind::Slot => v.try_to::<i32>().unwrap_or(0).to_variant(),
            TKind::ExtraLayer => {
                let mask = v.try_to::<i64>().unwrap_or(1) as u32;
                (9..=32)
                    .rev()
                    .find(|l| mask & (1 << (l - 1)) != 0)
                    .map(|l| l - 8)
                    .unwrap_or(0)
                    .to_variant()
            }
            TKind::Int(..) => (v.try_to::<i64>().unwrap_or(0) as f64).to_variant(),
            TKind::Float(..) | TKind::Unit => v.try_to::<f64>().unwrap_or(0.0).to_variant(),
            TKind::GrassFps => self
                .turf_stuff()
                .map(|m| m.get_shader_parameter("animation_fps"))
                .and_then(|v| v.try_to::<f64>().ok())
                .unwrap_or(0.0)
                .to_variant(),
            TKind::Bool => v.try_to::<bool>().unwrap_or(false).to_variant(),
            TKind::Vec2(..) => {
                let val = v.try_to::<Vector2>().unwrap_or(Vector2::ONE);
                (if comp == 0 { val.x } else { val.y } as f64).to_variant()
            }
            TKind::Vec2i(..) => {
                let val = v.try_to::<Vector2i>().unwrap_or(Vector2i::ZERO);
                (if comp == 0 { val.x } else { val.y } as f64).to_variant()
            }
            TKind::Text => v.try_to::<GString>().unwrap_or_default().to_variant(),
            TKind::Noise => v,
            TKind::Action(_) => Variant::nil(),
        }
    }
    fn build_terrain_setting(&self, i: usize, t: &TSetting) -> Gd<Control> {
        let c = self.build_terrain_setting_widget(i, t);
        let parts: Vec<Gd<Node>> = match t.sort {
            TKind::Action(_) => Vec::new(),
            TKind::Vec2(..) | TKind::Vec2i(..) => c.get_children().iter_shared().collect(),
            _ => vec![c.clone().upcast()],
        };
        for (comp, mut part) in parts.into_iter().enumerate() {
            Shows::TerrainRow {
                index: i,
                comp: comp as i64,
            }
            .tag(&mut part);
        }
        c
    }
    fn build_terrain_setting_widget(&self, i: usize, t: &TSetting) -> Gd<Control> {
        let shown = |comp: i64| self.terrain_row_shown(t, comp);
        let id = i as i64;
        let on = |comp: i64| {
            self.cb("on_terrain_setting")
                .bind(&[id.to_variant(), comp.to_variant()])
        };
        match t.sort {
            TKind::Enum(items) => {
                let mut ob = ui_kit::option_button(items, shown(0).to::<i32>(), 100.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::Slot => {
                let names = self.slot_names();
                let refs: Vec<&str> = names.iter().map(|x| x.as_str()).collect();
                let mut ob = ui_kit::option_button(&refs, shown(0).to::<i32>(), 100.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::ExtraLayer => {
                let mut items = vec!["None".to_string()];
                items.extend((9..=32).map(|l| l.to_string()));
                let refs: Vec<&str> = items.iter().map(|x| x.as_str()).collect();
                let mut ob = ui_kit::option_button(&refs, shown(0).to::<i32>(), 65.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::Int(min, max) => {
                let mut sb = spin(min as f64, max as f64, 1.0, shown(0).to::<f64>());
                sb.set_allow_greater(t.prop.ends_with("pixel_size"));
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Float(min, max, step) => {
                let mut sb = spin(min, max, step, shown(0).to::<f64>());
                sb.set_allow_greater(true);
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::GrassFps => {
                let mut sb = spin(0.0, 60.0, 1.0, shown(0).to::<f64>());
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Unit => {
                let mut s = EditorSpinSlider::new_alloc();
                s.set_min(0.0);
                s.set_max(1.0);
                s.set_step(0.0105);
                s.set_value_no_signal(shown(0).to::<f64>());
                s.set_flat(true);
                s.set_custom_minimum_size(sz(105.0, 25.0));
                s.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                s.connect("value_changed", &on(0));
                s.upcast()
            }
            TKind::Bool => {
                let mut cb = CheckBox::new_alloc();
                cb.set_flat(true);
                cb.set_pressed_no_signal(shown(0).to::<bool>());
                cb.connect("toggled", &on(0));
                cb.upcast()
            }
            TKind::Vec2(min, max, step) => {
                let mut hb = HBoxContainer::new_alloc();
                for k in 0..2 {
                    let mut sb = spin(min, max, step, shown(k).to::<f64>());
                    sb.set_allow_greater(true);
                    sb.set_prefix(if k == 0 { "x" } else { "y" });
                    sb.connect("value_changed", &on(k));
                    hb.add_child(&sb);
                }
                hb.upcast()
            }
            TKind::Text => {
                let mut le = LineEdit::new_alloc();
                le.set_text(&shown(0).to::<GString>());
                le.set_placeholder("(generated on save)");
                le.set_custom_minimum_size(sz(180.0, 25.0));
                le.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                le.connect("text_submitted", &on(0));
                le.upcast()
            }
            TKind::Noise => {
                let mut picker = EditorResourcePicker::new_alloc();
                picker.set_base_type("Noise");
                if let Ok(r) = shown(0).try_to::<Gd<godot::classes::Resource>>() {
                    picker.set_edited_resource(&r);
                }
                picker.set_custom_minimum_size(sz(150.0, 25.0));
                picker.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                picker.connect("resource_changed", &on(0));
                picker.upcast()
            }
            TKind::Action("glb") if self.export_progress() >= 0.0 => self.export_progress_row(),
            TKind::Action(a) => {
                let mut b = Button::new_alloc();
                b.set_text(match a {
                    "save" => "Save",
                    "import" => "Import...",
                    "bake" => "Bake",
                    "erosion" | "flowers" => {
                        let class = if a == "erosion" {
                            "MarchingSquaresErosion"
                        } else {
                            "MarchingSquaresWildFlowers"
                        };
                        if self.terrain_child(class).is_some() {
                            "Select"
                        } else {
                            "Add"
                        }
                    }
                    _ => "Export",
                });
                b.set_name(&ui_kit::node_name(t.label));
                b.set_custom_minimum_size(sz(70.0, 25.0));
                b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                b.connect(
                    "pressed",
                    &self.cb("on_terrain_setting").bind(&[
                        Variant::nil(),
                        id.to_variant(),
                        0i64.to_variant(),
                    ]),
                );
                b.upcast()
            }
            TKind::Vec2i(min, max) => {
                let mut hb = HBoxContainer::new_alloc();
                for k in 0..2 {
                    let mut sb = spin(min as f64, max as f64, 1.0, shown(k).to::<f64>());
                    sb.set_prefix(if k == 0 { "x" } else { "y" });
                    sb.connect("value_changed", &on(k));
                    hb.add_child(&sb);
                }
                hb.upcast()
            }
        }
    }
    pub(super) fn terrain_setting_changed(&mut self, value: Variant, index: usize, comp: i64) {
        let Some(t) = TERRAIN_SETTINGS.get(index) else {
            return;
        };
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        let obj = terrain.clone().upcast::<Object>();
        if let TKind::GrassFps = t.sort {
            let Some(mat) = self.turf_stuff() else { return };
            let old = mat.get_shader_parameter("animation_fps");
            let new = value.try_to::<f64>().unwrap_or(0.0) as f32;
            ur.create_action("terrain grass animation fps");
            let m = mat.upcast::<Object>();
            ur.add_do_method(
                &m,
                "set_shader_parameter",
                &["animation_fps".to_variant(), new.to_variant()],
            );
            ur.add_undo_method(
                &m,
                "set_shader_parameter",
                &["animation_fps".to_variant(), old],
            );
            ur.commit_action();
            self.settings_snapshot = self.terrain_settings_snapshot();
            return;
        }
        if let TKind::Action(a) = t.sort {
            self.terrain_action(a);
            return;
        }
        if self.layout_locked(t.prop) {
            self.rebuild_strip_deferred();
            return;
        }
        let old = terrain.get(t.prop);
        let number = value
            .try_to::<f64>()
            .or_else(|_| value.try_to::<i64>().map(|n| n as f64))
            .unwrap_or(0.0);
        let new: Variant = match t.sort {
            TKind::Text => GString::from(
                value
                    .try_to::<GString>()
                    .unwrap_or_default()
                    .to_string()
                    .trim(),
            )
            .to_variant(),
            TKind::Noise => value.clone(),
            TKind::Action(_) => return,
            TKind::Enum(_) | TKind::Slot | TKind::Int(..) => (number.round() as i64).to_variant(),
            TKind::Float(..) | TKind::Unit => number.to_variant(),
            TKind::Bool => value.try_to::<bool>().unwrap_or(false).to_variant(),
            TKind::ExtraLayer => {
                let sel = value.try_to::<i64>().unwrap_or(0);
                let mut mask = old.try_to::<i64>().unwrap_or(1) as u32 & 0xff;
                if sel > 0 {
                    mask |= 1 << (sel + 8 - 1);
                }
                (mask as i64).to_variant()
            }
            TKind::Vec2(..) => {
                let mut v = old.try_to::<Vector2>().unwrap_or(Vector2::ONE);
                let x = value.try_to::<f64>().unwrap_or(0.0) as f32;
                if comp == 0 {
                    v.x = x
                } else {
                    v.y = x
                }
                v.to_variant()
            }
            TKind::Vec2i(..) => {
                let mut v = old.try_to::<Vector2i>().unwrap_or(Vector2i::ZERO);
                let x = value.try_to::<f64>().unwrap_or(0.0).round() as i32;
                if comp == 0 {
                    v.x = x
                } else {
                    v.y = x
                }
                v.to_variant()
            }
            TKind::GrassFps => return,
        };
        if new == old {
            return;
        }
        ur.create_action(&format!("terrain setting {}", t.label.to_lowercase()));
        ur.add_do_property(&obj, t.prop, &new);
        ur.add_undo_property(&obj, t.prop, &old);
        ur.commit_action();
        self.settings_snapshot = self.terrain_settings_snapshot();
        if t.prop == "show_grid_in_editor" {
            self.sync_grid_toggle();
        }
        if matches!(
            t.prop,
            "grid_type" | "chunk_size" | "cell_size" | "ring_radius" | "storage_mode"
        ) {
            self.selection.clear();
            self.rebuild_strip_deferred();
        }
        self.redraw();
    }
}
pub(super) const ATLAS_POLYGON_RESOLUTION: i32 = 16;
impl MarchingSquaresTerrainPlugin {
    fn terrain_action(&mut self, action: &str) {
        let Some(mut terrain) = self.terrain.clone() else {
            return;
        };
        match action {
            "save" => {
                terrain.call("save_terrain_data", &[]);
            }
            "bake" => {
                terrain.call("bake_static_meshes", &[]);
            }
            "glb" => self.request_atlas_export(),
            "import" => self.open_import_dialog(),
            "erosion" => self.add_or_select_child("MarchingSquaresErosion", "Erosion"),
            "flowers" => self.add_or_select_child("MarchingSquaresWildFlowers", "WildFlowers"),
            _ => {}
        }
    }
    pub(super) fn start_atlas_export(&mut self) {
        let Some(mut terrain) = self.terrain.clone() else {
            return;
        };
        let path = terrain.get("export_path");
        let err = terrain.call("begin_export_glb_baked", &[path.clone(), ATLAS_POLYGON_RESOLUTION.to_variant()]);
        if err.try_to::<i64>().unwrap_or(-1) != 0 {
            godot_print!("Marching Squares Terrain: Export GLB to {path} -> {err}");
        }
        self.rebuild_strip_deferred();
    }
    fn terrain_child(&self, class: &str) -> Option<Gd<Node>> {
        let t = self.terrain.as_ref()?;
        t.get_children().iter_shared().find(|c| c.is_class(class))
    }
    fn add_or_select_child(&mut self, class: &str, name: &str) {
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let node = match self.terrain_child(class) {
            Some(n) => n,
            None => {
                let Ok(mut n) = godot::classes::ClassDb::singleton()
                    .instantiate(class)
                    .try_to::<Gd<Node>>()
                else {
                    return;
                };
                n.set_name(name);
                if let Ok(mut e) = n.clone().try_cast::<crate::terrain::erosion::MarchingSquaresErosion>() {
                    e.bind_mut().set_terrain(Some(terrain.clone()));
                } else if let Ok(mut f) =
                    n.clone().try_cast::<crate::terrain::wild_flowers::MarchingSquaresWildFlowers>()
                {
                    f.bind_mut().set_terrain(Some(terrain.clone()));
                }
                let owner = EditorInterface::singleton()
                    .get_edited_scene_root()
                    .unwrap_or_else(|| terrain.clone().upcast());
                let Some(mut ur) = self.base().get_undo_redo() else {
                    return;
                };
                let tn = terrain.clone().upcast::<Node>();
                ur.create_action(&format!("Add {class}"));
                ur.add_do_method(&tn, "add_child", &[n.to_variant(), true.to_variant()]);
                ur.add_do_method(&n, "set_owner", &[owner.to_variant()]);
                ur.add_do_reference(&n);
                ur.add_undo_method(&tn, "remove_child", &[n.to_variant()]);
                ur.commit_action();
                n
            }
        };
        EditorInterface::singleton().call_deferred("edit_node", &[node.to_variant()]);
    }
    fn open_import_dialog(&mut self) {
        use godot::classes::EditorFileDialog;
        use godot::classes::file_dialog::{Access, FileMode};
        let Some(mut base) = EditorInterface::singleton().get_base_control() else {
            godot_warn!("Marching Squares Terrain: no editor base control to show the file dialog");
            return;
        };
        let mut d = EditorFileDialog::new_alloc();
        d.set_title(
            "Import original MarchingSquaresTerrain data (scene, data folder or metadata.res)",
        );
        d.set_file_mode(FileMode::OPEN_ANY);
        d.set_access(Access::FILESYSTEM);
        d.add_filter("*.tscn ; Original scene (reads grid settings + data_directory)");
        d.add_filter("*.res ; MSTChunkData (metadata.res)");
        let on = self.cb("on_import_mst_path").bind(&[d.to_variant()]);
        d.connect("file_selected", &on);
        d.connect("dir_selected", &on);
        let free = Callable::from_object_method(&d, "queue_free");
        d.connect("canceled", &free);
        base.add_child(&d);
        d.popup_file_dialog();
    }
}
#[godot_api(secondary)]
impl MarchingSquaresTerrainPlugin {
    #[func]
    fn on_export_progressed(&mut self, done: i64, total: i64, terrain: Gd<MarchingSquaresTerrain>) {
        let strip_shows_progress = self.update_export_progress_row(&terrain, done, total);
        self.show_export_progress_notice(&terrain, done, total, strip_shows_progress);
    }
    fn update_export_progress_row(
        &mut self,
        terrain: &Gd<MarchingSquaresTerrain>,
        done: i64,
        total: i64,
    ) -> bool {
        if self.terrain.as_ref() != Some(terrain) {
            return false;
        }
        let Some(peel) = self.peel.as_ref().filter(|p| p.is_visible_in_tree()) else {
            return false;
        };
        let Some(row) = peel.find_child_ex("ExportGlbProgress").owned(false).done() else {
            return false;
        };
        if let Some(mut bar) = row
            .find_child_ex("ExportProgress")
            .owned(false)
            .done()
            .and_then(|n| n.try_cast::<godot::classes::ProgressBar>().ok())
        {
            bar.set_value(if total > 0 { done as f64 / total as f64 } else { 1.0 });
        }
        if let Some(mut status) = row
            .find_child_ex("ExportStatus")
            .owned(false)
            .done()
            .and_then(|n| n.try_cast::<Label>().ok())
        {
            status.set_text(export_status_text(done >= total));
        }
        true
    }
    #[func]
    fn on_export_finished(&mut self, path: GString, error: i64, terrain: Gd<MarchingSquaresTerrain>) {
        self.drop_export_notice();
        if error == godot::global::Error::ERR_SKIP.ord() as i64 {
            godot_print!("Marching Squares Terrain: Export GLB to {path} cancelled");
        } else {
            godot_print!("Marching Squares Terrain: Export GLB to {path} -> {error}");
        }
        if self.state.tool == Tool::TerrainSettings && self.terrain.as_ref() == Some(&terrain) {
            self.rebuild_strip_deferred();
        }
    }
    #[func]
    fn on_export_cancel(&mut self) {
        if let Some(t) = &mut self.terrain {
            t.bind_mut().cancel_export();
        }
    }
    #[func]
    fn on_import_mst_path(&mut self, path: GString, mut dialog: Gd<godot::classes::Node>) {
        dialog.queue_free();
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        let mut obj = terrain.clone().upcast::<Object>();
        let before: Vec<(&str, Variant)> = [
            "contents",
            "grid_type",
            "cell_size",
            "blend_mode",
            "default_wall_slot",
            "grass_subdivisions",
            "grass_size",
            "tex_assign",
        ]
        .into_iter()
        .map(|prop| (prop, terrain.get(prop)))
        .collect();
        if !obj.call("import_mst", &[path.to_variant()]).booleanize() {
            godot_warn!("Marching Squares Terrain: {path} was not imported; the terrain is unchanged");
            return;
        }
        ur.create_action("Import MST terrain data");
        ur.add_do_method(&obj, "import_mst", &[path.to_variant()]);
        for (prop, old) in &before {
            ur.add_undo_property(&obj, *prop, old);
        }
        ur.add_undo_method(&obj, "reassemble", &[]);
        ur.commit_action_ex().execute(false).done();
        self.selection.clear();
        self.refresh_chunk_list();
        self.rebuild_strip_deferred();
        self.redraw();
    }
}
impl MarchingSquaresTerrainPlugin {
    pub(super) fn tex_assign(&self) -> Option<Gd<MarchingSquaresTexturePreset>> {
        self.terrain
            .as_ref()
            .and_then(|t| t.bind().texture_set_ref())
    }
    fn slot_res(&self, i: usize) -> Option<Gd<MarchingSquaresTextureSlot>> {
        self.tex_assign().and_then(|ts| ts.bind().pocket(i))
    }
    fn slot_preview(&self, i: usize) -> Option<Gd<ImageTexture>> {
        let img = self.tex_assign()?.bind().preview_image(i, 32)?;
        ImageTexture::create_from_image(&img)
    }
    pub(super) fn rebuild_texture_panel(&mut self) {
        let Some(mut vbox) = self.texture_vbox.clone() else {
            return;
        };
        for c in vbox.get_children().iter_shared() {
            vbox.remove_child(&c);
            c.free();
        }
        self.slot_buttons.clear();
        self.slot_previews.clear();
        self.slot_tex_pickers.clear();
        self.slot_name_edits.clear();
        vbox.add_child(&ui_kit::panel_title(
            "Textures",
            ui_kit::glyph("vertex_paint_tool.svg"),
            "Texture slots of this terrain's texture set. Click a slot to paint with it (same as the Material dropdown).",
        ));
        if self.tex_assign().is_none() {
            let mut l = Label::new_alloc();
            l.set_text("No texture set. Pick a Texture Preset in the bottom bar.");
            l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
            l.set_modulate(ui_kit::DIM_MODULATE);
            vbox.add_child(&l);
            return;
        }
        let group = ButtonGroup::new_gd();
        for i in 0..SLOT_COUNT {
            let Some(slot) = self.slot_res(i) else {
                continue;
            };
            let (name, tex, pattern, scale, tint, has_grass, grass_tint, sprite) = {
                let b = slot.bind();
                (
                    b.display_name.clone(),
                    b.texture.clone(),
                    b.pattern,
                    b.magnify,
                    b.tint,
                    b.has_grass,
                    b.grass_tint,
                    b.grass_sprite.clone(),
                )
            };
            let void = i == SLOT_COUNT - 1;
            let mut head = HBoxContainer::new_alloc();
            let mut preview = TextureRect::new_alloc();
            preview.set_custom_minimum_size(sz(36.0, 36.0));
            preview.set_expand_mode(godot::classes::texture_rect::ExpandMode::IGNORE_SIZE);
            preview.set_stretch_mode(godot::classes::texture_rect::StretchMode::SCALE);
            preview.set_texture_filter(godot::classes::canvas_item::TextureFilter::NEAREST);
            preview.set_tooltip_text("Preview of this slot (texture or pattern, tinted).");
            if let Some(p) = self.slot_preview(i) {
                preview.set_texture(&p);
            }
            head.add_child(&preview);
            self.slot_previews.push(preview);
            let mut col = VBoxContainer::new_alloc();
            col.add_theme_constant_override("separation", 0);
            col.set_h_size_flags(SizeFlags::EXPAND_FILL);
            let mut sel = Button::new_alloc();
            sel.set_text(&format!(
                "Texture {}{}",
                i + 1,
                if void { " (void)" } else { "" }
            ));
            sel.set_flat(true);
            sel.set_toggle_mode(true);
            sel.set_button_group(&group);
            sel.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            sel.set_tooltip_text("Paint with this texture (Material).");
            sel.set_pressed_no_signal(i == self.state.paint_slot as usize);
            sel.connect("pressed", &self.cb1("on_slot_select", i as i64));
            col.add_child(&sel);
            self.slot_buttons.push(sel);
            let mut le = LineEdit::new_alloc();
            le.set_flat(true);
            le.set_text(&name);
            le.set_placeholder("Texture name");
            le.set_custom_minimum_size(sz(60.0, 22.0));
            le.set_tooltip_text("Texture name (shown in the Material dropdown).");
            le.connect("text_changed", &self.cb1("on_slot_name", i as i64));
            Shows::Slot(i, SlotProp::Name).tag(&mut le);
            col.add_child(&le);
            self.slot_name_edits.push((i, le.clone()));
            head.add_child(&col);
            vbox.add_child(&head);
            let mut picker = EditorResourcePicker::new_alloc();
            picker.set_base_type("Texture2D");
            picker.set_custom_minimum_size(sz(60.0, 25.0));
            picker.set_tooltip_text("Ground texture of this slot (white-ish pixel art, tinted by Color). Empty = the procedural Pattern.");
            if let Some(t) = &tex {
                picker.set_edited_resource(&t.clone().upcast::<godot::classes::Resource>());
            }
            picker.connect("resource_changed", &self.cb1("on_slot_texture", i as i64));
            Shows::Slot(i, SlotProp::Texture).tag(&mut picker);
            self.slot_tex_pickers.push((i, picker.clone()));
            vbox.add_child(&ui_kit::form_row("Texture", &picker));
            if !void {
                let mut pat = ui_kit::compact_option(&PATTERNS[..13], pattern.clamp(0, 12));
                pat.set_tooltip_text("Ground pattern: picks the bundled ground texture of this pattern (textures/ground/). Without a texture the pattern is generated procedurally.");
                pat.connect("item_selected", &self.cb1("on_slot_pattern", i as i64));
                Shows::Slot(i, SlotProp::Pattern).tag(&mut pat);
                vbox.add_child(&ui_kit::form_row("Pattern", &pat));
                let mut row = HBoxContainer::new_alloc();
                let mut slider = HSlider::new_alloc();
                slider.set_min(0.1005);
                slider.set_max(40.0);
                slider.set_step(0.1005);
                slider.set_value_no_signal(scale as f64);
                slider.set_custom_minimum_size(sz(50.0, 20.0));
                slider.set_h_size_flags(SizeFlags::EXPAND_FILL);
                slider.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                slider.set_tooltip_text("Texture / pattern scale.");
                let value = mst_value_label::value_label(scale as f64, 0.1, 30.0);
                slider.connect(
                    "value_changed",
                    &self
                        .cb("on_slot_scale")
                        .bind(&[(i as i64).to_variant(), value.to_variant()]),
                );
                slider.connect(
                    "value_changed",
                    &mst_value_label::value_label_updater(value.clone(), 0.1),
                );
                Shows::Slot(i, SlotProp::Magnify).tag(&mut slider);
                row.add_child(&slider);
                row.add_child(&value);
                vbox.add_child(&ui_kit::form_row("Scale", &row));
                let mut cp = ColorPickerButton::new_alloc();
                cp.set_pick_color(tint);
                cp.set_custom_minimum_size(sz(40.0, 25.0));
                cp.set_edit_alpha(false);
                cp.set_tooltip_text("Ground color (multiplies the texture / pattern).");
                cp.connect("color_changed", &self.cb1("on_slot_tint", i as i64));
                Shows::Slot(i, SlotProp::Tint).tag(&mut cp);
                vbox.add_child(&ui_kit::form_row("Color", &cp));
                let mut grass = HBoxContainer::new_alloc();
                let mut cb = CheckBox::new_alloc();
                cb.set_pressed_no_signal(has_grass);
                cb.set_tooltip_text("Grow grass on this texture.");
                cb.connect("toggled", &self.cb1("on_slot_has_grass", i as i64));
                Shows::Slot(i, SlotProp::HasGrass).tag(&mut cb);
                grass.add_child(&cb);
                let mut gp = ColorPickerButton::new_alloc();
                gp.set_pick_color(grass_tint);
                gp.set_tooltip_text("Grass color on this texture.");
                gp.set_custom_minimum_size(sz(40.0, 25.0));
                gp.set_h_size_flags(SizeFlags::EXPAND_FILL);
                gp.set_edit_alpha(false);
                gp.connect("color_changed", &self.cb1("on_slot_grass_tint", i as i64));
                Shows::Slot(i, SlotProp::GrassTint).tag(&mut gp);
                grass.add_child(&gp);
                vbox.add_child(&ui_kit::form_row("Grass", &grass));
                let mut sp = EditorResourcePicker::new_alloc();
                sp.set_base_type("Texture2D");
                sp.set_custom_minimum_size(sz(60.0, 25.0));
                sp.set_tooltip_text("Grass sprite on this texture: white leaves on alpha, colored by Grass (original MST: grass_sprite_tex_N). Empty = the terrain's grass texture.");
                if let Some(t) = &sprite {
                    sp.set_edited_resource(&t.clone().upcast::<godot::classes::Resource>());
                }
                sp.connect(
                    "resource_changed",
                    &self.cb1("on_slot_grass_sprite", i as i64),
                );
                Shows::Slot(i, SlotProp::GrassSprite).tag(&mut sp);
                vbox.add_child(&ui_kit::form_row("Sprite", &sp));
            }
            vbox.add_child(&HSeparator::new_alloc());
        }
        self.build_auto_rules_section(&mut vbox);
        vbox.set_meta(
            AUTO_RULES_SHOWN_META,
            &self.auto_rules_values().to_variant(),
        );
        let mut m = godot::classes::MarginContainer::new_alloc();
        m.add_theme_constant_override("margin_bottom", 7);
        let mut b = Button::new_alloc();
        b.set_text("Export Texture Preset");
        b.set_tooltip_text(&format!(
            "Save this terrain's texture set as a preset in {PRESET_DIR}."
        ));
        b.set_h_size_flags(SizeFlags::EXPAND_FILL);
        b.connect("pressed", &self.cb("on_export_preset"));
        m.add_child(&b);
        vbox.add_child(&m);
    }
    fn auto_rules_values(&self) -> VarArray {
        let Some(rules) = self
            .tex_assign()
            .and_then(|ts| ts.bind().auto_rules.clone())
        else {
            return VarArray::new();
        };
        let storage = godot::register::info::PropertyUsageFlags::STORAGE.ord() as i64;
        let stored = |p: &VarDictionary| {
            p.get("usage")
                .and_then(|u| u.try_to::<i64>().ok())
                .is_some_and(|usage| usage & storage != 0)
        };
        rules
            .get_property_list()
            .iter_shared()
            .filter(stored)
            .filter_map(|p| p.get("name").map(|n| n.to_string()))
            .map(|name| rules.get(name.as_str()))
            .collect()
    }
    fn auto_rules_shown_stale(&self) -> bool {
        self.texture_vbox
            .as_ref()
            .filter(|v| v.is_instance_valid() && v.has_meta(AUTO_RULES_SHOWN_META))
            .is_some_and(|v| {
                v.get_meta(AUTO_RULES_SHOWN_META) != self.auto_rules_values().to_variant()
            })
    }
    pub(super) fn sync_shown_widgets(&mut self) {
        let roots = [
            self.peel.clone().map(|p| p.upcast::<Node>()),
            self.texture_vbox.clone().map(|v| v.upcast::<Node>()),
        ];
        let mut renewed = Vec::new();
        for root in roots.into_iter().flatten().filter(|r| r.is_instance_valid()) {
            for node in root
                .find_children_ex("*")
                .owned(false)
                .done()
                .iter_shared()
            {
                let Some(shows) = Shows::of(&node) else {
                    continue;
                };
                let Ok(widget) = node.try_cast::<Control>() else {
                    continue;
                };
                let model = match shows {
                    Shows::TerrainRow { index, comp } => TERRAIN_SETTINGS
                        .get(index)
                        .map(|t| self.terrain_row_shown(t, comp)),
                    Shows::Slot(i, prop) => self.slot_res(i).map(|s| prop.shown(&s)),
                };
                if let Some(v) = model
                    && show_on_widget(&widget, &v)
                    && let Shows::Slot(i, _) = shows
                    && !renewed.contains(&i)
                {
                    renewed.push(i);
                }
            }
        }
        for i in renewed {
            self.renew_pocket_prelook(i);
        }
    }
    pub(super) fn refresh_texture_panel_selection(&mut self) {
        let slot = self.state.paint_slot as usize;
        for (i, b) in self.slot_buttons.iter_mut().enumerate() {
            if b.is_instance_valid() {
                b.set_pressed_no_signal(i == slot);
            }
        }
    }
    pub(super) fn renew_pocket_prelook(&mut self, i: usize) {
        let p = self.slot_preview(i);
        if let (Some(r), Some(p)) = (self.slot_previews.get_mut(i), p)
            && r.is_instance_valid()
        {
            r.set_texture(&p);
        }
    }
    pub(super) fn set_slot_prop(&mut self, i: usize, prop: &str, value: Variant) {
        let Some(slot) = self.slot_res(i) else { return };
        let old = slot.get(prop);
        if old == value {
            return;
        }
        let obj = slot.upcast::<Object>();
        if let Some(mut ur) = self.base().get_undo_redo() {
            ur.create_action_ex(&format!("terrain texture {} {}", i + 1, prop))
                .merge_mode(godot::classes::undo_redo::MergeMode::ENDS)
                .done();
            ur.add_do_property(&obj, prop, &value);
            ur.add_undo_property(&obj, prop, &old);
            ur.commit_action();
        }
        if matches!(prop, "texture" | "pattern" | "tint") {
            self.renew_pocket_prelook(i);
        }
    }
    pub(super) fn set_slot_pattern(&mut self, i: usize, pattern: i32) {
        let Some(slot) = self.slot_res(i) else { return };
        let (old_p, old_t) = (slot.get("pattern"), slot.get("texture"));
        let tex = crate::terrain::textures::pattern_texture(pattern);
        let obj = slot.upcast::<Object>();
        if let Some(mut ur) = self.base().get_undo_redo() {
            ur.create_action(&format!("terrain texture {} pattern", i + 1));
            ur.add_do_property(&obj, "pattern", &pattern.to_variant());
            ur.add_do_property(&obj, "texture", &tex.to_variant());
            ur.add_undo_property(&obj, "pattern", &old_p);
            ur.add_undo_property(&obj, "texture", &old_t);
            ur.commit_action();
        }
        self.renew_pocket_prelook(i);
        if let Some((_, p)) = self
            .slot_tex_pickers
            .iter_mut()
            .find(|(k, p)| *k == i && p.is_instance_valid())
        {
            p.set_edited_resource(tex.map(|t| t.upcast::<godot::classes::Resource>()).as_ref());
        }
    }
    pub(super) fn list_presets(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Some(dir) = DirAccess::open(PRESET_DIR) else {
            return out;
        };
        let mut files: Vec<String> = dir
            .get_files()
            .as_slice()
            .iter()
            .map(|f| f.to_string())
            .collect();
        files.sort();
        for f in files {
            let f = f.trim_end_matches(".remap").to_string();
            if !(f.ends_with(".tres") || f.ends_with(".res")) {
                continue;
            }
            let path = format!("{PRESET_DIR}{f}");
            let Some(res) = ResourceLoader::singleton().load(&path) else {
                continue;
            };
            let Ok(p) = res.try_cast::<MarchingSquaresTexturePreset>() else {
                continue;
            };
            let name = p.bind().preset_name.to_string();
            let name = if name.is_empty() {
                f.trim_end_matches(".tres")
                    .trim_end_matches(".res")
                    .replace('_', " ")
            } else {
                name
            };
            out.push((path, name));
        }
        out
    }
    fn current_preset_index(&self) -> i32 {
        let Some(ts) = self.tex_assign() else {
            return 0;
        };
        let path = if ts.has_meta(PRESET_META) {
            ts.get_meta(PRESET_META).to_string()
        } else {
            ts.get_path().to_string()
        };
        self.list_presets()
            .iter()
            .position(|p| p.0 == path)
            .map(|i| i as i32 + 1)
            .unwrap_or(0)
    }
    pub(super) fn apply_preset(&mut self, index: usize) {
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let new: Gd<MarchingSquaresTexturePreset> = if index == 0 {
            let mut p = MarchingSquaresTexturePreset::new_default();
            p.set_name("Textures");
            p
        } else {
            let Some((path, _)) = self.list_presets().into_iter().nth(index - 1) else {
                return;
            };
            let Some(res) = ResourceLoader::singleton().load(&path) else {
                return;
            };
            let Some(dup) = res.duplicate_resource_ex().deep_internal().done_or_null() else {
                return;
            };
            let Ok(mut p) = dup.try_cast::<MarchingSquaresTexturePreset>() else {
                return;
            };
            p.set_meta(PRESET_META, &path.to_variant());
            let name = p.bind().preset_name.to_string();
            p.set_name(if name.is_empty() {
                "Textures"
            } else {
                name.as_str()
            });
            p
        };
        let old = terrain.get("tex_assign");
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.upcast::<Object>();
            ur.create_action("terrain texture preset");
            ur.add_do_property(&obj, "tex_assign", &new.to_variant());
            ur.add_undo_property(&obj, "tex_assign", &old);
            ur.commit_action();
        }
        self.refresh_material_names();
        if self.state.tool == Tool::VertexPaint {
            self.to_gd().call_deferred("rebuild_texture_panel_now", &[]);
        }
    }
    pub(super) fn unveil_ship_out_popup(&mut self) {
        let Some(mut base) = EditorInterface::singleton().get_base_control() else {
            godot_warn!(
                "Marching Squares Terrain: no editor base control to show the save-preset dialog"
            );
            return;
        };
        let Some(root) = super::editor_scenes::instantiate::<MstSavePresetDialog>(
            super::editor_scenes::SAVE_PRESET_DIALOG,
        ) else {
            return;
        };
        let parts = {
            let r = root.bind();
            (
                r.save_dialog.clone(),
                r.prompt.clone(),
                r.preset_name.clone(),
                r.overwrite_dialog.clone(),
            )
        };
        let (Some(mut dlg), Some(mut l), Some(mut le), Some(mut overwrite)) = parts else {
            godot_error!(
                "Marching Squares Terrain: the save-preset dialog scene does not assign all of its parts"
            );
            root.free();
            return;
        };
        dlg.set_size(Vector2i::new(
            (400.0 * ui_kit::editor_scale()) as i32,
            (150.0 * ui_kit::editor_scale()) as i32,
        ));
        l.set_text(&format!("Preset name (saved to {PRESET_DIR}):"));
        let current = self
            .tex_assign()
            .map(|t| t.bind().preset_name.to_string())
            .filter(|n| !n.is_empty());
        le.set_text(current.as_deref().unwrap_or("new_texture_preset"));
        le.select_all();
        dlg.register_text_enter(&le);
        let rootv = root.to_variant();
        let dismiss = Callable::from_object_method(&root, "queue_free");
        dlg.connect("confirmed", &self.cb("on_preset_name_chosen").bind(&[rootv.clone()]));
        dlg.connect("canceled", &dismiss);
        overwrite.connect(
            "confirmed",
            &self.cb("on_preset_overwrite_chosen").bind(&[rootv]),
        );
        overwrite.connect("canceled", &dismiss);
        base.add_child(&root);
        dlg.popup_centered();
        le.grab_focus();
    }
    pub(super) fn preset_name_chosen(&mut self, mut root: Gd<MstSavePresetDialog>) {
        let (name, overwrite) = {
            let r = root.bind();
            (
                r.preset_name.as_ref().map(|le| le.get_text().to_string()),
                r.overwrite_dialog.clone(),
            )
        };
        let Some(name) = name else {
            root.queue_free();
            return;
        };
        match (self.export_preset(name, false), overwrite) {
            (PresetSave::AlreadyExists(path), Some(mut c)) => {
                c.set_text(&format!("{path} already exists. Overwrite it?"));
                c.popup_centered();
            }
            _ => root.queue_free(),
        }
    }
    pub(super) fn preset_overwrite_chosen(&mut self, mut root: Gd<MstSavePresetDialog>) {
        let name = root
            .bind()
            .preset_name
            .as_ref()
            .map(|le| le.get_text().to_string());
        root.queue_free();
        if let Some(name) = name {
            self.export_preset(name, true);
        }
    }
    pub(super) fn export_preset(&mut self, name: String, overwrite: bool) -> PresetSave {
        let name = name.trim().to_string();
        if name.is_empty() {
            return PresetSave::Done;
        }
        let file = snake_case(&name);
        let path = format!("{PRESET_DIR}{file}.tres");
        if !overwrite && ResourceLoader::singleton().exists(&path) {
            return PresetSave::AlreadyExists(path);
        }
        let Some(ts) = self.tex_assign() else {
            return PresetSave::Done;
        };
        let Some(dup) = ts.duplicate_resource_ex().deep_internal().done_or_null() else {
            return PresetSave::Done;
        };
        let Ok(mut p) = dup.try_cast::<MarchingSquaresTexturePreset>() else {
            return PresetSave::Done;
        };
        p.bind_mut().preset_name = name.as_str().into();
        p.set_name(name.as_str());
        if p.has_meta(PRESET_META) {
            p.remove_meta(PRESET_META);
        }
        let _ = DirAccess::make_dir_recursive_absolute(PRESET_DIR);
        let err = ResourceSaver::singleton().save_ex(&p).path(&path).done();
        if err != godot::global::Error::OK {
            godot_error!("Marching Squares Terrain: could not save preset {path}: {err:?}");
            return PresetSave::Done;
        }
        if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
            fs.update_file(&path);
        }
        let mut ts = ts;
        ts.set_meta(PRESET_META, &path.to_variant());
        ts.bind_mut().preset_name = name.as_str().into();
        ts.set_name(name.as_str());
        godot_print!("Marching Squares Terrain: saved texture preset {path}");
        if self.state.tool == Tool::VertexPaint {
            self.rebuild_strip_deferred();
        }
        PresetSave::Done
    }
}
fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_us = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && !prev_us
                && !out.is_empty()
                && out.chars().last().is_some_and(|c| c.is_ascii_lowercase())
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "texture_preset".into()
    } else {
        out
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn snake() {
        assert_eq!(super::snake_case("Biomes - Icey"), "biomes_icey");
        assert_eq!(super::snake_case("FlatToon"), "flat_toon");
        assert_eq!(super::snake_case("  "), "texture_preset");
    }
}
const SHOWS_META: &str = "mst_shows";
#[derive(Clone, Copy)]
enum SlotProp {
    Name,
    Texture,
    Pattern,
    Magnify,
    Tint,
    HasGrass,
    GrassTint,
    GrassSprite,
}
impl SlotProp {
    const ALL: [SlotProp; 8] = [
        SlotProp::Name,
        SlotProp::Texture,
        SlotProp::Pattern,
        SlotProp::Magnify,
        SlotProp::Tint,
        SlotProp::HasGrass,
        SlotProp::GrassTint,
        SlotProp::GrassSprite,
    ];
    fn prop(self) -> &'static str {
        match self {
            SlotProp::Name => "display_name",
            SlotProp::Texture => "texture",
            SlotProp::Pattern => "pattern",
            SlotProp::Magnify => "magnify",
            SlotProp::Tint => "tint",
            SlotProp::HasGrass => "has_grass",
            SlotProp::GrassTint => "grass_tint",
            SlotProp::GrassSprite => "grass_sprite",
        }
    }
    fn shown(self, slot: &Gd<MarchingSquaresTextureSlot>) -> Variant {
        let v = slot.get(self.prop());
        match self {
            SlotProp::Pattern => v.try_to::<i32>().unwrap_or(0).clamp(0, 12).to_variant(),
            SlotProp::Magnify => v.try_to::<f64>().unwrap_or(1.0).to_variant(),
            _ => v,
        }
    }
}
#[derive(Clone, Copy)]
enum Shows {
    TerrainRow { index: usize, comp: i64 },
    Slot(usize, SlotProp),
}
impl Shows {
    fn tag(self, widget: &mut Gd<impl Inherits<Object>>) {
        let code: [i64; 3] = match self {
            Shows::TerrainRow { index, comp } => [0, index as i64, comp],
            Shows::Slot(i, prop) => [1, i as i64, prop as i64],
        };
        let code: VarArray = code.iter().map(|c| c.to_variant()).collect();
        widget.upcast_mut().set_meta(SHOWS_META, &code.to_variant());
    }
    fn of(node: &Gd<Node>) -> Option<Shows> {
        if !node.has_meta(SHOWS_META) {
            return None;
        }
        let code = node.get_meta(SHOWS_META).try_to::<VarArray>().ok()?;
        let at = |k: usize| code.get(k).and_then(|v| v.try_to::<i64>().ok());
        let (kind, a, b) = (at(0)?, at(1)?, at(2)?);
        match kind {
            0 => Some(Shows::TerrainRow {
                index: a as usize,
                comp: b,
            }),
            1 => Some(Shows::Slot(
                a as usize,
                *SlotProp::ALL.get(b as usize)?,
            )),
            _ => None,
        }
    }
}
fn shown_number(v: &Variant) -> Option<f64> {
    v.try_to::<f64>()
        .or_else(|_| v.try_to::<i64>().map(|n| n as f64))
        .or_else(|_| v.try_to::<bool>().map(|b| if b { 1.0 } else { 0.0 }))
        .ok()
}
fn show_on_widget(widget: &Gd<Control>, v: &Variant) -> bool {
    use godot::classes::{BaseButton, Range};
    if let Ok(mut ob) = widget.clone().try_cast::<OptionButton>() {
        let i = shown_number(v).unwrap_or(-1.0) as i32;
        let changed = ob.get_selected() != i;
        if changed {
            ob.select(i);
        }
        return changed;
    }
    if let Ok(mut cp) = widget.clone().try_cast::<ColorPickerButton>() {
        let Ok(c) = v.try_to::<Color>() else {
            return false;
        };
        let changed = cp.get_pick_color() != c;
        if changed {
            cp.set_pick_color(c);
        }
        return changed;
    }
    if let Ok(mut b) = widget.clone().try_cast::<BaseButton>() {
        let on = shown_number(v).is_some_and(|n| n != 0.0);
        let changed = b.is_toggle_mode() && b.is_pressed() != on;
        if changed {
            b.set_pressed_no_signal(on);
        }
        return changed;
    }
    if let Ok(mut r) = widget.clone().try_cast::<Range>() {
        let Some(n) = shown_number(v) else {
            return false;
        };
        let changed = r.get_value() != n;
        if changed {
            r.set_value_no_signal(n);
        }
        echo_to_readouts(&r);
        return changed;
    }
    if let Ok(mut le) = widget.clone().try_cast::<LineEdit>() {
        let Ok(text) = v.try_to::<GString>() else {
            return false;
        };
        let changed = !le.has_focus() && le.get_text() != text;
        if changed {
            le.set_text(&text);
        }
        return changed;
    }
    if let Ok(mut picker) = widget.clone().try_cast::<EditorResourcePicker>() {
        let wanted = v.try_to::<Gd<godot::classes::Resource>>().ok();
        let changed = picker.get_edited_resource() != wanted;
        if changed {
            picker.set_edited_resource(wanted.as_ref());
        }
        return changed;
    }
    false
}
fn echo_to_readouts(range: &Gd<godot::classes::Range>) {
    let value = range.get_value().to_variant();
    for link in range.get_signal_connection_list("value_changed").iter_shared() {
        let Some(cb) = link.get("callable").and_then(|c| c.try_to::<Callable>().ok()) else {
            continue;
        };
        if cb
            .object()
            .is_some_and(|o| o.try_cast::<mst_value_label::MstValueLabel>().is_ok())
        {
            cb.call(std::slice::from_ref(&value));
        }
    }
}
fn tint_swatch(c: [f32; 3]) -> Option<Gd<godot::classes::Texture2D>> {
    use godot::classes::{Image, ImageTexture, image::Format};
    let n = ui_kit::px(12.0).max(8);
    let mut img = Image::create_empty(n, n, false, Format::RGBA8)?;
    img.fill(Color::from_rgb(0.08, 0.08, 0.09));
    img.fill_rect(
        Rect2i::new(Vector2i::new(1, 1), Vector2i::new(n - 2, n - 2)),
        Color::from_rgb(c[0], c[1], c[2]),
    );
    Some(ImageTexture::create_from_image(&img)?.upcast())
}
