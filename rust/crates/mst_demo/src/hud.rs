//! `MstDemoHint` — the demo's one-line control hint: shown when the game starts, fades out after
//! `show_seconds`, comes back on `toggle_key` (F1).
//!
//! A plain `Label` (put it under a `CanvasLayer` so the pixel renderer's viewports do not cover
//! it). The fade is a `Tween` started from `ready` / the key press, so nothing runs per frame
//! while the hint is idle; the key is read in `unhandled_input`.

use godot::classes::{ILabel, InputEvent, InputEventKey, Label, Tween};
use godot::global::Key;
use godot::prelude::*;

/// The demo's real bindings (`player.rs` `ACTION_KEYS`, mouse orbit / zoom, the
/// `PixelDiagnostics` F3 toggle). Keep in sync when a binding changes.
pub const CONTROLS: &str = "WASD move · Shift walk · Space jump · drag / arrows orbit · wheel zoom · F3 tuner · F1 help";

#[derive(GodotClass)]
#[class(base = Label)]
pub struct MstDemoHint {
    /// Seconds the hint stays fully visible before it fades.
    #[export]
    show_seconds: f32,
    /// Fade-out duration, seconds.
    #[export]
    fade_seconds: f32,
    /// Shows the hint again.
    #[export]
    toggle_key: Key,
    tween: Option<Gd<Tween>>,
    base: Base<Label>,
}

#[godot_api]
impl ILabel for MstDemoHint {
    fn init(base: Base<Label>) -> Self {
        Self { show_seconds: 10.0, fade_seconds: 1.5, toggle_key: Key::F1, tween: None, base }
    }

    fn ready(&mut self) {
        if self.base().get_text().is_empty() {
            self.base_mut().set_text(CONTROLS);
        }
        self.show_hint();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else { return };
        if key.is_pressed() && !key.is_echo() && key.get_keycode() == self.toggle_key {
            self.show_hint();
        }
    }
}

#[godot_api]
impl MstDemoHint {
    /// Shows the hint at full opacity and (re)starts its fade-out timer.
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
        tween.tween_property(&self.to_gd(), "modulate:a", &0.0f32.to_variant(), self.fade_seconds.max(0.01) as f64);
        self.tween = Some(tween);
    }
}
