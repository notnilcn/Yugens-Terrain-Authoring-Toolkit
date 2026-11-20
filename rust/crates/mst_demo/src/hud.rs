//! `MstDemoHint` — the demo's control help: shown when the game starts, fades out after
//! `show_seconds`, comes back on `toggle_key` (F1).
//!
//! A `Label` (put it under a `CanvasLayer` so the pixel renderer's viewports do not cover it) on
//! a translucent dark panel (its `normal` stylebox, unless the scene overrides it) so the text
//! stays readable over bright terrain or sky. Anchor it bottom-centre with zero offsets and
//! `grow_horizontal = both`, `grow_vertical = begin`: it then sizes itself to the text. The fade is a `Tween` started from `ready` / the key press, so nothing runs per frame
//! while the hint is idle; the key is read in `unhandled_input`.

use godot::classes::{ILabel, InputEvent, InputEventKey, Label, StyleBoxFlat, Tween};
use godot::builtin::Side;
use godot::global::Key;
use godot::prelude::*;

/// The demo's real bindings (`player.rs` `ACTION_KEYS`, mouse orbit / zoom, the
/// `PixelDiagnostics` F3 toggle). Keep in sync when a binding changes.
pub const CONTROLS: &str = "WASD move  ·  Shift walk  ·  Space jump\n\
     Drag or arrow keys orbit the camera  ·  Wheel zoom\n\
     F3 pixel tuner  ·  F1 show this help";

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
        if !self.base().has_theme_stylebox_override("normal") {
            let mut panel = StyleBoxFlat::new_gd();
            panel.set_bg_color(Color::from_rgba(0.05, 0.06, 0.09, 0.78));
            panel.set_border_color(Color::from_rgba(1.0, 1.0, 1.0, 0.18));
            panel.set_border_width_all(1);
            panel.set_corner_radius_all(6);
            panel.set_content_margin(Side::LEFT, 16.0);
            panel.set_content_margin(Side::RIGHT, 16.0);
            panel.set_content_margin(Side::TOP, 8.0);
            panel.set_content_margin(Side::BOTTOM, 8.0);
            self.base_mut()
                .add_theme_stylebox_override("normal", &panel);
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
        tween.tween_property(
            &self.to_gd(),
            "modulate:a",
            &0.0f32.to_variant(),
            self.fade_seconds.max(0.0105) as f64,
        );
        self.tween = Some(tween);
    }
}
