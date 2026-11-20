//! Scene edits shared by the pixel editor UI (`PixelizerEditorPlugin`) and the "Pixel World"
//! menu next to the pixel-preview toggle (`PixelPreviewPlugin`): adding effects with their
//! references wired up, applying lighting presets and re-wiring `sun`/`environment` refs.
//! Every edit is one `EditorUndoRedoManager` action.

use godot::classes::camera_3d::ProjectionType;
use godot::classes::environment::{AmbientSource, BgMode, ToneMapper};
use godot::classes::{
    Camera3D, ClassDb, DirectionalLight3D, EditorInterface, EditorUndoRedoManager, Environment,
    Node, Node3D, ProceduralSkyMaterial, Sky, VisualInstance3D, WorldEnvironment,
};
use godot::prelude::*;

use super::presets::PRESETS;
use super::sections::{self, RENDERER};

pub fn scene_root() -> Option<Gd<Node>> {
    EditorInterface::singleton().get_edited_scene_root()
}

/// Every node of `class` in the edited scene (root included), in tree order.
pub fn find_all(class: &str) -> Vec<Gd<Node>> {
    let Some(root) = scene_root() else {
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

pub fn find_in_scene(class: &str) -> Option<Gd<Node>> {
    find_all(class).into_iter().next()
}

/// The editor's 3D camera (viewport 0).
pub fn editor_camera() -> Option<Gd<godot::classes::Camera3D>> {
    EditorInterface::singleton()
        .get_editor_viewport_3d()
        .and_then(|v| v.get_camera_3d())
}

/// Ground point in front of the editor camera: the terrain hit if there is a terrain, else
/// 12 m ahead clamped to y >= 0.
fn spawn_point() -> Vector3 {
    let Some(cam) = editor_camera() else {
        return Vector3::ZERO;
    };
    let t = cam.get_global_transform();
    let fwd = -t.basis.col_c();
    if let Some(mut terrain) = find_in_scene("MarchingSquaresTerrain") {
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

/// World AABB of the terrain's (internal) chunk meshes.
fn terrain_bounds() -> Option<Aabb> {
    let terrain = find_in_scene("MarchingSquaresTerrain")?;
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

fn first_light() -> Option<Gd<DirectionalLight3D>> {
    find_in_scene("DirectionalLight3D").and_then(|n| n.try_cast().ok())
}

fn first_world_env() -> Option<Gd<WorldEnvironment>> {
    find_in_scene("WorldEnvironment").and_then(|n| n.try_cast().ok())
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

/// The selected `MarchingSquaresTerrain` (or the terrain above a selected node), else the
/// first terrain in the edited scene.
pub fn target_terrain() -> Option<Gd<Node>> {
    let selected = EditorInterface::singleton()
        .get_selection()
        .map(|s| s.get_selected_nodes())
        .unwrap_or_default();
    for n in selected.iter_shared() {
        let mut cur = Some(n);
        while let Some(c) = cur {
            if c.is_class("MarchingSquaresTerrain") {
                return Some(c);
            }
            cur = c.get_parent();
        }
    }
    find_in_scene("MarchingSquaresTerrain")
}

/// Display name for a new node of `class` ("PixelSky" -> "Sky").
fn vertex_node_moniker(class: &str) -> String {
    match class {
        RENDERER => "PixelRenderer".into(),
        sections::EROSION => "Erosion".into(),
        sections::WILD_FLOWERS => "WildFlowers".into(),
        c => c.trim_start_matches("Pixel").to_string(),
    }
}

/// Adds a `class` node under the scene root (one undo step), together with what it needs:
/// a `Sun` light for sky/day-night, a `WorldEnvironment` for day-night, a camera for the
/// renderer. Effects are placed where the editor camera looks (on the terrain); water is sized
/// to the terrain. Terrain features (erosion, wild flowers) become children of the selected /
/// first terrain. Adding an effect opts in to its project feature (`features::for_category`).
/// Returns the new node; with `select` it is selected (deferred), otherwise the selection is
/// kept (the Pixel World panel follows the selection, so adding a node its UI does not edit
/// would close the panel).
pub fn add_effect(
    mut ur: Gd<EditorUndoRedoManager>,
    class: &str,
    select: bool,
) -> Option<Gd<Node>> {
    let Some(root) = scene_root() else {
        godot_warn!("Pixel World: open a scene first.");
        return None;
    };
    if class == RENDERER
        && let Some(r) = find_in_scene(RENDERER)
    {
        // A second renderer would fight the first; select the existing one instead.
        EditorInterface::singleton().call_deferred("edit_node", &[r.to_variant()]);
        return Some(r);
    }
    let parent = if sections::is_terrain_child(class) {
        let Some(t) = target_terrain() else {
            godot_warn!(
                "Pixel World: {class} works on its parent terrain; add a MarchingSquaresTerrain first."
            );
            return None;
        };
        t
    } else {
        root.clone()
    };
    // Adding an effect is opting in to it: turn its project feature on (else the new node
    // builds and draws nothing).
    if let Some(f) = crate::features::for_category(class)
        && !crate::features::active(f)
    {
        crate::features::assign_active(f, true);
        godot_print!(
            "Pixel World: turned the \"{}\" feature on for this project.",
            crate::features::info_of(f).label
        );
    }
    let Ok(mut node) = ClassDb::singleton().instantiate(class).try_to::<Gd<Node>>() else {
        godot_error!("Pixel World: cannot instantiate {class}");
        return None;
    };
    node.set_name(&vertex_node_moniker(class));
    let mut extra: Vec<Gd<Node>> = Vec::new();

    if let Ok(mut n3) = node.clone().try_cast::<Node3D>() {
        let p = spawn_point();
        match class {
            "PixelLightVolume" => n3.set_position(p + Vector3::new(0.0, 1.5005, 0.0)),
            "PixelFireflies" => n3.set_position(p + Vector3::new(0.0, 1.2005, 0.0)),
            // Dust motes: the falling stack starts at the lowest ground and covers the terrain's
            // height range plus the band the motes live in above the ground.
            "PixelParticleField" => match terrain_bounds() {
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
            // The flock circles around the node: put it on the ground under the view.
            "PixelBirds" => n3.set_position(p),
            // Precipitation is centred where the camera looks at the node's height.
            "PixelWeather" => n3.set_position(Vector3::new(0.0, p.y, 0.0)),
            "PixelWater" => match terrain_bounds() {
                Some(b) => {
                    // Cover the terrain (plus a rim), a quarter of the way up its height range.
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

    // References: an existing sun/environment, or new ones added in the same action.
    if SUN_USERS.contains(&class) {
        let sun = match first_light() {
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
        let env = match first_world_env() {
            Some(w) => w.upcast::<Node>(),
            None => {
                let w = new_world_env();
                extra.push(w.clone());
                w
            }
        };
        node.set("environment", &env.to_variant());
    }
    if class == RENDERER && find_in_scene("Camera3D").is_none() {
        extra.push(new_overview_camera().upcast());
    }
    ur.create_action_ex(&format!("Add {class}"))
        .custom_context(&root)
        .done();
    let rootv = root.to_variant();
    for n in extra.iter() {
        ur.add_do_method(&root, "add_child", &[n.to_variant(), true.to_variant()]);
        ur.add_do_method(n, "set_owner", &[rootv.clone()]);
        ur.add_do_reference(n);
        ur.add_undo_method(&root, "remove_child", &[n.to_variant()]);
    }
    ur.add_do_method(
        &parent,
        "add_child",
        &[node.to_variant(), true.to_variant()],
    );
    ur.add_do_method(&node, "set_owner", &[rootv.clone()]);
    ur.add_do_reference(&node);
    ur.add_undo_method(&parent, "remove_child", &[node.to_variant()]);
    ur.commit_action();
    if select {
        EditorInterface::singleton().call_deferred("edit_node", &[node.to_variant()]);
    }
    Some(node)
}

/// A plain orthographic `Camera3D` named `Camera` looking down at the terrain (or the view's
/// ground point) from 45° yaw / 50° pitch, framing the terrain.
fn new_overview_camera() -> Gd<Camera3D> {
    let (target, extent) = match terrain_bounds() {
        Some(b) => (b.center(), b.size.x.max(b.size.z)),
        None => (spawn_point(), 24.0),
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

/// Classes with a `sun` (DirectionalLight3D) reference that "+ Add" / "Wire refs" fill in.
const SUN_USERS: &[&str] = &["PixelSky", "PixelDayNight", "PixelWeather", "PixelSunShafts"];

/// Fills empty `sun`/`environment` references of sky, day/night, weather and sun-shaft nodes
/// (undoable).
/// Returns how many references were set.
pub fn wire_references(mut ur: Gd<EditorUndoRedoManager>) -> usize {
    let Some(root) = scene_root() else { return 0 };
    let sun = first_light().map(|l| l.to_variant());
    let env = first_world_env().map(|w| w.to_variant());
    let mut edits: Vec<(Gd<Node>, &str, Variant)> = Vec::new();
    for &class in SUN_USERS {
        for n in find_all(class) {
            if let Some(s) = &sun
                && n.get("sun").is_nil()
            {
                edits.push((n.clone(), "sun", s.clone()));
            }
            if matches!(class, "PixelDayNight" | "PixelWeather")
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

/// Applies lighting preset `idx` to whichever of day/night, sky and renderer exist.
pub fn apply_preset(mut ur: Gd<EditorUndoRedoManager>, idx: usize) {
    let Some(p) = PRESETS.get(idx) else { return };
    let mut targets: Vec<(&str, Gd<Node>)> = Vec::new();
    for (class, _, _) in p.values {
        if !targets.iter().any(|(c, _)| c == class)
            && let Some(n) = find_in_scene(class)
        {
            targets.push((class, n));
        }
    }
    if targets.is_empty() {
        godot_warn!("Pixel World: add a Day/Night, Sky or Renderer node before applying a preset.");
        return;
    }
    let Some(root) = scene_root() else { return };
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
