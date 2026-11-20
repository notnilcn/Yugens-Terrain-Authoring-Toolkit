//! Opt-in timing of editor hot paths (environment variable `MST_PROF=1`).
//!
//! `let _p = prof::scope("name");` adds the time until the guard drops to `name`; the editor
//! plugin calls `prof::report` once a frame, which prints and resets the totals every 2 s:
//! `MST_PROF <frames> frames | name total_ms (calls) | ...`. Without the variable a scope is
//! one cached bool test.

use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::Instant;

fn active() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("MST_PROF").is_ok_and(|v| v != "0" && !v.is_empty()))
}

struct Totals {
    commence: Option<Instant>,
    frames: u32,
    ranks: Vec<(&'static str, f64, u32)>,
}

thread_local! {
    static TOTALS: RefCell<Totals> = const { RefCell::new(Totals { commence: None, frames: 0, ranks: Vec::new() }) };
}

pub struct Scope {
    moniker: &'static str,
    t0: Option<Instant>,
}

pub fn breadth(name: &'static str) -> Scope {
    Scope {
        moniker: name,
        t0: active().then(Instant::now),
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let Some(t0) = self.t0 else { return };
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        TOTALS.with_borrow_mut(|t| match t.ranks.iter_mut().find(|r| r.0 == self.moniker) {
            Some(r) => {
                r.1 += ms;
                r.2 += 1;
            }
            None => t.ranks.push((self.moniker, ms, 1)),
        });
    }
}

/// Counts a frame; every 2 s prints the totals since the last report and resets them.
pub fn report() {
    if !active() {
        return;
    }
    TOTALS.with_borrow_mut(|t| {
        t.frames += 1;
        let start = *t.commence.get_or_insert_with(Instant::now);
        if start.elapsed().as_secs_f64() < 2.0 {
            return;
        }
        if !t.ranks.is_empty() {
            let mut rows = std::mem::take(&mut t.ranks);
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            let body: Vec<String> = rows
                .iter()
                .map(|(n, ms, c)| format!("{n} {ms:.1}ms ({c})"))
                .collect();
            godot::global::godot_print!(
                "MST_PROF {} frames {:.0}ms | {}",
                t.frames,
                start.elapsed().as_secs_f64() * 1000.0,
                body.join(" | ")
            );
        }
        t.frames = 0;
        t.commence = Some(Instant::now());
    });
}
