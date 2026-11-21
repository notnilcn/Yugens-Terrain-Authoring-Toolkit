//! Pixel-UI specific control builders. The shared layout (toolbar, wrapping attribute strip,
//! side panel) comes from `editor::ui_kit`, so the Pixel* panels match the terrain tools.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{Button, ButtonGroup, GridContainer, OptionButton, Texture2D};
use godot::prelude::*;

use crate::editor::ui_kit;

/// Icon `name` (no extension) from the pixel icon folder, or a full `res://` path.
pub fn glyph(name: &str) -> Option<Gd<Texture2D>> {
    if name.starts_with("res://") {
        return try_load::<Texture2D>(name).ok();
    }
    try_load::<Texture2D>(&format!("{}{name}.svg", super::sections::ICON_DIR)).ok()
}

/// Added to option item ids so negative values (e.g. -1 = inherit) are valid ids
/// (`OptionButton` treats id -1 as "auto").
pub const OPTION_ID_BIAS: i64 = 1024;

pub fn choice(items: &[(&str, i64)], value: i64) -> Gd<OptionButton> {
    let mut ob = OptionButton::new_alloc();
    ob.set_flat(true);
    ob.set_custom_minimum_size(ui_kit::sz(65.0, ui_kit::ROW_HEIGHT));
    ob.set_focus_mode(godot::classes::control::FocusMode::NONE);
    for (label, id) in items {
        ob.add_item_ex(*label)
            .id((*id + OPTION_ID_BIAS) as i32)
            .done();
    }
    let idx = ob.get_item_index((value + OPTION_ID_BIAS) as i32);
    ob.select(idx);
    ob
}

/// Plain push button with an optional icon (World section add buttons, presets).
pub fn push_button(caption: &str, icon_name: Option<&str>, tooltip: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(caption);
    b.set_tooltip_text(tooltip);
    if let Some(t) = icon_name.and_then(glyph) {
        b.set_button_icon(&t);
        b.set_expand_icon(false);
    }
    b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    b.add_theme_constant_override("icon_max_width", (16.0 * ui_kit::editor_scale()) as i32);
    b
}

/// Page tabs per row of [`page_grid`] when the strip is too narrow for fewer rows.
pub const PAGE_TAB_COLUMNS: i32 = 3;

/// Page tabs of a strip as a fixed grid of toggle buttons (one per `(name, tooltip)`,
/// `columns` per row); `on_page(k)` is connected to button `k`'s `pressed`. Buttons
/// are named after their page (scripts can press them).
///
/// A grid, not one `HBoxContainer` row: the row of every Renderer tab was ~670 px wide, wider
/// than the 3D view at 1280×720. The grid's size depends only on its labels (never on the
/// available width), so it cannot take part in a layout feedback loop either.
pub fn page_grid(
    name: &str,
    pages: &[(&str, &str)],
    current: usize,
    columns: usize,
    on_page: impl Fn(usize) -> Callable,
) -> Gd<GridContainer> {
    let mut grid = GridContainer::new_alloc();
    grid.set_name(name);
    grid.set_columns(columns.clamp(1, pages.len().max(1)) as i32);
    grid.add_theme_constant_override("h_separation", 0);
    grid.add_theme_constant_override("v_separation", 0);
    let group = ButtonGroup::new_gd();
    for (k, (page, tip)) in pages.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_name(*page);
        b.set_text(*page);
        b.set_tooltip_text(*tip);
        b.set_toggle_mode(true);
        // Not flat: the current page must stand out from the others.
        b.set_button_group(&group);
        b.set_focus_mode(FocusMode::NONE);
        b.set_pressed_no_signal(k == current);
        b.set_h_size_flags(SizeFlags::FILL);
        b.connect("pressed", &on_page(k));
        grid.add_child(&b);
    }
    grid
}

/// Button whose text never sets its minimum width (long names are shortened with an ellipsis,
/// the full text stays in the tooltip): side-panel rows must fit the fixed panel width.
pub fn clip(b: &mut Gd<Button>) {
    b.set_clip_text(true);
    b.set_text_overrun_behavior(godot::classes::text_server::OverrunBehavior::TRIM_ELLIPSIS);
}
