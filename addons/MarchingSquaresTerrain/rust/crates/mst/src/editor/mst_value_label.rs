//! A read-out beside a slider in the editor's panels that shows the slider's current value
//! with a fixed number of decimals.
//!
//! Intent: whoever builds a slider row expects the number next to it to follow the slider
//! as it is dragged, rounded to the precision the slider moves in, without writing any
//! formatting of their own.
//!
//! Responsibility: it turns each value it is sent into its own text. It does not know which
//! slider drives it, what the value means or when the value is applied; the row that owns
//! both connects them.
//!
//! Why a node: it is a piece of the editor's user interface and has to sit in the tree next
//! to the slider it reports on.
//!
//! Why a new type: a plain label cannot format a number it receives, and the alternative of
//! a label driven by a closure stops working after the extension is reloaded in the editor,
//! because such connections are dropped on reload while a connection to a registered method
//! survives it.
use super::ui_kit::sz;
use godot::classes::Label;
use godot::classes::control::SizeFlags;
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = Label, internal)]
pub struct MstValueLabel {
    #[export]
    decimals: i64,
    base: Base<Label>,
}
#[godot_api]
impl MstValueLabel {
    #[func]
    fn _mst_assign_worth(&mut self, value: f64) {
        let d = self.decimals.max(0) as usize;
        self.base_mut().set_text(&format!("{value:.d$}"));
    }
}
pub fn value_label(value: f64, step: f64, min_w: f32) -> Gd<Label> {
    let mut l = MstValueLabel::new_alloc();
    l.bind_mut().decimals = decimals(step) as i64;
    let mut v = l.upcast::<Label>();
    v.set_text(&format!("{value:.*}", decimals(step)));
    v.set_vertical_alignment(VerticalAlignment::CENTER);
    v.set_horizontal_alignment(HorizontalAlignment::RIGHT);
    v.set_custom_minimum_size(sz(min_w, 0.0));
    v.set_v_size_flags(SizeFlags::FILL);
    v.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.7505));
    v
}
pub fn value_label_updater(label: Gd<Label>, _step: f64) -> Callable {
    label.callable("_mst_assign_worth")
}
pub fn decimals(step: f64) -> usize {
    let mut d = 0;
    let mut s = step.abs();
    while d < 3 && (s - s.round()).abs() > 1e-6 {
        s *= 10.0;
        d += 1;
    }
    d
}
#[cfg(test)]
mod tests {
    #[test]
    fn decimals() {
        assert_eq!(super::decimals(1.0), 0);
        assert_eq!(super::decimals(0.5), 1);
        assert_eq!(super::decimals(0.25), 2);
        assert_eq!(super::decimals(0.005), 3);
    }
}
