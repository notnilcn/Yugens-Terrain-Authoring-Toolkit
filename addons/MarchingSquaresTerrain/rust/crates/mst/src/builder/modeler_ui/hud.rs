//! Layout: the viewport HUD's tool and message content and its Tool Options popup.
//! The adjustment / bevel content is the shared live adjust panel (`plugin/adjust.rs`, both
//! layouts); this HUD stays hidden while it is open, so there is one panel at the bottom of the
//! viewport at a time.
//!
//! The panel sits at the bottom centre of the viewport: bold title, detail line, form, buttons
//! Snap / Options / Cancel / Done, all `FOCUS_NONE`; the *ToolOptions* popup with the tool's form
//! and its commands. Content priority is tool > message, the done button reads "Create" / "Done",
//! and error messages dismiss (informational ones stay in the overlay status lines). Tool Options shows the strip fields the tool uses (`ui::field_visible`,
//! the MST strip's rule).

use godot::builtin::Side;
use godot::classes::control::{GrowDirection, LayoutPreset, MouseFilter};
use godot::classes::{
    Button, Control, EditorInterface, HBoxContainer, HFlowContainer, Label, OptionButton,
    PanelContainer, PopupPanel, StyleBoxFlat, VBoxContainer,
};
use godot::prelude::*;
use pt_core::build::ShapeKind;

use super::super::forms::{FieldForm, Row};
use super::super::ui::{self, Field, SyncState, Tool, Values};
use super::toolbar::editor_icon;
use crate::editor::ui_kit;

/// HUD button commands (`on_xg_hud`).
pub const DONE: i64 = 0;
pub const CANCEL: i64 = 1;
pub const OPTIONS: i64 = 2;
pub const REMOVE_POINT: i64 = 4;

/// What the HUD shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Content {
    pub heading: String,
    pub detail: String,
    pub error: bool,
    /// Tool content (Snap / Options / Cancel / Done) vs message (Dismiss).
    pub tool: bool,
    pub done: String,
}

/// Tool (any drawing tool) > error message; none in the Select tool without an error message
/// (informational messages stay in the top-left overlay status) or while the adjust panel is
/// open.
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
        // Informational messages ("Created Box", layout switches) already show in the
        // top-left overlay status lines; a card at the bottom centre would cover the model
        // and swallow clicks on the faces under it. Only errors get the dismissable card.
        if !error {
            return None;
        }
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

/// The per-tool instruction when the tool has no status line yet.
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
    sort: Gd<OptionButton>,
    pub tool_form: FieldForm,
    remove_point: Gd<Button>,
    shown: Option<Content>,
    /// The editor 3D viewport (0..3) the HUD floats in: the one that last had input.
    viewport: i32,
}

fn style(scale: f32) -> Gd<StyleBoxFlat> {
    // Base colour α 0.9405, 1 px dark border, radius 6, margin 10, 4 px shadow.
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

/// The viewport control (`Node3DEditorViewport`) of editor 3D viewport `idx` (0..3).
pub fn host(idx: i32) -> Option<Gd<Control>> {
    let vp = EditorInterface::singleton()
        .get_editor_viewport_3d_ex()
        .idx(idx)
        .done()?;
    vp.get_parent()?.get_parent()?.try_cast::<Control>().ok()
}

/// Which editor 3D viewport (0..3) renders through `cam` (the HUD follows the camera of the
/// last `forward_3d_gui_input`).
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

pub fn assemble(addressee: &Gd<Object>) -> Hud {
    let s = ui_kit::editor_scale();
    let mut panel = PanelContainer::new_alloc();
    panel.set_name("ModelerCoreHUD");
    panel.add_theme_stylebox_override("panel", &style(s));
    panel.set_mouse_filter(MouseFilter::STOP);
    let mut col = VBoxContainer::new_alloc();
    col.set_name("Column");
    col.add_theme_constant_override("separation", (6.0 * s) as i32);
    let mut title = Label::new_alloc();
    title.set_name("Title");
    if let Some(f) = EditorInterface::singleton()
        .get_editor_theme()
        .and_then(|t| t.get_font("bold", "EditorFonts"))
    {
        title.add_theme_font_override("font", &f);
    }
    col.add_child(&title);
    let mut detail = Label::new_alloc();
    detail.set_name("Detail");
    detail.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
    detail.set_custom_minimum_size(Vector2::new(280.0 * s, 0.0));
    col.add_child(&detail);
    let mut buttons = HBoxContainer::new_alloc();
    buttons.set_name("Buttons");
    buttons.set_alignment(godot::classes::box_container::AlignmentMode::END);
    let mk = |name: &str, text: &str, cmd: i64| {
        let mut b = Button::new_alloc();
        b.set_name(name);
        b.set_text(text);
        b.set_focus_mode(godot::classes::control::FocusMode::NONE);
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_xg_hud").bind(&[cmd.to_variant()]),
        );
        b
    };
    let mut snap = Button::new_alloc();
    snap.set_name("Snap");
    snap.set_toggle_mode(true);
    if let Some(t) = editor_icon("Snap") {
        snap.set_button_icon(&t);
    }
    snap.set_tooltip_text("Snap");
    snap.set_focus_mode(godot::classes::control::FocusMode::NONE);
    snap.connect(
        "pressed",
        &Callable::from_object_method(addressee, "upon_deed")
            .bind(&[(ui::Action::SnapGrid as i64).to_variant()]),
    );
    buttons.add_child(&snap);
    let options = mk("Options", "Options", OPTIONS);
    buttons.add_child(&options);
    let cancel = mk("Cancel", "Cancel", CANCEL);
    buttons.add_child(&cancel);
    let done = mk("Done", "Done", DONE);
    buttons.add_child(&done);
    col.add_child(&buttons);
    panel.add_child(&col);
    // Tool Options popup: the shape kind, the tool's strip fields and the point commands.
    let mut tool_options = PopupPanel::new_alloc();
    tool_options.set_name("ToolOptions");
    let mut tcol = VBoxContainer::new_alloc();
    let mut krow = HBoxContainer::new_alloc();
    let mut kl = Label::new_alloc();
    kl.set_text("Shape");
    kl.set_custom_minimum_size(Vector2::new(150.0 * s, 0.0));
    krow.add_child(&kl);
    let mut kind = OptionButton::new_alloc();
    for k in ShapeKind::ALL {
        kind.add_item(k.moniker());
    }
    kind.set_focus_mode(godot::classes::control::FocusMode::NONE);
    kind.connect(
        "item_selected",
        &Callable::from_object_method(addressee, "on_kind"),
    );
    krow.add_child(&kind);
    tcol.add_child(&krow);
    let tool_form = FieldForm::new("ToolForm");
    tcol.add_child(&tool_form.base);
    let mut cmds = HFlowContainer::new_alloc();
    cmds.set_name("Commands");
    let remove_point = mk("RemovePoint", "Remove Point", REMOVE_POINT);
    cmds.add_child(&remove_point);
    tcol.add_child(&cmds);
    tool_options.add_child(&tcol);
    panel.add_child(&tool_options);
    panel.set_visible(false);
    if let Some(mut h) = host(0) {
        h.add_child(&panel);
    }
    Hud {
        panel,
        heading: title,
        detail,
        snap,
        choices: options,
        abort: cancel,
        done,
        tool_options,
        sort: kind,
        tool_form,
        remove_point,
        shown: None,
        viewport: 0,
    }
}

/// Every strip field a drawing tool can show (the Tool Options form's rows; the visible ones
/// follow `ui::field_visible`).
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

/// Whether a Tool Options row shows: the strip's rule (`field_visible`, plus the Smooth / Arched
/// checks).
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
    /// Shows `c` (None hides the HUD). Rebuilds only when the content changed.
    pub fn show(&mut self, c: Option<Content>, st: &SyncState) {
        if !self.panel.is_instance_valid() {
            return;
        }
        self.snap.set_pressed_no_signal(st.snap_grid);
        if c == self.shown {
            return;
        }
        self.shown = c.clone();
        // Parented lazily: at editor start-up the 3D viewport may not exist yet.
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

    /// The viewport the HUD is in.
    pub fn viewport(&self) -> i32 {
        self.viewport
    }

    /// Moves the HUD to editor 3D viewport `idx` (change-only: called when the input camera
    /// changes). Only the panel's parent changes; its content and anchors stay.
    pub fn set_viewport(&mut self, idx: i32) {
        if idx == self.viewport || !self.panel.is_instance_valid() {
            return;
        }
        let Some(mut h) = host(idx) else { return };
        self.viewport = idx;
        if let Some(mut p) = self.panel.get_parent() {
            // Runs inside the plugin's `forward_3d_gui_input`: a Tool Options SpinBox holding
            // typed text applies it on focus loss (hide / reparent), which would emit back into
            // the bound plugin. Its signals are blocked for the move (the text is dropped, R3).
            let tree = self.panel.clone().upcast::<Node>();
            ui_kit::set_block_signals_tree(&tree, true);
            // The Tool Options popup is the panel's child: close it rather than move it open.
            self.tool_options.hide();
            p.remove_child(&self.panel);
            h.add_child(&self.panel);
            ui_kit::set_block_signals_tree(&tree, false);
        }
    }

    /// Whether the Done button means Dismiss (message content).
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

    /// Opens / refreshes the Tool Options popup above the HUD.
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
        if let Some(mut p) = self
            .sort
            .get_parent()
            .and_then(|p| p.try_cast::<Control>().ok())
        {
            p.set_visible(st.tool == Tool::Shape);
        }
        self.remove_point
            .set_visible(matches!(st.tool, Tool::Bezier | Tool::Knife | Tool::Shape));
        self.tool_options.reset_size();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(message: &str) -> SyncState {
        SyncState {
            visible: true,
            message: message.into(),
            ..SyncState::default()
        }
    }

    /// Informational messages stay in the overlay status lines; only errors get the card.
    #[test]
    fn only_error_messages_get_the_card() {
        assert_eq!(payload(&state("Created Box"), ShapeKind::Box, false), None);
        assert_eq!(payload(&state(""), ShapeKind::Box, false), None);
        let c = payload(
            &state("Poly shape: cannot triangulate"),
            ShapeKind::Box,
            false,
        )
        .expect("error card");
        assert!(c.error && !c.tool);
        // A drawing tool keeps its panel.
        let mut st = state("Created Box");
        st.tool = Tool::Shape;
        assert!(payload(&st, ShapeKind::Box, false).is_some_and(|c| c.tool));
    }
}
