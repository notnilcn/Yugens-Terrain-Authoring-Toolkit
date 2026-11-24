//! The off-screen machinery behind the pixel renderer, as one swappable scene.
//!
//! Intent: the passes that turn the 3D view into pixel art (the low-resolution colour view,
//! the views that carry depth, normals and outline groups, the pass that resolves them into
//! one picture, and the layer that scales that picture up to the screen) are authored
//! together in a scene. An author who wants a different arrangement copies that scene,
//! edits it, and hands it to the renderer; this node, at the root of such a scene, is how
//! the scene says which of its parts plays which role.
//!
//! Responsibility: it names its own parts and reports, in the editor, any role left
//! unfilled. It does not drive them: sizing, aiming and feeding the passes every frame stays
//! with the renderer, which takes the parts from here once, when the pipeline comes to life.
//! This file also decides when a pipeline exists, bringing in the author's scene or the
//! built-in one when none was chosen, and is the one place that knows where the built-in
//! scene lives.
//!
//! Why a node: the parts it names are nodes in the same scene, and the only way for a scene
//! to point at its own nodes so that a rename in the editor follows along is from a node
//! inside it.
//!
//! Why a new type: a plain node at the root can hold the parts but cannot say which is
//! which, so the renderer would have to find them by name, and an edited copy of the scene
//! would break silently instead of telling its author what is missing.
//!
//! Nodes created in code: the pipeline is instantiated only while pixel output is wanted,
//! in the game or in an editor preview, and freed when it is not, so it cannot be a
//! permanent part of the scene the renderer is placed in; what it consists of is still
//! authored in a scene.
use godot::classes::{
    Camera3D, CanvasLayer, ColorRect, INode, Node, PackedScene, ShaderMaterial, SubViewport,
};
use godot::prelude::*;
use std::sync::Once;

const PIPELINE_SCENE: &str = "res://addons/MarchingSquaresTerrain/render/pixel_pipeline.tscn";

#[derive(GodotClass)]
#[class(tool, init, base = Node)]
pub struct PixelPipeline {
    #[export]
    resolve_viewport: Option<Gd<SubViewport>>,
    #[export]
    color_viewport: Option<Gd<SubViewport>>,
    #[export]
    meta_viewport: Option<Gd<SubViewport>>,
    #[export]
    id_viewport: Option<Gd<SubViewport>>,
    #[export]
    color_camera: Option<Gd<Camera3D>>,
    #[export]
    meta_camera: Option<Gd<Camera3D>>,
    #[export]
    id_camera: Option<Gd<Camera3D>>,
    #[export]
    resolve_rect: Option<Gd<ColorRect>>,
    #[export]
    #[var(get = get_display)]
    display: Option<Gd<CanvasLayer>>,
    #[export]
    #[var(get = get_upscale_rect)]
    upscale_rect: Option<Gd<ColorRect>>,
    base: Base<Node>,
}

#[godot_api]
impl PixelPipeline {
    #[func]
    fn get_display(&self) -> Option<Gd<CanvasLayer>> {
        self.display.clone().filter(|d| d.is_instance_valid())
    }
    #[func]
    fn get_upscale_rect(&self) -> Option<Gd<ColorRect>> {
        self.upscale_rect.clone().filter(|r| r.is_instance_valid())
    }
}

#[godot_api]
impl INode for PixelPipeline {
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        let roles: [(&str, bool); 10] = [
            ("Resolve Viewport", self.resolve_viewport.is_some()),
            ("Color Viewport", self.color_viewport.is_some()),
            ("Meta Viewport", self.meta_viewport.is_some()),
            ("Id Viewport", self.id_viewport.is_some()),
            ("Color Camera", self.color_camera.is_some()),
            ("Meta Camera", self.meta_camera.is_some()),
            ("Id Camera", self.id_camera.is_some()),
            ("Resolve Rect", self.resolve_rect.is_some()),
            ("Display", self.display.is_some()),
            ("Upscale Rect", self.upscale_rect.is_some()),
        ];
        for (role, set) in roles {
            if !set {
                w.push(&format!("Assign {role}: the pixel renderer cannot run without it."));
            }
        }
        for (role, rect) in [
            ("Resolve Rect", &self.resolve_rect),
            ("Upscale Rect", &self.upscale_rect),
        ] {
            if rect.as_ref().is_some_and(|r| material_of(r).is_none()) {
                w.push(&format!("Give {role} a ShaderMaterial."));
            }
        }
        w
    }
}

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
    pub display_rect: Gd<ColorRect>,
    display: Option<Gd<CanvasLayer>>,
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
        let parts = match root.clone().try_cast::<PixelPipeline>() {
            Ok(p) => {
                let parts = Self::collect(&p.bind());
                if parts.is_none() {
                    report_broken(
                        &path,
                        "leaves some of its viewports, cameras or shader rects unassigned",
                    );
                }
                parts
            }
            Err(_) => {
                report_broken(&path, "does not have a PixelPipeline at its root");
                None
            }
        };
        if parts.is_none() {
            root.free();
        }
        parts
    }

    fn collect(p: &PixelPipeline) -> Option<PipelineParts> {
        let resolve_rect = p.resolve_rect.clone()?;
        let display_rect = p.upscale_rect.clone()?;
        Some(PipelineParts {
            root: p.to_gd().upcast(),
            resolve_vp: p.resolve_viewport.clone()?,
            color_vp: p.color_viewport.clone()?,
            meta_vp: p.meta_viewport.clone()?,
            id_vp: p.id_viewport.clone()?,
            color_cam: p.color_camera.clone()?,
            meta_cam: p.meta_camera.clone()?,
            id_cam: p.id_camera.clone()?,
            resolve_mat: material_of(&resolve_rect)?,
            upscale_mat: material_of(&display_rect)?,
            display_rect,
            display: Some(p.display.clone()?),
        })
    }

    pub fn lift_display_rect_out(&mut self) {
        if let Ok(mut p) = self.root.clone().try_cast::<PixelPipeline>() {
            let mut p = p.bind_mut();
            p.display = None;
            p.upscale_rect = None;
        }
        if let Some(mut display) = self.display.take() {
            display.remove_child(&self.display_rect);
            self.root.remove_child(&display);
            display.free();
        }
    }
}
