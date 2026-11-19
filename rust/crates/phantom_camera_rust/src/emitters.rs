// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/phantom_camera/phantom_camera_noise_emitter_2d.gd, phantom_camera_noise_emitter_3d.gd
//
//! Noise emitter nodes — the trauma grow/duration/decay state machine that
//! feeds `PhantomCameraManager.noise_2d_emitted` / `noise_3d_emitted`.
//!
//! Ported from the GDScript oracle (v0.11.0.3, read-only at
//! `parity/pcam_oracle/phantom_camera/`):
//!
//! | Class | Oracle file | Anchors |
//! |---|---|---|
//! | [`PhantomCameraNoiseEmitter2D`] | `scripts/phantom_camera/phantom_camera_noise_emitter_2d.gd` (264 L) | exports `:20-59`, `_play` setter `:66-83`, `_enter_tree` `:116-117`, `_process` `:120-156`, `_set_layer` `:159-171`, public API `:179-197`, setters `:204-262` |
//! | [`PhantomCameraNoiseEmitter3D`] | `scripts/phantom_camera/phantom_camera_noise_emitter_3d.gd` (265 L) | exports `:20-62`, `_play` setter `:68-85`, `_enter_tree` `:118-119`, `_process` `:122-156`, `_set_layer` `:159-171`, public API `:178-198`, setters `:205-263` |
//!
//! Both emitters emit through [`PhantomCameraManager::notify_noise_emitted`]
//! instead of caching `_phantom_camera_manager` in `_enter_tree`
//! (`..._emitter_2d.gd:117`, `..._emitter_3d.gd:119`): the seam resolves the
//! manager per emission and is a silent no-op when none is registered, which is
//! the same observable behaviour for the addon's own scenes (the manager autoload
//! always exists) without a stale `Gd` reference.
//!
//! # Behaviour preserved on purpose
//!
//! * `growth_time == 0` divides by zero in the oracle, so `min(inf, 1) == 1`
//!   (`..._emitter_2d.gd:130`). Guarded explicitly here — see the
//!   `growth_trauma` helpers — because a Rust `f64 / 0.0` is only `inf` for a
//!   positive numerator and `NaN` for `0.0 / 0.0`; the guard makes both cases
//!   `1.0`, exactly like the float path the oracle relies on.
//! * `set_duration` forces `0.0` to `0.001` (`..._emitter_2d.gd:235-236`) so the
//!   duration countdown cannot stall at zero.
//! * The `_play` setter's stop path (`..._emitter_2d.gd:76-81`) reseeds the
//!   noise resource (`noise_seed = randi() & 1000`) when the resource randomises
//!   its seed, otherwise resets its noise clock. It runs twice per completed
//!   decay, because the oracle's `_process` assigns `_play = false` and then
//!   `preview = false`, which assigns `_play = false` again
//!   (`..._emitter_2d.gd:149-150`) — the port calls the same setters in the same
//!   order so the RNG sees the same number of draws.
//! * `preview` is an alias for `_play` (`..._emitter_2d.gd:25-30`): both
//!   setters drive the same state machine.
//! * The 2D `_process` has an `else: noise.set_trauma(1)` branch that the 3D one
//!   does **not** (`..._emitter_2d.gd:135-136` vs `..._emitter_3d.gd:131-136`).
//!   That asymmetry is an oracle bug and is ported as-is.
//!
//! # Deliberately omitted (editor-only)
//!
//! * `@icon(...)`, `_get_configuration_warnings` (`..._emitter_2d.gd:104-108`)
//!   and `_validate_property` (`:111-113`) — inspector warnings and the
//!   `duration`-hidden-while-continuous rule; no runtime effect.
//! * The private state variables (`_play`, `_should_grow`, `_elasped_play_time`,
//!   `_noise_output`, …) are plain Rust fields, so they are absent from
//!   `get_property_list`. They are `PROPERTY_USAGE_SCRIPT_VARIABLE`-only in the
//!   oracle and never stored in a `.tscn`.
//! * `_noise_output` (the last emitted transform) is not stored at all: nothing
//!   in the addon reads it back apart from the signal emission itself.

use godot::classes::{INode2D, INode3D, Node2D, Node3D};
use godot::global::randi;
use godot::prelude::*;

use crate::abi::PcamKind;
use crate::manager::PhantomCameraManager;
use crate::resources::{PhantomCameraNoise2D, PhantomCameraNoise3D};

/// `phantom_camera_noise_emitter_2d.gd:1-264`.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct PhantomCameraNoiseEmitter2D {
    base: Base<Node2D>,

    /// `phantom_camera_noise_emitter_2d.gd:20-22`.
    #[export]
    #[var(get = get_noise, set = set_noise)]
    noise: Option<Gd<PhantomCameraNoise2D>>,

    /// `phantom_camera_noise_emitter_2d.gd:25-30` — alias for `_play`.
    #[export]
    #[var(get = get_preview, set = set_preview)]
    preview: bool,

    /// `phantom_camera_noise_emitter_2d.gd:33-35`.
    #[export]
    #[var(get = get_continuous, set = set_continuous)]
    continuous: bool,

    /// `phantom_camera_noise_emitter_2d.gd:39-41`.
    #[export]
    #[var(get = get_growth_time, set = set_growth_time, hint = EXP_EASING, hint_string = "positive_only,suffix: s")]
    growth_time: f64,

    /// `phantom_camera_noise_emitter_2d.gd:45-47`.
    #[export]
    #[var(get = get_duration, set = set_duration, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater,suffix: s")]
    duration: f64,

    /// `phantom_camera_noise_emitter_2d.gd:51-53`.
    #[export]
    #[var(get = get_decay_time, set = set_decay_time, hint = EXP_EASING, hint_string = "attenuation,positive_only,suffix: s")]
    decay_time: f64,

    /// `phantom_camera_noise_emitter_2d.gd:57-59`.
    #[export(flags_2d_render)]
    #[var(get = get_noise_emitter_layer, set = set_noise_emitter_layer)]
    noise_emitter_layer: i64,

    /// `phantom_camera_noise_emitter_2d.gd:66-83`.
    play: bool,

    /// `phantom_camera_noise_emitter_2d.gd:85`.
    start_duration_countdown: bool,

    /// `phantom_camera_noise_emitter_2d.gd:87`.
    decay_countdown: f64,

    /// `phantom_camera_noise_emitter_2d.gd:89`.
    should_grow: bool,

    /// `phantom_camera_noise_emitter_2d.gd:91`.
    should_decay: bool,

    /// `phantom_camera_noise_emitter_2d.gd:93` (`_elasped_play_time`, sic).
    elapsed_play_time: f64,
}

#[godot_api]
impl INode2D for PhantomCameraNoiseEmitter2D {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            noise: None,
            preview: false,
            continuous: false,
            growth_time: 0.0,
            duration: 1.0,
            decay_time: 0.0,
            noise_emitter_layer: 1,
            play: false,
            start_duration_countdown: false,
            decay_countdown: 0.0,
            should_grow: false,
            should_decay: false,
            elapsed_play_time: 0.0,
        }
    }

    /// `phantom_camera_noise_emitter_2d.gd:120-156`.
    fn process(&mut self, delta: f64) {
        if !self.play && !self.should_decay {
            return;
        }

        let Some(mut noise) = self.noise.clone() else {
            godot_error!(
                "Noise resource missing in {}",
                self.base().get_name().to_string()
            );
            self.write_play(false);
            return;
        };

        self.elapsed_play_time += delta;

        if self.should_grow {
            let trauma = growth_trauma(self.elapsed_play_time, self.growth_time);
            noise.bind_mut().set_trauma(trauma);
            if self.elapsed_play_time >= self.growth_time {
                self.should_grow = false;
                self.start_duration_countdown = true;
                noise.bind_mut().set_trauma(1.0);
            }
        } else {
            // `:135-136` — present in 2D, absent in 3D (oracle asymmetry).
            noise.bind_mut().set_trauma(1.0);
        }

        if !self.continuous
            && self.start_duration_countdown
            && self.elapsed_play_time >= self.duration + self.growth_time
        {
            self.should_decay = true;
            self.start_duration_countdown = false;
        }

        if self.should_decay {
            self.decay_countdown += delta;
            let trauma = decay_trauma(self.decay_countdown, self.decay_time);
            noise.bind_mut().set_trauma(trauma);
            if self.decay_countdown >= self.decay_time {
                noise.bind_mut().set_trauma(0.0);
                self.write_play(false);
                self.set_preview(false);
                self.should_decay = false;
                self.elapsed_play_time = 0.0;
                self.decay_countdown = 0.0;
            }
        }

        let output = noise.bind_mut().get_noise_transform(delta);
        PhantomCameraManager::notify_noise_emitted(
            PcamKind::TwoD,
            output.to_variant(),
            self.noise_emitter_layer,
        );
    }
}

#[godot_api]
impl PhantomCameraNoiseEmitter2D {
    /// `phantom_camera_noise_emitter_2d.gd:66-83` — the `_play` property setter.
    ///
    /// Assigning `true` (re)starts the grow phase; assigning `false` requests a
    /// decay and re-rolls the noise resource's seed.
    fn write_play(&mut self, value: bool) {
        // `_play = value` is the setter's first statement
        // (`phantom_camera_noise_emitter_2d.gd:69`), so both branches update the
        // backing field before running their side effects.
        self.play = value;
        if value {
            self.elapsed_play_time = 0.0;
            self.decay_countdown = 0.0;
            self.play = true;
            self.should_grow = true;
            self.start_duration_countdown = false;
            self.should_decay = false;
        } else {
            self.should_decay = true;
            if let Some(mut noise) = self.noise.clone() {
                let randomize = noise.bind().get_randomize_noise_seed();
                let mut noise = noise.bind_mut();
                if randomize {
                    // `:79` — `randi() & 1000`, a mask, not a modulo.
                    noise.set_noise_seed(randi() & 1000);
                } else {
                    noise.reset_noise_time();
                }
            }
        }
    }

    /// `phantom_camera_noise_emitter_2d.gd:159-171`.
    #[func]
    fn _set_layer(&self, current_layers: i64, layer_number: i64, value: bool) -> i64 {
        let mut mask = current_layers;
        if !(1..=20).contains(&layer_number) {
            godot_error!("Layer must be between 1 and 20.");
        } else if value {
            mask |= 1 << (layer_number - 1);
        } else {
            mask &= !(1 << (layer_number - 1));
        }
        mask
    }

    /// `phantom_camera_noise_emitter_2d.gd:179-181`.
    #[func]
    pub fn emit(&mut self) {
        if self.play {
            self.write_play(false);
        }
        self.write_play(true);
    }

    /// `phantom_camera_noise_emitter_2d.gd:184-185`.
    #[func]
    pub fn is_emitting(&self) -> bool {
        self.play
    }

    /// `phantom_camera_noise_emitter_2d.gd:188-192`.
    ///
    /// The oracle declares `should_decay := true` as a default argument;
    /// GDExtension cannot publish default arguments, so the parameter is
    /// required and the caller passes the same `true` explicitly.
    #[func]
    pub fn stop(&mut self, should_decay: bool) {
        if should_decay {
            self.should_decay = true;
        } else {
            self.write_play(false);
        }
    }

    /// `phantom_camera_noise_emitter_2d.gd:195-196`.
    #[func]
    pub fn toggle(&mut self) {
        let next = !self.play;
        self.write_play(next);
    }

    /// `phantom_camera_noise_emitter_2d.gd:204-206`.
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise2D>>) {
        self.noise = value;
        // `:206` calls `update_configuration_warnings()`, editor-only.
    }

    /// `phantom_camera_noise_emitter_2d.gd:209-210`.
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise2D>> {
        self.noise.clone()
    }

    /// `phantom_camera_noise_emitter_2d.gd:214-216`.
    #[func]
    pub fn set_continuous(&mut self, value: bool) {
        self.continuous = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_emitter_2d.gd:219-220`.
    #[func]
    pub fn get_continuous(&self) -> bool {
        self.continuous
    }

    /// `phantom_camera_noise_emitter_2d.gd:224-225`.
    #[func]
    pub fn set_growth_time(&mut self, value: f64) {
        self.growth_time = value;
    }

    /// `phantom_camera_noise_emitter_2d.gd:228-229`.
    #[func]
    pub fn get_growth_time(&self) -> f64 {
        self.growth_time
    }

    /// `phantom_camera_noise_emitter_2d.gd:233-236`.
    #[func]
    pub fn set_duration(&mut self, value: f64) {
        self.duration = value;
        if self.duration == 0.0 {
            self.duration = 0.001;
        }
    }

    /// `phantom_camera_noise_emitter_2d.gd:239-240`.
    #[func]
    pub fn get_duration(&self) -> f64 {
        self.duration
    }

    /// `phantom_camera_noise_emitter_2d.gd:244-245`.
    #[func]
    pub fn set_decay_time(&mut self, value: f64) {
        self.decay_time = value;
    }

    /// `phantom_camera_noise_emitter_2d.gd:248-249`.
    #[func]
    pub fn get_decay_time(&self) -> f64 {
        self.decay_time
    }

    /// `phantom_camera_noise_emitter_2d.gd:253-254`.
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }

    /// `phantom_camera_noise_emitter_2d.gd:257-258`.
    #[func]
    pub fn set_noise_emitter_value(&mut self, value: i64, enabled: bool) {
        self.noise_emitter_layer = self._set_layer(self.noise_emitter_layer, value, enabled);
    }

    /// `phantom_camera_noise_emitter_2d.gd:261-262`.
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }

    /// `phantom_camera_noise_emitter_2d.gd:25-30` — the `preview` setter.
    ///
    /// The oracle's accessor is an inline lambda (`@preview_setter`); gdext needs
    /// a named `#[func]` for a custom property accessor, so this registers as
    /// `set_preview` instead.
    #[func]
    fn set_preview(&mut self, value: bool) {
        self.preview = value;
        self.write_play(value);
    }

    /// `phantom_camera_noise_emitter_2d.gd:29-30` — the `preview` getter.
    #[func]
    fn get_preview(&self) -> bool {
        self.preview
    }
}

/// `phantom_camera_noise_emitter_3d.gd:1-265`.
#[derive(GodotClass)]
#[class(base=Node3D, tool)]
pub struct PhantomCameraNoiseEmitter3D {
    base: Base<Node3D>,

    /// `phantom_camera_noise_emitter_3d.gd:20-22`.
    #[export]
    #[var(get = get_noise, set = set_noise)]
    noise: Option<Gd<PhantomCameraNoise3D>>,

    /// `phantom_camera_noise_emitter_3d.gd:25-30` — alias for `_play`.
    #[export]
    #[var(get = get_preview, set = set_preview)]
    preview: bool,

    /// `phantom_camera_noise_emitter_3d.gd:35-37`.
    #[export]
    #[var(get = get_continuous, set = set_continuous)]
    continuous: bool,

    /// `phantom_camera_noise_emitter_3d.gd:41-43`.
    #[export]
    #[var(get = get_growth_time, set = set_growth_time, hint = EXP_EASING, hint_string = "positive_only,suffix: s")]
    growth_time: f64,

    /// `phantom_camera_noise_emitter_3d.gd:48-50`.
    #[export]
    #[var(get = get_duration, set = set_duration, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater,suffix: s")]
    duration: f64,

    /// `phantom_camera_noise_emitter_3d.gd:54-56`.
    #[export]
    #[var(get = get_decay_time, set = set_decay_time, hint = EXP_EASING, hint_string = "attenuation,positive_only,suffix: s")]
    decay_time: f64,

    /// `phantom_camera_noise_emitter_3d.gd:60-62`.
    #[export(flags_3d_render)]
    #[var(get = get_noise_emitter_layer, set = set_noise_emitter_layer)]
    noise_emitter_layer: i64,

    /// `phantom_camera_noise_emitter_3d.gd:68-85`.
    play: bool,

    /// `phantom_camera_noise_emitter_3d.gd:87`.
    start_duration_countdown: bool,

    /// `phantom_camera_noise_emitter_3d.gd:89`.
    decay_countdown: f64,

    /// `phantom_camera_noise_emitter_3d.gd:91`.
    should_grow: bool,

    /// `phantom_camera_noise_emitter_3d.gd:93`.
    should_decay: bool,

    /// `phantom_camera_noise_emitter_3d.gd:95` (`_elasped_play_time`, sic).
    elapsed_play_time: f64,
}

#[godot_api]
impl INode3D for PhantomCameraNoiseEmitter3D {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            noise: None,
            preview: false,
            continuous: false,
            growth_time: 0.0,
            duration: 1.0,
            decay_time: 0.0,
            noise_emitter_layer: 1,
            play: false,
            start_duration_countdown: false,
            decay_countdown: 0.0,
            should_grow: false,
            should_decay: false,
            elapsed_play_time: 0.0,
        }
    }

    /// `phantom_camera_noise_emitter_3d.gd:122-156`.
    fn process(&mut self, delta: f64) {
        if !self.play && !self.should_decay {
            return;
        }

        let Some(mut noise) = self.noise.clone() else {
            godot_error!(
                "Noise resource missing in {}",
                self.base().get_name().to_string()
            );
            self.write_play(false);
            return;
        };

        self.elapsed_play_time += delta;

        if self.should_grow {
            let trauma = growth_trauma(self.elapsed_play_time, self.growth_time);
            noise.bind_mut().set_trauma(trauma);
            if self.elapsed_play_time >= self.growth_time {
                self.should_grow = false;
                self.start_duration_countdown = true;
                noise.bind_mut().set_trauma(1.0);
            }
        }
        // NOTE: the 3D oracle has no `else: noise.set_trauma(1)` branch here
        // (`phantom_camera_noise_emitter_3d.gd:131-136`); parity keeps it out.

        if !self.continuous
            && self.start_duration_countdown
            && self.elapsed_play_time >= self.duration + self.growth_time
        {
            self.should_decay = true;
            self.start_duration_countdown = false;
        }

        if self.should_decay {
            self.decay_countdown += delta;
            let trauma = decay_trauma(self.decay_countdown, self.decay_time);
            noise.bind_mut().set_trauma(trauma);
            if self.decay_countdown >= self.decay_time {
                noise.bind_mut().set_trauma(0.0);
                self.write_play(false);
                self.set_preview(false);
                self.should_decay = false;
                self.elapsed_play_time = 0.0;
                self.decay_countdown = 0.0;
            }
        }

        let output = noise.bind_mut().get_noise_transform(delta);
        PhantomCameraManager::notify_noise_emitted(
            PcamKind::ThreeD,
            output.to_variant(),
            self.noise_emitter_layer,
        );
    }
}

#[godot_api]
impl PhantomCameraNoiseEmitter3D {
    /// `phantom_camera_noise_emitter_3d.gd:68-85` — the `_play` property setter.
    fn write_play(&mut self, value: bool) {
        // `_play = value` is the setter's first statement
        // (`phantom_camera_noise_emitter_3d.gd:71`), so both branches update the
        // backing field before running their side effects.
        self.play = value;
        if value {
            self.elapsed_play_time = 0.0;
            self.decay_countdown = 0.0;
            self.play = true;
            self.should_grow = true;
            self.start_duration_countdown = false;
            self.should_decay = false;
        } else {
            self.should_decay = true;
            if let Some(mut noise) = self.noise.clone() {
                let randomize = noise.bind().get_randomize_noise_seed();
                let mut noise = noise.bind_mut();
                if randomize {
                    // `:81` — `randi() & 1000`, a mask, not a modulo.
                    noise.set_noise_seed(randi() & 1000);
                } else {
                    noise.reset_noise_time();
                }
            }
        }
    }

    /// `phantom_camera_noise_emitter_3d.gd:159-171`.
    #[func]
    fn _set_layer(&self, current_layers: i64, layer_number: i64, value: bool) -> i64 {
        let mut mask = current_layers;
        if !(1..=20).contains(&layer_number) {
            godot_error!("Layer must be between 1 and 20.");
        } else if value {
            mask |= 1 << (layer_number - 1);
        } else {
            mask &= !(1 << (layer_number - 1));
        }
        mask
    }

    /// `phantom_camera_noise_emitter_3d.gd:178-180`.
    #[func]
    pub fn emit(&mut self) {
        if self.play {
            self.write_play(false);
        }
        self.write_play(true);
    }

    /// `phantom_camera_noise_emitter_3d.gd:184-185`.
    #[func]
    pub fn is_emitting(&self) -> bool {
        self.play
    }

    /// `phantom_camera_noise_emitter_3d.gd:189-193`.
    ///
    /// `should_decay := true` in the oracle; GDExtension cannot publish default
    /// arguments, so the parameter is required.
    #[func]
    pub fn stop(&mut self, should_decay: bool) {
        if should_decay {
            self.should_decay = true;
        } else {
            self.write_play(false);
        }
    }

    /// `phantom_camera_noise_emitter_3d.gd:197-198`.
    #[func]
    pub fn toggle(&mut self) {
        let next = !self.play;
        self.write_play(next);
    }

    /// `phantom_camera_noise_emitter_3d.gd:205-207`.
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise3D>>) {
        self.noise = value;
        // `:207` calls `update_configuration_warnings()`, editor-only.
    }

    /// `phantom_camera_noise_emitter_3d.gd:210-211`.
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise3D>> {
        self.noise.clone()
    }

    /// `phantom_camera_noise_emitter_3d.gd:215-217`.
    #[func]
    pub fn set_continuous(&mut self, value: bool) {
        self.continuous = value;
        self.base_mut().notify_property_list_changed();
    }

    /// `phantom_camera_noise_emitter_3d.gd:220-221`.
    #[func]
    pub fn get_continuous(&self) -> bool {
        self.continuous
    }

    /// `phantom_camera_noise_emitter_3d.gd:225-226`.
    #[func]
    pub fn set_growth_time(&mut self, value: f64) {
        self.growth_time = value;
    }

    /// `phantom_camera_noise_emitter_3d.gd:229-230`.
    #[func]
    pub fn get_growth_time(&self) -> f64 {
        self.growth_time
    }

    /// `phantom_camera_noise_emitter_3d.gd:234-237`.
    #[func]
    pub fn set_duration(&mut self, value: f64) {
        self.duration = value;
        if self.duration == 0.0 {
            self.duration = 0.001;
        }
    }

    /// `phantom_camera_noise_emitter_3d.gd:240-241`.
    #[func]
    pub fn get_duration(&self) -> f64 {
        self.duration
    }

    /// `phantom_camera_noise_emitter_3d.gd:245-246`.
    #[func]
    pub fn set_decay_time(&mut self, value: f64) {
        self.decay_time = value;
    }

    /// `phantom_camera_noise_emitter_3d.gd:249-250`.
    #[func]
    pub fn get_decay_time(&self) -> f64 {
        self.decay_time
    }

    /// `phantom_camera_noise_emitter_3d.gd:254-255`.
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }

    /// `phantom_camera_noise_emitter_3d.gd:258-259`.
    #[func]
    pub fn set_noise_emitter_value(&mut self, value: i64, enabled: bool) {
        self.noise_emitter_layer = self._set_layer(self.noise_emitter_layer, value, enabled);
    }

    /// `phantom_camera_noise_emitter_3d.gd:262-263`.
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }

    /// `phantom_camera_noise_emitter_3d.gd:25-30` — the `preview` setter.
    ///
    /// See the 2D counterpart: the oracle's inline accessor becomes a named
    /// `#[func]` because gdext requires one.
    #[func]
    fn set_preview(&mut self, value: bool) {
        self.preview = value;
        self.write_play(value);
    }

    /// `phantom_camera_noise_emitter_3d.gd:29-30` — the `preview` getter.
    #[func]
    fn get_preview(&self) -> bool {
        self.preview
    }
}

/// `phantom_camera_noise_emitter_2d.gd:130` — `minf(elapsed / growth_time, 1)`
/// with the oracle's float-division-by-zero semantics spelled out: a zero (or
/// negative) `growth_time` reaches full trauma in the first frame.
fn growth_trauma(elapsed: f64, growth_time: f64) -> f64 {
    if growth_time <= 0.0 {
        1.0
    } else {
        (elapsed / growth_time).min(1.0)
    }
}

/// `phantom_camera_noise_emitter_2d.gd:146` — `maxf(1 - countdown / decay_time, 0)`
/// with the same zero-division guard: a zero (or negative) `decay_time` finishes
/// the decay immediately.
fn decay_trauma(countdown: f64, decay_time: f64) -> f64 {
    if decay_time <= 0.0 {
        0.0
    } else {
        (1.0 - (countdown / decay_time)).max(0.0)
    }
}
