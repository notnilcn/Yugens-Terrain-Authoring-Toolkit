//! Moves builder meshes in and out of files and turns ordinary meshes into builder meshes.
//!
//! The export-options and conversion dialogs are instantiated from their scenes the first
//! time someone asks for them and then reused, because they are part of the editor's own
//! window and exist only once the person editing opens one.
use super::super::data::to_p;
use super::super::modeler_import;
use super::super::node::{PixelBuilderMesh, surface_arrays};
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use super::convert_dialog::PixelBuilderConvertDialog;
use super::export_options::PixelBuilderExportOptions;
use super::editor_dialogs::add_to_editor;
use super::editor_scenes;
use super::objects::affine;
use godot::classes::file_access::ModeFlags;
use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{
    AcceptDialog, ArrayMesh, CheckBox, ConfirmationDialog, CsgShape3D, EditorFileDialog,
    EditorInterface, FileAccess, Material, Mesh, MeshInstance3D, OptionButton, SpinBox,
};
use godot::prelude::*;
use pt_modeler::bezier::BezierRecipe;
use pt_modeler::compile::{CompileOptions, bake};
use pt_modeler::convert::{self, ImportOptions, MergeMode};
use pt_modeler::xg_export::{self, ExportObject, ExportOptions};
use pt_modeler::{BMesh, ShapeSpec, modeler_meta};
use pt_core::math::Vec3 as PVec3;
use std::collections::BTreeSet;
pub enum Recipe {
    Shape(ShapeSpec),
    Bezier(BezierRecipe),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Obj,
    Stl,
    Ply,
    Res,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportSettings {
    pub selected_only: bool,
    pub apply_transforms: bool,
    pub vertex_colors: bool,
    pub right_handed: bool,
    pub combine: bool,
    pub stl_binary: bool,
}
impl Default for ExportSettings {
    fn default() -> Self {
        ExportSettings {
            selected_only: false,
            apply_transforms: false,
            vertex_colors: false,
            right_handed: true,
            combine: false,
            stl_binary: true,
        }
    }
}
impl ExportSettings {
    pub fn classic_obj(&self) -> bool {
        !self.selected_only
            && !self.apply_transforms
            && !self.vertex_colors
            && self.right_handed
            && !self.combine
    }
    fn fetch(&self, i: usize) -> bool {
        [
            self.selected_only,
            self.apply_transforms,
            self.vertex_colors,
            self.right_handed,
            self.combine,
            self.stl_binary,
        ][i]
    }
    fn assign(&mut self, i: usize, v: bool) {
        match i {
            0 => self.selected_only = v,
            1 => self.apply_transforms = v,
            2 => self.vertex_colors = v,
            3 => self.right_handed = v,
            4 => self.combine = v,
            _ => self.stl_binary = v,
        }
    }
}
#[derive(Default)]
pub struct Dialogs {
    pub choices: Option<Gd<AcceptDialog>>,
    pub option_checks: Vec<Gd<CheckBox>>,
    pub transmute: Option<Gd<ConfirmationDialog>>,
    pub convert_merge: Option<Gd<OptionButton>>,
    pub convert_checks: Vec<Gd<CheckBox>>,
    pub convert_angle: Option<Gd<SpinBox>>,
    pub reusable: Option<Gd<EditorFileDialog>>,
}
impl Dialogs {
    pub fn free_all(&mut self) {
        let nodes: Vec<Gd<Node>> = [
            self.choices.take().map(|d| d.upcast::<Node>()),
            self.transmute.take().map(|d| d.upcast::<Node>()),
            self.reusable.take().map(|d| d.upcast::<Node>()),
        ]
        .into_iter()
        .flatten()
        .collect();
        for n in nodes {
            if n.is_instance_valid() {
                n.free();
            }
        }
        self.option_checks.clear();
        self.convert_checks.clear();
        self.convert_merge = None;
        self.convert_angle = None;
    }
}
fn write_bytes(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut f =
        FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| format!("cannot write {path}"))?;
    f.store_buffer(&PackedByteArray::from(bytes));
    f.close();
    Ok(())
}
pub fn mtl_entries(
    mats: &[Option<Gd<Material>>],
    used: &BTreeSet<u16>,
    names: &[String],
) -> Vec<(String, [f32; 3], Option<String>)> {
    use godot::classes::BaseMaterial3D;
    use godot::classes::base_material_3d::TextureParam;
    used.iter()
        .map(|s| {
            let m = mats.get(*s as usize).cloned().flatten();
            let c = m
                .as_ref()
                .and_then(super::super::ui::material_color)
                .unwrap_or(Color::WHITE);
            let tex = m
                .and_then(|m| m.try_cast::<BaseMaterial3D>().ok())
                .and_then(|b| b.get_texture(TextureParam::ALBEDO))
                .map(|t| t.get_path().to_string())
                .filter(|p| !p.is_empty())
                .map(|p| {
                    godot::classes::ProjectSettings::singleton()
                        .globalize_path(&p)
                        .to_string()
                });
            let name = names
                .get(*s as usize)
                .cloned()
                .unwrap_or_else(|| format!("slot_{s}"));
            (name, [c.r, c.g, c.b], tex)
        })
        .collect()
}
pub struct Triangles {
    pub positions: Vec<PVec3>,
    pub tris: Vec<[u32; 3]>,
    pub slots: Vec<u16>,
    pub normals: Vec<PVec3>,
    pub has_normals: bool,
    pub mats: Array<Option<Gd<Material>>>,
}
pub fn read_triangles(mesh: &Gd<Mesh>, owner: Option<&Gd<MeshInstance3D>>) -> Triangles {
    let mut t = Triangles {
        positions: Vec::new(),
        tris: Vec::new(),
        slots: Vec::new(),
        normals: Vec::new(),
        has_normals: true,
        mats: Array::new(),
    };
    for s in 0..mesh.get_surface_count() {
        let arrays = mesh.surface_get_arrays(s);
        let Ok(verts) = arrays
            .at(ArrayType::VERTEX.ord() as usize)
            .try_to::<PackedVector3Array>()
        else {
            continue;
        };
        let base = t.positions.len() as u32;
        t.positions
            .extend(verts.as_slice().iter().map(|p| to_p(*p)));
        match arrays
            .at(ArrayType::NORMAL.ord() as usize)
            .try_to::<PackedVector3Array>()
        {
            Ok(n) if n.len() == verts.len() => {
                t.normals.extend(n.as_slice().iter().map(|p| to_p(*p)))
            }
            _ => {
                t.has_normals = false;
                t.normals
                    .extend(std::iter::repeat_n(PVec3::ZERO, verts.len()));
            }
        }
        let idx: Vec<u32> = match arrays
            .at(ArrayType::INDEX.ord() as usize)
            .try_to::<PackedInt32Array>()
        {
            Ok(i) if !i.is_empty() => i.as_slice().iter().map(|x| *x as u32).collect(),
            _ => (0..verts.len() as u32).collect(),
        };
        for tri in idx.chunks_exact(3) {
            t.tris.push([base + tri[0], base + tri[1], base + tri[2]]);
            t.slots.push(s as u16);
        }
        let over = owner.and_then(|o| {
            if s < o.get_surface_override_material_count() {
                o.get_surface_override_material(s)
            } else {
                None
            }
        });
        let mat = over.or_else(|| mesh.surface_get_material(s));
        t.mats.push(mat.as_ref());
    }
    t
}
impl PixelBuilderPlugin {
    pub(super) fn unveil_ship_out_xtra(&mut self, fmt: Format) {
        let Some(t) = self.target.clone() else { return };
        let mut dlg = match self.export_dialog.clone().filter(|d| d.is_instance_valid()) {
            Some(d) => d,
            None => {
                let d = super::editor_dialogs::file_dialog(
                    None,
                    Some(FileMode::SAVE_FILE),
                    Access::FILESYSTEM,
                    &self.to_gd().callable("on_export_file"),
                );
                self.export_dialog = Some(d.clone());
                d
            }
        };
        dlg.clear_filters();
        let (ext, desc) = match fmt {
            Format::Obj => ("obj", "Wavefront OBJ"),
            Format::Stl => ("stl", "STL"),
            Format::Ply => ("ply", "PLY (ASCII)"),
            Format::Res => ("res", "Godot mesh resource"),
        };
        dlg.add_filter_ex(&format!("*.{ext}"))
            .description(desc)
            .done();
        if fmt == Format::Res {
            dlg.add_filter_ex("*.tres")
                .description("Godot mesh resource (text)")
                .done();
        }
        dlg.set_title(&format!("Export builder mesh ({})", ext.to_uppercase()));
        dlg.set_current_file(&format!("{}.{ext}", t.get_name()));
        dlg.popup_file_dialog();
    }
    pub(super) fn unveil_ship_out_choices(&mut self) {
        if self
            .xg_dialogs
            .choices
            .as_ref()
            .is_none_or(|d| !d.is_instance_valid())
        {
            let Some(root) = editor_scenes::instantiate::<PixelBuilderExportOptions>(
                editor_scenes::EXPORT_OPTIONS_DIALOG,
            ) else {
                return;
            };
            let checks: Option<Vec<Gd<CheckBox>>> = {
                let r = root.bind();
                [
                    r.selected_only.clone(),
                    r.apply_transforms.clone(),
                    r.vertex_colours.clone(),
                    r.right_handed.clone(),
                    r.combine.clone(),
                    r.stl_binary.clone(),
                ]
                .into_iter()
                .collect()
            };
            let dlg = root.upcast::<AcceptDialog>();
            let Some(mut checks) = checks else {
                godot_error!(
                    "Pixel Builder: the export options scene does not assign all of its choices"
                );
                dlg.free();
                return;
            };
            for (i, cb) in checks.iter_mut().enumerate() {
                cb.connect(
                    "toggled",
                    &self
                        .to_gd()
                        .callable("on_export_opt")
                        .bind(&[(i as i64).to_variant()]),
                );
            }
            add_to_editor(&dlg);
            self.xg_dialogs.choices = Some(dlg);
            self.xg_dialogs.option_checks = checks;
        }
        let o = self.export_opts;
        for (i, cb) in self.xg_dialogs.option_checks.iter_mut().enumerate() {
            cb.set_pressed_no_signal(o.fetch(i));
        }
        if let Some(d) = &mut self.xg_dialogs.choices {
            d.popup_centered();
        }
    }
    pub(super) fn set_export_opt(&mut self, i: usize, on: bool) {
        self.export_opts.assign(i.min(5), on);
    }
    fn export_targets(&self) -> Vec<Gd<PixelBuilderMesh>> {
        let Some(t) = self.target.clone() else {
            return Vec::new();
        };
        let mut out = vec![t.clone()];
        if self.export_opts.combine
            && let Some(sel) = EditorInterface::singleton().get_selection()
        {
            for n in sel.get_selected_nodes().iter_shared() {
                if !super::super::solid_role::is_operand(&n)
                    && let Ok(b) = n.try_cast::<PixelBuilderMesh>()
                    && b.instance_id() != t.instance_id()
                    && b.bind().contents().is_some()
                {
                    out.push(b);
                }
            }
        }
        out
    }
    pub(super) fn export_xg(&mut self, path: &str) -> Result<String, String> {
        let lower = path.to_lowercase();
        let o = self.export_opts;
        let targets = self.export_targets();
        if targets.is_empty() {
            return Err("no builder mesh selected".into());
        }
        let selected = (o.selected_only).then(|| self.sel_faces.clone());
        if o.selected_only && (self.mode != Mode::Face || self.sel_faces.is_empty()) {
            return Err("Selected faces only: select faces in Face mode first".into());
        }
        let stem = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("mesh")
            .rsplit_once('.')
            .map(|(a, _)| a)
            .unwrap_or("mesh")
            .to_string();
        let mtl_name = format!("{stem}.mtl");
        struct Src {
            moniker: String,
            cage: BMesh,
            xf: Transform3D,
            units: f32,
            mats: Vec<Option<Gd<Material>>>,
        }
        let srcs: Vec<Src> = targets
            .iter()
            .filter_map(|t| {
                let d = t.bind().contents()?;
                let db = d.bind();
                Some(Src {
                    moniker: t.get_name().to_string(),
                    cage: db.cage().clone(),
                    xf: t.get_global_transform(),
                    units: db.units(),
                    mats: db.stuff_pockets().iter_shared().collect(),
                })
            })
            .collect();
        let objs: Vec<ExportObject> = srcs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut e = ExportObject::new(s.moniker.clone(), &s.cage);
                e.warp = affine(s.xf);
                e.facets = if i == 0 { selected.as_ref() } else { None };
                e.bake = CompileOptions {
                    units_per_repeat: s.units,
                    world_offset: to_p(s.xf.origin),
                };
                let prefix = if srcs.len() > 1 {
                    format!("{stem}_{}", s.moniker)
                } else {
                    stem.clone()
                };
                e.material_names = (0..s.mats.len().max(1))
                    .map(|k| format!("{prefix}_slot{k}"))
                    .collect();
                e
            })
            .collect();
        let opt = ExportOptions {
            apply_transforms: o.apply_transforms,
            right_handed: o.right_handed,
            quads: true,
            ngons: true,
            vertex_colors: o.vertex_colors,
            include_uvs: true,
            include_normals: true,
            material_library: lower.ends_with(".obj").then(|| mtl_name.clone()),
        };
        let what = if objs.len() > 1 {
            format!(" ({} meshes)", objs.len())
        } else {
            String::new()
        };
        if lower.ends_with(".stl") {
            if o.stl_binary {
                write_bytes(path, &xg_export::into_stl_bin(&objs, &opt)?)?;
            } else {
                write_bytes(path, xg_export::into_stl_text(&objs, &opt).as_bytes())?;
            }
            return Ok(format!("Exported {path}{what}"));
        }
        if lower.ends_with(".ply") {
            write_bytes(path, xg_export::to_ply(&objs, &opt).as_bytes())?;
            return Ok(format!("Exported {path}{what}"));
        }
        if lower.ends_with(".obj") {
            write_bytes(path, xg_export::to_obj_with(&objs, &opt).as_bytes())?;
            let mut entries = Vec::new();
            for (s, e) in srcs.iter().zip(&objs) {
                let used: BTreeSet<u16> = s
                    .cage
                    .facets
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| e.facets.is_none_or(|f| f.contains(i)))
                    .map(|(_, f)| f.stuff)
                    .collect();
                entries.extend(mtl_entries(&s.mats, &used, &e.material_names));
            }
            let dir = &path[..path.len()
                - path
                    .rsplit(['/', '\\'])
                    .next()
                    .map(|s| s.len())
                    .unwrap_or(0)];
            write_bytes(
                &format!("{dir}{mtl_name}"),
                convert::to_mtl(&entries).as_bytes(),
            )?;
            return Ok(format!("Exported {path}{what}"));
        }
        Err("unknown extension (use .obj, .stl or .ply)".into())
    }
    pub(super) fn export_res(&mut self, path: &str) -> Result<String, String> {
        use godot::classes::ResourceSaver;
        let (combined, count) = self.export_mesh()?;
        let err = ResourceSaver::singleton()
            .save_ex(&combined)
            .path(path)
            .done();
        if err != godot::global::Error::OK {
            return Err(format!("saving {path} failed ({err:?})"));
        }
        let what = if count > 1 {
            format!(" ({count} meshes)")
        } else {
            String::new()
        };
        Ok(format!("Exported {path}{what}"))
    }
    pub(super) fn export_mesh(&mut self) -> Result<(Gd<ArrayMesh>, usize), String> {
        let targets = self.export_targets();
        if targets.is_empty() {
            return Err("no builder mesh selected".into());
        }
        if self.export_opts.selected_only && (self.mode != Mode::Face || self.sel_faces.is_empty())
        {
            return Err("Selected faces only: select faces in Face mode first".into());
        }
        let texel = self.prefs.lightmap_texel;
        let mut combined = ArrayMesh::new_gd();
        for (i, t) in targets.iter().enumerate() {
            let mut t = t.clone();
            let xf = t.get_global_transform();
            let built = t.get_mesh();
            let (src, node_surface): (Gd<Mesh>, Vec<Option<i32>>) =
                if i == 0 && self.export_opts.selected_only {
                    let (mut part, from) = selected_part(&t, &self.sel_faces)?;
                    if texel > 0.0 && part.lightmap_unwrap(xf, texel) != godot::global::Error::OK {
                        return Err(format!("lightmap unwrap failed on {}", t.get_name()));
                    }
                    (part.upcast(), from)
                } else {
                    let unwrapped = if texel > 0.0 {
                        t.bind_mut().unwrapped_mesh(texel)
                    } else {
                        None
                    };
                    let src: Gd<Mesh> = match unwrapped {
                        Some(u) => u.upcast(),
                        None => built
                            .clone()
                            .ok_or_else(|| format!("{} has no compiled mesh", t.get_name()))?,
                    };
                    let from = (0..src.get_surface_count()).map(Some).collect();
                    (src, from)
                };
            if targets.len() == 1 {
                combined.set_lightmap_size_hint(src.get_lightmap_size_hint());
            }
            let mi = t.clone().upcast::<MeshInstance3D>();
            for s in 0..src.get_surface_count() {
                let mut arrays = src.surface_get_arrays(s);
                if self.export_opts.apply_transforms {
                    warp_seqs(&mut arrays, xf);
                }
                combined.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
                let on_node = node_surface.get(s as usize).copied().flatten();
                let material = mi
                    .get_material_override()
                    .or_else(|| {
                        on_node
                            .filter(|n| *n < mi.get_surface_override_material_count())
                            .and_then(|n| mi.get_surface_override_material(n))
                    })
                    .or_else(|| {
                        on_node
                            .zip(built.as_ref())
                            .and_then(|(n, b)| b.surface_get_material(n))
                    })
                    .or_else(|| src.surface_get_material(s));
                if let Some(m) = material {
                    let si = combined.get_surface_count() - 1;
                    let dup = m.duplicate_resource();
                    combined.surface_set_material(si, &dup);
                }
            }
        }
        if combined.get_surface_count() == 0 {
            return Err("the mesh is empty".into());
        }
        combined.set_name(&targets[0].get_name().to_string());
        Ok((combined, targets.len()))
    }
    pub(super) fn convert_selected(&mut self) {
        let Some(src) = self.convert_src.clone() else {
            self.message = "Convert: select a MeshInstance3D or CSG shape".into();
            return;
        };
        if let Ok(mi) = src.clone().try_cast::<MeshInstance3D>()
            && mi
                .get_mesh()
                .is_some_and(|m| modeler_import::envelope(&m).is_some())
        {
            let o = self.convert_opts;
            self.convert_with(o);
            return;
        }
        self.open_convert_dialog();
    }
    fn open_convert_dialog(&mut self) {
        if self
            .xg_dialogs
            .transmute
            .as_ref()
            .is_none_or(|d| !d.is_instance_valid())
        {
            let Some(root) =
                editor_scenes::instantiate::<PixelBuilderConvertDialog>(editor_scenes::CONVERT_DIALOG)
            else {
                return;
            };
            let parts = {
                let r = root.bind();
                (
                    r.merge.clone(),
                    r.keep_materials.clone(),
                    r.keep_smoothing.clone(),
                    r.smoothing_angle.clone(),
                )
            };
            let mut dlg = root.upcast::<ConfirmationDialog>();
            let (Some(ob), Some(keep_materials), Some(keep_smoothing), Some(sb)) = parts else {
                godot_error!("Pixel Builder: the convert dialog scene does not assign all of its choices");
                dlg.free();
                return;
            };
            dlg.connect("confirmed", &self.to_gd().callable("on_convert_confirmed"));
            add_to_editor(&dlg);
            self.xg_dialogs.transmute = Some(dlg);
            self.xg_dialogs.convert_merge = Some(ob);
            self.xg_dialogs.convert_checks = vec![keep_materials, keep_smoothing];
            self.xg_dialogs.convert_angle = Some(sb);
        }
        let o = self.convert_opts;
        if let Some(ob) = &mut self.xg_dialogs.convert_merge {
            ob.select(match o.merge {
                MergeMode::Triangles => 0,
                MergeMode::Quads => 1,
                MergeMode::NGons => 2,
            });
        }
        if let [a, b] = self.xg_dialogs.convert_checks.as_mut_slice() {
            a.set_pressed_no_signal(o.keep_materials);
            b.set_pressed_no_signal(o.keep_smoothing);
        }
        if let Some(sb) = &mut self.xg_dialogs.convert_angle {
            sb.set_value_no_signal(o.smoothing_angle as f64);
        }
        let modeler = self.is_modeler();
        if let Some(d) = &mut self.xg_dialogs.transmute {
            d.set_title(if modeler {
                "Make Editable"
            } else {
                "Convert to builder mesh"
            });
            d.set_ok_button_text(if modeler { "Make Editable" } else { "Convert" });
            d.popup_centered();
        }
    }
    pub(super) fn convert_confirmed(&mut self) {
        let mut o = self.convert_opts;
        if let Some(ob) = &self.xg_dialogs.convert_merge {
            o.merge = match ob.get_selected() {
                0 => MergeMode::Triangles,
                1 => MergeMode::Quads,
                _ => MergeMode::NGons,
            };
        }
        if let [a, b] = self.xg_dialogs.convert_checks.as_slice() {
            o.keep_materials = a.is_pressed();
            o.keep_smoothing = b.is_pressed();
        }
        if let Some(sb) = &self.xg_dialogs.convert_angle {
            o.smoothing_angle = sb.get_value() as f32;
        }
        self.convert_opts = o;
        self.convert_with(o);
    }
    pub(super) fn convert_source(
        &self,
        src: &Gd<Node3D>,
        opts: &ImportOptions,
    ) -> Result<(BMesh, Array<Option<Gd<Material>>>, String), String> {
        let (mesh, owner) = if let Ok(mi) = src.clone().try_cast::<MeshInstance3D>() {
            (
                mi.get_mesh().ok_or("the MeshInstance3D has no mesh")?,
                Some(mi),
            )
        } else if let Ok(mut csg) = src.clone().try_cast::<CsgShape3D>() {
            if !csg.is_root_shape() {
                return Err("select the root of the CSG tree (only it has a baked mesh)".into());
            }
            let baked = csg.bake_static_mesh().filter(|m| m.get_surface_count() > 0);
            let baked = baked.ok_or(
                "the CSG shape has no mesh yet (it updates a frame after a change); try again",
            )?;
            (baked.upcast::<Mesh>(), None)
        } else {
            return Err("select a MeshInstance3D or CSG shape".into());
        };
        let mut fallback = String::new();
        if let Some(env) = modeler_import::envelope(&mesh) {
            let decoded = modeler_import::scan_in(&env)
                .and_then(|x| modeler_meta::to_bmesh(&x, &modeler_meta::XImportOptions::default()));
            match decoded {
                Ok(mut imp) => {
                    let slots_before = imp.surface_of_slot.len();
                    let compacted = compact_slots(&mut imp.cage, &mut imp.surface_of_slot);
                    let mut mats: Array<Option<Gd<Material>>> = Array::new();
                    for surf in &imp.surface_of_slot {
                        let m = surf.and_then(|s| {
                            let s = s as i32;
                            let over = owner.as_ref().and_then(|o| {
                                if s < o.get_surface_override_material_count() {
                                    o.get_surface_override_material(s)
                                } else {
                                    None
                                }
                            });
                            over.or_else(|| {
                                if s < mesh.get_surface_count() {
                                    mesh.surface_get_material(s)
                                } else {
                                    None
                                }
                            })
                        });
                        mats.push(m.as_ref());
                    }
                    let mut note = String::from(
                        "Modeler mesh: n-gons, slots, smoothing, UVs and colours kept",
                    );
                    if imp.split_faces + imp.detached_faces + imp.dropped_triangles > 0 {
                        note += &format!(
                            " ({} faces split, {} detached, {} degenerate triangles dropped)",
                            imp.split_faces, imp.detached_faces, imp.dropped_triangles
                        );
                    }
                    if compacted {
                        note += &format!(
                            "; slot numbers past {MAX_IMPORT_SLOTS} renumbered ({slots_before} → {} slots)",
                            imp.surface_of_slot.len()
                        );
                    }
                    return Ok((imp.cage, mats, note));
                }
                Err(e) => {
                    fallback = format!(
                        " (the Modeler data could not be read: {e}; converted from the triangles)"
                    )
                }
            }
        }
        let t = read_triangles(&mesh, owner.as_ref());
        let normals = t.has_normals.then_some(t.normals.as_slice());
        let m = convert::from_triangles_with(&t.positions, &t.tris, &t.slots, normals, opts);
        let merge = match opts.merge {
            MergeMode::Triangles => "triangles",
            MergeMode::Quads => "quads",
            MergeMode::NGons => "n-gons",
        };
        Ok((m, t.mats, format!("merged into {merge}{fallback}")))
    }
}
pub fn classic_convert() -> ImportOptions {
    ImportOptions {
        merge: MergeMode::NGons,
        keep_materials: true,
        keep_smoothing: false,
        smoothing_angle: 0.0,
        braze: 1e-4,
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_export_opt(&mut self, on: bool, token_id: i64) {
        self.set_export_opt(token_id.max(0) as usize, on);
    }
    #[func]
    fn on_convert_confirmed(&mut self) {
        self.convert_confirmed();
    }
    #[func]
    fn debug_export_options(&mut self, opts: VarDictionary) {
        for (i, k) in [
            "selected_only",
            "apply_transforms",
            "vertex_colors",
            "right_handed",
            "combine",
            "stl_binary",
        ]
        .iter()
        .enumerate()
        {
            if let Some(v) = opts.get(*k).and_then(|v| v.try_to::<bool>().ok()) {
                self.export_opts.assign(i, v);
            }
        }
    }
    #[func]
    fn debug_convert(
        &mut self,
        merge: i64,
        keep_materials: bool,
        keep_smoothing: bool,
        smoothing_angle: f32,
    ) {
        let o = ImportOptions {
            merge: match merge {
                0 => MergeMode::Triangles,
                1 => MergeMode::Quads,
                _ => MergeMode::NGons,
            },
            keep_materials,
            keep_smoothing,
            smoothing_angle,
            braze: 1e-4,
        };
        self.convert_with(o);
    }
}
pub const MAX_IMPORT_SLOTS: usize = 256;
pub fn compact_slots(
    m: &mut pt_modeler::BMesh,
    surface_of_slot: &mut Vec<Option<usize>>,
) -> bool {
    if surface_of_slot.len() <= MAX_IMPORT_SLOTS {
        return false;
    }
    let used: std::collections::BTreeSet<u16> = m.facets.iter().map(|f| f.stuff).collect();
    let map: std::collections::BTreeMap<u16, u16> = used
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, i as u16))
        .collect();
    for f in &mut m.facets {
        f.stuff = map[&f.stuff];
    }
    *surface_of_slot = used
        .iter()
        .map(|s| surface_of_slot.get(*s as usize).copied().flatten())
        .collect();
    true
}
fn selected_part(
    t: &Gd<PixelBuilderMesh>,
    faces: &BTreeSet<usize>,
) -> Result<(Gd<ArrayMesh>, Vec<Option<i32>>), String> {
    let data = t.bind().contents().ok_or("mesh has no data")?;
    let origin = if t.is_inside_tree() {
        t.get_global_position()
    } else {
        t.get_position()
    };
    let compiled = {
        let d = data.bind();
        bake(
            d.cage(),
            &CompileOptions {
                units_per_repeat: d.units(),
                world_offset: to_p(origin),
            },
        )
    };
    let built_surfaces = t.get_mesh().map(|m| m.get_surface_count()).unwrap_or(0);
    let mut part = ArrayMesh::new_gd();
    let mut from = Vec::new();
    for s in &compiled.surfaces {
        let kept = s.keep_faces(faces);
        if kept.ordinals.is_empty() {
            continue;
        }
        part.add_surface_from_arrays(PrimitiveType::TRIANGLES, &surface_arrays(&kept));
        let on_node =
            (0..built_surfaces).find(|n| t.bind().get_surface_slot(*n as i64) == s.stuff as i64);
        if on_node.is_none()
            && let Some(m) = data.bind().stuff_pockets().get(s.stuff as usize).flatten()
        {
            let last = part.get_surface_count() - 1;
            part.surface_set_material(last, &m);
        }
        from.push(on_node);
    }
    if part.get_surface_count() == 0 {
        return Err("Selected faces only: the selection has no faces to write".into());
    }
    Ok((part, from))
}
fn warp_seqs(arrays: &mut VarArray, xf: Transform3D) {
    let vi = ArrayType::VERTEX.ord() as usize;
    let ni = ArrayType::NORMAL.ord() as usize;
    let ti = ArrayType::TANGENT.ord() as usize;
    let nb = xf.basis.inverse().transposed();
    if let Ok(mut p) = arrays.at(vi).try_to::<PackedVector3Array>() {
        for v in p.as_mut_slice() {
            *v = xf * *v;
        }
        arrays.set(vi, &p.to_variant());
    }
    if let Ok(mut n) = arrays.at(ni).try_to::<PackedVector3Array>() {
        for v in n.as_mut_slice() {
            *v = (nb * *v).try_normalized().unwrap_or(*v);
        }
        arrays.set(ni, &n.to_variant());
    }
    if let Ok(mut t) = arrays.at(ti).try_to::<PackedFloat32Array>()
        && t.len() % 4 == 0
    {
        let mirrored = xf.basis.determinant() < 0.0;
        for c in t.as_mut_slice().chunks_mut(4) {
            let v = (nb * Vector3::new(c[0], c[1], c[2]))
                .try_normalized()
                .unwrap_or(Vector3::new(c[0], c[1], c[2]));
            c[0] = v.x;
            c[1] = v.y;
            c[2] = v.z;
            if mirrored {
                c[3] = -c[3];
            }
        }
        arrays.set(ti, &t.to_variant());
    }
}
