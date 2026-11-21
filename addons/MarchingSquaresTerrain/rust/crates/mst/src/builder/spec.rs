use godot::prelude::*;
use pt_core::build::{ShapeKind, ShapeSpec};
use pt_core::math::v3;
pub fn to_dict(s: &ShapeSpec) -> VarDictionary {
    let mut d = VarDictionary::new();
    d.set("kind", s.sort as i64);
    d.set("size", Vector3::new(s.size.x, s.size.y, s.size.z));
    d.set("segments", s.stretches as i64);
    d.set("steps", s.steps as i64);
    d.set("thickness", s.thickness);
    d.set("smooth", s.glide);
    d.set("rings", s.rings as i64);
    d.set("arc", s.arc);
    d.set("inner", s.inner);
    d.set("opening", s.opening);
    d.set("door_height", s.door_height);
    d.set("arched", s.arched);
    d.set("profile", s.silhouette as i64);
    d.set("side", s.flank as i64);
    d.set("segments_z", s.segments_z as i64);
    d.set("height_cuts", s.height_cuts as i64);
    d.set("torus_arc", s.torus_arc);
    d.set("tube_arc", s.tube_arc);
    d.set("arch_band", s.arch_band);
    d.set("arch_degrees", s.arch_degrees);
    d.set("end_caps", s.end_caps);
    d.set("step_height", s.step_height);
    d.set("equal_steps", s.equal_steps);
    d.set("subdivisions", s.subdivisions as i64);
    d
}
pub fn from_dict(d: &VarDictionary, base: ShapeSpec) -> ShapeSpec {
    let mut s = base;
    let get = |k: &str| d.get(k);
    if let Some(v) = get("kind").and_then(|v| v.try_to::<i64>().ok()) {
        s.sort = ShapeKind::from_i64(v);
    }
    if let Some(v) = get("size").and_then(|v| v.try_to::<Vector3>().ok()) {
        s.size = v3(v.x, v.y, v.z);
    }
    if let Some(v) = get("segments").and_then(|v| v.try_to::<i64>().ok()) {
        s.stretches = v.clamp(1, 200) as u32;
    }
    if let Some(v) = get("steps").and_then(|v| v.try_to::<i64>().ok()) {
        s.steps = v.clamp(1, 128) as u32;
    }
    if let Some(v) = get("thickness").and_then(|v| v.try_to::<f64>().ok()) {
        s.thickness = v as f32;
    }
    if let Some(v) = get("smooth").and_then(|v| v.try_to::<bool>().ok()) {
        s.glide = v;
    }
    if let Some(v) = get("rings").and_then(|v| v.try_to::<i64>().ok()) {
        s.rings = v.clamp(0, 16) as u32;
    }
    let float = |k: &str| {
        get(k)
            .and_then(|v| v.try_to::<f64>().ok())
            .map(|v| v as f32)
    };
    if let Some(v) = float("arc") {
        s.arc = v.clamp(5.0, 355.0);
    }
    if let Some(v) = float("inner") {
        s.inner = v.clamp(0.0105, 0.9505);
    }
    if let Some(v) = float("opening") {
        s.opening = v.max(0.0105);
    }
    if let Some(v) = float("door_height") {
        s.door_height = v.max(0.0105);
    }
    if let Some(v) = get("arched").and_then(|v| v.try_to::<bool>().ok()) {
        s.arched = v;
    }
    if let Some(v) = get("profile").and_then(|v| v.try_to::<i64>().ok()) {
        s.silhouette = v.clamp(0, 5) as u8;
    }
    if let Some(v) = get("side").and_then(|v| v.try_to::<i64>().ok()) {
        s.flank = v.clamp(0, 2) as u8;
    }
    let int = |k: &str| {
        get(k).and_then(|v| {
            v.try_to::<i64>()
                .ok()
                .or_else(|| v.try_to::<f64>().ok().map(|f| f.round() as i64))
        })
    };
    let boolean = |k: &str| get(k).and_then(|v| v.try_to::<bool>().ok());
    if let Some(v) = int("segments_z") {
        s.segments_z = v.clamp(0, 64) as u32;
    }
    if let Some(v) = int("height_cuts") {
        s.height_cuts = v.clamp(0, 255) as u32;
    }
    if let Some(v) = float("torus_arc") {
        s.torus_arc = v.clamp(0.0, 360.0);
    }
    if let Some(v) = float("tube_arc") {
        s.tube_arc = v.clamp(0.0, 360.0);
    }
    if let Some(v) = boolean("arch_band") {
        s.arch_band = v;
    }
    if let Some(v) = float("arch_degrees") {
        s.arch_degrees = v.clamp(1.0, 360.0);
    }
    if let Some(v) = boolean("end_caps") {
        s.end_caps = v;
    }
    if let Some(v) = float("step_height") {
        s.step_height = if v > 0.0 { v.max(0.0105) } else { 0.0 };
    }
    if let Some(v) = boolean("equal_steps") {
        s.equal_steps = v;
    }
    if let Some(v) = int("subdivisions") {
        s.subdivisions = v.clamp(1, 5) as u32;
    }
    s
}
