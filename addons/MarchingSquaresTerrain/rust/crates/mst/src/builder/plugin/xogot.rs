//! The UI-layout switch and the Xogot layout's entry points (docs/port/07-xogot-ui.md §8):
//! building / tearing down the active layout, `apply_layout` (deferred, idempotent), the Xogot
//! widgets' `#[func]` callbacks (they all end in the builder's shared commands) and the
//! non-persisting test hooks `debug_set_layout` / `debug_layout`.
//!
//! Re-entrancy: every Xogot control is connected to these `#[func]`s; syncs use `*_no_signal`
//! setters; popups are never connected back to the plugin; a layout switch requested from a
//! control runs deferred and frees the old widgets with `queue_free`.

use godot::classes::{EditorInterface, Input, InputEvent, InputEventKey, InputEventMouseButton};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use super::super::commands;
use super::super::forms::MeshProp;
use super::super::layout::{self, Layout};
use super::super::ui::{self, Action, Field, Mode, SyncState, Tool, Values};
use super::super::xogot_ui::{self, LayoutUi, hud, options, paint, toolbar};
use super::PixelBuilderPlugin;
use pt_core::build::ShapeKind;

impl PixelBuilderPlugin {
    pub(super) fn is_xogot(&self) -> bool {
        matches!(self.ui, Some(LayoutUi::Xogot(_)))
    }

    /// The `SyncState` fields the Xogot layout reads (the rest is filled by `sync_ui`).
    pub(super) fn xogot_state(&self) -> SyncState {
        let has_target = self.target.is_some();
        let face_slot = self.data().and_then(|d| self.sel_faces.first().and_then(|f| d.bind().mesh().faces.get(*f).map(|x| x.material as i64)));
        let t = self.target.as_ref();
        SyncState {
            visible: self.visible,
            can_create: has_target || self.host.is_some(),
            sel_faces: if self.mode == Mode::Face { self.sel_faces.len() } else { 0 },
            message: self.message.clone(),
            tool_detail: self.tool_lines().join("\n"),
            prefs: self.prefs,
            adjusting: self.adjust.is_some(),
            live_uv2: t.is_some_and(|t| t.get("live_uv2").try_to::<bool>().unwrap_or(false)),
            clear_grass: t.is_some_and(|t| t.get("clear_grass").try_to::<bool>().unwrap_or(false)),
            face_slot: if self.mode == Mode::Face { face_slot } else { None },
            ..SyncState::default()
        }
    }

    fn hud_content(&self, st: &SyncState) -> Option<hud::Content> {
        hud::content(st, self.spec.kind, self.bz.values().block)
    }

    /// Syncs the Xogot widgets (`sync_ui`).
    pub(super) fn sync_xogot(&mut self, v: &Values, st: &SyncState) {
        let mats: Vec<Option<Gd<godot::classes::Material>>> = self.data().map(|d| d.bind().material_slots().iter_shared().collect()).unwrap_or_default();
        let groups = self.smoothing_groups();
        let content = self.hud_content(st);
        self.hud_key = hud_key(&content);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.sync(v, st, &mats, &groups, content);
        }
    }

    /// Builds layout `l` and adds it to the editor (Builder menu first, so it stays before the
    /// Xogot toolbar in the 3D menu bar).
    pub(super) fn build_layout(&mut self, l: Layout) {
        self.sync_menus(self.visible);
        let gd = self.to_gd().upcast::<Object>();
        let spec = self.spec;
        let v = self.values_with(&spec);
        let st = SyncState { visible: self.visible, ..self.xogot_state() };
        let mut u = LayoutUi::build(l, &gd, &v, &st);
        for (c, ctl) in u.controls() {
            self.base_mut().add_control_to_container(c, &ctl);
        }
        u.set_visible(self.visible, self.target.is_some());
        if let LayoutUi::Xogot(x) = &mut u
            && let Some(cam) = self.camera.clone().filter(|c| c.is_instance_valid())
            && let Some(i) = hud::viewport_of(&cam)
        {
            x.hud.set_viewport(i);
        }
        self.ui = Some(u);
        self.palette_slots = None;
        self.refresh_menu_layout();
    }

    /// Removes the active layout's widgets (`now`: free at once, from `exit_tree`).
    pub(super) fn teardown_layout(&mut self, now: bool) {
        let Some(u) = self.ui.take() else { return };
        for (c, ctl) in u.controls() {
            if ctl.is_instance_valid() {
                self.base_mut().remove_control_from_container(c, &ctl);
            }
        }
        u.free(now);
    }

    /// Writes the setting and swaps after the current signal (`call_deferred`).
    pub(super) fn switch_layout(&mut self, l: Layout) {
        l.store();
        self.to_gd().call_deferred("apply_layout", &[]);
    }

    /// The Builder menu's *Xogot layout* check item.
    pub(super) fn refresh_menu_layout(&mut self) {
        let x = self.is_xogot();
        if let Some(m) = &self.builder_menu
            && m.is_instance_valid()
            && let Some(mut p) = m.get_popup()
            && let Some(i) = (0..p.get_item_count()).find(|i| p.get_item_id(*i) == 11)
        {
            p.set_item_checked(i, x);
        }
    }

    pub(super) fn open_xg_options(&mut self, category: Option<usize>) {
        if !self.is_xogot() {
            // MST layout: the Builder menu's *Modeler Options…* opens the preferences.
            self.open_preferences();
            return;
        }
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.toolbar.handles_popup.hide();
            x.options.open(&gd, &v, &st, category);
        }
    }

    /// The complete `SyncState` (as `sync_ui` builds it).
    fn full_state(&self) -> SyncState {
        let has_target = self.target.is_some();
        SyncState {
            mode: self.mode,
            tool: self.tool,
            xform: self.xform_mode,
            snap_grid: self.snap_grid,
            snap_cell: self.snap_cell,
            snap_vertex: self.snap_vertex,
            soft: self.soft,
            uv_lock: self.uv_lock,
            reshape: (self.reshape || (self.bz.editing.is_some() && self.tool == Tool::Bezier)) && has_target,
            has_target,
            has_spec: self.data().is_some_and(|d| d.bind().shape_spec().is_some() || d.bind().bezier_recipe().is_some()),
            can_convert: self.convert_src.is_some(),
            insert_vert: self.insert_vert,
            surface_draw: self.surface_draw,
            xray: self.xray,
            entity: self.target.as_ref().map(|t| t.get("entity_type").try_to::<i32>().unwrap_or(0)).unwrap_or(0),
            bz_knot: self.bz.selected.is_some(),
            ..self.xogot_state()
        }
    }

    /// *Generate Lightmap UVs*: Live UV2 on at the preferences' texel (one undo step), after
    /// Xogot's atlas size check.
    pub(super) fn act_generate_uv2(&mut self) {
        let Some(t) = self.target.clone() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let texel = self.prefs.lightmap_texel.max(0.01);
        if let Some(d) = t.bind().data()
            && let Err(e) = pt_core::build::lightmap::check_atlas(d.bind().mesh(), super::super::data::to_p(t.get_global_transform().basis.get_scale()), texel)
        {
            self.message = format!("Generate lightmap UVs: {e} (Preferences › Lightmap texel)");
            return;
        }
        let on = t.get("live_uv2").try_to::<bool>().unwrap_or(false);
        let cur = t.get("lightmap_texel").try_to::<f32>().unwrap_or(-1.0);
        if on && (cur - texel).abs() < 1e-6 {
            // Already on at this texel: no empty undo step.
            self.message = format!("Lightmap UVs: already generated at texel {texel} (Live UV2 on)");
            return;
        }
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action("Pixel Builder: Generate lightmap UVs");
        ur.add_do_property(&t, "lightmap_texel", &texel.to_variant());
        ur.add_do_property(&t, "live_uv2", &true.to_variant());
        ur.add_undo_property(&t, "lightmap_texel", &t.get("lightmap_texel"));
        ur.add_undo_property(&t, "live_uv2", &t.get("live_uv2"));
        // The property change refreshes the inspector, which may call back into editor plugins.
        ur.call_deferred("commit_action", &[]);
        // The commit runs after this call: re-sync then (Options › Live UV2, inspector rows).
        self.to_gd().call_deferred("sync_ui_deferred", &[]);
        self.message = format!("Lightmap UVs: unwrapping at texel {texel} (Live UV2 on)");
    }

    /// HUD *Create* / *Done*: the tool's Enter path (knife, Bezier, wall / poly); a plain shape is
    /// placed at once (like Alt-click); the click tools (paint, vertex paint, place) end.
    fn hud_done(&mut self) {
        match self.tool {
            Tool::Knife | Tool::Bezier => self.synth_key(Key::ENTER),
            Tool::Shape if self.spec.kind.is_polyline() || self.poly_outline.is_some() || !self.wall_pts.is_empty() => self.synth_key(Key::ENTER),
            Tool::Shape => {
                if self.hover_place.is_none()
                    && let Some(cam) = self.view_camera()
                {
                    let size = cam.get_viewport().map(|v| v.get_visible_rect().size).unwrap_or(Vector2::new(800.0, 600.0));
                    self.hover_place = self.placement(&cam, size * 0.5);
                }
                self.stamp_shape(false);
            }
            _ => self.set_tool(Tool::Select),
        }
    }

    /// Xogot layout: a scene-graph action (Detach / Duplicate to a new object, Mirror, Merge
    /// objects) opens its confirmation instead of running; true when it did (or, with
    /// `debug_auto_confirm`, ran it confirmed). The MST layout runs them directly (adjust panel).
    /// An action that cannot run now is not asked about: it runs and reports why.
    pub(super) fn ask_confirmation(&mut self, a: Action) -> bool {
        if !self.is_xogot() || self.requirement(a as i64).is_some() {
            return false;
        }
        let Some((title, fields)) = commands::confirmation(a, self.mode, self.xg.to_new_object) else { return false };
        self.last_confirmation = title.to_string();
        if self.auto_confirm {
            self.run_confirmed(a);
            return true;
        }
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            let c = x.confirm.get_or_insert_with(|| xogot_ui::Confirm::build(&gd));
            c.open(&gd, a, title, &fields, &v, &st);
        }
        true
    }

    /// Runs `a` past its confirmation; its options were set in the dialog, so no adjust panel.
    fn run_confirmed(&mut self, a: Action) {
        let old = (self.no_adjust, self.no_confirm);
        self.no_adjust = true;
        self.no_confirm = true;
        self.run_action(a);
        (self.no_adjust, self.no_confirm) = old;
    }

    /// The camera of the last viewport input, else viewport 0's.
    fn view_camera(&self) -> Option<Gd<godot::classes::Camera3D>> {
        self.camera.clone().filter(|c| c.is_instance_valid()).or_else(|| EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done().and_then(|v| v.get_camera_3d()))
    }

    /// Feeds a synthetic key through the viewport dispatch (HUD Done / Cancel / Remove Point =
    /// the tool's Enter / Esc / Backspace paths).
    fn synth_key(&mut self, k: Key) {
        let Some(cam) = self.view_camera() else { return };
        let mut ev = InputEventKey::new_gd();
        ev.set_keycode(k);
        ev.set_pressed(true);
        use godot::classes::IEditorPlugin;
        self.forward_3d_gui_input(Some(cam), Some(ev.upcast()));
    }
}

/// The HUD key (content + text) the overlay compares.
fn hud_key(c: &Option<hud::Content>) -> String {
    c.as_ref().map(|c| format!("{}|{}|{}", c.title, c.detail, c.tool)).unwrap_or_default()
}

#[godot_api(secondary)]
impl PixelBuilderPlugin {
    /// Swaps to the layout of the setting (or test override). Idempotent: the deferred call and
    /// `settings_changed` both arrive.
    #[func]
    pub(super) fn apply_layout(&mut self) {
        let want = Layout::load();
        if self.ui.as_ref().is_some_and(|u| u.layout() == want) {
            self.sync_ui();
            return;
        }
        // End transient interaction; the edit state itself stays.
        self.adjust_commit();
        self.hide_adjust_panel();
        self.cancel_drag();
        self.cancel_shape();
        self.bz.cancel_live();
        self.cancel_vpaint_stroke();
        self.marquee = None;
        let uv_was_open = self.uv_open();
        self.teardown_layout(false);
        self.build_layout(want);
        // The UV panel follows the layout: back into its window for MST (the dock goes), into
        // the "Mesh UV" dock for Xogot when it was showing (else on its next open).
        if want == Layout::Mst {
            self.place_uv(false);
        }
        if uv_was_open {
            self.open_uv_editor();
        }
        self.palette_slots = None;
        self.sync_ui();
        self.sync_menus(self.visible);
        self.redraw();
    }

    /// The preferences row / Options row / Builder menu: 0 MST, 1 Xogot.
    #[func]
    fn on_layout_choice(&mut self, index: i64) {
        self.switch_layout(Layout::from_i64(index));
    }

    /// Test hook: swap the layout for this session only (`n` 0 / 1; -1 back to the setting).
    /// Never writes the Editor Setting.
    #[func]
    fn debug_set_layout(&mut self, n: i64) {
        layout::set_override(n);
        self.apply_layout();
    }

    /// Test hook: the active layout's state.
    #[func]
    fn debug_layout(&mut self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let st = self.full_state();
        d.set("layout", self.ui.as_ref().map(|u| u.layout().name()).unwrap_or(""));
        d.set("setting", Layout::load().name());
        match &self.ui {
            Some(LayoutUi::Mst(u)) => {
                d.set("mst_toolbar", u.toolbar.is_inside_tree());
                d.set("mst_strip", u.strip.is_inside_tree());
            }
            Some(LayoutUi::Xogot(x)) => {
                let tb = &x.toolbar;
                d.set("toolbar", tb.hbox.is_inside_tree() && tb.hbox.is_visible());
                d.set("toolbar_parent", tb.hbox.get_parent().map(|p| p.get_class().to_string()).unwrap_or_default().as_str());
                let mut vis = VarDictionary::new();
                for (name, b) in [("add_mesh", &tb.add_mesh), ("make_editable", &tb.make_editable), ("snap", &tb.snap), ("paint", &tb.paint), ("handles", &tb.handles), ("options", &tb.options)] {
                    vis.set(name, b.is_visible());
                }
                vis.set("actions", tb.actions.is_visible());
                for (m, b) in &tb.modes {
                    vis.set(format!("mode_{}", m.name().to_lowercase()).as_str(), b.is_visible());
                    if b.is_pressed() {
                        d.set("mode_pressed", m.name());
                    }
                }
                d.set("buttons", &vis);
                d.set("snap_pressed", tb.snap.is_pressed());
                d.set("handles_open", tb.handles_popup.is_visible());
                let rows: PackedStringArray = tb.handles_form.labels().iter().map(|s| GString::from(s.as_str())).collect();
                d.set("handles_rows", &rows);
                d.set("palette_open", tb.palette.is_visible());
                d.set("paint_open", x.paint.popup.is_visible());
                d.set("paint_page", x.paint.page as i64);
                d.set("options_open", x.options.dialog.is_visible());
                let cats: PackedStringArray = x.options.shown.iter().map(|i| GString::from(options::CATEGORIES[*i].1)).collect();
                d.set("options_categories", &cats);
                d.set("options_current", options::CATEGORIES[x.options.current].1);
                let rows: PackedStringArray = x.options.form.labels().iter().map(|s| GString::from(s.as_str())).collect();
                d.set("options_rows", &rows);
                d.set("hud_visible", x.hud.panel.is_visible());
                d.set("hud_viewport", x.hud.viewport() as i64);
                let parent_vp = x.hud.panel.get_parent().and_then(|p| (0..4).find(|i| hud::host(*i).is_some_and(|h| h.instance_id() == p.instance_id()))).map(|i| i as i64).unwrap_or(-1);
                d.set("hud_parent_viewport", parent_vp);
                if let Some(c) = x.hud.shown() {
                    d.set("hud_title", c.title.as_str());
                    d.set("hud_detail", c.detail.as_str());
                }
                let bt: PackedStringArray = x.hud.button_texts().iter().map(|s| GString::from(s.as_str())).collect();
                d.set("hud_buttons", &bt);
                d.set("tool_options_open", x.hud.tool_options.is_visible());
                let tr: PackedStringArray = x.hud.tool_form.labels().iter().map(|s| GString::from(s.as_str())).collect();
                d.set("tool_options_rows", &tr);
            }
            None => {}
        }
        d.set("visible", st.visible);
        d.set("uv_host", self.uv_host());
        d.set("uv_open", self.uv_open());
        d
    }

    /// Test hook: the Actions menu of the current mode as `[{section, id, title, enabled, reason}]`
    /// (fills the real popup like opening it does).
    #[func]
    fn debug_xogot_actions(&mut self) -> VarArray {
        self.on_xg_actions_popup();
        let st = self.full_state();
        let mut out = VarArray::new();
        for sec in commands::actions_menu(self.mode, |a, m| ui::action_visible(a, m, &st)) {
            for e in sec.entries {
                let mut d = VarDictionary::new();
                d.set("section", sec.title.unwrap_or(""));
                d.set("id", e.action as i64);
                d.set("title", e.title.as_str());
                let r = self.requirement(e.action as i64);
                d.set("enabled", r.is_none());
                d.set("reason", r.unwrap_or(""));
                out.push(&d.to_variant());
            }
        }
        out
    }

    /// Test hook: opens a Xogot surface (`palette`, `paint`, `handles`, `options`, `tool_options`);
    /// true when it is visible afterwards.
    #[func]
    fn debug_xogot_open(&mut self, which: GString) -> bool {
        self.on_xg_open(which.clone());
        let w = which.to_string();
        let Some(LayoutUi::Xogot(x)) = &self.ui else { return false };
        match w.as_str() {
            "palette" => x.toolbar.palette.is_visible(),
            "paint" => x.paint.popup.is_visible(),
            "handles" => x.toolbar.handles_popup.is_visible(),
            "options" => x.options.dialog.is_visible(),
            "tool_options" => x.hud.tool_options.is_visible(),
            _ => false,
        }
    }

    /// Test hook: what the file dialog does with `path` (1 load palette, 2 save palette, 0 load
    /// a material into the current slot), without the dialog.
    #[func]
    fn xg_file_debug(&mut self, purpose: i64, path: GString) {
        let p = path.to_string();
        match purpose {
            xogot_ui::FILE_PALETTE_LOAD => self.palette_load(&p),
            xogot_ui::FILE_PALETTE_SAVE => self.palette_save(&p),
            _ => {
                let res = godot::classes::ResourceLoader::singleton().load(&p);
                let slot = self.slot;
                self.set_slot_material(res, slot);
            }
        }
        self.sync_ui();
    }

    /// Test hook: closes every Xogot popup.
    #[func]
    fn debug_xogot_close(&mut self) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.toolbar.palette.hide();
            x.toolbar.handles_popup.hide();
            x.paint.popup.hide();
            x.options.dialog.hide();
            x.hud.tool_options.hide();
        }
    }

    /// Test hook: types `query` into the Options search (as `text_changed` would).
    #[func]
    fn debug_xogot_options_search(&mut self, query: GString) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.options.search.set_text(&query);
        }
        self.on_xg_options_search(query);
    }

    /// A Toggle row of a Xogot form: runs the toggle action when its state differs.
    #[func]
    fn on_xg_toggle(&mut self, on: bool, id: i64) {
        let Some(a) = Action::from_i64(id) else { return };
        let spec = self.spec;
        let st = self.full_state();
        let cur = ui::action_pressed(a, &self.values_with(&spec), &st).unwrap_or(!on);
        if cur != on {
            self.run_action(a);
        } else {
            self.sync_ui();
        }
    }

    /// A per-mesh property row (Options › Generated Data).
    #[func]
    fn on_xg_mesh_prop(&mut self, on: bool, which: i64) {
        let Some(t) = self.target.clone() else { return };
        let prop = if which == MeshProp::ClearGrass as i64 { "clear_grass" } else { "live_uv2" };
        let Some(mut ur) = self.base().get_undo_redo() else { return };
        ur.create_action(&format!("Pixel Builder: {prop}"));
        ur.add_do_property(&t, prop, &on.to_variant());
        ur.add_undo_property(&t, prop, &t.get(prop));
        ur.call_deferred("commit_action", &[]);
        self.to_gd().call_deferred("sync_ui_deferred", &[]);
    }

    /// Confirmation OK: runs the pending action.
    #[func]
    fn on_xg_confirmed(&mut self) {
        let a = match &mut self.ui {
            Some(LayoutUi::Xogot(x)) => x.confirm.as_mut().and_then(|c| c.action.take()),
            _ => None,
        };
        if let Some(a) = a {
            self.run_confirmed(a);
        }
    }

    /// Confirmation Cancel / close: drops the pending action.
    #[func]
    fn on_xg_confirm_canceled(&mut self) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui
            && let Some(c) = &mut x.confirm
        {
            c.action = None;
        }
        self.message = String::new();
        self.redraw();
    }

    /// Test hook: confirmations accept at once (`true`) or open the dialog (`false`, default).
    #[func]
    fn debug_auto_confirm(&mut self, on: bool) {
        self.auto_confirm = on;
    }

    /// Test hook: the confirmation state `{open, action, title, text, rows, last}`.
    #[func]
    fn debug_confirmation(&mut self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("last", self.last_confirmation.as_str());
        if let Some(LayoutUi::Xogot(x)) = &self.ui
            && let Some(c) = &x.confirm
        {
            d.set("open", c.dialog.is_visible());
            d.set("action", c.action.map(|a| a as i64).unwrap_or(-1));
            d.set("title", &c.dialog.get_title());
            d.set("text", &c.dialog.get_text());
            let rows: PackedStringArray = c.form.labels().iter().map(|s| GString::from(s.as_str())).collect();
            d.set("rows", &rows);
        } else {
            d.set("open", false);
            d.set("action", -1i64);
        }
        d
    }

    /// Test hook: presses OK (`accept`) or Cancel on an open confirmation (no signal: the
    /// handlers are called directly).
    #[func]
    fn debug_confirm(&mut self, accept: bool) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui
            && let Some(c) = &mut x.confirm
        {
            c.dialog.hide();
        }
        if accept {
            self.on_xg_confirmed();
        } else {
            self.on_xg_confirm_canceled();
        }
    }

    #[func]
    fn sync_ui_deferred(&mut self) {
        self.sync_ui();
    }

    /// A toolbar / HUD button that opens a surface.
    #[func]
    fn on_xg_open(&mut self, which: GString) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        let mats: Vec<Option<Gd<godot::classes::Material>>> = self.data().map(|d| d.bind().material_slots().iter_shared().collect()).unwrap_or_default();
        let groups = self.smoothing_groups();
        let category = self.adjust.as_ref().map(|a| a.action).and_then(options::category_of);
        let w = which.to_string();
        if w == "options" {
            self.open_xg_options(category);
            return;
        }
        let Some(LayoutUi::Xogot(x)) = &mut self.ui else { return };
        match w.as_str() {
            "palette" => x.toolbar.open_palette(&gd, st.can_create),
            "handles" => x.toolbar.open_handles(&gd, &v, &st),
            "paint" => {
                let below = x.toolbar.paint.clone().upcast();
                x.paint.open(&below);
                x.paint.refresh(&gd, &v, &st, &mats, &groups);
            }
            "tool_options" => x.hud.open_tool_options(&gd, &v, &st),
            _ => {}
        }
    }

    /// An Add Mesh tile (`activate_shape` / `start_tool`): Alt places the shape at once.
    #[func]
    fn on_palette_tile(&mut self, id: i64) {
        let alt = Input::singleton().is_key_pressed(Key::ALT);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.toolbar.palette.hide();
            x.toolbar.last_tile = id;
        }
        if id == toolbar::TILE_BEZIER_PLANE || id == toolbar::TILE_BEZIER_BLOCK {
            self.on_field(if id == toolbar::TILE_BEZIER_BLOCK { 1.0 } else { 0.0 }, Field::BzKind as i64);
            self.run_action(Action::ToolBezier);
            return;
        }
        let old = self.spec;
        let (kind, band) = if id == toolbar::TILE_ARCH_BAND { (ShapeKind::Arch, true) } else { (ShapeKind::from_i64(id), false) };
        self.spec.kind = kind;
        if kind == ShapeKind::Arch {
            self.spec.arch_band = band;
        }
        if self.reshape && self.target.is_some() {
            self.spec_changed(old);
        } else {
            if self.mode != Mode::Object {
                self.set_mode(Mode::Object);
            }
            self.set_tool(Tool::Shape);
            if alt {
                // Place it at once: at the last hover point, else the view centre.
                if self.hover_place.is_none()
                    && let Some(cam) = self.view_camera()
                {
                    let size = cam.get_viewport().map(|v| v.get_visible_rect().size).unwrap_or(Vector2::new(800.0, 600.0));
                    self.hover_place = self.placement(&cam, size * 0.5);
                }
                self.stamp_shape(false);
            }
        }
        self.sync_ui();
        self.redraw();
    }

    /// Fills the Actions menu (`about_to_popup`).
    #[func]
    fn on_xg_actions_popup(&mut self) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let sections = commands::actions_menu(self.mode, |a, m| ui::action_visible(a, m, &st));
        let reasons: Vec<(Action, Option<&'static str>)> = sections.iter().flat_map(|s| s.entries.iter()).map(|e| (e.action, self.requirement(e.action as i64))).collect();
        let table = self.shortcut_table.clone();
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.toolbar.fill_actions(
                &gd,
                &sections,
                |a| reasons.iter().find(|r| r.0 == a).and_then(|r| r.1),
                |a| super::super::shortcuts::entry_of_action(a).and_then(|e| super::super::shortcuts::index(e.id)).and_then(|i| table.get(i).cloned().flatten()),
            );
        }
    }

    /// An Actions menu entry: re-checked, then run like the context menu runs it.
    #[func]
    fn on_xg_action_id(&mut self, id: i64) {
        if self.requirement(id).is_some() && !matches!(Action::from_i64(id), Some(Action::UvEditor | Action::ExportGltf | Action::ExportObj | Action::ExportStl | Action::ExportPly | Action::ExportOptions | Action::BoolUnion | Action::BoolSubtract | Action::BoolIntersect)) {
            return;
        }
        self.run_menu_command(id);
    }

    #[func]
    fn on_xg_options_category(&mut self, index: i64) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui
            && let Some(c) = x.options.shown.get(index as usize).copied()
        {
            x.options.current = c;
            x.options.show_current(&gd, &v, &st);
        }
    }

    #[func]
    fn on_xg_options_search(&mut self, _text: GString) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.options.fill_list();
            x.options.show_current(&gd, &v, &st);
        }
    }

    /// *Reset to Defaults*: the preferences and the session's operation settings (not the layout).
    #[func]
    fn on_xg_options_action(&mut self, action: StringName) {
        if action == StringName::from("reset") {
            self.on_options_reset();
        }
    }

    #[func]
    fn on_options_reset(&mut self) {
        self.xg = ui::XgOpts::default();
        self.opts = ui::ExtraOpts::default();
        self.on_prefs_reset();
    }

    #[func]
    fn on_xg_paint_page(&mut self, page: i64) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.paint.page = page.clamp(0, 2) as usize;
            x.paint.invalidate();
        }
        self.sync_ui();
    }

    #[func]
    fn on_xg_paint_sample(&mut self) {
        self.sample_selection();
    }

    #[func]
    fn on_xg_paint_option(&mut self, id: i64) {
        match id {
            paint::option::LOAD_PALETTE => self.open_palette_dialog(false),
            paint::option::SAVE_PALETTE => self.open_palette_dialog(true),
            paint::option::RESET_PALETTE => self.palette_reset(),
            paint::option::ADD_SLOT => self.add_slot(),
            paint::option::BLEND => self.run_action(Action::BlendMaterial),
            paint::option::INVERT_ALPHA => self.run_action(Action::InvertAlpha),
            p if (paint::option::PRESET..paint::option::PRESET + 5).contains(&p) => self.add_preset_slot((p - paint::option::PRESET) as usize),
            _ => {}
        }
        self.sync_ui();
    }

    fn xg_file(&mut self, purpose: i64, slot: i64) {
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.open_file(purpose, slot);
        }
    }

    #[func]
    fn on_xg_slot_browse(&mut self, slot: i64) {
        self.xg_file(xogot_ui::FILE_SLOT, slot);
    }

    #[func]
    fn on_xg_file(&mut self, path: GString) {
        let purpose = match &self.ui {
            Some(LayoutUi::Xogot(x)) => x.file_purpose,
            _ => return,
        };
        let p = path.to_string();
        match purpose.0 {
            xogot_ui::FILE_SLOT => {
                let res = godot::classes::ResourceLoader::singleton().load(&p);
                self.set_slot_material(res, purpose.1);
            }
            xogot_ui::FILE_PALETTE_LOAD => self.palette_load(&p),
            _ => self.palette_save(&p),
        }
        self.sync_ui();
    }

    /// Right-click on a paint tile (material / colour / smoothing chip): its menu.
    #[func]
    fn on_xg_tile_input(&mut self, event: Gd<InputEvent>, kind: i64, index: i64) {
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else { return };
        if !mb.is_pressed() {
            return;
        }
        if mb.get_button_index() == MouseButton::LEFT && mb.is_alt_pressed() && kind == 0 {
            // Alt-click a material tile: take the selection's material into this slot's place.
            // The press is consumed so the tile's `pressed` (on release) does not then paint the
            // selection with the slot (`gui_input` is emitted before the button handles it).
            if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
                x.paint.popup.set_input_as_handled();
            }
            self.sample_selection();
            return;
        }
        if mb.get_button_index() != MouseButton::RIGHT {
            return;
        }
        let at = mb.get_global_position();
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            // The event is relative to the popover's own window.
            let p = x.paint.popup.get_position();
            x.paint.open_menu(kind, index, at + Vector2::new(p.x as f32, p.y as f32));
        }
    }

    /// A tile menu entry (`id = cmd * 1000 + index`).
    #[func]
    fn on_xg_paint_menu(&mut self, id: i64) {
        use paint::menu as m;
        let (cmd, i) = (id / 1000, id % 1000);
        match cmd {
            m::SLOT_SELECT => self.select_faces_with_slot(i),
            m::SLOT_BROWSE => self.xg_file(xogot_ui::FILE_SLOT, i),
            m::SLOT_INSPECT => {
                if let Some(mat) = self.data().and_then(|d| d.bind().material_slots().get(i as usize).flatten()) {
                    // Deferred: edit_resource re-enters every plugin's handles / edit.
                    EditorInterface::singleton().call_deferred("edit_resource", &[mat.to_variant()]);
                }
            }
            m::SLOT_CLEAR => self.set_slot_material(None, i),
            m::COLOUR_REPLACE => self.swatch_store(i as usize),
            m::COLOUR_USE => self.swatch_use(i as usize, false),
            m::COLOUR_SELECT => {
                if let Some(c) = self.swatches.get(i as usize).copied() {
                    self.select_faces_with_colour(c);
                }
            }
            m::COLOUR_CLEAR => {
                if (i as usize) < self.swatches.len() {
                    self.swatches[i as usize] = ui::SWATCHES[i as usize];
                    self.refresh_swatches();
                }
            }
            m::GROUP_SELECT => self.smoothing_cmd(3, i),
            m::GROUP_MERGE => self.smoothing_cmd(5, i),
            m::GROUP_CLEAR => self.smoothing_cmd(4, i),
            _ => {}
        }
        self.sync_ui();
    }

    /// A colour tile: use it and paint it onto the selection.
    #[func]
    fn on_xg_swatch(&mut self, i: i64) {
        self.swatch_use(i.clamp(0, 7) as usize, true);
    }

    #[func]
    fn on_xg_color(&mut self, c: Color) {
        self.color = c;
        self.sync_ui();
    }

    #[func]
    fn on_xg_smoothing(&mut self, cmd: i64, group: i64) {
        self.smoothing_cmd(cmd, group);
    }

    /// Selects every face using material slot `slot` (both layouts).
    #[func]
    fn on_select_slot(&mut self, slot: i64) {
        self.select_faces_with_slot(slot);
    }

    /// Selects every face with swatch `i`'s colour (both layouts).
    #[func]
    fn on_select_colour(&mut self, i: i64) {
        if let Some(c) = self.swatches.get(i.clamp(0, 7) as usize).copied() {
            self.select_faces_with_colour(c);
        }
    }

    #[func]
    fn on_swatch_use(&mut self, i: i64) {
        self.swatch_use(i.clamp(0, 7) as usize, false);
    }

    #[func]
    fn on_swatch_store(&mut self, i: i64) {
        self.swatch_store(i.clamp(0, 7) as usize);
    }

    /// HUD buttons: Done / Cancel run the tool's Enter / Esc paths; Dismiss clears the message.
    #[func]
    fn on_xg_hud(&mut self, cmd: i64) {
        let message = matches!(&self.ui, Some(LayoutUi::Xogot(x)) if x.hud.is_message());
        match cmd {
            hud::DONE if message => self.message.clear(),
            hud::DONE => self.hud_done(),
            hud::CANCEL => self.synth_key(Key::ESCAPE),
            hud::REMOVE_POINT => self.synth_key(Key::BACKSPACE),
            hud::OPTIONS => {
                self.on_xg_open("tool_options".into());
                return;
            }
            _ => {}
        }
        self.sync_ui();
        self.redraw();
    }

    /// Deferred from the overlay draw when the HUD's content changed (messages and tool status
    /// set outside `sync_ui`).
    #[func]
    fn refresh_xogot_hud(&mut self) {
        if !self.is_xogot() {
            return;
        }
        let st = self.full_state();
        let content = self.hud_content(&st);
        self.hud_key = hud_key(&content);
        if let Some(LayoutUi::Xogot(x)) = &mut self.ui {
            x.hud.show(content, &st);
        }
    }
}

impl PixelBuilderPlugin {
    /// Registers the inspector "Editable Mesh" section.
    pub(super) fn add_inspector(&mut self) {
        let mut ins = super::super::inspector::PixelBuilderInspector::new_gd();
        ins.bind_mut().plugin = Some(self.to_gd().upcast());
        self.base_mut().add_inspector_plugin(&ins);
        self.inspector = Some(ins);
    }

    pub(super) fn remove_inspector(&mut self) {
        if let Some(ins) = self.inspector.take() {
            self.base_mut().remove_inspector_plugin(&ins);
        }
    }

    /// Overlay hook: schedules `refresh_xogot_hud` when the HUD content key changed.
    pub(super) fn check_xogot_hud(&mut self) {
        if !self.is_xogot() {
            return;
        }
        let st = self.full_state();
        let key = hud_key(&self.hud_content(&st));
        if key != self.hud_key {
            self.hud_key = key;
            self.to_gd().call_deferred("refresh_xogot_hud", &[]);
        }
    }
}
