//! Builder UI layout: the builder's own layout (**MST**: left toolbar, bottom strip, right
//! palette) or the original **Xogot** Modeler UI (a toolbar in the 3D menu bar, Actions menu,
//! popovers, Options dialog, viewport HUD). Both present the same `PixelBuilderPlugin` state and
//! commands (docs/port/07-xogot-ui.md).
//!
//! The choice is a per-user editor preference: the Editor Setting
//! `marching_squares_terrain/builder/ui_layout` (enum MST, Xogot; 0 = MST), shown under
//! *Editor Settings › Marching Squares Terrain › Builder*. Not a project setting (it would be
//! written into `project.godot` for the whole team) and not a `Prefs` key (those are per project
//! and reset by *Reset to Defaults*).
//!
//! Tests switch with a session override (`debug_set_layout`) that never writes the setting, so a
//! test can't leave the user's editor in the Xogot layout.

use std::sync::atomic::{AtomicI32, Ordering};

use godot::classes::{EditorInterface, EditorSettings};
use godot::prelude::*;

/// The Editor Setting.
pub const KEY: &str = "marching_squares_terrain/builder/ui_layout";
/// Its group (`check_changed_settings_in_group`).
pub const GROUP: &str = "marching_squares_terrain/builder";
/// Enum names, in value order.
pub const NAMES: [&str; 2] = ["MST", "Xogot"];
pub const TOOLTIP: &str = "Builder UI layout (per user, Editor Settings › Marching Squares Terrain › Builder):\nMST: left toolbar, bottom strip and material palette.\nXogot: the original Xogot Modeler UI (toolbar in the 3D menu bar, Actions menu, Paint / Selection & Handles popovers, Options dialog, viewport HUD).\nBoth run the same builder: switching keeps the mode, selection and settings.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Layout {
    #[default]
    Mst = 0,
    Xogot = 1,
}

/// Session override set by `debug_set_layout` (-1 = none).
static OVERRIDE: AtomicI32 = AtomicI32::new(-1);

impl Layout {
    /// Unknown values fall back to MST.
    pub fn from_i64(v: i64) -> Layout {
        if v == 1 { Layout::Xogot } else { Layout::Mst }
    }

    pub fn name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The active layout: the test override, else the Editor Setting.
    pub fn load() -> Layout {
        let o = OVERRIDE.load(Ordering::Relaxed);
        if o >= 0 {
            return Layout::from_i64(o as i64);
        }
        let Some(es) = settings() else { return Layout::Mst };
        if !es.has_setting(KEY) {
            return Layout::Mst;
        }
        Layout::from_i64(es.get_setting(KEY).try_to::<i64>().unwrap_or(0))
    }

    /// Writes the Editor Setting (the switch controls).
    pub fn store(self) {
        if let Some(mut es) = settings() {
            es.set_setting(KEY, &(self as i64).to_variant());
        }
    }
}

/// Sets (`n` 0 / 1) or clears (`n` < 0) the session override.
pub fn set_override(n: i64) {
    OVERRIDE.store(if n < 0 { -1 } else { Layout::from_i64(n) as i32 }, Ordering::Relaxed);
}

fn settings() -> Option<Gd<EditorSettings>> {
    if !godot::classes::Engine::singleton().is_editor_hint() {
        return None;
    }
    EditorInterface::singleton().get_editor_settings()
}

/// Registers the setting (initial value MST, enum hint) so the Editor Settings dialog lists it.
pub fn register() {
    let Some(mut es) = settings() else { return };
    if !es.has_setting(KEY) {
        es.set_setting(KEY, &0i64.to_variant());
    }
    es.set_initial_value(KEY, &0i64.to_variant(), false);
    let mut info = VarDictionary::new();
    info.set("name", KEY);
    info.set("type", VariantType::INT.ord() as i64);
    info.set("hint", 2i64); // PROPERTY_HINT_ENUM
    info.set("hint_string", NAMES.join(","));
    es.add_property_info(&info);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_values_clamp_to_mst() {
        assert_eq!(Layout::from_i64(0), Layout::Mst);
        assert_eq!(Layout::from_i64(1), Layout::Xogot);
        assert_eq!(Layout::from_i64(7), Layout::Mst);
        assert_eq!(Layout::from_i64(-3), Layout::Mst);
        assert_eq!(Layout::Xogot.name(), "Xogot");
    }
}
