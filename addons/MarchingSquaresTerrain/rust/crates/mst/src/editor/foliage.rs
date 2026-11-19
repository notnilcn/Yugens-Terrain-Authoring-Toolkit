//! Foliage tool (toolbar slot / key 5, where the original Grass Mask tool was): one palette for
//! everything that grows or stands on the terrain. "Grass" paints the original grass mask
//! (per-sample data, unchanged format); every `PixelScatterItem` of the terrain paints props.
//! The palette lives in the right side panel (shared with Vertex Painting): thumbnails of the
//! terrain's Scatter Items, "+ Tree"-style entries for built-in props the terrain does not have
//! yet, and Add Mesh / Scene..., Edit and Remove buttons.
//!
//! LMB-drag paints the selected entries, RMB-drag or Shift+LMB-drag erases them. Painting by
//! hand ignores the items' procedural filters (`ScatterRules::hand_painted`); *Fill Terrain*
//! uses them. A stroke (grass mask + props) is one undo action.

use std::collections::HashMap;

use godot::classes::control::SizeFlags;
use godot::classes::item_list::{IconMode, SelectMode};
use godot::classes::{
    Button, Camera3D, EditorFileDialog, EditorInterface, HBoxContainer, InputEventMouseButton, ItemList, Label, Mesh, Object, PackedScene, ResourceLoader, Texture2D,
};
use godot::global::MouseButton;
use godot::prelude::*;

use pt_core::brush::{self, Brush, BrushShape, PaintTarget};
use pt_core::math::{IVec2, v2};

use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::{SCATTER_BUILTINS, Setting, Tool};
use super::ui_kit;
use crate::terrain::MarchingSquaresTerrain;
use crate::terrain::scatter::PixelScatterItem;
use crate::util::from_iv;

/// One row of the palette.
#[derive(Clone, PartialEq)]
enum Entry {
    Grass,
    /// Index into the terrain's `scatter_items` and the item's instance id.
    Item(usize, InstanceId),
    /// A built-in prop the terrain has no item for yet: selecting it adds the item.
    Builtin(&'static str),
}

/// Session state of the Foliage tool.
#[derive(Default)]
pub struct Foliage {
    /// "Grass" is selected.
    grass: bool,
    /// Selected `PixelScatterItem`s (instance ids, so removals / reorders keep the selection).
    items: Vec<InstanceId>,
    /// Selected built-in props the terrain has no item for yet ("+ Tree"): the item is added
    /// (one undo step) when a stroke or Fill Terrain first needs it, so opening the tool or
    /// clicking around the palette never edits the terrain.
    builtins: Vec<&'static str>,
    /// Terrain the selection belongs to: the default selection is applied again when another
    /// terrain is edited (item ids of one terrain mean nothing on another).
    for_terrain: Option<InstanceId>,
    /// Items added from `builtins`: if one disappears again (undo), its "+" entry is selected.
    added: HashMap<InstanceId, &'static str>,
    list: Option<Gd<ItemList>>,
    entries: Vec<Entry>,
    edit_button: Option<Gd<Button>>,
    remove_button: Option<Gd<Button>>,
    /// Signature of the terrain's item list the panel was built for (rebuilt when it changes:
    /// undo / redo, inspector edits).
    signature: u64,
    /// Thumbnails by preview key (see [`preview_key`]).
    thumbs: HashMap<String, Gd<Texture2D>>,
    /// Current stroke: erasing (RMB / Shift+LMB) and the painting rules of its items.
    erasing: bool,
    rules: Vec<(u16, pt_core::scatter::ScatterRules)>,
    /// Shown in the viewport hint after a stroke that placed / erased nothing (why).
    pub(super) note: Option<&'static str>,
}

fn preview_key(item: &PixelScatterItem) -> String {
    let id = |r: Option<i64>| r.unwrap_or(0);
    let c = item.color;
    format!(
        "{}:{}:{}:{}:{:.3},{:.3},{:.3}",
        item.shape,
        item.shape_seed,
        id(item.mesh.as_ref().map(|m| m.instance_id().to_i64())),
        id(item.scene.as_ref().map(|m| m.instance_id().to_i64())),
        c.r,
        c.g,
        c.b
    )
}

fn items_of(t: &Gd<MarchingSquaresTerrain>) -> Vec<Option<Gd<PixelScatterItem>>> {
    let arr: Array<Option<Gd<PixelScatterItem>>> = t.get("scatter_items").try_to().unwrap_or_default();
    arr.iter_shared().collect()
}

fn signature_of(t: &Gd<MarchingSquaresTerrain>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for it in items_of(t) {
        match it {
            Some(it) => {
                it.instance_id().to_i64().hash(&mut h);
                let b = it.bind();
                b.get_label().to_string().hash(&mut h);
                preview_key(&b).hash(&mut h);
            }
            None => 0i64.hash(&mut h),
        }
    }
    h.finish()
}

impl MarchingSquaresTerrainPlugin {
    // ------------------------------------------------------------------ selection

    /// Indices (into `scatter_items`) of the selected props.
    fn foliage_selected_items(&self) -> Vec<usize> {
        let Some(t) = &self.terrain else { return Vec::new() };
        items_of(t).iter().enumerate().filter(|(_, it)| it.as_ref().is_some_and(|it| self.foliage.items.contains(&it.instance_id()))).map(|(i, _)| i).collect()
    }

    /// Something to paint is selected (grass, a prop or a not-yet-added built-in).
    pub(super) fn foliage_has_selection(&self) -> bool {
        self.foliage.grass || !self.foliage.builtins.is_empty() || !self.foliage_selected_items().is_empty()
    }

    /// Names of what LMB paints, for the viewport hint ("Tree, Bush").
    fn foliage_selection_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.foliage.grass {
            out.push("Grass".to_string());
        }
        if let Some(t) = &self.terrain {
            let items = items_of(t);
            for i in self.foliage_selected_items() {
                if let Some(Some(it)) = items.get(i) {
                    out.push(it.bind().get_label().to_string());
                }
            }
        }
        out.extend(self.foliage.builtins.iter().map(|k| k.to_string()));
        out
    }

    /// One-line help at the bottom of the 3D view: what LMB paints right now, or what to do
    /// first when nothing is picked (replaces the static [`Tool::hint`] for this tool).
    pub(super) fn foliage_hint(&self) -> String {
        if let Some(n) = self.foliage.note {
            return n.to_string();
        }
        let names = self.foliage_selection_names();
        if names.is_empty() {
            return "Pick Grass, a tree or a rock in the Foliage palette on the right  ·  then LMB-drag on the terrain to paint it".to_string();
        }
        let erase = if self.state.erase_all { "RMB / Shift+LMB: erase every prop" } else { "RMB / Shift+LMB: erase" };
        format!("Painting {}  ·  LMB-drag: paint  ·  {erase}  ·  Shift+wheel: size  ·  palette on the right: pick what to paint", names.join(", "))
    }

    /// Default selection, applied when the tool first opens on a terrain (and again for every
    /// other terrain): the terrain's Tree, else its first item, else the built-in Tree (added
    /// on the first stroke), so LMB paints trees straight away. Ids of items that no longer
    /// exist (undo, inspector edits) are dropped.
    fn foliage_default_selection(&mut self) {
        let Some(t) = self.terrain.clone() else { return };
        let items: Vec<Gd<PixelScatterItem>> = items_of(&t).into_iter().flatten().collect();
        let ids: Vec<InstanceId> = items.iter().map(|it| it.instance_id()).collect();
        let added = &self.foliage.added;
        let mut back: Vec<&'static str> = Vec::new();
        self.foliage.items.retain(|id| {
            let keep = ids.contains(id);
            if !keep && let Some(k) = added.get(id) {
                back.push(k);
            }
            keep
        });
        self.foliage.builtins.extend(back);
        let builtin_of = |it: &Gd<PixelScatterItem>| {
            let b = it.bind();
            if b.mesh.is_some() || b.scene.is_some() {
                return None;
            }
            SCATTER_BUILTINS.iter().find(|(_, s)| *s == b.shape).map(|(k, _)| *k)
        };
        // A pending built-in the terrain got meanwhile (undo / redo of its add, inspector):
        // select that item instead.
        let pending = std::mem::take(&mut self.foliage.builtins);
        for kind in pending {
            match items.iter().find(|it| builtin_of(it) == Some(kind)) {
                Some(it) if !self.foliage.items.contains(&it.instance_id()) => self.foliage.items.push(it.instance_id()),
                Some(_) => {}
                None => self.foliage.builtins.push(kind),
            }
        }
        if self.foliage.for_terrain == Some(t.instance_id()) {
            return;
        }
        self.foliage.for_terrain = Some(t.instance_id());
        self.foliage.grass = false;
        self.foliage.builtins.clear();
        self.foliage.added.clear();
        self.foliage.note = None;
        let pick = items.iter().find(|it| builtin_of(it) == Some("Tree")).or(items.first());
        match pick {
            Some(it) => self.foliage.items = vec![it.instance_id()],
            None => self.foliage.builtins = vec!["Tree"],
        }
    }

    /// Adds the selected built-ins the terrain has no item for yet (one undo step) and selects
    /// the new items.
    fn foliage_materialize(&mut self) {
        if self.foliage.builtins.is_empty() {
            return;
        }
        let Some(terrain) = self.terrain.clone() else { return };
        let kinds = std::mem::take(&mut self.foliage.builtins);
        let old = terrain.get("scatter_items");
        let arr: Array<Option<Gd<PixelScatterItem>>> = old.try_to().unwrap_or_default();
        let mut arr = arr.duplicate_shallow();
        for kind in &kinds {
            let item = PixelScatterItem::create_builtin(kind.to_lowercase().as_str().into());
            self.foliage.items.push(item.instance_id());
            self.foliage.added.insert(item.instance_id(), kind);
            arr.push(Some(&item));
        }
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.upcast::<Object>();
            ur.create_action(&format!("terrain add scatter item {}", kinds.join(", ")));
            ur.add_do_property(&obj, "scatter_items", &arr.to_variant());
            ur.add_undo_property(&obj, "scatter_items", &old);
            ur.commit_action();
        }
        self.to_gd().call_deferred("rebuild_foliage_panel_now", &[]);
    }

    // ------------------------------------------------------------------ palette panel

    /// Rebuilds the palette in the right side panel (Foliage tool only).
    pub(super) fn rebuild_foliage_panel(&mut self) {
        let Some(mut vbox) = self.texture_vbox.clone() else { return };
        for c in vbox.get_children().iter_shared() {
            vbox.remove_child(&c);
            c.free();
        }
        self.slot_buttons.clear();
        self.slot_previews.clear();
        self.slot_tex_pickers.clear();
        self.foliage.list = None;
        let Some(t) = self.terrain.clone() else { return };
        self.foliage_default_selection();
        self.foliage.signature = signature_of(&t);

        vbox.add_child(&ui_kit::panel_title(
            "Foliage",
            ui_kit::icon("scatter_tool.svg"),
            "What the Foliage tool paints: grass and the terrain's Scatter Items (PixelScatterItem resources).",
        ));
        let mut hint = Label::new_alloc();
        hint.set_text("1. Click what to paint below (Ctrl/Shift+click: several).\n2. LMB-drag on the terrain to paint it.\nRMB / Shift+LMB: erase  ·  Shift+wheel: brush size");
        hint.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        hint.set_modulate(ui_kit::DIM_MODULATE);
        vbox.add_child(&hint);

        // Entries: Grass, the terrain's items, then built-ins it has no item for.
        let items = items_of(&t);
        let mut entries = vec![Entry::Grass];
        for (i, it) in items.iter().enumerate() {
            if let Some(it) = it {
                entries.push(Entry::Item(i, it.instance_id()));
            }
        }
        for (kind, shape) in SCATTER_BUILTINS {
            let has = items.iter().flatten().any(|it| {
                let b = it.bind();
                b.shape == shape && b.mesh.is_none() && b.scene.is_none()
            });
            if !has {
                entries.push(Entry::Builtin(kind));
            }
        }

        let mut list = ItemList::new_alloc();
        list.set_name("FoliagePalette");
        list.set_select_mode(SelectMode::MULTI);
        list.set_icon_mode(IconMode::TOP);
        list.set_max_columns(0);
        list.set_same_column_width(true);
        list.set_fixed_column_width(ui_kit::px(76.0));
        list.set_fixed_icon_size(Vector2i::new(ui_kit::px(56.0), ui_kit::px(56.0)));
        list.set_max_text_lines(2);
        list.set_auto_height(true);
        list.set_h_size_flags(SizeFlags::EXPAND_FILL);
        list.set_tooltip_text("Click to paint it, Ctrl/Shift+click to paint several together. Double-click a prop to edit it in the inspector.");
        let scatter_icon = ui_kit::icon("scatter_tool.svg");
        for (row, e) in entries.iter().enumerate() {
            let row = row as i32;
            match e {
                Entry::Grass => {
                    list.add_item("Grass");
                    if let Some(ic) = ui_kit::icon("grass_mask_tool.svg") {
                        list.set_item_icon(row, &ic);
                    }
                    list.set_item_tooltip(row, "Grass (the terrain's grass mask). Grows only on textures with Grass ticked in the Vertex Painting panel.");
                    if self.foliage.grass {
                        list.select_ex(row).single(false).done();
                    }
                }
                Entry::Item(i, id) => {
                    let Some(Some(it)) = items.get(*i) else { continue };
                    let (label, key) = {
                        let b = it.bind();
                        (b.get_label().to_string(), preview_key(&b))
                    };
                    list.add_item(&label);
                    list.set_item_tooltip(row, &format!("{label} (Scatter Item {}). Double-click to edit it.", i + 1));
                    match self.foliage.thumbs.get(&key) {
                        Some(tex) => list.set_item_icon(row, tex),
                        None => {
                            if let Some(ic) = &scatter_icon {
                                list.set_item_icon(row, ic);
                            }
                            self.queue_foliage_thumb(&it.bind(), key);
                        }
                    }
                    if self.foliage.items.contains(id) {
                        list.select_ex(row).single(false).done();
                    }
                }
                Entry::Builtin(kind) => {
                    // Looks like any other prop (a dimmed entry reads as disabled); the "+"
                    // says the terrain gets the item when it is first painted.
                    list.add_item(&format!("+ {kind}"));
                    list.set_item_tooltip(row, &format!("Built-in {kind}: click it and paint. This terrain has no {kind} item yet: the first stroke adds it to its Scatter Items (undoable)."));
                    if self.foliage.builtins.contains(kind) {
                        list.select_ex(row).single(false).done();
                    }
                    let tmp = PixelScatterItem::create_builtin(kind.to_lowercase().as_str().into());
                    let key = preview_key(&tmp.bind());
                    match self.foliage.thumbs.get(&key) {
                        Some(tex) => list.set_item_icon(row, tex),
                        None => {
                            if let Some(ic) = &scatter_icon {
                                list.set_item_icon(row, ic);
                            }
                            self.queue_foliage_thumb(&tmp.bind(), key);
                        }
                    }
                }
            }
        }
        list.connect("multi_selected", &self.to_gd().callable("on_foliage_selected"));
        list.connect("item_activated", &self.to_gd().callable("on_foliage_activated"));
        vbox.add_child(&list);
        self.foliage.entries = entries;
        self.foliage.list = Some(list);

        let mut row = HBoxContainer::new_alloc();
        let mut edit = Button::new_alloc();
        edit.set_text("Edit");
        edit.set_tooltip_text("Shows the selected prop's settings (mesh, scale, placement rules, collision...) in the inspector.");
        edit.set_h_size_flags(SizeFlags::EXPAND_FILL);
        edit.connect("pressed", &self.to_gd().callable("on_foliage_edit"));
        row.add_child(&edit);
        let mut rm = Button::new_alloc();
        rm.set_text("Remove");
        rm.set_tooltip_text("Removes the selected props from this terrain's Scatter Items, with every instance of them (undoable).");
        rm.set_h_size_flags(SizeFlags::EXPAND_FILL);
        rm.connect("pressed", &self.to_gd().callable("on_foliage_remove"));
        row.add_child(&rm);
        vbox.add_child(&row);
        self.foliage.edit_button = Some(edit);
        self.foliage.remove_button = Some(rm);
        let mut add = Button::new_alloc();
        add.set_text("Add Mesh / Scene...");
        add.set_tooltip_text("Adds a prop from a mesh (.mesh / .tres / .res / .obj) or a scene / model (.tscn / .glb / .gltf / .blend / .fbx) and selects it.");
        add.set_h_size_flags(SizeFlags::EXPAND_FILL);
        add.connect("pressed", &self.to_gd().callable("on_foliage_add_file"));
        vbox.add_child(&add);
        self.refresh_foliage_buttons();
    }

    fn refresh_foliage_buttons(&mut self) {
        let any = !self.foliage_selected_items().is_empty();
        for b in [&mut self.foliage.edit_button, &mut self.foliage.remove_button].into_iter().flatten() {
            if b.is_instance_valid() {
                b.set_disabled(!any);
            }
        }
    }

    fn queue_foliage_thumb(&self, item: &PixelScatterItem, key: String) {
        let Some(res) = item.preview_resource() else { return };
        let Some(mut prev) = EditorInterface::singleton().get_resource_previewer() else { return };
        // Deferred: a cached preview calls back at once, into this (still bound) plugin.
        // Files (scenes / models / meshes) get the FileSystem dock's thumbnail by path; the
        // edited-resource previewer does not render imported scenes.
        let path = res.get_path().to_string();
        let (method, target) = if !path.is_empty() && !path.contains("::") { ("queue_resource_preview", path.to_variant()) } else { ("queue_edited_resource_preview", res.to_variant()) };
        let args = [target, self.to_gd().to_variant(), "on_foliage_thumb".to_variant(), key.to_variant()];
        prev.call_deferred(method, &args);
    }

    /// Rebuilds the palette when the terrain's item list changed behind the tool's back (undo,
    /// redo, inspector edits). Called from `process`; cheap when nothing changed.
    pub(super) fn foliage_poll(&mut self) {
        if self.state.tool != Tool::Scatter || !self.visible {
            return;
        }
        let Some(t) = &self.terrain else { return };
        if signature_of(t) != self.foliage.signature {
            self.rebuild_foliage_panel();
        }
    }

    // ------------------------------------------------------------------ input

    /// Mouse buttons of the Foliage tool. Returns "consumed".
    pub(super) fn foliage_mouse_button(&mut self, camera: &Gd<Camera3D>, mb: &Gd<InputEventMouseButton>, shift: bool, ctrl: bool) -> bool {
        let button = mb.get_button_index();
        let (is_l, is_r) = (button == MouseButton::LEFT, button == MouseButton::RIGHT);
        if !mb.is_pressed() {
            if let Some(s) = &mut self.stroke {
                if is_l {
                    s.lmb = false;
                }
                if is_r {
                    s.rmb = false;
                }
                if !s.lmb && !s.rmb {
                    self.finish_stroke();
                }
                return true;
            }
            return false;
        }
        if !(is_l || is_r) || ctrl {
            return self.stroke.is_some();
        }
        if self.stroke.is_some() {
            return true;
        }
        let Some(h) = self.hover else { return false };
        let erasing = is_r || shift || mb.is_shift_pressed();
        self.foliage_default_selection();
        if !self.foliage_has_selection() && !(erasing && self.state.erase_all) {
            // Said in the viewport hint (the Output panel is easy to miss).
            self.foliage.note = Some("Nothing to paint: click Grass, a tree or a rock in the Foliage palette on the right first");
            self.base().update_overlays();
            return true;
        }
        self.foliage.note = None;
        if !erasing {
            self.foliage_materialize();
        }
        let props = self.foliage_selected_items();
        self.foliage.erasing = erasing;
        let slope = (self.state.slope_min, self.state.slope_max);
        self.foliage.rules = match &self.terrain {
            Some(t) if !erasing => t.bind().scatter_hand_rules(&props, slope, godot::global::randi() as u32),
            _ => Vec::new(),
        };
        let before = self.terrain.as_ref().map(|t| t.bind().scatter_bytes());
        let mut st = self.new_stroke(camera, mb, Tool::Scatter, h, is_l, is_r);
        st.scatter_before = before;
        self.stroke = Some(st);
        self.foliage_dab(true);
        true
    }

    /// Mouse motion during a Foliage stroke.
    pub(super) fn foliage_motion(&mut self) {
        self.foliage_dab(false);
    }

    /// The Foliage brush: round, hard-edged, the tool's Size.
    pub(super) fn foliage_brush(&self) -> Brush {
        Brush { shape: BrushShape::Round, radius: self.state.brush.radius, falloff: false, ..Default::default() }
    }

    fn foliage_dab(&mut self, first: bool) {
        let (Some(h), Some(mut t)) = (self.hover, self.terrain.clone()) else { return };
        let brush = self.foliage_brush();
        let erase = self.foliage.erasing;
        let slope = (self.state.slope_min, self.state.slope_max);
        if self.foliage.grass {
            let fp = brush::footprint(&t.bind().store().config, &brush, v2(h.x, h.z));
            self.apply_footprint(&fp, |st, fp| {
                let fp = brush::filter_slope(st, fp, slope.0, slope.1);
                brush::op_paint(st, &fp, PaintTarget::Grass, if erase { 0 } else { 255 })
            });
        }
        let items = self.foliage_selected_items();
        let center = v2(h.x, h.z);
        // The terrain refills the changed tiles once per frame (its `_process`).
        {
            let mut tb = t.bind_mut();
            if erase {
                if self.state.erase_all {
                    tb.scatter_erase_items(None, &brush, center)
                } else {
                    !items.is_empty() && tb.scatter_erase_items(Some(&items), &brush, center)
                }
            } else {
                tb.scatter_paint_items(&self.foliage.rules, &brush, center, self.state.scatter_density, first)
            }
        };
    }

    /// Cursor color: green paints, red erases.
    pub(super) fn foliage_cursor_color(&self) -> Color {
        let erasing = if self.stroke.is_some() { self.foliage.erasing } else { godot::classes::Input::singleton().is_key_pressed(godot::global::Key::SHIFT) };
        if erasing { Color::from_rgb(1.0, 0.3, 0.25) } else { Color::from_rgb(0.45, 1.0, 0.35) }
    }

    /// Ends a Foliage stroke: one undo action for the grass mask (chunk blobs) and the props
    /// (scatter blob) it changed.
    pub(super) fn finish_foliage_stroke(&mut self, stroke: super::plugin::Stroke) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let mut before = VarDictionary::new();
        let mut after = VarDictionary::new();
        let mut coords: Vec<IVec2> = Vec::new();
        {
            let t = terrain.bind();
            for (c, b) in &stroke.before {
                let a = t.chunk_bytes(*c);
                if a != *b {
                    coords.push(*c);
                    before.set(from_iv(*c), &PackedByteArray::from(b.clone().unwrap_or_default().as_slice()));
                    after.set(from_iv(*c), &PackedByteArray::from(a.unwrap_or_default().as_slice()));
                }
            }
        }
        let scatter = stroke.scatter_before.and_then(|b| {
            let a = terrain.bind().scatter_bytes();
            (a != b).then_some((b, a))
        });
        if coords.is_empty() && scatter.is_none() {
            self.foliage.note = Some(if self.foliage.erasing {
                "Nothing erased here (RMB erases only the selected entries; tick Erase All for every prop)"
            } else {
                "Nothing painted here: the spot is full (lower Density or erase), under water, built over or outside Slope Min / Max"
            });
            self.base().update_overlays();
            return;
        }
        if !coords.is_empty() {
            terrain.bind_mut().write_chunks(&coords);
        }
        if scatter.is_some() {
            terrain.bind_mut().scatter_write();
        }
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        let obj = terrain.upcast::<Object>();
        let what = match (coords.is_empty(), scatter.is_none()) {
            (true, _) => "props",
            (false, true) => "grass",
            (false, false) => "grass + props",
        };
        let name = format!("terrain foliage {} {what}", if self.foliage.erasing { "erase" } else { "paint" });
        ur.create_action(&name);
        if !coords.is_empty() {
            ur.add_do_method(&obj, "set_chunks_data", &[after.to_variant()]);
            ur.add_undo_method(&obj, "set_chunks_data", &[before.to_variant()]);
        }
        if let Some((b, a)) = scatter {
            ur.add_do_method(&obj, "set_scatter_data", &[PackedByteArray::from(a.as_slice()).to_variant()]);
            ur.add_undo_method(&obj, "set_scatter_data", &[PackedByteArray::from(b.as_slice()).to_variant()]);
        }
        ur.commit_action_ex().execute(false).done();
    }

    // ------------------------------------------------------------------ actions

    /// Fill Terrain / Clear for the selected props (one undo step each).
    pub(super) fn foliage_action(&mut self, s: Setting) {
        let Some(mut t) = self.terrain.clone() else { return };
        if s == Setting::ScatterFill {
            self.foliage_materialize();
        }
        let items = self.foliage_selected_items();
        if items.is_empty() {
            self.foliage.note = Some("Fill Terrain / Clear act on props: select one or more props (not only Grass) in the palette first");
            self.base().update_overlays();
            return;
        }
        let before = t.bind().scatter_bytes();
        {
            let mut tb = t.bind_mut();
            for &i in &items {
                match s {
                    Setting::ScatterFill => tb.scatter_fill_cells(i as i64),
                    _ => tb.scatter_clear(i as i64),
                }
            }
            tb.update();
        }
        self.commit_scatter_undo(if s == Setting::ScatterFill { "terrain foliage fill" } else { "terrain foliage clear" }, before);
    }

    /// Appends `item` to the terrain's Scatter Items (undoable) and selects it alone.
    fn foliage_add_item(&mut self, item: Gd<PixelScatterItem>, name: &str) {
        let Some(terrain) = self.terrain.clone() else { return };
        let old = terrain.get("scatter_items");
        let arr: Array<Option<Gd<PixelScatterItem>>> = old.try_to().unwrap_or_default();
        let mut arr = arr.duplicate_shallow();
        arr.push(Some(&item));
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.upcast::<Object>();
            ur.create_action(&format!("terrain add scatter item {name}"));
            ur.add_do_property(&obj, "scatter_items", &arr.to_variant());
            ur.add_undo_property(&obj, "scatter_items", &old);
            ur.commit_action();
        }
        self.foliage.items = vec![item.instance_id()];
        self.foliage.grass = false;
        self.foliage.builtins.clear();
        self.to_gd().call_deferred("rebuild_foliage_panel_now", &[]);
    }
}

#[godot_api(secondary)]
impl MarchingSquaresTerrainPlugin {
    #[func]
    fn rebuild_foliage_panel_now(&mut self) {
        if self.state.tool == Tool::Scatter {
            self.rebuild_foliage_panel();
        }
    }

    /// Palette selection changed (ItemList `multi_selected`).
    #[func]
    fn on_foliage_selected(&mut self, _index: i64, _selected: bool) {
        let Some(list) = self.foliage.list.clone() else { return };
        let rows: Vec<usize> = list.get_selected_items().as_slice().iter().map(|r| *r as usize).collect();
        let mut grass = false;
        let mut items = Vec::new();
        let mut builtins = Vec::new();
        for r in rows {
            match self.foliage.entries.get(r) {
                Some(Entry::Grass) => grass = true,
                Some(Entry::Item(_, id)) => items.push(*id),
                Some(Entry::Builtin(kind)) => builtins.push(*kind),
                None => {}
            }
        }
        self.foliage.grass = grass;
        self.foliage.items = items;
        self.foliage.builtins = builtins;
        self.foliage.note = None;
        self.refresh_foliage_buttons();
        self.base().update_overlays();
    }

    /// Double-click: edit the prop in the inspector.
    #[func]
    fn on_foliage_activated(&mut self, index: i64) {
        if let Some(Entry::Item(_, id)) = self.foliage.entries.get(index as usize).cloned() {
            self.foliage.items = vec![id];
            self.on_foliage_edit();
        }
    }

    #[func]
    fn on_foliage_edit(&mut self) {
        let Some(t) = &self.terrain else { return };
        let Some(i) = self.foliage_selected_items().first().copied() else { return };
        if let Some(Some(item)) = items_of(t).get(i) {
            // Inspector only: the terrain stays the edited node, so the tool stays open.
            EditorInterface::singleton().inspect_object_ex(item).inspector_only(true).done();
        }
    }

    /// Remove: drops the selected props and their instances (one undo step restoring both).
    #[func]
    fn on_foliage_remove(&mut self) {
        let Some(mut terrain) = self.terrain.clone() else { return };
        let mut idx = self.foliage_selected_items();
        if idx.is_empty() {
            return;
        }
        idx.sort_unstable_by(|a, b| b.cmp(a));
        let items_before = terrain.get("scatter_items");
        let before = terrain.bind().scatter_bytes();
        for &i in &idx {
            terrain.bind_mut().remove_scatter_item(i as i64);
        }
        if let Some(mut ur) = self.base().get_undo_redo() {
            let obj = terrain.clone().upcast::<Object>();
            ur.create_action("terrain remove scatter items");
            for &i in &idx {
                ur.add_do_method(&obj, "remove_scatter_item", &[(i as i64).to_variant()]);
            }
            ur.add_undo_property(&obj, "scatter_items", &items_before);
            ur.add_undo_method(&obj, "set_scatter_data", &[PackedByteArray::from(before.as_slice()).to_variant()]);
            ur.commit_action_ex().execute(false).done();
        }
        self.foliage.items.clear();
        self.to_gd().call_deferred("rebuild_foliage_panel_now", &[]);
    }

    #[func]
    fn on_foliage_add_file(&mut self) {
        use godot::classes::file_dialog::{Access, FileMode};
        let mut d = EditorFileDialog::new_alloc();
        d.set_title("Add a prop from a mesh or a scene");
        d.set_file_mode(FileMode::OPEN_FILE);
        d.set_access(Access::RESOURCES);
        d.add_filter("*.tscn, *.scn, *.glb, *.gltf, *.blend, *.fbx, *.obj ; Scenes and models");
        d.add_filter("*.mesh, *.tres, *.res ; Meshes");
        let on = self.to_gd().callable("on_foliage_file").bind(&[d.to_variant()]);
        d.connect("file_selected", &on);
        let free = Callable::from_object_method(&d, "queue_free");
        d.connect("canceled", &free);
        if let Some(mut base) = EditorInterface::singleton().get_base_control() {
            base.add_child(&d);
        }
        d.popup_file_dialog();
    }

    #[func]
    fn on_foliage_file(&mut self, path: GString, mut dialog: Gd<godot::classes::Node>) {
        dialog.queue_free();
        let Some(res) = ResourceLoader::singleton().load(&path) else {
            godot_warn!("Marching Squares Terrain: could not load {path}.");
            return;
        };
        let mut item = PixelScatterItem::new_gd();
        let stem = path.to_string().rsplit('/').next().unwrap_or("Prop").rsplit_once('.').map(|x| x.0.to_string()).unwrap_or_default();
        {
            let mut b = item.bind_mut();
            if let Ok(scene) = res.clone().try_cast::<PackedScene>() {
                b.scene = Some(scene);
            } else if let Ok(mesh) = res.try_cast::<Mesh>() {
                b.mesh = Some(mesh);
            } else {
                godot_warn!("Marching Squares Terrain: {path} is neither a mesh nor a scene.");
                return;
            }
            b.shape = 0;
            b.display_name = stem.replace('_', " ").as_str().into();
        }
        self.foliage_add_item(item, &stem);
    }

    /// Thumbnail ready (`EditorResourcePreview`).
    #[func]
    fn on_foliage_thumb(&mut self, _path: GString, preview: Option<Gd<Texture2D>>, _thumb: Option<Gd<Texture2D>>, key: Variant) {
        let (Some(tex), Ok(key)) = (preview, key.try_to::<GString>()) else { return };
        let key = key.to_string();
        self.foliage.thumbs.insert(key.clone(), tex.clone());
        let Some(mut list) = self.foliage.list.clone().filter(|l| l.is_instance_valid()) else { return };
        let Some(t) = self.terrain.clone() else { return };
        let items = items_of(&t);
        for (row, e) in self.foliage.entries.iter().enumerate() {
            let k = match e {
                Entry::Item(i, _) => items.get(*i).cloned().flatten().map(|it| preview_key(&it.bind())),
                Entry::Builtin(kind) => Some(preview_key(&PixelScatterItem::create_builtin(kind.to_lowercase().as_str().into()).bind())),
                Entry::Grass => None,
            };
            if k.as_deref() == Some(key.as_str()) {
                list.set_item_icon(row as i32, &tex);
            }
        }
    }
}
