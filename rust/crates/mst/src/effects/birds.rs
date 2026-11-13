//! `PixelBirds` — a small boids flock of 2-frame flapping pixel birds that circle, land on high
//! ground and scatter when the camera comes close.
//!
//! After the ideas in docs/port/03-refs-features.md §1.5 / #16 (boid flock with
//! neighbour distance / speed / rotation coefficient, landing spots and scaring).
//!
//! `PixelBirds` is a plain `Node3D` that owns its `MultiMeshInstance3D` as an *internal* child
//! (`Flock`), so the inspector shows only flock settings and Godot's *MultiMesh* menu does not
//! appear. Scenes saved while it was a `MultiMeshInstance3D` still load (old mesh-instance
//! properties are accepted and dropped in `on_set`; `layers` is kept).

use godot::classes::camera_3d::ProjectionType;
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{ArrayMesh, INode3D, MultiMesh, MultiMeshInstance3D, Node, Node3D, ShaderMaterial};
use godot::prelude::*;

use super::{Rng, active_camera, camera_focus, find_terrain, ground_height};
use crate::globals;
use crate::util::*;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Fly,
    /// Gliding down to a landing spot (world position).
    Land(Vector3),
    Perched,
}

#[derive(Clone, Copy)]
struct Bird {
    pos: Vector3,
    vel: Vector3,
    state: State,
    phase: f32,
    /// Seconds since the state changed.
    timer: f32,
}

/// A flock of pixel birds flying around this node. Each bird is a tiny mesh (body + two wings)
/// whose wings snap between two frames (up / down), with glides in between; on the ground the
/// wings fold. Birds take part in the pixel pipeline (metadata, texel snapping), so they render
/// as crisp sprites at the renderer's pixel size.
///
/// The flock circles at `altitude` above the node; every `land_interval` seconds it settles on
/// the highest ground nearby (needs a `MarchingSquaresTerrain` in the scene), and takes off
/// after `perch_seconds` or when the camera focus comes within `scare_distance`.
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelBirds {
    #[export(range = (1.0, 256.0, 1.0, or_greater))]
    #[var(set = set_count)]
    count: i32,
    /// Radius of the circle the flock flies around the node.
    #[export(range = (1.0, 200.0, 0.1, or_greater))]
    radius: f32,
    /// Flying height above the node.
    #[export(range = (0.0, 100.0, 0.1, or_greater))]
    altitude: f32,
    /// Cruise speed (m/s).
    #[export(range = (0.1, 30.0, 0.1))]
    speed: f32,
    /// Birds closer than this influence each other.
    #[export(range = (0.1, 20.0, 0.05))]
    neighbor_distance: f32,
    #[export(range = (0.0, 4.0, 0.01))]
    separation: f32,
    #[export(range = (0.0, 4.0, 0.01))]
    alignment: f32,
    #[export(range = (0.0, 4.0, 0.01))]
    cohesion: f32,
    /// How fast a bird turns towards its steering target (MD's rotation coefficient).
    #[export(range = (0.1, 10.0, 0.05))]
    turn_rate: f32,
    /// Wing beats per second.
    #[export(range = (0.0, 20.0, 0.1))]
    #[var(set = set_flap_speed)]
    flap_speed: f32,
    /// Wingspan (m).
    #[export(range = (0.05, 4.0, 0.01))]
    #[var(set = set_size)]
    size: f32,
    /// Body / wing color (toon lit; pick a palette tone, not black, so birds read as
    /// birds rather than glyphs).
    #[export]
    #[var(set = set_color)]
    color: Color,
    /// Wing tip and tail color.
    #[export]
    #[var(set = set_tip_color)]
    tip_color: Color,
    /// Smallest on-screen wingspan in low-res texels (birds far away are drawn at least this
    /// big, so the silhouette and flap stay readable). 0 = true size.
    #[export(range = (0.0, 32.0, 1.0))]
    #[var(set = set_min_texels)]
    min_texels: f32,
    /// Largest on-screen wingspan in low-res texels (birds close to the camera). 0 = no cap.
    #[export(range = (0.0, 64.0, 1.0))]
    #[var(set = set_max_texels)]
    max_texels: f32,
    /// Flying birds steer out of a tube of this radius (m) around the first stretch (3 x the
    /// radius) of the view ray, so they never cross the near view as big smudges; birds that still end
    /// up closer than half of it to the camera are hidden. 0 = off.
    #[export(range = (0.0, 50.0, 0.1))]
    camera_clearance: f32,
    /// Draw renderer outlines around the birds.
    #[export]
    #[var(set = set_outline)]
    outline: bool,
    #[export]
    #[var(set = set_seed)]
    seed: i32,
    /// Render layers of the flock (the pixel renderer's metadata layer is added automatically).
    /// Same name as the old MultiMeshInstance3D property, so saved scenes keep it.
    #[export(flags_3d_render)]
    #[var(set = set_layers)]
    layers: u32,

    #[export_group(name = "Landing")]
    /// Settle on high ground from time to time.
    #[export]
    landing: bool,
    /// Seconds of flying between landings.
    #[export(range = (1.0, 600.0, 0.5))]
    land_interval: f32,
    /// Seconds on the ground before taking off again.
    #[export(range = (0.5, 600.0, 0.5))]
    perch_seconds: f32,
    /// Perched birds scatter when the camera focus comes closer than this (0 = never).
    #[export(range = (0.0, 100.0, 0.1))]
    scare_distance: f32,
    /// Simulate in the editor too.
    #[export_subgroup(name = "Preview", prefix = "preview_")]
    #[export]
    preview_in_editor: bool,

    birds: Vec<Bird>,
    /// Camera position and focus of the last simulated frame.
    view: Option<(Vector3, Vector3)>,
    flock_timer: f32,
    orbit: f32,
    terrain: Option<Gd<Node>>,
    /// The scene's PixelRenderer (low-res height for the on-screen size limits).
    renderer: Option<Gd<Node>>,
    texel: Vector2,
    material: Option<Gd<ShaderMaterial>>,
    rng: Rng,
    /// Internal flock node (never saved).
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
            speed: 4.5,
            neighbor_distance: 2.5,
            separation: 1.4,
            alignment: 0.6,
            cohesion: 0.5,
            turn_rate: 2.2,
            flap_speed: 4.0,
            size: 0.8,
            color: Color::from_rgb(0.9, 0.88, 0.82),
            tip_color: Color::from_rgb(0.36, 0.34, 0.4),
            min_texels: 9.0,
            max_texels: 18.0,
            camera_clearance: 6.0,
            outline: false,
            seed: 7,
            landing: true,
            land_interval: 30.0,
            perch_seconds: 10.0,
            scare_distance: 5.0,
            preview_in_editor: true,
            birds: Vec::new(),
            view: None,
            flock_timer: 0.0,
            orbit: 0.0,
            terrain: None,
            renderer: None,
            texel: Vector2::ZERO,
            material: None,
            rng: Rng(7),
            layers: 1,
            flock: None,
            base,
        }
    }

    fn on_set(&mut self, property: StringName, _value: Variant) -> bool {
        // Compatibility with scenes saved when PixelBirds was a MultiMeshInstance3D.
        super::fireflies::is_legacy_multimesh_prop(&property.to_string())
    }

    fn ready(&mut self) {
        self.to_gd().add_to_group(globals::REFRESH_GROUP);
        self.rebuild();
        self.refresh_pixel_settings();
        self.base_mut().set_process(true);
    }

    fn process(&mut self, delta: f64) {
        if is_editor() && !self.preview_in_editor {
            return;
        }
        let dt = (delta as f32).min(0.1);
        self.simulate(dt);
        self.upload();
        self.sync_texels();
    }
}

impl PixelBirds {
    /// The internal flock node (created on first use).
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

    /// Passes the PixelRenderer's texel size (world units, as a function of camera distance)
    /// to the shader, which keeps the on-screen wingspan within `min_texels`..`max_texels`.
    fn sync_texels(&mut self) {
        if self.renderer.as_ref().is_none_or(|r| !r.is_instance_valid()) {
            let node = self.to_gd().upcast::<Node>();
            self.renderer = find_renderer(&node);
        }
        let node = self.to_gd().upcast::<Node>();
        let texel = match (&mut self.renderer, active_camera(&node)) {
            (Some(r), Some(cam)) => {
                let h = r.call("get_low_res_size", &[]).try_to::<Vector2i>().map(|v| v.y as f32).unwrap_or(0.0);
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
            if let Some(mut m) = self.material.clone() {
                m.set_shader_parameter("texel_size", &texel.to_variant());
            }
        }
    }

    fn rebuild(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        let n = self.count.max(1) as usize;
        self.rng = Rng(self.seed as u64 ^ 0xB1_2D5);
        let home = self.home();
        self.birds = (0..n)
            .map(|_| {
                let a = self.rng.next_f32() * std::f32::consts::TAU;
                let r = self.radius * (0.6 + 0.4 * self.rng.next_f32());
                let pos = home + Vector3::new(a.cos() * r, self.rng.signed() * 1.5, a.sin() * r);
                let vel = Vector3::new(-a.sin(), 0.0, a.cos()) * self.speed;
                Bird { pos, vel, state: State::Fly, phase: self.rng.next_f32(), timer: 0.0 }
            })
            .collect();
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_use_custom_data(true);
        mm.set_mesh(&bird_mesh());
        mm.set_instance_count(n as i32);
        self.flock().set_multimesh(&mm);
        if self.material.is_none() {
            self.material = Some(shader_material("bird.gdshader"));
        }
        self.update_material();
        self.upload();
    }

    fn update_material(&mut self) {
        let Some(mut m) = self.material.clone() else { return };
        m.set_shader_parameter("color", &self.color.to_variant());
        m.set_shader_parameter("tip_color", &self.tip_color.to_variant());
        m.set_shader_parameter("min_texels", &self.min_texels.to_variant());
        m.set_shader_parameter("max_texels", &self.max_texels.to_variant());
        m.set_shader_parameter("texel_size", &self.texel.to_variant());
        m.set_shader_parameter("span", &self.size.to_variant());
        m.set_shader_parameter("flap_speed", &self.flap_speed.to_variant());
        m.set_shader_parameter("pt_flags", &object_flags(1, self.outline, 0, true).to_variant());
        if let Some(f) = &mut self.flock {
            f.set_material_override(&m);
        }
    }

    /// Flock centre in world space (the circle is around the node at `altitude`).
    fn home(&self) -> Vector3 {
        if self.base().is_inside_tree() {
            self.base().get_global_position() + Vector3::new(0.0, self.altitude, 0.0)
        } else {
            Vector3::new(0.0, self.altitude, 0.0)
        }
    }

    fn ground(&mut self, p: Vector3) -> f32 {
        if self.terrain.as_ref().is_none_or(|t| !t.is_instance_valid()) {
            let node = self.to_gd().upcast::<Node>();
            self.terrain = find_terrain(&node);
        }
        let fallback = self.base().get_global_position().y;
        ground_height(&mut self.terrain, p, fallback)
    }

    /// Picks landing spots on the highest of a few sampled points near the flock centre.
    fn start_landing(&mut self) {
        let home = self.home();
        let mut best = (Vector3::ZERO, f32::MIN);
        for _ in 0..24 {
            let p = home + Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * self.radius * 0.8;
            let h = self.ground(p);
            if h > best.1 {
                best = (Vector3::new(p.x, h, p.z), h);
            }
        }
        if best.1 == f32::MIN {
            return;
        }
        for i in 0..self.birds.len() {
            let off = Vector3::new(self.rng.signed(), 0.0, self.rng.signed()) * 1.2;
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
                b.vel = dir * self.speed + Vector3::UP * self.speed * 1.2;
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
        self.orbit = (self.orbit + dt * self.speed / self.radius.max(0.1)).rem_euclid(std::f32::consts::TAU);
        let goal = home + Vector3::new(self.orbit.cos(), 0.0, self.orbit.sin()) * self.radius;

        // Flock-level state: landing / scaring.
        self.flock_timer += dt;
        let any_fly = self.birds.iter().any(|b| b.state == State::Fly);
        if self.landing && any_fly && self.birds.iter().all(|b| b.state == State::Fly) && self.flock_timer > self.land_interval {
            self.start_landing();
        }
        let perched: Vec<Vector3> = self.birds.iter().filter(|b| b.state == State::Perched).map(|b| b.pos).collect();
        if !perched.is_empty() {
            let long = self.birds.iter().any(|b| b.state == State::Perched && b.timer > self.perch_seconds);
            let node = self.to_gd().upcast::<Node>();
            let near = self.scare_distance > 0.0
                && active_camera(&node).is_some_and(|c| {
                    let f = camera_focus(&c, perched[0].y);
                    perched.iter().any(|p| Vector2::new(p.x - f.x, p.z - f.z).length() < self.scare_distance)
                });
            if long || near {
                self.scatter();
            }
        }

        // Near-camera zone: the first 3 x clearance of the view ray towards the camera's focus
        // (birds further down the ray, e.g. high over the player, stay).
        let view = if self.camera_clearance > 0.0 {
            let node = self.to_gd().upcast::<Node>();
            active_camera(&node).map(|c| {
                let o = c.get_global_position();
                let f = camera_focus(&c, home.y - self.altitude);
                let d = f - o;
                let reach = self.camera_clearance * 3.0;
                let end = if d.length() > reach { o + d.normalized() * reach } else { f };
                (o, end)
            })
        } else {
            None
        };
        self.view = view;

        let n = self.birds.len();
        let snapshot: Vec<(Vector3, Vector3, bool)> = self.birds.iter().map(|b| (b.pos, b.vel, b.state == State::Fly)).collect();
        let nd2 = self.neighbor_distance * self.neighbor_distance;
        for i in 0..n {
            let mut b = self.birds[i];
            b.timer += dt;
            match b.state {
                State::Fly => {
                    let (mut sep, mut ali, mut coh, mut k) = (Vector3::ZERO, Vector3::ZERO, Vector3::ZERO, 0.0f32);
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
                    // Keep out of the camera's near view (steer sideways and up).
                    if let Some((o, f)) = view {
                        let (q, d) = closest_on_segment(b.pos, o, f);
                        let c = self.camera_clearance;
                        if d < c {
                            let away = if d > 1e-3 { (b.pos - q) / d } else { Vector3::UP };
                            desired += (away + Vector3::UP * 0.5) * (c - d) / c * self.speed * 2.5;
                        }
                    }
                    // Keep clear of the ground.
                    let g = self.ground(b.pos);
                    if b.pos.y < g + 2.0 {
                        desired.y += (g + 2.0 - b.pos.y) * 3.0;
                    }
                    b.vel += (desired - b.vel) * (self.turn_rate * dt).min(1.0);
                    let s = b.vel.length();
                    let (lo, hi) = (self.speed * 0.6, self.speed * 1.3);
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
                    if d < 0.15 {
                        b.pos = spot;
                        b.vel = Vector3::new(b.vel.x, 0.0, b.vel.z).normalized() * 0.01;
                        b.state = State::Perched;
                        b.timer = 0.0;
                    } else {
                        // Slow down on approach.
                        let want = to / d * (self.speed * (d / 4.0).clamp(0.25, 1.0));
                        b.vel += (want - b.vel) * (self.turn_rate * 1.5 * dt).min(1.0);
                        b.pos += b.vel * dt;
                    }
                }
                State::Perched => {}
            }
            self.birds[i] = b;
        }
    }

    /// Writes the bird transforms (heading + bank) and frame data into the MultiMesh.
    fn upload(&mut self) {
        let Some(mut mm) = self.flock.as_ref().filter(|f| f.is_instance_valid()).and_then(|f| f.get_multimesh()) else { return };
        if mm.get_instance_count() as usize != self.birds.len() {
            return;
        }
        let inv = if self.base().is_inside_tree() { self.base().get_global_transform().affine_inverse() } else { Transform3D::IDENTITY };
        let s = self.size;
        let hide = self.view.map(|(o, _)| (o, self.camera_clearance * 0.5));
        for (i, b) in self.birds.iter().enumerate() {
            // Too close to the camera (e.g. a bird that just took off under it): hide.
            let s = match hide {
                Some((o, r)) if b.state != State::Perched && (b.pos - o).length() < r => 0.0,
                _ => s,
            };
            let flat = Vector3::new(b.vel.x, 0.0, b.vel.z);
            let fwd = if flat.length_squared() > 1e-8 { flat.normalized() } else { Vector3::FORWARD };
            let right = Vector3::UP.cross(fwd).normalized();
            let basis = Basis::from_cols(right * s, Vector3::UP * s, fwd * s);
            mm.set_instance_transform(i as i32, inv * Transform3D::new(basis, b.pos));
            let perched = if b.state == State::Perched { 1.0 } else { 0.0 };
            // Glide while descending to land or when steadily sinking.
            let glide = if matches!(b.state, State::Land(_)) || b.vel.y < -0.4 * self.speed { 1.0 } else { 0.0 };
            mm.set_instance_custom_data(i as i32, Color::from_rgba(b.phase, perched, glide, 0.0));
        }
        // Birds roam far from the node: cull with the whole flight area.
        let r = self.radius * 1.6 + 4.0;
        let a = self.altitude;
        mm.set_custom_aabb(Aabb::new(Vector3::new(-r, -a - 4.0, -r), Vector3::new(2.0 * r, a + r, 2.0 * r)));
    }
}

/// First `PixelRenderer` in the scene, if any.
fn find_renderer(node: &Gd<Node>) -> Option<Gd<Node>> {
    if !node.is_inside_tree() {
        return None;
    }
    let tree = node.get_tree();
    let root: Gd<Node> = if is_editor() {
        godot::classes::EditorInterface::singleton().get_edited_scene_root()?
    } else {
        tree.get_current_scene().unwrap_or_else(|| tree.get_root().upcast())
    };
    root.find_children_ex("*").type_("PixelRenderer").recursive(true).owned(false).done().iter_shared().next()
}

/// Closest point on segment `a`-`b` to `p`, and its distance.
fn closest_on_segment(p: Vector3, a: Vector3, b: Vector3) -> (Vector3, f32) {
    let ab = b - a;
    let l2 = ab.length_squared();
    let t = if l2 > 1e-8 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let q = a + ab * t;
    (q, (p - q).length())
}

/// Bird in its local frame (+Z forward, unit wingspan): a small body (back ridge + belly) and
/// two broad wings with an elbow. UV.x = wing weight the shader flaps / folds (0 root, 0.5
/// elbow, 1 tip). bird.gdshader lays the mesh out in the camera plane (sprite), so the z extent
/// becomes the sprite's height: deep enough that wings stay >= 1 texel thick.
fn bird_mesh() -> Gd<ArrayMesh> {
    let v = |x: f32, y: f32, z: f32| Vector3::new(x, y, z);
    let none = Vector2::ZERO;
    let (nose, back, l, r, top, belly) =
        (v(0.0, 0.03, 0.27), v(0.0, 0.02, -0.18), v(-0.07, 0.0, 0.03), v(0.07, 0.0, 0.03), v(0.0, 0.07, 0.05), v(0.0, -0.04, 0.03));
    let mut tris: Vec<[(Vector3, Vector2); 3]> = vec![
        // body: back ridge + belly
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
        let (rf, rb) = ((v(x(0.05), 0.02, 0.12), w(0.0)), (v(x(0.05), 0.02, -0.1), w(0.0)));
        let (ef, eb) = ((v(x(0.26), 0.02, 0.07), w(0.5)), (v(x(0.26), 0.02, -0.14), w(0.5)));
        let (tf, tb) = ((v(x(0.5), 0.02, -0.06), w(1.0)), (v(x(0.46), 0.02, -0.2), w(1.0)));
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
        let n = (tri[2].0 - tri[0].0).cross(tri[1].0 - tri[0].0).normalized();
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
    /// Re-applies the metadata layer (called when the meta layer changes).
    #[func]
    fn refresh_pixel_settings(&mut self) {
        let mask = self.layers | globals::meta_mask();
        if let Some(f) = &mut self.flock {
            f.set_layer_mask(mask);
        }
    }

    /// The internal flock instance (read-only access for tools and tests).
    #[func]
    pub fn get_flock(&self) -> Option<Gd<MultiMeshInstance3D>> {
        self.flock.clone().filter(|f| f.is_instance_valid())
    }

    #[func]
    fn set_layers(&mut self, v: u32) {
        self.layers = v;
        self.refresh_pixel_settings();
    }

    /// Makes perched birds take off now.
    #[func]
    fn scare(&mut self) {
        self.scatter();
    }

    /// Sends the flock to land on the highest ground nearby now.
    #[func]
    fn land_now(&mut self) {
        self.start_landing();
    }

    /// World positions of the birds.
    #[func]
    fn get_bird_positions(&self) -> PackedVector3Array {
        self.birds.iter().map(|b| b.pos).collect()
    }

    #[func]
    fn set_count(&mut self, v: i32) {
        self.count = v.max(1);
        self.rebuild();
    }
    #[func]
    fn set_seed(&mut self, v: i32) {
        self.seed = v;
        self.rebuild();
    }
    #[func]
    fn set_flap_speed(&mut self, v: f32) {
        self.flap_speed = v.max(0.0);
        self.update_material();
    }
    #[func]
    fn set_size(&mut self, v: f32) {
        self.size = v.max(0.01);
        self.update_material();
    }
    #[func]
    fn set_color(&mut self, v: Color) {
        self.color = v;
        self.update_material();
    }
    #[func]
    fn set_tip_color(&mut self, v: Color) {
        self.tip_color = v;
        self.update_material();
    }
    #[func]
    fn set_min_texels(&mut self, v: f32) {
        self.min_texels = v.max(0.0);
        self.update_material();
    }
    #[func]
    fn set_max_texels(&mut self, v: f32) {
        self.max_texels = v.max(0.0);
        self.update_material();
    }
    #[func]
    fn set_outline(&mut self, v: bool) {
        self.outline = v;
        self.update_material();
    }
}
