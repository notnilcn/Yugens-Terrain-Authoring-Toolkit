// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/phantom_camera/phantom_camera_2d.gd
//
//! `PhantomCamera2D` — the 2D camera rig node.
//!
//! Ported from `addons/phantom_camera/scripts/phantom_camera/phantom_camera_2d.gd`
//! (v0.11.0.3, MIT, Marcus Skov), which is preserved as the read-only parity
//! oracle. All `:NNN` anchors below are lines of that file.
//!
//! Algorithm anchors:
//! - lifecycle `:706-748`, scheduling `:750-757`, `process_logic` `:760-789`
//! - follow `:799-801`, `_set_follow_position` `:804-905`,
//!   `_interpolate_position` `:1005-1041`, `_smooth_damp` `:1044-1065`
//! - FRAMED incremental dead zone `:854-904`
//! - lookahead `:914-1002`
//! - limits `:1068-1072`, `update_limit_all_sides` `:1223-1287`
//! - priority/tween-skip plumbing `:1302-1310`, `:1359-1363`
//!
//! Deliberately omitted (editor-only, never runs in a build):
//! `_validate_property` inspector hiding `:589-703`, the `_draw` frame preview
//! `:1075-1086`, `_draw_camera_2d_limit` `:1100-1102`, and the
//! `Engine.is_editor_hint()` branch of `_limit_checker` `:792-796`.
//!
//! `show_viewfinder_in_play` is stored for ABI parity but inert: the viewfinder
//! panel is out of scope for this port.

// The if/if let nesting below deliberately mirrors the oracle's branch structure
// so every block keeps a 1:1 line anchor to the GDScript; collapsing the branches
// (clippy::collapsible_if) would destroy that correspondence.
#![allow(clippy::collapsible_if)]
use godot::builtin::{Array, NodePath, Rect2, Transform2D, Vector2, Vector2i, Vector4, Vector4i};
use godot::classes::{CollisionShape2D, Engine, Node2D, Path2D, TileMapLayer};
use godot::prelude::*;

use crate::abi::{
    LIMIT_BOTTOM_DEFAULT, LIMIT_LEFT_DEFAULT, LIMIT_RIGHT_DEFAULT, LIMIT_TOP_DEFAULT, PcamKind,
    TWEEN_DEFAULT_DURATION, TWEEN_DEFAULT_EASE, TWEEN_DEFAULT_TRANSITION, set_layer,
};
use crate::manager::PhantomCameraManager;
use crate::resources::{PhantomCameraNoise2D, PhantomCameraTween};

/// Godot's `Side` enum, used by `set_limit`/`get_limit` and the manager's
/// `limit_2d_changed` dispatch (`phantom_camera_2d.gd:1283-1286`).
pub const SIDE_LEFT: i64 = 0;
pub const SIDE_TOP: i64 = 1;
pub const SIDE_RIGHT: i64 = 2;
pub const SIDE_BOTTOM: i64 = 3;

/// `draw_limits` is a `static var` in the oracle (`:549`): one flag shared by every
/// instance. Mirrored as an atomic to keep the shared semantics without `unsafe`.
static DRAW_LIMITS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Godot `clampf` — unlike `f64::clamp` this never panics when `min > max`.
fn clampf(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

fn clampf32(value: real, min: real, max: real) -> real {
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

/// Godot `signf` — returns `0.0` for both zeros, unlike `f64::signum`.
fn signf(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `Vector2[i]` as the oracle writes it (gdext needs `Vector2Axis`, not an int).
fn axis2(value: &Vector2, index: usize) -> real {
    if index == 0 { value.x } else { value.y }
}

fn set_axis2(value: &mut Vector2, index: usize, component: real) {
    if index == 0 {
        value.x = component;
    } else {
        value.y = component;
    }
}

/// Godot `Math::roundi`.
fn roundi(value: real) -> i32 {
    let value = f64_of(value);
    if value >= 0.0 {
        (value + 0.5).floor() as i32
    } else {
        (value - 0.5).ceil() as i32
    }
}

#[derive(GodotClass)]
#[class(base = Node2D, tool)]
pub struct PhantomCamera2D {
    base: Base<Node2D>,

    // ----- exported surface (`#region Exported Properties`, `:103-510`) -----
    /// `:110-121`.
    #[export]
    #[var(set = set_priority_override, get = get_priority_override)]
    pub priority_override: bool,
    /// `:133-135` — plain `int`, `PROPERTY_HINT_NONE`.
    #[export]
    #[var(set = set_priority, get = get_priority)]
    pub priority: i64,
    /// `:142-178` — `PROPERTY_HINT_ENUM`, hint string per the ABI dump.
    #[export]
    #[var(
        set = set_follow_mode,
        get = get_follow_mode,
        hint = ENUM,
        hint_string = "None:0,Glued:1,Simple:2,Group:3,Path:4,Framed:5"
    )]
    pub follow_mode: i64,
    /// `:183-185`.
    #[export]
    #[var(set = set_follow_target, get = get_follow_target)]
    pub follow_target: Option<Gd<Node2D>>,
    /// `:188-190`.
    #[export]
    #[var(set = set_follow_targets, get = get_follow_targets)]
    pub follow_targets: Array<Gd<Node2D>>,
    /// `:196-198`.
    #[export]
    #[var(set = set_follow_path, get = get_follow_path)]
    pub follow_path: Option<Gd<Path2D>>,
    /// `:203-205` — `PROPERTY_HINT_LINK`.
    #[export]
    #[var(set = set_zoom, get = get_zoom, hint = LINK)]
    pub zoom: Vector2,
    /// `:213-215`.
    #[export]
    #[var(set = set_snap_to_pixel, get = get_snap_to_pixel)]
    pub snap_to_pixel: bool,
    /// `:223-228`.
    #[export]
    pub frame_preview: bool,
    /// `:238-240` — the oracle defaults this to a fresh `PhantomCameraTween`.
    #[export]
    #[var(set = set_tween_resource, get = get_tween_resource)]
    pub tween_resource: Option<Gd<PhantomCameraTween>>,
    /// `:248-250`.
    #[export]
    #[var(set = set_tween_on_load, get = get_tween_on_load)]
    pub tween_on_load: bool,
    /// `:257` — `PROPERTY_HINT_ENUM`.
    #[export]
    #[var(
        set = set_inactive_update_mode,
        get = get_inactive_update_mode,
        hint = ENUM,
        hint_string = "Always:0,Never:1"
    )]
    pub inactive_update_mode: i64,
    /// `:262-264`.
    #[export(flags_2d_render)]
    #[var(set = set_host_layers, get = get_host_layers)]
    pub host_layers: i64,
    /// `:269-271`.
    #[export]
    #[var(set = set_follow_offset, get = get_follow_offset)]
    pub follow_offset: Vector2,
    /// `:276-278`.
    #[export]
    #[var(set = set_follow_damping, get = get_follow_damping)]
    pub follow_damping: bool,
    /// `:284-287` — `PROPERTY_HINT_LINK`.
    #[export]
    #[var(set = set_follow_damping_value, get = get_follow_damping_value, hint = LINK)]
    pub follow_damping_value: Vector2,
    /// `:291-293` — `PROPERTY_HINT_ENUM`.
    #[export]
    #[var(
        set = set_lock_axis,
        get = get_lock_axis,
        hint = ENUM,
        hint_string = "None:0,X:1,Y:2,Xy:3"
    )]
    pub follow_axis_lock: i64,
    /// `:300-303`.
    #[export]
    #[var(set = set_rotate_with_target, get = get_rotate_with_target)]
    pub rotate_with_target: bool,
    /// `:306-308` — `PROPERTY_HINT_RANGE`, radians-as-degrees.
    #[export]
    #[var(
        set = set_rotation_offset,
        get = get_rotation_offset,
        hint = RANGE,
        hint_string = "-360.0,360.0,0.001,radians_as_degrees"
    )]
    pub rotation_offset: f64,
    /// `:311-313`.
    #[export]
    #[var(set = set_rotation_damping, get = get_rotation_damping)]
    pub rotation_damping: bool,
    /// `:316-318` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_rotation_damping_value,
        get = get_rotation_damping_value,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub rotation_damping_value: f64,
    /// `:322-324`.
    #[export]
    #[var(set = set_lookahead, get = get_lookahead)]
    pub lookahead: bool,
    /// `:331-333` — `PROPERTY_HINT_LINK` with a seconds suffix.
    #[export]
    #[var(
        set = set_lookahead_time,
        get = get_lookahead_time,
        hint = LINK,
        hint_string = "suffix:s"
    )]
    pub lookahead_time: Vector2,
    /// `:339-341` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_lookahead_acceleration,
        get = get_lookahead_acceleration,
        hint = RANGE,
        hint_string = "0.0,1.0,0.001,or_greater"
    )]
    pub lookahead_acceleration: f64,
    /// `:347-349` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_lookahead_deceleration,
        get = get_lookahead_deceleration,
        hint = RANGE,
        hint_string = "0.0,1.0,0.001,or_greater"
    )]
    pub lookahead_deceleration: f64,
    /// `:354-356`.
    #[export]
    #[var(set = set_lookahead_max, get = get_lookahead_max)]
    pub lookahead_max: bool,
    /// `:360-362` — `PROPERTY_HINT_LINK` with a `px/s` suffix.
    #[export]
    #[var(
        set = set_lookahead_max_value,
        get = get_lookahead_max_value,
        hint = LINK,
        hint_string = "suffix:px/s"
    )]
    pub lookahead_max_value: Vector2,
    /// `:373-375`.
    #[export]
    #[var(set = set_auto_zoom, get = get_auto_zoom)]
    pub auto_zoom: bool,
    /// `:380-382`.
    #[export]
    #[var(set = set_auto_zoom_min, get = get_auto_zoom_min)]
    pub auto_zoom_min: f64,
    /// `:387-389`.
    #[export]
    #[var(set = set_auto_zoom_max, get = get_auto_zoom_max)]
    pub auto_zoom_max: f64,
    /// `:396-398`.
    #[export]
    #[var(set = set_auto_zoom_margin, get = get_auto_zoom_margin)]
    pub auto_zoom_margin: Vector4,
    /// `:407-412` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_dead_zone_width,
        get = get_dead_zone_width,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_width: f64,
    /// `:419-424` — `PROPERTY_HINT_RANGE`.
    #[export]
    #[var(
        set = set_dead_zone_height,
        get = get_dead_zone_height,
        hint = RANGE,
        hint_string = "0.0,1.0"
    )]
    pub dead_zone_height: f64,
    /// `:429` — inert (viewfinder panel out of scope).
    #[export]
    pub show_viewfinder_in_play: bool,
    /// `:435-441`.
    #[export]
    #[var(set = set_draw_limits, get = get_draw_limits)]
    pub draw_limits: bool,
    /// `:445-447`.
    #[export]
    #[var(set = set_limit_left, get = get_limit_left)]
    pub limit_left: i64,
    /// `:450-452`.
    #[export]
    #[var(set = set_limit_top, get = get_limit_top)]
    pub limit_top: i64,
    /// `:455-457`.
    #[export]
    #[var(set = set_limit_right, get = get_limit_right)]
    pub limit_right: i64,
    /// `:460-462`.
    #[export]
    #[var(set = set_limit_bottom, get = get_limit_bottom)]
    pub limit_bottom: i64,
    /// `:475-477` — `PROPERTY_HINT_NODE_PATH_VALID_TYPES`, hint string
    /// `TileMapLayer,CollisionShape2D`.
    #[export]
    #[var(
        set = set_limit_target,
        get = get_limit_target,
        hint = NODE_PATH_VALID_TYPES,
        hint_string = "TileMapLayer,CollisionShape2D"
    )]
    pub limit_target: NodePath,
    /// `:482-484`.
    #[export]
    #[var(set = set_limit_margin, get = get_limit_margin)]
    pub limit_margin: Vector4i,
    /// `:492-494`.
    #[export]
    #[var(set = set_noise, get = get_noise)]
    pub noise: Option<Gd<PhantomCameraNoise2D>>,
    /// `:500-504`.
    #[export]
    #[var(set = set_preview_noise, get = get_preview_noise)]
    pub _preview_noise: bool,
    /// `:508-510`.
    #[export(flags_2d_render)]
    #[var(set = set_noise_emitter_layer, get = get_noise_emitter_layer)]
    pub noise_emitter_layer: i64,

    // ----- private state (`#region Private Variables`, `:512-569`) -----
    is_active_pcam: bool,
    should_follow: bool,
    follow_target_physics_based: bool,
    follow_target_physics_class: i64,
    character_body_2d: Option<Gd<godot::classes::CharacterBody2D>>,
    rigid_body_2d: Option<Gd<godot::classes::RigidBody2D>>,
    has_multiple_follow_targets: bool,
    follow_targets_single_target_index: i64,
    /// Rust mirror of the oracle's `_follow_targets` (`:526`).
    follow_targets_cache: Vec<Gd<Node2D>>,
    follow_velocity_ref: Vector2,
    rotation_velocity_ref: f64,
    has_lookahead_acceleration: bool,
    has_lookahead_deceleration: bool,
    tween_skip: bool,
    follow_framed_initial_set: bool,
    lookahead_offset: Vector2,
    lookahead_offset_velocity_ref: Vector2,
    lookahead_sample_pos_prev: Vector2,
    lookahead_has_prev_sample: bool,
    follow_axis_is_locked: bool,
    follow_axis_lock_value: Vector2,
    limit_sides: Vector4i,
    limit_node: Option<Gd<Node2D>>,
    limit_inactive_pcam: bool,
    follow_target_position: Vector2,
    transform_output: Transform2D,
    transform_noise: Transform2D,
    has_noise_resource: bool,
    /// Pending `noise_emitted` payload for the host (see `take_noise_output`).
    pending_noise: Option<Transform2D>,
    /// `follow_axis_lock` assigned before the node was ready (`:1610-1628`).
    pending_axis_lock: Option<i64>,

    // ----- public non-exported script variables (`:571-585`) -----
    tween_duration_fallback: f64,
    tween_transition_fallback: i64,
    tween_ease_fallback: i64,
    pub viewport_position: Vector2,
}

#[godot_api]
impl PhantomCamera2D {
    // ------------------------------------------------------------------
    // Signals (`#region Signals`, `:19-61`)
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
    fn dead_zone_changed();
    /// 2D's `dead_zone_reached` carries the touched sides as `±1` components
    /// (`:39`, emitted at `:899-900`).
    #[signal]
    fn dead_zone_reached(side: Vector2);
    #[signal]
    fn tween_started();
    #[signal]
    fn is_tweening();
    #[signal]
    fn tween_interrupted(pcam: Gd<PhantomCamera2D>);
    #[signal]
    fn tween_completed();
    #[signal]
    fn noise_emitted(noise_output: Transform2D);
    #[signal]
    fn physics_target_changed();

    /// ABI discriminator replacing the oracle's `get_class()` override
    /// (`:1961-1966`); see `PORT-PLAN.md` §Cross-class contract.
    #[func]
    pub fn pcam_kind(&self) -> GString {
        GString::from(PcamKind::TwoD.as_str())
    }

    // ------------------------------------------------------------------
    // Typed seams used by PhantomCameraHost / PhantomCameraManager.
    // ------------------------------------------------------------------

    /// `pcam.host_layers & host_layers != 0` (`phantom_camera_host.gd:364-366`).
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

    /// `set_is_active(self, value)` without the host type check (`:1424-1429`).
    pub fn host_set_active(&mut self, value: bool) {
        self.is_active_pcam = value;
    }

    /// `set_tween_skip` without the host type check (`:1302-1310`).
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

    pub fn emit_tween_interrupted(&mut self, pcam: &Gd<PhantomCamera2D>) {
        self.signals().tween_interrupted().emit(pcam);
    }

    /// Drains the last `noise_emitted` payload.
    ///
    /// The oracle connects `noise_emitted` to the host (`phantom_camera_host.gd:517-518`);
    /// the port keeps cross-class traffic typed, so the host pulls instead. Same
    /// observable result: the payload is latched until the host consumes it.
    pub fn take_noise_output(&mut self) -> Option<Transform2D> {
        self.pending_noise.take()
    }

    /// Manager fan-out for `PhantomCameraNoiseEmitter2D` output (`:1161-1163`).
    pub fn pcam_noise_emitted(&mut self, emitter_noise_output: Transform2D, emitter_layer: i64) {
        if self.noise_emitter_layer & emitter_layer != 0 {
            self.signals().noise_emitted().emit(emitter_noise_output);
            self.pending_noise = Some(emitter_noise_output);
        }
    }

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    /// `:706-730`.
    fn enter_tree_impl(&mut self) {
        self.tween_skip = !self.tween_on_load;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_pcam_added(PcamKind::TwoD, &this);
        // `:712` assigns through the property setter.
        self.set_priority_override(false);

        match self.follow_mode {
            0 => self.is_parents_physics(None),
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

        let callable = self.base().callable("pcam_check_visibility");
        if !self.base().is_connected("visibility_changed", &callable) {
            self.base_mut().connect("visibility_changed", &callable);
        }

        // Single-target modes only reached `_check_physics_body` through the oracle's
        // `set_follow_target` (`:1469-1489`), which the scene loader called before
        // `_ready`. Resolve it here instead so the idle/physics split is correct.
        if matches!(self.follow_mode, 1 | 2 | 5) {
            if let Some(target) = self.follow_target_node() {
                self.check_physics_body(&target);
            }
        }

        self.update_limit_all_sides();
    }

    /// `:733-738`.
    fn exit_tree_impl(&mut self) {
        // parity: `:734-735` assigns `follow_targets = []` through
        // `set_follow_targets`, which returns early unless the mode is GROUP — so
        // the oracle line is a no-op. Kept as a no-op.
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_pcam_removed(PcamKind::TwoD, &this);
    }

    /// `:741-748`.
    fn ready_impl(&mut self) {
        self.transform_output = self.base().get_global_transform();
        if !Engine::singleton().is_editor_hint() {
            self._preview_noise = true;
        }
        // Scene-assigned references resolve lazily (their oracle setters ran before
        // `_ready`); re-apply the side effects that matter at runtime.
        self.sync_noise_resource();
        self.resolve_limit_target();
        if let Some(lock) = self.pending_axis_lock.take() {
            self.apply_axis_lock(lock);
        }
        self.transform_output = self.base().get_global_transform();
    }

    /// `:750-752` — an ACTIVE pcam never self-ticks; the host pumps `process_logic`.
    fn process_impl(&mut self, delta: f64) {
        if self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }

    /// `:755-757`.
    fn physics_process_impl(&mut self, delta: f64) {
        if !self.follow_target_physics_based || self.is_active_pcam {
            return;
        }
        self.process_logic(delta);
    }

    /// `:760-789`.
    #[func]
    pub fn process_logic(&mut self, delta: f64) {
        if self.is_active_pcam {
            if self.has_noise_resource && self._preview_noise {
                self.transform_noise = self.noise_transform(delta);
            }
        } else {
            match self.inactive_update_mode {
                1 => return,
                _ => {
                    if self.limit_inactive_pcam {
                        let pos = self.base().get_global_position();
                        let clamped = self.set_limit_clamp_position(pos);
                        self.base_mut().set_global_position(clamped);
                    }
                }
            }
        }

        // `_limit_checker()` (`:792-796`) is gated on `Engine.is_editor_hint()`: omitted.

        if self.should_follow {
            self.follow(delta);
        } else {
            self.transform_output = self.base().get_global_transform();
        }

        if self.follow_axis_is_locked {
            match self.follow_axis_lock {
                1 => self.transform_output.origin.x = self.follow_axis_lock_value.x,
                2 => self.transform_output.origin.y = self.follow_axis_lock_value.y,
                3 => {
                    self.transform_output.origin.x = self.follow_axis_lock_value.x;
                    self.transform_output.origin.y = self.follow_axis_lock_value.y;
                }
                _ => {}
            }
        }
    }

    /// `:799-801`.
    fn follow(&mut self, delta: f64) {
        self.set_follow_position(delta);
        let target = self.follow_target_position;
        self.interpolate_position(target, delta);
    }

    /// `:804-905`.
    ///
    /// parity: the FRAMED branch (`:854-904`) mutates the *persistent*
    /// `_follow_target_position` incrementally (`-=` against the previous frame's
    /// value) instead of recomputing it. That statefulness is what makes the dead
    /// zone sticky, so it is reproduced verbatim.
    fn set_follow_position(&mut self, delta: f64) {
        let editor = Engine::singleton().is_editor_hint();
        match self.follow_mode {
            1 => {
                if let Some(target) = self.follow_target_node() {
                    self.follow_target_position = target.get_global_position();
                    if self.lookahead && !editor {
                        let pos = self.follow_target_position;
                        self.follow_target_position = self.apply_lookahead(pos, delta);
                    }
                }
            }
            2 => {
                self.follow_target_position = self.target_position_offset();
                if self.lookahead && !editor {
                    let pos = self.follow_target_position;
                    self.follow_target_position = self.apply_lookahead(pos, delta);
                }
            }
            3 => self.set_follow_position_group(delta),
            4 => {
                if let Some(path) = self.valid_follow_path() {
                    let path_position = path.get_global_position();
                    let mut follow_position = self.target_position_offset();
                    if self.lookahead && !editor {
                        follow_position = self.apply_lookahead(follow_position, delta);
                    }
                    let local = follow_position - path_position;
                    let closest: Vector2 = match path.get_curve() {
                        Some(curve) => curve.get_closest_point(local),
                        None => local,
                    };
                    self.follow_target_position = closest + path_position;
                }
            }
            5 => self.set_follow_position_framed(delta, editor),
            _ => {}
        }
    }

    /// `:816-840`.
    fn set_follow_position_group(&mut self, delta: f64) {
        let editor = Engine::singleton().is_editor_hint();
        if self.has_multiple_follow_targets {
            let mut rect = match self.follow_targets_cache.first() {
                Some(first) => Rect2::new(first.get_global_position(), Vector2::ZERO),
                None => Rect2::new(Vector2::ZERO, Vector2::ZERO),
            };
            for target in &self.follow_targets_cache {
                rect = rect.expand(target.get_global_position());
            }
            if self.auto_zoom {
                rect = rect.grow_individual(
                    self.auto_zoom_margin.x,
                    self.auto_zoom_margin.y,
                    self.auto_zoom_margin.z,
                    self.auto_zoom_margin.w,
                );
                let screen = PhantomCameraManager::screen_size();
                let screen_x = f64_of(screen.x as real);
                let screen_y = f64_of(screen.y as real);
                let aspect = if screen_y == 0.0 {
                    0.0
                } else {
                    screen_x / screen_y
                };
                let size_x = f64_of(rect.size.x);
                let size_y = f64_of(rect.size.y);
                let zoomed = if size_x > size_y * aspect {
                    if size_x == 0.0 {
                        self.auto_zoom_max
                    } else {
                        clampf(screen_x / size_x, self.auto_zoom_min, self.auto_zoom_max)
                    }
                } else if size_y == 0.0 {
                    self.auto_zoom_max
                } else {
                    clampf(screen_y / size_y, self.auto_zoom_min, self.auto_zoom_max)
                };
                self.zoom = Vector2::new(real_of(zoomed), real_of(zoomed));
            }
            let mut target_position = rect.center() + self.follow_offset;
            if self.lookahead && !editor {
                target_position = self.apply_lookahead(target_position, delta);
            }
            self.follow_target_position = target_position;
        } else {
            let index = self.follow_targets_single_target_index.max(0) as usize;
            if let Some(target) = self.follow_targets_cache.get(index) {
                self.follow_target_position = target.get_global_position() + self.follow_offset;
            }
        }
    }

    /// `:854-904` — the incremental FRAMED dead zone.
    fn set_follow_position_framed(&mut self, delta: f64, editor: bool) {
        if editor || !self.is_active_pcam {
            self.follow_target_position = self.target_position_offset();
            return;
        }

        let mut target_position = self.target_position_offset();
        let viewport_rect = self.base().get_viewport_rect().size;
        if self.lookahead && !editor {
            target_position = self.apply_lookahead(target_position, delta);
        }

        let half = viewport_rect / 2.0;
        let dead_zone_x = if self.zoom.x == 0.0 {
            0.0
        } else {
            half.x / self.zoom.x * real_of(self.dead_zone_width)
        };
        let dead_zone_y = if self.zoom.y == 0.0 {
            0.0
        } else {
            half.y / self.zoom.y * real_of(self.dead_zone_height)
        };

        let global_position = self.base().get_global_position();
        let mut viewport_target_offset = Vector2::ZERO;
        let mut side_reached = Vector2i::ZERO;

        let left_dead_zone = global_position.x - dead_zone_x;
        let right_dead_zone = global_position.x + dead_zone_x;
        if left_dead_zone > target_position.x {
            viewport_target_offset.x = left_dead_zone - target_position.x;
            self.follow_target_position.x -= viewport_target_offset.x;
            side_reached.x = 1;
        } else if right_dead_zone < target_position.x {
            viewport_target_offset.x = right_dead_zone - target_position.x;
            self.follow_target_position.x -= viewport_target_offset.x;
            side_reached.x = -1;
        }

        let top_dead_zone = global_position.y - dead_zone_y;
        let bottom_dead_zone = global_position.y + dead_zone_y;
        if top_dead_zone > target_position.y {
            viewport_target_offset.y = top_dead_zone - target_position.y;
            self.follow_target_position.y -= viewport_target_offset.y;
            side_reached.y = 1;
        } else if bottom_dead_zone < target_position.y {
            viewport_target_offset.y = bottom_dead_zone - target_position.y;
            self.follow_target_position.y -= viewport_target_offset.y;
            side_reached.y = -1;
        }

        if side_reached != Vector2i::ZERO {
            self.signals()
                .dead_zone_reached()
                .emit(Vector2::new(side_reached.x as real, side_reached.y as real));
        }

        if self.show_viewfinder_in_play {
            let mut local = Transform2D::IDENTITY;
            local.origin = target_position + viewport_target_offset;
            let canvas = match self.base().get_viewport() {
                Some(viewport) => viewport.get_canvas_transform(),
                None => Transform2D::IDENTITY,
            };
            let screen = (canvas * local).origin;
            self.viewport_position = Vector2::new(
                if viewport_rect.x == 0.0 {
                    0.0
                } else {
                    screen.x / viewport_rect.x
                },
                if viewport_rect.y == 0.0 {
                    0.0
                } else {
                    screen.y / viewport_rect.y
                },
            );
        }
    }

    /// `:1005-1041`.
    fn interpolate_position(&mut self, mut target_position: Vector2, delta: f64) {
        let editor = Engine::singleton().is_editor_hint();
        let mut output_rotation = self.base().get_global_transform().rotation();

        if self.rotate_with_target {
            if let Some(target) = self.follow_target_node() {
                if self.rotation_damping && !editor {
                    let (output, velocity) = crate::math::smooth_damp(
                        f64_of(target.get_rotation()) + self.rotation_offset,
                        f64_of(self.transform_output.rotation()),
                        self.rotation_velocity_ref,
                        self.rotation_damping_value,
                        delta,
                    );
                    self.rotation_velocity_ref = velocity;
                    output_rotation = real_of(output);
                } else {
                    output_rotation = target.get_rotation() + real_of(self.rotation_offset);
                }
            } else {
                godot_warn!("PhantomCamera2D: rotate_with_target without a valid follow_target");
            }
        }

        // `:1022` — the raw target position is applied to the PCam itself.
        self.base_mut().set_global_position(target_position);

        if !editor && self.follow_damping {
            let current = self.base().get_global_position();
            for i in 0..2 {
                let (output, velocity) = crate::math::smooth_damp(
                    f64_of(axis2(&current, i)),
                    f64_of(axis2(&self.transform_output.origin, i)),
                    f64_of(axis2(&self.follow_velocity_ref, i)),
                    f64_of(axis2(&self.follow_damping_value, i)),
                    delta,
                );
                set_axis2(&mut self.follow_velocity_ref, i, real_of(velocity));
                set_axis2(&mut target_position, i, real_of(output));
            }
        }

        if self.limit_inactive_pcam && !self.tween_skip {
            target_position = self.set_limit_clamp_position(target_position);
        }

        self.transform_output = Transform2D::from_angle_origin(output_rotation, target_position);
    }

    /// `:966-1002`.
    fn apply_lookahead(&mut self, target_position: Vector2, delta: f64) -> Vector2 {
        let mut velocity = self.get_follow_target_velocity(target_position, delta);

        if self.lookahead_max {
            velocity = Vector2::new(
                clampf32(
                    velocity.x,
                    -self.lookahead_max_value.x,
                    self.lookahead_max_value.x,
                ),
                clampf32(
                    velocity.y,
                    -self.lookahead_max_value.y,
                    self.lookahead_max_value.y,
                ),
            );
        }

        let desired = Vector2::new(
            velocity.x * self.lookahead_time.x,
            velocity.y * self.lookahead_time.y,
        );
        for i in 0..2 {
            let desired_axis = f64_of(axis2(&desired, i));
            let offset_axis = f64_of(axis2(&self.lookahead_offset, i));
            // `:978` — accelerating toward `desired` vs decelerating away from it.
            let climbing = signf(desired_axis - offset_axis) == signf(desired_axis);
            let (smooth, smooth_time) = if climbing {
                (self.has_lookahead_acceleration, self.lookahead_acceleration)
            } else {
                (self.has_lookahead_deceleration, self.lookahead_deceleration)
            };

            if smooth {
                let (output, velocity_out) = crate::math::smooth_damp(
                    desired_axis,
                    offset_axis,
                    f64_of(axis2(&self.lookahead_offset_velocity_ref, i)),
                    smooth_time,
                    delta,
                );
                set_axis2(&mut self.lookahead_offset, i, real_of(output));
                set_axis2(
                    &mut self.lookahead_offset_velocity_ref,
                    i,
                    real_of(velocity_out),
                );
            } else {
                set_axis2(&mut self.lookahead_offset, i, axis2(&desired, i));
            }
        }

        target_position + self.lookahead_offset
    }

    /// `:924-963`.
    fn get_follow_target_velocity(&mut self, target_position: Vector2, delta: f64) -> Vector2 {
        match self.follow_target_physics_class {
            0 => {
                if let Some(body) = &self.character_body_2d {
                    return body.get_velocity();
                }
            }
            1 => {
                if let Some(body) = &self.rigid_body_2d {
                    return body.get_linear_velocity();
                }
            }
            _ => {
                if self.follow_mode != 3 {
                    if let Some(target) = self.follow_target_node() {
                        let mut object = target.clone().upcast::<godot::classes::Object>();
                        if object.has_method("get_velocity") {
                            if let Ok(value) = object.call("get_velocity", &[]).try_to::<Vector2>()
                            {
                                return value;
                            }
                        }
                        if let Ok(value) = object.get("velocity").try_to::<Vector2>() {
                            return value;
                        }
                    }
                }
            }
        }

        if !self.lookahead_has_prev_sample {
            self.lookahead_has_prev_sample = true;
            self.lookahead_sample_pos_prev = target_position;
            return Vector2::ZERO;
        }

        let current_position = if self.follow_mode == 3 {
            if self.has_multiple_follow_targets {
                target_position
            } else {
                let index = self.follow_targets_single_target_index.max(0) as usize;
                match self.follow_targets_cache.get(index) {
                    Some(target) => target.get_global_position() + self.follow_offset,
                    None => target_position,
                }
            }
        } else {
            self.target_position_offset()
        };

        let divisor = real_of(delta.max(0.0001));
        let velocity = (current_position - self.lookahead_sample_pos_prev) / divisor;
        self.lookahead_sample_pos_prev = current_position;
        velocity
    }

    /// `:914-918`.
    fn reset_lookahead(&mut self) {
        self.lookahead_offset = Vector2::ZERO;
        self.lookahead_offset_velocity_ref = Vector2::ZERO;
        self.lookahead_has_prev_sample = false;
    }

    /// `:1092-1093`.
    fn target_position_offset(&self) -> Vector2 {
        match self.follow_target_node() {
            Some(target) => target.get_global_position() + self.follow_offset,
            None => self.follow_offset,
        }
    }

    /// `:1068-1072`.
    fn set_limit_clamp_position(&self, mut value: Vector2) -> Vector2 {
        let frame = self.camera_frame_rect().size;
        value.x = clampf32(
            value.x,
            self.limit_sides.x as real + frame.x / 2.0,
            self.limit_sides.z as real - frame.x / 2.0,
        );
        value.y = clampf32(
            value.y,
            self.limit_sides.y as real + frame.y / 2.0,
            self.limit_sides.w as real - frame.y / 2.0,
        );
        value
    }

    /// `:1082-1086` — the `_draw` consumer of this rect is editor-only and omitted.
    fn camera_frame_rect(&self) -> Rect2 {
        let screen = PhantomCameraManager::screen_size();
        let zoom = self.get_zoom();
        let size = Vector2::new(
            if zoom.x == 0.0 {
                0.0
            } else {
                screen.x as real / zoom.x
            },
            if zoom.y == 0.0 {
                0.0
            } else {
                screen.y as real / zoom.y
            },
        );
        Rect2::new(-size / 2.0, size)
    }

    /// `:1105-1109`.
    fn check_limit_is_not_default(&mut self) {
        let default = Vector4i::new(
            LIMIT_LEFT_DEFAULT as i32,
            LIMIT_TOP_DEFAULT as i32,
            LIMIT_RIGHT_DEFAULT as i32,
            LIMIT_BOTTOM_DEFAULT as i32,
        );
        self.limit_inactive_pcam = self.limit_sides != default;
    }

    /// `:1112-1114` — connected to `visibility_changed` in `_enter_tree`.
    fn check_visibility(&mut self) {
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_visibility_changed(&this);
    }

    /// `:1116-1121`.
    fn follow_target_tree_exiting_impl(&mut self, target: &Gd<Node>) {
        if let Some(current) = self.follow_target_node() {
            if current.instance_id() == target.instance_id() {
                self.should_follow = false;
            }
        }
        self.follow_targets_cache
            .retain(|cached| cached.instance_id() != target.instance_id());
    }

    /// `:1123-1133`.
    fn should_follow_checker(&mut self) {
        if self.follow_mode == 0 {
            self.should_follow = false;
            return;
        }
        if self.follow_mode != 3 {
            self.should_follow = self.follow_target_node().is_some();
        }
    }

    /// `:1135-1158`.
    fn follow_targets_size_check(&mut self) {
        let mut targets_size = 0i64;
        self.follow_target_physics_based = false;
        self.follow_targets_cache.clear();
        let targets: Vec<Gd<Node2D>> = self.follow_targets.iter_shared().collect();
        for (i, target) in targets.iter().enumerate() {
            if !target.is_instance_valid() {
                continue;
            }
            self.follow_targets_cache.push(target.clone());
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

    /// `:1181-1206`. The project-setting "jitter tips" `printerr` is editor advice;
    /// only the state transition is load bearing.
    fn check_physics_body(&mut self, target: &Gd<Node2D>) {
        self.character_body_2d = None;
        self.rigid_body_2d = None;
        self.follow_target_physics_class = 2;

        if let Ok(character) = target.clone().try_cast::<godot::classes::CharacterBody2D>() {
            self.character_body_2d = Some(character);
            self.follow_target_physics_class = 0;
            self.follow_target_physics_based = true;
        } else if let Ok(rigid) = target.clone().try_cast::<godot::classes::RigidBody2D>() {
            self.rigid_body_2d = Some(rigid);
            self.follow_target_physics_class = 1;
            self.follow_target_physics_based = true;
        } else {
            let this = self.base().clone().upcast::<Node>();
            self.is_parents_physics(Some(&this));
        }
        self.signals().physics_target_changed().emit();
    }

    /// `:1209-1214`.
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
                    .try_cast::<godot::classes::PhysicsBody2D>()
                    .is_ok()
                {
                    self.follow_target_physics_based = true;
                }
            }
            current = parent;
        }
    }

    /// `:1610-1628` — the `await ready` becomes a deferred apply via
    /// `pending_axis_lock`.
    fn apply_axis_lock(&mut self, value: i64) {
        let editor = Engine::singleton().is_editor_hint();
        if value != 0 && !editor {
            self.follow_axis_is_locked = true;
            match value {
                1 => self.follow_axis_lock_value.x = self.transform_output.origin.x,
                2 => self.follow_axis_lock_value.y = self.transform_output.origin.y,
                3 => {
                    self.follow_axis_lock_value.x = self.transform_output.origin.x;
                    self.follow_axis_lock_value.y = self.transform_output.origin.y;
                }
                _ => {}
            }
        } else {
            self.follow_axis_is_locked = false;
        }
    }

    // ------------------------------------------------------------------
    // Reference resolution helpers (the oracle used typed setters)
    // ------------------------------------------------------------------

    fn follow_target_node(&self) -> Option<Gd<Node2D>> {
        self.follow_target
            .clone()
            .filter(|target| target.is_instance_valid())
    }

    fn valid_follow_path(&self) -> Option<Gd<Path2D>> {
        self.follow_path
            .clone()
            .filter(|path| path.is_instance_valid())
    }

    fn connect_tree_exiting(&mut self, target: &Gd<Node2D>) {
        // parity: `phantom_camera_2d.gd:1147` / `:1484` — the oracle connects
        // `tree_exiting.connect(_follow_target_tree_exiting.bind(target))`, so the
        // handler takes the target as its single argument while the signal itself
        // carries none.
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

    fn sync_noise_resource(&mut self) {
        if self.noise.is_some() {
            self.has_noise_resource = true;
            self.set_noise_trauma();
        } else {
            self.has_noise_resource = false;
            self.transform_noise = Transform2D::IDENTITY;
        }
    }

    /// Seam note: the noise resource API was not part of the frozen cross-module
    /// seam, but `resources.rs` exposes `set_trauma(&mut self, f64)`
    /// (`phantom_camera_noise_2d.gd:112-114`), so it is called typed.
    fn set_noise_trauma(&mut self) {
        if let Some(mut noise) = self.noise.clone() {
            noise.bind_mut().set_trauma(1.0);
        }
    }

    /// `phantom_camera_noise_2d.gd:119-131`.
    fn noise_transform(&mut self, delta: f64) -> Transform2D {
        match self.noise.clone() {
            Some(mut noise) => noise.bind_mut().get_noise_transform(delta),
            None => Transform2D::IDENTITY,
        }
    }

    /// `:1850-1890` — `limit_target` resolution (`_limit_node`).
    fn resolve_limit_target(&mut self) {
        let path = self.limit_target.clone();
        if path.is_empty() {
            self.limit_node = None;
            return;
        }
        let node = self.base().get_node_or_null(&path);
        let Some(node) = node else {
            godot_warn!("PhantomCamera2D: Limit Target cannot be found");
            self.limit_node = None;
            return;
        };
        if let Ok(tile_map) = node.clone().try_cast::<TileMapLayer>() {
            let callable = self.base().callable("pcam_tile_map_changed");
            let mut tile_map_node = tile_map.clone().upcast::<Node>();
            if !tile_map_node.is_connected("changed", &callable) {
                tile_map_node.connect("changed", &callable);
            }
            self.limit_node = Some(tile_map.upcast::<Node2D>());
            return;
        }
        if let Ok(collision) = node.clone().try_cast::<CollisionShape2D>() {
            if collision.get_shape().is_none() {
                godot_warn!("PhantomCamera2D: No Shape2D in the limit target");
                self.reset_limit();
                self.limit_target = NodePath::default();
                self.limit_node = None;
                return;
            }
            self.limit_node = Some(collision.upcast::<Node2D>());
            return;
        }
        godot_warn!("PhantomCamera2D: Limit Target is not a TileMapLayer or CollisionShape2D node");
    }

    fn limit_target_exist_error(&self) {
        if !self.limit_target.is_empty() {
            godot_warn!(
                "PhantomCamera2D: Unable to set Limit Side while a Limit Target is assigned"
            );
        }
    }

    // ------------------------------------------------------------------
    // Public API (`#region Public Functions`, `:1219-1343`)
    // ------------------------------------------------------------------

    /// `:1223-1287`.
    #[func]
    pub fn update_limit_all_sides(&mut self) {
        match self.compute_limit_sides() {
            Some(sides) => self.limit_sides = sides,
            // `:1234` / `:1256` — the oracle bails out before touching `_limit_sides`.
            None => return,
        }

        self.check_limit_is_not_default();
        if !self.is_active_pcam {
            return;
        }
        PhantomCameraManager::notify_limit_2d_changed(SIDE_LEFT, self.limit_sides.x as i64);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_TOP, self.limit_sides.y as i64);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_RIGHT, self.limit_sides.z as i64);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_BOTTOM, self.limit_sides.w as i64);
        PhantomCameraManager::notify_draw_limit_2d(self.get_draw_limits());
    }

    /// The rect -> sides step of `:1223-1287`.
    fn compute_limit_sides(&self) -> Option<Vector4i> {
        let Some(node) = self.limit_node.clone() else {
            return Some(Vector4i::new(
                self.limit_left as i32,
                self.limit_top as i32,
                self.limit_right as i32,
                self.limit_bottom as i32,
            ));
        };

        let rect = if let Ok(tile_map) = node.clone().try_cast::<TileMapLayer>() {
            // parity: `:1234` bails when the TileMapLayer has no TileSet.
            let tile_set = tile_map.get_tile_set()?;
            let used = tile_map.get_used_rect();
            let tile_size = tile_set.get_tile_size();
            let scale = tile_map.get_scale();
            let tile_size_v = Vector2::new(tile_size.x as real, tile_size.y as real);
            let tile_map_size =
                Vector2::new(used.size.x as real, used.size.y as real) * tile_size_v * scale;
            let tile_map_position = tile_map.get_global_position()
                + Vector2::new(used.position.x as real, used.position.y as real)
                    * tile_size_v
                    * scale;
            let margin_min = Vector2::new(self.limit_margin.x as real, self.limit_margin.y as real);
            let margin_max = Vector2::new(self.limit_margin.z as real, self.limit_margin.w as real);
            Rect2::new(
                tile_map_position + margin_min,
                tile_map_size - margin_min - margin_max,
            )
        } else if let Ok(collision) = node.clone().try_cast::<CollisionShape2D>() {
            // parity: `:1256` bails when the CollisionShape2D has no Shape2D.
            let shape = collision.get_shape()?;
            let shape_rect = shape.get_rect();
            let shape_position = collision.get_global_position() + shape_rect.position;
            let raw = Rect2::new(shape_position, shape_rect.size);
            let margin_min = Vector2::new(self.limit_margin.x as real, self.limit_margin.y as real);
            let margin_max = Vector2::new(self.limit_margin.z as real, self.limit_margin.w as real);
            Rect2::new(
                raw.position + margin_min,
                raw.size - margin_min - margin_max,
            )
        } else {
            return None;
        };

        Some(Vector4i::new(
            roundi(rect.position.x),
            roundi(rect.position.y),
            roundi(rect.position.x + rect.size.x),
            roundi(rect.position.y + rect.size.y),
        ))
    }

    /// `:1290-1296`.
    #[func]
    pub fn reset_limit(&mut self) {
        PhantomCameraManager::notify_limit_2d_changed(SIDE_LEFT, LIMIT_LEFT_DEFAULT);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_TOP, LIMIT_TOP_DEFAULT);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_RIGHT, LIMIT_RIGHT_DEFAULT);
        PhantomCameraManager::notify_limit_2d_changed(SIDE_BOTTOM, LIMIT_BOTTOM_DEFAULT);
        let draw = self.get_draw_limits();
        PhantomCameraManager::notify_draw_limit_2d(draw);
    }

    /// `:1302-1306`.
    #[func]
    pub fn set_tween_skip(&mut self, caller: Gd<Node>, value: bool) {
        if caller.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.tween_skip = value;
        } else {
            godot_warn!("PhantomCamera2D: set_tween_skip can only be called by PhantomCameraHost");
        }
    }

    /// `:1308-1309`.
    #[func]
    pub fn get_tween_skip(&self) -> bool {
        self.tween_skip
    }

    /// `:1312-1313`.
    #[func]
    pub fn get_transform_output(&self) -> Transform2D {
        self.transform_output
    }

    /// `:1317-1318`.
    #[func]
    pub fn get_noise_transform(&self) -> Transform2D {
        self.transform_noise
    }

    /// `:1323-1324`.
    #[func]
    pub fn emit_noise(&mut self, value: Transform2D) {
        self.signals().noise_emitted().emit(value);
        self.pending_noise = Some(value);
    }

    /// `:1329-1334`.
    #[func]
    pub fn teleport_position(&mut self) {
        self.follow_velocity_ref = Vector2::ZERO;
        self.reset_lookahead();
        self.set_follow_position(0.0);
        self.transform_output.origin = self.follow_target_position;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_teleport(&this);
    }

    /// `:1340-1341`.
    #[func]
    pub fn is_following(&self) -> bool {
        self.should_follow
    }

    // ------------------------------------------------------------------
    // Setters & getters (`#region Setter & Getter Functions`, `:1346-1968`)
    // ------------------------------------------------------------------

    /// `:1349-1352`.
    #[func]
    pub fn set_zoom(&mut self, value: Vector2) {
        self.zoom = value;
        self.base_mut().queue_redraw();
    }

    /// `:1354-1355`.
    #[func]
    pub fn get_zoom(&self) -> Vector2 {
        self.zoom
    }

    /// `:1359-1363`.
    #[func]
    pub fn set_priority(&mut self, value: i64) {
        self.priority = value.max(0);
        if !self.base().is_node_ready() {
            return;
        }
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_priority_changed(&this);
    }

    /// `:1366-1367`.
    #[func]
    pub fn get_priority(&self) -> i64 {
        self.priority
    }

    /// `:110-121`.
    #[func]
    pub fn set_priority_override(&mut self, value: bool) {
        self.priority_override = value;
        if Engine::singleton().is_editor_hint() {
            let this = self.base().clone().upcast::<Node>();
            PhantomCameraManager::notify_priority_override(&this, value);
        }
    }

    /// `:120-121`.
    #[func]
    pub fn get_priority_override(&self) -> bool {
        self.priority_override
    }

    /// `:1371-1372`.
    #[func]
    pub fn set_tween_resource(&mut self, value: Option<Gd<PhantomCameraTween>>) {
        self.tween_resource = value;
    }

    /// `:1376-1377`.
    #[func]
    pub fn get_tween_resource(&self) -> Option<Gd<PhantomCameraTween>> {
        self.tween_resource.clone()
    }

    /// `:1382-1384`.
    #[func]
    pub fn set_tween_duration(&mut self, value: f64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().duration = value,
            None => {
                godot_warn!("PhantomCamera2D: no tween_resource assigned; duration not stored");
                self.tween_duration_fallback = value;
            }
        }
    }

    /// `:1388-1392`.
    ///
    /// The oracle always has a `PhantomCameraTween` (auto-created in the export
    /// initializer), so a fresh PCam reports 1.0 / linear / ease-in-out. With no
    /// resource assigned the port falls back to the same ABI defaults instead of the
    /// oracle's null-path `0.0`.
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

    /// `:1397-1398`.
    #[func]
    pub fn set_tween_transition(&mut self, value: i64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().transition = value,
            None => {
                godot_warn!("PhantomCamera2D: no tween_resource assigned; transition not stored");
                self.tween_transition_fallback = value;
            }
        }
    }

    /// `:1402-1406`.
    #[func]
    pub fn get_tween_transition(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().transition,
            None => self.tween_transition_fallback,
        }
    }

    /// `:1410-1411`.
    #[func]
    pub fn set_tween_ease(&mut self, value: i64) {
        match self.tween_resource.clone() {
            Some(mut resource) => resource.bind_mut().ease = value,
            None => {
                godot_warn!("PhantomCamera2D: no tween_resource assigned; ease not stored");
                self.tween_ease_fallback = value;
            }
        }
    }

    /// `:1414-1418`.
    #[func]
    pub fn get_tween_ease(&self) -> i64 {
        match &self.tween_resource {
            Some(resource) => resource.bind().ease,
            None => self.tween_ease_fallback,
        }
    }

    /// `:1424-1429`.
    #[func]
    pub fn set_is_active(&mut self, node: Gd<Node>, value: bool) {
        if node.try_cast::<crate::host::PhantomCameraHost>().is_ok() {
            self.is_active_pcam = value;
            self.base_mut().queue_redraw();
        } else {
            godot_warn!("PhantomCamera2D: PCams can only be set from the PhantomCameraHost");
        }
    }

    /// `:1434-1435`.
    #[func]
    pub fn is_active(&self) -> bool {
        self.is_active_pcam
    }

    /// `:1439-1440`.
    #[func]
    pub fn set_tween_on_load(&mut self, value: bool) {
        self.tween_on_load = value;
    }

    /// `:1443-1444`.
    #[func]
    pub fn get_tween_on_load(&self) -> bool {
        self.tween_on_load
    }

    /// `:1447-1450`.
    #[func]
    pub fn set_host_layers(&mut self, value: i64) {
        self.host_layers = value;
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_host_layer_changed(&this);
    }

    /// `:1453-1454`.
    #[func]
    pub fn set_host_layers_value(&mut self, layer: i64, value: bool) {
        self.host_layers = set_layer(self.host_layers, layer, value);
    }

    /// `:1457-1458`.
    #[func]
    pub fn get_host_layers(&self) -> i64 {
        self.host_layers
    }

    /// `:1464-1465`.
    #[func]
    pub fn get_follow_mode(&self) -> i64 {
        self.follow_mode
    }

    /// `:142-178`.
    #[func]
    pub fn set_follow_mode(&mut self, value: i64) {
        self.follow_mode = value;
        self.signals().follow_mode_changed().emit();

        if value == 0 {
            self.should_follow = false;
            // `:149` `top_level = false` (CanvasItem property).
            self.base_mut().set("top_level", &false.to_variant());
            self.is_parents_physics(None);
            return;
        }

        match value {
            3 => self.follow_targets_size_check(),
            4 => {
                if self.valid_follow_path().is_some() {
                    self.should_follow_checker();
                } else {
                    self.should_follow = false;
                }
            }
            _ => self.should_follow_checker(),
        }

        if value == 5 {
            // parity: `:165-171` only connects `dead_zone_changed` when
            // `_follow_framed_initial_set` is set, and nothing ever sets it true —
            // `_on_dead_zone_changed` is unreachable dead code in the oracle too.
            if self.follow_framed_initial_set && self.follow_target_node().is_some() {
                self.follow_framed_initial_set = false;
            }
        }

        // `:173` `top_level = true`.
        self.base_mut().set("top_level", &true.to_variant());
        self.reset_lookahead();
    }

    /// `:1469-1489`.
    #[func]
    pub fn set_follow_target(&mut self, value: Option<Gd<Node2D>>) {
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
                self.connect_tree_exiting(target);
            }
            None => self.should_follow = false,
        }
        self.reset_lookahead();
        self.signals().follow_target_changed().emit();
    }

    /// `:1492-1500`.
    #[func]
    pub fn erase_follow_target(&mut self) {
        if self.follow_target_node().is_none() {
            return;
        }
        self.should_follow = false;
        self.follow_target = None;
        self.follow_target_physics_based = false;
        self.character_body_2d = None;
        self.rigid_body_2d = None;
        self.follow_target_physics_class = 2;
        self.signals().follow_target_changed().emit();
    }

    /// `:1503-1504`.
    #[func]
    pub fn get_follow_target(&self) -> Option<Gd<Node2D>> {
        self.follow_target_node()
    }

    /// `:1508-1513`.
    #[func]
    pub fn set_follow_path(&mut self, value: Option<Gd<Path2D>>) {
        self.follow_path = value.filter(|path| path.is_instance_valid());
        if self.valid_follow_path().is_some() {
            self.should_follow_checker();
        } else {
            self.should_follow = false;
        }
    }

    /// `:1516-1517`.
    #[func]
    pub fn erase_follow_path(&mut self) {
        self.follow_path = None;
    }

    /// `:1520-1521`.
    #[func]
    pub fn get_follow_path(&self) -> Option<Gd<Path2D>> {
        self.valid_follow_path()
    }

    /// `:1525-1530`.
    #[func]
    pub fn set_follow_targets(&mut self, value: Array<Gd<Node2D>>) {
        if self.follow_mode != 3 {
            return;
        }
        self.follow_targets = value;
        self.follow_targets_size_check();
        self.reset_lookahead();
    }

    /// `:1533-1542`.
    #[func]
    pub fn append_follow_targets(&mut self, value: Gd<Node2D>) {
        if !value.is_instance_valid() {
            godot_warn!("PhantomCamera2D: value is not a valid Node2D instance");
            return;
        }
        if self.contains_follow_target(&value) {
            godot_warn!("PhantomCamera2D: value is already part of the Follow Group");
            return;
        }
        self.follow_targets.push(&value);
        self.follow_targets_size_check();
    }

    /// `:1545-1552`.
    #[func]
    pub fn append_follow_targets_array(&mut self, value: Array<Gd<Node2D>>) {
        for target in value.iter_shared() {
            if !target.is_instance_valid() {
                continue;
            }
            if self.contains_follow_target(&target) {
                godot_warn!("PhantomCamera2D: value is already part of the Follow Group");
                continue;
            }
            self.follow_targets.push(&target);
            self.follow_targets_size_check();
        }
    }

    /// `:1555-1557`.
    #[func]
    pub fn erase_follow_targets(&mut self, value: Gd<Node2D>) {
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

    /// `:1560-1561`.
    #[func]
    pub fn get_follow_targets(&self) -> Array<Gd<Node2D>> {
        self.follow_targets.clone()
    }

    fn contains_follow_target(&self, value: &Gd<Node2D>) -> bool {
        self.follow_targets
            .iter_shared()
            .any(|target| target.instance_id() == value.instance_id())
    }

    /// `:1565-1579`.
    ///
    /// parity: the oracle's `match value:` compares the *offset `Vector2`* against
    /// the `FollowLockAxis` enum, so no arm can ever match and
    /// `_follow_axis_lock_value` is never updated here. Kept dead.
    #[func]
    pub fn set_follow_offset(&mut self, value: Vector2) {
        let temp_offset = self.follow_offset;
        self.follow_offset = value;
        if self.follow_axis_lock != 0 {
            let _ = temp_offset - value;
        }
    }

    /// `:1583-1584`.
    #[func]
    pub fn get_follow_offset(&self) -> Vector2 {
        self.follow_offset
    }

    /// `:1588-1590`.
    #[func]
    pub fn set_follow_damping(&mut self, value: bool) {
        self.follow_damping = value;
    }

    /// `:1593-1594`.
    #[func]
    pub fn get_follow_damping(&self) -> bool {
        self.follow_damping
    }

    /// `:1598-1602`.
    #[func]
    pub fn set_follow_damping_value(&mut self, value: Vector2) {
        self.follow_damping_value = Vector2::new(value.x.max(0.0), value.y.max(0.0));
    }

    /// `:1605-1606`.
    #[func]
    pub fn get_follow_damping_value(&self) -> Vector2 {
        self.follow_damping_value
    }

    /// `:1610-1628` (`await ready` becomes [`Self::pending_axis_lock`]).
    #[func]
    pub fn set_lock_axis(&mut self, value: i64) {
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

    /// `:1631-1632`.
    #[func]
    pub fn get_lock_axis(&self) -> i64 {
        self.follow_axis_lock
    }

    /// `:1636-1638`.
    #[func]
    pub fn set_rotate_with_target(&mut self, value: bool) {
        self.rotate_with_target = value;
    }

    /// `:1641-1642`.
    #[func]
    pub fn get_rotate_with_target(&self) -> bool {
        self.rotate_with_target
    }

    /// `:1646-1647`.
    #[func]
    pub fn set_rotation_offset(&mut self, value: f64) {
        self.rotation_offset = value;
    }

    /// `:1650-1651`.
    #[func]
    pub fn get_rotation_offset(&self) -> f64 {
        self.rotation_offset
    }

    /// `:1655-1657`.
    #[func]
    pub fn set_rotation_damping(&mut self, value: bool) {
        self.rotation_damping = value;
    }

    /// `:1660-1661`.
    #[func]
    pub fn get_rotation_damping(&self) -> bool {
        self.rotation_damping
    }

    /// `:1665-1666`.
    #[func]
    pub fn set_rotation_damping_value(&mut self, value: f64) {
        self.rotation_damping_value = value;
    }

    /// `:1669-1670`.
    #[func]
    pub fn get_rotation_damping_value(&self) -> f64 {
        self.rotation_damping_value
    }

    /// `:1674-1675`.
    #[func]
    pub fn set_snap_to_pixel(&mut self, value: bool) {
        self.snap_to_pixel = value;
    }

    /// `:1678-1679`.
    #[func]
    pub fn get_snap_to_pixel(&self) -> bool {
        self.snap_to_pixel
    }

    /// `:1683-1685`.
    #[func]
    pub fn set_auto_zoom(&mut self, value: bool) {
        self.auto_zoom = value;
    }

    /// `:1688-1689`.
    #[func]
    pub fn get_auto_zoom(&self) -> bool {
        self.auto_zoom
    }

    /// `:1693-1694`.
    #[func]
    pub fn set_auto_zoom_min(&mut self, value: f64) {
        self.auto_zoom_min = value;
    }

    /// `:1697-1698`.
    #[func]
    pub fn get_auto_zoom_min(&self) -> f64 {
        self.auto_zoom_min
    }

    /// `:1702-1703`.
    #[func]
    pub fn set_auto_zoom_max(&mut self, value: f64) {
        self.auto_zoom_max = value;
    }

    /// `:1706-1707`.
    #[func]
    pub fn get_auto_zoom_max(&self) -> f64 {
        self.auto_zoom_max
    }

    /// `:1711-1712`.
    #[func]
    pub fn set_auto_zoom_margin(&mut self, value: Vector4) {
        self.auto_zoom_margin = value;
    }

    /// `:1715-1716`.
    #[func]
    pub fn get_auto_zoom_margin(&self) -> Vector4 {
        self.auto_zoom_margin
    }

    /// `:1719-1722`.
    #[func]
    pub fn set_lookahead(&mut self, value: bool) {
        self.lookahead = value;
        self.reset_lookahead();
    }

    /// `:1724-1725`.
    #[func]
    pub fn get_lookahead(&self) -> bool {
        self.lookahead
    }

    /// `:1728-1732`.
    #[func]
    pub fn set_lookahead_time(&mut self, value: Vector2) {
        self.lookahead_time = Vector2::new(value.x.max(0.0), value.y.max(0.0));
    }

    /// `:1734-1735`.
    #[func]
    pub fn get_lookahead_time(&self) -> Vector2 {
        self.lookahead_time
    }

    /// `:1738-1740`.
    #[func]
    pub fn set_lookahead_max(&mut self, value: bool) {
        self.lookahead_max = value;
    }

    /// `:1742-1743`.
    #[func]
    pub fn get_lookahead_max(&self) -> bool {
        self.lookahead_max
    }

    /// `:1746-1750`.
    #[func]
    pub fn set_lookahead_max_value(&mut self, value: Vector2) {
        self.lookahead_max_value = Vector2::new(value.x.max(0.0), value.y.max(0.0));
    }

    /// `:1752-1753`.
    #[func]
    pub fn get_lookahead_max_value(&self) -> Vector2 {
        self.lookahead_max_value
    }

    /// `:1756-1761`.
    #[func]
    pub fn set_lookahead_acceleration(&mut self, value: f64) {
        self.lookahead_acceleration = value.max(0.0);
        self.has_lookahead_acceleration = !is_zero_approx(self.lookahead_acceleration);
    }

    /// `:1763-1764`.
    #[func]
    pub fn get_lookahead_acceleration(&self) -> f64 {
        self.lookahead_acceleration
    }

    /// `:1767-1772`.
    #[func]
    pub fn set_lookahead_deceleration(&mut self, value: f64) {
        self.lookahead_deceleration = value.max(0.0);
        self.has_lookahead_deceleration = !is_zero_approx(self.lookahead_deceleration);
    }

    /// `:1774-1775`.
    #[func]
    pub fn get_lookahead_deceleration(&self) -> f64 {
        self.lookahead_deceleration
    }

    /// `:1780-1786`.
    #[func]
    pub fn set_limit(&mut self, side: i64, value: i64) {
        match side {
            SIDE_LEFT => self.set_limit_left(value),
            SIDE_TOP => self.set_limit_top(value),
            SIDE_RIGHT => self.set_limit_right(value),
            SIDE_BOTTOM => self.set_limit_bottom(value),
            _ => godot_warn!("PhantomCamera2D: Not a valid Side."),
        }
    }

    /// `:1789-1797`.
    #[func]
    pub fn get_limit(&self, side: i64) -> i64 {
        match side {
            SIDE_LEFT => self.limit_left,
            SIDE_TOP => self.limit_top,
            SIDE_RIGHT => self.limit_right,
            SIDE_BOTTOM => self.limit_bottom,
            _ => {
                godot_warn!("PhantomCamera2D: Not a valid Side.");
                -1
            }
        }
    }

    /// `:1801-1804`.
    #[func]
    pub fn set_limit_left(&mut self, value: i64) {
        self.limit_target_exist_error();
        self.limit_left = value;
        self.update_limit_all_sides();
    }

    /// `:1807-1808`.
    #[func]
    pub fn get_limit_left(&self) -> i64 {
        self.limit_left
    }

    /// `:1812-1815`.
    #[func]
    pub fn set_limit_top(&mut self, value: i64) {
        self.limit_target_exist_error();
        self.limit_top = value;
        self.update_limit_all_sides();
    }

    /// `:1818-1819`.
    #[func]
    pub fn get_limit_top(&self) -> i64 {
        self.limit_top
    }

    /// `:1823-1826`.
    #[func]
    pub fn set_limit_right(&mut self, value: i64) {
        self.limit_target_exist_error();
        self.limit_right = value;
        self.update_limit_all_sides();
    }

    /// `:1829-1830`.
    #[func]
    pub fn get_limit_right(&self) -> i64 {
        self.limit_right
    }

    /// `:1834-1837`.
    #[func]
    pub fn set_limit_bottom(&mut self, value: i64) {
        self.limit_target_exist_error();
        self.limit_bottom = value;
        self.update_limit_all_sides();
    }

    /// `:1840-1841`.
    #[func]
    pub fn get_limit_bottom(&self) -> i64 {
        self.limit_bottom
    }

    /// `:1850-1890` (`await ready` replaced by lazy resolution in `_ready`).
    #[func]
    pub fn set_limit_target(&mut self, value: NodePath) {
        self.limit_target = value;
        if !self.base().is_node_ready() {
            return;
        }
        self.resolve_limit_target();
        self.update_limit_all_sides();
    }

    /// `:1893-1897`.
    #[func]
    pub fn get_limit_target(&self) -> NodePath {
        self.limit_target.clone()
    }

    /// `:1901-1903`.
    #[func]
    pub fn set_limit_margin(&mut self, value: Vector4i) {
        self.limit_margin = value;
        self.update_limit_all_sides();
    }

    /// `:1905-1906`.
    #[func]
    pub fn get_limit_margin(&self) -> Vector4i {
        self.limit_margin
    }

    /// `:1918-1925`.
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise2D>>) {
        self.noise = value;
        self.sync_noise_resource();
    }

    /// `:1928-1929`.
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise2D>> {
        self.noise.clone()
    }

    /// `:1931-1932`.
    #[func]
    pub fn has_noise_resource(&self) -> bool {
        self.has_noise_resource
    }

    /// `:1936-1937`.
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }

    /// `:1940-1941`.
    #[func]
    pub fn set_noise_emitter_layer_value(&mut self, layer: i64, enabled: bool) {
        self.noise_emitter_layer = set_layer(self.noise_emitter_layer, layer, enabled);
    }

    /// `:1944-1945`.
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }

    /// `:1949-1950`.
    #[func]
    pub fn set_inactive_update_mode(&mut self, value: i64) {
        self.inactive_update_mode = value;
    }

    /// `:1953-1954`.
    #[func]
    pub fn get_inactive_update_mode(&self) -> i64 {
        self.inactive_update_mode
    }

    /// `:1957-1958`.
    #[func]
    pub fn get_follow_target_physics_based(&self) -> bool {
        self.follow_target_physics_based
    }

    // ----- `#[var]` accessors for the remaining script-style exports -----

    /// `:407-412`.
    #[func]
    pub fn set_dead_zone_width(&mut self, value: f64) {
        self.dead_zone_width = value;
        self.signals().dead_zone_changed().emit();
    }

    /// `:411-412`.
    #[func]
    pub fn get_dead_zone_width(&self) -> f64 {
        self.dead_zone_width
    }

    /// `:419-424`.
    #[func]
    pub fn set_dead_zone_height(&mut self, value: f64) {
        self.dead_zone_height = value;
        self.signals().dead_zone_changed().emit();
    }

    /// `:423-424`.
    #[func]
    pub fn get_dead_zone_height(&self) -> f64 {
        self.dead_zone_height
    }

    /// `:435-441`.
    #[func]
    pub fn set_draw_limits(&mut self, value: bool) {
        DRAW_LIMITS.store(value, std::sync::atomic::Ordering::Relaxed);
        if Engine::singleton().is_editor_hint() {
            PhantomCameraManager::notify_draw_limit_2d(value);
        }
    }

    /// `:440-441`.
    #[func]
    pub fn get_draw_limits(&self) -> bool {
        DRAW_LIMITS.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// `:500-504`.
    #[func]
    pub fn set_preview_noise(&mut self, value: bool) {
        self._preview_noise = value;
        if !value {
            self.transform_noise = Transform2D::IDENTITY;
        }
    }

    /// `:503-504`.
    #[func]
    pub fn get_preview_noise(&self) -> bool {
        self._preview_noise
    }

    // ----- handlers reachable from `Base::callable` -----
    //
    // The oracle connected engine signals to private methods (`_check_visibility`,
    // `_follow_target_tree_exiting`, `_on_tile_map_changed`). gdext needs a
    // registered method to build the `Callable`, so those handlers exist as
    // `pcam_*` `#[func]`s; see the port report for this ABI addition.

    /// `:1112-1114`.
    #[func]
    pub fn pcam_check_visibility(&mut self) {
        self.check_visibility();
    }

    /// `:1116-1121`.
    #[func]
    pub fn pcam_follow_target_tree_exiting(&mut self, target: Gd<Node>) {
        self.follow_target_tree_exiting_impl(&target);
    }

    /// `:1088-1089`.
    #[func]
    pub fn pcam_tile_map_changed(&mut self) {
        self.update_limit_all_sides();
    }
}

#[godot_api]
impl INode2D for PhantomCamera2D {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            priority_override: false,
            priority: 0,
            follow_mode: 0,
            follow_target: None,
            follow_targets: Array::new(),
            follow_path: None,
            zoom: Vector2::ONE,
            snap_to_pixel: false,
            frame_preview: true,
            tween_resource: None,
            tween_on_load: true,
            inactive_update_mode: 0,
            host_layers: 1,
            follow_offset: Vector2::ZERO,
            follow_damping: false,
            follow_damping_value: Vector2::new(0.1, 0.1),
            follow_axis_lock: 0,
            rotate_with_target: false,
            rotation_offset: 0.0,
            rotation_damping: false,
            rotation_damping_value: 0.1,
            lookahead: false,
            lookahead_time: Vector2::new(0.5, 0.5),
            lookahead_acceleration: 0.2,
            lookahead_deceleration: 0.2,
            lookahead_max: false,
            lookahead_max_value: Vector2::new(200.0, 200.0),
            auto_zoom: false,
            auto_zoom_min: 1.0,
            auto_zoom_max: 5.0,
            auto_zoom_margin: Vector4::new(0.0, 0.0, 0.0, 0.0),
            dead_zone_width: 0.0,
            dead_zone_height: 0.0,
            show_viewfinder_in_play: false,
            draw_limits: false,
            limit_left: LIMIT_LEFT_DEFAULT,
            limit_top: LIMIT_TOP_DEFAULT,
            limit_right: LIMIT_RIGHT_DEFAULT,
            limit_bottom: LIMIT_BOTTOM_DEFAULT,
            limit_target: NodePath::default(),
            limit_margin: Vector4i::new(0, 0, 0, 0),
            noise: None,
            _preview_noise: true,
            noise_emitter_layer: 0,
            is_active_pcam: false,
            should_follow: false,
            follow_target_physics_based: false,
            follow_target_physics_class: 2,
            character_body_2d: None,
            rigid_body_2d: None,
            has_multiple_follow_targets: false,
            follow_targets_single_target_index: 0,
            follow_targets_cache: Vec::new(),
            follow_velocity_ref: Vector2::ZERO,
            rotation_velocity_ref: 0.0,
            has_lookahead_acceleration: true,
            has_lookahead_deceleration: true,
            tween_skip: false,
            follow_framed_initial_set: false,
            lookahead_offset: Vector2::ZERO,
            lookahead_offset_velocity_ref: Vector2::ZERO,
            lookahead_sample_pos_prev: Vector2::ZERO,
            lookahead_has_prev_sample: false,
            follow_axis_is_locked: false,
            follow_axis_lock_value: Vector2::ZERO,
            // parity: `:551` initializes `_limit_sides` from `_limit_sides_default`,
            // which is declared *after* it (`:552`), so a fresh instance really holds
            // `(0, 0, 0, 0)` until `_enter_tree` -> `update_limit_all_sides`. The ABI
            // dump confirms the default is all-zero.
            limit_sides: Vector4i::new(0, 0, 0, 0),
            limit_node: None,
            limit_inactive_pcam: false,
            follow_target_position: Vector2::ZERO,
            transform_output: Transform2D::IDENTITY,
            transform_noise: Transform2D::IDENTITY,
            has_noise_resource: false,
            pending_noise: None,
            pending_axis_lock: None,
            tween_duration_fallback: 0.0,
            tween_transition_fallback: TWEEN_DEFAULT_TRANSITION,
            tween_ease_fallback: TWEEN_DEFAULT_EASE,
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
