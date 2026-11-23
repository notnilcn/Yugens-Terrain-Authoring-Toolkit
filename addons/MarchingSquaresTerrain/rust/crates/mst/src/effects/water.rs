//! A body of water: a stylised surface, light patterns on what lies under it, and
//! waterfalls.
//!
//! Intent: a scene author places one at sea level, or marks raised water on terrain tiles,
//! and gets water drawn in the pixel-art style, with caustics on the ground beneath it and
//! waterfalls wherever water spills over a drop in the terrain.
//!
//! Responsibility: it owns the water's surface, the cells that hold raised water and the
//! falls generated from them, and it publishes its level and caustics for the terrain's
//! shading. It tells the terrain when its grass needs to know where water is. It does not
//! own the terrain's shape; it rebuilds its falls when the terrain reports a rebuild.
//!
//! Why a node: it has a place and a height in the world, it listens to the terrain's
//! signals, and it needs per-frame callbacks to publish its state.
//!
//! Why a new type: a built-in mesh can show a plane, but not falls that follow the
//! terrain's edges or water levels the terrain shading can read.
//!
//! What the scene supplies: the terrain the water belongs to and the mesh node that shows
//! the surface are both assigned by the scene author; the node never goes looking for
//! either, and says so in the editor when one is missing.
//!
//! Created in code: the waterfall mesh, because whether any falls exist depends on the
//! terrain's shape and the water cells, which are not known until the terrain is built.
//!
//! Not exported: the falls found last time and whether they must be rebuilt, which are
//! derived from the terrain; the terrain it is listening to; the render
//! layers it last applied; and the handle to the falls it builds.

use super::feature_gate::{gate, gate_warnings};
use crate::features::Feature;
use crate::globals;
use crate::internal_children::add_internal;
use crate::object_flags::object_flags;
use crate::shader_materials::shader_material;
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::object::ConnectFlags;
use godot::classes::{INode3D, MeshInstance3D, Node3D, PlaneMesh, ShaderMaterial};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelWater {
    #[export]
    #[var(set = assign_measure)]
    size: Vector2,
    #[export(range = (0.0, 256.0, 1.0))]
    #[var(set = assign_refinements)]
    subdivisions: i32,
    #[export]
    #[var(set = assign_stuff)]
    stuff: Option<Gd<ShaderMaterial>>,
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,
    #[export]
    #[var(get = get_surface, set = set_surface)]
    surface: Option<Gd<MeshInstance3D>>,
    #[export_group(name = "Caustics", prefix = "caustics_")]
    #[export(range = (0.0, 2.0, 0.0105))]
    caustics_strength: f32,
    #[export(range = (0.0505, 4.0, 0.0105))]
    caustics_scale: f32,
    #[export(range = (1.0, 6.0, 1.0))]
    caustics_bands: i32,
    #[export(range = (0.0, 4.0, 0.0105))]
    caustics_reach: f32,
    #[export_group(name = "Waterfalls")]
    #[export]
    #[var(get = get_terrain, set = set_terrain)]
    terrain: Option<Gd<MarchingSquaresTerrain>>,
    #[export]
    #[var(set = set_water_cells)]
    water_cells: PackedVector3Array,
    #[export_subgroup(name = "Falls", prefix = "waterfall_")]
    #[export(range = (1.0, 64.0, 1.0))]
    #[var(set = set_waterfall_min_levels)]
    waterfall_min_levels: i32,
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    #[var(set = set_waterfall_level_height)]
    waterfall_level_height: f32,
    #[export]
    #[var(set = assign_cascade_stuff)]
    cascade_stuff: Option<Gd<ShaderMaterial>>,
    falls_dirty: bool,
    falls_node: Option<Gd<MeshInstance3D>>,
    connected_terrain: Option<Gd<MarchingSquaresTerrain>>,
    fall_info: Vec<pt_core::waterfall::Fall>,
    applied_mask: u32,
    base: Base<Node3D>,
}
const LEGACY_PROPS: &[&str] = &[
    "mesh",
    "material_override",
    "material_overlay",
    "cast_shadow",
    "skeleton",
    "skin",
    "gi_mode",
    "gi_lightmap_scale",
    "gi_lightmap_texel_scale",
    "visibility_range_begin",
    "visibility_range_end",
    "lod_bias",
    "extra_cull_margin",
    "custom_aabb",
    "sorting_offset",
    "sorting_use_aabb_center",
    "ignore_occlusion_culling",
    "transparency",
];
#[godot_api]
impl INode3D for PixelWater {
    fn init(base: Base<Node3D>) -> Self {
        PixelWater {
            size: Vector2::new(64.0, 64.0),
            subdivisions: 64,
            stuff: None,
            caustics_strength: 0.3505,
            caustics_scale: 0.5505,
            caustics_bands: 3,
            caustics_reach: 0.5,
            terrain: None,
            water_cells: PackedVector3Array::new(),
            waterfall_min_levels: 5,
            waterfall_level_height: 0.0,
            cascade_stuff: None,
            falls_dirty: true,
            falls_node: None,
            surface: None,
            connected_terrain: None,
            fall_info: Vec::new(),
            applied_mask: 0,
            layers: 1,
            base,
        }
    }
    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        let name = property.to_string();
        LEGACY_PROPS.contains(&name.as_str())
            || name.starts_with("instance_shader_parameters/")
            || name.starts_with("surface_material_override/")
    }
    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        crate::features::observe(&self.to_gd());
        self.reassemble();
        self.refresh_pixel_settings();
        self._on_pixel_features_changed();
    }
    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        if self.falls_dirty {
            self.rebuild_falls();
        }
        let visible = self.base().is_visible_in_tree();
        let level = self.base().get_global_position().y;
        let a = Vector4::new(if visible { 1.0 } else { 0.0 }, level, 0.0, 0.0);
        let b = Vector4::new(
            self.caustics_strength,
            self.caustics_scale,
            self.caustics_bands.max(1) as f32,
            self.caustics_reach,
        );
        globals::assign("pt_water_a", &a.to_variant());
        globals::assign("pt_water_b", &b.to_variant());
    }
    fn exit_tree(&mut self) {
        globals::assign("pt_water_a", &Vector4::ZERO.to_variant());
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::Water);
        if self.surface.is_none() {
            w.push("Add a MeshInstance3D child and assign it to Surface: it shows the water.");
        }
        if self.terrain.is_none() {
            w.push("Assign the terrain this water belongs to: without it there are no waterfalls and the grass does not know where the water is.");
        }
        w
    }
}
fn same_cell(p: Vector3, c: Vector2i) -> bool {
    p.x.round() as i32 == c.x && p.y.round() as i32 == c.y
}
#[expect(
    deprecated,
    reason = "the terrain offers its terrace height to Rust only through its generated property getter"
)]
fn terrace_height(t: &Gd<MarchingSquaresTerrain>) -> f32 {
    t.bind().get_level_height()
}
impl PixelWater {
    fn live_terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.terrain.clone().filter(|t| t.is_instance_valid())
    }
    fn live_surface(&self) -> Option<Gd<MeshInstance3D>> {
        self.surface.clone().filter(|s| s.is_instance_valid())
    }
    fn listen_to(&mut self, terrain: Option<&Gd<MarchingSquaresTerrain>>) {
        if self.connected_terrain.as_ref() == terrain {
            return;
        }
        let cb = self.to_gd().callable("_on_terrain_rebuilt");
        if let Some(mut old) = self.connected_terrain.take()
            && old.is_instance_valid()
            && old.is_connected("chunks_rebuilt", &cb)
        {
            old.disconnect("chunks_rebuilt", &cb);
        }
        if let Some(t) = terrain {
            let mut t = t.clone();
            if !t.is_connected("chunks_rebuilt", &cb) {
                t.connect_flags("chunks_rebuilt", &cb, ConnectFlags::DEFERRED);
            }
            self.connected_terrain = Some(t);
        }
    }
    fn rebuild_falls(&mut self) {
        let _p = crate::prof::breadth("water.rebuild_falls");
        if !self.base().is_inside_tree() || !crate::features::active(Feature::Water) {
            return;
        }
        self.falls_dirty = false;
        let ft = self.live_terrain();
        self.listen_to(ft.as_ref());
        if let Some(mut t) = ft.clone() {
            t.run_deferred_gd(|mut t| {
                if t.is_instance_valid() {
                    t.bind_mut().refresh_grass_water();
                }
            });
        }
        let mut cells: Vec<Vector3> = self.water_cells.as_slice().to_vec();
        if let Some(t) = &ft {
            let stored = t.bind().get_raised_water_cells();
            for v in stored.as_slice() {
                if !cells
                    .iter()
                    .any(|p| same_cell(*p, Vector2i::new(v.x as i32, v.y as i32)))
                {
                    cells.push(*v);
                }
            }
        }
        let all_cells: PackedVector3Array = cells.into_iter().collect();
        let terrain = if all_cells.is_empty() { None } else { ft };
        let built = terrain.as_ref().and_then(|t| {
            let t3 = t.clone().upcast::<Node3D>();
            let ty = t3.get_global_position().y;
            let level = if self.waterfall_level_height > 0.0 {
                self.waterfall_level_height
            } else {
                terrace_height(t).abs().max(1e-3)
            };
            let sea = self
                .base()
                .is_visible_in_tree()
                .then(|| self.base().get_global_position().y - ty);
            let min_drop = level * self.waterfall_min_levels.max(1) as f32;
            super::waterfall::assemble(t, &super::waterfall::water_map(&all_cells), sea, min_drop)
                .map(|f| (f, t3))
        });
        let Some((falls, t3)) = built else {
            self.fall_info.clear();
            if let Some(mut n) = self.falls_node.take()
                && n.is_instance_valid()
            {
                n.queue_free();
            }
            return;
        };
        self.fall_info = falls.falls;
        if self.cascade_stuff.is_none() {
            self.cascade_stuff = Some(shader_material("waterfall.gdshader"));
        }
        let mut node = match &self.falls_node {
            Some(n) if n.is_instance_valid() => n.clone(),
            _ => {
                let mut n = MeshInstance3D::new_alloc();
                n.set_name("Waterfalls");
                n.set_as_top_level(true);
                n.set_cast_shadows_setting(ShadowCastingSetting::OFF);
                add_internal(&mut self.to_gd(), &n);
                self.falls_node = Some(n.clone());
                n
            }
        };
        crate::node_setters::set_global_transform(&mut node, t3.get_global_transform());
        match &falls.cage {
            Some(m) => node.set_mesh(m),
            None => node.set_mesh(Gd::<godot::classes::Mesh>::null_arg()),
        }
        match &self.cascade_stuff {
            Some(m) => node.set_material_override(m),
            None => node.set_material_override(Gd::<godot::classes::Material>::null_arg()),
        }
        self.apply_fall_layers();
    }
    fn apply_fall_layers(&mut self) {
        let mask = globals::meta_mask();
        let flags = object_flags(1, false, 0, false);
        let layers = self.layers | mask;
        if let Some(n) = &mut self.falls_node
            && n.is_instance_valid()
        {
            n.set_layer_mask(layers | mask);
            n.set_instance_shader_parameter("pt_flags", &flags.to_variant());
        }
    }
    fn reassemble(&mut self) {
        if self.stuff.is_none() {
            self.stuff = Some(shader_material("water.gdshader"));
        }
        let Some(mut node) = self.live_surface() else {
            return;
        };
        node.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        let mut mesh = node
            .get_mesh()
            .and_then(|m| m.try_cast::<PlaneMesh>().ok())
            .unwrap_or_else(PlaneMesh::new_gd);
        mesh.set_size(Vector2::new(
            self.size.x.max(0.0105),
            self.size.y.max(0.0105),
        ));
        let sub = self.subdivisions.clamp(0, 256);
        mesh.set_subdivide_width(sub);
        mesh.set_subdivide_depth(sub);
        node.set_mesh(&mesh);
        match &self.stuff {
            Some(m) => node.set_material_override(m),
            None => node.set_material_override(Gd::<godot::classes::Material>::null_arg()),
        }
        self.refresh_pixel_settings();
    }
}
#[godot_api]
impl PixelWater {
    #[func]
    fn get_terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.live_terrain()
    }
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::Water) {
            self.falls_dirty = true;
        } else {
            globals::assign("pt_water_a", &Vector4::ZERO.to_variant());
        }
    }
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = globals::meta_mask();
        self.layers &= !(self.applied_mask | mask);
        if self.layers == 0 {
            self.layers = 1;
        }
        self.applied_mask = mask;
        let flags = object_flags(1, false, 0, false);
        let layers = self.layers | mask;
        if let Some(mut node) = self.live_surface() {
            node.set_layer_mask(layers);
            node.set_instance_shader_parameter("pt_flags", &flags.to_variant());
        }
        self.apply_fall_layers();
    }
    #[func]
    fn set_layers(&mut self, v: u32) {
        self.layers = v;
        if self.base().is_inside_tree() {
            self.refresh_pixel_settings();
        }
    }
    #[func]
    pub fn get_surface(&self) -> Option<Gd<MeshInstance3D>> {
        self.live_surface()
    }
    #[func]
    fn set_surface(&mut self, v: Option<Gd<MeshInstance3D>>) {
        self.surface = v;
        if self.base().is_inside_tree() {
            self.reassemble();
        }
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn _on_terrain_rebuilt(&mut self, _chunks: Array<Vector2i>) {
        self.falls_dirty = true;
    }
    #[func]
    pub fn set_cell_water(&mut self, cell: Vector2i, level: f32) {
        let mut v: Vec<Vector3> = self
            .water_cells
            .as_slice()
            .iter()
            .copied()
            .filter(|p| !same_cell(*p, cell))
            .collect();
        v.push(Vector3::new(cell.x as f32, cell.y as f32, level));
        self.water_cells = v.into_iter().collect();
        self.falls_dirty = true;
    }
    #[func]
    pub fn clear_cell_water(&mut self, cell: Vector2i) {
        self.water_cells = self
            .water_cells
            .as_slice()
            .iter()
            .copied()
            .filter(|p| !same_cell(*p, cell))
            .collect();
        self.falls_dirty = true;
    }
    #[func]
    pub fn get_waterfalls(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        for f in &self.fall_info {
            let mut d = VarDictionary::new();
            d.set("cell", Vector2i::new(f.cell.x, f.cell.y));
            d.set("neighbor", Vector2i::new(f.neighbor.x, f.neighbor.y));
            d.set("top", f.top);
            d.set("bottom", f.bottom);
            d.set("waterfall", f.waterfall);
            d.set("into_water", f.into_water);
            out.push(&d);
        }
        out
    }
    #[func]
    pub fn update_waterfalls(&mut self) {
        self.rebuild_falls();
    }
    #[func]
    fn set_terrain(&mut self, v: Option<Gd<MarchingSquaresTerrain>>) {
        self.terrain = v;
        self.falls_dirty = true;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_water_cells(&mut self, v: PackedVector3Array) {
        self.water_cells = v;
        self.falls_dirty = true;
    }
    #[func]
    fn set_waterfall_min_levels(&mut self, v: i32) {
        self.waterfall_min_levels = v.max(1);
        self.falls_dirty = true;
    }
    #[func]
    fn set_waterfall_level_height(&mut self, v: f32) {
        self.waterfall_level_height = v.max(0.0);
        self.falls_dirty = true;
    }
    #[func]
    fn assign_cascade_stuff(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.cascade_stuff = v;
        self.falls_dirty = true;
    }
    #[func]
    fn assign_measure(&mut self, v: Vector2) {
        self.size = v;
        if self.base().is_inside_tree() {
            self.reassemble();
        }
    }
    #[func]
    fn assign_refinements(&mut self, v: i32) {
        self.subdivisions = v.clamp(0, 256);
        if self.base().is_inside_tree() {
            self.reassemble();
        }
    }
    #[func]
    fn assign_stuff(&mut self, v: Option<Gd<ShaderMaterial>>) {
        self.stuff = v;
        if self.base().is_inside_tree() {
            self.reassemble();
        }
    }
}
