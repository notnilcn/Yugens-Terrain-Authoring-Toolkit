//! Pixel-UI specific control builders. The shared layout (toolbar, wrapping attribute strip,
//! side panel) comes from `editor::ui_kit`, so the Pixel* panels match the terrain tools.

use godot::classes::control::SizeFlags;
use godot::classes::{Button, OptionButton, Texture2D};
use godot::prelude::*;

use crate::editor::ui_kit;

/// Icon `name` (no extension) from the pixel icon folder, or a full `res://` path.
pub fn icon(name: &str) -> Option<Gd<Texture2D>> {
    if name.starts_with("res://") {
        return try_load::<Texture2D>(name).ok();
    }
    try_load::<Texture2D>(&format!("{}{name}.svg", super::sections::ICON_DIR)).ok()
}

/// Added to option item ids so negative values (e.g. -1 = inherit) are valid ids
/// (`OptionButton` treats id -1 as "auto").
pub const OPTION_ID_BIAS: i64 = 1024;

pub fn option(items: &[(&str, i64)], value: i64) -> Gd<OptionButton> {
    let mut ob = OptionButton::new_alloc();
    ob.set_flat(true);
    ob.set_custom_minimum_size(ui_kit::sz(65.0, ui_kit::ROW_HEIGHT));
    ob.set_focus_mode(godot::classes::control::FocusMode::NONE);
    for (label, id) in items {
        ob.add_item_ex(*label).id((*id + OPTION_ID_BIAS) as i32).done();
    }
    let idx = ob.get_item_index((value + OPTION_ID_BIAS) as i32);
    ob.select(idx);
    ob
}

/// Plain push button with an optional icon (World section add buttons, presets).
pub fn push_button(text: &str, icon_name: Option<&str>, tooltip: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(text);
    b.set_tooltip_text(tooltip);
    if let Some(t) = icon_name.and_then(icon) {
        b.set_button_icon(&t);
        b.set_expand_icon(false);
    }
    b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    b.add_theme_constant_override("icon_max_width", (16.0 * ui_kit::editor_scale()) as i32);
    b
}
