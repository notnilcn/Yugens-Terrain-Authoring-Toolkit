//! Drives the heads-up panel the builder shows over the 3D viewport while a tool is active.
//!
//! The panel is instantiated from its scene when the author picks this layout and freed
//! with the layout, because it lives in the editor's viewport rather than in any scene
//! being edited. The tool settings it offers are generated from the builder's field
//! tables and narrowed to whichever tool is active.
use super::super::forms::{FieldForm, Row};
use super::super::plugin::editor_scenes;
use super::super::ui::{self, Field, SyncState, Tool, Values};
use super::assigned::assigned;
use super::hud_panel::PixelModelerHud;
use super::toolbar::editor_icon;
use crate::editor::ui_kit;
use godot::builtin::Side;
use godot::classes::control::{GrowDirection, LayoutPreset};
use godot::classes::{
    Button, Control, EditorInterface, HBoxContainer, Label, OptionButton, PanelContainer,
    PopupPanel, StyleBoxFlat, VBoxContainer,
};
use godot::prelude::*;
use pt_modeler::ShapeKind;
pub const DONE: i64 = 0;
pub const CANCEL: i64 = 1;
pub const OPTIONS: i64 = 2;
pub const REMOVE_POINT: i64 = 4;
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Content {
    pub heading: String,
    pub detail: String,
    pub error: bool,
    pub tool: bool,
    pub done: String,
}
pub fn payload(st: &SyncState, kind: ShapeKind, bezier_block: bool) -> Option<Content> {
    if st.adjusting || !st.visible {
        return None;
    }
    let tool_title = match st.tool {
        Tool::Shape if kind == ShapeKind::Poly => Some("Poly Shape".to_string()),
        Tool::Shape => Some(kind.moniker().to_string()),
        Tool::Bezier => Some(
            if bezier_block {
                "Bezier Block"
            } else {
                "Bezier Plane"
            }
            .to_string(),
        ),
        Tool::Knife => Some("Cut".into()),
        Tool::Paint => Some("Paint".into()),
        Tool::VertexPaint => Some("Vertex Paint".into()),
        Tool::Place => Some("Place on Cell".into()),
        Tool::EditPivot => Some("Edit Pivot".into()),
        Tool::Select => None,
    };
    if let Some(title) = tool_title {
        let detail = if st.tool_detail.is_empty() {
            tool_hint(st.tool).to_string()
        } else {
            st.tool_detail.clone()
        };
        let done = if matches!(st.tool, Tool::Shape | Tool::Bezier) {
            "Create"
        } else {
            "Done"
        };
        return Some(Content {
            heading: title,
            detail,
            error: false,
            tool: true,
            done: done.into(),
        });
    }
    if !st.message.is_empty() {
        let error = [
            "fail",
            "cannot",
            "could not",
            "select ",
            "refused",
            "nothing",
        ]
        .iter()
        .any(|w| st.message.to_lowercase().contains(w));
        return Some(Content {
            heading: "Modeler".into(),
            detail: st.message.clone(),
            error,
            tool: false,
            done: String::new(),
        });
    }
    None
}
fn tool_hint(t: Tool) -> &'static str {
    match t {
        Tool::Shape => {
            "Drag on the grid for the footprint, move up for the height, click to finish. A plain click stamps the shape."
        }
        Tool::Bezier => {
            "Click to add knots, drag knots and tangents. Create (Enter) builds the mesh."
        }
        Tool::Knife => "Click a face, then points on it; click the first point or Done to cut.",
        Tool::Paint => "Click or drag over faces to paint the current slot and colour.",
        Tool::VertexPaint => "Brush per-corner colours; Shift+click picks a colour.",
        Tool::Place => "Click a terrain cell to seat the mesh on it.",
        Tool::EditPivot => "Drag the pivot handles; the part stays where it is.",
        Tool::Select => "",
    }
}
pub struct Hud {
    pub panel: Gd<PanelContainer>,
    heading: Gd<Label>,
    detail: Gd<Label>,
    snap: Gd<Button>,
    choices: Gd<Button>,
    abort: Gd<Button>,
    done: Gd<Button>,
    pub tool_options: Gd<PopupPanel>,
    shape_row: Gd<HBoxContainer>,
    sort: Gd<OptionButton>,
    pub tool_form: FieldForm,
    remove_point: Gd<Button>,
    shown: Option<Content>,
    viewport: i32,
}
fn style(scale: f32) -> Gd<StyleBoxFlat> {
    let mut st = StyleBoxFlat::new_gd();
    let theme = EditorInterface::singleton().get_editor_theme();
    let mut bg = theme
        .as_ref()
        .map(|t| t.get_color("base_color", "Editor"))
        .unwrap_or(Color::from_rgb(0.1305, 0.1505, 0.1805));
    bg.a = 0.9405;
    st.set_bg_color(bg);
    st.set_border_color(
        theme
            .as_ref()
            .map(|t| t.get_color("dark_color_3", "Editor"))
            .unwrap_or(Color::from_rgb(0.0805, 0.0805, 0.1005)),
    );
    st.set_border_width_all(1);
    st.set_corner_radius_all((6.0 * scale) as i32);
    st.set_content_margin_all(10.0 * scale);
    st.set_shadow_color(Color::from_rgba(0.0, 0.0, 0.0, 0.2505));
    st.set_shadow_size((4.0 * scale) as i32);
    st
}
pub fn host(idx: i32) -> Option<Gd<Control>> {
    let vp = EditorInterface::singleton()
        .get_editor_viewport_3d_ex()
        .idx(idx)
        .done()?;
    vp.get_parent()?.get_parent()?.try_cast::<Control>().ok()
}
pub fn viewport_of(cam: &Gd<godot::classes::Camera3D>) -> Option<i32> {
    let ei = EditorInterface::singleton();
    (0..4).find(|i| {
        ei.get_editor_viewport_3d_ex()
            .idx(*i)
            .done()
            .and_then(|v| v.get_camera_3d())
            .is_some_and(|c| c == *cam)
    })
}
const SCENE: &str = "modeler_hud.tscn";
pub fn assemble(addressee: &Gd<Object>) -> Option<Hud> {
    let mut panel = editor_scenes::instantiate::<PixelModelerHud>(SCENE)?;
    let parts = HudParts::of(&panel);
    let Some(parts) = parts else {
        panel.free();
        return None;
    };
    let HudParts {
        mut column,
        mut heading,
        mut detail,
        mut snap,
        mut choices,
        mut abort,
        mut done,
        tool_options,
        shape_row,
        mut shape_caption,
        mut sort,
        mut tool_form_holder,
        mut remove_point,
    } = parts;
    let s = ui_kit::editor_scale();
    panel.add_theme_stylebox_override("panel", &style(s));
    column.add_theme_constant_override("separation", (6.0 * s) as i32);
    if let Some(f) = EditorInterface::singleton()
        .get_editor_theme()
        .and_then(|t| t.get_font("bold", "EditorFonts"))
    {
        heading.add_theme_font_override("font", &f);
    }
    detail.set_custom_minimum_size(Vector2::new(280.0 * s, 0.0));
    if let Some(t) = editor_icon("Snap") {
        snap.set_button_icon(&t);
    }
    snap.connect(
        "pressed",
        &Callable::from_object_method(addressee, "upon_deed")
            .bind(&[(ui::Action::SnapGrid as i64).to_variant()]),
    );
    for (b, cmd) in [
        (&mut choices, OPTIONS),
        (&mut abort, CANCEL),
        (&mut done, DONE),
        (&mut remove_point, REMOVE_POINT),
    ] {
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_xg_hud").bind(&[cmd.to_variant()]),
        );
    }
    shape_caption.set_custom_minimum_size(Vector2::new(150.0 * s, 0.0));
    for k in ShapeKind::ALL {
        sort.add_item(k.moniker());
    }
    sort.connect(
        "item_selected",
        &Callable::from_object_method(addressee, "on_kind"),
    );
    let tool_form = FieldForm::new("ToolForm");
    tool_form_holder.add_child(&tool_form.base);
    if let Some(mut h) = host(0) {
        h.add_child(&panel);
    }
    Some(Hud {
        panel: panel.upcast(),
        heading,
        detail,
        snap,
        choices,
        abort,
        done,
        tool_options,
        shape_row,
        sort,
        tool_form,
        remove_point,
        shown: None,
        viewport: 0,
    })
}
struct HudParts {
    column: Gd<VBoxContainer>,
    heading: Gd<Label>,
    detail: Gd<Label>,
    snap: Gd<Button>,
    choices: Gd<Button>,
    abort: Gd<Button>,
    done: Gd<Button>,
    tool_options: Gd<PopupPanel>,
    shape_row: Gd<HBoxContainer>,
    shape_caption: Gd<Label>,
    sort: Gd<OptionButton>,
    tool_form_holder: Gd<VBoxContainer>,
    remove_point: Gd<Button>,
}
impl HudParts {
    fn of(panel: &Gd<PixelModelerHud>) -> Option<HudParts> {
        let p = panel.bind();
        Some(HudParts {
            column: assigned(&p.column, SCENE, "column")?,
            heading: assigned(&p.heading, SCENE, "heading")?,
            detail: assigned(&p.detail, SCENE, "detail")?,
            snap: assigned(&p.snap, SCENE, "snap")?,
            choices: assigned(&p.options, SCENE, "options")?,
            abort: assigned(&p.cancel, SCENE, "cancel")?,
            done: assigned(&p.done, SCENE, "done")?,
            tool_options: assigned(&p.tool_options, SCENE, "tool_options")?,
            shape_row: assigned(&p.shape_row, SCENE, "shape_row")?,
            shape_caption: assigned(&p.shape_caption, SCENE, "shape_caption")?,
            sort: assigned(&p.shape, SCENE, "shape")?,
            tool_form_holder: assigned(&p.tool_form_holder, SCENE, "tool_form_holder")?,
            remove_point: assigned(&p.remove_point, SCENE, "remove_point")?,
        })
    }
}
pub fn tool_rows() -> Vec<Row> {
    let mut rows: Vec<Row> = Field::ALL
        .iter()
        .copied()
        .filter(|f| ui::is_tool_field(*f))
        .map(Row::Field)
        .collect();
    rows.push(Row::ShapeSmooth);
    rows.push(Row::ShapeArched);
    rows
}
pub fn tool_row_visible(r: &Row, v: &Values, st: &SyncState) -> bool {
    let k = v.spec.sort;
    match r {
        Row::Field(f) => ui::field_visible(*f, v, st),
        Row::ShapeSmooth => {
            st.shape_fields() && (k.uses().3 || (k.uses_modeler().arch_band && v.spec.arch_band))
        }
        Row::ShapeArched => st.shape_fields() && k.uses_extra().1,
        _ => false,
    }
}
impl Hud {
    pub fn show(&mut self, c: Option<Content>, st: &SyncState) {
        if !self.panel.is_instance_valid() {
            return;
        }
        self.snap.set_pressed_no_signal(st.snap_grid);
        if c == self.shown {
            return;
        }
        self.shown = c.clone();
        if c.is_some()
            && self.panel.get_parent().is_none()
            && let Some(mut h) = host(self.viewport)
        {
            h.add_child(&self.panel);
        }
        let Some(c) = c else {
            self.panel.set_visible(false);
            return;
        };
        if let Ok(mut card) = self.panel.clone().try_cast::<PixelModelerHud>() {
            let mut card = card.bind_mut();
            if c.tool {
                card.showing_tool();
            } else if !card.admit_result(&c.detail) {
                drop(card);
                self.panel.set_visible(false);
                self.shown = None;
                return;
            }
        }
        self.heading.set_text(&c.heading);
        self.detail.set_text(&c.detail);
        let err = EditorInterface::singleton()
            .get_editor_theme()
            .map(|t| t.get_color("error_color", "Editor"))
            .unwrap_or(Color::from_rgb(1.0, 0.4505, 0.4005));
        if c.error {
            self.detail.add_theme_color_override("font_color", err);
        } else {
            self.detail.remove_theme_color_override("font_color");
        }
        self.snap.set_visible(c.tool);
        self.choices.set_visible(c.tool);
        self.abort.set_visible(c.tool);
        self.done
            .set_text(if c.tool { c.done.as_str() } else { "Dismiss" });
        self.panel.set_visible(true);
        let m = 16.0 * ui_kit::editor_scale();
        self.panel.set_anchors_preset(LayoutPreset::CENTER_BOTTOM);
        self.panel.set_h_grow_direction(GrowDirection::BOTH);
        self.panel.set_v_grow_direction(GrowDirection::BEGIN);
        self.panel.set_offset(Side::LEFT, 0.0);
        self.panel.set_offset(Side::RIGHT, 0.0);
        self.panel.set_offset(Side::TOP, -m);
        self.panel.set_offset(Side::BOTTOM, -m);
        self.panel.reset_size();
    }
    pub fn viewport(&self) -> i32 {
        self.viewport
    }
    pub fn set_viewport(&mut self, idx: i32) {
        if idx == self.viewport || !self.panel.is_instance_valid() {
            return;
        }
        let Some(mut h) = host(idx) else { return };
        self.viewport = idx;
        if let Some(mut p) = self.panel.get_parent() {
            let tree = self.panel.clone().upcast::<Node>();
            ui_kit::set_block_signals_tree(&tree, true);
            self.tool_options.hide();
            p.remove_child(&self.panel);
            h.add_child(&self.panel);
            ui_kit::set_block_signals_tree(&tree, false);
        }
    }
    pub fn is_message(&self) -> bool {
        self.shown.as_ref().is_some_and(|c| !c.tool)
    }
    pub fn shown(&self) -> Option<&Content> {
        self.shown.as_ref()
    }
    pub fn button_texts(&self) -> Vec<String> {
        [&self.snap, &self.choices, &self.abort, &self.done]
            .iter()
            .filter(|b| b.is_visible())
            .map(|b| {
                if b.get_text().is_empty() {
                    "Snap".to_string()
                } else {
                    b.get_text().to_string()
                }
            })
            .collect()
    }
    pub fn unveil_instrument_choices(
        &mut self,
        addressee: &Gd<Object>,
        v: &Values,
        st: &SyncState,
    ) {
        self.renew_instrument_choices(addressee, v, st);
        let r =
            self.panel.get_screen_transform() * Rect2::new(Vector2::ZERO, self.panel.get_size());
        self.tool_options.reset_size();
        let h = self.tool_options.get_size().y;
        self.tool_options
            .popup_ex()
            .rect(Rect2i::new(
                Vector2i::new(r.position.x as i32, (r.position.y as i32 - h).max(0)),
                Vector2i::ZERO,
            ))
            .done();
    }
    pub fn renew_instrument_choices(&mut self, addressee: &Gd<Object>, v: &Values, st: &SyncState) {
        let rows = tool_rows();
        self.tool_form.set_rows(addressee, &rows, v, st);
        self.tool_form.show_where(|r| tool_row_visible(r, v, st));
        self.sort.select(v.spec.sort as i32);
        self.shape_row.set_visible(st.tool == Tool::Shape);
        self.remove_point
            .set_visible(matches!(st.tool, Tool::Bezier | Tool::Knife | Tool::Shape));
        self.tool_options.reset_size();
    }
}
