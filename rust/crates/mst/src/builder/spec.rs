//! `ShapeSpec` <-> Dictionary (the "recipe" stored with a mesh so a shape can be re-generated).

use godot::prelude::*;
use pt_core::build::{ShapeKind, ShapeSpec};
use pt_core::math::v3;

pub fn to_dict(s: &ShapeSpec) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("kind", s.kind as i64);
    d.set("size", Vector3::new(s.size.x, s.size.y, s.size.z));
    d.set("segments", s.segments as i64);
    d.set("steps", s.steps as i64);
    d.set("thickness", s.thickness);
    d.set("smooth", s.smooth);
    d.set("rings", s.rings as i64);
    d.set("arc", s.arc);
    d.set("inner", s.inner);
    d.set("opening", s.opening);
    d.set("door_height", s.door_height);
    d.set("arched", s.arched);
    d.set("profile", s.profile as i64);
    d.set("side", s.side as i64);
    d
}

/// Reads known keys over a default spec (missing / wrong-typed keys keep defaults).
pub fn from_dict(d: &VarDictionary, base: ShapeSpec) -> ShapeSpec {
    let mut s = base;
    let get = |k: &str| d.get(k);
    if let Some(v) = get("kind").and_then(|v| v.try_to::<i64>().ok()) {
        s.kind = ShapeKind::from_i64(v);
    }
    if let Some(v) = get("size").and_then(|v| v.try_to::<Vector3>().ok()) {
        s.size = v3(v.x, v.y, v.z);
    }
    if let Some(v) = get("segments").and_then(|v| v.try_to::<i64>().ok()) {
        s.segments = v.clamp(1, 128) as u32;
    }
    if let Some(v) = get("steps").and_then(|v| v.try_to::<i64>().ok()) {
        s.steps = v.clamp(1, 128) as u32;
    }
    if let Some(v) = get("thickness").and_then(|v| v.try_to::<f64>().ok()) {
        s.thickness = v as f32;
    }
    if let Some(v) = get("smooth").and_then(|v| v.try_to::<bool>().ok()) {
        s.smooth = v;
    }
    if let Some(v) = get("rings").and_then(|v| v.try_to::<i64>().ok()) {
        s.rings = v.clamp(0, 16) as u32;
    }
    let float = |k: &str| get(k).and_then(|v| v.try_to::<f64>().ok()).map(|v| v as f32);
    if let Some(v) = float("arc") {
        s.arc = v.clamp(5.0, 355.0);
    }
    if let Some(v) = float("inner") {
        s.inner = v.clamp(0.01, 0.95);
    }
    if let Some(v) = float("opening") {
        s.opening = v.max(0.01);
    }
    if let Some(v) = float("door_height") {
        s.door_height = v.max(0.01);
    }
    if let Some(v) = get("arched").and_then(|v| v.try_to::<bool>().ok()) {
        s.arched = v;
    }
    if let Some(v) = get("profile").and_then(|v| v.try_to::<i64>().ok()) {
        s.profile = v.clamp(0, 5) as u8;
    }
    if let Some(v) = get("side").and_then(|v| v.try_to::<i64>().ok()) {
        s.side = v.clamp(0, 2) as u8;
    }
    s
}
