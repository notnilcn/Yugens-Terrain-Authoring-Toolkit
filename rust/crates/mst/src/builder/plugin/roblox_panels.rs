//! What the Roblox layout's Material, Color and Align popups and the inspector's Part
//! section do to the scene, each as one undoable step. The choices they offer and the
//! numbers behind them come from the plain rules beside the ribbon.
use super::super::roblox_ui::align::{self, Frame, RelativeTo};
use super::super::roblox_ui::material_catalog::{self as catalog, Entry, Origin, Shelf, SlotChoice};
use super::super::roblox_ui::part_rules::{self, PartField};
use super::super::roblox_ui::pickers::{chip, material_icon, material_name};
use super::grouping::top_selection;
use super::paint_cmds::prefab_stuff;
use super::*;
use crate::editor::terrain_ui::PRESET_DIR;
use crate::terrain::textures::{MarchingSquaresTexturePreset, pattern_texture};
use godot::classes::base_material_3d::{TextureFilter, TextureParam};
use godot::classes::undo_redo::MergeMode;
use godot::classes::{
    ClassDb, DirAccess, EditorFileSystemDirectory, Image, ImageTexture, InputEvent,
    InputEventMouseButton, Material, Node3D, ResourceLoader, StandardMaterial3D, Texture2D,
    VisualInstance3D,
};
use pt_modeler::{topo, xg_paint};
const PICK_KEY: &str = "pixel_picker_key";
const RECENT_CAP: usize = 12;
const PROJECT_FILE_CAP: usize = 300;
#[derive(Default)]
pub(super) struct PanelsState {
    pub(super) click_paint: bool,
    part_material: Option<Gd<Material>>,
    catalog: Vec<Entry>,
    icons: Vec<Option<Gd<Texture2D>>>,
    shown: Vec<usize>,
    shelf: Option<Shelf>,
    query: String,
    recent: Vec<String>,
    generated: Vec<(String, Gd<Material>)>,
}
fn project_material_files() -> Vec<String> {
    let Some(fs) = EditorInterface::singleton().get_resource_filesystem() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut stack: Vec<Gd<EditorFileSystemDirectory>> = fs.get_filesystem().into_iter().collect();
    while let Some(dir) = stack.pop() {
        for i in 0..dir.get_file_count() {
            let ty = dir.get_file_type(i);
            if ClassDb::singleton().is_parent_class(&ty, "Material") {
                out.push(dir.get_file_path(i).to_string());
            }
        }
        for i in 0..dir.get_subdir_count() {
            if let Some(d) = dir.get_subdir(i) {
                stack.push(d);
            }
        }
        if out.len() >= PROJECT_FILE_CAP {
            break;
        }
    }
    out.sort();
    out.truncate(PROJECT_FILE_CAP);
    out
}
fn texture_presets() -> Vec<(String, Gd<MarchingSquaresTexturePreset>)> {
    let mut files: Vec<String> = DirAccess::get_files_at(PRESET_DIR)
        .as_slice()
        .iter()
        .map(|f| f.to_string())
        .filter(|f| f.ends_with(".tres") || f.ends_with(".res"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|f| {
            let path = format!("{PRESET_DIR}{f}");
            let p = ResourceLoader::singleton()
                .load(&path)?
                .try_cast::<MarchingSquaresTexturePreset>()
                .ok()?;
            Some((path, p))
        })
        .collect()
}
fn image_icon(img: Option<Gd<Image>>) -> Option<Gd<Texture2D>> {
    ImageTexture::create_from_image(&img?).map(|t| t.upcast())
}
fn slot_key(m: &Gd<Material>) -> Option<String> {
    if m.has_meta(PICK_KEY) {
        return m.get_meta(PICK_KEY).try_to::<GString>().ok().map(|s| s.to_string());
    }
    let path = m.get_path().to_string();
    (!path.is_empty() && !path.contains("::")).then_some(path)
}
fn terrain_material(path: &str, slot: usize, name: &str) -> Option<Gd<Material>> {
    let preset = ResourceLoader::singleton()
        .load(path)?
        .try_cast::<MarchingSquaresTexturePreset>()
        .ok()?;
    let s = preset.bind().pocket(slot)?;
    let (texture, tint) = {
        let b = s.bind();
        (b.texture.clone().or_else(|| pattern_texture(b.pattern)), b.tint)
    };
    let mut m = StandardMaterial3D::new_gd();
    m.set_name(name);
    m.set_albedo(tint);
    m.set_roughness(1.0);
    m.set_flag(godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR, true);
    m.set_texture_filter(TextureFilter::NEAREST_WITH_MIPMAPS);
    if let Some(t) = texture {
        m.set_texture(TextureParam::ALBEDO, &t);
    }
    Some(m.upcast())
}
fn world_corners(n: &Gd<Node3D>) -> Vec<[f32; 3]> {
    let mut corners = Vec::new();
    let mut visuals: Vec<Gd<VisualInstance3D>> = Vec::new();
    if let Ok(v) = n.clone().try_cast::<VisualInstance3D>() {
        visuals.push(v);
    }
    for c in n
        .find_children_ex("*")
        .type_("VisualInstance3D")
        .owned(false)
        .done()
        .iter_shared()
    {
        if let Ok(v) = c.try_cast::<VisualInstance3D>()
            && v.is_visible_in_tree()
        {
            visuals.push(v);
        }
    }
    for v in &visuals {
        let a = v.get_aabb();
        let xf = v.get_global_transform();
        for i in 0..8 {
            let c = Vector3::new(
                if i & 1 == 0 { a.position.x } else { a.position.x + a.size.x },
                if i & 2 == 0 { a.position.y } else { a.position.y + a.size.y },
                if i & 4 == 0 { a.position.z } else { a.position.z + a.size.z },
            );
            let w = xf * c;
            corners.push([w.x, w.y, w.z]);
        }
    }
    if corners.is_empty() {
        let p = n.get_global_position();
        corners.push([p.x, p.y, p.z]);
    }
    corners
}
fn node_frame(n: &Gd<Node3D>) -> Frame {
    let b = n.get_global_transform().basis.orthonormalized();
    [b.col_a(), b.col_b(), b.col_c()].map(|v| [v.x, v.y, v.z])
}
impl PixelBuilderPlugin {
    pub(super) fn panels_requirement(&self, _a: Action) -> Option<&'static str> {
        None
    }
    pub(super) fn panels_pressed(&self, a: Action) -> Option<bool> {
        match a {
            Action::ClickToPaint => Some(self.panels.click_paint),
            _ => None,
        }
    }
    fn ribbon_button(&self, a: Action) -> Option<Gd<godot::classes::Button>> {
        match &self.ui {
            Some(LayoutUi::Roblox(r)) => r.ribbon.button(a),
            _ => None,
        }
    }
    pub(super) fn act_align_panel(&mut self) {
        let anchor = self.ribbon_button(Action::AlignPanel);
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.pickers.open_align(anchor);
        }
    }
    pub(super) fn act_material_picker(&mut self) {
        self.build_catalog();
        self.show_catalog();
        let anchor = self.ribbon_button(Action::MaterialPicker);
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.pickers.open_material(anchor);
        }
    }
    pub(super) fn act_color_picker(&mut self) {
        let (tint, anchor) = (self.tint, self.ribbon_button(Action::ColorPicker));
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.pickers.open_colour(tint, anchor);
        }
    }
    pub(super) fn act_click_to_paint(&mut self) {
        self.toggle_click_mode(super::roblox_rules::ClickMode::Paint);
        self.message = if self.panels.click_paint {
            "Click a part to paint it with the current colour".into()
        } else {
            "Click to paint: off".into()
        };
    }
    fn build_catalog(&mut self) {
        let mut entries = Vec::new();
        let mut icons = Vec::new();
        if let Some(d) = self.contents() {
            for (i, m) in d.bind().stuff_pockets().iter_shared().enumerate() {
                let name = m.as_ref().map(material_name).unwrap_or_else(|| "(default)".into());
                entries.push(Entry {
                    name: format!("Slot {i}: {name}"),
                    origin: Origin::Slot(i),
                });
                icons.push(m.as_ref().and_then(material_icon));
            }
        }
        for path in project_material_files() {
            let m = ResourceLoader::singleton()
                .load(&path)
                .and_then(|r| r.try_cast::<Material>().ok());
            entries.push(Entry {
                name: catalog::display_name(&path),
                origin: Origin::File(path),
            });
            icons.push(m.as_ref().and_then(material_icon));
        }
        for (i, p) in xg_paint::Preset::ALL.iter().enumerate() {
            let a = p.stuff().albedo;
            entries.push(Entry {
                name: p.heading().to_string(),
                origin: Origin::Prefab(i),
            });
            icons.push(chip(Color::from_rgba(a[0], a[1], a[2], 1.0)));
        }
        for (path, preset) in texture_presets() {
            let b = preset.bind();
            for slot in 0..b.slots.len() {
                entries.push(Entry {
                    name: b.pocket_moniker(slot),
                    origin: Origin::TerrainSlot {
                        preset: path.clone(),
                        slot,
                    },
                });
                icons.push(image_icon(b.preview_image(slot, 32)));
            }
        }
        self.panels.catalog = entries;
        self.panels.icons = icons;
    }
    fn show_catalog(&mut self) {
        let shelf = catalog::opening_shelf(&self.panels.catalog, self.panels.shelf);
        let shown = catalog::visible(
            &self.panels.catalog,
            &self.panels.recent,
            shelf,
            &self.panels.query,
        );
        let rows: Vec<(String, Option<Gd<Texture2D>>)> = shown
            .iter()
            .map(|i| (self.panels.catalog[*i].name.clone(), self.panels.icons[*i].clone()))
            .collect();
        self.panels.shown = shown;
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.pickers.show_shelf(shelf as i64);
            r.ribbon.pickers.show_materials(&rows);
        }
    }
    fn entry_material(&mut self, origin: &Origin, name: &str) -> Option<Gd<Material>> {
        let key = origin.key();
        if let Some((_, m)) = self.panels.generated.iter().find(|g| g.0 == key) {
            return Some(m.clone());
        }
        let mut made = match origin {
            Origin::Slot(_) => return None,
            Origin::File(path) => {
                return ResourceLoader::singleton()
                    .load(path)
                    .and_then(|r| r.try_cast::<Material>().ok());
            }
            Origin::Prefab(i) => prefab_stuff(*xg_paint::Preset::ALL.get(*i)?).upcast::<Material>(),
            Origin::TerrainSlot { preset, slot } => terrain_material(preset, *slot, name)?,
        };
        made.set_meta(PICK_KEY, &key.to_variant());
        self.panels.generated.push((key, made.clone()));
        Some(made)
    }
    fn apply_catalog_entry(&mut self, idx: usize) {
        let Some(entry) = self.panels.catalog.get(idx).cloned() else {
            return;
        };
        let scope = super::roblox_rules::material_scope(
            self.contents().is_some(),
            self.mode,
            !self.sel_faces.is_empty(),
        );
        let faces_mode = match scope {
            Ok(super::roblox_rules::MaterialScope::NewParts) => {
                self.choose_part_material(&entry);
                return;
            }
            Ok(scope) => scope == super::roblox_rules::MaterialScope::SelectedFaces,
            Err(reason) => {
                self.message = reason.into();
                return;
            }
        };
        let material = match entry.origin {
            Origin::Slot(_) => None,
            _ => match self.entry_material(&entry.origin, &entry.name) {
                Some(m) => Some(m),
                None => {
                    self.message = format!("Material: cannot load {}", entry.name);
                    return;
                }
            },
        };
        self.unshare_data();
        let Some(mut d) = self.contents() else { return };
        let (before, old_mats) = (d.bind().get_blob(), d.bind().stuff_pockets());
        let mut mats = old_mats.duplicate_shallow();
        let mut m = d.bind().cage().clone();
        let faces: BTreeSet<usize> = if faces_mode {
            self.sel_faces.iter().copied().filter(|f| *f < m.facets.len()).collect()
        } else {
            (0..m.facets.len()).collect()
        };
        let slot = match (&entry.origin, &material) {
            (Origin::Slot(i), _) => *i,
            (_, Some(mat)) => {
                let keys: Vec<Option<String>> = mats
                    .iter_shared()
                    .map(|x| x.as_ref().and_then(slot_key))
                    .collect();
                let used = m.facets.iter().map(|f| f.stuff as usize + 1).max().unwrap_or(0);
                match catalog::slot_for(&keys, used, &entry.origin.key()) {
                    SlotChoice::Reuse(i) => i,
                    SlotChoice::Append { at } => {
                        while mats.len() < at {
                            mats.push(None::<&Gd<Material>>);
                        }
                        mats.push(Some(mat));
                        at
                    }
                }
            }
            _ => return,
        };
        selection_edits::assign_stuff(&mut m, &faces, slot.min(u16::MAX as usize) as u16);
        d.bind_mut().substitute(m);
        d.set("materials", &mats.to_variant());
        d.emit_changed();
        let after = d.bind().get_blob();
        let Some(mut ur) = self.open_action("Pixel Builder: Apply material") else {
            return;
        };
        ur.add_do_method(&d, "set_blob", &[after.to_variant()]);
        ur.add_do_property(&d, "materials", &mats.to_variant());
        ur.add_undo_method(&d, "set_blob", &[before.to_variant()]);
        ur.add_undo_property(&d, "materials", &old_mats.to_variant());
        ur.commit_action_ex().execute(false).done();
        if !matches!(entry.origin, Origin::Slot(_)) {
            catalog::remember(&mut self.panels.recent, &entry.origin.key(), RECENT_CAP);
        }
        self.pocket = slot as i64;
        self.palette_slots = None;
        self.message = format!(
            "{} applied to {} face{}",
            entry.name,
            faces.len(),
            if faces.len() == 1 { "" } else { "s" }
        );
    }
    fn choose_part_material(&mut self, entry: &Entry) {
        let Some(m) = self.entry_material(&entry.origin, &entry.name) else {
            self.message = format!("Material: cannot load {}", entry.name);
            return;
        };
        self.panels.part_material = Some(m);
        catalog::remember(&mut self.panels.recent, &entry.origin.key(), RECENT_CAP);
        self.message = format!("{}: new parts will use this material", entry.name);
    }
    pub(super) fn give_new_part_material(&mut self, node: &Gd<PixelBuilderMesh>) {
        let Some(mat) = self.panels.part_material.clone() else {
            return;
        };
        let Some(mut d) = node.bind().contents() else {
            return;
        };
        let mut m = d.bind().cage().clone();
        let faces: BTreeSet<usize> = (0..m.facets.len()).collect();
        selection_edits::assign_stuff(&mut m, &faces, 0);
        d.bind_mut().substitute(m);
        let mut mats: Array<Option<Gd<Material>>> = Array::new();
        mats.push(Some(&mat));
        d.set("materials", &mats.to_variant());
        d.emit_changed();
    }
    fn own_data(&mut self, node: &Gd<PixelBuilderMesh>) -> Option<Gd<PixelBuilderMeshData>> {
        if self.target.as_ref().is_some_and(|t| t.instance_id() == node.instance_id()) {
            self.unshare_data();
            return self.contents();
        }
        node.bind().contents()
    }
    fn repaint(
        &mut self,
        node: &Gd<PixelBuilderMesh>,
        faces: Option<&BTreeSet<usize>>,
        tint: Color,
    ) -> Option<(Gd<PixelBuilderMeshData>, PackedByteArray, PackedByteArray)> {
        let mut d = self.own_data(node)?;
        let before = d.bind().get_blob();
        let mut m = d.bind().cage().clone();
        let faces = faces.cloned().unwrap_or_else(|| (0..m.facets.len()).collect());
        selection_edits::assign_tint_keeping_masks(&mut m, &faces, ui_color_bytes(tint));
        d.bind_mut().substitute(m);
        d.emit_changed();
        let after = d.bind().get_blob();
        (after != before).then_some((d, before, after))
    }
    fn paint_parts(
        &mut self,
        targets: &[(Gd<PixelBuilderMesh>, Option<BTreeSet<usize>>)],
        tint: Color,
        merge: MergeMode,
    ) -> usize {
        let steps: Vec<_> = targets
            .iter()
            .filter_map(|(n, faces)| self.repaint(n, faces.as_ref(), tint))
            .collect();
        if steps.is_empty() {
            return 0;
        }
        let Some(mut ur) = self.base().get_undo_redo() else {
            return 0;
        };
        super::undo_steps::open_scene_action(&mut ur, "Pixel Builder: Paint part", merge);
        for (d, before, after) in &steps {
            ur.add_do_method(d, "set_blob", &[after.to_variant()]);
            ur.add_undo_method(d, "set_blob", &[before.to_variant()]);
        }
        ur.commit_action_ex().execute(false).done();
        steps.len()
    }
    fn click_paint(&mut self, node: Gd<PixelBuilderMesh>, face: Option<usize>, click: ClickPaint) {
        let name = node.get_name();
        let data = node.bind().contents();
        match (click, face, data) {
            (ClickPaint::PickColour, Some(f), Some(d)) => {
                let tint = d.bind().cage().facets.get(f).map(|x| x.tint);
                if let Some(t) = tint {
                    self.tint = Color::from_rgba8(t[0], t[1], t[2], t[3]);
                    if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
                        r.ribbon.pickers.show_colour(self.tint);
                    }
                    self.message = format!("Picked the colour of {name}");
                }
            }
            (ClickPaint::FlatArea, Some(f), Some(d)) => {
                let faces = topo::coplanar_faces(d.bind().cage(), f, 1.0);
                let tint = self.tint;
                self.message = if self.paint_parts(&[(node, Some(faces))], tint, MergeMode::DISABLE) > 0 {
                    format!("Painted a flat area of {name}")
                } else {
                    format!("{name}: that flat area already has this colour")
                };
            }
            (ClickPaint::PickColour | ClickPaint::FlatArea, _, _) => {
                self.message = format!("{name}: no face under the cursor");
            }
            (ClickPaint::WholePart, _, _) => {
                let tint = self.tint;
                self.message = if self.paint_parts(&[(node, None)], tint, MergeMode::DISABLE) > 0 {
                    format!("Painted {name}")
                } else {
                    format!("{name} already has this colour")
                };
            }
        }
    }
    pub(super) fn panels_event(&mut self, cam: &Gd<Camera3D>, event: &Gd<InputEvent>) -> Option<bool> {
        let mb = super::parts_model::left_button(event);
        let claim = super::roblox_rules::click_claim(
            self.panels.click_paint,
            self.is_roblox(),
            mb.as_ref().map(|m| m.is_pressed()),
        );
        if claim == super::roblox_rules::ClickClaim::Pass {
            return None;
        }
        if let Some(mb) = mb.filter(|_| claim == super::roblox_rules::ClickClaim::Act) {
            let pos = mb.get_position();
            let (o, d) = (cam.project_ray_origin(pos), cam.project_ray_normal(pos));
            match self.builder_mesh_under(o, d) {
                Some(n) => {
                    let face = face_under_on(&n, o, d);
                    let click = ClickPaint::of(mb.is_shift_pressed(), mb.is_ctrl_pressed());
                    self.click_paint(n, face, click);
                }
                None => self.message = "Click to paint: no builder mesh under the cursor".into(),
            }
            self.sync_ui();
            self.redraw();
        }
        Some(true)
    }
    fn align_selection(&mut self) {
        let (mode, axes, active, local) = match &self.ui {
            Some(LayoutUi::Roblox(r)) => r.ribbon.pickers.align_settings(),
            _ => return,
        };
        let nodes: Vec<Gd<Node3D>> = top_selection()
            .into_iter()
            .filter_map(|n| n.try_cast::<Node3D>().ok())
            .collect();
        if nodes.len() < 2 {
            self.message = "Align: select at least two objects".into();
            return;
        }
        if !axes.iter().any(|a| *a) {
            self.message = "Align: tick at least one axis".into();
            return;
        }
        let active_at = self
            .target
            .as_ref()
            .and_then(|t| nodes.iter().position(|n| n.instance_id() == t.instance_id()))
            .unwrap_or(nodes.len() - 1);
        let frame = if local {
            node_frame(&nodes[active_at])
        } else {
            align::WORLD
        };
        let Some(boxes) = nodes
            .iter()
            .map(|n| align::frame_bounds(&world_corners(n), frame))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let relative = if active {
            RelativeTo::Active(active_at)
        } else {
            RelativeTo::SelectionBounds
        };
        let offsets: Vec<[f32; 3]> = align::align_offsets(&boxes, mode, axes, relative)
            .into_iter()
            .map(|o| align::offset_in_world(o, frame))
            .collect();
        let moves: Vec<(Gd<Node3D>, Vector3, Vector3)> = nodes
            .iter()
            .zip(&offsets)
            .filter(|(_, o)| o.iter().any(|x| x.abs() > 1e-6))
            .map(|(n, o)| {
                let world = Vector3::new(o[0], o[1], o[2]);
                let local = match n.get_parent().and_then(|p| p.try_cast::<Node3D>().ok()) {
                    Some(p) => p.get_global_transform().basis.inverse() * world,
                    None => world,
                };
                let old = n.get_position();
                (n.clone(), old, old + local)
            })
            .collect();
        if moves.is_empty() {
            self.message = "Align: already aligned".into();
            return;
        }
        let Some(mut ur) = self.open_action("Pixel Builder: Align") else {
            return;
        };
        for (n, old, new) in &moves {
            ur.add_do_property(n, "position", &new.to_variant());
            ur.add_undo_property(n, "position", &old.to_variant());
        }
        ur.commit_action();
        self.message = format!("Aligned {} object{}", moves.len(), if moves.len() == 1 { "" } else { "s" });
    }
    fn resize_part(
        &mut self,
        node: &Gd<PixelBuilderMesh>,
        axis: usize,
        value: f32,
    ) -> Result<Option<ResizeStep>, ()> {
        let Some(mut d) = self.own_data(node) else {
            return Ok(None);
        };
        let (spec, next) = {
            let b = d.bind();
            let Some(spec) = b.shape_spec() else {
                return Err(());
            };
            let size = part_rules::with_axis([spec.size.x, spec.size.y, spec.size.z], axis, value);
            (spec, part_rules::resized(&spec, b.cage(), size))
        };
        let Some((next_spec, mesh)) = next else {
            return Err(());
        };
        if next_spec == spec {
            return Ok(None);
        }
        let (before, old_form) = (d.bind().get_blob(), d.get("form"));
        {
            let mut b = d.bind_mut();
            b.substitute(mesh);
            b.set_shape_spec(&next_spec);
        }
        d.emit_changed();
        let (after, new_form) = (d.bind().get_blob(), d.get("form"));
        Ok(Some(ResizeStep {
            data: d,
            before,
            after,
            old_form,
            new_form,
        }))
    }
    fn part_size(&mut self, nodes: &[Gd<PixelBuilderMesh>], axis: usize, value: f32) {
        let mut steps = Vec::new();
        let mut read_only = 0;
        for n in nodes {
            match self.resize_part(n, axis, value) {
                Ok(Some(step)) => steps.push(step),
                Ok(None) => {}
                Err(()) => read_only += 1,
            }
        }
        if read_only > 0 {
            self.message = if nodes.len() == 1 {
                "Size: this mesh no longer matches its recipe, so its size is read only".into()
            } else {
                format!("Size: {read_only} of {} parts no longer match their recipe and keep their size", nodes.len())
            };
        }
        if steps.is_empty() {
            return;
        }
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        super::undo_steps::open_scene_action(&mut ur, "Pixel Builder: Part size", MergeMode::ENDS);
        for st in &steps {
            ur.add_do_method(&st.data, "set_blob", &[st.after.to_variant()]);
            ur.add_do_property(&st.data, "form", &st.new_form);
            ur.add_undo_method(&st.data, "set_blob", &[st.before.to_variant()]);
            ur.add_undo_property(&st.data, "form", &st.old_form);
        }
        ur.commit_action_ex().execute(false).done();
    }
    fn part_property(
        &mut self,
        name: &str,
        property: &str,
        changes: Vec<(Gd<PixelBuilderMesh>, Variant)>,
    ) {
        let changes: Vec<_> = changes
            .into_iter()
            .map(|(n, v)| (n.get(property), n, v))
            .filter(|(old, _, v)| old != v)
            .collect();
        if changes.is_empty() {
            return;
        }
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        super::undo_steps::open_scene_action(&mut ur, name, MergeMode::ENDS);
        for (old, n, v) in &changes {
            ur.add_do_property(n, property, v);
            ur.add_undo_property(n, property, old);
        }
        ur.commit_action();
    }
}
struct ResizeStep {
    data: Gd<PixelBuilderMeshData>,
    before: PackedByteArray,
    after: PackedByteArray,
    old_form: Variant,
    new_form: Variant,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClickPaint {
    WholePart,
    FlatArea,
    PickColour,
}
impl ClickPaint {
    fn of(shift: bool, ctrl: bool) -> ClickPaint {
        if ctrl {
            ClickPaint::PickColour
        } else if shift {
            ClickPaint::FlatArea
        } else {
            ClickPaint::WholePart
        }
    }
}
fn face_under_on(node: &Gd<PixelBuilderMesh>, o: Vector3, d: Vector3) -> Option<usize> {
    let data = node.bind().contents()?;
    let inv = node.get_global_transform().affine_inverse();
    let hit = pick::intersect_ray_closest(
        data.bind().cage(),
        to_p(inv * o),
        to_p((inv.basis * d).normalized()),
        true,
    )?;
    Some(hit.face)
}
fn live(parts: &Array<Gd<PixelBuilderMesh>>) -> Vec<Gd<PixelBuilderMesh>> {
    parts.iter_shared().filter(|n| n.is_instance_valid()).collect()
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_material_search(&mut self, text: GString) {
        self.panels.query = text.to_string();
        self.show_catalog();
    }
    #[func]
    fn on_material_view(&mut self, grid: bool) {
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.pickers.set_grid(grid);
        }
    }
    #[func]
    fn on_material_shelf(&mut self, tab: i64) {
        self.panels.shelf = Some(Shelf::from_i64(tab));
        self.show_catalog();
    }
    #[func]
    fn on_material_item(&mut self, index: i64, _at: Vector2, button: i64) {
        if button != MouseButton::LEFT.ord() as i64 {
            return;
        }
        let Some(idx) = self.panels.shown.get(index as usize).copied() else {
            return;
        };
        self.apply_catalog_entry(idx);
        self.build_catalog();
        self.show_catalog();
        self.sync_ui();
        self.redraw();
    }
    #[func]
    fn on_palette_input(&mut self, event: Gd<InputEvent>) {
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else {
            return;
        };
        if !mb.is_pressed() || mb.get_button_index() != MouseButton::LEFT {
            return;
        }
        let picked = match &mut self.ui {
            Some(LayoutUi::Roblox(r)) => r.ribbon.pickers.palette_pick(mb.get_position()),
            _ => None,
        };
        let Some(c) = picked else { return };
        self.tint = c;
        if self.panels.click_paint {
            self.message = "Click a part to paint it with this colour".into();
        } else if self.requirement(Action::PaintColour as i64).is_none() {
            self.run_action(Action::PaintColour);
            return;
        } else {
            self.message = "Colour chosen: select a part to paint, or turn on click to paint".into();
        }
        self.sync_ui();
    }
    #[func]
    fn on_align_apply(&mut self) {
        self.align_selection();
        self.sync_ui();
        self.redraw();
    }
    #[func]
    fn on_part_field(&mut self, value: f64, field: i64, parts: Array<Gd<PixelBuilderMesh>>) {
        let Some(f) = PartField::from_i64(field) else {
            return;
        };
        let nodes = live(&parts);
        let v = value as f32;
        match f {
            PartField::SizeX | PartField::SizeY | PartField::SizeZ => {
                self.part_size(&nodes, f.axis(), v)
            }
            PartField::PositionX | PartField::PositionY | PartField::PositionZ => {
                let changes = nodes
                    .into_iter()
                    .map(|n| {
                        let p = n.get_position();
                        let [x, y, z] = part_rules::with_axis([p.x, p.y, p.z], f.axis(), v);
                        (n, Vector3::new(x, y, z).to_variant())
                    })
                    .collect();
                self.part_property("Pixel Builder: Part position", "position", changes);
            }
            PartField::OrientationX | PartField::OrientationY | PartField::OrientationZ => {
                let changes = nodes
                    .into_iter()
                    .map(|n| {
                        let r = n.get_rotation_degrees();
                        let [x, y, z] = part_rules::with_axis([r.x, r.y, r.z], f.axis(), v);
                        (n, Vector3::new(x, y, z).to_variant())
                    })
                    .collect();
                self.part_property("Pixel Builder: Part orientation", "rotation_degrees", changes);
            }
            PartField::Transparency => {
                let t = v.clamp(0.0, 1.0).to_variant();
                let changes = nodes.into_iter().map(|n| (n, t.clone())).collect();
                self.part_property("Pixel Builder: Part transparency", "transparency", changes);
            }
        }
        self.redraw();
    }
    #[func]
    fn on_part_color(&mut self, c: Color, parts: Array<Gd<PixelBuilderMesh>>) {
        self.tint = c;
        let targets: Vec<_> = live(&parts).into_iter().map(|n| (n, None)).collect();
        self.paint_parts(&targets, c, MergeMode::ENDS);
        self.redraw();
    }
    #[func]
    fn on_part_collision(&mut self, index: i64, parts: Array<Gd<PixelBuilderMesh>>) {
        let kind = (index.clamp(0, 2) as i32).to_variant();
        let changes = live(&parts).into_iter().map(|n| (n, kind.clone())).collect();
        self.part_property("Pixel Builder: Part collision", "entity_type", changes);
    }
}
#[cfg(test)]
mod tests {
    use super::ClickPaint;
    #[test]
    fn click_to_paint_follows_the_paint_tool_modifiers() {
        assert_eq!(ClickPaint::of(false, false), ClickPaint::WholePart);
        assert_eq!(ClickPaint::of(true, false), ClickPaint::FlatArea);
        assert_eq!(ClickPaint::of(false, true), ClickPaint::PickColour);
        assert_eq!(ClickPaint::of(true, true), ClickPaint::PickColour);
    }
}
