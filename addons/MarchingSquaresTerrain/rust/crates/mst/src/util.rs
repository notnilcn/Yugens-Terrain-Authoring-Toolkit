//! Small helpers shared by the nodes: type conversions, mesh upload, shader loading.

use godot::classes::mesh::{ArrayCustomFormat, ArrayFormat, ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, Engine, Node, Shader, ShaderMaterial};
use godot::prelude::*;
use pt_core::math::{IVec2, Vec2 as PVec2, Vec3 as PVec3, iv2, v2};
use pt_core::mesh::MeshArrays;

pub const SHADER_DIR: &str = "res://addons/MarchingSquaresTerrain/shaders/";

pub fn is_editor() -> bool {
    Engine::singleton().is_editor_hint()
}

pub fn to_iv(v: Vector2i) -> IVec2 {
    iv2(v.x, v.y)
}
pub fn from_iv(v: IVec2) -> Vector2i {
    Vector2i::new(v.x, v.y)
}
pub fn to_v2(v: Vector2) -> PVec2 {
    v2(v.x, v.y)
}
pub fn from_v3(v: PVec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}

/// Packs the per-object flags read by `pixel_common.gdshaderinc` (`pt_flags`).
/// Layout: `pt_core::meta::object_flags` (palette and dither on).
pub fn object_flags(pixel_size: i32, outline: bool, outline_id: i32, snap: bool) -> i32 {
    pt_core::meta::object_flags(pixel_size, outline, outline_id, snap, true, true)
}

/// [`object_flags`] for a writer whose pixel size may exceed 4: reports the size of `owner`'s
/// slot to the resolve pass (`render::object::note_pixel_size`), which only tests the block
/// sizes in use.
pub fn object_flags_for(owner: InstanceId, slot: u8, pixel_size: i32, outline: bool, outline_id: i32, snap: bool) -> i32 {
    crate::render::object::note_pixel_size(owner, slot, pixel_size);
    object_flags(pixel_size, outline, outline_id, snap)
}

pub fn load_shader(name: &str) -> Option<Gd<Shader>> {
    try_load::<Shader>(&format!("{SHADER_DIR}{name}")).ok()
}

pub fn shader_material(name: &str) -> Gd<ShaderMaterial> {
    let mut m = ShaderMaterial::new_gd();
    if let Some(s) = load_shader(name) {
        m.set_shader(&s);
    } else {
        godot_error!("Marching Squares Terrain: missing shader {SHADER_DIR}{name}");
    }
    m
}

/// Uploads indexed mesh arrays (positions, normals, UV, UV2, CUSTOM0/1 as RGBA float).
pub fn build_array_mesh(m: &MeshArrays) -> Option<Gd<ArrayMesh>> {
    build_array_mesh_concat(&[m])
}

/// One single-surface `ArrayMesh` of several `MeshArrays` (indices offset per part): region
/// batching draws many chunks' meshes with one draw call per pass.
pub fn build_array_mesh_concat(parts: &[&MeshArrays]) -> Option<Gd<ArrayMesh>> {
    let verts: usize = parts.iter().map(|m| m.positions.len()).sum();
    let tris: usize = parts.iter().map(|m| m.indices.len()).sum();
    if tris == 0 {
        return None;
    }
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let (mut pos, mut nrm) = (Vec::with_capacity(verts), Vec::with_capacity(verts));
    let (mut uv, mut uv2) = (Vec::with_capacity(verts), Vec::with_capacity(verts));
    let (mut c0, mut c1) = (Vec::with_capacity(verts * 4), Vec::with_capacity(verts * 4));
    let mut idx = Vec::with_capacity(tris);
    for m in parts {
        let base = pos.len() as i32;
        let n = m.positions.len();
        pos.extend(m.positions.iter().map(|p| Vector3::new(p[0], p[1], p[2])));
        // Parts with missing attributes (old baked meshes) are padded, so the arrays stay aligned.
        let pad3 = |out: &mut Vec<Vector3>, src: &[[f32; 3]]| {
            out.extend(src.iter().take(n).map(|p| Vector3::new(p[0], p[1], p[2])));
            out.resize(base as usize + n, Vector3::UP);
        };
        pad3(&mut nrm, &m.normals);
        for (out, src) in [(&mut uv, &m.uv), (&mut uv2, &m.uv2)] {
            out.extend(src.iter().take(n).map(|p| Vector2::new(p[0], p[1])));
            out.resize(base as usize + n, Vector2::ZERO);
        }
        for (out, src) in [(&mut c0, &m.custom0), (&mut c1, &m.custom1)] {
            out.extend(src.iter().take(n).flat_map(|c| c.iter().copied()));
            out.resize((base as usize + n) * 4, 0.0);
        }
        idx.extend(m.indices.iter().map(|i| i + base));
    }
    arrays.set(ArrayType::VERTEX.ord() as usize, &PackedVector3Array::from(pos.as_slice()).to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &PackedVector3Array::from(nrm.as_slice()).to_variant());
    arrays.set(ArrayType::TEX_UV.ord() as usize, &PackedVector2Array::from(uv.as_slice()).to_variant());
    arrays.set(ArrayType::TEX_UV2.ord() as usize, &PackedVector2Array::from(uv2.as_slice()).to_variant());
    arrays.set(ArrayType::CUSTOM0.ord() as usize, &PackedFloat32Array::from(c0.as_slice()).to_variant());
    arrays.set(ArrayType::CUSTOM1.ord() as usize, &PackedFloat32Array::from(c1.as_slice()).to_variant());
    arrays.set(ArrayType::INDEX.ord() as usize, &PackedInt32Array::from(idx.as_slice()).to_variant());
    let rgba = ArrayCustomFormat::RGBA_FLOAT.ord() as u64;
    let flags = (rgba << ArrayFormat::CUSTOM0_SHIFT.ord()) | (rgba << ArrayFormat::CUSTOM1_SHIFT.ord());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays_ex(PrimitiveType::TRIANGLES, &arrays)
        .flags(ArrayFormat::from_ord(flags))
        .done();
    Some(mesh)
}

/// Surface 0 of a mesh as `MeshArrays` (baked chunk meshes join their region's mesh). Only
/// RGBA-float custom channels are read back; other formats are left zero.
pub fn mesh_arrays_of(mesh: &Gd<godot::classes::Mesh>) -> Option<MeshArrays> {
    if mesh.get_surface_count() < 1 {
        return None;
    }
    let a = mesh.surface_get_arrays(0);
    let get = |t: ArrayType| a.get(t.ord() as usize).unwrap_or_default();
    let pos = get(ArrayType::VERTEX).try_to::<PackedVector3Array>().ok()?;
    let mut m = MeshArrays { positions: pos.as_slice().iter().map(|v| [v.x, v.y, v.z]).collect(), ..Default::default() };
    if let Ok(n) = get(ArrayType::NORMAL).try_to::<PackedVector3Array>() {
        m.normals = n.as_slice().iter().map(|v| [v.x, v.y, v.z]).collect();
    }
    if let Ok(v) = get(ArrayType::TEX_UV).try_to::<PackedVector2Array>() {
        m.uv = v.as_slice().iter().map(|v| [v.x, v.y]).collect();
    }
    if let Ok(v) = get(ArrayType::TEX_UV2).try_to::<PackedVector2Array>() {
        m.uv2 = v.as_slice().iter().map(|v| [v.x, v.y]).collect();
    }
    for (t, out) in [(ArrayType::CUSTOM0, &mut m.custom0), (ArrayType::CUSTOM1, &mut m.custom1)] {
        if let Ok(v) = get(t).try_to::<PackedFloat32Array>()
            && v.len() == m.positions.len() * 4
        {
            *out = v.as_slice().chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        }
    }
    m.indices = match get(ArrayType::INDEX).try_to::<PackedInt32Array>() {
        Ok(i) if !i.is_empty() => i.as_slice().to_vec(),
        _ => (0..m.positions.len() as i32).collect(),
    };
    Some(m)
}

/// Adds `child` as an internal (unsaved, hidden from the scene dock) child of `parent`.
pub fn add_internal(parent: &mut Gd<impl Inherits<Node>>, child: &Gd<impl Inherits<Node>>) {
    use godot::classes::node::InternalMode;
    parent.upcast_mut::<Node>().add_child_ex(child).internal(InternalMode::FRONT).done();
}

/// An exported node reference as the inspector should see it: `None` once the node was freed
/// (deleted in the editor). godot-rust panics when a freed object is returned to the engine,
/// so exported node fields use this in their getter (`#[var(get = ...)]`).
pub fn live<T: GodotClass>(n: &Option<Gd<T>>) -> Option<Gd<T>> {
    n.as_ref().filter(|n| n.is_instance_valid()).cloned()
}

/// An exported node reference with the documented fallback "the first sibling of type `T`"
/// (effects' `sun` / `environment` slots). The explicit reference always wins; the fallback is
/// resolved once and cached, and only looked up again when the cached node left the parent or
/// the parent's child count changed after a miss, so per-frame callers never scan siblings.
pub struct SiblingFallback<T: GodotClass> {
    cached: Option<Gd<T>>,
    /// Parent child count at the last miss (`None` = look again).
    miss_at: Option<i32>,
}

impl<T: GodotClass> Default for SiblingFallback<T> {
    fn default() -> Self {
        Self { cached: None, miss_at: None }
    }
}

impl<T: GodotClass + Inherits<Node>> SiblingFallback<T> {
    pub fn resolve(&mut self, owner: &Gd<Node>, explicit: &Option<Gd<T>>) -> Option<Gd<T>> {
        if let Some(n) = live(explicit) {
            return Some(n);
        }
        let parent = owner.get_parent()?;
        if let Some(c) = self.cached.as_ref().filter(|c| c.is_instance_valid())
            && c.clone().upcast::<Node>().get_parent().as_ref() == Some(&parent)
        {
            return Some(c.clone());
        }
        let count = parent.get_child_count();
        if self.cached.is_none() && self.miss_at == Some(count) {
            return None;
        }
        self.cached = parent.get_children().iter_shared().find_map(|n| n.try_cast::<T>().ok());
        self.miss_at = if self.cached.is_none() { Some(count) } else { None };
        self.cached.clone()
    }

    /// Forget the cached fallback (the owner left the tree).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A repeating `Timer` child of `owner` calling `owner.<method>()` every `seconds` (editor
/// plugins' periodic refreshes, instead of counting time in `_process`). Configured before it
/// enters the tree; `autostart` starts it as it does.
pub fn add_repeating_timer(owner: &mut Gd<impl Inherits<Node>>, seconds: f64, method: &str, autostart: bool) -> Gd<godot::classes::Timer> {
    let mut t = godot::classes::Timer::new_alloc();
    t.set_name(method);
    t.set_wait_time(seconds);
    t.set_one_shot(false);
    t.set_autostart(autostart);
    let cb = Callable::from_object_method(&owner.clone().upcast::<Node>(), method);
    t.connect("timeout", &cb);
    owner.upcast_mut::<Node>().add_child(&t);
    t
}

/// Frees a node if it is still valid. It leaves its parent at once (`queue_free` alone keeps
/// it drawn until the end of the frame, next to its replacement, and makes the replacement
/// get an auto-generated name such as `@Node3D@12` instead of e.g. `Chunk (1, 2)`).
pub fn free_node(n: Option<Gd<impl Inherits<Node>>>) {
    if let Some(n) = n {
        let mut n = n.upcast::<Node>();
        if n.is_instance_valid() {
            if let Some(mut p) = n.get_parent() {
                p.remove_child(&n);
            }
            n.queue_free();
        }
    }
}

/// Spaces the CamelCase items of an enum hint ("HexRings,FlatWalls" -> "Hex Rings,Flat
/// Walls") so the inspector names enum values like the editor strips do. Call it from
/// `on_validate_property`; stored values are unchanged.
pub fn space_enum_hint(p: &mut godot::register::info::PropertyInfo) {
    if p.hint_info.hint != godot::register::info::PropertyHint::ENUM {
        return;
    }
    let src = p.hint_info.hint_string.to_string();
    let spaced = space_camel_list(&src);
    if spaced != src {
        p.hint_info.hint_string = spaced.as_str().into();
    }
}

fn space_camel_list(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 8);
    let mut prev: Option<char> = None;
    for c in src.chars() {
        if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase()) {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn spaces_camel_case_enum_items() {
        assert_eq!(super::space_camel_list("Square,Triangle,Hex,HexRings"), "Square,Triangle,Hex,Hex Rings");
        assert_eq!(super::space_camel_list("Anchor:0,LowRes:1"), "Anchor:0,Low Res:1");
    }
}
