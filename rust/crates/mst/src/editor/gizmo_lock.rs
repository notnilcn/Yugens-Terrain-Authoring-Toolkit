//! Hides Godot's own move / rotate / scale gizmo on a node while an MST tool paints in the
//! viewport (it was drawn over the terrain and hid the brush cursor).
//!
//! Same technique as the builder (Unibuilder `_sync_native_gizmo_lock`): the editor's
//! `_edit_lock_` meta, next to a marker meta so only our own lock is ever removed (a node the
//! user locked himself is left alone). The lock never reaches a scene file: the owning plugin
//! calls [`GizmoLock::strip`] from `_apply_changes` (run by the editor right before it packs a
//! scene) and [`GizmoLock::sync`] puts it back on the next frame.

use godot::classes::{EditorInterface, Node3D, Object};
use godot::prelude::*;

const EDIT_LOCK_META: &str = "_edit_lock_";
/// Marker meta: the `_edit_lock_` next to it is ours.
const MST_LOCK_META: &str = "_mst_tool_lock";

#[derive(Default)]
pub struct GizmoLock {
    target: Option<Gd<Node3D>>,
}

impl GizmoLock {
    /// Locks `want` (or nothing): unlocks the previous target, locks the new one, and nudges the
    /// 3D editor so it rebuilds its gizmo. Cheap when nothing changed; call it every frame.
    pub fn sync(&mut self, want: Option<Gd<Node3D>>) {
        let want = want.filter(|w| w.is_instance_valid() && w.is_inside_tree());
        let same = want.as_ref().map(|w| w.instance_id()) == self.target.as_ref().filter(|t| t.is_instance_valid()).map(|t| t.instance_id());
        if same && self.target.as_ref().is_none_or(|t| t.has_meta(MST_LOCK_META) || t.has_meta(EDIT_LOCK_META)) {
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

    /// Removes our lock from the target (scene save, plugin exit). `sync` adds it back.
    pub fn strip(&mut self) {
        if let Some(t) = &mut self.target
            && t.is_instance_valid()
            && t.has_meta(MST_LOCK_META)
        {
            t.remove_meta(MST_LOCK_META);
            t.remove_meta(EDIT_LOCK_META);
        }
    }

    /// Unlocks and forgets the target; true when a lock was removed.
    fn release(&mut self) -> bool {
        let Some(mut t) = self.target.take() else { return false };
        if t.is_instance_valid() && t.has_meta(MST_LOCK_META) {
            t.remove_meta(MST_LOCK_META);
            t.remove_meta(EDIT_LOCK_META);
            return true;
        }
        false
    }

    /// Unlocks the target (tool left, plugin hidden or removed).
    pub fn clear(&mut self) {
        if self.release() {
            nudge_selection();
        }
    }
}

/// The 3D editor rebuilds its transform gizmo on a selection change (Unibuilder's "selection
/// nudge").
fn nudge_selection() {
    if let Some(sel) = EditorInterface::singleton().get_selection() {
        sel.upcast::<Object>().call_deferred("emit_signal", &["selection_changed".to_variant()]);
    }
}
