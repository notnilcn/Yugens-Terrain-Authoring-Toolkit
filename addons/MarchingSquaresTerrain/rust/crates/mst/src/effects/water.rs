//! `PixelWater` — a banded water plane: Gerstner swell, sky reflection band, shallow turquoise,
//! caustics (also projected onto the terrain) and pulsing shoreline foam. With raised water
//! cells it also draws pools and animated waterfalls on a hex / triangle `MarchingSquaresTerrain`
//! (`effects::waterfall`, `waterfall.gdshader`).
//!
//! `PixelWater` is a plain `Node3D` that owns its meshes as *internal* children (the plane
//! `Surface` and the `Waterfalls`), so the inspector shows only water settings and Godot's
//! MeshInstance3D "Mesh" menu does not appear. Scenes saved while it was a `MeshInstance3D`
//! still load: the old `mesh` / `material_override` / `cast_shadow` /
//! `instance_shader_parameters/*` entries are accepted and dropped (`on_set`), `layers`
//! is kept as the water's render layers.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::object::ConnectFlags;
use godot::classes::camera_3d::ProjectionType;
use godot::classes::Camera3D;
use godot::classes::{INode3D, MeshInstance3D, Node, Node3D, PlaneMesh, ShaderMaterial};
use godot::prelude::*;

use crate::features::Feature;
use crate::globals;
use crate::util::*;
use crate::writes::ShaderParams;

/// A water plane for the pixel renderer. Depth bands, shoreline foam and sparkles are
/// derived from the terrain height texture published by `MarchingSquaresTerrain`, so it needs no
/// depth buffer. Place it at the water level (its Y position); terrain below that height
/// is flooded.
///
/// The water takes part in the metadata pass (so it gets crisp silhouettes against
/// terrain) but draws no outline of its own.
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelWater {
    /// Plane size in world units (centred on the node).
    #[export]
    #[var(set = assign_measure)]
    size: Vector2,
    /// Grid subdivisions per side (more = smoother waves).
    #[export(range = (0.0, 256.0, 1.0))]
    #[var(set = assign_refinements)]
    subdivisions: i32,
    /// Water material; created with `water.gdshader` when empty. Edit its shader
    /// parameters to change colors, foam and waves.
    #[export]
    #[var(set = assign_stuff)]
    stuff: Option<Gd<ShaderMaterial>>,
    /// Render layers of the water plane and waterfalls (the pixel renderer's metadata layer is
    /// added automatically). Same name as the old MeshInstance3D property, so saved scenes keep it.
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,
    /// Keep the plane centred under the camera's view (moved in whole grid cells, so the
    /// world-space waves and foam stay put): a sea that never shows its edge. Leave off for
    /// lakes and ponds that must stay where they are placed.
    #[export]
    follow_camera: bool,
    /// Fraction of the plane's half size over which its rim fades into the horizon colour
    /// (the sky published by PixelDayNight, else the material's `horizon_color`); 0 = a hard
    /// edge. Pair with `follow_camera` for an open sea.
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_fade: f32,

    #[export_group(name = "Caustics", prefix = "caustics_")]
    /// Brightness of the banded caustic web in the shallows and on nearby terrain (0 = off).
    #[export(range = (0.0, 2.0, 0.0105))]
    caustics_strength: f32,
    /// Caustic pattern frequency per world unit.
    #[export(range = (0.0505, 4.0, 0.0105))]
    caustics_scale: f32,
    /// Brightness steps of the caustic lines.
    #[export(range = (1.0, 6.0, 1.0))]
    caustics_bands: i32,
    /// How far above the water line the terrain still catches reflected caustics (m).
    #[export(range = (0.0, 4.0, 0.0105))]
    caustics_reach: f32,

    #[export_group(name = "Waterfalls")]
    /// Terrain the raised water sits on (a hex / triangle `MarchingSquaresTerrain`); empty =
    /// the first one in the scene.
    #[export]
    #[var(get = get_terrain, set = set_terrain)]
    terrain: Option<Gd<Node>>,
    /// Raised water (pools, rivers, lakes above the sea): one `(q, r, surface height)` per cell,
    /// heights in terrain-local units. Together with the sea plane this is one continuous
    /// water sheet; wherever it is higher than a neighbour's surface the water spills over the
    /// shared edge. Also editable with `set_cell_water` / `clear_cell_water`.
    #[export]
    #[var(set = set_water_cells)]
    water_cells: PackedVector3Array,
    // Subgroup prefix: the inspector shows "Min Levels" / "Level Height" / "Material" instead
    // of cut-off "Waterfall M…" labels (saved names unchanged).
    #[export_subgroup(name = "Falls", prefix = "waterfall_")]
    /// Drops of at least this many levels become waterfalls (foam pool, spray); smaller ones
    /// are short cascades.
    #[export(range = (1.0, 64.0, 1.0))]
    #[var(set = set_waterfall_min_levels)]
    waterfall_min_levels: i32,
    /// Height of one level for `waterfall_min_levels` (0 = the terrain's `level_height`).
    #[export(range = (0.0, 10.0, 0.0105, or_greater))]
    #[var(set = set_waterfall_level_height)]
    waterfall_level_height: f32,
    /// Material of the pools / waterfalls; created with `waterfall.gdshader` when empty.
    #[export]
    #[var(set = assign_cascade_stuff)]
    cascade_stuff: Option<Gd<ShaderMaterial>>,

    /// `follow_camera`: the plane is scaled by this power of two so it covers the view out to
    /// where the camera sees the water (runtime only).
    follow_scale: f32,
    falls_dirty: bool,
    falls_node: Option<Gd<MeshInstance3D>>,
    /// Internal plane mesh (never saved).
    skin: Option<Gd<MeshInstance3D>>,
    connected_terrain: Option<Gd<Node>>,
    fall_info: Vec<pt_core::waterfall::Fall>,
    applied_mask: u32,
    base: Base<Node3D>,
}

/// Properties of the old `MeshInstance3D`-based PixelWater still found in saved scenes; the node
/// rebuilds its meshes and material itself, so they are accepted and ignored.
/// Largest power-of-two growth of a `follow_camera` plane (a low orthographic orbit sees the
/// water out to the far plane).
const MAX_FOLLOW_SCALE: f32 = 64.0;

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
            follow_scale: 1.0,
            falls_dirty: true,
            falls_node: None,
            skin: None,
            connected_terrain: None,
            fall_info: Vec::new(),
            applied_mask: 0,
            layers: 1,
            follow_camera: false,
            rim_fade: 0.0,
            base,
        }
    }

    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        // Compatibility with scenes saved when PixelWater was a MeshInstance3D.
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
        // Water level and caustics for the terrain/grass shaders (pixel_weather.gdshaderinc).
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
        if visible {
            self.place_plane();
        }
    }

    fn exit_tree(&mut self) {
        globals::assign("pt_water_a", &Vector4::ZERO.to_variant());
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        super::gate_warnings(Feature::Water)
    }
}

fn same_cell(p: Vector3, c: Vector2i) -> bool {
    p.x.round() as i32 == c.x && p.y.round() as i32 == c.y
}

impl PixelWater {
    /// The terrain the raised water belongs to (exported, else the first in the scene); keeps
    /// a (deferred) connection to its `chunks_rebuilt` signal so edits re-mesh the falls.
    fn falls_terrain(&mut self) -> Option<Gd<Node>> {
        let t = match &self.terrain {
            Some(t) if t.is_instance_valid() => Some(t.clone()),
            _ => super::find_terrain(&self.to_gd().upcast()),
        }?;
        if self.connected_terrain.as_ref() != Some(&t) {
            let cb = self.to_gd().callable("_on_terrain_rebuilt");
            if let Some(mut old) = self.connected_terrain.take()
                && old.is_instance_valid()
                && old.is_connected("chunks_rebuilt", &cb)
            {
                old.disconnect("chunks_rebuilt", &cb);
            }
            let mut tt = t.clone();
            if !tt.is_connected("chunks_rebuilt", &cb) {
                tt.connect_flags("chunks_rebuilt", &cb, ConnectFlags::DEFERRED);
            }
            self.connected_terrain = Some(t.clone());
        }
        Some(t)
    }

    /// Re-finds the falls and rebuilds the pool / waterfall mesh (an internal child).
    fn rebuild_falls(&mut self) {
        let _p = crate::prof::breadth("water.rebuild_falls");
        if !self.base().is_inside_tree() || !crate::features::active(Feature::Water) {
            return;
        }
        self.falls_dirty = false;
        // The terrain culls grass under raised water: let it re-read the cells (deferred, as
        // it reads this node's `water_cells` back).
        let grass_terrain = match &self.terrain {
            Some(t) if t.is_instance_valid() => Some(t.clone()),
            _ => super::find_terrain(&self.to_gd().upcast()),
        };
        if let Some(mut t) = grass_terrain
            && t.has_method("refresh_grass_water")
        {
            t.call_deferred("refresh_grass_water", &[]);
        }
        // Raised cells: this node's `water_cells` plus the water levels saved in the terrain
        // data (`set_cell_water_levels`); the node's own entries win.
        let ft = self.falls_terrain();
        let mut cells: Vec<Vector3> = self.water_cells.as_slice().to_vec();
        if let Some(t) = &ft
            && t.has_method("get_raised_water_cells")
            && let Ok(stored) = t
                .clone()
                .call("get_raised_water_cells", &[])
                .try_to::<PackedVector3Array>()
        {
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
            let t3 = t.clone().try_cast::<godot::classes::Node3D>().ok()?;
            let ty = t3.get_global_position().y;
            let level = if self.waterfall_level_height > 0.0 {
                self.waterfall_level_height
            } else {
                t.get("level_height")
                    .try_to::<f32>()
                    .unwrap_or(1.0)
                    .abs()
                    .max(1e-3)
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
                // Internal child (no owner): never saved with the scene.
                add_internal(&mut self.to_gd(), &n);
                self.falls_node = Some(n.clone());
                n
            }
        };
        crate::writes::set_global_transform(&mut node, t3.get_global_transform());
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

    /// The waterfall mesh joins the metadata pass like the water plane (no outline of its own).
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

    /// The internal plane mesh node (created on first use).
    fn skin(&mut self) -> Gd<MeshInstance3D> {
        if let Some(s) = &self.skin
            && s.is_instance_valid()
        {
            return s.clone();
        }
        let mut n = MeshInstance3D::new_alloc();
        n.set_name("Surface");
        n.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        add_internal(&mut self.to_gd(), &n);
        self.skin = Some(n.clone());
        n
    }

    /// `follow_camera`: moves the plane (in whole grid cells of the node's lattice) under the
    /// point the camera looks at on the water; publishes the plane rect for the rim fade.
    /// Change-only writes.
    fn place_plane(&mut self) {
        let mut skin = self.skin();
        let node_xf = self.base().get_global_transform();
        let mut offset = Vector3::ZERO;
        let mut scale_k = 1.0f32;
        let mut view_far = 0.0f32;
        if self.follow_camera {
            let me: Gd<Node> = self.to_gd().upcast();
            if let Some(cam) = super::active_camera(&me) {
                view_far = cam.get_far();
                let focus = super::camera_focus(&cam, node_xf.origin.y);
                // Ground footprint of the view: where the four corner rays meet the water (or
                // the far plane, for rays that pass over it), plus the rim's fade band. Grow the
                // plane in powers of two, so its lattice changes only on large zoom steps. The
                // old orthographic-only estimate (tilt floored at 0.25) left a low orbit, or any
                // perspective camera, looking past the plane's edge into the void.
                let reach = Self::view_reach(&cam, node_xf.origin.y, focus);
                let half = 0.5 * self.size.x.min(self.size.y).max(0.0105);
                let need = reach / (1.0 - self.rim_fade.clamp(0.0, 0.9));
                while half * scale_k < need && scale_k < MAX_FOLLOW_SCALE {
                    scale_k *= 2.0;
                }
                let local = node_xf.affine_inverse() * focus;
                let cells = (self.subdivisions.clamp(0, 256) + 1) as f32;
                let step = Vector2::new(
                    (self.size.x * scale_k / cells).max(0.0105),
                    (self.size.y * scale_k / cells).max(0.0105),
                );
                offset = Vector3::new(
                    (local.x / step.x).round() * step.x,
                    0.0,
                    (local.z / step.y).round() * step.y,
                );
            }
        }
        if scale_k != self.follow_scale {
            self.follow_scale = scale_k;
            self.reassemble();
        }
        if skin.get_position() != offset {
            skin.set_position(offset);
        }
        if let Some(m) = &mut self.stuff {
            let centre = node_xf * offset;
            let scale = node_xf.basis.get_scale();
            let rect = Vector4::new(
                centre.x,
                centre.z,
                self.size.x * scale_k * 0.5 * scale.x.abs(),
                self.size.y * scale_k * 0.5 * scale.z.abs(),
            );
            m.param("plane_rect", &rect.to_variant());
            m.param("rim_fade", &self.rim_fade.clamp(0.0, 1.0).to_variant());
            m.param("view_far", &view_far.to_variant());
        }
    }

    /// Largest horizontal distance from `focus` to where the view's corner rays meet the plane
    /// `y = level`. Orthographic: a ray that misses it, or meets it past the far plane, counts
    /// to the far plane (an ortho view has no horizon: past the plane is the void).
    /// Perspective: rays over the horizon are skipped (the rim fades into the sky behind it
    /// there) and hits count to a quarter of the far plane at most, so the grid stays fine.
    fn view_reach(cam: &Gd<Camera3D>, level: f32, focus: Vector3) -> f32 {
        let ortho = cam.get_projection() == ProjectionType::ORTHOGONAL;
        let size = cam
            .get_viewport()
            .map(|vp| vp.get_visible_rect().size)
            .unwrap_or(Vector2::new(1920.0, 1080.0));
        let far = if ortho {
            cam.get_far().max(1.0)
        } else {
            (cam.get_far() * 0.25).max(1.0)
        };
        let mut reach = 0.0f32;
        for p in [
            Vector2::ZERO,
            Vector2::new(size.x, 0.0),
            Vector2::new(0.0, size.y),
            size,
            Vector2::new(size.x * 0.5, 0.0),
        ] {
            let o = cam.project_ray_origin(p);
            let d = cam.project_ray_normal(p);
            let t = if d.y < -1e-4 {
                ((level - o.y) / d.y).clamp(0.0, far)
            } else if ortho {
                far
            } else {
                continue;
            };
            let h = o + d * t;
            reach = reach.max(Vector2::new(h.x - focus.x, h.z - focus.z).length());
        }
        if reach.is_finite() { reach } else { far }
    }

    /// Updates the plane mesh and material.
    fn reassemble(&mut self) {
        if self.stuff.is_none() {
            self.stuff = Some(shader_material("water.gdshader"));
        }
        let mut node = self.skin();
        let mut mesh = node
            .get_mesh()
            .and_then(|m| m.try_cast::<PlaneMesh>().ok())
            .unwrap_or_else(PlaneMesh::new_gd);
        let k = if self.follow_camera { self.follow_scale } else { 1.0 };
        mesh.set_size(Vector2::new(
            (self.size.x * k).max(0.0105),
            (self.size.y * k).max(0.0105),
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
    /// `terrain`, or `None` once that node was freed (`util::live`).
    #[func]
    fn get_terrain(&self) -> Option<Gd<Node>> {
        crate::util::live(&self.terrain)
    }

    /// Water feature gate (`features::CALLBACK`): off = no surfaces, falls or caustics.
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if super::gate(&self.to_gd(), Feature::Water) {
            self.falls_dirty = true;
        } else {
            globals::assign("pt_water_a", &Vector4::ZERO.to_variant());
        }
    }

    /// Re-applies the metadata layer and per-object flags (called when the meta layer changes).
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = globals::meta_mask();
        // `layers` of old scenes includes the metadata bit that was applied then: drop it.
        self.layers &= !(self.applied_mask | mask);
        if self.layers == 0 {
            self.layers = 1;
        }
        self.applied_mask = mask;
        let flags = object_flags(1, false, 0, false);
        let layers = self.layers | mask;
        let mut node = self.skin();
        node.set_layer_mask(layers);
        node.set_instance_shader_parameter("pt_flags", &flags.to_variant());
        self.apply_fall_layers();
    }

    #[func]
    fn set_layers(&mut self, v: u32) {
        self.layers = v;
        if self.base().is_inside_tree() {
            self.refresh_pixel_settings();
        }
    }

    /// The internal plane mesh instance (read-only access for tools and tests).
    #[func]
    pub fn get_surface(&mut self) -> Option<Gd<MeshInstance3D>> {
        if self.base().is_inside_tree() {
            Some(self.skin())
        } else {
            None
        }
    }

    #[func]
    fn _on_terrain_rebuilt(&mut self, _chunks: Array<Vector2i>) {
        self.falls_dirty = true;
    }

    /// Raises the water of `cell` to `level` (terrain-local height).
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

    /// Removes the raised water of `cell`.
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

    /// The edges where raised water spills over: `{cell, neighbor, top, bottom, waterfall,
    /// into_water}` (heights terrain-local). Rebuilt on the next frame after edits;
    /// `update_waterfalls()` forces it now.
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

    /// Re-finds the falls and rebuilds the pool / waterfall mesh now.
    #[func]
    pub fn update_waterfalls(&mut self) {
        self.rebuild_falls();
    }

    #[func]
    fn set_terrain(&mut self, v: Option<Gd<Node>>) {
        self.terrain = v;
        self.falls_dirty = true;
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
