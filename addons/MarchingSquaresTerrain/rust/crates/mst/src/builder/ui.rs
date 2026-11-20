//! Builder UI, in the MarchingSquaresTerrain editor style: a left icon toolbar (modes, tools,
//! the operations of the current mode, snapping, import/export), a bottom attribute strip
//! (shape parameters, color, grid step, operation amounts) and a right material palette with
//! the face UV controls.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, ButtonGroup, CheckBox, ColorPickerButton, Control, EditorResourcePicker, GridContainer,
    HBoxContainer, HFlowContainer, HSeparator, Label, Material, MenuButton, OptionButton,
    PopupMenu, ScrollContainer, SpinBox, Texture2D, VBoxContainer, VSeparator,
};
use godot::prelude::*;
use pt_core::build::{ShapeKind, ShapeSpec};

use crate::editor::ui_kit;

const ICONS: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Object,
    Face,
    Edge,
    Vertex,
}

impl Mode {
    pub fn moniker(self) -> &'static str {
        match self {
            Mode::Object => "Object",
            Mode::Face => "Face",
            Mode::Edge => "Edge",
            Mode::Vertex => "Vertex",
        }
    }
    pub fn bit(self) -> u8 {
        match self {
            Mode::Object => O,
            Mode::Face => F,
            Mode::Edge => E,
            Mode::Vertex => V,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    /// Select elements and drag the move handles (Shift+drag a face/edge handle extrudes).
    #[default]
    Select,
    /// Click-drag a footprint, move up for the height, click to finish (click alone stamps).
    Shape,
    /// Click / drag over faces to paint the current material slot and color.
    Paint,
    /// Click a terrain cell to seat the selected mesh on it.
    Place,
    /// Click a face, click points on it, close the polygon to cut it in as a sub-face.
    Knife,
    /// Brush / fill per-corner vertex colors (the vertex colour brush).
    VertexPaint,
    /// Bezier Plane / Block: click knots, drag knots / tangents, Enter creates.
    Bezier,
}

/// What the element handles do in the Select tool (move / rotate / scale gizmo modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum XformMode {
    #[default]
    Move,
    Rotate,
    Scale,
}

impl Tool {
    pub fn moniker(self) -> &'static str {
        match self {
            Tool::Select => "Select / Move",
            Tool::Shape => "Draw shape",
            Tool::Paint => "Paint material",
            Tool::Place => "Place on cell",
            Tool::Knife => "Knife",
            Tool::VertexPaint => "Vertex paint",
            Tool::Bezier => "Bezier",
        }
    }
}

/// Toolbar buttons (bound to `upon_deed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Action {
    ModeObject = 0,
    ModeFace,
    ModeEdge,
    ModeVertex,
    ToolSelect,
    ToolShape,
    ToolPaint,
    ToolPlace,
    Extrude,
    Inset,
    Bevel,
    Subdivide,
    Merge,
    Connect,
    Fill,
    Detach,
    Duplicate,
    Rotate,
    Flip,
    Delete,
    Weld,
    Grow,
    Coplanar,
    Similar,
    Material,
    SnapGrid,
    SnapCell,
    ExportObj,
    ExportGltf,
    Convert,
    GizmoMove,
    GizmoRotate,
    GizmoScale,
    SoftSelect,
    SnapVertex,
    InsertLoop,
    Bridge,
    SelectLoop,
    Shrink,
    Holes,
    Mirror,
    ToolKnife,
    ShapeParams,
    BoolUnion,
    BoolSubtract,
    BoolIntersect,
    UvEditor,
    ConformNormals,
    Preferences,
    ToolVertexPaint,
    BlendMaterial,
    InvertAlpha,
    UvLock,
    BakeMesh,
    SubdivideEdges,
    ConnectEdges,
    FlipEdge,
    SplitVerts,
    Triangulate,
    MergeObjects,
    CenterPivot,
    PivotToSelection,
    FreezeTransform,
    Collapse,
    InsertVertex,
    TrimWalls,
    SurfaceDraw,
    SetCollider,
    SetTrigger,
    /// Appended last: `debug_action` indices of the earlier actions stay stable.
    XRay,
    // Extended actions, appended: ids 70..
    Dissolve,
    OffsetElements,
    InvertSelection,
    SelectByColor,
    SelectBySmoothing,
    ToolBezier,
    ReusablePart,
    IndividualOrigins,
    ExportStl,
    ExportPly,
    ExportOptions,
    // Alternate UI layout (appended: ids 81..). The first three and Rotate −90° are menu-only
    // (`MENU_ACTIONS`); the others have MST toolbar buttons too.
    SelectAll,
    ClearSelection,
    SelectRing,
    GenerateUv2,
    RotateNeg,
    PaintColour,
    // I5 (appended: id 87): menu-only.
    /// Export the compiled mesh as a Godot mesh resource (`.res` / `.tres`).
    ExportRes,
}

pub const O: u8 = 1;
pub const F: u8 = 2;
pub const E: u8 = 4;
pub const V: u8 = 8;
pub const ALL: u8 = O | F | E | V;

/// (action, icon, tooltip, radio group: 1 = modes, 2 = tools, 3 = toggle, 0 = push, modes shown)
const BUTTONS: &[(Action, &str, &str, u8, u8)] = &[
    (
        Action::ModeObject,
        "mode_object",
        "Object mode [1]: whole meshes (use Godot's gizmo to move them)",
        1,
        ALL,
    ),
    (
        Action::ModeFace,
        "mode_face",
        "Face mode [2]: click faces (Shift adds), drag on empty space for a box",
        1,
        ALL,
    ),
    (
        Action::ModeEdge,
        "mode_edge",
        "Edge mode [3]: pick edges",
        1,
        ALL,
    ),
    (
        Action::ModeVertex,
        "mode_vertex",
        "Vertex mode [4]: pick vertices",
        1,
        ALL,
    ),
    (
        Action::ToolSelect,
        "move",
        "Select / move [W]: drag the axis handles (centre = view plane)\nShift+drag a handle extrudes faces / edges first",
        2,
        ALL,
    ),
    (
        Action::ToolShape,
        "shape",
        "Draw shape [C]: drag the footprint on the grid / terrain cells, move up for the height, click to finish\nA plain click stamps the shape at its current size\nHold Shift while you click to finish (or stamp) to append the shape to the selected mesh instead of adding a new one\n(Shift+C is the Bezier tool, not append)\nRight-click / Esc cancels",
        2,
        ALL,
    ),
    (
        Action::ToolPaint,
        "paint",
        "Paint [B]: click or drag over faces to give them the current material slot and color\nShift+click fills the coplanar region, Ctrl+click picks the slot of a face",
        2,
        ALL,
    ),
    (
        Action::ToolVertexPaint,
        "vertex_paint",
        "Vertex paint [5]: brush per-corner vertex colors (Radius, Strength, Softness) or Fill whole faces\nChannel RGB / Alpha paints only the color or only the blend mask of a blend material\nShift+click picks the color under a corner, Esc leaves the tool",
        2,
        ALL,
    ),
    (
        Action::ToolPlace,
        "place_cell",
        "Place on cell [P]: click a terrain cell to seat the selected mesh on it",
        2,
        O,
    ),
    (
        Action::ToolBezier,
        "shape_bezier",
        "Bezier plane / block [Shift+C]: click the ground or a surface to add knots, click the curve to insert one\nDrag knots and their tangent handles; G / X / Y / Z move the selected knot (X / Y / Z: world, local, off; Shift: plane; type a distance)\nBackspace / Del removes the selected (or last) knot, Enter creates (Block: move for the height, Enter / click again)\nEsc / right-click cancels",
        2,
        ALL,
    ),
    (
        Action::ToolKnife,
        "cut",
        "Knife [K]: click a face, then click points on it (snaps to its vertices / edges and the grid)\nClick the first point, Enter or double-click to cut the polygon in as its own face\nBackspace removes the last point, Esc / right-click cancels",
        2,
        F | E | V,
    ),
    (
        Action::GizmoMove,
        "move",
        "Move handles [Shift+W]: drag an axis arrow, the centre square moves in the view plane\nHold V while dragging to snap onto the vertex under the cursor",
        7,
        F | E | V,
    ),
    (
        Action::GizmoRotate,
        "gizmo_rotate",
        "Rotate handles [Shift+E]: drag a ring to turn the selection about its centre\n(snaps to the Rot° step while grid snapping is on)",
        7,
        F | E | V,
    ),
    (
        Action::GizmoScale,
        "gizmo_scale",
        "Scale handles [Shift+R]: drag an axis handle, or the centre square for uniform scale\n(snaps to 0.1 steps while grid snapping is on)",
        7,
        F | E | V,
    ),
    (
        Action::SoftSelect,
        "prop_smooth",
        "Soft selection [O]: moves, rotations and scales pull nearby vertices along\nwithin Radius, weighted by the Falloff curve",
        3,
        F | E | V,
    ),
    (
        Action::SnapVertex,
        "snap",
        "Vertex snap: moves always snap onto the vertex under the cursor\n(hold V for a one-off snap while this is off)",
        3,
        F | E | V,
    ),
    (
        Action::IndividualOrigins,
        "pivot_individual",
        "Individual origins: rotate and scale handles turn / scale every connected piece\nof the selection about its own centre (off: about the centre of the whole selection)",
        3,
        F | E | V,
    ),
    (
        Action::UvLock,
        "uv_lock",
        "UV lock: textures stay put while faces are moved, rotated or scaled\nManual UVs ride along rigid moves and are refitted when faces stretch;\nautomatic UVs keep their projection when a face tilts",
        3,
        F | E | V,
    ),
    (
        Action::Extrude,
        "extrude",
        "Extrude [E]: faces along their normals, open edges outwards, by Dist\nFaces: Extrude method in the strip (face normal / vertex normal / individual); edges: As group",
        0,
        F | E,
    ),
    (
        Action::Inset,
        "inset",
        "Inset [I]: shrink the selected faces into their own faces by Inset\nInset by: Fraction (towards the centre), Width or Equal (a rim Inset metres wide, mitred corners)",
        0,
        F,
    ),
    (
        Action::Bevel,
        "bevel",
        "Bevel [Ctrl+B]: the selected edges (Edge mode) or corners (Vertex mode) by Bevel over Segs segments\nAmount type Offset / Width / Depth / Percent, Profile (0.5 round), Clamp overlap, Harden normals;\nconnected edge rims inset their corners. 1 segment chamfers, more round the edge",
        0,
        E | V,
    ),
    (
        Action::InsertLoop,
        "insert_loop",
        "Insert edge loop [Ctrl+R]: Cuts loops across the quad ring through the selected edge",
        0,
        E,
    ),
    (
        Action::Bridge,
        "bridge",
        "Bridge: join two open (boundary) edges, or two equal open edge runs, with faces",
        0,
        E,
    ),
    (
        Action::Subdivide,
        "subdivide",
        "Subdivide: split the selected faces into quads, Subdiv pieces along every edge\n(concave faces use their kernel centre); Object mode: every face of the mesh",
        0,
        O | F,
    ),
    (
        Action::SubdivideEdges,
        "subdiv_edges",
        "Subdivide edges: split each selected edge into Cuts+1 equal pieces\n(manual UVs and vertex paint are interpolated along the edge)",
        0,
        E,
    ),
    (
        Action::Triangulate,
        "triangulate",
        "Triangulate [Ctrl+T]: split the selected faces into triangles\nObject mode: every face of the mesh",
        0,
        O | F,
    ),
    (
        Action::FlipEdge,
        "flip_edge",
        "Flip face edge: turn the triangulation diagonal of the selected quads\n(rotates each face's corner loop by one)",
        0,
        F,
    ),
    (
        Action::Merge,
        "merge",
        "Merge [M]: faces into one n-gon / vertices into one",
        0,
        F | V,
    ),
    (
        Action::Collapse,
        "collapse",
        "Collapse vertices: merge the selected vertices into one at their centre (To first: onto the first picked one)\n(refused when it would fold an edge onto a third face; paint and UVs stay on their corners)",
        0,
        V,
    ),
    (
        Action::InsertVertex,
        "insert_vert",
        "Insert vertex [Shift+V]: click an edge to split it at the cursor\n(stays on until toggled off; Esc leaves it)",
        3,
        E,
    ),
    (
        Action::TrimWalls,
        "shape_trim_walls",
        "Trim walls: sweep the trim Profile (T depth x T height) along the base\n(or under the top) of the selected wall faces",
        0,
        F,
    ),
    (
        Action::Dissolve,
        "dissolve",
        "Dissolve: remove the selected elements and merge the faces around them into n-gons\nFaces: merge them into one; edges: join the two faces of each edge; vertices: merge the faces around them",
        0,
        F | E | V,
    ),
    (
        Action::OffsetElements,
        "offset",
        "Offset elements: move the selection by the Offset x / y / z of the strip\nmeasured in Space (Global world axes, Local node axes, Normal: y along the selection normal; Per face moves each face along its own)",
        0,
        F | E | V,
    ),
    (
        Action::Connect,
        "connect",
        "Connect [J]: split faces between selected vertices",
        0,
        V,
    ),
    (
        Action::ConnectEdges,
        "connect_edges",
        "Connect edges [J]: split each face that has two selected edges\nwith a new edge between their midpoints",
        0,
        E,
    ),
    (
        Action::SplitVerts,
        "split",
        "Split vertices: give every face touching a selected vertex its own copy\n(the faces can then be moved apart)",
        0,
        V,
    ),
    (
        Action::Fill,
        "fill",
        "Fill hole [F]: close open boundaries through the selection\nEntire hole off (strip): only the selected run of border vertices, closed by its chord\nThe key fills only in Edge / Vertex mode with a selection; otherwise F frames the selection (Godot)",
        0,
        E | V,
    ),
    (
        Action::Detach,
        "detach",
        "Detach: split the selected faces from their neighbours\nTo new object (strip): move them into a new builder mesh instead",
        0,
        F,
    ),
    (
        Action::Duplicate,
        "duplicate",
        "Duplicate [Ctrl+D]: copy the selected faces\nTo new object (strip): copy them into a new builder mesh instead",
        0,
        F,
    ),
    (
        Action::Rotate,
        "rotate",
        "Rotate 90° [R]: turn the selection around Y (Shift+R: -90°)",
        0,
        F | E | V,
    ),
    (
        Action::Flip,
        "flip",
        "Flip normals: of the selected faces (all faces in Object mode)",
        0,
        O | F,
    ),
    (
        Action::Delete,
        "delete",
        "Delete [Del]: remove the selected faces / edges / vertices",
        0,
        F | E | V,
    ),
    (
        Action::Weld,
        "weld",
        "Weld: merge vertices closer than the Weld distance (the selected ones in Vertex mode)\n(Merge [M] joins the selected vertices whatever their distance)",
        0,
        O | V,
    ),
    (
        Action::Grow,
        "grow",
        "Grow selection [G]: add the neighbours of the selection\nGrow° (strip, Face / Edge mode): only neighbours within that angle of the selection",
        0,
        F | E | V,
    ),
    (
        Action::Shrink,
        "shrink",
        "Shrink selection [Shift+G]: drop the border of the selection",
        0,
        F | E | V,
    ),
    (
        Action::SelectLoop,
        "select_loop",
        "Select loop [L]: the edge loop through the selected edges (Alt+L: edge ring)\nFace mode: the quad loop. Double-click an edge / face does the same",
        0,
        F | E,
    ),
    (
        Action::Holes,
        "holes",
        "Select holes: every open (boundary) edge",
        0,
        E,
    ),
    (
        Action::InvertSelection,
        "invert_selection",
        "Invert selection: select everything that is not selected (faces / edges / vertices of the mode)",
        0,
        F | E | V,
    ),
    (
        Action::SelectByColor,
        "select_color",
        "Select by colour: every element sharing a face / vertex-paint colour with the selection",
        0,
        F | E | V,
    ),
    (
        Action::SelectBySmoothing,
        "select_smoothing",
        "Select by smoothing group: every face in the smoothing groups of the selected faces\n(flat faces select the other flat faces)",
        0,
        F,
    ),
    (
        Action::Coplanar,
        "coplanar",
        "Select coplanar: faces in the plane of the selection and connected to it",
        0,
        F,
    ),
    (
        Action::Similar,
        "similar",
        "Select similar: faces with the same material slot",
        0,
        F,
    ),
    (
        Action::Material,
        "material",
        "Assign material: give the selected faces the material slot and color (all faces in Object mode)",
        0,
        O | F,
    ),
    (
        Action::BlendMaterial,
        "blend_material",
        "New blend material: give the selected faces (all in Object mode) a material that blends two layers\nby the painted vertex alpha (layer A starts from their current slot). Paint the mask with Vertex paint, Channel Alpha",
        0,
        O | F,
    ),
    (
        Action::InvertAlpha,
        "invert",
        "Invert alpha: swap the two blend layers on the selected faces (all faces in Object mode)",
        0,
        O | F,
    ),
    (
        Action::SnapGrid,
        "snap_grid",
        "Grid snap: moves and shapes snap to the Grid step ([ / ] halve / double it)",
        3,
        ALL,
    ),
    (
        Action::SnapCell,
        "snap_cell",
        "Cell snap: placement, shapes and vertices snap to terrain cells (centres / corners)",
        3,
        ALL,
    ),
    (
        Action::SurfaceDraw,
        "surface_draw",
        "Draw on surface: shapes start on the builder face under the cursor\nand stand along its normal (walls, poly shapes and floors on any slope)",
        3,
        ALL,
    ),
    (
        Action::XRay,
        "xray",
        "X-ray: box selection and clicked vertices / edges also take elements hidden\nbehind other faces (off: only what the camera sees)",
        3,
        F | E | V,
    ),
    (
        Action::ExportObj,
        "export_obj",
        "Export OBJ: the mesh as Wavefront OBJ (+ MTL)\nExport options: selected faces only, world transforms, vertex colours, handedness, combine meshes",
        0,
        O | F,
    ),
    (
        Action::ExportGltf,
        "export_gltf",
        "Export glTF: the mesh as .gltf / .glb",
        0,
        O,
    ),
    (
        Action::ExportStl,
        "export_stl",
        "Export STL: the mesh as STL (binary or ASCII, see Export options) for 3D printing / other tools",
        0,
        O | F,
    ),
    (
        Action::ExportPly,
        "export_ply",
        "Export PLY: the mesh as ASCII PLY (n-gons, UVs, optional vertex colours)",
        0,
        O | F,
    ),
    (
        Action::ExportOptions,
        "export_options",
        "Export options: selected faces only, apply world transforms, vertex colours, right-handed,\ncombine the selected builder meshes, STL binary / ASCII (used by OBJ, STL and PLY)",
        0,
        O | F,
    ),
    (
        Action::BakeMesh,
        "bake_mesh",
        "Bake to MeshInstance3D: replace the builder mesh by a plain MeshInstance3D (+ static collider)\nwith lightmap UV2 unwrapped at the Lightmap texel of the preferences (undoable)",
        0,
        O,
    ),
    (
        Action::Convert,
        "convert",
        "Convert: turn the selected MeshInstance3D or CSG shape into an editable builder mesh\nOpens the import options (merge triangles / quads / n-gons, materials, smoothing);\nmeshes made by Modeler Modeler keep their original n-gons, slots, smoothing, UVs and colours",
        0,
        O,
    ),
    (
        Action::Mirror,
        "mirror",
        "Mirror: mirror across the mesh's local planes ticked in the strip (X / Y / Z, several at once)\nMirror copy on: adds a mirrored copy per axis (seam welded); off: flips the mesh itself",
        0,
        O,
    ),
    (
        Action::ReusablePart,
        "reusable_part",
        "Create reusable part: save this builder mesh (with its children) as a .tscn scene\nand replace it by an instance of that scene (undoable)",
        0,
        O,
    ),
    (
        Action::ShapeParams,
        "shape_params",
        "Shape parameters: reopen the recipe this mesh was drawn from;\nthe strip's shape fields then rebuild it live (each change one undo step)",
        3,
        O,
    ),
    (
        Action::BoolUnion,
        "bool_union",
        "Boolean union: merge the other selected builder mesh into this one (it is removed)\nCtrl+click it in the Scene dock to add it to the selection",
        0,
        O,
    ),
    (
        Action::BoolSubtract,
        "bool_subtract",
        "Boolean subtract: carve the other selected builder mesh out of this one",
        0,
        O,
    ),
    (
        Action::BoolIntersect,
        "bool_intersect",
        "Boolean intersect: keep only the volume shared with the other selected builder mesh",
        0,
        O,
    ),
    (
        Action::CenterPivot,
        "center_pivot",
        "Center pivot: move the node origin to the centre of the mesh bounds\n(the mesh stays where it is)",
        0,
        O,
    ),
    (
        Action::PivotToSelection,
        "pivot_selection",
        "Pivot to selection: move the node origin to the centre of the selected elements",
        0,
        F | E | V,
    ),
    (
        Action::FreezeTransform,
        "freeze_transform",
        "Freeze transform: bake the node's rotation and scale (and position) into the mesh\nand reset the transform; children stay in place",
        0,
        O,
    ),
    (
        Action::MergeObjects,
        "merge_objects",
        "Merge objects: append the other selected builder meshes into this one (they are removed)\nCtrl+click them in the Scene dock to add them to the selection",
        0,
        O,
    ),
    (
        Action::SetCollider,
        "set_collider",
        "Collider: the mesh becomes an invisible static collider in the game (orange in the editor)\nPress again for a normal mesh",
        3,
        O,
    ),
    (
        Action::SetTrigger,
        "set_trigger",
        "Trigger: the mesh becomes an invisible Area3D trigger volume in the game (blue in the editor)\nPress again for a normal mesh",
        3,
        O,
    ),
    (
        Action::ConformNormals,
        "flip",
        "Conform normals: make the face windings consistent in every connected part\n(the majority orientation of each part wins)",
        0,
        O,
    ),
    (
        Action::SelectRing,
        "select_ring",
        "Select ring [Alt+L]: the edge ring through the selected edges (the rungs of the quad strip)\nFace mode: the faces of the ring",
        0,
        F | E,
    ),
    (
        Action::PaintColour,
        "paint_colour",
        "Paint colour: give the selected faces the current color only (their material slot stays)\nObject mode: every face",
        0,
        O | F,
    ),
    (
        Action::GenerateUv2,
        "lightmap_uv",
        "Generate lightmap UVs: unwrap UV2 for baked lighting at the Lightmap texel of the preferences\n(turns on Live UV2 of the mesh, so the UV2 follows later edits; checks the atlas size first)",
        0,
        O,
    ),
    (
        Action::UvEditor,
        "uv_editor",
        "UV editor [U]: open the UV window of this mesh (manual UVs: move / scale / rotate,\nplanar / box projection, flip, fit, sew, copy / paste, texel density)",
        0,
        ALL,
    ),
    (
        Action::Preferences,
        "tools",
        "Preferences: handle and vertex sizes, overlay colors, default snap steps, shortcuts",
        0,
        ALL,
    ),
];

/// Menu-only actions (Actions menu, context menu, keys): (action, tooltip, modes).
pub const MENU_ACTIONS: &[(Action, &str, u8)] = &[
    (
        Action::SelectAll,
        "Select all [Ctrl+A]: every face / edge / vertex of the mode",
        F | E | V,
    ),
    (
        Action::ClearSelection,
        "Clear selection [Esc]: deselect every element",
        F | E | V,
    ),
    (
        Action::RotateNeg,
        "Rotate -90° [Alt+R]: turn the selection around Y the other way",
        F | E | V,
    ),
    (
        Action::ExportRes,
        "Export Godot mesh: the compiled ArrayMesh with its materials as a .res / .tres resource
(Export options: world transforms, combine the selected builder meshes)",
        O,
    ),
];

/// Every toolbar and menu-only action (tooltip, modes).
fn action_row(a: Action) -> Option<(&'static str, u8)> {
    BUTTONS
        .iter()
        .find(|b| b.0 == a)
        .map(|b| (b.2, b.4))
        .or_else(|| MENU_ACTIONS.iter().find(|m| m.0 == a).map(|m| (m.1, m.2)))
}

/// The toolbar buttons as (action, modes): the MST toolbar's command list.
pub fn toolbar_actions() -> impl Iterator<Item = (Action, u8)> {
    BUTTONS.iter().map(|b| (b.0, b.4))
}

/// Toolbar sections: (header, tooltip). Index = [`section_of`].
const SECTIONS: [(&str, &str); 11] = [
    (
        "Mode",
        "Selection mode: Object [1], Face [2], Edge [3], Vertex [4]",
    ),
    (
        "Tool",
        "Viewport tool: Select / move [W], Draw shape [C], Bezier [Shift+C], Paint [B], Vertex paint [5], Place on cell [P], Knife [K]",
    ),
    (
        "Handles",
        "What the Select tool's handles do: move [Shift+W], rotate [Shift+E], scale [Shift+R]; individual origins, soft selection [O], vertex snap, UV lock",
    ),
    (
        "Edit",
        "Operations on the selected faces / edges / vertices of the current mode",
    ),
    (
        "Select",
        "Grow (by angle), shrink, invert, select loops, holes, coplanar, same-material, same-colour or same-smoothing faces; assign the material\nShift+click / Shift+box: Add, Subtract or Toggle; box: Intersect or Complete (Preferences)",
    ),
    (
        "Snap",
        "Grid ([ / ] halve / double the step), terrain-cell snapping and drawing on builder faces",
    ),
    (
        "Object",
        "Whole-mesh operations: triangulate / subdivide, mirror, pivot, freeze, conform normals, shape recipe, reusable part, collider / trigger",
    ),
    (
        "Combine",
        "Boolean operations with a second selected builder mesh; merge other builder meshes into this one",
    ),
    ("Panels", "UV editor window and builder preferences"),
    (
        "Blend",
        "Blend materials: a new two-layer blend material for the faces, invert the painted mask",
    ),
    (
        "Export",
        "Export OBJ / glTF / STL / PLY (+ options), bake to a plain MeshInstance3D, convert a MeshInstance3D / CSG shape",
    ),
];

fn section_of(a: Action, group: u8) -> usize {
    match a {
        Action::Grow
        | Action::Coplanar
        | Action::Similar
        | Action::Material
        | Action::Shrink
        | Action::SelectLoop
        | Action::Holes => 4,
        Action::InvertSelection | Action::SelectByColor | Action::SelectBySmoothing => 4,
        Action::SelectRing | Action::PaintColour => 4,
        Action::GenerateUv2 => 6,
        Action::ExportObj | Action::ExportGltf | Action::Convert | Action::BakeMesh => 10,
        Action::ExportStl | Action::ExportPly | Action::ExportOptions | Action::ExportRes => 10,
        Action::Mirror
        | Action::ShapeParams
        | Action::ConformNormals
        | Action::SetCollider
        | Action::SetTrigger => 6,
        Action::ReusablePart => 6,
        Action::IndividualOrigins => 2,
        Action::Dissolve | Action::OffsetElements => 3,
        Action::ToolBezier => 1,
        Action::UvEditor | Action::Preferences => 8,
        Action::BoolUnion | Action::BoolSubtract | Action::BoolIntersect | Action::MergeObjects => {
            7
        }
        Action::CenterPivot | Action::PivotToSelection | Action::FreezeTransform => 6,
        Action::SoftSelect | Action::SnapVertex | Action::UvLock => 2,
        Action::SurfaceDraw => 5,
        Action::XRay => 4,
        // Whole-object versions sit with the object tools only in Object mode; the buttons are
        // shared, so they stay in Edit (shown in both).
        Action::BlendMaterial | Action::InvertAlpha => 9,
        _ => match group {
            1 => 0,
            2 => 1,
            7 => 2,
            3 => 5,
            _ => 3,
        },
    }
}

/// Terrain-toolbar tooltip layout: "Title [Shortcut]", a blank line, the details, then the modes
/// the button is shown in.
fn format_tooltip(a: Action, tip: &str, mask: u8) -> String {
    let (first, rest) = tip.split_once('\n').unwrap_or((tip, ""));
    let (title, body) = match first.split_once(": ") {
        Some((t, b)) => (t.to_string(), b.to_string()),
        None => (first.to_string(), String::new()),
    };
    let mut out = bound_title(a, &title);
    let mut details: Vec<String> = Vec::new();
    if !body.is_empty() {
        let mut b = body;
        if let Some(c) = b.get(..1) {
            b = c.to_uppercase() + &b[1..];
        }
        details.push(b);
    }
    details.extend(rest.lines().map(str::to_string));
    if !details.is_empty() {
        out += "\n\n";
        out += &details.join("\n");
    }
    if mask != ALL {
        let modes: Vec<&str> = [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex]
            .iter()
            .filter(|m| mask & m.bit() != 0)
            .map(|m| m.moniker())
            .collect();
        out += &format!(
            "\n\n[{} mode{}]",
            modes.join(" / "),
            if modes.len() == 1 { "" } else { "s" }
        );
    }
    out
}

impl Action {
    pub fn from_i64(v: i64) -> Option<Action> {
        BUTTONS
            .iter()
            .map(|b| b.0)
            .chain(MENU_ACTIONS.iter().map(|m| m.0))
            .find(|a| *a as i64 == v)
    }
    /// Modes the toolbar button (or menu entry) is shown in (`O | F | E | V` bits, see [`Mode::bit`]).
    pub fn mask(self) -> u8 {
        action_row(self).map(|r| r.1).unwrap_or(0)
    }
    /// Menu title: the tooltip's title without its key ("Extrude", "Select loop").
    pub fn heading(self) -> String {
        let t = action_row(self).map(|r| r.0).unwrap_or("");
        let first = t.lines().next().unwrap_or("");
        strip_key(first.split(": ").next().unwrap_or("")).to_string()
    }
}

/// `title` without its trailing ` [key]` part.
fn strip_key(title: &str) -> &str {
    match title.rfind(" [") {
        Some(i) if title.ends_with(']') => &title[..i],
        _ => title,
    }
}

/// A tooltip / hover title with the action's *current* shortcut (Editor Settings › Shortcuts ›
/// Mst Builder) in brackets instead of the hard-coded default. Titles of actions without a
/// registered shortcut keep their text.
fn bound_title(a: Action, title: &str) -> String {
    if super::shortcuts::entry_of_action(a).is_none() {
        return title.to_string();
    }
    let base = strip_key(title);
    match super::shortcuts::action_key_text(a) {
        Some(k) if !k.is_empty() => format!("{base} [{k}]"),
        _ => base.to_string(),
    }
}

/// Palette menu ids (`on_palette_menu`).
pub const PALETTE_LOAD: i64 = 0;
pub const PALETTE_SAVE: i64 = 1;
pub const PALETTE_RESET: i64 = 2;

/// Bottom strip fields (bound to `on_field`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Field {
    SizeX = 0,
    SizeY,
    SizeZ,
    Segments,
    Steps,
    Thickness,
    Rings,
    Slot,
    GridStep,
    Distance,
    Inset,
    Bevel,
    Cuts,
    BevelSegs,
    SoftRadius,
    Falloff,
    RotSnap,
    MirrorAxis,
    Arc,
    Inner,
    Opening,
    DoorHeight,
    Profile,
    Side,
    PaintApply,
    PaintChannel,
    PaintRadius,
    PaintStrength,
    PaintSoftness,
    PaintAlpha,
    InsetMode,
    SubdivSegs,
    GizmoOrient,
    TrimPlace,
    TrimDepth,
    TrimHeight,
    // Extended fields (appended: ids 36..).
    OffsetX,
    OffsetY,
    OffsetZ,
    OffsetSpace,
    OffsetPerFace,
    GrowAngle,
    ExtrudeMethod,
    EdgesGroup,
    InsetIndividual,
    CollapseFirst,
    WeldDist,
    FillEntire,
    MirrorX,
    MirrorY,
    MirrorZ,
    MirrorDup,
    ToNewObject,
    SegmentsZ,
    HeightCuts,
    TorusArc,
    TubeArc,
    ArchBand,
    ArchDegrees,
    EndCaps,
    StepHeight,
    EqualSteps,
    Subdivisions,
    PolyHoles,
    BzKind,
    BzClosed,
    BzSmooth,
    BzWidth,
    BzRows,
    BzColumns,
    BzHeight,
    BzTangent,
    BzRoll,
    // Bevel options (appended: ids 73..).
    BevelAmountType,
    BevelProfile,
    BevelClamp,
    BevelHarden,
    // Smoothing display (I4, appended: ids 77..): tint faces by group, normals and their length.
    SmoothTint,
    ShowNormals,
    NormalLength,
}

impl Field {
    pub const ALL: [Field; 80] = [
        Field::SizeX,
        Field::SizeY,
        Field::SizeZ,
        Field::Segments,
        Field::Steps,
        Field::Thickness,
        Field::Rings,
        Field::Slot,
        Field::GridStep,
        Field::Distance,
        Field::Inset,
        Field::Bevel,
        Field::Cuts,
        Field::BevelSegs,
        Field::SoftRadius,
        Field::Falloff,
        Field::RotSnap,
        Field::MirrorAxis,
        Field::Arc,
        Field::Inner,
        Field::Opening,
        Field::DoorHeight,
        Field::Profile,
        Field::Side,
        Field::PaintApply,
        Field::PaintChannel,
        Field::PaintRadius,
        Field::PaintStrength,
        Field::PaintSoftness,
        Field::PaintAlpha,
        Field::InsetMode,
        Field::SubdivSegs,
        Field::GizmoOrient,
        Field::TrimPlace,
        Field::TrimDepth,
        Field::TrimHeight,
        Field::OffsetX,
        Field::OffsetY,
        Field::OffsetZ,
        Field::OffsetSpace,
        Field::OffsetPerFace,
        Field::GrowAngle,
        Field::ExtrudeMethod,
        Field::EdgesGroup,
        Field::InsetIndividual,
        Field::CollapseFirst,
        Field::WeldDist,
        Field::FillEntire,
        Field::MirrorX,
        Field::MirrorY,
        Field::MirrorZ,
        Field::MirrorDup,
        Field::ToNewObject,
        Field::SegmentsZ,
        Field::HeightCuts,
        Field::TorusArc,
        Field::TubeArc,
        Field::ArchBand,
        Field::ArchDegrees,
        Field::EndCaps,
        Field::StepHeight,
        Field::EqualSteps,
        Field::Subdivisions,
        Field::PolyHoles,
        Field::BzKind,
        Field::BzClosed,
        Field::BzSmooth,
        Field::BzWidth,
        Field::BzRows,
        Field::BzColumns,
        Field::BzHeight,
        Field::BzTangent,
        Field::BzRoll,
        Field::BevelAmountType,
        Field::BevelProfile,
        Field::BevelClamp,
        Field::BevelHarden,
        Field::SmoothTint,
        Field::ShowNormals,
        Field::NormalLength,
    ];
    pub fn from_i64(v: i64) -> Option<Field> {
        Self::ALL.iter().copied().find(|f| *f as i64 == v)
    }
    /// (label, min, max, step, tooltip)
    pub fn portray(self) -> (&'static str, f64, f64, f64, &'static str) {
        match self {
            Field::SizeX => (
                "W",
                0.0505,
                256.0,
                0.0505,
                "Shape width (x). Hex shapes use the terrain cell size.",
            ),
            Field::SizeY => ("H", 0.0505, 256.0, 0.0505, "Shape height (y)"),
            Field::SizeZ => ("D", 0.0505, 256.0, 0.0505, "Shape depth (z)"),
            Field::Segments => ("Seg", 1.0, 200.0, 1.0, "Radial segments / subdivisions"),
            Field::Steps => ("Steps", 1.0, 128.0, 1.0, "Stair steps / sphere rings"),
            Field::Thickness => ("Thick", 0.0105, 64.0, 0.0105, "Wall thickness (arch, pipe)"),
            Field::Rings => ("Rings", 0.0, 16.0, 1.0, "Hex cluster radius in cells"),
            Field::Slot => (
                "Slot",
                0.0,
                63.0,
                1.0,
                "Material slot for new shapes, painting and the Material button",
            ),
            Field::GridStep => (
                "Grid",
                0.0624,
                64.0,
                0.0624,
                "Grid step for snapping ([ / ] halve / double)",
            ),
            Field::Distance => ("Dist", -64.0, 64.0, 0.0505, "Extrude distance"),
            Field::Inset => (
                "Inset",
                0.0105,
                64.0,
                0.0105,
                "Inset amount: fraction towards the face centre (Fraction), or the rim width in metres (Width / Equal)",
            ),
            Field::Bevel => (
                "Bevel",
                0.0015,
                64.0,
                0.0105,
                "Bevel amount, measured as the Amount type says (Percent: 0–100 % of the adjacent edges)",
            ),
            Field::Cuts => (
                "Cuts",
                1.0,
                16.0,
                1.0,
                "Edge subdivision cuts / inserted edge loops",
            ),
            Field::BevelSegs => (
                "Segs",
                1.0,
                32.0,
                1.0,
                "Bevel segments 1–32 (1 = chamfer, more = rounded along the Profile)",
            ),
            Field::SoftRadius => (
                "Radius",
                0.0505,
                64.0,
                0.0505,
                "Soft selection radius (world units)",
            ),
            Field::Falloff => ("Falloff", 0.0, 6.0, 1.0, "Soft selection falloff curve"),
            Field::RotSnap => (
                "Rot°",
                0.0,
                90.0,
                1.0,
                "Rotate handle snap step in degrees (with grid snapping on; 0 = free)",
            ),
            Field::MirrorAxis => (
                "Axis",
                0.0,
                2.0,
                1.0,
                "Mirror plane: the mesh's local X, Y or Z = 0 plane",
            ),
            Field::Arc => ("Arc°", 5.0, 355.0, 1.0, "Curved stairs sweep in degrees"),
            Field::Inner => (
                "Inner",
                0.0105,
                0.9505,
                0.0105,
                "Curved stairs inner radius (fraction of the outer)",
            ),
            Field::Opening => ("Door W", 0.0105, 64.0, 0.0105, "Doorway width"),
            Field::DoorHeight => ("Door H", 0.0105, 64.0, 0.0105, "Doorway height"),
            Field::Profile => ("Profile", 0.0, 5.0, 1.0, "Trim cross-section"),
            Field::Side => ("Side", 0.0, 2.0, 1.0, "Wall side of the drawn line"),
            Field::PaintApply => (
                "Apply",
                0.0,
                1.0,
                1.0,
                "Brush blends vertices toward the color inside the radius; Fill paints whole faces",
            ),
            Field::PaintChannel => (
                "Channel",
                0.0,
                2.0,
                1.0,
                "RGBA paints color and blend mask, RGB only the color, Alpha only the blend mask of a blend material",
            ),
            Field::PaintRadius => (
                "Radius",
                0.0105,
                64.0,
                0.0505,
                "Vertex paint brush radius (world units)",
            ),
            Field::PaintStrength => (
                "Strength",
                0.0,
                1.0,
                0.0505,
                "How strongly one stroke blends toward the paint color",
            ),
            Field::PaintSoftness => (
                "Soft",
                0.0,
                1.0,
                0.0505,
                "Falloff: 0 paints the full radius evenly, 1 fades from the centre",
            ),
            Field::PaintAlpha => (
                "Alpha",
                0.0,
                1.0,
                0.0505,
                "Blend mask value painted in the Alpha channel (0 = layer A, 1 = layer B)",
            ),
            Field::InsetMode => (
                "Inset by",
                0.0,
                2.0,
                1.0,
                "Fraction: corners move towards the centre; Width: a rim Inset metres wide; Equal: mitred corners, the same width everywhere",
            ),
            Field::SubdivSegs => (
                "Subdiv",
                2.0,
                16.0,
                2.0,
                "Subdivide: pieces along every edge (even)",
            ),
            Field::GizmoOrient => (
                "Axes",
                0.0,
                2.0,
                1.0,
                "Handle orientation: Global (world axes), Local (the node's axes), Normal (the selection's average normal)",
            ),
            Field::TrimPlace => (
                "Trim at",
                0.0,
                1.0,
                1.0,
                "Trim walls: along the base of the walls, or hung under their top",
            ),
            Field::TrimDepth => (
                "T depth",
                0.0055,
                4.0,
                0.0055,
                "Trim walls: how far the moulding stands out of the wall",
            ),
            Field::TrimHeight => (
                "T height",
                0.0105,
                4.0,
                0.0105,
                "Trim walls: moulding height",
            ),
            Field::OffsetX => (
                "Off x",
                -64.0,
                64.0,
                0.0505,
                "Offset elements: x of the move (in the chosen Space)",
            ),
            Field::OffsetY => (
                "Off y",
                -64.0,
                64.0,
                0.0505,
                "Offset elements: y of the move (Normal space: along the normal)",
            ),
            Field::OffsetZ => (
                "Off z",
                -64.0,
                64.0,
                0.0505,
                "Offset elements: z of the move (in the chosen Space)",
            ),
            Field::OffsetSpace => (
                "Space",
                0.0,
                2.0,
                1.0,
                "Offset elements: Global (world axes), Local (the node's axes) or Normal (y = the selection's normal)",
            ),
            Field::OffsetPerFace => (
                "Per face",
                0.0,
                1.0,
                1.0,
                "Offset elements in Normal space: every selected face moves along its own normal (shared corners average)",
            ),
            Field::GrowAngle => (
                "Grow°",
                0.0,
                180.0,
                1.0,
                "Grow selection: only take neighbours whose normal is within this angle (0 = every neighbour)",
            ),
            Field::ExtrudeMethod => (
                "Extrude",
                0.0,
                2.0,
                1.0,
                "Extrude faces: Face normal (as one, along the selected faces' normals), Vertex normal (as one, along the averaged vertex normals), Individual (every face along its own normal)",
            ),
            Field::EdgesGroup => (
                "As group",
                0.0,
                1.0,
                1.0,
                "Extrude edges as one connected strip (off: every edge gets its own new face)",
            ),
            Field::InsetIndividual => (
                "Individual",
                0.0,
                1.0,
                1.0,
                "Inset by Width: inset every face on its own (off: connected flat faces are inset as one region)",
            ),
            Field::CollapseFirst => (
                "To first",
                0.0,
                1.0,
                1.0,
                "Collapse vertices onto the first selected vertex instead of their centre",
            ),
            Field::WeldDist => (
                "Weld dist",
                0.0005,
                10.0,
                0.0005,
                "Weld distance: vertices closer than this are merged (Object mode: the whole mesh, Vertex mode: the selected ones)",
            ),
            Field::FillEntire => (
                "Entire hole",
                0.0,
                1.0,
                1.0,
                "Fill hole: close the whole hole through the selection (off: only the selected run of border vertices, closed by its chord)",
            ),
            Field::MirrorX => (
                "Mirror X",
                0.0,
                1.0,
                1.0,
                "Mirror across the mesh's local X = 0 plane",
            ),
            Field::MirrorY => (
                "Mirror Y",
                0.0,
                1.0,
                1.0,
                "Mirror across the mesh's local Y = 0 plane",
            ),
            Field::MirrorZ => (
                "Mirror Z",
                0.0,
                1.0,
                1.0,
                "Mirror across the mesh's local Z = 0 plane",
            ),
            Field::MirrorDup => (
                "Mirror copy",
                0.0,
                1.0,
                1.0,
                "Mirror: add a mirrored copy per axis (seam welded); off flips the mesh itself",
            ),
            Field::ToNewObject => (
                "To new object",
                0.0,
                1.0,
                1.0,
                "Detach / Duplicate: put the faces into a new builder mesh next to this one",
            ),
            Field::SegmentsZ => (
                "Seg z",
                0.0,
                64.0,
                1.0,
                "Plane: subdivisions along z (0 = the same as Seg)",
            ),
            Field::HeightCuts => (
                "H cuts",
                0.0,
                255.0,
                1.0,
                "Cylinder / pipe: horizontal cuts through the side wall",
            ),
            Field::TorusArc => (
                "Ring°",
                0.0,
                360.0,
                5.0,
                "Torus: sweep around the ring in degrees (< 360 opens it)",
            ),
            Field::TubeArc => (
                "Tube°",
                0.0,
                360.0,
                5.0,
                "Torus: sweep around the tube in degrees (< 360 cuts it open)",
            ),
            Field::ArchBand => (
                "Band",
                0.0,
                1.0,
                1.0,
                "Arch: Modeler's curved band (Arch° around, end caps) instead of a wall with a round opening",
            ),
            Field::ArchDegrees => ("Arch°", 1.0, 360.0, 1.0, "Arch band: degrees around"),
            Field::EndCaps => ("Caps", 0.0, 1.0, 1.0, "Arch band: close both ends"),
            Field::StepHeight => (
                "Step H",
                0.0,
                64.0,
                0.0105,
                "Stairs by height: step height in metres (0 = use Steps)",
            ),
            Field::EqualSteps => (
                "Equal",
                0.0,
                1.0,
                1.0,
                "Stairs by height: equal steps (off: Step H steps with a shorter top step)",
            ),
            Field::Subdivisions => ("Subdiv", 1.0, 5.0, 1.0, "Icosphere subdivisions"),
            Field::PolyHoles => (
                "Holes",
                0.0,
                1.0,
                1.0,
                "Poly shape: after closing the outline, click holes (close each on its first point); Enter creates",
            ),
            Field::BzKind => (
                "Bezier",
                0.0,
                1.0,
                1.0,
                "Plane: a flat ribbon along the curve; Block: the ribbon extruded by Height (Enter, then move for the height)",
            ),
            Field::BzClosed => (
                "Closed",
                0.0,
                1.0,
                1.0,
                "Close the curve into a loop (clicking the first knot closes it too)",
            ),
            Field::BzSmooth => (
                "Smooth",
                0.0,
                1.0,
                1.0,
                "Smooth shading along the ribbon; new knots get smooth tangents",
            ),
            Field::BzWidth => ("Width", 0.0105, 64.0, 0.0105, "Ribbon width"),
            Field::BzRows => ("Rows", 1.0, 128.0, 1.0, "Samples per curve segment"),
            Field::BzColumns => ("Cols", 1.0, 128.0, 1.0, "Quads across the ribbon"),
            Field::BzHeight => ("Height", 0.0105, 64.0, 0.0105, "Bezier block height"),
            Field::BzTangent => (
                "Tangent",
                0.0,
                2.0,
                1.0,
                "Selected knot: Free (independent tangents), Aligned (opposite, own lengths), Mirrored (opposite, same length)",
            ),
            Field::BzRoll => (
                "Roll°",
                -360.0,
                360.0,
                1.0,
                "Selected knot: roll of the ribbon around the curve",
            ),
            Field::BevelAmountType => (
                "Amount",
                0.0,
                3.0,
                1.0,
                "Bevel amount type: Offset (distance from the edge / along the edges of a corner), Width (of the new face), Depth (of the cut) or Percent of the adjacent edge",
            ),
            Field::BevelProfile => (
                "B profile",
                0.0,
                1.0,
                0.0505,
                "Bevel profile of rounded bevels: 0.5 circular, lower concave, higher convex / boxy",
            ),
            Field::BevelClamp => (
                "Clamp",
                0.0,
                1.0,
                1.0,
                "Clamp overlap: shrink the amount where it would cross a neighbouring vertex (off: refuse such a bevel)",
            ),
            Field::BevelHarden => (
                "Harden",
                0.0,
                1.0,
                1.0,
                "Harden normals: the new bevel faces get their own smoothing group per connected bevel",
            ),
            Field::SmoothTint => (
                "Tint groups",
                0.0,
                1.0,
                1.0,
                "Tint faces by smoothing group: every group in its chip colour (faceted faces stay clear)",
            ),
            Field::ShowNormals => (
                "Normals",
                0.0,
                1.0,
                1.0,
                "Show normals: a line along the shading normal at every face corner",
            ),
            Field::NormalLength => (
                "Length",
                0.0,
                10.0,
                0.0105,
                "Length of the normal lines (mesh units)",
            ),
        }
    }
    /// Drop-down choices (fields shown as an OptionButton instead of a SpinBox).
    pub fn choices(self) -> Option<&'static [&'static str]> {
        match self {
            Field::Falloff => Some(&[
                "Smooth",
                "Sphere",
                "Root",
                "Inverse square",
                "Sharp",
                "Linear",
                "Constant",
            ]),
            Field::MirrorAxis => Some(&["X", "Y", "Z"]),
            Field::Profile => Some(&pt_core::build::prims::TrimSection::NAMES),
            Field::Side => Some(&["Centre", "Left", "Right"]),
            Field::PaintApply => Some(&["Brush", "Fill"]),
            Field::PaintChannel => Some(&["RGBA", "RGB", "Alpha"]),
            Field::InsetMode => Some(&["Fraction", "Width", "Equal"]),
            Field::GizmoOrient => Some(&["Global", "Local", "Normal"]),
            Field::TrimPlace => Some(&["Bottom", "Top"]),
            Field::OffsetSpace => Some(&["Global", "Local", "Normal"]),
            Field::ExtrudeMethod => Some(&["Face normal", "Vertex normal", "Individual"]),
            Field::BzKind => Some(&["Plane", "Block"]),
            Field::BzTangent => Some(&["Free", "Aligned", "Mirrored"]),
            Field::BevelAmountType => Some(&pt_core::build::xg_bevel::AmountType::NAMES),
            _ => None,
        }
    }
    /// Fields shown as a CheckBox (value 0 / 1).
    pub fn is_check(self) -> bool {
        matches!(
            self,
            Field::OffsetPerFace
                | Field::EdgesGroup
                | Field::InsetIndividual
                | Field::CollapseFirst
                | Field::FillEntire
                | Field::MirrorX
                | Field::MirrorY
                | Field::MirrorZ
                | Field::MirrorDup
                | Field::ToNewObject
                | Field::ArchBand
                | Field::EndCaps
                | Field::EqualSteps
                | Field::PolyHoles
                | Field::BzClosed
                | Field::BzSmooth
                | Field::BevelClamp
                | Field::SmoothTint
                | Field::ShowNormals
                | Field::BevelHarden
        )
    }
    fn check_curve(self) -> bool {
        matches!(
            self,
            Field::BzKind
                | Field::BzClosed
                | Field::BzSmooth
                | Field::BzWidth
                | Field::BzRows
                | Field::BzColumns
                | Field::BzHeight
                | Field::BzTangent
                | Field::BzRoll
        )
    }
    /// Modes / tools the field is shown for: (shape tool, edit modes mask)
    pub fn shown(self, mode: Mode, tool: Tool, shape: bool) -> bool {
        if tool == Tool::Bezier {
            return self.check_curve() || matches!(self, Field::Slot | Field::GridStep);
        }
        let select = !shape && tool == Tool::Select;
        match self {
            Field::BzKind
            | Field::BzClosed
            | Field::BzSmooth
            | Field::BzWidth
            | Field::BzRows
            | Field::BzColumns
            | Field::BzHeight
            | Field::BzTangent
            | Field::BzRoll => false,
            Field::OffsetX | Field::OffsetY | Field::OffsetZ | Field::OffsetSpace => {
                select && mode != Mode::Object
            }
            Field::OffsetPerFace => select && mode == Mode::Face,
            Field::GrowAngle => select && matches!(mode, Mode::Face | Mode::Edge),
            Field::ExtrudeMethod | Field::InsetIndividual | Field::ToNewObject => {
                select && mode == Mode::Face
            }
            Field::EdgesGroup => select && mode == Mode::Edge,
            Field::CollapseFirst => select && mode == Mode::Vertex,
            Field::WeldDist => !shape && matches!(mode, Mode::Object | Mode::Vertex),
            Field::FillEntire => select && matches!(mode, Mode::Edge | Mode::Vertex),
            Field::MirrorX | Field::MirrorY | Field::MirrorZ | Field::MirrorDup => {
                !shape && mode == Mode::Object
            }
            Field::SegmentsZ
            | Field::HeightCuts
            | Field::TorusArc
            | Field::TubeArc
            | Field::ArchBand
            | Field::ArchDegrees
            | Field::EndCaps
            | Field::StepHeight
            | Field::EqualSteps
            | Field::Subdivisions
            | Field::PolyHoles => shape,
            Field::SizeX
            | Field::SizeY
            | Field::SizeZ
            | Field::Segments
            | Field::Steps
            | Field::Thickness
            | Field::Rings => shape,
            Field::Slot | Field::GridStep => true,
            Field::PaintApply
            | Field::PaintChannel
            | Field::PaintRadius
            | Field::PaintStrength
            | Field::PaintSoftness
            | Field::PaintAlpha => tool == Tool::VertexPaint && !shape,
            Field::Distance | Field::Inset if tool == Tool::Knife => false,
            Field::Distance => !shape && matches!(mode, Mode::Face | Mode::Edge),
            Field::Inset => !shape && mode == Mode::Face,
            Field::Cuts => !shape && mode == Mode::Edge,
            Field::Bevel
            | Field::BevelSegs
            | Field::BevelAmountType
            | Field::BevelProfile
            | Field::BevelClamp
            | Field::BevelHarden => !shape && matches!(mode, Mode::Edge | Mode::Vertex),
            Field::SoftRadius | Field::Falloff | Field::RotSnap => {
                !shape && mode != Mode::Object && tool == Tool::Select
            }
            // Replaced by the Mirror X / Y / Z checks (kept for `debug_field` ids).
            Field::MirrorAxis => false,
            Field::Profile => shape || mode == Mode::Face,
            Field::Arc | Field::Inner | Field::Opening | Field::DoorHeight | Field::Side => shape,
            Field::InsetMode | Field::TrimPlace | Field::TrimDepth | Field::TrimHeight => {
                !shape && mode == Mode::Face
            }
            Field::SubdivSegs => !shape && matches!(mode, Mode::Face | Mode::Object),
            Field::GizmoOrient => !shape && mode != Mode::Object && tool == Tool::Select,
            // Smoothing display: the palette's Smoothing section / the Paint popover's page.
            Field::SmoothTint | Field::ShowNormals | Field::NormalLength => false,
        }
    }
}

/// Face UV fields of the palette (bound to `on_uv_field`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum UvField {
    OffsetU = 0,
    OffsetV,
    ScaleU,
    ScaleV,
    Rotation,
}

impl UvField {
    pub const ALL: [UvField; 5] = [
        UvField::OffsetU,
        UvField::OffsetV,
        UvField::ScaleU,
        UvField::ScaleV,
        UvField::Rotation,
    ];
    pub fn from_i64(v: i64) -> Option<UvField> {
        Self::ALL.iter().copied().find(|f| *f as i64 == v)
    }
    pub fn portray(self) -> (&'static str, f64, f64, f64) {
        match self {
            UvField::OffsetU => ("Offset U", -64.0, 64.0, 0.0624),
            UvField::OffsetV => ("Offset V", -64.0, 64.0, 0.0624),
            UvField::ScaleU => ("Scale U", -64.0, 64.0, 0.0624),
            UvField::ScaleV => ("Scale V", -64.0, 64.0, 0.0624),
            UvField::Rotation => ("Rotation", -360.0, 360.0, 15.0),
        }
    }
}

pub struct Ui {
    pub toolbar: Gd<HFlowContainer>,
    /// The toolbar's scrolling column (the control added to the editor).
    pub rail: Gd<ScrollContainer>,
    /// The bottom strip's scrolling, height-capped root (the control added to the editor).
    pub peel: Gd<ScrollContainer>,
    pub palette: Gd<ScrollContainer>,
    pub buttons: Vec<(Action, Gd<Button>, u8)>,
    /// (section, separator, header) of the toolbar; hidden while none of its buttons is.
    sections: Vec<(usize, Gd<HSeparator>, Gd<Label>)>,
    pub fields: Vec<(Field, Gd<HBoxContainer>, Gd<SpinBox>)>,
    pub choices: Vec<(Field, Gd<HBoxContainer>, Gd<OptionButton>)>,
    pub checks: Vec<(Field, Gd<CheckBox>)>,
    pub arched: Gd<CheckBox>,
    pub kind_box: Gd<HBoxContainer>,
    pub sort: Gd<OptionButton>,
    pub glide: Gd<CheckBox>,
    pub tint: Gd<ColorPickerButton>,
    pub facts: Gd<Label>,
    pub slots: Gd<VBoxContainer>,
    pub slot_buttons: Vec<Gd<Button>>,
    pub slot_group: Gd<ButtonGroup>,
    pub uv_fields: Vec<(UvField, Gd<SpinBox>)>,
    pub uv_world: Gd<CheckBox>,
    /// Smoothing section: group chips (rebuilt on change), the chip menu and the display fields.
    pub chips: Gd<HFlowContainer>,
    /// (group, chip) of the current chips.
    pub chip_buttons: Vec<(u16, Gd<Button>)>,
    pub chip_hint: Gd<Label>,
    pub chip_menu: Gd<PopupMenu>,
    pub attr_fields: Vec<(Field, FieldCtl)>,
    /// Paint color swatches (click = use, Ctrl+click = store the current color).
    pub swatches: Vec<Gd<Button>>,
    /// Name + shortcut of the toolbar button under the mouse.
    pub hover: Gd<Label>,
    target: Gd<Object>,
}

/// Toolbar icon size (px at editor scale 1): the 16 px SVGs are rasterised at this size
/// instead of being stretched, so they stay sharp.
const ICON_PX: f32 = 24.0;
/// Toolbar cell size (px at editor scale 1).
const CELL_PX: f32 = 36.0;

fn glyph(name: &str) -> Option<Gd<Texture2D>> {
    mono_icon(&format!("{ICONS}{name}.svg"), ICON_PX / 16.0)
        .or_else(|| try_load::<Texture2D>(&format!("{ICONS}{name}.svg")).ok())
}

/// Icon color of the editor theme (the MST toolbar's monochrome look).
fn theme_icon_color() -> Color {
    if !godot::classes::Engine::singleton().is_editor_hint() {
        return Color::from_rgb(0.8805, 0.8805, 0.8805);
    }
    godot::classes::EditorInterface::singleton()
        .get_base_control()
        .map(|c| {
            c.get_theme_color_ex("font_color")
                .theme_type("Label")
                .done()
        })
        .unwrap_or(Color::from_rgb(0.8805, 0.8805, 0.8805))
}

/// Rasterises an SVG icon at `scale` × the editor scale (sharp at any size), recoloured to one
/// editor-theme tone (the source icons are multi-coloured;
/// the MST toolbar is monochrome). Luminance keeps the inner detail as light / dark shades.
pub fn mono_icon(path: &str, scale: f32) -> Option<Gd<Texture2D>> {
    use godot::classes::{FileAccess, Image, ImageTexture, image::Format};
    if !FileAccess::file_exists(path) {
        return None;
    }
    let text = FileAccess::get_file_as_string(path);
    let mut img = Image::new_gd();
    if text.is_empty()
        || img
            .load_svg_from_string_ex(&text)
            .scale(scale * ui_kit::editor_scale())
            .done()
            != godot::global::Error::OK
    {
        return None;
    }
    img.convert(Format::RGBA8);
    let tone = theme_icon_color();
    let mut px = img.get_data().to_vec();
    for p in px.chunks_exact_mut(4) {
        let lum = (0.2995 * p[0] as f32 + 0.5875 * p[1] as f32 + 0.1145 * p[2] as f32) / 255.0;
        let k = 0.6205 + 0.3805 * lum;
        p[0] = (tone.r * k * 255.0).round() as u8;
        p[1] = (tone.g * k * 255.0).round() as u8;
        p[2] = (tone.b * k * 255.0).round() as u8;
    }
    let (w, h) = (img.get_width(), img.get_height());
    let img = Image::create_from_data(
        w,
        h,
        false,
        Format::RGBA8,
        &PackedByteArray::from(px.as_slice()),
    )?;
    ImageTexture::create_from_image(&img).map(|t| t.upcast())
}

/// Current values shown by the strip and palette.
pub struct Values<'a> {
    pub spec: &'a ShapeSpec,
    pub pocket: i64,
    pub grid_step: f32,
    pub span: f32,
    pub inset: f32,
    pub chamfer: f32,
    pub cuts: i64,
    pub tint: Color,
    /// offset u, v, scale u, v, rotation
    pub tex_coord: [f32; 5],
    pub uv_world: bool,
    pub bevel_segs: i64,
    pub soft_radius: f32,
    pub falloff: i64,
    pub rot_snap: f32,
    pub mirror_axis: i64,
    /// Vertex paint: fill (vs brush), channel mask, radius, strength, softness, alpha.
    pub vp_fill: bool,
    pub vp_channel: i64,
    pub vp_radius: f32,
    pub vp_strength: f32,
    pub vp_softness: f32,
    pub vp_alpha: f32,
    pub opts: ExtraOpts,
    pub swatches: [Color; 8],
    pub xg: XgOpts,
    pub bz: BzOpts,
}

/// Options of the extended actions (strip fields; defaults keep the older results).
#[derive(Clone, Copy, Debug)]
pub struct XgOpts {
    pub offset: [f32; 3],
    pub offset_space: i64,
    pub offset_per_face: bool,
    pub grow_angle: f32,
    pub extrude_method: i64,
    pub edges_group: bool,
    pub inset_individual: bool,
    pub collapse_first: bool,
    pub weld_dist: f32,
    pub fill_entire: bool,
    pub mirror: [bool; 3],
    pub mirror_dup: bool,
    pub to_new_object: bool,
    pub poly_holes: bool,
    /// Rotate / scale handles act per connected piece.
    pub individual_origins: bool,
    /// Bevel options: amount type (`AmountType` index), profile, clamp overlap, harden.
    pub bevel_type: i64,
    pub bevel_profile: f32,
    pub bevel_clamp: bool,
    pub bevel_harden: bool,
    /// Smoothing display: tint faces by group, normal lines and their length.
    pub smooth_tint: bool,
    pub show_normals: bool,
    pub normal_length: f32,
    /// Current smoothing group of the paint panel (Smooth joins it; 0 = a new group).
    pub smooth_group: u16,
}

impl Default for XgOpts {
    fn default() -> Self {
        XgOpts {
            offset: [0.0, 0.2505, 0.0],
            offset_space: 0,
            offset_per_face: false,
            grow_angle: 0.0,
            extrude_method: 0,
            edges_group: true,
            inset_individual: true,
            collapse_first: false,
            weld_dist: 0.0015,
            fill_entire: true,
            mirror: [true, false, false],
            mirror_dup: true,
            to_new_object: false,
            poly_holes: false,
            individual_origins: false,
            bevel_type: 0,
            bevel_profile: 0.5,
            bevel_clamp: true,
            bevel_harden: false,
            smooth_tint: false,
            show_normals: false,
            normal_length: 0.1005,
            smooth_group: 0,
        }
    }
}

/// Bezier tool settings (defaults: smooth, width 0.2005, 8 rows, 1 column, open) and
/// the selected knot's tangent mode / roll.
#[derive(Clone, Copy, Debug)]
pub struct BzOpts {
    pub block: bool,
    pub is_shut: bool,
    pub glide: bool,
    pub breadth: f32,
    pub ranks: i64,
    pub columns: i64,
    pub stature: f32,
    pub toucher: i64,
    pub roll: f32,
}

impl Default for BzOpts {
    fn default() -> Self {
        BzOpts {
            block: false,
            is_shut: false,
            glide: true,
            breadth: 0.2005,
            ranks: 8,
            columns: 1,
            stature: 1.0,
            toucher: 0,
            roll: 0.0,
        }
    }
}

/// Extra settings (inset mode, subdivide segments, handle axes, trim walls) and the paint color
/// swatches.
#[derive(Clone, Copy, Debug)]
pub struct ExtraOpts {
    pub inset_mode: i64,
    pub subdiv_segs: i64,
    pub gizmo_orient: i64,
    pub trim_place: i64,
    pub trim_depth: f32,
    pub trim_height: f32,
}

impl Default for ExtraOpts {
    fn default() -> Self {
        ExtraOpts {
            inset_mode: 0,
            subdiv_segs: 2,
            gizmo_orient: 0,
            trim_place: 0,
            trim_depth: 0.0604,
            trim_height: 0.1504,
        }
    }
}

/// Vertex colour brush swatches.
pub const SWATCHES: [Color; 8] = [
    Color::from_rgb(1.0, 1.0, 1.0),
    Color::from_rgb(0.9014, 0.1984, 0.2014),
    Color::from_rgb(0.1984, 0.8014, 0.2984),
    Color::from_rgb(0.2514, 0.4984, 0.9514),
    Color::from_rgb(0.9514, 0.7484, 0.1484),
    Color::from_rgb(0.5984, 0.3014, 0.7984),
    Color::from_rgb(0.1484, 0.7984, 0.7514),
    Color::from_rgb(0.1514, 0.1484, 0.1714),
];

impl Values<'_> {
    pub fn fetch(&self, f: Field) -> f64 {
        let s = self.spec;
        let b = |v: bool| if v { 1.0 } else { 0.0 };
        match f {
            Field::SizeX => s.size.x as f64,
            Field::SizeY => s.size.y as f64,
            Field::SizeZ => s.size.z as f64,
            Field::Segments => s.stretches as f64,
            Field::Steps => s.steps as f64,
            Field::Thickness => s.thickness as f64,
            Field::Rings => s.rings as f64,
            Field::Slot => self.pocket as f64,
            Field::GridStep => self.grid_step as f64,
            Field::Distance => self.span as f64,
            Field::Inset => self.inset as f64,
            Field::Bevel => self.chamfer as f64,
            Field::Cuts => self.cuts as f64,
            Field::BevelSegs => self.bevel_segs as f64,
            Field::SoftRadius => self.soft_radius as f64,
            Field::Falloff => self.falloff as f64,
            Field::RotSnap => self.rot_snap as f64,
            Field::MirrorAxis => self.mirror_axis as f64,
            Field::Arc => s.arc as f64,
            Field::Inner => s.inner as f64,
            Field::Opening => s.opening as f64,
            Field::DoorHeight => s.door_height as f64,
            Field::Profile => s.silhouette as f64,
            Field::Side => s.flank as f64,
            Field::PaintApply => self.vp_fill as i64 as f64,
            Field::PaintChannel => self.vp_channel as f64,
            Field::PaintRadius => self.vp_radius as f64,
            Field::PaintStrength => self.vp_strength as f64,
            Field::PaintSoftness => self.vp_softness as f64,
            Field::PaintAlpha => self.vp_alpha as f64,
            Field::InsetMode => self.opts.inset_mode as f64,
            Field::SubdivSegs => self.opts.subdiv_segs as f64,
            Field::GizmoOrient => self.opts.gizmo_orient as f64,
            Field::TrimPlace => self.opts.trim_place as f64,
            Field::TrimDepth => self.opts.trim_depth as f64,
            Field::TrimHeight => self.opts.trim_height as f64,
            Field::OffsetX => self.xg.offset[0] as f64,
            Field::OffsetY => self.xg.offset[1] as f64,
            Field::OffsetZ => self.xg.offset[2] as f64,
            Field::OffsetSpace => self.xg.offset_space as f64,
            Field::OffsetPerFace => b(self.xg.offset_per_face),
            Field::GrowAngle => self.xg.grow_angle as f64,
            Field::ExtrudeMethod => self.xg.extrude_method as f64,
            Field::EdgesGroup => b(self.xg.edges_group),
            Field::InsetIndividual => b(self.xg.inset_individual),
            Field::CollapseFirst => b(self.xg.collapse_first),
            Field::WeldDist => self.xg.weld_dist as f64,
            Field::FillEntire => b(self.xg.fill_entire),
            Field::MirrorX => b(self.xg.mirror[0]),
            Field::MirrorY => b(self.xg.mirror[1]),
            Field::MirrorZ => b(self.xg.mirror[2]),
            Field::MirrorDup => b(self.xg.mirror_dup),
            Field::ToNewObject => b(self.xg.to_new_object),
            Field::SegmentsZ => s.segments_z as f64,
            Field::HeightCuts => s.height_cuts as f64,
            Field::TorusArc => s.torus_arc as f64,
            Field::TubeArc => s.tube_arc as f64,
            Field::ArchBand => b(s.arch_band),
            Field::ArchDegrees => s.arch_degrees as f64,
            Field::EndCaps => b(s.end_caps),
            Field::StepHeight => s.step_height as f64,
            Field::EqualSteps => b(s.equal_steps),
            Field::Subdivisions => s.subdivisions as f64,
            Field::PolyHoles => b(self.xg.poly_holes),
            Field::BzKind => b(self.bz.block),
            Field::BzClosed => b(self.bz.is_shut),
            Field::BzSmooth => b(self.bz.glide),
            Field::BzWidth => self.bz.breadth as f64,
            Field::BzRows => self.bz.ranks as f64,
            Field::BzColumns => self.bz.columns as f64,
            Field::BzHeight => self.bz.stature as f64,
            Field::BzTangent => self.bz.toucher as f64,
            Field::BzRoll => self.bz.roll as f64,
            Field::BevelAmountType => self.xg.bevel_type as f64,
            Field::BevelProfile => self.xg.bevel_profile as f64,
            Field::BevelClamp => b(self.xg.bevel_clamp),
            Field::BevelHarden => b(self.xg.bevel_harden),
            Field::SmoothTint => b(self.xg.smooth_tint),
            Field::ShowNormals => b(self.xg.show_normals),
            Field::NormalLength => self.xg.normal_length as f64,
        }
    }
}

/// Toolbar / palette state mirrored by `Ui::sync` (and the alternate layout's sync).
#[derive(Clone, Default)]
pub struct SyncState {
    pub mode: Mode,
    pub tool: Tool,
    pub xform: XformMode,
    pub snap_grid: bool,
    pub snap_cell: bool,
    pub snap_vertex: bool,
    pub soft: bool,
    pub uv_lock: bool,
    pub reshape: bool,
    pub has_target: bool,
    pub has_spec: bool,
    pub can_convert: bool,
    pub insert_vert: bool,
    pub surface_draw: bool,
    /// Picks and box selections go through faces.
    pub xray: bool,
    /// Entity type of the target (0 normal, 1 collider, 2 trigger).
    pub entity: i32,
    /// Bezier tool: a knot is selected (its tangent / roll fields show).
    pub bz_knot: bool,
    // Alternate layout.
    /// The builder plugin is shown (`make_visible(true)`).
    pub visible: bool,
    /// A builder mesh or a host node is selected (new shapes can be drawn).
    pub can_create: bool,
    /// Selected faces / elements of the mode.
    pub sel_faces: usize,
    /// The builder's status message (errors, counts).
    pub message: String,
    /// Status line of the active drawing tool (the overlay text).
    pub tool_detail: String,
    pub prefs: super::prefs::Prefs,
    /// A live adjust session is open (its panel owns the bottom of the viewport).
    pub adjusting: bool,
    /// Per-mesh settings of the target: Live UV2, clear grass.
    pub live_uv2: bool,
    pub clear_grass: bool,
    /// Slot of the first selected face (Paint popover summary).
    pub face_slot: Option<i64>,
}

impl SyncState {
    /// The shape fields show (the Shape tool, or a shape recipe being re-edited).
    pub fn shape_fields(&self) -> bool {
        (self.tool == Tool::Shape || self.reshape) && self.tool != Tool::Bezier
    }
}

/// Shape icon for a kind (in the Shape drop-down).
fn shape_icon(k: ShapeKind) -> &'static str {
    match k {
        ShapeKind::Box => "shape_cube",
        ShapeKind::Plane => "shape_plane",
        ShapeKind::Cylinder => "shape_cylinder",
        ShapeKind::Cone => "shape_cone",
        ShapeKind::Sphere => "shape_sphere",
        ShapeKind::Stairs => "shape_stairs",
        ShapeKind::Arch => "shape_arch",
        ShapeKind::Wedge => "shape_prism",
        ShapeKind::Pipe => "shape_pipe",
        ShapeKind::HexPrism | ShapeKind::HexCluster => "shape",
        ShapeKind::Torus => "shape_torus",
        ShapeKind::Door => "shape_door",
        ShapeKind::CurvedStairs => "shape_curved_stairs",
        ShapeKind::Trim => "shape_trim",
        ShapeKind::Wall => "shape_wall",
        ShapeKind::Poly => "shape_poly",
        ShapeKind::Prism => "shape_prism",
        ShapeKind::Sprite => "shape_sprite",
        ShapeKind::Icosphere => "shape_icosphere",
    }
}

/// The builder's (tinted) icon of a shape kind, at palette-tile size.
pub fn shape_tile_icon(k: ShapeKind) -> Option<Gd<Texture2D>> {
    glyph(shape_icon(k))
}

pub fn style_swatch(b: &mut Gd<Button>, c: Color) {
    let mut st = godot::classes::StyleBoxFlat::new_gd();
    st.set_bg_color(Color::from_rgba(c.r, c.g, c.b, 1.0));
    st.set_corner_radius_all(3);
    for state in ["normal", "hover", "pressed", "focus"] {
        b.add_theme_stylebox_override(state, &st);
    }
}

/// Status-line text for a toolbar button: its title and shortcut (first tooltip line).
pub fn hover_text(a: Action) -> String {
    action_row(a)
        .map(|r| {
            bound_title(
                a,
                r.0.lines()
                    .next()
                    .unwrap_or("")
                    .split(": ")
                    .next()
                    .unwrap_or(""),
            )
        })
        .unwrap_or_default()
}

/// The full tooltip of an action ("Title [Key]", details, modes), as on the MST toolbar.
pub fn action_tooltip(a: Action) -> String {
    action_row(a)
        .map(|r| format_tooltip(a, r.0, r.1))
        .unwrap_or_default()
}

fn label(caption: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(caption);
    l
}

fn spin(min: f64, max: f64, step: f64, value: f64, tip: &str, width: f32) -> Gd<SpinBox> {
    let mut sb = SpinBox::new_alloc();
    sb.set_min(min);
    sb.set_max(max);
    sb.set_step(step);
    sb.set_value_no_signal(value);
    sb.set_tooltip_text(tip);
    sb.set_custom_minimum_size(Vector2::new(width, 0.0));
    sb
}

/// One strip field's control: a labelled SpinBox, a labelled OptionButton or a CheckBox.
pub enum FieldCtl {
    Spin(Gd<HBoxContainer>, Gd<SpinBox>),
    Option(Gd<HBoxContainer>, Gd<OptionButton>),
    Check(Gd<CheckBox>),
}

impl FieldCtl {
    /// The control to add to a container.
    pub fn widget(&self) -> Gd<Control> {
        match self {
            FieldCtl::Spin(hb, _) | FieldCtl::Option(hb, _) => hb.clone().upcast(),
            FieldCtl::Check(cb) => cb.clone().upcast(),
        }
    }
    /// Mirrors the field's value without emitting signals.
    pub fn sync(&mut self, f: Field, v: &Values) {
        match self {
            FieldCtl::Spin(_, sb) => sb.set_value_no_signal(v.fetch(f)),
            FieldCtl::Option(_, ob) => ob.select(v.fetch(f) as i32),
            FieldCtl::Check(cb) => cb.set_pressed_no_signal(v.fetch(f) >= 0.5),
        }
    }
}

/// Builds the control of strip field `f`, bound to `on_field` / `on_field_option` /
/// `on_field_check` of `target`. The strip and the live adjust panel both use it, so both edit
/// the same value.
pub fn make_field(addressee: &Gd<Object>, f: Field, v: &Values) -> FieldCtl {
    let (text, min, max, step, tip) = f.portray();
    if f.is_check() {
        let mut cb = CheckBox::new_alloc();
        cb.set_text(text);
        cb.set_tooltip_text(tip);
        cb.set_focus_mode(FocusMode::NONE);
        cb.set_pressed_no_signal(v.fetch(f) >= 0.5);
        cb.connect(
            "toggled",
            &Callable::from_object_method(addressee, "on_field_check")
                .bind(&[(f as i64).to_variant()]),
        );
        return FieldCtl::Check(cb);
    }
    if let Some(choices) = f.choices() {
        let mut hb = HBoxContainer::new_alloc();
        let mut l = label(text);
        l.set_tooltip_text(tip);
        hb.add_child(&l);
        let mut ob = OptionButton::new_alloc();
        for c in choices {
            ob.add_item(*c);
        }
        ob.select(v.fetch(f) as i32);
        ob.set_tooltip_text(tip);
        ob.set_focus_mode(FocusMode::NONE);
        ob.connect(
            "item_selected",
            &Callable::from_object_method(addressee, "on_field_option")
                .bind(&[(f as i64).to_variant()]),
        );
        hb.add_child(&ob);
        return FieldCtl::Option(hb, ob);
    }
    let mut hb = HBoxContainer::new_alloc();
    let mut l = label(text);
    l.set_tooltip_text(tip);
    hb.add_child(&l);
    let mut sb = spin(min, max, step, v.fetch(f), tip, 64.0);
    sb.set_allow_greater(matches!(
        f,
        Field::SizeX | Field::SizeY | Field::SizeZ | Field::Distance | Field::Bevel
    ));
    sb.connect(
        "value_changed",
        &Callable::from_object_method(addressee, "on_field").bind(&[(f as i64).to_variant()]),
    );
    hb.add_child(&sb);
    FieldCtl::Spin(hb, sb)
}

pub fn assemble(addressee: &Gd<Object>, v: &Values) -> Ui {
    // ---------------------------------------------------------------- left toolbar
    // Same look as the terrain toolbar (`ui_kit::Toolbar`): 30 px flat icon buttons, centred in
    // their cells, groups split by a separator plus a small section header.
    //
    // A fixed two-cell-wide grid (an `HFlowContainer` as wide as a section header) in a
    // vertical-only `ScrollContainer`: a `VFlowContainer` wrapped into as many columns as the
    // left panel's height needed (6 at 1600×900 in Face mode) and took that width from the 3D
    // viewport; the rail scrolls instead.
    let mut toolbar = HFlowContainer::new_alloc();
    toolbar.set_name("PixelBuilderToolbar");
    toolbar.set_custom_minimum_size(ui_kit::sz(CELL_PX * 2.0, CELL_PX));
    toolbar.set_alignment(godot::classes::flow_container::AlignmentMode::BEGIN);
    toolbar.add_theme_constant_override("h_separation", 0);
    toolbar.add_theme_constant_override("v_separation", 0);
    let mut rail = ScrollContainer::new_alloc();
    rail.set_name("PixelBuilderToolbarRail");
    rail.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
    rail.set_vertical_scroll_mode(godot::classes::scroll_container::ScrollMode::AUTO);
    rail.set_vertical_custom_step(ui_kit::sz(0.0, CELL_PX).y);
    rail.add_theme_stylebox_override("focus", &godot::classes::StyleBoxEmpty::new_gd());
    rail.add_child(&toolbar);
    // The strip's hover readout (name + shortcut of the toolbar button under the mouse). Built
    // before the buttons so their `mouse_entered` / `mouse_exited` signals can target the label
    // itself (deferred) instead of the plugin: a `mouse_exited` fired while the plugin was
    // mutably bound (a toolbar rebuild / container sort during `select`) re-entered
    // `PixelBuilderPlugin::on_button_hover` and panicked with "already bound" (r4 #2).
    // A fixed, clipped width: a label that grew with its text re-wrapped the strip, moved the
    // toolbar under the pointer and fed an enter/exit -> re-sort loop.
    let mut hover = Label::new_alloc();
    hover.set_name("PixelBuilderHover");
    hover.add_theme_color_override("font_color", Color::from_rgb(0.6205, 0.7805, 0.9505));
    hover.set_custom_minimum_size(Vector2::new(220.0 * ui_kit::editor_scale(), 0.0));
    hover.set_clip_text(true);
    hover.set_text_overrun_behavior(godot::classes::text_server::OverrunBehavior::TRIM_ELLIPSIS);
    hover.set_tooltip_text("Name and shortcut of the builder button under the mouse");
    let mut buttons = Vec::new();
    let mut sections = Vec::new();
    let groups = [
        ButtonGroup::new_gd(),
        ButtonGroup::new_gd(),
        ButtonGroup::new_gd(),
    ];
    let mut last_section = None;
    // Grouped by section (stable), so every section has exactly one header.
    let mut ordered: Vec<&(Action, &str, &str, u8, u8)> = BUTTONS.iter().collect();
    ordered.sort_by_key(|b| section_of(b.0, b.3));
    for (a, ic, tip, group, mask) in ordered {
        let sec = section_of(*a, *group);
        if last_section != Some(sec) {
            let mut sep = HSeparator::new_alloc();
            sep.set_name(&format!("{}Separator", SECTIONS[sec].0));
            // A full grid row (the flow starts a new line for it).
            sep.set_custom_minimum_size(ui_kit::sz(CELL_PX * 2.0, 0.0));
            toolbar.add_child(&sep);
            let mut head = Label::new_alloc();
            head.set_name(&format!("{}Header", SECTIONS[sec].0));
            head.set_text(SECTIONS[sec].0);
            head.set_tooltip_text(SECTIONS[sec].1);
            head.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
            head.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
            head.add_theme_font_size_override("font_size", (11.0 * ui_kit::editor_scale()) as i32);
            head.add_theme_color_override("font_color", Color::from_rgb(0.6205, 0.7805, 0.9505));
            head.set_custom_minimum_size(ui_kit::sz(CELL_PX * 2.0, 0.0));
            toolbar.add_child(&head);
            sections.push((sec, sep, head));
            last_section = Some(sec);
        }
        let mut b = ui_kit::instrument_clicker(
            &format!("{a:?}"),
            &format_tooltip(*a, tip, *mask),
            glyph(ic),
        );
        b.set_custom_minimum_size(ui_kit::sz(CELL_PX - 2.0, CELL_PX - 2.0));
        // Icons drawn at their rasterised size, with a slim margin (the flat editor style's
        // content margins shrank them to ~14 px).
        b.set_expand_icon(false);
        for state in [
            "normal",
            "hover",
            "pressed",
            "hover_pressed",
            "disabled",
            "focus",
        ] {
            let theme = godot::classes::EditorInterface::singleton().get_editor_theme();
            let src = theme.and_then(|t| t.get_stylebox(state, "Button"));
            if let Some(sb) = src.map(|x| x.duplicate_resource()) {
                let mut sb = sb;
                sb.set_content_margin_all(2.0 * ui_kit::editor_scale());
                b.add_theme_stylebox_override(state, &sb);
            }
        }
        if *group == 1 || *group == 2 || *group == 7 {
            b.set_button_group(&groups[if *group == 7 { 2 } else { *group as usize - 1 }]);
        } else if *group != 3 {
            b.set_toggle_mode(false);
        }
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "upon_deed").bind(&[(*a as i64).to_variant()]),
        );
        // Hover readout: straight to the label, deferred; never through the plugin (see above).
        b.connect_flags(
            "mouse_entered",
            &Callable::from_object_method(&hover, "set_text")
                .bind(&[GString::from(hover_text(*a).as_str()).to_variant()]),
            godot::classes::object::ConnectFlags::DEFERRED,
        );
        b.connect_flags(
            "mouse_exited",
            &Callable::from_object_method(&hover, "set_text")
                .bind(&[GString::new().to_variant()]),
            godot::classes::object::ConnectFlags::DEFERRED,
        );
        toolbar.add_child(&ui_kit::midpoint(ui_kit::sz(CELL_PX, CELL_PX), &b));
        buttons.push((*a, b, *mask));
    }

    // ---------------------------------------------------------------- bottom strip
    // The terrain strips' root and flow (`ui_kit::strip_root` + `MstFlow`): the rows wrap and
    // the strip is as tall as they are up to the shared height cap (`ui_kit::strip_cap`), then
    // scrolls. A bare `HFlowContainer` reported its full wrapped height (447 px in Face mode at
    // 1600×900) and the 3D viewport was squeezed to its 39 px minimum.
    let peel = ui_kit::strip_root("PixelBuilderStripRoot");
    let mut strip = ui_kit::MstFlow::new_alloc();
    strip.bind_mut().h_gap = ui_kit::sz(4.0, 0.0).x;
    strip.set_name("PixelBuilderStrip");
    strip.set_h_size_flags(SizeFlags::EXPAND_FILL);
    strip.set_v_size_flags(SizeFlags::FILL);
    {
        let mut peel = peel.clone();
        peel.add_child(&strip);
    }

    strip.add_child(&hover);

    let mut kind_box = HBoxContainer::new_alloc();
    kind_box.add_child(&label("Shape"));
    let mut kind = OptionButton::new_alloc();
    for k in ShapeKind::ALL {
        match glyph(shape_icon(k)) {
            Some(t) => kind.add_icon_item(&t, k.moniker()),
            None => kind.add_item(k.moniker()),
        }
    }
    kind.select(v.spec.sort as i32);
    kind.set_focus_mode(FocusMode::NONE);
    kind.connect(
        "item_selected",
        &Callable::from_object_method(addressee, "on_kind"),
    );
    kind_box.add_child(&kind);
    strip.add_child(&kind_box);

    let mut smooth = CheckBox::new_alloc();
    smooth.set_text("Smooth");
    smooth.set_pressed_no_signal(v.spec.glide);
    smooth.set_tooltip_text("Smooth shading for round shapes");
    smooth.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_smooth"),
    );

    let mut color = ColorPickerButton::new_alloc();
    color.set_custom_minimum_size(Vector2::new(28.0, 0.0));
    color.set_pick_color(v.tint);
    color.set_tooltip_text("Face color (multiplies the slot material)");
    color.connect(
        "color_changed",
        &Callable::from_object_method(addressee, "on_color"),
    );

    let mut arched = CheckBox::new_alloc();
    arched.set_text("Arched");
    arched.set_pressed_no_signal(v.spec.arched);
    arched.set_tooltip_text("Round-topped doorway");
    arched.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_arched"),
    );

    // Vertex colour brush swatches.
    let mut swatch_box = HBoxContainer::new_alloc();
    swatch_box.add_theme_constant_override("separation", 2);
    let mut swatches = Vec::new();
    for (i, c) in v.swatches.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_custom_minimum_size(ui_kit::sz(14.0, 20.0));
        b.set_focus_mode(FocusMode::NONE);
        b.set_tooltip_text("Color swatch: click to use it, Ctrl+click to store the current color in it, Alt+click to select the faces painted with it");
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_swatch").bind(&[(i as i64).to_variant()]),
        );
        style_swatch(&mut b, *c);
        swatch_box.add_child(&b);
        swatches.push(b);
    }

    let mut fields = Vec::new();
    let mut options = Vec::new();
    let mut checks = Vec::new();
    for f in Field::ALL {
        match make_field(addressee, f, v) {
            FieldCtl::Check(cb) => {
                strip.add_child(&cb);
                checks.push((f, cb));
            }
            FieldCtl::Option(hb, ob) => {
                strip.add_child(&hb);
                options.push((f, hb, ob));
            }
            FieldCtl::Spin(hb, sb) => {
                if f == Field::Slot {
                    strip.add_child(&VSeparator::new_alloc());
                }
                strip.add_child(&hb);
                if f == Field::Slot {
                    let mut cl = label("Color");
                    cl.set_tooltip_text("Paint color (picker) and swatches: click a swatch to use it, Ctrl+click to store the current color in it");
                    strip.add_child(&cl);
                    strip.add_child(&color);
                    strip.add_child(&swatch_box);
                    strip.add_child(&VSeparator::new_alloc());
                }
                if f == Field::Rings {
                    strip.add_child(&smooth);
                }
                if f == Field::DoorHeight {
                    strip.add_child(&arched);
                }
                fields.push((f, hb, sb));
            }
        }
    }
    let mut info = Label::new_alloc();
    info.add_theme_font_size_override("font_size", 12);
    info.set_v_size_flags(SizeFlags::SHRINK_CENTER);
    strip.add_child(&info);

    // ---------------------------------------------------------------- right palette
    let mut palette = ScrollContainer::new_alloc();
    palette.set_name("PixelBuilderPalette");
    palette.set_custom_minimum_size(Vector2::new(210.0, 0.0));
    palette.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
    let mut col = VBoxContainer::new_alloc();
    col.set_h_size_flags(SizeFlags::EXPAND_FILL);
    let mut title = label("Materials");
    title.set_tooltip_text("Material slots of the selected builder mesh.\nClick a slot to make it current (and assign it to the selected faces).\nDrop or load any Material into a slot's picker.");
    col.add_child(&title);
    let slots = VBoxContainer::new_alloc();
    col.add_child(&slots);
    let mut add = Button::new_alloc();
    add.set_text("+ Slot");
    add.set_tooltip_text("Add a material slot (a vertex-color StandardMaterial3D you can replace)");
    add.set_focus_mode(FocusMode::NONE);
    add.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_add_slot"),
    );
    // Paint row: + Slot, built-in presets, palette files, sample (`plugin/paint_cmds.rs`).
    let mut paint_row = HFlowContainer::new_alloc();
    paint_row.set_name("PaintRow");
    paint_row.add_child(&add);
    let mut presets = MenuButton::new_alloc();
    presets.set_name("Presets");
    presets.set_text("+ Preset");
    presets.set_flat(false);
    presets.set_tooltip_text("Add a slot with a built-in material: Grid, Checker (patterned, vertex colour),\nNo Draw (transparent), Collider (red), Trigger (orange)");
    if let Some(mut p) = presets.get_popup() {
        for (i, pr) in pt_core::build::xg_paint::Preset::ALL.iter().enumerate() {
            p.add_item_ex(pr.heading()).id(i as i32).done();
        }
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_preset_menu"),
        );
    }
    paint_row.add_child(&presets);
    let mut pal = MenuButton::new_alloc();
    pal.set_name("PaletteMenu");
    pal.set_text("Palette");
    pal.set_flat(false);
    pal.set_tooltip_text("Palette files: the slots' materials and the colour swatches.\nSave / Load as JSON (Modeler Modeler's palette JSON) or as a .tres resource (Modeler's palette resource)");
    if let Some(mut p) = pal.get_popup() {
        p.add_item_ex("Load Palette…")
            .id(PALETTE_LOAD as i32)
            .done();
        p.add_item_ex("Save Palette…")
            .id(PALETTE_SAVE as i32)
            .done();
        p.add_item_ex("Reset Colours")
            .id(PALETTE_RESET as i32)
            .done();
        p.set_item_tooltip(2, "Swatches back to the default colours (the slots stay)");
        p.connect(
            "id_pressed",
            &Callable::from_object_method(addressee, "on_palette_menu"),
        );
    }
    paint_row.add_child(&pal);
    let mut sample = Button::new_alloc();
    sample.set_name("Sample");
    sample.set_text("Sample");
    sample.set_tooltip_text("Sample from selection: the slot, colour and smoothing group most selected faces use\nbecome current (all faces in Object mode)");
    sample.set_focus_mode(FocusMode::NONE);
    sample.connect(
        "pressed",
        &Callable::from_object_method(addressee, "on_paint_sample"),
    );
    paint_row.add_child(&sample);
    col.add_child(&paint_row);
    col.add_child(&HSeparator::new_alloc());
    // ---------------------------------------------------------------- smoothing
    let mut smt = label("Smoothing");
    smt.set_tooltip_text("Smoothing groups: faces in one group shade smooth across their shared edges; group 0 is faceted.\nThe actions use the selected faces (all faces in Object mode).");
    col.add_child(&smt);
    let mut srow = HFlowContainer::new_alloc();
    srow.set_name("SmoothingRow");
    for (text, cmd, tip) in [
        (
            "Smooth",
            0i64,
            "Smooth: the selected faces join the current group (the outlined chip), else a new one",
        ),
        (
            "Faceted",
            1,
            "Faceted: flat shading for the selected faces (group 0)",
        ),
        (
            "New group",
            2,
            "New group from selection: the selected faces get the next free group (1-23)",
        ),
    ] {
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_tooltip_text(tip);
        b.set_focus_mode(FocusMode::NONE);
        b.connect(
            "pressed",
            &Callable::from_object_method(addressee, "on_xg_smoothing")
                .bind(&[cmd.to_variant(), 0i64.to_variant()]),
        );
        srow.add_child(&b);
    }
    col.add_child(&srow);
    let mut chips = HFlowContainer::new_alloc();
    chips.set_name("Chips");
    col.add_child(&chips);
    let mut chip_hint = label("No smoothing groups in this mesh.");
    chip_hint.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.6005));
    col.add_child(&chip_hint);
    let mut chip_menu = PopupMenu::new_alloc();
    chip_menu.set_name("ChipMenu");
    chip_menu.connect(
        "id_pressed",
        &Callable::from_object_method(addressee, "on_xg_paint_menu"),
    );
    col.add_child(&chip_menu);
    let mut attr_row = HFlowContainer::new_alloc();
    attr_row.set_name("SmoothingDisplay");
    let mut attr_fields = Vec::new();
    for f in [Field::SmoothTint, Field::ShowNormals, Field::NormalLength] {
        let c = make_field(addressee, f, v);
        attr_row.add_child(&c.widget());
        attr_fields.push((f, c));
    }
    col.add_child(&attr_row);
    col.add_child(&HSeparator::new_alloc());
    let mut uvt = label("Face UV");
    uvt.set_tooltip_text("Automatic box projection per face: every face is mapped along its dominant axis.\nWorld aligned keeps textures continuous across meshes.\nSelecting one face loads its values; Apply writes them to the selected faces.");
    col.add_child(&uvt);
    let mut grid = GridContainer::new_alloc();
    grid.set_columns(2);
    let mut uv_fields = Vec::new();
    for (i, f) in UvField::ALL.iter().enumerate() {
        let (text, min, max, step) = f.portray();
        grid.add_child(&label(text));
        let mut sb = spin(min, max, step, v.tex_coord[i] as f64, text, 90.0);
        sb.set_allow_greater(true);
        sb.set_allow_lesser(true);
        sb.connect(
            "value_changed",
            &Callable::from_object_method(addressee, "on_uv_field")
                .bind(&[(*f as i64).to_variant()]),
        );
        grid.add_child(&sb);
        uv_fields.push((*f, sb));
    }
    col.add_child(&grid);
    let mut uv_world = CheckBox::new_alloc();
    uv_world.set_text("World aligned");
    uv_world.set_pressed_no_signal(v.uv_world);
    uv_world.connect(
        "toggled",
        &Callable::from_object_method(addressee, "on_uv_world"),
    );
    col.add_child(&uv_world);
    let mut row = HBoxContainer::new_alloc();
    for (text, method, tip) in [
        (
            "Apply",
            "on_uv_apply",
            "Apply the UV values to the selected faces (all faces in Object mode)",
        ),
        ("Reset", "on_uv_reset", "Reset the UV values (and apply)"),
        (
            "↻ 90°",
            "on_uv_rotate",
            "Rotate the selected faces' UVs by 90°",
        ),
    ] {
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_tooltip_text(tip);
        b.set_focus_mode(FocusMode::NONE);
        b.connect("pressed", &Callable::from_object_method(addressee, method));
        row.add_child(&b);
    }
    col.add_child(&row);
    palette.add_child(&col);

    let mut ui = Ui {
        toolbar,
        rail,
        peel,
        palette,
        buttons,
        sections,
        fields,
        choices: options,
        checks,
        arched,
        kind_box,
        sort: kind,
        glide: smooth,
        tint: color,
        facts: info,
        swatches,
        hover,
        slots,
        slot_buttons: Vec::new(),
        slot_group: ButtonGroup::new_gd(),
        uv_fields,
        uv_world,
        chips,
        chip_buttons: Vec::new(),
        chip_hint,
        chip_menu,
        attr_fields,
        target: addressee.clone(),
    };
    ui.show_fields(v, &SyncState::default());
    ui
}

impl Ui {
    pub fn assign_pat(&mut self, i: usize, c: Color) {
        if let Some(b) = self.swatches.get_mut(i) {
            style_swatch(b, c);
        }
    }

    /// Shows the strip fields that matter for the current kind / mode / tool.
    pub fn show_fields(&mut self, v: &Values, st: &SyncState) {
        let shape = st.shape_fields();
        for (f, hb, _) in &mut self.fields {
            hb.set_visible(field_visible(*f, v, st));
        }
        for (f, hb, _) in &mut self.choices {
            hb.set_visible(field_visible(*f, v, st));
        }
        for (f, cb) in &mut self.checks {
            cb.set_visible(field_visible(*f, v, st));
        }
        let (_, _, _, smooth, _) = v.spec.sort.uses();
        let (_, door, _, _) = v.spec.sort.uses_extra();
        let band = v.spec.sort.uses_modeler().arch_band && v.spec.arch_band;
        self.kind_box.set_visible(shape);
        self.glide.set_visible((smooth || band) && shape);
        self.arched.set_visible(door && shape);
    }

    /// Mirrors values into the controls without emitting signals.
    pub fn sync(&mut self, v: &Values, st: &SyncState) {
        let mode = st.mode;
        for (f, _, sb) in &mut self.fields {
            sb.set_value_no_signal(v.fetch(*f));
        }
        for (f, _, ob) in &mut self.choices {
            ob.select(v.fetch(*f) as i32);
        }
        for (f, cb) in &mut self.checks {
            cb.set_pressed_no_signal(v.fetch(*f) >= 0.5);
        }
        self.arched.set_pressed_no_signal(v.spec.arched);
        self.sort.select(v.spec.sort as i32);
        self.glide.set_pressed_no_signal(v.spec.glide);
        self.tint.set_pick_color(v.tint);
        self.show_fields(v, st);
        for (i, (_, sb)) in self.uv_fields.iter_mut().enumerate() {
            sb.set_value_no_signal(v.tex_coord[i] as f64);
        }
        self.uv_world.set_pressed_no_signal(v.uv_world);
        for (f, c) in &mut self.attr_fields {
            c.sync(*f, v);
            if *f == Field::NormalLength {
                c.widget().set_visible(v.xg.show_normals);
            }
        }
        for (a, b, mask) in &mut self.buttons {
            let visible = action_visible(*a, *mask, st);
            b.set_visible(visible);
            if *a == Action::Dissolve {
                let what = match mode {
                    Mode::Face => {
                        "Dissolve faces: merge the selected faces into one n-gon (their inner edges and vertices go)"
                    }
                    Mode::Edge => {
                        "Dissolve edges: remove the selected edges, joining the two faces of each"
                    }
                    _ => {
                        "Dissolve vertices: remove the selected vertices, merging the faces around each into one"
                    }
                };
                b.set_tooltip_text(&format_tooltip(*a, what, *mask));
            }
            // The button sits in a centring cell: hide the cell too, so the column closes up.
            if let Some(mut cell) = b.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                cell.set_visible(visible);
            }
            let Some(on) = action_pressed(*a, v, st) else {
                continue;
            };
            b.set_pressed_no_signal(on);
        }
        for (sec, sep, head) in &mut self.sections {
            let any = self.buttons.iter().any(|(a, b, _)| {
                b.is_visible()
                    && BUTTONS
                        .iter()
                        .find(|x| x.0 == *a)
                        .is_some_and(|x| section_of(*a, x.3) == *sec)
            });
            sep.set_visible(any);
            head.set_visible(any);
        }
        for (i, b) in self.slot_buttons.iter_mut().enumerate() {
            b.set_pressed_no_signal(i as i64 == v.pocket);
        }
    }

    /// Rebuilds the material slot rows (slot button + resource picker per slot).
    pub fn rebuild_slots(&mut self, materials: &[Option<Gd<Material>>], current: i64) {
        for mut c in self.slots.get_children().iter_shared() {
            self.slots.remove_child(&c);
            c.queue_free();
        }
        self.slot_buttons.clear();
        self.slot_group = ButtonGroup::new_gd();
        for (i, m) in materials.iter().enumerate() {
            let mut row = HBoxContainer::new_alloc();
            let mut b = Button::new_alloc();
            b.set_text(&format!("{i}"));
            b.set_toggle_mode(true);
            b.set_button_group(&self.slot_group);
            b.set_custom_minimum_size(Vector2::new(28.0, 28.0));
            b.set_focus_mode(FocusMode::NONE);
            b.set_tooltip_text(&format!(
                "Slot {i}: make current; assigns it to the selected faces"
            ));
            b.set_pressed_no_signal(i as i64 == current);
            if let Some(c) = m.as_ref().and_then(material_color) {
                b.add_theme_color_override(
                    "font_color",
                    if c.luminance() > 0.5 {
                        Color::BLACK
                    } else {
                        Color::WHITE
                    },
                );
                b.add_theme_color_override(
                    "font_pressed_color",
                    Color::from_rgb(1.0, 0.7005, 0.2005),
                );
                let mut sb = godot::classes::StyleBoxFlat::new_gd();
                sb.set_bg_color(c);
                b.add_theme_stylebox_override("normal", &sb);
            }
            b.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_slot_pick")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&b);
            let mut picker = EditorResourcePicker::new_alloc();
            picker.set_base_type("Material");
            picker.set_h_size_flags(SizeFlags::EXPAND_FILL);
            if let Some(m) = m {
                picker.set_edited_resource(m);
            }
            picker.connect(
                "resource_changed",
                &Callable::from_object_method(&self.target, "on_slot_material")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&picker);
            let mut sel = Button::new_alloc();
            sel.set_text("Sel");
            sel.set_flat(true);
            sel.set_focus_mode(FocusMode::NONE);
            sel.set_tooltip_text(&format!("Select every face using slot {i} (Face mode)"));
            sel.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_select_slot")
                    .bind(&[(i as i64).to_variant()]),
            );
            row.add_child(&sel);
            self.slots.add_child(&row);
            self.slot_buttons.push(b);
        }
    }

    /// Rebuilds the smoothing group chips (`groups` = (group, faces)); `current` is outlined.
    pub fn rebuild_chips(&mut self, groups: &[(u16, usize)], current: u16) {
        for mut c in self.chips.get_children().iter_shared() {
            self.chips.remove_child(&c);
            c.queue_free();
        }
        self.chip_buttons.clear();
        for (g, n) in groups {
            let mut b = Button::new_alloc();
            b.set_text(&format!("{g} ({n})"));
            b.set_toggle_mode(true);
            b.set_pressed_no_signal(*g == current);
            b.set_focus_mode(FocusMode::NONE);
            b.set_tooltip_text(&format!(
                "Smoothing group {g}, {n} face{}.\nClick: select its faces (and make it current for Smooth).\nRight-click: select, merge into the selected faces' group, or clear",
                if *n == 1 { "" } else { "s" }
            ));
            let c = pt_core::build::xg_paint::gliding_cluster_tint(*g);
            let col = Color::from_rgba(c[0], c[1], c[2], 1.0);
            let style = |border: Color, width: i32| {
                let mut sb = godot::classes::StyleBoxFlat::new_gd();
                sb.set_bg_color(col.darkened(0.5505));
                sb.set_border_color(border);
                sb.set_border_width_all(width);
                sb.set_corner_radius_all(4);
                sb.set_content_margin_all(3.0);
                sb
            };
            let (normal, on) = (
                style(col, 1),
                style(Color::from_rgb(1.0, 0.7005, 0.2005), 2),
            );
            b.add_theme_stylebox_override("normal", &normal);
            b.add_theme_stylebox_override("hover", &normal);
            b.add_theme_stylebox_override("pressed", &on);
            b.add_theme_stylebox_override("hover_pressed", &on);
            b.add_theme_color_override("font_color", col.lightened(0.3005));
            b.add_theme_color_override("font_pressed_color", Color::WHITE);
            b.connect(
                "pressed",
                &Callable::from_object_method(&self.target, "on_xg_smoothing")
                    .bind(&[3i64.to_variant(), (*g as i64).to_variant()]),
            );
            b.connect(
                "gui_input",
                &Callable::from_object_method(&self.target, "on_chip_input")
                    .bind(&[(*g as i64).to_variant()]),
            );
            self.chips.add_child(&b);
            self.chip_buttons.push((*g, b));
        }
        self.chips.set_visible(!groups.is_empty());
        self.chip_hint.set_visible(groups.is_empty());
    }

    /// Shows the current group pressed (a click on a toggle chip flips it on its own).
    pub fn sync_chips(&mut self, current: u16) {
        for (g, b) in &mut self.chip_buttons {
            b.set_pressed_no_signal(*g == current);
        }
    }

    /// The chip's right-click menu (ids `cmd * 1000 + group`, handled by `on_xg_paint_menu`).
    pub fn open_chip_menu(&mut self, g: i64, at: Vector2) {
        use super::modeler_ui::paint::menu as m;
        self.chip_menu.clear();
        for (t, cmd) in [
            ("Select Faces", m::GROUP_SELECT),
            ("Merge into Selection's Group", m::GROUP_MERGE),
            ("Clear Group (faceted)", m::GROUP_CLEAR),
        ] {
            self.chip_menu
                .add_item_ex(t)
                .id((cmd * 1000 + g) as i32)
                .done();
        }
        self.chip_menu.reset_size();
        self.chip_menu
            .popup_ex()
            .rect(Rect2i::new(
                Vector2i::new(at.x as i32, at.y as i32),
                Vector2i::ZERO,
            ))
            .done();
    }

    /// Rebuilds the toolbar tooltips (the shortcuts may have been rebound).
    pub fn refresh_tooltips(&mut self) {
        for (a, b, mask) in &mut self.buttons {
            if *a == Action::Dissolve {
                continue; // set per mode by `sync`
            }
            if let Some(row) = BUTTONS.iter().find(|x| x.0 == *a) {
                b.set_tooltip_text(&format_tooltip(*a, row.2, *mask));
            }
        }
    }

    pub fn assign_shown(&mut self, v: bool, palette: bool) {
        self.rail.set_visible(v);
        self.peel.set_visible(v);
        self.palette.set_visible(v && palette);
    }

    pub fn controls(&self) -> (Gd<Control>, Gd<Control>, Gd<Control>) {
        (
            self.rail.clone().upcast(),
            self.peel.clone().upcast(),
            self.palette.clone().upcast(),
        )
    }
}

/// Whether the toolbar / menu control of action `a` (shown in `mask` modes) is visible now: the
/// MST toolbar's rule, also used by the alternate layout's builder-extras section.
pub fn action_visible(a: Action, mask: u8, st: &SyncState) -> bool {
    let (mode, tool, has_target) = (st.mode, st.tool, st.has_target);
    match a {
        Action::Convert => st.can_convert,
        Action::ExportObj
        | Action::ExportGltf
        | Action::Weld
        | Action::Flip
        | Action::Material
        | Action::ToolPlace
        | Action::Mirror
        | Action::BoolUnion
        | Action::BoolSubtract
        | Action::BoolIntersect
        | Action::ConformNormals
        | Action::BlendMaterial
        | Action::InvertAlpha
        | Action::SetCollider
        | Action::SetTrigger
        | Action::MergeObjects
        | Action::CenterPivot
        | Action::FreezeTransform
        | Action::ExportStl
        | Action::ExportPly
        | Action::ExportOptions
        | Action::ExportRes
        | Action::ReusablePart
        | Action::GenerateUv2
        | Action::PaintColour
        | Action::BakeMesh => has_target && (mask & mode.bit()) != 0,
        Action::UvEditor => has_target,
        Action::Preferences => true,
        Action::ShapeParams => st.has_spec && mode == Mode::Object,
        Action::GizmoMove
        | Action::GizmoRotate
        | Action::GizmoScale
        | Action::SoftSelect
        | Action::SnapVertex
        | Action::UvLock
        | Action::IndividualOrigins => {
            has_target && (mask & mode.bit()) != 0 && tool == Tool::Select
        }
        Action::ModeFace
        | Action::ModeEdge
        | Action::ModeVertex
        | Action::ToolPaint
        | Action::ToolVertexPaint => has_target,
        _ => (mask & mode.bit()) != 0,
    }
}

/// Pressed state of a toggle / radio action (`None` = a push action).
pub fn action_pressed(a: Action, v: &Values, st: &SyncState) -> Option<bool> {
    let (mode, tool) = (st.mode, st.tool);
    Some(match a {
        Action::ModeObject => mode == Mode::Object,
        Action::ModeFace => mode == Mode::Face,
        Action::ModeEdge => mode == Mode::Edge,
        Action::ModeVertex => mode == Mode::Vertex,
        Action::ToolSelect => tool == Tool::Select,
        Action::ToolShape => tool == Tool::Shape,
        Action::ToolPaint => tool == Tool::Paint,
        Action::ToolPlace => tool == Tool::Place,
        Action::ToolKnife => tool == Tool::Knife,
        Action::ToolVertexPaint => tool == Tool::VertexPaint,
        Action::ToolBezier => tool == Tool::Bezier,
        Action::IndividualOrigins => v.xg.individual_origins,
        Action::UvLock => st.uv_lock,
        Action::GizmoMove => st.xform == XformMode::Move,
        Action::GizmoRotate => st.xform == XformMode::Rotate,
        Action::GizmoScale => st.xform == XformMode::Scale,
        Action::SoftSelect => st.soft,
        Action::SnapVertex => st.snap_vertex,
        Action::ShapeParams => st.reshape,
        Action::SnapGrid => st.snap_grid,
        Action::SnapCell => st.snap_cell,
        Action::InsertVertex => st.insert_vert,
        Action::SurfaceDraw => st.surface_draw,
        Action::XRay => st.xray,
        Action::SetCollider => st.entity == 1,
        Action::SetTrigger => st.entity == 2,
        _ => return None,
    })
}

/// Whether strip field `f` applies to the current shape kind (sizes, segments, ...).
fn kind_ok(f: Field, spec: &ShapeSpec, xg: &XgOpts, mode: Mode, shape: bool) -> bool {
    let k = spec.sort;
    let (seg, steps, thick, _, rings) = k.uses();
    let (arc, door, profile, side) = k.uses_extra();
    let xu = k.uses_modeler();
    let band = xu.arch_band && spec.arch_band;
    let by_height = xu.step_height && spec.step_height > 0.0;
    let grid = k.is_grid_shape();
    match f {
        Field::SizeX | Field::SizeZ => !grid,
        Field::Segments => seg,
        Field::Steps => steps && !by_height,
        Field::SegmentsZ => xu.segments_z,
        Field::HeightCuts => xu.height_cuts,
        Field::TorusArc | Field::TubeArc => xu.torus_arcs,
        Field::ArchBand => xu.arch_band,
        Field::ArchDegrees | Field::EndCaps => band,
        Field::StepHeight => xu.step_height,
        Field::EqualSteps => xu.equal_steps && by_height,
        Field::Subdivisions => xu.subdivisions,
        Field::PolyHoles => k == ShapeKind::Poly,
        Field::OffsetPerFace => xg.offset_space == 2,
        Field::Thickness => thick,
        Field::Rings => rings,
        Field::Arc | Field::Inner => arc,
        Field::Opening | Field::DoorHeight => door,
        Field::Profile => profile || (!shape && mode == Mode::Face),
        Field::Side => side,
        _ => true,
    }
}

/// Whether strip field `f` is shown now: the one visibility rule of the MST strip, the alternate
/// layout's Tool Options popup and the Paint popover's vertex-paint rows.
pub fn field_visible(f: Field, v: &Values, st: &SyncState) -> bool {
    let shape = st.shape_fields();
    if !(kind_ok(f, v.spec, &v.xg, st.mode, shape) && f.shown(st.mode, st.tool, shape)) {
        return false;
    }
    let brush_only = matches!(
        f,
        Field::PaintRadius | Field::PaintStrength | Field::PaintSoftness
    );
    !((matches!(f, Field::SoftRadius | Field::Falloff) && !st.soft)
        || (f == Field::RotSnap && st.xform != XformMode::Rotate)
        || (brush_only && v.vp_fill)
        || (f == Field::PaintAlpha && v.vp_channel != 2)
        || (f == Field::BzHeight && !v.bz.block)
        || (matches!(f, Field::BzRoll | Field::BzTangent) && !st.bz_knot)
        || (f == Field::InsetIndividual && v.opts.inset_mode != 1))
}

/// Strip fields a drawing tool shows (alternate layout: the HUD's Tool Options popup): the fields
/// `Field::shown` lists for the Shape (or reshape) and Bezier tools.
pub fn is_tool_field(f: Field) -> bool {
    !matches!(f, Field::Slot | Field::GridStep)
        && [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex]
            .iter()
            .any(|m| f.shown(*m, Tool::Shape, true) || f.shown(*m, Tool::Bezier, false))
}

/// Representative color of a material (albedo of standard materials, `albedo` parameter of
/// shader materials).
pub fn material_color(m: &Gd<Material>) -> Option<Color> {
    if let Ok(s) = m.clone().try_cast::<godot::classes::BaseMaterial3D>() {
        return Some(s.get_albedo());
    }
    if let Ok(s) = m.clone().try_cast::<godot::classes::ShaderMaterial>() {
        for p in ["albedo", "albedo_color", "color"] {
            if let Ok(c) = s.get_shader_parameter(p).try_to::<Color>() {
                return Some(c);
            }
        }
    }
    None
}
