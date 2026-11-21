use crate::globals;
use godot::prelude::*;
use std::cell::{Cell, RefCell};
thread_local! {
    static PROVIDERS: RefCell<Vec<InstanceId>> = const { RefCell::new(Vec::new()) };
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
pub fn register_provider(node: &Gd<Node>) {
    let id = node.instance_id();
    let count = PROVIDERS.with_borrow_mut(|p| {
        if !p.contains(&id) {
            p.push(id);
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
pub fn unregister_provider(node: &Gd<Node>) {
    let id = node.instance_id();
    let empty = PROVIDERS.with_borrow_mut(|p| {
        p.retain(|x| *x != id);
        p.is_empty()
    });
    if empty {
        publish_neutral();
    }
}
pub fn publish_neutral() {
    globals::assign("pt_cloud_c", &Vector4::ZERO.to_variant());
}
fn scope_of(token_id: InstanceId) -> Option<Option<InstanceId>> {
    let obj = Gd::<Object>::try_from_instance_id(token_id).ok()?;
    let scope = obj.get("cloud_camera_scope");
    Some(
        scope
            .try_to::<Option<Gd<Node>>>()
            .ok()
            .flatten()
            .filter(|n| n.is_instance_valid())
            .map(|n| n.instance_id()),
    )
}
pub fn active_provider(
    me: InstanceId,
    my_scope: Option<InstanceId>,
    lens: Option<InstanceId>,
) -> Option<InstanceId> {
    let providers = PROVIDERS.with_borrow(|p| p.clone());
    let scopes: Vec<(InstanceId, Option<InstanceId>)> = providers
        .into_iter()
        .filter_map(|id| {
            if id == me {
                Some((id, my_scope))
            } else {
                scope_of(id).map(|s| (id, s))
            }
        })
        .collect();
    if let Some(cam) = lens
        && let Some((id, _)) = scopes.iter().find(|(_, s)| *s == Some(cam))
    {
        return Some(*id);
    }
    scopes.iter().find(|(_, s)| s.is_none()).map(|(id, _)| *id)
}
