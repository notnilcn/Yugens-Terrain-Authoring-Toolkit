//! Builder UI, in the MarchingSquaresTerrain editor style: a left icon toolbar (modes, tools,
//! the operations of the current mode, snapping, import/export), a bottom attribute strip
//! (shape parameters, color, grid step, operation amounts) and a right material palette with
//! the face UV controls.

use godot::classes::control::{FocusMode, SizeFlags};
use godot::classes::{
    Button, ButtonGroup, CheckBox, ColorPickerButton, Control, EditorResourcePicker, GridContainer, HBoxContainer, HFlowContainer,
    HSeparator, Label, Material, OptionButton, ScrollContainer, SpinBox, Texture2D, VBoxContainer, VFlowContainer, VSeparator,
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
    pub fn name(self) -> &'static str {
        match self {
            Mode::Object => "Object",
            Mode::Face => "Face",
            Mode::Edge => "Edge",
            Mode::Vertex => "Vertex",
        }
    }
    fn bit(self) -> u8 {
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
    /// Brush / fill per-corner vertex colors (Unibuilder's vertex paint mode).
    VertexPaint,
}

/// What the element handles do in the Select tool (Unibuilder's W / E / R gizmo modes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum XformMode {
    #[default]
    Move,
    Rotate,
    Scale,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select / Move",
            Tool::Shape => "Draw shape",
            Tool::Paint => "Paint material",
            Tool::Place => "Place on cell",
            Tool::Knife => "Knife",
            Tool::VertexPaint => "Vertex paint",
        }
    }
}

/// Toolbar buttons (bound to `on_action`).
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
}

const O: u8 = 1;
const F: u8 = 2;
const E: u8 = 4;
const V: u8 = 8;
const ALL: u8 = O | F | E | V;

/// (action, icon, tooltip, radio group: 1 = modes, 2 = tools, 3 = toggle, 0 = push, modes shown)
const BUTTONS: &[(Action, &str, &str, u8, u8)] = &[
    (Action::ModeObject, "mode_object", "Object mode [1]: whole meshes (use Godot's gizmo to move them)", 1, ALL),
    (Action::ModeFace, "mode_face", "Face mode [2]: click faces (Shift adds), drag on empty space for a box", 1, ALL),
    (Action::ModeEdge, "mode_edge", "Edge mode [3]: pick edges", 1, ALL),
    (Action::ModeVertex, "mode_vertex", "Vertex mode [4]: pick vertices", 1, ALL),
    (Action::ToolSelect, "move", "Select / move [W]: drag the axis handles (centre = view plane)\nShift+drag a handle extrudes faces / edges first", 2, ALL),
    (Action::ToolShape, "shape", "Draw shape [C]: drag the footprint on the grid / terrain cells, move up for the height, click to finish\nA plain click stamps the shape at its current size; Shift appends to the selected mesh\nRight-click / Esc cancels", 2, ALL),
    (Action::ToolPaint, "paint", "Paint [B]: click or drag over faces to give them the current material slot and color\nShift+click fills the coplanar region, Ctrl+click picks the slot of a face", 2, ALL),
    (Action::ToolVertexPaint, "vertex_paint", "Vertex paint [5]: brush per-corner vertex colors (Radius, Strength, Softness) or Fill whole faces\nChannel RGB / Alpha paints only the color or only the blend mask of a blend material\nShift+click picks the color under a corner, Esc leaves the tool", 2, ALL),
    (Action::ToolPlace, "place_cell", "Place on cell [P]: click a terrain cell to seat the selected mesh on it", 2, O),
    (Action::ToolKnife, "cut", "Knife [K]: click a face, then click points on it (snaps to its vertices / edges and the grid)\nClick the first point, Enter or double-click to cut the polygon in as its own face\nBackspace removes the last point, Esc / right-click cancels", 2, F | E | V),
    (Action::GizmoMove, "move", "Move handles [Shift+W]: drag an axis arrow, the centre square moves in the view plane\nHold V while dragging to snap onto the vertex under the cursor", 7, F | E | V),
    (Action::GizmoRotate, "gizmo_rotate", "Rotate handles [Shift+E]: drag a ring to turn the selection about its centre\n(snaps to the Rot° step while grid snapping is on)", 7, F | E | V),
    (Action::GizmoScale, "gizmo_scale", "Scale handles [Shift+R]: drag an axis handle, or the centre square for uniform scale\n(snaps to 0.1 steps while grid snapping is on)", 7, F | E | V),
    (Action::SoftSelect, "prop_smooth", "Soft selection [O]: moves, rotations and scales pull nearby vertices along\nwithin Radius, weighted by the Falloff curve", 3, F | E | V),
    (Action::SnapVertex, "snap", "Vertex snap: moves always snap onto the vertex under the cursor\n(hold V for a one-off snap while this is off)", 3, F | E | V),
    (Action::UvLock, "uv_lock", "UV lock: textures stay put while faces are moved, rotated or scaled\nManual UVs ride along rigid moves and are refitted when faces stretch;\nautomatic UVs keep their projection when a face tilts", 3, F | E | V),
    (Action::Extrude, "extrude", "Extrude [E]: faces along their normals, open edges outwards, by Dist", 0, F | E),
    (Action::Inset, "inset", "Inset [I]: shrink the selected faces into their own faces by Inset\nInset by: Fraction (towards the centre), Width or Equal (a rim Inset metres wide, mitred corners)", 0, F),
    (Action::Bevel, "bevel", "Bevel the selected edges by Bevel over Segs strips [Ctrl+B]\n1 segment chamfers, more round the edge (smooth shaded)", 0, E),
    (Action::InsertLoop, "insert_loop", "Insert edge loop [Ctrl+R]: Cuts loops across the quad ring through the selected edge", 0, E),
    (Action::Bridge, "bridge", "Bridge: join two open (boundary) edges, or two equal open edge runs, with faces", 0, E),
    (Action::Subdivide, "subdivide", "Subdivide: split the selected faces into quads, Subdiv pieces along every edge\n(concave faces use their kernel centre); Object mode: every face of the mesh", 0, O | F),
    (Action::SubdivideEdges, "subdiv_edges", "Subdivide edges: split each selected edge into Cuts+1 equal pieces\n(manual UVs and vertex paint are interpolated along the edge)", 0, E),
    (Action::Triangulate, "triangulate", "Triangulate [Ctrl+T]: split the selected faces into triangles\nObject mode: every face of the mesh", 0, O | F),
    (Action::FlipEdge, "flip_edge", "Flip face edge: turn the triangulation diagonal of the selected quads\n(rotates each face's corner loop by one)", 0, F),
    (Action::Merge, "merge", "Merge [M]: faces into one n-gon / vertices into one", 0, F | V),
    (Action::Collapse, "collapse", "Collapse vertices: merge the selected vertices into one at their centre\n(refused when it would fold an edge onto a third face; paint and UVs stay on their corners)", 0, V),
    (Action::InsertVertex, "insert_vert", "Insert vertex [Shift+V]: click an edge to split it at the cursor\n(stays on until toggled off; Esc leaves it)", 3, E),
    (Action::TrimWalls, "shape_trim_walls", "Trim walls: sweep the trim Profile (T depth x T height) along the base\n(or under the top) of the selected wall faces", 0, F),
    (Action::Connect, "connect", "Connect [J]: split faces between selected vertices", 0, V),
    (Action::ConnectEdges, "connect_edges", "Connect edges [J]: split each face that has two selected edges\nwith a new edge between their midpoints", 0, E),
    (Action::SplitVerts, "split", "Split vertices: give every face touching a selected vertex its own copy\n(the faces can then be moved apart)", 0, V),
    (Action::Fill, "fill", "Fill hole [F]: close open boundaries through the selection", 0, E | V),
    (Action::Detach, "detach", "Detach: split the selected faces from their neighbours", 0, F),
    (Action::Duplicate, "duplicate", "Duplicate [Ctrl+D]: copy the selected faces", 0, F),
    (Action::Rotate, "rotate", "Rotate 90° [R]: turn the selection around Y (Shift+R: -90°)", 0, F | E | V),
    (Action::Flip, "flip", "Flip normals: of the selected faces (all faces in Object mode)", 0, O | F),
    (Action::Delete, "delete", "Delete [Del]: remove the selected faces / edges / vertices", 0, F | E | V),
    (Action::Weld, "weld", "Weld: merge coincident vertices (the selected ones in Vertex mode)", 0, O | V),
    (Action::Grow, "grow", "Grow selection [G]: add the neighbours of the selection", 0, F | E | V),
    (Action::Shrink, "shrink", "Shrink selection [Shift+G]: drop the border of the selection", 0, F | E | V),
    (Action::SelectLoop, "select_loop", "Select loop [L]: the edge loop through the selected edges (Alt+L: edge ring)\nFace mode: the quad loop. Double-click an edge / face does the same", 0, F | E),
    (Action::Holes, "holes", "Select holes: every open (boundary) edge", 0, E),
    (Action::Coplanar, "coplanar", "Select coplanar: faces in the plane of the selection and connected to it", 0, F),
    (Action::Similar, "similar", "Select similar: faces with the same material slot", 0, F),
    (Action::Material, "material", "Assign material: give the selected faces the material slot and color (all faces in Object mode)", 0, O | F),
    (Action::BlendMaterial, "blend_material", "New blend material: give the selected faces (all in Object mode) a material that blends two layers\nby the painted vertex alpha (layer A starts from their current slot). Paint the mask with Vertex paint, Channel Alpha", 0, O | F),
    (Action::InvertAlpha, "invert", "Invert alpha: swap the two blend layers on the selected faces (all faces in Object mode)", 0, O | F),
    (Action::SnapGrid, "snap_grid", "Grid snap: moves and shapes snap to the Grid step ([ / ] halve / double it)", 3, ALL),
    (Action::SnapCell, "snap_cell", "Cell snap: placement, shapes and vertices snap to terrain cells (centres / corners)", 3, ALL),
    (Action::SurfaceDraw, "surface_draw", "Draw on surface: shapes start on the builder face under the cursor\nand stand along its normal (walls, poly shapes and floors on any slope)", 3, ALL),
    (Action::XRay, "xray", "X-ray: box selection and clicked vertices / edges also take elements hidden\nbehind other faces (off: only what the camera sees, like Unibuilder)", 3, F | E | V),
    (Action::ExportObj, "export_obj", "Export OBJ: the mesh as Wavefront OBJ (+ MTL)", 0, O),
    (Action::ExportGltf, "export_gltf", "Export glTF: the mesh as .gltf / .glb", 0, O),
    (Action::BakeMesh, "bake_mesh", "Bake to MeshInstance3D: replace the builder mesh by a plain MeshInstance3D (+ static collider)\nwith lightmap UV2 unwrapped at the Lightmap texel of the preferences (undoable)", 0, O),
    (Action::Convert, "convert", "Convert: turn the selected MeshInstance3D into an editable builder mesh", 0, O),
    (Action::Mirror, "mirror", "Mirror: add a copy mirrored across the mesh's local Axis plane (seam welded)", 0, O),
    (Action::ShapeParams, "shape_params", "Shape parameters: reopen the recipe this mesh was drawn from;\nthe strip's shape fields then rebuild it live (each change one undo step)", 3, O),
    (Action::BoolUnion, "bool_union", "Boolean union: merge the other selected builder mesh into this one (it is removed)\nCtrl+click it in the Scene dock to add it to the selection", 0, O),
    (Action::BoolSubtract, "bool_subtract", "Boolean subtract: carve the other selected builder mesh out of this one", 0, O),
    (Action::BoolIntersect, "bool_intersect", "Boolean intersect: keep only the volume shared with the other selected builder mesh", 0, O),
    (Action::CenterPivot, "center_pivot", "Center pivot: move the node origin to the centre of the mesh bounds\n(the mesh stays where it is)", 0, O),
    (Action::PivotToSelection, "pivot_selection", "Pivot to selection: move the node origin to the centre of the selected elements", 0, F | E | V),
    (Action::FreezeTransform, "freeze_transform", "Freeze transform: bake the node's rotation and scale (and position) into the mesh\nand reset the transform; children stay in place", 0, O),
    (Action::MergeObjects, "merge_objects", "Merge objects: append the other selected builder meshes into this one (they are removed)\nCtrl+click them in the Scene dock to add them to the selection", 0, O),
    (Action::SetCollider, "set_collider", "Collider: the mesh becomes an invisible static collider in the game (orange in the editor)\nPress again for a normal mesh", 3, O),
    (Action::SetTrigger, "set_trigger", "Trigger: the mesh becomes an invisible Area3D trigger volume in the game (blue in the editor)\nPress again for a normal mesh", 3, O),
    (Action::ConformNormals, "flip", "Conform normals: make the face windings consistent in every connected part\n(the majority orientation of each part wins)", 0, O),
    (Action::UvEditor, "uv_editor", "UV editor [U]: open the UV window of this mesh (manual UVs: move / scale / rotate,\nplanar / box projection, flip, fit, sew, copy / paste, texel density)", 0, ALL),
    (Action::Preferences, "tools", "Preferences: handle and vertex sizes, overlay colors, default snap steps, shortcuts", 0, ALL),
];

/// Toolbar sections: (header, tooltip). Index = [`section_of`].
const SECTIONS: [(&str, &str); 11] = [
    ("Mode", "Selection mode: Object [1], Face [2], Edge [3], Vertex [4]"),
    ("Tool", "Viewport tool: Select / move [W], Draw shape [C], Paint [B], Vertex paint [5], Place on cell [P], Knife [K]"),
    ("Handles", "What the Select tool's handles do: move [Shift+W], rotate [Shift+E], scale [Shift+R]; soft selection [O], vertex snap, UV lock"),
    ("Edit", "Operations on the selected faces / edges / vertices of the current mode"),
    ("Select", "Grow, shrink and select loops, holes, coplanar or same-material faces; assign the material"),
    ("Snap", "Grid ([ / ] halve / double the step), terrain-cell snapping and drawing on builder faces"),
    ("Object", "Whole-mesh operations: triangulate / subdivide, mirror, pivot, freeze, conform normals, shape recipe, collider / trigger"),
    ("Combine", "Boolean operations with a second selected builder mesh; merge other builder meshes into this one"),
    ("Panels", "UV editor window and builder preferences"),
    ("Blend", "Blend materials: a new two-layer blend material for the faces, invert the painted mask"),
    ("Export", "Export OBJ / glTF, bake to a plain MeshInstance3D, convert a MeshInstance3D"),
];

fn section_of(a: Action, group: u8) -> usize {
    match a {
        Action::Grow | Action::Coplanar | Action::Similar | Action::Material | Action::Shrink | Action::SelectLoop | Action::Holes => 4,
        Action::ExportObj | Action::ExportGltf | Action::Convert | Action::BakeMesh => 10,
        Action::Mirror | Action::ShapeParams | Action::ConformNormals | Action::SetCollider | Action::SetTrigger => 6,
        Action::UvEditor | Action::Preferences => 8,
        Action::BoolUnion | Action::BoolSubtract | Action::BoolIntersect | Action::MergeObjects => 7,
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
fn format_tooltip(tip: &str, mask: u8) -> String {
    let (first, rest) = tip.split_once('\n').unwrap_or((tip, ""));
    let (title, body) = match first.split_once(": ") {
        Some((t, b)) => (t.to_string(), b.to_string()),
        None => (first.to_string(), String::new()),
    };
    let mut out = title;
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
        let modes: Vec<&str> = [Mode::Object, Mode::Face, Mode::Edge, Mode::Vertex].iter().filter(|m| mask & m.bit() != 0).map(|m| m.name()).collect();
        out += &format!("\n\n[{} mode{}]", modes.join(" / "), if modes.len() == 1 { "" } else { "s" });
    }
    out
}

impl Action {
    pub fn from_i64(v: i64) -> Option<Action> {
        BUTTONS.iter().map(|b| b.0).find(|a| *a as i64 == v)
    }
}

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
}

impl Field {
    pub const ALL: [Field; 36] = [
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
    ];
    pub fn from_i64(v: i64) -> Option<Field> {
        Self::ALL.iter().copied().find(|f| *f as i64 == v)
    }
    /// (label, min, max, step, tooltip)
    fn describe(self) -> (&'static str, f64, f64, f64, &'static str) {
        match self {
            Field::SizeX => ("W", 0.05, 256.0, 0.05, "Shape width (x). Hex shapes use the terrain cell size."),
            Field::SizeY => ("H", 0.05, 256.0, 0.05, "Shape height (y)"),
            Field::SizeZ => ("D", 0.05, 256.0, 0.05, "Shape depth (z)"),
            Field::Segments => ("Seg", 1.0, 128.0, 1.0, "Radial segments / subdivisions"),
            Field::Steps => ("Steps", 1.0, 128.0, 1.0, "Stair steps / sphere rings"),
            Field::Thickness => ("Thick", 0.01, 64.0, 0.01, "Wall thickness (arch, pipe)"),
            Field::Rings => ("Rings", 0.0, 16.0, 1.0, "Hex cluster radius in cells"),
            Field::Slot => ("Slot", 0.0, 63.0, 1.0, "Material slot for new shapes, painting and the Material button"),
            Field::GridStep => ("Grid", 0.0625, 64.0, 0.0625, "Grid step for snapping ([ / ] halve / double)"),
            Field::Distance => ("Dist", -64.0, 64.0, 0.05, "Extrude distance"),
            Field::Inset => ("Inset", 0.01, 64.0, 0.01, "Inset amount: fraction towards the face centre (Fraction), or the rim width in metres (Width / Equal)"),
            Field::Bevel => ("Bevel", 0.01, 64.0, 0.01, "Bevel width"),
            Field::Cuts => ("Cuts", 1.0, 16.0, 1.0, "Edge subdivision cuts / inserted edge loops"),
            Field::BevelSegs => ("Segs", 1.0, 8.0, 1.0, "Bevel segments (1 = chamfer, more = rounded)"),
            Field::SoftRadius => ("Radius", 0.05, 64.0, 0.05, "Soft selection radius (world units)"),
            Field::Falloff => ("Falloff", 0.0, 6.0, 1.0, "Soft selection falloff curve"),
            Field::RotSnap => ("Rot°", 0.0, 90.0, 1.0, "Rotate handle snap step in degrees (with grid snapping on; 0 = free)"),
            Field::MirrorAxis => ("Axis", 0.0, 2.0, 1.0, "Mirror plane: the mesh's local X, Y or Z = 0 plane"),
            Field::Arc => ("Arc°", 5.0, 355.0, 1.0, "Curved stairs sweep in degrees"),
            Field::Inner => ("Inner", 0.01, 0.95, 0.01, "Curved stairs inner radius (fraction of the outer)"),
            Field::Opening => ("Door W", 0.01, 64.0, 0.05, "Doorway width"),
            Field::DoorHeight => ("Door H", 0.01, 64.0, 0.05, "Doorway height"),
            Field::Profile => ("Profile", 0.0, 5.0, 1.0, "Trim cross-section"),
            Field::Side => ("Side", 0.0, 2.0, 1.0, "Wall side of the drawn line"),
            Field::PaintApply => ("Apply", 0.0, 1.0, 1.0, "Brush blends vertices toward the color inside the radius; Fill paints whole faces"),
            Field::PaintChannel => ("Channel", 0.0, 2.0, 1.0, "RGBA paints color and blend mask, RGB only the color, Alpha only the blend mask of a blend material"),
            Field::PaintRadius => ("Radius", 0.01, 64.0, 0.05, "Vertex paint brush radius (world units)"),
            Field::PaintStrength => ("Strength", 0.0, 1.0, 0.05, "How strongly one stroke blends toward the paint color"),
            Field::PaintSoftness => ("Soft", 0.0, 1.0, 0.05, "Falloff: 0 paints the full radius evenly, 1 fades from the centre"),
            Field::PaintAlpha => ("Alpha", 0.0, 1.0, 0.05, "Blend mask value painted in the Alpha channel (0 = layer A, 1 = layer B)"),
            Field::InsetMode => ("Inset by", 0.0, 2.0, 1.0, "Fraction: corners move towards the centre; Width: a rim Inset metres wide; Equal: mitred corners, the same width everywhere (Unibuilder)"),
            Field::SubdivSegs => ("Subdiv", 2.0, 16.0, 2.0, "Subdivide: pieces along every edge (even)"),
            Field::GizmoOrient => ("Axes", 0.0, 2.0, 1.0, "Handle orientation: Global (world axes), Local (the node's axes), Normal (the selection's average normal)"),
            Field::TrimPlace => ("Trim at", 0.0, 1.0, 1.0, "Trim walls: along the base of the walls, or hung under their top"),
            Field::TrimDepth => ("T depth", 0.005, 4.0, 0.005, "Trim walls: how far the moulding stands out of the wall"),
            Field::TrimHeight => ("T height", 0.01, 4.0, 0.01, "Trim walls: moulding height"),
        }
    }
    /// Drop-down choices (fields shown as an OptionButton instead of a SpinBox).
    pub fn options(self) -> Option<&'static [&'static str]> {
        match self {
            Field::Falloff => Some(&["Smooth", "Sphere", "Root", "Inverse square", "Sharp", "Linear", "Constant"]),
            Field::MirrorAxis => Some(&["X", "Y", "Z"]),
            Field::Profile => Some(&pt_core::build::prims::TrimProfile::NAMES),
            Field::Side => Some(&["Centre", "Left", "Right"]),
            Field::PaintApply => Some(&["Brush", "Fill"]),
            Field::PaintChannel => Some(&["RGBA", "RGB", "Alpha"]),
            Field::InsetMode => Some(&["Fraction", "Width", "Equal"]),
            Field::GizmoOrient => Some(&["Global", "Local", "Normal"]),
            Field::TrimPlace => Some(&["Bottom", "Top"]),
            _ => None,
        }
    }
    /// Modes / tools the field is shown for: (shape tool, edit modes mask)
    fn shown(self, mode: Mode, tool: Tool, shape: bool) -> bool {
        match self {
            Field::SizeX | Field::SizeY | Field::SizeZ | Field::Segments | Field::Steps | Field::Thickness | Field::Rings => shape,
            Field::Slot | Field::GridStep => true,
            Field::PaintApply | Field::PaintChannel | Field::PaintRadius | Field::PaintStrength | Field::PaintSoftness | Field::PaintAlpha => {
                tool == Tool::VertexPaint && !shape
            }
            Field::Distance | Field::Inset if tool == Tool::Knife => false,
            Field::Distance => !shape && matches!(mode, Mode::Face | Mode::Edge),
            Field::Inset => !shape && mode == Mode::Face,
            Field::Bevel | Field::Cuts | Field::BevelSegs => !shape && mode == Mode::Edge,
            Field::SoftRadius | Field::Falloff | Field::RotSnap => !shape && mode != Mode::Object && tool == Tool::Select,
            Field::MirrorAxis => !shape && mode == Mode::Object,
            Field::Profile => shape || mode == Mode::Face,
            Field::Arc | Field::Inner | Field::Opening | Field::DoorHeight | Field::Side => shape,
            Field::InsetMode | Field::TrimPlace | Field::TrimDepth | Field::TrimHeight => !shape && mode == Mode::Face,
            Field::SubdivSegs => !shape && matches!(mode, Mode::Face | Mode::Object),
            Field::GizmoOrient => !shape && mode != Mode::Object && tool == Tool::Select,
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
    pub const ALL: [UvField; 5] = [UvField::OffsetU, UvField::OffsetV, UvField::ScaleU, UvField::ScaleV, UvField::Rotation];
    pub fn from_i64(v: i64) -> Option<UvField> {
        Self::ALL.iter().copied().find(|f| *f as i64 == v)
    }
    fn describe(self) -> (&'static str, f64, f64, f64) {
        match self {
            UvField::OffsetU => ("Offset U", -64.0, 64.0, 0.0625),
            UvField::OffsetV => ("Offset V", -64.0, 64.0, 0.0625),
            UvField::ScaleU => ("Scale U", -64.0, 64.0, 0.0625),
            UvField::ScaleV => ("Scale V", -64.0, 64.0, 0.0625),
            UvField::Rotation => ("Rotation", -360.0, 360.0, 15.0),
        }
    }
}

pub struct Ui {
    pub toolbar: Gd<VFlowContainer>,
    pub strip: Gd<HFlowContainer>,
    pub palette: Gd<ScrollContainer>,
    pub buttons: Vec<(Action, Gd<Button>, u8)>,
    /// (section, separator, header) of the toolbar; hidden while none of its buttons is.
    sections: Vec<(usize, Gd<HSeparator>, Gd<Label>)>,
    pub fields: Vec<(Field, Gd<HBoxContainer>, Gd<SpinBox>)>,
    pub options: Vec<(Field, Gd<HBoxContainer>, Gd<OptionButton>)>,
    pub arched: Gd<CheckBox>,
    pub kind_box: Gd<HBoxContainer>,
    pub kind: Gd<OptionButton>,
    pub smooth: Gd<CheckBox>,
    pub color: Gd<ColorPickerButton>,
    pub info: Gd<Label>,
    pub slots: Gd<VBoxContainer>,
    pub slot_buttons: Vec<Gd<Button>>,
    pub slot_group: Gd<ButtonGroup>,
    pub uv_fields: Vec<(UvField, Gd<SpinBox>)>,
    pub uv_world: Gd<CheckBox>,
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

fn icon(name: &str) -> Option<Gd<Texture2D>> {
    mono_icon(&format!("{ICONS}{name}.svg"), ICON_PX / 16.0).or_else(|| try_load::<Texture2D>(&format!("{ICONS}{name}.svg")).ok())
}

/// Icon color of the editor theme (the MST toolbar's monochrome look).
fn theme_icon_color() -> Color {
    if !godot::classes::Engine::singleton().is_editor_hint() {
        return Color::from_rgb(0.88, 0.88, 0.88);
    }
    godot::classes::EditorInterface::singleton()
        .get_base_control()
        .map(|c| c.get_theme_color_ex("font_color").theme_type("Label").done())
        .unwrap_or(Color::from_rgb(0.88, 0.88, 0.88))
}

/// Rasterises an SVG icon at `scale` × the editor scale (sharp at any size), recoloured to one
/// editor-theme tone (Unibuilder's icons are multi-coloured;
/// the MST toolbar is monochrome). Luminance keeps the inner detail as light / dark shades.
pub fn mono_icon(path: &str, scale: f32) -> Option<Gd<Texture2D>> {
    use godot::classes::{FileAccess, Image, ImageTexture, image::Format};
    if !FileAccess::file_exists(path) {
        return None;
    }
    let text = FileAccess::get_file_as_string(path);
    let mut img = Image::new_gd();
    if text.is_empty() || img.load_svg_from_string_ex(&text).scale(scale * ui_kit::editor_scale()).done() != godot::global::Error::OK {
        return None;
    }
    img.convert(Format::RGBA8);
    let tone = theme_icon_color();
    let mut px = img.get_data().to_vec();
    for p in px.chunks_exact_mut(4) {
        let lum = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0;
        let k = 0.62 + 0.38 * lum;
        p[0] = (tone.r * k * 255.0).round() as u8;
        p[1] = (tone.g * k * 255.0).round() as u8;
        p[2] = (tone.b * k * 255.0).round() as u8;
    }
    let (w, h) = (img.get_width(), img.get_height());
    let img = Image::create_from_data(w, h, false, Format::RGBA8, &PackedByteArray::from(px.as_slice()))?;
    ImageTexture::create_from_image(&img).map(|t| t.upcast())
}


/// Current values shown by the strip and palette.
pub struct Values<'a> {
    pub spec: &'a ShapeSpec,
    pub slot: i64,
    pub grid_step: f32,
    pub distance: f32,
    pub inset: f32,
    pub bevel: f32,
    pub cuts: i64,
    pub color: Color,
    /// offset u, v, scale u, v, rotation
    pub uv: [f32; 5],
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
}

/// Settings of the W10 Unibuilder ports (inset mode, subdivide segments, handle axes, trim
/// walls) and the paint color swatches.
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
        ExtraOpts { inset_mode: 0, subdiv_segs: 2, gizmo_orient: 0, trim_place: 0, trim_depth: 0.06, trim_height: 0.15 }
    }
}

/// Unibuilder paint panel swatches (`paint_panel.gd` `DEFAULT_SWATCHES`).
pub const SWATCHES: [Color; 8] = [
    Color::from_rgb(1.0, 1.0, 1.0),
    Color::from_rgb(0.9, 0.2, 0.2),
    Color::from_rgb(0.2, 0.8, 0.3),
    Color::from_rgb(0.25, 0.5, 0.95),
    Color::from_rgb(0.95, 0.75, 0.15),
    Color::from_rgb(0.6, 0.3, 0.8),
    Color::from_rgb(0.15, 0.8, 0.75),
    Color::from_rgb(0.15, 0.15, 0.17),
];

impl Values<'_> {
    pub fn get(&self, f: Field) -> f64 {
        let s = self.spec;
        match f {
            Field::SizeX => s.size.x as f64,
            Field::SizeY => s.size.y as f64,
            Field::SizeZ => s.size.z as f64,
            Field::Segments => s.segments as f64,
            Field::Steps => s.steps as f64,
            Field::Thickness => s.thickness as f64,
            Field::Rings => s.rings as f64,
            Field::Slot => self.slot as f64,
            Field::GridStep => self.grid_step as f64,
            Field::Distance => self.distance as f64,
            Field::Inset => self.inset as f64,
            Field::Bevel => self.bevel as f64,
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
            Field::Profile => s.profile as f64,
            Field::Side => s.side as f64,
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
        }
    }
}

/// Toolbar / palette state mirrored by `Ui::sync`.
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
}

/// Unibuilder shape icon for a kind (in the Shape drop-down).
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
    }
}

fn style_swatch(b: &mut Gd<Button>, c: Color) {
    let mut st = godot::classes::StyleBoxFlat::new_gd();
    st.set_bg_color(Color::from_rgba(c.r, c.g, c.b, 1.0));
    st.set_corner_radius_all(3);
    for state in ["normal", "hover", "pressed", "focus"] {
        b.add_theme_stylebox_override(state, &st);
    }
}

/// Status-line text for a toolbar button: its title and shortcut (first tooltip line).
pub fn hover_text(a: Action) -> String {
    BUTTONS.iter().find(|b| b.0 == a).map(|b| b.2.lines().next().unwrap_or("").split(": ").next().unwrap_or("").to_string()).unwrap_or_default()
}

fn label(text: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
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

pub fn build(target: &Gd<Object>, v: &Values) -> Ui {
    // ---------------------------------------------------------------- left toolbar
    // Same look as the terrain toolbar (`ui_kit::Toolbar`): 30 px flat icon buttons, centred in
    // their cells, groups split by a separator plus a small section header.
    let mut toolbar = VFlowContainer::new_alloc();
    toolbar.set_name("PixelBuilderToolbar");
    toolbar.set_custom_minimum_size(ui_kit::sz(CELL_PX, CELL_PX));
    toolbar.set_alignment(godot::classes::flow_container::AlignmentMode::BEGIN);
    let mut buttons = Vec::new();
    let mut sections = Vec::new();
    let groups = [ButtonGroup::new_gd(), ButtonGroup::new_gd(), ButtonGroup::new_gd()];
    let mut last_section = None;
    // Grouped by section (stable), so every section has exactly one header.
    let mut ordered: Vec<&(Action, &str, &str, u8, u8)> = BUTTONS.iter().collect();
    ordered.sort_by_key(|b| section_of(b.0, b.3));
    for (a, ic, tip, group, mask) in ordered {
        let sec = section_of(*a, *group);
        if last_section != Some(sec) {
            let mut sep = HSeparator::new_alloc();
            sep.set_name(&format!("{}Separator", SECTIONS[sec].0));
            toolbar.add_child(&sep);
            let mut head = Label::new_alloc();
            head.set_name(&format!("{}Header", SECTIONS[sec].0));
            head.set_text(SECTIONS[sec].0);
            head.set_tooltip_text(SECTIONS[sec].1);
            head.set_mouse_filter(godot::classes::control::MouseFilter::PASS);
            head.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
            head.add_theme_font_size_override("font_size", (11.0 * ui_kit::editor_scale()) as i32);
            head.add_theme_color_override("font_color", Color::from_rgb(0.62, 0.78, 0.95));
            head.set_custom_minimum_size(ui_kit::sz(CELL_PX * 2.0, 0.0));
            toolbar.add_child(&head);
            sections.push((sec, sep, head));
            last_section = Some(sec);
        }
        let mut b = ui_kit::tool_button(&format!("{a:?}"), &format_tooltip(tip, *mask), icon(ic));
        b.set_custom_minimum_size(ui_kit::sz(CELL_PX - 2.0, CELL_PX - 2.0));
        // Icons drawn at their rasterised size, with a slim margin (the flat editor style's
        // content margins shrank them to ~14 px).
        b.set_expand_icon(false);
        for state in ["normal", "hover", "pressed", "hover_pressed", "disabled", "focus"] {
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
        b.connect("pressed", &Callable::from_object_method(target, "on_action").bind(&[(*a as i64).to_variant()]));
        b.connect("mouse_entered", &Callable::from_object_method(target, "on_button_hover").bind(&[(*a as i64).to_variant()]));
        b.connect("mouse_exited", &Callable::from_object_method(target, "on_button_hover").bind(&[(-1i64).to_variant()]));
        toolbar.add_child(&ui_kit::center(ui_kit::sz(CELL_PX, CELL_PX), &b));
        buttons.push((*a, b, *mask));
    }

    // ---------------------------------------------------------------- bottom strip
    let mut strip = HFlowContainer::new_alloc();
    strip.set_name("PixelBuilderStrip");
    strip.set_h_size_flags(SizeFlags::EXPAND_FILL);

    let mut hover = Label::new_alloc();
    hover.set_name("PixelBuilderHover");
    hover.add_theme_color_override("font_color", Color::from_rgb(0.62, 0.78, 0.95));
    hover.set_custom_minimum_size(Vector2::new(150.0 * ui_kit::editor_scale(), 0.0));
    hover.set_tooltip_text("Name and shortcut of the builder button under the mouse");
    strip.add_child(&hover);

    let mut kind_box = HBoxContainer::new_alloc();
    kind_box.add_child(&label("Shape"));
    let mut kind = OptionButton::new_alloc();
    for k in ShapeKind::ALL {
        match icon(shape_icon(k)) {
            Some(t) => kind.add_icon_item(&t, k.name()),
            None => kind.add_item(k.name()),
        }
    }
    kind.select(v.spec.kind as i32);
    kind.set_focus_mode(FocusMode::NONE);
    kind.connect("item_selected", &Callable::from_object_method(target, "on_kind"));
    kind_box.add_child(&kind);
    strip.add_child(&kind_box);

    let mut smooth = CheckBox::new_alloc();
    smooth.set_text("Smooth");
    smooth.set_pressed_no_signal(v.spec.smooth);
    smooth.set_tooltip_text("Smooth shading for round shapes");
    smooth.connect("toggled", &Callable::from_object_method(target, "on_smooth"));

    let mut color = ColorPickerButton::new_alloc();
    color.set_custom_minimum_size(Vector2::new(28.0, 0.0));
    color.set_pick_color(v.color);
    color.set_tooltip_text("Face color (multiplies the slot material)");
    color.connect("color_changed", &Callable::from_object_method(target, "on_color"));

    let mut arched = CheckBox::new_alloc();
    arched.set_text("Arched");
    arched.set_pressed_no_signal(v.spec.arched);
    arched.set_tooltip_text("Round-topped doorway");
    arched.connect("toggled", &Callable::from_object_method(target, "on_arched"));

    // Unibuilder paint panel swatches.
    let mut swatch_box = HBoxContainer::new_alloc();
    swatch_box.add_theme_constant_override("separation", 2);
    let mut swatches = Vec::new();
    for (i, c) in v.swatches.iter().enumerate() {
        let mut b = Button::new_alloc();
        b.set_custom_minimum_size(ui_kit::sz(14.0, 20.0));
        b.set_focus_mode(FocusMode::NONE);
        b.set_tooltip_text("Color swatch: click to use it, Ctrl+click to store the current color in it");
        b.connect("pressed", &Callable::from_object_method(target, "on_swatch").bind(&[(i as i64).to_variant()]));
        style_swatch(&mut b, *c);
        swatch_box.add_child(&b);
        swatches.push(b);
    }

    let mut fields = Vec::new();
    let mut options = Vec::new();
    for f in Field::ALL {
        let (text, min, max, step, tip) = f.describe();
        if let Some(choices) = f.options() {
            let mut hb = HBoxContainer::new_alloc();
            let mut l = label(text);
            l.set_tooltip_text(tip);
            hb.add_child(&l);
            let mut ob = OptionButton::new_alloc();
            for c in choices {
                ob.add_item(*c);
            }
            ob.select(v.get(f) as i32);
            ob.set_tooltip_text(tip);
            ob.set_focus_mode(FocusMode::NONE);
            ob.connect("item_selected", &Callable::from_object_method(target, "on_field_option").bind(&[(f as i64).to_variant()]));
            hb.add_child(&ob);
            strip.add_child(&hb);
            options.push((f, hb, ob));
            continue;
        }
        let mut hb = HBoxContainer::new_alloc();
        let mut l = label(text);
        l.set_tooltip_text(tip);
        hb.add_child(&l);
        let mut sb = spin(min, max, step, v.get(f), tip, 64.0);
        sb.set_allow_greater(matches!(f, Field::SizeX | Field::SizeY | Field::SizeZ | Field::Distance | Field::Bevel));
        sb.connect("value_changed", &Callable::from_object_method(target, "on_field").bind(&[(f as i64).to_variant()]));
        hb.add_child(&sb);
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
    add.connect("pressed", &Callable::from_object_method(target, "on_add_slot"));
    col.add_child(&add);
    col.add_child(&HSeparator::new_alloc());
    let mut uvt = label("Face UV");
    uvt.set_tooltip_text("Automatic box projection per face: every face is mapped along its dominant axis.\nWorld aligned keeps textures continuous across meshes.\nSelecting one face loads its values; Apply writes them to the selected faces.");
    col.add_child(&uvt);
    let mut grid = GridContainer::new_alloc();
    grid.set_columns(2);
    let mut uv_fields = Vec::new();
    for (i, f) in UvField::ALL.iter().enumerate() {
        let (text, min, max, step) = f.describe();
        grid.add_child(&label(text));
        let mut sb = spin(min, max, step, v.uv[i] as f64, text, 90.0);
        sb.set_allow_greater(true);
        sb.set_allow_lesser(true);
        sb.connect("value_changed", &Callable::from_object_method(target, "on_uv_field").bind(&[(*f as i64).to_variant()]));
        grid.add_child(&sb);
        uv_fields.push((*f, sb));
    }
    col.add_child(&grid);
    let mut uv_world = CheckBox::new_alloc();
    uv_world.set_text("World aligned");
    uv_world.set_pressed_no_signal(v.uv_world);
    uv_world.connect("toggled", &Callable::from_object_method(target, "on_uv_world"));
    col.add_child(&uv_world);
    let mut row = HBoxContainer::new_alloc();
    for (text, method, tip) in [
        ("Apply", "on_uv_apply", "Apply the UV values to the selected faces (all faces in Object mode)"),
        ("Reset", "on_uv_reset", "Reset the UV values (and apply)"),
        ("↻ 90°", "on_uv_rotate", "Rotate the selected faces' UVs by 90°"),
    ] {
        let mut b = Button::new_alloc();
        b.set_text(text);
        b.set_tooltip_text(tip);
        b.set_focus_mode(FocusMode::NONE);
        b.connect("pressed", &Callable::from_object_method(target, method));
        row.add_child(&b);
    }
    col.add_child(&row);
    palette.add_child(&col);

    let mut ui = Ui {
        toolbar,
        strip,
        palette,
        buttons,
        sections,
        fields,
        options,
        arched,
        kind_box,
        kind,
        smooth,
        color,
        info,
        swatches,
        hover,
        slots,
        slot_buttons: Vec::new(),
        slot_group: ButtonGroup::new_gd(),
        uv_fields,
        uv_world,
        target: target.clone(),
    };
    ui.show_fields(v.spec.kind, Mode::Object, Tool::Select, false);
    ui
}

impl Ui {
    pub fn set_swatch(&mut self, i: usize, c: Color) {
        if let Some(b) = self.swatches.get_mut(i) {
            style_swatch(b, c);
        }
    }

    /// Shows the strip fields that matter for the current kind / mode / tool.
    pub fn show_fields(&mut self, k: ShapeKind, mode: Mode, tool: Tool, reshape: bool) {
        let (seg, steps, thick, smooth, rings) = k.uses();
        let (arc, door, profile, side) = k.uses_extra();
        let grid = k.is_grid_shape();
        let shape = tool == Tool::Shape || reshape;
        let kind_ok = |f: Field| match f {
            Field::SizeX | Field::SizeZ => !grid,
            Field::Segments => seg,
            Field::Steps => steps,
            Field::Thickness => thick,
            Field::Rings => rings,
            Field::Arc | Field::Inner => arc,
            Field::Opening | Field::DoorHeight => door,
            Field::Profile => profile || (!shape && mode == Mode::Face),
            Field::Side => side,
            _ => true,
        };
        for (f, hb, _) in &mut self.fields {
            hb.set_visible(kind_ok(*f) && f.shown(mode, tool, shape));
        }
        for (f, hb, _) in &mut self.options {
            hb.set_visible(kind_ok(*f) && f.shown(mode, tool, shape));
        }
        self.kind_box.set_visible(shape);
        self.smooth.set_visible(smooth && shape);
        self.arched.set_visible(door && shape);
    }

    /// Mirrors values into the controls without emitting signals.
    pub fn sync(&mut self, v: &Values, st: &SyncState) {
        let (mode, tool, snap_grid, snap_cell, has_target, can_convert) = (st.mode, st.tool, st.snap_grid, st.snap_cell, st.has_target, st.can_convert);
        for (f, _, sb) in &mut self.fields {
            sb.set_value_no_signal(v.get(*f));
        }
        for (f, _, ob) in &mut self.options {
            ob.select(v.get(*f) as i32);
        }
        self.arched.set_pressed_no_signal(v.spec.arched);
        self.kind.select(v.spec.kind as i32);
        self.smooth.set_pressed_no_signal(v.spec.smooth);
        self.color.set_pick_color(v.color);
        self.show_fields(v.spec.kind, mode, tool, st.reshape);
        for (f, hb, _) in &mut self.fields {
            let brush_only = matches!(*f, Field::PaintRadius | Field::PaintStrength | Field::PaintSoftness);
            if (*f == Field::SoftRadius && !st.soft)
                || (*f == Field::RotSnap && st.xform != XformMode::Rotate)
                || (brush_only && v.vp_fill)
                || (*f == Field::PaintAlpha && v.vp_channel != 2)
            {
                hb.set_visible(false);
            }
        }
        for (f, hb, _) in &mut self.options {
            if *f == Field::Falloff && !st.soft {
                hb.set_visible(false);
            }
        }
        for (i, (_, sb)) in self.uv_fields.iter_mut().enumerate() {
            sb.set_value_no_signal(v.uv[i] as f64);
        }
        self.uv_world.set_pressed_no_signal(v.uv_world);
        for (a, b, mask) in &mut self.buttons {
            let visible = match a {
                Action::Convert => can_convert,
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
                | Action::BakeMesh => has_target && (*mask & mode.bit()) != 0,
                Action::UvEditor => has_target,
                Action::Preferences => true,
                Action::ShapeParams => st.has_spec && mode == Mode::Object,
                Action::GizmoMove | Action::GizmoRotate | Action::GizmoScale | Action::SoftSelect | Action::SnapVertex | Action::UvLock => {
                    has_target && (*mask & mode.bit()) != 0 && tool == Tool::Select
                }
                Action::ModeFace | Action::ModeEdge | Action::ModeVertex | Action::ToolPaint | Action::ToolVertexPaint => has_target,
                _ => (*mask & mode.bit()) != 0,
            };
            b.set_visible(visible);
            // The button sits in a centring cell: hide the cell too, so the column closes up.
            if let Some(mut cell) = b.get_parent().and_then(|p| p.try_cast::<Control>().ok()) {
                cell.set_visible(visible);
            }
            let on = match a {
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
                Action::UvLock => st.uv_lock,
                Action::GizmoMove => st.xform == XformMode::Move,
                Action::GizmoRotate => st.xform == XformMode::Rotate,
                Action::GizmoScale => st.xform == XformMode::Scale,
                Action::SoftSelect => st.soft,
                Action::SnapVertex => st.snap_vertex,
                Action::ShapeParams => st.reshape,
                Action::SnapGrid => snap_grid,
                Action::SnapCell => snap_cell,
                Action::InsertVertex => st.insert_vert,
                Action::SurfaceDraw => st.surface_draw,
                Action::XRay => st.xray,
                Action::SetCollider => st.entity == 1,
                Action::SetTrigger => st.entity == 2,
                _ => continue,
            };
            b.set_pressed_no_signal(on);
        }
        for (sec, sep, head) in &mut self.sections {
            let any = self.buttons.iter().any(|(a, b, _)| {
                b.is_visible() && BUTTONS.iter().find(|x| x.0 == *a).is_some_and(|x| section_of(*a, x.3) == *sec)
            });
            sep.set_visible(any);
            head.set_visible(any);
        }
        for (i, b) in self.slot_buttons.iter_mut().enumerate() {
            b.set_pressed_no_signal(i as i64 == v.slot);
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
            b.set_tooltip_text(&format!("Slot {i}: make current; assigns it to the selected faces"));
            b.set_pressed_no_signal(i as i64 == current);
            if let Some(c) = m.as_ref().and_then(material_color) {
                b.add_theme_color_override("font_color", if c.luminance() > 0.5 { Color::BLACK } else { Color::WHITE });
                b.add_theme_color_override("font_pressed_color", Color::from_rgb(1.0, 0.7, 0.2));
                let mut sb = godot::classes::StyleBoxFlat::new_gd();
                sb.set_bg_color(c);
                b.add_theme_stylebox_override("normal", &sb);
            }
            b.connect("pressed", &Callable::from_object_method(&self.target, "on_slot_pick").bind(&[(i as i64).to_variant()]));
            row.add_child(&b);
            let mut picker = EditorResourcePicker::new_alloc();
            picker.set_base_type("Material");
            picker.set_h_size_flags(SizeFlags::EXPAND_FILL);
            if let Some(m) = m {
                picker.set_edited_resource(m);
            }
            picker.connect("resource_changed", &Callable::from_object_method(&self.target, "on_slot_material").bind(&[(i as i64).to_variant()]));
            row.add_child(&picker);
            self.slots.add_child(&row);
            self.slot_buttons.push(b);
        }
    }

    pub fn set_visible(&mut self, v: bool, palette: bool) {
        self.toolbar.set_visible(v);
        self.strip.set_visible(v);
        self.palette.set_visible(v && palette);
    }

    pub fn controls(&self) -> (Gd<Control>, Gd<Control>, Gd<Control>) {
        (self.toolbar.clone().upcast(), self.strip.clone().upcast(), self.palette.clone().upcast())
    }
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
