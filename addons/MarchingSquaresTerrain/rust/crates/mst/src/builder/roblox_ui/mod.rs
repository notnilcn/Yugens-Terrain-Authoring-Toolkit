pub mod align;
pub mod grid_plane;
pub mod material_catalog;
pub mod palette;
pub mod part_inspector;
pub mod part_rules;
pub mod pickers;
pub mod ribbon;
pub mod ribbon_root;
use super::ui::Action;
use godot::classes::{Control, EditorInterface};
use godot::prelude::*;
pub use ribbon::{RibbonCmd, RibbonMenu};
pub struct RobloxUi {
    pub ribbon: ribbon::Ribbon,
    pub grid_plane: Option<grid_plane::GridPlaneSwitch>,
    pub in_menu: bool,
}
#[derive(Clone, Debug, Default)]
pub struct RibbonState {
    pub buttons: Vec<(Action, Option<&'static str>, Option<bool>)>,
    pub grid_step: f32,
    pub rot_snap_on: bool,
    pub rot_snap_deg: f32,
    pub surface_draw: bool,
    pub snap_vertex: bool,
    pub snap_cell: bool,
    pub collisions: bool,
    pub grid_plane: Option<bool>,
    pub compact: bool,
}
impl RobloxUi {
    pub fn assemble(addressee: &Gd<Object>) -> Option<RobloxUi> {
        ribbon::Ribbon::assemble(addressee).map(|ribbon| {
            let mut grid_plane = grid_plane::GridPlaneSwitch::assemble(addressee);
            if let Some(g) = &mut grid_plane {
                g.host_on_viewport(0);
            }
            RobloxUi {
                ribbon,
                grid_plane,
                in_menu: false,
            }
        })
    }
    pub fn host_below_spatial_toolbar(&mut self) -> bool {
        let Some(main) = EditorInterface::singleton().get_editor_main_screen() else {
            return false;
        };
        let Some(mut editor) = main
            .get_children()
            .iter_shared()
            .find(|c| c.is_class("Node3DEditor"))
        else {
            return false;
        };
        let after = editor
            .get_children()
            .iter_shared()
            .position(|c| c.is_class("MarginContainer"))
            .map(|i| i as i32 + 1)
            .unwrap_or(0);
        let root = self.ribbon.root.clone();
        if let Some(mut p) = root.get_parent() {
            p.remove_child(&root);
        }
        editor.add_child(&root);
        editor.move_child(&root, after);
        true
    }
    pub fn root(&self) -> Gd<Control> {
        self.ribbon.root.clone()
    }
    pub fn sync(&mut self, st: &RibbonState) {
        self.ribbon.sync(st);
        if let Some(g) = &mut self.grid_plane {
            g.sync(st.grid_plane);
        }
    }
    pub fn free(self, now: bool) {
        if let Some(g) = self.grid_plane {
            g.free(now);
        }
        let mut root = self.ribbon.root;
        if !root.is_instance_valid() {
            return;
        }
        if now {
            root.free();
        } else {
            if let Some(mut p) = root.get_parent() {
                p.remove_child(&root);
            }
            root.queue_free();
        }
    }
}
