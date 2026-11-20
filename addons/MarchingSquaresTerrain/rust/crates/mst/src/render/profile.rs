//! `PixelObjectProfile` — a per-target override inside a `PixelObject` (the original pixel
//! pipeline's per-target profile resource). Stored in `PixelObject.profiles`; `target` is a path
//! relative to that PixelObject. Every value left at -1 inherits the owning PixelObject's
//! effective value.

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
    #[var(set = assign_active)]
    pub active: bool,
    /// Block size multiplier (>= 1); -1 inherits.
    #[export(range = (-1.0, 16.0, 1.0, or_greater))]
    #[var(set = set_pixel_size)]
    pub pixel_size: i32,
    /// -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_outline)]
    pub outline: i32,
    /// Outline palette id 0-253; -1 inherits, -2 allocates an automatic id for this target
    /// (the original's AUTO id mode).
    #[export(range = (-2.0, 253.0, 1.0))]
    #[var(set = set_outline_id)]
    pub outline_id: i32,
    /// Palette LUT grading: -1 inherit, 0 skip, 1 grade.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = assign_swatchset)]
    pub palette: i32,
    /// Ordered dither: -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_dither)]
    pub dither: i32,
    /// Register `outline_color` for this target's outline id.
    #[export]
    #[var(set = assign_supersede_contour_tint)]
    pub supersede_contour_tint: bool,
    /// Alpha = opacity (0 = darken).
    #[export]
    #[var(set = assign_contour_tint)]
    pub contour_tint: Color,
    base: Base<Resource>,
}

#[godot_api]
impl IResource for PixelObjectProfile {
    fn init(base: Base<Resource>) -> Self {
        PixelObjectProfile {
            target: NodePath::default(),
            active: true,
            pixel_size: -1,
            outline: -1,
            outline_id: -1,
            palette: -1,
            dither: -1,
            supersede_contour_tint: false,
            contour_tint: Color::from_rgba(0.0405, 0.0505, 0.0905, 1.0),
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
    fn assign_active(&mut self, v: bool) {
        self.active = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = if v < 1 {
            -1
        } else {
            v.min(pt_core::meta::MAX_PIXEL_SIZE as i32)
        };
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
    fn assign_swatchset(&mut self, v: i32) {
        self.palette = v.clamp(-1, 1);
        self.base_mut().emit_changed();
    }
    #[func]
    fn set_dither(&mut self, v: i32) {
        self.dither = v.clamp(-1, 1);
        self.base_mut().emit_changed();
    }
    #[func]
    fn assign_supersede_contour_tint(&mut self, v: bool) {
        self.supersede_contour_tint = v;
        self.base_mut().emit_changed();
    }
    #[func]
    fn assign_contour_tint(&mut self, v: Color) {
        self.contour_tint = v;
        self.base_mut().emit_changed();
    }

    /// True when any value overrides the owning PixelObject.
    #[func]
    pub fn has_overrides(&self) -> bool {
        self.pixel_size >= 1
            || self.outline >= 0
            || self.outline_id != -1
            || self.palette >= 0
            || self.dither >= 0
            || self.supersede_contour_tint
    }
}
