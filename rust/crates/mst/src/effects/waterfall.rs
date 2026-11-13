//! Waterfall / raised-water mesh for `PixelWater` (see `pt_core::waterfall`): pools on raised
//! water cells and animated fall sheets, base foam and spray where the water spills over an
//! edge of a `MarchingSquaresTerrain` cell grid. Drawn with `waterfall.gdshader`.

use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, Node};
use godot::prelude::*;
use pt_core::waterfall::{Fall, FallParams, WaterMap, build_water_mesh, find_falls};

use crate::terrain::node::MarchingSquaresTerrain;

/// The raised water of one terrain, ready to mesh.
pub struct Falls {
    pub falls: Vec<Fall>,
    pub mesh: Option<Gd<ArrayMesh>>,
}

/// Raised water cells from `(q, r, surface y)` triples (terrain-local Y).
pub fn water_map(cells: &PackedVector3Array) -> WaterMap {
    cells.as_slice().iter().map(|v| (pt_core::math::iv2(v.x.round() as i32, v.y.round() as i32), v.z)).collect()
}

/// Finds the falls of `terrain` (a `MarchingSquaresTerrain`) and builds their mesh. `sea` and
/// `min_drop` are in terrain-local units.
pub fn build(terrain: &Gd<Node>, water: &WaterMap, sea: Option<f32>, min_drop: f32) -> Option<Falls> {
    let t = terrain.clone().try_cast::<MarchingSquaresTerrain>().ok()?;
    let tb = t.bind();
    let store = tb.store();
    if store.config.grid == pt_core::grid::GridType::Square {
        return None;
    }
    let p = FallParams { min_drop, sea, ..Default::default() };
    let falls = find_falls(store, water, &p);
    let verts = build_water_mesh(store, water, &falls, &p);
    if verts.is_empty() {
        return Some(Falls { falls, mesh: None });
    }
    let v3 = |p: pt_core::math::Vec3| Vector3::new(p.x, p.y, p.z);
    let pos: PackedVector3Array = verts.iter().map(|w| v3(w.pos)).collect();
    let nrm: PackedVector3Array = verts.iter().map(|w| v3(w.normal)).collect();
    let uv: PackedVector2Array = verts.iter().map(|w| Vector2::new(w.uv.x, w.uv.y)).collect();
    let uv2: PackedVector2Array = verts.iter().map(|w| Vector2::new(w.uv2.x, w.uv2.y)).collect();
    // COLOR.r = part kind / 4 (0 pool, 1 fall, 2 foam, 3 spray, 4 landing basin).
    let col: PackedColorArray = verts.iter().map(|w| Color::from_rgba(w.kind / 4.0, 0.0, 0.0, 1.0)).collect();
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::TEX_UV.ord() as usize, &uv.to_variant());
    arrays.set(ArrayType::TEX_UV2.ord() as usize, &uv2.to_variant());
    arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    // Spray billboards grow in the vertex shader: pad the bounds so they are not culled.
    let aabb = mesh.get_aabb().grow(1.5);
    mesh.set_custom_aabb(aabb);
    Some(Falls { falls, mesh: Some(mesh) })
}
