use super::*;
impl PixelBuilderPlugin {
    pub(super) fn open_preferences(&mut self) {
        if self
            .prefs_panel
            .as_ref()
            .is_none_or(|p| !p.popup.is_instance_valid())
        {
            let gd = self.to_gd().upcast::<Object>();
            let panel = prefs::build_panel(&gd, &self.prefs);
            if let Some(mut base) = EditorInterface::singleton().get_base_control() {
                base.add_child(&panel.popup);
            }
            self.prefs_panel = Some(panel);
        }
        let p = self.prefs;
        if let Some(panel) = &mut self.prefs_panel {
            prefs::sync_panel(panel, &p);
            panel.popup.popup_centered();
        }
    }
    pub(super) fn apply_prefs(&mut self, startup: bool) {
        prefs::set_handle_scale(self.prefs.handle_scale);
        if startup {
            self.grid_step = self.prefs.grid_step;
            self.rot_snap = self.prefs.rot_snap;
            self.snap_grid = self.prefs.snap_grid;
        }
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    pub(super) fn on_pref(&mut self, value: Variant, key: i64) {
        let Some(k) = prefs::Key::from_i64(key) else {
            return;
        };
        let before = self.prefs.worth(k);
        self.prefs.assign(k, &value);
        if self.prefs.worth(k) != before {
            self.prefs.save_key(k);
        }
        match k {
            prefs::Key::GridStep => self.grid_step = self.prefs.grid_step,
            prefs::Key::RotSnap => self.rot_snap = self.prefs.rot_snap,
            prefs::Key::SnapGrid => self.snap_grid = self.prefs.snap_grid,
            prefs::Key::Hover if !self.prefs.hover => {
                self.hover_face = None;
                self.hover_edge = None;
                self.hover_vert = None;
            }
            prefs::Key::AdjustPanel if !self.prefs.adjust_panel => self.adjust_commit(),
            _ => {}
        }
        self.apply_prefs(false);
        self.sync_ui();
        self.redraw();
    }
    #[func]
    pub(super) fn on_prefs_reset(&mut self) {
        self.prefs = Prefs::default();
        self.prefs.save();
        self.apply_prefs(true);
        let p = self.prefs;
        if let Some(panel) = &mut self.prefs_panel {
            prefs::sync_panel(panel, &p);
        }
        self.sync_ui();
        self.redraw();
    }
    #[func]
    pub(super) fn on_editor_settings_changed(&mut self) {
        self.shortcut_table = super::super::shortcuts::reingest();
        if let Some(u) = &mut self.ui {
            u.refresh_tooltips();
        }
        self.refresh_menu_keys();
        let changed = EditorInterface::singleton()
            .get_editor_settings()
            .is_some_and(|es| es.check_changed_settings_in_group(super::super::layout::GROUP));
        if changed
            || self
                .ui
                .as_ref()
                .is_some_and(|u| u.layout() != super::super::layout::Layout::ingest())
        {
            self.apply_layout();
        }
    }
}
