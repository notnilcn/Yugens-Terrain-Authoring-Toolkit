//! Scatter props on the terrain: the `PixelScatterItem` resource and the terrain's scatter
//! layer (one `MultiMeshInstance3D` per tile and item part, internal children of a `Scatter`
//! node under the terrain, never saved). Instances live in `MarchingSquaresTerrainData`
//! (one blob per grid type, `pt_core::scatter::ScatterSet`) and are re-snapped to the surface
//! whenever the chunks under them are remeshed.
//!
//! Items can carry optional collision (`collision`: cylinder or box sized from the AABB of the
//! item's parts): one internal `StaticBody3D` per tile and item with a shape per instance, on
//! the item's `collision_layer`, rebuilt together with the tile's MultiMeshes (never saved).
//!
//! Placement logic (paint, per-cell hash, filters, jitter) is `pt_core::scatter`; credits:
//! Terrain3D instancer (MIT), hex_map_godot feature manager (MIT), Minor Deity (idea only).

use std::collections::{BTreeSet, HashMap};

use godot::classes::base_material_3d::{Flags as BmFlags, TextureParam, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    ArrayMesh, BaseMaterial3D, BoxShape3D, CollisionShape3D, CylinderShape3D, Material, Mesh, MeshInstance3D, MultiMesh, MultiMeshInstance3D, Node3D, PackedScene, Shape3D, ShaderMaterial,
    StaticBody3D,
};
use godot::prelude::*;

use pt_core::brush::Brush;
use pt_core::math::{IVec2, Vec2 as PVec2, Vec3 as PVec3, v2, v3};
use pt_core::scatter::{PropMesh, PropShape, ScatterRules, ScatterSet, TileSet, instance_transform};

use super::MarchingSquaresTerrain;
use crate::globals;
use crate::util::*;

/// One kind of scatter prop: a built-in low-poly shape, a mesh or a scene, plus placement rules.
#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelScatterItem {
    /// Name shown in the Scatter tool (empty = shape / resource name).
    #[export]
    pub display_name: GString,
    /// Built-in low-poly prop used when neither `mesh` nor `scene` is set.
    #[export(enum = (Custom, Rock, Bush, Tree, Pine, Flowers))]
    pub shape: i32,
    /// Seed of the built-in shape (different rocks / bushes).
    #[export]
    pub shape_seed: i32,
    /// Custom mesh (every surface becomes one MultiMesh part with its own material).
    #[export]
    pub mesh: Option<Gd<Mesh>>,
    /// Custom scene: its MeshInstance3D descendants are instanced (no scripts, no physics).
    #[export]
    pub scene: Option<Gd<PackedScene>>,
    /// Multiplies the albedo of every part.
    #[export]
    pub color: Color,

    #[export_group(name = "Placement")]
    /// Instances per square world unit at full brush weight.
    #[export(range = (0.001, 4.0, 0.001, or_greater))]
    pub density: f32,
    #[export(range = (0.05, 8.0, 0.01, or_greater))]
    pub scale_min: f32,
    #[export(range = (0.05, 8.0, 0.01, or_greater))]
    pub scale_max: f32,
    /// Random yaw range in degrees.
    #[export(range = (0.0, 360.0, 1.0))]
    pub random_yaw: f32,
    /// 0 = stand upright, 1 = follow the terrain normal.
    #[export(range = (0.0, 1.0, 0.01))]
    pub align_to_normal: f32,
    /// Vertical offset in instance units (negative sinks the prop into the ground).
    #[export]
    pub y_offset: f32,
    /// Per-cell mode: at most this many instances per cell.
    #[export(range = (1.0, 16.0, 1.0))]
    pub per_cell_max: i32,
    /// Hex grids, per-cell mode: stand on the 7 whole small hexes of each cell (kg_hex /
    /// BitCraft top decorations: centre + ring, exact small-hex centres) instead of jittered
    /// points; uneven, submerged (cell water level), paved or built-on small hexes are skipped.
    /// `density × small-hex area` is the chance per small hex, `per_cell_max` caps each cell.
    #[export]
    pub small_hexes: bool,
    /// Floor texture slots the item may stand on (none ticked = any).
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
    /// World size of the cluster noise patches (0 = no clustering).
    #[export(range = (0.0, 256.0, 0.5, or_greater))]
    pub cluster_scale: f32,
    /// Noise level needed for a placement (−1 = everywhere, higher = smaller patches).
    #[export(range = (-1.0, 1.0, 0.01))]
    pub cluster_threshold: f32,

    #[export_group(name = "Color")]
    /// ± hue shift in turns.
    #[export(range = (0.0, 0.5, 0.005))]
    pub hue_jitter: f32,
    /// ± brightness.
    #[export(range = (0.0, 0.5, 0.01))]
    pub value_jitter: f32,

    #[export_group(name = "Pixel")]
    #[export(range = (1.0, 4.0, 1.0))]
    pub pixel_size: i32,
    #[export]
    pub outline: bool,
    #[export(range = (0.0, 7.0, 1.0))]
    pub outline_id: i32,
    #[export]
    pub cast_shadows: bool,
    /// Wind sway of the upper part (world units, 0 = static).
    #[export(range = (0.0, 1.0, 0.01))]
    pub wind_sway: f32,

    #[export_group(name = "Collision")]
    /// Static collision per instance: none, an upright cylinder or a box, sized from the AABB
    /// of the item's parts (times `collision_size`) and scaled with each instance.
    #[export(enum = (None, Cylinder, Box))]
    pub collision: i32,
    /// Shape size relative to the parts' AABB: x = width (cylinder diameter), y = height,
    /// z = depth (box only). E.g. (0.35, 1, 0.35) for a tree trunk plus the inner canopy.
    #[export]
    pub collision_size: Vector3,
    /// Physics layers of the colliders.
    #[export(flags_3d_physics)]
    pub collision_layer: u32,
    /// Instances scaled below this get no collider (small pebbles stay walkable).
    #[export(range = (0.0, 8.0, 0.01, or_greater))]
    pub collision_min_scale: f32,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for PixelScatterItem {
    fn init(base: Base<Resource>) -> Self {
        let r = ScatterRules::default();
        PixelScatterItem {
            display_name: GString::new(),
            shape: 1,
            shape_seed: 1,
            mesh: None,
            scene: None,
            color: Color::WHITE,
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
            collision_layer: 1,
            collision_min_scale: 0.0,
            base,
        }
    }
}

#[godot_api]
impl PixelScatterItem {
    /// A ready-to-use built-in item (`rock`, `bush`, `tree`, `pine`, `flowers`) with sensible rules.
    #[func]
    pub fn create_builtin(kind: GString) -> Gd<PixelScatterItem> {
        let mut g = PixelScatterItem::new_gd();
        {
            let mut b = g.bind_mut();
            let k = kind.to_string().to_lowercase();
            let (shape, density, smin, smax, slope, align, sway, cl) = match k.as_str() {
                "bush" => (2, 0.04, 0.7, 1.3, 30.0, 0.3, 0.04, (14.0, 0.0)),
                "tree" => (3, 0.02, 0.8, 1.4, 25.0, 0.0, 0.05, (22.0, -0.1)),
                "pine" => (4, 0.025, 0.8, 1.5, 32.0, 0.0, 0.03, (18.0, 0.0)),
                "flowers" => (5, 0.08, 0.8, 1.2, 25.0, 0.6, 0.06, (9.0, 0.25)),
                _ => (1, 0.03, 0.5, 1.4, 55.0, 0.8, 0.0, (0.0, 0.0)),
            };
            b.shape = shape;
            b.display_name = GString::from(format!("{}{}", &k[..1].to_uppercase(), &k[1..]).as_str());
            b.density = density;
            b.scale_min = smin;
            b.scale_max = smax;
            b.slope_max = slope;
            b.align_to_normal = align;
            b.wind_sway = sway;
            b.cluster_scale = cl.0;
            b.cluster_threshold = cl.1;
            if shape == 1 {
                b.y_offset = -0.12;
                b.hue_jitter = 0.01;
            }
        }
        g
    }

    /// Name shown in the editor.
    #[func]
    pub fn get_label(&self) -> GString {
        if !self.display_name.is_empty() {
            return self.display_name.clone();
        }
        if let Some(m) = &self.mesh {
            let p = m.get_path().to_string();
            if !p.is_empty() {
                return p.rsplit('/').next().unwrap_or("Mesh").into();
            }
            return "Mesh".into();
        }
        if let Some(s) = &self.scene {
            return s.get_path().to_string().rsplit('/').next().unwrap_or("Scene").into();
        }
        pt_core::scatter::PROP_SHAPES.get(self.shape.clamp(0, 5) as usize).copied().unwrap_or("Item").into()
    }
}

impl PixelScatterItem {
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

    /// Hash of everything that changes the built nodes (polled to rebuild after inspector edits).
    fn signature(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.shape.hash(h);
        self.shape_seed.hash(h);
        self.mesh.as_ref().map(|m| m.instance_id().to_i64()).hash(h);
        self.scene.as_ref().map(|m| m.instance_id().to_i64()).hash(h);
        for f in [self.color.r, self.color.g, self.color.b, self.color.a, self.align_to_normal, self.y_offset, self.wind_sway] {
            f.to_bits().hash(h);
        }
        (self.pixel_size, self.outline, self.outline_id, self.cast_shadows).hash(h);
        (self.collision, self.collision_layer).hash(h);
        for f in [self.collision_size.x, self.collision_size.y, self.collision_size.z, self.collision_min_scale] {
            f.to_bits().hash(h);
        }
    }
}

// --------------------------------------------------------------------------- parts

/// One drawable part of an item: a single-surface mesh, its transform in the item and material.
struct Part {
    mesh: Gd<Mesh>,
    xform: Transform3D,
    material: Gd<Material>,
}

fn prop_mesh(shape: PropShape, seed: u32) -> Option<Gd<Mesh>> {
    let m = PropMesh::build(shape, seed);
    if m.positions.is_empty() {
        return None;
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let pos: PackedVector3Array = m.positions.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect();
    let nrm: PackedVector3Array = m.normals.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect();
    let col: PackedColorArray = m.colors.iter().map(|c| Color::from_rgba(c[0], c[1], c[2], c[3])).collect();
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    Some(mesh.upcast())
}

/// Scatter material for an item part, derived from the part's own material when it is a
/// StandardMaterial3D/ORMMaterial3D (albedo color + texture, vertex color, alpha scissor).
/// Shader materials are used as they are.
fn part_material(item: &PixelScatterItem, src: Option<Gd<Material>>, builtin: bool) -> Gd<Material> {
    if let Some(m) = &src
        && m.clone().try_cast::<ShaderMaterial>().is_ok()
    {
        return m.clone();
    }
    let mut mat = shader_material("mst_scatter.gdshader");
    let mut albedo = item.color;
    let mut vertex_color = builtin;
    if let Some(b) = src.and_then(|m| m.try_cast::<BaseMaterial3D>().ok()) {
        let a = b.get_albedo();
        albedo = Color::from_rgba(albedo.r * a.r, albedo.g * a.g, albedo.b * a.b, albedo.a * a.a);
        vertex_color = b.get_flag(BmFlags::ALBEDO_FROM_VERTEX_COLOR);
        if let Some(t) = b.get_texture(TextureParam::ALBEDO) {
            mat.set_shader_parameter("albedo_texture", &t.to_variant());
            mat.set_shader_parameter("use_albedo_texture", &true.to_variant());
        }
        let tr = b.get_transparency();
        if tr == Transparency::ALPHA_SCISSOR || tr == Transparency::ALPHA || tr == Transparency::ALPHA_HASH {
            mat.set_shader_parameter("use_alpha_scissor", &true.to_variant());
            mat.set_shader_parameter("alpha_scissor_threshold", &b.get_alpha_scissor_threshold().to_variant());
        }
    }
    mat.set_shader_parameter("albedo_color", &albedo.to_variant());
    mat.set_shader_parameter("use_vertex_color", &vertex_color.to_variant());
    mat.set_shader_parameter("wind_sway", &item.wind_sway.to_variant());
    mat.set_shader_parameter("pt_flags", &object_flags(item.pixel_size, item.outline, item.outline_id, false).to_variant());
    mat.upcast()
}

fn split_surfaces(item: &PixelScatterItem, mesh: &Gd<Mesh>, xform: Transform3D, overrides: &dyn Fn(i32) -> Option<Gd<Material>>, out: &mut Vec<Part>) {
    for s in 0..mesh.get_surface_count() {
        let arrays = mesh.surface_get_arrays(s);
        let mut single = ArrayMesh::new_gd();
        let prim = mesh.clone().try_cast::<ArrayMesh>().map(|a| a.surface_get_primitive_type(s)).unwrap_or(PrimitiveType::TRIANGLES);
        single.add_surface_from_arrays(prim, &arrays);
        let src = overrides(s).or_else(|| mesh.surface_get_material(s));
        out.push(Part { mesh: single.upcast(), xform, material: part_material(item, src, false) });
    }
}

fn collect_scene(item: &PixelScatterItem, node: &Gd<Node>, xform: Transform3D, out: &mut Vec<Part>) {
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
    if let Some(mesh) = &item.mesh {
        split_surfaces(item, mesh, Transform3D::IDENTITY, &|_| None, &mut out);
    } else if let Some(scene) = &item.scene {
        if let Some(root) = scene.instantiate() {
            // The root's own transform is the scene's placement, not part of the prop.
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
    } else if let Some(shape) = PropShape::from_i64(item.shape as i64)
        && let Some(mesh) = prop_mesh(shape, item.shape_seed as u32)
    {
        out.push(Part { mesh, xform: Transform3D::IDENTITY, material: part_material(item, None, true) });
    }
    out
}

/// Collision of an item in item space (instance scale applied per instance).
#[derive(Clone, Copy)]
struct Collider {
    cylinder: bool,
    centre: Vector3,
    size: Vector3,
    layer: u32,
    min_scale: f32,
}

fn build_collider(item: &PixelScatterItem, parts: &[Part]) -> Option<Collider> {
    if item.collision <= 0 || item.collision_layer == 0 {
        return None;
    }
    let (mut lo, mut hi) = (Vector3::splat(f32::MAX), Vector3::splat(f32::MIN));
    for p in parts {
        let b = p.mesh.get_aabb();
        for i in 0..8 {
            let c = b.position + Vector3::new(if i & 1 != 0 { b.size.x } else { 0.0 }, if i & 2 != 0 { b.size.y } else { 0.0 }, if i & 4 != 0 { b.size.z } else { 0.0 });
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
    // Upright from the AABB floor, centred on the AABB's footprint.
    let centre = Vector3::new((lo.x + hi.x) * 0.5, lo.y + size.y * 0.5, (lo.z + hi.z) * 0.5);
    Some(Collider { cylinder: item.collision == 1, centre, size, layer: item.collision_layer, min_scale: item.collision_min_scale })
}

/// One static body holding a shape per instance (`xforms` are terrain-local instance transforms).
fn collision_body(c: &Collider, xforms: &[(Transform3D, Color)], name: &str) -> Option<Gd<StaticBody3D>> {
    let mut body: Option<Gd<StaticBody3D>> = None;
    for (xf, _) in xforms {
        let k = xf.basis.col_b().length();
        if k < c.min_scale.max(1e-4) {
            continue;
        }
        let shape: Gd<Shape3D> = if c.cylinder {
            let mut cy = CylinderShape3D::new_gd();
            cy.set_radius(0.5 * c.size.x.max(c.size.z) * k);
            cy.set_height(c.size.y * k);
            cy.upcast()
        } else {
            let mut bx = BoxShape3D::new_gd();
            bx.set_size(c.size * k);
            bx.upcast()
        };
        let basis = Basis::from_cols(xf.basis.col_a() / k, xf.basis.col_b() / k, xf.basis.col_c() / k);
        let mut cs = CollisionShape3D::new_alloc();
        cs.set_shape(&shape);
        cs.set_transform(Transform3D::new(basis, *xf * c.centre));
        let b = body.get_or_insert_with(|| {
            let mut b = StaticBody3D::new_alloc();
            b.set_name(name);
            b.set_collision_layer(c.layer);
            b.set_collision_mask(0);
            b
        });
        b.add_child(&cs);
    }
    body
}

// --------------------------------------------------------------------------- layer

/// Per-item data cached for building.
struct ItemCache {
    parts: Vec<Part>,
    align: f32,
    y_offset: f32,
    shadows: bool,
    collider: Option<Collider>,
}

/// The terrain's scatter state (owned by `MarchingSquaresTerrain`).
#[derive(Default)]
pub struct ScatterLayer {
    pub set: ScatterSet,
    root: Option<Gd<Node3D>>,
    nodes: HashMap<IVec2, Vec<Gd<Node3D>>>,
    dirty: BTreeSet<IVec2>,
    all_dirty: bool,
    items: Vec<Option<ItemCache>>,
    signature: u64,
    /// Instance ids of `scatter_items` the stored instances refer to (None = not known yet,
    /// e.g. right after loading): an inspector edit that removes / reorders items re-indexes
    /// the instances against it (`scatter_sync_item_ids`).
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
    fn current_item_ids(&self) -> Vec<i64> {
        self.scatter_items.iter_shared().map(|it| it.map(|i| i.instance_id().to_i64()).unwrap_or(0)).collect()
    }

    /// Keeps stored instances on their items when the item list is edited outside the Scatter
    /// tool (inspector remove / move): instances follow their item's new index and those of a
    /// removed item are dropped. Instances of empty slots stay on their index.
    fn scatter_sync_item_ids(&mut self) {
        let now = self.current_item_ids();
        let Some(old) = self.scatter.item_ids.replace(now.clone()) else { return };
        if old == now || self.scatter.set.is_empty() {
            return;
        }
        let mut used = vec![false; now.len()];
        let map: Vec<Option<u16>> = old
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let j = if *id == 0 { (now.get(i) == Some(&0)).then_some(i) } else { (0..now.len()).find(|&j| now[j] == *id && !used[j]) };
                j.map(|j| {
                    used[j] = true;
                    j as u16
                })
            })
            .collect();
        if map.iter().enumerate().all(|(i, m)| *m == Some(i as u16)) {
            return;
        }
        self.scatter.set.remap_items(&map);
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

    /// Height, normal and floor slot of the terrain-local surface (None on void cells / outside).
    pub fn surface_at(&self, p: PVec2) -> Option<(f32, PVec3, u8)> {
        let cfg = &self.store().config;
        let s = cfg.world_to_sample(p);
        let slot = self.store().sample(s).map(|sm| sm.floor).unwrap_or(0);
        if slot == 15 {
            return None;
        }
        let h = self.local_height_at(p)?;
        let e = (cfg.cell_size_safe().x * 0.2).max(0.05);
        let hx = |dx: f32, dz: f32| self.local_height_at(p + v2(dx, dz)).unwrap_or(h);
        let gx = (hx(e, 0.0) - hx(-e, 0.0)) / (2.0 * e);
        let gz = (hx(0.0, e) - hx(0.0, -e)) / (2.0 * e);
        Some((h, v3(-gx, 1.0, -gz).normalized(), slot))
    }

    /// Loads the instances of the current grid type from the data resource.
    pub(super) fn scatter_reload(&mut self) {
        let grid = self.grid();
        let set = self.data.as_ref().and_then(|d| d.bind().scatter_bytes(grid)).and_then(|b| ScatterSet::from_bytes(&b)).unwrap_or_default();
        self.scatter.set = set;
        self.scatter.all_dirty = true;
        // Stored instances match the item list as it is now.
        self.scatter.item_ids = Some(self.current_item_ids());
    }

    /// Writes the instances into the data resource.
    pub fn scatter_write(&mut self) {
        let grid = self.grid();
        let bytes = self.scatter.set.to_bytes();
        if let Some(d) = &mut self.data {
            d.bind_mut().write_scatter(grid, if self.scatter.set.is_empty() { None } else { Some(&bytes) });
        }
    }

    /// Marks the scatter tiles over rebuilt terrain chunks (re-snap after sculpting).
    pub(super) fn scatter_mark_chunks(&mut self, chunks: &[IVec2]) {
        if self.scatter.set.is_empty() {
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
            let pad = cfg.cell_size_safe() * 1.5;
            for t in self.scatter.set.tiles_in_rect(min - pad, max + pad) {
                if self.scatter.set.tiles.contains_key(&t) || self.scatter.nodes.contains_key(&t) {
                    self.scatter.dirty.insert(t);
                }
            }
        }
    }

    fn ensure_scatter_root(&mut self) -> Option<Gd<Node3D>> {
        if let Some(r) = &self.scatter.root
            && r.is_instance_valid()
        {
            return Some(r.clone());
        }
        let mut root = Node3D::new_alloc();
        root.set_name("Scatter");
        let mut this = self.to_gd();
        add_internal(&mut this, &root);
        self.scatter.root = Some(root.clone());
        self.scatter.nodes.clear();
        self.scatter.all_dirty = true;
        Some(root)
    }

    /// Rebuilds dirty scatter tiles (called from `update`).
    pub(super) fn scatter_flush(&mut self) {
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
                        ItemCache { parts, align: b.align_to_normal, y_offset: b.y_offset, shadows: b.cast_shadows, collider }
                    })
                })
                .collect();
            self.scatter.all_dirty = true;
        }
        if !self.scatter.all_dirty && self.scatter.dirty.is_empty() {
            return;
        }
        let Some(mut root) = self.ensure_scatter_root() else { return };
        let tiles: Vec<IVec2> = if std::mem::take(&mut self.scatter.all_dirty) {
            self.scatter.dirty.clear();
            self.scatter.nodes.keys().chain(self.scatter.set.tiles.keys()).copied().collect::<BTreeSet<_>>().into_iter().collect()
        } else {
            std::mem::take(&mut self.scatter.dirty).into_iter().collect()
        };
        let layers = self.render_layers | globals::meta_mask();
        for t in tiles {
            for n in self.scatter.nodes.remove(&t).unwrap_or_default() {
                free_node(Some(n));
            }
            let Some(list) = self.scatter.set.tiles.get(&t) else { continue };
            // Group by item, with the surface sampled once per instance.
            let mut by_item: HashMap<u16, Vec<(Transform3D, Color)>> = HashMap::new();
            for inst in list {
                // Distant LOD ring: no props on low-detail chunks (like their grass). Their
                // tiles are re-flushed when the chunks are remeshed (`scatter_mark_chunks`).
                if !self.lod_low.is_empty() && self.lod_low.contains(&self.store().config.chunk_at(inst.pos)) {
                    continue;
                }
                let Some(Some(cache)) = self.scatter.items.get(inst.item as usize) else { continue };
                let Some((h, n, _)) = self.surface_at(inst.pos) else { continue };
                // Builder floors, raised water and cleared cells (`set_cells_cleared`).
                if !self.grass_water.is_empty() && self.grass_water.hides_prop(&self.store().config, v3(inst.pos.x, h, inst.pos.y)) {
                    continue;
                }
                let ([x, y, z], o) = instance_transform(inst, h, n, cache.align, cache.y_offset);
                let basis = Basis::from_cols(Vector3::new(x.x, x.y, x.z), Vector3::new(y.x, y.y, y.z), Vector3::new(z.x, z.y, z.z));
                let xf = Transform3D::new(basis, Vector3::new(o.x, o.y, o.z));
                by_item.entry(inst.item).or_default().push((xf, Color::from_rgba(inst.hue, inst.value, 0.0, 0.0)));
            }
            let mut made = Vec::new();
            let mut items: Vec<_> = by_item.into_iter().collect();
            items.sort_by_key(|(k, _)| *k);
            for (item, xforms) in items {
                let Some(Some(cache)) = self.scatter.items.get(item as usize) else { continue };
                for (pi, part) in cache.parts.iter().enumerate() {
                    let mut mm = MultiMesh::new_gd();
                    mm.set_transform_format(TransformFormat::TRANSFORM_3D);
                    // Compatibility multiplies vertex COLOR by the instance color even when
                    // it is unused (it then reads 0): always provide white instance colors.
                    mm.set_use_colors(true);
                    mm.set_use_custom_data(true);
                    mm.set_mesh(&part.mesh);
                    mm.set_instance_count(xforms.len() as i32);
                    for (k, (x, c)) in xforms.iter().enumerate() {
                        mm.set_instance_transform(k as i32, *x * part.xform);
                        mm.set_instance_color(k as i32, Color::WHITE);
                        mm.set_instance_custom_data(k as i32, *c);
                    }
                    let mut mmi = MultiMeshInstance3D::new_alloc();
                    mmi.set_name(&format!("Tile ({}, {}) item {item} part {pi}", t.x, t.y));
                    mmi.set_multimesh(&mm);
                    mmi.set_material_override(&part.material);
                    mmi.set_layer_mask(layers);
                    mmi.set_cast_shadows_setting(if cache.shadows { ShadowCastingSetting::ON } else { ShadowCastingSetting::OFF });
                    root.add_child(&mmi);
                    made.push(mmi.upcast());
                }
                if let Some(c) = &cache.collider
                    && let Some(body) = collision_body(c, &xforms, &format!("Tile ({}, {}) item {item} collision", t.x, t.y))
                {
                    root.add_child(&body);
                    made.push(body.upcast());
                }
            }
            if !made.is_empty() {
                self.scatter.nodes.insert(t, made);
            }
        }
    }

    // ------------------------------------------------------------------ editing (Rust)

    /// Paints (`erase = false`) or erases scatter under a brush; returns whether anything changed.
    /// `slope` narrows the item's own slope range (the tool's slope filter).
    pub fn scatter_brush(&mut self, item: Option<usize>, brush: &Brush, center: PVec2, erase: bool, per_cell: bool, slope: (f32, f32)) -> bool {
        let t = if erase {
            let it = item.map(|i| i as u16);
            self.scatter.set.erase(brush, center, it)
        } else {
            let Some(i) = item else { return false };
            let Some(res) = self.scatter_item(i) else { return false };
            let mut rules = res.bind().rules();
            rules.slope_min = rules.slope_min.max(slope.0);
            rules.slope_max = rules.slope_max.min(slope.1);
            let mut set = std::mem::take(&mut self.scatter.set);
            let surf = |p: PVec2| self.surface_at(p);
            let t = if per_cell {
                let cfg = self.store().config.clone();
                let cells: Vec<IVec2> = pt_core::brush::footprint(&cfg, brush, center).into_iter().filter(|(_, w)| *w >= 0.5).map(|(c, _)| c).collect();
                self.scatter_fill(&mut set, &rules, i as u16, &cfg, &cells)
            } else {
                set.paint(&rules, i as u16, brush, center, &surf)
            };
            self.scatter.set = set;
            t
        };
        let changed = !t.is_empty();
        self.scatter.mark_tiles(&t);
        changed
    }

    /// Per-cell fill of `cells`: jittered points, or the small-hex spots on hex grids.
    fn scatter_fill(&self, set: &mut ScatterSet, rules: &ScatterRules, item: u16, cfg: &pt_core::grid::GridConfig, cells: &[IVec2]) -> pt_core::scatter::TileSet {
        let surf = |p: PVec2| self.surface_at(p);
        if rules.small_hex && matches!(cfg.grid, pt_core::grid::GridType::Hex | pt_core::grid::GridType::HexRings) {
            let spots = |c: IVec2| -> Vec<(IVec2, PVec2)> {
                self.decoration_spots_local(c, rules.height_min.max(-1.0e6)).into_iter().map(|(s, p)| (s, pt_core::math::v2(p.x, p.z))).collect()
            };
            set.fill_small_hexes(rules, item, cfg, cells, &spots, &surf)
        } else {
            set.fill_cells(rules, item, cfg, cells, &surf)
        }
    }

    /// Removes the props (any item) standing on blocked small hexes inside `[lo, hi]`.
    pub fn scatter_remove_on_small_hexes(&mut self, lo: PVec2, hi: PVec2, blocked: &dyn Fn(IVec2) -> bool) {
        let cfg = self.store().config.clone();
        let t = self.scatter.set.remove_on_small_hexes(&cfg, lo, hi, blocked);
        if !t.is_empty() {
            self.scatter.mark_tiles(&t);
            self.scatter_write();
        }
    }

    /// World → terrain-local (also before the node enters the tree).
    fn scatter_local(&self, world: Vector3) -> Vector3 {
        let xf = if self.base().is_inside_tree() { self.base().get_global_transform() } else { self.base().get_transform() };
        xf.affine_inverse() * world
    }

    pub fn scatter_bytes(&self) -> Vec<u8> {
        self.scatter.set.to_bytes()
    }

    pub fn scatter_item_labels(&self) -> Vec<String> {
        (0..self.scatter_items.len()).map(|i| self.scatter_item(i).map(|it| it.bind().get_label().to_string()).unwrap_or_else(|| format!("<empty {i}>"))).collect()
    }
}

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Number of scatter instances (all items, or one item index).
    #[func]
    pub fn get_scatter_count(&self, item: i64) -> i64 {
        if item < 0 { self.scatter.set.len() as i64 } else { self.scatter.set.count_item(item as u16) as i64 }
    }

    /// Terrain-local XZ positions of the scatter instances (all items, or one item index),
    /// including the ones hidden on distant low-detail chunks.
    #[func]
    pub fn get_scatter_positions(&self, item: i64) -> PackedVector2Array {
        self.scatter
            .set
            .tiles
            .values()
            .flatten()
            .filter(|i| item < 0 || i.item as i64 == item)
            .map(|i| Vector2::new(i.pos.x, i.pos.y))
            .collect()
    }

    /// Paints scatter item `item` around a world position (round brush of `radius`).
    #[func]
    pub fn scatter_paint(&mut self, item: i64, world: Vector3, radius: f32) {
        let l = self.scatter_local(world);
        let b = Brush { radius, falloff: false, ..Default::default() };
        if self.scatter_brush(Some(item.max(0) as usize), &b, v2(l.x, l.z), false, false, (0.0, 90.0)) {
            self.scatter_write();
        }
    }

    /// Erases scatter around a world position (`item < 0` = every item).
    #[func]
    pub fn scatter_erase(&mut self, item: i64, world: Vector3, radius: f32) {
        let l = self.scatter_local(world);
        let b = Brush { radius, falloff: false, ..Default::default() };
        if self.scatter_brush((item >= 0).then_some(item as usize), &b, v2(l.x, l.z), true, false, (0.0, 90.0)) {
            self.scatter_write();
        }
    }

    /// Per-cell deterministic scatter of `item` over every cell of the terrain (replaces that
    /// item's instances). Same result on every machine for the same terrain and item rules.
    #[func]
    pub fn scatter_fill_cells(&mut self, item: i64) {
        let Some(res) = self.scatter_item(item.max(0) as usize) else { return };
        let rules = res.bind().rules();
        let cfg = self.store().config.clone();
        let cells: Vec<IVec2> = if cfg.grid == pt_core::grid::GridType::Square {
            // Square cells are addressed by their lower vertex.
            self.store().chunk_coords().into_iter().flat_map(|c| {
                let o = c * cfg.chunk_stride();
                let n = cfg.chunk_size;
                (0..n).flat_map(move |z| (0..n).map(move |x| o + pt_core::math::iv2(x, z)))
            }).collect()
        } else {
            self.store().chunk_coords().into_iter().flat_map(|c| cfg.chunk_cells(c)).collect()
        };
        let mut set = std::mem::take(&mut self.scatter.set);
        let t = self.scatter_fill(&mut set, &rules, item as u16, &cfg, &cells);
        self.scatter.set = set;
        self.scatter.mark_tiles(&t);
        self.scatter_write();
    }

    /// Removes every scatter instance (`item < 0`) or those of one item.
    #[func]
    pub fn scatter_clear(&mut self, item: i64) {
        if item < 0 {
            self.scatter.set.tiles.clear();
        } else {
            let it = item as u16;
            for v in self.scatter.set.tiles.values_mut() {
                v.retain(|i| i.item != it);
            }
            self.scatter.set.tiles.retain(|_, v| !v.is_empty());
        }
        self.scatter.mark_all();
        self.scatter_write();
    }

    /// Raw scatter blob of the current grid type (undo/redo, save tools).
    #[func]
    pub fn get_scatter_data(&self) -> PackedByteArray {
        PackedByteArray::from(self.scatter.set.to_bytes().as_slice())
    }

    /// Restores a scatter blob (empty clears) and rebuilds the props.
    #[func]
    pub fn set_scatter_data(&mut self, bytes: PackedByteArray) {
        self.scatter.set = ScatterSet::from_bytes(bytes.as_slice()).unwrap_or_default();
        // The blob belongs to the item list as it is now (undo restores both together).
        self.scatter.item_ids = Some(self.current_item_ids());
        self.scatter.mark_all();
        self.scatter_write();
        self.update();
    }

    /// Removes scatter item `index` from `scatter_items` and its instances; instances of
    /// later items move down one index with their items.
    #[func]
    pub fn remove_scatter_item(&mut self, index: i64) {
        if index < 0 || index as usize >= self.scatter_items.len() {
            return;
        }
        let mut items = self.scatter_items.duplicate_shallow();
        items.remove(index as usize);
        self.scatter_items = items;
        self.scatter.set.remove_item(index as u16);
        self.scatter.item_ids = Some(self.current_item_ids());
        self.scatter.mark_all();
        self.scatter_write();
        self.update();
    }

    /// Rebuilds the scatter nodes now (e.g. after editing an item's mesh from a script).
    #[func]
    pub fn rebuild_scatter(&mut self) {
        self.scatter.signature = 0;
        self.scatter.mark_all();
        self.update();
    }
}
