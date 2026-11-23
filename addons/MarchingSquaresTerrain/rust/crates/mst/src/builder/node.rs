//! A piece of hand-modelled scenery that stays editable after it is placed.
//!
//! Intent: a level author should be able to drop a mesh into a terrain scene, shape it
//! with the builder tools, and have it behave like any other piece of the level: it is
//! drawn with its materials, it can be walked on, it can sit on a terrain cell and follow
//! the ground when the terrain changes, it keeps grass from growing through its floor,
//! and it can play the part of an invisible blocker or trigger instead of visible detail.
//!
//! Responsibility: it owns turning its editable shape into what the engine draws and
//! collides with, keeping that in step whenever the shape or its placement changes, and
//! telling the terrain which ground it covers. It does not edit the shape; the builder
//! tools do that through the shape data it holds. It does not own the shape data either,
//! which is a resource of its own so it can be shared, saved and undone on its own.
//!
//! Why a node: it has a place, a transform and a parent in the level, it must be drawn,
//! and it needs the engine to tell it when it moves, enters or leaves the level and is
//! about to be saved.
//!
//! Why a new type: a built-in mesh node shows a finished mesh but cannot rebuild it from
//! editable data, keep collision and grass clearing in step with it, or snap to terrain
//! cells. Doing that from outside would leave every placed mesh dependent on an editor
//! tool being open, and the game would lose it entirely.
//!
//! A mesh can also stand in for empty space: marked as a cutter, or kept as a hidden part
//! of a combined solid, it is shown to the author as a ghost or not at all, and the game
//! neither collides with it nor clears grass under it.
//!
//! Nodes created in code: the collision body is generated from the shape when the game
//! runs, because whether one exists and which kind it is follows from the shape and the
//! chosen role, and it is rebuilt whenever they change. On request it also generates a
//! plain mesh copy of itself, with collision, for the author to keep in place of the
//! editable one.
//!
//! Not exported: what it is in the middle of doing, such as a rebuild or a delayed unwrap
//! that is pending, or a warning it has already given; what it last reported to the
//! terrain, which it must remember so it can take it back; values it derives from the
//! shape data, which would disagree with their source if stored; the material copies it
//! makes to tint or fade the author's materials, which are caches of those materials and
//! the role it plays; and the collision body it generates, which is rebuilt from the shape
//! rather than authored.
use super::data::{PixelBuilderMeshData, to_p};
use super::solid_role;
use crate::editor_detection::is_editor;
use crate::math_conversions::{from_iv, to_iv};
use crate::internal_children::free_node;
use godot::classes::base_material_3d::Flags;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::notify::Node3DNotification;
use godot::classes::object::ConnectFlags;
use godot::classes::{
    Area3D, ArrayMesh, CollisionObject3D, CollisionShape3D, ConcavePolygonShape3D,
    ConvexPolygonShape3D, IMeshInstance3D, Material, MeshInstance3D, RenderingServer,
    StandardMaterial3D, StaticBody3D,
};
use godot::prelude::*;
use godot::register::info::{PropertyInfo, PropertyUsageFlags};
use pt_modeler::compile::{CompileOptions, Compiled, Surface, bake};
use pt_modeler::snap;
use pt_core::math::v2;
const EDIT_LOCK_META: &str = "_edit_lock_";
const UV2_TEXEL_META: &str = "pt_uv2_texel";
const UV2_SCALE_META: &str = "pt_uv2_scale";
const MIN_FOOTPRINT_SAMPLE_STEP: f32 = 0.01;
use crate::terrain::MarchingSquaresTerrain;

#[derive(GodotClass)]
#[class(tool, base = MeshInstance3D)]
pub struct PixelBuilderMesh {
    #[export]
    #[var(set = set_data)]
    contents: Option<Gd<PixelBuilderMeshData>>,
    #[export]
    collision: bool,
    #[export(flags_3d_physics)]
    impact_stratum: u32,
    #[export]
    anchor_cell: Vector2i,
    #[export]
    anchored: bool,
    #[export]
    #[var(set = set_clear_grass)]
    clear_grass: bool,
    #[export]
    #[var(set = set_live_uv2)]
    live_uv2: bool,
    #[export(range = (0.0105, 4.0, 0.0105))]
    lightmap_texel: f32,
    uv2_gen: u32,
    atlas_refused: f32,
    #[export(enum = (Normal = 0, Collider = 1, Trigger = 2))]
    #[var(set = set_entity_type)]
    entity_type: i32,
    #[export]
    #[var(set = set_terrain)]
    terrain: Option<Gd<MarchingSquaresTerrain>>,
    #[export]
    pixel_object: Option<Gd<crate::render::object::PixelObject>>,
    footprint_sent: Vec<Vec<Vector3>>,
    blocked_smalls: Array<Vector2i>,
    default_mat: Option<Gd<StandardMaterial3D>>,
    tintable: Vec<(InstanceId, Gd<Material>)>,
    faded: Vec<(InstanceId, u32, Gd<Material>)>,
    body: Option<Gd<CollisionObject3D>>,
    body_shape: Option<Gd<CollisionShape3D>>,
    rebuild_queued: bool,
    surface_slots: Vec<u16>,
    lock_stripped_for_save: bool,
    #[export_group(name = "Grass Clearing")]
    #[export(range = (0.0, 4.0, 0.0005, or_greater))]
    #[var(set = set_footprint_band)]
    footprint_band: f32,
    #[export(range = (0.01, 1.0, 0.0005, or_greater))]
    #[var(set = set_footprint_sample_step)]
    footprint_sample_step: f32,
    #[export_group(name = "Live UV2")]
    #[export(range = (0.0, 5.0, 0.05, or_greater))]
    live_uv2_delay: f64,
    #[export_group(name = "Role Preview")]
    #[export]
    #[var(set = set_collider_tint)]
    collider_tint: Color,
    #[export]
    #[var(set = set_trigger_tint)]
    trigger_tint: Color,
    #[export]
    #[var(set = set_ghost_fill_tint)]
    ghost_fill_tint: Color,
    #[export]
    #[var(set = set_ghost_edge_tint)]
    ghost_edge_tint: Color,
    #[export(range = (0.0, 0.5, 0.0005, or_greater))]
    #[var(set = set_ghost_edge_grow)]
    ghost_edge_grow: f32,
    base: Base<MeshInstance3D>,
}
#[godot_api]
impl IMeshInstance3D for PixelBuilderMesh {
    fn init(base: Base<MeshInstance3D>) -> Self {
        PixelBuilderMesh {
            contents: None,
            collision: true,
            impact_stratum: 1,
            anchor_cell: Vector2i::ZERO,
            anchored: false,
            clear_grass: true,
            entity_type: 0,
            terrain: None,
            pixel_object: None,
            live_uv2: false,
            lightmap_texel: 0.2005,
            uv2_gen: 0,
            atlas_refused: 0.0,
            footprint_sent: Vec::new(),
            blocked_smalls: Array::new(),
            default_mat: None,
            tintable: Vec::new(),
            faded: Vec::new(),
            body: None,
            body_shape: None,
            rebuild_queued: false,
            surface_slots: Vec::new(),
            lock_stripped_for_save: false,
            footprint_band: 0.3505,
            footprint_sample_step: 0.1505,
            live_uv2_delay: 0.5,
            collider_tint: Color::from_rgba(1.0, 0.5505, 0.1505, 0.4005),
            trigger_tint: Color::from_rgba(0.3505, 0.5505, 1.0, 0.4005),
            ghost_fill_tint: Color::from_rgba(1.0, 0.25, 0.75, 0.3),
            ghost_edge_tint: Color::from_rgba(1.0, 0.3, 0.8, 0.9),
            ghost_edge_grow: 0.01,
            base,
        }
    }
    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        if property == "transparency" {
            self.queue_rebuild();
        }
        false
    }
    fn ready(&mut self) {
        if self.base().has_meta(super::plugin::PLUGIN_LOCK_META) {
            self.base_mut().remove_meta(super::plugin::PLUGIN_LOCK_META);
            self.base_mut().remove_meta(EDIT_LOCK_META);
        }
        if !is_editor() && (self.entity_type != 0 || self.is_negated()) {
            self.base_mut().set_visible(false);
        }
        if is_editor() && self.is_negated() {
            self.refresh_entity_display();
        }
        if self.contents.is_none() && is_editor() {
            let d = PixelBuilderMeshData::new_gd();
            self.set_data(Some(d));
        }
        if is_editor() {
            self.base_mut().set_notify_transform(true);
        }
        self.reassemble();
    }
    fn exit_tree(&mut self) {
        if !self.footprint_sent.is_empty() || !self.blocked_smalls.is_empty() {
            self.footprint_sent.clear();
            self.notify_terrain_footprint();
        }
    }
    fn on_notification(&mut self, what: Node3DNotification) {
        match what {
            Node3DNotification::TRANSFORM_CHANGED => self.queue_rebuild(),
            Node3DNotification::EDITOR_PRE_SAVE => {
                if self.base().has_meta(super::plugin::PLUGIN_LOCK_META) {
                    self.base_mut().remove_meta(super::plugin::PLUGIN_LOCK_META);
                    self.base_mut().remove_meta(EDIT_LOCK_META);
                    self.lock_stripped_for_save = true;
                }
                if self.live_uv2 {
                    self.uv2_gen = self.uv2_gen.wrapping_add(1);
                    let g = self.uv2_gen;
                    self.apply_live_uv2(g);
                }
                if self
                    .base()
                    .get_material_override()
                    .is_some_and(|m| m.has_meta(solid_role::DISPLAY_TINT_META))
                {
                    self.base_mut()
                        .set_material_override(Gd::<Material>::null_arg());
                }
            }
            Node3DNotification::EDITOR_POST_SAVE => {
                if std::mem::take(&mut self.lock_stripped_for_save) {
                    self.base_mut().set_meta(EDIT_LOCK_META, &true.to_variant());
                    self.base_mut()
                        .set_meta(super::plugin::PLUGIN_LOCK_META, &true.to_variant());
                }
                self.refresh_entity_display();
            }
            _ => {}
        }
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        match self.assigned_terrain() {
            None if self.clear_grass || self.anchored => {
                w.push("Assign the Terrain this mesh sits on: without one it clears no grass and cannot follow a terrain cell.");
            }
            Some(t) if self.clear_grass && !t.bind().lists_builder_mesh(&self.to_gd()) => {
                w.push("Add this mesh to its terrain's Builder Meshes: the terrain only clears grass under meshes listed there.");
            }
            _ => {}
        }
        w
    }
    fn on_validate_property(&self, property: &mut PropertyInfo) {
        if property.property_name == StringName::from("mesh") && !self.live_uv2 {
            property.usage = PropertyUsageFlags::from_ord(
                property.usage.ord() & !PropertyUsageFlags::STORAGE.ord(),
            );
        }
    }
}
impl PixelBuilderMesh {
    pub fn contents(&self) -> Option<Gd<PixelBuilderMeshData>> {
        self.contents.clone()
    }
    fn is_negated(&self) -> bool {
        solid_role::is_negated(&self.base().clone().upcast())
    }
    fn takes_no_part_in_world(&self) -> bool {
        solid_role::takes_no_part_in_world(&self.base().clone().upcast())
    }
    fn world_footprints(&self) -> Vec<Vec<Vector3>> {
        if !self.clear_grass || !self.base().is_inside_tree() || self.takes_no_part_in_world() {
            return Vec::new();
        }
        let Some(d) = &self.contents else {
            return Vec::new();
        };
        let xf = self.base().get_global_transform();
        pt_modeler::footprint::floor_footprints(d.bind().cage(), self.footprint_band)
            .into_iter()
            .map(|poly| {
                poly.into_iter()
                    .map(|p| xf * super::data::from_p(p))
                    .collect()
            })
            .collect()
    }
    fn sync_footprint(&mut self) {
        let now = self.world_footprints();
        let same = now.len() == self.footprint_sent.len()
            && now.iter().zip(&self.footprint_sent).all(|(a, b)| {
                a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.distance_to(*q) < 1e-3)
            });
        if same {
            return;
        }
        self.footprint_sent = now;
        self.notify_terrain_footprint();
    }
    pub fn assigned_terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.terrain.clone().filter(|t| t.is_instance_valid())
    }
    fn notify_terrain_footprint(&mut self) {
        let Some(terrain) = self.assigned_terrain() else {
            return;
        };
        let hex = matches!(
            terrain.bind().grid(),
            pt_core::grid::GridType::Hex | pt_core::grid::GridType::HexRings
        );
        let mut t = terrain.upcast::<Node>();
        t.call_deferred("refresh_grass_water", &[]);
        if !hex {
            return;
        }
        if !self.blocked_smalls.is_empty() {
            t.call_deferred(
                "set_small_hexes_blocked",
                &[self.blocked_smalls.to_variant(), false.to_variant()],
            );
        }
        let step = self.footprint_sample_step.max(MIN_FOOTPRINT_SAMPLE_STEP);
        let mut smalls: Array<Vector2i> = Array::new();
        let mut seen = std::collections::HashSet::new();
        for poly in &self.footprint_sent {
            let pts: Vec<pt_core::math::Vec2> = poly.iter().map(|p| v2(p.x, p.z)).collect();
            let (lo, hi) = pts.iter().fold(
                (v2(f32::MAX, f32::MAX), v2(f32::MIN, f32::MIN)),
                |(a, b), p| {
                    (
                        v2(a.x.min(p.x), a.y.min(p.y)),
                        v2(b.x.max(p.x), b.y.max(p.y)),
                    )
                },
            );
            let y = poly[0].y;
            let mut x = lo.x;
            while x <= hi.x {
                let mut z = lo.y;
                while z <= hi.y {
                    let s = t
                        .call("world_to_small_hex", &[Vector3::new(x, y, z).to_variant()])
                        .to::<Vector2i>();
                    if seen.insert((s.x, s.y)) {
                        let c = t
                            .call("small_hex_to_world", &[s.to_variant(), y.to_variant()])
                            .to::<Vector3>();
                        if point_in_poly(v2(c.x, c.z), &pts) {
                            smalls.push(s);
                        }
                    }
                    z += step;
                }
                x += step;
            }
        }
        if !smalls.is_empty() {
            t.call_deferred(
                "set_small_hexes_blocked",
                &[smalls.to_variant(), true.to_variant()],
            );
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
        if let Some(old) = &mut self.contents
            && old.is_connected("changed", &cb)
        {
            old.disconnect("changed", &cb);
        }
        self.contents = v;
        if let Some(d) = &mut self.contents {
            d.connect_flags("changed", &cb, ConnectFlags::DEFERRED);
        }
        self.queue_rebuild();
    }
    #[func]
    fn on_tint_source_changed(&mut self) {
        self.tintable.clear();
        self.faded.clear();
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
    fn schedule_live_uv2(&mut self) {
        if !self.live_uv2 || !is_editor() || !self.base().is_inside_tree() {
            return;
        }
        self.uv2_gen = self.uv2_gen.wrapping_add(1);
        let mut tree = self.base().get_tree();
        let mut t = tree.create_timer(self.live_uv2_delay.max(0.0));
        let cb = self
            .to_gd()
            .callable("apply_live_uv2")
            .bind(&[self.uv2_gen.to_variant()]);
        t.connect("timeout", &cb);
    }
    #[func]
    fn apply_live_uv2(&mut self, generation: u32) {
        if generation != self.uv2_gen || !self.live_uv2 {
            return;
        }
        let texel = self.lightmap_texel.max(0.0105);
        if let Some(m) = self.unwrapped_mesh(texel) {
            let same = self
                .base()
                .get_mesh()
                .is_some_and(|c| c.instance_id() == m.instance_id());
            if !same {
                self.base_mut().set_mesh(&m);
            }
        }
    }
    #[func]
    fn refresh_solid_role(&mut self) {
        self.refresh_entity_display();
        self.sync_footprint();
        self.queue_rebuild();
    }
    #[func]
    fn set_entity_type(&mut self, v: i32) {
        self.entity_type = v.clamp(0, 2);
        self.refresh_entity_display();
        self.queue_rebuild();
    }
    #[func]
    fn set_footprint_band(&mut self, v: f32) {
        self.footprint_band = v.max(0.0);
        self.sync_footprint();
    }
    #[func]
    fn set_footprint_sample_step(&mut self, v: f32) {
        self.footprint_sample_step = v.max(MIN_FOOTPRINT_SAMPLE_STEP);
        self.footprint_sent.clear();
        self.sync_footprint();
    }
    #[func]
    fn set_collider_tint(&mut self, v: Color) {
        self.collider_tint = v;
        self.refresh_entity_display();
    }
    #[func]
    fn set_trigger_tint(&mut self, v: Color) {
        self.trigger_tint = v;
        self.refresh_entity_display();
    }
    #[func]
    fn set_ghost_fill_tint(&mut self, v: Color) {
        self.ghost_fill_tint = v;
        self.refresh_entity_display();
    }
    #[func]
    fn set_ghost_edge_tint(&mut self, v: Color) {
        self.ghost_edge_tint = v;
        self.refresh_entity_display();
    }
    #[func]
    fn set_ghost_edge_grow(&mut self, v: f32) {
        self.ghost_edge_grow = v.max(0.0);
        self.refresh_entity_display();
    }
    #[func]
    fn set_terrain(&mut self, v: Option<Gd<MarchingSquaresTerrain>>) {
        if self.terrain != v {
            self.footprint_sent.clear();
            self.notify_terrain_footprint();
        }
        self.terrain = v;
        self.base_mut().update_configuration_warnings();
        self.queue_rebuild();
    }
    #[func]
    fn set_clear_grass(&mut self, v: bool) {
        self.clear_grass = v;
        self.sync_footprint();
    }
    #[func]
    pub fn get_grass_footprints(&self) -> Array<PackedVector3Array> {
        self.world_footprints()
            .into_iter()
            .map(|p| p.into_iter().collect::<PackedVector3Array>())
            .collect()
    }
    #[func]
    pub fn queue_rebuild(&mut self) {
        if self.rebuild_queued || !self.base().is_inside_tree() {
            return;
        }
        self.rebuild_queued = true;
        self.to_gd().call_deferred("reassemble", &[]);
    }
    #[func]
    pub fn reassemble(&mut self) {
        self.rebuild_queued = false;
        let Some(data) = self.contents.clone() else {
            self.base_mut()
                .set_mesh(Gd::<godot::classes::Mesh>::null_arg());
            return;
        };
        let origin = if self.base().is_inside_tree() {
            self.base().get_global_position()
        } else {
            self.base().get_position()
        };
        let (compiled, slots) = {
            let d = data.bind();
            let opt = CompileOptions {
                units_per_repeat: d.units(),
                world_offset: to_p(origin),
            };
            (bake(d.cage(), &opt), d.stuff_pockets())
        };
        if !is_editor()
            && self.live_uv2
            && self.base().get_mesh().is_some_and(|m| cage_holds_uv_b(&m))
        {
            self.sync_footprint();
            self.update_collision(&compiled);
            return;
        }
        let mesh = self.upload(&compiled, &slots);
        self.sync_footprint();
        if is_editor() && self.live_uv2 {
            self.schedule_live_uv2();
        }
        let had = self
            .base()
            .get_mesh()
            .map(|m| m.get_surface_count())
            .unwrap_or(0);
        match &mesh {
            Some(m) => self.base_mut().set_mesh(m),
            None => self
                .base_mut()
                .set_mesh(Gd::<godot::classes::Mesh>::null_arg()),
        }
        if !is_editor() {
            self.update_collision(&compiled);
            let now = mesh.map(|m| m.get_surface_count()).unwrap_or(0);
            if now != had || had == 0 {
                for i in 0..now {
                    self.base_mut()
                        .set_surface_override_material(i, Gd::<Material>::null_arg());
                }
                self.notify_pixel_object();
            }
        }
    }
    #[func]
    pub fn get_surface_slot(&self, surface: i64) -> i64 {
        self.surface_slots
            .get(surface as usize)
            .map(|s| *s as i64)
            .unwrap_or(-1)
    }
    #[func]
    pub fn unwrapped_mesh(&mut self, texel_size: f32) -> Option<Gd<ArrayMesh>> {
        if self.rebuild_queued || self.base().get_mesh().is_none() {
            self.reassemble();
        }
        let src = self.base().get_mesh()?;
        let xf = if self.base().is_inside_tree() {
            self.base().get_global_transform()
        } else {
            self.base().get_transform()
        };
        let current = uv2_stamp(&src).is_some_and(|st| {
            pt_modeler::lightmap::uv_b_present_check(
                Some(st),
                cage_holds_uv_b(&src),
                texel_size,
                to_p(xf.basis.get_scale()),
            )
        });
        use pt_modeler::lightmap::{Uv2Result, plan_unwrap};
        match plan_unwrap(texel_size, src.get_surface_count() as usize, current, false) {
            Uv2Result::Skipped => return None,
            Uv2Result::Current => return src.try_cast::<ArrayMesh>().ok(),
            _ => {}
        }
        if let Some(d) = self.contents.clone()
            && let Err(e) = pt_modeler::lightmap::check_atlas(
                d.bind().cage(),
                to_p(xf.basis.get_scale()),
                texel_size,
            )
        {
            if self.atlas_refused != texel_size {
                self.atlas_refused = texel_size;
                godot_warn!("PixelBuilderMesh \"{}\": {e}", self.base().get_name());
            }
            return None;
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
            godot_warn!(
                "PixelBuilderMesh: lightmap unwrap failed on \"{}\" ({err:?})",
                self.base().get_name()
            );
            return None;
        }
        copy.set_meta(UV2_TEXEL_META, &texel_size.to_variant());
        copy.set_meta(UV2_SCALE_META, &xf.basis.get_scale().to_variant());
        Some(copy)
    }
    #[func(gd_self)]
    pub fn make_baked(this: Gd<Self>, lightmap_texel: f32) -> Gd<MeshInstance3D> {
        let (mesh, collision) = {
            let mut t = this.clone();
            let mut g = t.bind_mut();
            if g.rebuild_queued || g.base().get_mesh().is_none() {
                g.reassemble();
            }
            let mesh: Option<Gd<godot::classes::Mesh>> = if lightmap_texel > 0.0 {
                g.unwrapped_mesh(lightmap_texel)
                    .map(|m| m.upcast())
                    .or_else(|| g.base().get_mesh())
            } else {
                g.base().get_mesh()
            };
            let collision = match (g.collision, g.contents.clone()) {
                (true, Some(data)) => {
                    let d = data.bind();
                    let c = bake(
                        d.cage(),
                        &CompileOptions {
                            units_per_repeat: d.units(),
                            world_offset: to_p(g.base().get_global_position()),
                        },
                    );
                    Some((c.collision, g.impact_stratum))
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
        let skip = [
            "script",
            "mesh",
            "contents",
            "collision",
            "impact_stratum",
            "anchor_cell",
            "anchored",
        ];
        let mine = this.get_property_list();
        let theirs: std::collections::HashSet<String> = baked
            .get_property_list()
            .iter_shared()
            .filter_map(|d| d.get("name").map(|n| n.to_string()))
            .collect();
        let plugin_lock = this.has_meta(super::plugin::PLUGIN_LOCK_META);
        for p in mine.iter_shared() {
            let (Some(name), Some(usage)) = (
                p.get("name").map(|n| n.to_string()),
                p.get("usage").and_then(|u| u.try_to::<i64>().ok()),
            ) else {
                continue;
            };
            if usage & PropertyUsageFlags::STORAGE.ord() as i64 == 0
                || skip.contains(&name.as_str())
            {
                continue;
            }
            if (name == format!("metadata/{EDIT_LOCK_META}") && plugin_lock)
                || name == format!("metadata/{}", super::plugin::PLUGIN_LOCK_META)
            {
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
            let faces: PackedVector3Array = tris
                .iter()
                .map(|p| Vector3::new(p[0], p[1], p[2]))
                .collect();
            shape.set_faces(&faces);
            cs.set_shape(&shape);
            body.add_child(&cs);
            baked.add_child(&body);
        }
        baked
    }
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
    #[func]
    pub fn resnap(&mut self, terrain: Gd<MarchingSquaresTerrain>) {
        if self.anchored {
            let c = self.anchor_cell;
            self.place_on_cell(terrain, c);
        }
    }
}
pub fn surface_arrays(s: &Surface) -> VarArray {
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let pos: PackedVector3Array = s
        .positions
        .iter()
        .map(|p| Vector3::new(p[0], p[1], p[2]))
        .collect();
    let nrm: PackedVector3Array = s
        .normals
        .iter()
        .map(|p| Vector3::new(p[0], p[1], p[2]))
        .collect();
    let uv: PackedVector2Array = s.uvs.iter().map(|p| Vector2::new(p[0], p[1])).collect();
    let col: PackedColorArray = s
        .tints
        .iter()
        .map(|c| Color::from_rgba(c[0], c[1], c[2], c[3]))
        .collect();
    let idx: PackedInt32Array = s.ordinals.iter().copied().collect();
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
    arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
    arrays
}
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
fn cage_holds_uv_b(m: &Gd<godot::classes::Mesh>) -> bool {
    m.clone().try_cast::<ArrayMesh>().is_ok_and(|a| {
        a.get_surface_count() > 0
            && (a.surface_get_format(0).ord() as i64
                & godot::classes::mesh::ArrayFormat::TEX_UV2.ord() as i64)
                != 0
    })
}
fn uv2_stamp(m: &Gd<godot::classes::Mesh>) -> Option<pt_modeler::lightmap::Uv2Stamp> {
    if !m.has_meta(UV2_TEXEL_META) || !m.has_meta(UV2_SCALE_META) {
        return None;
    }
    let texel = m
        .get_meta_ex(UV2_TEXEL_META)
        .default(&Variant::nil())
        .done()
        .try_to::<f32>()
        .ok()?;
    let scale = m
        .get_meta_ex(UV2_SCALE_META)
        .default(&Variant::nil())
        .done()
        .try_to::<Vector3>()
        .ok()?;
    Some(pt_modeler::lightmap::Uv2Stamp {
        texel,
        magnify: to_p(scale),
    })
}
pub fn world_cell(terrain: &Gd<MarchingSquaresTerrain>, world: Vector3) -> Vector2i {
    let l = terrain.get_global_transform().affine_inverse() * world;
    let t = terrain.bind();
    from_iv(snap::cell_center(&t.store().config, v2(l.x, l.z)).0)
}
impl PixelBuilderMesh {
    fn fallback_stuff(&mut self) -> Gd<StandardMaterial3D> {
        self.default_mat
            .get_or_insert_with(|| {
                let mut m = StandardMaterial3D::new_gd();
                m.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
                m.set_roughness(1.0);
                m
            })
            .clone()
    }
    fn tintable_look(&mut self, mat: Gd<Material>) -> Gd<Material> {
        use super::tint_rules::{SlotLook, needs_tintable_copy};
        let look = match mat.clone().try_cast::<godot::classes::BaseMaterial3D>() {
            Ok(b) => SlotLook::Standard {
                takes_vertex_colour: b.get_flag(Flags::ALBEDO_FROM_VERTEX_COLOR),
            },
            Err(_) => SlotLook::Custom,
        };
        if !needs_tintable_copy(look) {
            return mat;
        }
        let id = mat.instance_id();
        if let Some((_, copy)) = self.tintable.iter().find(|(k, _)| *k == id) {
            return copy.clone();
        }
        let cb = self.to_gd().callable("on_tint_source_changed");
        let mut source = mat.clone();
        if !source.is_connected("changed", &cb) {
            source.connect_flags("changed", &cb, ConnectFlags::DEFERRED);
        }
        let Ok(mut copy) = mat
            .duplicate_resource()
            .try_cast::<godot::classes::BaseMaterial3D>()
        else {
            return mat;
        };
        copy.set_flag(Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        let copy: Gd<Material> = copy.upcast();
        self.tintable.push((id, copy.clone()));
        copy
    }
    fn faded_look(&mut self, mat: Gd<Material>) -> Gd<Material> {
        use super::tint_rules::{SlotLook, faded_alpha, fades_through_material};
        let method = RenderingServer::singleton().get_current_rendering_method().to_string();
        if !fades_through_material(&method) {
            return mat;
        }
        let t = self.base().get_transparency();
        let Ok(b) = mat.clone().try_cast::<godot::classes::BaseMaterial3D>() else {
            return mat;
        };
        let look = SlotLook::Standard {
            takes_vertex_colour: b.get_flag(Flags::ALBEDO_FROM_VERTEX_COLOR),
        };
        let Some(alpha) = faded_alpha(look, b.get_albedo().a, t) else {
            return mat;
        };
        let (id, key) = (mat.instance_id(), t.to_bits());
        if let Some((.., copy)) = self.faded.iter().find(|(k, b, _)| *k == id && *b == key) {
            return copy.clone();
        }
        let Ok(mut copy) = mat
            .duplicate_resource()
            .try_cast::<godot::classes::BaseMaterial3D>()
        else {
            return mat;
        };
        copy.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        let mut albedo = copy.get_albedo();
        albedo.a = alpha;
        copy.set_albedo(albedo);
        let copy: Gd<Material> = copy.upcast();
        self.faded.retain(|(k, ..)| *k != id);
        self.faded.push((id, key, copy.clone()));
        copy
    }
    fn upload(
        &mut self,
        c: &Compiled,
        slots: &Array<Option<Gd<Material>>>,
    ) -> Option<Gd<ArrayMesh>> {
        self.surface_slots.clear();
        if c.surfaces.iter().all(|s| s.ordinals.is_empty()) {
            return None;
        }
        let mut mesh = ArrayMesh::new_gd();
        for s in &c.surfaces {
            if s.ordinals.is_empty() {
                continue;
            }
            mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &surface_arrays(s));
            let si = mesh.get_surface_count() - 1;
            let mat: Gd<Material> = match slots.get(s.stuff as usize).flatten() {
                Some(m) => self.tintable_look(m),
                None => self.fallback_stuff().upcast(),
            };
            let mat = self.faded_look(mat);
            mesh.surface_set_material(si, &mat);
            self.surface_slots.push(s.stuff);
        }
        Some(mesh)
    }
    fn update_collision(&mut self, c: &Compiled) {
        let trigger = self.entity_type == 2;
        if (!self.collision && self.entity_type == 0)
            || c.collision.is_empty()
            || self.takes_no_part_in_world()
        {
            free_node(self.body.take());
            return;
        }
        if self
            .body
            .as_ref()
            .is_some_and(|b| !b.is_instance_valid() || b.is_class("Area3D") != trigger)
        {
            free_node(self.body.take());
        }
        let body = match &self.body {
            Some(b) if b.is_instance_valid() => b.clone(),
            _ => {
                let mut b: Gd<CollisionObject3D> = if trigger {
                    Area3D::new_alloc().upcast()
                } else {
                    StaticBody3D::new_alloc().upcast()
                };
                b.set_name("BuilderCollision");
                let mut shape = CollisionShape3D::new_alloc();
                shape.set_name("Shape");
                b.add_child(&shape);
                self.body_shape = Some(shape);
                use godot::classes::node::InternalMode;
                self.base_mut()
                    .add_child_ex(&b)
                    .internal(InternalMode::FRONT)
                    .done();
                self.body = Some(b.clone());
                b
            }
        };
        let mut body = body;
        body.set_collision_layer(self.impact_stratum);
        let faces: PackedVector3Array = c
            .collision
            .iter()
            .map(|p| Vector3::new(p[0], p[1], p[2]))
            .collect();
        let shape: Gd<godot::classes::Shape3D> = if trigger {
            let mut s = ConvexPolygonShape3D::new_gd();
            s.set_points(&faces);
            s.upcast()
        } else {
            let mut s = ConcavePolygonShape3D::new_gd();
            s.set_faces(&faces);
            s.upcast()
        };
        if let Some(mut cs) = self.body_shape.clone().filter(|s| s.is_instance_valid()) {
            cs.set_shape(&shape);
        }
    }
    fn refresh_entity_display(&mut self) {
        if !is_editor() {
            return;
        }
        let ours = self
            .base()
            .get_material_override()
            .is_some_and(|m| m.has_meta(solid_role::DISPLAY_TINT_META));
        if self.is_negated() {
            let ghost = solid_role::ghost_material(
                self.ghost_fill_tint,
                self.ghost_edge_tint,
                self.ghost_edge_grow,
            );
            self.base_mut().set_material_override(&ghost);
            return;
        }
        if self.entity_type == 0 {
            if ours {
                self.base_mut()
                    .set_material_override(Gd::<Material>::null_arg());
            }
            return;
        }
        let mut m = StandardMaterial3D::new_gd();
        m.set_meta(solid_role::DISPLAY_TINT_META, &true.to_variant());
        m.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        m.set_albedo(if self.entity_type == 1 {
            self.collider_tint
        } else {
            self.trigger_tint
        });
        self.base_mut().set_material_override(&m);
    }
    fn notify_pixel_object(&self) {
        if let Some(p) = self.pixel_object.clone().filter(|p| p.is_instance_valid()) {
            crate::render::object::PixelObject::refresh_settings_deferred(&p);
        }
    }
}
