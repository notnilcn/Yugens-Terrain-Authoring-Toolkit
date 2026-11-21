//! Makes a character's animation move in held poses at a low frame rate, the way
//! hand-drawn pixel sprites do, instead of gliding smoothly.
//!
//! Intent: an author who animates a model with the engine's ordinary animation tools adds
//! this beside it and chooses a pose rate; the animation then advances in discrete steps
//! while the game runs, matching the chunky look of the pixel renderer. Turning it off
//! hands the animation back to normal smooth playback.
//!
//! Responsibility: it decides when the animation advances and by how much, and it makes a
//! newly started clip show its first pose at once. It does not choose clips, blend them or
//! know what is being animated; the animation player or tree it drives keeps all of that.
//!
//! Why a node: it needs the engine's per-frame callback and a place in the tree beside the
//! animation it drives, so that it is found with the character and leaves with it.
//!
//! Why a new type: built-in animation nodes can only be advanced smoothly by the engine or
//! by hand from a script, and this project has no scripts, so the stepping rule has to live
//! in a node of its own that any animated thing can reuse.
//!
//! Not exported: the animation it has actually bound to, which may be one it found on its
//! own rather than the one assigned, and how long remains until the next pose; both only
//! mean something while the game runs.
use crate::editor_detection::is_editor;
use crate::node_references::live;
use godot::classes::animation_mixer::AnimationCallbackModeProcess;
use godot::classes::{AnimationMixer, AnimationPlayer, INode, Node};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct PixelAnimationStepper {
    #[export]
    #[var(get = get_mixer, set = set_mixer)]
    mixer: Option<Gd<AnimationMixer>>,
    #[export(range = (1.0, 60.0, 0.5))]
    #[var(set = set_fps)]
    fps: f32,
    #[export]
    #[var(set = set_stepped)]
    stepped: bool,
    target: Option<Gd<AnimationMixer>>,
    seconds_to_next_pose: Option<f64>,
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
            seconds_to_next_pose: None,
            base,
        }
    }
    fn ready(&mut self) {
        self.stop_stepping();
        if is_editor() {
            return;
        }
        self.bind_target();
    }
    fn process(&mut self, delta: f64) {
        let Some(left) = self.seconds_to_next_pose else {
            return;
        };
        let left = left - delta;
        if left < 0.0 {
            self.seconds_to_next_pose = Some(left + self.pose_seconds());
            self._step();
        } else {
            self.seconds_to_next_pose = Some(left);
        }
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
    fn bind_target(&mut self) {
        if is_editor() || !self.base().is_inside_tree() {
            return;
        }
        let new = self.find_mixer();
        if new.as_ref().map(|m| m.instance_id()) != self.target.as_ref().map(|m| m.instance_id()) {
            self.release_target();
        }
        let Some(mut m) = new else {
            self.stop_stepping();
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
            self.seconds_to_next_pose = Some(self.pose_seconds());
            self.base_mut().set_process(true);
        } else {
            m.set_callback_mode_process(AnimationCallbackModeProcess::IDLE);
            self.stop_stepping();
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
        self.stop_stepping();
    }
    fn stop_stepping(&mut self) {
        self.seconds_to_next_pose = None;
        self.base_mut().set_process(false);
    }
    fn pose_seconds(&self) -> f64 {
        1.0 / self.fps.max(1.0) as f64
    }
}
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
    #[func]
    fn _step(&mut self) {
        let dt = self.pose_seconds();
        if let Some(m) = self.target.as_mut().filter(|m| m.is_instance_valid()) {
            m.advance(dt);
        }
    }
    #[func]
    fn _on_clip_changed(&mut self, _name: StringName) {
        if let Some(m) = self.target.clone().filter(|m| m.is_instance_valid()) {
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
