//! Editor-side plumbing for `PixelRenderer::editor_preview`: finds the editor's first 3D
//! viewport, its camera and the `SubViewportContainer` the pixelated image is overlaid on.
//!
//! Only ever called when `Engine::is_editor_hint()` is true (`EditorInterface` does not exist
//! in exported games).

use godot::classes::{Camera3D, Control, EditorInterface, SubViewport};
use godot::prelude::*;

/// The editor 3D viewport the preview is attached to.
pub struct EditorHost {
    pub viewport: Gd<SubViewport>,
    /// The viewport's `SubViewportContainer` (the overlay is added as its child).
    pub container: Gd<Control>,
}

impl EditorHost {
    /// Editor viewport 0 (top-left in multi-viewport layouts).
    pub fn find() -> Option<EditorHost> {
        let viewport = EditorInterface::singleton().get_editor_viewport_3d_ex().idx(0).done()?;
        let container = viewport.get_parent()?.try_cast::<Control>().ok()?;
        Some(EditorHost { viewport, container })
    }

    pub fn is_valid(&self) -> bool {
        self.viewport.is_instance_valid() && self.container.is_instance_valid()
    }

    pub fn camera(&self) -> Option<Gd<Camera3D>> {
        self.viewport.get_camera_3d()
    }

    /// On-screen size in pixels (the container; the viewport itself may be scaled down).
    pub fn screen_size(&self) -> Vector2 {
        self.container.get_size()
    }

    /// False while another main screen (2D, Script, …) is shown: the preview then stops rendering.
    pub fn is_shown(&self) -> bool {
        self.container.is_visible_in_tree()
    }
}
