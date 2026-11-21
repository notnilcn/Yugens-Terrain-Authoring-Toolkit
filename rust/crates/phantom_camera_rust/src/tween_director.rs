use godot::classes::{INode, Node};
use godot::prelude::*;
use crate::manager::PhantomCameraManager;
use crate::resources::TweenDirectorResource;
#[derive(GodotClass)]
#[class(base=Node, tool)]
pub struct PhantomCameraTweenDirector {
    base: Base<Node>,
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
    fn enter_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_added(&self.to_gd().upcast::<Node>());
    }
    fn exit_tree(&mut self) {
        PhantomCameraManager::notify_tween_director_removed(&self.to_gd().upcast::<Node>());
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
}
impl PhantomCameraTweenDirector {
    pub fn director_resources(&self) -> Array<Gd<TweenDirectorResource>> {
        self.tween_director.clone()
    }
}
