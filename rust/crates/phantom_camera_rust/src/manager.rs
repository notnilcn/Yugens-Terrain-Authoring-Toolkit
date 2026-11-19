// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/managers/phantom_camera_manager.gd
//
//! `PhantomCameraManager` — the addon's registry singleton.
//!
//! Ported from `scripts/managers/phantom_camera_manager.gd` (190 L, oracle at
//! `parity/pcam_oracle/phantom_camera/`). Line anchors:
//!
//! * signals `:9-37`
//! * registry lists `:43-46`, read-only accessors `:54-68`, `screen_size` `:70`
//! * `_enter_tree` `:76-79`, `_exit_tree` `:84-85`, `_ready` `:88-107`
//! * add/remove dispatch `:117-163`, getters `:166-173`,
//!   viewfinder `:175-182`, `scene_changed` `:184-188`
//!
//! # Deviations forced by the port
//!
//! * The oracle discriminates 2D/3D with `is_instance_of(caller, PhantomCamera2D)`
//!   / `caller.is_class("PhantomCamera3D")` (`:132-150`). gdext cannot name
//!   another GDScript class, so the port keeps the same `#[func]` signatures and
//!   dispatches on the *registered class name* via `Object::is_class`, which is
//!   what `is_instance_of` reduces to for native extension classes. Both PCam
//!   classes also call the typed seams below, so the class-name path is only
//!   taken by GDScript callers.
//! * The registry lists are `Array<Gd<Node>>` instead of
//!   `Array[PhantomCameraHost]` / `Array[PhantomCamera2D]` / `Array[Node]` /
//!   `Array[PhantomCameraTweenDirector]` — same Variant type (`Array`), but the
//!   element hint string of the four read-only properties is reproduced
//!   explicitly with `hint_string = "PhantomCameraHost"` etc. so the ABI table
//!   still matches; only Godot's typed-array enforcement is looser.
//! * Private backing variables (`_phantom_camera_host_list`, `_viewfinder`) are
//!   plain Rust fields and therefore absent from `get_property_list` (they are
//!   `PROPERTY_USAGE_SCRIPT_VARIABLE`-only in the oracle and never stored in a
//!   `.tscn`).
//!
//! # Runtime seams for the other modules
//!
//! Hosts, PCs and emitters never touch this node's fields: they call
//! [`PhantomCameraManager::notify_*`], which resolve the registered instance via
//! [`PhantomCameraManager::active`] (engine singleton first, `/root` node second
//! — the oracle's `Engine.get_singleton(...)` /
//! `get_tree().root.get_node(...)` pair), update the registry and emit the
//! matching oracle signal. With no manager in the tree every seam is a silent
//! no-op, so a harness can instantiate a host without the autoload.

// `pcam_priority_override`'s second signal argument is spelled `shouldOverride`
// in the oracle (`phantom_camera_manager.gd:35`) and that name is part of the
// recorded ABI (`abi_gd.json`), so the Rust parameter keeps the camel-case form.
// gdext re-emits the signal signature inside the macro expansion, where a
// per-item `#[allow]` does not reach it, hence the module-level allow.
#![allow(non_snake_case)]

use godot::classes::{Control, Engine, INode, Node, ProjectSettings, SceneTree};
use godot::prelude::*;

use crate::abi::{self, PcamKind};

/// `phantom_camera_manager.gd:1-190`.
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PhantomCameraManager {
    base: Base<Node>,

    /// `phantom_camera_manager.gd:43` / `:54-56`.
    #[var(
        get = get_phantom_camera_hosts,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCameraHost",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_hosts: Array<Gd<Node>>,

    /// `phantom_camera_manager.gd:44` / `:58-60`.
    #[var(
        get = get_phantom_camera_2ds,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCamera2D",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_2ds: Array<Gd<Node>>,

    /// `phantom_camera_manager.gd:45` / `:62-64` — deliberately untyped in the
    /// oracle too, to support `disable_3d` export templates.
    #[var(
        get = get_phantom_camera_3ds,
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "Node",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_3ds: Array<Gd<Node>>,

    /// `phantom_camera_manager.gd:46` / `:66-68`.
    ///
    /// The oracle's property getter is an inline lambda and there is no named
    /// `get_phantom_camera_tween_directors()` script function, so the accessor
    /// stays generated (`get_phantom_camera_tween_directors`, registered as a
    /// method by gdext) instead of being a hand-written `#[func]`.
    #[var(
        no_set,
        hint = ARRAY_TYPE,
        hint_string = "PhantomCameraTweenDirector",
        usage_flags = [SCRIPT_VARIABLE]
    )]
    phantom_camera_tween_directors: Array<Gd<Node>>,

    /// `phantom_camera_manager.gd:70`.
    #[var(usage_flags = [SCRIPT_VARIABLE])]
    screen_size: Vector2i,

    /// `phantom_camera_manager.gd:48` — set only through `set_viewfinder`.
    viewfinder: Option<Gd<Control>>,
}

// ---------------------------------------------------------------------------
// Node lifecycle — `phantom_camera_manager.gd:76-107`
// ---------------------------------------------------------------------------

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

    /// `phantom_camera_manager.gd:76-79`.
    fn enter_tree(&mut self) {
        if !Engine::singleton().has_singleton(abi::PCAM_MANAGER_NODE_NAME) {
            let instance = self.to_gd().upcast::<Object>();
            Engine::singleton().register_singleton(abi::PCAM_MANAGER_NODE_NAME, &instance);
        }
        Engine::singleton().set_physics_jitter_fix(0.0);
    }

    /// `phantom_camera_manager.gd:84-85`.
    fn exit_tree(&mut self) {
        Engine::singleton().unregister_singleton(abi::PCAM_MANAGER_NODE_NAME);
    }

    /// `phantom_camera_manager.gd:88-107`.
    fn ready(&mut self) {
        self.screen_size = project_screen_size();

        if Engine::singleton().is_editor_hint() {
            // `:97-102` — editor: follow ProjectSettings.
            // MST vendoring: an object-method callable, not `Callable::from_fn`. gdext tracks
            // Rust callables connected in the editor and warns ("TypedSignal connections are now
            // auto-disconnected") at editor shutdown for every one still connected, and the
            // receiver here (ProjectSettings) outlives the extension.
            let callable = self.base().callable("_pcam_manager_settings_changed");
            let mut settings = ProjectSettings::singleton();
            let _ = settings.connect("settings_changed", &callable);
        } else {
            // `:105-107` — runtime: follow the root viewport's size.
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

// ---------------------------------------------------------------------------
// Signals (`phantom_camera_manager.gd:9-37`) and the public `#[func]` surface
// (`:117-188`). gdext allows exactly one `#[godot_api]` block per class, so the
// signal declarations and the methods share it.
// ---------------------------------------------------------------------------

#[godot_api]
impl PhantomCameraManager {
    /// Editor-only `ProjectSettings.settings_changed` receiver (`:97-102`). MST vendoring
    /// addition; see `ready`.
    #[func]
    fn _pcam_manager_settings_changed(&mut self) {
        self.screen_size = project_screen_size();
    }

    /// `phantom_camera_manager.gd:9`.
    #[signal]
    fn noise_2d_emitted(noise_output: Transform2D, emitter_layer: i64);

    /// `phantom_camera_manager.gd:10`.
    #[signal]
    fn noise_3d_emitted(noise_output: Transform3D, emitter_layer: i64);

    /// `phantom_camera_manager.gd:13`.
    #[signal]
    fn pcam_host_added_to_scene(pcam_host: Gd<Node>);

    /// `phantom_camera_manager.gd:14`.
    #[signal]
    fn pcam_host_removed_from_scene(pcam_host: Gd<Node>);

    /// `phantom_camera_manager.gd:17`.
    #[signal]
    fn pcam_added_to_scene(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:18`.
    #[signal]
    fn pcam_removed_from_scene(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:21`.
    #[signal]
    fn pcam_priority_changed(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:22`.
    #[signal]
    fn pcam_visibility_changed(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:24`.
    #[signal]
    fn pcam_teleport(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:27`.
    #[signal]
    fn limit_2d_changed(side: i64, limit: i64);

    /// `phantom_camera_manager.gd:28`.
    #[signal]
    fn draw_limit_2d(enabled: bool);

    /// `phantom_camera_manager.gd:31`.
    #[signal]
    fn camera_3d_resource_changed(property: GString, value: Variant);

    /// `phantom_camera_manager.gd:34`.
    #[signal]
    fn viewfinder_pcam_host_switch(pcam_host: Gd<Node>);

    /// `phantom_camera_manager.gd:35` — the oracle spells the argument
    /// `shouldOverride`, so the ABI table records that exact name (see the
    /// module-level `non_snake_case` note).
    #[signal]
    fn pcam_priority_override(pcam: Gd<Node>, shouldOverride: bool);

    /// `phantom_camera_manager.gd:36`.
    #[signal]
    fn pcam_dead_zone_changed(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:37`.
    #[signal]
    fn pcam_host_layer_changed(pcam: Gd<Node>);

    /// `phantom_camera_manager.gd:117-122`.
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

    /// `phantom_camera_manager.gd:124-129`.
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

    /// `phantom_camera_manager.gd:132-140`.
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

    /// `phantom_camera_manager.gd:142-150`.
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

    /// `phantom_camera_manager.gd:153-157`.
    #[func]
    fn pcam_tween_director_added(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraTweenDirector") {
            self.phantom_camera_tween_directors.push(&caller);
        } else {
            godot_error!("This function can only be called from a PhantomCameraTweenDirector node");
        }
    }

    /// `phantom_camera_manager.gd:159-163`.
    #[func]
    fn pcam_tween_director_removed(&mut self, caller: Gd<Node>) {
        if caller.is_class("PhantomCameraTweenDirector") {
            self.phantom_camera_tween_directors.erase(&caller);
        } else {
            godot_error!("This function can only be called from a PhantomCameraTweenDirector node");
        }
    }

    /// `phantom_camera_manager.gd:166-167`.
    #[func]
    fn get_phantom_camera_hosts(&self) -> Array<Gd<Node>> {
        self.phantom_camera_hosts.clone()
    }

    /// `phantom_camera_manager.gd:169-170`.
    #[func]
    fn get_phantom_camera_2ds(&self) -> Array<Gd<Node>> {
        self.phantom_camera_2ds.clone()
    }

    /// `phantom_camera_manager.gd:172-173`.
    #[func]
    fn get_phantom_camera_3ds(&self) -> Array<Gd<Node>> {
        self.phantom_camera_3ds.clone()
    }

    /// `phantom_camera_manager.gd:175-179`.
    #[func]
    fn set_viewfinder(&mut self, caller: Gd<Node>, value: Option<Gd<Control>>) {
        if caller.is_class("PhantomCameraHost") {
            self.viewfinder = value;
        } else {
            godot_error!("Viewfinder can only be set internally by the addon");
        }
    }

    /// `phantom_camera_manager.gd:181-182`.
    #[func]
    fn get_viewfinder(&self) -> Option<Gd<Control>> {
        self.viewfinder.clone()
    }

    /// `phantom_camera_manager.gd:184-188`.
    #[func]
    fn scene_changed(&mut self) {
        self.phantom_camera_2ds.clear();
        self.phantom_camera_3ds.clear();
        self.phantom_camera_hosts.clear();
        self.phantom_camera_tween_directors.clear();
    }
}

// ---------------------------------------------------------------------------
// Internal registry mutators (no signals)
// ---------------------------------------------------------------------------

impl PhantomCameraManager {
    /// `phantom_camera_manager.gd:133-138` — append to the 2D/3D registry.
    fn push_pcam_registry(&mut self, kind: PcamKind, pcam: &Gd<Node>) {
        match kind {
            PcamKind::TwoD => self.phantom_camera_2ds.push(pcam),
            PcamKind::ThreeD => self.phantom_camera_3ds.push(pcam),
        }
    }

    /// `phantom_camera_manager.gd:143-149` — drop from the 2D/3D registry.
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

/// Emit one of this node's signals (`phantom_camera_manager.gd:9-37`).
///
/// The signal is emitted through an `Object` view of the instance, so no
/// `Base::base_mut()` guard is alive while the engine runs the connected
/// callables. That matters: the host's `_pcam_visibility_changed` handler
/// synchronously walks `phantom_camera_2ds`
/// (`phantom_camera_host.gd:1296-1302` → `:369-374`), and a nested
/// `Gd::bind()` on an instance that is currently mutably borrowed would abort.
/// `Gd<Object>`'s `DerefMut` only touches the engine object, so a callback can
/// re-enter the manager exactly like it can in GDScript.
fn emit_on(manager: &Gd<PhantomCameraManager>, signal: &str, args: &[Variant]) {
    let mut object = manager.clone().upcast::<Object>();
    let _ = object.emit_signal(signal, args);
}

// ---------------------------------------------------------------------------
// Cross-module seams
// ---------------------------------------------------------------------------

impl PhantomCameraManager {
    /// The registered manager instance, or `None` when the manager is absent.
    ///
    /// Mirrors the oracle's two lookups: `Engine.get_singleton("PhantomCameraManager")`
    /// (`phantom_camera_tween_director.gd:39`, `phantom_camera_host.gd:283`) and
    /// `get_tree().root.get_node("PhantomCameraManager")`
    /// (`phantom_camera_noise_emitter_2d.gd:117`). The engine singleton wins, so
    /// no host depends on node-tree lookup order (`PORT-PLAN.md` §"Cross-class
    /// contract").
    pub fn active() -> Option<Gd<PhantomCameraManager>> {
        // MST vendoring: `has_singleton` first, `get_singleton` logs an ERROR when absent (editor).
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

    /// [`Self::active`], creating the manager on demand in the editor.
    ///
    /// MST vendoring: the editor only instantiates autoloads whose root carries a `@tool`
    /// *script*, so the `PhantomCameraManager` autoload scene (a GDExtension class without a
    /// script) never exists in the editor, while upstream's `@tool` GDScript manager does. Every
    /// host then warned and editor previews had no registry. The first "added" seam in the editor
    /// registers a fresh manager as the engine singleton immediately (so this and every later
    /// lookup succeed) and parents it to `/root` deferred (the tree may be locked). Removal seams
    /// keep using [`Self::active`], so editor shutdown never creates one.
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

    /// `phantom_camera_manager.gd:117-122` — host entered the scene.
    pub fn notify_host_added(host: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        manager.bind_mut().phantom_camera_hosts.push(host);
        emit_on(&manager, "pcam_host_added_to_scene", &[host.to_variant()]);
    }

    /// `phantom_camera_manager.gd:124-129` — host left the scene.
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

    /// `phantom_camera_manager.gd:132-140` — PCam entered the scene.
    pub fn notify_pcam_added(kind: PcamKind, pcam: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        manager.bind_mut().push_pcam_registry(kind, pcam);
        emit_on(&manager, "pcam_added_to_scene", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:142-150` — PCam left the scene.
    pub fn notify_pcam_removed(kind: PcamKind, pcam: &Gd<Node>) {
        let Some(mut manager) = Self::active() else {
            return;
        };
        manager.bind_mut().erase_pcam_registry(kind, pcam);
        emit_on(&manager, "pcam_removed_from_scene", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:153-157`.
    pub fn notify_tween_director_added(director: &Gd<Node>) {
        let Some(mut manager) = Self::active_or_editor() else {
            return;
        };
        let mut manager = manager.bind_mut();
        manager.phantom_camera_tween_directors.push(director);
    }

    /// `phantom_camera_manager.gd:159-163`.
    pub fn notify_tween_director_removed(director: &Gd<Node>) {
        let Some(mut manager) = Self::active() else {
            return;
        };
        let mut manager = manager.bind_mut();
        manager.phantom_camera_tween_directors.erase(director);
    }

    /// `phantom_camera_manager.gd:21` — `pcam_priority_changed`.
    pub fn notify_priority_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_priority_changed", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:22` — `pcam_visibility_changed`.
    pub fn notify_visibility_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_visibility_changed", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:24` — `pcam_teleport`.
    pub fn notify_teleport(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_teleport", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:37` — `pcam_host_layer_changed`.
    pub fn notify_host_layer_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_host_layer_changed", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:36` — `pcam_dead_zone_changed`.
    pub fn notify_dead_zone_changed(pcam: &Gd<Node>) {
        Self::emit_signal_for("pcam_dead_zone_changed", &[pcam.to_variant()]);
    }

    /// `phantom_camera_manager.gd:35` — `pcam_priority_override`.
    pub fn notify_priority_override(pcam: &Gd<Node>, should_override: bool) {
        Self::emit_signal_for(
            "pcam_priority_override",
            &[pcam.to_variant(), should_override.to_variant()],
        );
    }

    /// `phantom_camera_manager.gd:27` — `limit_2d_changed(side, limit)`.
    pub fn notify_limit_2d_changed(side: i64, limit: i64) {
        Self::emit_signal_for("limit_2d_changed", &[side.to_variant(), limit.to_variant()]);
    }

    /// `phantom_camera_manager.gd:28` — `draw_limit_2d(enabled)`.
    pub fn notify_draw_limit_2d(enabled: bool) {
        Self::emit_signal_for("draw_limit_2d", &[enabled.to_variant()]);
    }

    /// `phantom_camera_manager.gd:31` — `camera_3d_resource_changed`.
    pub fn notify_camera_3d_resource_changed(property: &str, value: Variant) {
        Self::emit_signal_for(
            "camera_3d_resource_changed",
            &[GString::from(property).to_variant(), value],
        );
    }

    /// `phantom_camera_manager.gd:34` — `viewfinder_pcam_host_switch`.
    pub fn notify_viewfinder_host_switch(host: &Gd<Node>) {
        Self::emit_signal_for("viewfinder_pcam_host_switch", &[host.to_variant()]);
    }

    /// `phantom_camera_manager.gd:9-10` — noise emitted by an emitter node.
    ///
    /// `output` is the emitter's `Transform2D`/`Transform3D`; it stays a
    /// `Variant` so both families share one seam (each signal signature carries
    /// its concrete transform type).
    pub fn notify_noise_emitted(kind: PcamKind, output: Variant, emitter_layer: i64) {
        let signal = match kind {
            PcamKind::TwoD => "noise_2d_emitted",
            PcamKind::ThreeD => "noise_3d_emitted",
        };
        Self::emit_signal_for(signal, &[output, emitter_layer.to_variant()]);
    }

    /// `phantom_camera_manager.gd:58-60` — registered 2D PCs.
    pub fn pcam_2ds() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_2ds.clone()
        })
    }

    /// `phantom_camera_manager.gd:62-64` — registered 3D PCs.
    pub fn pcam_3ds() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_3ds.clone()
        })
    }

    /// `phantom_camera_manager.gd:54-56` — registered hosts.
    pub fn hosts() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_hosts.clone()
        })
    }

    /// `phantom_camera_manager.gd:66-68` — registered tween directors.
    pub fn tween_directors() -> Array<Gd<Node>> {
        Self::active().map_or_else(Array::new, |manager| {
            manager.bind().phantom_camera_tween_directors.clone()
        })
    }

    /// `phantom_camera_manager.gd:70` — the cached viewport size used by the 2D
    /// limit maths.
    pub fn screen_size() -> Vector2i {
        Self::active().map_or(Vector2i::ONE, |manager| manager.bind().screen_size)
    }

    /// `phantom_camera_manager.gd:181-182` — the viewfinder control registered by
    /// the active host.
    pub fn viewfinder() -> Option<Gd<Control>> {
        Self::active().and_then(|manager| manager.bind().viewfinder.clone())
    }

    /// Resolve the manager, emit a signal on it, ignore a missing instance.
    fn emit_signal_for(signal: &str, args: &[Variant]) {
        let Some(manager) = Self::active() else {
            return;
        };
        emit_on(&manager, signal, args);
    }
}

/// `phantom_camera_manager.gd:90-93` / `:98-101` — `display/window/size/*`.
fn project_screen_size() -> Vector2i {
    Vector2i::new(
        project_setting("display/window/size/viewport_width"),
        project_setting("display/window/size/viewport_height"),
    )
}

/// Read one integer project setting, defaulting to `1` (the oracle's
/// `Vector2i.ONE` starting value) when the setting is missing.
fn project_setting(name: &str) -> i32 {
    ProjectSettings::singleton()
        .get_setting(name)
        .try_to::<i64>()
        .map_or(1, |value| value as i32)
}
