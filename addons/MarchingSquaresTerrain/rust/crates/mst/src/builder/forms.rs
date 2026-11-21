//! A form of labelled settings whose rows are chosen at run time.
//!
//! The builder's popovers and dialogs show different settings depending on the tool, the
//! shape and the category the author is looking at, so the rows and their controls are
//! generated from a list of settings each time that list changes, instead of being laid
//! out once.
use super::layout::Layout;
use super::prefs::{self, Key};
use super::ui::{Action, Field, SyncState, UvField, Values};
use crate::editor::ui_kit;
use godot::classes::control::{FocusMode, MouseFilter, SizeFlags};
use godot::classes::{
    Button, CheckBox, Control, GridContainer, HBoxContainer, Label, OptionButton, SpinBox,
};
use godot::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum MeshProp {
    LiveUv2 = 0,
    ClearGrass = 1,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Field(Field),
    Toggle(Action, &'static str),
    Pref(Key),
    Uv(UvField),
    UvWorld,
    UvButtons,
    Mesh(MeshProp, &'static str),
    Layout,
    ShapeSmooth,
    ShapeArched,
}
pub fn field_label(f: Field) -> &'static str {
    match f {
        Field::Distance => "Distance",
        Field::ExtrudeMethod => "Method",
        Field::EdgesGroup => "Edges as Group",
        Field::Inset => "Inset Width",
        Field::InsetMode => "Inset By",
        Field::InsetIndividual => "Individual Faces",
        Field::Bevel => "Amount",
        Field::BevelAmountType => "Amount Type",
        Field::BevelSegs => "Segments",
        Field::BevelProfile => "Profile",
        Field::BevelClamp => "Clamp Overlap",
        Field::BevelHarden => "Harden Normals",
        Field::WeldDist => "Distance",
        Field::CollapseFirst => "Collapse to First Vertex",
        Field::Cuts => "Subdivisions",
        Field::SubdivSegs => "Face Subdivisions",
        Field::FillEntire => "Fill Entire Hole",
        Field::ToNewObject => "Create New Mesh Object",
        Field::OffsetX => "Offset X",
        Field::OffsetY => "Offset Y",
        Field::OffsetZ => "Offset Z",
        Field::OffsetSpace => "Space",
        Field::OffsetPerFace => "Per Face",
        Field::MirrorX => "X",
        Field::MirrorY => "Y",
        Field::MirrorZ => "Z",
        Field::MirrorDup => "Duplicate",
        Field::GrowAngle => "Restrict by Angle (°)",
        Field::SoftRadius => "Radius",
        Field::Falloff => "Curve",
        Field::GridStep => "Snap Increment",
        Field::GizmoOrient => "Orientation",
        Field::RotSnap => "Rotate Snap (°)",
        Field::Profile => "Trim Profile",
        Field::TrimPlace => "Trim At",
        Field::TrimDepth => "Trim Depth",
        Field::TrimHeight => "Trim Height",
        Field::SmoothTint => "Tint Faces by Smoothing Group",
        Field::ShowNormals => "Show Normals",
        Field::NormalLength => "Normal Length",
        _ => f.portray().0,
    }
}
pub fn row_label(r: &Row) -> String {
    match r {
        Row::Field(f) => field_label(*f).to_string(),
        Row::Toggle(_, t) | Row::Mesh(_, t) => t.to_string(),
        Row::Pref(k) => k.portray().0.to_string(),
        Row::Uv(u) => u.portray().0.to_string(),
        Row::UvWorld => "World Aligned".into(),
        Row::UvButtons => "Face UVs".into(),
        Row::Layout => "UI Layout".into(),
        Row::ShapeSmooth => "Smooth".into(),
        Row::ShapeArched => "Arched".into(),
    }
}
fn row_tip(r: &Row) -> String {
    match r {
        Row::Field(f) => f.portray().4.to_string(),
        Row::Toggle(a, _) => super::ui::action_tooltip(*a),
        Row::Pref(k) => k.portray().1.to_string(),
        Row::Uv(_) | Row::UvWorld | Row::UvButtons => "Automatic box projection per face (the palette's Face UV): selecting one face loads its values; Apply writes them to the selected faces".into(),
        Row::Mesh(MeshProp::LiveUv2, _) => "Live UV2 of the edited builder mesh: lightmap UV2 unwrapped half a second after each edit (texel: Lightmap Texel)".into(),
        Row::Mesh(MeshProp::ClearGrass, _) => "Clear grass of the edited builder mesh: terrain grass under its footprint is removed".into(),
        Row::Layout => super::layout::TOOLTIP.into(),
        Row::ShapeSmooth => "Smooth shading for round shapes".into(),
        Row::ShapeArched => "Round-topped doorway".into(),
    }
}
enum Ctl {
    Spin(Gd<SpinBox>),
    Option(Gd<OptionButton>),
    Check(Gd<CheckBox>),
    Pref(Gd<Control>),
    None,
}
pub struct FieldForm {
    pub base: Gd<GridContainer>,
    ranks: Vec<(Row, Gd<Label>, Gd<Control>, Ctl)>,
}
fn spin(min: f64, max: f64, step: f64) -> Gd<SpinBox> {
    let mut sb = SpinBox::new_alloc();
    sb.set_min(min);
    sb.set_max(max);
    sb.set_step(step);
    sb.set_custom_minimum_size(Vector2::new(96.0 * ui_kit::editor_scale(), 0.0));
    sb
}
impl FieldForm {
    pub fn new(name: &str) -> FieldForm {
        let mut root = GridContainer::new_alloc();
        root.set_name(name);
        root.set_columns(2);
        root.set_h_size_flags(SizeFlags::EXPAND_FILL);
        root.add_theme_constant_override("v_separation", (4.0 * ui_kit::editor_scale()) as i32);
        FieldForm {
            base: root,
            ranks: Vec::new(),
        }
    }
    pub fn ranks(&self) -> Vec<Row> {
        self.ranks.iter().map(|r| r.0).collect()
    }
    pub fn labels(&self) -> Vec<String> {
        self.ranks
            .iter()
            .filter(|r| r.1.is_visible())
            .map(|r| r.1.get_text().to_string())
            .collect()
    }
    pub fn set_rows(&mut self, addressee: &Gd<Object>, rows: &[Row], v: &Values, st: &SyncState) {
        if self.ranks() != rows {
            for mut c in self.base.get_children().iter_shared() {
                self.base.remove_child(&c);
                c.queue_free();
            }
            self.ranks.clear();
            for r in rows {
                let (label, ctl, kind) = build_row(addressee, r, v, st);
                self.base.add_child(&label);
                self.base.add_child(&ctl);
                self.ranks.push((*r, label, ctl, kind));
            }
        }
        self.sync(v, st);
    }
    pub fn sync(&mut self, v: &Values, st: &SyncState) {
        for (r, _, _, c) in &mut self.ranks {
            match (r, c) {
                (Row::Field(f), Ctl::Spin(sb)) => sb.set_value_no_signal(v.fetch(*f)),
                (Row::Field(f), Ctl::Option(ob)) => ob.select(v.fetch(*f) as i32),
                (Row::Field(f), Ctl::Check(cb)) => cb.set_pressed_no_signal(v.fetch(*f) >= 0.5),
                (Row::Toggle(a, _), Ctl::Check(cb)) => {
                    cb.set_pressed_no_signal(super::ui::action_pressed(*a, v, st).unwrap_or(false))
                }
                (Row::Pref(k), Ctl::Pref(c)) => prefs::sync_control(c, *k, &st.prefs),
                (Row::Uv(u), Ctl::Spin(sb)) => {
                    sb.set_value_no_signal(v.tex_coord[*u as usize] as f64)
                }
                (Row::UvWorld, Ctl::Check(cb)) => cb.set_pressed_no_signal(v.uv_world),
                (Row::Mesh(p, _), Ctl::Check(cb)) => {
                    cb.set_pressed_no_signal(match p {
                        MeshProp::LiveUv2 => st.live_uv2,
                        MeshProp::ClearGrass => st.clear_grass,
                    });
                    cb.set_disabled(!st.has_target);
                }
                (Row::Layout, Ctl::Option(ob)) => ob.select(Layout::ingest() as i32),
                (Row::ShapeSmooth, Ctl::Check(cb)) => cb.set_pressed_no_signal(v.spec.glide),
                (Row::ShapeArched, Ctl::Check(cb)) => cb.set_pressed_no_signal(v.spec.arched),
                _ => {}
            }
        }
    }
    pub fn sift(&mut self, query: &str) {
        let q = query.trim().to_lowercase();
        for (r, l, c, _) in &mut self.ranks {
            let show = q.is_empty() || row_label(r).to_lowercase().contains(&q);
            l.set_visible(show);
            c.set_visible(show);
        }
    }
    pub fn show_where(&mut self, keep: impl Fn(&Row) -> bool) {
        for (r, l, c, _) in &mut self.ranks {
            let show = keep(r);
            l.set_visible(show);
            c.set_visible(show);
        }
    }
}
fn build_row(
    addressee: &Gd<Object>,
    r: &Row,
    v: &Values,
    st: &SyncState,
) -> (Gd<Label>, Gd<Control>, Ctl) {
    let s = ui_kit::editor_scale();
    let mut label = Label::new_alloc();
    label.set_text(&row_label(r));
    let tip = row_tip(r);
    label.set_tooltip_text(&tip);
    label.set_mouse_filter(MouseFilter::PASS);
    label.set_custom_minimum_size(Vector2::new(150.0 * s, 0.0));
    let check = |pressed: bool| {
        let mut cb = CheckBox::new_alloc();
        cb.set_focus_mode(FocusMode::NONE);
        cb.set_pressed_no_signal(pressed);
        cb
    };
    let (mut ctl, kind): (Gd<Control>, Ctl) = match r {
        Row::Field(f) => {
            let id = (*f as i64).to_variant();
            let (_, min, max, step, _) = f.portray();
            if f.is_check() {
                let mut cb = check(v.fetch(*f) >= 0.5);
                cb.connect(
                    "toggled",
                    &Callable::from_object_method(addressee, "on_field_check").bind(&[id]),
                );
                (cb.clone().upcast(), Ctl::Check(cb))
            } else if let Some(choices) = f.choices() {
                let mut ob = OptionButton::new_alloc();
                for c in choices {
                    ob.add_item(*c);
                }
                ob.select(v.fetch(*f) as i32);
                ob.set_focus_mode(FocusMode::NONE);
                ob.connect(
                    "item_selected",
                    &Callable::from_object_method(addressee, "on_field_option").bind(&[id]),
                );
                (ob.clone().upcast(), Ctl::Option(ob))
            } else {
                let mut sb = spin(min, max, step);
                sb.set_allow_greater(matches!(
                    f,
                    Field::SizeX | Field::SizeY | Field::SizeZ | Field::Distance | Field::Bevel
                ));
                sb.set_value_no_signal(v.fetch(*f));
                sb.connect(
                    "value_changed",
                    &Callable::from_object_method(addressee, "on_field").bind(&[id]),
                );
                (sb.clone().upcast(), Ctl::Spin(sb))
            }
        }
        Row::Toggle(a, _) => {
            let mut cb = check(super::ui::action_pressed(*a, v, st).unwrap_or(false));
            cb.connect(
                "toggled",
                &Callable::from_object_method(addressee, "on_xg_toggle")
                    .bind(&[(*a as i64).to_variant()]),
            );
            (cb.clone().upcast(), Ctl::Check(cb))
        }
        Row::Pref(k) => {
            let c = prefs::fabricate_widget(addressee, *k, &st.prefs);
            (c.clone(), Ctl::Pref(c))
        }
        Row::Uv(u) => {
            let (_, min, max, step) = u.portray();
            let mut sb = spin(min, max, step);
            sb.set_allow_greater(true);
            sb.set_allow_lesser(true);
            sb.set_value_no_signal(v.tex_coord[*u as usize] as f64);
            sb.connect(
                "value_changed",
                &Callable::from_object_method(addressee, "on_uv_field")
                    .bind(&[(*u as i64).to_variant()]),
            );
            (sb.clone().upcast(), Ctl::Spin(sb))
        }
        Row::UvWorld => {
            let mut cb = check(v.uv_world);
            cb.connect(
                "toggled",
                &Callable::from_object_method(addressee, "on_uv_world"),
            );
            (cb.clone().upcast(), Ctl::Check(cb))
        }
        Row::UvButtons => {
            let mut hb = HBoxContainer::new_alloc();
            for (text, method, tip) in [
                (
                    "Apply",
                    "on_uv_apply",
                    "Apply the UV values to the selected faces (all faces in Object mode)",
                ),
                ("Reset", "on_uv_reset", "Reset the UV values (and apply)"),
                (
                    "↻ 90°",
                    "on_uv_rotate",
                    "Rotate the selected faces' UVs by 90°",
                ),
            ] {
                let mut b = Button::new_alloc();
                b.set_text(text);
                b.set_tooltip_text(tip);
                b.set_focus_mode(FocusMode::NONE);
                b.connect("pressed", &Callable::from_object_method(addressee, method));
                hb.add_child(&b);
            }
            (hb.upcast(), Ctl::None)
        }
        Row::Mesh(p, _) => {
            let mut cb = check(false);
            cb.connect(
                "toggled",
                &Callable::from_object_method(addressee, "on_xg_mesh_prop")
                    .bind(&[(*p as i64).to_variant()]),
            );
            (cb.clone().upcast(), Ctl::Check(cb))
        }
        Row::ShapeSmooth | Row::ShapeArched => {
            let smooth = *r == Row::ShapeSmooth;
            let mut cb = check(if smooth { v.spec.glide } else { v.spec.arched });
            cb.connect(
                "toggled",
                &Callable::from_object_method(
                    addressee,
                    if smooth { "on_smooth" } else { "on_arched" },
                ),
            );
            (cb.clone().upcast(), Ctl::Check(cb))
        }
        Row::Layout => {
            let mut ob = OptionButton::new_alloc();
            for n in super::layout::NAMES {
                ob.add_item(n);
            }
            ob.select(Layout::ingest() as i32);
            ob.set_focus_mode(FocusMode::NONE);
            ob.connect(
                "item_selected",
                &Callable::from_object_method(addressee, "on_layout_choice"),
            );
            (ob.clone().upcast(), Ctl::Option(ob))
        }
    };
    ctl.set_h_size_flags(SizeFlags::EXPAND_FILL);
    ctl.set_tooltip_text(&tip);
    (label, ctl, kind)
}
