use godot::classes::{EditorInterface, Node3D, Object};
use godot::prelude::*;
const EDIT_LOCK_META: &str = "_edit_lock_";
const MST_LOCK_META: &str = "_mst_tool_lock";
#[derive(Default)]
pub struct GizmoLock {
    target: Option<Gd<Node3D>>,
}
impl GizmoLock {
    pub fn sync(&mut self, want: Option<Gd<Node3D>>) {
        let want = want.filter(|w| w.is_instance_valid() && w.is_inside_tree());
        let same = want.as_ref().map(|w| w.instance_id())
            == self
                .target
                .as_ref()
                .filter(|t| t.is_instance_valid())
                .map(|t| t.instance_id());
        if same
            && self
                .target
                .as_ref()
                .is_none_or(|t| t.has_meta(MST_LOCK_META) || t.has_meta(EDIT_LOCK_META))
        {
            return;
        }
        let mut changed = self.release();
        if let Some(mut t) = want {
            if !t.has_meta(EDIT_LOCK_META) {
                t.set_meta(EDIT_LOCK_META, &true.to_variant());
                t.set_meta(MST_LOCK_META, &true.to_variant());
                changed = true;
            }
            self.target = Some(t);
        }
        if changed {
            nudge_selection();
        }
    }
    pub fn peel(&mut self) {
        if let Some(t) = &mut self.target
            && t.is_instance_valid()
            && t.has_meta(MST_LOCK_META)
        {
            t.remove_meta(MST_LOCK_META);
            t.remove_meta(EDIT_LOCK_META);
        }
    }
    fn release(&mut self) -> bool {
        let Some(mut t) = self.target.take() else {
            return false;
        };
        if t.is_instance_valid() && t.has_meta(MST_LOCK_META) {
            t.remove_meta(MST_LOCK_META);
            t.remove_meta(EDIT_LOCK_META);
            return true;
        }
        false
    }
    pub fn purge(&mut self) {
        if self.release() {
            nudge_selection();
        }
    }
}
fn nudge_selection() {
    if let Some(sel) = EditorInterface::singleton().get_selection() {
        sel.upcast::<Object>()
            .call_deferred("emit_signal", &["selection_changed".to_variant()]);
    }
}
