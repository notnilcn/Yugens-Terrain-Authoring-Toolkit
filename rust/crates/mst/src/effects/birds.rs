//! A flock of birds that wheels over the world, settles on the ground and scatters.
//!
//! Intent: a scene author places one above a stretch of terrain and gets a small living
//! flock: birds that circle together, flap and glide, now and then land on high ground, and
//! take off again after a while or when the view comes close. They keep out of the camera's
//! way and stay a readable size on screen whatever the zoom.
//!
//! Responsibility: it owns the birds' movement and drawing. It reads the ground height from
//! the terrain and the pixel size from the renderer, and it follows the project switch for
//! its kind of effect. It decides nothing about the terrain, the camera or the renderer,
//! and nothing reacts to the birds.
//!
//! Why a node: it has a home position in the world, it needs a per-frame callback to fly
//! the flock, and it must follow the active camera and the terrain and renderer the scene
//! author assigns to it. It never goes looking for them, and says so in the editor when
//! one is missing.
//!
//! Why a new type: a built-in particle system cannot flock, land on the terrain or avoid
//! the camera, and keeping one bird per node would scatter a single flock's rules across
//! many nodes.
//!
//! Created in code: the flock is drawn by one internal instanced mesh whose instance set is
//! the flock itself, sized and refilled from the node's own settings and simulation, so a
//! declared node would hold nothing an author could set.
//!
//! Not exported: the birds' positions, headings and landing state, the flock timers, random
//! state and the camera's reach and pixel size it last measured, which only mean something
//! while the flock flies; and handles to its internal drawing parts and their buffers.

use super::camera_view::{active_camera, camera_focus};
use super::feature_gate::{gate, gate_warnings};
use super::rng::Rng;
use crate::editor_detection::is_editor;
use crate::features::Feature;
use crate::globals;
use crate::internal_children::add_internal;
use crate::object_flags::object_flags;
use crate::shader_materials::shader_material;
use crate::terrain::MarchingSquaresTerrain;
use godot::classes::camera_3d::ProjectionType;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    ArrayMesh, INode3D, MultiMesh, MultiMeshInstance3D, Node, Node3D, ShaderMaterial,
};
use godot::prelude::*;

use crate::instance_buffer::InstanceBuffer;
use crate::shader_param_cache::ShaderParams;
#[derive(Clone, Copy, PartialEq)]
enum State {
    Fly,
    Land(Vector3),
    Perched,
}
#[derive(Clone, Copy)]
struct Bird {
    pos: Vector3,
    vel: Vector3,
    state: State,
    phase: f32,
    timer: f32,
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelBirds {
    #[export(range = (1.0, 256.0, 1.0, or_greater))]
    #[var(set = assign_tally)]
    count: i32,
    #[export(range = (1.0, 200.0, 0.1005, or_greater))]
    radius: f32,
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    altitude: f32,
    #[export]
    #[var(set = set_terrain)]
    terrain: Option<Gd<MarchingSquaresTerrain>>,
    #[export]
    #[var(set = set_pixel_renderer)]
    pixel_renderer: Option<Gd<crate::render::renderer::PixelRenderer>>,
    #[export_group(name = "Flocking")]
    #[export(range = (0.1005, 30.0, 0.1005))]
    speed: f32,
    #[export(range = (0.1005, 20.0, 0.0505))]
    neighbor_distance: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    separation: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    alignment: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    cohesion: f32,
    #[export(range = (0.1005, 10.0, 0.0505))]
    steer_rate: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    min_speed_ratio: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    max_speed_ratio: f32,
    #[export(range = (0.0, 10.0, 0.0505))]
    camera_avoid_strength: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    camera_avoid_lift: f32,
    #[export(range = (0.0, 10.0, 0.0505))]
    ground_push: f32,
    #[export(range = (0.0, 2.0, 0.0105))]
    glide_dive_ratio: f32,
    #[export_group(name = "Appearance")]
    #[export(range = (0.0, 20.0, 0.1005))]
    #[var(set = set_flap_speed)]
    flap_speed: f32,
    #[export(range = (0.0505, 4.0, 0.0105))]
    #[var(set = assign_measure)]
    size: f32,
    #[export]
    #[var(set = assign_tint)]
    tint: Color,
    #[export]
    #[var(set = assign_hint_tint)]
    hint_tint: Color,
    #[export(range = (0.0, 32.0, 1.0))]
    #[var(set = set_min_texels)]
    min_texels: f32,
    #[export(range = (0.0, 64.0, 1.0))]
    #[var(set = set_max_texels)]
    max_texels: f32,
    #[export(range = (0.0, 50.0, 0.1005))]
    camera_clearance: f32,
    #[export(range = (0.0, 50.0, 0.1005))]
    ground_clearance: f32,
    #[export]
    #[var(set = set_outline)]
    outline: bool,
    #[export]
    #[var(set = set_seed)]
    seed: i32,
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,
    #[export_group(name = "Landing")]
    #[export]
    landing: bool,
    #[export(range = (1.0, 600.0, 0.5))]
    land_interval: f32,
    #[export(range = (0.5, 600.0, 0.5))]
    perch_seconds: f32,
    #[export(range = (0.0, 100.0, 0.1005))]
    scare_distance: f32,
    #[export(range = (0.0, 10.0, 0.0105))]
    landing_spread: f32,
    #[export(range = (0.0105, 2.0, 0.0105))]
    touchdown_distance: f32,
    #[export(range = (0.1005, 20.0, 0.1005))]
    approach_slow_distance: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    approach_min_speed_ratio: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    approach_steer_boost: f32,
    #[export(range = (0.0, 4.0, 0.0105))]
    takeoff_lift: f32,
    #[export_subgroup(name = "Preview", prefix = "preview_")]
    #[export]
    preview_in_editor: bool,
    birds: Vec<Bird>,
    view: Option<(Vector3, Vector3)>,
    flock_timer: f32,
    orbit: f32,
    texel: Vector2,
    stuff: Option<Gd<ShaderMaterial>>,
    rng: Rng,
    buffer: InstanceBuffer,
    flock: Option<Gd<MultiMeshInstance3D>>,
    base: Base<Node3D>,
}
#[godot_api]
impl INode3D for PixelBirds {
    fn init(base: Base<Node3D>) -> Self {
        PixelBirds {
            count: 7,
            radius: 14.0,
            altitude: 10.0,
            speed: 4.5005,
            neighbor_distance: 2.5005,
            separation: 1.4005,
            alignment: 0.6005,
            cohesion: 0.5,
            steer_rate: 2.2005,
            min_speed_ratio: 0.6005,
            max_speed_ratio: 1.3005,
            camera_avoid_strength: 2.5005,
            camera_avoid_lift: 0.5,
            ground_push: 3.0,
            glide_dive_ratio: 0.4005,
            flap_speed: 4.0,
            size: 0.8005,
            tint: Color::from_rgb(0.9005, 0.8805, 0.8205),
            hint_tint: Color::from_rgb(0.3605, 0.3405, 0.4005),
            min_texels: 9.0,
            max_texels: 18.0,
            camera_clearance: 6.0,
            ground_clearance: 2.0,
            outline: false,
            seed: 7,
            landing: true,
            land_interval: 30.0,
            perch_seconds: 10.0,
            scare_distance: 5.0,
            landing_spread: 1.2005,
            touchdown_distance: 0.1505,
            approach_slow_distance: 4.0,
            approach_min_speed_ratio: 0.2505,
            approach_steer_boost: 1.5005,
            takeoff_lift: 1.2005,
            preview_in_editor: true,
            birds: Vec::new(),
            view: None,
            flock_timer: 0.0,
            orbit: 0.0,
            terrain: None,
            pixel_renderer: None,
            texel: Vector2::ZERO,
            stuff: None,
            rng: Rng(7),
            layers: 1,
            flock: None,
            buffer: InstanceBuffer::default(),
            base,
        }
    }
    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        super::legacy_multimesh_properties::is_legacy_multimesh_prop(&property.to_string())
    }
    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        crate::features::observe(&self.to_gd());
        self.reassemble();
        self.refresh_pixel_settings();
        gate(&self.to_gd(), Feature::Birds);
    }
    fn process(&mut self, delta: f64) {
        let dt = (crate::clock::step(delta) as f32).min(0.1005);
        if is_editor() && !self.preview_in_editor {
            return;
        }
        if dt > 0.0 {
            self.simulate(dt);
            self.upload();
        }
        self.sync_texels();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = gate_warnings(Feature::Birds);
        if self.terrain.is_none() {
            w.push("Assign the terrain the birds fly over: without it they cannot land or keep clear of the ground.");
        }
        if self.pixel_renderer.is_none() {
            w.push("Assign the pixel renderer: without it the birds cannot keep a steady size on screen.");
        }
        w
    }
}
impl PixelBirds {
    fn flock(&mut self) -> Gd<MultiMeshInstance3D> {
        if let Some(f) = &self.flock
            && f.is_instance_valid()
        {
            return f.clone();
        }
        let mut n = MultiMeshInstance3D::new_alloc();
        n.set_name("Flock");
        n.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        n.set_layer_mask(self.layers | globals::meta_mask());
        add_internal(&mut self.to_gd(), &n);
        self.flock = Some(n.clone());
        n
    }
    fn sync_texels(&mut self) {
        let renderer = self.pixel_renderer.clone().filter(|r| r.is_instance_valid());
        let node = self.to_gd().upcast::<Node>();
        let texel = match (renderer, active_camera(&node)) {
            (Some(r), Some(cam)) => {
                let h = r.bind().get_low_res_size().y as f32;
                if h <= 0.0 {
                    Vector2::ZERO
                } else if cam.get_projection() == ProjectionType::ORTHOGONAL {
                    Vector2::new(cam.get_size() / h, 0.0)
                } else {
                    Vector2::new(0.0, 2.0 * (cam.get_fov().to_radians() * 0.5).tan() / h)
                }
            }
            _ => Vector2::ZERO,
        };
        if texel != self.texel {
            self.texel = texel;
            if let Some(mut m) = self.stuff.clone() {
                m.param("texel_size", &texel.to_variant());
            }
        }
    }
    fn reassemble(&mut self) {
        if !self.base().is_inside_tree() || !crate::features::active(Feature::Birds) {
            return;
        }
        let n = self.count.max(1) as usize;
        self.rng = Rng(self.seed as u64 ^ 0xB1_2D5);
        let home = self.home();
        self.birds = (0..n)
            .map(|_| {
                let a = self.rng.next_f32() * std::f32::consts::TAU;
                let r = self.radius * (0.6005 + 0.4005 * self.rng.next_f32());
                let pos = home + Vector3::new(a.cos() * r, self.rng.signed() * 1.5005, a.sin() * r);
                let vel = Vector3::new(-a.sin(), 0.0, a.cos()) * self.speed;
                Bird {
                    pos,
                    vel,
                    state: State::Fly,
                    phase: self.rng.next_f32(),
                    timer: 0.0,
                }
            })
            .collect();
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&bird_mesh());
        mm.set_instance_count(n as i32);
        self.flock().set_multimesh(&mm);
        self.buffer.revert();
        if self.stuff.is_none() {
            self.stuff = Some(shader_material("bird.gdshader"));
        }
        self.revise_stuff();
        self.upload();
    }
    fn revise_stuff(&mut self) {
        let Some(mut m) = self.stuff.clone() else {
            return;
        };
        m.param("color", &self.tint.to_variant());
        m.param("tip_color", &self.hint_tint.to_variant());
        m.param("min_texels", &self.min_texels.to_variant());
        m.param("max_texels", &self.max_texels.to_variant());
        m.param("texel_size", &self.texel.to_variant());
        m.param("span", &self.size.to_variant());
        m.param("flap_speed", &self.flap_speed.to_variant());
        m.param(
            "pt_flags",
            &object_flags(1, self.outline, 0, true).to_variant(),
        );
        if let Some(f) = &mut self.flock {
            f.set_material_override(&m);
        }
    }
    fn home(&self) -> Vector3 {
        if self.base().is_inside_tree() {
            self.base().get_global_position() + Vector3::new(0.0, self.altitude, 0.0)
        } else {
            Vector3::new(0.0, self.altitude, 0.0)
        }
    }
    fn ground(&self, p: Vector3) -> f32 {
        let fallback = self.base().get_global_position().y;
        match self.terrain.as_ref().filter(|t| t.is_instance_valid()) {
            Some(t) => t.bind().get_height_at(p, fallback),
            None => fallback,
        }
    }
    fn start_landing(&mut self) {
        let home = self.home();
        let mut best = (Vector3::ZERO, f32::MIN);
        for _ in 0..24 {
            let p = home
                + Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.radius * 0.8005;
            let h = self.ground(p);
            if h > best.1 {
                best = (Vector3::new(p.x, h, p.z), h);
            }
        }
        if best.1 == f32::MIN {
            return;
        }
        for i in 0..self.birds.len() {
            let off =
                Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.landing_spread;
            let mut spot = best.0 + off;
            spot.y = self.ground(spot);
            self.birds[i].state = State::Land(spot);
            self.birds[i].timer = 0.0;
        }
    }
    fn scatter(&mut self) {
        for b in &mut self.birds {
            if b.state != State::Fly {
                let dir = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()).normalized();
                b.vel = dir * self.speed + Vector3::UP * self.speed * self.takeoff_lift;
                b.state = State::Fly;
                b.timer = 0.0;
            }
        }
        self.flock_timer = 0.0;
    }
    fn simulate(&mut self, dt: f32) {
        if self.birds.is_empty() {
            return;
        }
        let home = self.home();
        self.orbit = (self.orbit + dt * self.speed / self.radius.max(0.1005))
            .rem_euclid(std::f32::consts::TAU);
        let goal = home + Vector3::new(self.orbit.cos(), 0.0, self.orbit.sin()) * self.radius;
        self.flock_timer += dt;
        let any_fly = self.birds.iter().any(|b| b.state == State::Fly);
        if self.landing
            && any_fly
            && self.birds.iter().all(|b| b.state == State::Fly)
            && self.flock_timer > self.land_interval
        {
            self.start_landing();
        }
        let perched: Vec<Vector3> = self
            .birds
            .iter()
            .filter(|b| b.state == State::Perched)
            .map(|b| b.pos)
            .collect();
        if !perched.is_empty() {
            let long = self
                .birds
                .iter()
                .any(|b| b.state == State::Perched && b.timer > self.perch_seconds);
            let node = self.to_gd().upcast::<Node>();
            let near = self.scare_distance > 0.0
                && active_camera(&node).is_some_and(|c| {
                    let f = camera_focus(&c, perched[0].y);
                    perched
                        .iter()
                        .any(|p| Vector2::new(p.x - f.x, p.z - f.z).length() < self.scare_distance)
                });
            if long || near {
                self.scatter();
            }
        }
        let view = if self.camera_clearance > 0.0 {
            let node = self.to_gd().upcast::<Node>();
            active_camera(&node).map(|c| {
                let o = c.get_global_position();
                let f = camera_focus(&c, home.y - self.altitude);
                let d = f - o;
                let reach = self.camera_clearance * 3.0;
                let end = if d.length() > reach {
                    o + d.normalized() * reach
                } else {
                    f
                };
                (o, end)
            })
        } else {
            None
        };
        self.view = view;
        let n = self.birds.len();
        let snapshot: Vec<(Vector3, Vector3, bool)> = self
            .birds
            .iter()
            .map(|b| (b.pos, b.vel, b.state == State::Fly))
            .collect();
        let nd2 = self.neighbor_distance * self.neighbor_distance;
        for i in 0..n {
            let mut b = self.birds[i];
            b.timer += dt;
            match b.state {
                State::Fly => {
                    let (mut sep, mut ali, mut coh, mut k) =
                        (Vector3::ZERO, Vector3::ZERO, Vector3::ZERO, 0.0f32);
                    for (j, (p, v, fly)) in snapshot.iter().enumerate() {
                        if j == i || !fly {
                            continue;
                        }
                        let d = b.pos - *p;
                        let d2 = d.length_squared();
                        if d2 < nd2 && d2 > 1e-6 {
                            sep += d / d2;
                            ali += *v;
                            coh += *p;
                            k += 1.0;
                        }
                    }
                    let mut desired = (goal - b.pos).normalized() * self.speed;
                    if k > 0.0 {
                        desired += sep * self.separation * self.speed * 0.5;
                        desired += (ali / k - b.vel) * self.alignment;
                        desired += (coh / k - b.pos) * self.cohesion;
                    }
                    if let Some((o, f)) = view {
                        let (q, d) = closest_on_segment(b.pos, o, f);
                        let c = self.camera_clearance;
                        if d < c {
                            let away = if d > 1e-3 {
                                (b.pos - q) / d
                            } else {
                                Vector3::UP
                            };
                            desired += (away + Vector3::UP * self.camera_avoid_lift) * (c - d) / c
                                * self.speed
                                * self.camera_avoid_strength;
                        }
                    }
                    let g = self.ground(b.pos);
                    let floor = g + self.ground_clearance;
                    if b.pos.y < floor {
                        desired.y += (floor - b.pos.y) * self.ground_push;
                    }
                    b.vel += (desired - b.vel) * (self.steer_rate * dt).min(1.0);
                    let s = b.vel.length();
                    let lo = self.speed * self.min_speed_ratio;
                    let hi = (self.speed * self.max_speed_ratio).max(lo);
                    if s > 1e-4 {
                        b.vel = b.vel * (s.clamp(lo, hi) / s);
                    } else {
                        b.vel = Vector3::new(1.0, 0.0, 0.0) * lo;
                    }
                    b.pos += b.vel * dt;
                }
                State::Land(spot) => {
                    let to = spot - b.pos;
                    let d = to.length();
                    if d < self.touchdown_distance {
                        b.pos = spot;
                        b.vel = Vector3::new(b.vel.x, 0.0, b.vel.z).normalized() * 0.0105;
                        b.state = State::Perched;
                        b.timer = 0.0;
                    } else {
                        let slow = (d / self.approach_slow_distance.max(1e-3))
                            .clamp(self.approach_min_speed_ratio.min(1.0), 1.0);
                        let want = to / d * (self.speed * slow);
                        b.vel += (want - b.vel)
                            * (self.steer_rate * self.approach_steer_boost * dt).min(1.0);
                        b.pos += b.vel * dt;
                    }
                }
                State::Perched => {}
            }
            self.birds[i] = b;
        }
    }
    fn upload(&mut self) {
        let Some(mut mm) = self
            .flock
            .as_ref()
            .filter(|f| f.is_instance_valid())
            .and_then(|f| f.get_multimesh())
        else {
            return;
        };
        if mm.get_instance_count() as usize != self.birds.len() {
            return;
        }
        let inv = if self.base().is_inside_tree() {
            self.base().get_global_transform().affine_inverse()
        } else {
            Transform3D::IDENTITY
        };
        let s = self.size;
        let hide = self.view.map(|(o, _)| (o, self.camera_clearance * 0.5));
        self.buffer.purge();
        for b in &self.birds {
            let s = match hide {
                Some((o, r)) if b.state != State::Perched && (b.pos - o).length() < r => 0.0,
                _ => s,
            };
            let flat = Vector3::new(b.vel.x, 0.0, b.vel.z);
            let fwd = if flat.length_squared() > 1e-8 {
                flat.normalized()
            } else {
                Vector3::FORWARD
            };
            let right = Vector3::UP.cross(fwd).normalized();
            let basis = Basis::from_cols(right * s, Vector3::UP * s, fwd * s);
            let perched = if b.state == State::Perched { 1.0 } else { 0.0 };
            let glide = if matches!(b.state, State::Land(_)) || b.vel.y < -self.glide_dive_ratio * self.speed {
                1.0
            } else {
                0.0
            };
            self.buffer.stack(
                inv * Transform3D::new(basis, b.pos),
                Color::from_rgba(b.phase, perched, glide, 0.0),
            );
        }
        let r = self.radius * 1.6005 + 4.0;
        let a = self.altitude;
        self.buffer.upload(
            &mut mm,
            Aabb::new(
                Vector3::new(-r, -a - 4.0, -r),
                Vector3::new(2.0 * r, a + r, 2.0 * r),
            ),
        );
    }
}
fn closest_on_segment(p: Vector3, a: Vector3, b: Vector3) -> (Vector3, f32) {
    let ab = b - a;
    let l2 = ab.length_squared();
    let t = if l2 > 1e-8 {
        ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = a + ab * t;
    (q, (p - q).length())
}
fn bird_mesh() -> Gd<ArrayMesh> {
    let v = |x: f32, y: f32, z: f32| Vector3::new(x, y, z);
    let none = Vector2::ZERO;
    let (nose, back, l, r, top, belly) = (
        v(0.0, 0.0305, 0.2705),
        v(0.0, 0.0205, -0.1805),
        v(-0.0705, 0.0, 0.0305),
        v(0.0705, 0.0, 0.0305),
        v(0.0, 0.0705, 0.0505),
        v(0.0, -0.0405, 0.0305),
    );
    let mut tris: Vec<[(Vector3, Vector2); 3]> = vec![
        [(nose, none), (top, none), (l, none)],
        [(nose, none), (r, none), (top, none)],
        [(back, none), (l, none), (top, none)],
        [(back, none), (top, none), (r, none)],
        [(nose, none), (l, none), (belly, none)],
        [(nose, none), (belly, none), (r, none)],
        [(back, none), (belly, none), (l, none)],
        [(back, none), (r, none), (belly, none)],
    ];
    for side in [-1.0f32, 1.0] {
        let x = |a: f32| a * side;
        let w = |k: f32| Vector2::new(k, 0.0);
        let (rf, rb) = (
            (v(x(0.0505), 0.0205, 0.1205), w(0.0)),
            (v(x(0.0505), 0.0205, -0.1005), w(0.0)),
        );
        let (ef, eb) = (
            (v(x(0.2605), 0.0205, 0.0705), w(0.5)),
            (v(x(0.2605), 0.0205, -0.1405), w(0.5)),
        );
        let (tf, tb) = (
            (v(x(0.5), 0.0205, -0.0605), w(1.0)),
            (v(x(0.4605), 0.0205, -0.2005), w(1.0)),
        );
        let mut wing = vec![[rf, ef, rb], [rb, ef, eb], [ef, tf, eb], [eb, tf, tb]];
        if side > 0.0 {
            for tri in &mut wing {
                tri.swap(1, 2);
            }
        }
        tris.extend(wing);
    }
    let mut pos = PackedVector3Array::new();
    let mut uv = PackedVector2Array::new();
    let mut nrm = PackedVector3Array::new();
    for tri in &tris {
        let n = (tri[2].0 - tri[0].0)
            .cross(tri[1].0 - tri[0].0)
            .normalized();
        for (p, u) in tri {
            pos.push(*p);
            uv.push(*u);
            nrm.push(n);
        }
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    mesh
}
#[godot_api]
impl PixelBirds {
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = self.layers | globals::meta_mask();
        if let Some(f) = &mut self.flock {
            f.set_layer_mask(mask);
        }
    }
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if gate(&self.to_gd(), Feature::Birds) && self.get_flock().is_none() {
            self.reassemble();
            self.refresh_pixel_settings();
        }
    }
    #[func]
    pub fn get_flock(&self) -> Option<Gd<MultiMeshInstance3D>> {
        self.flock.clone().filter(|f| f.is_instance_valid())
    }
    #[func]
    fn set_terrain(&mut self, v: Option<Gd<MarchingSquaresTerrain>>) {
        self.terrain = v;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_pixel_renderer(&mut self, v: Option<Gd<crate::render::renderer::PixelRenderer>>) {
        self.pixel_renderer = v;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_layers(&mut self, v: u32) {
        self.layers = v;
        self.refresh_pixel_settings();
    }
    #[func]
    fn scare(&mut self) {
        self.scatter();
    }
    #[func]
    fn land_now(&mut self) {
        self.start_landing();
    }
    #[func]
    fn get_bird_positions(&self) -> PackedVector3Array {
        self.birds.iter().map(|b| b.pos).collect()
    }
    #[func]
    fn assign_tally(&mut self, v: i32) {
        self.count = v.max(1);
        self.reassemble();
    }
    #[func]
    fn set_seed(&mut self, v: i32) {
        self.seed = v;
        self.reassemble();
    }
    #[func]
    fn set_flap_speed(&mut self, v: f32) {
        self.flap_speed = v.max(0.0);
        self.revise_stuff();
    }
    #[func]
    fn assign_measure(&mut self, v: f32) {
        self.size = v.max(0.0105);
        self.revise_stuff();
    }
    #[func]
    fn assign_tint(&mut self, v: Color) {
        self.tint = v;
        self.revise_stuff();
    }
    #[func]
    fn assign_hint_tint(&mut self, v: Color) {
        self.hint_tint = v;
        self.revise_stuff();
    }
    #[func]
    fn set_min_texels(&mut self, v: f32) {
        self.min_texels = v.max(0.0).min(self.max_texels);
        self.revise_stuff();
    }
    #[func]
    fn set_max_texels(&mut self, v: f32) {
        self.max_texels = v.max(self.min_texels);
        self.revise_stuff();
    }
    #[func]
    fn set_outline(&mut self, v: bool) {
        self.outline = v;
        self.revise_stuff();
    }
}
