//! The shared cloud-shadow field (port of the original `PixelizerCloudField`).
//!
//! Cloud *providers* (`PixelSky` nodes) register here, and exactly one of them drives the
//! field each frame. The field itself is the `pt_cloud_*` global shader parameters, so every
//! consumer (terrain, grass, objects, water, the god-ray pass, custom shaders that include
//! `pixel_noise.gdshaderinc`) reads it without registering anything.
//!
//! A provider with a `cloud_camera_scope` camera is active only while that camera is the
//! active one, so a two-camera scene can switch cloud fields by switching cameras. Providers
//! without a scope are fallbacks, in registration order. With no provider the field is
//! neutral (`pt_cloud_c.x = 0`, clouds off), so consumers never read stale values.
//!
//! Not ported: the original's per-material push list (`register_material` /
//! `unregister_material` / pruning). It only existed because the split addons had no shared
//! uniforms; here the field lives in global shader parameters.

use std::cell::RefCell;

use godot::prelude::*;

use crate::globals;

thread_local! {
    static PROVIDERS: RefCell<Vec<InstanceId>> = const { RefCell::new(Vec::new()) };
    static WARNED: RefCell<bool> = const { RefCell::new(false) };
}

/// Registers a provider (idempotent). Warns once when several are registered.
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

/// Unregisters a provider; the field turns neutral when none is left.
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

/// Clouds off; the remaining parameters keep their last values (gated by `pt_cloud_c.x`).
pub fn publish_neutral() {
    globals::set("pt_cloud_c", &Vector4::ZERO.to_variant());
}

/// The camera scope of a registered provider (`cloud_camera_scope` property), if any.
fn scope_of(id: InstanceId) -> Option<Option<InstanceId>> {
    let obj = Gd::<Object>::try_from_instance_id(id).ok()?;
    let scope = obj.get("cloud_camera_scope");
    Some(scope.try_to::<Option<Gd<Node>>>().ok().flatten().filter(|n| n.is_instance_valid()).map(|n| n.instance_id()))
}

/// The provider driving the field for `camera`: the first provider scoped to the camera,
/// else the first unscoped provider, else none (neutral field). `me`'s scope is passed in
/// because the asking provider is usually bound while it asks.
pub fn active_provider(me: InstanceId, my_scope: Option<InstanceId>, camera: Option<InstanceId>) -> Option<InstanceId> {
    let providers = PROVIDERS.with_borrow(|p| p.clone());
    let scopes: Vec<(InstanceId, Option<InstanceId>)> = providers
        .into_iter()
        .filter_map(|id| if id == me { Some((id, my_scope)) } else { scope_of(id).map(|s| (id, s)) })
        .collect();
    if let Some(cam) = camera
        && let Some((id, _)) = scopes.iter().find(|(_, s)| *s == Some(cam))
    {
        return Some(*id);
    }
    scopes.iter().find(|(_, s)| s.is_none()).map(|(id, _)| *id)
}
