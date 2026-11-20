//! Builder plugin I/O:
//!
//! * export STL (ASCII / binary) and PLY next to OBJ / glTF, with the export options
//!   (selected faces only, world transforms, vertex colours, handedness, combine the selected
//!   builder meshes) through `pt_core::build::xg_export`;
//! * Convert ("Make Editable") options — merge triangles / quads / n-gons, keep materials, keep
//!   smoothing, smoothing angle — and CSG shapes as a source (CSG is baked through
//!   `bake_static_mesh`);
//! * meshes carrying `_modeler` metadata convert losslessly (`builder::modeler_import`).

use std::collections::BTreeSet;

use godot::classes::file_access::ModeFlags;
use godot::classes::file_dialog::{Access, FileMode};
use godot::classes::mesh::ArrayType;
use godot::classes::{
    AcceptDialog, CheckBox, ConfirmationDialog, CsgShape3D, EditorFileDialog, EditorInterface,
    FileAccess, HBoxContainer, Label, Material, Mesh, MeshInstance3D, OptionButton, SpinBox,
    VBoxContainer,
};
use godot::prelude::*;

use pt_core::build::bezier::BezierRecipe;
use pt_core::build::compile::CompileOptions;
use pt_core::build::convert::{self, ImportOptions, MergeMode};
use pt_core::build::xg_export::{self, ExportObject, ExportOptions};
use pt_core::build::{BMesh, ShapeSpec, modeler_meta};
use pt_core::math::Vec3 as PVec3;

use super::super::data::{PixelBuilderMeshData, to_p};
use super::super::modeler_import;
use super::super::node::PixelBuilderMesh;
use super::super::ui::Mode;
use super::PixelBuilderPlugin;
use super::objects::affine;

/// The recipe a created mesh remembers.
pub enum Recipe {
    Shape(ShapeSpec),
    Bezier(BezierRecipe),
}

/// Export file formats of the exporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Obj,
    Stl,
    Ply,
    /// Godot mesh resource: the compiled `ArrayMesh` + materials.
    Res,
}

/// Export options (the popup), shared by OBJ / STL / PLY.
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
    /// The classic OBJ export (unchanged output) applies.
    pub fn classic_obj(&self) -> bool {
        !self.selected_only
            && !self.apply_transforms
            && !self.vertex_colors
            && self.right_handed
            && !self.combine
    }
    /// (label, tooltip) per option row, in `on_export_opt` id order.
    const ROWS: [(&'static str, &'static str); 6] = [
        (
            "Selected faces only",
            "Only the faces selected in Face mode (the export buttons show in Face mode too)",
        ),
        (
            "Apply world transforms",
            "Write world-space positions (the node's position, rotation and scale baked in)",
        ),
        (
            "Vertex colours",
            "Write face / vertex-paint colours (OBJ: after the position, PLY: red green blue alpha)",
        ),
        (
            "Right-handed (Godot axes)",
            "Off mirrors X for left-handed tools",
        ),
        (
            "Combine selected builder meshes",
            "One file with every builder mesh selected in the Scene dock (Ctrl+click them)",
        ),
        ("STL binary", "Off writes ASCII STL"),
    ];
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

/// Dialogs of these features (freed in `exit_tree`).
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

fn add_to_editor(n: &Gd<impl Inherits<Node>>) {
    if let Some(mut base) = EditorInterface::singleton().get_base_control() {
        base.add_child(n);
    }
}

fn write_bytes(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut f =
        FileAccess::open(path, ModeFlags::WRITE).ok_or_else(|| format!("cannot write {path}"))?;
    f.store_buffer(&PackedByteArray::from(bytes));
    f.close();
    Ok(())
}

/// MTL text for the slots `used` of `mats`, named by `names` (shared with the classic OBJ export).
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

/// Triangles of a mesh's surfaces (Godot winding), with per-position normals and the surface
/// materials (`owner`'s surface overrides first).
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
    // ================================================================== export

    /// Opens the save dialog for an OBJ / STL / PLY export.
    pub(super) fn unveil_ship_out_xtra(&mut self, fmt: Format) {
        let Some(t) = self.target.clone() else { return };
        let mut dlg = match self.export_dialog.clone().filter(|d| d.is_instance_valid()) {
            Some(d) => d,
            None => {
                let mut d = EditorFileDialog::new_alloc();
                d.set_file_mode(FileMode::SAVE_FILE);
                d.set_access(Access::FILESYSTEM);
                d.connect("file_selected", &self.to_gd().callable("on_export_file"));
                add_to_editor(&d);
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

    /// The export options popup.
    pub(super) fn unveil_ship_out_choices(&mut self) {
        if self
            .xg_dialogs
            .choices
            .as_ref()
            .is_none_or(|d| !d.is_instance_valid())
        {
            let mut dlg = AcceptDialog::new_alloc();
            dlg.set_title("Builder export options");
            let mut col = VBoxContainer::new_alloc();
            let mut head = Label::new_alloc();
            head.set_text(
                "Used by Export OBJ / STL / PLY; Godot Mesh uses world transforms and combine
(glTF exports the whole mesh)",
            );
            col.add_child(&head);
            let mut checks = Vec::new();
            for (i, (text, tip)) in ExportSettings::ROWS.iter().enumerate() {
                let mut cb = CheckBox::new_alloc();
                cb.set_text(*text);
                cb.set_tooltip_text(*tip);
                cb.connect(
                    "toggled",
                    &self
                        .to_gd()
                        .callable("on_export_opt")
                        .bind(&[(i as i64).to_variant()]),
                );
                col.add_child(&cb);
                checks.push(cb);
            }
            dlg.add_child(&col);
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

    /// Builder meshes exported together (the target, plus the other selected ones when combining).
    fn export_targets(&self) -> Vec<Gd<PixelBuilderMesh>> {
        let Some(t) = self.target.clone() else {
            return Vec::new();
        };
        let mut out = vec![t.clone()];
        if self.export_opts.combine
            && let Some(sel) = EditorInterface::singleton().get_selection()
        {
            for n in sel.get_selected_nodes().iter_shared() {
                if let Ok(b) = n.try_cast::<PixelBuilderMesh>()
                    && b.instance_id() != t.instance_id()
                    && b.bind().contents().is_some()
                {
                    out.push(b);
                }
            }
        }
        out
    }

    /// OBJ (with options) / STL / PLY export of the target(s).
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
        // Snapshot every mesh first (ExportObject borrows them).
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

    /// Godot Mesh export: the compiled `ArrayMesh` of the target (and, with *Combine*, of the
    /// other selected builder meshes) saved with `ResourceSaver` as `.res` / `.tres`.
    ///
    /// Surfaces are copied with the effective material of each surface duplicated into the copy;
    /// one combined mesh, `lightmap_size_hint` kept for a single input, `ResourceSaver.save`. The
    /// copy is a new resource, so saving never moves the node's own mesh to the file. *Apply
    /// world transforms* bakes each node's transform into its surface arrays; with several meshes
    /// and no world transforms each keeps its local space. Lightmap UV2 comes along when the mesh
    /// has it (Live UV2 / the preferences' texel, as in the glTF export). Selected-only / vertex
    /// colours / handedness don't apply.
    pub(super) fn export_res(&mut self, path: &str) -> Result<String, String> {
        use godot::classes::mesh::PrimitiveType;
        use godot::classes::{ArrayMesh, ResourceSaver};
        let targets = self.export_targets();
        if targets.is_empty() {
            return Err("no builder mesh selected".into());
        }
        let texel = self.prefs.lightmap_texel;
        let mut combined = ArrayMesh::new_gd();
        for t in &targets {
            let mut t = t.clone();
            // UV2 at the preferences' texel when it fits the atlas (else the plain compiled mesh).
            let unwrapped = if texel > 0.0 {
                t.bind_mut().unwrapped_mesh(texel)
            } else {
                None
            };
            let src: Gd<Mesh> = match unwrapped {
                Some(u) => u.upcast(),
                None => t
                    .get_mesh()
                    .ok_or_else(|| format!("{} has no compiled mesh", t.get_name()))?,
            };
            if targets.len() == 1 {
                combined.set_lightmap_size_hint(src.get_lightmap_size_hint());
            }
            let xf = t.get_global_transform();
            let mi = t.clone().upcast::<MeshInstance3D>();
            for s in 0..src.get_surface_count() {
                let mut arrays = src.surface_get_arrays(s);
                if self.export_opts.apply_transforms {
                    warp_seqs(&mut arrays, xf);
                }
                combined.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
                let material = mi
                    .get_material_override()
                    .or_else(|| {
                        if s < mi.get_surface_override_material_count() {
                            mi.get_surface_override_material(s)
                        } else {
                            None
                        }
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
        let err = ResourceSaver::singleton()
            .save_ex(&combined)
            .path(path)
            .done();
        if err != godot::global::Error::OK {
            return Err(format!("saving {path} failed ({err:?})"));
        }
        let what = if targets.len() > 1 {
            format!(" ({} meshes)", targets.len())
        } else {
            String::new()
        };
        Ok(format!("Exported {path}{what}"))
    }

    // ================================================================== convert

    /// Convert: meshes with editable metadata convert at once (lossless); other meshes / CSG
    /// open the options.
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
            let mut dlg = ConfirmationDialog::new_alloc();
            dlg.set_title("Convert to builder mesh");
            dlg.set_ok_button_text("Convert");
            let mut col = VBoxContainer::new_alloc();
            let mut row = HBoxContainer::new_alloc();
            let mut l = Label::new_alloc();
            l.set_text("Merge triangles into");
            row.add_child(&l);
            let mut ob = OptionButton::new_alloc();
            for t in ["Triangles", "Quads", "N-gons"] {
                ob.add_item(t);
            }
            ob.set_tooltip_text("Triangles: keep every triangle; Quads: coplanar triangle pairs become quads (Modeler \"Detect quads\"); N-gons: every flat region becomes one face");
            row.add_child(&ob);
            col.add_child(&row);
            let mut checks = Vec::new();
            for (text, tip) in [
                (
                    "Keep materials",
                    "One material slot per surface of the source (off: everything on slot 0)",
                ),
                (
                    "Keep smoothing",
                    "Smoothing groups from the source's normals (smooth where the source is smooth)",
                ),
            ] {
                let mut cb = CheckBox::new_alloc();
                cb.set_text(text);
                cb.set_tooltip_text(tip);
                col.add_child(&cb);
                checks.push(cb);
            }
            let mut row = HBoxContainer::new_alloc();
            let mut l = Label::new_alloc();
            l.set_text("Smoothing angle°");
            row.add_child(&l);
            let mut sb = SpinBox::new_alloc();
            sb.set_min(0.0);
            sb.set_max(180.0);
            sb.set_step(1.0);
            sb.set_tooltip_text("Without Keep smoothing: neighbouring faces closer than this angle share a smoothing group (0 = all flat)");
            row.add_child(&sb);
            col.add_child(&row);
            dlg.add_child(&col);
            dlg.connect("confirmed", &self.to_gd().callable("on_convert_confirmed"));
            add_to_editor(&dlg);
            self.xg_dialogs.transmute = Some(dlg);
            self.xg_dialogs.convert_merge = Some(ob);
            self.xg_dialogs.convert_checks = checks;
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
            // The alternate layout names it after its toolbar button.
            d.set_title(if modeler {
                "Make Editable"
            } else {
                "Convert to builder mesh"
            });
            d.set_ok_button_text(if modeler { "Make Editable" } else { "Convert" });
            d.popup_centered();
        }
    }

    /// Reads the convert dialog into `convert_opts` and converts.
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

    /// The editable mesh + slot materials of the convert source.
    fn convert_source(
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
        // Editable meshes: the editable source stored with the mesh. Metadata that does not
        // decode falls back to the triangle convert below, with a note.
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

    /// Replaces the convert source by an editable builder mesh (the original is hidden, one undo).
    pub(super) fn convert_with(&mut self, opts: ImportOptions) {
        let Some(src) = self.convert_src.clone() else {
            self.message = "Convert: select a MeshInstance3D or CSG shape".into();
            return;
        };
        let (m, mats, note) = match self.convert_source(&src, &opts) {
            Ok(r) => r,
            Err(e) => {
                self.message = format!("Convert: {e}");
                self.base().update_overlays();
                return;
            }
        };
        if m.facets.is_empty() {
            self.message = "Convert: no triangles found".into();
            return;
        }
        if let Err(e) = m.sanity_check() {
            self.message = format!("Convert failed: {e}");
            return;
        }
        let (Some(parent), Some(root)) = (
            src.get_parent(),
            EditorInterface::singleton().get_edited_scene_root(),
        ) else {
            return;
        };
        let mut node = PixelBuilderMesh::new_alloc();
        let mut data = PixelBuilderMeshData::new_gd();
        data.bind_mut().substitute(m);
        data.set("materials", &mats.to_variant());
        node.bind_mut().set_data(Some(data));
        node.set_name(&format!("{}Builder", src.get_name()));
        node.set_transform(src.get_transform());
        let Some(mut ur) = self.base().get_undo_redo() else {
            return;
        };
        ur.create_action("Pixel Builder: Convert to builder mesh");
        ur.add_do_method(
            &parent,
            "add_child",
            &[node.to_variant(), true.to_variant()],
        );
        ur.add_do_method(
            &parent,
            "move_child",
            &[node.to_variant(), (src.get_index() + 1).to_variant()],
        );
        ur.add_do_method(&node, "set_owner", &[root.to_variant()]);
        ur.add_do_property(&src, "visible", &false.to_variant());
        ur.add_do_reference(&node);
        ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
        ur.add_undo_property(&src, "visible", &src.is_visible().to_variant());
        ur.commit_action();
        self.message = format!("Converted {} (original hidden; {note})", src.get_name());
        // Deferred: selecting the new node re-enters this plugin (`handles` / `edit`).
        EditorInterface::singleton().call_deferred("edit_node", &[node.to_variant()]);
    }
}

/// The Convert options that reproduce the builder's classic Convert (n-gons, slots kept, flat).
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

    /// Test / scripting hook: export options by name (`selected_only`, `apply_transforms`,
    /// `vertex_colors`, `right_handed`, `combine`, `stl_binary`); missing keys keep their value.
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

    /// Test / scripting hook: Convert the selected MeshInstance3D / CSG shape with options
    /// (merge 0 triangles / 1 quads / 2 n-gons), without the dialog.
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

/// Highest slot count an imported mesh keeps its slot numbers for; past it the used
/// slots are renumbered densely (a damaged `submeshIndex` up to 65535 would otherwise make a
/// material array that long).
pub const MAX_IMPORT_SLOTS: usize = 256;

/// Renumbers the face slots to the used ones, in order, when the slot count exceeds
/// [`MAX_IMPORT_SLOTS`]; `surface_of_slot` follows. True when renumbered.
pub fn compact_slots(
    m: &mut pt_core::build::BMesh,
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

/// Bakes `xf` into one surface's positions / normals / tangents: normals by the inverse
/// transpose (a non-uniform scale keeps them perpendicular), a mirroring transform flips the
/// tangent sign.
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
