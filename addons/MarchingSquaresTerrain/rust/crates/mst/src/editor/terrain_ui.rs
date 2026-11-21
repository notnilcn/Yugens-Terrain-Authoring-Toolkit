//! UI of the terrain plugin, reproducing the original MarchingSquaresTerrain editor:
//! left icon toolbar, bottom attribute strip (per-tool attributes, Chunk Manager and Terrain
//! Settings panels), right texture panel (Vertex Painting only), texture presets + export.

use godot::classes::control::SizeFlags;
use godot::classes::{
    Button, ButtonGroup, CheckBox, ColorPickerButton, ConfirmationDialog, Control,
    DirAccess, EditorInterface, EditorResourcePicker, EditorSpinSlider, HBoxContainer, HSeparator,
    HSlider, ImageTexture, Label, LineEdit, OptionButton, ResourceLoader, ResourceSaver, SpinBox,
    TextureRect, VBoxContainer, VFlowContainer,
};
use godot::prelude::*;

use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::{
    self, BRUSH_MODES, BRUSH_TYPES, EXTRA_TOOLS, GROUP_STARTS, Kind, MERGE_MODES, QUICK_PAINTS,
    SMOOTH_MODES, Setting, TINT_COLORS, TOOLS, Tool,
};
use super::ui_kit::{self, Strip, sz};
use crate::terrain::textures::{
    MarchingSquaresTexturePreset, MarchingSquaresTextureSlot, SLOT_COUNT,
};

/// Folder scanned for texture presets; "Export Texture Preset" saves here.
pub const PRESET_DIR: &str = "res://addons/MarchingSquaresTerrain/texture_presets/";
/// Metadata on a terrain's (duplicated) preset remembering which file it came from.
const PRESET_META: &str = "mst_preset_path";
/// Number carried by a control's signal: `item_selected` sends an int, `value_changed` a
/// float, `toggled` a bool. `Variant::try_to::<f64>` is strict (an int variant fails), so read
/// whichever it is.
pub(super) fn setting_num(v: &Variant) -> f64 {
    v.try_to::<f64>()
        .or_else(|_| v.try_to::<i64>().map(|i| i as f64))
        .or_else(|_| v.try_to::<bool>().map(|b| b as i32 as f64))
        .unwrap_or(0.0)
}

/// Name of the toolbar's compact-mode switch.
const COMPACT_BUTTON: &str = "ToolbarCompact";
/// Editor project metadata (section, key) remembering the toolbar's compact mode.
const TOOLBAR_META: (&str, &str) = ("marching_squares_terrain", "toolbar_compact");

fn toolbar_compact() -> bool {
    ui_kit::editor_state(TOOLBAR_META.1)
        .map(|v| v.booleanize())
        .unwrap_or(false)
}

/// Arrow of the compact switch: points the way the toolbar will grow / shrink.
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

    // ------------------------------------------------------------------ toolbar

    pub(super) fn build_toolbar(&mut self) -> Gd<VFlowContainer> {
        let mut tb = ui_kit::Toolbar::new("MarchingSquaresToolbar");
        let compact = toolbar_compact();
        tb.deed(
            COMPACT_BUTTON,
            "Collapse",
            "Show / hide the tool names (icons only).",
            compact_icon(compact),
            &self.cb("on_toolbar_compact"),
        );
        tb.separator();
        let all = TOOLS.iter().enumerate().chain(
            EXTRA_TOOLS
                .iter()
                .enumerate()
                .map(|(k, t)| (TOOLS.len() + k, t)),
        );
        for (i, t) in all {
            // Groups: sculpt | paint | inspect + manage | Rust extras.
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
        self.add_grid_toggle(&mut tb);
        tb.assign_terse(compact);
        tb.base
    }

    /// Toolbar compact-mode switch: shows / hides the tool labels and remembers the choice per
    /// project (editor project metadata).
    pub(super) fn toggle_toolbar_compact(&mut self) {
        let compact = !toolbar_compact();
        ui_kit::set_editor_state(TOOLBAR_META.1, &compact.to_variant());
        let Some(tb) = self.toolbar.clone() else {
            return;
        };
        ui_kit::set_toolbar_compact(&tb, compact);
        if let Some(b) = tb.find_child_ex(COMPACT_BUTTON).owned(false).done()
            && let Ok(mut b) = b.try_cast::<Button>()
            && let Some(i) = compact_icon(compact)
        {
            b.set_button_icon(&i);
        }
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
        self.foliage.note = None;
        if let Some(i) = TOOLS.iter().position(|x| *x == t.button()).or_else(|| {
            EXTRA_TOOLS
                .iter()
                .position(|x| *x == t)
                .map(|k| TOOLS.len() + k)
        }) && let Some(b) = self.tool_buttons.get_mut(i)
        {
            b.set_pressed_no_signal(true);
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
        // The right side panel: textures (Vertex Painting) or the Foliage palette.
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

    /// Original size scale factor: brush ranges follow chunk and cell size.
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

    // ------------------------------------------------------------------ attribute strip

    pub(super) fn rebuild_strip(&mut self) {
        let Some(root) = self.peel.clone() else {
            return;
        };
        let mut strip = Strip::reassemble(root);
        self.setting_controls.clear();
        if self.state.tool == Tool::Stamp {
            self.stamp_library();
        }
        self.chunk_list = None;
        let tool = self.state.tool;
        if tool != Tool::TerrainSettings {
            let mut title =
                strip.heading(tool.moniker(), ui_kit::glyph(tool.glyph()), &tool.tooltip());
            title.set_name("ToolTitle");
        }
        match self.state.tool {
            Tool::ChunkManager => self.build_chunk_strip(&mut strip),
            Tool::TerrainSettings => {
                // Stable strip height across pages: the page tabs stay under the pointer.
                strip.pin_height("terrain_settings");
                self.build_settings_strip(&mut strip)
            }
            t => {
                for &s in tools::settings_for(t) {
                    let c = self.build_setting(&mut strip, s, t);
                    let mut cc = c.clone();
                    cc.set_tooltip_text(tools::tooltip(s));
                    // Named after the attribute (scripts / tests can find e.g. "Falloff").
                    cc.set_name(tools::portray(s, t).0);
                    self.setting_controls.push((s, c));
                }
            }
        }
        self.refresh_dependent_states();
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
            Kind::Spin(min, max, step) => {
                let mut sl = strip.spin_slider(label, min, max, step, v, &self.cb1("on_float", id));
                // Heights may be typed beyond the drag range (terrains taller than ±50); angles,
                // ease and rotations stay inside their declared range.
                if matches!(
                    s,
                    Setting::Ceiling | Setting::Height | Setting::Floor | Setting::StampHeight
                ) {
                    ui_kit::open_range(&mut sl, true, true);
                }
                sl.upcast()
            }
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
                ob.set_name(label);
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
        self.state.assign(s, v);
        let v = self.state.fetch(s);
        for (id, c) in &self.setting_controls {
            if *id != s {
                continue;
            }
            if let Ok(mut r) = c.clone().try_cast::<godot::classes::Range>() {
                ui_kit::show_value(&mut r, v);
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
        h.set_tooltip_text("Drag handles on every sample of the selected chunk (the original's chunk gizmo; one undo step per drag). While on, clicks do not add or remove chunks.");
        strip.separator();
        strip.hint(if self.vertex_handles_shown() {
            "LMB-drag handle: move sample  ·  Ctrl+LMB: select"
        } else {
            "LMB +: add chunk  ·  LMB chunk: remove  ·  Ctrl+LMB: select"
        });
    }

    /// A click can add chunk `c`: it is empty and touches an existing chunk (or the terrain
    /// has none yet). Only these slots get a "+" handle.
    pub(super) fn chunk_addable(&self, c: pt_core::math::IVec2) -> bool {
        self.terrain.as_ref().is_some_and(|t| {
            let t = t.bind();
            let st = t.store();
            !st.has_chunk(c)
                && (st.chunks.is_empty()
                    || st.config.chunk_neighbors(c).iter().any(|n| st.has_chunk(*n)))
        })
    }

    /// Chunk Manager with Vertex Handles on and a chunk to show them on: clicks only drag
    /// handles (no chunk add / remove).
    pub(super) fn vertex_handles_shown(&self) -> bool {
        self.state.tool == Tool::ChunkManager
            && self.terrain.as_ref().is_some_and(|t| {
                t.get("chunk_handles").try_to::<bool>().unwrap_or(false)
                    && t.bind().store().has_chunk(crate::util::to_iv(
                        t.get("handle_chunk").try_to::<Vector2i>().unwrap_or_default(),
                    ))
            })
    }

    /// Chunk Manager part of the viewport status line: what a click on the hovered slot does.
    pub(super) fn chunk_status(&self) -> Option<String> {
        if self.state.tool != Tool::ChunkManager {
            return None;
        }
        if self.vertex_handles_shown() {
            return Some("  |  Vertex Handles: LMB-drag a handle (chunk add / remove off)".into());
        }
        let c = self.hover_chunk?;
        let exists = self.terrain.as_ref()?.bind().store().has_chunk(c);
        Some(if exists {
            format!("  |  LMB: remove chunk ({}, {})", c.x, c.y)
        } else if self.chunk_addable(c) {
            format!("  |  LMB: add chunk ({}, {})", c.x, c.y)
        } else {
            format!(
                "  |  chunk ({}, {}) cannot be added: new chunks must touch an existing chunk",
                c.x, c.y
            )
        })
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
        // Always have a chunk selected while there are chunks (the chunk at the origin, else
        // the first), so the list, merge mode and handles act on something visible.
        if self.selected_chunk.is_none_or(|s| !coords.contains(&s)) && !coords.is_empty() {
            let c = coords
                .iter()
                .copied()
                .find(|c| *c == pt_core::math::iv2(0, 0))
                .unwrap_or(coords[0]);
            self.selected_chunk = Some(c);
            if let Some(t) = self.terrain.as_mut() {
                t.set("handle_chunk", &crate::util::from_iv(c).to_variant());
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
        // The chunk vertex handles follow the selected chunk.
        if let (Some(c), Some(t)) = (c, self.terrain.as_mut()) {
            t.set("handle_chunk", &crate::util::from_iv(c).to_variant());
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

/// Terrain Settings label of a `MarchingSquaresTerrain` property (the inspector shows the same
/// label, `inspector_labels.rs`). Composite rows (extra collision layer, grass FPS) and
/// actions have no single property and are not listed.
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

/// Pages of the Terrain Settings strip: (name, tooltip).
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

/// Settings fixed once chunks exist (see `MarchingSquaresTerrain::layout_fixed`).
fn layout_prop(prop: &str) -> bool {
    matches!(prop, "chunk_size" | "ring_radius")
}

const LAYOUT_FIXED_TIP: &str =
    "Locked: chunks already exist for this grid type. Clear the terrain data or switch the grid type to change it.";

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
        self.terrain
            .as_ref()
            .map(|t| t.get(prop))
            .unwrap_or_default()
    }

    /// Version of the undo history that holds the terrain's actions.
    pub(super) fn terrain_undo_version(&self) -> Option<u64> {
        let t = self.terrain.as_ref()?;
        let ur = self.base().get_undo_redo()?;
        let id = ur.get_object_history_id(t);
        Some(ur.get_history_undo_redo(id)?.get_version())
    }

    fn layout_fixed(&self) -> bool {
        self.terrain.as_ref().is_some_and(|t| t.bind().layout_fixed())
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
        // Page tabs on the first row, the page's fields in columns directly under them (one
        // block next to the tool title, no empty space beside the tabs).
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
        let tool = Tool::TerrainSettings;
        let mut title = ui_kit::title_button(tool.moniker(), ui_kit::glyph(tool.glyph()));
        // Tabs on one row when title + tabs fit the strip, else on two rows (the header is
        // pinned, so it must not be clipped).
        let tabs_in = |columns: usize| {
            ui_kit::page_tabs("SettingsPages", &SETTINGS_PAGES, page, columns, |k| {
                self.cb1("on_settings_page", k as i64)
            })
        };
        let one_row = tabs_in(SETTINGS_PAGES.len());
        let need = title.get_combined_minimum_size().x
            + one_row.get_combined_minimum_size().x
            + sz(24.0, 0.0).x;
        let mut tabs = if need <= avail {
            one_row
        } else {
            one_row.free();
            tabs_in(SETTINGS_PAGES.len().div_ceil(2))
        };
        tabs.set_h_size_flags(SizeFlags::SHRINK_BEGIN);
        // Title + page tabs: one header pinned above the scrolling page body (N16).
        let mut header = HBoxContainer::new_alloc();
        header.set_name("SettingsHeader");
        header.add_theme_constant_override("separation", ui_kit::px(6.0));
        title.set_name("ToolTitle");
        title.set_tooltip_text(&tool.tooltip());
        title.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
        header.add_child(&title);
        header.add_child(&godot::classes::VSeparator::new_alloc());
        header.add_child(&tabs);
        strip.pin_header(&header);
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
        let fixed = self.layout_fixed();
        let built: Vec<Gd<HBoxContainer>> = rows
            .iter()
            .map(|(i, t)| {
                let mut c = self.build_terrain_setting(*i, t);
                let tip = if layout_prop(t.prop) && fixed {
                    format!("{}\n{LAYOUT_FIXED_TIP}", t.tip)
                } else {
                    t.tip.to_string()
                };
                c.set_tooltip_text(&tip);
                let mut row = ui_kit::settings_row(t.label, &c);
                row.set_tooltip_text(&tip);
                row
            })
            .collect();
        // As few rows per column as fit next to the title (2 at least): a short strip leaves
        // the viewport as tall as possible.
        let widths: Vec<f32> = built
            .iter()
            .map(|r| r.get_combined_minimum_size().x)
            .collect();
        // The header has its own row now: the columns get the full strip width.
        let lead = sz(16.0, 0.0).x;
        self.settings_strip_w = avail;
        // Per column: its widest row, the separator and the box separation on both sides of it.
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
        block.add_child(&cols);
        strip.custom("", &block);
    }

    fn build_terrain_setting(&self, i: usize, t: &TSetting) -> Gd<Control> {
        let v = if t.prop.is_empty() {
            Variant::nil()
        } else {
            self.terrain_prop(t.prop)
        };
        let id = i as i64;
        let on = |comp: i64| {
            self.cb("on_terrain_setting")
                .bind(&[id.to_variant(), comp.to_variant()])
        };
        match t.sort {
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
                let sel = (9..=32)
                    .rev()
                    .find(|l| mask & (1 << (l - 1)) != 0)
                    .map(|l| l - 8)
                    .unwrap_or(0);
                let mut ob = ui_kit::option_button(&refs, sel as i32, 65.0);
                ob.connect("item_selected", &on(0));
                ob.upcast()
            }
            TKind::Int(min, max) => {
                let mut sb = spin(
                    min as f64,
                    max as f64,
                    1.0,
                    v.try_to::<i64>().unwrap_or(0) as f64,
                );
                // Pixel sizes are unbounded; the range is only the arrows' / drag's span.
                sb.set_allow_greater(t.prop.ends_with("pixel_size"));
                if layout_prop(t.prop) && self.layout_fixed() {
                    // The terrain refuses a new chunk layout once chunks exist: show the field
                    // read-only with the reason instead of a value that snaps back.
                    sb.set_editable(false);
                    sb.set_modulate(ui_kit::DISABLED_MODULATE);
                }
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Float(min, max, step) => {
                // Clamped to the documented range (typed values too).
                let mut sb = spin(min, max, step, v.try_to::<f64>().unwrap_or(0.0));
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::GrassFps => {
                let fps = self
                    .turf_stuff()
                    .map(|m| m.get_shader_parameter("animation_fps"))
                    .and_then(|v| v.try_to::<f64>().ok())
                    .unwrap_or(0.0);
                let mut sb = spin(0.0, 60.0, 1.0, fps);
                sb.connect("value_changed", &on(0));
                sb.upcast()
            }
            TKind::Unit => {
                let mut s = EditorSpinSlider::new_alloc();
                s.set_min(0.0);
                s.set_max(1.0);
                s.set_step(0.0105);
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
                b.set_name(t.label);
                b.set_custom_minimum_size(sz(70.0, 25.0));
                b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                // `pressed` has no arguments: pass a nil value in front of the bound index.
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
            let new = setting_num(&value) as f32;
            if old.try_to::<f64>().is_ok_and(|o| o as f32 == new) {
                return;
            }
            // The grass material is a Resource: without the terrain as context the action went
            // to the Global history, so the scene's Ctrl+Z skipped it and undid an older
            // terrain edit instead.
            ur.create_action_ex("terrain grass animation fps")
                .custom_context(&obj)
                .done();
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
            self.settings_version = self.terrain_undo_version();
            return;
        }
        if let TKind::Action(a) = t.sort {
            self.terrain_action(a);
            return;
        }
        if layout_prop(t.prop) && self.layout_fixed() {
            // Refused by the terrain: no (no-op) undo step; put the field back.
            self.rebuild_strip_deferred();
            return;
        }
        let old = terrain.get(t.prop);
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
            TKind::Action(_) => unreachable!(),
            TKind::Enum(_) | TKind::Slot | TKind::Int(..) => {
                (setting_num(&value).round() as i64).to_variant()
            }
            TKind::Float(..) | TKind::Unit => setting_num(&value).to_variant(),
            TKind::Bool => value.try_to::<bool>().unwrap_or(false).to_variant(),
            TKind::ExtraLayer => {
                let sel = setting_num(&value).round() as i64;
                let mut mask = old.try_to::<i64>().unwrap_or(1) as u32 & 0xff;
                if sel > 0 {
                    mask |= 1 << (sel + 8 - 1);
                }
                (mask as i64).to_variant()
            }
            TKind::Vec2(..) => {
                let mut v = old.try_to::<Vector2>().unwrap_or(Vector2::ONE);
                let x = setting_num(&value) as f32;
                if comp == 0 {
                    v.x = x
                } else {
                    v.y = x
                }
                v.to_variant()
            }
            TKind::Vec2i(..) => {
                let mut v = old.try_to::<Vector2i>().unwrap_or(Vector2i::ZERO);
                let x = setting_num(&value).round() as i32;
                if comp == 0 {
                    v.x = x
                } else {
                    v.y = x
                }
                v.to_variant()
            }
            TKind::GrassFps => unreachable!(),
        };
        if new == old {
            return;
        }
        if t.prop == "grid_type" {
            let n = terrain.bind().store().chunk_coords().len();
            if n > 0 {
                // Each grid type keeps its own chunks: switching shows the other grid's
                // (often none), which looked like the island being deleted. Ask first; the
                // dropdown shows the current type again until confirmed.
                self.confirm_grid_type(index, new, n);
                self.rebuild_strip_deferred();
                return;
            }
        }
        self.commit_terrain_setting(index, new);
    }

    /// Writes Terrain Settings row `index` = `new` through the undo manager (one step).
    fn commit_terrain_setting(&mut self, index: usize, new: Variant) {
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
        let old = terrain.get(t.prop);
        if new == old {
            return;
        }
        ur.create_action(&format!("terrain setting {}", t.label.to_lowercase()));
        ur.add_do_property(&obj, t.prop, &new);
        ur.add_undo_property(&obj, t.prop, &old);
        ur.commit_action();
        self.settings_version = self.terrain_undo_version();
        if t.prop == "show_grid_in_editor" {
            self.sync_grid_toggle();
        }
        if matches!(
            t.prop,
            "grid_type" | "chunk_size" | "cell_size" | "ring_radius" | "storage_mode"
        ) {
            // Layout / brush ranges depend on these: rebuild after the signal returns. Reload
            // now so the rebuilt strip sees the new grid's chunks (Chunk Size lock).
            if let Some(mut t) = self.terrain.clone() {
                t.bind_mut().update();
            }
            self.selection.clear();
            self.rebuild_strip_deferred();
        }
        self.redraw();
    }
}

// ---------------------------------------------------------------------- grid type confirmation

const GRID_NAMES: [&str; 4] = ["Square", "Triangle", "Hex", "Hex Rings"];

impl MarchingSquaresTerrainPlugin {
    fn confirm_grid_type(&mut self, index: usize, new: Variant, chunks: usize) {
        let Some(mut base) = EditorInterface::singleton().get_base_control() else {
            return;
        };
        let name = |v: i64| GRID_NAMES.get(v as usize).copied().unwrap_or("?");
        let cur = self.terrain_prop("grid_type").try_to::<i64>().unwrap_or(0);
        let to = setting_num(&new) as i64;
        let mut d = ConfirmationDialog::new_alloc();
        d.set_title("Change Grid Type?");
        d.set_text(&format!(
            "This terrain has {chunks} chunk{} on the {} grid.\n\nEach grid type keeps its own chunks: \
             switching to {} shows that grid's chunks (none if it was never used), so the \
             current terrain disappears from the view.\nThe {} chunks are kept and come back \
             when you switch back or press Ctrl+Z.",
            if chunks == 1 { "" } else { "s" },
            name(cur),
            name(to),
            name(cur),
        ));
        d.set_ok_button_text(&format!("Switch to {}", name(to)));
        d.set_autowrap(true);
        d.set_size(Vector2i::new(
            (440.0 * ui_kit::editor_scale()) as i32,
            (180.0 * ui_kit::editor_scale()) as i32,
        ));
        let on_ok = self
            .cb("on_grid_type_confirmed")
            .bind(&[new, (index as i64).to_variant(), d.to_variant()]);
        d.connect("confirmed", &on_ok);
        let free = Callable::from_object_method(&d, "queue_free");
        d.connect("canceled", &free);
        base.add_child(&d);
        d.popup_centered();
    }
}

// ---------------------------------------------------------------------- storage actions

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
        let mut d = EditorFileDialog::new_alloc();
        d.set_title(
            "Import original MarchingSquaresTerrain data (scene, data folder or metadata.res)",
        );
        d.set_file_mode(FileMode::OPEN_ANY);
        d.set_access(Access::FILESYSTEM);
        d.add_filter("*.tscn ; Original scene (reads grid settings + data_directory)");
        d.add_filter("*.res ; MSTChunkData (metadata.res)");
        // The dialog's real minimum (its path bar and file list need ~500 px): reported as its
        // min_size instead of silently clamping a smaller request.
        let k = ui_kit::editor_scale();
        d.set_min_size(Vector2i::new((500.0 * k) as i32, (300.0 * k) as i32));
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
    /// "Change Grid Type?" confirmed.
    #[func]
    fn on_grid_type_confirmed(&mut self, new: Variant, index: i64, mut dialog: Gd<Node>) {
        dialog.queue_free();
        self.commit_terrain_setting(index.max(0) as usize, new);
    }

    /// Import dialog result: one undoable action (`import_mst` / restore the old data + grid settings).
    #[func]
    fn on_import_mst_path(&mut self, path: GString, mut dialog: Gd<godot::classes::Node>) {
        dialog.queue_free();
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        let obj = terrain.clone().upcast::<Object>();
        ur.create_action("Import MST terrain data");
        ur.add_do_method(&obj, "import_mst", &[path.to_variant()]);
        for prop in [
            "contents",
            "grid_type",
            "cell_size",
            "blend_mode",
            "default_wall_slot",
            "grass_subdivisions",
            "grass_size",
        ] {
            ur.add_undo_property(&obj, prop, &terrain.get(prop));
        }
        ur.add_undo_method(&obj, "reassemble", &[]);
        ur.commit_action();
        self.selection.clear();
        self.refresh_chunk_list();
        self.rebuild_strip_deferred();
        self.redraw();
    }
}

// ---------------------------------------------------------------------- texture panel

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
        self.slot_uis.clear();
        // What the controls below show: `sync_texture_panel` rebuilds when these change.
        self.texture_panel_ids = self.texture_ids();
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
        // At the top, not under the 16 slots: reachable without scrolling the whole panel.
        let mut m = godot::classes::MarginContainer::new_alloc();
        m.add_theme_constant_override("margin_bottom", 4);
        let mut b = Button::new_alloc();
        b.set_name("ExportTexturePreset");
        b.set_text("Export Texture Preset");
        b.set_tooltip_text(&format!(
            "Save this terrain's texture set as a preset in {PRESET_DIR}."
        ));
        b.set_h_size_flags(SizeFlags::EXPAND_FILL);
        b.connect("pressed", &self.cb("on_export_preset"));
        m.add_child(&b);
        vbox.add_child(&m);
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
            le.set_tooltip_text("Texture name (shown in the Material dropdown). Enter applies.");
            // Applied on Enter / leaving the field, not per keystroke: the field's own Ctrl+Z
            // (text undo while typing) must not commit new History entries (QA #54).
            le.connect("text_submitted", &self.cb1("on_slot_name", i as i64));
            le.connect_flags(
                "focus_exited",
                &self.cb1("on_slot_name_left", i as i64),
                godot::classes::object::ConnectFlags::DEFERRED,
            );
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
            vbox.add_child(&ui_kit::form_row("Texture", &picker));
            let mut ui = SlotUi {
                slot: i,
                name: le.clone(),
                texture: picker.clone(),
                pattern: None,
                scale: None,
                tint: None,
                has_grass: None,
                grass_tint: None,
                sprite: None,
            };

            if !void {
                let mut pat = ui_kit::compact_option(&PATTERNS[..13], pattern.clamp(0, 12));
                pat.set_tooltip_text("Ground pattern: picks the bundled ground texture of this pattern (textures/ground/). Without a texture the pattern is generated procedurally.");
                pat.connect("item_selected", &self.cb1("on_slot_pattern", i as i64));
                vbox.add_child(&ui_kit::form_row("Pattern", &pat));
                ui.pattern = Some(pat.clone());

                // Scale: slider + value read-out.
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
                let value = ui_kit::value_label(scale as f64, 0.1005, 30.0);
                slider.connect(
                    "value_changed",
                    &self
                        .cb("on_slot_scale")
                        .bind(&[(i as i64).to_variant(), value.to_variant()]),
                );
                row.add_child(&slider);
                row.add_child(&value);
                vbox.add_child(&ui_kit::form_row("Scale", &row));
                ui.scale = Some((slider.clone(), value.clone()));

                let mut cp = ColorPickerButton::new_alloc();
                cp.set_pick_color(tint);
                cp.set_custom_minimum_size(sz(40.0, 25.0));
                cp.set_edit_alpha(false);
                cp.set_tooltip_text("Ground color (multiplies the texture / pattern).");
                cp.connect("color_changed", &self.cb1("on_slot_tint", i as i64));
                vbox.add_child(&ui_kit::form_row("Color", &cp));
                ui.tint = Some(cp.clone());

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
                ui.has_grass = Some(cb.clone());
                ui.grass_tint = Some(gp.clone());

                // Grass instance sprite (original: grass_sprite_tex_N); empty = terrain default.
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
                vbox.add_child(&ui_kit::form_row("Sprite", &sp));
                ui.sprite = Some(sp.clone());
            }
            self.slot_uis.push(ui);
            vbox.add_child(&HSeparator::new_alloc());
        }
        self.build_auto_rules_section(&mut vbox);
    }

    pub(super) fn refresh_texture_panel_selection(&mut self) {
        let slot = self.state.paint_slot as usize;
        if let Some(b) = self.slot_buttons.get_mut(slot)
            && b.is_instance_valid()
        {
            b.set_pressed_no_signal(true);
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

    /// Sets a slot property through the undo manager (original: no undo).
    pub(super) fn set_slot_prop(&mut self, i: usize, prop: &str, value: Variant) {
        let Some(slot) = self.slot_res(i) else { return };
        let old = slot.get(prop);
        if old == value {
            return;
        }
        let obj = slot.upcast::<Object>();
        // Slots are resources: the terrain as context keeps their edits in the scene's history
        // (in order with the strokes), not the Global one.
        let ctx = self.terrain.clone().map(|t| t.upcast::<Object>());
        if let Some(mut ur) = self.base().get_undo_redo() {
            // History names follow the panel's labels, not the resource property names.
            let what = match prop {
                "magnify" => "scale",
                "display_name" => "name",
                "has_grass" => "grass",
                "grass_tint" => "grass tint",
                "grass_sprite" => "grass sprite",
                p => p,
            };
            ur.create_action_ex(&format!("terrain texture {} {}", i + 1, what))
                .merge_mode(godot::classes::undo_redo::MergeMode::ENDS)
                .custom_context(ctx.as_ref())
                .done();
            ur.add_do_property(&obj, prop, &value);
            ur.add_undo_property(&obj, prop, &old);
            ur.commit_action();
        }
        if matches!(prop, "texture" | "pattern" | "tint") {
            self.renew_pocket_prelook(i);
        }
    }

    /// Pattern change: also switches the slot to the bundled ground texture of that pattern
    /// (one undo step), so the choice is visible whether or not the slot had a texture.
    pub(super) fn set_slot_pattern(&mut self, i: usize, pattern: i32) {
        let Some(slot) = self.slot_res(i) else { return };
        let (old_p, old_t) = (slot.get("pattern"), slot.get("texture"));
        let tex = crate::terrain::textures::pattern_texture(pattern);
        let obj = slot.upcast::<Object>();
        let ctx = self.terrain.clone().map(|t| t.upcast::<Object>());
        if let Some(mut ur) = self.base().get_undo_redo() {
            ur.create_action_ex(&format!("terrain texture {} pattern", i + 1))
                .custom_context(ctx.as_ref())
                .done();
            ur.add_do_property(&obj, "pattern", &pattern.to_variant());
            ur.add_do_property(&obj, "texture", &tex.to_variant());
            ur.add_undo_property(&obj, "pattern", &old_p);
            ur.add_undo_property(&obj, "texture", &old_t);
            ur.commit_action();
        }
        self.renew_pocket_prelook(i);
        if let Some(ui) = self
            .slot_uis
            .iter_mut()
            .find(|u| u.slot == i && u.texture.is_instance_valid())
        {
            ui.texture
                .set_edited_resource(tex.map(|t| t.upcast::<godot::classes::Resource>()).as_ref());
        }
    }

    /// Instance ids of the texture set and its slots (what the texture panel was built for).
    pub(super) fn texture_ids(&self) -> Vec<Option<InstanceId>> {
        let ts = self.tex_assign();
        let mut ids = vec![ts.as_ref().map(|t| t.instance_id())];
        ids.extend((0..SLOT_COUNT).map(|i| self.slot_res(i).map(|s| s.instance_id())));
        ids
    }

    /// Undo / redo / Inspector edits of the texture set: the panel, the Material names and the
    /// preset dropdown show the restored values (QA #9). A replaced set or slot (preset undo)
    /// rebuilds the panel and the strip; otherwise the controls are updated in place (no
    /// signals), so the panel keeps its scroll position and an open picker its state.
    pub(super) fn sync_texture_panel(&mut self) {
        if self.state.tool != Tool::VertexPaint {
            return;
        }
        if self.texture_ids() != self.texture_panel_ids {
            self.rebuild_texture_panel();
            self.rebuild_strip_deferred();
            return;
        }
        self.refresh_material_names();
        let Some(ts) = self.tex_assign() else { return };
        let mut stale_previews = Vec::new();
        for ui in &mut self.slot_uis {
            let Some(slot) = ts.bind().pocket(ui.slot) else {
                continue;
            };
            let b = slot.bind();
            let mut look = false;
            if ui.name.is_instance_valid()
                && !ui.name.has_focus()
                && ui.name.get_text() != b.display_name
            {
                ui.name.set_text(&b.display_name);
            }
            let res = |t: &Option<Gd<godot::classes::Texture2D>>| {
                t.clone().map(|t| t.upcast::<godot::classes::Resource>())
            };
            if ui.texture.is_instance_valid() && ui.texture.get_edited_resource() != res(&b.texture)
            {
                ui.texture.set_edited_resource(res(&b.texture).as_ref());
                look = true;
            }
            if let Some(p) = ui.pattern.as_mut().filter(|p| p.is_instance_valid()) {
                let want = b.pattern.clamp(0, 12);
                if p.get_selected() != want {
                    p.select(want);
                    look = true;
                }
            }
            if let Some((sl, lab)) = ui.scale.as_mut().filter(|s| s.0.is_instance_valid()) {
                let want = b.magnify as f64;
                // The slider snaps to its step: compare within half a step.
                if (sl.get_value() - want).abs() > sl.get_step() * 0.5 {
                    sl.set_value_no_signal(want);
                    lab.call("_mst_assign_worth", &[want.to_variant()]);
                }
            }
            if let Some(cp) = ui.tint.as_mut().filter(|c| c.is_instance_valid())
                && cp.get_pick_color() != b.tint
            {
                cp.set_pick_color(b.tint);
                look = true;
            }
            if let Some(cb) = ui.has_grass.as_mut().filter(|c| c.is_instance_valid()) {
                cb.set_pressed_no_signal(b.has_grass);
            }
            if let Some(gp) = ui.grass_tint.as_mut().filter(|c| c.is_instance_valid())
                && gp.get_pick_color() != b.grass_tint
            {
                gp.set_pick_color(b.grass_tint);
            }
            if let Some(sp) = ui.sprite.as_mut().filter(|c| c.is_instance_valid())
                && sp.get_edited_resource() != res(&b.grass_sprite)
            {
                sp.set_edited_resource(res(&b.grass_sprite).as_ref());
            }
            if look {
                stale_previews.push(ui.slot);
            }
        }
        for i in stale_previews {
            self.renew_pocket_prelook(i);
        }
    }

    // ------------------------------------------------------------------ presets

    /// `(path, display name)` of every texture preset in [`PRESET_DIR`].
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

    /// Applies preset `index` (0 = "None" = fresh defaults) as a *copy*, so painting/editing
    /// never changes the preset file.
    pub(super) fn apply_preset(&mut self, index: usize) {
        let Some(terrain) = self.terrain.clone() else {
            return;
        };
        let new: Gd<MarchingSquaresTexturePreset> = if index == 0 {
            let mut p = MarchingSquaresTexturePreset::new_default();
            // Short name in the inspector's resource picker (else the long class name).
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

    /// "Export Texture Preset": name field + OK / Cancel. OK stays disabled (and Enter does
    /// nothing) while the name is empty, with the reason shown in the dialog (QA #18, #19).
    pub(super) fn unveil_ship_out_popup(&mut self) {
        let mut base = EditorInterface::singleton()
            .get_base_control()
            .expect("editor base control");
        let mut dlg = ConfirmationDialog::new_alloc();
        dlg.set_title("Save Preset");
        // The dialog closes itself only after a valid name was saved (`on_export_confirmed`).
        dlg.set_hide_on_ok(false);
        dlg.set_cancel_button_text("Cancel");
        // An honest minimum (what the name field and the two-line target path need), and the
        // size the dialog had when it was last closed.
        let k = ui_kit::editor_scale();
        dlg.set_min_size(Vector2i::new((340.0 * k) as i32, (140.0 * k) as i32));
        dlg.set_size(remembered_size(
            SAVE_PRESET_SIZE_META,
            Vector2i::new((420.0 * k) as i32, (150.0 * k) as i32),
        ));
        let mut v = VBoxContainer::new_alloc();
        let mut l = Label::new_alloc();
        l.set_text("Preset name:");
        v.add_child(&l);
        let mut le = LineEdit::new_alloc();
        let current = self
            .tex_assign()
            .map(|t| t.bind().preset_name.to_string())
            .filter(|n| !n.is_empty());
        le.set_text(current.as_deref().unwrap_or("new_texture_preset"));
        le.select_all();
        v.add_child(&le);
        // Target file on two short lines (the full path on one line made the dialog 558 px wide);
        // clipped rather than widening the dialog's minimum.
        let mut file = Label::new_alloc();
        file.set_clip_text(true);
        file.set_text_overrun_behavior(godot::classes::text_server::OverrunBehavior::TRIM_ELLIPSIS);
        file.set_modulate(ui_kit::DIM_MODULATE);
        v.add_child(&file);
        dlg.add_child(&v);
        dlg.register_text_enter(&le);
        let on_edit = self
            .cb("on_export_name_changed")
            .bind(&[dlg.to_variant(), file.to_variant()]);
        le.connect("text_changed", &on_edit);
        export_name_feedback(&le.get_text().to_string(), &mut dlg, &mut file);
        let (on_ok, on_cancel) = (
            self.cb("on_export_confirmed")
                .bind(&[le.to_variant(), dlg.to_variant()]),
            self.cb("on_export_closed").bind(&[dlg.to_variant()]),
        );
        dlg.connect("confirmed", &on_ok);
        // Cancel, Esc and the title bar's X.
        dlg.connect("canceled", &on_cancel);
        base.add_child(&dlg);
        // In the tree (themed) now: make the advertised minimum the content's real one, so a
        // requested size is either honoured exactly or refused at the same minimum Godot
        // enforces (it opened 400x179 for a 400x150 request while reporting min 140, N9).
        let need = dlg.get_contents_minimum_size();
        let min = dlg.get_min_size();
        let min = Vector2i::new(min.x.max(need.x.ceil() as i32), min.y.max(need.y.ceil() as i32));
        dlg.set_min_size(min);
        let cur = dlg.get_size();
        dlg.set_size(Vector2i::new(cur.x.max(min.x), cur.y.max(min.y)));
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
            let mut base = EditorInterface::singleton()
                .get_base_control()
                .expect("editor base control");
            let mut c = ConfirmationDialog::new_alloc();
            c.set_title("Overwrite File?");
            c.set_text(&format!("{path} already exists. Overwrite it?"));
            let (on_ok, on_cancel) = (
                self.cb("on_export_overwrite")
                    .bind(&[name.to_variant(), c.to_variant()]),
                Callable::from_object_method(&c, "queue_free"),
            );
            c.connect("confirmed", &on_ok);
            c.connect("canceled", &on_cancel);
            base.add_child(&c);
            c.popup_centered();
            return;
        }
        let Some(ts) = self.tex_assign() else { return };
        let Some(dup) = ts.duplicate_resource_ex().deep_internal().done_or_null() else {
            return;
        };
        let Ok(mut p) = dup.try_cast::<MarchingSquaresTexturePreset>() else {
            return;
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

/// Save Preset dialog: shows the file a name saves to, or why it cannot be saved (empty name,
/// OK disabled).
pub(super) fn export_name_feedback(
    name: &str,
    dlg: &mut Gd<ConfirmationDialog>,
    file: &mut Gd<Label>,
) {
    let empty = name.trim().is_empty();
    if let Some(mut ok) = dlg.get_ok_button() {
        ok.set_disabled(empty);
    }
    if empty {
        file.set_text("Enter a name to save the preset.");
        file.set_modulate(Color::from_rgb(1.0, 0.55, 0.45));
    } else {
        let split = PRESET_DIR.find("texture_presets").unwrap_or(PRESET_DIR.len());
        let (head, tail) = PRESET_DIR.split_at(split);
        file.set_text(&format!(
            "Saved as {head}\n{tail}{}.tres",
            snake_case(name.trim())
        ));
        file.set_modulate(ui_kit::DIM_MODULATE);
    }
}

/// Controls of one texture slot in the side panel (kept to refresh them after undo / redo).
pub(super) struct SlotUi {
    pub slot: usize,
    pub name: Gd<LineEdit>,
    pub texture: Gd<EditorResourcePicker>,
    pub pattern: Option<Gd<OptionButton>>,
    pub scale: Option<(Gd<HSlider>, Gd<Label>)>,
    pub tint: Option<Gd<ColorPickerButton>>,
    pub has_grass: Option<Gd<CheckBox>>,
    pub grass_tint: Option<Gd<ColorPickerButton>>,
    pub sprite: Option<Gd<EditorResourcePicker>>,
}

/// Editor project metadata key remembering the Save Preset dialog size.
pub(super) const SAVE_PRESET_SIZE_META: &str = "save_preset_dialog_size";

/// Size stored under `key` (editor project metadata), else `fallback`.
pub(super) fn remembered_size(key: &str, fallback: Vector2i) -> Vector2i {
    ui_kit::editor_state(key)
        .and_then(|v| v.try_to::<Vector2i>().ok())
        .filter(|s| s.x > 0 && s.y > 0)
        .unwrap_or(fallback)
}

/// Stores a dialog size for [`remembered_size`].
pub(super) fn remember_size(key: &str, size: Vector2i) {
    ui_kit::set_editor_state(key, &size.to_variant());
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

/// Small color swatch icon for the Tint dropdown.
fn tint_swatch(c: [f32; 3]) -> Gd<godot::classes::Texture2D> {
    use godot::classes::{Image, ImageTexture, image::Format};
    let n = ui_kit::px(12.0).max(8);
    let mut img = Image::create_empty(n, n, false, Format::RGBA8).expect("swatch image");
    img.fill(Color::from_rgb(0.08, 0.08, 0.09));
    img.fill_rect(
        Rect2i::new(Vector2i::new(1, 1), Vector2i::new(n - 2, n - 2)),
        Color::from_rgb(c[0], c[1], c[2]),
    );
    ImageTexture::create_from_image(&img)
        .expect("swatch texture")
        .upcast()
}
