//! Paint commands shared by both layouts: colour-only paint, select faces by slot / colour,
//! sample from the selection, smoothing groups (Smooth / Faceted / new group / select / merge /
//! clear), the built-in material presets and palette load / save / reset. The Xogot layout's
//! Paint popover calls them; the MST layout reaches them through the toolbar (*Paint colour*),
//! the palette (slot *Select*), the swatches (Alt+click) and the context menu.
//!
//! Palettes are files in both of Xogot's formats (port of `material_presets.rs`
//! `save_palette` / `load_palette`): `.json` (the Codable JSON, `Palette::to_xogot_json`) and
//! `.tres` (a bare `Resource` whose `_xogot_modeler_palette` meta dictionary holds the version,
//! the material resources, the colours, `labels` and `emptyColorPresetIndices`). Both are written
//! in Xogot's shape (ten colours, at least ten slots) so Xogot loads them; the builder reads either
//! shape. The `.tres` label quirk (Xogot writes colour labels under `labels`, reads material labels
//! from it) is handled as `xg_paint` documents: material labels go to `labels`, colour labels to
//! `colorPresetLabels`.
//!
//! Every mesh change goes through `apply_edit` (one undo step). Engine-free logic is
//! `pt_core::build::xg_paint` (port of xogot-rust `editor_core/src/editor_state.rs`
//! `set_shading_intent` / `create_smoothing_group_from_selection`,
//! `xogot_modeler/src/shared/attributes.rs` merge / clear / match,
//! `xogot_modeler/src/shared/material_presets.rs` generated materials and palette JSON).

use std::collections::BTreeSet;

use godot::classes::base_material_3d::{CullMode, Flags, ShadingMode, TextureFilter, TextureParam, Transparency};
use godot::classes::image::Format;
use godot::classes::{EditorFileDialog, EditorInterface, FileAccess, Image, ImageTexture, InputEvent, InputEventMouseButton, Material, Resource, ResourceLoader, ResourceSaver, StandardMaterial3D};
use godot::global::MouseButton;
use godot::prelude::*;

use pt_core::build::{ops, xg_paint};

use super::super::ui::{self, Mode};
use super::{LayoutUi, PixelBuilderPlugin};

/// Meta on a generated preset material: its `modeler-builtin:` reference (palette save).
pub const PRESET_META: &str = "xogot_preset";

/// A `StandardMaterial3D` from a built-in preset (port of `generatedMaterial(_:)`).
pub fn preset_material(p: xg_paint::Preset) -> Gd<StandardMaterial3D> {
    let pm = p.material();
    let mut m = StandardMaterial3D::new_gd();
    m.set_name(&pm.name);
    m.set_albedo(Color::from_rgba(pm.albedo[0], pm.albedo[1], pm.albedo[2], pm.albedo[3]));
    m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, pm.vertex_color);
    m.set_roughness(pm.roughness);
    m.set_uv1_scale(Vector3::new(pm.uv1_scale[0], pm.uv1_scale[1], pm.uv1_scale[2]));
    if pm.transparent {
        m.set_transparency(Transparency::ALPHA);
    }
    if pm.unshaded {
        m.set_shading_mode(ShadingMode::UNSHADED);
    }
    if pm.cull_disabled {
        m.set_cull_mode(CullMode::DISABLED);
    }
    if pm.anisotropic_filter {
        m.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS_ANISOTROPIC);
    }
    if let Some(pat) = pm.pattern {
        let px = xg_paint::pattern_rgba8(pat);
        let n = xg_paint::PATTERN_SIZE as i32;
        if let Some(mut img) = Image::create_from_data(n, n, false, Format::RGBA8, &PackedByteArray::from(px.as_slice())) {
            img.generate_mipmaps();
            if let Some(mut tex) = ImageTexture::create_from_image(&img) {
                tex.set_name(pat.name());
                m.set_texture(TextureParam::ALBEDO, &tex);
            }
        }
    }
    m.set_meta(PRESET_META, &p.reference().to_variant());
    m
}

fn color_of(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

impl PixelBuilderPlugin {
    /// Selected faces, or every face in Object mode (for paint commands).
    fn selected_paint_faces(&self) -> BTreeSet<usize> {
        let Some(d) = self.data() else { return BTreeSet::new() };
        let db = d.bind();
        self.all_or_selected_faces(db.mesh())
    }

    /// *Paint colour*: the selected faces (every face in Object mode) get the current colour;
    /// their slots stay.
    pub(super) fn act_paint_colour(&mut self) {
        if self.mode != Mode::Object && (self.mode != Mode::Face || self.sel_faces.is_empty()) {
            self.message = "Paint colour: select faces (Face mode) or use Object mode".into();
            return;
        }
        let col = super::ui_color_bytes(self.color);
        self.apply_edit("Pixel Builder: Paint colour", |m, s| {
            let f = s.all_or_selected_faces(m);
            ops::set_color(m, &f, col);
        });
    }

    /// Face mode with every face whose material slot is `slot` selected.
    pub(super) fn select_faces_with_slot(&mut self, slot: i64) {
        let Some(d) = self.data() else { return };
        self.adjust_commit();
        let faces: BTreeSet<usize> = d.bind().mesh().faces.iter().enumerate().filter(|(_, f)| f.material as i64 == slot).map(|(i, _)| i).collect();
        if self.mode != Mode::Face {
            self.set_mode(Mode::Face);
        }
        self.message = format!("{} face{} use slot {slot}", faces.len(), if faces.len() == 1 { "" } else { "s" });
        self.sel_faces = faces;
        self.sync_ui();
        self.redraw();
    }

    /// Face mode with every face whose colour (face colour) equals `c`.
    pub(super) fn select_faces_with_colour(&mut self, c: Color) {
        let Some(d) = self.data() else { return };
        self.adjust_commit();
        let want = super::ui_color_bytes(c);
        let faces: BTreeSet<usize> = d.bind().mesh().faces.iter().enumerate().filter(|(_, f)| f.color == want).map(|(i, _)| i).collect();
        if self.mode != Mode::Face {
            self.set_mode(Mode::Face);
        }
        self.message = format!("{} face{} use this colour", faces.len(), if faces.len() == 1 { "" } else { "s" });
        self.sel_faces = faces;
        self.sync_ui();
        self.redraw();
    }

    /// *Sample from Selection* (Xogot's *Match*): the slot, colour and smoothing group most of
    /// the selected faces use become current (`match_material` / `match_color` /
    /// `match_smoothing`).
    pub(super) fn sample_selection(&mut self) {
        let Some(d) = self.data() else { return };
        let faces = self.selected_paint_faces();
        let (slot, col, smooth) = {
            let db = d.bind();
            (xg_paint::sample_material(db.mesh(), &faces), xg_paint::sample_color(db.mesh(), &faces), xg_paint::sample_smoothing(db.mesh(), &faces))
        };
        if let Some(s) = slot {
            self.slot = s as i64;
        }
        if let Some(c) = col {
            self.color = color_of(c);
        }
        if let Some(g) = smooth {
            self.xg.smooth_group = g;
        }
        self.message = match (slot, smooth) {
            (Some(s), Some(g)) if g > 0 => format!("Sampled slot {s}, the colour and smoothing group {g}"),
            (Some(s), _) => format!("Sampled slot {s} and the colour (faceted)"),
            _ => "Sample: select faces first".into(),
        };
        self.sync_ui();
    }

    /// Smoothing page commands: 0 Smooth, 1 Faceted, 2 New from selection, 3 select group,
    /// 4 clear group, 5 merge group into the selection's group.
    pub(super) fn smoothing_cmd(&mut self, cmd: i64, group: i64) {
        let faces = self.selected_paint_faces();
        let g = group.clamp(0, u16::MAX as i64) as u16;
        // The current group (chip / sample) is the panel group of `smooth_faces`; it is
        // forgotten when no face uses it any more.
        let panel = self.xg.smooth_group;
        match cmd {
            0 => self.apply_edit("Pixel Builder: Smooth", |m, s| match xg_paint::smooth_faces(m, &faces, &[], panel) {
                Some(g) => {
                    s.message = format!("Smoothing group {g}");
                    s.xg.smooth_group = g;
                }
                None => s.message = "Smooth: select faces (or no free group 1-23)".into(),
            }),
            1 => self.apply_edit("Pixel Builder: Faceted", |m, _| xg_paint::facet_faces(m, &faces)),
            2 => self.apply_edit("Pixel Builder: New smoothing group", |m, s| match xg_paint::new_group_from_faces(m, &faces) {
                Some(g) => {
                    s.message = format!("New smoothing group {g}");
                    s.xg.smooth_group = g;
                }
                None => s.message = "New group: select faces (groups 1-23 are all used)".into(),
            }),
            3 => {
                let Some(d) = self.data() else { return };
                let sel = xg_paint::faces_in_group(d.bind().mesh(), g);
                self.adjust_commit();
                if self.mode != Mode::Face {
                    self.set_mode(Mode::Face);
                }
                self.message = format!("Smoothing group {g}: {} faces selected", sel.len());
                self.sel_faces = sel;
                self.xg.smooth_group = g;
                self.redraw();
            }
            4 => self.apply_edit("Pixel Builder: Clear smoothing group", |m, s| {
                let n = xg_paint::clear_group(m, g);
                s.message = format!("Smoothing group {g} cleared ({n} faces faceted)");
            }),
            5 => {
                let Some(d) = self.data() else { return };
                let Some(dest) = xg_paint::sample_smoothing(d.bind().mesh(), &faces) else {
                    self.message = "Merge: select faces of the destination group first".into();
                    return;
                };
                self.apply_edit("Pixel Builder: Merge smoothing group", |m, s| match xg_paint::merge_group(m, g, dest) {
                    Ok(n) => {
                        s.message = format!("{n} faces moved from group {g} to {dest}");
                        s.xg.smooth_group = dest;
                    }
                    Err(e) => s.message = e,
                });
            }
            _ => {}
        }
        if let Some(d) = self.data()
            && xg_paint::faces_in_group(d.bind().mesh(), self.xg.smooth_group).is_empty()
        {
            self.xg.smooth_group = 0;
        }
        self.sync_ui();
    }

    /// Smoothing groups of the edited mesh (index, face count).
    pub(super) fn smoothing_groups(&self) -> Vec<(u16, usize)> {
        self.data().map(|d| xg_paint::smoothing_groups(d.bind().mesh()).into_iter().map(|g| (g.index, g.face_count)).collect()).unwrap_or_default()
    }

    /// Adds a slot holding built-in preset `i` (Grid, Checker, No Draw, Collider, Trigger).
    pub(super) fn add_preset_slot(&mut self, i: usize) {
        let Some(p) = xg_paint::Preset::ALL.get(i).copied() else { return };
        let Some(data) = self.data() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let old = data.bind().material_slots();
        let mut new = old.duplicate_shallow();
        let mat: Gd<Material> = preset_material(p).upcast();
        self.slot = new.len() as i64;
        new.push(Some(&mat));
        self.commit_materials("Pixel Builder: Add preset slot", &data, old, new);
    }

    /// The palette of the edited mesh: slot material paths (or preset references) and the
    /// swatches.
    pub(super) fn current_palette(&self) -> xg_paint::Palette {
        let mats: Vec<String> = self
            .data()
            .map(|d| {
                d.bind()
                    .material_slots()
                    .iter_shared()
                    .map(|m| match m {
                        Some(m) if m.has_meta(PRESET_META) => m.get_meta(PRESET_META).to_string(),
                        // `scene.tscn::id` sub-resources have no file of their own.
                        Some(m) => Some(m.get_path().to_string()).filter(|p| !p.contains("::")).unwrap_or_default(),
                        None => String::new(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let colors: Vec<[f32; 4]> = self.swatches.iter().map(|c| [c.r, c.g, c.b, c.a]).collect();
        xg_paint::Palette::from_slots(&mats, &colors)
    }

    /// Saves the palette: `.tres` / `.res` as Xogot's palette resource, anything else as its
    /// JSON (`Palette::to_xogot_json`). Both in Xogot's shape (ten colours, at least ten slots).
    pub(super) fn palette_save(&mut self, path: &str) {
        self.message = match self.palette_save_inner(path) {
            Ok(note) => format!("Palette saved to {path}{note}"),
            Err(e) => format!("Palette: {e}"),
        };
    }

    fn palette_save_inner(&mut self, path: &str) -> Result<String, String> {
        let pal = self.current_palette();
        if !is_resource_path(path) {

            let slots: Vec<Option<Gd<Material>>> = self.data().map(|d| d.bind().material_slots().iter_shared().collect()).unwrap_or_default();
            let lost = slots.iter().zip(&pal.materials).filter(|(m, r)| m.is_some() && r.path.is_empty()).count();
            let mut f = FileAccess::open(path, godot::classes::file_access::ModeFlags::WRITE).ok_or_else(|| format!("could not write {path}"))?;
            f.store_string(&pal.to_xogot_json());
            f.close();

            return Ok(if lost > 0 { format!(" ({lost} slot material{} without a file are not in the JSON: save as .tres to keep them)", if lost == 1 { "" } else { "s" }) } else { String::new() });
        }
        let xp = pal.to_xogot();
        let slots: Vec<Option<Gd<Material>>> = self.data().map(|d| d.bind().material_slots().iter_shared().collect()).unwrap_or_default();
        let mut materials = VarArray::new();
        let mut labels = PackedStringArray::new();
        for i in 0..xp.materials.len() {
            let m = slots.get(i).cloned().flatten();
            labels.push(&m.as_ref().map(|m| m.get_name()).unwrap_or_default());
            match m {
                Some(m) => materials.push(&m.to_variant()),
                None => materials.push(&Variant::nil()),
            }
        }
        let colors: PackedColorArray = xp.colors.iter().map(|c| Color::from_rgba(c[0], c[1], c[2], c[3])).collect();
        let color_labels: PackedStringArray = (0..xp.colors.len()).map(|i| GString::from(xp.labels.get(&i).map(|s| s.as_str()).unwrap_or(""))).collect();
        let empty: PackedInt32Array = xp.empty_color_indices.iter().map(|i| *i as i32).collect();
        let mut dict = VarDictionary::new();
        dict.set(xg_paint::TRES_KEY_VERSION, xg_paint::PALETTE_VERSION);
        dict.set(xg_paint::TRES_KEY_MATERIALS, &materials);
        dict.set(xg_paint::TRES_KEY_COLORS, &colors);
        dict.set(xg_paint::TRES_KEY_LABELS, &labels);
        dict.set(xg_paint::TRES_KEY_EMPTY, &empty);
        dict.set(xg_paint::TRES_KEY_COLOR_LABELS, &color_labels);
        let mut res = Resource::new_gd();
        res.set_meta(xg_paint::PALETTE_MARKER, &dict.to_variant());
        let err = ResourceSaver::singleton().save_ex(&res).path(path).done();
        if err != godot::global::Error::OK {
            return Err(format!("could not save {path} ({err:?})"));
        }
        Ok(String::new())
    }

    /// Loads a palette (`.tres` / `.res` resource or JSON, builder or Xogot shape): swatches
    /// from its colours (cleared colours keep the swatch), slots from its materials (resources,
    /// built-ins generated); slots the palette leaves empty stay as they are. One undo step for
    /// the slots.
    pub(super) fn palette_load(&mut self, path: &str) {
        let loaded = if is_resource_path(path) { read_tres_palette(path) } else { read_json_palette(path) };
        let (colors, empty, mats) = match loaded {
            Ok(x) => x,
            Err(e) => {
                self.message = format!("Palette: {e}");
                return;
            }
        };
        for (i, c) in colors.iter().take(self.swatches.len()).enumerate() {
            if !empty.contains(&i) {
                self.swatches[i] = Color::from_rgba(c[0], c[1], c[2], c[3]);
            }
        }
        let mut applied = 0;
        if let Some(data) = self.data() {
            let old = data.bind().material_slots();
            let mut new = old.duplicate_shallow();
            for (i, mat) in mats.iter().enumerate() {
                let Some(mat) = mat else { continue };
                while new.len() <= i {
                    new.push(None::<&Gd<Material>>);
                }
                new.set(i, Some(mat));
                applied += 1;
            }
            if applied > 0 {
                self.commit_materials("Pixel Builder: Load palette", &data, old, new);
            }
        }
        self.refresh_swatches();
        self.message = format!("Palette loaded from {path} ({applied} slot{})", if applied == 1 { "" } else { "s" });
        self.sync_ui();
    }

    /// Opens the shared palette Load / Save dialog (both layouts).
    pub(super) fn open_palette_dialog(&mut self, save: bool) {
        use godot::classes::file_dialog::{Access, FileMode};
        if self.palette_dialog.as_ref().is_none_or(|d| !d.is_instance_valid()) {
            let mut d = EditorFileDialog::new_alloc();
            d.set_name("PixelBuilderPaletteFiles");
            d.set_access(Access::RESOURCES);
            d.connect("file_selected", &self.to_gd().callable("on_palette_file"));
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&d);
            }
            self.palette_dialog = Some(d);
        }
        self.palette_dialog_save = save;
        let Some(d) = &mut self.palette_dialog else { return };
        d.clear_filters();
        d.add_filter_ex("*.tres, *.res").description("Mesh palette resources").done();
        d.add_filter_ex("*.json").description("Mesh palettes (JSON)").done();
        d.set_file_mode(if save { FileMode::SAVE_FILE } else { FileMode::OPEN_FILE });
        d.set_title(if save { "Save Palette" } else { "Load Palette" });
        if save {
            d.set_current_file("palette.tres");
        }
        d.popup_file_dialog();
    }

    /// The palette state for tests: `{slot, color, smooth_group, groups, swatches}`.
    pub(super) fn paint_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("slot", self.slot);
        d.set("color", self.color);
        d.set("smooth_group", self.xg.smooth_group as i64);
        let groups: PackedInt32Array = self.smoothing_groups().iter().map(|g| g.0 as i32).collect();
        d.set("groups", &groups);
        let sw: PackedColorArray = self.swatches.iter().copied().collect();
        d.set("swatches", &sw);
        if let Some(LayoutUi::Mst(u)) = &self.ui {
            d.set("chips", u.chips.get_child_count() as i64);
            d.set("chip_hint", u.chip_hint.is_visible());
        }
        d
    }

    /// Swatches back to the default colours.
    pub(super) fn palette_reset(&mut self) {
        self.swatches = ui::SWATCHES;
        self.refresh_swatches();
        self.sync_ui();
    }

    /// Restyles every swatch of the active layout.
    pub(super) fn refresh_swatches(&mut self) {
        let sw = self.swatches;
        if let Some(u) = &mut self.ui {
            for (i, c) in sw.iter().enumerate() {
                u.set_swatch(i, *c);
            }
        }
    }

    /// Swatch `i`: use its colour (`paint` also paints it onto the selected faces).
    pub(super) fn swatch_use(&mut self, i: usize, paint: bool) {
        let Some(c) = self.swatches.get(i).copied() else { return };
        self.color = c;
        if paint {
            self.run_action(ui::Action::PaintColour);
        } else {
            self.sync_ui();
        }
    }

    /// Swatch `i` stores the current colour.
    pub(super) fn swatch_store(&mut self, i: usize) {
        if i < self.swatches.len() {
            self.swatches[i] = self.color;
            self.refresh_swatches();
            self.sync_ui();
        }
    }
}

fn is_resource_path(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.ends_with(".tres") || l.ends_with(".res")
}

/// Material of a palette reference: a built-in preset (generated) or a resource path.
fn resolve_reference(path: &str) -> Option<Gd<Material>> {
    if let Some(p) = xg_paint::Preset::from_reference(path) {
        return Some(preset_material(p).upcast());
    }
    if path.is_empty() || !ResourceLoader::singleton().exists(path) {
        return None;
    }
    ResourceLoader::singleton().load(path).and_then(|x| x.try_cast::<Material>().ok())
}

type LoadedPalette = (Vec<[f32; 4]>, BTreeSet<usize>, Vec<Option<Gd<Material>>>);

/// A JSON palette (port of `load_palette`'s JSON branch: `Palette::from_json` validates).
fn read_json_palette(path: &str) -> Result<LoadedPalette, String> {
    if !FileAccess::file_exists(path) {
        return Err(format!("{path} does not exist"));
    }
    let pal = xg_paint::Palette::from_json(&FileAccess::get_file_as_string(path).to_string())?;
    let mats = pal.materials.iter().map(|r| resolve_reference(&r.path)).collect();
    Ok((pal.colors, pal.empty_color_indices, mats))
}

/// A `.tres` palette (port of `palette_values` + `load_palette`): the resource must carry the
/// `_xogot_modeler_palette` dictionary of version 1; stored materials are used as they are
/// (embedded ones included). Loaded uncached so a re-saved file is read again.
fn read_tres_palette(path: &str) -> Result<LoadedPalette, String> {
    let unsupported = || "This resource is not a supported mesh palette.".to_string();
    let res = ResourceLoader::singleton()
        .load_ex(path)
        .cache_mode(godot::classes::resource_loader::CacheMode::IGNORE)
        .done()
        .ok_or_else(unsupported)?;
    if !res.has_meta(xg_paint::PALETTE_MARKER) {
        return Err(unsupported());
    }
    let d = res.get_meta(xg_paint::PALETTE_MARKER).try_to::<VarDictionary>().map_err(|_| unsupported())?;
    if d.get(xg_paint::TRES_KEY_VERSION).and_then(|v| v.try_to::<i64>().ok()) != Some(xg_paint::PALETTE_VERSION) {
        return Err(unsupported());
    }
    let mats_arr = d.get(xg_paint::TRES_KEY_MATERIALS).and_then(|v| v.try_to::<VarArray>().ok()).ok_or_else(unsupported)?;
    let colors_arr = d.get(xg_paint::TRES_KEY_COLORS).and_then(|v| v.try_to::<PackedColorArray>().ok()).ok_or_else(unsupported)?;
    let empty: BTreeSet<usize> = d
        .get(xg_paint::TRES_KEY_EMPTY)
        .and_then(|v| v.try_to::<PackedInt32Array>().ok())
        .map(|a| a.as_slice().iter().filter(|i| **i >= 0).map(|i| *i as usize).collect())
        .unwrap_or_default();
    let mats: Vec<Option<Gd<Material>>> = mats_arr.iter_shared().map(|v| v.try_to::<Gd<Material>>().ok()).collect();
    let colors: Vec<[f32; 4]> = colors_arr.as_slice().iter().map(|c| [c.r, c.g, c.b, c.a]).collect();
    // `validated`: finite colours, cleared indices naming existing colours.
    let paths: Vec<String> = mats.iter().map(|_| String::new()).collect();
    let mut check = xg_paint::Palette::from_slots(&paths, &colors);
    check.empty_color_indices = empty.clone();
    check.validated()?;
    Ok((colors, empty, mats))
}

#[godot_api(secondary)]
impl PixelBuilderPlugin {
    /// MST palette *+ Preset* menu: a slot with built-in preset `id`.
    #[func]
    fn on_preset_menu(&mut self, id: i64) {
        self.add_preset_slot(id.max(0) as usize);
        self.sync_ui();
    }

    /// MST palette *Palette* menu: Load… / Save… / Reset Colours.
    #[func]
    fn on_palette_menu(&mut self, id: i64) {
        match id {
            ui::PALETTE_LOAD => self.open_palette_dialog(false),
            ui::PALETTE_SAVE => self.open_palette_dialog(true),
            ui::PALETTE_RESET => {
                self.palette_reset();
                self.message = "Swatches reset to the default colours".into();
            }
            _ => {}
        }
        self.sync_ui();
    }

    /// The palette dialog's file.
    #[func]
    fn on_palette_file(&mut self, path: GString) {
        let p = path.to_string();
        if self.palette_dialog_save {
            self.palette_save(&p);
        } else {
            self.palette_load(&p);
        }
        self.sync_ui();
    }

    #[func]
    fn on_paint_sample(&mut self) {
        self.sample_selection();
    }

    /// Right-click on an MST smoothing chip: its menu.
    #[func]
    fn on_chip_input(&mut self, event: Gd<InputEvent>, group: i64) {
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else { return };
        if !mb.is_pressed() || mb.get_button_index() != MouseButton::RIGHT {
            return;
        }
        let at = mb.get_global_position();
        if let Some(LayoutUi::Mst(u)) = &mut self.ui {
            u.open_chip_menu(group, at);
        }
    }

    /// Test hook: save (`save`) or load a palette file without the dialog; returns the message.
    #[func]
    fn debug_palette_file(&mut self, path: GString, save: bool) -> GString {
        self.palette_dialog_save = save;
        self.on_palette_file(path);
        self.message.as_str().into()
    }

    /// Test hook: `{slot, color, smooth_group, groups, swatches, chips, chip_hint}`.
    #[func]
    fn debug_paint_state(&self) -> VarDictionary {
        self.paint_state()
    }

    /// Test hook: the smoothing overlay `{shown, tint_triangles, normal_lines, rebuilds}`.
    #[func]
    fn debug_attr_overlay(&mut self) -> VarDictionary {
        self.attr_overlay_state()
    }

    /// Test hook: forces a redraw (as a hover / edit would).
    #[func]
    fn debug_redraw(&mut self) {
        self.redraw();
    }
}
