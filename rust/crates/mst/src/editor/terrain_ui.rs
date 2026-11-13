//! UI of the terrain plugin, reproducing the original MarchingSquaresTerrain editor:
//! left icon toolbar, bottom attribute strip (per-tool attributes, Chunk Manager and Terrain
//! Settings panels), right texture panel (Vertex Painting only), texture presets + export.

use godot::classes::control::SizeFlags;
use godot::classes::{
    AcceptDialog, Button, ButtonGroup, CheckBox, ColorPickerButton, ConfirmationDialog, Control, DirAccess, EditorInterface, EditorResourcePicker,
    EditorSpinSlider, HBoxContainer, HSeparator, HSlider, ImageTexture, Label, LineEdit, OptionButton, ResourceLoader, ResourceSaver, SpinBox,
    TextureRect, VBoxContainer, VFlowContainer,
};
use godot::prelude::*;

use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::{self, BRUSH_MODES, BRUSH_TYPES, EXTRA_TOOLS, GROUP_STARTS, Kind, MERGE_MODES, QUICK_PAINTS, SCATTER_ADD, SCATTER_MODES, Setting, TINT_COLORS, TOOLS, Tool};
use super::ui_kit::{self, Strip, sz};
use crate::terrain::textures::{MarchingSquaresTexturePreset, MarchingSquaresTextureSlot, SLOT_COUNT};

/// Folder scanned for texture presets; "Export Texture Preset" saves here.
pub const PRESET_DIR: &str = "res://addons/MarchingSquaresTerrain/texture_presets/";
/// Metadata on a terrain's (duplicated) preset remembering which file it came from.
const PRESET_META: &str = "mst_preset_path";
const PATTERNS: [&str; 14] = ["Grass", "Meadow", "Forest", "Sand", "Dirt", "Cliff", "Dark Rock", "Snow", "Gravel", "Clay", "Moss", "Path", "Plain", "Void"];

impl MarchingSquaresTerrainPlugin {
    fn cb(&self, method: &str) -> Callable {
        Callable::from_object_method(&self.to_gd(), method)
    }

    fn cb1(&self, method: &str, arg: impl ToGodot) -> Callable {
        self.cb(method).bind(&[arg.to_variant()])
    }

    // ------------------------------------------------------------------ toolbar

    pub(super) fn build_toolbar(&mut self) -> Gd<VFlowContainer> {
        let mut tb = ui_kit::Toolbar::new("MarchingSquaresToolbar");
        tb.separator();
        for (i, t) in TOOLS.iter().enumerate() {
            if GROUP_STARTS.contains(&i) {
                tb.separator();
            }
            let b = tb.button(t.name(), &t.tooltip(), ui_kit::icon(t.icon()), &self.cb1("on_tool", i as i64));
            self.tool_buttons.push(b);
        }
        // Rust tools, appended after the original nine behind a separator.
        tb.separator();
        for (k, t) in EXTRA_TOOLS.iter().enumerate() {
            let b = tb.button(t.name(), &t.tooltip(), ui_kit::icon(t.icon()), &self.cb1("on_tool", (TOOLS.len() + k) as i64));
            self.tool_buttons.push(b);
        }
        tb.separator();
        self.add_grid_toggle(&mut tb);
        tb.root
    }

    pub(super) fn select_tool(&mut self, t: Tool) {
        self.finish_stroke();
        // The Brush button remembers its mode (Drag / Raise-Lower).
        let t = match t {
            Tool::Brush | Tool::RaiseLower if self.state.raise_mode => Tool::RaiseLower,
            Tool::Brush | Tool::RaiseLower => Tool::Brush,
            t => t,
        };
        self.state.tool = t;
        self.selection.clear();
        self.extra.pasting = false;
        if let Some(i) = TOOLS.iter().position(|x| *x == t.button()).or_else(|| EXTRA_TOOLS.iter().position(|x| *x == t).map(|k| TOOLS.len() + k))
            && let Some(b) = self.tool_buttons.get_mut(i)
        {
            b.set_pressed_no_signal(true);
        }
        if t == Tool::ChunkManager
            && let Some(c) = self.selected_chunk
            && let Some(m) = self.terrain.as_ref().and_then(|tr| tr.bind().merge_mode_of(c))
        {
            self.state.merge_mode = m;
        }
        let paint = t == Tool::VertexPaint && self.visible;
        if let Some(p) = &mut self.texture_panel {
            p.set_visible(paint);
        }
        if t == Tool::VertexPaint {
            self.rebuild_texture_panel();
        }
        self.rebuild_strip();
        self.redraw();
        self.base().update_overlays();
    }

    /// Original size scale factor: brush ranges follow chunk and cell size.
    pub(super) fn size_scale(&self) -> f64 {
        let Some(t) = &self.terrain else { return 1.0 };
        let t = t.bind();
        let cfg = &t.store().config;
        let dim = cfg.chunk_size as f64 + 1.0;
        let cs = cfg.cell_size_safe();
        ((dim / 33.0 + dim / 33.0) / 2.0).clamp(0.5, 2.0) * ((cs.x + cs.y) as f64 / 4.0).clamp(0.3, 1.0) * if cfg.grid.is_cell_mode() { 2.0 } else { 1.0 }
    }

    // ------------------------------------------------------------------ attribute strip

    pub(super) fn rebuild_strip(&mut self) {
        let Some(root) = self.strip.clone() else { return };
        let mut strip = Strip::rebuild(root);
        self.setting_controls.clear();
        if self.state.tool == Tool::Stamp {
            self.stamp_library();
        }
        self.chunk_list = None;
        let tool = self.state.tool;
        let mut title = strip.title(tool.name(), ui_kit::icon(tool.icon()), &tool.tooltip());
        title.set_name("ToolTitle");
        match self.state.tool {
            Tool::ChunkManager => self.build_chunk_strip(&mut strip),
            Tool::TerrainSettings => self.build_settings_strip(&mut strip),
            t => {
                for &s in tools::settings_for(t) {
                    let c = self.build_setting(&mut strip, s, t);
                    let mut cc = c.clone();
                    cc.set_tooltip_text(tools::tooltip(s));
                    // Named after the attribute (scripts / tests can find e.g. "Falloff").
                    cc.set_name(tools::describe(s, t).0);
                    self.setting_controls.push((s, c));
                }
            }
        }
        self.refresh_dependent_states();
    }

    fn build_setting(&self, strip: &mut Strip, s: Setting, t: Tool) -> Gd<Control> {
        let (label, kind) = tools::describe(s, t);
        let v = self.state.get(s);
        let id = s as i64;
        match kind {
            Kind::HSlider(min, max, step) => {
                let (min, max) = if s == Setting::Size { (min * self.size_scale().min(1.0), max * self.size_scale()) } else { (min, max) };
                strip.hslider(label, min, max, step, v, &self.cb1("on_float", id)).upcast()
            }
            Kind::Spin(min, max, step) => strip.spin_slider(label, min, max, step, v, &self.cb1("on_float", id)).upcast(),
            Kind::Check => strip.checkbox(label, v != 0.0, &self.cb1("on_bool", id)).upcast(),
            Kind::Text => {
                let mut le = strip.line_edit(label, "New name here...", &self.cb1("on_text", id));
                le.set_text("");
                le.upcast()
            }
            Kind::Button(text) => strip.button("", text, &self.cb1("on_action", id)).upcast(),
            Kind::Option => {
                let names: Vec<String> = match s {
                    Setting::BrushType => BRUSH_TYPES.iter().map(|x| x.to_string()).collect(),
                    Setting::BrushMode => BRUSH_MODES.iter().map(|x| x.to_string()).collect(),
                    Setting::Material => self.slot_names(),
                    Setting::QuickPaint => QUICK_PAINTS.iter().map(|q| q.name.to_string()).collect(),
                    Setting::TexturePreset => {
                        let mut v = vec!["None".to_string()];
                        v.extend(self.list_presets().into_iter().map(|p| p.1));
                        v
                    }
                    Setting::ScatterItem => {
                        let v = self.terrain.as_ref().map(|t| t.bind().scatter_item_labels()).unwrap_or_default();
                        if v.is_empty() { vec!["(no items)".to_string()] } else { v }
                    }
                    Setting::ScatterAdd => SCATTER_ADD.iter().map(|x| x.to_string()).collect(),
                    Setting::ScatterMode => SCATTER_MODES.iter().map(|x| x.to_string()).collect(),
                    Setting::StampImage => self.stamp_names(),
                    Setting::StampMode => pt_core::stamp::STAMP_MODES_ALL.iter().map(|x| x.to_string()).collect(),
                    Setting::TintColor => TINT_COLORS.iter().map(|x| x.0.to_string()).collect(),
                    _ => Vec::new(),
                };
                let refs: Vec<&str> = names.iter().map(|x| x.as_str()).collect();
                let wide = matches!(s, Setting::TexturePreset | Setting::QuickPaint | Setting::Material | Setting::BrushMode | Setting::ScatterItem | Setting::ScatterAdd | Setting::StampImage);
                let selected = if s == Setting::TexturePreset { self.current_preset_index() } else { v as i32 };
                let mut ob = strip.option(label, &refs, selected, if wide { 100.0 } else { 65.0 }, &self.cb1("on_option", id));
                ob.set_name(label);
                if s == Setting::QuickPaint {
                    ob.set_item_tooltip(3, "Paints the selected Material (Vertex Painting) on the ground.");
                }
                if s == Setting::BrushMode {
                    for (i, icon) in ["brush_tool.svg", "raise_lower_tool.svg"].into_iter().enumerate() {
                        if let Some(ic) = ui_kit::icon(icon) {
                            ob.set_item_icon(i as i32, &ic);
                        }
                    }
                }
                if s == Setting::TintColor {
                    for (i, (_, c)) in TINT_COLORS.iter().enumerate() {
                        ob.set_item_icon(i as i32, &tint_swatch(*c));
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

    /// Writes a setting and mirrors it into its control without re-emitting signals.
    pub(super) fn set_setting(&mut self, s: Setting, v: f64) {
        self.state.set(s, v);
        let v = self.state.get(s);
        for (id, c) in &self.setting_controls {
            if *id != s {
                continue;
            }
            if let Ok(mut r) = c.clone().try_cast::<godot::classes::Range>() {
                r.set_value_no_signal(v);
            } else if let Ok(mut cb) = c.clone().try_cast::<CheckBox>() {
                cb.set_pressed_no_signal(v != 0.0);
            } else if let Ok(mut ob) = c.clone().try_cast::<OptionButton>() {
                ob.select(v as i32);
            }
        }
        self.refresh_dependent_states();
        self.base().update_overlays();
    }

    /// Grid Aligned only works on cell grids with hexagon/triangle-style stamps; Grid Size only when aligned.
    pub(super) fn refresh_dependent_states(&mut self) {
        let cell = self.terrain.as_ref().is_some_and(|t| t.bind().store().config.grid.is_cell_mode());
        let aligned_ok = cell && !self.state.brush.falloff;
        let aligned = self.state.brush.grid_aligned && aligned_ok;
        for (id, c) in &mut self.setting_controls {
            match id {
                Setting::GridAligned => {
                    ui_kit::set_enabled(c, aligned_ok);
                    c.set_tooltip_text(if aligned_ok {
                        tools::tooltip(Setting::GridAligned)
                    } else {
                        "Grid Aligned needs a Triangle/Hexagon/Hex Rings grid and Falloff off."
                    });
                }
                Setting::GridSize => ui_kit::set_enabled(c, aligned),
                Setting::Falloff if self.state.tool == Tool::Bridge => ui_kit::set_enabled(c, false),
                Setting::Ceiling | Setting::Height | Setting::Floor if self.state.tool == Tool::Stamp => ui_kit::set_enabled(c, self.state.stamp_limits),
                _ => {}
            }
        }
    }

    pub(super) fn slot_name(&self, i: usize) -> String {
        self.terrain
            .as_ref()
            .and_then(|t| t.bind().texture_set_ref())
            .map(|ts| ts.bind().slot_name(i))
            .unwrap_or_else(|| format!("Texture {}", i + 1))
    }

    pub(super) fn slot_names(&self) -> Vec<String> {
        (0..SLOT_COUNT).map(|i| format!("{}. {}", i + 1, self.slot_name(i))).collect()
    }

    /// Refreshes the Material dropdown labels (after renames / preset changes).
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

    // ------------------------------------------------------------------ chunk manager strip

    fn build_chunk_strip(&mut self, strip: &mut Strip) {
        let ob = strip.option("Chunk", &[], -1, 130.0, &self.cb("on_chunk_selected"));
        let mut ob = ob;
        ob.set_tooltip_text("Chunk to edit: its merge mode, vertex handles and Remove act on it (or Ctrl+LMB a chunk in the viewport).");
        self.chunk_list = Some(ob.clone());
        self.refresh_chunk_list();
        let n = self.terrain.as_ref().map(|t| t.bind().store().chunk_coords().len()).unwrap_or(0);
        strip.hint(&format!("{n} chunk{}", if n == 1 { "" } else { "s" }));
        let mut rm = strip.button("", "Remove Chunk", &self.cb("on_remove_selected_chunk"));
        rm.set_tooltip_text("Removes the selected chunk (undoable). In the viewport: LMB a chunk (red X) removes it, LMB a green + adds one.");
        ui_kit::set_enabled(&mut rm, self.selected_chunk.is_some());
        strip.separator();
        let mut mm = strip.option("Merge Mode", &MERGE_MODES, self.state.merge_mode, 120.0, &self.cb("on_merge_mode"));
        mm.set_tooltip_text("Merge mode of the selected chunk (Ctrl+LMB a chunk to select it).");
        strip.separator();
        let mut b = strip.button("", "Apply Mode to All Chunks", &self.cb("on_apply_merge_all"));
        b.set_tooltip_text("Applies the merge mode to every chunk of this terrain.");
        strip.separator();
        let handles = self.terrain.as_ref().is_some_and(|t| t.get("chunk_handles").to::<bool>());
        let mut h = strip.checkbox("Vertex Handles", handles, &self.cb("on_chunk_handles"));
        h.set_tooltip_text("Drag handles on every sample of the selected chunk (the original's chunk gizmo; one undo step per drag).");
        strip.separator();
        strip.hint("LMB +: add chunk  ·  LMB chunk: remove  ·  Ctrl+LMB: select");
    }

    pub(super) fn refresh_chunk_list(&mut self) {
        let Some(mut ob) = self.chunk_list.clone() else { return };
        if !ob.is_instance_valid() {
            self.chunk_list = None;
            return;
        }
        ob.clear();
        let coords = self.terrain.as_ref().map(|t| t.bind().store().chunk_coords()).unwrap_or_default();
        // Always have a chunk selected while there are chunks (the chunk at the origin, else
        // the first), so the list, merge mode and handles act on something visible.
        if self.selected_chunk.is_none_or(|s| !coords.contains(&s)) && !coords.is_empty() {
            let c = coords.iter().copied().find(|c| *c == pt_core::math::iv2(0, 0)).unwrap_or(coords[0]);
            self.selected_chunk = Some(c);
            if let Some(t) = self.terrain.as_mut() {
                t.set("handle_chunk", &crate::util::from_iv(c).to_variant());
            }
            if let Some(m) = self.terrain.as_ref().and_then(|t| t.bind().merge_mode_of(c)) {
                self.state.merge_mode = m;
            }
        }
        for (i, c) in coords.iter().enumerate() {
            ob.add_item(&format!("Chunk ({}, {})", c.x, c.y));
            ob.set_item_metadata(i as i32, &Vector2i::new(c.x, c.y).to_variant());
        }
        let sel = self.selected_chunk.and_then(|s| coords.iter().position(|c| *c == s)).map(|i| i as i32).unwrap_or(-1);
        ob.select(sel);
    }

    pub(super) fn select_chunk(&mut self, c: Option<pt_core::math::IVec2>) {
        self.selected_chunk = c;
        // The chunk vertex handles follow the selected chunk.
        if let (Some(c), Some(t)) = (c, self.terrain.as_mut()) {
            t.set("handle_chunk", &crate::util::from_iv(c).to_variant());
        }
        if let Some(c) = c
            && let Some(m) = self.terrain.as_ref().and_then(|t| t.bind().merge_mode_of(c))
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

// ---------------------------------------------------------------------- terrain settings

#[derive(Clone, Copy)]
enum TKind {
    Enum(&'static [&'static str]),
    Int(i64, i64),
    Float(f64, f64, f64),
    /// 0–1 `EditorSpinSlider` (thresholds).
    Unit,
    Vec2(f64, f64, f64),
    Vec2i(i64, i64),
    Bool,
    /// One of the 16 texture slots.
    Slot,
    /// Highest collision bit 9..32 ("None" = none).
    ExtraLayer,
    /// `animation_fps` of the grass material.
    GrassFps,
    /// Free text (`LineEdit`, applied on Enter).
    Text,
    /// `Noise` resource picker.
    Noise,
    /// Button running a storage / bake action (`prop` names the action).
    Action(&'static str),
}

struct TSetting {
    label: &'static str,
    prop: &'static str,
    kind: TKind,
    tip: &'static str,
}

const fn ts(label: &'static str, prop: &'static str, kind: TKind, tip: &'static str) -> TSetting {
    TSetting { label, prop, kind, tip }
}

const TERRAIN_SETTINGS: [TSetting; 40] = [
    ts("Grid Type", "grid_type", TKind::Enum(&["Square", "Triangle", "Hex", "Hex Rings"]), "Square = marching squares over vertex heights. Triangle / Hexagon / Hex Rings = flat cells."),
    ts("Chunk Size", "chunk_size", TKind::Int(2, 64), "Cells per chunk edge (the original's Dimensions)."),
    ts("Ring Radius", "ring_radius", TKind::Int(1, 16), "Hex Rings: radius of the hex disk each chunk owns."),
    ts("Cell Size", "cell_size", TKind::Vec2(0.1, 64.0, 0.1), "Cell size in world units (hex/triangle cells use X only)."),
    ts("Wrap Chunks", "wrap_chunks", TKind::Vec2i(0, 128), "Hex Rings: torus wrap in chunks (x = columns, y = rows); 0 = no wrap."),
    ts("Blend Mode", "blend_mode", TKind::Enum(&["Smooth", "Hard Squares", "Hard Vertices"]), "Texture blending between slots."),
    ts("Merge Mode", "default_merge_mode", TKind::Enum(&MERGE_MODES), "Merge mode given to newly added chunks."),
    ts("Edge Style", "hex_edge_style", TKind::Enum(&["Flat Walls", "Terraces", "Slopes", "Hex Prism"]), "Triangle / hex grids: steps up to the merge threshold become ramps (Flat Walls, Slopes) or stepped terraces (Terraces); larger steps are vertical cliffs, or sloped rock faces with Slopes. Hex Prism: stacked prism columns with flat tops, bevelled rims and a groove per level height (smooth flags ignored)."),
    ts("Instanced", "hex_edge_instanced", TKind::Bool, "Hex grids with Hex Prism: draw the columns as GPU instances (one MultiMesh instance per cell, rebuilt in the vertex shader from the cell map) instead of a meshed chunk; height edits only re-upload the map."),
    ts("Floor Slot", "default_floor_slot", TKind::Slot, "Floor texture of newly added chunks."),
    ts("Wall Slot", "default_wall_slot", TKind::Slot, "Wall texture of newly added chunks."),
    ts("Noise Amp", "new_chunk_noise_amplitude", TKind::Float(0.0, 256.0, 0.1), "Height amplitude of fractal noise for new chunks (0 = flat)."),
    ts("Noise Freq", "new_chunk_noise_frequency", TKind::Float(0.0, 4.0, 0.001), "Frequency of the new-chunk noise."),
    ts("Noise Seed", "noise_seed", TKind::Int(-2147483648, 2147483647), "Seed of the new-chunk noise."),
    ts("Noise Hmap", "noise_hmap", TKind::Noise, "Noise resource for new chunks (the original's noise_hmap): heights = noise x Noise Height. Overrides Noise Amp / Freq."),
    ts("Noise Height", "noise_hmap_height", TKind::Float(0.0, 1024.0, 0.5), "Height range of Noise Hmap (the original's dimensions.y)."),
    ts("Extra Collision Layer", "collision_layer", TKind::ExtraLayer, "Adds one extra physics layer (9-32) to the terrain collision."),
    ts("Grass", "grass_enabled", TKind::Bool, "Grow grass on slots with \"Has grass\"."),
    ts("Animation FPS", "", TKind::GrassFps, "> 0 animates the grass with limited fps; 0 = smooth wind."),
    ts("Subdivisions", "grass_subdivisions", TKind::Int(1, 8), "Grass blades per cell = subdivisions²."),
    ts("Grass Size", "grass_size", TKind::Vec2(0.05, 16.0, 0.05), "Grass blade size (width, height)."),
    ts("Ledge Cutoff", "grass_ledge_threshold", TKind::Unit, "Grass is not planted on cliff-edge bands stronger than this."),
    ts("Min Height", "grass_min_height", TKind::Float(-1024.0, 1024.0, 0.1), "No grass below this height (e.g. your water level)."),
    ts("Pixel Size", "pixel_size", TKind::Int(1, 4), "Pixel size multiplier in the pixel renderer."),
    ts("Editor Grid", "show_grid_in_editor", TKind::Bool, "Cell lines on the terrain while editing (in game: show_grid)."),
    ts("Chunk Lines", "grid_chunk_lines", TKind::Bool, "The grid overlay also draws chunk borders."),
    ts("Editor Fog", "fog_in_editor", TKind::Bool, "Show the fog of war (cell data) while editing."),
    ts("Debug View", "debug_view", TKind::Enum(&["Off", "Slope", "Height Contours", "Slot Ids", "Chunk Borders", "Cell Ids", "Cell Data"]), "Terrain debug view (shader)."),
    ts("LOD Distance", "lod_distance", TKind::Float(0.0, 4000.0, 1.0), "Distant low-detail ring: chunks farther than this from the LOD focus (lod_focus, else where the camera looks) get flat cell tops, plain walls, no grass and no props. 0 = off."),
    ts("LOD In Editor", "lod_in_editor", TKind::Bool, "Apply the distant LOD ring in the editor too (off: the editor always shows full detail)."),
    ts("Erosion", "", TKind::Action("erosion"), "Select this terrain's MarchingSquaresErosion child, adding one if there is none (undoable): Minor Deity's droplet erosion with an Erode button."),
    ts("Wild Flowers", "", TKind::Action("flowers"), "Select this terrain's MarchingSquaresWildFlowers child, adding one if there is none (undoable): Minor Deity's clustered wild flowers with Scatter / Clear buttons."),
    ts("Storage Mode", "storage_mode", TKind::Enum(&["Resource", "Baked", "Runtime"]), "Resource: all chunks in the data resource. Baked / Runtime: one file per chunk in Data Directory, written on scene save (only changed chunks); Baked also stores meshes for fast loading."),
    ts("Bake Grass", "bake_grass", TKind::Bool, "Baked storage: also store the grass multimeshes."),
    ts("Bake Collision", "bake_collision", TKind::Bool, "Baked storage: also store the collision faces."),
    ts("Data Directory", "data_directory", TKind::Text, "Folder of the per-chunk files (empty = generated next to the scene on save). Press Enter to apply."),
    ts("Save Data", "", TKind::Action("save"), "Write the terrain data now (changed chunks only)."),
    ts("Import MST", "", TKind::Action("import"), "Import the original MarchingSquaresTerrain's data: pick its scene (.tscn), its terrain data folder or a metadata.res."),
    ts("Bake Meshes", "", TKind::Action("bake"), "Bake the chunks into a static MeshInstance3D / StaticBody3D copy next to the terrain (undoable)."),
    ts("Export GLB", "", TKind::Action("glb"), "Geometry baker: export every chunk to the terrain's export_path as a GLB with a baked texture atlas (slot textures, blending, walls)."),
];

/// Terrain Settings label of a `MarchingSquaresTerrain` property (the inspector shows the same
/// label, `inspector_labels.rs`). Composite rows (extra collision layer, grass FPS) and
/// actions have no single property and are not listed.
pub(super) fn terrain_setting_label(prop: &str) -> Option<&'static str> {
    TERRAIN_SETTINGS
        .iter()
        .find(|t| !t.prop.is_empty() && t.prop == prop && !matches!(t.kind, TKind::ExtraLayer | TKind::GrassFps | TKind::Action(_)))
        .map(|t| t.label)
}

/// Pages of the Terrain Settings strip: (name, tooltip).
pub(super) const SETTINGS_PAGES: [(&str, &str); 6] = [
    ("Grid", "Grid type, chunk and cell size, blending, merge mode, collision."),
    ("New Chunks", "Textures and noise given to newly added chunks."),
    ("Grass", "Grass growth, animation and blade size."),
    ("Display", "Pixel size, grid overlay, fog and debug views."),
    ("LOD & Nature", "Distant low-detail ring; droplet erosion and wild flowers (child nodes with their own strip)."),
    ("Storage", "Where the terrain data lives; save, import, bake and export."),
];

fn settings_page_of(t: &TSetting) -> usize {
    match t.prop {
        "grid_type" | "chunk_size" | "ring_radius" | "cell_size" | "wrap_chunks" | "blend_mode" | "default_merge_mode" | "hex_edge_style" | "hex_edge_instanced" | "collision_layer" => 0,
        "default_floor_slot" | "default_wall_slot" | "new_chunk_noise_amplitude" | "new_chunk_noise_frequency" | "noise_seed" | "noise_hmap" | "noise_hmap_height" => 1,
        "" if matches!(t.kind, TKind::GrassFps) => 2,
        "grass_enabled" | "grass_subdivisions" | "grass_size" | "grass_ledge_threshold" | "grass_min_height" => 2,
        "pixel_size" | "show_grid_in_editor" | "grid_chunk_lines" | "fog_in_editor" | "debug_view" => 3,
        "lod_distance" | "lod_in_editor" => 4,
        "" if matches!(t.kind, TKind::Action("erosion" | "flowers")) => 4,
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

impl MarchingSquaresTerrainPlugin {
    fn terrain_prop(&self, prop: &str) -> Variant {
        self.terrain.as_ref().map(|t| t.get(prop)).unwrap_or_default()
    }

    fn grass_material(&self) -> Option<Gd<godot::classes::ShaderMaterial>> {
        self.terrain_prop("grass_material").try_to::<Gd<godot::classes::ShaderMaterial>>().ok()
    }

    fn build_settings_strip(&mut self, strip: &mut Strip) {
        let grid = self.terrain_prop("grid_type").try_to::<i32>().unwrap_or(0);
        let rings = grid == 3;
        let storage = self.terrain_prop("storage_mode").try_to::<i32>().unwrap_or(0);
        let page = self.settings_page.min(SETTINGS_PAGES.len() - 1);
        // Page tabs on the first row, the page's fields in columns directly under them (one
        // block next to the tool title, no empty space beside the tabs).
        let mut tabs = ui_kit::page_tabs("SettingsPages", &SETTINGS_PAGES, page, |k| self.cb1("on_settings_page", k as i64));
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
                "bake_grass" | "bake_collision" => storage == 1,
                "data_directory" => storage != 0,
                _ => !matches!(t.kind, TKind::Action("save")) || storage != 0,
            })
            .collect();
        let built: Vec<Gd<HBoxContainer>> = rows
            .iter()
            .map(|(i, t)| {
                let mut c = self.build_terrain_setting(*i, t);
                c.set_tooltip_text(t.tip);
                let mut row = ui_kit::settings_row(t.label, &c);
                row.set_tooltip_text(t.tip);
                row
            })
            .collect();
        // As few rows per column as fit next to the title (2 at least): a short strip leaves
        // the viewport as tall as possible.
        let widths: Vec<f32> = built.iter().map(|r| r.get_combined_minimum_size().x).collect();
        let lead = strip.flow.get_children().iter_shared().filter_map(|c| c.try_cast::<Control>().ok()).map(|c| c.get_combined_minimum_size().x).sum::<f32>() + sz(64.0, 0.0).x;
        let avail = {
            let w = strip.root.get_size().x;
            if w > 1.0 { w } else { EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done().map(|v| v.get_visible_rect().size.x).unwrap_or(1200.0) }
        };
        // Per column: its widest row, the separator and the box separation on both sides of it.
        let gap = sz(ui_kit::CELL_GAP * 2.0 + 8.0, 0.0).x;
        let min_rows = 2.min(built.len().max(1));
        let per_col = (min_rows..=built.len().max(min_rows))
            .find(|r| lead + widths.chunks(*r).map(|c| c.iter().cloned().fold(0.0, f32::max) + gap).sum::<f32>() <= avail)
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
    }

    fn build_terrain_setting(&self, i: usize, t: &TSetting) -> Gd<Control> {
        let v = if t.prop.is_empty() { Variant::nil() } else { self.terrain_prop(t.prop) };
        let id = i as i64;
        let on = |comp: i64| self.cb("on_terrain_setting").bind(&[id.to_variant(), comp.to_variant()]);
        match t.kind {
            TKind::Enum(items) => {
                let mut ob = ui_kit::option_button(items, v.try_to::<i32>().unwrap_or(0), 100.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::Slot => {
                let names = self.slot_names();
                let refs: Vec<&str> = names.iter().map(|x| x.as_str()).collect();
                let mut ob = ui_kit::option_button(&refs, v.try_to::<i32>().unwrap_or(0), 100.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::ExtraLayer => {
                let mut items = vec!["None".to_string()];
                items.extend((9..=32).map(|l| l.to_string()));
                let refs: Vec<&str> = items.iter().map(|x| x.as_str()).collect();
                let mask = v.try_to::<i64>().unwrap_or(1) as u32;
                let sel = (9..=32).rev().find(|l| mask & (1 << (l - 1)) != 0).map(|l| l - 8).unwrap_or(0);
                let mut ob = ui_kit::option_button(&refs, sel as i32, 65.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::Int(min, max) => {
                let mut sb = spin(min as f64, max as f64, 1.0, v.try_to::<i64>().unwrap_or(0) as f64);
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Float(min, max, step) => {
                let mut sb = spin(min, max, step, v.try_to::<f64>().unwrap_or(0.0));
                sb.set_allow_greater(true);
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::GrassFps => {
                let fps = self.grass_material().map(|m| m.get_shader_parameter("animation_fps")).and_then(|v| v.try_to::<f64>().ok()).unwrap_or(0.0);
                let mut sb = spin(0.0, 60.0, 1.0, fps);
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Unit => {
                let mut s = EditorSpinSlider::new_alloc();
                s.set_min(0.0);
                s.set_max(1.0);
                s.set_step(0.01);
                s.set_value_no_signal(v.try_to::<f64>().unwrap_or(0.0));
                s.set_flat(true);
                s.set_custom_minimum_size(sz(105.0, 25.0));
                s.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                s.connect("value_changed", &on(0));
                s.upcast()
            }
            TKind::Bool => {
                let mut cb = CheckBox::new_alloc();
                cb.set_flat(true);
                cb.set_pressed_no_signal(v.try_to::<bool>().unwrap_or(false));
                cb.connect("toggled", &on(0));
                cb.upcast()
            }
            TKind::Vec2(min, max, step) => {
                let val = v.try_to::<Vector2>().unwrap_or(Vector2::ONE);
                let mut hb = HBoxContainer::new_alloc();
                for (k, c) in [val.x, val.y].into_iter().enumerate() {
                    let mut sb = spin(min, max, step, c as f64);
                    sb.set_allow_greater(true);
                    sb.set_prefix(if k == 0 { "x" } else { "y" });
                    sb.connect("value_changed", &on(k as i64));
                    hb.add_child(&sb);
                }
                hb.upcast()
            }
            TKind::Text => {
                let mut le = LineEdit::new_alloc();
                le.set_text(&v.try_to::<GString>().unwrap_or_default());
                le.set_placeholder("(generated on save)");
                le.set_custom_minimum_size(sz(180.0, 25.0));
                le.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                le.connect("text_submitted", &on(0));
                le.upcast()
            }
            TKind::Noise => {
                let mut picker = EditorResourcePicker::new_alloc();
                picker.set_base_type("Noise");
                if let Ok(r) = v.try_to::<Gd<godot::classes::Resource>>() {
                    picker.set_edited_resource(&r);
                }
                picker.set_custom_minimum_size(sz(150.0, 25.0));
                picker.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                picker.connect("resource_changed", &on(0));
                picker.upcast()
            }
            TKind::Action(a) => {
                let mut b = Button::new_alloc();
                b.set_text(match a {
                    "save" => "Save",
                    "import" => "Import...",
                    "bake" => "Bake",
                    "erosion" | "flowers" => {
                        let class = if a == "erosion" { "MarchingSquaresErosion" } else { "MarchingSquaresWildFlowers" };
                        if self.terrain_child(class).is_some() { "Select" } else { "Add" }
                    }
                    _ => "Export",
                });
                b.set_name(t.label);
                b.set_custom_minimum_size(sz(70.0, 25.0));
                b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                // `pressed` has no arguments: pass a nil value in front of the bound index.
                b.connect("pressed", &self.cb("on_terrain_setting").bind(&[Variant::nil(), id.to_variant(), 0i64.to_variant()]));
                b.upcast()
            }
            TKind::Vec2i(min, max) => {
                let val = v.try_to::<Vector2i>().unwrap_or(Vector2i::ZERO);
                let mut hb = HBoxContainer::new_alloc();
                for (k, c) in [val.x, val.y].into_iter().enumerate() {
                    let mut sb = spin(min as f64, max as f64, 1.0, c as f64);
                    sb.set_prefix(if k == 0 { "x" } else { "y" });
                    sb.connect("value_changed", &on(k as i64));
                    hb.add_child(&sb);
                }
                hb.upcast()
            }
        }
    }

    /// A Terrain Settings control changed: write the property through the undo manager.
    pub(super) fn terrain_setting_changed(&mut self, value: Variant, index: usize, comp: i64) {
        let Some(t) = TERRAIN_SETTINGS.get(index) else { return };
        let Some(terrain) = self.terrain.clone() else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.clone().upcast::<Object>();
        if let TKind::GrassFps = t.kind {
            let Some(mat) = self.grass_material() else { return };
            let old = mat.get_shader_parameter("animation_fps");
            let new = value.try_to::<f64>().unwrap_or(0.0) as f32;
            ur.create_action("terrain grass animation fps");
            let m = mat.upcast::<Object>();
            ur.add_do_method(&m, "set_shader_parameter", &["animation_fps".to_variant(), new.to_variant()]);
            ur.add_undo_method(&m, "set_shader_parameter", &["animation_fps".to_variant(), old]);
            ur.commit_action();
            return;
        }
        if let TKind::Action(a) = t.kind {
            self.terrain_action(a);
            return;
        }
        let old = terrain.get(t.prop);
        let new: Variant = match t.kind {
            TKind::Text => GString::from(value.try_to::<GString>().unwrap_or_default().to_string().trim()).to_variant(),
            TKind::Noise => value.clone(),
            TKind::Action(_) => unreachable!(),
            TKind::Enum(_) | TKind::Slot | TKind::Int(..) => (value.try_to::<f64>().unwrap_or(0.0).round() as i64).to_variant(),
            TKind::Float(..) | TKind::Unit => value.try_to::<f64>().unwrap_or(0.0).to_variant(),
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
                if comp == 0 { v.x = x } else { v.y = x }
                v.to_variant()
            }
            TKind::Vec2i(..) => {
                let mut v = old.try_to::<Vector2i>().unwrap_or(Vector2i::ZERO);
                let x = value.try_to::<f64>().unwrap_or(0.0).round() as i32;
                if comp == 0 { v.x = x } else { v.y = x }
                v.to_variant()
            }
            TKind::GrassFps => unreachable!(),
        };
        if new == old {
            return;
        }
        ur.create_action(&format!("terrain setting {}", t.label.to_lowercase()));
        ur.add_do_property(&obj, t.prop, &new);
        ur.add_undo_property(&obj, t.prop, &old);
        ur.commit_action();
        if t.prop == "show_grid_in_editor" {
            self.sync_grid_toggle();
        }
        if matches!(t.prop, "grid_type" | "chunk_size" | "cell_size" | "ring_radius" | "storage_mode") {
            // Layout / brush ranges depend on these: rebuild after the signal returns.
            self.selection.clear();
            self.rebuild_strip_deferred();
        }
        self.redraw();
    }
}

// ---------------------------------------------------------------------- storage actions

impl MarchingSquaresTerrainPlugin {
    fn terrain_action(&mut self, action: &str) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        match action {
            "save" => {
                terrain.call("save_terrain_data", &[]);
            }
            "bake" => {
                terrain.call("bake_static_meshes", &[]);
            }
            "glb" => {
                let path = terrain.get("export_path");
                // Geometry baker: textured atlas per chunk (16 px per triangle pair).
                let err = terrain.call("export_glb_baked", &[path.clone(), 16.to_variant()]);
                godot_print!("Marching Squares Terrain: Export GLB to {path} -> {err}");
            }
            "import" => self.open_import_dialog(),
            "erosion" => self.add_or_select_child("MarchingSquaresErosion", "Erosion"),
            "flowers" => self.add_or_select_child("MarchingSquaresWildFlowers", "WildFlowers"),
            _ => {}
        }
    }

    /// First child of the edited terrain of `class`.
    fn terrain_child(&self, class: &str) -> Option<Gd<Node>> {
        let t = self.terrain.as_ref()?;
        t.get_children().iter_shared().find(|c| c.is_class(class))
    }

    /// Selects the terrain's `class` child (its MST-style strip has the Erode / Scatter / Clear
    /// buttons), adding one first when there is none (one undo step).
    fn add_or_select_child(&mut self, class: &str, name: &str) {
        let Some(terrain) = self.terrain.clone() else { return };
        let node = match self.terrain_child(class) {
            Some(n) => n,
            None => {
                let Ok(mut n) = godot::classes::ClassDb::singleton().instantiate(class).try_to::<Gd<Node>>() else { return };
                n.set_name(name);
                let owner = EditorInterface::singleton().get_edited_scene_root().unwrap_or_else(|| terrain.clone().upcast());
                let Some(mut ur) = self.base().get_undo_redo() else { return };
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
        let mut d = EditorFileDialog::new_alloc();
        d.set_title("Import original MarchingSquaresTerrain data (scene, data folder or metadata.res)");
        d.set_file_mode(FileMode::OPEN_ANY);
        d.set_access(Access::FILESYSTEM);
        d.add_filter("*.tscn ; Original scene (reads grid settings + data_directory)");
        d.add_filter("*.res ; MSTChunkData (metadata.res)");
        let on = self.cb("on_import_mst_path").bind(&[d.to_variant()]);
        d.connect("file_selected", &on);
        d.connect("dir_selected", &on);
        let free = Callable::from_object_method(&d, "queue_free");
        d.connect("canceled", &free);
        if let Some(mut base) = EditorInterface::singleton().get_base_control() {
            base.add_child(&d);
        }
        d.popup_file_dialog();
    }
}

#[godot_api(secondary)]
impl MarchingSquaresTerrainPlugin {
    /// Import dialog result: one undoable action (`import_mst` / restore the old data + grid settings).
    #[func]
    fn on_import_mst_path(&mut self, path: GString, mut dialog: Gd<godot::classes::Node>) {
        dialog.queue_free();
        let Some(terrain) = self.terrain.clone() else { return };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.clone().upcast::<Object>();
        ur.create_action("Import MST terrain data");
        ur.add_do_method(&obj, "import_mst", &[path.to_variant()]);
        for prop in ["data", "grid_type", "cell_size", "blend_mode", "default_wall_slot", "grass_subdivisions", "grass_size"] {
            ur.add_undo_property(&obj, prop, &terrain.get(prop));
        }
        ur.add_undo_method(&obj, "rebuild", &[]);
        ur.commit_action();
        self.selection.clear();
        self.refresh_chunk_list();
        self.rebuild_strip_deferred();
        self.redraw();
    }
}

// ---------------------------------------------------------------------- texture panel

impl MarchingSquaresTerrainPlugin {
    pub(super) fn texture_set(&self) -> Option<Gd<MarchingSquaresTexturePreset>> {
        self.terrain.as_ref().and_then(|t| t.bind().texture_set_ref())
    }

    fn slot_res(&self, i: usize) -> Option<Gd<MarchingSquaresTextureSlot>> {
        self.texture_set().and_then(|ts| ts.bind().slot(i))
    }

    fn slot_preview(&self, i: usize) -> Option<Gd<ImageTexture>> {
        let img = self.texture_set()?.bind().preview_image(i, 32)?;
        ImageTexture::create_from_image(&img)
    }

    pub(super) fn rebuild_texture_panel(&mut self) {
        let Some(mut vbox) = self.texture_vbox.clone() else { return };
        for c in vbox.get_children().iter_shared() {
            vbox.remove_child(&c);
            c.free();
        }
        self.slot_buttons.clear();
        self.slot_previews.clear();
        self.slot_tex_pickers.clear();
        vbox.add_child(&ui_kit::panel_title(
            "Textures",
            ui_kit::icon("vertex_paint_tool.svg"),
            "Texture slots of this terrain's texture set. Click a slot to paint with it (same as the Material dropdown).",
        ));
        if self.texture_set().is_none() {
            let mut l = Label::new_alloc();
            l.set_text("No texture set. Pick a Texture Preset in the bottom bar.");
            l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
            l.set_modulate(ui_kit::DIM_MODULATE);
            vbox.add_child(&l);
            return;
        }
        let group = ButtonGroup::new_gd();
        for i in 0..SLOT_COUNT {
            let Some(slot) = self.slot_res(i) else { continue };
            let (name, tex, pattern, scale, tint, has_grass, grass_tint, sprite) = {
                let b = slot.bind();
                (b.display_name.clone(), b.texture.clone(), b.pattern, b.scale, b.tint, b.has_grass, b.grass_tint, b.grass_sprite.clone())
            };
            let void = i == SLOT_COUNT - 1;

            // Header: preview + "Texture N" select button + name.
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
            sel.set_text(&format!("Texture {}{}", i + 1, if void { " (void)" } else { "" }));
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
            col.add_child(&le);
            head.add_child(&col);
            vbox.add_child(&head);

            // Texture picker (empty = procedural pattern below). It fills the row, so it shows a
            // thumbnail of the texture, or a readable "<empty>" while none is set.
            let mut picker = EditorResourcePicker::new_alloc();
            picker.set_base_type("Texture2D");
            picker.set_custom_minimum_size(sz(60.0, 25.0));
            picker.set_tooltip_text("Ground texture of this slot (white-ish pixel art, tinted by Color). Empty = the procedural Pattern.");
            if let Some(t) = &tex {
                picker.set_edited_resource(&t.clone().upcast::<godot::classes::Resource>());
            }
            picker.connect("resource_changed", &self.cb1("on_slot_texture", i as i64));
            self.slot_tex_pickers.push((i, picker.clone()));
            vbox.add_child(&ui_kit::form_row("Texture", &picker));

            if !void {
                let mut pat = ui_kit::compact_option(&PATTERNS[..13], pattern.clamp(0, 12));
                pat.set_tooltip_text("Ground pattern: picks the bundled ground texture of this pattern (textures/ground/). Without a texture the pattern is generated procedurally.");
                pat.connect("item_selected", &self.cb1("on_slot_pattern", i as i64));
                vbox.add_child(&ui_kit::form_row("Pattern", &pat));

                // Scale: slider + value read-out.
                let mut row = HBoxContainer::new_alloc();
                let mut slider = HSlider::new_alloc();
                slider.set_min(0.1);
                slider.set_max(40.0);
                slider.set_step(0.1);
                slider.set_value_no_signal(scale as f64);
                slider.set_custom_minimum_size(sz(50.0, 20.0));
                slider.set_h_size_flags(SizeFlags::EXPAND_FILL);
                slider.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                slider.set_tooltip_text("Texture / pattern scale.");
                let value = ui_kit::value_label(scale as f64, 0.1, 30.0);
                slider.connect("value_changed", &self.cb("on_slot_scale").bind(&[(i as i64).to_variant(), value.to_variant()]));
                row.add_child(&slider);
                row.add_child(&value);
                vbox.add_child(&ui_kit::form_row("Scale", &row));

                let mut cp = ColorPickerButton::new_alloc();
                cp.set_pick_color(tint);
                cp.set_custom_minimum_size(sz(40.0, 25.0));
                cp.set_edit_alpha(false);
                cp.set_tooltip_text("Ground color (multiplies the texture / pattern).");
                cp.connect("color_changed", &self.cb1("on_slot_tint", i as i64));
                vbox.add_child(&ui_kit::form_row("Color", &cp));

                let mut grass = HBoxContainer::new_alloc();
                let mut cb = CheckBox::new_alloc();
                cb.set_pressed_no_signal(has_grass);
                cb.set_tooltip_text("Grow grass on this texture.");
                cb.connect("toggled", &self.cb1("on_slot_has_grass", i as i64));
                grass.add_child(&cb);
                let mut gp = ColorPickerButton::new_alloc();
                gp.set_pick_color(grass_tint);
                gp.set_tooltip_text("Grass color on this texture.");
                gp.set_custom_minimum_size(sz(40.0, 25.0));
                gp.set_h_size_flags(SizeFlags::EXPAND_FILL);
                gp.set_edit_alpha(false);
                gp.connect("color_changed", &self.cb1("on_slot_grass_tint", i as i64));
                grass.add_child(&gp);
                vbox.add_child(&ui_kit::form_row("Grass", &grass));

                // Grass instance sprite (original: grass_sprite_tex_N); empty = terrain default.
                let mut sp = EditorResourcePicker::new_alloc();
                sp.set_base_type("Texture2D");
                sp.set_custom_minimum_size(sz(60.0, 25.0));
                sp.set_tooltip_text("Grass sprite on this texture: white leaves on alpha, colored by Grass (original MST: grass_sprite_tex_N). Empty = the terrain's grass texture.");
                if let Some(t) = &sprite {
                    sp.set_edited_resource(&t.clone().upcast::<godot::classes::Resource>());
                }
                sp.connect("resource_changed", &self.cb1("on_slot_grass_sprite", i as i64));
                vbox.add_child(&ui_kit::form_row("Sprite", &sp));
            }
            vbox.add_child(&HSeparator::new_alloc());
        }
        self.build_auto_rules_section(&mut vbox);
        let mut m = godot::classes::MarginContainer::new_alloc();
        m.add_theme_constant_override("margin_bottom", 7);
        let mut b = Button::new_alloc();
        b.set_text("Export Texture Preset");
        b.set_tooltip_text(&format!("Save this terrain's texture set as a preset in {PRESET_DIR}."));
        b.set_h_size_flags(SizeFlags::EXPAND_FILL);
        b.connect("pressed", &self.cb("on_export_preset"));
        m.add_child(&b);
        vbox.add_child(&m);
    }

    pub(super) fn refresh_texture_panel_selection(&mut self) {
        let slot = self.state.paint_slot as usize;
        if let Some(b) = self.slot_buttons.get_mut(slot)
            && b.is_instance_valid()
        {
            b.set_pressed_no_signal(true);
        }
    }

    pub(super) fn refresh_slot_preview(&mut self, i: usize) {
        let p = self.slot_preview(i);
        if let (Some(r), Some(p)) = (self.slot_previews.get_mut(i), p)
            && r.is_instance_valid()
        {
            r.set_texture(&p);
        }
    }

    /// Sets a slot property through the undo manager (original: no undo).
    pub(super) fn set_slot_prop(&mut self, i: usize, prop: &str, value: Variant) {
        let Some(slot) = self.slot_res(i) else { return };
        let old = slot.get(prop);
        if old == value {
            return;
        }
        let obj = slot.upcast::<Object>();
        if let Some(mut ur) = self.base().get_undo_redo() {
            ur.create_action_ex(&format!("terrain texture {} {}", i + 1, prop)).merge_mode(godot::classes::undo_redo::MergeMode::ENDS).done();
            ur.add_do_property(&obj, prop, &value);
            ur.add_undo_property(&obj, prop, &old);
            ur.commit_action();
        }
        if matches!(prop, "texture" | "pattern" | "tint") {
            self.refresh_slot_preview(i);
        }
    }

    /// Pattern change: also switches the slot to the bundled ground texture of that pattern
    /// (one undo step), so the choice is visible whether or not the slot had a texture.
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
        self.refresh_slot_preview(i);
        if let Some((_, p)) = self.slot_tex_pickers.iter_mut().find(|(k, p)| *k == i && p.is_instance_valid()) {
            p.set_edited_resource(tex.map(|t| t.upcast::<godot::classes::Resource>()).as_ref());
        }
    }

    // ------------------------------------------------------------------ presets

    /// `(path, display name)` of every texture preset in [`PRESET_DIR`].
    pub(super) fn list_presets(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let Some(dir) = DirAccess::open(PRESET_DIR) else { return out };
        let mut files: Vec<String> = dir.get_files().as_slice().iter().map(|f| f.to_string()).collect();
        files.sort();
        for f in files {
            let f = f.trim_end_matches(".remap").to_string();
            if !(f.ends_with(".tres") || f.ends_with(".res")) {
                continue;
            }
            let path = format!("{PRESET_DIR}{f}");
            let Some(res) = ResourceLoader::singleton().load(&path) else { continue };
            let Ok(p) = res.try_cast::<MarchingSquaresTexturePreset>() else { continue };
            let name = p.bind().preset_name.to_string();
            let name = if name.is_empty() { f.trim_end_matches(".tres").trim_end_matches(".res").replace('_', " ") } else { name };
            out.push((path, name));
        }
        out
    }

    fn current_preset_index(&self) -> i32 {
        let Some(ts) = self.texture_set() else { return 0 };
        let path = if ts.has_meta(PRESET_META) { ts.get_meta(PRESET_META).to_string() } else { ts.get_path().to_string() };
        self.list_presets().iter().position(|p| p.0 == path).map(|i| i as i32 + 1).unwrap_or(0)
    }

    /// Applies preset `index` (0 = "None" = fresh defaults) as a *copy*, so painting/editing
    /// never changes the preset file.
    pub(super) fn apply_preset(&mut self, index: usize) {
        let Some(terrain) = self.terrain.clone() else { return };
        let new: Gd<MarchingSquaresTexturePreset> = if index == 0 {
            let mut p = MarchingSquaresTexturePreset::new_default();
            // Short name in the inspector's resource picker (else the long class name).
            p.set_name("Textures");
            p
        } else {
            let Some((path, _)) = self.list_presets().into_iter().nth(index - 1) else { return };
            let Some(res) = ResourceLoader::singleton().load(&path) else { return };
            let Some(dup) = res.duplicate_resource_ex().deep_internal().done_or_null() else { return };
            let Ok(mut p) = dup.try_cast::<MarchingSquaresTexturePreset>() else { return };
            p.set_meta(PRESET_META, &path.to_variant());
            let name = p.bind().preset_name.to_string();
            p.set_name(if name.is_empty() { "Textures" } else { name.as_str() });
            p
        };
        let old = terrain.get("texture_set");
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.upcast::<Object>();
            ur.create_action("terrain texture preset");
            ur.add_do_property(&obj, "texture_set", &new.to_variant());
            ur.add_undo_property(&obj, "texture_set", &old);
            ur.commit_action();
        }
        self.refresh_material_names();
        if self.state.tool == Tool::VertexPaint {
            self.to_gd().call_deferred("rebuild_texture_panel_now", &[]);
        }
    }

    pub(super) fn open_export_dialog(&mut self) {
        let mut base = EditorInterface::singleton().get_base_control().expect("editor base control");
        let mut dlg = AcceptDialog::new_alloc();
        dlg.set_title("Save Preset");
        dlg.set_size(Vector2i::new((400.0 * ui_kit::editor_scale()) as i32, (150.0 * ui_kit::editor_scale()) as i32));
        let mut v = VBoxContainer::new_alloc();
        let mut l = Label::new_alloc();
        l.set_text(&format!("Preset name (saved to {PRESET_DIR}):"));
        v.add_child(&l);
        let mut le = LineEdit::new_alloc();
        let current = self.texture_set().map(|t| t.bind().preset_name.to_string()).filter(|n| !n.is_empty());
        le.set_text(current.as_deref().unwrap_or("new_texture_preset"));
        le.select_all();
        v.add_child(&le);
        dlg.add_child(&v);
        dlg.register_text_enter(&le);
        let (on_ok, on_cancel) = (self.cb("on_export_confirmed").bind(&[le.to_variant(), dlg.to_variant()]), Callable::from_object_method(&dlg, "queue_free"));
        dlg.connect("confirmed", &on_ok);
        dlg.connect("canceled", &on_cancel);
        base.add_child(&dlg);
        dlg.popup_centered();
        le.grab_focus();
    }

    /// Saves the terrain's texture set as `<PRESET_DIR>/<snake_name>.tres` (asks before overwriting).
    pub(super) fn export_preset(&mut self, name: String, overwrite: bool) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let file = snake_case(&name);
        let path = format!("{PRESET_DIR}{file}.tres");
        if !overwrite && ResourceLoader::singleton().exists(&path) {
            let mut base = EditorInterface::singleton().get_base_control().expect("editor base control");
            let mut c = ConfirmationDialog::new_alloc();
            c.set_title("Overwrite File?");
            c.set_text(&format!("{path} already exists. Overwrite it?"));
            let (on_ok, on_cancel) = (self.cb("on_export_overwrite").bind(&[name.to_variant(), c.to_variant()]), Callable::from_object_method(&c, "queue_free"));
            c.connect("confirmed", &on_ok);
            c.connect("canceled", &on_cancel);
            base.add_child(&c);
            c.popup_centered();
            return;
        }
        let Some(ts) = self.texture_set() else { return };
        let Some(dup) = ts.duplicate_resource_ex().deep_internal().done_or_null() else { return };
        let Ok(mut p) = dup.try_cast::<MarchingSquaresTexturePreset>() else { return };
        p.bind_mut().preset_name = name.as_str().into();
        p.set_name(name.as_str());
        if p.has_meta(PRESET_META) {
            p.remove_meta(PRESET_META);
        }
        let _ = DirAccess::make_dir_recursive_absolute(PRESET_DIR);
        let err = ResourceSaver::singleton().save_ex(&p).path(&path).done();
        if err != godot::global::Error::OK {
            godot_error!("Marching Squares Terrain: could not save preset {path}: {err:?}");
            return;
        }
        if let Some(mut fs) = EditorInterface::singleton().get_resource_filesystem() {
            fs.update_file(&path);
        }
        // The terrain now "uses" that preset (as a copy).
        let mut ts = ts;
        ts.set_meta(PRESET_META, &path.to_variant());
        ts.bind_mut().preset_name = name.as_str().into();
        ts.set_name(name.as_str());
        godot_print!("Marching Squares Terrain: saved texture preset {path}");
        if self.state.tool == Tool::VertexPaint {
            self.rebuild_strip_deferred();
        }
    }
}

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_us = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !prev_us && !out.is_empty() && out.chars().last().is_some_and(|c| c.is_ascii_lowercase()) {
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
    if out.is_empty() { "texture_preset".into() } else { out }
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

/// Small color swatch icon for the Tint dropdown.
fn tint_swatch(c: [f32; 3]) -> Gd<godot::classes::Texture2D> {
    use godot::classes::{Image, ImageTexture, image::Format};
    let n = ui_kit::px(12.0).max(8);
    let mut img = Image::create_empty(n, n, false, Format::RGBA8).expect("swatch image");
    img.fill(Color::from_rgb(0.08, 0.08, 0.09));
    img.fill_rect(Rect2i::new(Vector2i::new(1, 1), Vector2i::new(n - 2, n - 2)), Color::from_rgb(c[0], c[1], c[2]));
    ImageTexture::create_from_image(&img).expect("swatch texture").upcast()
}
