//! The scene's authored plan for which rigs may hand off to which, and how.
//!
//! Intent: a scene author often wants specific hand-offs choreographed - from this rig to
//! that one with this tween, but not the other way round. Placing one of these in the scene,
//! pointing it at the rigs a hand-off leaves and arrives at, and filling in its plans lets
//! the author describe those hand-offs instead of leaving every switch to the general rules.
//!
//! Responsibility: it owns which of the scene's rigs its plans speak about, and the plans
//! themselves, and it registers itself so the scene's camera driver can find them while a
//! switch happens. The rigs are held here, as references the scene assigns, because they are
//! members of this scene; the plans keep only what is reusable between scenes - the tween
//! and the rules for matching. It does not apply the tweens, does not choose the live rig,
//! and does not interpret the plans; it only makes them reachable.
//!
//! Why a node: the plans and the rigs they name belong to one scene and should be found and
//! forgotten with it, and the driver needs a single thing in the tree it can ask while a
//! switch is in progress.
//!
//! Why a new type: a plain resource would have no lifetime in the tree to register and
//! unregister with and could not hold references to the scene's rigs, and the hand-offs are
//! scene composition rather than a reusable asset, so keeping them on a resource shared
//! between scenes would put one scene's hand-offs into another's.
//!
//! Not exported: nothing beyond the rigs and the plan list; those are the whole of what a
//! scene author sets here.
//!
//! The addon's registry is the one global it reaches for, and only to announce that the
//! plans have entered or left the scene.

use godot::classes::{INode, Node};
use godot::prelude::*;
use crate::manager::PhantomCameraManager;
use crate::pcam2d::PhantomCamera2D;
use crate::pcam3d::PhantomCamera3D;
use crate::resources::TweenDirectorResource;
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PhantomCameraTweenDirector {
    base: Base<Node>,
    #[export]
    #[var(get = get_tween_director, set = set_tween_director)]
    tween_director: Array<Gd<TweenDirectorResource>>,
    #[export]
    #[var(get = get_from_phantom_cameras, set = set_from_phantom_cameras)]
    from_phantom_cameras: Array<Gd<Node>>,
    #[export]
    #[var(get = get_to_phantom_cameras, set = set_to_phantom_cameras)]
    to_phantom_cameras: Array<Gd<Node>>,
}
#[godot_api]
impl INode for PhantomCameraTweenDirector {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            tween_director: Array::new(),
            from_phantom_cameras: Array::new(),
            to_phantom_cameras: Array::new(),
        }
    }
    fn enter_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_added(&self.to_gd().upcast::<Node>());
    }
    fn exit_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_removed(&self.to_gd().upcast::<Node>());
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut warnings = PackedStringArray::new();
        if !all_phantom_cameras(&self.from_phantom_cameras) {
            warnings.push(
                "Assign only PhantomCamera2D or PhantomCamera3D nodes to from_phantom_cameras.",
            );
        }
        if !all_phantom_cameras(&self.to_phantom_cameras) {
            warnings.push(
                "Assign only PhantomCamera2D or PhantomCamera3D nodes to to_phantom_cameras.",
            );
        }
        warnings
    }
}
#[godot_api]
impl PhantomCameraTweenDirector {
    #[func]
    pub fn set_tween_director(&mut self, value: Array<Gd<TweenDirectorResource>>) {
        self.tween_director = value;
    }
    #[func]
    pub fn get_tween_director(&self) -> Array<Gd<TweenDirectorResource>> {
        self.tween_director.clone()
    }
    #[func]
    pub fn set_from_phantom_cameras(&mut self, value: Array<Gd<Node>>) {
        self.from_phantom_cameras = value;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    pub fn get_from_phantom_cameras(&self) -> Array<Gd<Node>> {
        self.from_phantom_cameras.clone()
    }
    #[func]
    pub fn set_to_phantom_cameras(&mut self, value: Array<Gd<Node>>) {
        self.to_phantom_cameras = value;
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    pub fn get_to_phantom_cameras(&self) -> Array<Gd<Node>> {
        self.to_phantom_cameras.clone()
    }
}
impl PhantomCameraTweenDirector {
    pub fn director_resources(&self) -> Array<Gd<TweenDirectorResource>> {
        self.tween_director.clone()
    }
    pub fn from_cameras(&self) -> Vec<Gd<Node>> {
        assigned_nodes(&self.from_phantom_cameras)
    }
    pub fn to_cameras(&self) -> Vec<Gd<Node>> {
        assigned_nodes(&self.to_phantom_cameras)
    }
}
fn assigned_nodes(list: &Array<Gd<Node>>) -> Vec<Gd<Node>> {
    list.clone()
        .upcast_any_array()
        .iter_shared()
        .filter_map(|entry| entry.try_to::<Gd<Node>>().ok())
        .filter(|node| node.is_instance_valid())
        .collect()
}
fn all_phantom_cameras(list: &Array<Gd<Node>>) -> bool {
    assigned_nodes(list).iter().all(|node| {
        node.clone().try_cast::<PhantomCamera2D>().is_ok()
            || node.clone().try_cast::<PhantomCamera3D>().is_ok()
    })
}
