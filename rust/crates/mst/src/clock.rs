use crate::editor_detection::is_editor;
use crate::globals;
use godot::classes::Engine;
use godot::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
const ROLLOVER_SECONDS: f32 = 3600.0;
static TIME_BITS: AtomicU32 = AtomicU32::new(0);
static LAST_FRAME: AtomicU64 = AtomicU64::new(0);
static ANIMATE_IN_EDITOR: AtomicBool = AtomicBool::new(false);
pub fn running() -> bool {
    !is_editor() || ANIMATE_IN_EDITOR.load(Ordering::Relaxed)
}
pub fn set_animate_in_editor(on: bool) {
    ANIMATE_IN_EDITOR.store(on, Ordering::Relaxed);
}
pub fn now() -> f32 {
    f32::from_bits(TIME_BITS.load(Ordering::Relaxed))
}
pub fn tick(delta: f64) {
    let frame = Engine::singleton().get_process_frames() + 1;
    if LAST_FRAME.swap(frame, Ordering::Relaxed) == frame {
        return;
    }
    if !running() {
        return;
    }
    let t = (now() + delta as f32).rem_euclid(ROLLOVER_SECONDS);
    TIME_BITS.store(t.to_bits(), Ordering::Relaxed);
    globals::assign("pt_time", &t.to_variant());
}
pub fn step(delta: f64) -> f64 {
    tick(delta);
    if running() { delta } else { 0.0 }
}
