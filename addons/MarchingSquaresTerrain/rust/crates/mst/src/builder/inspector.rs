//! Adds the editable-mesh section to the inspector of meshes the builder can work on.
//!
//! The section is instantiated from its scene in code because the inspector discards and
//! rebuilds its controls for every object it shows, so a fresh section is needed each
//! time a mesh is inspected. The scene decides what the section contains; this file only
//! decides when it appears, which of its parts apply to the inspected object, and wires
//! its buttons to the builder plugin that is handed in at runtime.

use super::inspector_stats::PixelBuilderInspectorStats;
use super::node::PixelBuilderMesh;
use super::ui::Action;
use godot::classes::{
    BaseButton, Button, ConfirmationDialog, Control, CsgShape3D, EditorInspectorPlugin,
    FoldableContainer, IEditorInspectorPlugin, Label, Node, Object, PackedScene,
};
use godot::obj::Inherits;
use godot::prelude::*;

const SECTION_SCENE: &str =
    "res://addons/MarchingSquaresTerrain/editor/builder/editable_mesh_inspector.tscn";

#[derive(GodotClass)]
#[class(tool, init, base = EditorInspectorPlugin)]
pub struct PixelBuilderInspector {
    pub plugin: Option<Gd<Object>>,
    pub stats: Option<Gd<PixelBuilderInspectorStats>>,
    base: Base<EditorInspectorPlugin>,
}

fn part<T: GodotClass + Inherits<Node>>(root: &Gd<Control>, path: &str) -> Option<Gd<T>> {
    let found = root
        .get_node_or_null(path)
        .and_then(|n| n.try_cast::<T>().ok());
    if found.is_none() {
        godot_warn!("Pixel Builder inspector: the section scene is missing \"{path}\"");
    }
    found
}

fn discard(root: &Gd<Control>, path: &str) {
    if let Some(n) = root.get_node_or_null(path) {
        n.free();
    }
}

fn instantiate_section() -> Option<Gd<Control>> {
    let scene = try_load::<PackedScene>(SECTION_SCENE)
        .inspect_err(|e| godot_warn!("Pixel Builder inspector: cannot load {SECTION_SCENE}: {e}"))
        .ok()?;
    let root = scene.instantiate()?;
    match root.try_cast::<Control>() {
        Ok(c) => Some(c),
        Err(n) => {
            godot_warn!("Pixel Builder inspector: {SECTION_SCENE} must have a Control root");
            n.free();
            None
        }
    }
}

fn wire(root: &Gd<Control>, plugin: &Gd<Object>, path: &str, a: Action) {
    if let Some(mut b) = part::<BaseButton>(root, path) {
        b.connect(
            "pressed",
            &Callable::from_object_method(plugin, "upon_deed").bind(&[(a as i64).to_variant()]),
        );
    }
}

#[godot_api]
impl IEditorInspectorPlugin for PixelBuilderInspector {
    fn can_handle(&self, entity: Option<Gd<Object>>) -> bool {
        let Some(o) = entity else { return false };
        if crate::editor::pixel_handles(&o) {
            return false;
        }
        let csg_root = o
            .clone()
            .try_cast::<CsgShape3D>()
            .is_ok_and(|c| c.is_root_shape());
        o.clone().try_cast::<PixelBuilderMesh>().is_ok() || o.is_class("MeshInstance3D") || csg_root
    }
    fn parse_begin(&mut self, entity: Option<Gd<Object>>) {
        let (Some(o), Some(plugin)) = (entity, self.plugin.clone()) else {
            return;
        };
        let Some(root) = instantiate_section() else {
            return;
        };
        if let Some(mut title) = part::<Label>(&root, "Title")
            && let Some(f) = godot::classes::EditorInterface::singleton()
                .get_editor_theme()
                .and_then(|t| t.get_font("bold", "EditorFonts"))
        {
            title.add_theme_font_override("font", &f);
        }
        match o.clone().try_cast::<PixelBuilderMesh>() {
            Err(_) => {
                for p in [
                    "Content/Mesh",
                    "Content/Recipe",
                    "Content/UvLightmap",
                    "Content/Collision",
                    "Content/ExportData",
                ] {
                    discard(&root, p);
                }
                wire(
                    &root,
                    &plugin,
                    "Content/NotEditable/MakeEditable",
                    Action::Convert,
                );
            }
            Ok(node) => {
                discard(&root, "Content/NotEditable");
                let data = node.bind().contents();
                let (shape, bezier) = data
                    .as_ref()
                    .map(|d| {
                        (
                            d.bind().shape_spec().is_some(),
                            d.bind().bezier_recipe().is_some(),
                        )
                    })
                    .unwrap_or((false, false));
                if shape || bezier {
                    if bezier {
                        if let Some(mut f) = part::<FoldableContainer>(&root, "Content/Recipe") {
                            f.set_title("Bezier");
                        }
                        if let Some(mut b) =
                            part::<Button>(&root, "Content/Recipe/Column/EditRecipe")
                        {
                            b.set_text("Edit Bezier Shape");
                        }
                    }
                    wire(
                        &root,
                        &plugin,
                        "Content/Recipe/Column/EditRecipe",
                        Action::ShapeParams,
                    );
                } else {
                    discard(&root, "Content/Recipe");
                }
                for (path, a) in [
                    ("Content/UvLightmap/Column/OpenUvEditor", Action::UvEditor),
                    ("Content/UvLightmap/Column/GenerateUv2", Action::GenerateUv2),
                    ("Content/Collision/Column/SetCollider", Action::SetCollider),
                    ("Content/Collision/Column/SetTrigger", Action::SetTrigger),
                    ("Content/ExportData/Column/ExportNative", Action::ExportRes),
                ] {
                    wire(&root, &plugin, path, a);
                }
                if let Some(mut confirm) =
                    part::<ConfirmationDialog>(&root, "Content/ExportData/Column/StripConfirm")
                {
                    confirm.connect(
                        "confirmed",
                        &Callable::from_object_method(&plugin, "upon_deed")
                            .bind(&[(Action::BakeMesh as i64).to_variant()]),
                    );
                    if let Some(mut strip) =
                        part::<BaseButton>(&root, "Content/ExportData/Column/Strip")
                    {
                        strip.connect(
                            "pressed",
                            &Callable::from_object_method(&confirm, "popup_centered")
                                .bind(&[Vector2i::ZERO.to_variant()]),
                        );
                    }
                }
                if let Some(mut stats) =
                    part::<PixelBuilderInspectorStats>(&root, "Content/Mesh/Column/Stats")
                {
                    {
                        let mut sb = stats.bind_mut();
                        sb.attach(node.clone());
                        sb.renew();
                    }
                    if let Some(mut d) = data {
                        d.connect_flags(
                            "changed",
                            &stats.callable("renew"),
                            godot::classes::object::ConnectFlags::DEFERRED,
                        );
                    }
                    self.stats = Some(stats);
                }
            }
        }
        self.base_mut().add_custom_control(&root);
    }
}
