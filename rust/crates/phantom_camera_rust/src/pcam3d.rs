// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/phantom_camera/phantom_camera_3d.gd
//
//! `PhantomCamera3D` — the 3D camera rig node.
//!
//! Ported from `addons/phantom_camera/scripts/phantom_camera/phantom_camera_3d.gd`
//! (v0.11.0.3, MIT, Marcus Skov), preserved as the read-only parity oracle. All
//! `:NNN` anchors below are lines of that file.
//!
//! Algorithm anchors:
//! - lifecycle `:865-952` (incl. the THIRD_PERSON `SpringArm3D` bootstrap `:914-941`)
//! - scheduling `:955-965`, `process_logic` `:968-1008`
//! - follow `:1011-1013`, `_set_follow_position` `:1020-1156`,
//!   `_interpolate_position` `:1198-1235`
//! - look at `:1015-1017`, `_set_look_at_position` `:1159-1176`,
//!   `_look_at_target_quat` `:1238-1256`, `_interpolate_rotation` `:1258-1291`
//! - FRAMED side offsets `:1326-1345` (note the `0.5001` bottom-edge epsilon)
//! - limits/axis locks `:988-1008`, `:2140-2170`
//! - `Camera3DResource` forwarding `:2248-2454`
//!
//! Omitted (editor-only), each with a `// parity:` note at the use site:
//! `_validate_property` `:722-862`, the `align_*_with_view` tool buttons
//! `:561-600`, the gizmo line bookkeeping that only drives `update_gizmos()`
//! (`:1489-1498`), and the editor branch of the FRAMED follow preview
//! `:1128-1147`.
//!
//! `show_viewfinder_in_play` is stored for ABI parity but inert (the viewfinder
//! panel is out of scope).

// The if/if let nesting below deliberately mirrors the oracle's branch structure
// so every block keeps a 1:1 line anchor to the GDScript; collapsing the branches
// (clippy::collapsible_if) would destroy that correspondence.
#![allow(clippy::collapsible_if)]
use godot::builtin::{Array, Transform3D, Vector2, Vector3};
use godot::classes::{Engine, Node3D, Path3D, Shape3D, SpringArm3D};
use godot::prelude::*;

use crate::abi::{
    PcamKind, TWEEN_DEFAULT_DURATION, TWEEN_DEFAULT_EASE, TWEEN_DEFAULT_TRANSITION, set_layer,
};
use crate::manager::PhantomCameraManager;
use crate::resources::{Camera3DResource, PhantomCameraNoise3D, PhantomCameraTween};

/// Godot `clampf` — never panics when `min > max`.
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
    value.abs() < 0.00001
}

/// `Vector3[i]` as the oracle writes it (gdext needs `Vector3Axis`, not an int).
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

    // ----- exported surface (`#region Exported Properties`, `:123-616`) -----
    /// `:130-141`.
    #[export]
    #[var(set = set_priority_override, get = get_priority_override)]
    pub priority_override: bool,
    /// `:154-156`.
    #[export]
    #[var(set = set_priority, get = get_priority)]
    pub priority: i64,
    /// `:163-217`.
    #[export]
    #[var(
        set = set_follow_mode,
        get = get_follow_mode,
        hint = ENUM,
        hint_string = "None:0,Glued:1,Simple:2,Group:3,Path:4,Framed:5,Third Person:6"
    )]
    pub follow_mode: i64,
    /// `:222-224`.
    #[export]
    #[var(set = set_follow_target, get = get_follow_target)]
    pub follow_target: Option<Gd<Node3D>>,
    /// `:227-229`.
    #[export]
    #[var(set = set_follow_targets, get = get_follow_targets)]
    pub follow_targets: Array<Gd<Node3D>>,
    /// `:235-237`.
    #[export]
    #[var(set = set_follow_path, get = get_follow_path)]
    pub follow_path: Option<Gd<Path3D>>,
    /// `:244-274`.
    #[export]
    #[var(
        set = set_look_at_mode,
        get = get_look_at_mode,
        hint = ENUM,
        hint_string = "None:0,Mimic:1,Simple:2,Group:3"
    )]
    pub look_at_mode: i64,
    /// `:279-281`.
    #[export]
    #[var(set = set_look_at_target, get = get_look_at_target)]
    pub look_at_target: Option<Gd<Node3D>>,
    /// `:285-287`.
    #[export]
    #[var(set = set_look_at_targets, get = get_look_at_targets)]
    pub look_at_targets: Array<Gd<Node3D>>,
    /// `:297-299`.
    #[export]
    #[var(set = set_tween_resource, get = get_tween_resource)]
    pub tween_resource: Option<Gd<PhantomCameraTween>>,
    /// `:307-309`.
    #[export]
    #[var(set = set_tween_on_load, get = get_tween_on_load)]
    pub tween_on_load: bool,
    /// `:316-318`.
    #[export]
    #[var(
        set = set_inactive_update_mode,
        get = get_inactive_update_mode,
        hint = ENUM,
        hint_string = "Always:0,Never:1"
    )]
    pub inactive_update_mode: i64,
    /// `:323-325` — `PROPERTY_HINT_LAYERS_3D_RENDER`.
    #[export(flags_3d_render)]
    #[var(set = set_host_layers, get = get_host_layers)]
    pub host_layers: i64,
    /// `:330-332`.
    #[export]
    #[var(set = set_camera_3d_resource, get = get_camera_3d_resource)]
    pub camera_3d_resource: Option<Gd<Camera3DResource>>,
    /// `:336-338`.
    #[export]
    #[var(set = set_attributes, get = get_attributes)]
    pub attributes: Option<Gd<godot::classes::CameraAttributes>>,
    /// `:342-344`.
    #[export]
    #[var(set = set_environment, get = get_environment)]
    pub environment: Option<Gd<godot::classes::Environment>>,
    /// `:348-350`.
    ///
    /// Seam note: `godot::classes::Compositor` is not emitted by this gdext build's
    /// codegen, so the field is a `Resource` narrowed by an explicit
    /// `PROPERTY_HINT_RESOURCE_TYPE` of `Compositor` — the exact ABI the oracle
    /// exports (type `Object`, hint 17, hint string `Compositor`).
    #[export]
    #[var(
        set = set_compositor,
        get = get_compositor,
        hint = RESOURCE_TYPE,
        hint_string = "Compositor"
    )]
    pub compositor: Option<Gd<godot::classes::Resource>>,
    /// `:355-357`.
    #[export]
    #[var(set = set_follow_offset, get = get_follow_offset)]
    pub follow_offset: Vector3,
    /// `:362-364`.
    #[export]
    #[var(set = set_follow_damping, get = get_follow_damping)]
    pub follow_damping: bool,
    /// `:370-373`.
    #[export]
    #[var(set = set_follow_damping_value, get = get_follow_damping_value, hint = LINK)]
    pub follow_damping_value: Vector3,
    /// `:378-380`.
    #[export]
    #[var(
        set = set_follow_axis_lock,
        get = get_follow_axis_lock,
        hint = ENUM,
        hint_string = "None:0,X:1,Y:2,Z:3,Xy:4,Xz:5,Yz:6,Xyz:7"
    )]
    pub follow_axis_lock: i64,
    /// `:381-382` — `_follow_axis_is_locked`.
    follow_axis_is_locked: bool,
    /// `:382` — `_follow_axis_lock_value`.
    follow_axis_lock_value: Vector3,
    /// `:387-389`.
    #[export]
    #[var(set = set_follow_distance, get = get_follow_distance)]
    pub follow_distance: f64,
    /// `:398-400`.
    #[export]
    #[var(set = set_auto_follow_distance, get = get_auto_follow_distance)]
    pub auto_follow_distance: bool,
    /// `:409-411`.
    #[export]
    #[var(set = set_auto_follow_distance_min, get = get_auto_follow_distance_min)]
    pub auto_follow_distance_min: f64,
    /// `:414-416`.
    #[export]
    #[var(set = set_auto_follow_distance_max, get = get_auto_follow_distance_max)]
    pub auto_follow_distance_max: f64,
    /// `:426-428`.
    #[export]
    #[var(
        set = set_auto_follow_distance_divisor,
        get = get_auto_follow_distance_divisor
    )]
    pub auto_follow_distance_divisor: f64,
    /// `:437-442`.
    #[export]
    #[var(
        set = set_dead_zone_width,
        get = get_dead_zone_width,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_width: f64,
    /// `:449-454`.
    #[export]
    #[var(
        set = set_dead_zone_height,
        get = get_dead_zone_height,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_height: f64,
    /// `:458` — inert (viewfinder out of scope).
    #[export]
    pub show_viewfinder_in_play: bool,
    /// `:466-468` — `PROPERTY_HINT_RANGE`, radians-as-degrees.
    #[export]
    #[var(
        set = set_vertical_rotation_offset,
        get = get_vertical_rotation_offset,
        hint = RANGE,
        hint_string = "-360.0,360.0,0.1,or_greater,or_less,radians_as_degrees"
    )]
    pub vertical_rotation_offset: f64,
    /// `:472-474` — `PROPERTY_HINT_RANGE`, radians-as-degrees.
    #[export]
    #[var(
        set = set_horizontal_rotation_offset,
        get = get_horizontal_rotation_offset,
        hint = RANGE,
        hint_string = "-360.0,360.0,0.1,or_greater,or_less,radians_as_degrees"
    )]
    pub horizontal_rotation_offset: f64,
    /// `:477-479`.
    #[export]
    #[var(set = set_spring_length, get = get_spring_length)]
    pub spring_length: f64,
    /// `:482-484`.
    #[export(flags_3d_physics)]
    #[var(set = set_collision_mask, get = get_collision_mask)]
    pub collision_mask: i64,
    /// `:487-489`.
    #[export]
    #[var(set = set_shape, get = get_shape)]
    pub shape: Option<Gd<Shape3D>>,
    /// `:492-494`.
    #[export]
    #[var(set = set_margin, get = get_margin)]
    pub margin: f64,
    /// `:499-501`.
    #[export]
    #[var(set = set_look_at_offset, get = get_look_at_offset)]
    pub look_at_offset: Vector3,
    /// `:506-508`.
    #[export]
    #[var(set = set_look_at_damping, get = get_look_at_damping)]
    pub look_at_damping: bool,
    /// `:514-516` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_look_at_damping_value,
        get = get_look_at_damping_value,
        hint = RANGE,
        hint_string = "0.0,1.0,0.001,or_greater"
    )]
    pub look_at_damping_value: f64,
    /// `:521-523`.
    #[export]
    #[var(set = set_up, get = get_up)]
    pub up: Vector3,
    /// `:527-529`.
    #[export]
    #[var(set = set_up_target, get = get_up_target)]
    pub up_target: Option<Gd<Node3D>>,
    /// `:535-537`.
    #[export]
    #[var(set = set_noise, get = get_noise)]
    pub noise: Option<Gd<PhantomCameraNoise3D>>,
    /// `:543-547`.
    #[export]
    #[var(set = set_preview_noise, get = get_preview_noise)]
    pub _preview_noise: bool,
    /// `:551-553`.
    #[export(flags_3d_render)]
    #[var(set = set_noise_emitter_layer, get = get_noise_emitter_layer)]
    pub noise_emitter_layer: i64,
    /// `:596-600` — editor "align with view" viewport selector.
    #[export]
    #[var(set = set_viewport_index, get = get_viewport_index, hint = RANGE, hint_string = "0.0,3.0")]
    pub viewport_index: i64,
    /// `:606-608`.
    #[export]
    #[var(set = set_draw_follow_line, get = get_draw_follow_line)]
    pub draw_follow_line: bool,
    /// `:612-614`.
    #[export]
    #[var(set = set_draw_look_at_line, get = get_draw_look_at_line)]
    pub draw_look_at_line: bool,

    // ----- private state (`#region Private Variables`, `:618-670`) -----
    is_active_pcam: bool,
    is_third_person_follow: bool,
    /// `:623` — `null` means "self" (`_camera_target = self`).
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
    /// `:633` — written by `_look_at_targets_size_check` and never read (oracle too).
    #[allow(dead_code)]
    look_at_target_physics_based: bool,
    /// Pending `noise_emitted` payload for the host.
    pending_noise: Option<Transform3D>,
    /// Deferred `await arm.ready` continuation (`:935-941`).
    pending_spring_arm: Option<(Gd<SpringArm3D>, Vector3)>,
    /// `follow_axis_lock` assigned before the node was ready (`:2140-2170`).
    pending_axis_lock: Option<i64>,

    // ----- public non-exported script variables (`:672-715`) -----
    tween_duration_fallback: f64,
    tween_transition_fallback: i64,
    tween_ease_fallback: i64,
    /// `:684-686` — the oracle's own `keep_aspect` shim, written by the setter only.
    #[allow(dead_code)]
    keep_aspect_value: i64,
    pub viewport_position: Vector2,
}

#[godot_api]
impl PhantomCamera3D {
    // ------------------------------------------------------------------
    // Signals (`#region Signals`, `:19-67`)
    // ------------------------------------------------------------------
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
    /// 3D's `dead_zone_reached` has **no** arguments (`:42`), unlike 2D's.
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

    /// ABI discriminator replacing `get_class()` (`:2470-2476`).
    #[func]
    pub fn pcam_kind(&self) -> GString {
        GString::from(PcamKind::ThreeD.as_str())
    }

    // ------------------------------------------------------------------
    // Typed seams used by PhantomCameraHost / PhantomCameraManager.
    // ------------------------------------------------------------------

    /// `phantom_camera_host.gd:364-366`.
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

    /// The oracle hosts the `Camera3DResource` override apply in the host
    /// (`phantom_camera_host.gd:941-955`); the port keeps those property reads
    /// typed on the resource instead of duck-typing through `Gd<Node>`.
    pub fn camera_resource(&self) -> Option<Gd<Camera3DResource>> {
        self.camera_3d_resource.clone()
    }

    pub fn attributes_resource(&self) -> Option<Gd<godot::classes::CameraAttributes>> {
        self.attributes.clone()
    }

    /// `has_noise_resource()` (`:2222-2223`).
    pub fn pcam_has_noise_resource(&self) -> bool {
        self.has_noise_resource
    }

    pub fn take_noise_output(&mut self) -> Option<Transform3D> {
        self.pending_noise.take()
    }

    /// `:1458-1460`.
    pub fn pcam_noise_emitted(&mut self, emitter_noise_output: Transform3D, emitter_layer: i64) {
        if self.noise_emitter_layer & emitter_layer != 0 {
            self.signals().noise_emitted().emit(emitter_noise_output);
            self.pending_noise = Some(emitter_noise_output);
        }
    }

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// `:865-903`.
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

        // Single-target follow modes only reached `_check_physics_body` through the
        // oracle's `set_follow_target` (`:1695-1718`), which ran before `_ready`.
        if matches!(self.follow_mode, 1 | 2 | 5 | 6) {
            if let Some(target) = self.follow_target_node() {
                self.check_physics_body(&target);
            }
        }
        // Same for the non-group look-at modes (`:2037-2053`).
        if matches!(self.look_at_mode, 1 | 2) {
            if let Some(target) = self.look_at_target_node() {
                self.check_physics_body(&target);
            }
        }
    }

    /// `:906-911`.
    fn exit_tree_impl(&mut self) {
        // parity: `:907-908` assigns `follow_targets = []` through
        // `set_follow_targets`, which returns early unless the mode is GROUP — a
        // no-op in the oracle too.
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_pcam_removed(PcamKind::ThreeD, &this);
    }

    /// `:914-952`.
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

        // `:950` — deliberately overwrites the origin set above (oracle behavior).
        self.transform_output = self.base().get_global_transform();

        self.sync_noise_resource();
        self.sync_camera_resource();
        if let Some(lock) = self.pending_axis_lock.take() {
            self.apply_axis_lock(lock);
        }
    }

    /// `:920-941` — the `SpringArm3D` bootstrap and its deferred ordering.
    ///
    /// Order reproduced exactly: configure -> `add_excluded_object` ->
    /// `get_parent().add_child.call_deferred(arm)` -> `await arm.ready` (here: the
    /// arm's `ready` signal) -> `reparent.call_deferred(arm)` -> `_camera_target` ->
    /// restore global position/rotation -> `_has_follow_spring_arm = true` ->
    /// `top_level = false`.
    fn bootstrap_spring_arm(&mut self) {
        if self.follow_spring_arm.is_some() {
            return;
        }

        // code-instantiated: the oracle creates this `SpringArm3D` at runtime
        // because a scene-authored arm cannot be reconfigured per PCam.
        let mut arm = SpringArm3D::new_alloc();
        // `:921` assigns the member immediately, so the setters below (`:1958`,
        // `:1971`, `:1991`, `:2003`) reach the arm before it enters the tree.
        self.follow_spring_arm = Some(arm.clone());
        arm.set("top_level", &true.to_variant());
        // parity: `:923` reads the `spring_length` *property*, whose getter (`:1962`)
        // returns `follow_distance` — `set_spring_length` (`:1955`) only ever writes
        // `follow_distance`, so the raw field keeps its ABI default.
        arm.set_length(real_of(self.follow_distance));
        arm.set_collision_mask(self.collision_mask as u32);
        if let Some(shape) = self.shape.clone() {
            arm.set_shape(&shape);
        }
        arm.set_margin(real_of(self.margin));

        let initial_rotation = self.base().get_global_rotation();

        if self.should_follow {
            if let Some(target) = self.follow_target_node() {
                self.exclude_object_from_arm(&mut arm, &target);
            }
        }

        // Resolves the `await arm.ready` continuation when the deferred add_child
        // lands the arm in the tree.
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

    /// `:930` — `add_excluded_object(follow_target)`; GDScript relies on Godot's
    /// implicit `Object -> RID` conversion, so `get_rid()` is called by name.
    fn exclude_object_from_arm(&self, arm: &mut Gd<SpringArm3D>, target: &Gd<Node3D>) {
        let mut object = target.clone().upcast::<godot::classes::Object>();
        if !object.has_method("get_rid") {
            return;
        }
        if let Ok(rid) = object.call("get_rid", &[]).try_to::<Rid>() {
            arm.add_excluded_object(rid);
        }
    }

    /// `:936-941` — the continuation of the oracle's `await _follow_spring_arm.ready`.
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
        // `:941` `top_level = false`.
        self.base_mut().set("top_level", &false.to_variant());
    }

    /// `:955-960`.
    fn process_impl(&mut self, delta: f64) {
        // `:956-957` `update_gizmos()` is editor-only: omitted.
        if self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }

    /// `:963-965`.
    fn physics_process_impl(&mut self, delta: f64) {
        if !self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }

    /// `:968-1008`.
    #[func]
    pub fn process_logic(&mut self, delta: f64) {
        if self.is_active_pcam {
            if self.has_noise_resource && self._preview_noise {
                self.transform_noise = self.noise_transform(delta);
            }
        } else if self.inactive_update_mode == 1 {
            // `:973-976` — `InactiveUpdateMode.NEVER`.
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

    /// `:1011-1013`.
    fn follow(&mut self, delta: f64) {
        self.set_follow_position(delta);
        self.interpolate_position(delta);
    }

    /// `:1015-1017`.
    fn look_at(&mut self, delta: f64) {
        self.set_look_at_position();
        self.interpolate_rotation(delta);
    }

    /// `:1020-1156`.
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

    /// `:1029-1053`.
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
            if let Some(target) = self.follow_targets.get(index) {
                if target.is_instance_valid() {
                    self.follow_target_output_position = target.get_global_position()
                        + self.follow_offset
                        + self.base().get_global_basis().col_c()
                            * real_of(self.auto_follow_distance_min);
                }
            }
        }
    }

    /// `:1063-1125`.
    ///
    /// parity: `:1071-1074` assigns `_current_rotation = global_rotation`
    /// immediately before testing `if _current_rotation != global_rotation`, and the
    /// same pattern repeats at `:1093` / `:1100`. Both branches are unreachable dead
    /// code in the oracle; they are reproduced here (never taken) so the structure —
    /// and the `0.5001` bottom-edge epsilon in `_get_framed_side_offset` — stays
    /// identical.
    fn set_follow_position_framed(&mut self, editor: bool) {
        if !editor {
            if !self.is_active_pcam {
                self.follow_target_output_position = self.get_target_position_offset_distance();
                return;
            }

            // `:1068-1070` — normalized viewport position of the follow target.
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

            // parity: dead branch (`:1073`) — kept for structure.
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
                    // parity: second dead branch (`:1093`).
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
            // parity: TODO the editor-only unprojected preview (`:1128-1147`) is
            // omitted; it only drives the inspector's dead-zone overlay.
            self.follow_target_output_position = self.get_target_position_offset_distance();
        }
    }

    /// `:1159-1176`.
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

    /// `:1178-1179`.
    fn target_position_offset(&self) -> Vector3 {
        match self.follow_target_node() {
            Some(target) => target.get_global_position() + self.follow_offset,
            None => self.follow_offset,
        }
    }

    /// `:1182-1184`.
    fn get_target_position_offset_distance(&self) -> Vector3 {
        self.target_position_offset()
            + self.base().get_transform().basis.col_c() * real_of(self.follow_distance)
    }

    /// `:1187-1192` — editor-only initial Third Person position.
    ///
    /// GDScript evaluates `basis.z * follow_distance * Q(v) * Q(h)` left-to-right
    /// (Vector3 * float, then two `Vector3 * Quaternion` rotations); the port applies
    /// the same two rotations as bases.
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

    /// `:1198-1235`.
    fn interpolate_position(&mut self, delta: f64) {
        let editor = Engine::singleton().is_editor_hint();

        if self.follow_damping && !editor {
            if !self.is_third_person_follow {
                let target_position = self.follow_target_output_position;
                self.base_mut().set_global_position(target_position);
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
                self.base_mut().set_global_basis(target_basis);
            }
        } else if self.has_follow_spring_arm {
            if let Some(arm) = self.follow_spring_arm.clone() {
                let basis = arm.get_global_basis();
                self.transform_output.basis = basis;
                self.base_mut().set_global_basis(basis);
            }
        }
    }

    /// `:1238-1256`.
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
            // parity: `:1247-1254` — the degenerate look-at falls back to the
            // current basis and nudges the X euler by ±90°.
            let mut degrees = self.base().get_global_rotation_degrees();
            if target_basis.col_c() == Vector3::UP {
                degrees.x = -90.0;
            } else {
                degrees.x = 90.0;
            }
            self.base_mut().set_global_rotation_degrees(degrees);
            self.transform_output.basis = self.base().get_global_basis();
            return self.base().get_quaternion();
        }

        target_basis.get_quaternion().normalized()
    }

    /// `:1258-1291`.
    fn interpolate_rotation(&mut self, delta: f64) {
        if self.has_up_target {
            if let Some(target) = self.up_target_node() {
                self.up_direction = target.get_global_basis().col_b();
            }
        }

        let mut target_quat =
            self.look_at_target_quat(self.look_at_target_output_position, self.up_direction);

        if self.look_at_damping {
            let current_quat = self.base().get_quaternion().normalized();
            let damping_time = self.look_at_damping_value.max(0.0001);
            let t = (delta / damping_time).min(1.0);

            // parity: `:1269` `current_quat.dot(target_quat)` — Godot's
            // `Quaternion::dot` sums `x*x + y*y + z*z + w*w` left to right in
            // `real_t`; gdext's `dot` routes through glam, whose horizontal sum
            // rounds differently, and this dot feeds the `is_zero_approx` early-out
            // below, so one ulp decides whether the damping step runs at all.
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
            // `:1285` `current_quat * ratio_a + target_quat * ratio_b` goes through
            // `Quaternion::operator*(real_t)` / `operator+`, i.e. in Godot's `real_t`
            // (f32), so the weights are narrowed before the products rather than after.
            let output = current_quat * real_of(ratio_a) + target_quat * real_of(ratio_b);

            self.transform_output.basis = Basis::from_quaternion(output);
            self.base_mut().set_quaternion(output);
        } else {
            self.transform_output.basis = Basis::from_quaternion(target_quat);
            self.base_mut().set_quaternion(target_quat);
        }
    }

    /// `:1326-1345`.
    ///
    /// parity: the bottom-edge test uses `0.5001` (not `0.5`) so a zero-height dead
    /// zone can actually resolve, exactly as the oracle comment at `:1341` says.
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
        if position.y > 0.5001 + real_of(self.dead_zone_height) / 2.0 {
            frame_out_bounds.y = -1.0;
        }

        frame_out_bounds
    }

    /// `:1318-1319`.
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

    /// `:1363-1364`.
    fn check_visibility(&mut self) {
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_visibility_changed(&this);
    }

    /// `:1367-1372`.
    fn follow_target_tree_exiting_impl(&mut self, target: &Gd<Node>) {
        if let Some(current) = self.follow_target_node() {
            if current.instance_id() == target.instance_id() {
                self.should_follow = false;
            }
        }
    }

    /// `:1374-1384`.
    fn should_follow_checker(&mut self) {
        if self.follow_mode == 0 {
            self.should_follow = false;
            return;
        }
        if self.follow_mode != 3 {
            self.should_follow = self.follow_target_node().is_some();
        }
    }

    /// `:1386-1409`.
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

    /// `:1412-1416`.
    fn look_at_target_tree_exiting_impl(&mut self, target: &Gd<Node>) {
        if let Some(current) = self.look_at_target_node() {
            if current.instance_id() == target.instance_id() {
                self.should_look_at = false;
            }
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

    /// `:1418-1420`.
    fn up_target_tree_exiting_impl(&mut self) {
        self.set_up_target(None);
    }

    /// `:1422-1431`.
    fn should_look_at_checker(&mut self) {
        if self.look_at_mode == 0 {
            self.should_look_at = false;
            return;
        }
        if self.look_at_mode != 3 {
            self.should_look_at = self.look_at_target_node().is_some();
        }
    }

    /// `:1434-1455`.
    ///
    /// parity: `:1436` resets `_look_at_target_physics_based`, not the follow flag,
    /// yet `_check_physics_body` below can set `_follow_target_physics_based` — so
    /// look-at group targets can flip the PCam onto the physics tick. Reproduced.
    fn look_at_targets_size_check(&mut self) {
        let mut targets_size = 0i64;
        self.look_at_target_physics_based = false;

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

    /// `:1463-1474`.
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

    /// `:1477-1482`.
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
            if let Some(parent) = &parent {
                if parent
                    .clone()
                    .try_cast::<godot::classes::PhysicsBody3D>()
                    .is_ok()
                {
                    self.follow_target_physics_based = true;
                }
            }
            current = parent;
        }
    }

    /// `:1485-1487`.
    fn camera_resource_changed_impl(&mut self) {
        self.signals().camera_3d_resource_changed().emit();
    }

    /// `:1493-1498` — the `update_gizmos()` call is editor-only and omitted; the
    /// flag bookkeeping is kept because `draw_follow_gizmo_line()` /
    /// `draw_look_at_gizmo_line()` expose it (`:2462-2467`).
    fn check_draw_gizmo(&mut self) {
        self.draw_gizmo = self.draw_follow_gizmo_line || self.draw_look_at_gizmo_line;
    }

    /// `:2140-2170` (`await ready` becomes [`Self::pending_axis_lock`]).
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

    // ----- reference resolution -----

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
        // parity: `:1397` — the oracle connects
        // `tree_exiting.connect(_follow_target_tree_exiting.bind(follow_targets[i]))`,
        // so the handler takes the target as its single argument; `tree_exiting`
        // itself carries none, hence the `bind` here.
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
        // parity: `:1443` — `_look_at_target_tree_exiting.bind(look_at_targets[i])`.
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

    /// `:623` — `_camera_target` is `self` outside THIRD_PERSON.
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

    /// `:2249-2254` — connects `camera_3d_resource.changed`.
    fn sync_camera_resource(&mut self) {
        if let Some(resource) = self.camera_3d_resource.clone() {
            let callable = self.base().callable("pcam_camera_resource_changed");
            if !resource.is_connected("changed", &callable) {
                let mut object = resource.upcast::<godot::classes::Object>();
                object.connect("changed", &callable);
            }
        }
    }

    // ------------------------------------------------------------------
    // Public API (`#region Public Functions`, `:1502-1574`)
    // ------------------------------------------------------------------

    /// `:1528-1529`.
    #[func]
    pub fn get_transform_output(&self) -> Transform3D {
        self.transform_output
    }

    /// `:1533-1534`.
    #[func]
    pub fn get_noise_transform(&self) -> Transform3D {
        self.transform_noise
    }

    /// `:1539-1540`.
    #[func]
    pub fn emit_noise(&mut self, value: Transform3D) {
        self.signals().noise_emitted().emit(value);
        self.pending_noise = Some(value);
    }

    /// `:1545-1549`.
    #[func]
    pub fn teleport_position(&mut self) {
        self.follow_velocity_ref = Vector3::ZERO;
        self.set_follow_position(0.0);
        self.transform_output.origin = self.follow_target_output_position;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_teleport(&this);
    }

    /// `:1555-1556`.
    #[func]
    pub fn is_following(&self) -> bool {
        self.should_follow
    }

    /// `:1561-1562`.
    #[func]
    pub fn is_looking(&self) -> bool {
        self.should_look_at
    }

    /// `:1567-1568`.
    #[func]
    pub fn get_follow_target_position(&self) -> Vector3 {
        self.follow_target_position
    }

    /// `:1571-1572`.
    #[func]
    pub fn get_look_at_target_position(&self) -> Vector3 {
        self.look_at_target_output_position
    }

    // ------------------------------------------------------------------
    // Setters & getters (`:1577-2477`)
    // ------------------------------------------------------------------

    /// `:1582-1586`.
    #[func]
    pub fn set_tween_skip(&mut self, caller: Gd<Node>, value: bool) {
        if caller.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.tween_skip = value;
        } else {
            godot_warn!("PhantomCamera3D: set_tween_skip can only be called by PhantomCameraHost");
        }
    }

    /// `:1588-1589`.
    #[func]
    pub fn get_tween_skip(&self) -> bool {
        self.tween_skip
    }

    /// `:1593-1597`.
    #[func]
    pub fn set_priority(&mut self, value: i64) {
        self.priority = value.max(0);
        if !self.base().is_node_ready() {
            return;
        }
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_priority_changed(&this);
    }

    /// `:1599-1600`.
    #[func]
    pub fn get_priority(&self) -> i64 {
        self.priority
    }

    /// `:130-141`.
    #[func]
    pub fn set_priority_override(&mut self, value: bool) {
        self.priority_override = value;
        if Engine::singleton().is_editor_hint() {
            let this = self.base().clone().upcast::<Node>();
            PhantomCameraManager::notify_priority_override(&this, value);
        }
    }

    /// `:140-141`.
    #[func]
    pub fn get_priority_override(&self) -> bool {
        self.priority_override
    }

    /// `:1604-1605`.
    #[func]
    pub fn set_tween_resource(&mut self, value: Option<Gd<PhantomCameraTween>>) {
        self.tween_resource = value;
    }

    /// `:1608-1609`.
    #[func]
    pub fn get_tween_resource(&self) -> Option<Gd<PhantomCameraTween>> {
        self.tween_resource.clone()
    }

    /// `:1613-1614`.
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

    /// `:1617-1621` (see the 2D counterpart for the ABI-default fallback note).
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

    /// `:1626-1627`.
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

    /// `:1630-1634`.
    #[func]
    pub fn get_tween_transition(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().transition,
            None => self.tween_transition_fallback,
        }
    }

    /// `:1639-1640`.
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

    /// `:1643-1647`.
    #[func]
    pub fn get_tween_ease(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().ease,
            None => self.tween_ease_fallback,
        }
    }

    /// `:1652-1656`.
    #[func]
    pub fn set_is_active(&mut self, node: Gd<Node>, value: bool) {
        if node.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.is_active_pcam = value;
        } else {
            godot_warn!("PhantomCamera3D: PCams can only be set from the PhantomCameraHost");
        }
    }

    /// `:1660-1661`.
    #[func]
    pub fn is_active(&self) -> bool {
        self.is_active_pcam
    }

    /// `:1665-1666`.
    #[func]
    pub fn set_tween_on_load(&mut self, value: bool) {
        self.tween_on_load = value;
    }

    /// `:1668-1669`.
    #[func]
    pub fn get_tween_on_load(&self) -> bool {
        self.tween_on_load
    }

    /// `:1673-1676`.
    #[func]
    pub fn set_host_layers(&mut self, value: i64) {
        self.host_layers = value;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_host_layer_changed(&this);
    }

    /// `:1679-1680`.
    #[func]
    pub fn set_host_layers_value(&mut self, layer: i64, value: bool) {
        self.host_layers = set_layer(self.host_layers, layer, value);
    }

    /// `:1683-1684`.
    #[func]
    pub fn get_host_layers(&self) -> i64 {
        self.host_layers
    }

    /// `:1690-1691`.
    #[func]
    pub fn get_follow_mode(&self) -> i64 {
        self.follow_mode
    }

    /// `:163-217`.
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

        // `:189-194` — the follow gizmo line is editor-only bookkeeping.
        if value == 1 {
            self.draw_follow_gizmo_line = false;
            self.check_draw_gizmo();
        } else {
            self.draw_follow_gizmo_line = self.draw_follow_line;
            self.check_draw_gizmo();
        }

        if value == 5 {
            // parity: `:196-202` — see the 2D note: `_follow_framed_initial_set` is
            // never set true, so `_on_dead_zone_changed` stays unreachable.
            if self.follow_framed_initial_set && self.follow_target_node().is_some() {
                self.follow_framed_initial_set = false;
            }
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

    /// `:1695-1718`.
    #[func]
    pub fn set_follow_target(&mut self, value: Option<Gd<Node3D>>) {
        if self.follow_mode == 0 || self.follow_mode == 3 {
            return;
        }
        let new_target = value.filter(|target| target.is_instance_valid());
        if let (Some(current), Some(new)) = (self.follow_target_node(), &new_target) {
            if current.instance_id() == new.instance_id() {
                return;
            }
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
                if self.follow_mode == 6 {
                    if let Some(mut arm) = self.follow_spring_arm.clone() {
                        self.exclude_object_from_arm(&mut arm, target);
                    }
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

    /// `:1720-1721` — the 3D version is a bare null assignment.
    #[func]
    pub fn erase_follow_target(&mut self) {
        self.follow_target = None;
    }

    /// `:1723-1724`.
    #[func]
    pub fn get_follow_target(&self) -> Option<Gd<Node3D>> {
        self.follow_target_node()
    }

    /// `:1728-1733`.
    #[func]
    pub fn set_follow_path(&mut self, value: Option<Gd<Path3D>>) {
        self.follow_path = value.filter(|path| path.is_instance_valid());
        if self.valid_follow_path().is_some() {
            self.should_follow_checker();
        } else {
            self.should_follow = false;
        }
    }

    /// `:1736-1737`.
    #[func]
    pub fn erase_follow_path(&mut self) {
        self.follow_path = None;
    }

    /// `:1740-1741`.
    #[func]
    pub fn get_follow_path(&self) -> Option<Gd<Path3D>> {
        self.valid_follow_path()
    }

    /// `:1745-1749`.
    #[func]
    pub fn set_follow_targets(&mut self, value: Array<Gd<Node3D>>) {
        if self.follow_mode != 3 {
            return;
        }
        self.follow_targets = value;
        self.follow_targets_size_check();
    }

    /// `:1753-1762`.
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

    /// `:1765-1772`.
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

    /// `:1775-1777`.
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

    /// `:1781-1782`.
    #[func]
    pub fn get_follow_targets(&self) -> Array<Gd<Node3D>> {
        self.follow_targets.clone()
    }

    /// `:1786-1811`.
    ///
    /// parity: `:1792` `match value:` compares the *offset `Vector3`* against the
    /// `FollowLockAxis` enum, so no arm matches and the lock value is never adjusted.
    #[func]
    pub fn set_follow_offset(&mut self, value: Vector3) {
        let temp_offset = self.follow_offset;
        self.follow_offset = value;
        if self.follow_axis_lock != 0 {
            let _ = temp_offset - value;
        }
    }

    /// `:1814-1815`.
    #[func]
    pub fn get_follow_offset(&self) -> Vector3 {
        self.follow_offset
    }

    /// `:1819-1821`.
    #[func]
    pub fn set_follow_damping(&mut self, value: bool) {
        self.follow_damping = value;
    }

    /// `:1824-1825`.
    #[func]
    pub fn get_follow_damping(&self) -> bool {
        self.follow_damping
    }

    /// `:1829-1834`.
    #[func]
    pub fn set_follow_damping_value(&mut self, value: Vector3) {
        self.follow_damping_value =
            Vector3::new(value.x.max(0.0), value.y.max(0.0), value.z.max(0.0));
    }

    /// `:1837-1838`.
    #[func]
    pub fn get_follow_damping_value(&self) -> Vector3 {
        self.follow_damping_value
    }

    /// `:1842-1843`.
    #[func]
    pub fn set_follow_distance(&mut self, value: f64) {
        self.follow_distance = value;
    }

    /// `:1846-1847`.
    #[func]
    pub fn get_follow_distance(&self) -> f64 {
        self.follow_distance
    }

    /// `:1851-1853`.
    #[func]
    pub fn set_auto_follow_distance(&mut self, value: bool) {
        self.auto_follow_distance = value;
    }

    /// `:1856-1857`.
    #[func]
    pub fn get_auto_follow_distance(&self) -> bool {
        self.auto_follow_distance
    }

    /// `:1861-1862`.
    #[func]
    pub fn set_auto_follow_distance_min(&mut self, value: f64) {
        self.auto_follow_distance_min = value;
    }

    /// `:1865-1866`.
    #[func]
    pub fn get_auto_follow_distance_min(&self) -> f64 {
        self.auto_follow_distance_min
    }

    /// `:1870-1871`.
    #[func]
    pub fn set_auto_follow_distance_max(&mut self, value: f64) {
        self.auto_follow_distance_max = value;
    }

    /// `:1873-1874`.
    #[func]
    pub fn get_auto_follow_distance_max(&self) -> f64 {
        self.auto_follow_distance_max
    }

    /// `:1878-1879`.
    #[func]
    pub fn set_auto_follow_distance_divisor(&mut self, value: f64) {
        self.auto_follow_distance_divisor = value;
    }

    /// `:1882-1883`.
    #[func]
    pub fn get_auto_follow_distance_divisor(&self) -> f64 {
        self.auto_follow_distance_divisor
    }

    /// `:1888-1892`.
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

    /// `:1896-1900`.
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

    /// `:1905-1909`.
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

    /// `:1913-1917`.
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

    /// `:1922-1926`.
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

    /// `:1930-1934`.
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

    /// `:1938-1939`.
    #[func]
    pub fn set_vertical_rotation_offset(&mut self, value: f64) {
        self.vertical_rotation_offset = value;
    }

    /// `:1942-1943`.
    #[func]
    pub fn get_vertical_rotation_offset(&self) -> f64 {
        self.vertical_rotation_offset
    }

    /// `:1946-1947`.
    #[func]
    pub fn set_horizontal_rotation_offset(&mut self, value: f64) {
        self.horizontal_rotation_offset = value;
    }

    /// `:1950-1951`.
    #[func]
    pub fn get_horizontal_rotation_offset(&self) -> f64 {
        self.horizontal_rotation_offset
    }

    /// `:1955-1958`.
    ///
    /// parity: `spring_length` writes `follow_distance` (`:1956`), i.e. the two
    /// properties are aliases and `get_spring_length` returns `follow_distance`.
    #[func]
    pub fn set_spring_length(&mut self, value: f64) {
        self.follow_distance = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_length(real_of(value));
        }
    }

    /// `:1962-1963`.
    #[func]
    pub fn get_spring_length(&self) -> f64 {
        self.follow_distance
    }

    /// `:1968-1971`.
    #[func]
    pub fn set_collision_mask(&mut self, value: i64) {
        self.collision_mask = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_collision_mask(value as u32);
        }
    }

    /// `:1975-1978`.
    #[func]
    pub fn set_collision_mask_value(&mut self, layer: i64, enabled: bool) {
        self.collision_mask = set_layer(self.collision_mask, layer, enabled);
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_collision_mask(self.collision_mask as u32);
        }
    }

    /// `:1982-1983`.
    #[func]
    pub fn get_collision_mask(&self) -> i64 {
        self.collision_mask
    }

    /// `:1988-1991`.
    #[func]
    pub fn set_shape(&mut self, value: Option<Gd<Shape3D>>) {
        self.shape = value.clone();
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            match &value {
                Some(shape) => arm.set_shape(shape),
                // `SpringArm3D.shape` is nullable; gdext's setter is not.
                None => arm.set("shape", &Variant::nil()),
            }
        }
    }

    /// `:1994-1995`.
    #[func]
    pub fn get_shape(&self) -> Option<Gd<Shape3D>> {
        self.shape.clone()
    }

    /// `:2000-2003`.
    #[func]
    pub fn set_margin(&mut self, value: f64) {
        self.margin = value;
        if let Some(mut arm) = self.follow_spring_arm.clone() {
            arm.set_margin(real_of(value));
        }
    }

    /// `:2007-2008`.
    #[func]
    pub fn get_margin(&self) -> f64 {
        self.margin
    }

    /// `:2011-2014`.
    #[func]
    pub fn set_draw_follow_line(&mut self, value: bool) {
        self.draw_follow_line = value;
        self.draw_follow_gizmo_line = value;
        self.check_draw_gizmo();
    }

    /// `:2016-2017`.
    #[func]
    pub fn get_draw_follow_line(&self) -> bool {
        self.draw_follow_line
    }

    /// `:2020-2023`.
    #[func]
    pub fn set_draw_look_at_line(&mut self, value: bool) {
        self.draw_look_at_line = value;
        self.draw_look_at_gizmo_line = value;
        self.check_draw_gizmo();
    }

    /// `:2025-2026`.
    #[func]
    pub fn get_draw_look_at_line(&self) -> bool {
        self.draw_look_at_line
    }

    /// `:2032-2033`.
    #[func]
    pub fn get_look_at_mode(&self) -> i64 {
        self.look_at_mode
    }

    /// `:244-274`.
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

    /// `:2037-2053`.
    #[func]
    pub fn set_look_at_target(&mut self, value: Option<Gd<Node3D>>) {
        if self.look_at_mode == 0 {
            return;
        }
        let new_target = value.filter(|target| target.is_instance_valid());
        if let (Some(current), Some(new)) = (self.look_at_target_node(), &new_target) {
            if current.instance_id() == new.instance_id() {
                return;
            }
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

    /// `:2056-2057`.
    #[func]
    pub fn get_look_at_target(&self) -> Option<Gd<Node3D>> {
        self.look_at_target_node()
    }

    /// `:2061-2067`.
    #[func]
    pub fn set_look_at_targets(&mut self, value: Array<Gd<Node3D>>) {
        if self.look_at_mode != 3 {
            return;
        }
        self.look_at_targets = value;
        self.look_at_targets_size_check();
    }

    /// `:2070-2079`.
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

    /// `:2083-2090`.
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

    /// `:2093-2098`.
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

    /// `:2103-2105` — deprecated alias kept for ABI parity.
    #[func]
    pub fn erase_look_at_targets_member(&mut self, value: Gd<Node3D>) {
        godot_warn!(
            "PhantomCamera3D: erase_look_at_targets_member is deprecated, use erase_look_at_targets instead"
        );
        self.erase_look_at_targets(value);
    }

    /// `:2108-2109`.
    #[func]
    pub fn get_look_at_targets(&self) -> Array<Gd<Node3D>> {
        self.look_at_targets.clone()
    }

    /// `:2113-2114`.
    #[func]
    pub fn set_look_at_offset(&mut self, value: Vector3) {
        self.look_at_offset = value;
    }

    /// `:2117-2118`.
    #[func]
    pub fn get_look_at_offset(&self) -> Vector3 {
        self.look_at_offset
    }

    /// `:2122-2124`.
    #[func]
    pub fn set_look_at_damping(&mut self, value: bool) {
        self.look_at_damping = value;
    }

    /// `:2127-2128`.
    #[func]
    pub fn get_look_at_damping(&self) -> bool {
        self.look_at_damping
    }

    /// `:2132-2133`.
    #[func]
    pub fn set_look_at_damping_value(&mut self, value: f64) {
        self.look_at_damping_value = value;
    }

    /// `:2136-2137`.
    #[func]
    pub fn get_look_at_damping_value(&self) -> f64 {
        self.look_at_damping_value
    }

    /// `:2140-2170` (`await ready` becomes [`Self::pending_axis_lock`]).
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

    /// `:2173-2174`.
    #[func]
    pub fn get_follow_axis_lock(&self) -> i64 {
        self.follow_axis_lock
    }

    /// `:2178-2185`.
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

    /// `:2188-2189`.
    #[func]
    pub fn get_up(&self) -> Vector3 {
        self.up
    }

    /// `:2193-2202`.
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

    /// `:2205-2206`.
    #[func]
    pub fn get_up_target(&self) -> Option<Gd<Node3D>> {
        self.up_target_node()
    }

    /// `:2210-2217`.
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise3D>>) {
        self.noise = value;
        self.sync_noise_resource();
    }

    /// `:2219-2220`.
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise3D>> {
        self.noise.clone()
    }

    /// `:2222-2223`.
    #[func]
    pub fn has_noise_resource(&self) -> bool {
        self.has_noise_resource
    }

    /// `:2227-2228`.
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }

    /// `:2231-2232`.
    #[func]
    pub fn set_noise_emitter_layer_value(&mut self, layer: i64, enabled: bool) {
        self.noise_emitter_layer = set_layer(self.noise_emitter_layer, layer, enabled);
    }

    /// `:2235-2236`.
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }

    /// `:2240-2241`.
    #[func]
    pub fn set_inactive_update_mode(&mut self, value: i64) {
        self.inactive_update_mode = value;
    }

    /// `:2244-2245`.
    #[func]
    pub fn get_inactive_update_mode(&self) -> i64 {
        self.inactive_update_mode
    }

    /// `:2249-2254`.
    #[func]
    pub fn set_camera_3d_resource(&mut self, value: Option<Gd<Camera3DResource>>) {
        self.camera_3d_resource = value;
        self.signals().camera_3d_resource_changed().emit();
        self.sync_camera_resource();
    }

    /// `:2257-2258`.
    #[func]
    pub fn get_camera_3d_resource(&self) -> Option<Gd<Camera3DResource>> {
        self.camera_3d_resource.clone()
    }

    /// Helper mirroring the oracle's `camera_3d_resource_property_changed` emit.
    fn emit_resource_property(&mut self, property: &str, value: Variant) {
        self.signals()
            .camera_3d_resource_property_changed()
            .emit(&StringName::from(property), &value);
    }

    /// `:2261-2266`.
    ///
    /// parity: `set_keep_aspect` stores the PCam's own `keep_aspect` shim and emits
    /// the change, but never writes it into the `Camera3DResource` — the host applies
    /// it straight to the `Camera3D` (`phantom_camera_host.gd:682`). Reproduced.
    #[func]
    pub fn set_keep_aspect(&mut self, value: i64) {
        if self.camera_3d_resource.is_none() {
            godot_warn!(
                "PhantomCamera3D: Can't assign a keep_aspect value without a Camera3DResource"
            );
            return;
        }
        self.keep_aspect_value = value;
        self.emit_resource_property("keep_aspect", value.to_variant());
    }

    /// `:2268-2270`.
    #[func]
    pub fn get_keep_aspect(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("keep_aspect"),
            None => Variant::nil(),
        }
    }

    /// `:2276-2281`.
    #[func]
    pub fn set_cull_mask(&mut self, value: i64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a cull_mask value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().write("cull_mask", value.to_variant());
        self.emit_resource_property("cull_mask", value.to_variant());
    }

    /// `:2286-2292`.
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
        resource.bind_mut().write("cull_mask", mask.to_variant());
        self.emit_resource_property("cull_mask", mask.to_variant());
    }

    /// `:2295-2297`.
    #[func]
    pub fn get_cull_mask(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("cull_mask"),
            None => Variant::nil(),
        }
    }

    /// `:2301-2303`.
    #[func]
    pub fn set_environment(&mut self, value: Option<Gd<godot::classes::Environment>>) {
        self.environment = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("environment", emitted);
    }

    /// `:2306-2307`.
    #[func]
    pub fn get_environment(&self) -> Option<Gd<godot::classes::Environment>> {
        self.environment.clone()
    }

    /// `:2311-2313`.
    #[func]
    pub fn set_attributes(&mut self, value: Option<Gd<godot::classes::CameraAttributes>>) {
        self.attributes = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("attributes", emitted);
    }

    /// `:2316-2317`.
    #[func]
    pub fn get_attributes(&self) -> Option<Gd<godot::classes::CameraAttributes>> {
        self.attributes.clone()
    }

    /// `:2321-2322`.
    #[func]
    pub fn set_compositor(&mut self, value: Option<Gd<godot::classes::Resource>>) {
        self.compositor = value.clone();
        let emitted = match value {
            Some(resource) => resource.to_variant(),
            None => Variant::nil(),
        };
        self.emit_resource_property("compositor", emitted);
    }

    /// `:2325-2326`.
    #[func]
    pub fn get_compositor(&self) -> Option<Gd<godot::classes::Resource>> {
        self.compositor.clone()
    }

    /// `:2332-2337`.
    #[func]
    pub fn set_h_offset(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a h_offset value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().write("h_offset", value.to_variant());
        self.emit_resource_property("h_offset", value.to_variant());
    }

    /// `:2340-2342`.
    #[func]
    pub fn get_h_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("h_offset"),
            None => Variant::nil(),
        }
    }

    /// `:2348-2353`.
    #[func]
    pub fn set_v_offset(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a v_offset value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().write("v_offset", value.to_variant());
        self.emit_resource_property("v_offset", value.to_variant());
    }

    /// `:2356-2358`.
    #[func]
    pub fn get_v_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("v_offset"),
            None => Variant::nil(),
        }
    }

    /// `:2364-2369`.
    #[func]
    pub fn set_projection(&mut self, value: i64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!(
                "PhantomCamera3D: Can't assign a projection value without a Camera3DResource"
            );
            return;
        };
        resource.bind_mut().write("projection", value.to_variant());
        self.emit_resource_property("projection", value.to_variant());
    }

    /// `:2372-2374`.
    #[func]
    pub fn get_projection(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("projection"),
            None => Variant::nil(),
        }
    }

    /// `:2380-2385`.
    #[func]
    pub fn set_fov(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a fov value without a Camera3DResource");
            return;
        };
        resource.bind_mut().write("fov", value.to_variant());
        self.emit_resource_property("fov", value.to_variant());
    }

    /// `:2388-2390`.
    #[func]
    pub fn get_fov(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("fov"),
            None => Variant::nil(),
        }
    }

    /// `:2396-2401`.
    #[func]
    pub fn set_size(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a size value without a Camera3DResource");
            return;
        };
        resource.bind_mut().write("size", value.to_variant());
        self.emit_resource_property("size", value.to_variant());
    }

    /// `:2404-2406`.
    #[func]
    pub fn get_size(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("size"),
            None => Variant::nil(),
        }
    }

    /// `:2412-2417`.
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
            .write("frustum_offset", value.to_variant());
        self.emit_resource_property("frustum_offset", value.to_variant());
    }

    /// `:2420-2422`.
    #[func]
    pub fn get_frustum_offset(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("frustum_offset"),
            None => Variant::nil(),
        }
    }

    /// `:2428-2433`.
    #[func]
    pub fn set_near(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a near value without a Camera3DResource");
            return;
        };
        resource.bind_mut().write("near", value.to_variant());
        self.emit_resource_property("near", value.to_variant());
    }

    /// `:2436-2438`.
    #[func]
    pub fn get_near(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("near"),
            None => Variant::nil(),
        }
    }

    /// `:2444-2449`.
    #[func]
    pub fn set_far(&mut self, value: f64) {
        let Some(mut resource) = self.camera_3d_resource.clone() else {
            godot_warn!("PhantomCamera3D: Can't assign a far value without a Camera3DResource");
            return;
        };
        resource.bind_mut().write("far", value.to_variant());
        self.emit_resource_property("far", value.to_variant());
    }

    /// `:2452-2454`.
    #[func]
    pub fn get_far(&self) -> Variant {
        match &self.camera_3d_resource {
            Some(resource) => resource.bind().read("far"),
            None => Variant::nil(),
        }
    }

    /// `:2457-2458`.
    #[func]
    pub fn get_follow_target_physics_based(&self) -> bool {
        self.follow_target_physics_based
    }

    /// `:2462-2463`.
    #[func]
    pub fn draw_follow_gizmo_line(&self) -> bool {
        self.draw_follow_gizmo_line
    }

    /// `:2466-2467`.
    #[func]
    pub fn draw_look_at_gizmo_line(&self) -> bool {
        self.draw_look_at_gizmo_line
    }

    /// `:596-600`.
    #[func]
    pub fn set_viewport_index(&mut self, value: i64) {
        self.viewport_index = value;
    }

    /// `:599-600`.
    #[func]
    pub fn get_viewport_index(&self) -> i64 {
        self.viewport_index
    }

    /// `:437-442`.
    #[func]
    pub fn set_dead_zone_width(&mut self, value: f64) {
        self.dead_zone_width = value;
        self.signals().dead_zone_changed().emit();
    }

    /// `:441-442`.
    #[func]
    pub fn get_dead_zone_width(&self) -> f64 {
        self.dead_zone_width
    }

    /// `:449-454`.
    #[func]
    pub fn set_dead_zone_height(&mut self, value: f64) {
        self.dead_zone_height = value;
        self.signals().dead_zone_changed().emit();
    }

    /// `:453-454`.
    #[func]
    pub fn get_dead_zone_height(&self) -> f64 {
        self.dead_zone_height
    }

    /// `:543-547`.
    #[func]
    pub fn set_preview_noise(&mut self, value: bool) {
        self._preview_noise = value;
        if !value {
            self.transform_noise = Transform3D::IDENTITY;
        }
    }

    /// `:546-547`.
    #[func]
    pub fn get_preview_noise(&self) -> bool {
        self._preview_noise
    }

    // ----- handlers reachable from `Base::callable` -----

    /// `:1363-1364`.
    #[func]
    pub fn pcam_check_visibility(&mut self) {
        self.check_visibility();
    }

    /// `:1367-1372`.
    #[func]
    pub fn pcam_follow_target_tree_exiting(&mut self, target: Gd<Node>) {
        self.follow_target_tree_exiting_impl(&target);
    }

    /// `:1412-1416`.
    #[func]
    pub fn pcam_look_at_target_tree_exiting(&mut self, target: Gd<Node>) {
        self.look_at_target_tree_exiting_impl(&target);
    }

    /// `:1418-1420`.
    #[func]
    pub fn pcam_up_target_tree_exiting(&mut self) {
        self.up_target_tree_exiting_impl();
    }

    /// `:1485-1487`.
    #[func]
    pub fn pcam_camera_resource_changed(&mut self) {
        self.camera_resource_changed_impl();
    }

    /// `:935-941` — continuation of `await _follow_spring_arm.ready`.
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
            follow_damping_value: Vector3::new(0.1, 0.1, 0.1),
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
            margin: 0.01,
            look_at_offset: Vector3::ZERO,
            look_at_damping: false,
            look_at_damping_value: 0.25,
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
            look_at_target_physics_based: false,
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
            keep_aspect_value: 1,
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
