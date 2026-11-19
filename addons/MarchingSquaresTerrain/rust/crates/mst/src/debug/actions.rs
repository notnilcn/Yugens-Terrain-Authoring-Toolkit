//! Per-class quick actions and live read-outs of the `PixelDiagnostics` tuner: weather
//! buttons (rain, snow, storm, lightning), time-of-day jumps, fire / smoke / bird triggers and
//! a status line per effect section, so every pixel effect can be driven from the overlay
//! without a script.

use std::cell::Cell;
use std::rc::Rc;

use godot::classes::{Button, CheckBox, Control, HFlowContainer, Label, Node, Node3D};
use godot::prelude::*;

use crate::effects;
use crate::features;

/// A button row action: label, tooltip, what it does with the section's object.
type Action = (&'static str, &'static str, fn(&mut Gd<Object>));

fn small_button(text: &str, tip: &str) -> Gd<Button> {
    let mut b = Button::new_alloc();
    b.set_text(text);
    b.set_tooltip_text(tip);
    b.add_theme_font_size_override("font_size", 12);
    b
}

/// Where the active camera looks at the height of `obj` (a Node3D), for "at view" actions.
fn view_point(obj: &Gd<Object>) -> Option<Vector3> {
    let n = obj.clone().try_cast::<Node3D>().ok()?;
    let cam = n.get_viewport()?.get_camera_3d()?;
    let y = n.get_global_position().y;
    let mut p = effects::camera_focus(&cam, y);
    // Snap to the ground when there is a terrain (the view ray hits the node's plane).
    let mut terrain = effects::find_terrain(&n.clone().upcast::<Node>());
    p.y = effects::ground_height(&mut terrain, p, p.y);
    Some(p)
}

fn time_of_day(o: &mut Gd<Object>, t: f32) {
    o.set("time_override", &(-1.0f32).to_variant());
    o.set("time_of_day", &t.to_variant());
}

const DAY_NIGHT: &[Action] = &[
    ("Dawn", "time_of_day = 0.25 (sunrise)", |o| time_of_day(o, 0.25)),
    ("Noon", "time_of_day = 0.5", |o| time_of_day(o, 0.5)),
    ("Dusk", "time_of_day = 0.75 (sunset)", |o| time_of_day(o, 0.75)),
    ("Night", "time_of_day = 0.0 (midnight)", |o| time_of_day(o, 0.0)),
    ("Run / pause", "Toggle `running` (time advances in the game)", |o| {
        let r = o.get("running").try_to::<bool>().unwrap_or(false);
        o.set("running", &(!r).to_variant());
    }),
];

const FIRE: &[Action] = &[
    ("Ignite at view", "Light the grass where the camera looks (ignite_radius x 3)", |o| {
        if let Some(p) = view_point(o) {
            let r = o.get("ignite_radius").try_to::<f32>().unwrap_or(0.8) * 3.0;
            o.call("ignite", &[p.to_variant(), r.to_variant()]);
        }
    }),
    ("Extinguish all", "Put out every fire", |o| {
        o.call("extinguish_all", &[]);
    }),
    ("Reset", "Regrow everything and clear the fire state", |o| {
        o.call("reset_fire", &[]);
    }),
];

const SMOKE: &[Action] = &[
    ("Burst at view", "Throw a ring of 24 dust puffs where the camera looks", |o| {
        if let Some(p) = view_point(o) {
            o.call("burst", &[p.to_variant(), 24.to_variant()]);
        }
    }),
    ("Burst here", "Throw a ring of 24 dust puffs at the emitter", |o| {
        if let Ok(n) = o.clone().try_cast::<Node3D>() {
            o.call("burst", &[n.get_global_position().to_variant(), 24.to_variant()]);
        }
    }),
];

const BIRDS: &[Action] = &[
    ("Scare", "Perched birds take off", |o| {
        o.call("scare", &[]);
    }),
    ("Land", "The flock lands on the highest ground nearby", |o| {
        o.call("land_now", &[]);
    }),
];

const SKY: &[Action] = &[("Deck on / off", "Toggle the cloud deck", |o| {
    let on = o.get("deck_enabled").try_to::<bool>().unwrap_or(false);
    o.set("deck_enabled", &(!on).to_variant());
})];

fn actions_for(class: &str) -> &'static [Action] {
    match class {
        "PixelDayNight" => DAY_NIGHT,
        "PixelFireSystem" => FIRE,
        "PixelSmoke" => SMOKE,
        "PixelBirds" => BIRDS,
        "PixelSky" => SKY,
        _ => &[],
    }
}

fn flow() -> Gd<HFlowContainer> {
    let mut f = HFlowContainer::new_alloc();
    f.add_theme_constant_override("h_separation", 4);
    f.add_theme_constant_override("v_separation", 4);
    f
}

/// Adds the quick-action rows of `class` (if any) to `container`. Returns whether it added any.
pub fn add_actions(container: &mut Gd<Control>, class: &str, obj: &Gd<Object>) -> bool {
    if class == "PixelWeather" {
        add_weather(container, obj);
        return true;
    }
    let actions = actions_for(class);
    if actions.is_empty() {
        return false;
    }
    let mut row = flow();
    for (text, tip, run) in actions {
        let mut b = small_button(text, tip);
        let (mut o, run) = (obj.clone(), *run);
        b.connect(
            "pressed",
            &Callable::from_fn("pixel_diagnostics_action", move |_: &[&Variant]| {
                if o.is_instance_valid() {
                    run(&mut o);
                }
            }),
        );
        row.add_child(&b);
    }
    container.add_child(&row);
    true
}

/// Weather row: one button per weather (built-ins and custom presets), an *Instant* switch,
/// a lightning strike and a dry / thaw reset.
fn add_weather(container: &mut Gd<Control>, obj: &Gd<Object>) {
    let instant = Rc::new(Cell::new(false));
    let mut row = flow();
    let names = obj.clone().call("get_weather_names", &[]).try_to::<PackedStringArray>().unwrap_or_default();
    for name in names.as_slice() {
        let mut b = small_button(&name.to_string(), &format!("set_weather(\"{name}\")"));
        let (mut o, name, instant) = (obj.clone(), name.clone(), instant.clone());
        b.connect(
            "pressed",
            &Callable::from_fn("pixel_diagnostics_weather", move |_: &[&Variant]| {
                if o.is_instance_valid() {
                    let m = if instant.get() { "set_weather_instant" } else { "set_weather" };
                    o.call(m, &[name.to_variant()]);
                }
            }),
        );
        row.add_child(&b);
    }
    container.add_child(&row);

    let mut row = flow();
    let mut inst = CheckBox::new_alloc();
    inst.set_text("Instant");
    inst.set_tooltip_text("Switch the weather (and the wet / snowy ground) at once instead of blending over transition_seconds");
    inst.add_theme_font_size_override("font_size", 12);
    let flag = instant.clone();
    inst.connect(
        "toggled",
        &Callable::from_fn("pixel_diagnostics_instant", move |args: &[&Variant]| {
            flag.set(args.first().and_then(|v| v.try_to::<bool>().ok()).unwrap_or(false));
        }),
    );
    row.add_child(&inst);
    let mut snow = small_button("Snow overlay", "Toggle snow_enabled: snow on top of any weather");
    let mut o = obj.clone();
    snow.connect(
        "pressed",
        &Callable::from_fn("pixel_diagnostics_snow", move |_: &[&Variant]| {
            if o.is_instance_valid() {
                let on = o.get("snow_enabled").try_to::<bool>().unwrap_or(false);
                o.set("snow_enabled", &(!on).to_variant());
            }
        }),
    );
    row.add_child(&snow);
    let mut bolt = small_button("Lightning", "strike_lightning(): a bolt and a flash now (needs lightning_enabled)");
    let mut o = obj.clone();
    bolt.connect(
        "pressed",
        &Callable::from_fn("pixel_diagnostics_bolt", move |_: &[&Variant]| {
            if o.is_instance_valid() {
                o.call("strike_lightning", &[]);
            }
        }),
    );
    row.add_child(&bolt);
    container.add_child(&row);
}

fn f(o: &mut Gd<Object>, method: &str) -> f32 {
    o.call(method, &[]).try_to::<f32>().unwrap_or(0.0)
}

fn i(o: &mut Gd<Object>, method: &str) -> i64 {
    o.call(method, &[]).try_to::<i64>().unwrap_or(0)
}

/// `hh:mm` of a 0..1 time of day.
pub fn clock_text(t: f32) -> String {
    let m = (t.rem_euclid(1.0) * 24.0 * 60.0).round() as i32 % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// One-line live state of an effect section (refreshed a few times per second), or None for
/// classes without one.
pub fn live_text(class: &str, o: &mut Gd<Object>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(feat) = features::for_class(class)
        && !features::enabled(feat)
    {
        parts.push(format!("feature \"{}\" off: tick it under Features", features::info_of(feat).label));
    }
    match class {
        "PixelWeather" => {
            let name = o.call("get_weather_name", &[]).to_string();
            let p = f(o, "get_transition_progress");
            let arrow = if p < 0.999 { format!(" (blending {:.0}%)", p * 100.0) } else { String::new() };
            parts.push(format!("{name}{arrow} | wet {:.2} | snow {:.2}", f(o, "get_wetness"), f(o, "get_snow_cover")));
        }
        "PixelDayNight" => {
            let ov = o.get("time_override").try_to::<f32>().unwrap_or(-1.0);
            let t = if ov >= 0.0 { ov } else { o.get("time_of_day").try_to::<f32>().unwrap_or(0.0) };
            let running = o.get("running").try_to::<bool>().unwrap_or(false);
            parts.push(format!(
                "{}{} | night {:.2}{}",
                clock_text(t),
                if ov >= 0.0 { " (override)" } else { "" },
                f(o, "get_night_factor"),
                if running { "" } else { " | paused" }
            ));
        }
        "PixelFireSystem" => parts.push(format!(
            "burning {} | burnt {:.0}% | particles {}",
            i(o, "get_burning_count"),
            f(o, "get_burnt_fraction") * 100.0,
            i(o, "get_particle_count")
        )),
        "PixelSmoke" => parts.push(format!("puffs {}", i(o, "get_puff_count"))),
        "PixelBirds" => {
            let n = o.call("get_bird_positions", &[]).try_to::<PackedVector3Array>().map(|a| a.len()).unwrap_or(0);
            parts.push(format!("birds {n}"));
        }
        "PixelSky" => {
            let off = o.call("get_cloud_offset", &[]).try_to::<Vector2>().unwrap_or(Vector2::ZERO);
            parts.push(format!("cloud offset {:.1}, {:.1}", off.x, off.y));
        }
        _ => {}
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// Adds an (initially empty) live read-out label to `container`.
pub fn live_label(container: &mut Gd<Control>) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.add_theme_font_size_override("font_size", 12);
    l.set_modulate(Color::from_rgba(0.75, 0.95, 1.0, 0.9));
    l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
    container.add_child(&l);
    l
}

#[cfg(test)]
mod tests {
    use super::clock_text;

    #[test]
    fn clock() {
        assert_eq!(clock_text(0.0), "00:00");
        assert_eq!(clock_text(0.5), "12:00");
        assert_eq!(clock_text(0.75), "18:00");
        assert_eq!(clock_text(0.9999), "00:00");
        assert_eq!(clock_text(1.25), "06:00");
    }
}
