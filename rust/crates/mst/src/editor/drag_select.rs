use godot::classes::{Camera3D, Input, InputEventMouseButton};
use godot::global::Key;
use godot::prelude::*;
use pt_core::math::{v2, v3};
use pt_core::selection;
use super::plugin::MarchingSquaresTerrainPlugin;
use super::tools::Tool;
impl MarchingSquaresTerrainPlugin {
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
    pub(super) fn drag_motion(&mut self, mouse: Vector2) {
        let Some(st) = self.sweep.as_ref() else {
            return;
        };
        let Some(t) = self.terrain.as_ref() else {
            return;
        };
        let start = st.start_local;
        let scale = t.get_global_transform().basis.get_scale().y.abs().max(1e-4);
        let (edges, ctrl) = (
            t.bind().store().ridges,
            Input::singleton().is_key_pressed(Key::CTRL),
        );
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
    pub(super) fn hover_selected(&self) -> bool {
        let (Some(h), Some(t)) = (self.hover, self.terrain.as_ref()) else {
            return false;
        };
        let s = t.bind().store().config.world_to_sample(v2(h.x, h.z));
        self.selection.iter().any(|(c, _)| *c == s)
    }
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
