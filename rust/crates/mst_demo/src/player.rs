//! `MstDemoPlayer` — the demo's third-person knight (port of godot-pixelizer-3d `demo/player.gd`),
//! with its camera driven through a Phantom Camera (`PhantomCamera3D`, follow mode THIRD_PERSON).
//!
//! * WASD moves relative to the camera yaw (Shift walks), Space jumps, gravity from the project
//!   settings; only the `visual` child turns (smoothly) to face the movement.
//! * Arrow keys orbit, any mouse button held + drag orbits (as in the original: the mouse is not
//!   captured, so the F3 diagnostics tuner stays usable), the wheel zooms (multiplicative, eased).
//!   Zoom is linked to pitch: zooming in flattens the view, zooming out looks further down, while
//!   keeping any pitch the user dragged in on top. Orbit and zoom ease toward their targets every
//!   rendered frame. C toggles between this camera and the scene's overview camera.
//!   Pixel stability while orbiting (port of Stable3D's `StableOrthographicCameraController`):
//!   the eases land exactly on their targets within `orbit_settle_deg` (no long tail of
//!   sub-texel rotations, each of which re-anchors `PixelRenderer`'s snap lattice and crawls the
//!   picture), and `orbit_yaw_step_deg` optionally shows the yaw in discrete steps (grid snap).
//! * Projection: by default a low-FOV perspective camera (`camera_fov` 15°, `distance` 38 m:
//!   nearly orthographic, framing ~10 m vertically). The camera carries a `pixel_snap_distance`
//!   meta (its arm length), so `PixelRenderer` snaps it to the texel lattice of the focus plane
//!   and pixels stay locked to the world around the player while walking. `orthographic` keeps
//!   the old option: the view size follows `distance` (same framing as the FOV at that
//!   distance) and the camera sits `ortho_camera_distance` back along its view axis.
//!   The pcam's spring-arm collision is off by default (`camera_collision`): pulling a far,
//!   narrow camera in front of a cliff zooms the view in to a few metres (and in ortho only
//!   near-clips the cliff away); the pitch raise and the x-ray handle occluders instead.
//! * Occlusion: every frame five rays (feet to head) go from the player back toward the camera.
//!   Each ray is tested against the terrain's own height march (`raycast`: the visual surface)
//!   and against physics bodies on `occlusion_mask` (layer 1 terrain + layer 3 scatter props:
//!   trees and big rocks carry `PixelScatterItem.collision`). A ray only counts when either hit
//!   lies at least `occlusion_min_height` above the feet, so a one-level step or a kerb behind the heels does
//!   not count. When `occlusion_trigger_points` or more rays are blocked at the user's pitch,
//!   the shown pitch rises (eased, fast) to the lowest pitch that clears every ray plus a
//!   margin, capped at `occlusion_pitch_max_deg` (65°, so the view never turns top-down; the
//!   x-ray shows whatever the cap leaves hidden). Once the user's pitch has stayed below the
//!   trigger for `occlusion_release_delay`
//!   seconds (hysteresis), the added pitch eases back to zero (`occlusion_return_speed`,
//!   ~1 s).
//! * X-ray: as a fallback for what stays hidden (mid-transition, overhangs), every mesh under
//!   `visual` gets a two-pass `material_overlay` (`xray_mask_shader`, `xray_shader`): pass 1
//!   stencil-marks the knight's visible pixels, pass 2 draws the rest of its shape without a
//!   depth test as a flat, dithered palette silhouette. Both discard in the metadata / id passes,
//!   so the pixel resolve does not outline the silhouette. The silhouette's `amount` is gated by
//!   the same rays toward the camera (where it really is, and the shown pitch it is heading to):
//!   it fades in when `xray_trigger_points` (2) rays hit a real occluder and out when fewer than
//!   `xray_trigger_points - 1` do, so grass, flowers, small props
//!   and ground steps around the feet never show it.
//! * Animation: `Knight.fbx` ships the KayKit `Rig_Medium` rig without clips. With
//!   `animation_source` set (the CC0 KayKit clips GLB), [`crate::anim::Animator`] shares those
//!   clips onto the knight's skeleton and blends idle / walk / run by ground speed plus jump
//!   start / air / land through an `AnimationTree`. Without it the skeleton is posed procedurally
//!   (idle breathing, a walk/run cycle, a tucked jump pose).
//!
//! Scene lookup never uses `SceneTree::current_scene`: terrain, `PlayerSpawn` and the overview
//! camera come from the exported node slots, else from the scene that owns the player (top of
//! the owner chain, or the parent chain when instanced from code). Placement waits until the
//! terrain reports a height at the spawn point and a physics ray finds its collider there.
//!
//! The controller owns yaw / pitch / distance and pushes them to the pcam once per frame
//! (`set_third_person_rotation_degrees`, `set_spring_length`, `set_follow_distance`) via dynamic
//! calls: Phantom Camera is a separate, optional library. Without a `PhantomCamera3D` the
//! controller places `camera` itself with the same orbit math as the original demo.

use std::f32::consts::{PI, TAU};

use godot::classes::camera_3d::ProjectionType;
use godot::classes::{
    Camera3D, CharacterBody3D, ICharacterBody3D, Input, InputEvent, InputEventKey, InputEventMouseButton,
    InputEventMouseMotion, InputMap, Material, MeshInstance3D, PackedScene, PhysicsRayQueryParameters3D,
    ProjectSettings, Shader, ShaderMaterial, Skeleton3D,
};

use crate::anim::{Animator, Gait};
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
const ACT_CAM_TOGGLE: &str = "camera_toggle";
const ACT_WALK: &str = "player_walk";

/// Heights above the feet (metres) of the occlusion rays: feet, knees, hips, chest, head.
/// Occlusion probes per second (camera avoidance and x-ray); the camera easing runs per frame.
const OCCLUSION_PROBE_HZ: f32 = 30.0;
const BODY_RAYS: [f32; 5] = [0.2, 0.55, 0.9, 1.25, 1.6];
/// The body origin sits at the capsule centre, 0.9 m above the feet (`player.tscn`).
const FEET_BELOW_ORIGIN: f32 = 0.9;

const ACTION_KEYS: [(&str, Key); 11] = [
    (ACT_FORWARD, Key::W),
    (ACT_BACK, Key::S),
    (ACT_LEFT, Key::A),
    (ACT_RIGHT, Key::D),
    (ACT_JUMP, Key::SPACE),
    (ACT_CAM_LEFT, Key::LEFT),
    (ACT_CAM_RIGHT, Key::RIGHT),
    (ACT_CAM_UP, Key::UP),
    (ACT_CAM_DOWN, Key::DOWN),
    (ACT_CAM_TOGGLE, Key::C),
    (ACT_WALK, Key::SHIFT),
];

/// Demo third-person player: camera-relative WASD, jump, gravity, orbit / zoom camera.
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
    /// On the first spawn the knight turns towards the camera, this many degrees off (a 3/4
    /// view of the face instead of the flat back of the cape). 180 = faces away.
    #[export]
    spawn_face_deg: f32,
    /// Drop the player onto the terrain on the first physics frames (after the terrain has a
    /// height and a collider at the spawn point) and whenever it falls below `fall_limit`.
    #[export]
    spawn_on_terrain: bool,
    #[export]
    fall_limit: f32,

    #[export_group(name = "Scene")]
    /// The `MarchingSquaresTerrain`. Empty: the first one in the owning scene.
    #[export]
    terrain: Option<Gd<Node3D>>,
    /// Spawn point. Empty: a node named `PlayerSpawn` in the owning scene, else the current position.
    #[export]
    spawn_point: Option<Gd<Node3D>>,
    /// Camera that C toggles to. Empty: a `Camera3D` named `Camera` at the owning scene's root.
    #[export]
    overview_camera: Option<Gd<Camera3D>>,

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
    /// A `PhantomCamera3D` (follow mode THIRD_PERSON) following a child of this body. Optional.
    #[export]
    phantom_camera: Option<Gd<Node3D>>,
    /// The real `Camera3D` (with a `PhantomCameraHost` child when a pcam is used).
    #[export]
    camera: Option<Gd<Camera3D>>,
    /// Orthographic instead of the (default) low-FOV perspective camera. Both are snapped to
    /// the pixel renderer's texel lattice.
    #[export]
    #[var(set = set_orthographic)]
    orthographic: bool,
    /// Vertical field of view of `camera` (degrees). Low values look nearly orthographic; the
    /// orthographic view size uses the same framing.
    #[export(range = (1.0, 90.0, 0.5))]
    camera_fov: f32,
    /// Perspective only: let the pcam's spring arm pull the camera in front of occluders.
    #[export]
    camera_collision: bool,
    /// Orthographic only: how far back along its view axis the camera sits. The view size comes
    /// from `distance`; this only has to clear the terrain between camera and player.
    #[export]
    ortho_camera_distance: f32,
    /// Camera pivot above the body origin (fallback camera only; the pcam follows its target).
    #[export]
    pivot_height: f32,
    #[export]
    #[var(set = set_distance)]
    distance: f32,
    #[export]
    min_distance: f32,
    #[export]
    max_distance: f32,
    #[export]
    #[var(set = set_yaw_deg)]
    yaw_deg: f32,
    #[export]
    #[var(set = set_pitch_deg)]
    pitch_deg: f32,
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
    /// Distance factor per wheel step.
    #[export]
    zoom_factor: f32,
    /// Pitch the zoom pulls toward at `min_distance` (degrees down); flatter when close.
    #[export]
    zoom_pitch_near_deg: f32,
    /// Pitch the zoom pulls toward at `max_distance`.
    #[export]
    zoom_pitch_far_deg: f32,
    /// Link pitch to zoom (see `zoom_pitch_near_deg` / `zoom_pitch_far_deg`).
    #[export]
    zoom_pitch_link: bool,
    /// How fast the shown orbit / zoom catch up with their targets (1/s; 0 = instant).
    #[export]
    camera_smoothing: f32,
    /// The eased yaw / pitch land exactly on their targets within this many degrees (the zoom
    /// within the same fraction of a radian, relative), like Stable3D's orbit settle tolerance.
    #[export(range = (0.0, 5.0, 0.01))]
    orbit_settle_deg: f32,
    /// Show the yaw in steps of this many degrees (0 = continuous). Each step re-renders the
    /// view once instead of crawling every frame of an orbit.
    #[export(range = (0.0, 90.0, 0.1))]
    orbit_yaw_step_deg: f32,

    #[export_group(name = "Occlusion")]
    /// Raise the pitch when terrain hides the player.
    #[export]
    occlusion_avoid: bool,
    /// Highest pitch the avoidance may raise the camera to (degrees down).
    #[export]
    occlusion_pitch_max_deg: f32,
    /// Extra pitch above the lowest clear one, so the view does not graze the cliff edge.
    #[export]
    occlusion_margin_deg: f32,
    /// How fast the pitch rises when blocked / falls back when clear (1/s).
    #[export]
    occlusion_rise_speed: f32,
    #[export]
    occlusion_return_speed: f32,
    /// Seconds the user's pitch must stay clear before the added pitch starts to return.
    #[export]
    occlusion_release_delay: f32,
    /// Blocked body rays (of 5, feet to head) that raise the pitch.
    #[export(range = (1.0, 5.0, 1.0))]
    occlusion_trigger_points: i32,
    /// A hit lower than this above the feet (metres) does not count (steps, kerbs, small props).
    #[export]
    occlusion_min_height: f32,
    /// Physics layers that count as occluders (default: 1 terrain, 3 scatter-prop colliders).
    #[export(flags_3d_physics)]
    occlusion_mask: u32,
    /// Show the x-ray silhouette of the hidden parts of the player.
    #[export]
    xray: bool,
    /// Blocked body rays (of 5) toward the camera that fade the silhouette in; it fades out
    /// again below `xray_trigger_points - 1`.
    #[export(range = (1.0, 5.0, 1.0))]
    xray_trigger_points: i32,
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
    spawned: bool,
    /// The first-spawn facing (`spawn_face_deg`) was applied.
    faced: bool,
    spawn_marker_used: bool,
    player_view: bool,
    /// The pcam's authored spring-arm collision mask (used in perspective only).
    pcam_collision_mask: i64,
    rig: Option<Rig>,
    animator: Option<Animator>,
    /// Eased (shown) camera state; `yaw_deg` / `pitch_deg` / `distance` are the targets.
    cur_yaw: f32,
    cur_pitch: f32,
    cur_distance: f32,
    /// Pitch added by occlusion avoidance: target and shown (eased) value, degrees.
    occ_target: f32,
    cur_occ: f32,
    /// Avoidance is holding the pitch up; seconds the user's pitch has been clear since.
    occ_raised: bool,
    occ_clear_time: f32,
    /// Seconds to the next occlusion probe, and seconds gathered since the last one.
    occ_probe_wait: f32,
    occ_probe_dt: f32,
    /// Blocked body rays at the user's / shown pitch (last update).
    occ_points_base: u32,
    occ_points_shown: u32,
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
            faced: false,
            spawn_on_terrain: true,
            fall_limit: -40.0,
            terrain: None,
            spawn_point: None,
            overview_camera: None,
            visual: None,
            animation_source: None,
            walk_clip_speed: 1.8,
            run_clip_speed: 5.0,
            animation_blend_time: 0.15,
            procedural_animation: true,
            stride_length: 1.5,
            phantom_camera: None,
            camera: None,
            orthographic: false,
            camera_fov: 15.0,
            camera_collision: false,
            ortho_camera_distance: 80.0,
            pivot_height: 0.6,
            distance: 38.0,
            min_distance: 8.0,
            max_distance: 100.0,
            yaw_deg: 0.0,
            pitch_deg: 45.0,
            pitch_min_deg: -10.0,
            pitch_max_deg: 89.9,
            keyboard_orbit_speed: 120.0,
            mouse_orbit_sensitivity: 0.4,
            zoom_factor: 1.15,
            zoom_pitch_near_deg: 18.0,
            zoom_pitch_far_deg: 65.0,
            zoom_pitch_link: true,
            camera_smoothing: 14.0,
            orbit_settle_deg: 0.3,
            orbit_yaw_step_deg: 0.0,
            occlusion_avoid: true,
            occlusion_pitch_max_deg: 65.0,
            occlusion_margin_deg: 6.0,
            occlusion_rise_speed: 9.0,
            occlusion_return_speed: 5.0,
            occlusion_release_delay: 0.25,
            occlusion_trigger_points: 2,
            occlusion_min_height: 0.6,
            occlusion_mask: 0b101,
            xray: true,
            xray_trigger_points: 2,
            xray_mask_shader: None,
            xray_shader: None,
            xray_color: Color::from_rgb(0.27, 0.56, 1.0),
            xray_rim_color: Color::from_rgb(0.80, 0.93, 1.0),
            dragging: false,
            spawned: false,
            spawn_marker_used: false,
            player_view: true,
            pcam_collision_mask: 1,
            rig: None,
            animator: None,
            cur_yaw: 0.0,
            cur_pitch: 45.0,
            cur_distance: 38.0,
            occ_target: 0.0,
            cur_occ: 0.0,
            occ_raised: false,
            occ_clear_time: 0.0,
            occ_probe_wait: 0.0,
            occ_probe_dt: 0.0,
            occ_points_base: 0,
            occ_points_shown: 0,
            xray_on: false,
            xray_amount: 0.0,
            xray_materials: Vec::new(),
            base,
        }
    }

    fn ready(&mut self) {
        ensure_input_actions();
        self.pitch_deg = self.clamp_pitch(self.pitch_deg);
        self.distance = self.clamp_distance(self.distance);
        if self.phantom_camera.as_ref().is_some_and(|p| !p.is_class("PhantomCamera3D")) {
            godot_warn!("MstDemoPlayer: phantom_camera is not a PhantomCamera3D (Phantom Camera library missing?); driving the camera directly");
            self.phantom_camera = None;
        }
        if let Some(pcam) = self.phantom_camera.as_ref() {
            self.pcam_collision_mask = pcam.get("collision_mask").try_to::<i64>().unwrap_or(1);
        }
        if let Some(cam) = self.camera.as_mut() {
            cam.set_current(true);
        }
        if self.overview_camera.is_none() {
            self.overview_camera = self
                .scene_root()
                .get_node_or_null("Camera")
                .and_then(|n| n.try_cast::<Camera3D>().ok())
                .filter(|c| Some(c) != self.camera.as_ref());
        }
        if let (Some(visual), Some(source)) = (self.visual.as_ref(), self.animation_source.as_ref()) {
            let gait = Gait { walk_speed: self.walk_clip_speed, run_speed: self.run_clip_speed };
            self.animator = Animator::build(visual, source, gait, self.animation_blend_time);
        }
        if self.animator.is_none() && self.procedural_animation {
            self.rig = self.visual.as_ref().and_then(Rig::find);
        }
        self.setup_xray();
        self.snap_camera();
        self.update_camera();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.is_action_pressed(ACT_CAM_TOGGLE) {
            self.set_player_view(!self.player_view);
            self.mark_handled();
            return;
        }
        if !self.player_view {
            return;
        }
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
                // The model faces +Z; camera yaw 0 sits on +Z looking along -Z.
                let face = (self.yaw_deg + self.spawn_face_deg).to_radians();
                if let Some(visual) = self.visual.as_mut() {
                    let mut rot = visual.get_rotation();
                    rot.y = face;
                    visual.set_rotation(rot);
                }
            }
            // Jump the camera to the spawned body: a damped follow would otherwise glide in from
            // the rig's origin (under the terrain on hex_island).
            if let Some(pcam) = self.phantom_camera.as_mut() {
                pcam.call_deferred("teleport_position", &[]);
            }
        }

        let input = Input::singleton();
        let mut raw = Vector2::ZERO;
        if self.player_view {
            raw = input.get_vector(ACT_LEFT, ACT_RIGHT, ACT_FORWARD, ACT_BACK);
        }

        // Camera-relative: (right, forward) rotated by the shown camera yaw; yaw 0 looks along -Z.
        let planar = Vector2::new(raw.x, -raw.y).rotated(self.cur_yaw.to_radians());
        let mut wish = Vector3::new(planar.x, 0.0, -planar.y);
        if wish.length_squared() > 1.0 {
            wish = wish.normalized();
        }

        let gravity = ProjectSettings::singleton()
            .get_setting("physics/3d/default_gravity")
            .try_to::<f32>()
            .unwrap_or(9.8);
        let on_floor = self.base().is_on_floor();
        let mut velocity = self.base().get_velocity();
        let mut jumped = false;
        if !on_floor {
            velocity.y -= gravity * self.gravity_scale * dt;
        } else if self.player_view && input.is_action_pressed(ACT_JUMP) {
            velocity.y = self.jump_velocity;
            jumped = true;
        }
        let walking = self.player_view && input.is_action_pressed(ACT_WALK);
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
        } else if let Some(rig) = self.rig.as_mut() {
            rig.animate(dt, speed, max_speed, grounded, stride);
        }
    }

    /// Camera input and easing run per rendered frame (smoother than the physics tick).
    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        if self.player_view {
            let orbit = Input::singleton().get_vector(ACT_CAM_LEFT, ACT_CAM_RIGHT, ACT_CAM_UP, ACT_CAM_DOWN);
            if orbit.length_squared() > 0.0 {
                self.yaw_deg = wrap_deg(self.yaw_deg - orbit.x * self.keyboard_orbit_speed * dt);
                self.pitch_deg = self.clamp_pitch(self.pitch_deg - orbit.y * self.keyboard_orbit_speed * dt);
            }
        }
        let k = if self.camera_smoothing > 0.0 { 1.0 - (-self.camera_smoothing * dt).exp() } else { 1.0 };
        // Stable3D's FInterpTo + settle tolerance: ease, then land exactly on the target.
        let settle = self.orbit_settle_deg.max(0.0);
        let dy = shortest_deg(self.cur_yaw, self.yaw_deg);
        self.cur_yaw = if dy.abs() <= settle { self.yaw_deg } else { wrap_deg(self.cur_yaw + dy * k) };
        let dp = self.pitch_deg - self.cur_pitch;
        self.cur_pitch = if dp.abs() <= settle { self.pitch_deg } else { self.cur_pitch + dp * k };
        // Zoom eases in log space so each wheel step feels the same at any distance.
        let (from, to) = (self.cur_distance.max(0.01).ln(), self.distance.max(0.01).ln());
        self.cur_distance = if (to - from).abs() <= settle.to_radians() { self.distance } else { (from + (to - from) * k).exp() };
        self.update_occlusion(dt);
        self.update_xray(dt);
        self.update_camera();
    }
}

#[godot_api]
impl MstDemoPlayer {
    /// Switches between the player camera and the scene's overview camera.
    #[func]
    pub fn set_player_view(&mut self, on: bool) {
        let on = on || self.overview_camera.is_none();
        self.player_view = on;
        if on {
            if let Some(cam) = self.camera.as_mut() {
                cam.set_current(true);
            }
        } else if let Some(cam) = self.overview_camera.as_mut() {
            cam.set_current(true);
        }
        self.dragging = false;
    }

    #[func]
    pub fn is_player_view(&self) -> bool {
        self.player_view
    }

    /// True once the player has been placed on the terrain.
    #[func]
    pub fn is_spawned(&self) -> bool {
        self.spawned
    }

    /// Distance the camera sits from its follow target: `distance` in perspective,
    /// `max(distance, ortho_camera_distance)` in orthographic.
    #[func]
    pub fn get_camera_arm_length(&self) -> f32 {
        if self.orthographic { self.ortho_camera_distance.max(self.cur_distance) } else { self.cur_distance }
    }

    /// Pitch (degrees) currently added by occlusion avoidance.
    #[func]
    pub fn get_occlusion_pitch(&self) -> f32 {
        self.cur_occ
    }

    /// Pitch the camera is shown at (user / zoom pitch plus occlusion avoidance).
    #[func]
    pub fn get_shown_pitch(&self) -> f32 {
        self.shown_pitch()
    }

    /// True when occluders hide a meaningful part of the player (`occlusion_trigger_points` or
    /// more body rays) at the shown camera angle.
    #[func]
    pub fn is_occluded(&self) -> bool {
        self.occ_points_shown >= self.trigger_points()
    }

    /// Blocked body rays (0-5) at the user's pitch (`shown = false`) or at the shown pitch, as
    /// of the last camera update.
    #[func]
    pub fn get_occlusion_points(&self, shown: bool) -> i32 {
        (if shown { self.occ_points_shown } else { self.occ_points_base }) as i32
    }

    /// Whether avoidance is holding the pitch up (the user's pitch is, or was until less than
    /// `occlusion_release_delay` ago, blocked).
    #[func]
    pub fn is_occlusion_raised(&self) -> bool {
        self.occ_raised
    }

    /// Current x-ray silhouette strength (0 = hidden, 1 = full).
    #[func]
    pub fn get_xray_amount(&self) -> f32 {
        self.xray_amount
    }

    /// Orbits by a mouse delta in pixels.
    #[func]
    pub fn orbit_pixels(&mut self, relative: Vector2) {
        self.yaw_deg = wrap_deg(self.yaw_deg - relative.x * self.mouse_orbit_sensitivity);
        self.pitch_deg = self.clamp_pitch(self.pitch_deg + relative.y * self.mouse_orbit_sensitivity);
    }

    /// Changes the camera distance (positive = farther), with the zoom-linked pitch.
    #[func]
    pub fn zoom(&mut self, amount: f32) {
        self.set_zoom_target(self.distance + amount);
    }

    /// Zooms by wheel steps (positive = farther): `distance *= zoom_factor ^ steps`.
    #[func]
    pub fn zoom_steps(&mut self, steps: f32) {
        self.set_zoom_target(self.distance * self.zoom_factor.max(1.001).powf(steps));
    }

    /// Jumps the eased camera state to its targets (no glide).
    #[func]
    pub fn snap_camera(&mut self) {
        self.cur_yaw = self.yaw_deg;
        self.cur_pitch = self.pitch_deg;
        self.cur_distance = self.distance;
    }

    #[func]
    fn set_orthographic(&mut self, v: bool) {
        self.orthographic = v;
        self.update_camera();
    }

    #[func]
    fn set_distance(&mut self, v: f32) {
        self.distance = v;
        self.cur_distance = v;
        self.update_camera();
    }

    #[func]
    fn set_yaw_deg(&mut self, v: f32) {
        self.yaw_deg = v;
        self.cur_yaw = v;
        self.update_camera();
    }

    #[func]
    fn set_pitch_deg(&mut self, v: f32) {
        self.pitch_deg = v;
        self.cur_pitch = v;
        self.update_camera();
    }
}

impl MstDemoPlayer {
    fn clamp_pitch(&self, v: f32) -> f32 {
        v.clamp(self.pitch_min_deg.min(self.pitch_max_deg), self.pitch_max_deg.max(self.pitch_min_deg))
    }

    /// Yaw the camera is shown at: the eased yaw, in `orbit_yaw_step_deg` steps when set.
    fn shown_yaw(&self) -> f32 {
        let step = self.orbit_yaw_step_deg;
        if step > 0.0 { wrap_deg((self.cur_yaw / step).round() * step) } else { self.cur_yaw }
    }

    fn shown_pitch(&self) -> f32 {
        let top = self.pitch_max_deg.max(self.pitch_min_deg);
        (self.cur_pitch + self.cur_occ).min(top.max(self.cur_pitch))
    }

    fn trigger_points(&self) -> u32 {
        self.occlusion_trigger_points.clamp(1, BODY_RAYS.len() as i32) as u32
    }

    /// How many of the body rays (feet to head, [`BODY_RAYS`], from the body axis: a start
    /// offset would begin inside a wall the knight leans on) an occluder blocks for a camera
    /// at `pitch` / `yaw`. Each ray runs from the body back toward the camera (along the view
    /// axis, up to the arm length); a hit lower than `occlusion_min_height` above the feet does
    /// not count (a step or kerb behind the heels). The terrain is tested with its own height
    /// march (`MarchingSquaresTerrain.raycast`, the visual surface) as well as with physics:
    /// physics rays against the terrain's trimesh walls miss them entirely on hex_island
    /// (seen in `tests/player_capture.gd raydebug`), which left the old avoidance stuck or blind.
    /// None without a physics space.
    fn blocked_count(&self, pitch: f32, yaw: f32) -> Option<u32> {
        let back = Vector3::new(0.0, 0.0, 1.0)
            .rotated(Vector3::RIGHT, -pitch.to_radians())
            .rotated(Vector3::UP, yaw.to_radians());
        self.blocked_along(back, self.get_camera_arm_length())
    }

    /// Blocked body rays toward where the camera really is. The pcam follows with damping, so
    /// right after the knight steps behind a cliff the camera can still be well below the
    /// shown pitch; the x-ray must follow what the camera actually sees.
    fn blocked_for_camera(&self) -> Option<u32> {
        let cam = self.camera.as_ref().filter(|c| c.is_inside_tree())?;
        let target = self.base().get_global_position() + Vector3::UP * self.pivot_height;
        let to_cam = cam.get_global_position() - target;
        let len = to_cam.length();
        if len < 0.01 {
            return None;
        }
        if self.orthographic {
            // Ortho: what matters is the view axis, out to where the camera sits.
            let back = cam.get_global_transform().basis.col_c().normalized();
            return self.blocked_along(back, len);
        }
        self.blocked_along(to_cam / len, len)
    }

    fn blocked_along(&self, back: Vector3, len: f32) -> Option<u32> {
        let body = self.base();
        if !body.is_inside_tree() {
            return None;
        }
        let mut space = body.get_world_3d()?.get_direct_space_state()?;
        let len = (len - 0.5).max(1.0);
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
            let mut query = PhysicsRayQueryParameters3D::create(from, from + back * len)?;
            query.set_collision_mask(self.occlusion_mask);
            query.set_exclude(&exclude);
            // Physics (prop colliders, terrain trimesh) and the terrain's height march: either
            // hit high enough counts (a low physics hit, e.g. a pebble, must not hide a cliff).
            let phys = hit_pos(&space.intersect_ray(&query));
            let terr = terrain
                .as_mut()
                .and_then(|t| t.call("raycast", vslice![from, back, len]).try_to::<VarDictionary>().ok())
                .and_then(|d| hit_pos(&d));
            if [phys, terr].into_iter().flatten().any(|pos| pos.y - feet.y >= self.occlusion_min_height) {
                n += 1;
            }
        }
        Some(n)
    }

    /// Raises the pitch while the user's pitch is blocked (to the lowest pitch that clears every
    /// body ray, plus a margin) and eases it back once the user's pitch has been clear for
    /// `occlusion_release_delay` seconds.
    fn update_occlusion(&mut self, dt: f32) {
        let (base, yaw) = (self.cur_pitch, self.shown_yaw());
        // The body-ray probes (physics rays plus terrain height marches, up to ~80 a frame while
        // the view is raised) run at OCCLUSION_PROBE_HZ; the easing below runs every frame.
        self.occ_probe_wait -= dt;
        self.occ_probe_dt += dt;
        let probe = self.occ_probe_wait <= 0.0;
        if probe {
            self.occ_probe_wait = 1.0 / OCCLUSION_PROBE_HZ;
        }
        let since_probe = self.occ_probe_dt;
        if probe {
            self.occ_probe_dt = 0.0;
        }
        if !self.occlusion_avoid || !self.player_view {
            self.occ_raised = false;
            self.occ_target = 0.0;
            self.occ_points_base = 0;
        } else if !probe {
            // Between probes: keep the last target.
        } else if let Some(n) = self.blocked_count(base, yaw) {
            self.occ_points_base = n;
            if n >= self.trigger_points() {
                self.occ_raised = true;
                self.occ_clear_time = 0.0;
            } else if self.occ_raised {
                // Hysteresis: hold the raised view a moment so walking along a ledge (rays
                // flickering around the trigger) does not bob the camera.
                self.occ_clear_time += since_probe;
                if self.occ_clear_time >= self.occlusion_release_delay {
                    self.occ_raised = false;
                }
            }
            self.occ_target = if self.occ_raised { (self.lowest_clear_pitch(base, yaw) - base).max(0.0) } else { 0.0 };
        }
        let rate = if self.occ_target > self.cur_occ { self.occlusion_rise_speed } else { self.occlusion_return_speed };
        let k = if rate > 0.0 { 1.0 - (-rate * dt).exp() } else { 1.0 };
        // Exponential ease plus a minimum speed (degrees per second, scaled by the rate), so the
        // tail of the ease does not linger: from 37° the return is done in ~0.7 s.
        let diff = self.occ_target - self.cur_occ;
        let step = (diff.abs() * k).max(rate * 4.0 * dt).min(diff.abs());
        self.cur_occ += step.copysign(diff);
        if (self.cur_occ - self.occ_target).abs() < 0.05 {
            self.cur_occ = self.occ_target;
        }
        // Both where the camera really is (it lags the shown pitch) and where it is heading.
        if probe {
            let shown = self.shown_pitch();
            // Unraised view: the shown pitch is the base pitch just probed.
            let at_shown = if shown == base && self.occlusion_avoid && self.player_view {
                self.occ_points_base
            } else {
                self.blocked_count(shown, yaw).unwrap_or(0)
            };
            self.occ_points_shown = self.blocked_for_camera().unwrap_or(0).max(at_shown);
        }
    }

    /// Lowest pitch at or above `base` (in 3° steps) where no body ray is blocked, plus the
    /// margin; `occlusion_pitch_max_deg` when none is.
    fn lowest_clear_pitch(&self, base: f32, yaw: f32) -> f32 {
        let top = self.occlusion_pitch_max_deg.max(base);
        let mut p = base;
        while p < top {
            if self.blocked_count(p, yaw).unwrap_or(0) == 0 {
                return if p > base { (p + self.occlusion_margin_deg).min(top) } else { base };
            }
            p = (p + 3.0).min(top);
        }
        top
    }

    /// Fades the silhouette in when `xray_trigger_points` body rays toward the camera are
    /// blocked, out below `xray_trigger_points - 1` (never without a real occluder).
    fn update_xray(&mut self, dt: f32) {
        if self.xray_materials.len() < 2 {
            return;
        }
        let n = self.occ_points_shown;
        let enter = self.xray_trigger_points.clamp(1, BODY_RAYS.len() as i32) as u32;
        if !self.player_view || n + 1 < enter {
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

    fn clamp_distance(&self, v: f32) -> f32 {
        v.clamp(self.min_distance.min(self.max_distance), self.max_distance.max(self.min_distance))
    }

    /// The pitch the zoom link pulls toward at distance `d` (log-distance lerp near → far).
    fn zoom_pitch(&self, d: f32) -> f32 {
        let (lo, hi) = (self.min_distance.max(0.01), self.max_distance.max(self.min_distance + 0.01));
        let t = ((d.max(lo) / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0);
        self.zoom_pitch_near_deg + (self.zoom_pitch_far_deg - self.zoom_pitch_near_deg) * t
    }

    /// New distance target; with the zoom link, pitch moves by the link curve's change so the
    /// user's own pitch offset survives zooming.
    fn set_zoom_target(&mut self, d: f32) {
        let d = self.clamp_distance(d);
        if self.zoom_pitch_link {
            let delta = self.zoom_pitch(d) - self.zoom_pitch(self.distance);
            self.pitch_deg = self.clamp_pitch(self.pitch_deg + delta);
        }
        self.distance = d;
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

    /// Places the body on the terrain surface; false while the terrain is not ready there yet.
    fn spawn(&mut self) -> bool {
        if !self.base().is_inside_tree() {
            return false;
        }
        if !self.spawn_marker_used {
            self.spawn_marker_used = true;
            if self.spawn_point.is_none() {
                self.spawn_point = self
                    .scene_root()
                    .find_child_ex("PlayerSpawn")
                    .recursive(true)
                    .owned(false)
                    .done()
                    .and_then(|n| n.try_cast::<Node3D>().ok());
            }
            if let Some(marker) = self.spawn_point.as_ref() {
                let p = marker.get_global_position();
                self.base_mut().set_global_position(p);
            }
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

    /// Pushes yaw / pitch / distance to the pcam (or places the fallback camera directly).
    fn update_camera(&mut self) {
        let (ortho, distance, pitch, yaw) = (self.orthographic, self.cur_distance, self.shown_pitch(), self.shown_yaw());
        let arm = self.get_camera_arm_length();
        let fov = self.camera_fov.clamp(1.0, 179.0);
        if let Some(cam) = self.camera.as_mut() {
            if (cam.get_fov() - fov).abs() > 1e-4 {
                cam.set_fov(fov);
            }
            if ortho {
                if cam.get_projection() != ProjectionType::ORTHOGONAL {
                    cam.set_projection(ProjectionType::ORTHOGONAL);
                }
                // Same framing as a perspective camera of this fov at `distance`.
                let size = 2.0 * distance * (fov.to_radians() * 0.5).tan();
                cam.set_size(size);
            } else if cam.get_projection() != ProjectionType::PERSPECTIVE {
                cam.set_projection(ProjectionType::PERSPECTIVE);
            }
            // PixelRenderer snaps perspective cameras on the plane at this distance (the player).
            cam.set_meta("pixel_snap_distance", &arm.to_variant());
            // Keep the whole scene beyond the player inside the far plane.
            let far = arm + 300.0;
            if cam.get_far() < far {
                cam.set_far(far);
            }
        }
        if let Some(pcam) = self.phantom_camera.as_mut() {
            // Spring-arm collision only on request, and never in ortho (see the module docs).
            let mask = if ortho || !self.camera_collision { 0 } else { self.pcam_collision_mask };
            if pcam.get("collision_mask").try_to::<i64>().ok() != Some(mask) {
                pcam.call("set_collision_mask", vslice![mask]);
            }
            let rot = Vector3::new(-pitch, yaw, 0.0);
            pcam.call("set_third_person_rotation_degrees", vslice![rot]);
            pcam.call("set_spring_length", vslice![arm as f64]);
            pcam.call("set_follow_distance", vslice![arm as f64]);
        } else if self.base().is_inside_tree() {
            let pivot = self.base().get_global_position() + Vector3::new(0.0, self.pivot_height, 0.0);
            if let Some(cam) = self.camera.as_mut() {
                let offset = Vector3::new(0.0, 0.0, arm)
                    .rotated(Vector3::RIGHT, -pitch.to_radians())
                    .rotated(Vector3::UP, yaw.to_radians());
                cam.set_global_position(pivot + offset);
                // Looking straight down needs a different up vector.
                let up = if pitch.abs() > 89.5 { Vector3::FORWARD } else { Vector3::UP };
                cam.look_at_ex(pivot).up(up).done();
            }
        }
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

/// Signed shortest angular step (degrees) from `from` to `to`.
fn shortest_deg(from: f32, to: f32) -> f32 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
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
