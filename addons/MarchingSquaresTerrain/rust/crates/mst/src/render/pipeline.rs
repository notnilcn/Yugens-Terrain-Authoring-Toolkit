//! Brings the renderer's off-screen pipeline to life from the scene the author chose, or the
//! built-in one when they chose none, and hands back the parts the renderer drives every
//! frame. It is the one place that knows where the built-in pipeline scene lives. The
//! pipeline is built only while pixel output is wanted, in the game or in an editor preview,
//! and torn down when it is not, so it cannot be a permanent part of the scene the renderer
//! is placed in; what it consists of is still authored in a scene, and this file only
//! decides when one exists.
use godot::classes::{
    Camera3D, CanvasLayer, ColorRect, Node, PackedScene, ShaderMaterial, SubViewport,
};
use godot::prelude::*;
use std::sync::Once;

const PIPELINE_SCENE: &str = "res://addons/MarchingSquaresTerrain/render/pixel_pipeline.tscn";

pub struct PipelineParts {
    pub root: Gd<Node>,
    pub resolve_vp: Gd<SubViewport>,
    pub color_vp: Gd<SubViewport>,
    pub meta_vp: Gd<SubViewport>,
    pub id_vp: Gd<SubViewport>,
    pub color_cam: Gd<Camera3D>,
    pub meta_cam: Gd<Camera3D>,
    pub id_cam: Gd<Camera3D>,
    pub resolve_mat: Gd<ShaderMaterial>,
    pub upscale_mat: Gd<ShaderMaterial>,
    pub display: Gd<CanvasLayer>,
    pub display_rect: Gd<ColorRect>,
}

fn report_broken(path: &str, why: &str) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        godot_error!(
            "Marching Squares Terrain: pipeline scene {path} {why}; the pixel renderer is off."
        )
    });
}

fn material_of(rect: &Gd<ColorRect>) -> Option<Gd<ShaderMaterial>> {
    rect.get_material()?.try_cast::<ShaderMaterial>().ok()
}

impl PipelineParts {
    pub fn instantiate(chosen: Option<&Gd<PackedScene>>) -> Option<PipelineParts> {
        let scene = match chosen {
            Some(s) => s.clone(),
            None => match try_load::<PackedScene>(PIPELINE_SCENE) {
                Ok(s) => s,
                Err(_) => {
                    report_broken(PIPELINE_SCENE, "is missing");
                    return None;
                }
            },
        };
        let path = scene.get_path().to_string();
        let Some(root) = scene.instantiate() else {
            report_broken(&path, "cannot be instantiated");
            return None;
        };
        let parts = Self::collect(&root);
        if parts.is_none() {
            report_broken(
                &path,
                "does not have the expected viewports, cameras and shader rects",
            );
            root.free();
        }
        parts
    }

    fn collect(root: &Gd<Node>) -> Option<PipelineParts> {
        let resolve_rect = root.try_get_node_as::<ColorRect>("ResolveViewport/Resolve")?;
        let display_rect = root.try_get_node_as::<ColorRect>("PixelDisplay/Upscale")?;
        Some(PipelineParts {
            root: root.clone(),
            resolve_vp: root.try_get_node_as("ResolveViewport")?,
            color_vp: root.try_get_node_as("ResolveViewport/ColorViewport")?,
            meta_vp: root.try_get_node_as("ResolveViewport/MetaViewport")?,
            id_vp: root.try_get_node_as("ResolveViewport/IdViewport")?,
            color_cam: root.try_get_node_as("ResolveViewport/ColorViewport/ColorCamera")?,
            meta_cam: root.try_get_node_as("ResolveViewport/MetaViewport/MetaCamera")?,
            id_cam: root.try_get_node_as("ResolveViewport/IdViewport/IdCamera")?,
            resolve_mat: material_of(&resolve_rect)?,
            upscale_mat: material_of(&display_rect)?,
            display: root.try_get_node_as("PixelDisplay")?,
            display_rect,
        })
    }
}
