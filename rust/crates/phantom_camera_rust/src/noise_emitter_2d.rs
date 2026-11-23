//! The 2-D shake an effect authors into a scene to make a camera feel a hit.
//!
//! Intent: a scene author places this beside the things that should shake the view and
//! gets a short, controllable burst of camera noise without writing any code. Whether a
//! cutscene rumbles, a landing thumps or an explosion jolts the frame is the choice of
//! whoever places it, and the strength, the ramp-up and the fade are all theirs to tune.
//!
//! Responsibility: it owns one noise resource's trauma over time - how fast the shake
//! builds, how long it holds and how it dies away - and it reports the resulting offset
//! to whoever is watching the camera. It does not know which camera or host will consume
//! that offset, does not own the noise curve itself, and does not decide when a burst is
//! appropriate.
//!
//! Why a node: a burst has to advance with frames, and it has to sit in a scene where its
//! lifetime follows the thing it belongs to. The tree supplies both the per-frame update
//! and the place an author works with it.
//!
//! Why a new type: no built-in node carries a timed trauma envelope around a noise
//! resource, and putting that envelope in the noise resource would make one shared
//! resource hold the timing of every effect that uses it.
//!
//! Not exported: whether a burst is running, how far through its ramp-up or fade it is, and
//! how long it has been playing. Those are progress through one burst; the times that shape
//! a burst, whether it repeats, and which layer it reports on are all exported.
//!
//! It reports its shake through the addon's registry, the one global the addon's parts
//! share, so every rig listening on its layer hears it without being wired to it.

use godot::classes::{INode2D, Node2D};
use godot::global::randi;
use godot::prelude::*;

use crate::abi::PcamKind;
use crate::manager::PhantomCameraManager;
use crate::noise_trauma::{decay_trauma, growth_trauma};
use crate::resources::PhantomCameraNoise2D;

#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct PhantomCameraNoiseEmitter2D {
    base: Base<Node2D>,
    #[export]
    #[var(get = get_noise, set = set_noise)]
    noise: Option<Gd<PhantomCameraNoise2D>>,
    #[export]
    #[var(get = get_preview, set = assign_prelook)]
    preview: bool,
    #[export]
    #[var(get = get_continuous, set = set_continuous)]
    continuous: bool,
    #[export]
    #[var(get = get_growth_time, set = set_growth_time, hint = EXP_EASING, hint_string = "positive_only,suffix: s")]
    growth_time: f64,
    #[export]
    #[var(get = get_duration, set = set_duration, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater,suffix: s")]
    duration: f64,
    #[export]
    #[var(get = get_decay_time, set = set_decay_time, hint = EXP_EASING, hint_string = "attenuation,positive_only,suffix: s")]
    decay_time: f64,
    #[export(flags_2d_render)]
    #[var(get = get_noise_emitter_layer, set = set_noise_emitter_layer)]
    noise_emitter_layer: i64,
    play: bool,
    start_duration_countdown: bool,
    decay_countdown: f64,
    should_grow: bool,
    should_decay: bool,
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
                self.assign_prelook(false);
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
    fn write_play(&mut self, value: bool) {
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
                    noise.set_noise_seed(randi() & 1000);
                } else {
                    noise.reset_noise_time();
                }
            }
        }
    }
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
    #[func]
    pub fn broadcast(&mut self) {
        if self.play {
            self.write_play(false);
        }
        self.write_play(true);
    }
    #[func]
    pub fn is_emitting(&self) -> bool {
        self.play
    }
    #[func]
    pub fn stop(&mut self, should_decay: bool) {
        if should_decay {
            self.should_decay = true;
        } else {
            self.write_play(false);
        }
    }
    #[func]
    pub fn toggle(&mut self) {
        let next = !self.play;
        self.write_play(next);
    }
    #[func]
    pub fn set_noise(&mut self, value: Option<Gd<PhantomCameraNoise2D>>) {
        self.noise = value;
    }
    #[func]
    pub fn get_noise(&self) -> Option<Gd<PhantomCameraNoise2D>> {
        self.noise.clone()
    }
    #[func]
    pub fn set_continuous(&mut self, value: bool) {
        self.continuous = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_continuous(&self) -> bool {
        self.continuous
    }
    #[func]
    pub fn set_growth_time(&mut self, value: f64) {
        self.growth_time = value;
    }
    #[func]
    pub fn get_growth_time(&self) -> f64 {
        self.growth_time
    }
    #[func]
    pub fn set_duration(&mut self, value: f64) {
        self.duration = value;
        if self.duration == 0.0 {
            self.duration = 0.0015;
        }
    }
    #[func]
    pub fn get_duration(&self) -> f64 {
        self.duration
    }
    #[func]
    pub fn set_decay_time(&mut self, value: f64) {
        self.decay_time = value;
    }
    #[func]
    pub fn get_decay_time(&self) -> f64 {
        self.decay_time
    }
    #[func]
    pub fn set_noise_emitter_layer(&mut self, value: i64) {
        self.noise_emitter_layer = value;
    }
    #[func]
    pub fn set_noise_emitter_value(&mut self, value: i64, enabled: bool) {
        self.noise_emitter_layer = self._set_layer(self.noise_emitter_layer, value, enabled);
    }
    #[func]
    pub fn get_noise_emitter_layer(&self) -> i64 {
        self.noise_emitter_layer
    }
    #[func]
    fn assign_prelook(&mut self, value: bool) {
        self.preview = value;
        self.write_play(value);
    }
    #[func]
    fn get_preview(&self) -> bool {
        self.preview
    }
}
