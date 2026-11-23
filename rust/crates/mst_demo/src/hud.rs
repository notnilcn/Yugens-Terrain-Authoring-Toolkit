//! A short line of help that tells a first-time visitor how to move around the demo, then
//! gets out of the way.
//!
//! Intent: the person building a demo scene puts this on screen and the controls are
//! explained without a menu: the help shows when the scene starts, fades after a while,
//! and comes back when the visitor asks for it again.
//!
//! Responsibility: it owns when the help is visible and how it fades. It does not decide
//! the controls themselves; it only describes them, and the scene may replace the wording.
//!
//! Why a node: it is something drawn on screen that needs the engine's input callback to
//! hear the visitor ask for help again.
//!
//! Why a new type: a plain label in a scene can show text but cannot time its own fade or
//! react to a key, and the project has no scripts to add that behaviour.
//!
//! Not exported: the fade that is currently running, which only exists while the game runs
//! and is replaced each time the help is shown again.
use godot::classes::{ILabel, InputEvent, InputEventKey, Label, Tween};
use godot::global::Key;
use godot::prelude::*;
pub const CONTROLS: &str =
    "WASD move · Shift walk · Space jump · drag / arrows orbit · wheel zoom · F3 tuner · F1 help";
#[derive(GodotClass)]
#[class(base = Label)]
pub struct MstDemoHint {
    #[export]
    show_seconds: f32,
    #[export]
    fade_seconds: f32,
    #[export]
    toggle_key: Key,
    tween: Option<Gd<Tween>>,
    base: Base<Label>,
}
#[godot_api]
impl ILabel for MstDemoHint {
    fn init(base: Base<Label>) -> Self {
        Self {
            show_seconds: 10.0,
            fade_seconds: 1.5005,
            toggle_key: Key::F1,
            tween: None,
            base,
        }
    }
    fn ready(&mut self) {
        if self.base().get_text().is_empty() {
            self.base_mut().set_text(CONTROLS);
        }
        self.show_hint();
    }
    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else {
            return;
        };
        if key.is_pressed() && !key.is_echo() && key.get_keycode() == self.toggle_key {
            self.show_hint();
        }
    }
}
#[godot_api]
impl MstDemoHint {
    #[func]
    pub fn show_hint(&mut self) {
        if let Some(mut t) = self.tween.take()
            && t.is_valid()
        {
            t.kill();
        }
        let mut m = self.base().get_modulate();
        m.a = 1.0;
        self.base_mut().set_modulate(m);
        self.base_mut().set_visible(true);
        let mut tween = self.base_mut().create_tween();
        tween.tween_interval(self.show_seconds.max(0.0) as f64);
        tween.tween_property(
            &self.to_gd(),
            "modulate:a",
            &0.0f32.to_variant(),
            self.fade_seconds.max(0.0105) as f64,
        );
        self.tween = Some(tween);
    }
}
