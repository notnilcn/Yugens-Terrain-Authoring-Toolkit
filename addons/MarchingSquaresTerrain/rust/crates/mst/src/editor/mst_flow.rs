//! A row-wrapping container for the terrain editor's settings bar, which spreads its
//! controls over as few rows as fit and keeps those rows about the same width.
//!
//! Intent: the bar under the 3D viewport shows a different set of settings for every tool,
//! and the viewport can be any width. Whoever fills the bar expects its controls to wrap
//! onto more rows when space runs out, without leaving a long first row and a short,
//! ragged last one.
//!
//! Responsibility: it decides where each visible child sits and how tall it has to be to
//! show them all, and it tells the scrolling bar around it how much of that height to claim
//! from the editor, so that a tall set of settings scrolls instead of squeezing the view it
//! sits under. It does not create, order, size or style its children; whoever fills it
//! decides what goes in and in which order.
//!
//! Why a node: arranging children inside the user interface tree, and being told by the
//! engine when they must be rearranged or when the space around them changes, is what
//! containers in the tree are for.
//!
//! Why a new type: the engine's own flow containers wrap greedily, filling each row before
//! starting the next, which is exactly the ragged result this one exists to avoid. The
//! balancing rule itself is plain arithmetic kept apart from the node so it can be tested
//! without the engine.
//!
//! Not exported: the height it last needed for its rows, which is a result of the most
//! recent layout and would only disagree with the children if someone set it by hand.
use godot::classes::Control;
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = Container, internal)]
pub struct MstFlow {
    #[export]
    #[var(set = set_h_gap)]
    pub h_gap: f32,
    min_height: f32,
    base: Base<godot::classes::Container>,
}
#[godot_api]
impl godot::classes::IContainer for MstFlow {
    fn get_minimum_size(&self) -> Vector2 {
        let w = self
            .tiles()
            .iter()
            .map(|c| c.get_combined_minimum_size().x)
            .fold(0.0, f32::max);
        Vector2::new(w, self.min_height)
    }
    fn on_notification(&mut self, what: godot::classes::notify::ContainerNotification) {
        use godot::classes::notify::ContainerNotification as N;
        match what {
            N::SORT_CHILDREN => self.layout(),
            N::ENTER_TREE => self.watch_host(true),
            N::EXIT_TREE => self.watch_host(false),
            _ => {}
        }
    }
}
#[godot_api]
impl MstFlow {
    #[func]
    fn set_h_gap(&mut self, gap: f32) {
        self.h_gap = gap;
        self.base_mut().queue_sort();
    }
}
impl MstFlow {
    fn bar(&self) -> Option<Gd<Control>> {
        self.base()
            .get_parent()
            .and_then(|p| p.try_cast::<godot::classes::ScrollContainer>().ok())
            .map(|s| s.upcast())
    }
    fn host(&self) -> Option<Gd<Control>> {
        self.bar()
            .and_then(|b| b.get_parent())
            .and_then(|p| p.try_cast::<Control>().ok())
    }
    fn watch_host(&mut self, on: bool) {
        let Some(mut host) = self.host() else { return };
        let cb = Callable::from_object_method(&self.to_gd(), "queue_sort");
        let linked = host.is_connected("resized", &cb);
        if on && !linked {
            host.connect("resized", &cb);
        } else if !on && linked {
            host.disconnect("resized", &cb);
        }
    }
    fn claim_bar_height(&mut self, content: f32) {
        let Some(mut bar) = self.bar() else { return };
        let room = self.host().map(|h| h.get_size().y).unwrap_or(f32::INFINITY);
        let row = super::ui_kit::sz(0.0, super::ui_kit::ROW_HEIGHT).y;
        let want = bar_claim(content, room, row);
        let mut min = bar.get_custom_minimum_size();
        if (min.y - want).abs() > 0.5 {
            min.y = want;
            bar.set_custom_minimum_size(min);
        }
    }
    fn tiles(&self) -> Vec<Gd<Control>> {
        self.base()
            .get_children()
            .iter_shared()
            .filter_map(|c| c.try_cast::<Control>().ok())
            .filter(|c| c.is_visible() && !c.is_set_as_top_level())
            .collect()
    }
    fn layout(&mut self) {
        let cells = self.tiles();
        let sizes: Vec<Vector2> = cells
            .iter()
            .map(|c| c.get_combined_minimum_size())
            .collect();
        let avail = self.base().get_size().x;
        let starts = balanced_rows(
            &sizes.iter().map(|s| s.x).collect::<Vec<_>>(),
            self.h_gap,
            avail,
        );
        let mut y = 0.0;
        for (r, &s) in starts.iter().enumerate() {
            let e = starts.get(r + 1).copied().unwrap_or(cells.len());
            let h = sizes[s..e].iter().map(|v| v.y).fold(0.0, f32::max);
            let mut x = 0.0;
            for i in s..e {
                let rect = Rect2::new(Vector2::new(x, y), Vector2::new(sizes[i].x, h));
                self.base_mut().fit_child_in_rect(&cells[i], rect);
                x += sizes[i].x + self.h_gap;
            }
            y += h;
        }
        if (y - self.min_height).abs() > 0.5 {
            self.min_height = y;
            self.base_mut().update_minimum_size();
        }
        self.claim_bar_height(y);
    }
}
pub const BAR_SHARE: f32 = 0.6;
pub fn bar_claim(content: f32, room: f32, row: f32) -> f32 {
    let cap = if room.is_finite() { (room * BAR_SHARE).floor() } else { content };
    content.min(cap).max(row)
}
pub fn balanced_rows(widths: &[f32], gap: f32, avail: f32) -> Vec<usize> {
    if widths.is_empty() {
        return Vec::new();
    }
    let greedy = |limit: f32| {
        let mut starts = vec![0];
        let mut w = 0.0f32;
        for (i, &x) in widths.iter().enumerate() {
            let last = starts.last().copied().unwrap_or(0);
            let next = if i == last { x } else { w + gap + x };
            if i > last && next > limit + 1e-3 {
                starts.push(i);
                w = x;
            } else {
                w = next;
            }
        }
        starts
    };
    let first = greedy(avail.max(1.0));
    if first.len() <= 1 {
        return first;
    }
    let rows = first.len();
    let (mut lo, mut hi) = (widths.iter().cloned().fold(0.0, f32::max), avail.max(1.0));
    if hi < lo {
        return first;
    }
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if greedy(mid).len() <= rows {
            hi = mid
        } else {
            lo = mid
        }
    }
    greedy(hi)
}
#[cfg(test)]
mod tests {
    #[test]
    fn balanced_rows() {
        assert_eq!(
            super::balanced_rows(&[10.0, 10.0, 10.0], 2.0, 100.0),
            vec![0]
        );
        let w = [20.0; 5];
        assert_eq!(super::balanced_rows(&w, 0.0, 85.0), vec![0, 3]);
        let r = super::balanced_rows(&[50.0, 50.0, 200.0], 0.0, 120.0);
        assert_eq!(r.len(), 2);
        assert!(super::balanced_rows(&[], 0.0, 10.0).is_empty());
    }
    #[test]
    fn bar_claim_leaves_the_view_room() {
        assert_eq!(super::bar_claim(140.0, 400.0, 35.0), 140.0);
        assert_eq!(super::bar_claim(385.0, 427.0, 35.0), 256.0);
        assert_eq!(super::bar_claim(10.0, 400.0, 35.0), 35.0);
        assert_eq!(super::bar_claim(385.0, 20.0, 35.0), 35.0);
        assert_eq!(super::bar_claim(200.0, f32::INFINITY, 35.0), 200.0);
    }
}
