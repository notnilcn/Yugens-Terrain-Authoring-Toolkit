use godot::classes::{Engine, Node};
use godot::prelude::*;
pub fn live<T: GodotClass>(n: &Option<Gd<T>>) -> Option<Gd<T>> {
    n.as_ref().filter(|n| n.is_instance_valid()).cloned()
}
const MISS_RECHECK_FRAMES: u64 = 30;
struct Miss {
    sibling_count: i32,
    frame: u64,
}
pub struct SiblingFallback<T: GodotClass> {
    cached: Option<Gd<T>>,
    last_miss: Option<Miss>,
}
impl<T: GodotClass> Default for SiblingFallback<T> {
    fn default() -> Self {
        Self {
            cached: None,
            last_miss: None,
        }
    }
}
impl<T: GodotClass + Inherits<Node>> SiblingFallback<T> {
    pub fn settle(&mut self, owner: &Gd<Node>, explicit: &Option<Gd<T>>) -> Option<Gd<T>> {
        if let Some(n) = live(explicit) {
            return Some(n);
        }
        let parent = owner.get_parent()?;
        if let Some(c) = self.cached.as_ref().filter(|c| c.is_instance_valid())
            && c.clone().upcast::<Node>().get_parent().as_ref() == Some(&parent)
        {
            return Some(c.clone());
        }
        let sibling_count = parent.get_child_count();
        let frame = Engine::singleton().get_process_frames();
        if let Some(miss) = &self.last_miss
            && miss.sibling_count == sibling_count
            && frame.saturating_sub(miss.frame) < MISS_RECHECK_FRAMES
        {
            return None;
        }
        self.cached = parent
            .get_children()
            .iter_shared()
            .find_map(|n| n.try_cast::<T>().ok());
        self.last_miss = match self.cached {
            Some(_) => None,
            None => Some(Miss {
                sibling_count,
                frame,
            }),
        };
        self.cached.clone()
    }
    pub fn revert(&mut self) {
        *self = Self::default();
    }
}
