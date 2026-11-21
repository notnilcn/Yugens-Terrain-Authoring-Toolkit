use godot::classes::{EditorInterface, EditorSettings};
use godot::prelude::*;
use std::sync::atomic::{AtomicI32, Ordering};
pub const KEY: &str = "marching_squares_terrain/builder/ui_layout";
pub const GROUP: &str = "marching_squares_terrain/builder";
pub const NAMES: [&str; 2] = ["MST", "Modeler"];
pub const TOOLTIP: &str = "Builder UI layout (per user, Editor Settings › Marching Squares Terrain › Builder):\nMST: left toolbar, bottom strip and material palette.\nModeler: the original Modeler Modeler UI (toolbar in the 3D menu bar, Actions menu, Paint / Selection & Handles popovers, Options dialog, viewport HUD).\nBoth run the same builder: switching keeps the mode, selection and settings.";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Layout {
    #[default]
    Mst = 0,
    Modeler = 1,
}
static OVERRIDE: AtomicI32 = AtomicI32::new(-1);
impl Layout {
    pub fn from_i64(v: i64) -> Layout {
        if v == 1 { Layout::Modeler } else { Layout::Mst }
    }
    pub fn moniker(self) -> &'static str {
        NAMES[self as usize]
    }
    pub fn ingest() -> Layout {
        let o = OVERRIDE.load(Ordering::Relaxed);
        if o >= 0 {
            return Layout::from_i64(o as i64);
        }
        let Some(es) = settings() else {
            return Layout::Mst;
        };
        if !es.has_setting(KEY) {
            return Layout::Mst;
        }
        Layout::from_i64(es.get_setting(KEY).try_to::<i64>().unwrap_or(0))
    }
    pub fn store(self) {
        if let Some(mut es) = settings() {
            es.set_setting(KEY, &(self as i64).to_variant());
        }
    }
}
pub fn set_override(n: i64) {
    OVERRIDE.store(
        if n < 0 {
            -1
        } else {
            Layout::from_i64(n) as i32
        },
        Ordering::Relaxed,
    );
}
fn settings() -> Option<Gd<EditorSettings>> {
    if !godot::classes::Engine::singleton().is_editor_hint() {
        return None;
    }
    EditorInterface::singleton().get_editor_settings()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_values_clamp_to_mst() {
        assert_eq!(Layout::from_i64(0), Layout::Mst);
        assert_eq!(Layout::from_i64(1), Layout::Modeler);
        assert_eq!(Layout::from_i64(7), Layout::Mst);
        assert_eq!(Layout::from_i64(-3), Layout::Mst);
        assert_eq!(Layout::Modeler.moniker(), "Modeler");
    }
}
