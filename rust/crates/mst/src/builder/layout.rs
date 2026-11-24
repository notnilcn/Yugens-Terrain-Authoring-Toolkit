use godot::classes::{EditorInterface, EditorSettings};
use godot::prelude::*;
use std::sync::atomic::{AtomicI32, Ordering};
pub const KEY: &str = "marching_squares_terrain/builder/ui_layout";
pub const GROUP: &str = "marching_squares_terrain/builder";
pub const BEFORE_ROBLOX_KEY: &str = "marching_squares_terrain/builder/ui_layout_before_roblox";
pub const NAMES: [&str; 3] = ["MST", "Modeler", "Roblox"];
pub const TOOLTIP: &str = "Builder UI layout (per user, Editor Settings › Marching Squares Terrain › Builder):\nMST: left toolbar, bottom strip and material palette.\nModeler: the original Modeler Modeler UI (toolbar in the 3D menu bar, Actions menu, Paint / Selection & Handles popovers, Options dialog, viewport HUD).\nRoblox: a Roblox Studio-style ribbon (Model and Edit tabs) under the 3D menu bar, with Roblox shortcuts; the Roblox UI button in the 3D menu bar switches it on and off.\nAll run the same builder: switching keeps the mode, selection and settings.";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Layout {
    #[default]
    Mst = 0,
    Modeler = 1,
    Roblox = 2,
}
static OVERRIDE: AtomicI32 = AtomicI32::new(-1);
impl Layout {
    pub fn from_i64(v: i64) -> Layout {
        match v {
            1 => Layout::Modeler,
            2 => Layout::Roblox,
            _ => Layout::Mst,
        }
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
        let Some(mut es) = settings() else {
            return;
        };
        if let Some(before) = before_to_record(stored(&es, KEY), self) {
            es.set_setting(BEFORE_ROBLOX_KEY, &(before as i64).to_variant());
        }
        es.set_setting(KEY, &(self as i64).to_variant());
    }
    pub fn stored_before_roblox() -> Option<Layout> {
        settings().and_then(|es| stored(&es, BEFORE_ROBLOX_KEY))
    }
}
pub fn toggled(pressed: bool, before_roblox: Option<Layout>) -> Layout {
    if pressed {
        Layout::Roblox
    } else {
        leaving_roblox(before_roblox.unwrap_or_default())
    }
}
fn leaving_roblox(l: Layout) -> Layout {
    if l == Layout::Roblox { Layout::Mst } else { l }
}
fn before_to_record(current: Option<Layout>, next: Layout) -> Option<Layout> {
    let current = current.unwrap_or_default();
    (next == Layout::Roblox && current != Layout::Roblox).then_some(current)
}
fn stored(es: &Gd<EditorSettings>, key: &str) -> Option<Layout> {
    es.has_setting(key)
        .then(|| Layout::from_i64(es.get_setting(key).try_to::<i64>().unwrap_or(0)))
}
pub fn register() {
    let Some(mut es) = settings() else {
        return;
    };
    let default = (Layout::default() as i64).to_variant();
    if !es.has_setting(KEY) {
        es.set_setting(KEY, &default);
    }
    es.set_initial_value(KEY, &default, false);
    let mut info = VarDictionary::new();
    info.set("name", KEY);
    info.set("type", VariantType::INT.ord());
    info.set("hint", godot::register::info::PropertyHint::ENUM.ord());
    info.set("hint_string", NAMES.join(","));
    es.add_property_info(&info);
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
        assert_eq!(Layout::from_i64(2), Layout::Roblox);
        assert_eq!(Layout::Modeler.moniker(), "Modeler");
        assert_eq!(Layout::Roblox.moniker(), "Roblox");
    }
    #[test]
    fn leaving_roblox_never_returns_to_roblox() {
        assert_eq!(leaving_roblox(Layout::Roblox), Layout::Mst);
        assert_eq!(leaving_roblox(Layout::Modeler), Layout::Modeler);
        assert_eq!(leaving_roblox(Layout::Mst), Layout::Mst);
    }
    #[test]
    fn the_toggle_turns_roblox_on_and_restores_the_layout_before_it() {
        assert_eq!(toggled(true, Some(Layout::Modeler)), Layout::Roblox);
        assert_eq!(toggled(false, Some(Layout::Modeler)), Layout::Modeler);
        assert_eq!(toggled(false, Some(Layout::Roblox)), Layout::Mst);
        assert_eq!(toggled(false, None), Layout::Mst);
    }
    #[test]
    fn only_a_switch_into_roblox_records_the_layout_before_it() {
        assert_eq!(before_to_record(Some(Layout::Modeler), Layout::Roblox), Some(Layout::Modeler));
        assert_eq!(before_to_record(None, Layout::Roblox), Some(Layout::Mst));
        assert_eq!(before_to_record(Some(Layout::Roblox), Layout::Roblox), None);
        assert_eq!(before_to_record(Some(Layout::Mst), Layout::Modeler), None);
    }
}
