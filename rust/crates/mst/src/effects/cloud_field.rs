use crate::globals;
use godot::prelude::*;
use std::cell::{Cell, RefCell};
thread_local! {
    static PROVIDERS: RefCell<Vec<(InstanceId, Option<InstanceId>)>> = const { RefCell::new(Vec::new()) };
    static WARNED: RefCell<bool> = const { RefCell::new(false) };
    static SKY: Cell<Option<(InstanceId, SkyState)>> = const { Cell::new(None) };
}
#[derive(Clone, Copy, Debug)]
pub struct SkyState {
    pub wind: Vector2,
    pub cloud_scale: f32,
    pub coverage: f32,
}
pub fn sky() -> Option<SkyState> {
    SKY.with(|s| s.get()).map(|(_, s)| s)
}
pub fn publish_sky(owner: InstanceId, state: SkyState, only_if_empty: bool) {
    SKY.with(|s| {
        if !only_if_empty || s.get().is_none_or(|(o, _)| o == owner) {
            s.set(Some((owner, state)));
        }
    });
}
pub fn clear_sky(owner: InstanceId) {
    SKY.with(|s| {
        if s.get().is_some_and(|(o, _)| o == owner) {
            s.set(None);
        }
    });
}
pub fn register_provider(id: InstanceId, scope: Option<InstanceId>) {
    let count = PROVIDERS.with_borrow_mut(|p| {
        match p.iter_mut().find(|(x, _)| *x == id) {
            Some(entry) => entry.1 = scope,
            None => p.push((id, scope)),
        }
        p.len()
    });
    if count > 1 && !WARNED.with_borrow(|w| *w) {
        WARNED.with_borrow_mut(|w| *w = true);
        godot_warn!(
            "Marching Squares Terrain: {count} PixelSky cloud providers are registered. One field drives every cloud consumer: a provider with `cloud_camera_scope` set is active only for that camera, unscoped providers act as fallbacks in registration order."
        );
    }
}
pub fn unregister_provider(id: InstanceId) {
    let empty = PROVIDERS.with_borrow_mut(|p| {
        p.retain(|(x, _)| *x != id);
        p.is_empty()
    });
    if empty {
        publish_neutral();
    }
}
pub fn publish_neutral() {
    globals::assign("pt_cloud_c", &Vector4::ZERO.to_variant());
}
pub fn active_provider(
    me: InstanceId,
    my_scope: Option<InstanceId>,
    lens: Option<InstanceId>,
) -> Option<InstanceId> {
    let scopes = PROVIDERS.with_borrow_mut(|p| {
        if let Some(entry) = p.iter_mut().find(|(x, _)| *x == me) {
            entry.1 = my_scope;
        }
        p.clone()
    });
    if let Some(cam) = lens
        && let Some((id, _)) = scopes.iter().find(|(_, s)| *s == Some(cam))
    {
        return Some(*id);
    }
    scopes.iter().find(|(_, s)| s.is_none()).map(|(id, _)| *id)
}
