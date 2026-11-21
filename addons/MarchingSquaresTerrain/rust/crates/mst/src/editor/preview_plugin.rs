//! Lets the author see and set up the pixel-art look while working in the 3D editor.
//!
//! Intent: the pixel renderer, the animated effects and the project's heavier visual
//! features normally show only in a running game. An author editing a scene expects to
//! switch the editor viewport over to the pixel look, let effects animate or hold still,
//! add the effects a scene needs and apply a lighting preset, all from the viewport's own
//! menu bar.
//!
//! Responsibility: it owns those menu-bar controls and the decision of whether the editor
//! preview is on. It hands that decision to the scene's own pixel renderer when the scene
//! has one, and otherwise keeps a stand-in renderer of its own. It leaves the rendering,
//! the effects and what each preset changes to the nodes and code that implement them.
//!
//! Why a node: only an editor plugin can add controls to the 3D editor's menu bar and
//! follow the edited scene, and editor plugins live in the tree. The stand-in renderer it
//! keeps must itself be in the tree to draw.
//!
//! Why a new type: no built-in node routes the editor viewport through a custom renderer
//! or knows which effects and presets this project offers.
//!
//! Its menu-bar controls are instantiated from their scene when the plugin starts and freed
//! when it stops, because the editor's menu bar belongs to no scene an author edits and the
//! plugin's own lifetime decides when they exist.
//!
//! The stand-in renderer is created in code because whether it is needed is decided by the
//! scene being edited: it exists only while the preview is on and that scene has no
//! renderer of its own, and is freed as soon as either stops being true.
//!
//! Not exported: anything. The editor creates this node itself and never places it in a
//! scene or shows it in an Inspector, so none of its values can be authored. It holds
//! handles to the controls it added and whether the author asked for the preview, which
//! only exist while the editor runs.
use super::pixel_ui::{actions, presets::PRESETS, sections};
use crate::features;
use crate::node_children::free_node;
use crate::render::renderer::PixelRenderer;
use godot::classes::editor_plugin::CustomControlContainer;
use godot::classes::{
    Button, Control, EditorInterface, EditorPlugin, IEditorPlugin, MenuButton, Node, PopupMenu,
    Timer,
};
use godot::obj::Inherits;
use godot::prelude::*;
use sections::PREVIEW_LABEL;
const ANIMATE_SETTING: &str = "marching_squares_terrain/animate_effects_in_editor";
static PREVIEW_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn prelook_upon() -> bool {
    PREVIEW_ON.load(std::sync::atomic::Ordering::Relaxed)
}
#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelPreviewPlugin {
    controls: Option<Gd<Control>>,
    button: Option<Gd<Button>>,
    menu: Option<Gd<MenuButton>>,
    features_menu: Option<Gd<PopupMenu>>,
    wanted: bool,
    temp: Option<Gd<PixelRenderer>>,
    base: Base<EditorPlugin>,
}
fn part<T: Inherits<Node>>(root: &Gd<Control>, name: &str) -> Option<Gd<T>> {
    let found = root.try_get_node_as::<T>(name);
    if found.is_none() {
        godot_error!("Marching Squares Terrain: the pixel preview controls have no {name} node");
    }
    found
}
#[godot_api]
impl IEditorPlugin for PixelPreviewPlugin {
    fn enter_tree(&mut self) {
        let Some(controls) = super::editor_scenes::instantiate::<Control>(
            super::editor_scenes::PIXEL_PREVIEW_CONTROLS,
        ) else {
            return;
        };
        let gd = self.to_gd();
        if let Some(mut b) = part::<Button>(&controls, "PixelPreviewButton") {
            b.set_text(PREVIEW_LABEL);
            let tip = format!("{PREVIEW_LABEL}\n{}", b.get_tooltip_text());
            b.set_tooltip_text(&tip);
            b.connect("toggled", &gd.callable("on_toggled"));
            self.button = Some(b);
        }
        if let Some(mut a) = part::<Button>(&controls, "PixelAnimateButton") {
            let on = EditorInterface::singleton()
                .get_editor_settings()
                .filter(|es| es.has_setting(ANIMATE_SETTING))
                .and_then(|es| es.get_setting(ANIMATE_SETTING).try_to::<bool>().ok())
                .unwrap_or(false);
            crate::clock::set_animate_in_editor(on);
            a.set_pressed_no_signal(on);
            a.connect("toggled", &gd.callable("on_animate_toggled"));
        }
        if let Some(mut f) = part::<PopupMenu>(&controls, "PixelFeatures") {
            fill_features_menu(&mut f);
            f.connect("id_pressed", &gd.callable("on_feature"));
            f.connect("about_to_popup", &gd.callable("on_features_about_to_popup"));
            self.features_menu = Some(f);
        }
        if let Some(mut m) = part::<MenuButton>(&controls, "PixelWorldMenu") {
            if let Some(mut popup) = m.get_popup() {
                fill_world_menu(&mut popup, self.features_menu.as_mut());
                popup.connect("id_pressed", &gd.callable("on_menu"));
            }
            m.connect("about_to_popup", &gd.callable("on_menu_about_to_popup"));
            self.menu = Some(m);
        }
        if let Some(mut t) = part::<Timer>(&controls, "on_poll") {
            t.connect("timeout", &gd.callable("on_poll"));
        }
        let cb = gd.callable("on_project_settings_changed");
        self.base_mut().connect("project_settings_changed", &cb);
        self.base_mut()
            .add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &controls);
        self.controls = Some(controls);
    }
    fn exit_tree(&mut self) {
        PREVIEW_ON.store(false, std::sync::atomic::Ordering::Relaxed);
        self.drop_temp();
        self.button = None;
        self.menu = None;
        self.features_menu = None;
        if let Some(c) = self.controls.take() {
            self.base_mut()
                .remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_MENU, &c);
            c.free();
        }
        crate::clock::set_animate_in_editor(false);
    }
}
#[godot_api]
impl PixelPreviewPlugin {
    #[func]
    fn on_poll(&mut self) {
        self.renew();
    }
    #[func]
    fn on_toggled(&mut self, pressed: bool) {
        self.wanted = pressed;
        PREVIEW_ON.store(pressed, std::sync::atomic::Ordering::Relaxed);
        if let Some(mut r) = scene_renderer() {
            r.bind_mut().assign_studio_prelook(pressed);
        }
        self.renew();
    }
    #[func]
    fn on_animate_toggled(&mut self, pressed: bool) {
        crate::clock::set_animate_in_editor(pressed);
        if let Some(mut es) = EditorInterface::singleton().get_editor_settings() {
            es.set_setting(ANIMATE_SETTING, &pressed.to_variant());
        }
    }
    #[func]
    fn on_project_settings_changed(&mut self) {
        features::reingest();
    }
    #[func]
    fn on_feature(&mut self, token_id: i64) {
        if let Some(i) = features::ALL.get(token_id as usize) {
            features::assign_active(i.feature, !features::active(i.feature));
        }
        self.sync_feature_checks();
    }
    #[func]
    fn on_features_about_to_popup(&mut self) {
        self.sync_feature_checks();
    }
    #[func]
    fn on_menu(&mut self, token_id: i64) {
        let Some(ur) = self.base_mut().get_undo_redo() else {
            return;
        };
        let id = token_id as usize;
        if id < MENU_PRESET {
            if let Some((class, _, _)) = sections::ADDABLE.get(id) {
                if let Some(f) = features::for_category(class)
                    && !features::active(f)
                {
                    features::assign_active(f, true);
                    godot_print!(
                        "Pixel World: turned the \"{}\" feature on for this project.",
                        features::info_of(f).label
                    );
                }
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
        let Some(m) = &self.menu else { return };
        let Some(mut popup) = m.get_popup() else {
            return;
        };
        for (i, (class, _, unique)) in sections::ADDABLE.iter().enumerate() {
            let idx = popup.get_item_index(i as i32);
            popup.set_item_disabled(idx, *unique && actions::find_in_scene(class).is_some());
        }
    }
}
const MENU_PRESET: usize = 100;
const MENU_WIRE: usize = 200;
impl PixelPreviewPlugin {
    fn sync_feature_checks(&mut self) {
        let Some(f) = &mut self.features_menu else {
            return;
        };
        for (i, info) in features::ALL.iter().enumerate() {
            let idx = f.get_item_index(i as i32);
            f.set_item_checked(idx, features::active(info.feature));
        }
    }
    fn renew(&mut self) {
        let scene = EditorInterface::singleton().get_edited_scene_root();
        if let Some(r) = scene_renderer() {
            self.drop_temp();
            self.wanted = r.bind().is_editor_preview();
        } else if self.wanted && scene.is_some() {
            if self.temp.as_ref().is_none_or(|t| !t.is_instance_valid()) {
                let mut r = PixelRenderer::new_alloc();
                r.set_name("PixelPreviewRenderer");
                r.bind_mut().assign_studio_prelook(true);
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
fn scene_renderer() -> Option<Gd<PixelRenderer>> {
    let root: Gd<Node> = EditorInterface::singleton().get_edited_scene_root()?;
    if let Ok(r) = root.clone().try_cast::<PixelRenderer>() {
        return Some(r);
    }
    root.find_children_ex("*")
        .type_("PixelRenderer")
        .owned(false)
        .done()
        .iter_shared()
        .find_map(|n| n.try_cast::<PixelRenderer>().ok())
}
fn fill_features_menu(f: &mut Gd<PopupMenu>) {
    for (i, info) in features::ALL.iter().enumerate() {
        f.add_check_item_ex(info.label).id(i as i32).done();
        let idx = f.get_item_index(i as i32);
        f.set_item_tooltip(idx, &format!("{}\nOff: nodes of this feature build and draw nothing (their settings are kept). Saved in Project Settings > Marching Squares Terrain > Features.", info.hint));
    }
}
fn fill_world_menu(popup: &mut Gd<PopupMenu>, features_menu: Option<&mut Gd<PopupMenu>>) {
    popup.add_separator_ex().label("Add").done();
    for (i, (class, label, _)) in sections::ADDABLE.iter().enumerate() {
        match super::pixel_ui::widgets::glyph(sections::icon_for(class)) {
            Some(t) => popup.add_icon_item_ex(&t, *label).id(i as i32).done(),
            None => popup.add_item_ex(*label).id(i as i32).done(),
        }
    }
    popup
        .add_separator_ex()
        .label(sections::PRESET_HEADER)
        .done();
    for (i, p) in PRESETS.iter().enumerate() {
        popup
            .add_item_ex(p.moniker)
            .id((MENU_PRESET + i) as i32)
            .done();
        let idx = popup.get_item_index((MENU_PRESET + i) as i32);
        popup.set_item_tooltip(idx, &format!("Apply the {} preset to the scene's Day/Night, Sky, Weather and renderer god rays (undoable).", p.moniker));
    }
    popup.add_separator();
    popup
        .add_item_ex(sections::WIRE_LABEL)
        .id(MENU_WIRE as i32)
        .done();
    let idx = popup.get_item_index(MENU_WIRE as i32);
    popup.set_item_tooltip(idx, sections::WIRE_TOOLTIP);
    let Some(features_menu) = features_menu else {
        return;
    };
    popup.add_separator_ex().label("Project").done();
    features_menu.set_owner(Gd::null_arg());
    if let Some(mut parent) = features_menu.get_parent() {
        parent.remove_child(&*features_menu);
    }
    popup.add_submenu_node_item("Features", &*features_menu);
    let idx = popup.get_item_count() - 1;
    popup.set_item_tooltip(
        idx,
        "Switch the resource-intensive features on or off for the whole project.",
    );
}
