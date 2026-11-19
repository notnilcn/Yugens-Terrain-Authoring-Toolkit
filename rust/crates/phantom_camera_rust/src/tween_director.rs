// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Khvor' Gates contributors
// Portions derived from Phantom Camera 0.11.0.3 (MIT, Marcus Skov) addons/phantom_camera/scripts/phantom_camera/phantom_camera_tween_director.gd
//
//! `PhantomCameraTweenDirector` — the data container the host consults when it
//! decides which `PhantomCameraTween` a transition uses.
//!
//! Ported from `scripts/phantom_camera/phantom_camera_tween_director.gd` (60 L,
//! oracle at `parity/pcam_oracle/phantom_camera/`). Line anchors:
//!
//! * export `:21-24`
//! * `_pcam_manager` `:31`
//! * `_enter_tree` `:38-41`, `_exit_tree` `:44-45`
//! * accessors `:52-58`
//!
//! The node itself holds no per-frame behaviour: it registers with the manager
//! on `enter_tree` / `exit_tree` (`:41`, `:45`) and the host reads
//! [`Self::director_resources`] while resolving a transition. The oracle caches
//! the manager in `_pcam_manager`; the port calls the
//! [`PhantomCameraManager`] seams instead, which resolve the same instance and
//! are silent no-ops when it is absent (the oracle dereferences `null` there,
//! `:45`).
//!
//! # Deliberately omitted (editor-only)
//!
//! * `@icon(...)`.
//! * The `_pcam_manager` private variable is not registered as a property (it is
//!   `PROPERTY_USAGE_SCRIPT_VARIABLE`-only in the oracle and never stored in a
//!   `.tscn`), and no longer needs to be cached at all.

use godot::classes::{INode, Node};
use godot::prelude::*;

use crate::manager::PhantomCameraManager;
use crate::resources::TweenDirectorResource;

/// `phantom_camera_tween_director.gd:1-60`.
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PhantomCameraTweenDirector {
    base: Base<Node>,

    /// `phantom_camera_tween_director.gd:21-24`.
    #[export]
    #[var(get = get_tween_director, set = set_tween_director)]
    tween_director: Array<Gd<TweenDirectorResource>>,
}

#[godot_api]
impl INode for PhantomCameraTweenDirector {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            tween_director: Array::new(),
        }
    }

    /// `phantom_camera_tween_director.gd:38-41`.
    fn enter_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_added(&self.to_gd().upcast::<Node>());
    }

    /// `phantom_camera_tween_director.gd:44-45`.
    fn exit_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_removed(&self.to_gd().upcast::<Node>());
    }
}

#[godot_api]
impl PhantomCameraTweenDirector {
    /// `phantom_camera_tween_director.gd:52-54`.
    #[func]
    pub fn set_tween_director(&mut self, value: Array<Gd<TweenDirectorResource>>) {
        self.tween_director = value;
    }

    /// `phantom_camera_tween_director.gd:56-58`.
    #[func]
    pub fn get_tween_director(&self) -> Array<Gd<TweenDirectorResource>> {
        self.tween_director.clone()
    }
}

impl PhantomCameraTweenDirector {
    /// The `TweenDirectorResource` list, for the host's transition resolution.
    ///
    /// Same value as [`Self::get_tween_director`], callable from Rust without
    /// going through the Godot method table.
    pub fn director_resources(&self) -> Array<Gd<TweenDirectorResource>> {
        self.tween_director.clone()
    }
}
