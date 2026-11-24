//! The scene edits behind the pixel workspace's buttons. Adding an effect may also add the
//! sun, sky environment or camera it needs, which the author keeps as part of their scene;
//! which of those are created depends on what the edited scene is missing, so they are
//! made in code at the moment the author asks, through the editor's undo history.
use super::presets::PRESETS;
use super::sections::{self, RENDERER, STEPPER};
use godot::classes::camera_3d::ProjectionType;
use godot::classes::environment::{AmbientSource, BgMode, ToneMapper};
use godot::classes::{
    Camera3D, ClassDb, DirectionalLight3D, EditorInterface, EditorUndoRedoManager, Environment,
    Node, Node3D, ProceduralSkyMaterial, Sky, VisualInstance3D, WorldEnvironment,
};
use godot::prelude::*;
const TERRAIN: &str = "MarchingSquaresTerrain";
const SUN_USERS: [&str; 4] = ["PixelSky", "PixelDayNight", "PixelWeather", "PixelSunShafts"];
const ENVIRONMENT_USERS: [&str; 2] = ["PixelDayNight", "PixelWeather"];
fn edited_scene() -> Option<Gd<Node>> {
    EditorInterface::singleton().get_edited_scene_root()
}
pub fn edited_scene_nodes(class: &str) -> Vec<Gd<Node>> {
    let Some(root) = edited_scene() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if root.is_class(class) {
        out.push(root.clone());
    }
    out.extend(
        root.find_children_ex("*")
            .type_(class)
            .recursive(true)
            .owned(true)
            .done()
            .iter_shared(),
    );
    out
}
fn selected_nodes() -> Vec<Gd<Node>> {
    EditorInterface::singleton()
        .get_selection()
        .map(|s| s.get_selected_nodes().iter_shared().collect())
        .unwrap_or_default()
}
fn exported_ref(owner: &Gd<Node>, prop: &str) -> Option<Gd<Node>> {
    owner
        .get(prop)
        .try_to::<Gd<Node>>()
        .ok()
        .filter(|n| n.is_instance_valid())
}
fn chosen_reference(users: &[&str], prop: &str, class: &str) -> Option<Gd<Node>> {
    let selected = selected_nodes();
    if let Some(n) = selected.iter().find(|n| n.is_class(class)) {
        return Some(n.clone());
    }
    if let Some(n) = selected.iter().find_map(|n| exported_ref(n, prop)) {
        return Some(n);
    }
    users
        .iter()
        .flat_map(|c| edited_scene_nodes(c))
        .find_map(|n| exported_ref(&n, prop))
        .or_else(|| edited_scene_nodes(class).into_iter().next())
}
fn chosen_sun() -> Option<Gd<DirectionalLight3D>> {
    chosen_reference(&SUN_USERS, "sun", "DirectionalLight3D").and_then(|n| n.try_cast().ok())
}
fn chosen_world_env() -> Option<Gd<WorldEnvironment>> {
    chosen_reference(&ENVIRONMENT_USERS, "environment", "WorldEnvironment")
        .and_then(|n| n.try_cast().ok())
}
pub fn editor_camera() -> Option<Gd<godot::classes::Camera3D>> {
    EditorInterface::singleton()
        .get_editor_viewport_3d()
        .and_then(|v| v.get_camera_3d())
}
fn spawn_point(terrain: Option<&Gd<Node>>) -> Vector3 {
    let Some(cam) = editor_camera() else {
        return Vector3::ZERO;
    };
    let t = cam.get_global_transform();
    let fwd = -t.basis.col_c();
    if let Some(mut terrain) = terrain.cloned() {
        let hit = terrain.call(
            "ray_hit",
            &[
                t.origin.to_variant(),
                fwd.to_variant(),
                2000.0f32.to_variant(),
            ],
        );
        if let Ok(d) = hit.try_to::<VarDictionary>()
            && d.get("hit")
                .is_some_and(|h| h.try_to::<bool>().unwrap_or(false))
            && let Some(p) = d.get("position").and_then(|p| p.try_to::<Vector3>().ok())
        {
            return p;
        }
    }
    let p = t.origin + fwd * 12.0;
    Vector3::new(p.x, p.y.max(0.0), p.z)
}
fn terrain_bounds(terrain: Option<&Gd<Node>>) -> Option<Aabb> {
    let terrain = terrain?;
    let mut acc: Option<Aabb> = None;
    for c in terrain
        .get_children_ex()
        .include_internal(true)
        .done()
        .iter_shared()
    {
        let Ok(vi) = c.try_cast::<VisualInstance3D>() else {
            continue;
        };
        let b = vi.get_global_transform() * vi.get_aabb();
        acc = Some(match acc {
            Some(a) => a.merge(b),
            None => b,
        });
    }
    acc
}
fn new_sun() -> Gd<Node> {
    let mut l = DirectionalLight3D::new_alloc();
    l.set_name("Sun");
    l.set_rotation_degrees(Vector3::new(-50.0, -30.0, 0.0));
    l.set_shadow(true);
    l.upcast()
}
fn new_world_env() -> Gd<Node> {
    let mut sky_mat = ProceduralSkyMaterial::new_gd();
    sky_mat.set_sky_top_color(Color::from_rgb(0.3805, 0.5505, 0.8505));
    sky_mat.set_sky_horizon_color(Color::from_rgb(0.7505, 0.8205, 0.9205));
    let mut sky = Sky::new_gd();
    sky.set_material(&sky_mat);
    let mut env = Environment::new_gd();
    env.set_background(BgMode::SKY);
    env.set_sky(&sky);
    env.set_ambient_source(AmbientSource::COLOR);
    env.set_ambient_light_color(Color::from_rgb(0.6005, 0.6505, 0.7505));
    env.set_tonemapper(ToneMapper::LINEAR);
    let mut we = WorldEnvironment::new_alloc();
    we.set_name("WorldEnvironment");
    we.set_environment(&env);
    we.upcast()
}
pub fn selected_terrain() -> Option<Gd<Node>> {
    selected_nodes().iter().find_map(terrain_of)
}
pub fn sole_terrain() -> Option<Gd<Node>> {
    let mut all = edited_scene_nodes(TERRAIN);
    if all.len() == 1 { all.pop() } else { None }
}
pub fn terrain_of(node: &Gd<Node>) -> Option<Gd<Node>> {
    let mut cur = Some(node.clone());
    while let Some(c) = cur {
        if c.is_class(TERRAIN) {
            return Some(c);
        }
        cur = c.get_parent();
    }
    None
}
fn vertex_node_moniker(class: &str) -> String {
    match class {
        RENDERER => "PixelRenderer".into(),
        sections::EROSION => "Erosion".into(),
        sections::WILD_FLOWERS => "WildFlowers".into(),
        c => c.trim_start_matches("Pixel").to_string(),
    }
}
pub fn add_effect(mut ur: Gd<EditorUndoRedoManager>, class: &str) -> Option<Gd<Node>> {
    let Some(root) = edited_scene() else {
        godot_warn!("Pixel World: open a scene first.");
        return None;
    };
    if class == RENDERER
        && let Some(r) = edited_scene_nodes(RENDERER).into_iter().next()
    {
        EditorInterface::singleton().call_deferred("edit_node", &[r.to_variant()]);
        return Some(r);
    }
    let terrain_child = sections::is_terrain_child(class);
    let terrain = match selected_terrain() {
        Some(t) => Some(t),
        None if terrain_child => sole_terrain(),
        None => None,
    };
    let stepped = if class == STEPPER { chosen_mixer(&root) } else { None };
    let parent = if let Some((_, host)) = &stepped {
        host.clone()
    } else if terrain_child {
        let Some(t) = terrain.clone() else {
            godot_warn!(
                "Pixel World: {class} works on its parent terrain and this scene has several; select a MarchingSquaresTerrain (or a node under one) first."
            );
            return None;
        };
        t
    } else {
        root.clone()
    };
    let Ok(mut node) = ClassDb::singleton().instantiate(class).try_to::<Gd<Node>>() else {
        godot_error!("Pixel World: cannot instantiate {class}");
        return None;
    };
    node.set_name(&vertex_node_moniker(class));
    if terrain_child {
        node.set("terrain", &parent.to_variant());
    }
    if let Some((mixer, _)) = &stepped {
        node.set("mixer", &mixer.to_variant());
    }
    let mut extra: Vec<Gd<Node>> = Vec::new();
    if let Ok(mut n3) = node.clone().try_cast::<Node3D>() {
        let p = spawn_point(terrain.as_ref());
        match class {
            "PixelLightVolume" => n3.set_position(p + Vector3::new(0.0, 1.5005, 0.0)),
            "PixelFireflies" => n3.set_position(p + Vector3::new(0.0, 1.2005, 0.0)),
            "PixelParticleField" => match terrain_bounds(terrain.as_ref()) {
                Some(b) => {
                    n3.set_position(Vector3::new(b.center().x, b.position.y, b.center().z));
                    let spacing = node
                        .get("spacing")
                        .try_to::<f32>()
                        .unwrap_or(0.6005)
                        .max(0.0505);
                    let band = node.get("band").try_to::<f32>().unwrap_or(5.0);
                    let layers = ((b.size.y + band) / spacing).ceil().clamp(4.0, 48.0) as i32;
                    node.set("layers", &layers.to_variant());
                }
                None => n3.set_position(Vector3::new(p.x, p.y, p.z)),
            },
            "PixelObject" => n3.set_position(p),
            "PixelSmoke" => n3.set_position(p),
            "PixelBirds" => n3.set_position(p),
            "PixelWeather" => n3.set_position(Vector3::new(0.0, p.y, 0.0)),
            "PixelWater" => match terrain_bounds(terrain.as_ref()) {
                Some(b) => {
                    let c = b.center();
                    let level = b.position.y + b.size.y * 0.2505;
                    n3.set_position(Vector3::new(c.x, level, c.z));
                    node.set(
                        "size",
                        &Vector2::new(b.size.x + 16.0, b.size.z + 16.0).to_variant(),
                    );
                }
                None => n3.set_position(Vector3::new(p.x, 0.5, p.z)),
            },
            _ => {}
        }
    }
    if SUN_USERS.contains(&class) {
        let sun = match chosen_sun() {
            Some(l) => l.upcast::<Node>(),
            None => {
                let s = new_sun();
                extra.push(s.clone());
                s
            }
        };
        node.set("sun", &sun.to_variant());
    }
    if class == "PixelDayNight" {
        let env = match chosen_world_env() {
            Some(w) => w.upcast::<Node>(),
            None => {
                let w = new_world_env();
                extra.push(w.clone());
                w
            }
        };
        node.set("environment", &env.to_variant());
    }
    if class == RENDERER && edited_scene_nodes("Camera3D").is_empty() {
        extra.push(new_overview_camera(terrain.as_ref()).upcast());
    }
    ur.create_action_ex(&format!("Add {class}"))
        .custom_context(&root)
        .done();
    let rootv = root.to_variant();
    for n in extra.iter() {
        ur.add_do_method(&root, "add_child", &[n.to_variant(), true.to_variant()]);
        ur.add_do_method(n, "set_owner", std::slice::from_ref(&rootv));
        ur.add_do_reference(n);
        ur.add_undo_method(&root, "remove_child", &[n.to_variant()]);
    }
    ur.add_do_method(
        &parent,
        "add_child",
        &[node.to_variant(), true.to_variant()],
    );
    ur.add_do_method(&node, "set_owner", std::slice::from_ref(&rootv));
    ur.add_do_reference(&node);
    ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
    ur.commit_action();
    EditorInterface::singleton().call_deferred("edit_node", &[node.to_variant()]);
    Some(node)
}
fn mixers_under(node: &Gd<Node>) -> Vec<Gd<Node>> {
    let mut out: Vec<Gd<Node>> = Vec::new();
    if node.is_class("AnimationMixer") {
        out.push(node.clone());
    }
    out.extend(
        node.find_children_ex("*")
            .type_("AnimationMixer")
            .recursive(true)
            .owned(false)
            .done()
            .iter_shared(),
    );
    out
}
fn chosen_mixer(root: &Gd<Node>) -> Option<(Gd<Node>, Gd<Node>)> {
    let owned_host = |n: &Gd<Node>| -> Gd<Node> {
        let mut at = Some(n.clone());
        while let Some(cur) = at {
            if cur == *root || cur.get_owner().as_ref() == Some(root) {
                return cur;
            }
            at = cur.get_parent();
        }
        root.clone()
    };
    for sel in selected_nodes() {
        if let Some(mixer) = mixers_under(&sel).into_iter().next() {
            let host = if sel == mixer {
                mixer.get_parent().map_or_else(|| root.clone(), |p| owned_host(&p))
            } else {
                owned_host(&sel)
            };
            return Some((mixer, host));
        }
    }
    let all = mixers_under(root);
    match all.as_slice() {
        [only] => Some((only.clone(), root.clone())),
        [] => {
            godot_warn!("Pixel World: no AnimationPlayer / AnimationTree in this scene; the Animation Stepper was added without one (set its mixer in the Inspector).");
            None
        }
        _ => {
            godot_warn!("Pixel World: this scene has several animation players; select the character (or its AnimationPlayer) to wire the Animation Stepper to it, or set its mixer in the Inspector.");
            None
        }
    }
}
fn new_overview_camera(terrain: Option<&Gd<Node>>) -> Gd<Camera3D> {
    let (target, extent) = match terrain_bounds(terrain) {
        Some(b) => (b.center(), b.size.x.max(b.size.z)),
        None => (spawn_point(terrain), 24.0),
    };
    let basis = Basis::from_euler(
        EulerOrder::YXZ,
        Vector3::new(-50f32.to_radians(), 45f32.to_radians(), 0.0),
    );
    let distance = 40.0f32.max(extent);
    let mut cam = Camera3D::new_alloc();
    cam.set_name("Camera");
    cam.set_transform(Transform3D::new(basis, target + basis.col_c() * distance));
    cam.set_projection(ProjectionType::ORTHOGONAL);
    cam.set_size(extent.clamp(8.0, 200.0));
    cam.set_far(distance * 2.0 + 200.0);
    cam
}
pub fn wire_references(mut ur: Gd<EditorUndoRedoManager>) -> usize {
    let Some(root) = edited_scene() else { return 0 };
    let sun = chosen_sun().map(|l| l.to_variant());
    let env = chosen_world_env().map(|w| w.to_variant());
    let mut edits: Vec<(Gd<Node>, &str, Variant)> = Vec::new();
    for class in SUN_USERS {
        for n in edited_scene_nodes(class) {
            if let Some(s) = &sun
                && n.get("sun").is_nil()
            {
                edits.push((n.clone(), "sun", s.clone()));
            }
            if ENVIRONMENT_USERS.contains(&class)
                && let Some(e) = &env
                && n.get("environment").is_nil()
            {
                edits.push((n.clone(), "environment", e.clone()));
            }
        }
    }
    if edits.is_empty() {
        return 0;
    }
    ur.create_action_ex("Wire pixel effect references")
        .custom_context(&root)
        .done();
    for (n, prop, v) in &edits {
        ur.add_do_property(n, *prop, v);
        ur.add_undo_property(n, *prop, &Variant::nil());
    }
    ur.commit_action();
    edits.len()
}
pub fn apply_preset(mut ur: Gd<EditorUndoRedoManager>, idx: usize) {
    let Some(p) = PRESETS.get(idx) else { return };
    let mut targets: Vec<(&str, Gd<Node>)> = Vec::new();
    for (class, _, _) in p.values {
        if !targets.iter().any(|(c, _)| c == class)
            && let Some(n) = edited_scene_nodes(class).into_iter().next()
        {
            targets.push((class, n));
        }
    }
    if targets.is_empty() {
        godot_warn!("Pixel World: add a Day/Night, Sky or Renderer node before applying a preset.");
        return;
    }
    let Some(root) = edited_scene() else { return };
    ur.create_action_ex(&format!("Apply {} preset", p.moniker))
        .custom_context(&root)
        .done();
    for (class, prop, v) in p.values {
        let Some((_, n)) = targets.iter().find(|(c, _)| c == class) else {
            continue;
        };
        ur.add_do_property(n, *prop, &v.to_variant());
        ur.add_undo_property(n, *prop, &n.get(*prop));
    }
    ur.commit_action();
}
