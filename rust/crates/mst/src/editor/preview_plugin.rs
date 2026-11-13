//! "Pixel Preview" toggle in the 3D viewport toolbar.
//!
//! The button drives `PixelRenderer::editor_preview` of the first `PixelRenderer` in the edited
//! scene. Scenes without one get a temporary default renderer (a child of this plugin, never
//! saved) so the preview works everywhere. The button follows the scene's stored state when
//! tabs are switched.
//!
//! Next to it, a "Pixel World" menu adds pixel effects (with their sun/environment/camera
//! refs wired), applies lighting presets and wires references — available whatever is
//! selected, so the first pixel node can be added without the Add Node dialog.

use godot::classes::editor_plugin::CustomControlContainer;
use godot::classes::{Button, EditorInterface, EditorPlugin, IEditorPlugin, MenuButton, Node, Texture2D};
use godot::prelude::*;

use super::pixel_ui::{actions, presets::PRESETS, sections};
use sections::PREVIEW_LABEL;
use crate::render::renderer::PixelRenderer;
use crate::util::free_node;

const ICON: &str = "res://addons/MarchingSquaresTerrain/editor/icons/pixel/pixel_preview.svg";
const POLL_SECONDS: f64 = 0.25;

/// State of the Pixel Preview toggle (read by the Pixel World panel's hint).
static PREVIEW_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True while the editor viewport is shown through the pixel renderer.
pub fn preview_on() -> bool {
    PREVIEW_ON.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelPreviewPlugin {
    button: Option<Gd<Button>>,
    menu: Option<Gd<MenuButton>>,
    /// Preview requested for scenes without their own `PixelRenderer`.
    wanted: bool,
    temp: Option<Gd<PixelRenderer>>,
    poll: f64,
    base: Base<EditorPlugin>,
}

#[godot_api]
impl IEditorPlugin for PixelPreviewPlugin {
    fn enter_tree(&mut self) {
        let mut b = Button::new_alloc();
        b.set_name("PixelPreviewButton");
        b.set_toggle_mode(true);
        b.set_flat(true);
        b.set_focus_mode(godot::classes::control::FocusMode::NONE);
        b.set_tooltip_text(&format!(
            "{PREVIEW_LABEL}\nShow the editor viewport through the pixel renderer (the scene's PixelRenderer, or a default one).\nEditor grid, axes and gizmos are hidden while it is on."
        ));
        if let Ok(icon) = try_load::<Texture2D>(ICON) {
            b.set_button_icon(&icon);
        }
        b.set_text(PREVIEW_LABEL);
        let cb = self.to_gd().callable("on_toggled");
        b.connect("toggled", &cb);
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &b);
        self.button = Some(b);
        let m = self.build_menu();
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &m);
        self.menu = Some(m);
        self.base_mut().set_process(true);
    }

    fn exit_tree(&mut self) {
        PREVIEW_ON.store(false, std::sync::atomic::Ordering::Relaxed);
        self.drop_temp();
        if let Some(b) = self.button.take() {
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &b);
            b.free();
        }
        if let Some(m) = self.menu.take() {
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &m);
            m.free();
        }
    }

    fn process(&mut self, delta: f64) {
        self.poll -= delta;
        if self.poll <= 0.0 {
            self.poll = POLL_SECONDS;
            self.refresh();
        }
    }
}

#[godot_api]
impl PixelPreviewPlugin {
    #[func]
    fn on_toggled(&mut self, pressed: bool) {
        self.wanted = pressed;
        PREVIEW_ON.store(pressed, std::sync::atomic::Ordering::Relaxed);
        if let Some(mut r) = scene_renderer() {
            r.bind_mut().set_editor_preview(pressed);
        }
        self.refresh();
    }

    #[func]
    fn on_menu(&mut self, id: i64) {
        let Some(ur) = self.base_mut().get_undo_redo() else { return };
        let id = id as usize;
        if id < MENU_PRESET {
            if let Some((class, _, _)) = sections::ADDABLE.get(id) {
                actions::add_effect(ur, class);
            }
        } else if id < MENU_WIRE {
            actions::apply_preset(ur, id - MENU_PRESET);
        } else {
            let n = actions::wire_references(ur);
            godot_print!("Pixel World: wired {n} reference(s).");
        }
    }

    #[func]
    fn on_menu_about_to_popup(&mut self) {
        // Disable "Add" for unique nodes that already exist.
        let Some(m) = &self.menu else { return };
        let Some(mut popup) = m.get_popup() else { return };
        for (i, (class, _, unique)) in sections::ADDABLE.iter().enumerate() {
            let idx = popup.get_item_index(i as i32);
            popup.set_item_disabled(idx, *unique && actions::find_in_scene(class).is_some());
        }
    }
}

const MENU_PRESET: usize = 100;
const MENU_WIRE: usize = 200;

impl PixelPreviewPlugin {
    fn build_menu(&mut self) -> Gd<MenuButton> {
        let mut m = MenuButton::new_alloc();
        m.set_name("PixelWorldMenu");
        m.set_text("Pixel World");
        m.set_flat(true);
        m.set_switch_on_hover(true);
        m.set_tooltip_text("Add pixel effects to the edited scene (sun / environment / camera refs are wired), apply a lighting preset.");
        let Some(mut popup) = m.get_popup() else { return m };
        popup.add_separator_ex().label("Add").done();
        for (i, (class, label, _)) in sections::ADDABLE.iter().enumerate() {
            let icon = format!("{}{}.svg", sections::ICON_DIR, sections::icon_for(class));
            match try_load::<Texture2D>(&icon) {
                Ok(t) => popup.add_icon_item_ex(&t, *label).id(i as i32).done(),
                Err(_) => popup.add_item_ex(*label).id(i as i32).done(),
            }
        }
        popup.add_separator_ex().label(sections::PRESET_HEADER).done();
        for (i, p) in PRESETS.iter().enumerate() {
            popup.add_item_ex(p.name).id((MENU_PRESET + i) as i32).done();
            let idx = popup.get_item_index((MENU_PRESET + i) as i32);
            popup.set_item_tooltip(idx, &format!("Apply the {} preset to the scene's Day/Night, Sky, Weather and renderer god rays (undoable).", p.name));
        }
        popup.add_separator();
        popup.add_item_ex(sections::WIRE_LABEL).id(MENU_WIRE as i32).done();
        let idx = popup.get_item_index(MENU_WIRE as i32);
        popup.set_item_tooltip(idx, sections::WIRE_TOOLTIP);
        let gd = self.to_gd();
        popup.connect("id_pressed", &gd.callable("on_menu"));
        m.connect("about_to_popup", &gd.callable("on_menu_about_to_popup"));
        m
    }

    fn refresh(&mut self) {
        let scene = EditorInterface::singleton().get_edited_scene_root();
        if let Some(r) = scene_renderer() {
            // The scene's own renderer wins (its `editor_preview` is saved with the scene).
            self.drop_temp();
            self.wanted = r.bind().is_editor_preview();
        } else if self.wanted && scene.is_some() {
            if self.temp.as_ref().is_none_or(|t| !t.is_instance_valid()) {
                let mut r = PixelRenderer::new_alloc();
                r.set_name("PixelPreviewRenderer");
                r.bind_mut().set_editor_preview(true);
                self.base_mut().add_child(&r);
                self.temp = Some(r);
            }
        } else {
            self.drop_temp();
        }
        let wanted = self.wanted && scene.is_some();
        PREVIEW_ON.store(wanted, std::sync::atomic::Ordering::Relaxed);
        let wanted = self.wanted;
        if let Some(b) = &mut self.button {
            b.set_pressed_no_signal(wanted);
        }
    }

    fn drop_temp(&mut self) {
        free_node(self.temp.take());
    }
}

/// First `PixelRenderer` in the edited scene (including the root itself).
fn scene_renderer() -> Option<Gd<PixelRenderer>> {
    let root: Gd<Node> = EditorInterface::singleton().get_edited_scene_root()?;
    if let Ok(r) = root.clone().try_cast::<PixelRenderer>() {
        return Some(r);
    }
    root.find_children_ex("*").type_("PixelRenderer").owned(false).done().iter_shared().find_map(|n| n.try_cast::<PixelRenderer>().ok())
}
