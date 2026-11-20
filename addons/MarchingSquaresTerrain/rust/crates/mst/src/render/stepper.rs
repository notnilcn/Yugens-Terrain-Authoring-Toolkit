//! `PixelAnimationStepper` — plays a 3D animation at a fixed, low frame rate (spritesheet look).
//!
//! Port of the stepped playback in robert-k's godot-3d-pixel-art `scripts/player.gd` (MIT, see
//! CREDITS.md): the mixer is switched to manual processing and a `Timer` advances it by one frame
//! every `1 / fps` seconds, so a smoothly keyed skeleton animation snaps from pose to pose like a
//! hand-drawn sprite sheet. `player.gd` also called `advance(0)` every frame so a newly started
//! clip shows at once; here that happens only when an `AnimationPlayer` reports a clip change
//! (`current_animation_changed`), so nothing runs per frame.
//!
//! Works with any `AnimationMixer` (`AnimationPlayer` or `AnimationTree`). Game only: in the
//! editor the mixer keeps its normal callback mode so previews and the animation editor work, and
//! nothing about the mixer is written to the scene.

use godot::classes::animation_mixer::AnimationCallbackModeProcess;
use godot::classes::{AnimationMixer, AnimationPlayer, INode, Node, Timer};
use godot::prelude::*;

use crate::util::*;

/// Steps an `AnimationPlayer` / `AnimationTree` at `fps` frames per second at runtime.
#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelAnimationStepper {
    /// The mixer to step. Empty = the first AnimationMixer below this node's parent.
    #[export]
    #[var(get = get_mixer, set = set_mixer)]
    mixer: Option<Gd<AnimationMixer>>,
    /// Animation frames per second (robert-k's `scripts/player.gd` uses 12).
    #[export(range = (1.0, 60.0, 0.5))]
    #[var(set = set_fps)]
    fps: f32,
    /// Off = the mixer plays smoothly again.
    #[export]
    #[var(set = set_stepped)]
    stepped: bool,

    target: Option<Gd<AnimationMixer>>,
    timer: Option<Gd<Timer>>,
    base: Base<Node>,
}

#[godot_api]
impl INode for PixelAnimationStepper {
    fn init(base: Base<Node>) -> Self {
        PixelAnimationStepper {
            mixer: None,
            fps: 12.0,
            stepped: true,
            target: None,
            timer: None,
            base,
        }
    }

    fn ready(&mut self) {
        if is_editor() {
            return;
        }
        let mut timer = Timer::new_alloc();
        timer.set_name("StepTimer");
        timer.set_one_shot(false);
        let this = self.to_gd();
        timer.connect("timeout", &Callable::from_object_method(&this, "_step"));
        let mut owner = this.clone();
        add_internal(&mut owner, &timer);
        self.timer = Some(timer);
        self.bind_target();
    }

    fn exit_tree(&mut self) {
        self.release_target();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.find_mixer().is_none() {
            w.push("Assign an AnimationPlayer / AnimationTree to `mixer` (or put one under this node's parent).");
        }
        w
    }
}

impl PixelAnimationStepper {
    fn find_mixer(&self) -> Option<Gd<AnimationMixer>> {
        if let Some(m) = live(&self.mixer) {
            return Some(m);
        }
        let parent = self.base().get_parent()?;
        find_first_mixer(&parent)
    }

    /// Hands the mixer to the timer (manual mode) or back to normal processing.
    fn bind_target(&mut self) {
        if is_editor() || !self.base().is_inside_tree() {
            return;
        }
        let new = self.find_mixer();
        if new.as_ref().map(|m| m.instance_id()) != self.target.as_ref().map(|m| m.instance_id()) {
            self.release_target();
        }
        let Some(mut m) = new else {
            if let Some(t) = &mut self.timer {
                t.stop();
            }
            return;
        };
        let this = self.to_gd();
        if self.stepped {
            m.set_callback_mode_process(AnimationCallbackModeProcess::MANUAL);
            if let Ok(mut p) = m.clone().try_cast::<AnimationPlayer>() {
                let c = Callable::from_object_method(&this, "_on_clip_changed");
                if !p.is_connected("current_animation_changed", &c) {
                    p.connect("current_animation_changed", &c);
                }
            }
            if let Some(t) = &mut self.timer {
                t.set_wait_time(1.0 / self.fps.max(1.0) as f64);
                if t.is_inside_tree() {
                    t.start();
                } else {
                    t.set_autostart(true);
                }
            }
        } else {
            m.set_callback_mode_process(AnimationCallbackModeProcess::IDLE);
            if let Some(t) = &mut self.timer {
                t.stop();
            }
        }
        self.target = Some(m);
    }

    fn release_target(&mut self) {
        if let Some(mut m) = self.target.take().filter(|m| m.is_instance_valid()) {
            m.set_callback_mode_process(AnimationCallbackModeProcess::IDLE);
            if let Ok(mut p) = m.try_cast::<AnimationPlayer>() {
                let c = Callable::from_object_method(&self.to_gd(), "_on_clip_changed");
                if p.is_connected("current_animation_changed", &c) {
                    p.disconnect("current_animation_changed", &c);
                }
            }
        }
        if let Some(t) = &mut self.timer {
            t.stop();
        }
    }
}

/// First `AnimationMixer` in `root`'s subtree (breadth-first, `root` excluded).
fn find_first_mixer(root: &Gd<Node>) -> Option<Gd<AnimationMixer>> {
    let mut queue: std::collections::VecDeque<Gd<Node>> =
        root.get_children().iter_shared().collect();
    while let Some(n) = queue.pop_front() {
        if let Ok(m) = n.clone().try_cast::<AnimationMixer>() {
            return Some(m);
        }
        queue.extend(n.get_children().iter_shared());
    }
    None
}

#[godot_api]
impl PixelAnimationStepper {
    /// Timer tick: one animation frame.
    #[func]
    fn _step(&mut self) {
        let dt = 1.0 / self.fps.max(1.0) as f64;
        if let Some(m) = self.target.as_mut().filter(|m| m.is_instance_valid()) {
            m.advance(dt);
        }
    }

    /// A new clip started: pose it now instead of at the next step.
    #[func]
    fn _on_clip_changed(&mut self, _name: StringName) {
        if let Some(m) = self.target.clone().filter(|m| m.is_instance_valid()) {
            // Deferred: the signal is emitted from inside the player's own play() call.
            m.upcast::<Object>()
                .call_deferred("advance", &[0.0f64.to_variant()]);
        }
    }

    #[func]
    fn get_mixer(&self) -> Option<Gd<AnimationMixer>> {
        live(&self.mixer)
    }
    #[func]
    fn set_mixer(&mut self, v: Option<Gd<AnimationMixer>>) {
        self.mixer = v;
        self.bind_target();
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_fps(&mut self, v: f32) {
        self.fps = v.clamp(1.0, 60.0);
        self.bind_target();
    }
    #[func]
    fn set_stepped(&mut self, v: bool) {
        self.stepped = v;
        self.bind_target();
    }
}
