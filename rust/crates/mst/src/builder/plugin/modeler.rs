use super::super::commands;
use super::super::forms::MeshProp;
use super::super::layout::{self, Layout};
use super::super::modeler_ui::{self, LayoutUi, hud, options, paint, toolbar};
use super::super::ui::{self, Action, Field, Mode, SyncState, Tool, Values};
use super::PixelBuilderPlugin;
use godot::classes::{EditorInterface, Input, InputEvent, InputEventKey, InputEventMouseButton};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
use pt_modeler::ShapeKind;
impl PixelBuilderPlugin {
    pub(super) fn is_modeler(&self) -> bool {
        matches!(self.ui, Some(LayoutUi::Modeler(_)))
    }
    pub(super) fn modeler_condition(&self) -> SyncState {
        let has_target = self.target.is_some();
        let face_slot = self.contents().and_then(|d| {
            self.sel_faces
                .first()
                .and_then(|f| d.bind().cage().facets.get(*f).map(|x| x.stuff as i64))
        });
        let t = self.target.as_ref();
        SyncState {
            visible: self.visible,
            can_create: has_target || self.host.is_some(),
            sel_faces: if self.mode == Mode::Face {
                self.sel_faces.len()
            } else {
                0
            },
            message: self.message.clone(),
            tool_detail: self.tool_lines().join("\n"),
            prefs: self.prefs,
            adjusting: self.adjust.is_some(),
            live_uv2: t.is_some_and(|t| t.get("live_uv2").try_to::<bool>().unwrap_or(false)),
            clear_grass: t.is_some_and(|t| t.get("clear_grass").try_to::<bool>().unwrap_or(false)),
            face_slot: if self.mode == Mode::Face {
                face_slot
            } else {
                None
            },
            ..SyncState::default()
        }
    }
    fn hud_content(&self, st: &SyncState) -> Option<hud::Content> {
        hud::payload(st, self.spec.sort, self.bz.values().block)
    }
    pub(super) fn sync_modeler(&mut self, v: &Values, st: &SyncState) {
        let mats: Vec<Option<Gd<godot::classes::Material>>> = self
            .contents()
            .map(|d| d.bind().stuff_pockets().iter_shared().collect())
            .unwrap_or_default();
        let groups = self.gliding_clusters();
        let content = self.hud_content(st);
        self.hud_key = hud_key(&content);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.sync(v, st, &mats, &groups, content);
        }
    }
    pub(super) fn build_layout(&mut self, l: Layout) {
        self.sync_menus(self.visible);
        let gd = self.to_gd().upcast::<Object>();
        let spec = self.spec;
        let v = self.values_with(&spec);
        let st = SyncState {
            visible: self.visible,
            ..self.modeler_condition()
        };
        let Some(mut u) = LayoutUi::assemble(l, &gd, &v, &st) else {
            godot_error!("Pixel Builder: the side-panel layout's scenes are broken; no builder panels shown.");
            self.palette_slots = None;
            return;
        };
        if let LayoutUi::Roblox(r) = &mut u {
            r.in_menu = !r.host_below_spatial_toolbar();
        }
        for (c, ctl) in u.controls() {
            self.base_mut().add_control_to_container(c, &ctl);
        }
        u.assign_shown(self.visible, self.target.is_some());
        if let LayoutUi::Modeler(x) = &mut u
            && let Some(cam) = self.camera.clone().filter(|c| c.is_instance_valid())
            && let Some(i) = hud::viewport_of(&cam)
        {
            x.hud.set_viewport(i);
        }
        self.ui = Some(u);
        self.palette_slots = None;
        self.refresh_menu_layout();
        if l == Layout::Roblox {
            self.apply_roblox_rot_snap();
            self.enter_roblox_move_step();
        }
    }
    pub(super) fn teardown_layout(&mut self, now: bool) {
        let Some(u) = self.ui.take() else { return };
        for (c, ctl) in u.controls() {
            if ctl.is_instance_valid() {
                self.base_mut().remove_control_from_container(c, &ctl);
            }
        }
        u.free(now);
    }
    pub(super) fn switch_layout(&mut self, l: Layout) {
        l.store();
        self.to_gd().call_deferred("apply_layout", &[]);
    }
    pub(super) fn refresh_menu_layout(&mut self) {
        let x = self.is_modeler();
        if let Some(m) = &self.builder_menu
            && m.is_instance_valid()
            && let Some(mut p) = m.get_popup()
            && let Some(i) = (0..p.get_item_count()).find(|i| p.get_item_id(*i) == super::builder_menu::LAYOUT_ITEM_ID)
        {
            p.set_item_checked(i, x);
        }
    }
    pub(super) fn unveil_xtra_choices(&mut self, group_kind: Option<usize>) {
        if !self.is_modeler() {
            self.open_preferences();
            return;
        }
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.toolbar.handles_popup.hide();
            x.choices.unveil(&gd, &v, &st, group_kind);
        }
    }
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
            reshape: (self.reshape || (self.bz.editing.is_some() && self.tool == Tool::Bezier))
                && has_target,
            has_target,
            has_spec: self.contents().is_some_and(|d| {
                d.bind().shape_spec().is_some() || d.bind().bezier_recipe().is_some()
            }),
            can_convert: self.convert_src.is_some(),
            insert_vert: self.insert_vert,
            surface_draw: self.surface_draw,
            xray: self.xray,
            entity: self
                .target
                .as_ref()
                .map(|t| t.get("entity_type").try_to::<i32>().unwrap_or(0))
                .unwrap_or(0),
            bz_knot: self.bz.selected.is_some(),
            ..self.modeler_condition()
        }
    }
    pub(super) fn perform_produce_uv_b(&mut self) {
        let Some(t) = self.target.clone() else {
            self.message = "Select a builder mesh first".into();
            return;
        };
        let texel = self.prefs.lightmap_texel.max(0.0105);
        if let Some(d) = t.bind().contents()
            && let Err(e) = pt_modeler::lightmap::check_atlas(
                d.bind().cage(),
                super::super::data::to_p(t.get_global_transform().basis.get_scale()),
                texel,
            )
        {
            self.message = format!("Generate lightmap UVs: {e} (Preferences › Lightmap texel)");
            return;
        }
        let on = t.get("live_uv2").try_to::<bool>().unwrap_or(false);
        let cur = t.get("lightmap_texel").try_to::<f32>().unwrap_or(-1.0);
        if on && (cur - texel).abs() < 1e-6 {
            self.message =
                format!("Lightmap UVs: already generated at texel {texel} (Live UV2 on)");
            return;
        }
        let Some(mut ur) = self.open_action("Pixel Builder: Generate lightmap UVs") else {
            return;
        };
        ur.add_do_property(&t, "lightmap_texel", &texel.to_variant());
        ur.add_do_property(&t, "live_uv2", &true.to_variant());
        ur.add_undo_property(&t, "lightmap_texel", &t.get("lightmap_texel"));
        ur.add_undo_property(&t, "live_uv2", &t.get("live_uv2"));
        ur.call_deferred("commit_action", &[]);
        self.to_gd().call_deferred("sync_ui_deferred", &[]);
        self.message = format!("Lightmap UVs: unwrapping at texel {texel} (Live UV2 on)");
    }
    fn hud_done(&mut self) {
        match self.tool {
            Tool::Knife | Tool::Bezier => self.synth_key(Key::ENTER),
            Tool::Shape
                if self.spec.sort.is_polyline()
                    || self.poly_outline.is_some()
                    || !self.wall_pts.is_empty() =>
            {
                self.synth_key(Key::ENTER)
            }
            Tool::Shape => {
                if self.hover_place.is_none()
                    && let Some(cam) = self.view_camera()
                {
                    let size = cam
                        .get_viewport()
                        .map(|v| v.get_visible_rect().size)
                        .unwrap_or(Vector2::new(800.0, 600.0));
                    self.hover_place = self.locus(&cam, size * 0.5);
                }
                self.stamp_shape(false);
            }
            _ => self.assign_instrument(Tool::Select),
        }
    }
    pub(super) fn ask_confirmation(&mut self, a: Action) -> bool {
        if !self.is_modeler() || self.requirement(a as i64).is_some() {
            return false;
        }
        let Some((title, fields)) = commands::confirmation(a, self.mode, self.xg.to_new_object)
        else {
            return false;
        };
        self.last_confirmation = title.to_string();
        if self.auto_confirm {
            self.run_confirmed(a);
            return true;
        }
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            let c = x
                .confirm
                .get_or_insert_with(|| modeler_ui::Confirm::assemble(&gd));
            c.unveil(&gd, a, title, &fields, &v, &st);
        }
        true
    }
    fn run_confirmed(&mut self, a: Action) {
        let old = (self.no_adjust, self.no_confirm);
        self.no_adjust = true;
        self.no_confirm = true;
        self.run_action(a);
        (self.no_adjust, self.no_confirm) = old;
    }
    pub(super) fn view_camera(&self) -> Option<Gd<godot::classes::Camera3D>> {
        self.camera
            .clone()
            .filter(|c| c.is_instance_valid())
            .or_else(|| {
                EditorInterface::singleton()
                    .get_editor_viewport_3d_ex()
                    .idx(0)
                    .done()
                    .and_then(|v| v.get_camera_3d())
            })
    }
    fn synth_key(&mut self, k: Key) {
        let Some(cam) = self.view_camera() else {
            return;
        };
        let mut ev = InputEventKey::new_gd();
        ev.set_keycode(k);
        ev.set_pressed(true);
        use godot::classes::IEditorPlugin;
        self.forward_3d_gui_input(Some(cam), Some(ev.upcast()));
    }
}
fn hud_key(c: &Option<hud::Content>) -> String {
    c.as_ref()
        .map(|c| format!("{}|{}|{}", c.heading, c.detail, c.tool))
        .unwrap_or_default()
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    pub(super) fn apply_layout(&mut self) {
        let want = Layout::ingest();
        if self.ui.as_ref().is_some_and(|u| u.layout() == want) {
            self.sync_ui();
            return;
        }
        self.adjust_commit();
        self.hide_adjust_panel();
        self.cancel_drag();
        self.cancel_shape();
        self.bz.cancel_live();
        self.cancel_vpaint_stroke();
        self.marquee = None;
        let uv_was_open = self.tex_coord_unveil();
        let was_roblox = self.is_roblox();
        self.teardown_layout(false);
        self.build_layout(want);
        if super::roblox_rules::leaves_roblox(was_roblox, want) {
            self.leave_roblox();
        }
        if super::roblox_rules::inspector_sections_change(was_roblox, want)
            && let Some(mut shown) = EditorInterface::singleton()
                .get_inspector()
                .and_then(|i| i.get_edited_object())
        {
            shown.notify_property_list_changed();
        }
        if want == Layout::Mst {
            self.place_uv(false);
        }
        if uv_was_open {
            self.unveil_tex_coord_studio();
        }
        self.palette_slots = None;
        self.sync_ui();
        self.sync_menus(self.visible);
        self.redraw();
    }
    #[func]
    fn on_layout_choice(&mut self, index: i64) {
        self.switch_layout(Layout::from_i64(index));
    }
    #[func]
    fn debug_set_layout(&mut self, n: i64) {
        layout::set_override(n);
        self.apply_layout();
    }
    #[func]
    fn debug_layout(&mut self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let st = self.full_state();
        d.set(
            "layout",
            self.ui.as_ref().map(|u| u.layout().moniker()).unwrap_or(""),
        );
        d.set("setting", Layout::ingest().moniker());
        match &self.ui {
            Some(LayoutUi::Mst(u)) => {
                d.set("mst_toolbar", u.toolbar.is_inside_tree());
                d.set("mst_strip", u.peel.is_inside_tree());
            }
            Some(LayoutUi::Roblox(r)) => {
                let root = r.root();
                d.set("ribbon", root.is_inside_tree() && root.is_visible_in_tree());
                let mut chain = PackedStringArray::new();
                let mut at = root.get_parent();
                while let Some(n) = at.filter(|_| chain.len() < 6) {
                    chain.push(&GString::from(
                        format!("{}:{}", n.get_name(), n.get_class()).as_str(),
                    ));
                    at = n.get_parent();
                }
                d.set("ribbon_parents", &chain);
                d.set("ribbon_size", root.get_size());
                let mut disabled = VarDictionary::new();
                for b in &r.ribbon.buttons {
                    disabled.set(format!("{:?}", b.action).as_str(), b.button.is_disabled());
                }
                d.set("ribbon_disabled", &disabled);
            }
            Some(LayoutUi::Modeler(x)) => {
                let tb = &x.toolbar;
                d.set("toolbar", tb.hbox.is_inside_tree() && tb.hbox.is_visible());
                d.set(
                    "toolbar_parent",
                    tb.hbox
                        .get_parent()
                        .map(|p| p.get_class().to_string())
                        .unwrap_or_default()
                        .as_str(),
                );
                let mut vis = VarDictionary::new();
                for (name, b) in [
                    ("add_mesh", &tb.add_mesh),
                    ("make_editable", &tb.make_editable),
                    ("snap", &tb.snap),
                    ("paint", &tb.coat),
                    ("handles", &tb.handles),
                    ("options", &tb.choices),
                ] {
                    vis.set(name, b.is_visible());
                }
                vis.set("actions", tb.actions.is_visible());
                for (m, b) in &tb.modes {
                    vis.set(
                        format!("mode_{}", m.moniker().to_lowercase()).as_str(),
                        b.is_visible(),
                    );
                    if b.is_pressed() {
                        d.set("mode_pressed", m.moniker());
                    }
                }
                d.set("buttons", &vis);
                d.set("snap_pressed", tb.snap.is_pressed());
                d.set("handles_open", tb.handles_popup.is_visible());
                let rows: PackedStringArray = tb
                    .handles_form
                    .labels()
                    .iter()
                    .map(|s| GString::from(s.as_str()))
                    .collect();
                d.set("handles_rows", &rows);
                d.set("palette_open", tb.palette.is_visible());
                d.set("paint_open", x.coat.popup.is_visible());
                d.set("paint_page", x.coat.page as i64);
                d.set("options_open", x.choices.dialog.is_visible());
                let cats: PackedStringArray = x
                    .choices
                    .shown
                    .iter()
                    .map(|i| GString::from(options::CATEGORIES[*i].1))
                    .collect();
                d.set("options_categories", &cats);
                d.set("options_current", options::CATEGORIES[x.choices.current].1);
                let rows: PackedStringArray = x
                    .choices
                    .form
                    .labels()
                    .iter()
                    .map(|s| GString::from(s.as_str()))
                    .collect();
                d.set("options_rows", &rows);
                d.set("hud_visible", x.hud.panel.is_visible());
                d.set("hud_viewport", x.hud.viewport() as i64);
                let parent_vp = x
                    .hud
                    .panel
                    .get_parent()
                    .and_then(|p| {
                        (0..4).find(|i| {
                            hud::host(*i).is_some_and(|h| h.instance_id() == p.instance_id())
                        })
                    })
                    .map(|i| i as i64)
                    .unwrap_or(-1);
                d.set("hud_parent_viewport", parent_vp);
                if let Some(c) = x.hud.shown() {
                    d.set("hud_title", c.heading.as_str());
                    d.set("hud_detail", c.detail.as_str());
                }
                let bt: PackedStringArray = x
                    .hud
                    .button_texts()
                    .iter()
                    .map(|s| GString::from(s.as_str()))
                    .collect();
                d.set("hud_buttons", &bt);
                d.set("tool_options_open", x.hud.tool_options.is_visible());
                let tr: PackedStringArray = x
                    .hud
                    .tool_form
                    .labels()
                    .iter()
                    .map(|s| GString::from(s.as_str()))
                    .collect();
                d.set("tool_options_rows", &tr);
            }
            None => {}
        }
        d.set("visible", st.visible);
        d.set("uv_host", self.tex_coord_carrier());
        d.set("uv_open", self.tex_coord_unveil());
        d
    }
    #[func]
    fn debug_modeler_actions(&mut self) -> VarArray {
        self.on_xg_actions_popup();
        let st = self.full_state();
        let mut out = VarArray::new();
        for sec in commands::actions_menu(self.mode, |a, m| ui::action_visible(a, m, &st)) {
            for e in sec.entries {
                let mut d = VarDictionary::new();
                d.set("section", sec.heading.unwrap_or(""));
                d.set("id", e.deed as i64);
                d.set("title", e.heading.as_str());
                let r = self.requirement(e.deed as i64);
                d.set("enabled", r.is_none());
                d.set("reason", r.unwrap_or(""));
                out.push(&d.to_variant());
            }
        }
        out
    }
    #[func]
    fn debug_modeler_open(&mut self, which: GString) -> bool {
        self.on_xg_open(which.clone());
        let w = which.to_string();
        let Some(LayoutUi::Modeler(x)) = &self.ui else {
            return false;
        };
        match w.as_str() {
            "palette" => x.toolbar.palette.is_visible(),
            "paint" => x.coat.popup.is_visible(),
            "handles" => x.toolbar.handles_popup.is_visible(),
            "options" => x.choices.dialog.is_visible(),
            "tool_options" => x.hud.tool_options.is_visible(),
            _ => false,
        }
    }
    #[func]
    fn xg_file_debug(&mut self, purpose: i64, path: GString) {
        let p = path.to_string();
        match purpose {
            modeler_ui::FILE_PALETTE_LOAD => self.swatchset_ingest(&p),
            modeler_ui::FILE_PALETTE_SAVE => self.swatchset_stow(&p),
            _ => {
                let res = godot::classes::ResourceLoader::singleton().load(&p);
                let slot = self.pocket;
                self.assign_pocket_stuff(res, slot);
            }
        }
        self.sync_ui();
    }
    #[func]
    fn debug_modeler_close(&mut self) {
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.toolbar.palette.hide();
            x.toolbar.handles_popup.hide();
            x.coat.popup.hide();
            x.choices.dialog.hide();
            x.hud.tool_options.hide();
        }
    }
    #[func]
    fn debug_modeler_options_search(&mut self, query: GString) {
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.choices.search.set_text(&query);
        }
        self.on_xg_options_search(query);
    }
    #[func]
    fn on_xg_toggle(&mut self, on: bool, token_id: i64) {
        let Some(a) = Action::from_i64(token_id) else {
            return;
        };
        let spec = self.spec;
        let st = self.full_state();
        let cur = ui::action_pressed(a, &self.values_with(&spec), &st).unwrap_or(!on);
        if cur != on {
            self.run_action(a);
        } else {
            self.sync_ui();
        }
    }
    #[func]
    fn on_xg_mesh_prop(&mut self, on: bool, which: i64) {
        let Some(t) = self.target.clone() else { return };
        let prop = if which == MeshProp::ClearGrass as i64 {
            "clear_grass"
        } else {
            "live_uv2"
        };
        let Some(mut ur) = self.open_action(&format!("Pixel Builder: {prop}")) else {
            return;
        };
        ur.add_do_property(&t, prop, &on.to_variant());
        ur.add_undo_property(&t, prop, &t.get(prop));
        ur.call_deferred("commit_action", &[]);
        self.to_gd().call_deferred("sync_ui_deferred", &[]);
    }
    #[func]
    fn on_xg_confirmed(&mut self) {
        let a = match &mut self.ui {
            Some(LayoutUi::Modeler(x)) => x.confirm.as_mut().and_then(|c| c.deed.take()),
            _ => None,
        };
        if let Some(a) = a {
            self.run_confirmed(a);
        }
    }
    #[func]
    fn on_xg_confirm_canceled(&mut self) {
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui
            && let Some(c) = &mut x.confirm
        {
            c.deed = None;
        }
        self.message = String::new();
        self.redraw();
    }
    #[func]
    fn debug_auto_confirm(&mut self, on: bool) {
        self.auto_confirm = on;
    }
    #[func]
    fn debug_confirmation(&mut self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("last", self.last_confirmation.as_str());
        if let Some(LayoutUi::Modeler(x)) = &self.ui
            && let Some(c) = &x.confirm
        {
            d.set("open", c.dialog.is_visible());
            d.set("action", c.deed.map(|a| a as i64).unwrap_or(-1));
            d.set("title", &c.dialog.get_title());
            d.set("text", &c.dialog.get_text());
            let rows: PackedStringArray = c
                .form
                .labels()
                .iter()
                .map(|s| GString::from(s.as_str()))
                .collect();
            d.set("rows", &rows);
        } else {
            d.set("open", false);
            d.set("action", -1i64);
        }
        d
    }
    #[func]
    fn debug_confirm(&mut self, accept: bool) {
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui
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
    #[func]
    fn on_xg_open(&mut self, which: GString) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        let mats: Vec<Option<Gd<godot::classes::Material>>> = self
            .contents()
            .map(|d| d.bind().stuff_pockets().iter_shared().collect())
            .unwrap_or_default();
        let groups = self.gliding_clusters();
        let category = self
            .adjust
            .as_ref()
            .map(|a| a.deed)
            .and_then(options::category_of);
        let w = which.to_string();
        if w == "options" {
            self.unveil_xtra_choices(category);
            return;
        }
        let Some(LayoutUi::Modeler(x)) = &mut self.ui else {
            return;
        };
        match w.as_str() {
            "palette" => x.toolbar.unveil_swatchset(&gd, st.can_create),
            "handles" => x.toolbar.open_handles(&gd, &v, &st),
            "paint" => {
                let below = x.toolbar.coat.clone().upcast();
                x.coat.unveil(&below);
                x.coat.renew(&gd, &v, &st, &mats, &groups);
            }
            "tool_options" => x.hud.unveil_instrument_choices(&gd, &v, &st),
            _ => {}
        }
    }
    #[func]
    fn on_palette_tile(&mut self, token_id: i64) {
        let alt = Input::singleton().is_key_pressed(Key::ALT);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.toolbar.palette.hide();
            x.toolbar.last_tile = token_id;
        }
        if token_id == toolbar::TILE_BEZIER_PLANE || token_id == toolbar::TILE_BEZIER_BLOCK {
            self.on_field(
                if token_id == toolbar::TILE_BEZIER_BLOCK {
                    1.0
                } else {
                    0.0
                },
                Field::BzKind as i64,
            );
            self.run_action(Action::ToolBezier);
            return;
        }
        let old = self.spec;
        let (kind, band) = if token_id == toolbar::TILE_ARCH_BAND {
            (ShapeKind::Arch, true)
        } else {
            (ShapeKind::from_i64(token_id), false)
        };
        self.spec.sort = kind;
        if kind == ShapeKind::Arch {
            self.spec.arch_band = band;
        }
        if self.reshape && self.target.is_some() {
            self.spec_changed(old);
        } else {
            if self.mode != Mode::Object {
                self.assign_setting(Mode::Object);
            }
            self.assign_instrument(Tool::Shape);
            if alt {
                if self.hover_place.is_none()
                    && let Some(cam) = self.view_camera()
                {
                    let size = cam
                        .get_viewport()
                        .map(|v| v.get_visible_rect().size)
                        .unwrap_or(Vector2::new(800.0, 600.0));
                    self.hover_place = self.locus(&cam, size * 0.5);
                }
                self.stamp_shape(false);
            }
        }
        self.sync_ui();
        self.redraw();
    }
    #[func]
    fn on_xg_actions_popup(&mut self) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let sections = commands::actions_menu(self.mode, |a, m| ui::action_visible(a, m, &st));
        let reasons: Vec<(Action, Option<&'static str>)> = sections
            .iter()
            .flat_map(|s| s.entries.iter())
            .map(|e| (e.deed, self.requirement(e.deed as i64)))
            .collect();
        let table = self.shortcut_table.clone();
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.toolbar.flood_deeds(
                &gd,
                &sections,
                |a| reasons.iter().find(|r| r.0 == a).and_then(|r| r.1),
                |a| {
                    super::super::shortcuts::entry_of_action(a)
                        .and_then(|e| super::super::shortcuts::ordinal(e.id))
                        .and_then(|i| table.get(i).cloned().flatten())
                },
            );
        }
    }
    #[func]
    fn on_xg_action_id(&mut self, token_id: i64) {
        if self.requirement(token_id).is_some()
            && !matches!(
                Action::from_i64(token_id),
                Some(
                    Action::UvEditor
                        | Action::ExportGltf
                        | Action::ExportObj
                        | Action::ExportStl
                        | Action::ExportPly
                        | Action::ExportOptions
                        | Action::BoolUnion
                        | Action::BoolSubtract
                        | Action::BoolIntersect
                )
            )
        {
            return;
        }
        self.run_menu_command(token_id);
    }
    #[func]
    fn on_xg_options_category(&mut self, index: i64) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui
            && let Some(c) = x.choices.shown.get(index as usize).copied()
        {
            x.choices.current = c;
            x.choices.show_current(&gd, &v, &st);
        }
    }
    #[func]
    fn on_xg_options_search(&mut self, _text: GString) {
        let gd = self.to_gd().upcast::<Object>();
        let st = self.full_state();
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.choices.fill_list();
            x.choices.show_current(&gd, &v, &st);
        }
    }
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
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.coat.page = page.clamp(0, 2) as usize;
            x.coat.invalidate();
        }
        self.sync_ui();
    }
    #[func]
    fn on_xg_paint_sample(&mut self) {
        self.sample_selection();
    }
    #[func]
    fn on_xg_paint_option(&mut self, token_id: i64) {
        match token_id {
            paint::option::LOAD_PALETTE => self.unveil_swatchset_popup(false),
            paint::option::SAVE_PALETTE => self.unveil_swatchset_popup(true),
            paint::option::RESET_PALETTE => self.palette_reset(),
            paint::option::ADD_SLOT => self.insert_pocket(),
            paint::option::BLEND => self.run_action(Action::BlendMaterial),
            paint::option::INVERT_ALPHA => self.run_action(Action::InvertAlpha),
            p if (paint::option::PRESET..paint::option::PRESET + 5).contains(&p) => {
                self.add_preset_slot((p - paint::option::PRESET) as usize)
            }
            _ => {}
        }
        self.sync_ui();
    }
    fn xg_file(&mut self, purpose: i64, slot: i64) {
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.open_file(purpose, slot);
        }
    }
    #[func]
    fn on_xg_slot_browse(&mut self, slot: i64) {
        self.xg_file(modeler_ui::FILE_SLOT, slot);
    }
    #[func]
    fn on_xg_file(&mut self, path: GString) {
        let purpose = match &self.ui {
            Some(LayoutUi::Modeler(x)) => x.file_purpose,
            _ => return,
        };
        let p = path.to_string();
        match purpose.0 {
            modeler_ui::FILE_SLOT => {
                let res = godot::classes::ResourceLoader::singleton().load(&p);
                self.assign_pocket_stuff(res, purpose.1);
            }
            modeler_ui::FILE_PALETTE_LOAD => self.swatchset_ingest(&p),
            _ => self.swatchset_stow(&p),
        }
        self.sync_ui();
    }
    #[func]
    fn on_xg_tile_input(&mut self, happening: Gd<InputEvent>, kind: i64, index: i64) {
        let Ok(mb) = happening.try_cast::<InputEventMouseButton>() else {
            return;
        };
        if !mb.is_pressed() {
            return;
        }
        if mb.get_button_index() == MouseButton::LEFT && mb.is_alt_pressed() && kind == 0 {
            if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
                x.coat.popup.set_input_as_handled();
            }
            self.sample_selection();
            return;
        }
        if mb.get_button_index() != MouseButton::RIGHT {
            return;
        }
        let at = mb.get_global_position();
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            let p = x.coat.popup.get_position();
            x.coat
                .open_menu(kind, index, at + Vector2::new(p.x as f32, p.y as f32));
        }
    }
    #[func]
    fn on_xg_paint_menu(&mut self, token_id: i64) {
        use paint::menu as m;
        let (cmd, i) = (token_id / 1000, token_id % 1000);
        match cmd {
            m::SLOT_SELECT => self.select_faces_with_slot(i),
            m::SLOT_BROWSE => self.xg_file(modeler_ui::FILE_SLOT, i),
            m::SLOT_INSPECT => {
                if let Some(mat) = self
                    .contents()
                    .and_then(|d| d.bind().stuff_pockets().get(i as usize).flatten())
                {
                    EditorInterface::singleton()
                        .call_deferred("edit_resource", &[mat.to_variant()]);
                }
            }
            m::SLOT_CLEAR => self.assign_pocket_stuff(None, i),
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
    #[func]
    fn on_xg_swatch(&mut self, i: i64) {
        self.swatch_use(i.clamp(0, 7) as usize, true);
    }
    #[func]
    fn on_xg_color(&mut self, c: Color) {
        self.tint = c;
        self.sync_ui();
    }
    #[func]
    fn on_xg_smoothing(&mut self, cmd: i64, group: i64) {
        self.smoothing_cmd(cmd, group);
    }
    #[func]
    fn on_select_slot(&mut self, slot: i64) {
        self.select_faces_with_slot(slot);
    }
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
    #[func]
    fn on_xg_hud(&mut self, cmd: i64) {
        let message = matches!(&self.ui, Some(LayoutUi::Modeler(x)) if x.hud.is_message());
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
    #[func]
    fn refresh_modeler_hud(&mut self) {
        if !self.is_modeler() {
            return;
        }
        let st = self.full_state();
        let content = self.hud_content(&st);
        self.hud_key = hud_key(&content);
        if let Some(LayoutUi::Modeler(x)) = &mut self.ui {
            x.hud.show(content, &st);
        }
    }
}
impl PixelBuilderPlugin {
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
    pub(super) fn check_modeler_hud(&mut self) {
        if !self.is_modeler() {
            return;
        }
        let st = self.full_state();
        let key = hud_key(&self.hud_content(&st));
        if key != self.hud_key {
            self.hud_key = key;
            self.to_gd().call_deferred("refresh_modeler_hud", &[]);
        }
    }
}
