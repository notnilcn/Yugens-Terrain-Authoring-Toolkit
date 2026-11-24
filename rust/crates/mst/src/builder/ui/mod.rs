use crate::editor::ui_kit;
use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, ButtonGroup, CheckBox, ColorPickerButton, Control, EditorResourcePicker, GridContainer,
    HBoxContainer, HFlowContainer, HSeparator, Label, Material, MenuButton, OptionButton,
    PopupMenu, ScrollContainer, SpinBox, Texture2D, VBoxContainer, VSeparator,
};
use godot::prelude::*;
use pt_modeler::{ShapeKind, ShapeSpec};
const ICONS: &str = "res://addons/MarchingSquaresTerrain/editor/icons/builder/";
mod mst_layout;
pub use mst_layout::{FieldCtl, Ui, assemble, make_field};
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
    #[default]
    Select,
    Shape,
    Paint,
    Place,
    Knife,
    VertexPaint,
    Bezier,
    EditPivot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum XformMode {
    #[default]
    Move,
    Rotate,
    Scale,
    Combined,
    Pick,
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
            Tool::EditPivot => "Edit pivot",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Action {
    ModeObject = 0,
    ModeFace = 1,
    ModeEdge = 2,
    ModeVertex = 3,
    ToolSelect = 4,
    ToolShape = 5,
    ToolPaint = 6,
    ToolPlace = 7,
    Extrude = 8,
    Inset = 9,
    Bevel = 10,
    Subdivide = 11,
    Merge = 12,
    Connect = 13,
    Fill = 14,
    Detach = 15,
    Duplicate = 16,
    Rotate = 17,
    Flip = 18,
    Delete = 19,
    Weld = 20,
    Grow = 21,
    Coplanar = 22,
    Similar = 23,
    Material = 24,
    SnapGrid = 25,
    SnapCell = 26,
    ExportObj = 27,
    ExportGltf = 28,
    Convert = 29,
    GizmoMove = 30,
    GizmoRotate = 31,
    GizmoScale = 32,
    SoftSelect = 33,
    SnapVertex = 34,
    InsertLoop = 35,
    Bridge = 36,
    SelectLoop = 37,
    Shrink = 38,
    Holes = 39,
    Mirror = 40,
    ToolKnife = 41,
    ShapeParams = 42,
    BoolUnion = 43,
    BoolSubtract = 44,
    BoolIntersect = 45,
    UvEditor = 46,
    ConformNormals = 47,
    Preferences = 48,
    ToolVertexPaint = 49,
    BlendMaterial = 50,
    InvertAlpha = 51,
    UvLock = 52,
    BakeMesh = 53,
    SubdivideEdges = 54,
    ConnectEdges = 55,
    FlipEdge = 56,
    SplitVerts = 57,
    Triangulate = 58,
    MergeObjects = 59,
    CenterPivot = 60,
    PivotToSelection = 61,
    FreezeTransform = 62,
    Collapse = 63,
    InsertVertex = 64,
    TrimWalls = 65,
    SurfaceDraw = 66,
    SetCollider = 67,
    SetTrigger = 68,
    XRay = 69,
    Dissolve = 70,
    OffsetElements = 71,
    InvertSelection = 72,
    SelectByColor = 73,
    SelectBySmoothing = 74,
    ToolBezier = 75,
    ReusablePart = 76,
    IndividualOrigins = 77,
    ExportStl = 78,
    ExportPly = 79,
    ExportOptions = 80,
    SelectAll = 81,
    ClearSelection = 82,
    SelectRing = 83,
    GenerateUv2 = 84,
    RotateNeg = 85,
    PaintColour = 86,
    ExportRes = 87,
    RobloxSelect = 88,
    GizmoTransform = 89,
    FaceResize = 90,
    ToolEditPivot = 91,
    PivotSnap = 92,
    ToggleHandleSpace = 93,
    ToggleLock = 94,
    ClickToLock = 95,
    Collisions = 96,
    RotSnapToggle = 97,
    Negate = 98,
    RobloxUnion = 99,
    RobloxIntersect = 100,
    Separate = 101,
    InsertPartAtView = 102,
    GroupSelection = 103,
    GroupAsFolder = 104,
    Ungroup = 105,
    SelectChildren = 106,
    ToggleAnchor = 107,
    ClickToAnchor = 108,
    AlignPanel = 109,
    MaterialPicker = 110,
    ColorPicker = 111,
    ClickToPaint = 112,
    SnapBoundingBoxes = 113,
    GridPlane = 114,
    DuplicateObject = 115,
    DeleteObject = 116,
    ZoomToSelection = 117,
}
pub const O: u8 = 1;
pub const F: u8 = 2;
pub const E: u8 = 4;
pub const V: u8 = 8;
pub const ALL: u8 = O | F | E | V;
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
        "Draw shape [C]: drag the footprint on the grid / terrain cells, move up for the height, click to finish\nA plain click stamps the shape at its current size; Shift appends to the selected mesh\nRight-click / Esc cancels",
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
        "Fill hole [F]: close open boundaries through the selection\nEntire hole off (strip): only the selected run of border vertices, closed by its chord",
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
pub const ROBLOX_ACTIONS: &[(Action, &str, &str, u8)] = &[
    (
        Action::RobloxSelect,
        "select",
        "Select [1]: pick parts in the viewport with no handles on them",
        ALL,
    ),
    (
        Action::GizmoTransform,
        "transform",
        "Transform [5]: move, resize and rotate with one combined set of handles",
        ALL,
    ),
    (
        Action::FaceResize,
        "scale",
        "Resize one side: drag a face ball to move only that face (Ctrl: both sides)",
        ALL,
    ),
    (
        Action::ToolEditPivot,
        "pivot",
        "Pivot: move and rotate the pivot of the part without moving the part",
        O,
    ),
    (
        Action::PivotSnap,
        "pivot",
        "Snap pivot: snap the pivot to the grid while editing it",
        O,
    ),
    (
        Action::ToggleHandleSpace,
        "move",
        "Local / world handles [Ctrl+L]: align the handles with the part or with the world",
        ALL,
    ),
    (
        Action::ToggleLock,
        "lock",
        "Lock: locked parts cannot be selected or moved in the viewport",
        O,
    ),
    (
        Action::ClickToLock,
        "lock",
        "Click object to lock: each part clicked in the viewport is locked or unlocked",
        O,
    ),
    (
        Action::Collisions,
        "collisions",
        "Collisions: a dragged part stops at the first part it would overlap",
        ALL,
    ),
    (
        Action::RotSnapToggle,
        "snap_rotate",
        "Rotate snap: turn the rotation step on or off",
        ALL,
    ),
    (
        Action::Negate,
        "negate",
        "Negate [Ctrl+Shift+N]: mark the part as a hole that a later Union cuts away",
        O,
    ),
    (
        Action::RobloxUnion,
        "union",
        "Union [Ctrl+Shift+G]: join the selected parts into one; negated parts are cut away",
        O,
    ),
    (
        Action::RobloxIntersect,
        "intersect",
        "Intersect [Ctrl+Shift+I]: keep only the volume the selected parts share",
        O,
    ),
    (
        Action::Separate,
        "separate",
        "Separate [Ctrl+Shift+U]: split a union or intersection back into its parts",
        O,
    ),
    (
        Action::InsertPartAtView,
        "part",
        "Insert part: add a block in front of the camera, on top of what it looks at",
        ALL,
    ),
    (
        Action::GroupSelection,
        "group",
        "Group as Model [Ctrl+G]: put the selected parts under a new Model node",
        ALL,
    ),
    (
        Action::GroupAsFolder,
        "group",
        "Group as Folder [Ctrl+Alt+G]: put the selected parts under a new Folder node",
        ALL,
    ),
    (
        Action::Ungroup,
        "ungroup",
        "Ungroup [Ctrl+U]: move the children of the selected group up and remove the group",
        ALL,
    ),
    (
        Action::SelectChildren,
        "group",
        "Select children: select every child of the selected node",
        ALL,
    ),
    (
        Action::ToggleAnchor,
        "anchor",
        "Anchor: seat the part on its terrain cell so it follows the terrain
(the terrain anchor of this addon, not a physics anchor)",
        O,
    ),
    (
        Action::ClickToAnchor,
        "anchor",
        "Click object to anchor: each part clicked in the viewport is anchored or released",
        O,
    ),
    (
        Action::AlignPanel,
        "align",
        "Align: line the selected parts up on their minimum, centre or maximum along X, Y and Z",
        ALL,
    ),
    (
        Action::MaterialPicker,
        "material",
        "Material: browse the materials and apply one to the selection",
        O | F,
    ),
    (
        Action::ColorPicker,
        "color",
        "Color: pick a color from the palette and apply it to the selection",
        O | F,
    ),
    (
        Action::ClickToPaint,
        "color",
        "Click object to apply color: each part clicked in the viewport gets the current color; Shift-click paints only the flat area under the cursor, Ctrl-click picks that face's color",
        O,
    ),
    (
        Action::SnapBoundingBoxes,
        "snap",
        "Use Bounding Boxes: a moved part lands with the edge of its box on the move grid, not its pivot",
        ALL,
    ),
    (
        Action::GridPlane,
        "transform",
        "Grid plane: show or hide the drag grid under the part while the Transform tool is active",
        ALL,
    ),
    (
        Action::DuplicateObject,
        "part",
        "Duplicate: copy the selected parts next to the originals and select the copies",
        O,
    ),
    (
        Action::DeleteObject,
        "remove",
        "Delete: remove the selected parts from the scene",
        O,
    ),
    (
        Action::ZoomToSelection,
        "select",
        "Zoom to: frame the selected parts in the viewport (the 3D editor's Focus Selection, F)",
        O,
    ),
];
fn action_row(a: Action) -> Option<(&'static str, u8)> {
    BUTTONS
        .iter()
        .find(|b| b.0 == a)
        .map(|b| (b.2, b.4))
        .or_else(|| MENU_ACTIONS.iter().find(|m| m.0 == a).map(|m| (m.1, m.2)))
        .or_else(|| ROBLOX_ACTIONS.iter().find(|r| r.0 == a).map(|r| (r.2, r.3)))
}
pub fn toolbar_actions() -> impl Iterator<Item = (Action, u8)> {
    BUTTONS.iter().map(|b| (b.0, b.4))
}
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
            .chain(ROBLOX_ACTIONS.iter().map(|r| r.0))
            .find(|a| *a as i64 == v)
    }
    pub fn mask(self) -> u8 {
        action_row(self).map(|r| r.1).unwrap_or(0)
    }
    pub fn heading(self) -> String {
        let t = action_row(self).map(|r| r.0).unwrap_or("");
        let first = t.lines().next().unwrap_or("");
        strip_key(first.split(": ").next().unwrap_or("")).to_string()
    }
}
fn strip_key(title: &str) -> &str {
    match title.rfind(" [") {
        Some(i) if title.ends_with(']') => &title[..i],
        _ => title,
    }
}
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
pub const PALETTE_LOAD: i64 = 0;
pub const PALETTE_SAVE: i64 = 1;
pub const PALETTE_RESET: i64 = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Field {
    SizeX = 0,
    SizeY = 1,
    SizeZ = 2,
    Segments = 3,
    Steps = 4,
    Thickness = 5,
    Rings = 6,
    Slot = 7,
    GridStep = 8,
    Distance = 9,
    Inset = 10,
    Bevel = 11,
    Cuts = 12,
    BevelSegs = 13,
    SoftRadius = 14,
    Falloff = 15,
    RotSnap = 16,
    MirrorAxis = 17,
    Arc = 18,
    Inner = 19,
    Opening = 20,
    DoorHeight = 21,
    Profile = 22,
    Side = 23,
    PaintApply = 24,
    PaintChannel = 25,
    PaintRadius = 26,
    PaintStrength = 27,
    PaintSoftness = 28,
    PaintAlpha = 29,
    InsetMode = 30,
    SubdivSegs = 31,
    GizmoOrient = 32,
    TrimPlace = 33,
    TrimDepth = 34,
    TrimHeight = 35,
    OffsetX = 36,
    OffsetY = 37,
    OffsetZ = 38,
    OffsetSpace = 39,
    OffsetPerFace = 40,
    GrowAngle = 41,
    ExtrudeMethod = 42,
    EdgesGroup = 43,
    InsetIndividual = 44,
    CollapseFirst = 45,
    WeldDist = 46,
    FillEntire = 47,
    MirrorX = 48,
    MirrorY = 49,
    MirrorZ = 50,
    MirrorDup = 51,
    ToNewObject = 52,
    SegmentsZ = 53,
    HeightCuts = 54,
    TorusArc = 55,
    TubeArc = 56,
    ArchBand = 57,
    ArchDegrees = 58,
    EndCaps = 59,
    StepHeight = 60,
    EqualSteps = 61,
    Subdivisions = 62,
    PolyHoles = 63,
    BzKind = 64,
    BzClosed = 65,
    BzSmooth = 66,
    BzWidth = 67,
    BzRows = 68,
    BzColumns = 69,
    BzHeight = 70,
    BzTangent = 71,
    BzRoll = 72,
    BevelAmountType = 73,
    BevelProfile = 74,
    BevelClamp = 75,
    BevelHarden = 76,
    SmoothTint = 77,
    ShowNormals = 78,
    NormalLength = 79,
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
            Field::Profile => Some(&pt_modeler::prims::TrimSection::NAMES),
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
            Field::BevelAmountType => Some(&pt_modeler::xg_bevel::AmountType::NAMES),
            _ => None,
        }
    }
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
            Field::MirrorAxis => false,
            Field::Profile => shape || mode == Mode::Face,
            Field::Arc | Field::Inner | Field::Opening | Field::DoorHeight | Field::Side => shape,
            Field::InsetMode | Field::TrimPlace | Field::TrimDepth | Field::TrimHeight => {
                !shape && mode == Mode::Face
            }
            Field::SubdivSegs => !shape && matches!(mode, Mode::Face | Mode::Object),
            Field::GizmoOrient => !shape && mode != Mode::Object && tool == Tool::Select,
            Field::SmoothTint | Field::ShowNormals | Field::NormalLength => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum UvField {
    OffsetU = 0,
    OffsetV = 1,
    ScaleU = 2,
    ScaleV = 3,
    Rotation = 4,
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
const ICON_PX: f32 = 24.0;
pub fn action_glyph(a: Action) -> Option<Gd<Texture2D>> {
    BUTTONS.iter().find(|b| b.0 == a).and_then(|b| glyph(b.1))
}
pub fn roblox_icon_name(a: Action) -> Option<&'static str> {
    ROBLOX_ACTIONS.iter().find(|r| r.0 == a).map(|r| r.1)
}
pub fn roblox_tooltip(a: Action) -> String {
    let base = action_row(a)
        .map(|(tip, mask)| format_tooltip(a, tip, mask))
        .unwrap_or_default();
    let Some(e) = super::shortcuts::entry_of_action_in(a, true) else {
        return base;
    };
    let key = super::shortcuts::key_text(e.id);
    let (first, rest) = base.split_once('\n').unwrap_or((base.as_str(), ""));
    let title = strip_key(first);
    let title = if key.is_empty() {
        title.to_string()
    } else {
        format!("{title} [{key}]")
    };
    if rest.is_empty() {
        title
    } else {
        format!("{title}\n{rest}")
    }
}
fn glyph(name: &str) -> Option<Gd<Texture2D>> {
    mono_icon(&format!("{ICONS}{name}.svg"), ICON_PX / 16.0)
        .or_else(|| try_load::<Texture2D>(&format!("{ICONS}{name}.svg")).ok())
}
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
pub struct Values<'a> {
    pub spec: &'a ShapeSpec,
    pub pocket: i64,
    pub grid_step: f32,
    pub span: f32,
    pub inset: f32,
    pub chamfer: f32,
    pub cuts: i64,
    pub tint: Color,
    pub tex_coord: [f32; 5],
    pub uv_world: bool,
    pub bevel_segs: i64,
    pub soft_radius: f32,
    pub falloff: i64,
    pub rot_snap: f32,
    pub mirror_axis: i64,
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
    pub individual_origins: bool,
    pub bevel_type: i64,
    pub bevel_profile: f32,
    pub bevel_clamp: bool,
    pub bevel_harden: bool,
    pub smooth_tint: bool,
    pub show_normals: bool,
    pub normal_length: f32,
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
    pub xray: bool,
    pub entity: i32,
    pub bz_knot: bool,
    pub visible: bool,
    pub can_create: bool,
    pub sel_faces: usize,
    pub message: String,
    pub tool_detail: String,
    pub prefs: super::prefs::Prefs,
    pub adjusting: bool,
    pub live_uv2: bool,
    pub clear_grass: bool,
    pub face_slot: Option<i64>,
}
impl SyncState {
    pub fn shape_fields(&self) -> bool {
        (self.tool == Tool::Shape || self.reshape) && self.tool != Tool::Bezier
    }
}
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
        ShapeKind::CornerWedge => "shape_corner_wedge",
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
pub fn action_tooltip(a: Action) -> String {
    action_row(a)
        .map(|r| format_tooltip(a, r.0, r.1))
        .unwrap_or_default()
}
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
        Action::RobloxSelect => tool == Tool::Select && st.xform == XformMode::Pick,
        Action::GizmoTransform => tool == Tool::Select && st.xform == XformMode::Combined,
        Action::ToolEditPivot => tool == Tool::EditPivot,
        _ => return None,
    })
}
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
pub fn is_tool_field(f: Field) -> bool {
    !matches!(f, Field::Slot | Field::GridStep)
        && [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex]
            .iter()
            .any(|m| f.shown(*m, Tool::Shape, true) || f.shown(*m, Tool::Bezier, false))
}
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
