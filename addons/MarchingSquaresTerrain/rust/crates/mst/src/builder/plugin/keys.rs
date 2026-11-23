use super::key_rules::{KeyCmd, KeyContext, key_command};
use super::*;
pub(super) fn is_undo_redo_key(k: &Gd<InputEventKey>) -> bool {
    k.is_pressed()
        && !k.is_echo()
        && (k.is_ctrl_pressed() || k.is_meta_pressed())
        && !k.is_alt_pressed()
        && matches!(k.get_keycode(), Key::Z | Key::Y)
}
fn physical_digit(key: &Gd<InputEventKey>) -> Gd<InputEventKey> {
    let (code, physical) = (key.get_keycode().ord(), key.get_physical_keycode());
    let keypad = Key::KP_0.ord()..=Key::KP_9.ord();
    let digits = Key::KEY_0.ord()..=Key::KEY_9.ord();
    if !keypad.contains(&code) || !digits.contains(&physical.ord()) {
        return key.clone();
    }
    let mut typed = InputEventKey::new_gd();
    typed.set_keycode(physical);
    typed.set_physical_keycode(physical);
    typed.set_pressed(key.is_pressed());
    typed.set_echo(key.is_echo());
    typed.set_shift_pressed(key.is_shift_pressed());
    typed.set_ctrl_pressed(key.is_ctrl_pressed());
    typed.set_alt_pressed(key.is_alt_pressed());
    typed.set_meta_pressed(key.is_meta_pressed());
    typed
}
impl PixelBuilderPlugin {
    pub(super) fn consume_key(&self, camera: &Gd<Camera3D>) {
        let host = camera
            .get_viewport()
            .and_then(|v| v.get_parent())
            .and_then(|p| p.get_viewport())
            .or_else(|| self.base().get_viewport());
        if let Some(mut vp) = host {
            vp.set_input_as_handled();
        }
    }
    pub(super) fn on_key(&mut self, key: &Gd<InputEventKey>) -> bool {
        if !key.is_pressed() || key.is_echo() {
            return false;
        }
        let k = key.get_keycode();
        let ctrl = key.is_ctrl_pressed() || key.is_meta_pressed();
        if self.adjust.is_some() {
            match k {
                Key::ENTER | Key::KP_ENTER if !ctrl => {
                    self.adjust_commit();
                    self.sync_ui();
                    self.redraw();
                    return true;
                }
                Key::ESCAPE => {
                    self.adjust_cancel();
                    return true;
                }
                Key::Z if ctrl => {
                    self.adjust_commit();
                    return false;
                }
                _ => {}
            }
        }
        let typed = physical_digit(key);
        let roblox = self.is_roblox();
        for i in 0..super::super::shortcuts::ENTRIES.len() {
            if !super::super::shortcuts::active_in(super::super::shortcuts::ENTRIES[i].id, roblox)
                || !super::super::shortcuts::fits(&self.shortcut_table, i, &typed)
            {
                continue;
            }
            if let Some(cmd) = self.shortcut_command(super::super::shortcuts::ENTRIES[i].id) {
                self.run_key_command(cmd);
                return true;
            }
        }
        match k {
            Key::ESCAPE if !ctrl && !key.is_shift_pressed() && !key.is_alt_pressed() => {
                let ctx = super::key_rules::EscContext {
                    shaping: self.shape_drag.is_some(),
                    painting_stroke: self.vp.sweep.is_some(),
                    dragging: self.drag.is_some() || self.rbx.hold.is_some(),
                    tool: self.tool,
                    mode: self.mode,
                    anything_picked: !(self.sel_faces.is_empty()
                        && self.sel_edges.is_empty()
                        && self.sel_verts.is_empty()),
                };
                use super::key_rules::EscStep;
                match super::key_rules::escape_step(ctx) {
                    Some(EscStep::CancelShape) => self.cancel_shape(),
                    Some(EscStep::CancelStroke) => {
                        self.cancel_vpaint_stroke();
                    }
                    Some(EscStep::CancelDrag) => self.cancel_drag(),
                    Some(EscStep::BackToSelectTool) => self.assign_instrument(Tool::Select),
                    Some(EscStep::ClearPicked) => self.purge_picked(),
                    Some(EscStep::BackToObjectMode) => self.assign_setting(Mode::Object),
                    None => return false,
                }
                self.redraw();
                true
            }
            _ => false,
        }
    }
    pub(super) fn shortcut_command(&self, id: &str) -> Option<KeyCmd> {
        key_command(
            id,
            KeyContext {
                has_target: self.target.is_some(),
                mode: self.mode,
                tool: self.tool,
                edges_selected: !self.sel_edges.is_empty(),
                faces_selected: !self.sel_faces.is_empty(),
            },
        )
    }
    pub(super) fn run_key_command(&mut self, cmd: KeyCmd) {
        match cmd {
            KeyCmd::Run(a) => self.run_action(a),
            KeyCmd::CycleMode => self.cycle_mode(),
            KeyCmd::Grid(f) => {
                self.grid_step = (self.grid_step * f).clamp(0.0624, 64.0);
                self.sync_ui();
            }
        }
    }
}
