//! Connects the controls declared in the UV editor's scene to the editing surface they
//! drive, so the scene decides what the window offers and the surface decides what each
//! choice does.
use super::super::plugin::editor_scenes::unique;
use super::*;
use godot::classes::{
    BaseButton, Button, Container, EditorFileDialog, OptionButton, PanelContainer, SpinBox, Window,
};
const ACTIONS: [(&str, Act); 19] = [
    ("Manual", Act::Manual),
    ("Auto", Act::Auto),
    ("Planar", Act::Planar),
    ("Box", Act::Box),
    ("Spherical", Act::Spherical),
    ("Flip U", Act::FlipU),
    ("Flip V", Act::FlipV),
    ("Rot 90", Act::Rot90),
    ("Fit", Act::Fit),
    ("Fit U", Act::FitU),
    ("Fit V", Act::FitV),
    ("Sew", Act::Sew),
    ("Copy", Act::Copy),
    ("Paste", Act::Paste),
    ("Texel", Act::Texel),
    ("Island", Act::Island),
    ("Frame", Act::Frame),
    ("Frame Sel", Act::FrameSel),
    ("Scale", Act::Scale),
];
const FLAGS: [(&str, i64); 4] = [
    ("Lock 0-1", FLAG_LOCK),
    ("Split", FLAG_SPLIT),
    ("Snap", FLAG_SNAP),
    ("Tile", FLAG_TILE),
];
const AUTO_OPTIONS: [(&str, i64); 2] = [("AutoFill", AUTO_FILL), ("AutoAnchor", AUTO_ANCHOR)];
const AUTO_CHECKS: [(&str, i64); 4] = [
    ("AutoFlipU", AUTO_FLIP_U),
    ("AutoFlipV", AUTO_FLIP_V),
    ("AutoSwap", AUTO_SWAP),
    ("AutoGroup", AUTO_GROUP),
];
const TEMPLATE_CHECKS: [(&str, i64); 2] = [
    ("TemplateGrid", TPL_GRID),
    ("TemplateTransparent", TPL_TRANSPARENT),
];
fn child<T: Inherits<Node>>(parent: &Gd<Container>, name: &str) -> Option<Gd<T>> {
    let found = parent.try_get_node_as::<T>(name);
    if found.is_none() {
        godot_error!(
            "Pixel Builder: the UV editor scene has no {name} control under {}",
            parent.get_name()
        );
    }
    found
}
fn scaled(v: Vector2i, s: f32) -> Vector2i {
    Vector2i::new((v.x as f32 * s) as i32, (v.y as f32 * s) as i32)
}
fn retint_icons(panel: &Gd<PanelContainer>) {
    for node in panel
        .find_children_ex("*")
        .type_("Button")
        .owned(false)
        .done()
        .iter_shared()
    {
        let Ok(mut b) = node.try_cast::<Button>() else {
            continue;
        };
        let Some(icon) = b.get_button_icon() else {
            continue;
        };
        let path = icon.get_path().to_string();
        if let Some(t) = super::super::ui::mono_icon(&path, 1.0) {
            b.set_button_icon(&t);
        }
    }
}
pub fn wire_window(win: &mut Gd<Window>) -> Option<(Gd<PanelContainer>, Gd<PixelBuilderUvCanvas>)> {
    let panel = unique::<PanelContainer>(win, "PixelBuilderUvPanel")?;
    let mut canvas = unique::<PixelBuilderUvCanvas>(win, "Canvas")?;
    let bar = unique::<Container>(win, "Bar")?;
    let auto = unique::<Container>(win, "AutoUvs")?;
    let s = crate::editor::ui_kit::editor_scale();
    let size = win.get_size();
    win.set_size(scaled(size, s));
    let min = win.get_min_size();
    win.set_min_size(scaled(min, s));
    let floor = canvas.get_custom_minimum_size();
    canvas.set_custom_minimum_size(floor * s);
    retint_icons(&panel);
    let method = |name: &str| Callable::from_object_method(&canvas, name);
    let tagged = |name: &str, tag: i64| method(name).bind(&[tag.to_variant()]);
    let mut vert_mode = false;
    if let Some(mut faces) = child::<BaseButton>(&bar, "Faces") {
        faces.connect("pressed", &method("set_face_mode"));
    }
    if let Some(mut verts) = child::<BaseButton>(&bar, "Verts") {
        verts.connect("pressed", &method("set_vertex_mode"));
        vert_mode = verts.is_pressed();
    }
    for (name, act) in ACTIONS {
        if let Some(mut b) = child::<BaseButton>(&bar, name) {
            b.connect("pressed", &tagged("perform", act as i64));
        }
    }
    if let Some(mut b) = child::<BaseButton>(&auto, "Apply Auto") {
        b.connect("pressed", &tagged("perform", Act::ApplyAuto as i64));
    }
    let mut flags = Vec::new();
    for (name, flag) in FLAGS {
        if let Some(mut b) = child::<BaseButton>(&bar, name) {
            b.connect("toggled", &tagged("set_flag", flag));
            flags.push((b.is_pressed(), flag));
        }
    }
    if let Some(mut sb) = child::<SpinBox>(&bar, "Step") {
        sb.connect("value_changed", &method("set_step"));
    }
    for (name, axis) in [("ScaleU", 0), ("ScaleV", 1)] {
        if let Some(mut sb) = child::<SpinBox>(&bar, name) {
            sb.connect("value_changed", &tagged("set_scale_factor", axis));
        }
    }
    let mut options = Vec::new();
    for (name, id) in AUTO_OPTIONS {
        if let Some(mut ob) = child::<OptionButton>(&auto, name) {
            ob.connect("item_selected", &tagged("set_auto_option", id));
            options.push((ob.get_selected() as i64, id));
        }
    }
    let mut checks = Vec::new();
    for (name, id) in AUTO_CHECKS {
        if let Some(mut cb) = child::<BaseButton>(&auto, name) {
            cb.connect("toggled", &tagged("set_auto_check", id));
            checks.push((cb.is_pressed(), id));
        }
    }
    if let Some(mut sb) = child::<SpinBox>(&auto, "TemplateSize") {
        sb.connect("value_changed", &tagged("set_template", TPL_SIZE));
    }
    let mut template_checks = Vec::new();
    for (name, id) in TEMPLATE_CHECKS {
        if let Some(mut cb) = child::<BaseButton>(&auto, name) {
            cb.connect("toggled", &tagged("set_template_check", id));
            template_checks.push((cb.is_pressed(), id));
        }
    }
    if let Some(mut dlg) = unique::<EditorFileDialog>(win, "TemplateDialog") {
        dlg.set_current_file("uv_template.png");
        dlg.connect("file_selected", &method("export_template"));
    }
    {
        let mut c = canvas.bind_mut();
        c.set_flag(vert_mode, FLAG_VERTS);
        for (on, flag) in flags {
            c.set_flag(on, flag);
        }
        for (index, id) in options {
            c.set_auto_option(index, id);
        }
        for (on, id) in checks {
            c.set_auto_check(on, id);
        }
        for (on, id) in template_checks {
            c.set_template_check(on, id);
        }
    }
    Some((panel, canvas))
}
