//! Lets terrain kept in a data folder write itself out whenever the author saves the scene.
//!
//! Intent: an author who chose to keep a terrain's chunks in files beside the scene should
//! never have to remember a separate save step. Saving the scene in the editor should be
//! enough for the terrain on disk to match what they see.
//!
//! Responsibility: it only notices that the editor is about to save and asks the terrain it
//! belongs to to flush its data. It knows nothing about the data format, where files go or
//! whether anything changed; the terrain decides all of that.
//!
//! Why a node: the editor announces an upcoming save only to nodes in the edited tree, so
//! the listener has to sit in that tree beside the terrain it serves.
//!
//! Why a new type: no built-in node turns that announcement into an action, and the terrain
//! cannot listen for it itself. Listening would make the terrain receive every engine
//! notification, including those the engine sends while the terrain's own code is running,
//! and it cannot safely be entered a second time then.
//!
//! The terrain creates this listener in code, only in the editor, as an internal child that
//! is never stored in the scene: it exists for the editing session alone, so a scene could
//! not declare it without saving it into every scene that holds a terrain.
use super::MarchingSquaresTerrain;
use crate::editor_detection::is_editor;
use crate::node_children::add_internal;
use godot::classes::Node;
use godot::prelude::*;

#[derive(GodotClass)]
#[class(tool, init, base = Node, internal)]
pub struct MarchingSquaresTerrainSaveHook {
    base: Base<Node>,
}
#[godot_api]
impl godot::classes::INode for MarchingSquaresTerrainSaveHook {
    fn on_notification(&mut self, what: godot::classes::notify::NodeNotification) {
        if what != godot::classes::notify::NodeNotification::EDITOR_PRE_SAVE {
            return;
        }
        let Some(parent) = self.base().get_parent() else {
            return;
        };
        if let Ok(mut t) = parent.try_cast::<MarchingSquaresTerrain>() {
            let dir = t.bind().directory_mode();
            if dir {
                let _ = t.bind_mut().save_terrain_data();
            }
        }
    }
}
impl MarchingSquaresTerrain {
    pub(super) fn ensure_save_hook(&mut self) {
        if !is_editor() || self.base().has_node("MarchingSquaresTerrainSaveHook") {
            return;
        }
        let mut hook = MarchingSquaresTerrainSaveHook::new_alloc();
        hook.set_name("MarchingSquaresTerrainSaveHook");
        let mut this = self.to_gd();
        add_internal(&mut this, &hook);
    }
}
