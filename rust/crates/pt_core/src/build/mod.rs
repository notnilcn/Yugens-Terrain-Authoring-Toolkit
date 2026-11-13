//! Level-builder core: an editable n-gon mesh model for blocking out buildings and props in
//! the editor (shape generators, topology operations, auto UVs, picking, serialization).
//! The Godot side lives in `mst::builder`.

pub mod bevel;
pub mod blob;
pub mod compile;
pub mod convert;
pub mod cut;
pub mod edit_ops;
pub mod footprint;
pub mod lightmap;
pub mod loops;
pub mod model;
pub mod ops;
pub mod paint;
pub mod pick;
pub mod prims;
pub mod shapes;
pub mod snap;
pub mod topo;
pub mod tri;
pub mod trim_walls;
pub mod ub_ops;
pub mod uv;
pub mod uv_edit;
pub mod uv_lock;
pub mod xform;

pub use model::{BMesh, Edge, Face, UvParams, edge};
pub use shapes::{ShapeKind, ShapeSpec};
