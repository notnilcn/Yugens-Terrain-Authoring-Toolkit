//! `PixelFireEditorPlugin` — editor ignite tool for `PixelFireSystem` (auto-registered).
//!
//! While a `PixelFireSystem` with `ignite_tool` on is selected, a left click on the terrain in
//! the 3D viewport ignites the grass there (`ignite_radius`), Shift + click puts fires out.
//! The fire runs in the editor when the node's `preview_in_editor` is on; nothing is saved
//! (the fire state is runtime only). The Pixel World "Fire" section toggles the tool and has
//! Ignite / Extinguish / Reset buttons.

use godot::classes::editor_plugin::AfterGuiInput;
use godot::classes::{Camera3D, EditorPlugin, IEditorPlugin, Input, InputEvent, InputEventMouseButton, Object};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use crate::effects::fire::PixelFireSystem;
use crate::terrain::MarchingSquaresTerrain;

#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelFireEditorPlugin {
    fire: Option<Gd<PixelFireSystem>>,
    /// Hides Godot's transform gizmo on the fire system while the ignite tool is on.
    gizmo_lock: super::gizmo_lock::GizmoLock,
    base: Base<EditorPlugin>,
}

#[godot_api]
impl IEditorPlugin for PixelFireEditorPlugin {
    fn enter_tree(&mut self) {
        self.base_mut().set_process(true);
    }

    fn exit_tree(&mut self) {
        self.gizmo_lock.clear();
    }

    fn process(&mut self, _delta: f64) {
        let want = self.fire.clone().filter(|f| f.is_instance_valid() && f.get("ignite_tool").try_to::<bool>().unwrap_or(false));
        self.gizmo_lock.sync(want.map(|f| f.upcast()));
    }

    fn apply_changes(&mut self) {
        // Right before a scene is packed: never save the lock (`process` restores it).
        self.gizmo_lock.strip();
    }

    fn get_plugin_name(&self) -> GString {
        "Pixel Fire".into()
    }

    fn handles(&self, object: Gd<Object>) -> bool {
        object.is_class("PixelFireSystem")
    }

    fn edit(&mut self, object: Option<Gd<Object>>) {
        self.fire = object.and_then(|o| o.try_cast::<PixelFireSystem>().ok());
    }

    fn forward_3d_gui_input(&mut self, camera: Option<Gd<Camera3D>>, event: Option<Gd<InputEvent>>) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let (Some(camera), Some(event), Some(mut fire)) = (camera, event, self.fire.clone()) else { return pass };
        if !fire.is_instance_valid() || !fire.get("ignite_tool").try_to::<bool>().unwrap_or(false) {
            return pass;
        }
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else { return pass };
        if mb.get_button_index() != MouseButton::LEFT {
            return pass;
        }
        if !mb.is_pressed() {
            return AfterGuiInput::STOP.ord();
        }
        let Some(hit) = terrain_hit(&fire, &camera, mb.get_position()) else { return pass };
        let radius = fire.get("ignite_radius").try_to::<f32>().unwrap_or(1.0);
        if Input::singleton().is_key_pressed(Key::SHIFT) {
            fire.call("extinguish", &[hit.to_variant(), (radius * 2.0).to_variant()]);
        } else {
            fire.call("ignite", &[hit.to_variant(), radius.to_variant()]);
        }
        AfterGuiInput::STOP.ord()
    }
}

/// Where the camera ray under `pos` hits the fire system's terrain.
fn terrain_hit(fire: &Gd<PixelFireSystem>, camera: &Gd<Camera3D>, pos: Vector2) -> Option<Vector3> {
    let parent = fire.get_parent()?;
    let terrain: Gd<MarchingSquaresTerrain> = match parent.clone().try_cast::<MarchingSquaresTerrain>() {
        Ok(t) => t,
        Err(_) => crate::effects::find_terrain(&fire.clone().upcast())?.try_cast::<MarchingSquaresTerrain>().ok()?,
    };
    let o = camera.project_ray_origin(pos);
    let d = camera.project_ray_normal(pos);
    let hit = terrain.bind().raycast(o, d, 4000.0);
    if !hit.get("hit").and_then(|v| v.try_to::<bool>().ok()).unwrap_or(false) {
        return None;
    }
    hit.get("position").and_then(|v| v.try_to::<Vector3>().ok())
}
