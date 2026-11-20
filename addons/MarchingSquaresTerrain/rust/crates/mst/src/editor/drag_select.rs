//! Brush tool, "Drag" mode: the godot-pixelizer-3d MarchingSquaresTerrain brush UX.
//!
//! * LMB(+drag) outside the selection starts a new selection (white cells,
//!   `cursor::draw_selection`); Shift+LMB adds to it, Ctrl+LMB removes cells from it.
//! * LMB-drag starting on a selected cell moves every selected cell together: the height follows
//!   the mouse on a camera-facing vertical plane through the clicked point (`pt_core::selection`),
//!   live, one undo action per drag. Far / steep cameras are capped to one level per
//!   `selection::DRAG_PX_PER_STEP` pixels of mouse travel (`selection::drag_offset`). Stepped edge styles (Terraces, HexPrism) snap to their
//!   terrace step; hold Ctrl while dragging for free heights.
//! * The selection stays after a drag (drag it again to adjust). Alt+LMB-drag moves only the
//!   hovered cell. RMB / Alt / Esc clear the selection (`plugin.rs`).
//!
//! Differs from pixelizer-3d, which clears the selection on release unless Shift is held and
//! moves the selection wherever the drag starts: a second drag there silently started a new
//! selection, which read as "dragging does nothing".

use godot::classes::{Camera3D, Input, InputEventMouseButton};
use godot::global::Key;
use godot::prelude::*;

use pt_core::math::{v2, v3};
use pt_core::selection;

use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::Tool;

impl MarchingSquaresTerrainPlugin {
    /// LMB press with the Brush tool in Drag mode. Always consumed.
    pub(super) fn drag_press(
        &mut self,
        lens: &Gd<Camera3D>,
        mb: &Gd<InputEventMouseButton>,
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) -> bool {
        let Some(h) = self.hover else { return true };
        if ctrl || shift || (!alt && !self.hover_selected()) {
            // Paint (Shift adds; a plain press off the selection starts a new one) or erase
            // (Ctrl) the selection; the stroke (tool = Debug) keeps doing so while dragging
            // (`stroke_motion`).
            if !ctrl && !shift {
                self.selection.clear();
            }
            let fp = self.footprint(h);
            if ctrl {
                selection::deduct(&mut self.selection, &fp);
            } else {
                selection::merge(&mut self.selection, &fp);
            }
            let mut st = self.new_stroke(lens, mb, Tool::Debug, h, true, false);
            st.deselect = ctrl;
            self.sweep = Some(st);
            return true;
        }
        if alt {
            let s = self
                .terrain
                .as_ref()
                .map(|t| t.bind().store().config.world_to_sample(v2(h.x, h.z)));
            self.selection = s.into_iter().map(|s| (s, 1.0)).collect();
        }
        self.sweep = Some(self.new_stroke(lens, mb, Tool::Brush, h, true, false));
        self.stroke_begin();
        true
    }

    /// Mouse motion during a selection height drag: moves the selection (live preview).
    pub(super) fn drag_motion(&mut self, mouse: Vector2) {
        let Some(st) = self.sweep.as_ref() else {
            return;
        };
        let Some(t) = self.terrain.as_ref() else {
            return;
        };
        let start = st.start_local;
        // `world_per_pixel` is in world units; the drag works in terrain-local heights.
        let scale = t.get_global_transform().basis.get_scale().y.abs().max(1e-4);
        let (edges, ctrl) = (
            t.bind().store().ridges,
            Input::singleton().is_key_pressed(Key::CTRL),
        );
        // Speed limit from the level height even when free (Ctrl / unstepped): no speed jump.
        let max_rate = selection::drag_rate_limit(
            selection::stature_stride(&edges).unwrap_or(edges.terrace_step),
        );
        let raw = match self.mouse_ray {
            Some((o, d)) => selection::drag_offset(
                v3(o.x, o.y, o.z),
                v3(d.x, d.y, d.z),
                v3(start.x, start.y, start.z),
                st.start_mouse_y - mouse.y,
                st.world_per_pixel / scale,
                max_rate,
            ),
            None => 0.0,
        };
        let flatten = self.state.flatten.then_some(start.y);
        let step = if ctrl {
            None
        } else {
            selection::stature_stride(&edges)
        };
        let offset = selection::snap_offset(raw, flatten, step);
        if st.drag_offset == Some(offset) {
            return;
        }
        let (pattern, base) = (st.pattern.clone(), st.base_heights.clone());
        if let Some(st) = &mut self.sweep {
            st.drag_offset = Some(offset);
        }
        self.apply_footprint(&pattern, |store, fp| {
            pt_core::brush::op_offset(store, fp, &base, offset, flatten)
        });
    }

    /// True when the cell under the cursor is selected (a press there drags the selection).
    pub(super) fn hover_selected(&self) -> bool {
        let (Some(h), Some(t)) = (self.hover, self.terrain.as_ref()) else {
            return false;
        };
        let s = t.bind().store().config.world_to_sample(v2(h.x, h.z));
        self.selection.iter().any(|(c, _)| *c == s)
    }

    /// Status line hint for the Brush tool in Drag mode.
    pub(super) fn drag_status(&self) -> Option<String> {
        if self.state.tool != Tool::Brush {
            return None;
        }
        if let Some(o) = self
            .sweep
            .as_ref()
            .filter(|s| s.tool == Tool::Brush)
            .and_then(|s| s.drag_offset)
        {
            return Some(format!("  |  {:+.2}  (Ctrl: free height)", o));
        }
        Some(if self.selection.is_empty() {
            "  |  LMB: select cells".to_string()
        } else if self.hover_selected() {
            "  |  LMB-drag: move selection  Shift: add  Ctrl: remove  RMB/Esc: clear".to_string()
        } else {
            "  |  LMB: new selection (drag a white cell to move)  Shift: add  RMB/Esc: clear"
                .to_string()
        })
    }
}
