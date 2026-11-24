//! The fixed frame of the in-game diagnostics overlay, as one swappable scene.
//!
//! Intent: the panels, buttons and marker that make up the diagnostics overlay are laid out
//! in a scene, so the look of the overlay is edited in the editor rather than in code. An
//! author who wants a different layout copies that scene, rearranges it and hands it to the
//! overlay; this node, at the root of such a scene, is how the scene says which of its
//! parts plays which role.
//!
//! Responsibility: it names its own parts and reports, in the editor, any role left
//! unfilled. It does not fill or update them: the rows, readouts and picking stay with the
//! diagnostics overlay, which takes the parts from here once, when the frame comes to life.
//! This file also decides when a frame exists, bringing in the author's scene or the
//! built-in one when none was chosen, and is the one place that knows where the built-in
//! scene lives.
//!
//! Why a node: the parts it names are nodes in the same scene, and the only way for a scene
//! to point at its own nodes so that a rename in the editor follows along is from a node
//! inside it. It also has to be part of the drawn interface itself, so that showing or
//! hiding the overlay as a whole carries down to every panel in the frame.
//!
//! Why a new type: a plain node at the root can hold the parts but cannot say which is
//! which, so the overlay would have to find them by name, and an edited copy of the frame
//! would break silently instead of telling its author what is missing.
//!
//! Nodes created in code: the frame exists only while a game is running with diagnostics in
//! it, never in the editor, so it is instantiated then instead of being declared in every
//! scene that uses the overlay; its layout is still authored in a scene.
use godot::classes::{
    Button, Control, IControl, Label, MeshInstance3D, Node, OptionButton, PackedScene,
    VBoxContainer,
};
use godot::prelude::*;

const OVERLAY_SCENE: &str =
    "res://addons/MarchingSquaresTerrain/debug/pixel_diagnostics_overlay.tscn";

#[derive(GodotClass)]
#[class(tool, init, base = Control)]
pub struct PixelDiagnosticsOverlay {
    #[export]
    status_panel: Option<Gd<Control>>,
    #[export]
    status: Option<Gd<Label>>,
    #[export]
    tuner: Option<Gd<Control>>,
    #[export]
    rescan: Option<Gd<Button>>,
    #[export]
    debug_view: Option<Gd<OptionButton>>,
    #[export]
    effects_sections: Option<Gd<VBoxContainer>>,
    #[export]
    render_sections: Option<Gd<VBoxContainer>>,
    #[export]
    scene_sections: Option<Gd<VBoxContainer>>,
    #[export]
    object_panel: Option<Gd<Control>>,
    #[export]
    select: Option<Gd<Button>>,
    #[export]
    clear: Option<Gd<Button>>,
    #[export]
    object_hint: Option<Gd<Label>>,
    #[export]
    object_info: Option<Gd<Label>>,
    #[export]
    object_sections: Option<Gd<VBoxContainer>>,
    #[export]
    selection_marker: Option<Gd<MeshInstance3D>>,
    base: Base<Control>,
}

#[godot_api]
impl IControl for PixelDiagnosticsOverlay {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        let roles: [(&str, bool); 15] = [
            ("Status Panel", self.status_panel.is_some()),
            ("Status", self.status.is_some()),
            ("Tuner", self.tuner.is_some()),
            ("Rescan", self.rescan.is_some()),
            ("Debug View", self.debug_view.is_some()),
            ("Effects Sections", self.effects_sections.is_some()),
            ("Render Sections", self.render_sections.is_some()),
            ("Scene Sections", self.scene_sections.is_some()),
            ("Object Panel", self.object_panel.is_some()),
            ("Select", self.select.is_some()),
            ("Clear", self.clear.is_some()),
            ("Object Hint", self.object_hint.is_some()),
            ("Object Info", self.object_info.is_some()),
            ("Object Sections", self.object_sections.is_some()),
            ("Selection Marker", self.selection_marker.is_some()),
        ];
        for (role, set) in roles {
            if !set {
                w.push(&format!(
                    "Assign {role}: the diagnostics overlay stays off without it."
                ));
            }
        }
        w
    }
}

pub struct OverlayParts {
    pub root: Gd<Node>,
    pub status_panel: Gd<Control>,
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
    pub fn instantiate(chosen: Option<&Gd<PackedScene>>) -> Option<OverlayParts> {
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
        let parts = match root.clone().try_cast::<PixelDiagnosticsOverlay>() {
            Ok(o) => {
                let parts = Self::collect(&o.bind());
                if parts.is_none() {
                    godot_error!(
                        "PixelDiagnostics: overlay scene {path} leaves some of its panels unassigned; the overlay is off."
                    );
                }
                parts
            }
            Err(_) => {
                godot_error!(
                    "PixelDiagnostics: overlay scene {path} does not have a PixelDiagnosticsOverlay at its root; the overlay is off."
                );
                None
            }
        };
        if parts.is_none() {
            root.free();
        }
        parts
    }

    fn collect(o: &PixelDiagnosticsOverlay) -> Option<OverlayParts> {
        Some(OverlayParts {
            root: o.to_gd().upcast(),
            status_panel: o.status_panel.clone()?,
            status: o.status.clone()?,
            tuner: o.tuner.clone()?,
            rescan: o.rescan.clone()?,
            debug_view: o.debug_view.clone()?,
            tab_sections: vec![
                o.effects_sections.clone()?,
                o.render_sections.clone()?,
                o.scene_sections.clone()?,
            ],
            object_panel: o.object_panel.clone()?,
            select: o.select.clone()?,
            clear: o.clear.clone()?,
            object_hint: o.object_hint.clone()?,
            object_info: o.object_info.clone()?,
            object_sections: o.object_sections.clone()?,
            marker: o.selection_marker.clone()?,
        })
    }
}
