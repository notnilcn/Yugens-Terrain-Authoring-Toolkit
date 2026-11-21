//! A row-wrapping container for the terrain editor's settings bar, which spreads its
//! controls over as few rows as fit and keeps those rows about the same width.
//!
//! Intent: the bar under the 3D viewport shows a different set of settings for every tool,
//! and the viewport can be any width. Whoever fills the bar expects its controls to wrap
//! onto more rows when space runs out, without leaving a long first row and a short,
//! ragged last one.
//!
//! Responsibility: it decides only where each visible child sits and how tall it has to be
//! to show them all. It does not create, order, size or style its children; whoever fills
//! it decides what goes in and in which order.
//!
//! Why a node: arranging children inside the user interface tree, and being told by the
//! engine when they must be rearranged, is what containers in the tree are for.
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
        if what == godot::classes::notify::ContainerNotification::SORT_CHILDREN {
            self.layout();
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
    }
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
}
