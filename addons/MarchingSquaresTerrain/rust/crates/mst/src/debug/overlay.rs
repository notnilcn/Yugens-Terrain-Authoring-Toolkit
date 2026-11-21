//! Brings the diagnostics overlay's fixed frame to life from the scene the author chose, or
//! the built-in one when they chose none, and hands back the parts the overlay fills and
//! updates. It is the one place that knows where the built-in frame scene lives. The frame
//! exists only while a game is running with diagnostics in it, never in the editor, so it is
//! instantiated then instead of being declared in every scene that uses the overlay; its
//! layout is still authored in a scene.
use godot::classes::{
    Button, Control, Label, MeshInstance3D, Node, OptionButton, PackedScene, VBoxContainer,
};
use godot::prelude::*;

const OVERLAY_SCENE: &str =
    "res://addons/MarchingSquaresTerrain/debug/pixel_diagnostics_overlay.tscn";

pub struct OverlayParts {
    pub root: Gd<Node>,
    pub status: Gd<Label>,
    pub tuner: Gd<Control>,
    pub rescan: Gd<Button>,
    pub debug_view: Gd<OptionButton>,
    pub tab_sections: Vec<Gd<VBoxContainer>>,
    pub object_panel: Gd<Control>,
    pub select: Gd<Button>,
    pub clear: Gd<Button>,
    pub object_hint: Gd<Label>,
    pub object_info: Gd<Label>,
    pub object_sections: Gd<VBoxContainer>,
    pub marker: Gd<MeshInstance3D>,
}

impl OverlayParts {
    pub fn instantiate(chosen: Option<&Gd<PackedScene>>, tabs: &[&str]) -> Option<OverlayParts> {
        let scene = match chosen {
            Some(s) => s.clone(),
            None => match try_load::<PackedScene>(OVERLAY_SCENE) {
                Ok(s) => s,
                Err(_) => {
                    godot_error!(
                        "PixelDiagnostics: {OVERLAY_SCENE} is missing; the overlay is off."
                    );
                    return None;
                }
            },
        };
        let path = scene.get_path();
        let Some(root) = scene.instantiate() else {
            godot_error!("PixelDiagnostics: overlay scene {path} cannot be instantiated.");
            return None;
        };
        let parts = Self::collect(&root, tabs);
        if parts.is_none() {
            godot_error!(
                "PixelDiagnostics: overlay scene {path} does not have the expected panels; the overlay is off."
            );
            root.free();
        }
        parts
    }

    fn collect(root: &Gd<Node>, tabs: &[&str]) -> Option<OverlayParts> {
        let tab_sections = tabs
            .iter()
            .map(|t| {
                root.try_get_node_as::<VBoxContainer>(&format!(
                    "Overlay/Tuner/Column/Tabs/{t}/Sections"
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(OverlayParts {
            root: root.clone(),
            status: root.try_get_node_as("Overlay/StatusPanel/Status")?,
            tuner: root.try_get_node_as("Overlay/Tuner")?,
            rescan: root.try_get_node_as("Overlay/Tuner/Column/Header/Rescan")?,
            debug_view: root.try_get_node_as("Overlay/Tuner/Column/DebugViewRow/DebugView")?,
            tab_sections,
            object_panel: root.try_get_node_as("Overlay/ObjectPanel")?,
            select: root.try_get_node_as("Overlay/ObjectPanel/Column/Header/Select")?,
            clear: root.try_get_node_as("Overlay/ObjectPanel/Column/Header/Clear")?,
            object_hint: root.try_get_node_as("Overlay/ObjectPanel/Column/Hint")?,
            object_info: root.try_get_node_as("Overlay/ObjectPanel/Column/Info")?,
            object_sections: root.try_get_node_as("Overlay/ObjectPanel/Column/Scroll/Sections")?,
            marker: root.try_get_node_as("SelectionMarker")?,
        })
    }
}
