//! Lets the author see and set up the pixel-art look while working in the 3D editor.
//!
//! Intent: the pixel renderer, the animated effects and the project's heavier visual
//! features normally show only in a running game. An author editing a scene expects to
//! switch the editor viewport over to the pixel look, let effects animate or hold still,
//! add the effects a scene needs and apply a lighting preset, all from the viewport's own
//! menu bar. The same bar holds the switch between the builder's regular layout and its
//! Roblox Studio-style layout, so that switch is one click away from the viewport.
//!
//! Responsibility: it owns those menu-bar controls and the decision of whether the editor
//! preview is on. It hands that decision to the scene's own pixel renderer when the scene
//! has one, and otherwise keeps a stand-in renderer of its own. It leaves the rendering,
//! the effects and what each preset changes to the nodes and code that implement them. For
//! the layout switch it only records the choice in the person's editor settings; the builder
//! reacts to that setting on its own.
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
use crate::builder::layout::{self, Layout};
use crate::features;
use crate::internal_children::free_node;
use crate::render::renderer::PixelRenderer;
use godot::classes::editor_plugin::CustomControlContainer;
use super::preview_controls::PixelPreviewControls;
use godot::classes::{
    Button, Control, EditorInterface, EditorPlugin, IEditorPlugin, MenuButton, Node, PopupMenu,
};
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
    controls: Option<Gd<PixelPreviewControls>>,
    button: Option<Gd<Button>>,
    menu: Option<Gd<MenuButton>>,
    roblox_button: Option<Gd<Button>>,
    features_menu: Option<Gd<PopupMenu>>,
    wanted: bool,
    temp: Option<Gd<PixelRenderer>>,
    renderer: Option<Gd<PixelRenderer>>,
    base: Base<EditorPlugin>,
}
fn part<T: GodotClass>(found: Option<Gd<T>>, role: &str) -> Option<Gd<T>> {
    if found.is_none() {
        godot_error!(
            "Marching Squares Terrain: the pixel preview controls scene does not assign its {role}"
        );
    }
    found
}
#[godot_api]
impl IEditorPlugin for PixelPreviewPlugin {
    fn enter_tree(&mut self) {
        let Some(controls) = super::editor_scenes::instantiate::<PixelPreviewControls>(
            super::editor_scenes::PIXEL_PREVIEW_CONTROLS,
        ) else {
            return;
        };
        let gd = self.to_gd();
        let (preview, animate, roblox, features, world, poll) = {
            let c = controls.bind();
            (
                part(c.preview_button.clone(), "preview button"),
                part(c.animate_button.clone(), "animate button"),
                part(c.roblox_button.clone(), "Roblox layout button"),
                part(c.features_menu.clone(), "features menu"),
                part(c.world_menu.clone(), "pixel world menu"),
                part(c.poll_timer.clone(), "poll timer"),
            )
        };
        if let Some(mut b) = preview {
            b.set_text(PREVIEW_LABEL);
            let tip = format!("{PREVIEW_LABEL}\n{}", b.get_tooltip_text());
            b.set_tooltip_text(&tip);
            b.connect("toggled", &gd.callable("on_toggled"));
            self.button = Some(b);
        }
        if let Some(mut a) = animate {
            let on = EditorInterface::singleton()
                .get_editor_settings()
                .filter(|es| es.has_setting(ANIMATE_SETTING))
                .and_then(|es| es.get_setting(ANIMATE_SETTING).try_to::<bool>().ok())
                .unwrap_or(false);
            crate::clock::set_animate_in_editor(on);
            a.set_pressed_no_signal(on);
            a.connect("toggled", &gd.callable("on_animate_toggled"));
        }
        if let Some(mut r) = roblox {
            if let Some(t) = roblox_toggle_icon() {
                r.set_button_icon(&t);
            }
            r.set_pressed_no_signal(Layout::ingest() == Layout::Roblox);
            r.connect("toggled", &gd.callable("on_roblox_toggled"));
            self.roblox_button = Some(r);
        }
        if let Some(mut f) = features {
            fill_features_menu(&mut f);
            f.connect("id_pressed", &gd.callable("on_feature"));
            f.connect("about_to_popup", &gd.callable("on_features_about_to_popup"));
            self.features_menu = Some(f);
        }
        if let Some(mut m) = world {
            if let Some(mut popup) = m.get_popup() {
                fill_world_menu(&mut popup, self.features_menu.as_mut());
                popup.connect("id_pressed", &gd.callable("on_menu"));
            }
            m.connect("about_to_popup", &gd.callable("on_menu_about_to_popup"));
            self.menu = Some(m);
        }
        if let Some(mut t) = poll {
            t.connect("timeout", &gd.callable("on_poll"));
        }
        self.base_mut().connect(
            "project_settings_changed",
            &gd.callable("on_project_settings_changed"),
        );
        self.base_mut()
            .connect("scene_changed", &gd.callable("on_scene_changed"));
        let mut tree = self.base().get_tree();
        let watch = gd.callable("on_tree_node_came_or_went");
        tree.connect("node_added", &watch);
        tree.connect("node_removed", &watch);
        self.rescan_renderer();
        self.base_mut().add_control_to_container(
            CustomControlContainer::SPATIAL_EDITOR_MENU,
            &controls.clone().upcast::<Control>(),
        );
        self.controls = Some(controls);
    }
    fn exit_tree(&mut self) {
        let mut tree = self.base().get_tree();
        let watch = self.to_gd().callable("on_tree_node_came_or_went");
        for signal in ["node_added", "node_removed"] {
            if tree.is_connected(signal, &watch) {
                tree.disconnect(signal, &watch);
            }
        }
        PREVIEW_ON.store(false, std::sync::atomic::Ordering::Relaxed);
        self.drop_temp();
        self.renderer = None;
        self.button = None;
        self.roblox_button = None;
        self.menu = None;
        self.features_menu = None;
        if let Some(c) = self.controls.take() {
            let c = c.upcast::<Control>();
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
    fn on_scene_changed(&mut self, _root: Option<Gd<Node>>) {
        self.rescan_renderer();
    }
    #[func(gd_self)]
    fn on_tree_node_came_or_went(this: Gd<Self>, node: Gd<Node>) {
        renderer_came_or_went(this, node);
    }
    #[func]
    fn on_poll(&mut self) {
        self.renew();
    }
    #[func]
    fn on_toggled(&mut self, pressed: bool) {
        self.wanted = pressed;
        PREVIEW_ON.store(pressed, std::sync::atomic::Ordering::Relaxed);
        if let Some(mut r) = self.scene_renderer() {
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
    fn on_roblox_toggled(&mut self, pressed: bool) {
        layout::toggled(pressed, Layout::stored_before_roblox()).store();
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
            popup.set_item_disabled(idx, *unique && !actions::edited_scene_nodes(class).is_empty());
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
    fn rescan_renderer(&mut self) {
        self.renderer = edited_scene_renderer();
    }
    fn scene_renderer(&self) -> Option<Gd<PixelRenderer>> {
        self.renderer
            .clone()
            .filter(|r| r.is_instance_valid() && r.is_inside_tree())
    }
    fn renew(&mut self) {
        let scene = EditorInterface::singleton().get_edited_scene_root();
        if let Some(r) = self.scene_renderer() {
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
        let roblox = Layout::ingest() == Layout::Roblox;
        if let Some(r) = &mut self.roblox_button
            && r.is_pressed() != roblox
        {
            r.set_pressed_no_signal(roblox);
        }
    }
    fn drop_temp(&mut self) {
        free_node(self.temp.take());
    }
}
fn roblox_toggle_icon() -> Option<Gd<godot::classes::Texture2D>> {
    const PATH: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/roblox/ribbon.svg";
    let mut loader = godot::classes::ResourceLoader::singleton();
    if loader.exists(PATH) {
        return loader
            .load(PATH)
            .and_then(|r| r.try_cast::<godot::classes::Texture2D>().ok());
    }
    EditorInterface::singleton()
        .get_editor_theme()
        .and_then(|t| t.get_icon("TileMapDock", "EditorIcons"))
}
fn renderer_came_or_went(plugin: Gd<PixelPreviewPlugin>, node: Gd<Node>) {
    if node.try_cast::<PixelRenderer>().is_err() {
        return;
    }
    Callable::from_fn("rescan_renderer", move |_| {
        if plugin.is_instance_valid() {
            plugin.clone().bind_mut().rescan_renderer();
        }
    })
    .call_deferred(&[]);
}
fn edited_scene_renderer() -> Option<Gd<PixelRenderer>> {
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
