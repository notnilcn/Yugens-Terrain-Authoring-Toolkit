//! The shared effect clock: the `pt_time` global every addon shader animates with instead of
//! `TIME`.
//!
//! Why not `TIME`: the editor redraws every frame while any visible shader reads `TIME`, so a
//! scene with wind on its grass or waves on its water kept the editor rendering at full rate
//! even when nothing was being edited. `pt_time` only moves while effects are animating:
//! always in a running game, and in the editor only while "Animate Effects" is on (the
//! Pixel World menu in the 3D toolbar, stored per user in the editor settings).
//!
//! The clock has no node of its own: every addon node calls `tick` from its `_process`; the
//! first call of a frame advances it, the others are no-ops. Effect nodes integrate their own
//! motion with `step`, which is 0 while the clock is paused, so a paused editor writes nothing.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use godot::classes::Engine;
use godot::prelude::*;

use crate::globals;
use crate::util::is_editor;

/// `pt_time` wraps like Godot's `TIME` (`rendering/limits/time/time_rollover_secs` default),
/// so shaders never lose float precision in long sessions.
const ROLLOVER_SECONDS: f32 = 3600.0;

static TIME_BITS: AtomicU32 = AtomicU32::new(0);
/// Process frame of the last `tick` + 1 (0 = never ticked).
static LAST_FRAME: AtomicU64 = AtomicU64::new(0);
static ANIMATE_IN_EDITOR: AtomicBool = AtomicBool::new(false);

/// True when effects animate: always at runtime, in the editor only with "Animate Effects".
pub fn running() -> bool {
    !is_editor() || ANIMATE_IN_EDITOR.load(Ordering::Relaxed)
}

/// Editor: animate effects (clouds drift, birds fly, water waves) or keep them still so the
/// editor can idle. Has no effect at runtime, where effects always animate.
pub fn set_animate_in_editor(on: bool) {
    ANIMATE_IN_EDITOR.store(on, Ordering::Relaxed);
}

/// Current clock value (seconds, wraps every hour).
pub fn now() -> f32 {
    f32::from_bits(TIME_BITS.load(Ordering::Relaxed))
}

/// Advances the clock by this frame's `delta` once per process frame (later calls in the same
/// frame are ignored) and publishes `pt_time`. Call it first thing in `_process`.
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
    globals::set("pt_time", &t.to_variant());
}

/// Ticks the clock and returns the seconds an effect should advance this frame: `delta` while
/// the clock runs, 0 while it is paused (editor without "Animate Effects").
pub fn step(delta: f64) -> f64 {
    tick(delta);
    if running() { delta } else { 0.0 }
}
