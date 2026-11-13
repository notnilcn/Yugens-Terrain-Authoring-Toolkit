//! `PixelBuilderMesh` — a `MeshInstance3D` whose mesh is compiled from editable polygon data.
//!
//! Being a real `MeshInstance3D` means `PixelObject` pixelizes it like any other mesh (toon
//! shading, outlines, metadata). The compiled mesh is never saved (the `mesh` property loses
//! its STORAGE flag); it is rebuilt from `data` on load and whenever the data changes.

use godot::classes::base_material_3d::Flags;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::notify::Node3DNotification;
use godot::classes::object::ConnectFlags;
use godot::classes::{
    Area3D, ArrayMesh, CollisionObject3D, CollisionShape3D, ConcavePolygonShape3D, ConvexPolygonShape3D, IMeshInstance3D, Material, MeshInstance3D,
    StandardMaterial3D, StaticBody3D,
};
use godot::register::info::{PropertyInfo, PropertyUsageFlags};
use godot::prelude::*;
use pt_core::build::compile::{CompileOptions, Compiled, compile};
use pt_core::build::snap;
use pt_core::math::v2;

use super::data::{PixelBuilderMeshData, to_p};

const EDIT_LOCK_META: &str = "_edit_lock_";
/// Lightmap UV2 stamp metas on an unwrapped mesh (Unibuilder `UV2_TEXEL_META` / `UV2_SCALE_META`).
const UV2_TEXEL_META: &str = "pt_uv2_texel";
const UV2_SCALE_META: &str = "pt_uv2_scale";
use crate::terrain::MarchingSquaresTerrain;
use crate::util::*;

#[derive(GodotClass)]
#[class(tool, base = MeshInstance3D)]
pub struct PixelBuilderMesh {
    /// Editable polygon data.
    #[export]
    #[var(set = set_data)]
    data: Option<Gd<PixelBuilderMeshData>>,
    /// Builds a static trimesh collider at runtime.
    #[export]
    collision: bool,
    #[export(flags_3d_physics)]
    collision_layer: u32,
    /// Terrain cell this mesh was placed on (`place_on_cell`); `resnap` re-seats it after terrain edits.
    #[export]
    anchor_cell: Vector2i,
    #[export]
    anchored: bool,
    /// No terrain grass grows through the mesh's floor (its downward faces at ground level), and
    /// small-hex decorations under it are blocked (hex terrains).
    #[export]
    #[var(set = set_clear_grass)]
    clear_grass: bool,
    /// Unibuilder entity type: Normal (visible mesh), Collider (invisible at runtime, static
    /// trimesh collider) or Trigger (invisible at runtime, convex `Area3D`). Colliders and
    /// triggers are tinted orange / blue in the editor.
    /// Live lightmap UV2 (Unibuilder `live_uv2`): in the editor the compiled mesh gets UV2
    /// unwrapped at `lightmap_texel` half a second after each edit (and before saving), and is
    /// saved with the scene, so `LightmapGI` bakes it without *Bake to MeshInstance3D*; the game
    /// keeps that saved mesh (its UV2 matches the baked lightmap).
    #[export]
    #[var(set = set_live_uv2)]
    live_uv2: bool,
    #[export(range = (0.01, 4.0, 0.01))]
    lightmap_texel: f32,
    /// Debounce generation of the pending live UV2 unwrap.
    uv2_gen: u32,
    #[export(enum = (Normal = 0, Collider = 1, Trigger = 2))]
    #[var(set = set_entity_type)]
    entity_type: i32,
    /// World XZ floor polygons last sent to the terrain, and the small hexes this mesh blocked.
    footprint_sent: Vec<Vec<Vector3>>,
    blocked_smalls: Array<Vector2i>,

    default_mat: Option<Gd<StandardMaterial3D>>,
    body: Option<Gd<CollisionObject3D>>,
    rebuild_queued: bool,
    /// Surface index -> material slot of the last compile.
    surface_slots: Vec<u16>,
    /// The builder's gizmo lock was removed for a save and goes back after it.
    lock_stripped_for_save: bool,
    base: Base<MeshInstance3D>,
}

#[godot_api]
impl IMeshInstance3D for PixelBuilderMesh {
    fn init(base: Base<MeshInstance3D>) -> Self {
        PixelBuilderMesh {
            data: None,
            collision: true,
            collision_layer: 1,
            anchor_cell: Vector2i::ZERO,
            anchored: false,
            clear_grass: true,
            entity_type: 0,
            live_uv2: false,
            lightmap_texel: 0.2,
            uv2_gen: 0,
            footprint_sent: Vec::new(),
            blocked_smalls: Array::new(),
            default_mat: None,
            body: None,
            rebuild_queued: false,
            surface_slots: Vec::new(),
            lock_stripped_for_save: false,
            base,
        }
    }

    fn ready(&mut self) {
        // A builder gizmo lock that survived (crash, external save): drop it (Unibuilder's
        // `_heal_stale_plugin_lock`).
        if self.base().has_meta(super::plugin::PLUGIN_LOCK_META) {
            self.base_mut().remove_meta(super::plugin::PLUGIN_LOCK_META);
            self.base_mut().remove_meta(EDIT_LOCK_META);
        }
        if !is_editor() && self.entity_type != 0 {
            // Colliders and triggers are invisible in the game (`ub_mesh_instance.gd::_ready`).
            self.base_mut().set_visible(false);
        }
        if self.data.is_none() && is_editor() {
            let d = PixelBuilderMeshData::new_gd();
            self.set_data(Some(d));
        }
        if is_editor() {
            // World-space UVs follow the node while it is being moved around.
            self.base_mut().set_notify_transform(true);
        }
        self.rebuild();
    }

    fn exit_tree(&mut self) {
        // Give the ground back: grass regrows and blocked small hexes are freed.
        if !self.footprint_sent.is_empty() || !self.blocked_smalls.is_empty() {
            self.footprint_sent.clear();
            self.notify_terrain_footprint();
        }
    }

    fn on_notification(&mut self, what: Node3DNotification) {
        match what {
            Node3DNotification::TRANSFORM_CHANGED => self.queue_rebuild(),
            // Never save the builder's gizmo lock (Unibuilder `ub_mesh_instance.gd`
            // EDITOR_PRE_SAVE / POST_SAVE): strip it for the save, put it back afterwards.
            Node3DNotification::EDITOR_PRE_SAVE => {
                if self.base().has_meta(super::plugin::PLUGIN_LOCK_META) {
                    self.base_mut().remove_meta(super::plugin::PLUGIN_LOCK_META);
                    self.base_mut().remove_meta(EDIT_LOCK_META);
                    self.lock_stripped_for_save = true;
                }
                // Live UV2: make sure the saved mesh carries a current unwrap.
                if self.live_uv2 {
                    self.uv2_gen = self.uv2_gen.wrapping_add(1);
                    let g = self.uv2_gen;
                    self.apply_live_uv2(g);
                }
                // The editor-only entity tint is never saved either.
                if self.base().get_material_override().is_some_and(|m| m.has_meta("pt_entity_tint")) {
                    self.base_mut().set_material_override(Gd::<Material>::null_arg());
                }
            }
            Node3DNotification::EDITOR_POST_SAVE => {
                if std::mem::take(&mut self.lock_stripped_for_save) {
                    self.base_mut().set_meta(EDIT_LOCK_META, &true.to_variant());
                    self.base_mut().set_meta(super::plugin::PLUGIN_LOCK_META, &true.to_variant());
                }
                self.refresh_entity_display();
            }
            _ => {}
        }
    }

    fn on_validate_property(&self, property: &mut PropertyInfo) {
        // With live UV2 the unwrapped mesh is saved (it must match the baked lightmap).
        if property.property_name == StringName::from("mesh") && !self.live_uv2 {
            property.usage = PropertyUsageFlags::from_ord(property.usage.ord() & !PropertyUsageFlags::STORAGE.ord());
        }
    }
}

impl PixelBuilderMesh {
    pub fn data(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.data.clone()
    }

    fn world_footprints(&self) -> Vec<Vec<Vector3>> {
        if !self.clear_grass || !self.base().is_inside_tree() {
            return Vec::new();
        }
        let Some(d) = &self.data else { return Vec::new() };
        let xf = self.base().get_global_transform();
        pt_core::build::footprint::floor_footprints(d.bind().mesh(), 0.35)
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| xf * super::data::from_p(p)).collect())
            .collect()
    }

    /// Re-sends the floor footprint to the terrain when it changed (moved / edited mesh).
    fn sync_footprint(&mut self) {
        let now = self.world_footprints();
        let same = now.len() == self.footprint_sent.len()
            && now.iter().zip(&self.footprint_sent).all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.distance_to(*q) < 1e-3));
        if same {
            return;
        }
        self.footprint_sent = now;
        self.notify_terrain_footprint();
    }

    /// Asks the terrain to re-cull its grass (deferred: never call back into it from here) and
    /// moves this mesh's blocked small hexes to the current footprint.
    fn notify_terrain_footprint(&mut self) {
        let node = self.to_gd().upcast::<Node>();
        let Some(mut t) = crate::effects::find_terrain(&node) else { return };
        t.call_deferred("refresh_grass_water", &[]);
        let hex = matches!(t.get("grid_type").try_to::<i64>(), Ok(2 | 3));
        if !hex {
            return;
        }
        if !self.blocked_smalls.is_empty() {
            t.call_deferred("set_small_hexes_blocked", &[self.blocked_smalls.to_variant(), false.to_variant()]);
        }
        let mut smalls: Array<Vector2i> = Array::new();
        let mut seen = std::collections::HashSet::new();
        for poly in &self.footprint_sent {
            let pts: Vec<pt_core::math::Vec2> = poly.iter().map(|p| v2(p.x, p.z)).collect();
            let (lo, hi) = pts.iter().fold((v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN)), |(a, b), p| (v2(a.x.min(p.x), a.y.min(p.y)), v2(b.x.max(p.x), b.y.max(p.y))));
            let y = poly[0].y;
            let mut x = lo.x;
            while x <= hi.x {
                let mut z = lo.y;
                while z <= hi.y {
                    let s = t.call("world_to_small_hex", &[Vector3::new(x, y, z).to_variant()]).to::<Vector2i>();
                    if seen.insert((s.x, s.y)) {
                        let c = t.call("small_hex_to_world", &[s.to_variant(), y.to_variant()]).to::<Vector3>();
                        if point_in_poly(v2(c.x, c.z), &pts) {
                            smalls.push(s);
                        }
                    }
                    z += 0.15;
                }
                x += 0.15;
            }
        }
        if !smalls.is_empty() {
            t.call_deferred("set_small_hexes_blocked", &[smalls.to_variant(), true.to_variant()]);
        }
        self.blocked_smalls = smalls;
    }
}

fn point_in_poly(p: pt_core::math::Vec2, poly: &[pt_core::math::Vec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

#[godot_api]
impl PixelBuilderMesh {
    #[func]
    pub fn set_data(&mut self, v: Option<Gd<PixelBuilderMeshData>>) {
        let cb = self.to_gd().callable("queue_rebuild");
        if let Some(old) = &mut self.data
            && old.is_connected("changed", &cb)
        {
            old.disconnect("changed", &cb);
        }
        self.data = v;
        if let Some(d) = &mut self.data {
            d.connect_flags("changed", &cb, ConnectFlags::DEFERRED);
        }
        self.queue_rebuild();
    }

    #[func]
    fn set_live_uv2(&mut self, v: bool) {
        self.live_uv2 = v;
        self.base_mut().notify_property_list_changed();
        if v {
            self.schedule_live_uv2();
        } else {
            self.queue_rebuild();
        }
    }

    /// `_schedule_live_uv2`: unwraps 0.5 s after the last edit (each edit restarts the wait).
    fn schedule_live_uv2(&mut self) {
        if !self.live_uv2 || !is_editor() || !self.base().is_inside_tree() {
            return;
        }
        self.uv2_gen = self.uv2_gen.wrapping_add(1);
        let mut tree = self.base().get_tree();
        let mut t = tree.create_timer(0.5);
        let cb = self.to_gd().callable("apply_live_uv2").bind(&[self.uv2_gen.to_variant()]);
        t.connect("timeout", &cb);
    }

    /// `_apply_live_uv2`: swaps in the unwrapped copy if this is still the latest request.
    #[func]
    fn apply_live_uv2(&mut self, generation: u32) {
        if generation != self.uv2_gen || !self.live_uv2 {
            return;
        }
        let texel = self.lightmap_texel.max(0.01);
        if let Some(m) = self.unwrapped_mesh(texel) {
            let same = self.base().get_mesh().is_some_and(|c| c.instance_id() == m.instance_id());
            if !same {
                self.base_mut().set_mesh(&m);
            }
        }
    }

    #[func]
    fn set_entity_type(&mut self, v: i32) {
        self.entity_type = v.clamp(0, 2);
        self.refresh_entity_display();
        self.queue_rebuild();
    }

    #[func]
    fn set_clear_grass(&mut self, v: bool) {
        self.clear_grass = v;
        self.sync_footprint();
    }

    /// Floor polygons (world XZ, as Vector3 with the floor height) where terrain grass does not
    /// grow: the downward faces within 0.35 m of the mesh's lowest point. Empty with
    /// `clear_grass` off. Read by `MarchingSquaresTerrain` when it places grass.
    #[func]
    pub fn get_grass_footprints(&self) -> Array<PackedVector3Array> {
        self.world_footprints().into_iter().map(|p| p.into_iter().collect::<PackedVector3Array>()).collect()
    }

    /// Schedules a rebuild at the end of the frame (coalesces many edits).
    #[func]
    pub fn queue_rebuild(&mut self) {
        if self.rebuild_queued || !self.base().is_inside_tree() {
            return;
        }
        self.rebuild_queued = true;
        self.to_gd().call_deferred("rebuild", &[]);
    }

    /// Compiles the polygon data into the displayed mesh (and collider at runtime).
    #[func]
    pub fn rebuild(&mut self) {
        self.rebuild_queued = false;
        let Some(data) = self.data.clone() else {
            self.base_mut().set_mesh(Gd::<godot::classes::Mesh>::null_arg());
            return;
        };
        let origin = if self.base().is_inside_tree() { self.base().get_global_position() } else { self.base().get_position() };
        let (compiled, slots) = {
            let d = data.bind();
            let opt = CompileOptions { units_per_repeat: d.units(), world_offset: to_p(origin) };
            (compile(d.mesh(), &opt), d.material_slots())
        };
        if !is_editor() && self.live_uv2 && self.base().get_mesh().is_some_and(|m| mesh_has_uv2(&m)) {
            // The saved, lightmap-unwrapped mesh stays (`ub_mesh_instance.gd::rebuild`).
            self.sync_footprint();
            self.update_collision(&compiled);
            return;
        }
        let mesh = self.upload(&compiled, &slots);
        self.sync_footprint();
        if is_editor() && self.live_uv2 {
            self.schedule_live_uv2();
        }
        let had = self.base().get_mesh().map(|m| m.get_surface_count()).unwrap_or(0);
        match &mesh {
            Some(m) => self.base_mut().set_mesh(m),
            None => self.base_mut().set_mesh(Gd::<godot::classes::Mesh>::null_arg()),
        }
        if !is_editor() {
            self.update_collision(&compiled);
            let now = mesh.map(|m| m.get_surface_count()).unwrap_or(0);
            if now != had || had == 0 {
                for i in 0..now {
                    self.base_mut().set_surface_override_material(i, Gd::<Material>::null_arg());
                }
                self.notify_pixel_object();
            }
        }
    }

    /// Material slot used by compiled surface `surface` (-1 if none).
    #[func]
    pub fn get_surface_slot(&self, surface: i64) -> i64 {
        self.surface_slots.get(surface as usize).map(|s| *s as i64).unwrap_or(-1)
    }

    /// Copy of the compiled mesh with lightmap UV2 unwrapped at `texel_size` (Godot's
    /// `ArrayMesh.lightmap_unwrap`, as in Unibuilder's `_unwrapped_copy`). Null when the mesh
    /// is empty or the unwrap failed.
    #[func]
    pub fn unwrapped_mesh(&mut self, texel_size: f32) -> Option<Gd<ArrayMesh>> {
        if self.rebuild_queued || self.base().get_mesh().is_none() {
            self.rebuild();
        }
        let src = self.base().get_mesh()?;
        let xf = if self.base().is_inside_tree() { self.base().get_global_transform() } else { self.base().get_transform() };
        let current = uv2_stamp(&src).is_some_and(|st| {
            pt_core::build::lightmap::uv2_is_current(Some(st), mesh_has_uv2(&src), texel_size, to_p(xf.basis.get_scale()))
        });
        use pt_core::build::lightmap::{Uv2Result, plan_unwrap};
        match plan_unwrap(texel_size, src.get_surface_count() as usize, current, false) {
            Uv2Result::Skipped => return None,
            Uv2Result::Current => return src.try_cast::<ArrayMesh>().ok(),
            _ => {}
        }
        let mut copy = ArrayMesh::new_gd();
        for s in 0..src.get_surface_count() {
            let arrays = src.surface_get_arrays(s);
            copy.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            if let Some(m) = src.surface_get_material(s) {
                copy.surface_set_material(s, &m);
            }
        }
        let err = copy.lightmap_unwrap(xf, texel_size);
        if err != godot::global::Error::OK {
            godot_warn!("PixelBuilderMesh: lightmap unwrap failed on \"{}\" ({err:?})", self.base().get_name());
            return None;
        }
        copy.set_meta(UV2_TEXEL_META, &texel_size.to_variant());
        copy.set_meta(UV2_SCALE_META, &xf.basis.get_scale().to_variant());
        Some(copy)
    }

    /// A plain `MeshInstance3D` with the compiled mesh (lightmap UV2 unwrapped when
    /// `lightmap_texel` > 0), the node's stored properties and a static collider, without any
    /// editing data (Unibuilder `make_baked`).
    #[func(gd_self)]
    pub fn make_baked(this: Gd<Self>, lightmap_texel: f32) -> Gd<MeshInstance3D> {
        // Bound only for the Rust state; the property copy below runs unbound (Godot calls
        // back into `on_validate_property` / the property getters).
        let (mesh, collision) = {
            let mut t = this.clone();
            let mut g = t.bind_mut();
            if g.rebuild_queued || g.base().get_mesh().is_none() {
                g.rebuild();
            }
            let mesh: Option<Gd<godot::classes::Mesh>> = if lightmap_texel > 0.0 {
                g.unwrapped_mesh(lightmap_texel).map(|m| m.upcast()).or_else(|| g.base().get_mesh())
            } else {
                g.base().get_mesh()
            };
            let collision = match (g.collision, g.data.clone()) {
                (true, Some(data)) => {
                    let d = data.bind();
                    let c = compile(d.mesh(), &CompileOptions { units_per_repeat: d.units(), world_offset: to_p(g.base().get_global_position()) });
                    Some((c.collision, g.collision_layer))
                }
                _ => None,
            };
            (mesh, collision)
        };
        let mut baked = MeshInstance3D::new_alloc();
        baked.set_name(&this.get_name());
        if let Some(m) = &mesh {
            baked.set_mesh(m);
        }
        // Copy every stored property the plain MeshInstance3D also has (transform, layers,
        // shadows, GI mode, material override, metadata ...), except our own and the lock.
        let skip = ["script", "mesh", "data", "collision", "collision_layer", "anchor_cell", "anchored"];
        let mine = this.get_property_list();
        let theirs: std::collections::HashSet<String> = baked.get_property_list().iter_shared().filter_map(|d| d.get("name").map(|n| n.to_string())).collect();
        let plugin_lock = this.has_meta(super::plugin::PLUGIN_LOCK_META);
        for p in mine.iter_shared() {
            let (Some(name), Some(usage)) = (p.get("name").map(|n| n.to_string()), p.get("usage").and_then(|u| u.try_to::<i64>().ok())) else { continue };
            if usage & PropertyUsageFlags::STORAGE.ord() as i64 == 0 || skip.contains(&name.as_str()) {
                continue;
            }
            if (name == format!("metadata/{EDIT_LOCK_META}") && plugin_lock) || name == format!("metadata/{}", super::plugin::PLUGIN_LOCK_META) {
                continue;
            }
            if name.starts_with("metadata/") || theirs.contains(&name) {
                baked.set(&name, &this.get(&name));
            }
        }
        if let Some((tris, layer)) = collision
            && !tris.is_empty()
        {
            let mut body = StaticBody3D::new_alloc();
            body.set_name("StaticBody3D");
            body.set_collision_layer(layer);
            let mut cs = CollisionShape3D::new_alloc();
            cs.set_name("CollisionShape3D");
            let mut shape = ConcavePolygonShape3D::new_gd();
            let faces: PackedVector3Array = tris.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
            shape.set_faces(&faces);
            cs.set_shape(&shape);
            body.add_child(&cs);
            baked.add_child(&body);
        }
        baked
    }

    /// Moves the mesh so its origin sits on the centre of a terrain cell at the cell's height.
    #[func]
    pub fn place_on_cell(&mut self, terrain: Gd<MarchingSquaresTerrain>, cell: Vector2i) {
        let Some(p) = cell_world(&terrain, cell) else {
            godot_warn!("PixelBuilderMesh: terrain has no ground at cell {cell}");
            return;
        };
        self.anchor_cell = cell;
        self.anchored = true;
        self.base_mut().set_global_position(p);
    }

    /// Re-seats an anchored mesh on its cell (after the terrain was edited).
    #[func]
    pub fn resnap(&mut self, terrain: Gd<MarchingSquaresTerrain>) {
        if self.anchored {
            let c = self.anchor_cell;
            self.place_on_cell(terrain, c);
        }
    }
}

/// World position of a terrain cell centre at its surface height.
pub fn cell_world(terrain: &Gd<MarchingSquaresTerrain>, cell: Vector2i) -> Option<Vector3> {
    let t = terrain.bind();
    let cfg = &t.store().config;
    let c = to_iv(cell);
    let p = if cfg.grid.is_cell_mode() {
        cfg.sample_pos(c)
    } else {
        let cs = cfg.cell_size_safe();
        v2((c.x as f32 + 0.5) * cs.x, (c.y as f32 + 0.5) * cs.y)
    };
    let h = t.local_height_at(p)?;
    drop(t);
    Some(terrain.get_global_transform() * Vector3::new(p.x, h, p.y))
}

fn mesh_has_uv2(m: &Gd<godot::classes::Mesh>) -> bool {
    m.clone().try_cast::<ArrayMesh>().is_ok_and(|a| a.get_surface_count() > 0 && (a.surface_get_format(0).ord() as i64 & godot::classes::mesh::ArrayFormat::TEX_UV2.ord() as i64) != 0)
}

fn uv2_stamp(m: &Gd<godot::classes::Mesh>) -> Option<pt_core::build::lightmap::Uv2Stamp> {
    if !m.has_meta(UV2_TEXEL_META) || !m.has_meta(UV2_SCALE_META) {
        return None;
    }
    let texel = m.get_meta_ex(UV2_TEXEL_META).default(&Variant::nil()).done().try_to::<f32>().ok()?;
    let scale = m.get_meta_ex(UV2_SCALE_META).default(&Variant::nil()).done().try_to::<Vector3>().ok()?;
    Some(pt_core::build::lightmap::Uv2Stamp { texel, scale: to_p(scale) })
}

/// Terrain cell under a world point.
pub fn world_cell(terrain: &Gd<MarchingSquaresTerrain>, world: Vector3) -> Vector2i {
    let l = terrain.get_global_transform().affine_inverse() * world;
    let t = terrain.bind();
    from_iv(snap::cell_center(&t.store().config, v2(l.x, l.z)).0)
}

impl PixelBuilderMesh {
    fn default_material(&mut self) -> Gd<StandardMaterial3D> {
        self.default_mat
            .get_or_insert_with(|| {
                let mut m = StandardMaterial3D::new_gd();
                m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
                m.set_roughness(1.0);
                m
            })
            .clone()
    }

    fn upload(&mut self, c: &Compiled, slots: &Array<Option<Gd<Material>>>) -> Option<Gd<ArrayMesh>> {
        self.surface_slots.clear();
        if c.surfaces.iter().all(|s| s.indices.is_empty()) {
            return None;
        }
        let mut mesh = ArrayMesh::new_gd();
        for s in &c.surfaces {
            if s.indices.is_empty() {
                continue;
            }
            let mut arrays = VarArray::new();
            arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
            let pos: PackedVector3Array = s.positions.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
            let nrm: PackedVector3Array = s.normals.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
            let uv: PackedVector2Array = s.uvs.iter().map(|p| Vector2::new(p[0], p[1])).collect();
            let col: PackedColorArray = s.colors.iter().map(|c| Color::from_rgba(c[0], c[1], c[2], c[3])).collect();
            let idx: PackedInt32Array = s.indices.iter().copied().collect();
            arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
            arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
            arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
            arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
            arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
            mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
            let si = mesh.get_surface_count() - 1;
            let mat: Gd<Material> = slots
                .get(s.material as usize)
                .flatten()
                .unwrap_or_else(|| self.default_material().upcast());
            mesh.surface_set_material(si, &mat);
            self.surface_slots.push(s.material);
        }
        Some(mesh)
    }

    fn update_collision(&mut self, c: &Compiled) {
        // Colliders and triggers always collide (`_effective_collision_mode`); a trigger is an
        // Area3D with a convex shape (Unibuilder `refresh_collision`).
        let trigger = self.entity_type == 2;
        if (!self.collision && self.entity_type == 0) || c.collision.is_empty() {
            free_node(self.body.take());
            return;
        }
        if self.body.as_ref().is_some_and(|b| !b.is_instance_valid() || b.is_class("Area3D") != trigger) {
            free_node(self.body.take());
        }
        let body = match &self.body {
            Some(b) if b.is_instance_valid() => b.clone(),
            _ => {
                let mut b: Gd<CollisionObject3D> = if trigger { Area3D::new_alloc().upcast() } else { StaticBody3D::new_alloc().upcast() };
                b.set_name("BuilderCollision");
                let mut shape = CollisionShape3D::new_alloc();
                shape.set_name("Shape");
                b.add_child(&shape);
                // Through base_mut: add_child sends notifications back to this node.
                use godot::classes::node::InternalMode;
                self.base_mut().add_child_ex(&b).internal(InternalMode::FRONT).done();
                self.body = Some(b.clone());
                b
            }
        };
        let mut body = body;
        body.set_collision_layer(self.collision_layer);
        let faces: PackedVector3Array = c.collision.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
        let shape: Gd<godot::classes::Shape3D> = if trigger {
            let mut s = ConvexPolygonShape3D::new_gd();
            s.set_points(&faces);
            s.upcast()
        } else {
            let mut s = ConcavePolygonShape3D::new_gd();
            s.set_faces(&faces);
            s.upcast()
        };
        if let Some(mut cs) = body.get_node_or_null("Shape").and_then(|n| n.try_cast::<CollisionShape3D>().ok()) {
            cs.set_shape(&shape);
        }
    }

    /// Editor tint of colliders (orange) and triggers (blue), `_refresh_entity_display`.
    fn refresh_entity_display(&mut self) {
        if !is_editor() {
            return;
        }
        let ours = self.base().get_material_override().is_some_and(|m| m.has_meta("pt_entity_tint"));
        if self.entity_type == 0 {
            if ours {
                self.base_mut().set_material_override(Gd::<Material>::null_arg());
            }
            return;
        }
        let mut m = StandardMaterial3D::new_gd();
        m.set_meta("pt_entity_tint", &true.to_variant());
        m.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        m.set_albedo(if self.entity_type == 1 { Color::from_rgba(1.0, 0.55, 0.15, 0.4) } else { Color::from_rgba(0.35, 0.55, 1.0, 0.4) });
        self.base_mut().set_material_override(&m);
    }

    /// Asks the owning `PixelObject` (if any) to re-apply its materials to the new surfaces.
    fn notify_pixel_object(&self) {
        let mut n = self.base().get_parent();
        while let Some(p) = n {
            if p.is_class("PixelObject") {
                p.clone().call_deferred("refresh_pixel_settings", &[]);
                return;
            }
            n = p.get_parent();
        }
    }
}
