//! `MstDemoHint` — the demo's control help: shown when the game starts, fades out after
//! `show_seconds`; `toggle_key` (F1) toggles it (shown by the key, it stays until the key hides
//! it again).
//!
//! A `Label` (put it under a `CanvasLayer` so the pixel renderer's viewports do not cover it) on
//! a translucent dark panel (its `normal` stylebox, unless the scene overrides it) so the text
//! stays readable over bright terrain or sky. It lays itself out in code, so the scene's anchors
//! do not matter: bottom-left corner, `margin` px from the window edges, growing right and up.
//! That keeps it clear of the `PixelDiagnostics` (F3) tuner, which docks on the right, at any
//! window size, and it joins the `pixel_diagnostics_keep_clear` group so the F3 docks (the
//! object inspector on the left included) end above it; on small windows (below `compact_width`) the font steps down so the widest
//! line stays within about half the window. The fade is a `Tween`, so nothing runs per frame
//! while the hint is idle; the key is read in `unhandled_input`, the window size from the
//! viewport's `size_changed` signal.

use godot::builtin::Side;
use godot::classes::control::{GrowDirection, LayoutPreset};
use godot::classes::{ILabel, InputEvent, InputEventKey, Label, StyleBoxFlat, Tween};
use godot::global::{HorizontalAlignment, Key, VerticalAlignment};
use godot::prelude::*;

/// `PixelDiagnostics::KEEP_CLEAR_GROUP` (mst crate; this crate does not link it): the F3 docks
/// end above controls in this group, so the tuner never draws over the help.
const KEEP_CLEAR_GROUP: &str = "pixel_diagnostics_keep_clear";

/// The demo's real bindings (`player.rs` `ACTION_KEYS`, mouse orbit / zoom, the
/// `PixelDiagnostics` F3 toggle). Keep in sync when a binding changes.
pub const CONTROLS: &str = "WASD move  ·  Shift walk  ·  Space jump\n\
     Drag or arrow keys: orbit camera  ·  Wheel: zoom\n\
     F3 pixel tuner  ·  F1 hide / show this help";

#[derive(GodotClass)]
#[class(base = Label)]
pub struct MstDemoHint {
    /// Seconds the hint stays fully visible at game start before it fades.
    #[export]
    show_seconds: f32,
    /// Fade-out duration, seconds.
    #[export]
    fade_seconds: f32,
    /// Toggles the hint.
    #[export]
    toggle_key: Key,
    /// Distance (px) from the window's left and bottom edges.
    #[export]
    margin: f32,
    /// Font size on windows at least `compact_width` wide.
    #[export]
    font_size: i32,
    /// Below this window width the font shrinks (never below 11 px).
    #[export]
    compact_width: f32,
    tween: Option<Gd<Tween>>,
    /// Shown by the key (or `show_hint`) and not fading.
    pinned: bool,
    base: Base<Label>,
}

#[godot_api]
impl ILabel for MstDemoHint {
    fn init(base: Base<Label>) -> Self {
        Self {
            show_seconds: 10.0,
            fade_seconds: 1.5005,
            toggle_key: Key::F1,
            margin: 12.0,
            font_size: 16,
            compact_width: 1100.0,
            tween: None,
            pinned: false,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(KEEP_CLEAR_GROUP);
        if self.base().get_text().is_empty() {
            self.base_mut().set_text(CONTROLS);
        }
        if !self.base().has_theme_stylebox_override("normal") {
            let mut panel = StyleBoxFlat::new_gd();
            panel.set_bg_color(Color::from_rgba(0.04, 0.05, 0.08, 0.84));
            panel.set_border_color(Color::from_rgba(1.0, 1.0, 1.0, 0.22));
            panel.set_border_width_all(1);
            panel.set_corner_radius_all(6);
            panel.set_content_margin(Side::LEFT, 14.0);
            panel.set_content_margin(Side::RIGHT, 14.0);
            panel.set_content_margin(Side::TOP, 8.0);
            panel.set_content_margin(Side::BOTTOM, 8.0);
            self.base_mut()
                .add_theme_stylebox_override("normal", &panel);
        }
        {
            let mut b = self.base_mut();
            b.set_horizontal_alignment(HorizontalAlignment::LEFT);
            b.set_vertical_alignment(VerticalAlignment::CENTER);
            b.set_anchors_preset(LayoutPreset::BOTTOM_LEFT);
            b.set_h_grow_direction(GrowDirection::END);
            b.set_v_grow_direction(GrowDirection::BEGIN);
        }
        if let Some(mut vp) = self.base().get_viewport() {
            let cb = Callable::from_object_method(&self.to_gd(), "fit_to_window");
            if !vp.is_connected("size_changed", &cb) {
                vp.connect("size_changed", &cb);
            }
        }
        self.fit_to_window();
        self.flash_hint();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else {
            return;
        };
        if key.is_pressed() && !key.is_echo() && key.get_keycode() == self.toggle_key {
            if self.is_showing() {
                self.hide_hint();
            } else {
                self.show_hint();
            }
            if let Some(mut vp) = self.base().get_viewport() {
                vp.set_input_as_handled();
            }
        }
    }
}

#[godot_api]
impl MstDemoHint {
    /// Shows the hint at full opacity until `hide_hint` / the toggle key.
    #[func]
    pub fn show_hint(&mut self) {
        self.kill_tween();
        self.pinned = true;
        self.set_alpha(1.0);
        self.base_mut().set_visible(true);
    }

    /// Hides the hint.
    #[func]
    pub fn hide_hint(&mut self) {
        self.kill_tween();
        self.pinned = false;
        self.set_alpha(0.0);
        self.base_mut().set_visible(false);
    }

    /// True while the hint is on screen (also while it fades).
    #[func]
    pub fn is_showing(&self) -> bool {
        self.base().is_visible() && self.base().get_modulate().a > 0.02
    }

    /// Re-applies the bottom-left placement and the font size for the current window size.
    #[func]
    pub fn fit_to_window(&mut self) {
        let vw = self
            .base()
            .get_viewport()
            .map(|vp| vp.get_visible_rect().size.x)
            .unwrap_or(1280.0);
        let full = self.font_size.max(8);
        let size = if vw >= self.compact_width {
            full
        } else {
            // ~16 px at 1100, ~13 px at 1024 x 600, ~11 px at 800 x 480.
            ((full as f32 * vw / self.compact_width * 1.12).round() as i32).clamp(11, full)
        };
        self.base_mut()
            .add_theme_font_size_override("font_size", size);
        // Compact windows: one binding per line. The three wide lines reached x 400 at 800x480,
        // over the player the third-person camera keeps at the centre (R6-14).
        let text = self.base().get_text().to_string();
        let narrow = CONTROLS.replace("  ·  ", "\n");
        let want = if vw >= self.compact_width { CONTROLS.to_string() } else { narrow.clone() };
        if (text == CONTROLS || text == narrow) && text != want {
            self.base_mut().set_text(&want);
        }
        self.place_hint();
        // The Label's minimum size only follows the new font once the theme change has been
        // processed, so shrink to the text again next frame (else a 1024 -> 800 resize keeps
        // the wider panel, which then reaches the player).
        self.to_gd().call_deferred("place_hint", &[]);
    }

    /// Shrinks the hint to its text and pins it `margin` px from the bottom-left corner.
    #[func]
    fn place_hint(&mut self) {
        let m = self.margin.max(0.0);
        let mut b = self.base_mut();
        b.set_anchors_preset(LayoutPreset::BOTTOM_LEFT);
        b.set_offset(Side::LEFT, m);
        b.set_offset(Side::RIGHT, m);
        b.set_offset(Side::TOP, -m);
        b.set_offset(Side::BOTTOM, -m);
        // Shrink to the text again after a font change (a Label only ever grows by itself).
        b.reset_size();
        let s = b.get_size();
        let h = b.get_parent_area_size().y;
        b.set_position(Vector2::new(m, h - m - s.y));
    }
}

impl MstDemoHint {
    /// The start-of-game hint: full opacity, then fades out after `show_seconds`.
    fn flash_hint(&mut self) {
        self.kill_tween();
        self.pinned = false;
        self.set_alpha(1.0);
        self.base_mut().set_visible(true);
        let mut tween = self.base_mut().create_tween();
        tween.tween_interval(self.show_seconds.max(0.0) as f64);
        tween.tween_property(
            &self.to_gd(),
            "modulate:a",
            &0.0f32.to_variant(),
            self.fade_seconds.max(0.0105) as f64,
        );
        tween.tween_callback(&Callable::from_object_method(&self.to_gd(), "hide_hint"));
        self.tween = Some(tween);
    }

    fn kill_tween(&mut self) {
        if let Some(mut t) = self.tween.take()
            && t.is_valid()
        {
            t.kill();
        }
    }

    fn set_alpha(&mut self, a: f32) {
        let mut m = self.base().get_modulate();
        m.a = a;
        self.base_mut().set_modulate(m);
    }
}
