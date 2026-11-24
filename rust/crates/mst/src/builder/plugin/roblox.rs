use super::super::roblox_ui::{RibbonCmd, RibbonMenu, RibbonState, ribbon};
use super::roblox_rules::{self as rules};
use super::*;
impl PixelBuilderPlugin {
    pub(super) fn is_roblox(&self) -> bool {
        matches!(self.ui, Some(LayoutUi::Roblox(_)))
    }
    pub(super) fn roblox_requirement(&self, a: Action) -> Option<&'static str> {
        self.handles_requirement(a)
            .or_else(|| self.solid_requirement(a))
            .or_else(|| self.parts_requirement(a))
            .or_else(|| self.panels_requirement(a))
    }
    pub(super) fn ribbon_reason(&self, a: Action) -> Option<&'static str> {
        rules::ribbon_reason(a, self.roblox_requirement(a), || self.requirement(a as i64))
    }
    pub(super) fn sync_roblox(&mut self, v: &Values, st: &SyncState) {
        if !self.is_roblox() {
            return;
        }
        let buttons = ribbon::ribbon_actions()
            .into_iter()
            .map(|a| {
                let pressed = rules::first_pressed([
                    self.handles_pressed(a),
                    self.parts_pressed(a),
                    self.panels_pressed(a),
                    ui::action_pressed(a, v, st),
                ]);
                (a, self.ribbon_reason(a), pressed)
            })
            .collect();
        let rs = RibbonState {
            buttons,
            grid_step: self.grid_step,
            rot_snap_on: self.prefs.roblox_rot_snap_on,
            rot_snap_deg: self.prefs.roblox_rot_snap_deg,
            surface_draw: self.surface_draw,
            snap_vertex: self.snap_vertex,
            snap_cell: self.snap_cell,
            collisions: self.prefs.collisions,
            grid_plane: rules::grid_plane_switch(
                self.target.is_some(),
                self.tool,
                self.xform_mode,
                self.mode,
                !self.rbx.grid_plane_hidden,
            ),
            compact: self.prefs.roblox_compact,
        };
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.sync(&rs);
        }
    }
    pub(super) fn apply_roblox_rot_snap(&mut self) {
        self.rot_snap = rules::rot_snap_deg(
            self.is_roblox(),
            self.prefs.roblox_rot_snap_on,
            self.prefs.roblox_rot_snap_deg,
            self.prefs.rot_snap,
        );
    }
    pub(super) fn leave_roblox(&mut self) {
        (self.xform_mode, self.tool) = rules::after_leaving(self.xform_mode, self.tool);
        self.rot_snap = self.prefs.rot_snap;
        self.leave_roblox_move_step();
        self.set_click_modes(rules::ClickModes::on_leaving_roblox());
    }
    fn click_modes(&self) -> rules::ClickModes {
        rules::ClickModes {
            lock: self.rbx.click_lock,
            anchor: self.parts.click_anchor,
            paint: self.panels.click_paint,
        }
    }
    fn set_click_modes(&mut self, m: rules::ClickModes) {
        self.rbx.click_lock = m.lock;
        self.parts.click_anchor = m.anchor;
        self.panels.click_paint = m.paint;
    }
    pub(super) fn toggle_click_mode(&mut self, which: rules::ClickMode) {
        let next = self.click_modes().toggled(which);
        self.set_click_modes(next);
    }
    pub(super) fn toggle_handle_space(&mut self) {
        let (orient, message) = rules::next_handle_space(self.opts.gizmo_orient);
        self.opts.gizmo_orient = orient;
        self.message = message.into();
    }
    pub(super) fn toggle_roblox_rot_snap(&mut self) {
        self.prefs.roblox_rot_snap_on = !self.prefs.roblox_rot_snap_on;
        self.prefs.save_key(prefs::Key::RobloxRotSnapOn);
        self.apply_roblox_rot_snap();
    }
    fn run_ribbon_cmd(&mut self, c: RibbonCmd) {
        match c {
            RibbonCmd::Run(a) => self.run_action(a),
            RibbonCmd::Surface(on) => {
                if self.surface_draw != on {
                    self.run_action(Action::SurfaceDraw);
                }
            }
            RibbonCmd::Part(k) => {
                self.insert_part(k);
                self.sync_ui();
                self.redraw();
            }
        }
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_ribbon_tab(&mut self, edit: bool) {
        if let Some(LayoutUi::Roblox(r)) = &mut self.ui {
            r.ribbon.show_tab(edit);
        }
        self.sync_ui();
    }
    #[func]
    fn on_ribbon_menu(&mut self, id: i64, menu: i64) {
        if let Some(c) = RibbonMenu::from_i64(menu).and_then(|m| ribbon::command(m, id)) {
            self.run_ribbon_cmd(c);
        }
    }
    #[func]
    fn on_ribbon_rot_step(&mut self, deg: f64) {
        self.prefs.assign(prefs::Key::RobloxRotSnap, &deg.to_variant());
        self.prefs.save_key(prefs::Key::RobloxRotSnap);
        self.apply_roblox_rot_snap();
        self.sync_ui();
    }
}
