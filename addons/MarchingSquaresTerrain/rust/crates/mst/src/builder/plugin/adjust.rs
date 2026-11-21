use super::super::data::PixelBuilderMeshData;
use super::super::ui::{self, Action, Field, FieldCtl, Mode};
use super::PixelBuilderPlugin;
use super::editor_scenes;
use crate::editor::ui_kit;
use godot::builtin::Side;
use godot::classes::control::{GrowDirection, LayoutPreset};
use godot::classes::{
    Button, Control, EditorInterface, HFlowContainer, Label, PanelContainer, StyleBoxFlat, Timer,
};
use godot::prelude::*;
use pt_core::build::{BMesh, Edge};
use std::collections::BTreeSet;
pub struct Adjust {
    pub deed: Action,
    pub mode: Mode,
    pub contents: Gd<PixelBuilderMeshData>,
    pub before: PackedByteArray,
    pub base: BMesh,
    pub sel: (BTreeSet<usize>, BTreeSet<Edge>, BTreeSet<u32>, Option<u32>),
    pub last: PackedByteArray,
    pub moniker: String,
    pub fields: Vec<Field>,
    pub pending: bool,
    pub status: String,
}
pub struct AdjustUi {
    pub panel: Gd<PanelContainer>,
    heading: Gd<Label>,
    flow: Gd<HFlowContainer>,
    status: Gd<Label>,
    timer: Gd<Timer>,
    ctls: Vec<(Field, FieldCtl)>,
}
pub fn fields(a: Action, mode: Mode, to_new_object: bool) -> Option<Vec<Field>> {
    use Field as F;
    Some(match (a, mode) {
        (Action::Extrude, Mode::Face) => vec![F::Distance, F::ExtrudeMethod],
        (Action::Extrude, Mode::Edge) => vec![F::Distance, F::EdgesGroup],
        (Action::Inset, Mode::Face) => vec![F::Inset, F::InsetMode, F::InsetIndividual],
        (Action::Bevel, Mode::Edge | Mode::Vertex) => vec![
            F::Bevel,
            F::BevelAmountType,
            F::BevelSegs,
            F::BevelProfile,
            F::BevelClamp,
            F::BevelHarden,
        ],
        (Action::SubdivideEdges, Mode::Edge) => vec![F::Cuts],
        (Action::InsertLoop, Mode::Edge) => vec![F::Cuts],
        (Action::Subdivide, Mode::Face | Mode::Object) => vec![F::SubdivSegs],
        (Action::Collapse, Mode::Vertex) => vec![F::CollapseFirst],
        (Action::Weld, Mode::Vertex | Mode::Object) => vec![F::WeldDist],
        (Action::Fill, Mode::Edge | Mode::Vertex) => vec![F::FillEntire],
        (Action::OffsetElements, Mode::Face) => vec![
            F::OffsetX,
            F::OffsetY,
            F::OffsetZ,
            F::OffsetSpace,
            F::OffsetPerFace,
        ],
        (Action::OffsetElements, Mode::Edge | Mode::Vertex) => {
            vec![F::OffsetX, F::OffsetY, F::OffsetZ, F::OffsetSpace]
        }
        (Action::Mirror, Mode::Object) => vec![F::MirrorX, F::MirrorY, F::MirrorZ, F::MirrorDup],
        (Action::Detach | Action::Duplicate, _) if to_new_object => return None,
        _ => return None,
    })
}
fn hud_style(scale: f32) -> Gd<StyleBoxFlat> {
    let mut style = StyleBoxFlat::new_gd();
    let theme = EditorInterface::singleton().get_editor_theme();
    let mut bg = theme
        .as_ref()
        .map(|t| t.get_color("base_color", "Editor"))
        .unwrap_or(Color::from_rgb(0.1305, 0.1505, 0.1805));
    bg.a = 0.9405;
    style.set_bg_color(bg);
    style.set_border_color(
        theme
            .as_ref()
            .map(|t| t.get_color("dark_color_3", "Editor"))
            .unwrap_or(Color::from_rgb(0.0805, 0.0805, 0.1005)),
    );
    style.set_border_width_all(1);
    style.set_corner_radius_all((6.0 * scale) as i32);
    style.set_content_margin_all(8.0 * scale);
    style.set_shadow_color(Color::from_rgba(0.0, 0.0, 0.0, 0.2505));
    style.set_shadow_size((4.0 * scale) as i32);
    style
}
fn leading_number(t: &str) -> Option<f64> {
    let t = t.trim();
    let end = t
        .char_indices()
        .find(|(i, c)| {
            !(c.is_ascii_digit()
                || *c == '.'
                || ((*c == '-' || *c == '+') && *i == 0)
                || *c == 'e'
                || *c == 'E')
        })
        .map_or(t.len(), |(i, _)| i);
    t[..end].parse::<f64>().ok().filter(|v| v.is_finite())
}
fn set_ctl_blocked(c: &mut FieldCtl, on: bool) {
    match c {
        FieldCtl::Spin(_, sb) => {
            sb.set_block_signals(on);
            if let Some(mut le) = sb.get_line_edit() {
                le.set_block_signals(on);
            }
        }
        FieldCtl::Option(_, ob) => ob.set_block_signals(on),
        FieldCtl::Check(cb) => cb.set_block_signals(on),
    }
}
impl PixelBuilderPlugin {
    fn ensure_adjust_timer(&mut self) {
        if self
            .adjust_ui
            .as_ref()
            .is_none_or(|u| !u.panel.is_instance_valid())
        {
            self.build_adjust_panel();
        }
    }
    pub(super) fn adjust_begin(&mut self, a: Action, fields: Vec<Field>) {
        let Some(data) = self.contents() else { return };
        let (before, base) = {
            let d = data.bind();
            (d.get_blob(), d.cage().clone())
        };
        self.adjust = Some(Adjust {
            deed: a,
            mode: self.mode,
            contents: data,
            last: before.clone(),
            before,
            base,
            sel: (
                self.sel_faces.clone(),
                self.sel_edges.clone(),
                self.sel_verts.clone(),
                self.active_vert,
            ),
            moniker: format!("Pixel Builder: {}", a.heading()),
            fields,
            pending: false,
            status: String::new(),
        });
        self.adjust_running = true;
    }
    pub(super) fn adjust_started(&mut self) {
        self.adjust_running = false;
        let Some(adj) = &mut self.adjust else { return };
        let now = adj.contents.bind().get_blob();
        if now == adj.before {
            self.adjust = None;
            return;
        }
        adj.last = now;
        adj.status = self.message.clone();
        self.show_adjust_panel();
    }
    pub(super) fn adjust_field_changed(&mut self, f: Field) {
        let opts = self.chamfer_choices();
        let Some(adj) = &mut self.adjust else { return };
        if !adj.fields.contains(&f) {
            return;
        }
        adj.pending = true;
        let coarse = adj.deed == Action::Bevel && {
            let n = if adj.mode == Mode::Vertex {
                adj.sel.2.len()
            } else {
                adj.sel.1.len()
            };
            pt_core::build::xg_bevel::prelook_stretches(&opts, n) < opts.stretches
        };
        if coarse {
            self.adjust_rerun(true);
        }
        self.ensure_adjust_timer();
        if let Some(u) = &mut self.adjust_ui {
            u.timer.start();
        }
        self.sync_adjust_panel();
    }
    pub(super) fn adjust_rerun(&mut self, coarse: bool) {
        let Some(adj) = &mut self.adjust else { return };
        let mut data = adj.contents.clone();
        if !data.is_instance_valid() || data.bind().get_blob() != adj.last {
            self.adjust = None;
            self.hide_adjust_panel();
            return;
        }
        if !coarse {
            adj.pending = false;
        }
        let (a, base) = (adj.deed, adj.base.clone());
        (
            self.sel_faces,
            self.sel_edges,
            self.sel_verts,
            self.active_vert,
        ) = adj.sel.clone();
        data.bind_mut().substitute(base);
        self.message.clear();
        self.adjust_running = true;
        self.adjust_coarse = coarse;
        self.run_action(a);
        self.adjust_running = false;
        self.adjust_coarse = false;
        data.emit_changed();
        let blob = data.bind().get_blob();
        let msg = self.message.clone();
        if let Some(adj) = &mut self.adjust {
            adj.last = blob;
            adj.status = msg;
        }
        self.sync_adjust_panel();
        self.redraw();
    }
    pub(super) fn adjust_commit(&mut self) {
        if self.adjust_running || self.adjust.is_none() {
            return;
        }
        self.flush_adjust_text();
        if self.adjust.as_ref().is_some_and(|a| a.pending) {
            self.adjust_rerun(false);
        }
        self.stop_adjust_timer();
        let Some(adj) = self.adjust.take() else {
            return;
        };
        self.hide_adjust_panel();
        if !adj.contents.is_instance_valid() || adj.contents.bind().get_blob() != adj.last {
            return;
        }
        if adj.last != adj.before {
            self.commit_blob(&adj.moniker, &adj.contents, adj.before, adj.last);
        }
    }
    pub(super) fn adjust_cancel(&mut self) {
        if self.adjust_running {
            return;
        }
        self.stop_adjust_timer();
        let Some(adj) = self.adjust.take() else {
            return;
        };
        self.hide_adjust_panel();
        let mut data = adj.contents;
        if !data.is_instance_valid() || data.bind().get_blob() != adj.last {
            return;
        }
        data.bind_mut().substitute(adj.base);
        data.emit_changed();
        (
            self.sel_faces,
            self.sel_edges,
            self.sel_verts,
            self.active_vert,
        ) = adj.sel;
        self.message = format!("{} cancelled", adj.deed.heading());
        self.sync_ui();
        self.redraw();
    }
    fn stop_adjust_timer(&mut self) {
        if let Some(u) = &mut self.adjust_ui
            && u.timer.is_instance_valid()
        {
            u.timer.stop();
        }
    }
    fn adjust_host() -> Option<Gd<Control>> {
        let vp = EditorInterface::singleton()
            .get_editor_viewport_3d_ex()
            .idx(0)
            .done()?;
        vp.get_parent()?.get_parent()?.try_cast::<Control>().ok()
    }
    fn build_adjust_panel(&mut self) {
        let Some(mut panel) =
            editor_scenes::instantiate::<PanelContainer>(editor_scenes::ADJUST_PANEL)
        else {
            return;
        };
        let parts = (
            editor_scenes::unique::<Label>(&panel, "Heading"),
            editor_scenes::unique::<Button>(&panel, "Cancel"),
            editor_scenes::unique::<Button>(&panel, "Done"),
            editor_scenes::unique::<HFlowContainer>(&panel, "Fields"),
            editor_scenes::unique::<Label>(&panel, "Status"),
            editor_scenes::unique::<Timer>(&panel, "DebounceTimer"),
        );
        let (
            Some(title),
            Some(mut cancel),
            Some(mut done),
            Some(mut flow),
            Some(mut status),
            Some(mut timer),
        ) = parts
        else {
            panel.free();
            return;
        };
        let s = ui_kit::editor_scale();
        panel.add_theme_stylebox_override("panel", &hud_style(s));
        flow.set_custom_minimum_size(Vector2::new(360.0 * s, 0.0));
        status.add_theme_font_size_override("font_size", (12.0 * s) as i32);
        let gd = self.to_gd();
        cancel.connect("pressed", &gd.callable("on_adjust_cancel"));
        done.connect("pressed", &gd.callable("on_adjust_done"));
        timer.connect("timeout", &gd.callable("on_adjust_timer"));
        match Self::adjust_host() {
            Some(mut host) => host.add_child(&panel),
            None => self.base_mut().add_child(&panel),
        }
        self.adjust_ui = Some(AdjustUi {
            panel,
            heading: title,
            flow,
            status,
            timer,
            ctls: Vec::new(),
        });
    }
    fn show_adjust_panel(&mut self) {
        if self
            .adjust_ui
            .as_ref()
            .is_none_or(|u| !u.panel.is_instance_valid())
        {
            self.build_adjust_panel();
        }
        let Some(adj) = &self.adjust else { return };
        let (fields, title) = (adj.fields.clone(), adj.deed.heading());
        let gd = self.to_gd().upcast::<Object>();
        let spec = self.spec;
        let v = self.values_with(&spec);
        let ctls: Vec<(Field, FieldCtl)> = fields
            .iter()
            .map(|f| (*f, ui::make_field(&gd, *f, &v)))
            .collect();
        if let Some(u) = &mut self.adjust_ui {
            for mut c in u.flow.get_children().iter_shared() {
                u.flow.remove_child(&c);
                c.queue_free();
            }
            for (_, c) in &ctls {
                u.flow.add_child(&c.widget());
                if let FieldCtl::Spin(_, sb) = c
                    && let Some(mut le) = sb.get_line_edit()
                {
                    le.connect("text_submitted", &gd.callable("on_adjust_submitted"));
                }
            }
            u.ctls = ctls;
            u.heading.set_text(&format!("Adjust {title}"));
            u.panel.set_visible(true);
            let m = 16.0 * ui_kit::editor_scale();
            u.panel.set_anchors_preset(LayoutPreset::CENTER_BOTTOM);
            u.panel.set_h_grow_direction(GrowDirection::BOTH);
            u.panel.set_v_grow_direction(GrowDirection::BEGIN);
            u.panel.set_offset(Side::LEFT, 0.0);
            u.panel.set_offset(Side::RIGHT, 0.0);
            u.panel.set_offset(Side::TOP, -m);
            u.panel.set_offset(Side::BOTTOM, -m);
            u.panel.reset_size();
        }
        self.sync_adjust_panel();
    }
    fn flush_adjust_text(&mut self) {
        let mut typed: Vec<(Field, f64)> = Vec::new();
        if let Some(u) = &self.adjust_ui
            && u.panel.is_instance_valid()
        {
            for (f, c) in &u.ctls {
                if let FieldCtl::Spin(_, sb) = c
                    && let Some(le) = sb.get_line_edit()
                    && le.has_focus()
                    && let Some(v) = leading_number(&le.get_text().to_string())
                    && (v - sb.get_value()).abs() > 1e-9
                {
                    typed.push((
                        *f,
                        v.clamp(
                            if sb.is_lesser_allowed() {
                                f64::MIN
                            } else {
                                sb.get_min()
                            },
                            if sb.is_greater_allowed() {
                                f64::MAX
                            } else {
                                sb.get_max()
                            },
                        ),
                    ));
                }
            }
        }
        for (f, v) in typed {
            self.on_field(v, f as i64);
        }
    }
    pub(super) fn hide_adjust_panel(&mut self) {
        if let Some(u) = &mut self.adjust_ui
            && u.panel.is_instance_valid()
        {
            u.panel.set_block_signals(true);
            for (_, c) in &mut u.ctls {
                set_ctl_blocked(c, true);
            }
            u.panel.set_visible(false);
            for (_, c) in &mut u.ctls {
                set_ctl_blocked(c, false);
            }
            u.panel.set_block_signals(false);
        }
    }
    pub(super) fn sync_adjust_panel(&mut self) {
        let Some(adj) = &self.adjust else { return };
        let mut status = adj.status.clone();
        if adj.pending {
            status = format!("{status}  …");
        }
        let spec = self.spec;
        let v = self.values_with(&spec);
        if let Some(u) = &mut self.adjust_ui
            && u.panel.is_instance_valid()
        {
            for (f, c) in &mut u.ctls {
                c.sync(*f, &v);
            }
            u.status.set_text(&status);
            u.status.set_visible(!status.is_empty());
            u.panel.reset_size();
        }
    }
    pub(super) fn free_adjust_panel(&mut self) {
        self.hide_adjust_panel();
        if let Some(mut u) = self.adjust_ui.take()
            && u.panel.is_instance_valid()
        {
            for (_, c) in &mut u.ctls {
                set_ctl_blocked(c, true);
            }
            u.panel.queue_free();
        }
    }
    pub(super) fn adjust_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        d.set("active", self.adjust.is_some());
        if let Some(a) = &self.adjust {
            d.set("action", a.deed as i64);
            d.set("pending", a.pending);
            d.set("status", a.status.as_str());
            d.set(
                "fields",
                &PackedInt32Array::from(
                    a.fields
                        .iter()
                        .map(|f| *f as i32)
                        .collect::<Vec<_>>()
                        .as_slice(),
                ),
            );
        }
        let vis = self
            .adjust_ui
            .as_ref()
            .is_some_and(|u| u.panel.is_instance_valid() && u.panel.is_visible());
        d.set("visible", vis);
        if let Some(u) = &self.adjust_ui
            && u.panel.is_instance_valid()
        {
            d.set("status_text", &u.status.get_text());
            d.set("title", &u.heading.get_text());
            d.set("controls", u.ctls.len() as i64);
        }
        d
    }
}
#[godot_api(secondary)]
impl PixelBuilderPlugin {
    #[func]
    fn on_history_changed(&mut self) {
        if self.adjust_running {
            return;
        }
        self.refresh_attr_overlay();
        let Some(adj) = &self.adjust else { return };
        if adj.contents.is_instance_valid() && adj.contents.bind().get_blob() == adj.last {
            return;
        }
        self.stop_adjust_timer();
        self.adjust = None;
        self.hide_adjust_panel();
        self.sync_ui();
        self.redraw();
    }
    #[func]
    fn debug_adjust_type(&mut self, id: i64, text: GString) -> bool {
        let Some(u) = &mut self.adjust_ui else {
            return false;
        };
        for (f, c) in &mut u.ctls {
            if *f as i64 == id
                && let FieldCtl::Spin(_, sb) = c
                && let Some(mut le) = sb.get_line_edit()
            {
                le.grab_focus();
                le.set_text(&text);
                return le.has_focus();
            }
        }
        false
    }
    #[func]
    fn on_adjust_timer(&mut self) {
        if self.adjust.as_ref().is_some_and(|a| a.pending) {
            self.adjust_rerun(false);
        }
    }
    #[func]
    fn on_adjust_submitted(&mut self, _text: GString) {
        if let Some(u) = &self.adjust_ui
            && u.panel.is_instance_valid()
            && let Some(mut vp) = u.panel.get_viewport()
        {
            vp.call_deferred("gui_release_focus", &[]);
        }
    }
    #[func]
    fn on_adjust_done(&mut self) {
        self.adjust_commit();
        self.sync_ui();
        self.redraw();
    }
    #[func]
    fn on_adjust_cancel(&mut self) {
        self.adjust_cancel();
    }
    #[func]
    fn debug_action_adjust(&mut self, token_id: i64) {
        self.upon_deed(token_id);
    }
    #[func]
    fn debug_adjust_state(&self) -> VarDictionary {
        self.adjust_state()
    }
    #[func]
    fn debug_adjust_field(&mut self, token_id: i64, value: f64, flush: bool) {
        self.on_field(value, token_id);
        if flush {
            self.on_adjust_timer();
        }
    }
    #[func]
    fn debug_adjust_end(&mut self, cancel: bool) {
        if cancel {
            self.on_adjust_cancel();
        } else {
            self.on_adjust_done();
        }
    }
}
