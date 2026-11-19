// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/resources/*.gd
//
//! Resource classes of the Phantom Camera port.
//!
//! Ported from the GDScript oracle (v0.11.0.3, read-only at
//! `parity/pcam_oracle/phantom_camera/`):
//!
//! | Class | Oracle file | Anchors |
//! |---|---|---|
//! | [`PhantomCameraTween`] | `scripts/resources/tween_resource.gd` (41 L) | enums `:11-32`, exports `:35-41` |
//! | [`Camera3DResource`] | `scripts/resources/camera_3d_resource.gd` (110 L) | enums `:11-20`, setters `:23-99`, `_validate_property` `:102-110` |
//! | [`PhantomCameraNoise2D`] | `scripts/resources/phantom_camera_noise_2d.gd` (228 L) | `_init` `:88-91`, `_get_noise_from_seed` `:108-109`, `_trauma` setter `:78-80`, `get_noise_transform` `:119-131`, `reset_noise_time` `:134-135`, setters `:142-226` |
//! | [`PhantomCameraNoise3D`] | `scripts/resources/phantom_camera_noise_3d.gd` (301 L) | `_init` `:120-124`, `_trauma` setter `:108-114`, `_get_noise_from_seed` `:146-147`, `get_noise_transform` `:157-173`, `reset_noise_time` `:176-177`, setters `:184-299` |
//! | [`TweenDirectorResource`] | `scripts/resources/tween_director_resource.gd` (155 L) | enum `:15-19`, exports `:27-68`, `_validate_property` `:75-84`, setters `:92-153` |
//!
//! # Exported property hints are copied verbatim
//!
//! `abi_dump.gd` records each property's `hint`/`hint_string` pair, and the port
//! reproduces them byte for byte. gdext's `#[export(range = …)]`,
//! `#[export(enum = …)]` and `#[export(exp_easing = …)]` helpers normalise the
//! hint string differently from Godot's GDScript annotations (Rust's `f64`
//! `Display` prints `0` where Godot prints `0.0`, flag/suffix order is fixed,
//! and enum labels such as `Ease In` cannot be expressed as Rust identifiers),
//! so those properties carry an explicit `#[var(hint = …, hint_string = "…")]`
//! copied from the frozen ABI table instead. Booleans, object references,
//! `Vector2` and `@export_node_path`/`@export_flags_*` use the idiomatic
//! `#[export]`/`#[export(…)]` form, which does reproduce the oracle exactly.
//!
//! # Deliberately omitted (editor-only)
//!
//! * `@icon(...)` tool annotations — no runtime effect.
//! * `_validate_property` (`camera_3d_resource.gd:102-110`,
//!   `phantom_camera_noise_2d.gd:94-105`, `phantom_camera_noise_3d.gd:127-143`,
//!   `tween_director_resource.gd:75-84`). Those overrides only hide dependent
//!   properties in the inspector (`PROPERTY_USAGE_NO_EDITOR`); no runtime code
//!   reads them. Consequence: the four resources always present all of their
//!   properties to the editor, and `abi_dump` reports usage `6`
//!   (`STORAGE|EDITOR`) for every export instead of the oracle's
//!   `SCRIPT_VARIABLE|STORAGE|EDITOR` (`4102`) / `+CLASS_IS_ENUM` (`69638`).
//!   See the module note in `PORT-PLAN.md` §"Editor-only oracle code".
//! * Private backing variables (`_noise_algorithm`, `_noise_time`, `_trauma`,
//!   `_noise_positional_multiplier`, …) are plain Rust fields here, so they do
//!   not appear in `get_property_list`. They are unreachable from a `.tscn`
//!   (`PROPERTY_USAGE_SCRIPT_VARIABLE` only, never stored).
//!
//! # Behavioural quirks preserved on purpose
//!
//! * `PhantomCameraNoise3D`'s `_trauma` setter zeroes `_noise_time` whenever the
//!   value becomes exactly `0.0` (`phantom_camera_noise_3d.gd:108-114`); the 2D
//!   resource does **not** (`phantom_camera_noise_2d.gd:78-80`).
//! * 2D rotation scales by `rotational_multiplier / 100`
//!   (`phantom_camera_noise_2d.gd:129`), 3D converts with `deg_to_rad`
//!   (`phantom_camera_noise_3d.gd:165-167`).
//! * 3D position divides the multiplier by `10`
//!   (`phantom_camera_noise_3d.gd:170`), 2D does not
//!   (`phantom_camera_noise_2d.gd:127`).
//! * `set_noise_seed` only stores the seed; it never reseeds the
//!   `FastNoiseLite` instance (`phantom_camera_noise_2d.gd:172-173`). The seed
//!   reaches the noise pattern through `_get_noise_from_seed`'s first sample
//!   coordinate instead.

use godot::classes::fast_noise_lite::NoiseType;
use godot::classes::{FastNoiseLite, IResource, Resource};
use godot::global::randi;
use godot::prelude::*;

use crate::abi;

/// Round a GDScript `float` (`f64`) down to the engine's `real` precision.
///
/// `Vector2`/`Vector3`/`Transform2D` components are `real` (`f32` in a standard
/// build). GDScript narrows implicitly on assignment; Rust needs the cast
/// spelled out. Clippy's `cast_possible_truncation` is not part of
/// `clippy::all`, so this stays lint-clean, but the helper documents intent.
#[inline]
fn real_of(value: f64) -> real {
    value as real
}

// ---------------------------------------------------------------------------
// PhantomCameraTween — `scripts/resources/tween_resource.gd`
// ---------------------------------------------------------------------------

/// `tween_resource.gd:1-41` — how a `PhantomCamera2D/3D` transition looks.
///
/// The three fields are `pub` because the host (`host.rs`) reads them straight
/// out of the resource during its tween setup; `Camera3DResource::read`-style
/// indirection would only add noise (`PORT-PLAN.md` cross-module contract).
#[derive(GodotClass)]
#[class(base=Resource)]
pub struct PhantomCameraTween {
    base: Base<Resource>,

    /// `tween_resource.gd:35` — seconds the transition takes `0.0` = instant.
    #[export]
    pub duration: f64,

    /// `tween_resource.gd:38` — `TransitionType` (`abi::TransitionType`).
    #[export]
    #[var(hint = ENUM, hint_string = "Linear:0,Sine:1,Quint:2,Quart:3,Quad:4,Expo:5,Elastic:6,Cubic:7,Circ:8,Bounce:9,Back:10")]
    pub transition: i64,

    /// `tween_resource.gd:41` — `EaseType` (`abi::EaseType`).
    #[export]
    #[var(hint = ENUM, hint_string = "Ease In:0,Ease Out:1,Ease In Out:2,Ease Out In:3")]
    pub ease: i64,
}

#[godot_api]
impl IResource for PhantomCameraTween {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            duration: abi::TWEEN_DEFAULT_DURATION,
            transition: abi::TWEEN_DEFAULT_TRANSITION,
            ease: abi::TWEEN_DEFAULT_EASE,
        }
    }
}

// ---------------------------------------------------------------------------
// Camera3DResource — `scripts/resources/camera_3d_resource.gd`
// ---------------------------------------------------------------------------

/// `camera_3d_resource.gd:1-110` — `Camera3D` overrides applied while the
/// owning `PhantomCamera3D` is active.
///
/// Every setter mirrors the oracle's inline `set(value)` block: store, then
/// `emit_changed()`. `projection` additionally calls
/// `notify_property_list_changed()` (`:56`), which is a no-op here because
/// `_validate_property` is not ported (see the module doc) but is kept so the
/// inspector still refreshes.
///
/// The fields are `pub` because `host.rs` diffs them by Godot property name and
/// also applies them per frame; [`Self::read`]/[`Self::write`] are the
/// name-keyed seam it uses.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct Camera3DResource {
    base: Base<Resource>,

    /// `camera_3d_resource.gd:23-28` — `Camera3D.keep_aspect` override.
    #[export]
    #[var(set = set_keep_aspect, hint = ENUM, hint_string = "Keep Width:0,Keep Height:1")]
    pub keep_aspect: i64,

    /// `camera_3d_resource.gd:31-36` — `Camera3D.cull_mask` override.
    #[export(flags_3d_render)]
    #[var(set = set_cull_mask)]
    pub cull_mask: i64,

    /// `camera_3d_resource.gd:39-44` — `Camera3D.h_offset` override.
    #[export]
    #[var(set = set_h_offset, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater,or_less,hide_slider,suffix:m")]
    pub h_offset: f64,

    /// `camera_3d_resource.gd:47-50` — `Camera3D.v_offset` override.
    ///
    /// The oracle declares a setter without a getter block; the generated
    /// getter reads the backing field, which is what `#[var(set = …)]` does.
    #[export]
    #[var(set = set_v_offset, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater,or_less,hide_slider,suffix:m")]
    pub v_offset: f64,

    /// `camera_3d_resource.gd:53-59` — `Camera3D.projection` override.
    #[export]
    #[var(set = set_projection, hint = ENUM, hint_string = "Perspective:0,Orthogonal:1,Frustum:2")]
    pub projection: i64,

    /// `camera_3d_resource.gd:62-67` — `Camera3D.fov` override.
    #[export]
    #[var(set = set_fov, hint = RANGE, hint_string = "1.0,179.0,0.1,degrees")]
    pub fov: f64,

    /// `camera_3d_resource.gd:70-75` — `Camera3D.size` override.
    #[export]
    #[var(set = set_size, hint = RANGE, hint_string = "0.001,100.0,0.001,suffix:m,or_greater")]
    pub size: f64,

    /// `camera_3d_resource.gd:78-83` — `Camera3D.frustum_offset` override.
    #[export]
    #[var(set = set_frustum_offset)]
    pub frustum_offset: Vector2,

    /// `camera_3d_resource.gd:86-91` — `Camera3D.near` override.
    #[export]
    #[var(set = set_near, hint = RANGE, hint_string = "0.001,10.0,0.001,suffix:m,or_greater")]
    pub near: f64,

    /// `camera_3d_resource.gd:94-99` — `Camera3D.far` override.
    #[export]
    #[var(set = set_far, hint = RANGE, hint_string = "0.01,4000.0,0.001,suffix:m,or_greater")]
    pub far: f64,
}

#[godot_api]
impl IResource for Camera3DResource {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            keep_aspect: abi::KeepAspect::KeepHeight as i64,
            cull_mask: abi::CAMERA_3D_DEFAULT_CULL_MASK,
            h_offset: 0.0,
            v_offset: 0.0,
            projection: abi::ProjectionType::Perspective as i64,
            fov: 75.0,
            size: 1.0,
            frustum_offset: Vector2::ZERO,
            near: 0.05,
            far: 4000.0,
        }
    }
}

#[godot_api]
impl Camera3DResource {
    /// `camera_3d_resource.gd:23-28`.
    #[func]
    pub fn set_keep_aspect(&mut self, value: i64) {
        self.keep_aspect = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:31-36`.
    #[func]
    pub fn set_cull_mask(&mut self, value: i64) {
        self.cull_mask = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:39-44`.
    #[func]
    pub fn set_h_offset(&mut self, value: f64) {
        self.h_offset = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:47-50`.
    #[func]
    pub fn set_v_offset(&mut self, value: f64) {
        self.v_offset = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:53-59`.
    #[func]
    pub fn set_projection(&mut self, value: i64) {
        self.projection = value;
        self.base_mut().notify_property_list_changed();
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:62-67`.
    #[func]
    pub fn set_fov(&mut self, value: f64) {
        self.fov = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:70-75`.
    #[func]
    pub fn set_size(&mut self, value: f64) {
        self.size = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:78-83`.
    #[func]
    pub fn set_frustum_offset(&mut self, value: Vector2) {
        self.frustum_offset = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:86-91`.
    #[func]
    pub fn set_near(&mut self, value: f64) {
        self.near = value;
        self.base_mut().emit_changed();
    }

    /// `camera_3d_resource.gd:94-99`.
    #[func]
    pub fn set_far(&mut self, value: f64) {
        self.far = value;
        self.base_mut().emit_changed();
    }

    /// Read one override by its Godot property name (`host.rs` tween diff pass).
    ///
    /// Unknown names return `Variant::nil()`, mirroring `Object::get` on a
    /// property that does not exist (`camera_3d_resource.gd` has exactly these
    /// ten).
    pub fn read(&self, property: &str) -> Variant {
        match property {
            "keep_aspect" => self.keep_aspect.to_variant(),
            "cull_mask" => self.cull_mask.to_variant(),
            "h_offset" => self.h_offset.to_variant(),
            "v_offset" => self.v_offset.to_variant(),
            "projection" => self.projection.to_variant(),
            "fov" => self.fov.to_variant(),
            "size" => self.size.to_variant(),
            "frustum_offset" => self.frustum_offset.to_variant(),
            "near" => self.near.to_variant(),
            "far" => self.far.to_variant(),
            _ => Variant::nil(),
        }
    }

    /// Write one override by its Godot property name, through the oracle's
    /// setter (i.e. including `emit_changed()`).
    ///
    /// A value whose type does not convert leaves the field untouched — the
    /// oracle would raise a runtime conversion error from `set()`; silently
    /// ignoring it keeps the port panic-free (`PORT-PLAN.md` rule 4).
    pub fn write(&mut self, property: &str, value: Variant) {
        match property {
            "keep_aspect" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_keep_aspect(v);
                }
            }
            "cull_mask" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_cull_mask(v);
                }
            }
            "h_offset" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_h_offset(v);
                }
            }
            "v_offset" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_v_offset(v);
                }
            }
            "projection" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_projection(v);
                }
            }
            "fov" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_fov(v);
                }
            }
            "size" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_size(v);
                }
            }
            "frustum_offset" => {
                if let Ok(v) = value.try_to::<Vector2>() {
                    self.set_frustum_offset(v);
                }
            }
            "near" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_near(v);
                }
            }
            "far" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_far(v);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// PhantomCameraNoise2D — `scripts/resources/phantom_camera_noise_2d.gd`
// ---------------------------------------------------------------------------

/// `phantom_camera_noise_2d.gd:1-228` — noise pattern for 2D camera shake.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct PhantomCameraNoise2D {
    base: Base<Resource>,

    /// `phantom_camera_noise_2d.gd:16-18`.
    #[export]
    #[var(get = get_amplitude, set = set_amplitude, hint = RANGE, hint_string = "0.0,1000.0,0.001,or_greater")]
    amplitude: f64,

    /// `phantom_camera_noise_2d.gd:22-24`.
    #[export]
    #[var(get = get_frequency, set = set_frequency, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater")]
    frequency: f64,

    /// `phantom_camera_noise_2d.gd:28-30`.
    #[export]
    #[var(get = get_randomize_noise_seed, set = set_randomize_noise_seed)]
    randomize_noise_seed: bool,

    /// `phantom_camera_noise_2d.gd:34-36`.
    #[export]
    #[var(get = get_noise_seed, set = set_noise_seed)]
    noise_seed: i64,

    /// `phantom_camera_noise_2d.gd:39-41`.
    #[export]
    #[var(get = get_positional_noise, set = set_positional_noise)]
    positional_noise: bool,

    /// `phantom_camera_noise_2d.gd:44-46`.
    #[export]
    #[var(get = get_rotational_noise, set = set_rotational_noise)]
    rotational_noise: bool,

    /// `phantom_camera_noise_2d.gd:51-53`.
    #[export]
    #[var(get = get_positional_multiplier_x, set = set_positional_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_x: f64,

    /// `phantom_camera_noise_2d.gd:57-59`.
    #[export]
    #[var(get = get_positional_multiplier_y, set = set_positional_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_y: f64,

    /// `phantom_camera_noise_2d.gd:63-65`.
    #[export]
    #[var(get = get_rotational_multiplier, set = set_rotational_multiplier, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier: f64,

    /// `phantom_camera_noise_2d.gd:71`.
    ///
    /// Created in `_init` with `FastNoiseLite.new()`; `#[class(init)]` cannot be
    /// used because the oracle's `_init` seeds the algorithm
    /// (`phantom_camera_noise_2d.gd:88-91`), so the field stays an `Option` and
    /// is filled by [`IResource::init`].
    noise_algorithm: Option<Gd<FastNoiseLite>>,

    /// `phantom_camera_noise_2d.gd:73-76` — mirrors the two exported multipliers.
    noise_positional_multiplier: Vector2,

    /// `phantom_camera_noise_2d.gd:78-80` — 2D does *not* reset `_noise_time`.
    trauma: f64,

    /// `phantom_camera_noise_2d.gd:82`.
    noise_time: f64,
}

#[godot_api]
impl IResource for PhantomCameraNoise2D {
    /// `phantom_camera_noise_2d.gd:88-91`.
    fn init(base: Base<Resource>) -> Self {
        // `FastNoiseLite` is an engine class, not creatable through `new()`.
        let mut noise_algorithm = FastNoiseLite::new_gd();
        noise_algorithm.set_noise_type(NoiseType::PERLIN);
        // `randomize_noise_seed` defaults to `true`, so `_init` always seeds.
        noise_algorithm.set_seed(truncate_seed(randi()));
        noise_algorithm.set_frequency(0.5);

        Self {
            base,
            amplitude: 10.0,
            frequency: 0.5,
            randomize_noise_seed: true,
            noise_seed: 0,
            positional_noise: true,
            rotational_noise: false,
            positional_multiplier_x: 1.0,
            positional_multiplier_y: 1.0,
            rotational_multiplier: 1.0,
            noise_algorithm: Some(noise_algorithm),
            noise_positional_multiplier: Vector2::new(1.0, 1.0),
            trauma: 0.0,
            noise_time: 0.0,
        }
    }
}

#[godot_api]
impl PhantomCameraNoise2D {
    /// `phantom_camera_noise_2d.gd:108-109` — one noise sample scaled by
    /// `amplitude`. The seed is the sample's X coordinate, `_noise_time` its Y.
    #[func]
    fn _get_noise_from_seed(&self, noise_seed: i64) -> f64 {
        self.noise_from_seed(noise_seed)
    }

    /// Body of `phantom_camera_noise_2d.gd:108-109` (see the `#[func]` wrapper
    /// above, which keeps the oracle's script-method name).
    fn noise_from_seed(&self, noise_seed: i64) -> f64 {
        let Some(algorithm) = self.noise_algorithm.as_ref() else {
            return 0.0;
        };
        let sample = algorithm.get_noise_2d(noise_seed as f32, self.noise_time as f32);
        f64::from(sample) * self.amplitude
    }

    /// `phantom_camera_noise_2d.gd:112-113` — the `_trauma` setter body. 2D only
    /// stores the value; contrasts with the 3D resource's zero-time reset.
    fn write_trauma(&mut self, value: f64) {
        self.trauma = value;
    }

    /// `phantom_camera_noise_2d.gd:119-131`.
    #[func]
    pub fn get_noise_transform(&mut self, delta: f64) -> Transform2D {
        let mut output_position = Vector2::ZERO;
        let mut output_rotation = 0.0_f64;

        self.noise_time += delta;
        self.write_trauma(self.trauma.max(0.0));

        let trauma_squared = self.trauma.powi(2);
        if self.positional_noise {
            for axis in 0..2_i64 {
                let multiplier = if axis == 0 {
                    self.noise_positional_multiplier.x
                } else {
                    self.noise_positional_multiplier.y
                };
                let value = f64::from(multiplier)
                    * trauma_squared
                    * self.noise_from_seed(axis + self.noise_seed);
                if axis == 0 {
                    output_position.x = real_of(value);
                } else {
                    output_position.y = real_of(value);
                }
            }
        }
        if self.rotational_noise {
            // `rotational_multiplier / 100`, not `deg_to_rad` (2D quirk).
            output_rotation = self.rotational_multiplier / 100.0
                * trauma_squared
                * self.noise_from_seed(self.noise_seed);
        }

        Transform2D::from_angle_origin(real_of(output_rotation), output_position)
    }

    /// `phantom_camera_noise_2d.gd:134-135`.
    #[func]
    pub fn reset_noise_time(&mut self) {
        self.noise_time = 0.0;
    }

    /// `phantom_camera_noise_2d.gd:112-113`.
    #[func]
    pub fn set_trauma(&mut self, value: f64) {
        self.write_trauma(value);
    }

    /// `phantom_camera_noise_2d.gd:142-143`.
    #[func]
    pub fn set_amplitude(&mut self, value: f64) {
        self.amplitude = value;
    }

    /// `phantom_camera_noise_2d.gd:146-147`.
    #[func]
    pub fn get_amplitude(&self) -> f64 {
        self.amplitude
    }

    /// `phantom_camera_noise_2d.gd:151-153`.
    #[func]
    pub fn set_frequency(&mut self, value: f64) {
        self.frequency = value;
        if let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_frequency(value as f32);
        }
    }

    /// `phantom_camera_noise_2d.gd:156-157`.
    #[func]
    pub fn get_frequency(&self) -> f64 {
        self.frequency
    }

    /// `phantom_camera_noise_2d.gd:161-164`.
    #[func]
    pub fn set_randomize_noise_seed(&mut self, value: bool) {
        self.randomize_noise_seed = value;
        if value && let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_seed(truncate_seed(randi()));
        }
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_2d.gd:167-168`.
    #[func]
    pub fn get_randomize_noise_seed(&self) -> bool {
        self.randomize_noise_seed
    }

    /// `phantom_camera_noise_2d.gd:172-173`.
    #[func]
    pub fn set_noise_seed(&mut self, value: i64) {
        self.noise_seed = value;
    }

    /// `phantom_camera_noise_2d.gd:176-177`.
    #[func]
    pub fn get_noise_seed(&self) -> i64 {
        self.noise_seed
    }

    /// `phantom_camera_noise_2d.gd:181-183`.
    #[func]
    pub fn set_positional_noise(&mut self, value: bool) {
        self.positional_noise = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_2d.gd:186-187`.
    #[func]
    pub fn get_positional_noise(&self) -> bool {
        self.positional_noise
    }

    /// `phantom_camera_noise_2d.gd:191-193`.
    #[func]
    pub fn set_rotational_noise(&mut self, value: bool) {
        self.rotational_noise = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_2d.gd:196-197`.
    #[func]
    pub fn get_rotational_noise(&self) -> bool {
        self.rotational_noise
    }

    /// `phantom_camera_noise_2d.gd:201-203`.
    #[func]
    pub fn set_positional_multiplier_x(&mut self, value: f64) {
        self.positional_multiplier_x = value;
        self.noise_positional_multiplier.x = real_of(value);
    }

    /// `phantom_camera_noise_2d.gd:206-207`.
    #[func]
    pub fn get_positional_multiplier_x(&self) -> f64 {
        self.positional_multiplier_x
    }

    /// `phantom_camera_noise_2d.gd:211-213`.
    #[func]
    pub fn set_positional_multiplier_y(&mut self, value: f64) {
        self.positional_multiplier_y = value;
        self.noise_positional_multiplier.y = real_of(value);
    }

    /// `phantom_camera_noise_2d.gd:216-217`.
    #[func]
    pub fn get_positional_multiplier_y(&self) -> f64 {
        self.positional_multiplier_y
    }

    /// `phantom_camera_noise_2d.gd:221-222`.
    #[func]
    pub fn set_rotational_multiplier(&mut self, value: f64) {
        self.rotational_multiplier = value;
    }

    /// `phantom_camera_noise_2d.gd:225-226`.
    #[func]
    pub fn get_rotational_multiplier(&self) -> f64 {
        self.rotational_multiplier
    }
}

// ---------------------------------------------------------------------------
// PhantomCameraNoise3D — `scripts/resources/phantom_camera_noise_3d.gd`
// ---------------------------------------------------------------------------

/// `phantom_camera_noise_3d.gd:1-301` — noise pattern for 3D camera shake.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct PhantomCameraNoise3D {
    base: Base<Resource>,

    /// `phantom_camera_noise_3d.gd:16-18`.
    #[export]
    #[var(get = get_amplitude, set = set_amplitude, hint = RANGE, hint_string = "0.0,100.0,0.001,or_greater")]
    amplitude: f64,

    /// `phantom_camera_noise_3d.gd:22-24`.
    #[export]
    #[var(get = get_frequency, set = set_frequency, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater")]
    frequency: f64,

    /// `phantom_camera_noise_3d.gd:28-30`.
    #[export]
    #[var(get = get_randomize_noise_seed, set = set_randomize_noise_seed)]
    randomize_noise_seed: bool,

    /// `phantom_camera_noise_3d.gd:34-36`.
    #[export]
    #[var(get = get_noise_seed, set = set_noise_seed)]
    noise_seed: i64,

    /// `phantom_camera_noise_3d.gd:39-41`.
    #[export]
    #[var(get = get_rotational_noise, set = set_rotational_noise)]
    rotational_noise: bool,

    /// `phantom_camera_noise_3d.gd:45-47`.
    #[export]
    #[var(get = get_positional_noise, set = set_positional_noise)]
    positional_noise: bool,

    /// `phantom_camera_noise_3d.gd:52-54`.
    #[export]
    #[var(get = get_rotational_multiplier_x, set = set_rotational_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_x: f64,

    /// `phantom_camera_noise_3d.gd:58-60`.
    #[export]
    #[var(get = get_rotational_multiplier_y, set = set_rotational_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_y: f64,

    /// `phantom_camera_noise_3d.gd:64-66`.
    #[export]
    #[var(get = get_rotational_multiplier_z, set = set_rotational_multiplier_z, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_z: f64,

    /// `phantom_camera_noise_3d.gd:72-74`.
    #[export]
    #[var(get = get_positional_multiplier_x, set = set_positional_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_x: f64,

    /// `phantom_camera_noise_3d.gd:79-81`.
    #[export]
    #[var(get = get_positional_multiplier_y, set = set_positional_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_y: f64,

    /// `phantom_camera_noise_3d.gd:86-88`.
    #[export]
    #[var(get = get_positional_multiplier_z, set = set_positional_multiplier_z, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_z: f64,

    /// `phantom_camera_noise_3d.gd:94`.
    noise_algorithm: Option<Gd<FastNoiseLite>>,

    /// `phantom_camera_noise_3d.gd:96-100`.
    noise_rotational_multiplier: Vector3,

    /// `phantom_camera_noise_3d.gd:102-106`.
    noise_positional_multiplier: Vector3,

    /// `phantom_camera_noise_3d.gd:108-114` — zeroing `_noise_time` at `0.0` is
    /// the documented 3D-only quirk.
    trauma: f64,

    /// `phantom_camera_noise_3d.gd:114`.
    noise_time: f64,
}

#[godot_api]
impl IResource for PhantomCameraNoise3D {
    /// `phantom_camera_noise_3d.gd:120-124`.
    fn init(base: Base<Resource>) -> Self {
        let mut noise_algorithm = FastNoiseLite::new_gd();
        noise_algorithm.set_noise_type(NoiseType::PERLIN);
        noise_algorithm.set_seed(truncate_seed(randi()));
        noise_algorithm.set_frequency(0.2);

        Self {
            base,
            amplitude: 10.0,
            frequency: 0.2,
            randomize_noise_seed: true,
            noise_seed: 0,
            rotational_noise: true,
            positional_noise: false,
            rotational_multiplier_x: 1.0,
            rotational_multiplier_y: 1.0,
            rotational_multiplier_z: 1.0,
            positional_multiplier_x: 1.0,
            positional_multiplier_y: 1.0,
            positional_multiplier_z: 1.0,
            noise_algorithm: Some(noise_algorithm),
            noise_rotational_multiplier: Vector3::ONE,
            noise_positional_multiplier: Vector3::ONE,
            trauma: 0.0,
            noise_time: 0.0,
        }
    }
}

#[godot_api]
impl PhantomCameraNoise3D {
    /// `phantom_camera_noise_3d.gd:146-147` — one noise sample scaled by
    /// `amplitude` (3D uses the same 2D sample the oracle does).
    #[func]
    fn _get_noise_from_seed(&self, noise_seed: i64) -> f64 {
        self.noise_from_seed(noise_seed)
    }

    /// Body of `phantom_camera_noise_3d.gd:146-147`.
    fn noise_from_seed(&self, noise_seed: i64) -> f64 {
        let Some(algorithm) = self.noise_algorithm.as_ref() else {
            return 0.0;
        };
        let sample = algorithm.get_noise_2d(noise_seed as f32, self.noise_time as f32);
        f64::from(sample) * self.amplitude
    }

    /// `phantom_camera_noise_3d.gd:108-114` — storing `0.0` resets the noise
    /// clock, so a stopped shake restarts from the same pattern.
    fn write_trauma(&mut self, value: f64) {
        self.trauma = value;
        if self.trauma == 0.0 {
            self.noise_time = 0.0;
        }
    }

    /// `phantom_camera_noise_3d.gd:157-173`.
    #[func]
    pub fn get_noise_transform(&mut self, delta: f64) -> Transform3D {
        let mut output_rotation = Vector3::ZERO;
        let mut output_position = Vector3::ZERO;

        self.noise_time += delta;
        self.write_trauma(self.trauma.max(0.0));

        let trauma_squared = self.trauma.powi(2);
        let rotational = [
            f64::from(self.noise_rotational_multiplier.x),
            f64::from(self.noise_rotational_multiplier.y),
            f64::from(self.noise_rotational_multiplier.z),
        ];
        let positional = [
            f64::from(self.noise_positional_multiplier.x),
            f64::from(self.noise_positional_multiplier.y),
            f64::from(self.noise_positional_multiplier.z),
        ];

        let mut rotation = [0.0_f64; 3];
        let mut position = [0.0_f64; 3];
        for axis in 0..3_usize {
            let seed = axis as i64 + self.noise_seed;
            if self.rotational_noise {
                // 3D converts through `deg_to_rad` (2D divides by 100).
                rotation[axis] = godot::global::deg_to_rad(
                    rotational[axis] * trauma_squared * self.noise_from_seed(seed),
                );
            }
            if self.positional_noise {
                // 3D scales the positional multiplier by `1 / 10`.
                position[axis] +=
                    positional[axis] / 10.0 * trauma_squared * self.noise_from_seed(seed);
            }
        }

        output_rotation.x = real_of(rotation[0]);
        output_rotation.y = real_of(rotation[1]);
        output_rotation.z = real_of(rotation[2]);
        output_position.x = real_of(position[0]);
        output_position.y = real_of(position[1]);
        output_position.z = real_of(position[2]);

        Transform3D::new(
            Basis::from_quaternion(Quaternion::from_euler(output_rotation)),
            output_position,
        )
    }

    /// `phantom_camera_noise_3d.gd:176-177`.
    #[func]
    pub fn reset_noise_time(&mut self) {
        self.noise_time = 0.0;
    }

    /// `phantom_camera_noise_3d.gd:150-151`.
    #[func]
    pub fn set_trauma(&mut self, value: f64) {
        self.write_trauma(value);
    }

    /// `phantom_camera_noise_3d.gd:184-185`.
    #[func]
    pub fn set_amplitude(&mut self, value: f64) {
        self.amplitude = value;
    }

    /// `phantom_camera_noise_3d.gd:188-189`.
    #[func]
    pub fn get_amplitude(&self) -> f64 {
        self.amplitude
    }

    /// `phantom_camera_noise_3d.gd:193-195`.
    #[func]
    pub fn set_frequency(&mut self, value: f64) {
        self.frequency = value;
        if let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_frequency(value as f32);
        }
    }

    /// `phantom_camera_noise_3d.gd:198-199`.
    #[func]
    pub fn get_frequency(&self) -> f64 {
        self.frequency
    }

    /// `phantom_camera_noise_3d.gd:203-206`.
    #[func]
    pub fn set_randomize_noise_seed(&mut self, value: bool) {
        self.randomize_noise_seed = value;
        if value && let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_seed(truncate_seed(randi()));
        }
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_3d.gd:209-210`.
    #[func]
    pub fn get_randomize_noise_seed(&self) -> bool {
        self.randomize_noise_seed
    }

    /// `phantom_camera_noise_3d.gd:214-215`.
    #[func]
    pub fn set_noise_seed(&mut self, value: i64) {
        self.noise_seed = value;
    }

    /// `phantom_camera_noise_3d.gd:218-219`.
    #[func]
    pub fn get_noise_seed(&self) -> i64 {
        self.noise_seed
    }

    /// `phantom_camera_noise_3d.gd:223-225`.
    #[func]
    pub fn set_positional_noise(&mut self, value: bool) {
        self.positional_noise = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_3d.gd:228-229`.
    #[func]
    pub fn get_positional_noise(&self) -> bool {
        self.positional_noise
    }

    /// `phantom_camera_noise_3d.gd:233-235`.
    #[func]
    pub fn set_rotational_noise(&mut self, value: bool) {
        self.rotational_noise = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_3d.gd:238-239`.
    #[func]
    pub fn get_rotational_noise(&self) -> bool {
        self.rotational_noise
    }

    /// `phantom_camera_noise_3d.gd:243-245`.
    #[func]
    pub fn set_positional_multiplier_x(&mut self, value: f64) {
        self.positional_multiplier_x = value;
        self.noise_positional_multiplier.x = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:248-249`.
    #[func]
    pub fn get_positional_multiplier_x(&self) -> f64 {
        self.positional_multiplier_x
    }

    /// `phantom_camera_noise_3d.gd:253-255`.
    #[func]
    pub fn set_positional_multiplier_y(&mut self, value: f64) {
        self.positional_multiplier_y = value;
        self.noise_positional_multiplier.y = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:258-259`.
    #[func]
    pub fn get_positional_multiplier_y(&self) -> f64 {
        self.positional_multiplier_y
    }

    /// `phantom_camera_noise_3d.gd:263-265`.
    #[func]
    pub fn set_positional_multiplier_z(&mut self, value: f64) {
        self.positional_multiplier_z = value;
        self.noise_positional_multiplier.z = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:268-269`.
    #[func]
    pub fn get_positional_multiplier_z(&self) -> f64 {
        self.positional_multiplier_z
    }

    /// `phantom_camera_noise_3d.gd:273-275`.
    #[func]
    pub fn set_rotational_multiplier_x(&mut self, value: f64) {
        self.rotational_multiplier_x = value;
        self.noise_rotational_multiplier.x = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:278-279`.
    #[func]
    pub fn get_rotational_multiplier_x(&self) -> f64 {
        self.rotational_multiplier_x
    }

    /// `phantom_camera_noise_3d.gd:283-285`.
    #[func]
    pub fn set_rotational_multiplier_y(&mut self, value: f64) {
        self.rotational_multiplier_y = value;
        self.noise_rotational_multiplier.y = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:288-289`.
    #[func]
    pub fn get_rotational_multiplier_y(&self) -> f64 {
        self.rotational_multiplier_y
    }

    /// `phantom_camera_noise_3d.gd:293-295`.
    #[func]
    pub fn set_rotational_multiplier_z(&mut self, value: f64) {
        self.rotational_multiplier_z = value;
        self.noise_rotational_multiplier.z = real_of(value);
    }

    /// `phantom_camera_noise_3d.gd:298-299`.
    #[func]
    pub fn get_rotational_multiplier_z(&self) -> f64 {
        self.rotational_multiplier_z
    }
}

/// `FastNoiseLite.seed` is a 32-bit `int`; GDScript narrows the 0..2^32 `randi()`
/// result when assigning, which is a two's-complement truncation.
#[inline]
fn truncate_seed(seed: i64) -> i32 {
    seed as i32
}

// ---------------------------------------------------------------------------
// TweenDirectorResource — `scripts/resources/tween_director_resource.gd`
// ---------------------------------------------------------------------------

/// `tween_director_resource.gd:1-155` — conditional `PhantomCameraTween`
/// override keyed on the from/to cameras of a transition.
///
/// All seven fields are `pub`: `host.rs` matches them against the active/inactive
/// PCam and needs to walk the arrays without round-tripping through Godot.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct TweenDirectorResource {
    base: Base<Resource>,

    /// `tween_director_resource.gd:27-29`.
    #[export]
    #[var(get = get_tween_resource, set = set_tween_resource)]
    pub tween_resource: Option<Gd<PhantomCameraTween>>,

    /// `tween_director_resource.gd:35-37` — `Type` (`abi::TweenDirectorMatch`).
    #[export]
    #[var(get = get_from_type, set = set_from_type, hint = ENUM, hint_string = "Phantom Camera:0,Tween Resource:1,Any:2")]
    pub from_type: i64,

    /// `tween_director_resource.gd:40-42`.
    #[export(node_path = ("PhantomCamera2D", "PhantomCamera3D"))]
    #[var(get = get_from_phantom_cameras, set = set_from_phantom_cameras)]
    pub from_phantom_cameras: Array<NodePath>,

    /// `tween_director_resource.gd:47-49`.
    #[export]
    #[var(get = get_from_tween_resources, set = set_from_tween_resources)]
    pub from_tween_resources: Array<Gd<PhantomCameraTween>>,

    /// `tween_director_resource.gd:54-56`.
    #[export]
    #[var(get = get_to_type, set = set_to_type, hint = ENUM, hint_string = "Phantom Camera:0,Tween Resource:1,Any:2")]
    pub to_type: i64,

    /// `tween_director_resource.gd:59-61`.
    #[export(node_path = ("PhantomCamera2D", "PhantomCamera3D"))]
    #[var(get = get_to_phantom_cameras, set = set_to_phantom_cameras)]
    pub to_phantom_cameras: Array<NodePath>,

    /// `tween_director_resource.gd:66-68`.
    #[export]
    #[var(get = get_to_tween_resources, set = set_to_tween_resources)]
    pub to_tween_resources: Array<Gd<PhantomCameraTween>>,
}

#[godot_api]
impl IResource for TweenDirectorResource {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            tween_resource: None,
            from_type: abi::TweenDirectorMatch::PhantomCamera as i64,
            from_phantom_cameras: Array::new(),
            from_tween_resources: Array::new(),
            to_type: abi::TweenDirectorMatch::PhantomCamera as i64,
            to_phantom_cameras: Array::new(),
            to_tween_resources: Array::new(),
        }
    }
}

#[godot_api]
impl TweenDirectorResource {
    /// `tween_director_resource.gd:92-93`.
    #[func]
    pub fn set_tween_resource(&mut self, value: Option<Gd<PhantomCameraTween>>) {
        self.tween_resource = value;
    }

    /// `tween_director_resource.gd:96-97`.
    #[func]
    pub fn get_tween_resource(&self) -> Option<Gd<PhantomCameraTween>> {
        self.tween_resource.clone()
    }

    /// `tween_director_resource.gd:101-103`.
    #[func]
    pub fn set_from_type(&mut self, value: i64) {
        self.from_type = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `tween_director_resource.gd:106-107`.
    #[func]
    pub fn get_from_type(&self) -> i64 {
        self.from_type
    }

    /// `tween_director_resource.gd:111-112`.
    #[func]
    pub fn set_from_phantom_cameras(&mut self, value: Array<NodePath>) {
        self.from_phantom_cameras = value;
    }

    /// `tween_director_resource.gd:115-116`.
    #[func]
    pub fn get_from_phantom_cameras(&self) -> Array<NodePath> {
        self.from_phantom_cameras.clone()
    }

    /// `tween_director_resource.gd:120-121`.
    #[func]
    pub fn set_from_tween_resources(&mut self, value: Array<Gd<PhantomCameraTween>>) {
        self.from_tween_resources = value;
    }

    /// `tween_director_resource.gd:124-125`.
    #[func]
    pub fn get_from_tween_resources(&self) -> Array<Gd<PhantomCameraTween>> {
        self.from_tween_resources.clone()
    }

    /// `tween_director_resource.gd:129-131`.
    #[func]
    pub fn set_to_type(&mut self, value: i64) {
        self.to_type = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `tween_director_resource.gd:134-135`.
    #[func]
    pub fn get_to_type(&self) -> i64 {
        self.to_type
    }

    /// `tween_director_resource.gd:139-140`.
    #[func]
    pub fn set_to_phantom_cameras(&mut self, value: Array<NodePath>) {
        self.to_phantom_cameras = value;
    }

    /// `tween_director_resource.gd:143-144`.
    #[func]
    pub fn get_to_phantom_cameras(&self) -> Array<NodePath> {
        self.to_phantom_cameras.clone()
    }

    /// `tween_director_resource.gd:148-149`.
    #[func]
    pub fn set_to_tween_resources(&mut self, value: Array<Gd<PhantomCameraTween>>) {
        self.to_tween_resources = value;
    }

    /// `tween_director_resource.gd:152-153`.
    #[func]
    pub fn get_to_tween_resources(&self) -> Array<Gd<PhantomCameraTween>> {
        self.to_tween_resources.clone()
    }
}
