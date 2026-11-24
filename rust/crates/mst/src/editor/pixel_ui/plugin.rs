//! The editor's pixel-art workspace: a toolbar, a settings strip and a world panel in the 3D
//! editor for building and tuning a pixel-art scene without hunting through the Inspector.
//!
//! Intent: an author working in the 3D view picks a part of the pixel look, such as the
//! renderer, the sky, weather or an effect, and gets its most useful settings in one strip
//! beside the viewport. A side panel shows which pixel nodes the scene has, adds missing
//! ones with their references filled in, and applies lighting presets, all undoable.
//!
//! Responsibility: it presents and edits settings that live on the scene's own nodes and
//! records every change in the editor's undo history. It owns no settings itself, and it
//! leaves what each setting means to the node that has it; deciding which settings are worth
//! showing, and in what order, is its own job.
//!
//! Why a node: the editor only accepts extensions to its interface as plugin nodes in its
//! own tree, which also tell it when the plugin is enabled, which object is being edited
//! and when the 3D view is shown.
//!
//! Why a new type: the built-in Inspector lists every property of one node at a time; it
//! cannot gather the important settings of several pixel nodes into one place or add a
//! wired-up effect in one step.
//!
//! Nodes created in code: the toolbar's buttons, the strip's rows and the world panel's
//! rows are built from the data tables and from what the edited scene contains, and are
//! rebuilt whenever either changes. The world panel's fixed parts, including the clock that
//! keeps the strip in step with edits made elsewhere, are declared in its own scene, which
//! is instantiated when the plugin starts because the editor's docks belong to no scene an
//! author edits.
//!
//! Not exported: everything. No scene ever holds this node, so there is no Inspector in
//! which an author could set anything; what it keeps is the current selection and page and
//! handles to the controls it built, all of which last only while the editor is open.
use super::actions;
use super::census::{SceneCensus, TERRAIN};
use super::presets::PRESETS;
use super::world_panel::PixelWorldPanel;
use std::cell::Cell;
use super::sections::{self, ADDABLE, Kind, SECTIONS, Sec, Section, Store, icon_for};
use super::widgets as w;
use crate::editor::ui_kit::{self, CtlKind, Strip};
use godot::classes::control::{FocusMode, SizeFlags};
use godot::global::MouseButton;
use godot::classes::editor_plugin::CustomControlContainer;
use godot::classes::object::ConnectFlags;
use godot::classes::undo_redo::MergeMode;
use godot::classes::{
    Input, Button, CheckBox, ColorPickerButton, Control, EditorInterface, EditorPlugin, EditorSpinSlider,
    EditorUndoRedoManager, HBoxContainer, IEditorPlugin, Label, Node,
    Object, OptionButton, ProgressBar, RenderingServer, ScrollContainer, ShaderMaterial,
    Timer, VBoxContainer,
};
use godot::prelude::*;
#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelizerEditorPlugin {
    selected: Option<Gd<Node>>,
    sec: Option<Sec>,
    page: usize,
    toolbar: Option<Gd<ScrollContainer>>,
    peel: Option<Gd<ScrollContainer>>,
    world: Option<Gd<PixelWorldPanel>>,
    effect_rows: Option<Gd<VBoxContainer>>,
    preview_hint: Option<(Gd<Label>, bool)>,
    census: SceneCensus,
    buttons: Vec<(Sec, Gd<Button>)>,
    controls: Vec<(usize, Gd<Control>)>,
    progress: Vec<ProgressRow>,
    shown_target: Option<InstanceId>,
    visible: bool,
    shown_for_terrain: bool,
    refresh_timer: Option<Gd<Timer>>,
    drag: Option<SliderDrag>,
    base: Base<EditorPlugin>,
}
struct SliderDrag {
    field: usize,
    action: String,
    fd: &'static sections::Field,
    before: Variant,
    target: Gd<Node>,
}
struct ProgressRow {
    field: usize,
    bar: Gd<ProgressBar>,
    cell: Gd<Control>,
}
const BAKE_LUT_MENU: &str = "Bake PixelPalette LUT to PNG";
use sections::{WIRE_LABEL, WIRE_TOOLTIP};
fn field_tooltip(class: &str, fd: &sections::Field) -> String {
    let place = match fd.store {
        Store::Prop => format!("{class}.{}", fd.prop),
        Store::Axis(a) => format!("{class}.{}.{}", fd.prop, ["x", "y", "z"][a as usize % 3]),
        Store::Shader(m) => format!("{class}.{m} shader parameter \"{}\"", fd.prop),
    };
    let range = match fd.sort {
        Kind::Float { min, max, .. } => format!("\nRange {min} to {max} (drag, or click to type)."),
        Kind::Int {
            min,
            max,
            greater: false,
        } => format!("\nRange {min} to {max} (drag, or click to type)."),
        Kind::Int {
            min,
            max,
            greater: true,
        } => format!("\nRange {min} to {max}+ (drag, or click to type a larger value)."),
        _ => String::new(),
    };
    let hint = sections::field_hint(fd.prop)
        .map(|h| format!("\n{h}"))
        .unwrap_or_default();
    format!("{}{hint}\n{place}{range}", fd.label)
}
fn num(v: &Variant) -> f64 {
    v.try_to::<f64>()
        .or_else(|_| v.try_to::<i64>().map(|i| i as f64))
        .or_else(|_| v.try_to::<bool>().map(|b| b as i32 as f64))
        .unwrap_or(0.0)
}
#[godot_api]
impl IEditorPlugin for PixelizerEditorPlugin {
    fn enter_tree(&mut self) {
        let toolbar = self.build_toolbar();
        let strip = self.build_strip();
        self.base_mut()
            .add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, &toolbar);
        self.base_mut()
            .add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &strip);
        toolbar.clone().set_visible(false);
        strip.clone().set_visible(false);
        self.toolbar = Some(toolbar);
        self.peel = Some(strip);
        if let Some(world) = self.build_world_panel() {
            let panel = world.clone().upcast::<Control>();
            self.base_mut().add_control_to_container(
                CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT,
                &panel,
            );
            panel.clone().set_visible(false);
            self.world = Some(world);
        }
        let bake = Callable::from_object_method(&self.to_gd(), "on_bake_lut");
        self.base_mut().add_tool_menu_item(BAKE_LUT_MENU, &bake);
        let cb = Callable::from_object_method(&self.to_gd(), "on_history_changed");
        if let Some(mut ur) = self.base_mut().get_undo_redo() {
            ur.connect_flags("version_changed", &cb, ConnectFlags::DEFERRED);
        }
        let gd = self.to_gd();
        self.base_mut()
            .connect("scene_changed", &gd.callable("on_scene_changed"));
        if let Some(mut selection) = EditorInterface::singleton().get_selection() {
            selection.connect("selection_changed", &gd.callable("on_editor_selection_changed"));
        }
        let mut tree = self.base().get_tree();
        let watch = gd.callable("on_tree_node_came_or_went");
        tree.connect("node_added", &watch);
        tree.connect("node_removed", &watch);
        self.recount();
    }
    fn exit_tree(&mut self) {
        self.finish_drag();
        self.base_mut().remove_tool_menu_item(BAKE_LUT_MENU);
        if let Some(t) = self.toolbar.take() {
            self.base_mut().remove_control_from_container(
                CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT,
                &t,
            );
            t.free();
        }
        if let Some(s) = self.peel.take() {
            self.base_mut()
                .remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &s);
            s.free();
        }
        let on_selection = self.to_gd().callable("on_editor_selection_changed");
        if let Some(mut selection) = EditorInterface::singleton().get_selection()
            && selection.is_connected("selection_changed", &on_selection)
        {
            selection.disconnect("selection_changed", &on_selection);
        }
        let mut tree = self.base().get_tree();
        let watch = self.to_gd().callable("on_tree_node_came_or_went");
        for signal in ["node_added", "node_removed"] {
            if tree.is_connected(signal, &watch) {
                tree.disconnect(signal, &watch);
            }
        }
        if let Some(wp) = self.world.take() {
            let wp = wp.upcast::<Control>();
            self.base_mut().remove_control_from_container(
                CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT,
                &wp,
            );
            wp.free();
        }
        self.effect_rows = None;
        self.preview_hint = None;
        self.refresh_timer = None;
        self.buttons.clear();
        self.controls.clear();
        self.progress.clear();
    }
    fn get_plugin_name(&self) -> GString {
        "Pixelizer".into()
    }
    fn handles(&self, entity: Gd<Object>) -> bool {
        super::handles(&entity)
    }
    fn edit(&mut self, entity: Option<Gd<Object>>) {
        self.finish_drag();
        self.shown_for_terrain = false;
        self.selected = entity.and_then(|o| o.try_cast::<Node>().ok());
        if let Some(sel) = &self.selected {
            let class = sections::workspace_class(|c| sel.is_class(c)).unwrap_or(sections::RENDERER);
            let keep = match self.sec {
                Some(s) if sections::has_own_section(class) => {
                    sections::slice_ui(s).is_some_and(|section| section.class == Some(class))
                }
                Some(_) => true,
                None => false,
            };
            if !keep {
                self.sec = Some(sections::default_section(class));
                self.page = 0;
            }
        }
        if self.toolbar.is_some() {
            self.select_section(self.sec.unwrap_or(Sec::Renderer));
        }
    }
    fn make_visible(&mut self, shown: bool) {
        self.shown_for_terrain = false;
        self.show_workspace(shown);
        if !shown && self.sec == Some(Sec::GridFog) {
            self.to_gd().call_deferred("keep_grid_fog_for_terrain", &[]);
        }
    }
}
#[godot_api]
impl PixelizerEditorPlugin {
    #[func]
    fn keep_grid_fog_for_terrain(&mut self) {
        if self.visible || self.sec != Some(Sec::GridFog) {
            return;
        }
        let Some(terrain) = Self::lone_selected_terrain() else {
            return;
        };
        self.selected = Some(terrain);
        self.shown_for_terrain = true;
        self.show_workspace(true);
    }
    #[func]
    fn on_editor_selection_changed(&mut self) {
        if !self.shown_for_terrain {
            return;
        }
        match Self::lone_selected_terrain() {
            Some(terrain) => {
                if self.selected.as_ref() != Some(&terrain) {
                    self.selected = Some(terrain);
                    self.rebuild_strip();
                }
            }
            None => {
                self.shown_for_terrain = false;
                self.selected = None;
                self.show_workspace(false);
            }
        }
    }
    #[func]
    fn on_refresh_timer(&mut self) {
        if self.drag.is_some() && !Input::singleton().is_mouse_button_pressed(MouseButton::LEFT) {
            self.finish_drag();
        }
        if !self.visible {
            return;
        }
        self.renew();
        self.update_preview_hint(false);
    }
    #[func]
    fn on_scene_changed(&mut self, _root: Option<Gd<Node>>) {
        self.recount();
    }
    #[func(gd_self)]
    fn on_tree_node_came_or_went(this: Gd<Self>, node: Gd<Node>) {
        scene_node_came_or_went(this, node);
    }
    #[func]
    fn on_section(&mut self, idx: i64) {
        if let Some(s) = SECTIONS.get(idx as usize) {
            self.select_section(s.sec);
            if let Some(class) = s.class.filter(|c| *c != TERRAIN) {
                self.select_first_unless_selected(class);
            }
        }
    }
    #[func]
    fn on_page(&mut self, tab: i64) {
        let tabs = sections::class_tabs(self.sec.unwrap_or(Sec::Renderer));
        if let Some(t) = tabs.get(tab.max(0) as usize) {
            self.sec = Some(t.sec);
            self.page = t.page;
        }
        self.to_gd().call_deferred("rebuild_strip_now", &[]);
    }
    #[func]
    fn rebuild_strip_now(&mut self) {
        self.rebuild_strip();
    }
    #[func]
    fn on_history_changed(&mut self) {
        if self.visible {
            self.renew();
        }
    }
    #[func]
    fn on_float(&mut self, value: f64, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let v = match fd.sort {
            Kind::Int { .. } => (value.round() as i64).to_variant(),
            _ => value.to_variant(),
        };
        if let Some(drag) = &self.drag
            && drag.field == field as usize
        {
            if let Some(raw) = Self::raw_with(&drag.target, drag.fd, &v) {
                Self::write_raw(&drag.target, drag.fd, &raw);
            }
            return;
        }
        self.assign_worth(field as usize, v, false);
    }
    #[func]
    fn on_slider_grabbed(&mut self, field: i64) {
        self.finish_drag();
        let Some(s) = self.current() else { return };
        let Some(fd) = s.fields.get(field as usize) else {
            return;
        };
        let Some(target) = self.target(s) else { return };
        self.drag = Some(SliderDrag {
            field: field as usize,
            action: format!("Set {} {}", s.label, fd.label),
            fd,
            before: Self::raw_of(&target, fd),
            target,
        });
    }
    #[func]
    fn on_slider_released(&mut self, _field: i64) {
        self.finish_drag();
    }
    #[func]
    fn on_bool(&mut self, on: bool, field: i64) {
        self.assign_worth(field as usize, on.to_variant(), false);
    }
    #[func]
    fn on_color(&mut self, c: Color, field: i64) {
        self.assign_worth(field as usize, c.to_variant(), true);
    }
    #[func]
    fn upon_deed(&mut self, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let Kind::Action(method) = fd.sort else {
            return;
        };
        let Some(current) = self.current() else {
            return;
        };
        let Some(mut target) = self.target(current) else {
            return;
        };
        let r = target.call(method, &[]);
        if !r.is_nil() {
            godot_print!("{}: {method}() -> {r}", target.get_name());
        }
        self.renew();
    }
    #[func]
    fn upon_abort(&mut self, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let Kind::Progress { cancel, .. } = fd.sort else {
            return;
        };
        let Some(current) = self.current() else {
            return;
        };
        if let Some(mut target) = self.target(current) {
            target.call(cancel, &[]);
        }
        self.renew();
    }
    #[func]
    fn on_option(&mut self, index: i64, field: i64) {
        let Some(fd) = self.field(field) else { return };
        if let Kind::Enum(items) = fd.sort
            && let Some((_, id)) = items.get(index as usize)
        {
            self.assign_worth(field as usize, id.to_variant(), false);
        }
    }
    #[func]
    fn on_bake_lut(&mut self) {
        let edited = EditorInterface::singleton()
            .get_inspector()
            .and_then(|i| i.get_edited_object());
        let mut palette =
            edited.and_then(|o| o.try_cast::<crate::render::palette::PixelPalette>().ok());
        if palette.is_none() {
            let renderer = self
                .selected
                .clone()
                .filter(|s| s.is_instance_valid() && s.is_class(sections::RENDERER));
            palette = renderer.and_then(|r| {
                r.get("palette")
                    .try_to::<Gd<crate::render::palette::PixelPalette>>()
                    .ok()
            });
        }
        match palette {
            Some(mut p) => {
                let path = p.get("output_path").try_to::<GString>().unwrap_or_default();
                p.bind_mut().save_lut_png(path);
            }
            None => godot_warn!(
                "Pixelizer: select a PixelPalette (or a PixelRenderer with a palette), then use Tools > {BAKE_LUT_MENU}."
            ),
        }
    }
    #[func]
    fn on_add(&mut self, class: GString) {
        if let Some(ur) = self.undo() {
            actions::add_effect(ur, &class.to_string());
        }
    }
    #[func]
    fn on_select_existing(&mut self, class: GString) {
        let class = class.to_string();
        let all = actions::edited_scene_nodes(&class);
        if all.is_empty() {
            self.on_add(GString::from(class.as_str()));
            return;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|s| all.iter().position(|n| n == s));
        let next = cur.map(|i| (i + 1) % all.len()).unwrap_or(0);
        EditorInterface::singleton().call_deferred("edit_node", &[all[next].to_variant()]);
    }
    #[func]
    fn on_preset(&mut self, idx: i64) {
        if let Some(ur) = self.undo() {
            actions::apply_preset(ur, idx as usize);
        }
    }
    #[func]
    fn on_wire(&mut self) {
        if let Some(ur) = self.undo() {
            let n = actions::wire_references(ur);
            godot_print!("Pixel World: wired {n} reference(s).");
        }
    }
}
impl PixelizerEditorPlugin {
    fn show_workspace(&mut self, shown: bool) {
        if !shown {
            self.finish_drag();
        }
        self.visible = shown;
        if let Some(t) = self.refresh_timer.as_mut() {
            if shown { t.start() } else { t.stop() }
        }
        if let Some(t) = &mut self.toolbar {
            t.set_visible(shown);
        }
        if let Some(s) = &mut self.peel {
            s.set_visible(shown);
        }
        if let Some(wp) = &mut self.world {
            wp.set_visible(shown);
        }
        if shown {
            self.census = SceneCensus::of_edited_scene();
            self.rebuild_strip();
            self.rebuild_world();
        }
    }
    fn lone_selected_terrain() -> Option<Gd<Node>> {
        let selection = EditorInterface::singleton().get_selection()?;
        let nodes = selection.get_selected_nodes();
        match nodes.len() {
            1 => nodes.get(0).filter(|n| n.is_class(TERRAIN)),
            _ => None,
        }
    }
    fn build_toolbar(&mut self) -> Gd<ScrollContainer> {
        let gd = self.to_gd();
        let mut tb = ui_kit::Toolbar::new("PixelizerToolbar");
        for (i, s) in SECTIONS.iter().enumerate() {
            if sections::head(s.sec) != s.sec {
                continue;
            }
            if s.group_start {
                tb.separator();
            }
            let b = tb.button(
                s.label,
                s.tooltip,
                w::glyph(s.glyph),
                &Callable::from_object_method(&gd, "on_section").bind(&[(i as i64).to_variant()]),
            );
            self.buttons.push((s.sec, b));
        }
        tb.separator();
        tb.base
    }
    fn build_strip(&mut self) -> Gd<ScrollContainer> {
        ui_kit::strip_root("PixelizerAttributes")
    }
    fn select_section(&mut self, sec: Sec) {
        if self.sec != Some(sec) {
            self.page = 0;
        }
        self.sec = Some(sec);
        let head = sections::head(sec);
        for (s, b) in &mut self.buttons {
            b.set_pressed_no_signal(*s == head);
        }
        self.rebuild_strip();
    }
    fn current(&self) -> Option<&'static Section> {
        sections::slice_ui(self.sec.unwrap_or(Sec::Renderer))
    }
    fn field(&self, idx: i64) -> Option<&'static sections::Field> {
        self.current()?.fields.get(idx as usize)
    }
    fn target(&self, s: &Section) -> Option<Gd<Node>> {
        let class = s.class?;
        if class == TERRAIN {
            return self
                .selected
                .as_ref()
                .filter(|sel| sel.is_instance_valid() && sel.is_inside_tree())
                .and_then(actions::terrain_of)
                .or_else(|| actions::edited_scene_nodes(TERRAIN).into_iter().next());
        }
        self.selected
            .clone()
            .filter(|sel| sel.is_instance_valid() && sel.is_class(class) && sel.is_inside_tree())
    }
    fn select_first_unless_selected(&mut self, class: &str) {
        let selected_fits = self
            .selected
            .as_ref()
            .is_some_and(|sel| sel.is_instance_valid() && sel.is_class(class));
        if selected_fits {
            return;
        }
        if let Some(first) = actions::edited_scene_nodes(class).into_iter().next() {
            EditorInterface::singleton().call_deferred("edit_node", &[first.to_variant()]);
        }
    }
    fn recount(&mut self) {
        RECOUNT_QUEUED.set(false);
        let census = SceneCensus::of_edited_scene();
        if census == self.census {
            return;
        }
        self.census = census;
        if self.visible {
            self.rebuild_world();
            if self.current().is_some_and(|s| self.target(s).is_none()) {
                self.rebuild_strip();
            }
        }
    }
    fn shader_material(node: &Gd<Node>, prop: &str) -> Option<Gd<ShaderMaterial>> {
        node.get(prop).try_to::<Gd<ShaderMaterial>>().ok()
    }
    fn scan_in(node: &Gd<Node>, fd: &sections::Field) -> Variant {
        match fd.store {
            Store::Prop => node.get(fd.prop),
            Store::Axis(a) => {
                let v = node.get(fd.prop);
                if let Ok(v3) = v.try_to::<Vector3>() {
                    [v3.x, v3.y, v3.z][a as usize % 3].to_variant()
                } else if let Ok(v2) = v.try_to::<Vector2>() {
                    [v2.x, v2.y][a as usize % 2].to_variant()
                } else {
                    Variant::nil()
                }
            }
            Store::Shader(m) => {
                let Some(mat) = Self::shader_material(node, m) else {
                    return Variant::nil();
                };
                let v = mat.get_shader_parameter(fd.prop);
                if !v.is_nil() {
                    return v;
                }
                match mat.get_shader() {
                    Some(sh) => RenderingServer::singleton()
                        .shader_get_parameter_default(sh.get_rid(), fd.prop),
                    None => Variant::nil(),
                }
            }
        }
    }
    fn clear_strip(&mut self) -> Option<Strip> {
        self.finish_drag();
        let mut root = self.peel.clone()?;
        for mut c in root.get_children().iter_shared() {
            root.remove_child(&c);
            c.queue_free();
        }
        self.controls.clear();
        self.progress.clear();
        Some(Strip::reassemble(root))
    }
    fn rebuild_strip(&mut self) {
        let Some(mut strip) = self.clear_strip() else {
            return;
        };
        let gd = self.to_gd();
        let Some(s) = self.current() else {
            return;
        };
        let Some(h) = sections::slice_ui(sections::head(s.sec)) else {
            return;
        };
        let mut title = strip.heading(h.label, w::glyph(h.glyph), h.tooltip);
        title.set_name("SectionTitle");
        let target = self.target(s);
        self.shown_target = target.as_ref().map(|t| t.instance_id());
        let class = s.class.unwrap_or(sections::RENDERER);
        let Some(target) = target else {
            let mut b = strip.button(
                "",
                &format!("Add {}", s.label),
                &Callable::from_object_method(&gd, "on_add")
                    .bind(&[GString::from(class).to_variant()]),
            );
            if let Some(ic) = w::glyph(s.glyph) {
                b.set_button_icon(&ic);
            }
            b.set_tooltip_text(&format!(
                "Add a {class} to the edited scene (references wired, undoable)."
            ));
            if self.census.has(class) {
                strip.hint(&format!(
                    "Select a {class} to edit it here, or Add another."
                ));
            } else if sections::is_terrain_child(class) {
                if self.census.count(TERRAIN) == 1 {
                    strip.hint(&format!("No {class} in this scene: Add puts one under the terrain."));
                } else if self.census.has(TERRAIN) {
                    strip.hint(&format!("No {class} in this scene: select a terrain, then Add puts one under it."));
                } else {
                    b.set_disabled(true);
                    strip.hint(&format!(
                        "{} needs a MarchingSquaresTerrain: add a terrain to the scene first.",
                        s.label
                    ));
                }
            } else {
                strip.hint(&format!("No {class} in this scene."));
            }
            self.update_toolbar();
            return;
        };
        let tabs = sections::class_tabs(s.sec);
        if tabs.len() > 1 {
            let page = self.page.min(sections::pages(s.sec).len().max(1) - 1);
            let cur = tabs
                .iter()
                .position(|t| t.sec == s.sec && t.page == page)
                .unwrap_or(0);
            let names: Vec<(&str, &str)> = tabs.iter().map(|t| (t.label, t.tooltip)).collect();
            let tabs = ui_kit::page_tabs("SectionPages", &names, cur, |k| {
                Callable::from_object_method(&gd, "on_page").bind(&[(k as i64).to_variant()])
            });
            strip.custom("", &tabs);
            strip.separator();
        }
        let range = sections::page_range(s.sec, self.page, s.fields.len());
        for (i, fd) in s
            .fields
            .iter()
            .enumerate()
            .filter(|(i, _)| range.contains(i))
        {
            let id = (i as i64).to_variant();
            if let Kind::Action(method) = fd.sort {
                let mut b = strip.button(
                    "",
                    fd.label,
                    &Callable::from_object_method(&gd, "upon_deed").bind(&[id]),
                );
                b.set_name(&ui_kit::node_name(fd.label));
                b.set_tooltip_text(&format!("{}\n{class}.{method}() (undoable)", fd.label));
                self.controls.push((i, b.upcast()));
                continue;
            }
            if let Kind::Progress { getter, cancel } = fd.sort {
                let mut row = HBoxContainer::new_alloc();
                row.set_name(&ui_kit::node_name(fd.label));
                let mut bar = ProgressBar::new_alloc();
                bar.set_min(0.0);
                bar.set_max(1.0);
                bar.set_step(0.0015);
                bar.set_custom_minimum_size(ui_kit::sz(140.0, 18.0));
                bar.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                bar.set_tooltip_text(&format!(
                    "{}\nProgress of the background run ({class}.{getter}()).",
                    fd.label
                ));
                row.add_child(&bar);
                let mut b = Button::new_alloc();
                b.set_name("Cancel");
                b.set_text("Cancel");
                b.set_focus_mode(FocusMode::NONE);
                b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                b.set_tooltip_text(&format!(
                    "Stop the run without changing the terrain ({class}.{cancel}())."
                ));
                b.connect(
                    "pressed",
                    &Callable::from_object_method(&gd, "upon_abort").bind(&[id]),
                );
                row.add_child(&b);
                let cell = strip.custom_kind(CtlKind::Custom, fd.label, &row);
                let mut progress = ProgressRow {
                    field: i,
                    bar,
                    cell: cell.upcast(),
                };
                progress.show(&target, getter);
                self.progress.push(progress);
                continue;
            }
            let value = Self::scan_in(&target, fd);
            let tip = field_tooltip(class, fd);
            let control: Gd<Control> = match fd.sort {
                Kind::Float { min, max, step } => strip
                    .spin_slider(
                        fd.label,
                        min,
                        max,
                        step,
                        num(&value),
                        &Callable::from_object_method(&gd, "on_float").bind(&[id]),
                    )
                    .upcast(),
                Kind::Int { min, max, greater } => {
                    let mut sl = strip.spin_slider(
                        fd.label,
                        min,
                        max,
                        1.0,
                        num(&value),
                        &Callable::from_object_method(&gd, "on_float").bind(&[id]),
                    );
                    sl.set_allow_greater(greater);
                    let w = if max.abs().max(min.abs()) >= 100_000.0 {
                        95.0
                    } else {
                        60.0
                    };
                    sl.set_custom_minimum_size(ui_kit::sz(w, 0.0));
                    if let Some(mut m) = sl.get_parent().and_then(|p| p.try_cast::<Control>().ok())
                    {
                        m.set_custom_minimum_size(ui_kit::sz(w.max(80.0), ui_kit::ROW_HEIGHT));
                    }
                    sl.upcast()
                }
                Kind::Bool => strip
                    .checkbox(
                        fd.label,
                        num(&value) != 0.0,
                        &Callable::from_object_method(&gd, "on_bool").bind(&[id]),
                    )
                    .upcast(),
                Kind::Color => strip
                    .tint(
                        fd.label,
                        value.try_to::<Color>().unwrap_or(Color::WHITE),
                        &Callable::from_object_method(&gd, "on_color").bind(&[id]),
                    )
                    .upcast(),
                Kind::Enum(items) => {
                    let mut ob = w::choice(items, num(&value) as i64);
                    ob.connect(
                        "item_selected",
                        &Callable::from_object_method(&gd, "on_option").bind(&[id]),
                    );
                    strip.custom_kind(
                        CtlKind::Option,
                        fd.label,
                        &ui_kit::midpoint(ui_kit::sz(65.0, ui_kit::ROW_HEIGHT), &ob),
                    );
                    ob.upcast()
                }
                Kind::Action(_) | Kind::Progress { .. } => continue,
            };
            if let Ok(mut sl) = control.clone().try_cast::<EditorSpinSlider>() {
                sl.connect(
                    "grabbed",
                    &Callable::from_object_method(&gd, "on_slider_grabbed").bind(&[(i as i64).to_variant()]),
                );
                sl.connect(
                    "ungrabbed",
                    &Callable::from_object_method(&gd, "on_slider_released").bind(&[(i as i64).to_variant()]),
                );
            }
            let mut control = control;
            control.set_name(&ui_kit::node_name(fd.label));
            control.set_tooltip_text(&tip);
            self.controls.push((i, control));
        }
        self.update_toolbar();
    }
    fn update_toolbar(&mut self) {
        let present: Vec<(Sec, bool)> = SECTIONS
            .iter()
            .map(|s| {
                (s.sec, s.class.is_none_or(|c| self.census.has(c)))
            })
            .collect();
        let has_terrain = self.census.has(TERRAIN);
        for (sec, b) in &mut self.buttons {
            let on = present
                .iter()
                .find(|(s, _)| s == sec)
                .map(|p| p.1)
                .unwrap_or(true);
            b.set_modulate(if on {
                Color::WHITE
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.4005)
            });
            if let Some(s) = SECTIONS.iter().find(|s| s.sec == *sec) {
                let why = match s.class {
                    _ if on => String::new(),
                    Some(c) if sections::is_terrain_child(c) && !has_terrain => {
                        "\n\n(Dimmed: needs a MarchingSquaresTerrain in the scene first.)"
                            .to_string()
                    }
                    Some(c) => {
                        format!("\n\n(Dimmed: no {c} in the scene yet. Open it and press Add.)")
                    }
                    None => String::new(),
                };
                b.set_tooltip_text(&format!("{}{why}", s.tooltip));
            }
        }
    }
    fn renew(&mut self) {
        let Some(s) = self.current() else {
            return;
        };
        let target = self.target(s);
        if target.as_ref().map(|t| t.instance_id()) != self.shown_target {
            self.rebuild_strip();
            return;
        }
        let Some(target) = target else { return };
        let running = s.fields.iter().any(|fd| matches!(fd.sort, Kind::Progress { getter, .. } if Self::job_progress(&target, getter) >= 0.0));
        for row in &mut self.progress {
            if let Kind::Progress { getter, .. } = s.fields[row.field].sort {
                row.show(&target, getter);
            }
        }
        for (i, c) in &mut self.controls {
            let fd = &s.fields[*i];
            match fd.sort {
                Kind::Action(_) => {
                    if let Ok(mut b) = c.clone().try_cast::<Button>() {
                        b.set_disabled(running);
                    }
                    continue;
                }
                Kind::Progress { .. } => continue,
                _ => {}
            }
            let v = Self::scan_in(&target, fd);
            if v.is_nil() {
                continue;
            }
            match fd.sort {
                Kind::Float { .. } | Kind::Int { .. } => {
                    if let Ok(mut sl) = c.clone().try_cast::<EditorSpinSlider>()
                        && (sl.get_value() - num(&v)).abs() > 1e-9
                    {
                        sl.set_value_no_signal(num(&v));
                    }
                }
                Kind::Bool => {
                    if let Ok(mut cb) = c.clone().try_cast::<CheckBox>() {
                        cb.set_pressed_no_signal(num(&v) != 0.0);
                    }
                }
                Kind::Color => {
                    if let (Ok(mut cb), Ok(col)) = (
                        c.clone().try_cast::<ColorPickerButton>(),
                        v.try_to::<Color>(),
                    ) && cb.get_pick_color() != col
                        && !cb.get_popup().is_some_and(|p| p.is_visible())
                    {
                        cb.set_pick_color(col);
                    }
                }
                Kind::Action(_) | Kind::Progress { .. } => {}
                Kind::Enum(_) => {
                    if let Ok(mut ob) = c.clone().try_cast::<OptionButton>() {
                        let idx = ob.get_item_index((num(&v) as i64 + w::OPTION_ID_BIAS) as i32);
                        if idx != ob.get_selected() {
                            ob.select(idx);
                        }
                    }
                }
            }
        }
    }
    fn job_progress(addressee: &Gd<Node>, getter: &str) -> f64 {
        if !addressee.has_method(getter) {
            return -1.0;
        }
        num(&addressee.clone().call(getter, &[]))
    }
    fn undo(&mut self) -> Option<Gd<EditorUndoRedoManager>> {
        self.base_mut().get_undo_redo()
    }
    fn assign_worth(&mut self, field: usize, value: Variant, merge: bool) {
        let Some(s) = self.current() else {
            return;
        };
        let Some(fd) = s.fields.get(field) else {
            return;
        };
        let Some(target) = self.target(s) else { return };
        let Some(new) = Self::raw_with(&target, fd, &value) else {
            return;
        };
        let old = Self::raw_of(&target, fd);
        self.record(&format!("Set {} {}", s.label, fd.label), &target, fd, new, old, merge);
    }
    fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        if !drag.target.is_instance_valid() || !drag.target.is_inside_tree() {
            return;
        }
        let after = Self::raw_of(&drag.target, drag.fd);
        if after == drag.before {
            return;
        }
        self.record(&drag.action, &drag.target, drag.fd, after, drag.before, false);
    }
    fn raw_of(node: &Gd<Node>, fd: &sections::Field) -> Variant {
        match fd.store {
            Store::Prop | Store::Axis(_) => node.get(fd.prop),
            Store::Shader(_) => Self::scan_in(node, fd),
        }
    }
    fn raw_with(node: &Gd<Node>, fd: &sections::Field, value: &Variant) -> Option<Variant> {
        let Store::Axis(a) = fd.store else {
            return Some(value.clone());
        };
        let cur = node.get(fd.prop);
        let x = num(value) as f32;
        if let Ok(mut v3) = cur.try_to::<Vector3>() {
            match a {
                0 => v3.x = x,
                1 => v3.y = x,
                _ => v3.z = x,
            }
            Some(v3.to_variant())
        } else if let Ok(mut v2) = cur.try_to::<Vector2>() {
            if a == 0 {
                v2.x = x
            } else {
                v2.y = x
            }
            Some(v2.to_variant())
        } else {
            None
        }
    }
    fn write_raw(node: &Gd<Node>, fd: &sections::Field, raw: &Variant) {
        match fd.store {
            Store::Prop | Store::Axis(_) => node.clone().set(fd.prop, raw),
            Store::Shader(m) => {
                if let Some(mut mat) = Self::shader_material(node, m) {
                    mat.set_shader_parameter(fd.prop, raw);
                }
            }
        }
    }
    fn record(
        &mut self,
        action: &str,
        target: &Gd<Node>,
        fd: &sections::Field,
        new: Variant,
        old: Variant,
        merge: bool,
    ) {
        let Some(mut ur) = self.undo() else { return };
        let mode = if merge {
            MergeMode::ENDS
        } else {
            MergeMode::DISABLE
        };
        match fd.store {
            Store::Prop | Store::Axis(_) => {
                ur.create_action_ex(action)
                    .merge_mode(mode)
                    .custom_context(target)
                    .done();
                ur.add_do_property(target, fd.prop, &new);
                ur.add_undo_property(target, fd.prop, &old);
            }
            Store::Shader(m) => {
                let Some(mat) = Self::shader_material(target, m) else {
                    return;
                };
                ur.create_action_ex(action)
                    .merge_mode(mode)
                    .custom_context(target)
                    .done();
                let name = fd.prop.to_variant();
                ur.add_do_method(&mat, "set_shader_parameter", &[name.clone(), new]);
                ur.add_undo_method(&mat, "set_shader_parameter", &[name, old]);
            }
        }
        ur.commit_action();
    }
    fn build_world_panel(&mut self) -> Option<Gd<PixelWorldPanel>> {
        let world = super::super::editor_scenes::instantiate::<PixelWorldPanel>(
            super::super::editor_scenes::PIXEL_WORLD_PANEL,
        )?;
        let parts = {
            let w = world.bind();
            (
                w.margin.clone(),
                w.column.clone(),
                w.world_title.clone(),
                w.effect_rows.clone(),
                w.presets_title.clone(),
                w.preset_buttons.clone(),
                w.wire_button.clone(),
                w.preview_hint.clone(),
                w.refresh_timer.clone(),
            )
        };
        let (
            Some(mut margin),
            Some(mut column),
            Some(mut world_title),
            Some(effect_rows),
            Some(mut presets_title),
            Some(mut preset_buttons),
            Some(mut wire),
            Some(mut hint),
            Some(timer),
        ) = parts
        else {
            godot_error!("Pixelizer: the pixel world panel scene does not assign all of its parts");
            world.free();
            return None;
        };
        let gd = self.to_gd();
        let mut scroll = world.clone().upcast::<ScrollContainer>();
        ui_kit::style_side_panel(&mut scroll, &mut margin, &mut column);
        ui_kit::style_panel_title(&mut world_title);
        ui_kit::style_panel_title(&mut presets_title);
        presets_title.set_text(sections::PRESET_HEADER);
        for (i, p) in PRESETS.iter().enumerate() {
            let tip = format!(
                "Apply the {} preset to the scene's Day/Night, Sky, Weather and renderer god rays (undoable).",
                p.moniker
            );
            let mut b = w::push_button(p.moniker, None, &tip);
            b.set_focus_mode(FocusMode::NONE);
            b.set_h_size_flags(SizeFlags::EXPAND_FILL);
            b.connect(
                "pressed",
                &Callable::from_object_method(&gd, "on_preset").bind(&[(i as i64).to_variant()]),
            );
            preset_buttons.add_child(&b);
        }
        wire.set_text(WIRE_LABEL);
        wire.set_tooltip_text(WIRE_TOOLTIP);
        w::style_push_button(&mut wire);
        wire.connect("pressed", &Callable::from_object_method(&gd, "on_wire"));
        hint.set_custom_minimum_size(ui_kit::sz(60.0, 0.0));
        hint.set_modulate(ui_kit::DIM_MODULATE);
        let mut timer = timer;
        timer.connect("timeout", &gd.callable("on_refresh_timer"));
        self.effect_rows = Some(effect_rows);
        self.preview_hint = Some((hint, false));
        self.refresh_timer = Some(timer);
        Some(world)
    }
    fn rebuild_world(&mut self) {
        let Some(mut col) = self.effect_rows.clone().filter(|c| c.is_instance_valid()) else {
            return;
        };
        for mut c in col.get_children().iter_shared() {
            col.remove_child(&c);
            c.queue_free();
        }
        let gd = self.to_gd();
        let has_terrain = self.census.has(TERRAIN);
        for &(class, label, _) in ADDABLE.iter() {
            let count = self.census.count(class);
            let needs_terrain = sections::is_terrain_child(class);
            let blocked = needs_terrain && !has_terrain;
            let mut row = HBoxContainer::new_alloc();
            row.set_name(&ui_kit::node_name(label));
            let present = count > 0;
            let text = if count > 1 {
                format!("{label} ({count})")
            } else {
                label.to_string()
            };
            let where_to = if needs_terrain {
                " under the selected MarchingSquaresTerrain (or the scene's only one)"
            } else {
                " to the edited scene"
            };
            let tip = if present {
                format!(
                    "{label}: {count} in the scene. Click to select the {class} (again to cycle through them)."
                )
            } else if blocked {
                format!(
                    "{label} is not in the scene. It works on a MarchingSquaresTerrain: add a terrain first."
                )
            } else {
                format!(
                    "{label} is not in the scene. \"+ Add\" adds a {class}{where_to} (references wired, undoable)."
                )
            };
            let mut sel = w::push_button(&text, Some(icon_for(class)), &tip);
            sel.set_name("Name");
            sel.set_flat(true);
            sel.set_focus_mode(FocusMode::NONE);
            sel.set_h_size_flags(SizeFlags::EXPAND_FILL);
            sel.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            if present {
                sel.connect(
                    "pressed",
                    &Callable::from_object_method(&gd, "on_select_existing")
                        .bind(&[GString::from(class).to_variant()]),
                );
            } else {
                sel.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.5));
                sel.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
                sel.set_disabled(true);
            }
            row.add_child(&sel);
            let (act_text, method, act_tip) = if present {
                ("Select", "on_select_existing", tip.clone())
            } else {
                ("+ Add", "on_add", tip.clone())
            };
            let mut act = w::push_button(act_text, None, &act_tip);
            act.set_name(if present { "Select" } else { "Add" });
            act.set_focus_mode(FocusMode::NONE);
            act.set_custom_minimum_size(ui_kit::sz(64.0, 0.0));
            act.set_disabled(!present && blocked);
            act.connect(
                "pressed",
                &Callable::from_object_method(&gd, method)
                    .bind(&[GString::from(class).to_variant()]),
            );
            row.add_child(&act);
            col.add_child(&row);
        }
        self.update_preview_hint(true);
        self.update_toolbar();
    }
    fn update_preview_hint(&mut self, force: bool) {
        let on = super::super::preview_plugin::prelook_upon();
        let Some((hint, shown)) = &mut self.preview_hint else {
            return;
        };
        if !hint.is_instance_valid() || (!force && *shown == on) {
            return;
        }
        *shown = on;
        let label = sections::PREVIEW_LABEL;
        let text = if on {
            format!(
                "\"{label}\" is on: the 3D viewport shows the scene through the pixel renderer (editor grid hidden; gizmos draw on top)."
            )
        } else {
            format!(
                "Turn on \"{label}\" in the 3D viewport toolbar to see the scene through the pixel renderer."
            )
        };
        hint.set_text(&text);
        hint.set_tooltip_text(&text);
    }
}
impl ProgressRow {
    fn show(&mut self, addressee: &Gd<Node>, getter: &str) {
        let p = PixelizerEditorPlugin::job_progress(addressee, getter);
        if self.bar.is_instance_valid() {
            self.bar.set_value_no_signal(p.max(0.0));
        }
        if self.cell.is_instance_valid() && self.cell.is_visible() != (p >= 0.0) {
            self.cell.set_visible(p >= 0.0);
        }
    }
}
thread_local! {
    static RECOUNT_QUEUED: Cell<bool> = const { Cell::new(false) };
}
fn scene_node_came_or_went(plugin: Gd<PixelizerEditorPlugin>, node: Gd<Node>) {
    if RECOUNT_QUEUED.get() {
        return;
    }
    let Some(root) = EditorInterface::singleton().get_edited_scene_root() else {
        return;
    };
    if root != node && !root.is_ancestor_of(&node) {
        return;
    }
    RECOUNT_QUEUED.set(true);
    Callable::from_fn("recount_pixel_nodes", move |_| {
        RECOUNT_QUEUED.set(false);
        if plugin.is_instance_valid() {
            plugin.clone().bind_mut().recount();
        }
    })
    .call_deferred(&[]);
}
