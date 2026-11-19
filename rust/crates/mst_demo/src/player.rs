//! `MstDemoPlayer` — the demo's third-person knight (port of godot-pixelizer-3d `demo/player.gd`),
//! filmed by a standard Phantom Camera third-person rig (`demo/player.tscn`).
//!
//! * WASD moves relative to the camera's view direction (Shift walks), Space jumps, gravity from
//!   the project settings; only the `visual` child turns (smoothly) to face the movement.
//! * Camera: a `PhantomCamera3D` in follow mode THIRD_PERSON follows the body's `CameraTarget`
//!   on its spring arm, with the pcam's own follow damping; a `PhantomCameraHost` under the real
//!   `Camera3D` applies it. As in Phantom Camera's third-person example, the player only feeds
//!   input to the pcam: any mouse button held + drag (the mouse is not captured, so the F3
//!   diagnostics tuner stays usable) and the arrow keys orbit
//!   (`set_third_person_rotation_degrees`), the wheel zooms (`set_spring_length`). The scene
//!   authors the start view (the pcam's rotation and spring length, `CameraTarget` raised
//!   above the body so the knight sits below the frame centre) and the lens (`Camera3D` fov).
//!   The spring arm's collision is off in the scene: a camera pulled in front of a cliff would
//!   zoom the view in to a few metres; the x-ray shows the knight through occluders instead.
//! * Pixel snapping: the camera carries a `pixel_snap_distance` meta (the spring length), so
//!   `PixelRenderer` snaps it to the texel lattice of the focus plane and pixels stay locked to
//!   the world around the player while walking.
//! * X-ray: every mesh under `visual` gets a two-pass `material_overlay` (`xray_mask_shader`,
//!   `xray_shader`): pass 1 stencil-marks the knight's visible pixels, pass 2 draws the rest of
//!   its shape without a depth test as a flat, dithered palette silhouette. Both discard in the
//!   metadata / id passes, so the pixel resolve does not outline the silhouette. Its `amount`
//!   fades in when `xray_trigger_points` of five body rays (feet to head) toward the camera
//!   hit a real occluder (the terrain's own height march plus physics bodies on `xray_mask`;
//!   hits lower than `xray_min_height` above the feet do not count), and out when fewer than
//!   `xray_trigger_points - 1` do, so grass, small props and steps never show it.
//! * Animation: `Knight.fbx` ships the KayKit `Rig_Medium` rig without clips. With
//!   `animation_source` set (the CC0 KayKit clips GLB), [`crate::anim::Animator`] shares those
//!   clips onto the knight's skeleton and blends idle / walk / run by ground speed plus jump
//!   start / air / land through an `AnimationTree`. Without it the skeleton is posed procedurally
//!   (idle breathing, a walk/run cycle, a tucked jump pose). A visual that ships its own `Idle` /
//!   `Run` / `Jump` / `Fall` clips (the courtyard fox, `demo/fox_player.tscn`) plays those
//!   instead ([`crate::anim::ClipPlayer`]).
//!
//! The player starts where the rig is placed in the scene; with `spawn_on_terrain` it waits
//! there (no gravity) until the terrain reports a height and a physics ray finds its collider,
//! then stands on the surface. The terrain comes from the exported slot, else from the scene
//! that owns the player (top of the owner chain, or the parent chain when instanced from code);
//! `SceneTree::current_scene` is never used. Phantom Camera is a separate, optional library and
//! is reached through dynamic calls.

use std::f32::consts::{PI, TAU};

use godot::classes::{
    Camera3D, CharacterBody3D, ICharacterBody3D, Input, InputEvent, InputEventKey, InputEventMouseButton,
    InputEventMouseMotion, InputMap, Material, MeshInstance3D, PackedScene, PhysicsRayQueryParameters3D,
    ProjectSettings, Shader, ShaderMaterial, Skeleton3D,
};

use crate::anim::{Animator, ClipPlayer, Gait};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

const ACT_FORWARD: &str = "player_forward";
const ACT_BACK: &str = "player_back";
const ACT_LEFT: &str = "player_left";
const ACT_RIGHT: &str = "player_right";
const ACT_JUMP: &str = "player_jump";
const ACT_CAM_LEFT: &str = "camera_orbit_left";
const ACT_CAM_RIGHT: &str = "camera_orbit_right";
const ACT_CAM_UP: &str = "camera_orbit_up";
const ACT_CAM_DOWN: &str = "camera_orbit_down";
const ACT_WALK: &str = "player_walk";

/// X-ray occlusion probes per second (the fade itself runs per frame).
const XRAY_PROBE_HZ: f32 = 30.0;
/// Heights above the feet (metres) of the x-ray body rays: feet, knees, hips, chest, head.
const BODY_RAYS: [f32; 5] = [0.2, 0.55, 0.9, 1.25, 1.6];
/// The body origin sits at the capsule centre, 0.9 m above the feet (`player.tscn`).
const FEET_BELOW_ORIGIN: f32 = 0.9;

const ACTION_KEYS: [(&str, Key); 10] = [
    (ACT_FORWARD, Key::W),
    (ACT_BACK, Key::S),
    (ACT_LEFT, Key::A),
    (ACT_RIGHT, Key::D),
    (ACT_JUMP, Key::SPACE),
    (ACT_CAM_LEFT, Key::LEFT),
    (ACT_CAM_RIGHT, Key::RIGHT),
    (ACT_CAM_UP, Key::UP),
    (ACT_CAM_DOWN, Key::DOWN),
    (ACT_WALK, Key::SHIFT),
];

/// Demo third-person player: camera-relative WASD, jump, gravity; orbit / zoom input for its
/// Phantom Camera.
#[derive(GodotClass)]
#[class(base = CharacterBody3D)]
pub struct MstDemoPlayer {
    #[export_group(name = "Movement")]
    #[export]
    move_speed: f32,
    /// Speed while Shift is held.
    #[export]
    walk_speed: f32,
    #[export]
    acceleration: f32,
    #[export]
    jump_velocity: f32,
    #[export]
    gravity_scale: f32,
    #[export]
    turn_speed: f32,
    /// On the first landing the knight turns towards the camera, this many degrees off (a 3/4
    /// view of the face instead of the flat back of the cape). 180 = faces away.
    #[export]
    spawn_face_deg: f32,
    /// Drop the player onto the terrain on the first physics frames (after the terrain has a
    /// height and a collider under the start point) and whenever it falls below `fall_limit`.
    #[export]
    spawn_on_terrain: bool,
    #[export]
    fall_limit: f32,

    #[export_group(name = "Scene")]
    /// The `MarchingSquaresTerrain`. Empty: the first one in the owning scene.
    #[export]
    terrain: Option<Gd<Node3D>>,

    #[export_group(name = "Animation")]
    /// The node that turns to face the movement (the knight model).
    #[export]
    visual: Option<Gd<Node3D>>,
    /// Scene whose `AnimationPlayer` holds KayKit `Rig_Medium` clips (Idle, Walking_A,
    /// Running_A, Jump_Start, Jump_Idle, Jump_Land); shared onto the visual's skeleton.
    #[export]
    animation_source: Option<Gd<PackedScene>>,
    /// Ground speed (m/s) at which the walk clip's feet do not slide.
    #[export]
    walk_clip_speed: f32,
    /// Ground speed (m/s) at which the run clip's feet do not slide.
    #[export]
    run_clip_speed: f32,
    /// Cross-fade between animation states, seconds.
    #[export]
    animation_blend_time: f32,
    /// Procedurally pose the visual's `Skeleton3D` when no `animation_source` is usable.
    #[export]
    procedural_animation: bool,
    /// Ground distance covered per full walk cycle (two steps), metres.
    #[export]
    stride_length: f32,

    #[export_group(name = "Camera")]
    /// The `PhantomCamera3D` (follow mode THIRD_PERSON) that films the player.
    #[export]
    phantom_camera: Option<Gd<Node3D>>,
    /// The real `Camera3D` (with the `PhantomCameraHost` child).
    #[export]
    camera: Option<Gd<Camera3D>>,
    /// Closest / farthest spring length the wheel zooms to (metres).
    #[export]
    min_distance: f32,
    #[export]
    max_distance: f32,
    /// Lowest / highest camera pitch (degrees below the horizon).
    #[export]
    pitch_min_deg: f32,
    #[export]
    pitch_max_deg: f32,
    /// Arrow-key orbit speed, degrees per second.
    #[export]
    keyboard_orbit_speed: f32,
    /// Mouse-drag orbit, degrees per pixel.
    #[export]
    mouse_orbit_sensitivity: f32,
    /// Spring-length factor per wheel step.
    #[export]
    zoom_factor: f32,

    #[export_group(name = "X-Ray")]
    /// Show the x-ray silhouette of the hidden parts of the player.
    #[export]
    xray: bool,
    /// Blocked body rays (of 5) toward the camera that fade the silhouette in; it fades out
    /// again below `xray_trigger_points - 1`.
    #[export(range = (1.0, 5.0, 1.0))]
    xray_trigger_points: i32,
    /// A hit lower than this above the feet (metres) does not count (steps, kerbs, small props).
    #[export]
    xray_min_height: f32,
    /// Physics layers that count as occluders (default: 1 terrain, 3 scatter-prop colliders).
    #[export(flags_3d_physics)]
    xray_mask: u32,
    /// Pass 1 of the silhouette (stencil mask of the visible pixels).
    #[export]
    xray_mask_shader: Option<Gd<Shader>>,
    /// Pass 2 (the silhouette itself).
    #[export]
    xray_shader: Option<Gd<Shader>>,
    #[export]
    xray_color: Color,
    #[export]
    xray_rim_color: Color,

    dragging: bool,
    /// Action state, refreshed by `input` on every action event (never polled per frame):
    /// movement (x right, y back), camera orbit keys, walk and jump held.
    move_input: Vector2,
    orbit_input: Vector2,
    walk_held: bool,
    jump_held: bool,
    /// `physics/3d/default_gravity`, read once in `ready`.
    gravity: f32,
    spawned: bool,
    /// The first-landing facing (`spawn_face_deg`) was applied.
    faced: bool,
    rig: Option<Rig>,
    animator: Option<Animator>,
    clips: Option<ClipPlayer>,
    /// Seconds to the next x-ray probe; blocked body rays toward the camera at the last one.
    xray_probe_wait: f32,
    xray_points: u32,
    xray_on: bool,
    xray_amount: f32,
    xray_materials: Vec<Gd<ShaderMaterial>>,
    base: Base<CharacterBody3D>,
}

#[godot_api]
impl ICharacterBody3D for MstDemoPlayer {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            move_speed: 5.0,
            walk_speed: 1.8,
            acceleration: 12.0,
            jump_velocity: 4.5,
            gravity_scale: 1.0,
            turn_speed: 10.0,
            spawn_face_deg: 35.0,
            spawn_on_terrain: true,
            fall_limit: -40.0,
            terrain: None,
            visual: None,
            animation_source: None,
            walk_clip_speed: 1.8,
            run_clip_speed: 5.0,
            animation_blend_time: 0.15,
            procedural_animation: true,
            stride_length: 1.5,
            phantom_camera: None,
            camera: None,
            min_distance: 8.0,
            max_distance: 100.0,
            pitch_min_deg: -10.0,
            pitch_max_deg: 89.0,
            keyboard_orbit_speed: 120.0,
            mouse_orbit_sensitivity: 0.4,
            zoom_factor: 1.15,
            xray: true,
            xray_trigger_points: 2,
            xray_min_height: 0.6,
            xray_mask: 0b101,
            xray_mask_shader: None,
            xray_shader: None,
            xray_color: Color::from_rgb(0.27, 0.56, 1.0),
            xray_rim_color: Color::from_rgb(0.80, 0.93, 1.0),
            dragging: false,
            move_input: Vector2::ZERO,
            orbit_input: Vector2::ZERO,
            walk_held: false,
            jump_held: false,
            gravity: 9.8,
            spawned: false,
            faced: false,
            rig: None,
            animator: None,
            clips: None,
            xray_probe_wait: 0.0,
            xray_points: 0,
            xray_on: false,
            xray_amount: 0.0,
            xray_materials: Vec::new(),
            base,
        }
    }

    fn ready(&mut self) {
        ensure_input_actions();
        self.gravity = ProjectSettings::singleton().get_setting("physics/3d/default_gravity").try_to::<f32>().unwrap_or(9.8);
        if self.phantom_camera.as_ref().is_some_and(|p| !p.is_class("PhantomCamera3D")) {
            godot_warn!("MstDemoPlayer: phantom_camera is not a PhantomCamera3D (Phantom Camera library missing?)");
            self.phantom_camera = None;
        }
        if let Some(cam) = self.camera.as_mut() {
            cam.set_current(true);
        }
        if let (Some(visual), Some(source)) = (self.visual.as_ref(), self.animation_source.as_ref()) {
            let gait = Gait { walk_speed: self.walk_clip_speed, run_speed: self.run_clip_speed };
            self.animator = Animator::build(visual, source, gait, self.animation_blend_time);
        }
        if self.animator.is_none() {
            // A model that ships its own Idle / Run / Fall clips (the courtyard fox).
            self.clips = self.visual.as_ref().and_then(ClipPlayer::find);
        }
        if self.animator.is_none() && self.clips.is_none() && self.procedural_animation {
            self.rig = self.visual.as_ref().and_then(Rig::find);
        }
        self.setup_xray();
        self.sync_snap_distance();
    }

    /// Keeps the action state current. `input` (not `unhandled_input`) so a release is never
    /// missed when a GUI control consumes it, which would leave the knight walking.
    fn input(&mut self, event: Gd<InputEvent>) {
        if !event.is_action_type() {
            return;
        }
        let input = Input::singleton();
        self.move_input = input.get_vector(ACT_LEFT, ACT_RIGHT, ACT_FORWARD, ACT_BACK);
        self.orbit_input = input.get_vector(ACT_CAM_LEFT, ACT_CAM_RIGHT, ACT_CAM_UP, ACT_CAM_DOWN);
        self.walk_held = input.is_action_pressed(ACT_WALK);
        self.jump_held = input.is_action_pressed(ACT_JUMP);
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            let button = mb.get_button_index();
            if button == MouseButton::WHEEL_UP && mb.is_pressed() {
                self.zoom_steps(-wheel_factor(&mb));
                self.mark_handled();
            } else if button == MouseButton::WHEEL_DOWN && mb.is_pressed() {
                self.zoom_steps(wheel_factor(&mb));
                self.mark_handled();
            } else if [MouseButton::LEFT, MouseButton::MIDDLE, MouseButton::RIGHT].contains(&button) {
                self.dragging = mb.is_pressed();
            }
        } else if let Ok(mm) = event.try_cast::<InputEventMouseMotion>()
            && self.dragging
        {
            self.orbit_pixels(mm.get_relative());
        }
    }

    fn physics_process(&mut self, delta: f64) {
        let dt = delta as f32;
        if self.spawn_on_terrain && (!self.spawned || self.base().get_global_position().y < self.fall_limit) {
            self.spawned = self.spawn();
            if !self.spawned {
                // Terrain not ready: hold still (no gravity) instead of falling through it.
                self.base_mut().set_velocity(Vector3::ZERO);
                return;
            }
            if !self.faced {
                self.faced = true;
                // The model faces +Z; turn it toward the camera, `spawn_face_deg` off.
                let face = (self.view_yaw() + self.spawn_face_deg).to_radians();
                if let Some(visual) = self.visual.as_mut() {
                    let mut rot = visual.get_rotation();
                    rot.y = face;
                    visual.set_rotation(rot);
                }
            }
            // Jump the camera to the landed body instead of gliding there with the damping.
            if let Some(pcam) = self.phantom_camera.as_mut() {
                pcam.call_deferred("teleport_position", &[]);
            }
        }

        let raw = self.move_input;
        // Camera-relative: (right, forward) rotated by the view yaw; yaw 0 looks along -Z.
        let planar = Vector2::new(raw.x, -raw.y).rotated(self.view_yaw().to_radians());
        let mut wish = Vector3::new(planar.x, 0.0, -planar.y);
        if wish.length_squared() > 1.0 {
            wish = wish.normalized();
        }

        let gravity = self.gravity;
        let on_floor = self.base().is_on_floor();
        let mut velocity = self.base().get_velocity();
        let mut jumped = false;
        if !on_floor {
            velocity.y -= gravity * self.gravity_scale * dt;
        } else if self.jump_held {
            velocity.y = self.jump_velocity;
            jumped = true;
        }
        let walking = self.walk_held;
        let target = wish * if walking { self.walk_speed } else { self.move_speed };
        let k = (self.acceleration * dt).clamp(0.0, 1.0);
        velocity.x += (target.x - velocity.x) * k;
        velocity.z += (target.z - velocity.z) * k;
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        // Only the visual turns (the camera never inherits the body rotation). The model faces +Z.
        if wish.length_squared() > 0.001
            && let Some(visual) = self.visual.as_mut()
        {
            let face = wish.x.atan2(wish.z);
            let mut rot = visual.get_rotation();
            rot.y = lerp_angle(rot.y, face, (self.turn_speed * dt).clamp(0.0, 1.0));
            visual.set_rotation(rot);
        }

        let (speed, grounded, stride, max_speed) = {
            let body = self.base();
            let v = body.get_real_velocity();
            (Vector2::new(v.x, v.z).length(), body.is_on_floor(), self.stride_length, self.move_speed)
        };
        if let Some(animator) = self.animator.as_mut() {
            animator.update(dt, speed, grounded, jumped);
        } else if let Some(clips) = self.clips.as_mut() {
            clips.update(speed, grounded, jumped);
        } else if let Some(rig) = self.rig.as_mut() {
            rig.animate(dt, speed, max_speed, grounded, stride);
        }
    }

    /// Arrow-key orbit and the x-ray fade run per rendered frame.
    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        let orbit = self.orbit_input;
        if orbit.length_squared() > 0.0 {
            let step = self.keyboard_orbit_speed * dt;
            self.orbit_degrees(-orbit.x * step, -orbit.y * step);
        }
        self.update_xray(dt);
    }
}

#[godot_api]
impl MstDemoPlayer {
    /// True once the player stands on the terrain.
    #[func]
    pub fn is_spawned(&self) -> bool {
        self.spawned
    }

    /// Current x-ray silhouette strength (0 = hidden, 1 = full).
    #[func]
    pub fn get_xray_amount(&self) -> f32 {
        self.xray_amount
    }

    /// Body rays (0-5) toward the camera that an occluder blocked at the last x-ray probe.
    #[func]
    pub fn get_xray_points(&self) -> i32 {
        self.xray_points as i32
    }

    /// Orbits the pcam by a mouse delta in pixels.
    #[func]
    pub fn orbit_pixels(&mut self, relative: Vector2) {
        let s = self.mouse_orbit_sensitivity;
        self.orbit_degrees(-relative.x * s, relative.y * s);
    }

    /// Zooms by wheel steps (positive = farther): `spring_length *= zoom_factor ^ steps`, clamped
    /// to `min_distance..max_distance`. Only the spring length changes: the pcam's rotation is
    /// left as the orbit input set it (re-applying the damped camera's view here would undo an
    /// orbit made in the same moment).
    #[func]
    pub fn zoom_steps(&mut self, steps: f32) {
        let Some(d) = self.spring_length() else { return };
        let lo = self.min_distance.min(self.max_distance);
        let hi = self.max_distance.max(self.min_distance);
        let distance = (d * self.zoom_factor.max(1.001).powf(steps)).clamp(lo, hi);
        if let Some(pcam) = self.phantom_camera.as_mut() {
            pcam.call("set_spring_length", vslice![distance as f64]);
        }
        self.sync_snap_distance();
    }

    /// Places the camera: yaw (0 = behind the player looking along -Z), pitch (degrees below
    /// the horizon, clamped to `pitch_min_deg..pitch_max_deg`) and spring length (clamped to
    /// `min_distance..max_distance`).
    #[func]
    pub fn set_view(&mut self, yaw_deg: f32, pitch_deg: f32, distance: f32) {
        let pitch = pitch_deg.clamp(self.pitch_min_deg.min(self.pitch_max_deg), self.pitch_max_deg.max(self.pitch_min_deg));
        let distance = distance.clamp(self.min_distance.min(self.max_distance), self.max_distance.max(self.min_distance));
        if let Some(pcam) = self.phantom_camera.as_mut() {
            pcam.call("set_third_person_rotation_degrees", vslice![Vector3::new(-pitch, wrap_deg(yaw_deg), 0.0)]);
            pcam.call("set_spring_length", vslice![distance as f64]);
        }
        self.sync_snap_distance();
    }

    /// Camera yaw in degrees (0 = looking along -Z), from the camera's view direction.
    #[func]
    pub fn view_yaw(&self) -> f32 {
        let Some(cam) = self.view_camera() else { return 0.0 };
        let back = cam.get_global_transform().basis.col_c();
        if Vector2::new(back.x, back.z).length_squared() < 1e-8 {
            // Straight down: the camera's up vector points away from the view's back.
            let up = cam.get_global_transform().basis.col_b();
            return wrap_deg((-up.x).atan2(-up.z).to_degrees());
        }
        wrap_deg(back.x.atan2(back.z).to_degrees())
    }

    /// Camera pitch in degrees below the horizon.
    #[func]
    pub fn view_pitch(&self) -> f32 {
        let Some(cam) = self.view_camera() else { return 0.0 };
        let back = cam.get_global_transform().basis.col_c().normalized();
        back.y.clamp(-1.0, 1.0).asin().to_degrees()
    }
}

impl MstDemoPlayer {
    /// Turns the pcam's spring arm by `yaw` / `pitch` degrees (pitch positive = looking further
    /// down), like Phantom Camera's third-person example.
    fn orbit_degrees(&mut self, yaw: f32, pitch: f32) {
        let Some(pcam) = self.phantom_camera.as_mut() else { return };
        let rot = pcam.call("get_third_person_rotation_degrees", &[]).try_to::<Vector3>().unwrap_or(Vector3::ZERO);
        let lo = self.pitch_min_deg.min(self.pitch_max_deg);
        let hi = self.pitch_max_deg.max(self.pitch_min_deg);
        let down = (-rot.x + pitch).clamp(lo, hi);
        pcam.call("set_third_person_rotation_degrees", vslice![Vector3::new(-down, wrap_deg(rot.y + yaw), rot.z)]);
    }

    fn spring_length(&self) -> Option<f32> {
        self.phantom_camera.as_ref()?.clone().call("get_spring_length", &[]).try_to::<f64>().ok().map(|d| d as f32)
    }

    /// `PixelRenderer` snaps perspective cameras on the plane at `pixel_snap_distance` (the
    /// player, at the end of the spring arm).
    fn sync_snap_distance(&mut self) {
        let Some(d) = self.spring_length() else { return };
        if let Some(cam) = self.camera.as_mut()
            && cam.get_meta_ex("pixel_snap_distance").default(&(-1.0f32).to_variant()).done().try_to::<f32>().ok() != Some(d)
        {
            cam.set_meta("pixel_snap_distance", &d.to_variant());
        }
    }

    /// The camera the player walks relative to: `camera`, else the viewport's.
    fn view_camera(&self) -> Option<Gd<Camera3D>> {
        self.camera
            .clone()
            .filter(|c| c.is_inside_tree())
            .or_else(|| self.base().get_viewport().and_then(|vp| vp.get_camera_3d()))
    }

    /// Blocked body rays toward where the camera really is (it follows with damping).
    fn blocked_for_camera(&self) -> Option<u32> {
        let cam = self.camera.as_ref().filter(|c| c.is_inside_tree())?;
        let body = self.base();
        if !body.is_inside_tree() {
            return None;
        }
        let mut space = body.get_world_3d()?.get_direct_space_state()?;
        let cam_pos = cam.get_global_position();
        let feet = body.get_global_position() - Vector3::UP * FEET_BELOW_ORIGIN;
        let mut exclude = Array::new();
        exclude.push(body.get_rid());
        let mut terrain = self
            .terrain
            .clone()
            .filter(|t| t.is_instance_valid() && t.has_method("raycast"));
        let hit_pos = |d: &VarDictionary| d.get("position").and_then(|v| v.try_to::<Vector3>().ok());
        let mut n = 0;
        for h in BODY_RAYS {
            let from = feet + Vector3::UP * h;
            let to_cam = cam_pos - from;
            let len = to_cam.length() - 0.5;
            if len < 1.0 {
                continue;
            }
            let dir = to_cam.normalized();
            let mut query = PhysicsRayQueryParameters3D::create(from, from + dir * len)?;
            query.set_collision_mask(self.xray_mask);
            query.set_exclude(&exclude);
            // Physics (prop colliders, terrain trimesh) and the terrain's height march (the
            // visual surface; physics rays miss the trimesh walls on hex_island): either hit
            // high enough counts (a low physics hit, e.g. a pebble, must not hide a cliff).
            let phys = hit_pos(&space.intersect_ray(&query));
            let terr = terrain
                .as_mut()
                .and_then(|t| t.call("raycast", vslice![from, dir, len]).try_to::<VarDictionary>().ok())
                .and_then(|d| hit_pos(&d));
            if [phys, terr].into_iter().flatten().any(|pos| pos.y - feet.y >= self.xray_min_height) {
                n += 1;
            }
        }
        Some(n)
    }

    /// Fades the silhouette in when `xray_trigger_points` body rays toward the camera are
    /// blocked, out below `xray_trigger_points - 1` (never without a real occluder).
    fn update_xray(&mut self, dt: f32) {
        if self.xray_materials.len() < 2 {
            return;
        }
        self.xray_probe_wait -= dt;
        if self.xray_probe_wait <= 0.0 {
            self.xray_probe_wait = 1.0 / XRAY_PROBE_HZ;
            if self.terrain.is_none() {
                self.find_terrain();
            }
            self.xray_points = self.blocked_for_camera().unwrap_or(0);
        }
        let n = self.xray_points;
        let enter = self.xray_trigger_points.clamp(1, BODY_RAYS.len() as i32) as u32;
        if n + 1 < enter {
            self.xray_on = false;
        } else if n >= enter {
            self.xray_on = true;
        }
        let target = if self.xray_on { 1.0 } else { 0.0 };
        let k = 1.0 - (-12.0 * dt).exp();
        let mut a = self.xray_amount + (target - self.xray_amount) * k;
        if (a - target).abs() < 0.02 {
            a = target;
        }
        if a != self.xray_amount {
            self.xray_amount = a;
            self.xray_materials[1].set_shader_parameter("amount", &a.to_variant());
        }
    }

    /// Gives every mesh under `visual` the two-pass x-ray `material_overlay`.
    fn setup_xray(&mut self) {
        self.xray_materials.clear();
        let (Some(visual), Some(mask), Some(sil)) =
            (self.visual.clone(), self.xray_mask_shader.clone(), self.xray_shader.clone())
        else {
            return;
        };
        if !self.xray {
            return;
        }
        let mut pass2 = ShaderMaterial::new_gd();
        pass2.set_shader(&sil);
        pass2.set_render_priority(Material::RENDER_PRIORITY_MAX);
        pass2.set_shader_parameter("silhouette_color", &self.xray_color.to_variant());
        pass2.set_shader_parameter("rim_color", &self.xray_rim_color.to_variant());
        // Hidden until `update_xray` finds a real occluder.
        self.xray_amount = 0.0;
        self.xray_on = false;
        pass2.set_shader_parameter("amount", &0.0f32.to_variant());
        let mut pass1 = ShaderMaterial::new_gd();
        pass1.set_shader(&mask);
        pass1.set_render_priority(Material::RENDER_PRIORITY_MAX - 1);
        pass1.set_next_pass(&pass2);
        for n in visual.find_children_ex("*").type_("MeshInstance3D").recursive(true).owned(false).done().iter_shared() {
            if let Ok(mut mi) = n.try_cast::<MeshInstance3D>() {
                mi.set_material_overlay(&pass1);
            }
        }
        self.xray_materials = vec![pass1, pass2];
    }

    fn mark_handled(&self) {
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
    }

    /// The scene the player belongs to: the top of the owner chain (the scene file that instanced
    /// the player rig), else the topmost ancestor below the tree root (player added from code).
    fn scene_root(&self) -> Gd<Node> {
        let me = self.to_gd().upcast::<Node>();
        let mut top = me.clone();
        while let Some(owner) = top.get_owner() {
            top = owner;
        }
        if top != me {
            return top;
        }
        let root = Some(self.base().get_tree().get_root().upcast::<Node>());
        while let Some(parent) = top.get_parent() {
            if Some(&parent) == root.as_ref() {
                break;
            }
            top = parent;
        }
        top
    }

    fn find_terrain(&mut self) -> Option<Gd<Node3D>> {
        if self.terrain.as_ref().is_some_and(|t| t.is_instance_valid()) {
            return self.terrain.clone();
        }
        let scene = self.scene_root();
        let found = if scene.is_class("MarchingSquaresTerrain") {
            Some(scene)
        } else {
            scene
                .find_children_ex("*")
                .type_("MarchingSquaresTerrain")
                .recursive(true)
                .owned(false)
                .done()
                .iter_shared()
                .next()
        };
        self.terrain = found.and_then(|n| n.try_cast::<Node3D>().ok());
        self.terrain.clone()
    }

    /// Stands the body on the terrain surface under it; false while the terrain is not ready
    /// there yet.
    fn spawn(&mut self) -> bool {
        if !self.base().is_inside_tree() {
            return false;
        }
        let Some(mut terrain) = self.find_terrain() else {
            return true; // no terrain: just let gravity act
        };
        if !terrain.has_method("get_height_at") {
            return true;
        }
        let p = self.base().get_global_position();
        let h = terrain
            .call("get_height_at", vslice![Vector3::new(p.x, 0.0, p.z), f32::NAN])
            .try_to::<f32>()
            .unwrap_or(f32::NAN);
        if !h.is_finite() {
            return false; // samples not loaded yet
        }
        // Heights come from the samples; wait for the chunk collider as well.
        if !self.floor_collider_at(Vector3::new(p.x, h, p.z)) {
            return false;
        }
        let mut body = self.base_mut();
        body.set_global_position(Vector3::new(p.x, h + 1.2, p.z));
        body.set_velocity(Vector3::ZERO);
        true
    }

    fn floor_collider_at(&self, surface: Vector3) -> bool {
        let body = self.base();
        let Some(mut space) = body.get_world_3d().and_then(|w| w.get_direct_space_state()) else {
            return false;
        };
        let Some(mut query) =
            PhysicsRayQueryParameters3D::create(surface + Vector3::UP * 2.0, surface - Vector3::UP * 2.0)
        else {
            return false;
        };
        query.set_collision_mask(body.get_collision_mask());
        let mut exclude = Array::new();
        exclude.push(body.get_rid());
        query.set_exclude(&exclude);
        !space.intersect_ray(&query).is_empty()
    }
}

/// One posed bone: its index, rest rotation/position and global rest rotation.
struct Bone {
    idx: i32,
    rest: Quaternion,
    rest_pos: Vector3,
    global_rest: Quaternion,
}

/// Procedural animation for the KayKit `Rig_Medium` skeleton (T-pose rest, model faces +Z).
struct Rig {
    skeleton: Gd<Skeleton3D>,
    hips: Bone,
    spine: Bone,
    chest: Bone,
    head: Bone,
    arm: [Bone; 2],
    forearm: [Bone; 2],
    thigh: [Bone; 2],
    shin: [Bone; 2],
    foot: [Bone; 2],
    phase: f32,
    time: f32,
    move_w: f32,
    run_w: f32,
    air_w: f32,
}

impl Rig {
    fn find(visual: &Gd<Node3D>) -> Option<Self> {
        let skeleton = visual
            .find_children_ex("*")
            .type_("Skeleton3D")
            .recursive(true)
            .owned(false)
            .done()
            .iter_shared()
            .next()?
            .try_cast::<Skeleton3D>()
            .ok()?;
        let bone = |name: &str| -> Option<Bone> {
            let idx = skeleton.find_bone(name);
            if idx < 0 {
                return None;
            }
            let rest = skeleton.get_bone_rest(idx);
            Some(Bone {
                idx,
                rest: rest.basis.orthonormalized().get_quaternion(),
                rest_pos: rest.origin,
                global_rest: skeleton.get_bone_global_rest(idx).basis.orthonormalized().get_quaternion(),
            })
        };
        let pair = |a: &str, b: &str| -> Option<[Bone; 2]> { Some([bone(a)?, bone(b)?]) };
        let rig = Self {
            hips: bone("hips")?,
            spine: bone("spine")?,
            chest: bone("chest")?,
            head: bone("head")?,
            arm: pair("upperarm.l", "upperarm.r")?,
            forearm: pair("lowerarm.l", "lowerarm.r")?,
            thigh: pair("upperleg.l", "upperleg.r")?,
            shin: pair("lowerleg.l", "lowerleg.r")?,
            foot: pair("foot.l", "foot.r")?,
            skeleton: skeleton.clone(),
            phase: 0.0,
            time: 0.0,
            move_w: 0.0,
            run_w: 0.0,
            air_w: 0.0,
        };
        Some(rig)
    }

    /// Rotates `bone` by the model-space rotation `q` (applied on top of its rest pose).
    fn pose(&mut self, which: impl Fn(&Self) -> &Bone, q: Quaternion) {
        let b = which(self);
        let local = b.rest * (b.global_rest.inverse() * q * b.global_rest);
        let idx = b.idx;
        self.skeleton.set_bone_pose_rotation(idx, local.normalized());
    }

    fn animate(&mut self, dt: f32, speed: f32, max_speed: f32, grounded: bool, stride: f32) {
        let ease = |w: &mut f32, target: f32, rate: f32| *w += (target - *w) * (rate * dt).clamp(0.0, 1.0);
        let walk_target = if grounded { (speed / 1.5).clamp(0.0, 1.0) } else { 0.0 };
        let run_target = if grounded { ((speed - 2.0) / (max_speed - 2.0).max(0.1)).clamp(0.0, 1.0) } else { 0.0 };
        ease(&mut self.move_w, walk_target, 10.0);
        ease(&mut self.run_w, run_target, 6.0);
        ease(&mut self.air_w, if grounded { 0.0 } else { 1.0 }, 12.0);
        self.time += dt;
        if grounded {
            self.phase = (self.phase + speed * dt * TAU / stride.max(0.1)).rem_euclid(TAU);
        }

        let (m, r, a) = (self.move_w, self.run_w, self.air_w);
        let s = self.phase.sin();
        let breathe = (self.time * 2.2).sin();
        let x = |deg: f32| Quaternion::from_axis_angle(Vector3::RIGHT, deg.to_radians());
        let y = |deg: f32| Quaternion::from_axis_angle(Vector3::UP, deg.to_radians());
        let z = |deg: f32| Quaternion::from_axis_angle(Vector3::BACK, deg.to_radians());

        // Body: bob twice per cycle, lean into the run, breathe when idle.
        let bob = m * (0.025 + 0.025 * r) * (2.0 * self.phase).cos().abs();
        let hips_pos = self.hips.rest_pos + Vector3::new(0.0, bob - m * 0.02 + 0.004 * breathe * (1.0 - m), 0.0);
        let hips_idx = self.hips.idx;
        self.skeleton.set_bone_pose_position(hips_idx, hips_pos);
        self.pose(|r| &r.hips, y(8.0 * m * s));
        self.pose(|r| &r.spine, x(m * (4.0 + 8.0 * r) + 6.0 * a));
        self.pose(|r| &r.chest, y(-10.0 * m * s) * x(1.5 * breathe * (1.0 - m)));
        self.pose(|r| &r.head, x(-m * (3.0 + 6.0 * r) - 4.0 * a));

        let leg_amp = 28.0 + 14.0 * r;
        let arm_amp = 20.0 + 25.0 * r;
        for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
            // Left (+X) leg leads with the phase, the right one is half a cycle behind.
            let ph = self.phase + if i == 0 { 0.0 } else { PI };
            let swing = ph.sin(); // +1: leg forward
            let lift = (ph + 0.5 * PI).sin().max(0.0); // knee bends while the leg swings forward
            let jump_leg = if i == 0 { -35.0 } else { 10.0 };
            let thigh = x(-leg_amp * m * swing + jump_leg * a);
            let knee = x((8.0 + 45.0 * r) * m * lift + 12.0 * m + (if i == 0 { 60.0 } else { 25.0 }) * a);
            let foot = x(-10.0 * m * lift - 15.0 * a);
            // Arms hang from the T-pose (rotate down about Z), swing opposite to the legs.
            let lower = 72.0 - 8.0 * m - 45.0 * a + 2.0 * breathe * (1.0 - m);
            let arm = x(arm_amp * m * swing - 20.0 * a) * z(-side * lower);
            let elbow = y(-side * (10.0 + 30.0 * r * m + 25.0 * a));
            self.pose(|r| &r.thigh[i], thigh);
            self.pose(|r| &r.shin[i], knee);
            self.pose(|r| &r.foot[i], foot);
            self.pose(|r| &r.arm[i], arm);
            self.pose(|r| &r.forearm[i], elbow);
        }
    }
}

/// Registers the demo's input actions once and binds their physical keys once (idempotent, so
/// the player works in any project without InputMap setup).
fn ensure_input_actions() {
    let mut map = InputMap::singleton();
    for (action, key) in ACTION_KEYS {
        let name = StringName::from(action);
        if !map.has_action(&name) {
            map.add_action(&name);
        }
        let bound = map.action_get_events(&name).iter_shared().any(|e| {
            e.try_cast::<InputEventKey>().is_ok_and(|k| k.get_physical_keycode() == key)
        });
        if !bound {
            let mut ev = InputEventKey::new_gd();
            ev.set_physical_keycode(key);
            map.action_add_event(&name, &ev);
        }
    }
}

fn wrap_deg(v: f32) -> f32 {
    v.rem_euclid(360.0)
}

/// Wheel steps carried by a wheel event (trackpads report fractions; synthetic events 0).
fn wheel_factor(mb: &Gd<InputEventMouseButton>) -> f32 {
    let f = mb.get_factor();
    if f > 0.0 { f } else { 1.0 }
}

fn lerp_angle(from: f32, to: f32, w: f32) -> f32 {
    let diff = (to - from) % TAU;
    let dist = (2.0 * diff) % TAU - diff;
    from + dist * w
}
