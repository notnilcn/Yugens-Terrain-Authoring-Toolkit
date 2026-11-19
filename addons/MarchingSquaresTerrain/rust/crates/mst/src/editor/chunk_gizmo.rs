//! Chunk vertex handles (the original's `MarchingSquaresTerrainChunkGizmo` /
//! `…CellChunkGizmo`): with `chunk_handles` on, the selected terrain shows one drag handle per
//! sample of `handle_chunk` (vertices on square grids, cell centres otherwise). Dragging moves
//! the sample vertically; every drag is one undo step ("move terrain point").
//! Chunk Manager's chunk selection (Ctrl+click / the chunk list) sets `handle_chunk`.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::{Camera3D, EditorInterface, EditorNode3DGizmo, EditorNode3DGizmoPlugin, EditorPlugin, IEditorNode3DGizmoPlugin, Node3D};
use godot::prelude::*;

use crate::terrain::MarchingSquaresTerrain;

#[derive(GodotClass)]
#[class(tool, init, base = EditorNode3DGizmoPlugin)]
pub struct MarchingSquaresChunkGizmoPlugin {
    /// Handle id -> cell, per terrain instance (filled by `redraw`).
    cells: HashMap<i64, Vec<Vector2i>>,
    /// Chunk blobs before the current drag (restored on cancel, used for undo).
    drag_before: Option<VarDictionary>,
    material_ready: bool,
    base: Base<EditorNode3DGizmoPlugin>,
}

fn terrain_of(gizmo: &Option<Gd<EditorNode3DGizmo>>) -> Option<Gd<Node3D>> {
    gizmo.as_ref()?.get_node_3d().filter(|n| n.clone().try_cast::<MarchingSquaresTerrain>().is_ok())
}

impl MarchingSquaresChunkGizmoPlugin {
    fn cell(&self, terrain: &Gd<Node3D>, id: i32) -> Option<Vector2i> {
        self.cells.get(&terrain.instance_id().to_i64())?.get(id as usize).copied()
    }

    /// Blobs of the handle chunk and its surrounding chunks (shared borders / walls).
    pub(super) fn snapshot(terrain: &mut Gd<Node3D>, center: Vector2i) -> VarDictionary {
        let mut d = VarDictionary::new();
        let chunks = terrain.call("get_chunks", &[]).to::<Array<Vector2i>>();
        for c in chunks.iter_shared() {
            if (c.x - center.x).abs() <= 1 && (c.y - center.y).abs() <= 1 {
                d.set(c, &terrain.call("get_chunk_data", &[c.to_variant()]));
            }
        }
        d
    }
}

#[godot_api]
impl IEditorNode3DGizmoPlugin for MarchingSquaresChunkGizmoPlugin {
    fn get_gizmo_name(&self) -> GString {
        "MarchingSquaresTerrainChunkHandles".into()
    }

    fn has_gizmo(&self, node: Option<Gd<Node3D>>) -> bool {
        node.is_some_and(|n| n.try_cast::<MarchingSquaresTerrain>().is_ok())
    }

    fn redraw(&mut self, gizmo: Option<Gd<EditorNode3DGizmo>>) {
        let Some(mut g) = gizmo.clone() else { return };
        g.clear();
        let Some(mut terrain) = terrain_of(&gizmo) else { return };
        if !terrain.get("chunk_handles").to::<bool>() {
            return;
        }
        // Only for the selected terrain, like the original.
        let selected = EditorInterface::singleton()
            .get_selection()
            .map(|s| s.get_selected_nodes().iter_shared().any(|n| n.instance_id() == terrain.instance_id()))
            .unwrap_or(false);
        if !selected {
            return;
        }
        let chunk = terrain.get("handle_chunk").to::<Vector2i>();
        if !terrain.call("has_chunk", &[chunk.to_variant()]).to::<bool>() {
            return;
        }
        if !self.material_ready {
            self.base_mut().create_handle_material("handles");
            self.material_ready = true;
        }
        let cells = terrain.call("get_chunk_cells", &[chunk.to_variant()]).to::<Array<Vector2i>>();
        let inv = terrain.get_global_transform().affine_inverse();
        let mut points = PackedVector3Array::new();
        let mut ids = PackedInt32Array::new();
        let mut list = Vec::with_capacity(cells.len());
        for (i, c) in cells.iter_shared().enumerate() {
            let w = terrain.call("sample_to_world", &[c.to_variant()]).to::<Vector3>();
            points.push(inv * w);
            ids.push(i as i32);
            list.push(c);
        }
        self.cells.insert(terrain.instance_id().to_i64(), list);
        let mat = self.base().get_material("handles");
        g.add_handles(&points, mat.map(|m| m.upcast::<godot::classes::Material>()).as_ref(), &ids);
    }

    fn get_handle_name(&self, gizmo: Option<Gd<EditorNode3DGizmo>>, handle_id: i32, _secondary: bool) -> GString {
        let Some(t) = terrain_of(&gizmo) else { return GString::new() };
        match self.cell(&t, handle_id) {
            Some(c) => GString::from(format!("Sample ({}, {})", c.x, c.y).as_str()),
            None => GString::from(handle_id.to_string().as_str()),
        }
    }

    fn get_handle_value(&self, gizmo: Option<Gd<EditorNode3DGizmo>>, handle_id: i32, _secondary: bool) -> Variant {
        let Some(mut t) = terrain_of(&gizmo) else { return Variant::nil() };
        match self.cell(&t, handle_id) {
            Some(c) => t.call("get_sample_height", &[c.to_variant()]),
            None => Variant::nil(),
        }
    }

    fn set_handle(&mut self, gizmo: Option<Gd<EditorNode3DGizmo>>, handle_id: i32, _secondary: bool, camera: Option<Gd<Camera3D>>, screen_pos: Vector2) {
        let (Some(mut t), Some(cam)) = (terrain_of(&gizmo), camera) else { return };
        let Some(cell) = self.cell(&t, handle_id) else { return };
        if self.drag_before.is_none() {
            let chunk = t.get("handle_chunk").to::<Vector2i>();
            self.drag_before = Some(Self::snapshot(&mut t, chunk));
        }
        drag_height(&mut t, cell, &cam, screen_pos);
    }

    fn commit_handle(&mut self, gizmo: Option<Gd<EditorNode3DGizmo>>, _handle_id: i32, _secondary: bool, _restore: Variant, cancel: bool) {
        let Some(mut t) = terrain_of(&gizmo) else { return };
        let Some(before) = self.drag_before.take() else { return };
        commit_drag(&mut t, before, cancel);
    }
}

/// Moves `cell`'s sample to where the mouse ray meets the vertical plane through the handle
/// that faces the camera (movement along Y only; the original's `_set_handle`).
pub(super) fn drag_height(t: &mut Gd<Node3D>, cell: Vector2i, cam: &Gd<Camera3D>, screen_pos: Vector2) {
    {
        let handle = t.call("sample_to_world", &[cell.to_variant()]).to::<Vector3>();
        let origin = cam.project_ray_origin(screen_pos);
        let dir = cam.project_ray_normal(screen_pos);
        // Vertical plane through the handle facing the camera: movement only along Y.
        let mut n = Vector3::new(dir.x, 0.0, dir.z);
        if n.length() < 1e-4 {
            let b = cam.get_global_transform().basis;
            n = Vector3::new(b.col_c().x, 0.0, b.col_c().z);
        }
        if n.length() < 1e-4 {
            return;
        }
        let n = n.normalized();
        let denom = n.dot(dir);
        if denom.abs() < 1e-6 {
            return;
        }
        let s = n.dot(handle - origin) / denom;
        let hit = origin + dir * s;
        let local = t.get_global_transform().affine_inverse() * hit;
        t.call("set_sample_height", &[cell.to_variant(), local.y.to_variant()]);
        t.call_deferred("update_gizmos", &[]);
    }
}

/// Ends a handle drag: restores `before` on cancel, else one undo step "move terrain point".
pub(super) fn commit_drag(t: &mut Gd<Node3D>, before: VarDictionary, cancel: bool) {
    {
        if cancel {
            t.call("set_chunks_data", &[before.to_variant()]);
            t.call_deferred("update_gizmos", &[]);
            return;
        }
        let mut after = VarDictionary::new();
        for (k, _) in before.iter_shared() {
            let blob = t.call("get_chunk_data", &[k.clone()]);
            after.set(&k, &blob);
        }
        let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else { return };
        let obj = t.clone().upcast::<Object>();
        ur.create_action("move terrain point");
        ur.add_do_method(&obj, "set_chunks_data", &[after.to_variant()]);
        ur.add_do_method(&obj, "update_gizmos", &[]);
        ur.add_undo_method(&obj, "set_chunks_data", &[before.to_variant()]);
        ur.add_undo_method(&obj, "update_gizmos", &[]);
        // Executing writes the drag into the data resource (the drag itself only edited samples).
        ur.commit_action();
    }
}

/// Handle under `pos` (viewport) of the terrain's `handle_chunk`, if `chunk_handles` is on.
pub(super) fn handle_at(t: &Gd<Node3D>, cam: &Gd<Camera3D>, pos: Vector2, radius: f32) -> Option<Vector2i> {
    if !t.get("chunk_handles").try_to::<bool>().unwrap_or(false) {
        return None;
    }
    let mut t = t.clone();
    let chunk = t.get("handle_chunk");
    let cells = t.call("get_chunk_cells", &[chunk]).try_to::<Array<Vector2i>>().ok()?;
    let mut best: Option<(f32, Vector2i)> = None;
    for c in cells.iter_shared() {
        let w = t.call("sample_to_world", &[c.to_variant()]).try_to::<Vector3>().unwrap_or_default();
        if cam.is_position_behind(w) {
            continue;
        }
        let d = cam.unproject_position(w).distance_to(pos);
        if d <= radius && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

thread_local! {
    static GIZMO: RefCell<Option<Gd<MarchingSquaresChunkGizmoPlugin>>> = const { RefCell::new(None) };
}

/// Called by the terrain editor plugin's `enter_tree` / `exit_tree`.
pub fn register(plugin: &mut Gd<EditorPlugin>) {
    let g = MarchingSquaresChunkGizmoPlugin::new_gd();
    plugin.add_node_3d_gizmo_plugin(&g);
    GIZMO.with(|s| *s.borrow_mut() = Some(g));
}

pub fn unregister(plugin: &mut Gd<EditorPlugin>) {
    if let Some(g) = GIZMO.with(|s| s.borrow_mut().take()) {
        plugin.remove_node_3d_gizmo_plugin(&g);
    }
}
