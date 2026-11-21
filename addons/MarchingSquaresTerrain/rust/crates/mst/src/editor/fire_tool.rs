//! Lets the author set fire to the ground, or put it out, by clicking in the 3D editor.
//!
//! Intent: placing fire by typing positions would be impractical. With a fire system
//! selected and its ignite tool switched on, an author expects a click on the terrain to
//! start a fire there and a modified click to extinguish one, while clicks stop moving the
//! selected node.
//!
//! Responsibility: it turns viewport clicks into requests to start or stop fire at the spot
//! on the terrain under the mouse, and holds back the move gizmo while the tool is on. How
//! fire spreads and burns out belongs to the fire system, and where the ground is belongs
//! to the terrain.
//!
//! Why a node: the editor forwards viewport input only to editor plugins, and editor
//! plugins live in the tree.
//!
//! Why a new type: neither a built-in node nor the fire system itself can receive clicks
//! made in the editor viewport, and the fire system should not carry editor-only input
//! handling into the game.
//!
//! Not exported: anything. The editor creates this node itself and never places it in a
//! scene or shows it in an Inspector. It only remembers which fire system is being edited,
//! which the editor hands it at run time.
use crate::effects::fire::PixelFireSystem;
use godot::classes::editor_plugin::AfterGuiInput;
use godot::classes::{
    Camera3D, EditorPlugin, IEditorPlugin, Input, InputEvent, InputEventMouseButton, Object,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelFireEditorPlugin {
    fire: Option<Gd<PixelFireSystem>>,
    gizmo_lock: super::gizmo_lock::GizmoLock,
    base: Base<EditorPlugin>,
}
#[godot_api]
impl IEditorPlugin for PixelFireEditorPlugin {
    fn enter_tree(&mut self) {
        self.base_mut().set_process(true);
    }
    fn exit_tree(&mut self) {
        self.gizmo_lock.purge();
    }
    fn process(&mut self, _delta: f64) {
        let want = self.fire.clone().filter(|f| {
            f.is_instance_valid() && f.get("ignite_tool").try_to::<bool>().unwrap_or(false)
        });
        self.gizmo_lock.sync(want.map(|f| f.upcast()));
    }
    fn apply_changes(&mut self) {
        self.gizmo_lock.peel();
    }
    fn get_plugin_name(&self) -> GString {
        "Pixel Fire".into()
    }
    fn handles(&self, entity: Gd<Object>) -> bool {
        entity.is_class("PixelFireSystem")
    }
    fn edit(&mut self, entity: Option<Gd<Object>>) {
        self.fire = entity.and_then(|o| o.try_cast::<PixelFireSystem>().ok());
    }
    fn forward_3d_gui_input(
        &mut self,
        lens: Option<Gd<Camera3D>>,
        happening: Option<Gd<InputEvent>>,
    ) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let (Some(camera), Some(event), Some(mut fire)) = (lens, happening, self.fire.clone())
        else {
            return pass;
        };
        if !fire.is_instance_valid() || !fire.get("ignite_tool").try_to::<bool>().unwrap_or(false) {
            return pass;
        }
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else {
            return pass;
        };
        if mb.get_button_index() != MouseButton::LEFT {
            return pass;
        }
        if !mb.is_pressed() {
            return AfterGuiInput::STOP.ord();
        }
        let Some(hit) = terrain_hit(&fire, &camera, mb.get_position()) else {
            return pass;
        };
        let radius = fire.get("ignite_radius").try_to::<f32>().unwrap_or(1.0);
        if Input::singleton().is_key_pressed(Key::SHIFT) {
            fire.call(
                "extinguish",
                &[hit.to_variant(), (radius * 2.0).to_variant()],
            );
        } else {
            fire.call("ignite", &[hit.to_variant(), radius.to_variant()]);
        }
        AfterGuiInput::STOP.ord()
    }
}
fn terrain_hit(fire: &Gd<PixelFireSystem>, lens: &Gd<Camera3D>, pos: Vector2) -> Option<Vector3> {
    let terrain = fire.bind().terrain()?;
    let o = lens.project_ray_origin(pos);
    let d = lens.project_ray_normal(pos);
    let hit = terrain.bind().ray_hit(o, d, 4000.0);
    if !hit
        .get("hit")
        .and_then(|v| v.try_to::<bool>().ok())
        .unwrap_or(false)
    {
        return None;
    }
    hit.get("position").and_then(|v| v.try_to::<Vector3>().ok())
}
