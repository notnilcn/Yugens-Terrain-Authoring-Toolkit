//! Builder edits belong to the scene being edited, even when the mesh data lives in its own
//! resource file, so every step is opened against that scene's history and carries the
//! selection it was made with.
use super::*;
use godot::classes::EditorUndoRedoManager;
pub(in crate::builder) fn open_scene_action(
    ur: &mut Gd<EditorUndoRedoManager>,
    name: &str,
    merge: godot::classes::undo_redo::MergeMode,
) {
    match EditorInterface::singleton().get_edited_scene_root() {
        Some(root) => ur
            .create_action_ex(name)
            .merge_mode(merge)
            .custom_context(&root.upcast::<Object>())
            .done(),
        None => ur.create_action_ex(name).merge_mode(merge).done(),
    }
}
pub(super) fn reown_descendants_on_undo(
    ur: &mut Gd<EditorUndoRedoManager>,
    node: &Gd<Node>,
    root: &Gd<Node>,
) {
    for c in node.find_children_ex("*").owned(false).done().iter_shared() {
        if c.get_owner().is_some_and(|o| o.instance_id() == root.instance_id()) {
            ur.add_undo_method(&c, "set_owner", &[root.to_variant()]);
        }
    }
}
pub(super) fn pick_changed(before: &[i32], after: &[i32]) -> bool {
    before != after
}
fn encode_picked(
    mode: Mode,
    faces: &BTreeSet<usize>,
    edges: &BTreeSet<Edge>,
    verts: &BTreeSet<u32>,
    active: Option<u32>,
) -> Vec<i32> {
    let mut out = vec![mode.bit() as i32, faces.len() as i32];
    out.extend(faces.iter().map(|f| *f as i32));
    out.push(edges.len() as i32);
    out.extend(edges.iter().flat_map(|e| [e.0 as i32, e.1 as i32]));
    out.push(verts.len() as i32);
    out.extend(verts.iter().map(|v| *v as i32));
    out.push(active.map(|v| v as i32).unwrap_or(-1));
    out
}
type Picked = (BTreeSet<usize>, BTreeSet<Edge>, BTreeSet<u32>, Option<u32>);
fn decode_picked(s: &[i32]) -> Option<(i32, Picked)> {
    let mut it = s.iter().copied();
    let bit = it.next()?;
    let mut take = |n: usize| -> Option<Vec<i32>> {
        let v: Vec<i32> = it.by_ref().take(n).collect();
        (v.len() == n && v.iter().all(|x| *x >= 0)).then_some(v)
    };
    let faces = {
        let n = *take(1)?.first()? as usize;
        take(n)?.into_iter().map(|f| f as usize).collect()
    };
    let edges = {
        let n = *take(1)?.first()? as usize;
        take(n * 2)?
            .chunks_exact(2)
            .map(|p| pt_modeler::edge(p[0] as u32, p[1] as u32))
            .collect()
    };
    let verts = {
        let n = *take(1)?.first()? as usize;
        take(n)?.into_iter().map(|v| v as u32).collect()
    };
    let active = it.next().filter(|v| *v >= 0).map(|v| v as u32);
    Some((bit, (faces, edges, verts, active)))
}
impl PixelBuilderPlugin {
    pub(super) fn open_action(&self, name: &str) -> Option<Gd<EditorUndoRedoManager>> {
        let mut ur = self.base().get_undo_redo()?;
        open_scene_action(&mut ur, name, godot::classes::undo_redo::MergeMode::DISABLE);
        Some(ur)
    }
    pub(super) fn picked_snapshot(&self) -> PackedInt32Array {
        PackedInt32Array::from(
            encode_picked(
                self.mode,
                &self.sel_faces,
                &self.sel_edges,
                &self.sel_verts,
                self.active_vert,
            )
            .as_slice(),
        )
    }
    pub(super) fn picked_snapshot_of(mode: Mode, sel: &Picked) -> PackedInt32Array {
        PackedInt32Array::from(encode_picked(mode, &sel.0, &sel.1, &sel.2, sel.3).as_slice())
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    pub(super) fn restore_picked(&mut self, snapshot: PackedInt32Array) {
        let Some((bit, picked)) = decode_picked(snapshot.as_slice()) else {
            return;
        };
        if bit != self.mode.bit() as i32 || self.target.is_none() {
            return;
        }
        (
            self.sel_faces,
            self.sel_edges,
            self.sel_verts,
            self.active_vert,
        ) = picked;
        self.sync_ui();
        self.redraw();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_new_pick_or_mode_change_counts_as_a_selection_change() {
        let none = BTreeSet::new();
        let one_edge = BTreeSet::from([pt_modeler::edge(1, 2)]);
        let snap = |mode, edges: &BTreeSet<Edge>| encode_picked(mode, &none, edges, &BTreeSet::new(), None);
        let empty = snap(Mode::Edge, &BTreeSet::new());
        assert!(!pick_changed(&empty, &snap(Mode::Edge, &BTreeSet::new())));
        assert!(pick_changed(&empty, &snap(Mode::Edge, &one_edge)));
        assert!(pick_changed(&snap(Mode::Edge, &one_edge), &empty), "a deselect counts too");
        assert!(pick_changed(&empty, &snap(Mode::Vertex, &BTreeSet::new())));
    }
    #[test]
    fn picked_snapshot_round_trips() {
        let faces = BTreeSet::from([0usize, 4, 9]);
        let edges = BTreeSet::from([pt_modeler::edge(1, 2), pt_modeler::edge(7, 3)]);
        let verts = BTreeSet::from([5u32]);
        let s = encode_picked(Mode::Edge, &faces, &edges, &verts, Some(5));
        let (bit, (f, e, v, a)) = decode_picked(&s).expect("decodes");
        assert_eq!(bit, Mode::Edge.bit() as i32);
        assert_eq!((f, e, v, a), (faces, edges, verts, Some(5)));
        assert!(
            decode_picked(&[1, 3, 0]).is_none(),
            "truncated snapshot is rejected"
        );
    }
}
