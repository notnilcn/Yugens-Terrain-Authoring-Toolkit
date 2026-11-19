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
pub mod mesh_ops;
pub mod model;
pub mod ops;
pub mod paint;
pub mod pick;
pub mod poly2tri;
pub mod prims;
pub mod shapes;
pub mod snap;
pub mod topo;
pub mod tri;
pub mod trim_walls;
pub mod uv;
pub mod uv_edit;
pub mod uv_lock;
pub mod xform;
// Xogot Modeler ports (docs/port/06-xogot.md).
pub mod bezier;
pub mod xg_bevel;
pub mod xg_export;
pub mod xg_ops;
pub mod xg_paint;
pub mod xg_shapes;
pub mod xg_uv;
pub mod xogot_meta;

pub use model::{BMesh, Edge, Face, UvParams, edge};
pub use shapes::{ShapeKind, ShapeSpec};
