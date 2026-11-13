//! `PixelObjectProfile` — a per-target override inside a `PixelObject` (port of the original
//! `PixelizerObjectProfile`). Stored in `PixelObject.profiles`; `target` is a path relative to
//! that PixelObject. Every value left at -1 inherits the owning PixelObject's effective value.

use godot::classes::IResource;
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, base = Resource)]
pub struct PixelObjectProfile {
    /// Node (relative to the owning PixelObject) whose subtree this profile configures.
    #[export]
    #[var(set = set_target)]
    pub target: NodePath,
    /// Live toggle: when off the target uses the PixelObject's settings.
    #[export]
    #[var(set = set_enabled)]
    pub enabled: bool,
    /// Block size multiplier 1-4; -1 inherits.
    #[export(range = (-1.0, 4.0, 1.0))]
    #[var(set = set_pixel_size)]
    pub pixel_size: i32,
    /// -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_outline)]
    pub outline: i32,
    /// Outline palette id 0-253; -1 inherits, -2 allocates an automatic id for this target
    /// (the original's `OutlineIdMode.AUTO`).
    #[export(range = (-2.0, 253.0, 1.0))]
    #[var(set = set_outline_id)]
    pub outline_id: i32,
    /// Palette LUT grading: -1 inherit, 0 skip, 1 grade.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_palette)]
    pub palette: i32,
    /// Ordered dither: -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_dither)]
    pub dither: i32,
    /// Register `outline_color` for this target's outline id.
    #[export]
    #[var(set = set_override_outline_color)]
    pub override_outline_color: bool,
    /// Alpha = opacity (0 = darken).
    #[export]
    #[var(set = set_outline_color)]
    pub outline_color: Color,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for PixelObjectProfile {
    fn init(base: Base<Resource>) -> Self {
        PixelObjectProfile {
            target: NodePath::default(),
            enabled: true,
            pixel_size: -1,
            outline: -1,
            outline_id: -1,
            palette: -1,
            dither: -1,
            override_outline_color: false,
            outline_color: Color::from_rgba(0.04, 0.05, 0.09, 1.0),
            base,
        }
    }
}

#[godot_api]
impl PixelObjectProfile {
    #[func]
    fn set_target(&mut self, v: NodePath) {
        self.target = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_enabled(&mut self, v: bool) {
        self.enabled = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = if v < 1 { -1 } else { v.min(4) };
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_outline(&mut self, v: i32) {
        self.outline = v.clamp(-1, 1);
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_outline_id(&mut self, v: i32) {
        self.outline_id = v.clamp(-2, 253);
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_palette(&mut self, v: i32) {
        self.palette = v.clamp(-1, 1);
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_dither(&mut self, v: i32) {
        self.dither = v.clamp(-1, 1);
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_override_outline_color(&mut self, v: bool) {
        self.override_outline_color = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_outline_color(&mut self, v: Color) {
        self.outline_color = v;
        self.base_mut().emit_changed();
    }

    /// True when any value overrides the owning PixelObject.
    #[func]
    pub fn has_overrides(&self) -> bool {
        self.pixel_size >= 1 || self.outline >= 0 || self.outline_id != -1 || self.palette >= 0 || self.dither >= 0 || self.override_outline_color
    }
}
