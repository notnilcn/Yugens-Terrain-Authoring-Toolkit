//! Connects the controls declared in the UV editor's scene to the editing surface they
//! drive, so the scene decides what the window offers and the surface decides what each
//! choice does.
use super::window::PixelBuilderUvEditor;
use super::*;
use godot::classes::{BaseButton, Button, OptionButton, PanelContainer};
type Ref<T> = Option<Gd<T>>;
fn present<T: GodotClass>(found: &Ref<T>, role: &str) -> Ref<T> {
    if found.is_none() {
        godot_error!("Pixel Builder: the UV editor scene does not assign its {role} control");
    }
    found.clone()
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
pub fn wire_window(
    win: &mut Gd<PixelBuilderUvEditor>,
) -> Option<(Gd<PanelContainer>, Gd<PixelBuilderUvCanvas>)> {
    let w = win.bind();
    let panel = present(&w.panel, "panel")?;
    let mut canvas = present(&w.canvas, "canvas")?;
    let actions: [(&Ref<BaseButton>, &str, Act); 20] = [
        (&w.manual, "manual", Act::Manual),
        (&w.auto, "auto", Act::Auto),
        (&w.planar, "planar", Act::Planar),
        (&w.box_map, "box map", Act::Box),
        (&w.spherical, "spherical", Act::Spherical),
        (&w.flip_u, "flip u", Act::FlipU),
        (&w.flip_v, "flip v", Act::FlipV),
        (&w.rot90, "rot90", Act::Rot90),
        (&w.fit, "fit", Act::Fit),
        (&w.fit_u, "fit u", Act::FitU),
        (&w.fit_v, "fit v", Act::FitV),
        (&w.sew, "sew", Act::Sew),
        (&w.copy, "copy", Act::Copy),
        (&w.paste, "paste", Act::Paste),
        (&w.texel, "texel", Act::Texel),
        (&w.island, "island", Act::Island),
        (&w.frame, "frame", Act::Frame),
        (&w.frame_sel, "frame sel", Act::FrameSel),
        (&w.scale, "scale", Act::Scale),
        (&w.apply_auto, "apply auto", Act::ApplyAuto),
    ];
    let actions: Vec<(Ref<BaseButton>, Act)> = actions
        .into_iter()
        .map(|(b, role, act)| (present(b, role), act))
        .collect();
    let flags: Vec<(Ref<BaseButton>, i64)> = [
        (&w.lock01, "lock01", FLAG_LOCK),
        (&w.split, "split", FLAG_SPLIT),
        (&w.snap, "snap", FLAG_SNAP),
        (&w.tile, "tile", FLAG_TILE),
    ]
    .into_iter()
    .map(|(b, role, flag)| (present(b, role), flag))
    .collect();
    let options: Vec<(Ref<OptionButton>, i64)> = [
        (&w.auto_fill, "auto fill", AUTO_FILL),
        (&w.auto_anchor, "auto anchor", AUTO_ANCHOR),
    ]
    .into_iter()
    .map(|(b, role, id)| (present(b, role), id))
    .collect();
    let checks: Vec<(Ref<BaseButton>, i64)> = [
        (&w.auto_flip_u, "auto flip u", AUTO_FLIP_U),
        (&w.auto_flip_v, "auto flip v", AUTO_FLIP_V),
        (&w.auto_swap, "auto swap", AUTO_SWAP),
        (&w.auto_group, "auto group", AUTO_GROUP),
    ]
    .into_iter()
    .map(|(b, role, id)| (present(b, role), id))
    .collect();
    let template_checks: Vec<(Ref<BaseButton>, i64)> = [
        (&w.template_grid, "template grid", TPL_GRID),
        (&w.template_transparent, "template transparent", TPL_TRANSPARENT),
    ]
    .into_iter()
    .map(|(b, role, id)| (present(b, role), id))
    .collect();
    let faces = present(&w.faces, "faces");
    let verts = present(&w.verts, "verts");
    let step = present(&w.step, "step");
    let factors = [
        (present(&w.scale_u, "scale u"), 0i64),
        (present(&w.scale_v, "scale v"), 1i64),
    ];
    let template_size = present(&w.template_size, "template size");
    let dialog = present(&w.template_dialog, "template dialog");
    drop(w);
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
    if let Some(mut faces) = faces {
        faces.connect("pressed", &method("set_face_mode"));
    }
    if let Some(mut verts) = verts {
        verts.connect("pressed", &method("set_vertex_mode"));
        vert_mode = verts.is_pressed();
    }
    for (b, act) in actions {
        if let Some(mut b) = b {
            b.connect("pressed", &tagged("perform", act as i64));
        }
    }
    let mut flag_values = Vec::new();
    for (b, flag) in flags {
        if let Some(mut b) = b {
            b.connect("toggled", &tagged("set_flag", flag));
            flag_values.push((b.is_pressed(), flag));
        }
    }
    let mut step_value = None;
    if let Some(mut sb) = step {
        sb.connect("value_changed", &method("set_step"));
        step_value = Some(sb.get_value());
    }
    let mut factor_values = [None; 2];
    for (sb, axis) in factors {
        if let Some(mut sb) = sb {
            sb.connect("value_changed", &tagged("set_scale_factor", axis));
            factor_values[axis as usize] = Some(sb.get_value());
        }
    }
    let mut option_values = Vec::new();
    for (ob, id) in options {
        if let Some(mut ob) = ob {
            ob.connect("item_selected", &tagged("set_auto_option", id));
            option_values.push((ob.get_selected() as i64, id));
        }
    }
    let mut check_values = Vec::new();
    for (cb, id) in checks {
        if let Some(mut cb) = cb {
            cb.connect("toggled", &tagged("set_auto_check", id));
            check_values.push((cb.is_pressed(), id));
        }
    }
    let mut template_size_value = None;
    if let Some(mut sb) = template_size {
        sb.connect("value_changed", &tagged("set_template", TPL_SIZE));
        template_size_value = Some(sb.get_value());
    }
    let mut template_check_values = Vec::new();
    for (cb, id) in template_checks {
        if let Some(mut cb) = cb {
            cb.connect("toggled", &tagged("set_template_check", id));
            template_check_values.push((cb.is_pressed(), id));
        }
    }
    if let Some(mut dlg) = dialog {
        dlg.set_current_file("uv_template.png");
        dlg.connect("file_selected", &method("export_template"));
    }
    {
        let mut c = canvas.bind_mut();
        c.set_flag(vert_mode, FLAG_VERTS);
        for (on, flag) in flag_values {
            c.set_flag(on, flag);
        }
        if let Some(v) = step_value {
            c.set_step(v);
        }
        for (axis, v) in factor_values.into_iter().enumerate() {
            if let Some(v) = v {
                c.set_scale_factor(v, axis as i64);
            }
        }
        if let Some(v) = template_size_value {
            c.set_template(v, TPL_SIZE);
        }
        for (index, id) in option_values {
            c.set_auto_option(index, id);
        }
        for (on, id) in check_values {
            c.set_auto_check(on, id);
        }
        for (on, id) in template_check_values {
            c.set_template_check(on, id);
        }
    }
    Some((panel, canvas))
}
