#![allow(non_snake_case)]
use godot::classes::{Control, Engine, INode, Node, ProjectSettings, SceneTree};
use godot::prelude::*;
use crate::abi::{self, PcamKind};
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PhantomCameraManager {
    base: Base<Node>,
    #[var(
        get = get_phantom_camera_hosts,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCameraHost",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_hosts: Array<Gd<Node>>,
    #[var(
        get = get_phantom_camera_2ds,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCamera2D",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_2ds: Array<Gd<Node>>,
    #[var(
        get = get_phantom_camera_3ds,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "Node",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_3ds: Array<Gd<Node>>,
    #[var(
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCameraTweenDirector",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_tween_directors: Array<Gd<Node>>,
    #[var(usage_flags = [SCRIPT_VARIABLE])]
    screen_size: Vector2i,
    viewfinder: Option<Gd<Control>>,
}
#[godot_api]
impl INode for PhantomCameraManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            phantom_camera_hosts: Array::new(),
            phantom_camera_2ds: Array::new(),
            phantom_camera_3ds: Array::new(),
            phantom_camera_tween_directors: Array::new(),
            screen_size: Vector2i::ONE,
            viewfinder: None,
        }
    }
    fn enter_tree(&mut self) {
        if !Engine::singleton().has_singleton(abi::PCAM_MANAGER_NODE_NAME) {
            let instance = self.to_gd().upcast::<Object>();
            Engine::singleton().register_singleton(abi::PCAM_MANAGER_NODE_NAME, &instance);
        }
        Engine::singleton().set_physics_jitter_fix(0.0);
    }
    fn exit_tree(&mut self) {
        Engine::singleton().unregister_singleton(abi::PCAM_MANAGER_NODE_NAME);
    }
    fn ready(&mut self) {
        self.screen_size = project_screen_size();
        if Engine::singleton().is_editor_hint() {
            let callable = self.base().callable("_pcam_manager_settings_changed");
            let mut settings = ProjectSettings::singleton();
            let _ = settings.connect("settings_changed", &callable);
        } else {
            let instance_id = self.base().instance_id();
            let callable = Callable::from_fn("pcam_manager_size_changed", move |_args| {
                if let Ok(mut manager) =
                    Gd::<PhantomCameraManager>::try_from_instance_id(instance_id)
                {
                    let visible = {
                        let bound = manager.bind();
                        bound
                            .base()
                            .get_viewport()
                            .map(|viewport| viewport.get_visible_rect().size)
                    };
                    if let Some(size) = visible {
                        manager.bind_mut().screen_size =
                            Vector2i::new(size.x as i32, size.y as i32);
                    }
                }
                Variant::nil()
            });
            let mut root = self.base().get_tree().get_root();
            let _ = root.connect("size_changed", &callable);
        }
    }
}
#[godot_api]
impl PhantomCameraManager {
    #[func]
    fn _pcam_manager_settings_changed(&mut self) {
        self.screen_size = project_screen_size();
    }
    #[signal]
    fn noise_2d_emitted(noise_output: Transform2D, emitter_layer: i64);
    #[signal]
    fn noise_3d_emitted(noise_output: Transform3D, emitter_layer: i64);
    #[signal]
    fn pcam_host_added_to_scene(pcam_host: Gd<Node>);
    #[signal]
    fn pcam_host_removed_from_scene(pcam_host: Gd<Node>);
    #[signal]
    fn pcam_added_to_scene(pcam: Gd<Node>);
    #[signal]
    fn pcam_removed_from_scene(pcam: Gd<Node>);
    #[signal]
    fn pcam_priority_changed(pcam: Gd<Node>);
    #[signal]
    fn pcam_visibility_changed(pcam: Gd<Node>);
    #[signal]
    fn pcam_teleport(pcam: Gd<Node>);
    #[signal]
    fn limit_2d_changed(side: i64, limit: i64);
    #[signal]
    fn draw_limit_2d(enabled: bool);
    #[signal]
    fn camera_3d_resource_changed(property: GString, value: Variant);
    #[signal]
    fn viewfinder_pcam_host_switch(pcam_host: Gd<Node>);
    #[signal]
    fn pcam_priority_override(pcam: Gd<Node>, shouldOverride: bool);
    #[signal]
    fn pcam_dead_zone_changed(pcam: Gd<Node>);
    #[signal]
    fn pcam_host_layer_changed(pcam: Gd<Node>);
    #[func]
    fn pcam_host_added(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraHost") {
            self.phantom_camera_hosts.push(&caller);
            let this = self.to_gd();
            emit_on(&this, "pcam_host_added_to_scene", &[caller.to_variant()]);
        } else {
            godot_error!("This method can only be called from a PhantomCameraHost node");
        }
    }
    #[func]
    fn pcam_host_removed(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraHost") {
            self.phantom_camera_hosts.erase(&caller);
            let this = self.to_gd();
            emit_on(
                &this,
                "pcam_host_removed_from_scene",
                &[caller.to_variant()],
            );
        } else {
            godot_error!("This method can only be called from a PhantomCameraHost node");
        }
    }
    #[func]
    fn pcam_added(&mut self, caller: Variant) {
        let Ok(pcam) = caller.try_to::<Gd<Node>>() else {
            godot_error!("This method can only be called from a PhantomCamera node");
            return;
        };
        let kind = if pcam.is_class("PhantomCamera2D") {
            PcamKind::TwoD
        } else if pcam.is_class("PhantomCamera3D") {
            PcamKind::ThreeD
        } else {
            godot_error!("This method can only be called from a PhantomCamera node");
            return;
        };
        self.push_pcam_registry(kind, &pcam);
        let this = self.to_gd();
        emit_on(&this, "pcam_added_to_scene", &[pcam.to_variant()]);
    }
    #[func]
    fn pcam_removed(&mut self, caller: Variant) {
        let Ok(pcam) = caller.try_to::<Gd<Node>>() else {
            godot_error!("This method can only be called from a PhantomCamera node");
            return;
        };
        let kind = if pcam.is_class("PhantomCamera2D") {
            PcamKind::TwoD
        } else if pcam.is_class("PhantomCamera3D") {
            PcamKind::ThreeD
        } else {
            godot_error!("This method can only be called from a PhantomCamera node");
            return;
        };
        self.erase_pcam_registry(kind, &pcam);
        let this = self.to_gd();
        emit_on(&this, "pcam_removed_from_scene", &[pcam.to_variant()]);
    }
    #[func]
    fn pcam_tween_director_added(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraTweenDirector") {
            self.phantom_camera_tween_directors.push(&caller);
        } else {
            godot_error!("This function can only be called from a PhantomCameraTweenDirector node");
        }
    }
    #[func]
    fn pcam_tween_director_removed(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraTweenDirector") {
            self.phantom_camera_tween_directors.erase(&caller);
        } else {
            godot_error!("This function can only be called from a PhantomCameraTweenDirector node");
        }
    }
    #[func]
    fn get_phantom_camera_hosts(&self) -> Array<Gd<Node>> {
        self.phantom_camera_hosts.clone()
    }
    #[func]
    fn get_phantom_camera_2ds(&self) -> Array<Gd<Node>> {
        self.phantom_camera_2ds.clone()
    }
    #[func]
    fn get_phantom_camera_3ds(&self) -> Array<Gd<Node>> {
        self.phantom_camera_3ds.clone()
    }
    #[func]
    fn set_viewfinder(&mut self, caller: Gd<Node>, value: Option<Gd<Control>>) {
        if caller.is_class("PhantomCameraHost") {
            self.viewfinder = value;
        } else {
            godot_error!("Viewfinder can only be set internally by the addon");
        }
    }
    #[func]
    fn get_viewfinder(&self) -> Option<Gd<Control>> {
        self.viewfinder.clone()
    }
    #[func]
    fn scene_changed(&mut self) {
        self.phantom_camera_2ds.clear();
        self.phantom_camera_3ds.clear();
        self.phantom_camera_hosts.clear();
        self.phantom_camera_tween_directors.clear();
    }
}
impl PhantomCameraManager {
    fn push_pcam_registry(&mut self, kind: PcamKind, pcam: &Gd<Node>) {
        match kind {
            PcamKind::TwoD => self.phantom_camera_2ds.push(pcam),
            PcamKind::ThreeD => self.phantom_camera_3ds.push(pcam),
        }
    }
    fn erase_pcam_registry(&mut self, kind: PcamKind, pcam: &Gd<Node>) {
        match kind {
            PcamKind::TwoD => {
                self.phantom_camera_2ds.erase(pcam);
            }
            PcamKind::ThreeD => {
                self.phantom_camera_3ds.erase(pcam);
            }
        }
    }
}
fn emit_on(manager: &Gd<PhantomCameraManager>, signal: &str, args: &[Variant]) {
    let mut object = manager.clone().upcast::<Object>();
    let _ = object.emit_signal(signal, args);
}
impl PhantomCameraManager {
    pub fn active() -> Option<Gd<PhantomCameraManager>> {
        let engine = Engine::singleton();
        if engine.has_singleton(abi::PCAM_MANAGER_NODE_NAME)
            && let Some(instance) = engine.get_singleton(abi::PCAM_MANAGER_NODE_NAME)
            && let Ok(manager) = instance.try_cast::<PhantomCameraManager>()
        {
            return Some(manager);
        }
        let main_loop = Engine::singleton().get_main_loop()?;
        let tree = main_loop.try_cast::<SceneTree>().ok()?;
        let node = tree
            .get_root()
            .get_node_or_null(abi::PCAM_MANAGER_NODE_NAME)?;
        node.try_cast::<PhantomCameraManager>().ok()
    }
    pub fn active_or_editor() -> Option<Gd<PhantomCameraManager>> {
        if let Some(manager) = Self::active() {
            return Some(manager);
        }
        if !Engine::singleton().is_editor_hint() {
            return None;
        }
        let main_loop = Engine::singleton().get_main_loop()?;
        let tree = main_loop.try_cast::<SceneTree>().ok()?;
        let mut root = tree.get_root();
        let mut manager = PhantomCameraManager::new_alloc();
        manager.set_name(abi::PCAM_MANAGER_NODE_NAME);
        let instance = manager.clone().upcast::<Object>();
        Engine::singleton().register_singleton(abi::PCAM_MANAGER_NODE_NAME, &instance);
        root.call_deferred("add_child", &[manager.to_variant()]);
        Some(manager)
    }
    pub fn notify_host_added(host: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        manager.bind_mut().phantom_camera_hosts.push(host);
        emit_on(&manager, "pcam_host_added_to_scene", &[host.to_variant()]);
    }
    pub fn notify_host_removed(host: &Gd<Node>) {
        let Some(mut manager) = Self::active() else {
            return;
        };
        manager.bind_mut().phantom_camera_hosts.erase(host);
        emit_on(
            &manager,
            "pcam_host_removed_from_scene",
            &[host.to_variant()],
        );
    }
    pub fn notify_pcam_added(kind: PcamKind, pcam: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        manager.bind_mut().push_pcam_registry(kind, pcam);
        emit_on(&manager, "pcam_added_to_scene", &[pcam.to_variant()]);
    }
    pub fn notify_pcam_removed(kind: PcamKind, pcam: &Gd<Node>) {
        let Some(mut manager) = Self::active() else {
            return;
        };
        manager.bind_mut().erase_pcam_registry(kind, pcam);
        emit_on(&manager, "pcam_removed_from_scene", &[pcam.to_variant()]);
    }
    pub fn notify_tween_director_added(director: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        let mut manager = manager.bind_mut();
        manager.phantom_camera_tween_directors.push(director);
    }
    pub fn notify_tween_director_removed(director: &Gd<Node>) {
        let Some(mut manager) = Self::active() else {
            return;
        };
        let mut manager = manager.bind_mut();
        manager.phantom_camera_tween_directors.erase(director);
    }
    pub fn notify_priority_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_priority_changed", &[pcam.to_variant()]);
    }
    pub fn notify_visibility_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_visibility_changed", &[pcam.to_variant()]);
    }
    pub fn notify_teleport(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_teleport", &[pcam.to_variant()]);
    }
    pub fn notify_host_layer_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_host_layer_changed", &[pcam.to_variant()]);
    }
    pub fn notify_dead_zone_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_dead_zone_changed", &[pcam.to_variant()]);
    }
    pub fn notify_priority_override(pcam: &Gd<Node>, should_override: bool) {
        Self::emit_signal_for(
            "pcam_priority_override",
            &[pcam.to_variant(), should_override.to_variant()],
        );
    }
    pub fn notify_limit_2d_changed(side: i64, limit: i64) {
        Self::emit_signal_for("limit_2d_changed", &[side.to_variant(), limit.to_variant()]);
    }
    pub fn notify_draw_limit_2d(enabled: bool) {
        Self::emit_signal_for("draw_limit_2d", &[enabled.to_variant()]);
    }
    pub fn notify_camera_3d_resource_changed(property: &str, value: Variant) {
        Self::emit_signal_for(
            "camera_3d_resource_changed",
            &[GString::from(property).to_variant(), value],
        );
    }
    pub fn notify_viewfinder_host_switch(host: &Gd<Node>) {
        Self::emit_signal_for("viewfinder_pcam_host_switch", &[host.to_variant()]);
    }
    pub fn notify_noise_emitted(kind: PcamKind, output: Variant, emitter_layer: i64) {
        let signal = match kind {
            PcamKind::TwoD => "noise_2d_emitted",
            PcamKind::ThreeD => "noise_3d_emitted",
        };
        Self::emit_signal_for(signal, &[output, emitter_layer.to_variant()]);
    }
    pub fn pcam_2ds() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_2ds.clone()
        })
    }
    pub fn pcam_3ds() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_3ds.clone()
        })
    }
    pub fn hosts() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_hosts.clone()
        })
    }
    pub fn tween_directors() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_tween_directors.clone()
        })
    }
    pub fn screen_size() -> Vector2i {
        Self::active().map_or(Vector2i::ONE, |manager| manager.bind().screen_size)
    }
    pub fn viewfinder() -> Option<Gd<Control>> {
        Self::active().and_then(|manager| manager.bind().viewfinder.clone())
    }
    fn emit_signal_for(signal: &str, args: &[Variant]) {
        let Some(manager) = Self::active() else {
            return;
        };
        emit_on(&manager, signal, args);
    }
}
fn project_screen_size() -> Vector2i {
    Vector2i::new(
        project_setting("display/window/size/viewport_width"),
        project_setting("display/window/size/viewport_height"),
    )
}
fn project_setting(name: &str) -> i32 {
    ProjectSettings::singleton()
        .get_setting(name)
        .try_to::<i64>()
        .map_or(1, |value| value as i32)
}
