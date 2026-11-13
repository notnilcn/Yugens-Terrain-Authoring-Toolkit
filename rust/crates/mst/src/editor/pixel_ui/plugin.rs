//! `PixelizerEditorPlugin` — auto-registered editor plugin for the pixelizer nodes.
//!
//! Selecting a `PixelRenderer`, `PixelObject`, `PixelOrbitCamera`, a `PixelPalette` or one of
//! the effect nodes shows the MarchingSquaresTerrain editor layout:
//! * `SPATIAL_EDITOR_SIDE_LEFT`: icon toolbar, one button per section (effect category);
//! * `SPATIAL_EDITOR_BOTTOM`: the active section's key properties as labelled controls;
//! * `SPATIAL_EDITOR_SIDE_RIGHT`: the "Pixel World" panel — every pixel node in the scene with
//!   select/add buttons, lighting presets and "Wire refs".
//!
//! Sections edit whichever node of their class is selected, else the first one in the edited
//! scene; a missing effect shows an "Add" button. Every change goes through
//! `EditorUndoRedoManager` (slider drags merge into one action).

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::editor_plugin::CustomControlContainer;
use godot::classes::object::ConnectFlags;
use godot::classes::undo_redo::MergeMode;
use godot::classes::{
    Button, CheckBox, ColorPickerButton, Control, EditorInterface, EditorPlugin, EditorSpinSlider, EditorUndoRedoManager, GridContainer,
    HBoxContainer, HSeparator, IEditorPlugin, Label, Node, Object, OptionButton, ProgressBar, RenderingServer, ScrollContainer, ShaderMaterial,
    VBoxContainer, VFlowContainer,
};
use godot::prelude::*;

use super::actions;
use super::presets::PRESETS;
use super::sections::{self, ADDABLE, HANDLED, Kind, SECTIONS, Sec, Section, Store, icon_for};
use super::widgets as w;
use crate::editor::ui_kit::{self, CtlKind, Strip};

#[derive(GodotClass)]
#[class(tool, init, base = EditorPlugin)]
pub struct PixelizerEditorPlugin {
    selected: Option<Gd<Node>>,
    sec: Option<Sec>,
    /// Strip page of the current section (`sections::pages`).
    page: usize,
    toolbar: Option<Gd<VFlowContainer>>,
    strip: Option<Gd<ScrollContainer>>,
    world: Option<Gd<ScrollContainer>>,
    world_box: Option<Gd<VBoxContainer>>,
    /// Pixel World hint that follows the Pixel Preview toggle (and the state it was set for).
    preview_hint: Option<(Gd<Label>, bool)>,
    /// Pixel-node counts the world panel was built for (rebuilt when they change).
    world_sig: Vec<usize>,
    buttons: Vec<(Sec, Gd<Button>)>,
    /// (field index in the current section, control) for value refresh.
    controls: Vec<(usize, Gd<Control>)>,
    /// Node the strip currently edits (instance id), to rebuild when it appears/disappears.
    shown_target: Option<InstanceId>,
    visible: bool,
    refresh_timer: f64,
    base: Base<EditorPlugin>,
}

const BAKE_LUT_MENU: &str = "Bake PixelPalette LUT to PNG";use sections::{WIRE_LABEL, WIRE_TOOLTIP};

fn node_is(n: &Gd<Node>, class: &str) -> bool {
    n.is_class(class)
}

/// Tooltip of a strip control: name, where the value lives and its range.
fn field_tooltip(class: &str, fd: &sections::Field) -> String {
    let place = match fd.store {
        Store::Prop => format!("{class}.{}", fd.prop),
        Store::Axis(a) => format!("{class}.{}.{}", fd.prop, ["x", "y", "z"][a as usize % 3]),
        Store::Shader(m) => format!("{class}.{m} shader parameter \"{}\"", fd.prop),
    };
    let range = match fd.kind {
        Kind::Float { min, max, .. } => format!("\nRange {min} to {max} (drag, or click to type)."),
        Kind::Int { min, max } => format!("\nRange {min} to {max} (drag, or click to type)."),
        _ => String::new(),
    };
    let hint = sections::field_hint(fd.prop).map(|h| format!("\n{h}")).unwrap_or_default();
    format!("{}{hint}\n{place}{range}", fd.label)
}

fn num(v: &Variant) -> f64 {
    v.try_to::<f64>().or_else(|_| v.try_to::<i64>().map(|i| i as f64)).or_else(|_| v.try_to::<bool>().map(|b| b as i32 as f64)).unwrap_or(0.0)
}


#[godot_api]
impl IEditorPlugin for PixelizerEditorPlugin {
    fn enter_tree(&mut self) {
        let toolbar = self.build_toolbar();
        let strip = self.build_strip();
        let world = self.build_world_panel();
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, &toolbar);
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &strip);
        self.base_mut().add_control_to_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT, &world);
        toolbar.clone().set_visible(false);
        strip.clone().set_visible(false);
        world.clone().set_visible(false);
        self.toolbar = Some(toolbar);
        self.strip = Some(strip);
        self.world = Some(world);
        let bake = Callable::from_object_method(&self.to_gd(), "on_bake_lut");
        self.base_mut().add_tool_menu_item(BAKE_LUT_MENU, &bake);
        let cb = Callable::from_object_method(&self.to_gd(), "on_history_changed");
        if let Some(mut ur) = self.base_mut().get_undo_redo() {
            ur.connect_flags("version_changed", &cb, ConnectFlags::DEFERRED);
        }
        self.base_mut().set_process(true);
    }

    fn exit_tree(&mut self) {
        self.base_mut().remove_tool_menu_item(BAKE_LUT_MENU);
        if let Some(t) = self.toolbar.take() {
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_LEFT, &t);
            t.free();
        }
        if let Some(s) = self.strip.take() {
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_BOTTOM, &s);
            s.free();
        }
        if let Some(wp) = self.world.take() {
            self.base_mut().remove_control_from_container(CustomControlContainer::SPATIAL_EDITOR_SIDE_RIGHT, &wp);
            wp.free();
        }
        self.world_box = None;
        self.buttons.clear();
        self.controls.clear();
    }

    fn get_plugin_name(&self) -> GString {
        "Pixelizer".into()
    }

    fn handles(&self, object: Gd<Object>) -> bool {
        HANDLED.iter().any(|c| object.is_class(*c))
    }

    fn edit(&mut self, object: Option<Gd<Object>>) {
        self.selected = object.and_then(|o| o.try_cast::<Node>().ok());
        if let Some(sel) = &self.selected {
            let class = HANDLED.iter().find(|c| sel.is_class(**c)).copied().unwrap_or(sections::RENDERER);
            // Keep the current section if it edits the selected class (e.g. renderer sub-sections).
            let keep = self.sec.map(|s| sections::section(s).class == Some(class)).unwrap_or(false);
            if !keep {
                self.sec = Some(sections::default_section(class));
            }
        }
        if self.toolbar.is_some() {
            self.select_section(self.sec.unwrap_or(Sec::Renderer));
        }
    }

    fn make_visible(&mut self, visible: bool) {
        self.visible = visible;
        if let Some(t) = &mut self.toolbar {
            t.set_visible(visible);
        }
        if let Some(s) = &mut self.strip {
            s.set_visible(visible);
        }
        if let Some(wp) = &mut self.world {
            wp.set_visible(visible);
        }
        if visible {
            self.rebuild_strip();
            self.rebuild_world();
        }
    }

    fn process(&mut self, delta: f64) {
        if !self.visible {
            return;
        }
        self.refresh_timer += delta;
        if self.refresh_timer > 0.3 {
            self.refresh_timer = 0.0;
            self.refresh();
            self.update_preview_hint(false);
            if self.world_signature() != self.world_sig {
                self.rebuild_world();
            }
        }
    }
}

#[godot_api]
impl PixelizerEditorPlugin {
    #[func]
    fn on_section(&mut self, idx: i64) {
        if let Some(s) = SECTIONS.get(idx as usize) {
            self.select_section(s.sec);
        }
    }

    /// Strip tab pressed: a section / page of the current node (`sections::class_tabs`).
    /// Deferred: the tab itself is freed by the rebuild.
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
            self.refresh();
        }
    }

    #[func]
    fn on_float(&mut self, value: f64, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let v = match fd.kind {
            Kind::Int { .. } => (value.round() as i64).to_variant(),
            _ => value.to_variant(),
        };
        self.set_value(field as usize, v, true);
    }

    #[func]
    fn on_bool(&mut self, on: bool, field: i64) {
        self.set_value(field as usize, on.to_variant(), false);
    }

    #[func]
    fn on_color(&mut self, c: Color, field: i64) {
        self.set_value(field as usize, c.to_variant(), true);
    }

    /// Action button of the strip (Erode, Scatter, Clear): calls the method on the edited node,
    /// which records its own undo step.
    #[func]
    fn on_action(&mut self, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let Kind::Action(method) = fd.kind else { return };
        let Some(mut target) = self.target(self.current()) else { return };
        let r = target.call(method, &[]);
        if !r.is_nil() {
            godot_print!("{}: {method}() -> {r}", target.get_name());
        }
        // Show a background job's progress row right away.
        self.refresh();
    }

    /// Cancel button of a Progress row: calls its `cancel` method on the edited node.
    #[func]
    fn on_cancel(&mut self, field: i64) {
        let Some(fd) = self.field(field) else { return };
        let Kind::Progress { cancel, .. } = fd.kind else { return };
        if let Some(mut target) = self.target(self.current()) {
            target.call(cancel, &[]);
        }
        self.refresh();
    }

    #[func]
    fn on_option(&mut self, index: i64, field: i64) {
        let Some(fd) = self.field(field) else { return };
        if let Kind::Enum(items) = fd.kind
            && let Some((_, id)) = items.get(index as usize)
        {
            self.set_value(field as usize, id.to_variant(), false);
        }
    }

    /// Project > Tools > "Bake PixelPalette LUT to PNG": bakes the PixelPalette being edited in
    /// the inspector, else the selected / first renderer's palette, to its `output_path`.
    #[func]
    fn on_bake_lut(&mut self) {
        let edited = EditorInterface::singleton().get_inspector().and_then(|i| i.get_edited_object());
        let mut palette = edited.and_then(|o| o.try_cast::<crate::render::palette::PixelPalette>().ok());
        if palette.is_none() {
            let renderer = self.selected.clone().filter(|s| s.is_class(sections::RENDERER)).or_else(|| self.find_in_scene(sections::RENDERER));
            palette = renderer.and_then(|r| r.get("palette").try_to::<Gd<crate::render::palette::PixelPalette>>().ok());
        }
        match palette {
            Some(mut p) => {
                let path = p.get("output_path").try_to::<GString>().unwrap_or_default();
                p.bind_mut().save_lut_png(path);
            }
            None => godot_warn!("Pixelizer: select a PixelPalette (or a PixelRenderer with a palette), then use Tools > {BAKE_LUT_MENU}."),
        }
    }

    #[func]
    fn on_add(&mut self, class: GString) {
        if let Some(ur) = self.undo() {
            actions::add_effect(ur, &class.to_string());
        }
    }

    /// Selects the `nth` node of `class` (cycling), or adds one when there is none.
    #[func]
    fn on_select_existing(&mut self, class: GString) {
        let class = class.to_string();
        let all = actions::find_all(&class);
        if all.is_empty() {
            self.on_add(GString::from(class.as_str()));
            return;
        }
        let cur = self.selected.as_ref().and_then(|s| all.iter().position(|n| n == s));
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
    // ---- construction --------------------------------------------------------------------

    /// Same left toolbar as the terrain tools (`ui_kit::Toolbar`).
    fn build_toolbar(&mut self) -> Gd<VFlowContainer> {
        let gd = self.to_gd();
        let mut tb = ui_kit::Toolbar::new("PixelizerToolbar");
        for (i, s) in SECTIONS.iter().enumerate() {
            // One button per node: further sections of the same node are strip tabs.
            if sections::head(s.sec) != s.sec {
                continue;
            }
            if s.group_start {
                tb.separator();
            }
            let b = tb.button(s.label, s.tooltip, w::icon(s.icon), &Callable::from_object_method(&gd, "on_section").bind(&[(i as i64).to_variant()]));
            self.buttons.push((s.sec, b));
        }
        tb.separator();
        tb.root
    }

    /// Same wrapping attribute strip as the terrain tools (`ui_kit::Strip`).
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

    fn current(&self) -> &'static Section {
        sections::section(self.sec.unwrap_or(Sec::Renderer))
    }

    fn field(&self, idx: i64) -> Option<&'static sections::Field> {
        self.current().fields.get(idx as usize)
    }

    // ---- scene queries -------------------------------------------------------------------

    fn find_in_scene(&self, class: &str) -> Option<Gd<Node>> {
        actions::find_in_scene(class)
    }

    /// Node the section edits: the selection if it matches, else the first of its class.
    fn target(&self, s: &Section) -> Option<Gd<Node>> {
        let class = s.class?;
        if let Some(sel) = &self.selected
            && sel.is_instance_valid()
            && node_is(sel, class)
            && sel.is_inside_tree()
        {
            return Some(sel.clone());
        }
        self.find_in_scene(class)
    }

    fn shader_material(node: &Gd<Node>, prop: &str) -> Option<Gd<ShaderMaterial>> {
        node.get(prop).try_to::<Gd<ShaderMaterial>>().ok()
    }

    fn read(node: &Gd<Node>, fd: &sections::Field) -> Variant {
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
                let Some(mat) = Self::shader_material(node, m) else { return Variant::nil() };
                let v = mat.get_shader_parameter(fd.prop);
                if !v.is_nil() {
                    return v;
                }
                match mat.get_shader() {
                    Some(sh) => RenderingServer::singleton().shader_get_parameter_default(sh.get_rid(), fd.prop),
                    None => Variant::nil(),
                }
            }
        }
    }

    // ---- strip ---------------------------------------------------------------------------

    /// Clears the strip (queue_free: a strip control may be emitting the signal that got here).
    fn clear_strip(&mut self) -> Option<Strip> {
        let mut root = self.strip.clone()?;
        for mut c in root.get_children().iter_shared() {
            root.remove_child(&c);
            c.queue_free();
        }
        self.controls.clear();
        Some(Strip::rebuild(root))
    }

    fn rebuild_strip(&mut self) {
        let Some(mut strip) = self.clear_strip() else { return };
        let gd = self.to_gd();
        let s = self.current();
        // The node's title (its toolbar entry); its sections / pages are the tabs below.
        let h = sections::section(sections::head(s.sec));
        let mut title = strip.title(h.label, w::icon(h.icon), h.tooltip);
        title.set_name("SectionTitle");

        let target = self.target(s);
        self.shown_target = target.as_ref().map(|t| t.instance_id());
        let class = s.class.unwrap_or(sections::RENDERER);
        let Some(target) = target else {
            let mut b = strip.button("", &format!("Add {}", s.label), &Callable::from_object_method(&gd, "on_add").bind(&[GString::from(class).to_variant()]));
            if let Some(ic) = w::icon(s.icon) {
                b.set_button_icon(&ic);
            }
            b.set_tooltip_text(&format!("Add a {class} to the edited scene (references wired, undoable)."));
            if sections::is_terrain_child(class) {
                if self.find_in_scene("MarchingSquaresTerrain").is_some() {
                    strip.hint(&format!("No {class} in this scene: Add puts one under the selected (or first) terrain."));
                } else {
                    b.set_disabled(true);
                    strip.hint(&format!("{} needs a MarchingSquaresTerrain: add a terrain to the scene first.", s.label));
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
            let cur = tabs.iter().position(|t| t.sec == s.sec && t.page == page).unwrap_or(0);
            let names: Vec<(&str, &str)> = tabs.iter().map(|t| (t.label, t.tooltip)).collect();
            let tabs = ui_kit::page_tabs("SectionPages", &names, cur, |k| Callable::from_object_method(&gd, "on_page").bind(&[(k as i64).to_variant()]));
            strip.custom("", &tabs);
            strip.separator();
        }
        let range = sections::page_range(s.sec, self.page, s.fields.len());
        for (i, fd) in s.fields.iter().enumerate().filter(|(i, _)| range.contains(i)) {
            let id = (i as i64).to_variant();
            if let Kind::Action(method) = fd.kind {
                let mut b = strip.button("", fd.label, &Callable::from_object_method(&gd, "on_action").bind(&[id]));
                b.set_name(fd.label);
                b.set_tooltip_text(&format!("{}\n{class}.{method}() (undoable)", fd.label));
                self.controls.push((i, b.upcast()));
                continue;
            }
            if let Kind::Progress { getter, cancel } = fd.kind {
                let mut row = HBoxContainer::new_alloc();
                row.set_name(fd.label);
                let mut bar = ProgressBar::new_alloc();
                bar.set_name("Bar");
                bar.set_min(0.0);
                bar.set_max(1.0);
                bar.set_step(0.001);
                bar.set_custom_minimum_size(ui_kit::sz(140.0, 18.0));
                bar.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                bar.set_tooltip_text(&format!("{}\nProgress of the background run ({class}.{getter}()).", fd.label));
                row.add_child(&bar);
                let mut b = Button::new_alloc();
                b.set_name("Cancel");
                b.set_text("Cancel");
                b.set_focus_mode(FocusMode::NONE);
                b.set_v_size_flags(SizeFlags::SHRINK_CENTER);
                b.set_tooltip_text(&format!("Stop the run without changing the terrain ({class}.{cancel}())."));
                b.connect("pressed", &Callable::from_object_method(&gd, "on_cancel").bind(&[id]));
                row.add_child(&b);
                strip.custom_kind(CtlKind::Custom, fd.label, &row);
                let mut control: Gd<Control> = row.upcast();
                Self::show_progress(&mut control, &target, getter);
                self.controls.push((i, control));
                continue;
            }
            let value = Self::read(&target, fd);
            let tip = field_tooltip(class, fd);
            let control: Gd<Control> = match fd.kind {
                Kind::Float { min, max, step } => {
                    strip.spin_slider(fd.label, min, max, step, num(&value), &Callable::from_object_method(&gd, "on_float").bind(&[id])).upcast()
                }
                Kind::Int { min, max } => {
                    let mut sl = strip.spin_slider(fd.label, min, max, 1.0, num(&value), &Callable::from_object_method(&gd, "on_float").bind(&[id]));
                    // Wide ranges (droplet counts, seeds) need room for 7+ digits.
                    let w = if max.abs().max(min.abs()) >= 100_000.0 { 95.0 } else { 60.0 };
                    sl.set_custom_minimum_size(ui_kit::sz(w, 0.0));
                    if let Some(mut m) = sl.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                        m.set_custom_minimum_size(ui_kit::sz(w.max(80.0), ui_kit::ROW_HEIGHT));
                    }
                    sl.upcast()
                }
                Kind::Bool => strip.checkbox(fd.label, num(&value) != 0.0, &Callable::from_object_method(&gd, "on_bool").bind(&[id])).upcast(),
                Kind::Color => strip
                    .color(fd.label, value.try_to::<Color>().unwrap_or(Color::WHITE), &Callable::from_object_method(&gd, "on_color").bind(&[id]))
                    .upcast(),
                Kind::Enum(items) => {
                    let mut ob = w::option(items, num(&value) as i64);
                    ob.connect("item_selected", &Callable::from_object_method(&gd, "on_option").bind(&[id]));
                    strip.custom_kind(CtlKind::Option, fd.label, &ui_kit::center(ui_kit::sz(65.0, ui_kit::ROW_HEIGHT), &ob));
                    ob.upcast()
                }
                Kind::Action(_) | Kind::Progress { .. } => unreachable!(),
            };
            let mut control = control;
            control.set_name(fd.label);
            control.set_tooltip_text(&tip);
            self.controls.push((i, control));
        }
        self.update_toolbar();
    }

    /// Dims toolbar buttons whose node is missing from the scene.
    fn update_toolbar(&mut self) {
        let present: Vec<(Sec, bool)> = SECTIONS.iter().map(|s| (s.sec, s.class.is_none_or(|c| self.find_in_scene(c).is_some()))).collect();
        let has_terrain = self.find_in_scene("MarchingSquaresTerrain").is_some();
        for (sec, b) in &mut self.buttons {
            let on = present.iter().find(|(s, _)| s == sec).map(|p| p.1).unwrap_or(true);
            b.set_modulate(if on { Color::WHITE } else { Color::from_rgba(1.0, 1.0, 1.0, 0.4) });
            // Say why a button is dimmed (the node is missing; terrain features need a terrain).
            if let Some(s) = SECTIONS.iter().find(|s| s.sec == *sec) {
                let why = match s.class {
                    _ if on => String::new(),
                    Some(c) if sections::is_terrain_child(c) && !has_terrain => {
                        "\n\n(Dimmed: needs a MarchingSquaresTerrain in the scene first.)".to_string()
                    }
                    Some(c) => format!("\n\n(Dimmed: no {c} in the scene yet. Open it and press Add.)"),
                    None => String::new(),
                };
                b.set_tooltip_text(&format!("{}{why}", s.tooltip));
            }
        }
    }

    /// Re-reads values into the controls; rebuilds when the edited node changed.
    fn refresh(&mut self) {
        let s = self.current();
        let target = self.target(s);
        if target.as_ref().map(|t| t.instance_id()) != self.shown_target {
            self.rebuild_strip();
            return;
        }
        let Some(target) = target else { return };
        // A running background job (Progress field) disables the section's action buttons.
        let running = s.fields.iter().any(|fd| matches!(fd.kind, Kind::Progress { getter, .. } if Self::job_progress(&target, getter) >= 0.0));
        for (i, c) in &mut self.controls {
            let fd = &s.fields[*i];
            match fd.kind {
                Kind::Action(_) => {
                    if let Ok(mut b) = c.clone().try_cast::<Button>() {
                        b.set_disabled(running);
                    }
                    continue;
                }
                Kind::Progress { getter, .. } => {
                    Self::show_progress(c, &target, getter);
                    continue;
                }
                _ => {}
            }
            let v = Self::read(&target, fd);
            if v.is_nil() {
                continue;
            }
            match fd.kind {
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
                    if let (Ok(mut cb), Ok(col)) = (c.clone().try_cast::<ColorPickerButton>(), v.try_to::<Color>())
                        && cb.get_pick_color() != col
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

    /// `getter()` of a background job (0..1, < 0 = idle).
    fn job_progress(target: &Gd<Node>, getter: &str) -> f64 {
        if !target.has_method(getter) {
            return -1.0;
        }
        num(&target.clone().call(getter, &[]))
    }

    /// Updates a Progress row (bar value) and shows its strip cell only while the job runs.
    fn show_progress(row: &mut Gd<Control>, target: &Gd<Node>, getter: &str) {
        let p = Self::job_progress(target, getter);
        if let Some(mut bar) = row.try_get_node_as::<ProgressBar>("Bar") {
            bar.set_value_no_signal(p.max(0.0));
        }
        // The strip wraps the row in a labelled cell: hide the whole cell.
        let mut cell = row.get_parent().and_then(|p| p.try_cast::<Control>().ok()).unwrap_or_else(|| row.clone());
        if cell.is_visible() != (p >= 0.0) {
            cell.set_visible(p >= 0.0);
        }
    }

    // ---- edits ---------------------------------------------------------------------------

    fn undo(&mut self) -> Option<Gd<EditorUndoRedoManager>> {
        self.base_mut().get_undo_redo()
    }

    fn set_value(&mut self, field: usize, value: Variant, merge: bool) {
        let s = self.current();
        let Some(fd) = s.fields.get(field) else { return };
        let Some(target) = self.target(s) else { return };
        let old = Self::read(&target, fd);
        let Some(mut ur) = self.undo() else { return };
        let mode = if merge { MergeMode::ENDS } else { MergeMode::DISABLE };
        ur.create_action_ex(&format!("Set {} {}", s.label, fd.label)).merge_mode(mode).custom_context(&target).done();
        match fd.store {
            Store::Prop => {
                ur.add_do_property(&target, fd.prop, &value);
                ur.add_undo_property(&target, fd.prop, &old);
            }
            Store::Axis(a) => {
                let cur = target.get(fd.prop);
                let x = num(&value) as f32;
                let (new, prev) = if let Ok(mut v3) = cur.try_to::<Vector3>() {
                    let prev = v3;
                    match a {
                        0 => v3.x = x,
                        1 => v3.y = x,
                        _ => v3.z = x,
                    }
                    (v3.to_variant(), prev.to_variant())
                } else if let Ok(mut v2) = cur.try_to::<Vector2>() {
                    let prev = v2;
                    if a == 0 { v2.x = x } else { v2.y = x }
                    (v2.to_variant(), prev.to_variant())
                } else {
                    return;
                };
                ur.add_do_property(&target, fd.prop, &new);
                ur.add_undo_property(&target, fd.prop, &prev);
            }
            Store::Shader(m) => {
                let Some(mat) = Self::shader_material(&target, m) else { return };
                let name = fd.prop.to_variant();
                ur.add_do_method(&mat, "set_shader_parameter", &[name.clone(), value]);
                ur.add_undo_method(&mat, "set_shader_parameter", &[name, old]);
            }
        }
        ur.commit_action();
    }

    // ---- Pixel World panel (right) -------------------------------------------------------

    fn build_world_panel(&mut self) -> Gd<ScrollContainer> {
        // Same side panel as the terrain texture panel.
        let (scroll, col) = ui_kit::side_panel("PixelWorldPanel");
        self.world_box = Some(col);
        scroll
    }

    /// Node counts per `ADDABLE` class, used to notice scene changes.
    fn world_signature(&self) -> Vec<usize> {
        // Counts of every addable class, then the terrain count (Erosion / Wild Flowers rows
        // depend on a terrain being there).
        ADDABLE.iter().map(|(c, _, _)| actions::find_all(c).len()).chain(std::iter::once(actions::find_all("MarchingSquaresTerrain").len())).collect()
    }

    fn rebuild_world(&mut self) {
        let Some(mut col) = self.world_box.clone() else { return };
        for mut c in col.get_children().iter_shared() {
            col.remove_child(&c);
            c.queue_free();
        }
        self.world_sig = self.world_signature();
        let gd = self.to_gd();

        col.add_child(&ui_kit::panel_title(
            "Pixel World",
            w::icon("world"),
            "Every pixel node of the edited scene: Select (or click the name) selects it, again to cycle; \"+ Add\" adds a missing one. More instances: the Pixel World menu in the 3D toolbar.",
        ));
        col.add_child(&HSeparator::new_alloc());
        let has_terrain = self.world_sig.last().copied().unwrap_or(0) > 0;
        for (k, &(class, label, unique)) in ADDABLE.iter().enumerate() {
            let count = self.world_sig[k];
            // Erosion, Wild Flowers and Grass Fire are terrain features: added as a child of the
            // selected (or first) MarchingSquaresTerrain and do nothing without one.
            let needs_terrain = sections::is_terrain_child(class);
            let blocked = needs_terrain && !has_terrain;
            // Every row: [icon name] [action]. In the scene: the name is bright and the action is
            // "Select" (again to cycle). Not in the scene: the name is dimmed and the action is
            // "+ Add" (disabled while a terrain feature has no terrain to go under).
            let mut row = HBoxContainer::new_alloc();
            row.set_name(label);
            let present = count > 0;
            let text = if count > 1 { format!("{label} ({count})") } else { label.to_string() };
            let where_to = if needs_terrain { " as a child of the selected (or first) MarchingSquaresTerrain" } else { " to the edited scene" };
            let tip = if present {
                format!("{label}: {count} in the scene. Click to select the {class} (again to cycle through them).")
            } else if blocked {
                format!("{label} is not in the scene. It works on a MarchingSquaresTerrain: add a terrain first.")
            } else {
                format!("{label} is not in the scene. \"+ Add\" adds a {class}{where_to} (references wired, undoable).")
            };
            let mut sel = w::push_button(&text, Some(icon_for(class)), &tip);
            sel.set_name("Name");
            sel.set_flat(true);
            sel.set_focus_mode(FocusMode::NONE);
            sel.set_h_size_flags(SizeFlags::EXPAND_FILL);
            sel.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            if present {
                sel.connect("pressed", &Callable::from_object_method(&gd, "on_select_existing").bind(&[GString::from(class).to_variant()]));
            } else {
                // Dimmed, not a button: only "+ Add" acts on a missing node.
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
            act.connect("pressed", &Callable::from_object_method(&gd, method).bind(&[GString::from(class).to_variant()]));
            row.add_child(&act);
            let _ = unique;
            col.add_child(&row);
        }
        col.add_child(&HSeparator::new_alloc());
        col.add_child(&ui_kit::panel_title(
            sections::PRESET_HEADER,
            w::icon("presets"),
            "Time of day / weather presets for Day/Night, Sky, Weather and the renderer's god rays.",
        ));
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(3);
        for (i, p) in PRESETS.iter().enumerate() {
            let tip = format!("Apply the {} preset to the scene's Day/Night, Sky, Weather and renderer god rays (undoable).", p.name);
            let mut b = w::push_button(p.name, None, &tip);
            b.set_focus_mode(FocusMode::NONE);
            b.set_h_size_flags(SizeFlags::EXPAND_FILL);
            b.connect("pressed", &Callable::from_object_method(&gd, "on_preset").bind(&[(i as i64).to_variant()]));
            grid.add_child(&b);
        }
        col.add_child(&grid);
        col.add_child(&HSeparator::new_alloc());
        let mut wire = w::push_button(WIRE_LABEL, Some("world"), WIRE_TOOLTIP);
        wire.set_focus_mode(FocusMode::NONE);
        wire.connect("pressed", &Callable::from_object_method(&gd, "on_wire"));
        col.add_child(&wire);
        let mut hint = Label::new_alloc();
        hint.set_name("PreviewHint");
        hint.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
        hint.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        hint.set_custom_minimum_size(ui_kit::sz(60.0, 0.0));
        hint.set_modulate(ui_kit::DIM_MODULATE);
        col.add_child(&hint);
        self.preview_hint = Some((hint, false));
        self.update_preview_hint(true);
        self.update_toolbar();
    }

    /// The hint under the Pixel World panel says how to see the pixel image, or that it is on.
    fn update_preview_hint(&mut self, force: bool) {
        let on = super::super::preview_plugin::preview_on();
        let Some((hint, shown)) = &mut self.preview_hint else { return };
        if !hint.is_instance_valid() || (!force && *shown == on) {
            return;
        }
        *shown = on;
        let label = sections::PREVIEW_LABEL;
        let text = if on {
            format!("\"{label}\" is on: the 3D viewport shows the scene through the pixel renderer (editor grid and gizmos hidden).")
        } else {
            format!("Turn on \"{label}\" in the 3D viewport toolbar to see the scene through the pixel renderer.")
        };
        hint.set_text(&text);
        hint.set_tooltip_text(&text);
    }
}
