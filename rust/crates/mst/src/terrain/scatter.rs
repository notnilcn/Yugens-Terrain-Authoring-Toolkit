//! Places painted props, such as rocks, plants and trees, on a terrain and draws them in
//! batches.
//!
//! The drawing and colliding nodes are created in code because which patches of ground
//! hold which props comes from what the author paints, so their number is only known at
//! runtime. They are internal children of the terrain, rebuilt from its data, never stored
//! in the scene, and freed when their patch empties. A prop that comes from a scene is
//! instantiated only long enough to read its meshes and then freed, because that scene is
//! chosen by the author as data.
use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::math_conversions::to_iv;
use crate::internal_children::{add_internal, free_node};
use crate::object_flags::object_flags_for;
use crate::shader_materials::shader_material;
use godot::classes::base_material_3d::{Flags as BmFlags, TextureParam, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    ArrayMesh, BaseMaterial3D, BoxShape3D, CylinderShape3D, Material, Mesh, MeshInstance3D,
    MultiMesh, MultiMeshInstance3D, Node3D, PackedScene, ShaderMaterial, Shape3D, StaticBody3D,
};
use godot::prelude::*;
use pt_core::brush::Brush;
use pt_core::math::{IVec2, Vec2 as PVec2, Vec3 as PVec3, v2, v3};
use pt_core::scatter::{
    PropMesh, PropShape, ScatterInstance, ScatterRules, ScatterSet, TileSet, instance_transform,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const CHUNK_SCATTER_SNAP: f32 = 256.0;
pub const SCATTER_PRESET_DIR: &str = "res://addons/MarchingSquaresTerrain/scatter_presets/";
const FALLBACK_SCATTER_PRESET: &str = "rock";
pub fn scatter_preset_kinds() -> Vec<(String, i32)> {
    let mut out: Vec<(String, i32)> = godot::classes::ResourceLoader::singleton()
        .list_directory(SCATTER_PRESET_DIR)
        .as_slice()
        .iter()
        .filter_map(|f| {
            let f = f.to_string();
            let stem = f.strip_suffix(".tres")?;
            let preset = try_load::<PixelScatterItem>(&format!("{SCATTER_PRESET_DIR}{f}")).ok()?;
            let form = preset.bind().form;
            Some((kind_label(stem), form))
        })
        .collect();
    out.sort();
    out
}
fn kind_label(stem: &str) -> String {
    let mut c = stem.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}
fn load_scatter_preset(kind: &str) -> Option<Gd<PixelScatterItem>> {
    let path = format!("{SCATTER_PRESET_DIR}{kind}.tres");
    if kind.is_empty() || !godot::classes::ResourceLoader::singleton().exists(&path) {
        return None;
    }
    let preset = try_load::<PixelScatterItem>(&path).ok()?;
    Some(preset.duplicate_resource())
}

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelScatterItem {
    #[export]
    pub display_name: GString,
    #[export(enum = (Custom, Rock, Bush, Tree, Pine, Flowers))]
    pub form: i32,
    #[export]
    pub shape_seed: i32,
    #[export]
    pub cage: Option<Gd<Mesh>>,
    #[export]
    pub scene: Option<Gd<PackedScene>>,
    #[export]
    pub tint: Color,
    #[export_group(name = "Placement")]
    #[export(range = (0.0015, 4.0, 0.0015, or_greater))]
    pub density: f32,
    #[export(range = (0.0505, 8.0, 0.0105, or_greater))]
    pub scale_min: f32,
    #[export(range = (0.0505, 8.0, 0.0105, or_greater))]
    pub scale_max: f32,
    #[export(range = (0.0, 360.0, 1.0))]
    pub random_yaw: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    pub align_to_normal: f32,
    #[export]
    pub y_offset: f32,
    #[export(range = (1.0, 16.0, 1.0))]
    pub per_cell_max: i32,
    #[export]
    pub small_hexes: bool,
    #[export(flags = (Texture1, Texture2, Texture3, Texture4, Texture5, Texture6, Texture7, Texture8, Texture9, Texture10, Texture11, Texture12, Texture13, Texture14, Texture15, Texture16))]
    pub ground_slots: i32,
    #[export]
    pub seed: i32,
    #[export_group(name = "Filters")]
    #[export(range = (0.0, 90.0, 0.5))]
    pub slope_min: f32,
    #[export(range = (0.0, 90.0, 0.5))]
    pub slope_max: f32,
    #[export]
    pub height_min: f32,
    #[export]
    pub height_max: f32,
    #[export(range = (0.0, 256.0, 0.5, or_greater))]
    pub cluster_scale: f32,
    #[export(range = (-1.0, 1.0, 0.0105))]
    pub cluster_threshold: f32,
    #[export_group(name = "Color")]
    #[export(range = (0.0, 0.5, 0.0055))]
    pub hue_jitter: f32,
    #[export(range = (0.0, 0.5, 0.0105))]
    pub value_jitter: f32,
    #[export_group(name = "Pixel")]
    #[export(range = (1.0, 16.0, 1.0, or_greater))]
    pub pixel_size: i32,
    #[export]
    pub outline: bool,
    #[export(range = (0.0, 7.0, 1.0))]
    pub outline_id: i32,
    #[export]
    pub cast_shadows: bool,
    #[export(range = (0.0, 1.0, 0.0105))]
    pub wind_sway: f32,
    #[export_group(name = "Collision")]
    #[export(enum = (None, Cylinder, Box))]
    pub collision: i32,
    #[export]
    pub collision_size: Vector3,
    #[export(flags_3d_physics)]
    pub impact_stratum: u32,
    #[export(range = (0.0, 8.0, 0.0105, or_greater))]
    pub collision_min_scale: f32,
    base: Base<Resource>,
}
#[godot_api]
impl IResource for PixelScatterItem {
    fn init(base: Base<Resource>) -> Self {
        let r = ScatterRules::default();
        PixelScatterItem {
            display_name: GString::new(),
            form: 1,
            shape_seed: 1,
            cage: None,
            scene: None,
            tint: Color::WHITE,
            density: r.density,
            scale_min: r.scale_min,
            scale_max: r.scale_max,
            random_yaw: r.yaw_jitter,
            align_to_normal: 0.0,
            y_offset: 0.0,
            per_cell_max: 1,
            small_hexes: false,
            ground_slots: 0,
            seed: 1,
            slope_min: r.slope_min,
            slope_max: r.slope_max,
            height_min: -1000.0,
            height_max: 1000.0,
            cluster_scale: 0.0,
            cluster_threshold: 0.0,
            hue_jitter: r.hue_jitter,
            value_jitter: r.value_jitter,
            pixel_size: 1,
            outline: true,
            outline_id: 0,
            cast_shadows: true,
            wind_sway: 0.0,
            collision: 0,
            collision_size: Vector3::ONE,
            impact_stratum: 1,
            collision_min_scale: 0.0,
            base,
        }
    }
}
#[godot_api]
impl PixelScatterItem {
    #[func]
    pub fn create_builtin(kind: GString) -> Gd<PixelScatterItem> {
        let k = kind.to_string().to_lowercase();
        if let Some(p) = load_scatter_preset(&k) {
            return p;
        }
        let mut g = load_scatter_preset(FALLBACK_SCATTER_PRESET).unwrap_or_else(|| {
            godot_warn!(
                "PixelScatterItem: no scatter preset found in {SCATTER_PRESET_DIR}; using a plain item"
            );
            PixelScatterItem::new_gd()
        });
        if !k.is_empty() {
            g.bind_mut().display_name = GString::from(kind_label(&k).as_str());
        }
        g
    }
    #[func]
    pub fn get_label(&self) -> GString {
        if !self.display_name.is_empty() {
            return self.display_name.clone();
        }
        if let Some(m) = &self.cage {
            let p = m.get_path().to_string();
            if !p.is_empty() {
                return p.rsplit('/').next().unwrap_or("Mesh").into();
            }
            return "Mesh".into();
        }
        if let Some(s) = &self.scene {
            return s
                .get_path()
                .to_string()
                .rsplit('/')
                .next()
                .unwrap_or("Scene")
                .into();
        }
        pt_core::scatter::PROP_SHAPES
            .get(self.form.clamp(0, 5) as usize)
            .copied()
            .unwrap_or("Item")
            .into()
    }
}
impl PixelScatterItem {
    pub fn preview_resource(&self) -> Option<Gd<godot::classes::Resource>> {
        if let Some(s) = &self.scene {
            let root = s.instantiate()?;
            let mut mesh = ArrayMesh::new_gd();
            scene_preview(&root, Transform3D::IDENTITY, true, &mut mesh);
            root.free();
            return (mesh.get_surface_count() > 0).then(|| mesh.upcast());
        }
        if let Some(m) = &self.cage {
            return Some(m.clone().upcast());
        }
        let shape = PropShape::from_i64(self.form as i64)?;
        let mut mesh = prop_mesh(shape, self.shape_seed as u32)?
            .try_cast::<ArrayMesh>()
            .ok()?;
        let mut mat = godot::classes::StandardMaterial3D::new_gd();
        mat.set_flag(BmFlags::ALBEDO_FROM_VERTEX_COLOR, true);
        mat.set_albedo(self.tint);
        mesh.surface_set_material(0, &mat);
        Some(mesh.upcast())
    }
    pub fn rules(&self) -> ScatterRules {
        ScatterRules {
            density: self.density.max(1e-4),
            scale_min: self.scale_min,
            scale_max: self.scale_max,
            yaw_jitter: self.random_yaw,
            hue_jitter: self.hue_jitter,
            value_jitter: self.value_jitter,
            slope_min: self.slope_min,
            slope_max: self.slope_max,
            height_min: self.height_min,
            height_max: self.height_max,
            cluster_scale: self.cluster_scale,
            cluster_threshold: self.cluster_threshold,
            per_cell_max: self.per_cell_max.clamp(1, 16) as u32,
            slot_mask: self.ground_slots as u32 & 0xffff,
            seed: self.seed as u32,
            small_hex: self.small_hexes,
        }
    }
    fn signature(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.form.hash(h);
        self.shape_seed.hash(h);
        self.cage.as_ref().map(|m| m.instance_id().to_i64()).hash(h);
        self.scene
            .as_ref()
            .map(|m| m.instance_id().to_i64())
            .hash(h);
        for f in [
            self.tint.r,
            self.tint.g,
            self.tint.b,
            self.tint.a,
            self.align_to_normal,
            self.y_offset,
            self.wind_sway,
        ] {
            f.to_bits().hash(h);
        }
        (
            self.pixel_size,
            self.outline,
            self.outline_id,
            self.cast_shadows,
        )
            .hash(h);
        (self.collision, self.impact_stratum).hash(h);
        for f in [
            self.collision_size.x,
            self.collision_size.y,
            self.collision_size.z,
            self.collision_min_scale,
        ] {
            f.to_bits().hash(h);
        }
    }
}
struct Part {
    cage: Gd<Mesh>,
    xform: Transform3D,
    stuff: Gd<Material>,
}
fn prop_mesh(shape: PropShape, seed: u32) -> Option<Gd<Mesh>> {
    let m = PropMesh::assemble(shape, seed);
    if m.positions.is_empty() {
        return None;
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let pos: PackedVector3Array = m
        .positions
        .iter()
        .map(|p| Vector3::new(p.x, p.y, p.z))
        .collect();
    let nrm: PackedVector3Array = m
        .normals
        .iter()
        .map(|p| Vector3::new(p.x, p.y, p.z))
        .collect();
    let col: PackedColorArray = m
        .tints
        .iter()
        .map(|c| Color::from_rgba(c[0], c[1], c[2], c[3]))
        .collect();
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    Some(mesh.upcast())
}
fn part_material(
    item: &PixelScatterItem,
    src: Option<Gd<Material>>,
    builtin: bool,
) -> Gd<Material> {
    if let Some(m) = &src
        && m.clone().try_cast::<ShaderMaterial>().is_ok()
    {
        return m.clone();
    }
    let mut mat = shader_material("mst_scatter.gdshader");
    let mut albedo = item.tint;
    let mut vertex_color = builtin;
    if let Some(b) = src.and_then(|m| m.try_cast::<BaseMaterial3D>().ok()) {
        let a = b.get_albedo();
        albedo = Color::from_rgba(
            albedo.r * a.r,
            albedo.g * a.g,
            albedo.b * a.b,
            albedo.a * a.a,
        );
        vertex_color = b.get_flag(BmFlags::ALBEDO_FROM_VERTEX_COLOR);
        if let Some(t) = b.get_texture(TextureParam::ALBEDO) {
            mat.set_shader_parameter("albedo_texture", &t.to_variant());
            mat.set_shader_parameter("use_albedo_texture", &true.to_variant());
        }
        let tr = b.get_transparency();
        if tr == Transparency::ALPHA_SCISSOR
            || tr == Transparency::ALPHA
            || tr == Transparency::ALPHA_HASH
        {
            mat.set_shader_parameter("use_alpha_scissor", &true.to_variant());
            mat.set_shader_parameter(
                "alpha_scissor_threshold",
                &b.get_alpha_scissor_threshold().to_variant(),
            );
        }
    }
    mat.set_shader_parameter("albedo_color", &albedo.to_variant());
    mat.set_shader_parameter("use_vertex_color", &vertex_color.to_variant());
    mat.set_shader_parameter("wind_sway", &item.wind_sway.to_variant());
    mat.set_shader_parameter(
        "pt_flags",
        &object_flags_for(
            item.base().instance_id(),
            0,
            item.pixel_size,
            item.outline,
            item.outline_id,
            false,
        )
        .to_variant(),
    );
    mat.upcast()
}
fn split_surfaces(
    item: &PixelScatterItem,
    mesh: &Gd<Mesh>,
    xform: Transform3D,
    overrides: &dyn Fn(i32) -> Option<Gd<Material>>,
    out: &mut Vec<Part>,
) {
    for s in 0..mesh.get_surface_count() {
        let arrays = mesh.surface_get_arrays(s);
        let mut single = ArrayMesh::new_gd();
        let prim = mesh
            .clone()
            .try_cast::<ArrayMesh>()
            .map(|a| a.surface_get_primitive_type(s))
            .unwrap_or(PrimitiveType::TRIANGLES);
        single.add_surface_from_arrays(prim, &arrays);
        let src = overrides(s).or_else(|| mesh.surface_get_material(s));
        out.push(Part {
            cage: single.upcast(),
            xform,
            stuff: part_material(item, src, false),
        });
    }
}
fn scene_preview(node: &Gd<Node>, xform: Transform3D, is_root: bool, out: &mut Gd<ArrayMesh>) {
    let xform = match node.clone().try_cast::<Node3D>() {
        Ok(n3) if !is_root => xform * n3.get_transform(),
        _ => xform,
    };
    if let Ok(mi) = node.clone().try_cast::<MeshInstance3D>()
        && let Some(mesh) = mi.get_mesh()
    {
        for si in 0..mesh.get_surface_count() {
            let mut arrays = mesh.surface_get_arrays(si);
            let Ok(verts) = arrays
                .at(ArrayType::VERTEX.ord() as usize)
                .try_to::<PackedVector3Array>()
            else {
                continue;
            };
            let verts: PackedVector3Array = verts.as_slice().iter().map(|v| xform * *v).collect();
            arrays.set(ArrayType::VERTEX.ord() as usize, &verts.to_variant());
            if let Ok(n) = arrays
                .at(ArrayType::NORMAL.ord() as usize)
                .try_to::<PackedVector3Array>()
            {
                let n: PackedVector3Array = n
                    .as_slice()
                    .iter()
                    .map(|v| (xform.basis * *v).normalized())
                    .collect();
                arrays.set(ArrayType::NORMAL.ord() as usize, &n.to_variant());
            }
            for a in [ArrayType::TANGENT, ArrayType::BONES, ArrayType::WEIGHTS] {
                arrays.set(a.ord() as usize, &Variant::nil());
            }
            let mat = mi
                .get_material_override()
                .or_else(|| mi.get_surface_override_material(si))
                .or_else(|| mesh.surface_get_material(si));
            out.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            if let Some(m) = mat {
                let last = out.get_surface_count() - 1;
                out.surface_set_material(last, &m);
            }
        }
    }
    for c in node.get_children().iter_shared() {
        scene_preview(&c, xform, false, out);
    }
}
fn collect_scene(
    item: &PixelScatterItem,
    node: &Gd<Node>,
    xform: Transform3D,
    out: &mut Vec<Part>,
) {
    let xform = match node.clone().try_cast::<Node3D>() {
        Ok(n3) => xform * n3.get_transform(),
        Err(_) => xform,
    };
    if let Ok(mi) = node.clone().try_cast::<MeshInstance3D>()
        && let Some(mesh) = mi.get_mesh()
    {
        let ovr = mi.get_material_override();
        let mi2 = mi.clone();
        let f = move |s: i32| ovr.clone().or_else(|| mi2.get_surface_override_material(s));
        split_surfaces(item, &mesh, xform, &f, out);
    }
    for c in node.get_children().iter_shared() {
        collect_scene(item, &c, xform, out);
    }
}
fn build_parts(item: &PixelScatterItem) -> Vec<Part> {
    let mut out = Vec::new();
    if let Some(mesh) = &item.cage {
        split_surfaces(item, mesh, Transform3D::IDENTITY, &|_| None, &mut out);
    } else if let Some(scene) = &item.scene {
        if let Some(root) = scene.instantiate() {
            for c in root.get_children().iter_shared() {
                collect_scene(item, &c, Transform3D::IDENTITY, &mut out);
            }
            if let Ok(mi) = root.clone().try_cast::<MeshInstance3D>()
                && let Some(mesh) = mi.get_mesh()
            {
                split_surfaces(item, &mesh, Transform3D::IDENTITY, &|_| None, &mut out);
            }
            root.free();
        }
    } else if let Some(shape) = PropShape::from_i64(item.form as i64)
        && let Some(mesh) = prop_mesh(shape, item.shape_seed as u32)
    {
        out.push(Part {
            cage: mesh,
            xform: Transform3D::IDENTITY,
            stuff: part_material(item, None, true),
        });
    }
    out
}
fn footprint_radius(parts: &[Part]) -> f32 {
    let mut r: f32 = 0.1505;
    for p in parts {
        let a = p.xform * p.cage.get_aabb();
        for x in [a.position.x, a.position.x + a.size.x] {
            for z in [a.position.z, a.position.z + a.size.z] {
                r = r.max(Vector2::new(x, z).length() * std::f32::consts::FRAC_1_SQRT_2);
            }
        }
    }
    r
}
#[derive(Clone, Copy)]
struct Collider {
    tube: bool,
    centre: Vector3,
    size: Vector3,
    layer: u32,
    min_scale: f32,
}
fn build_collider(item: &PixelScatterItem, parts: &[Part]) -> Option<Collider> {
    if item.collision <= 0 || item.impact_stratum == 0 {
        return None;
    }
    let (mut lo, mut hi) = (Vector3::splat(f32::MAX), Vector3::splat(f32::MIN));
    for p in parts {
        let b = p.cage.get_aabb();
        for i in 0..8 {
            let c = b.position
                + Vector3::new(
                    if i & 1 != 0 { b.size.x } else { 0.0 },
                    if i & 2 != 0 { b.size.y } else { 0.0 },
                    if i & 4 != 0 { b.size.z } else { 0.0 },
                );
            let w = p.xform * c;
            lo = lo.coord_min(w);
            hi = hi.coord_max(w);
        }
    }
    if lo.x > hi.x {
        return None;
    }
    let ext = hi - lo;
    let k = item.collision_size.abs();
    let size = Vector3::new(ext.x * k.x, ext.y * k.y, ext.z * k.z);
    if size.y <= 1e-4 || size.x.max(size.z) <= 1e-4 {
        return None;
    }
    let centre = Vector3::new(
        (lo.x + hi.x) * 0.5,
        lo.y + size.y * 0.5,
        (lo.z + hi.z) * 0.5,
    );
    Some(Collider {
        tube: item.collision == 1,
        centre,
        size,
        layer: item.impact_stratum,
        min_scale: item.collision_min_scale,
    })
}
fn impact_physbody(
    c: &Collider,
    xforms: &[(Transform3D, Color)],
    name: &str,
) -> Option<Gd<StaticBody3D>> {
    let mut body: Option<Gd<StaticBody3D>> = None;
    for (xf, _) in xforms {
        let k = xf.basis.col_b().length();
        if k < c.min_scale.max(1e-4) {
            continue;
        }
        let shape: Gd<Shape3D> = if c.tube {
            let mut cy = CylinderShape3D::new_gd();
            cy.set_radius(0.5 * c.size.x.max(c.size.z) * k);
            cy.set_height(c.size.y * k);
            cy.upcast()
        } else {
            let mut bx = BoxShape3D::new_gd();
            bx.set_size(c.size * k);
            bx.upcast()
        };
        let basis = Basis::from_cols(
            xf.basis.col_a() / k,
            xf.basis.col_b() / k,
            xf.basis.col_c() / k,
        );
        let b = body.get_or_insert_with(|| {
            let mut b = StaticBody3D::new_alloc();
            b.set_name(name);
            b.set_collision_layer(c.layer);
            b.set_collision_mask(0);
            b
        });
        let owner = b.clone().upcast::<Object>();
        let id = b.create_shape_owner(&owner) as u32;
        b.shape_owner_add_shape(id, &shape);
        b.shape_owner_set_transform(id, Transform3D::new(basis, *xf * c.centre));
    }
    body
}
struct ItemCache {
    parts: Vec<Part>,
    align: f32,
    y_offset: f32,
    shadows: bool,
    collider: Option<Collider>,
}
fn group_of(tile: IVec2, group_tiles: i32) -> IVec2 {
    let n = group_tiles.max(1);
    pt_core::math::iv2(tile.x.div_euclid(n), tile.y.div_euclid(n))
}
#[derive(Default)]
struct TileNodes {
    parts: BTreeMap<(u16, usize), Vec<f32>>,
    bodies: HashMap<u16, (u64, Gd<StaticBody3D>)>,
}
impl TileNodes {
    fn free(self) {
        for (_, (_, b)) in self.bodies {
            free_node(Some(b));
        }
    }
}
const PROP_STRIDE: usize = 20;
fn hash_xforms(xforms: &[(Transform3D, Color)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (t, _) in xforms {
        for v in [t.basis.col_a(), t.basis.col_b(), t.basis.col_c(), t.origin] {
            [v.x, v.y, v.z].map(f32::to_bits).hash(&mut h);
        }
    }
    h.finish()
}
#[derive(Default)]
pub struct ScatterLayer {
    pub assign: ScatterSet,
    base: Option<Gd<Node3D>>,
    nodes: HashMap<IVec2, TileNodes>,
    batches: BTreeMap<(IVec2, u16, usize), Gd<MultiMeshInstance3D>>,
    dirty: BTreeSet<IVec2>,
    all_dirty: bool,
    stale_bodies: BTreeSet<IVec2>,
    last_flush_ms: u64,
    follow: BTreeSet<IVec2>,
    last_follow_ms: u64,
    items: Vec<Option<ItemCache>>,
    signature: u64,
    item_ids: Option<Vec<i64>>,
}
impl ScatterLayer {
    pub fn mark_tiles(&mut self, t: &TileSet) {
        self.dirty.extend(t.iter().copied());
    }
    pub fn mark_all(&mut self) {
        self.all_dirty = true;
    }
}
impl MarchingSquaresTerrain {
    pub(super) fn regroup_scatter(&mut self, group_tiles: i32) {
        let v = group_tiles.clamp(1, 16);
        if v == self.scatter_group_tiles {
            return;
        }
        self.scatter_group_tiles = v;
        free_node(self.scatter.base.take());
        self.scatter.mark_all();
    }
    fn current_item_ids(&self) -> Vec<i64> {
        self.scatter_items
            .iter_shared()
            .map(|it| it.map(|i| i.instance_id().to_i64()).unwrap_or(0))
            .collect()
    }
    fn scatter_sync_item_ids(&mut self) {
        let now = self.current_item_ids();
        let Some(old) = self.scatter.item_ids.replace(now.clone()) else {
            return;
        };
        if old == now || self.scatter.assign.check_vacant() {
            return;
        }
        let mut used = vec![false; now.len()];
        let map: Vec<Option<u16>> = old
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let j = if *id == 0 {
                    (now.get(i) == Some(&0)).then_some(i)
                } else {
                    (0..now.len()).find(|&j| now[j] == *id && !used[j])
                };
                j.map(|j| {
                    used[j] = true;
                    j as u16
                })
            })
            .collect();
        if map.iter().enumerate().all(|(i, m)| *m == Some(i as u16)) {
            return;
        }
        self.scatter.assign.remap_items(&map);
        self.scatter.mark_all();
        self.scatter_write();
    }
    fn items_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.scatter_items.len().hash(&mut h);
        for it in self.scatter_items.iter_shared() {
            match it {
                Some(i) => {
                    i.instance_id().to_i64().hash(&mut h);
                    i.bind().signature(&mut h);
                }
                None => 0u8.hash(&mut h),
            }
        }
        (self.render_layers, globals::meta_mask()).hash(&mut h);
        h.finish()
    }
    pub(super) fn scatter_item(&self, i: usize) -> Option<Gd<PixelScatterItem>> {
        self.scatter_items.get(i).flatten()
    }
    pub fn surface_at(&self, p: PVec2) -> Option<(f32, PVec3, u8)> {
        let cfg = &self.store().config;
        let s = cfg.world_to_sample(p);
        let slot = self.store().probe(s).map(|sm| sm.floor).unwrap_or(0);
        if slot == 15 {
            return None;
        }
        let h = self.local_height_at(p)?;
        let e = (cfg.cell_size_safe().x * 0.2005).max(0.0505);
        let hx = |dx: f32, dz: f32| self.local_height_at(p + v2(dx, dz)).unwrap_or(h);
        let gx = (hx(e, 0.0) - hx(-e, 0.0)) / (2.0 * e);
        let gz = (hx(0.0, e) - hx(0.0, -e)) / (2.0 * e);
        Some((h, v3(-gx, 1.0, -gz).unitized(), slot))
    }
    pub(super) fn scatter_reload(&mut self) {
        let grid = self.grid();
        let set = self
            .contents
            .as_ref()
            .and_then(|d| d.bind().scatter_bytes(grid))
            .and_then(|b| ScatterSet::from_bytes(&b))
            .unwrap_or_default();
        self.scatter.assign = set;
        self.scatter.all_dirty = true;
        self.scatter.item_ids = Some(self.current_item_ids());
    }
    pub fn scatter_write(&mut self) {
        let grid = self.grid();
        let bytes = self.scatter.assign.to_bytes();
        if let Some(d) = &mut self.contents {
            d.bind_mut().write_scatter(
                grid,
                if self.scatter.assign.check_vacant() {
                    None
                } else {
                    Some(&bytes)
                },
            );
        }
    }
    pub(super) fn scatter_mark_chunks(&mut self, chunks: &[IVec2]) {
        if self.scatter.assign.check_vacant() {
            return;
        }
        let cfg = self.store().config.clone();
        for c in chunks {
            let o = cfg.chunk_outline(*c);
            if o.is_empty() {
                continue;
            }
            let (mut min, mut max) = (o[0], o[0]);
            for p in &o {
                min = v2(min.x.min(p.x), min.y.min(p.y));
                max = v2(max.x.max(p.x), max.y.max(p.y));
            }
            let pad = cfg.cell_size_safe() * 1.5005;
            let editor = is_editor();
            for t in self.scatter.assign.tiles_in_rect(min - pad, max + pad) {
                if self.scatter.assign.tiles.contains_key(&t) || self.scatter.nodes.contains_key(&t)
                {
                    if editor {
                        self.scatter.follow.insert(t);
                    } else {
                        self.scatter.dirty.insert(t);
                    }
                }
            }
        }
    }
    fn ensure_scatter_root(&mut self) -> Option<Gd<Node3D>> {
        if let Some(r) = &self.scatter.base
            && r.is_instance_valid()
        {
            return Some(r.clone());
        }
        let mut root = Node3D::new_alloc();
        root.set_name("Scatter");
        let mut this = self.to_gd();
        add_internal(&mut this, &root);
        self.scatter.base = Some(root.clone());
        self.scatter.nodes.clear();
        self.scatter.batches.clear();
        self.scatter.all_dirty = true;
        Some(root)
    }
    pub(super) fn scatter_flush(&mut self) {
        let group_tiles = self.scatter_group_tiles;
        let sig = self.items_signature();
        if sig != self.scatter.signature {
            self.scatter.signature = sig;
            self.scatter_sync_item_ids();
            self.scatter.items = (0..self.scatter_items.len())
                .map(|i| {
                    self.scatter_item(i).map(|it| {
                        let b = it.bind();
                        let parts = build_parts(&b);
                        let collider = build_collider(&b, &parts);
                        ItemCache {
                            parts,
                            align: b.align_to_normal,
                            y_offset: b.y_offset,
                            shadows: b.cast_shadows,
                            collider,
                        }
                    })
                })
                .collect();
            self.scatter.all_dirty = true;
        }
        let body_quiet_ms = self.scatter_body_quiet_ms.max(0) as u64;
        let follow_interval_ms = self.scatter_follow_interval_ms.max(0) as u64;
        let now = godot::classes::Time::singleton().get_ticks_msec();
        if !self.scatter.follow.is_empty()
            && now.saturating_sub(self.scatter.last_follow_ms) >= follow_interval_ms
        {
            self.scatter.last_follow_ms = now;
            let follow = std::mem::take(&mut self.scatter.follow);
            self.scatter.dirty.extend(follow);
        }
        let quiet = now.saturating_sub(self.scatter.last_flush_ms) >= body_quiet_ms;
        if !self.scatter.stale_bodies.is_empty()
            && quiet
            && self.scatter.dirty.is_empty()
            && self.scatter.follow.is_empty()
            && !self.scatter.all_dirty
        {
            let stale = std::mem::take(&mut self.scatter.stale_bodies);
            self.scatter.dirty.extend(stale);
        }
        if !self.scatter.all_dirty && self.scatter.dirty.is_empty() {
            return;
        }
        let defer_bodies = is_editor() && !quiet;
        self.scatter.last_flush_ms = now;
        let Some(mut root) = self.ensure_scatter_root() else {
            return;
        };
        let tiles: Vec<IVec2> = if std::mem::take(&mut self.scatter.all_dirty) {
            self.scatter.dirty.clear();
            self.scatter
                .nodes
                .keys()
                .chain(self.scatter.assign.tiles.keys())
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            std::mem::take(&mut self.scatter.dirty)
                .into_iter()
                .collect()
        };
        let mut groups = BTreeSet::new();
        for t in tiles {
            let mut old = self.scatter.nodes.remove(&t).unwrap_or_default();
            groups.insert(group_of(t, group_tiles));
            let Some(list) = self.scatter.assign.tiles.get(&t) else {
                old.free();
                continue;
            };
            let mut by_item: HashMap<u16, Vec<(Transform3D, Color, bool)>> = HashMap::new();
            for inst in list {
                let drawn = self.lod_low.is_empty()
                    || !self
                        .lod_low
                        .contains(&self.store().config.chunk_at(inst.pos));
                let Some(Some(cache)) = self.scatter.items.get(inst.item as usize) else {
                    continue;
                };
                let Some((h, n, _)) = self.surface_at(inst.pos) else {
                    continue;
                };
                if !self.grass_water.check_vacant()
                    && self
                        .grass_water
                        .hides_prop(&self.store().config, v3(inst.pos.x, h, inst.pos.y))
                {
                    continue;
                }
                let ([x, y, z], o) = instance_transform(inst, h, n, cache.align, cache.y_offset);
                let basis = Basis::from_cols(
                    Vector3::new(x.x, x.y, x.z),
                    Vector3::new(y.x, y.y, y.z),
                    Vector3::new(z.x, z.y, z.z),
                );
                let xf = Transform3D::new(basis, Vector3::new(o.x, o.y, o.z));
                by_item.entry(inst.item).or_default().push((
                    xf,
                    Color::from_rgba(inst.hue, inst.worth, 0.0, 0.0),
                    drawn,
                ));
            }
            let mut tile = TileNodes::default();
            let mut items: Vec<_> = by_item.into_iter().collect();
            items.sort_by_key(|(k, _)| *k);
            for (item, all) in items {
                let Some(Some(cache)) = self.scatter.items.get(item as usize) else {
                    continue;
                };
                let drawn: Vec<(Transform3D, Color)> =
                    all.iter().filter(|e| e.2).map(|e| (e.0, e.1)).collect();
                let xforms: Vec<(Transform3D, Color)> = all.iter().map(|e| (e.0, e.1)).collect();
                for (pi, part) in cache.parts.iter().enumerate() {
                    if drawn.is_empty() {
                        break;
                    }
                    let mut buf = Vec::with_capacity(drawn.len() * PROP_STRIDE);
                    for (x, c) in &drawn {
                        crate::instance_buffer::push_instance(
                            &mut buf,
                            *x * part.xform,
                            Some(Color::WHITE),
                            Some(*c),
                        );
                    }
                    tile.parts.insert((item, pi), buf);
                }
                if let Some(c) = &cache.collider {
                    let hash = hash_xforms(&xforms);
                    match old.bodies.remove(&item) {
                        Some((h, b)) if h == hash && b.is_instance_valid() => {
                            tile.bodies.insert(item, (h, b));
                        }
                        Some((h, b)) if defer_bodies && b.is_instance_valid() => {
                            tile.bodies.insert(item, (h, b));
                            self.scatter.stale_bodies.insert(t);
                        }
                        prev => {
                            free_node(prev.map(|(_, b)| b));
                            if let Some(body) = impact_physbody(
                                c,
                                &xforms,
                                &format!("Tile_{}_{}_Item{item}_Collision", t.x, t.y),
                            ) {
                                root.add_child(&body);
                                tile.bodies.insert(item, (hash, body));
                            }
                        }
                    }
                }
            }
            old.free();
            groups.insert(group_of(t, group_tiles));
            if !tile.parts.is_empty() || !tile.bodies.is_empty() {
                self.scatter.nodes.insert(t, tile);
            }
        }
        self.scatter_upload_groups(groups, &mut root);
    }
    fn scatter_upload_groups(&mut self, groups: BTreeSet<IVec2>, root: &mut Gd<Node3D>) {
        let group_tiles = self.scatter_group_tiles;
        let layers = self.render_layers | globals::meta_mask();
        let mut tiles: Vec<IVec2> = self
            .scatter
            .nodes
            .keys()
            .copied()
            .filter(|t| groups.contains(&group_of(*t, group_tiles)))
            .collect();
        tiles.sort();
        let mut bufs: BTreeMap<(IVec2, u16, usize), Vec<f32>> = BTreeMap::new();
        for t in tiles {
            for ((item, pi), b) in &self.scatter.nodes[&t].parts {
                bufs.entry((group_of(t, group_tiles), *item, *pi))
                    .or_default()
                    .extend_from_slice(b);
            }
        }
        let stale: Vec<_> = self
            .scatter
            .batches
            .keys()
            .filter(|k| groups.contains(&k.0) && !bufs.contains_key(k))
            .copied()
            .collect();
        for k in stale {
            if let Some(mut n) = self.scatter.batches.remove(&k) {
                if n.is_instance_valid() {
                    root.remove_child(&n);
                    n.queue_free();
                }
            }
        }
        for ((g, item, pi), buf) in bufs {
            let Some(Some(cache)) = self.scatter.items.get(item as usize) else {
                continue;
            };
            let Some(part) = cache.parts.get(pi) else {
                continue;
            };
            let (mut mmi, mut mm) = match self
                .scatter
                .batches
                .get(&(g, item, pi))
                .filter(|n| n.is_instance_valid())
                .and_then(|n| n.get_multimesh().map(|m| (n.clone(), m)))
            {
                Some(pair) => pair,
                None => {
                    let mut mm = MultiMesh::new_gd();
                    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                    mm.set_use_colors(true);
                    mm.set_use_custom_data(true);
                    let mut mmi = MultiMeshInstance3D::new_alloc();
                    mmi.set_name(&format!("Group_{}_{}_Item{item}_Part{pi}", g.x, g.y));
                    mmi.set_multimesh(&mm);
                    root.add_child(&mmi);
                    self.scatter.batches.insert((g, item, pi), mmi.clone());
                    (mmi, mm)
                }
            };
            if mm.get_mesh().as_ref() != Some(&part.cage) {
                mm.set_mesh(&part.cage);
            }
            let count = (buf.len() / PROP_STRIDE) as i32;
            if mm.get_instance_count() != count {
                mm.set_instance_count(count);
            }
            mm.set_buffer(&PackedFloat32Array::from(buf.as_slice()));
            mmi.set_material_override(&part.stuff);
            mmi.set_layer_mask(layers);
            mmi.set_cast_shadows_setting(if cache.shadows {
                ShadowCastingSetting::ON
            } else {
                ShadowCastingSetting::OFF
            });
        }
    }
    pub fn scatter_brush(
        &mut self,
        item: Option<usize>,
        brush: &Brush,
        center: PVec2,
        erase: bool,
        per_cell: bool,
        slope: (f32, f32),
    ) -> bool {
        let t = if erase {
            let it = item.map(|i| i as u16);
            self.scatter.assign.erase(brush, center, it)
        } else {
            let Some(i) = item else { return false };
            let Some(res) = self.scatter_item(i) else {
                return false;
            };
            let mut rules = res.bind().rules();
            rules.slope_min = rules.slope_min.max(slope.0);
            rules.slope_max = rules.slope_max.min(slope.1);
            let mut set = std::mem::take(&mut self.scatter.assign);
            let surf = |p: PVec2| self.surface_at(p);
            let t = if per_cell {
                let cfg = self.store().config.clone();
                let cells: Vec<IVec2> = pt_core::brush::footprint(&cfg, brush, center)
                    .into_iter()
                    .filter(|(_, w)| *w >= 0.5)
                    .map(|(c, _)| c)
                    .collect();
                self.scatter_fill(&mut set, &rules, i as u16, &cfg, &cells)
            } else {
                set.coat(&rules, i as u16, brush, center, &surf)
            };
            self.scatter.assign = set;
            t
        };
        let changed = !t.is_empty();
        self.scatter.mark_tiles(&t);
        changed
    }
    pub fn scatter_hand_rules(
        &self,
        items: &[usize],
        slope: (f32, f32),
        stroke: u32,
    ) -> Vec<(u16, ScatterRules)> {
        items
            .iter()
            .filter_map(|&i| {
                let res = self.scatter_item(i)?;
                let b = res.bind();
                let mut r = b.rules().hand_painted(slope, stroke);
                let width = 2.0
                    * footprint_radius(&build_parts(&b))
                    * 0.5
                    * (r.scale_min + r.scale_max).max(0.0505);
                let spacing = (width * 1.1005).max(0.2005);
                r.density = 1.0 / (spacing * spacing);
                Some((i as u16, r))
            })
            .collect()
    }
    pub fn scatter_paint_items(
        &mut self,
        rules: &[(u16, ScatterRules)],
        brush: &Brush,
        center: PVec2,
        density: f32,
        at_least_one: bool,
    ) -> bool {
        if rules.is_empty() {
            return false;
        }
        let mut set = std::mem::take(&mut self.scatter.assign);
        let surf = |p: PVec2| {
            let s = self.surface_at(p)?;
            let hidden = !self.grass_water.check_vacant()
                && self
                    .grass_water
                    .hides_prop(&self.store().config, v3(p.x, s.0, p.y));
            (!hidden).then_some(s)
        };
        let t = set.paint_items(rules, brush, center, density, at_least_one, &surf);
        self.scatter.assign = set;
        self.scatter.mark_tiles(&t);
        !t.is_empty()
    }
    pub fn scatter_erase_items(
        &mut self,
        items: Option<&[usize]>,
        brush: &Brush,
        center: PVec2,
    ) -> bool {
        let ids: Option<Vec<u16>> = items.map(|v| v.iter().map(|i| *i as u16).collect());
        let t = self
            .scatter
            .assign
            .erase_items(brush, center, ids.as_deref());
        let changed = !t.is_empty();
        self.scatter.mark_tiles(&t);
        changed
    }
    fn scatter_fill(
        &self,
        set: &mut ScatterSet,
        rules: &ScatterRules,
        item: u16,
        cfg: &pt_core::grid::GridConfig,
        cells: &[IVec2],
    ) -> pt_core::scatter::TileSet {
        let surf = |p: PVec2| self.surface_at(p);
        if rules.small_hex
            && matches!(
                cfg.grid,
                pt_core::grid::GridType::Hex | pt_core::grid::GridType::HexRings
            )
        {
            let spots = |c: IVec2| -> Vec<(IVec2, PVec2)> {
                self.decoration_spots_local(c, rules.height_min.max(-1.0e6))
                    .into_iter()
                    .map(|(s, p)| (s, pt_core::math::v2(p.x, p.z)))
                    .collect()
            };
            set.fill_small_hexes(rules, item, cfg, cells, &spots, &surf)
        } else {
            set.fill_cells(rules, item, cfg, cells, &surf)
        }
    }
    pub fn scatter_remove_on_small_hexes(
        &mut self,
        lo: PVec2,
        hi: PVec2,
        blocked: &dyn Fn(IVec2) -> bool,
    ) {
        let cfg = self.store().config.clone();
        let t = self
            .scatter
            .assign
            .remove_on_small_hexes(&cfg, lo, hi, blocked);
        if !t.is_empty() {
            self.scatter.mark_tiles(&t);
            self.scatter_write();
        }
    }
    fn scatter_local(&self, world: Vector3) -> Vector3 {
        let xf = if self.base().is_inside_tree() {
            self.base().get_global_transform()
        } else {
            self.base().get_transform()
        };
        xf.affine_inverse() * world
    }
    pub fn scatter_bytes(&self) -> Vec<u8> {
        self.scatter.assign.to_bytes()
    }
}
#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    #[func]
    pub fn get_chunk_scatter_data(&self, chunk: Vector2i) -> PackedByteArray {
        let c = to_iv(chunk);
        let config = &self.store().config;
        let origin = config.sample_pos(config.chunk_center_cell(c));
        let mut mine: Vec<ScatterInstance> = self
            .scatter
            .assign
            .iter()
            .filter(|i| config.chunk_at(i.pos) == c)
            .map(|i| {
                let mut r = *i;
                r.pos = v2(
                    ((i.pos.x - origin.x) * CHUNK_SCATTER_SNAP).round() / CHUNK_SCATTER_SNAP,
                    ((i.pos.y - origin.y) * CHUNK_SCATTER_SNAP).round() / CHUNK_SCATTER_SNAP,
                );
                r
            })
            .collect();
        mine.sort_by(|a, b| {
            (a.pos.x, a.pos.y, a.item)
                .partial_cmp(&(b.pos.x, b.pos.y, b.item))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out = ScatterSet::new(self.scatter.assign.patch);
        for i in mine {
            out.splice(i);
        }
        PackedByteArray::from(out.to_bytes().as_slice())
    }
    #[func]
    pub fn set_chunk_scatter_data(&mut self, chunk: Vector2i, bytes: PackedByteArray) {
        let Some(given) = ScatterSet::from_bytes(bytes.as_slice()) else {
            godot_error!("MarchingSquaresTerrain.set_chunk_scatter_data: not a PTSC blob");
            return;
        };
        let c = to_iv(chunk);
        let config = self.store().config.clone();
        let origin = config.sample_pos(config.chunk_center_cell(c));
        let mut touched = TileSet::new();
        for (t, v) in self.scatter.assign.tiles.iter_mut() {
            let before = v.len();
            v.retain(|i| config.chunk_at(i.pos) != c);
            if v.len() != before {
                touched.insert(*t);
            }
        }
        self.scatter.assign.tiles.retain(|_, v| !v.is_empty());
        for i in given.iter() {
            let mut placed = *i;
            placed.pos = v2(i.pos.x + origin.x, i.pos.y + origin.y);
            if config.chunk_at(placed.pos) == c {
                touched.insert(self.scatter.assign.splice(placed));
            }
        }
        if !touched.is_empty() {
            self.scatter.mark_tiles(&touched);
            self.scatter_write();
            self.update();
        }
    }
    #[func]
    pub fn get_scatter_count(&self, item: i64) -> i64 {
        if item < 0 {
            self.scatter.assign.len() as i64
        } else {
            self.scatter.assign.count_item(item as u16) as i64
        }
    }
    #[func]
    pub fn get_scatter_positions(&self, item: i64) -> PackedVector2Array {
        self.scatter
            .assign
            .tiles
            .values()
            .flatten()
            .filter(|i| item < 0 || i.item as i64 == item)
            .map(|i| Vector2::new(i.pos.x, i.pos.y))
            .collect()
    }
    #[func]
    pub fn scatter_paint(&mut self, item: i64, world: Vector3, radius: f32) {
        let l = self.scatter_local(world);
        let b = Brush {
            radius,
            falloff: false,
            ..Default::default()
        };
        if self.scatter_brush(
            Some(item.max(0) as usize),
            &b,
            v2(l.x, l.z),
            false,
            false,
            (0.0, 90.0),
        ) {
            self.scatter_write();
        }
    }
    #[func]
    pub fn scatter_erase(&mut self, item: i64, world: Vector3, radius: f32) {
        let l = self.scatter_local(world);
        let b = Brush {
            radius,
            falloff: false,
            ..Default::default()
        };
        if self.scatter_brush(
            (item >= 0).then_some(item as usize),
            &b,
            v2(l.x, l.z),
            true,
            false,
            (0.0, 90.0),
        ) {
            self.scatter_write();
        }
    }
    #[func]
    pub fn scatter_fill_cells(&mut self, item: i64) {
        let Some(res) = self.scatter_item(item.max(0) as usize) else {
            return;
        };
        let rules = res.bind().rules();
        let cfg = self.store().config.clone();
        let cells: Vec<IVec2> = if cfg.grid == pt_core::grid::GridType::Square {
            self.store()
                .chunk_coords()
                .into_iter()
                .flat_map(|c| {
                    let o = c * cfg.chunk_stride();
                    let n = cfg.chunk_size;
                    (0..n).flat_map(move |z| (0..n).map(move |x| o + pt_core::math::iv2(x, z)))
                })
                .collect()
        } else {
            self.store()
                .chunk_coords()
                .into_iter()
                .flat_map(|c| cfg.chunk_cells(c))
                .collect()
        };
        let mut set = std::mem::take(&mut self.scatter.assign);
        let t = self.scatter_fill(&mut set, &rules, item as u16, &cfg, &cells);
        self.scatter.assign = set;
        self.scatter.mark_tiles(&t);
        self.scatter_write();
    }
    #[func]
    pub fn scatter_clear(&mut self, item: i64) {
        if item < 0 {
            self.scatter.assign.tiles.clear();
        } else {
            let it = item as u16;
            for v in self.scatter.assign.tiles.values_mut() {
                v.retain(|i| i.item != it);
            }
            self.scatter.assign.tiles.retain(|_, v| !v.is_empty());
        }
        self.scatter.mark_all();
        self.scatter_write();
    }
    #[func]
    pub fn get_scatter_data(&self) -> PackedByteArray {
        PackedByteArray::from(self.scatter.assign.to_bytes().as_slice())
    }
    #[func]
    pub fn set_scatter_data(&mut self, bytes: PackedByteArray) {
        self.scatter.assign = ScatterSet::from_bytes(bytes.as_slice()).unwrap_or_default();
        self.scatter.item_ids = Some(self.current_item_ids());
        self.scatter.mark_all();
        self.scatter_write();
        self.update();
    }
    #[func]
    pub fn remove_scatter_item(&mut self, index: i64) {
        if index < 0 || index as usize >= self.scatter_items.len() {
            return;
        }
        let mut items = self.scatter_items.duplicate_shallow();
        items.remove(index as usize);
        self.scatter_items = items;
        self.scatter.assign.remove_item(index as u16);
        self.scatter.item_ids = Some(self.current_item_ids());
        self.scatter.mark_all();
        self.scatter_write();
        self.update();
    }
    #[func]
    pub fn rebuild_scatter(&mut self) {
        self.scatter.signature = 0;
        self.scatter.mark_all();
        self.update();
    }
}
