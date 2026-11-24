//! How many of each pixel node the edited scene holds, counted in one pass when the scene
//! changes, so the workspace can dim, label and offer its sections without searching the
//! scene each time it redraws.
use super::sections::{ADDABLE, SECTIONS};
use godot::classes::EditorInterface;
use godot::prelude::*;
pub const TERRAIN: &str = "MarchingSquaresTerrain";
#[derive(Default, PartialEq)]
pub struct SceneCensus {
    counts: Vec<(&'static str, usize)>,
}
fn counted_classes() -> Vec<&'static str> {
    let mut classes: Vec<&'static str> = ADDABLE.iter().map(|(c, _, _)| *c).collect();
    classes.extend(SECTIONS.iter().filter_map(|s| s.class));
    classes.push(TERRAIN);
    classes.sort_unstable();
    classes.dedup();
    classes
}
impl SceneCensus {
    pub fn of_edited_scene() -> Self {
        let mut counts: Vec<(&'static str, usize)> =
            counted_classes().into_iter().map(|c| (c, 0)).collect();
        if let Some(root) = EditorInterface::singleton().get_edited_scene_root() {
            let descendants = root
                .find_children_ex("*")
                .recursive(true)
                .owned(true)
                .done();
            for n in std::iter::once(root.clone()).chain(descendants.iter_shared()) {
                for (class, count) in counts.iter_mut() {
                    if n.is_class(*class) {
                        *count += 1;
                    }
                }
            }
        }
        SceneCensus { counts }
    }
    pub fn count(&self, class: &str) -> usize {
        self.counts
            .iter()
            .find(|(c, _)| *c == class)
            .map_or(0, |(_, n)| *n)
    }
    pub fn has(&self, class: &str) -> bool {
        self.count(class) > 0
    }
}
