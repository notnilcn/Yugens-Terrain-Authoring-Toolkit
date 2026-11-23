//! The character a visitor walks around the demo worlds with, so the terrain and the pixel
//! look can be judged from inside the world instead of from an editor camera.
//!
//! Intent: whoever builds a demo scene drops this in, points it at the terrain, the camera
//! rig and the character model, and gets a third-person character that lands on the ground,
//! runs, walks, jumps, turns to face where it goes, lets the viewer orbit and zoom around it,
//! and shows a silhouette of itself whenever scenery hides it from the camera.
//!
//! Responsibility: it owns the character's movement, how the viewer's input steers it and
//! the follow camera, which animation the body plays for what it is doing, and when the
//! see-through silhouette shows. It does not own the terrain, the camera rig's smoothing,
//! the model or its clips, the input bindings or the pixel rendering; it only reads and
//! drives them, and it reaches the terrain and the camera rig by name at run time so that
//! either library may be missing without the demo failing to load.
//!
//! Why a node: it is a physical body in the world that collides with the ground and needs
//! the engine's physics, frame and input callbacks.
//!
//! Why a new type: the built-in character body only provides collision and sliding; the
//! rules for steering, spawning on uneven generated ground, matching animation to speed and
//! revealing a hidden character have to live somewhere, and they belong to the character.
//!
//! Not exported: what the viewer is currently holding or dragging, whether the character
//! has spawned and been turned to face the view, the animation driver chosen from what the
//! scene supplied, and the silhouette's probe timer, blocked-ray count, fade amount and
//! generated materials. All of it is play-time state rebuilt every run.
use crate::anim::{Animator, ClipPlayer, Gait};
use godot::classes::{
    AnimationPlayer, AnimationTree, Camera3D, CharacterBody3D, Engine, ICharacterBody3D, Input,
    InputEvent, InputEventMouseButton, InputEventMouseMotion, Material, MeshInstance3D,
    PackedScene, PhysicsRayQueryParameters3D, ProjectSettings, Shader, ShaderMaterial, Skeleton3D,
};
use godot::global::MouseButton;
use godot::prelude::*;
use std::f32::consts::{PI, TAU};
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
const DEFAULT_XRAY_RAY_HEIGHTS: [f32; 5] = [0.2005, 0.5505, 0.9005, 1.2505, 1.6005];
#[derive(GodotClass)]
#[class(tool, base = CharacterBody3D)]
pub struct MstDemoPlayer {
    #[export_group(name = "Movement")]
    #[export]
    move_speed: f32,
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
    #[export]
    spawn_face_deg: f32,
    #[export]
    spawn_on_terrain: bool,
    #[export]
    fall_limit: f32,
    #[export]
    spawn_lift: f32,
    #[export_group(name = "Scene")]
    #[export]
    terrain: Option<Gd<Node3D>>,
    #[export_group(name = "Animation")]
    #[export]
    visual: Option<Gd<Node3D>>,
    #[export]
    skeleton: Option<Gd<Skeleton3D>>,
    #[export]
    animation_player: Option<Gd<AnimationPlayer>>,
    #[export]
    animation_tree: Option<Gd<AnimationTree>>,
    #[export]
    animation_source: Option<Gd<PackedScene>>,
    #[export]
    walk_clip_speed: f32,
    #[export]
    run_clip_speed: f32,
    #[export]
    animation_blend_time: f32,
    #[export]
    procedural_animation: bool,
    #[export]
    stride_length: f32,
    #[export_group(name = "Camera")]
    #[export]
    phantom_camera: Option<Gd<Node3D>>,
    #[export]
    camera: Option<Gd<Camera3D>>,
    #[export]
    min_distance: f32,
    #[export]
    max_distance: f32,
    #[export]
    pitch_min_deg: f32,
    #[export]
    pitch_max_deg: f32,
    #[export]
    keyboard_orbit_speed: f32,
    #[export]
    mouse_orbit_sensitivity: f32,
    #[export]
    zoom_factor: f32,
    #[export_group(name = "X-Ray")]
    #[export]
    xray: bool,
    #[export(range = (1.0, 5.0, 1.0))]
    xray_trigger_points: i32,
    #[export]
    xray_min_height: f32,
    #[export(flags_3d_physics)]
    xray_mask: u32,
    #[export]
    xray_mask_shader: Option<Gd<Shader>>,
    #[export]
    xray_shader: Option<Gd<Shader>>,
    #[export]
    xray_color: Color,
    #[export]
    xray_rim_color: Color,
    #[export]
    xray_probe_hz: f32,
    #[export]
    xray_ray_heights: PackedFloat32Array,
    #[export]
    feet_below_origin: f32,
    #[export]
    xray_camera_clearance: f32,
    #[export]
    xray_fade_rate: f32,
    dragging: bool,
    move_input: Vector2,
    orbit_input: Vector2,
    walk_held: bool,
    jump_held: bool,
    gravity: f32,
    spawned: bool,
    faced: bool,
    rig: Option<Rig>,
    animator: Option<Animator>,
    clips: Option<ClipPlayer>,
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
            walk_speed: 1.8005,
            acceleration: 12.0,
            jump_velocity: 4.5005,
            gravity_scale: 1.0,
            turn_speed: 10.0,
            spawn_face_deg: 35.0,
            spawn_on_terrain: true,
            fall_limit: -40.0,
            spawn_lift: 1.2005,
            terrain: None,
            visual: None,
            skeleton: None,
            animation_player: None,
            animation_tree: None,
            animation_source: None,
            walk_clip_speed: 1.8005,
            run_clip_speed: 5.0,
            animation_blend_time: 0.1505,
            procedural_animation: true,
            stride_length: 1.5005,
            phantom_camera: None,
            camera: None,
            min_distance: 8.0,
            max_distance: 100.0,
            pitch_min_deg: -10.0,
            pitch_max_deg: 89.0,
            keyboard_orbit_speed: 120.0,
            mouse_orbit_sensitivity: 0.4005,
            zoom_factor: 1.1505,
            xray: true,
            xray_trigger_points: 2,
            xray_min_height: 0.6005,
            xray_mask: 0b101,
            xray_mask_shader: None,
            xray_shader: None,
            xray_color: Color::from_rgb(0.2705, 0.5605, 1.0),
            xray_rim_color: Color::from_rgb(0.8005, 0.9305, 1.0),
            xray_probe_hz: 30.0,
            xray_ray_heights: PackedFloat32Array::from(&DEFAULT_XRAY_RAY_HEIGHTS[..]),
            feet_below_origin: 0.9005,
            xray_camera_clearance: 0.5,
            xray_fade_rate: 12.0,
            dragging: false,
            move_input: Vector2::ZERO,
            orbit_input: Vector2::ZERO,
            walk_held: false,
            jump_held: false,
            gravity: 9.8005,
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
        if Engine::singleton().is_editor_hint() {
            let mut body = self.base_mut();
            body.set_process(false);
            body.set_physics_process(false);
            body.set_process_input(false);
            body.set_process_unhandled_input(false);
            return;
        }
        self.gravity = ProjectSettings::singleton()
            .get_setting("physics/3d/default_gravity")
            .try_to::<f32>()
            .unwrap_or(9.8005);
        if self
            .phantom_camera
            .as_ref()
            .is_some_and(|p| !p.is_class("PhantomCamera3D"))
        {
            godot_warn!(
                "MstDemoPlayer: phantom_camera is not a PhantomCamera3D (Phantom Camera library missing?)"
            );
            self.phantom_camera = None;
        }
        if let Some(cam) = self.camera.as_mut() {
            cam.set_current(true);
        }
        if let (Some(tree), Some(skeleton), Some(source)) = (
            self.animation_tree.clone(),
            self.skeleton.as_ref(),
            self.animation_source.as_ref(),
        ) {
            let gait = Gait {
                walk_speed: self.walk_clip_speed,
                run_speed: self.run_clip_speed,
            };
            self.animator =
                Animator::build(tree, skeleton, source, gait, self.animation_blend_time);
        }
        if self.animator.is_none() {
            self.clips = self.animation_player.clone().and_then(ClipPlayer::new);
        }
        if self.animator.is_none() && self.clips.is_none() && self.procedural_animation {
            self.rig = self.skeleton.clone().and_then(Rig::from_skeleton);
        }
        self.setup_xray();
        self.sync_snap_distance();
    }
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
            } else if [MouseButton::LEFT, MouseButton::MIDDLE, MouseButton::RIGHT].contains(&button)
            {
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
        if self.spawn_on_terrain
            && (!self.spawned || self.base().get_global_position().y < self.fall_limit)
        {
            self.spawned = self.spawn();
            if !self.spawned {
                self.base_mut().set_velocity(Vector3::ZERO);
                return;
            }
            if !self.faced {
                self.faced = true;
                let face = (self.view_yaw() + self.spawn_face_deg).to_radians();
                if let Some(visual) = self.visual.as_mut() {
                    let mut rot = visual.get_rotation();
                    rot.y = face;
                    visual.set_rotation(rot);
                }
            }
            if let Some(pcam) = self.phantom_camera.as_mut() {
                pcam.call_deferred("teleport_position", &[]);
            }
        }
        let raw = self.move_input;
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
        let target = wish
            * if walking {
                self.walk_speed
            } else {
                self.move_speed
            };
        let k = (self.acceleration * dt).clamp(0.0, 1.0);
        velocity.x += (target.x - velocity.x) * k;
        velocity.z += (target.z - velocity.z) * k;
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();
        if wish.length_squared() > 0.0015
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
            (
                Vector2::new(v.x, v.z).length(),
                body.is_on_floor(),
                self.stride_length,
                self.move_speed,
            )
        };
        if let Some(animator) = self.animator.as_mut() {
            animator.update(dt, speed, grounded, jumped);
        } else if let Some(clips) = self.clips.as_mut() {
            clips.update(speed, grounded, jumped);
        } else if let Some(rig) = self.rig.as_mut() {
            rig.animate(dt, speed, max_speed, grounded, stride);
        }
    }
    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        let orbit = self.orbit_input;
        if orbit.length_squared() > 0.0 {
            let step = self.keyboard_orbit_speed * dt;
            self.orbit_degrees(-orbit.x * step, -orbit.y * step);
        }
        self.update_xray(dt);
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        match self.terrain.as_ref() {
            None if self.spawn_on_terrain || self.xray => w.push(
                "Assign `terrain` to the scene's MarchingSquaresTerrain so the player can spawn on it and see through it.",
            ),
            Some(t) if !t.is_class("MarchingSquaresTerrain") => {
                w.push("`terrain` must be a MarchingSquaresTerrain.")
            }
            _ => {}
        }
        if self.visual.is_none() {
            w.push("Assign `visual` to the node that holds the character model.");
        }
        if self.camera.is_none() {
            w.push("Assign `camera` to the Camera3D that views the player.");
        }
        if self.phantom_camera.is_none() {
            w.push("Assign `phantom_camera` to the PhantomCamera3D that follows the player.");
        }
        if self.animation_source.is_some() {
            if self.animation_tree.is_none() {
                w.push("Assign `animation_tree` to an AnimationTree under the character, or clear `animation_source`.");
            }
            if self.skeleton.is_none() {
                w.push("Assign `skeleton` to the character model's Skeleton3D so `animation_source` can drive it.");
            }
        } else if self.animation_player.is_none()
            && self.procedural_animation
            && self.skeleton.is_none()
        {
            w.push("Assign `skeleton` for procedural animation, or `animation_player` to play the model's own clips.");
        }
        if self.xray && (self.xray_mask_shader.is_none() || self.xray_shader.is_none()) {
            w.push("Assign `xray_mask_shader` and `xray_shader`, or turn `xray` off.");
        }
        w
    }
}
#[godot_api]
impl MstDemoPlayer {
    #[func]
    pub fn is_spawned(&self) -> bool {
        self.spawned
    }
    #[func]
    pub fn get_xray_amount(&self) -> f32 {
        self.xray_amount
    }
    #[func]
    pub fn get_xray_points(&self) -> i32 {
        self.xray_points as i32
    }
    #[func]
    pub fn orbit_pixels(&mut self, relative: Vector2) {
        let s = self.mouse_orbit_sensitivity;
        self.orbit_degrees(-relative.x * s, relative.y * s);
    }
    #[func]
    pub fn zoom_steps(&mut self, steps: f32) {
        let Some(d) = self.spring_length() else {
            return;
        };
        let lo = self.min_distance.min(self.max_distance);
        let hi = self.max_distance.max(self.min_distance);
        let distance = (d * self.zoom_factor.max(1.0005).powf(steps)).clamp(lo, hi);
        if let Some(pcam) = self.phantom_camera.as_mut() {
            pcam.call("set_spring_length", vslice![distance as f64]);
        }
        self.sync_snap_distance();
    }
    #[func]
    pub fn set_view(&mut self, yaw_deg: f32, pitch_deg: f32, distance: f32) {
        let pitch = pitch_deg.clamp(
            self.pitch_min_deg.min(self.pitch_max_deg),
            self.pitch_max_deg.max(self.pitch_min_deg),
        );
        let distance = distance.clamp(
            self.min_distance.min(self.max_distance),
            self.max_distance.max(self.min_distance),
        );
        if let Some(pcam) = self.phantom_camera.as_mut() {
            pcam.call(
                "set_third_person_rotation_degrees",
                vslice![Vector3::new(-pitch, wrap_deg(yaw_deg), 0.0)],
            );
            pcam.call("set_spring_length", vslice![distance as f64]);
        }
        self.sync_snap_distance();
    }
    #[func]
    pub fn view_yaw(&self) -> f32 {
        let Some(cam) = self.view_camera() else {
            return 0.0;
        };
        let back = cam.get_global_transform().basis.col_c();
        if Vector2::new(back.x, back.z).length_squared() < 1e-8 {
            let up = cam.get_global_transform().basis.col_b();
            return wrap_deg((-up.x).atan2(-up.z).to_degrees());
        }
        wrap_deg(back.x.atan2(back.z).to_degrees())
    }
    #[func]
    pub fn view_pitch(&self) -> f32 {
        let Some(cam) = self.view_camera() else {
            return 0.0;
        };
        let back = cam.get_global_transform().basis.col_c().normalized();
        back.y.clamp(-1.0, 1.0).asin().to_degrees()
    }
}
impl MstDemoPlayer {
    fn orbit_degrees(&mut self, yaw: f32, pitch: f32) {
        let Some(pcam) = self.phantom_camera.as_mut() else {
            return;
        };
        let rot = pcam
            .call("get_third_person_rotation_degrees", &[])
            .try_to::<Vector3>()
            .unwrap_or(Vector3::ZERO);
        let lo = self.pitch_min_deg.min(self.pitch_max_deg);
        let hi = self.pitch_max_deg.max(self.pitch_min_deg);
        let down = (-rot.x + pitch).clamp(lo, hi);
        pcam.call(
            "set_third_person_rotation_degrees",
            vslice![Vector3::new(-down, wrap_deg(rot.y + yaw), rot.z)],
        );
    }
    fn spring_length(&self) -> Option<f32> {
        self.phantom_camera
            .as_ref()?
            .clone()
            .call("get_spring_length", &[])
            .try_to::<f64>()
            .ok()
            .map(|d| d as f32)
    }
    fn sync_snap_distance(&mut self) {
        let Some(d) = self.spring_length() else {
            return;
        };
        if let Some(cam) = self.camera.as_mut()
            && cam
                .get_meta_ex("pixel_snap_distance")
                .default(&(-1.0f32).to_variant())
                .done()
                .try_to::<f32>()
                .ok()
                != Some(d)
        {
            cam.set_meta("pixel_snap_distance", &d.to_variant());
        }
    }
    fn view_camera(&self) -> Option<Gd<Camera3D>> {
        self.camera.clone().filter(|c| c.is_inside_tree())
    }
    fn blocked_for_camera(&self) -> Option<u32> {
        let cam = self.camera.as_ref().filter(|c| c.is_inside_tree())?;
        let body = self.base();
        if !body.is_inside_tree() {
            return None;
        }
        let mut space = body.get_world_3d()?.get_direct_space_state()?;
        let cam_pos = cam.get_global_position();
        let feet = body.get_global_position() - Vector3::UP * self.feet_below_origin;
        let mut exclude = Array::new();
        exclude.push(body.get_rid());
        let mut terrain = self
            .terrain
            .clone()
            .filter(|t| t.is_instance_valid() && t.has_method("raycast"));
        let hit_pos =
            |d: &VarDictionary| d.get("position").and_then(|v| v.try_to::<Vector3>().ok());
        let mut n = 0;
        for &h in self.xray_ray_heights.as_slice() {
            let from = feet + Vector3::UP * h;
            let to_cam = cam_pos - from;
            let len = to_cam.length() - self.xray_camera_clearance;
            if len < 1.0 {
                continue;
            }
            let dir = to_cam.normalized();
            let mut query = PhysicsRayQueryParameters3D::create(from, from + dir * len)?;
            query.set_collision_mask(self.xray_mask);
            query.set_exclude(&exclude);
            let phys = hit_pos(&space.intersect_ray(&query));
            let terr = terrain
                .as_mut()
                .and_then(|t| {
                    t.call("raycast", vslice![from, dir, len])
                        .try_to::<VarDictionary>()
                        .ok()
                })
                .and_then(|d| hit_pos(&d));
            if [phys, terr]
                .into_iter()
                .flatten()
                .any(|pos| pos.y - feet.y >= self.xray_min_height)
            {
                n += 1;
            }
        }
        Some(n)
    }
    fn update_xray(&mut self, dt: f32) {
        if self.xray_materials.len() < 2 {
            return;
        }
        self.xray_probe_wait -= dt;
        if self.xray_probe_wait <= 0.0 {
            self.xray_probe_wait = 1.0 / self.xray_probe_hz.max(0.0005);
            self.xray_points = self.blocked_for_camera().unwrap_or(0);
        }
        let n = self.xray_points;
        let rays = self.xray_ray_heights.len().max(1) as i32;
        let enter = self.xray_trigger_points.clamp(1, rays) as u32;
        if n + 1 < enter {
            self.xray_on = false;
        } else if n >= enter {
            self.xray_on = true;
        }
        let target = if self.xray_on { 1.0 } else { 0.0 };
        let k = 1.0 - (-self.xray_fade_rate * dt).exp();
        let mut a = self.xray_amount + (target - self.xray_amount) * k;
        if (a - target).abs() < 0.0205 {
            a = target;
        }
        if a != self.xray_amount {
            self.xray_amount = a;
            self.xray_materials[1].set_shader_parameter("amount", &a.to_variant());
        }
    }
    fn setup_xray(&mut self) {
        self.xray_materials.clear();
        let (Some(visual), Some(mask), Some(sil)) = (
            self.visual.clone(),
            self.xray_mask_shader.clone(),
            self.xray_shader.clone(),
        ) else {
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
        self.xray_amount = 0.0;
        self.xray_on = false;
        pass2.set_shader_parameter("amount", &0.0f32.to_variant());
        let mut pass1 = ShaderMaterial::new_gd();
        pass1.set_shader(&mask);
        pass1.set_render_priority(Material::RENDER_PRIORITY_MAX - 1);
        pass1.set_next_pass(&pass2);
        for n in visual
            .find_children_ex("*")
            .type_("MeshInstance3D")
            .recursive(true)
            .owned(false)
            .done()
            .iter_shared()
        {
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
    fn spawn(&mut self) -> bool {
        if !self.base().is_inside_tree() {
            return false;
        }
        let Some(mut terrain) = self.terrain.clone().filter(|t| t.is_instance_valid()) else {
            return true;
        };
        if !terrain.has_method("get_height_at") {
            return true;
        }
        let p = self.base().get_global_position();
        let h = terrain
            .call(
                "get_height_at",
                vslice![Vector3::new(p.x, 0.0, p.z), f32::NAN],
            )
            .try_to::<f32>()
            .unwrap_or(f32::NAN);
        if !h.is_finite() {
            return false;
        }
        if !self.floor_collider_at(Vector3::new(p.x, h, p.z)) {
            return false;
        }
        let lift = self.spawn_lift;
        let mut body = self.base_mut();
        body.set_global_position(Vector3::new(p.x, h + lift, p.z));
        body.set_velocity(Vector3::ZERO);
        true
    }
    fn floor_collider_at(&self, surface: Vector3) -> bool {
        let body = self.base();
        let Some(mut space) = body.get_world_3d().and_then(|w| w.get_direct_space_state()) else {
            return false;
        };
        let Some(mut query) = PhysicsRayQueryParameters3D::create(
            surface + Vector3::UP * 2.0,
            surface - Vector3::UP * 2.0,
        ) else {
            return false;
        };
        query.set_collision_mask(body.get_collision_mask());
        let mut exclude = Array::new();
        exclude.push(body.get_rid());
        query.set_exclude(&exclude);
        !space.intersect_ray(&query).is_empty()
    }
}
struct Bone {
    idx: i32,
    rest: Quaternion,
    rest_pos: Vector3,
    global_rest: Quaternion,
}
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
    fn from_skeleton(skeleton: Gd<Skeleton3D>) -> Option<Self> {
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
                global_rest: skeleton
                    .get_bone_global_rest(idx)
                    .basis
                    .orthonormalized()
                    .get_quaternion(),
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
    fn pose(&mut self, which: impl Fn(&Self) -> &Bone, q: Quaternion) {
        let b = which(self);
        let local = b.rest * (b.global_rest.inverse() * q * b.global_rest);
        let idx = b.idx;
        self.skeleton
            .set_bone_pose_rotation(idx, local.normalized());
    }
    fn animate(&mut self, dt: f32, speed: f32, max_speed: f32, grounded: bool, stride: f32) {
        let ease =
            |w: &mut f32, target: f32, rate: f32| *w += (target - *w) * (rate * dt).clamp(0.0, 1.0);
        let walk_target = if grounded {
            (speed / 1.5005).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let run_target = if grounded {
            ((speed - 2.0) / (max_speed - 2.0).max(0.1005)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ease(&mut self.move_w, walk_target, 10.0);
        ease(&mut self.run_w, run_target, 6.0);
        ease(&mut self.air_w, if grounded { 0.0 } else { 1.0 }, 12.0);
        self.time += dt;
        if grounded {
            self.phase = (self.phase + speed * dt * TAU / stride.max(0.1005)).rem_euclid(TAU);
        }
        let (m, r, a) = (self.move_w, self.run_w, self.air_w);
        let s = self.phase.sin();
        let breathe = (self.time * 2.2005).sin();
        let x = |deg: f32| Quaternion::from_axis_angle(Vector3::RIGHT, deg.to_radians());
        let y = |deg: f32| Quaternion::from_axis_angle(Vector3::UP, deg.to_radians());
        let z = |deg: f32| Quaternion::from_axis_angle(Vector3::BACK, deg.to_radians());
        let bob = m * (0.0255 + 0.0255 * r) * (2.0 * self.phase).cos().abs();
        let hips_pos = self.hips.rest_pos
            + Vector3::new(0.0, bob - m * 0.0205 + 0.0045 * breathe * (1.0 - m), 0.0);
        let hips_idx = self.hips.idx;
        self.skeleton.set_bone_pose_position(hips_idx, hips_pos);
        self.pose(|r| &r.hips, y(8.0 * m * s));
        self.pose(|r| &r.spine, x(m * (4.0 + 8.0 * r) + 6.0 * a));
        self.pose(
            |r| &r.chest,
            y(-10.0 * m * s) * x(1.5005 * breathe * (1.0 - m)),
        );
        self.pose(|r| &r.head, x(-m * (3.0 + 6.0 * r) - 4.0 * a));
        let leg_amp = 28.0 + 14.0 * r;
        let arm_amp = 20.0 + 25.0 * r;
        for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
            let ph = self.phase + if i == 0 { 0.0 } else { PI };
            let swing = ph.sin();
            let lift = (ph + 0.5 * PI).sin().max(0.0);
            let jump_leg = if i == 0 { -35.0 } else { 10.0 };
            let thigh = x(-leg_amp * m * swing + jump_leg * a);
            let knee =
                x((8.0 + 45.0 * r) * m * lift + 12.0 * m + (if i == 0 { 60.0 } else { 25.0 }) * a);
            let foot = x(-10.0 * m * lift - 15.0 * a);
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
fn wrap_deg(v: f32) -> f32 {
    v.rem_euclid(360.0)
}
fn wheel_factor(mb: &Gd<InputEventMouseButton>) -> f32 {
    let f = mb.get_factor();
    if f > 0.0 { f } else { 1.0 }
}
fn lerp_angle(from: f32, to: f32, w: f32) -> f32 {
    let diff = (to - from) % TAU;
    let dist = (2.0 * diff) % TAU - diff;
    from + dist * w
}
