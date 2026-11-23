//! A camera rig for a 3-D scene: where the view should sit, what it should face, and how it
//! should move there.
//!
//! Intent: a scene author places one of these to describe a way of watching a 3-D level -
//! stuck to one thing, orbiting behind it, following one thing or several, tracing an
//! authored path, framing a group, or riding a collision-aware boom - and then tunes how
//! that watching feels: distances, damping, look-ahead, the up direction, dead zones, and
//! the camera look settings that belong to this framing. Several of these can sit in one
//! scene and the author switches between them without touching code.
//!
//! Responsibility: it owns one rig's following and facing rules, the motion toward the
//! result, and the camera-wide settings it contributes when it becomes live. It does not
//! own the camera that renders, does not decide which rig is live, and does not own the move
//! between rigs; those belong to the scene's camera driver. It also does not own the noise
//! curves it can be given, only the choice to apply them.
//!
//! Why a node: a rig is placed in the level where its position and facing mean something,
//! its targets are authored beside it, it has to advance with frames, and it should appear
//! and disappear with the part of the level it belongs to. All of that comes from a place in
//! the tree.
//!
//! Why a new type: no built-in 3-D node follows a target with damping, dead zones,
//! look-ahead, group framing, authored bounds or a spring boom, and a built-in camera would
//! have to be the rendering camera itself, which would rule out several alternative framings
//! in one scene.
//!
//! Not exported: the position and rotation the rig is currently converging on, the
//! velocities of that convergence, the bounds it resolved from an authored limit target, the
//! distance a boom settled on, the pending axis lock and noise it has not yet applied, the
//! last transform handed on, and whether this rig is the live one. Those are results of
//! running the rig; every value that decides how it behaves is exported, including its
//! priority, its layers, its targets and its look settings.
//!
//! It announces itself and its changes through the addon's registry, the one global the
//! addon's parts share, so that whatever drives a camera can find it without being wired to
//! it in the scene.
//!
//! When the rig rides a collision-aware boom, it builds that boom when the game starts and
//! moves itself onto it, rather than asking the scene author to declare one: the boom is
//! part of how this way of following works, not something an author arranges, and the rig
//! must stay where the author placed it so that whatever already refers to it keeps
//! working.

use godot::builtin::math::ApproxEq;
use godot::builtin::{Array, Transform3D, Vector2, Vector3};
use godot::classes::{Engine, Node3D, Path3D, Shape3D, SpringArm3D};
use godot::prelude::*;
use crate::abi::{
    PcamKind, TWEEN_DEFAULT_DURATION, TWEEN_DEFAULT_EASE, TWEEN_DEFAULT_TRANSITION, set_layer,
};
use crate::manager::PhantomCameraManager;
use crate::resources::{Camera3DResource, PhantomCameraNoise3D, PhantomCameraTween};
fn clampf(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
fn f64_of(value: real) -> f64 {
    value as f64
}
fn real_of(value: f64) -> real {
    value as real
}
fn is_zero_approx(value: f64) -> bool {
    value.abs() < 0.0005
}
fn axis3(value: &Vector3, index: usize) -> real {
    match index {
        0 => value.x,
        1 => value.y,
        _ => value.z,
    }
}
fn set_axis3(value: &mut Vector3, index: usize, component: real) {
    match index {
        0 => value.x = component,
        1 => value.y = component,
        _ => value.z = component,
    }
}
#[derive(GodotClass)]
#[class(base = Node3D, tool)]
pub struct PhantomCamera3D {
    base: Base<Node3D>,
    #[export]
    #[var(set = set_priority_override, get = get_priority_override)]
    pub priority_override: bool,
    #[export]
    #[var(set = set_priority, get = get_priority)]
    pub priority: i64,
    #[export]
    #[var(
        set = set_follow_mode,
        get = get_follow_mode,
        hint = ENUM,
        hint_string = "None:0,Glued:1,Simple:2,Group:3,Path:4,Framed:5,Third Person:6"
    )]
    pub follow_mode: i64,
    #[export]
    #[var(set = set_follow_target, get = get_follow_target)]
    pub follow_target: Option<Gd<Node3D>>,
    #[export]
    #[var(set = set_follow_targets, get = get_follow_targets)]
    pub follow_targets: Array<Gd<Node3D>>,
    #[export]
    #[var(set = set_follow_path, get = get_follow_path)]
    pub follow_path: Option<Gd<Path3D>>,
    #[export]
    #[var(
        set = set_look_at_mode,
        get = get_look_at_mode,
        hint = ENUM,
        hint_string = "None:0,Mimic:1,Simple:2,Group:3"
    )]
    pub look_at_mode: i64,
    #[export]
    #[var(set = set_look_at_target, get = get_look_at_target)]
    pub look_at_target: Option<Gd<Node3D>>,
    #[export]
    #[var(set = set_look_at_targets, get = get_look_at_targets)]
    pub look_at_targets: Array<Gd<Node3D>>,
    #[export]
    #[var(set = set_tween_resource, get = get_tween_resource)]
    pub tween_resource: Option<Gd<PhantomCameraTween>>,
    #[export]
    #[var(set = set_tween_on_load, get = get_tween_on_load)]
    pub tween_on_load: bool,
    #[export]
    #[var(
        set = set_inactive_update_mode,
        get = get_inactive_update_mode,
        hint = ENUM,
        hint_string = "Always:0,Never:1"
    )]
    pub inactive_update_mode: i64,
    #[export(flags_3d_render)]
    #[var(set = set_host_layers, get = get_host_layers)]
    pub host_layers: i64,
    #[export]
    #[var(set = set_camera_3d_resource, get = get_camera_3d_resource)]
    pub camera_3d_resource: Option<Gd<Camera3DResource>>,
    #[export]
    #[var(set = set_attributes, get = get_attributes)]
    pub attributes: Option<Gd<godot::classes::CameraAttributes>>,
    #[export]
    #[var(set = set_environment, get = get_environment)]
    pub environment: Option<Gd<godot::classes::Environment>>,
    #[export]
    #[var(
        set = set_compositor,
        get = get_compositor,
        hint = RESOURCE_TYPE,
        hint_string = "Compositor"
    )]
    pub compositor: Option<Gd<godot::classes::Resource>>,
    #[export]
    #[var(set = set_follow_offset, get = get_follow_offset)]
    pub follow_offset: Vector3,
    #[export]
    #[var(set = set_follow_damping, get = get_follow_damping)]
    pub follow_damping: bool,
    #[export]
    #[var(set = set_follow_damping_value, get = get_follow_damping_value, hint = LINK)]
    pub follow_damping_value: Vector3,
    #[export]
    #[var(
        set = set_follow_axis_lock,
        get = get_follow_axis_lock,
        hint = ENUM,
        hint_string = "None:0,X:1,Y:2,Z:3,Xy:4,Xz:5,Yz:6,Xyz:7"
    )]
    pub follow_axis_lock: i64,
    follow_axis_is_locked: bool,
    follow_axis_lock_value: Vector3,
    #[export]
    #[var(set = set_follow_distance, get = get_follow_distance)]
    pub follow_distance: f64,
    #[export]
    #[var(set = set_auto_follow_distance, get = get_auto_follow_distance)]
    pub auto_follow_distance: bool,
    #[export]
    #[var(set = set_auto_follow_distance_min, get = get_auto_follow_distance_min)]
    pub auto_follow_distance_min: f64,
    #[export]
    #[var(set = set_auto_follow_distance_max, get = get_auto_follow_distance_max)]
    pub auto_follow_distance_max: f64,
    #[export]
    #[var(
        set = set_auto_follow_distance_divisor,
        get = get_auto_follow_distance_divisor
    )]
    pub auto_follow_distance_divisor: f64,
    #[export]
    #[var(
        set = set_dead_zone_width,
        get = get_dead_zone_width,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_width: f64,
    #[export]
    #[var(
        set = set_dead_zone_height,
        get = get_dead_zone_height,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_height: f64,
    #[export]
    pub show_viewfinder_in_play: bool,
    #[export]
    #[var(
        set = set_vertical_rotation_offset,
        get = get_vertical_rotation_offset,
        hint = RANGE,
        hint_string = "-360.0,360.0,0.1,or_greater,or_less,radians_as_degrees"
    )]
    pub vertical_rotation_offset: f64,
    #[export]
    #[var(
        set = set_horizontal_rotation_offset,
        get = get_horizontal_rotation_offset,
        hint = RANGE,
        hint_string = "-360.0,360.0,0.1,or_greater,or_less,radians_as_degrees"
    )]
    pub horizontal_rotation_offset: f64,
    #[export]
    #[var(set = set_spring_length, get = get_spring_length)]
    pub spring_length: f64,
    #[export(flags_3d_physics)]
    #[var(set = set_collision_mask, get = get_collision_mask)]
    pub collision_mask: i64,
    #[export]
    #[var(set = set_shape, get = get_shape)]
    pub shape: Option<Gd<Shape3D>>,
    #[export]
    #[var(set = set_margin, get = get_margin)]
    pub margin: f64,
    #[export]
    #[var(set = set_look_at_offset, get = get_look_at_offset)]
    pub look_at_offset: Vector3,
    #[export]
    #[var(set = set_look_at_damping, get = get_look_at_damping)]
    pub look_at_damping: bool,
    #[export]
    #[var(
        set = set_look_at_damping_value,
        get = get_look_at_damping_value,
        hint = RANGE,
        hint_string = "0.0,1.0,0.001,or_greater"
    )]
    pub look_at_damping_value: f64,
    #[export]
    #[var(set = set_up, get = get_up)]
    pub up: Vector3,
    #[export]
    #[var(set = set_up_target, get = get_up_target)]
    pub up_target: Option<Gd<Node3D>>,
    #[export]
    #[var(set = set_noise, get = get_noise)]
    pub noise: Option<Gd<PhantomCameraNoise3D>>,
    #[export]
    #[var(set = set_preview_noise, get = get_preview_noise)]
    pub _preview_noise: bool,
    #[export(flags_3d_render)]
    #[var(set = set_noise_emitter_layer, get = get_noise_emitter_layer)]
    pub noise_emitter_layer: i64,
    #[export]
    #[var(set = assign_window_index, get = get_viewport_index, hint = RANGE, hint_string = "0.0,3.0")]
    pub viewport_index: i64,
    #[export]
    #[var(set = set_draw_follow_line, get = get_draw_follow_line)]
    pub draw_follow_line: bool,
    #[export]
    #[var(set = set_draw_look_at_line, get = get_draw_look_at_line)]
    pub draw_look_at_line: bool,
    is_active_pcam: bool,
    is_third_person_follow: bool,
    camera_target: Option<Gd<Node3D>>,
    should_follow: bool,
    follow_target_physics_based: bool,
    has_multiple_follow_targets: bool,
    follow_targets_single_target_index: i64,
    should_look_at: bool,
    has_multiple_look_at_targets: bool,
    look_at_targets_single_target_index: i64,
    current_rotation: Vector3,
    up_direction: Vector3,
    has_up_target: bool,
    follow_target_output_position: Vector3,
    look_at_target_output_position: Vector3,
    transform_output: Transform3D,
    transform_noise: Transform3D,
    tween_skip: bool,
    follow_velocity_ref: Vector3,
    follow_framed_initial_set: bool,
    follow_framed_offset: Vector3,
    follow_spring_arm: Option<Gd<SpringArm3D>>,
    has_follow_spring_arm: bool,
    has_noise_resource: bool,
    draw_gizmo: bool,
    draw_follow_gizmo_line: bool,
    follow_target_position: Vector3,
    draw_look_at_gizmo_line: bool,
    pending_noise: Option<Transform3D>,
    pending_spring_arm: Option<(Gd<SpringArm3D>, Vector3)>,
    pending_axis_lock: Option<i64>,
    tween_duration_fallback: f64,
    tween_transition_fallback: i64,
    tween_ease_fallback: i64,
    pub viewport_position: Vector2,
}
#[godot_api]
impl PhantomCamera3D {
    #[signal]
    fn became_active();
    #[signal]
    fn became_inactive();
    #[signal]
    fn follow_mode_changed();
    #[signal]
    fn follow_target_changed();
    #[signal]
    fn look_at_target_changed();
    #[signal]
    fn dead_zone_changed();
    #[signal]
    fn dead_zone_reached();
    #[signal]
    fn tween_started();
    #[signal]
    fn is_tweening();
    #[signal]
    fn tween_interrupted(pcam: Gd<PhantomCamera3D>);
    #[signal]
    fn tween_completed();
    #[signal]
    fn noise_emitted(noise_output: Transform3D);
    #[signal]
    fn physics_target_changed();
    #[signal]
    fn camera_3d_resource_property_changed(property: StringName, value: Variant);
    #[signal]
    fn camera_3d_resource_changed();
    #[func]
    pub fn pcam_kind(&self) -> GString {
        GString::from(PcamKind::ThreeD.as_str())
    }
    pub fn is_in_host_layer(&self, host_layers: i64) -> bool {
        self.host_layers & host_layers != 0
    }
    pub fn current_priority(&self) -> i64 {
        self.priority
    }
    pub fn has_follow_damping(&self) -> bool {
        self.follow_damping
    }
    pub fn pcam_is_visible(&self) -> bool {
        self.base().is_visible_in_tree()
    }
    pub fn host_set_active(&mut self, value: bool) {
        self.is_active_pcam = value;
    }
    pub fn host_set_tween_skip(&mut self, value: bool) {
        self.tween_skip = value;
    }
    pub fn emit_became_active(&mut self) {
        self.signals().became_active().emit();
    }
    pub fn emit_became_inactive(&mut self) {
        self.signals().became_inactive().emit();
    }
    pub fn emit_tween_started(&mut self) {
        self.signals().tween_started().emit();
    }
    pub fn emit_is_tweening(&mut self) {
        self.signals().is_tweening().emit();
    }
    pub fn emit_tween_completed(&mut self) {
        self.signals().tween_completed().emit();
    }
    pub fn emit_tween_interrupted(&mut self, pcam: &Gd<PhantomCamera3D>) {
        self.signals().tween_interrupted().emit(pcam);
    }
    pub fn camera_resource(&self) -> Option<Gd<Camera3DResource>> {
        self.camera_3d_resource.clone()
    }
    pub fn attributes_resource(&self) -> Option<Gd<godot::classes::CameraAttributes>> {
        self.attributes.clone()
    }
    pub fn pcam_has_noise_resource(&self) -> bool {
        self.has_noise_resource
    }
    pub fn take_noise_output(&mut self) -> Option<Transform3D> {
        self.pending_noise.take()
    }
    pub fn pcam_noise_emitted(&mut self, emitter_noise_output: Transform3D, emitter_layer: i64) {
        if self.noise_emitter_layer & emitter_layer != 0 {
            self.signals().noise_emitted().emit(emitter_noise_output);
            self.pending_noise = Some(emitter_noise_output);
        }
    }
    fn enter_tree_impl(&mut self) {
        self.tween_skip = !self.tween_on_load;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_pcam_added(PcamKind::ThreeD, &this);
        self.set_priority_override(false);
        let callable = self.base().callable("pcam_check_visibility");
        if !self.base().is_connected("visibility_changed", &callable) {
            self.base_mut().connect("visibility_changed", &callable);
        }
        match self.follow_mode {
            0 => self.is_parents_physics(None),
            4 => {
                if self.valid_follow_path().is_some() {
                    self.should_follow_checker();
                } else {
                    self.should_follow = false;
                }
            }
            3 => {
                self.follow_targets_size_check();
                self.should_follow_checker();
            }
            _ => self.should_follow_checker(),
        }
        self.should_look_at_checker();
        if self.look_at_mode == 3 {
            self.look_at_targets_size_check();
        }
        if matches!(self.follow_mode, 1 | 2 | 5 | 6)
            && let Some(target) = self.follow_target_node()
        {
            self.check_physics_body(&target);
        }
        if matches!(self.look_at_mode, 1 | 2)
            && let Some(target) = self.look_at_target_node()
        {
            self.check_physics_body(&target);
        }
    }
    fn exit_tree_impl(&mut self) {
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_pcam_removed(PcamKind::ThreeD, &this);
    }
    fn ready_impl(&mut self) {
        match self.follow_mode {
            6 => {
                self.is_third_person_follow = true;
                if self.should_follow {
                    self.transform_output.origin = self.get_target_position_offset_distance();
                }
                if !Engine::singleton().is_editor_hint() {
                    self.bootstrap_spring_arm();
                }
            }
            _ => self.transform_output.origin = self.base().get_global_position(),
        }
        if !Engine::singleton().is_editor_hint() {
            self._preview_noise = true;
        }
        self.transform_output = self.base().get_global_transform();
        self.sync_noise_resource();
        self.sync_camera_resource();
        if let Some(lock) = self.pending_axis_lock.take() {
            self.apply_axis_lock(lock);
        }
    }
    fn bootstrap_spring_arm(&mut self) {
        if self.follow_spring_arm.is_some() {
            return;
        }
        let mut arm = SpringArm3D::new_alloc();
        self.follow_spring_arm = Some(arm.clone());
        arm.set("top_level", &true.to_variant());
        arm.set_length(real_of(self.follow_distance));
        arm.set_collision_mask(self.collision_mask as u32);
        if let Some(shape) = self.shape.clone() {
            arm.set_shape(&shape);
        }
        arm.set_margin(real_of(self.margin));
        let initial_rotation = self.base().get_global_rotation();
        if self.should_follow
            && let Some(target) = self.follow_target_node()
        {
            self.exclude_object_from_arm(&mut arm, &target);
        }
        let callable = self.base().callable("pcam_spring_arm_ready");
        arm.connect("ready", &callable);
        match self.base().get_parent() {
            Some(mut parent) => {
                parent.call_deferred("add_child", &[arm.to_variant()]);
            }
            None => {
                godot_warn!("PhantomCamera3D: THIRD_PERSON needs a parent to host the SpringArm3D");
                arm.free();
                return;
            }
        }
        self.pending_spring_arm = Some((arm, initial_rotation));
    }
    fn exclude_object_from_arm(&self, arm: &mut Gd<SpringArm3D>, target: &Gd<Node3D>) {
        let mut object = target.clone().upcast::<godot::classes::Object>();
        if !object.has_method("get_rid") {
            return;
        }
        if let Ok(rid) = object.call("get_rid", &[]).try_to::<Rid>() {
            arm.add_excluded_object(rid);
        }
    }
    fn finish_spring_arm(&mut self) {
        let Some((mut arm, initial_rotation)) = self.pending_spring_arm.take() else {
            return;
        };
        let mut this = self.base().clone().upcast::<Node>();
        this.call_deferred("reparent", &[arm.clone().upcast::<Node3D>().to_variant()]);
        self.camera_target = Some(arm.clone().upcast::<Node3D>());
        let position = match self.follow_target_node() {
            Some(_) => self.target_position_offset(),
            None => self.base().get_global_position(),
        };
        arm.set_global_position(position);
        arm.set_global_rotation(initial_rotation);
        self.has_follow_spring_arm = true;
        self.follow_spring_arm = Some(arm);
        self.base_mut().set("top_level", &false.to_variant());
    }
    fn process_impl(&mut self, delta: f64) {
        if self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }
    fn physics_process_impl(&mut self, delta: f64) {
        if !self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }
    #[func]
    pub fn process_logic(&mut self, delta: f64) {
        if self.is_active_pcam {
            if self.has_noise_resource && self._preview_noise {
                self.transform_noise = self.noise_transform(delta);
            }
        } else if self.inactive_update_mode == 1 {
            return;
        }
        if self.should_follow {
            self.follow(delta);
        } else {
            self.transform_output.origin = self.base().get_global_position();
        }
        if self.should_look_at {
            self.look_at(delta);
        } else if !self.is_third_person_follow {
            self.transform_output.basis = self.base().get_global_basis();
        }
        if self.follow_axis_is_locked() {
            let lock = self.follow_axis_lock_value;
            match self.follow_axis_lock {
                1 => self.transform_output.origin.x = lock.x,
                2 => self.transform_output.origin.y = lock.y,
                3 => self.transform_output.origin.z = lock.z,
                4 => {
                    self.transform_output.origin.x = lock.x;
                    self.transform_output.origin.y = lock.y;
                }
                5 => {
                    self.transform_output.origin.x = lock.x;
                    self.transform_output.origin.z = lock.z;
                }
                6 => {
                    self.transform_output.origin.y = lock.y;
                    self.transform_output.origin.z = lock.z;
                }
                7 => {
                    self.transform_output.origin.x = lock.x;
                    self.transform_output.origin.y = lock.y;
                    self.transform_output.origin.z = lock.z;
                }
                _ => {}
            }
        }
    }
    fn follow(&mut self, delta: f64) {
        self.set_follow_position(delta);
        self.interpolate_position(delta);
    }
    fn look_at(&mut self, delta: f64) {
        self.set_look_at_position();
        self.interpolate_rotation(delta);
    }
    fn set_follow_position(&mut self, _delta: f64) {
        let editor = Engine::singleton().is_editor_hint();
        match self.follow_mode {
            1 => {
                if let Some(target) = self.follow_target_node() {
                    self.follow_target_output_position = target.get_global_position();
                }
            }
            2 => {
                self.follow_target_output_position = self.target_position_offset();
            }
            3 => self.set_follow_position_group(),
            4 => {
                if let Some(path) = self.valid_follow_path() {
                    let path_position = path.get_global_position();
                    let basis = path.get_global_basis();
                    let local = basis.inverse() * (self.target_position_offset() - path_position);
                    let closest: Vector3 = match path.get_curve() {
                        Some(curve) => curve.get_closest_point(local),
                        None => local,
                    };
                    self.follow_target_output_position = basis * closest + path_position;
                }
            }
            5 => self.set_follow_position_framed(editor),
            6 => {
                if !editor {
                    if !self.has_follow_spring_arm {
                        return;
                    }
                    self.follow_target_output_position = self.target_position_offset();
                } else {
                    self.follow_target_output_position =
                        self.get_target_position_offset_distance_direction();
                }
            }
            _ => {}
        }
    }
    fn set_follow_position_group(&mut self) {
        if self.has_multiple_follow_targets {
            let mut bounds = match self.follow_targets.front() {
                Some(first) => {
                    godot::builtin::Aabb::new(first.get_global_position(), Vector3::ZERO)
                }
                None => godot::builtin::Aabb::new(Vector3::ZERO, Vector3::ZERO),
            };
            for target in self.follow_targets.iter_shared() {
                if target.is_instance_valid() {
                    bounds = bounds.expand(target.get_global_position());
                }
            }
            let distance = if self.auto_follow_distance {
                let divisor = self.auto_follow_distance_divisor;
                let length = f64_of(bounds.longest_axis_size());
                let raw = self.auto_follow_distance_min
                    + (self.auto_follow_distance_max - self.auto_follow_distance_min)
                        * (if divisor == 0.0 {
                            0.0
                        } else {
                            length / divisor
                        });
                clampf(
                    raw,
                    self.auto_follow_distance_min,
                    self.auto_follow_distance_max,
                )
            } else {
                self.follow_distance
            };
            self.follow_target_output_position = bounds.center()
                + self.follow_offset
                + self.base().get_global_basis().col_c() * real_of(distance);
        } else {
            let index = self.follow_targets_single_target_index.max(0) as usize;
            if let Some(target) = self.follow_targets.get(index)
                && target.is_instance_valid()
            {
                self.follow_target_output_position = target.get_global_position()
                    + self.follow_offset
                    + self.base().get_global_basis().col_c()
                        * real_of(self.auto_follow_distance_min);
            }
        }
    }
    fn set_follow_position_framed(&mut self, editor: bool) {
        if !editor {
            if !self.is_active_pcam {
                self.follow_target_output_position = self.get_target_position_offset_distance();
                return;
            }
            let camera_position = self.get_raw_unprojected_position();
            let visible_rect_size = match self.base().get_viewport() {
                Some(viewport) => viewport.get_visible_rect().size,
                None => Vector2::ONE,
            };
            self.viewport_position = Vector2::new(
                if visible_rect_size.x == 0.0 {
                    0.0
                } else {
                    camera_position.x / visible_rect_size.x
                },
                if visible_rect_size.y == 0.0 {
                    0.0
                } else {
                    camera_position.y / visible_rect_size.y
                },
            );
            self.current_rotation = self.base().get_global_rotation();
            if self.current_rotation != self.base().get_global_rotation() {
                self.follow_target_output_position = self.get_target_position_offset_distance();
            }
            let side_offset = self.get_framed_side_offset();
            if side_offset != Vector2::ZERO {
                let mut target_position = self.target_position_offset() + self.follow_framed_offset;
                if self.dead_zone_width == 0.0 || self.dead_zone_height == 0.0 {
                    if self.dead_zone_width == 0.0 && self.dead_zone_height != 0.0 {
                        let mut global_pos = self.get_target_position_offset_distance();
                        global_pos.z = target_position.z;
                        self.follow_target_output_position = global_pos;
                    } else if self.dead_zone_width != 0.0 && self.dead_zone_height == 0.0 {
                        let mut global_pos = self.get_target_position_offset_distance();
                        global_pos.x = target_position.x;
                        self.follow_target_output_position = global_pos;
                    } else {
                        self.follow_target_output_position =
                            self.get_target_position_offset_distance();
                    }
                } else {
                    if self.current_rotation != self.base().get_global_rotation() {
                        let offset = self.target_position_offset();
                        let opposite: real =
                            real_of(-f64_of(self.base().get_global_rotation().x).sin())
                                * real_of(self.follow_distance)
                                + offset.y;
                        let mut global_pos = Vector3::ZERO;
                        global_pos.y = offset.y + opposite;
                        let inner = f64_of(real_of(self.follow_distance)).powi(2)
                            - f64_of(opposite).powi(2);
                        global_pos.z = real_of(inner.max(0.0).sqrt()) + offset.z;
                        global_pos.x = self.base().get_global_position().x;
                        self.follow_target_output_position = global_pos;
                        self.current_rotation = self.base().get_global_rotation();
                    } else {
                        self.signals().dead_zone_reached().emit();
                        let current_global_position = self.base().get_global_position();
                        let current_offset =
                            current_global_position - self.target_position_offset();
                        if side_offset.x == 0.0 {
                            self.follow_framed_offset.x = current_offset.x;
                        }
                        if side_offset.y == 0.0 {
                            self.follow_framed_offset.z = current_offset.z;
                        }
                        if side_offset.x == 0.0 {
                            target_position.x = current_global_position.x;
                        }
                        if side_offset.y == 0.0 {
                            target_position.z = current_global_position.z;
                        }
                        self.follow_target_output_position = target_position;
                    }
                }
            } else {
                self.follow_framed_offset =
                    self.base().get_global_position() - self.target_position_offset();
                self.follow_target_position = self.base().get_global_position();
                self.current_rotation = self.base().get_global_rotation();
            }
        } else {
            self.follow_target_output_position = self.get_target_position_offset_distance();
        }
    }
    fn set_look_at_position(&mut self) {
        match self.look_at_mode {
            1 => {
                if let Some(target) = self.look_at_target_node() {
                    self.look_at_target_output_position =
                        self.base().get_global_position() - target.get_global_basis().col_c();
                }
            }
            2 => {
                if let Some(target) = self.look_at_target_node() {
                    self.look_at_target_output_position = target.get_global_position();
                }
            }
            3 => {
                let targets: Vec<Gd<Node3D>> = self
                    .look_at_targets
                    .iter_shared()
                    .filter(|target| target.is_instance_valid())
                    .collect();
                if !self.has_multiple_look_at_targets {
                    let index = self.look_at_targets_single_target_index.max(0) as usize;
                    if let Some(target) = targets.get(index) {
                        self.look_at_target_output_position = target.get_global_position();
                    }
                } else {
                    let mut bounds = match targets.first() {
                        Some(first) => {
                            godot::builtin::Aabb::new(first.get_global_position(), Vector3::ZERO)
                        }
                        None => godot::builtin::Aabb::new(Vector3::ZERO, Vector3::ZERO),
                    };
                    for target in &targets {
                        bounds = bounds.expand(target.get_global_position());
                    }
                    self.look_at_target_output_position = bounds.center();
                }
            }
            _ => {}
        }
        self.look_at_target_output_position += self.look_at_offset;
    }
    fn target_position_offset(&self) -> Vector3 {
        match self.follow_target_node() {
            Some(target) => target.get_global_position() + self.follow_offset,
            None => self.follow_offset,
        }
    }
    fn get_target_position_offset_distance(&self) -> Vector3 {
        self.target_position_offset()
            + self.base().get_transform().basis.col_c() * real_of(self.follow_distance)
    }
    fn get_target_position_offset_distance_direction(&self) -> Vector3 {
        let Some(target) = self.follow_target_node() else {
            return self.target_position_offset();
        };
        let basis = target.get_global_basis();
        let vertical =
            Quaternion::from_axis_angle(basis.col_a(), real_of(self.vertical_rotation_offset));
        let horizontal =
            Quaternion::from_axis_angle(basis.col_b(), real_of(self.horizontal_rotation_offset));
        let rotated = Basis::from_quaternion(horizontal) * Basis::from_quaternion(vertical);
        self.target_position_offset() + rotated * (basis.col_c() * real_of(self.follow_distance))
    }
    fn interpolate_position(&mut self, delta: f64) {
        let editor = Engine::singleton().is_editor_hint();
        if self.follow_damping && !editor {
            if !self.is_third_person_follow {
                let target_position = self.follow_target_output_position;
                if !self
                    .base()
                    .get_global_position()
                    .approx_eq(&target_position)
                {
                    self.base_mut().set_global_position(target_position);
                }
                for i in 0..3 {
                    let (output, velocity) = crate::math::smooth_damp(
                        f64_of(axis3(&self.base().get_global_position(), i)),
                        f64_of(axis3(&self.transform_output.origin, i)),
                        f64_of(axis3(&self.follow_velocity_ref, i)),
                        f64_of(axis3(&self.follow_damping_value, i)),
                        delta,
                    );
                    set_axis3(&mut self.follow_velocity_ref, i, real_of(velocity));
                    set_axis3(&mut self.transform_output.origin, i, real_of(output));
                }
            } else {
                let mut target_position = self.camera_target_global_position();
                for i in 0..3 {
                    let (output, velocity) = crate::math::smooth_damp(
                        f64_of(axis3(&self.follow_target_output_position, i)),
                        f64_of(axis3(&target_position, i)),
                        f64_of(axis3(&self.follow_velocity_ref, i)),
                        f64_of(axis3(&self.follow_damping_value, i)),
                        delta,
                    );
                    set_axis3(&mut self.follow_velocity_ref, i, real_of(velocity));
                    set_axis3(&mut target_position, i, real_of(output));
                }
                self.set_camera_target_global_position(target_position);
                self.transform_output.origin = self.base().get_global_position();
            }
        } else {
            self.set_camera_target_global_position(self.follow_target_output_position);
            if self.base().is_inside_tree() {
                self.transform_output.origin = self.base().get_global_position();
            }
        }
        if self.is_third_person_follow && editor {
            if let Some(target) = self.follow_target_node() {
                let up = target.get_global_basis().col_b();
                let target_quat = self.look_at_target_quat(self.target_position_offset(), up);
                let target_basis = Basis::from_quaternion(target_quat);
                self.transform_output.basis = target_basis;
                if !self.base().get_global_basis().approx_eq(&target_basis) {
                    self.base_mut().set_global_basis(target_basis);
                }
            }
        } else if self.has_follow_spring_arm
            && let Some(arm) = self.follow_spring_arm.clone()
        {
            let basis = arm.get_global_basis();
            self.transform_output.basis = basis;
            if !self.base().get_global_basis().approx_eq(&basis) {
                self.base_mut().set_global_basis(basis);
            }
        }
    }
    fn look_at_target_quat(
        &mut self,
        target_position: Vector3,
        up_direction: Vector3,
    ) -> Quaternion {
        let global_position = self.base().get_global_position();
        let direction = -(target_position - global_position).normalized();
        let basis_z = direction.normalized();
        let basis_x = up_direction.cross(basis_z);
        let basis_y = basis_z.cross(basis_x.normalized());
        let target_basis = Basis::from_cols(basis_x, basis_y, basis_z);
        if target_basis.determinant() == 0.0 {
            let mut degrees = self.base().get_global_rotation_degrees();
            if target_basis.col_c() == Vector3::UP {
                degrees.x = -90.0;
            } else {
                degrees.x = 90.0;
            }
            if !self
                .base()
                .get_global_rotation_degrees()
                .approx_eq(&degrees)
            {
                self.base_mut().set_global_rotation_degrees(degrees);
            }
            self.transform_output.basis = self.base().get_global_basis();
            return self.base().get_quaternion();
        }
        target_basis.get_quaternion().normalized()
    }
    fn interpolate_rotation(&mut self, delta: f64) {
        if self.has_up_target
            && let Some(target) = self.up_target_node()
        {
            self.up_direction = target.get_global_basis().col_b();
        }
        let mut target_quat =
            self.look_at_target_quat(self.look_at_target_output_position, self.up_direction);
        if self.look_at_damping {
            let current_quat = self.base().get_quaternion().normalized();
            let damping_time = self.look_at_damping_value.max(0.0005);
            let t = (delta / damping_time).min(1.0);
            let mut dot = f64_of(
                current_quat.x * target_quat.x
                    + current_quat.y * target_quat.y
                    + current_quat.z * target_quat.z
                    + current_quat.w * target_quat.w,
            );
            if dot < 0.0 {
                target_quat = Quaternion::new(
                    -target_quat.x,
                    -target_quat.y,
                    -target_quat.z,
                    -target_quat.w,
                );
                dot = -dot;
            }
            let dot = clampf(dot, -1.0, 1.0);
            let theta = dot.acos() * t;
            let sin_theta = theta.sin();
            let sin_theta_total = dot.acos().sin();
            if is_zero_approx(sin_theta_total) {
                return;
            }
            let ratio_a = theta.cos() - dot * sin_theta / sin_theta_total;
            let ratio_b = sin_theta / sin_theta_total;
            let output = current_quat * real_of(ratio_a) + target_quat * real_of(ratio_b);
            self.transform_output.basis = Basis::from_quaternion(output);
            if !self.base().get_quaternion().approx_eq(&output) {
                self.base_mut().set_quaternion(output);
            }
        } else {
            self.transform_output.basis = Basis::from_quaternion(target_quat);
            if !self.base().get_quaternion().approx_eq(&target_quat) {
                self.base_mut().set_quaternion(target_quat);
            }
        }
    }
    fn get_framed_side_offset(&self) -> Vector2 {
        let mut frame_out_bounds = Vector2::ZERO;
        let position = self.viewport_position;
        if position.x < 0.5 - real_of(self.dead_zone_width) / 2.0 {
            frame_out_bounds.x = -1.0;
        }
        if position.y < 0.5 - real_of(self.dead_zone_height) / 2.0 {
            frame_out_bounds.y = 1.0;
        }
        if position.x > 0.5 + real_of(self.dead_zone_width) / 2.0 {
            frame_out_bounds.x = 1.0;
        }
        if position.y > 0.5005 + real_of(self.dead_zone_height) / 2.0 {
            frame_out_bounds.y = -1.0;
        }
        frame_out_bounds
    }
    fn get_raw_unprojected_position(&self) -> Vector2 {
        match self.viewport_camera() {
            Some(camera) => camera.unproject_position(self.target_position_offset()),
            None => Vector2::ZERO,
        }
    }
    fn viewport_camera(&self) -> Option<Gd<godot::classes::Camera3D>> {
        self.base()
            .get_viewport()
            .and_then(|viewport| viewport.get_camera_3d())
    }
    fn check_visibility(&mut self) {
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_visibility_changed(&this);
    }
    fn follow_target_tree_exiting_impl(&mut self, target: &Gd<Node>) {
        if let Some(current) = self.follow_target_node()
            && current.instance_id() == target.instance_id()
        {
            self.should_follow = false;
        }
    }
    fn should_follow_checker(&mut self) {
        if self.follow_mode == 0 {
            self.should_follow = false;
            return;
        }
        if self.follow_mode != 3 {
            self.should_follow = self.follow_target_node().is_some();
        }
    }
    fn follow_targets_size_check(&mut self) {
        let mut targets_size = 0i64;
        self.follow_target_physics_based = false;
        let targets: Vec<Gd<Node3D>> = self.follow_targets.iter_shared().collect();
        for (i, target) in targets.iter().enumerate() {
            if !target.is_instance_valid() {
                continue;
            }
            targets_size += 1;
            self.follow_targets_single_target_index = i as i64;
            self.check_physics_body(target);
            self.connect_tree_exiting(target);
        }
        match targets_size {
            0 => {
                self.should_follow = false;
                self.has_multiple_follow_targets = false;
            }
            1 => {
                self.should_follow = true;
                self.has_multiple_follow_targets = false;
            }
            _ => {
                self.should_follow = true;
                self.has_multiple_follow_targets = true;
            }
        }
    }
    fn look_at_target_tree_exiting_impl(&mut self, target: &Gd<Node>) {
        if let Some(current) = self.look_at_target_node()
            && current.instance_id() == target.instance_id()
        {
            self.should_look_at = false;
        }
        let mut index = None;
        for (i, item) in self.look_at_targets.iter_shared().enumerate() {
            if item.instance_id() == target.instance_id() {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            self.look_at_targets.remove(index);
            self.look_at_targets_size_check();
        }
    }
    fn up_target_tree_exiting_impl(&mut self) {
        self.set_up_target(None);
    }
    fn should_look_at_checker(&mut self) {
        if self.look_at_mode == 0 {
            self.should_look_at = false;
            return;
        }
        if self.look_at_mode != 3 {
            self.should_look_at = self.look_at_target_node().is_some();
        }
    }
    fn look_at_targets_size_check(&mut self) {
        let mut targets_size = 0i64;
        let targets: Vec<Gd<Node3D>> = self.look_at_targets.iter_shared().collect();
        for (i, target) in targets.iter().enumerate() {
            if !target.is_instance_valid() {
                continue;
            }
            targets_size += 1;
            self.look_at_targets_single_target_index = i as i64;
            self.check_physics_body(target);
            self.connect_look_at_tree_exiting(target);
        }
        match targets_size {
            0 => {
                self.should_look_at = false;
                self.has_multiple_look_at_targets = false;
            }
            1 => {
                self.should_look_at = true;
                self.has_multiple_look_at_targets = false;
            }
            _ => {
                self.should_look_at = true;
                self.has_multiple_look_at_targets = true;
            }
        }
    }
    fn check_physics_body(&mut self, target: &Gd<Node3D>) {
        if target
            .clone()
            .try_cast::<godot::classes::PhysicsBody3D>()
            .is_ok()
        {
            self.follow_target_physics_based = true;
        } else {
            let this = self.base().clone().upcast::<Node>();
            self.is_parents_physics(Some(&this));
        }
        self.signals().physics_target_changed().emit();
    }
    fn is_parents_physics(&mut self, target: Option<&Gd<Node>>) {
        let mut current = match target {
            Some(node) => Some(node.clone()),
            None => Some(self.base().clone().upcast::<Node>()),
        };
        loop {
            let Some(node) = current else {
                break;
            };
            let parent = node.get_parent();
            if let Some(parent) = &parent
                && parent
                    .clone()
                    .try_cast::<godot::classes::PhysicsBody3D>()
                    .is_ok()
            {
                self.follow_target_physics_based = true;
            }
            current = parent;
        }
    }
    fn camera_resource_changed_impl(&mut self) {
        self.signals().camera_3d_resource_changed().emit();
    }
    fn check_draw_gizmo(&mut self) {
        self.draw_gizmo = self.draw_follow_gizmo_line || self.draw_look_at_gizmo_line;
    }
    fn apply_axis_lock(&mut self, value: i64) {
        let editor = Engine::singleton().is_editor_hint();
        if value != 0 && !editor {
            self.follow_axis_is_locked = true;
            let origin = self.transform_output.origin;
            match value {
                1 => self.follow_axis_lock_value.x = origin.x,
                2 => self.follow_axis_lock_value.y = origin.y,
                3 => self.follow_axis_lock_value.z = origin.z,
                4 => {
                    self.follow_axis_lock_value.x = origin.x;
                    self.follow_axis_lock_value.y = origin.y;
                }
                5 => {
                    self.follow_axis_lock_value.x = origin.x;
                    self.follow_axis_lock_value.z = origin.z;
                }
                6 => {
                    self.follow_axis_lock_value.y = origin.y;
                    self.follow_axis_lock_value.z = origin.z;
                }
                7 => {
                    self.follow_axis_lock_value.x = origin.x;
                    self.follow_axis_lock_value.y = origin.y;
                    self.follow_axis_lock_value.z = origin.z;
                }
                _ => {}
            }
        } else {
            self.follow_axis_is_locked = false;
        }
    }
    fn follow_axis_is_locked(&self) -> bool {
        self.follow_axis_is_locked
    }
    fn follow_target_node(&self) -> Option<Gd<Node3D>> {
        self.follow_target
            .clone()
            .filter(|target| target.is_instance_valid())
    }
    fn look_at_target_node(&self) -> Option<Gd<Node3D>> {
        self.look_at_target
            .clone()
            .filter(|target| target.is_instance_valid())
    }
    fn up_target_node(&self) -> Option<Gd<Node3D>> {
        self.up_target
            .clone()
            .filter(|target| target.is_instance_valid())
    }
    fn valid_follow_path(&self) -> Option<Gd<Path3D>> {
        self.follow_path
            .clone()
            .filter(|path| path.is_instance_valid())
    }
    fn connect_tree_exiting(&mut self, target: &Gd<Node3D>) {
        let node = target.clone().upcast::<Node>();
        let callable = self
            .base()
            .callable("pcam_follow_target_tree_exiting")
            .bindv(&Array::from(&[node.to_variant()]));
        let mut node = node;
        if !node.is_connected("tree_exiting", &callable) {
            node.connect("tree_exiting", &callable);
        }
    }
    fn connect_look_at_tree_exiting(&mut self, target: &Gd<Node3D>) {
        let node = target.clone().upcast::<Node>();
        let callable = self
            .base()
            .callable("pcam_look_at_target_tree_exiting")
            .bindv(&Array::from(&[node.to_variant()]));
        let mut node = node;
        if !node.is_connected("tree_exiting", &callable) {
            node.connect("tree_exiting", &callable);
        }
    }
    fn camera_target_global_position(&self) -> Vector3 {
        match &self.camera_target {
            Some(target) => target.get_global_position(),
            None => self.base().get_global_position(),
        }
    }
    fn set_camera_target_global_position(&mut self, value: Vector3) {
        match self.camera_target.clone() {
            Some(mut target) => target.set_global_position(value),
            None => self.base_mut().set_global_position(value),
        }
    }
    fn sync_noise_resource(&mut self) {
        if self.noise.is_some() {
            self.has_noise_resource = true;
            if let Some(mut noise) = self.noise.clone() {
                noise.bind_mut().set_trauma(1.0);
            }
        } else {
            self.has_noise_resource = false;
            self.transform_noise = Transform3D::IDENTITY;
        }
    }
    fn noise_transform(&mut self, delta: f64) -> Transform3D {
        match self.noise.clone() {
            Some(mut noise) => noise.bind_mut().get_noise_transform(delta),
            None => Transform3D::IDENTITY,
        }
    }
    fn sync_camera_resource(&mut self) {
        if let Some(resource) = self.camera_3d_resource.clone() {
            let callable = self.base().callable("pcam_camera_resource_changed");
            if !resource.is_connected("changed", &callable) {
                let mut object = resource.upcast::<godot::classes::Object>();
                object.connect("changed", &callable);
            }
        }
    }
    #[func]
    pub fn get_transform_output(&self) -> Transform3D {
        self.transform_output
    }
    #[func]
    pub fn get_noise_transform(&self) -> Transform3D {
        self.transform_noise
    }
    #[func]
    pub fn emit_noise(&mut self, value: Transform3D) {
        self.signals().noise_emitted().emit(value);
        self.pending_noise = Some(value);
    }
    #[func]
    pub fn teleport_position(&mut self) {
        self.follow_velocity_ref = Vector3::ZERO;
        self.set_follow_position(0.0);
        self.transform_output.origin = self.follow_target_output_position;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_teleport(&this);
    }
    #[func]
    pub fn is_following(&self) -> bool {
        self.should_follow
    }
    #[func]
    pub fn is_looking(&self) -> bool {
        self.should_look_at
    }
    #[func]
    pub fn get_follow_target_position(&self) -> Vector3 {
        self.follow_target_position
    }
    #[func]
    pub fn get_look_at_target_position(&self) -> Vector3 {
        self.look_at_target_output_position
    }
    #[func]
    pub fn set_tween_skip(&mut self, caller: Gd<Node>, value: bool) {
        if caller.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.tween_skip = value;
        } else {
            godot_warn!("PhantomCamera3D: set_tween_skip can only be called by PhantomCameraHost");
        }
    }
    #[func]
    pub fn get_tween_skip(&self) -> bool {
        self.tween_skip
    }
    #[func]
    pub fn set_priority(&mut self, value: i64) {
        self.priority = value.max(0);
        if !self.base().is_node_ready() {
            return;
        }
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_priority_changed(&this);
    }
    #[func]
    pub fn get_priority(&self) -> i64 {
        self.priority
    }
    #[func]
    pub fn set_priority_override(&mut self, value: bool) {
        self.priority_override = value;
        if Engine::singleton().is_editor_hint() {
            let this = self.base().clone().upcast::<Node>();
            PhantomCameraManager::notify_priority_override(&this, value);
        }
    }
    #[func]
    pub fn get_priority_override(&self) -> bool {
        self.priority_override
    }
    #[func]
    pub fn set_tween_resource(&mut self, value: Option<Gd<PhantomCameraTween>>) {
        self.tween_resource = value;
    }
    #[func]
    pub fn get_tween_resource(&self) -> Option<Gd<PhantomCameraTween>> {
        self.tween_resource.clone()
    }
    #[func]
    pub fn set_tween_duration(&mut self, value: f64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().duration = value,
            None => {
                godot_warn!("PhantomCamera3D: no tween_resource assigned; duration not stored");
                self.tween_duration_fallback = value;
            }
        }
    }
    #[func]
    pub fn get_tween_duration(&self) -> f64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().duration,
            None => {
                if self.tween_duration_fallback > 0.0 {
                    self.tween_duration_fallback
                } else {
                    TWEEN_DEFAULT_DURATION
                }
            }
        }
    }
    #[func]
    pub fn set_tween_transition(&mut self, value: i64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().transition = value,
            None => {
                godot_warn!("PhantomCamera3D: no tween_resource assigned; transition not stored");
                self.tween_transition_fallback = value;
            }
        }
    }
    #[func]
    pub fn get_tween_transition(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().transition,
            None => self.tween_transition_fallback,
        }
    }
    #[func]
    pub fn set_tween_ease(&mut self, value: i64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().ease = value,
            None => {
                godot_warn!("PhantomCamera3D: no tween_resource assigned; ease not stored");
                self.tween_ease_fallback = value;
            }
        }
    }
    #[func]
    pub fn get_tween_ease(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().ease,
            None => self.tween_ease_fallback,
        }
    }
    #[func]
    pub fn set_is_active(&mut self, node: Gd<Node>, value: bool) {
        if node.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.is_active_pcam = value;
        } else {
            godot_warn!("PhantomCamera3D: PCams can only be set from the PhantomCameraHost");
        }
    }
    #[func]
    pub fn is_active(&self) -> bool {
        self.is_active_pcam
    }
    #[func]
    pub fn set_tween_on_load(&mut self, value: bool) {
        self.tween_on_load = value;
    }
    #[func]
    pub fn get_tween_on_load(&self) -> bool {
        self.tween_on_load
    }
    #[func]
    pub fn set_host_layers(&mut self, value: i64) {
        self.host_layers = value;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_host_layer_changed(&this);
    }
    #[func]
    pub fn set_host_layers_value(&mut self, layer: i64, value: bool) {
        self.host_layers = set_layer(self.host_layers, layer, value);
    }
    #[func]
    pub fn get_host_layers(&self) -> i64 {
        self.host_layers
    }
    #[func]
    pub fn get_follow_mode(&self) -> i64 {
        self.follow_mode
    }
    #[func]
    pub fn set_follow_mode(&mut self, value: i64) {
        self.follow_mode = value;
        self.signals().follow_mode_changed().emit();
        if value == 0 {
            self.should_follow = false;
            self.base_mut().set("top_level", &false.to_variant());
            self.draw_follow_gizmo_line = false;
            self.check_draw_gizmo();
            self.is_parents_physics(None);
            return;
        }
        match value {
            4 => {
                if self.valid_follow_path().is_some() {
                    self.should_follow_checker();
                } else {
                    self.should_follow = false;
                }
            }
            3 => self.follow_targets_size_check(),
            _ => self.should_follow_checker(),
        }
        if value == 1 {
            self.draw_follow_gizmo_line = false;
            self.check_draw_gizmo();
        } else {
            self.draw_follow_gizmo_line = self.draw_follow_line;
            self.check_draw_gizmo();
        }
        if value == 5 && self.follow_framed_initial_set && self.follow_target_node().is_some() {
            self.follow_framed_initial_set = false;
        }
        if value == 6 {
            self.base_mut().set("top_level", &false.to_variant());
            self.is_third_person_follow = true;
        } else {
            self.base_mut().set("top_level", &true.to_variant());
            self.is_third_person_follow = false;
        }
        if self.look_at_mode != 0 {
            godot_print!(
                "Warning: Using both Look At and Follow Mode on the same PCam3D has not been fully tested yet, proceed with caution!"
            );
        }
    }
    #[func]
    pub fn set_follow_target(&mut self, value: Option<Gd<Node3D>>) {
        if self.follow_mode == 0 || self.follow_mode == 3 {
            return;
        }
        let new_target = value.filter(|target| target.is_instance_valid());
        if let (Some(current), Some(new)) = (self.follow_target_node(), &new_target)
            && current.instance_id() == new.instance_id()
        {
            return;
        }
        self.follow_target = new_target.clone();
        self.follow_target_physics_based = false;
        match &new_target {
            Some(target) => {
                if self.follow_mode == 4 {
                    self.should_follow = self.valid_follow_path().is_some();
                } else {
                    self.should_follow = true;
                }
                self.check_physics_body(target);
                if self.follow_mode == 6
                    && let Some(mut arm) = self.follow_spring_arm.clone()
                {
                    self.exclude_object_from_arm(&mut arm, target);
                }
                self.connect_tree_exiting(target);
            }
            None => {
                if self.follow_mode != 3 {
                    self.should_follow = false;
                }
            }
        }
        self.signals().follow_target_changed().emit();
    }
    #[func]
    pub fn erase_follow_target(&mut self) {
        self.follow_target = None;
    }
    #[func]
    pub fn get_follow_target(&self) -> Option<Gd<Node3D>> {
        self.follow_target_node()
    }
    #[func]
    pub fn set_follow_path(&mut self, value: Option<Gd<Path3D>>) {
        self.follow_path = value.filter(|path| path.is_instance_valid());
        if self.valid_follow_path().is_some() {
            self.should_follow_checker();
        } else {
            self.should_follow = false;
        }
    }
    #[func]
    pub fn erase_follow_path(&mut self) {
        self.follow_path = None;
    }
    #[func]
    pub fn get_follow_path(&self) -> Option<Gd<Path3D>> {
        self.valid_follow_path()
    }
    #[func]
    pub fn set_follow_targets(&mut self, value: Array<Gd<Node3D>>) {
        if self.follow_mode != 3 {
            return;
        }
        self.follow_targets = value;
        self.follow_targets_size_check();
    }
    #[func]
    pub fn append_follow_targets(&mut self, value: Gd<Node3D>) {
        if !value.is_instance_valid() {
            godot_warn!("PhantomCamera3D: value is not a valid Node3D instance");
            return;
        }
        if self
            .follow_targets
            .iter_shared()
            .any(|target| target.instance_id() == value.instance_id())
        {
            godot_warn!("PhantomCamera3D: value is already part of the Follow Group");
            return;
        }
        self.follow_targets.push(&value);
        self.follow_targets_size_check();
    }
    #[func]
    pub fn append_follow_targets_array(&mut self, value: Array<Gd<Node3D>>) {
        for target in value.iter_shared() {
            if !target.is_instance_valid() {
                continue;
            }
            if self
                .follow_targets
                .iter_shared()
                .any(|item| item.instance_id() == target.instance_id())
            {
                godot_warn!("PhantomCamera3D: value is already part of the Follow Group");
                continue;
            }
            self.follow_targets.push(&target);
            self.follow_targets_size_check();
        }
    }
    #[func]
    pub fn erase_follow_targets(&mut self, value: Gd<Node3D>) {
        let mut index = None;
        for (i, target) in self.follow_targets.iter_shared().enumerate() {
            if target.instance_id() == value.instance_id() {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            self.follow_targets.remove(index);
        }
        self.follow_targets_size_check();
    }
    #[func]
    pub fn get_follow_targets(&self) -> Array<Gd<Node3D>> {
        self.follow_targets.clone()
    }
    #[func]
    pub fn set_follow_offset(&mut self, value: Vector3) {
        let temp_offset = self.follow_offset;
        self.follow_offset = value;
        if self.follow_axis_lock != 0 {
            let _ = temp_offset - value;
        }
    }
    #[func]
    pub fn get_follow_offset(&self) -> Vector3 {
        self.follow_offset
    }
    #[func]
    pub fn set_follow_damping(&mut self, value: bool) {
        self.follow_damping = value;
    }
    #[func]
    pub fn get_follow_damping(&self) -> bool {
        self.follow_damping
    }
    #[func]
    pub fn set_follow_damping_value(&mut self, value: Vector3) {
        self.follow_damping_value =
            Vector3::new(value.x.max(0.0), value.y.max(0.0), value.z.max(0.0));
    }
    #[func]
    pub fn get_follow_damping_value(&self) -> Vector3 {
        self.follow_damping_value
    }
    #[func]
    pub fn set_follow_distance(&mut self, value: f64) {
        self.follow_distance = value;
    }
    #[func]
    pub fn get_follow_distance(&self) -> f64 {
        self.follow_distance
    }
    #[func]
    pub fn set_auto_follow_distance(&mut self, value: bool) {
        self.auto_follow_distance = value;
    }
    #[func]
    pub fn get_auto_follow_distance(&self) -> bool {
        self.auto_follow_distance
    }
    #[func]
    pub fn set_auto_follow_distance_min(&mut self, value: f64) {
        self.auto_follow_distance_min = value;
    }
    #[func]
    pub fn get_auto_follow_distance_min(&self) -> f64 {
        self.auto_follow_distance_min
    }
    #[func]
    pub fn set_auto_follow_distance_max(&mut self, value: f64) {
        self.auto_follow_distance_max = value;
    }
    #[func]
    pub fn get_auto_follow_distance_max(&self) -> f64 {
        self.auto_follow_distance_max
    }
    #[func]
    pub fn set_auto_follow_distance_divisor(&mut self, value: f64) {
        self.auto_follow_distance_divisor = value;
    }
    #[func]
    pub fn get_auto_follow_distance_divisor(&self) -> f64 {
        self.auto_follow_distance_divisor
    }
    #[func]
    pub fn set_third_person_rotation(&mut self, value: Vector3) {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return;
        }
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_rotation(value);
        }
    }
    #[func]
    pub fn get_third_person_rotation(&self) -> Vector3 {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return Vector3::ZERO;
        }
        match &self.follow_spring_arm {
            Some(arm) => arm.get_rotation(),
            None => Vector3::ZERO,
        }
    }
    #[func]
    pub fn set_third_person_rotation_degrees(&mut self, value: Vector3) {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return;
        }
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_rotation_degrees(value);
        }
    }
    #[func]
    pub fn get_third_person_rotation_degrees(&self) -> Vector3 {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return Vector3::ZERO;
        }
        match &self.follow_spring_arm {
            Some(arm) => arm.get_rotation_degrees(),
            None => Vector3::ZERO,
        }
    }
    #[func]
    pub fn set_third_person_quaternion(&mut self, value: Quaternion) {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return;
        }
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_quaternion(value);
        }
    }
    #[func]
    pub fn get_third_person_quaternion(&self) -> Quaternion {
        if !self.is_third_person_follow {
            godot_warn!("PhantomCamera3D: Follow Mode is not set to Third Person");
            return Quaternion::IDENTITY;
        }
        match &self.follow_spring_arm {
            Some(arm) => arm.get_quaternion(),
            None => Quaternion::IDENTITY,
        }
    }
    #[func]
    pub fn set_vertical_rotation_offset(&mut self, value: f64) {
        self.vertical_rotation_offset = value;
    }
    #[func]
    pub fn get_vertical_rotation_offset(&self) -> f64 {
        self.vertical_rotation_offset
    }
    #[func]
    pub fn set_horizontal_rotation_offset(&mut self, value: f64) {
        self.horizontal_rotation_offset = value;
    }
    #[func]
    pub fn get_horizontal_rotation_offset(&self) -> f64 {
        self.horizontal_rotation_offset
    }
    #[func]
    pub fn set_spring_length(&mut self, value: f64) {
        self.follow_distance = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_length(real_of(value));
        }
    }
    #[func]
    pub fn get_spring_length(&self) -> f64 {
        self.follow_distance
    }
    #[func]
    pub fn set_collision_mask(&mut self, value: i64) {
        self.collision_mask = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_collision_mask(value as u32);
        }
    }
    #[func]
    pub fn set_collision_mask_value(&mut self, layer: i64, enabled: bool) {
        self.collision_mask = set_layer(self.collision_mask, layer, enabled);
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_collision_mask(self.collision_mask as u32);
        }
    }
    #[func]
    pub fn get_collision_mask(&self) -> i64 {
        self.collision_mask
    }
    #[func]
    pub fn set_shape(&mut self, value: Option<Gd<Shape3D>>) {
        self.shape = value.clone();
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            match &value {
                Some(shape) => arm.set_shape(shape),
                None => arm.set("shape", &Variant::nil()),
            }
        }
    }
    #[func]
    pub fn get_shape(&self) -> Option<Gd<Shape3D>> {
        self.shape.clone()
    }
    #[func]
    pub fn set_margin(&mut self, value: f64) {
        self.margin = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_margin(real_of(value));
        }
    }
    #[func]
    pub fn get_margin(&self) -> f64 {
        self.margin
    }
    #[func]
    pub fn set_draw_follow_line(&mut self, value: bool) {
        self.draw_follow_line = value;
        self.draw_follow_gizmo_line = value;
        self.check_draw_gizmo();
    }
    #[func]
    pub fn get_draw_follow_line(&self) -> bool {
        self.draw_follow_line
    }
    #[func]
    pub fn set_draw_look_at_line(&mut self, value: bool) {
        self.draw_look_at_line = value;
        self.draw_look_at_gizmo_line = value;
        self.check_draw_gizmo();
    }
    #[func]
    pub fn get_draw_look_at_line(&self) -> bool {
        self.draw_look_at_line
    }
    #[func]
    pub fn get_look_at_mode(&self) -> i64 {
        self.look_at_mode
    }
    #[func]
    pub fn set_look_at_mode(&mut self, value: i64) {
        self.look_at_mode = value;
        if value == 0 {
            self.should_look_at = false;
            self.draw_look_at_gizmo_line = false;
            self.check_draw_gizmo();
            return;
        }
        if value == 1 {
            self.draw_look_at_gizmo_line = false;
            self.check_draw_gizmo();
        } else {
            self.draw_look_at_gizmo_line = self.draw_look_at_line;
            self.check_draw_gizmo();
        }
        if value != 3 {
            if self.look_at_target_node().is_some() {
                self.should_look_at = true;
            }
        } else {
            self.look_at_targets_size_check();
        }
        if self.follow_mode != 0 {
            godot_print!(
                "Warning: Using both Look At and Follow Mode on the same PCam3D has not been fully tested yet, proceed with caution!"
            );
        }
    }
    #[func]
    pub fn set_look_at_target(&mut self, value: Option<Gd<Node3D>>) {
        if self.look_at_mode == 0 {
            return;
        }
        let new_target = value.filter(|target| target.is_instance_valid());
        if let (Some(current), Some(new)) = (self.look_at_target_node(), &new_target)
            && current.instance_id() == new.instance_id()
        {
            return;
        }
        self.look_at_target = new_target.clone();
        if self.look_at_mode != 3 {
            match &new_target {
                Some(target) => {
                    self.should_look_at = true;
                    self.check_physics_body(target);
                    self.connect_look_at_tree_exiting(target);
                }
                None => self.should_look_at = false,
            }
        } else if self.look_at_targets.is_empty() {
            self.should_look_at = false;
        }
        self.signals().look_at_target_changed().emit();
    }
    #[func]
    pub fn get_look_at_target(&self) -> Option<Gd<Node3D>> {
        self.look_at_target_node()
    }
    #[func]
    pub fn set_look_at_targets(&mut self, value: Array<Gd<Node3D>>) {
        if self.look_at_mode != 3 {
            return;
        }
        self.look_at_targets = value;
        self.look_at_targets_size_check();
    }
    #[func]
    pub fn append_look_at_target(&mut self, value: Gd<Node3D>) {
        if !value.is_instance_valid() {
            godot_warn!("PhantomCamera3D: value is an invalid Node3D instance");
            return;
        }
        if self
            .look_at_targets
            .iter_shared()
            .any(|target| target.instance_id() == value.instance_id())
        {
            godot_warn!("PhantomCamera3D: value is already part of the Look At Group");
            return;
        }
        self.look_at_targets.push(&value);
        self.look_at_targets_size_check();
    }
    #[func]
    pub fn append_look_at_targets_array(&mut self, value: Array<Gd<Node3D>>) {
        for target in value.iter_shared() {
            if !target.is_instance_valid() {
                continue;
            }
            if self
                .look_at_targets
                .iter_shared()
                .any(|item| item.instance_id() == target.instance_id())
            {
                godot_warn!("PhantomCamera3D: value is already part of the Look At Group");
                continue;
            }
            self.look_at_targets.push(&target);
            self.look_at_targets_size_check();
        }
    }
    #[func]
    pub fn erase_look_at_targets(&mut self, value: Gd<Node3D>) {
        let mut index = None;
        for (i, target) in self.look_at_targets.iter_shared().enumerate() {
            if target.instance_id() == value.instance_id() {
                index = Some(i);
                break;
            }
        }
        if let Some(index) = index {
            self.look_at_targets.remove(index);
            self.look_at_targets_size_check();
        } else {
            godot_warn!("PhantomCamera3D: value is not part of the Look At Group");
        }
    }
    #[func]
    pub fn erase_look_at_targets_member(&mut self, value: Gd<Node3D>) {
        godot_warn!(
            "PhantomCamera3D: erase_look_at_targets_member is deprecated, use erase_look_at_targets instead"
        );
        self.erase_look_at_targets(value);
    }
    #[func]
    pub fn get_look_at_targets(&self) -> Array<Gd<Node3D>> {
        self.look_at_targets.clone()
    }
    #[func]
    pub fn set_look_at_offset(&mut self, value: Vector3) {
        self.look_at_offset = value;
    }
    #[func]
    pub fn get_look_at_offset(&self) -> Vector3 {
        self.look_at_offset
    }
    #[func]
    pub fn set_look_at_damping(&mut self, value: bool) {
        self.look_at_damping = value;
    }
    #[func]
    pub fn get_look_at_damping(&self) -> bool {
        self.look_at_damping
    }
    #[func]
    pub fn set_look_at_damping_value(&mut self, value: f64) {
        self.look_at_damping_value = value;
    }
    #[func]
    pub fn get_look_at_damping_value(&self) -> f64 {
        self.look_at_damping_value
    }
    #[func]
    pub fn set_follow_axis_lock(&mut self, value: i64) {
        self.follow_axis_lock = value;
        if !self.base().is_node_ready() {
            if value == 0 {
                self.follow_axis_is_locked = false;
            } else {
                self.pending_axis_lock = Some(value);
            }
            return;
        }
        self.apply_axis_lock(value);
    }
    #[func]
    pub fn get_follow_axis_lock(&self) -> i64 {
        self.follow_axis_lock
    }
    #[func]
    pub fn set_up(&mut self, value: Vector3) {
        let mut value = value;
        if value == Vector3::ZERO {
            value = Vector3::UP;
            godot_warn!("PhantomCamera3D: Up value cannot be (0, 0, 0), resetting to (0, 1, 0).");
        }
        self.up = value;
        if !self.has_up_target {
            self.up_direction = value;
        }
    }
    #[func]
    pub fn get_up(&self) -> Vector3 {
        self.up
    }
    #[func]
    pub fn set_up_target(&mut self, value: Option<Gd<Node3D>>) {
        let new_target = value.filter(|target| target.is_instance_valid());
        self.up_target = new_target.clone();
        match &new_target {
            Some(target) => {
                self.has_up_target = true;
                let callable = self.base().callable("pcam_up_target_tree_exiting");
                let mut node = target.clone().upcast::<Node>();
                if !node.is_connected("tree_exiting", &callable) {
                    node.connect("tree_exiting", &callable);
                }
            }
            None => {
                self.has_up_target = false;
                self.up_direction = self.up;
            }
        }
    }
    #[func]
    pub fn get_up_target(&self) -> Option<Gd<Node3D>> {
        self.up_target_node()
    }
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise3D>>) {
        self.noise = value;
        self.sync_noise_resource();
    }
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise3D>> {
        self.noise.clone()
    }
    #[func]
    pub fn has_noise_resource(&self) -> bool {
        self.has_noise_resource
    }
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }
    #[func]
    pub fn set_noise_emitter_layer_value(&mut self, layer: i64, enabled: bool) {
        self.noise_emitter_layer = set_layer(self.noise_emitter_layer, layer, enabled);
    }
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }
    #[func]
    pub fn set_inactive_update_mode(&mut self, value: i64) {
        self.inactive_update_mode = value;
    }
    #[func]
    pub fn get_inactive_update_mode(&self) -> i64 {
        self.inactive_update_mode
    }
    #[func]
    pub fn set_camera_3d_resource(&mut self, value: Option<Gd<Camera3DResource>>) {
        self.camera_3d_resource = value;
        self.signals().camera_3d_resource_changed().emit();
        self.sync_camera_resource();
    }
    #[func]
    pub fn get_camera_3d_resource(&self) -> Option<Gd<Camera3DResource>> {
        self.camera_3d_resource.clone()
    }
    fn emit_resource_property(&mut self, property: &str, value: Variant) {
        self.signals()
            .camera_3d_resource_property_changed()
            .emit(&StringName::from(property), &value);
    }
    #[func]
    pub fn set_keep_aspect(&mut self, value: i64) {
        if self.camera_3d_resource.is_none() {
            godot_warn!(
                "PhantomCamera3D: Can't assign a keep_aspect value without a Camera3DResource"
            );
            return;
        }
        self.emit_resource_property("keep_aspect", value.to_variant());
    }
    #[func]
    pub fn get_keep_aspect(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("keep_aspect"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_cull_mask(&mut self, value: i64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a cull_mask value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().persist("cull_mask", value.to_variant());
        self.emit_resource_property("cull_mask", value.to_variant());
    }
    #[func]
    pub fn set_cull_mask_value(&mut self, layer_number: i64, value: bool) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a cull_mask value without a Camera3DResource"
            );
            return;
        };
        let current = self.get_cull_mask().try_to::<i64>().unwrap_or_default();
        let mask = set_layer(current, layer_number, value);
        resource.bind_mut().persist("cull_mask", mask.to_variant());
        self.emit_resource_property("cull_mask", mask.to_variant());
    }
    #[func]
    pub fn get_cull_mask(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("cull_mask"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_environment(&mut self, value: Option<Gd<godot::classes::Environment>>) {
        self.environment = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("environment", emitted);
    }
    #[func]
    pub fn get_environment(&self) -> Option<Gd<godot::classes::Environment>> {
        self.environment.clone()
    }
    #[func]
    pub fn set_attributes(&mut self, value: Option<Gd<godot::classes::CameraAttributes>>) {
        self.attributes = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("attributes", emitted);
    }
    #[func]
    pub fn get_attributes(&self) -> Option<Gd<godot::classes::CameraAttributes>> {
        self.attributes.clone()
    }
    #[func]
    pub fn set_compositor(&mut self, value: Option<Gd<godot::classes::Resource>>) {
        self.compositor = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("compositor", emitted);
    }
    #[func]
    pub fn get_compositor(&self) -> Option<Gd<godot::classes::Resource>> {
        self.compositor.clone()
    }
    #[func]
    pub fn set_h_offset(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a h_offset value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().persist("h_offset", value.to_variant());
        self.emit_resource_property("h_offset", value.to_variant());
    }
    #[func]
    pub fn get_h_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("h_offset"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_v_offset(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a v_offset value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().persist("v_offset", value.to_variant());
        self.emit_resource_property("v_offset", value.to_variant());
    }
    #[func]
    pub fn get_v_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("v_offset"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_projection(&mut self, value: i64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a projection value without a Camera3DResource"
            );
            return;
        };
        resource
            .bind_mut()
            .persist("projection", value.to_variant());
        self.emit_resource_property("projection", value.to_variant());
    }
    #[func]
    pub fn get_projection(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("projection"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_fov(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a fov value without a Camera3DResource");
            return;
        };
        resource.bind_mut().persist("fov", value.to_variant());
        self.emit_resource_property("fov", value.to_variant());
    }
    #[func]
    pub fn get_fov(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("fov"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_size(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a size value without a Camera3DResource");
            return;
        };
        resource.bind_mut().persist("size", value.to_variant());
        self.emit_resource_property("size", value.to_variant());
    }
    #[func]
    pub fn get_size(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("size"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_frustum_offset(&mut self, value: Vector2) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a frustum_offset value without a Camera3DResource"
            );
            return;
        };
        resource
            .bind_mut()
            .persist("frustum_offset", value.to_variant());
        self.emit_resource_property("frustum_offset", value.to_variant());
    }
    #[func]
    pub fn get_frustum_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("frustum_offset"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_near(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a near value without a Camera3DResource");
            return;
        };
        resource.bind_mut().persist("near", value.to_variant());
        self.emit_resource_property("near", value.to_variant());
    }
    #[func]
    pub fn get_near(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("near"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn set_far(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a far value without a Camera3DResource");
            return;
        };
        resource.bind_mut().persist("far", value.to_variant());
        self.emit_resource_property("far", value.to_variant());
    }
    #[func]
    pub fn get_far(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("far"),
            None => Variant::nil(),
        }
    }
    #[func]
    pub fn get_follow_target_physics_based(&self) -> bool {
        self.follow_target_physics_based
    }
    #[func]
    pub fn draw_follow_gizmo_line(&self) -> bool {
        self.draw_follow_gizmo_line
    }
    #[func]
    pub fn draw_look_at_gizmo_line(&self) -> bool {
        self.draw_look_at_gizmo_line
    }
    #[func]
    pub fn assign_window_index(&mut self, value: i64) {
        self.viewport_index = value;
    }
    #[func]
    pub fn get_viewport_index(&self) -> i64 {
        self.viewport_index
    }
    #[func]
    pub fn set_dead_zone_width(&mut self, value: f64) {
        self.dead_zone_width = value;
        self.signals().dead_zone_changed().emit();
    }
    #[func]
    pub fn get_dead_zone_width(&self) -> f64 {
        self.dead_zone_width
    }
    #[func]
    pub fn set_dead_zone_height(&mut self, value: f64) {
        self.dead_zone_height = value;
        self.signals().dead_zone_changed().emit();
    }
    #[func]
    pub fn get_dead_zone_height(&self) -> f64 {
        self.dead_zone_height
    }
    #[func]
    pub fn set_preview_noise(&mut self, value: bool) {
        self._preview_noise = value;
        if !value {
            self.transform_noise = Transform3D::IDENTITY;
        }
    }
    #[func]
    pub fn get_preview_noise(&self) -> bool {
        self._preview_noise
    }
    #[func]
    pub fn pcam_check_visibility(&mut self) {
        self.check_visibility();
    }
    #[func]
    pub fn pcam_follow_target_tree_exiting(&mut self, target: Gd<Node>) {
        self.follow_target_tree_exiting_impl(&target);
    }
    #[func]
    pub fn pcam_look_at_target_tree_exiting(&mut self, target: Gd<Node>) {
        self.look_at_target_tree_exiting_impl(&target);
    }
    #[func]
    pub fn pcam_up_target_tree_exiting(&mut self) {
        self.up_target_tree_exiting_impl();
    }
    #[func]
    pub fn pcam_camera_resource_changed(&mut self) {
        self.camera_resource_changed_impl();
    }
    #[func]
    pub fn pcam_spring_arm_ready(&mut self) {
        self.finish_spring_arm();
    }
}
#[godot_api]
impl INode3D for PhantomCamera3D {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            priority_override: false,
            priority: 0,
            follow_mode: 0,
            follow_target: None,
            follow_targets: Array::new(),
            follow_path: None,
            look_at_mode: 0,
            look_at_target: None,
            look_at_targets: Array::new(),
            tween_resource: None,
            tween_on_load: true,
            inactive_update_mode: 0,
            host_layers: 1,
            camera_3d_resource: None,
            attributes: None,
            environment: None,
            compositor: None,
            follow_offset: Vector3::ZERO,
            follow_damping: false,
            follow_damping_value: Vector3::new(0.1005, 0.1005, 0.1005),
            follow_axis_lock: 0,
            follow_distance: 1.0,
            auto_follow_distance: false,
            auto_follow_distance_min: 1.0,
            auto_follow_distance_max: 5.0,
            auto_follow_distance_divisor: 10.0,
            dead_zone_width: 0.0,
            dead_zone_height: 0.0,
            show_viewfinder_in_play: false,
            vertical_rotation_offset: 0.0,
            horizontal_rotation_offset: 0.0,
            spring_length: 1.0,
            collision_mask: 1,
            shape: None,
            margin: 0.0105,
            look_at_offset: Vector3::ZERO,
            look_at_damping: false,
            look_at_damping_value: 0.2505,
            up: Vector3::UP,
            up_target: None,
            noise: None,
            _preview_noise: true,
            noise_emitter_layer: 0,
            viewport_index: 0,
            draw_follow_line: false,
            draw_look_at_line: false,
            is_active_pcam: false,
            is_third_person_follow: false,
            camera_target: None,
            should_follow: false,
            follow_target_physics_based: false,
            has_multiple_follow_targets: false,
            follow_targets_single_target_index: 0,
            should_look_at: false,
            has_multiple_look_at_targets: false,
            look_at_targets_single_target_index: 0,
            current_rotation: Vector3::ZERO,
            up_direction: Vector3::UP,
            has_up_target: false,
            follow_target_output_position: Vector3::ZERO,
            look_at_target_output_position: Vector3::ZERO,
            transform_output: Transform3D::IDENTITY,
            transform_noise: Transform3D::IDENTITY,
            tween_skip: false,
            follow_velocity_ref: Vector3::ZERO,
            follow_framed_initial_set: false,
            follow_framed_offset: Vector3::ZERO,
            follow_spring_arm: None,
            has_follow_spring_arm: false,
            has_noise_resource: false,
            draw_gizmo: false,
            draw_follow_gizmo_line: false,
            follow_target_position: Vector3::ZERO,
            draw_look_at_gizmo_line: false,
            pending_noise: None,
            pending_spring_arm: None,
            pending_axis_lock: None,
            tween_duration_fallback: 0.0,
            tween_transition_fallback: TWEEN_DEFAULT_TRANSITION,
            tween_ease_fallback: TWEEN_DEFAULT_EASE,
            follow_axis_is_locked: false,
            follow_axis_lock_value: Vector3::ZERO,
            viewport_position: Vector2::ZERO,
        }
    }
    fn enter_tree(&mut self) {
        self.enter_tree_impl();
    }
    fn exit_tree(&mut self) {
        self.exit_tree_impl();
    }
    fn ready(&mut self) {
        self.ready_impl();
    }
    fn process(&mut self, delta: f64) {
        self.process_impl(delta);
    }
    fn physics_process(&mut self, delta: f64) {
        self.physics_process_impl(delta);
    }
}
