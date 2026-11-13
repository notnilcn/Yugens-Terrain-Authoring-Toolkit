//! Editor tool definitions, in the original MarchingSquaresTerrain order. The Rust-only
//! Minor Deity-style raise/lower sculpting is folded into the Brush tool as a second
//! "Brush Mode" (internally `Tool::RaiseLower`, which shares the Brush toolbar button).
//! Attributes ("settings") are listed per tool in the original global order.

use pt_core::brush::{Brush, BrushShape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// MST brush: select an area, drag up/down to set its height (Shift adds to the selection).
    Brush,
    Level,
    Smooth,
    Bridge,
    GrassMask,
    VertexPaint,
    Debug,
    ChunkManager,
    TerrainSettings,
    /// Brush in "Raise / Lower" mode (Minor Deity style): hold LMB to raise, RMB to lower,
    /// both to flatten; Ctrl picks limits. Not a toolbar entry of its own.
    RaiseLower,
    // ---- Rust additions, appended after the original toolbar behind a separator.
    /// Paint / erase scatter props (`PixelScatterItem`s of the terrain).
    Scatter,
    /// Heightmap stamps with a live preview.
    Stamp,
    /// Terrain3D-style gradient: pull the terrain towards the ramp between two picked points.
    Gradient,
    /// Cell selection with copy / paste (60° / 90° rotation, mirror).
    Select,
    /// Paints saved per-cell tints (territory, biome hints, paths); stored in the terrain data.
    Tint,
}

/// Rust tools appended after [`TOOLS`] (toolbar indices `TOOLS.len()..`).
pub const EXTRA_TOOLS: [Tool; 5] = [Tool::Scatter, Tool::Stamp, Tool::Gradient, Tool::Select, Tool::Tint];

/// Toolbar order (= original toolbox order; keys 1-9).
pub const TOOLS: [Tool; 9] = [
    Tool::Brush,
    Tool::Level,
    Tool::Smooth,
    Tool::Bridge,
    Tool::GrassMask,
    Tool::VertexPaint,
    Tool::Debug,
    Tool::ChunkManager,
    Tool::TerrainSettings,
];

/// A toolbar `HSeparator` is placed *before* these indices (and at both ends).
pub const GROUP_STARTS: [usize; 2] = [4, 6];

/// Items of the Brush tool's "Brush Mode" dropdown.
pub const BRUSH_MODES: [&str; 2] = ["Drag", "Raise / Lower"];

const BRUSH_SHORTCUTS: &str = "[SHORTCUTS]
• Shift+LMB+Drag: Add cells to the current draw selection.
• Shift+MWU/MWD: Increase or decrease the brush size.
• Ctrl+MWU/MWD: Increase or decrease the strength.
• Alt/RMB/Esc: Reset the current draw selection.
• 1-9: Switch tools.
These shortcuts apply to all brush related tools.";

impl Tool {
    /// Button name (= original tool label).
    pub fn name(self) -> &'static str {
        match self {
            Tool::Brush => "Brush",
            Tool::Level => "Level",
            Tool::Smooth => "Smooth",
            Tool::Bridge => "Bridge",
            Tool::GrassMask => "Grass Mask",
            Tool::VertexPaint => "Vertex Painting",
            Tool::Debug => "Debug Brush",
            Tool::ChunkManager => "Chunk Manager",
            Tool::TerrainSettings => "Terrain Settings",
            Tool::RaiseLower => "Brush (Raise / Lower)",
            Tool::Scatter => "Scatter",
            Tool::Stamp => "Stamp",
            Tool::Gradient => "Gradient",
            Tool::Select => "Select",
            Tool::Tint => "Tint",
        }
    }

    /// The toolbar entry this tool belongs to.
    pub fn button(self) -> Tool {
        if self == Tool::RaiseLower { Tool::Brush } else { self }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Brush => "brush_tool.svg",
            Tool::Level => "level_tool.svg",
            Tool::Smooth => "smooth_tool.svg",
            Tool::Bridge => "bridge_tool.svg",
            Tool::GrassMask => "grass_mask_tool.svg",
            Tool::VertexPaint => "vertex_paint_tool.svg",
            Tool::Debug => "toolicon.svg",
            Tool::ChunkManager => "chunk_manager_tool.svg",
            Tool::TerrainSettings => "terrain_settings_tool.png",
            Tool::RaiseLower => "raise_lower_tool.svg",
            Tool::Scatter => "scatter_tool.svg",
            Tool::Stamp => "stamp_tool.svg",
            Tool::Gradient => "gradient_tool.svg",
            Tool::Select => "select_tool.svg",
            Tool::Tint => "tint_tool.svg",
        }
    }

    /// Multi-line tooltip (original text; Rust additions marked by the shortcut lists).
    pub fn tooltip(self) -> String {
        let key = TOOLS.iter().position(|t| *t == self.button()).map(|i| i + 1).unwrap_or(0);
        let body = match self {
            Tool::Brush => format!(
                "Brush Tool [{key}]\n\nUsed to elevate or lower terrain.\n\n[MODES]\n• Drag: select an area, then drag up/down to set its height (original MST brush).\n• Raise / Lower: hold LMB to raise (up to Ceiling), RMB to lower (down to Floor),\n  LMB+RMB to flatten towards Height. Ctrl+LMB / MMB / RMB pick Ceiling / Height / Floor.\n\n{BRUSH_SHORTCUTS}"
            ),
            Tool::Level => format!(
                "Level Tool [{key}]\n\nUsed to level terrain to a certain height.\n\n[SHORTCUTS]\n• Ctrl+LMB: Set the terrain level height to the hovered cell's Y value."
            ),
            Tool::Smooth => format!("Smooth Tool [{key}]\n\nUsed to smooth neighbouring terrain to their average height."),
            Tool::Bridge => format!(
                "Bridge Tool [{key}]\n\nUsed to create a bridge between two points.\n\n[INFO]\nLMB-drag from the start to the end point.\nThe bridge curve falloff can be set via the \"ease value\" attribute\n(Godot ease(): 1 = linear, <0 = in-out, -1 = no easing)."
            ),
            Tool::GrassMask => format!(
                "Grass Mask Tool [{key}]\n\nUsed to control where grass gets placed.\n\n[SHORTCUTS]\n• LMB: grow grass (remove it when \"Mask Mode\" is on).\n• RMB: remove grass."
            ),
            Tool::VertexPaint => format!(
                "Vertex Paint Tool [{key}]\n\nUsed to paint textures onto the terrain.\n\n[INFO]\n• There are 16 available textures.\n    • Every texture can have grass (\"Has grass\" in the right panel).\n    • 16 is a void material making terrain invisible.\n• Texture presets can be used to quickly swap between texture pallets.\n    • They can be exported via the right hand panel at the bottom.\n• \"Quick Paints\" are a way to quickly set textures while modelling the terrain.\n\n[SHORTCUTS]\n• Ctrl+LMB: Pick the material under the cursor."
            ),
            Tool::Debug => format!(
                "Debug Brush Tool [{key}]\n\nUsed to print data about selected cells.\n\n[DEBUG INFO]\n• Global position\n• Sample / chunk / local coordinates\n• Height, floor and wall textures, grass, smooth flag"
            ),
            Tool::ChunkManager => format!(
                "Chunk Management Tool [{key}]\n\nUsed to create, delete and change chunk settings.\n\n[INFO]\nChunk cell merge modes:\n• CUBIC: The most blocky of all the modes. Has minimal smoothing between cells.\n• POLYHEDRON: Blocky terrain with slight cell smoothing (DEFAULT CHUNK STATE).\n• ROUNDED_POLYHEDRON: A good 50/50 mix between having smooth and blocky terrain.\n• SEMI_ROUND: Mostly smooth terrain with the occasional blocky cells.\n• SPHERICAL: All the terrain will become smooth.\n\n[SHORTCUTS]\n• LMB on an empty neighbour: add a chunk. LMB on a chunk: remove it.\n• CTRL+LMB: Change the currently selected chunk to the hovered chunk."
            ),
            Tool::TerrainSettings => format!(
                "Terrain Settings Tool [{key}]\n\nUsed to tweak global terrain settings.\n\n[INFO]\n• The \"Blend Mode\" dropdown menu sets the terrain's texture blending mode.\n• \"Noise Amp\" > 0 makes new chunks procedural instead of flat.\n• \"Animation Fps\" > 0 makes the grass sprites move with limited fps.\n    • Keeping it at 0 gives the grass a smooth wind based effect.\n• \"Ledge Threshold\" controls how close grass sprites get spawned to cliff edges."
            ),
            Tool::RaiseLower => Tool::Brush.tooltip(),
            Tool::Scatter => "Scatter Tool\n\nPaints props (rocks, bushes, trees, custom meshes or scenes) onto the terrain.\nItems are PixelScatterItem resources in the terrain's Scatter Items list.\n\n[MODES]\n• Paint: density-based placement under the brush (falloff thins the edge).\n• Per Cell: 0-N items per cell from a deterministic hash (same result everywhere).\n\n[SHORTCUTS]\n• LMB: paint the selected item.\n• RMB: erase the selected item (all items with \"Erase All\").\n• Shift+MWU/MWD: brush size. Alt+MWU/MWD: brush rotation.\nProps follow the terrain when it is sculpted.".to_string(),
            Tool::Stamp => "Stamp Tool\n\nStamps a heightmap (mountain, crater, mesa, valley, hill or an image in\naddons/MarchingSquaresTerrain/stamps/) onto the terrain. The preview follows the cursor.\n\n[MODES]\n• Add: raise by the stamp. Max / Min: only raise / only lower towards the stamp.\n• Replace: set the area to the stamp (relative to the centre height).\n\n[SHORTCUTS]\n• LMB: stamp (one undo step). LMB-drag: pin the centre, drag out size and rotation, release to stamp.\n• Shift+MWU/MWD: size. Ctrl+MWU/MWD: height. Alt+MWU/MWD or Z / X: rotate\n  (60° steps on hex/triangle grids, 15° otherwise, with Snap Rotation).\n• Negative Height carves (valleys, craters).\n• Limits: clamp to Floor / Ceiling; Flatten / Macro use Level (Minor Deity stamp limits).".to_string(),
            Tool::Gradient => "Gradient Tool\n\nPulls the terrain towards the straight ramp between two points.\n\n[SHORTCUTS]\n• Ctrl+LMB: pick the start point (position + height).\n• Ctrl+RMB: pick the end point.\n• LMB-drag: paint the ramp with the brush (Strength per stroke step).".to_string(),
            Tool::Tint => "Tint Tool\n\nPaints a color over whole cells (territory, biome hints, paths, decals).\nTints are saved with the terrain data and drawn by the terrain shader in the\neditor and in game, under fog of war and under runtime set_cell_tint() tints.\n\n[SHORTCUTS]\n• LMB: paint the Tint color at Opacity.\n• RMB: erase tints.\n• Shift+MWU/MWD: brush size.".to_string(),
            Tool::Select => "Select Tool\n\nSelects cells to copy and paste them (height, textures, grass).\n\n[SHORTCUTS]\n• LMB-drag: add cells under the brush to the selection. Alt/Esc: clear it.\n• C: copy the selection (anchor = hovered cell). V or \"Paste\": paste mode.\n• Paste mode: Z / X rotate (60° on hex, 90° on square), M mirrors,\n  LMB pastes (one undo step), RMB / Esc leaves paste mode.".to_string(),
        };
        body
    }

    /// Undo action names of the original plugin.
    pub fn undo_name(self) -> &'static str {
        match self {
            Tool::VertexPaint => "terrain vertex paint",
            Tool::GrassMask => "terrain grass mask draw",
            Tool::Bridge => "terrain bridge",
            Tool::Smooth => "terrain smooth",
            Tool::Level => "terrain level",
            Tool::RaiseLower => "terrain raise/lower",
            Tool::Scatter => "terrain scatter",
            Tool::Stamp => "terrain stamp",
            Tool::Gradient => "terrain gradient",
            Tool::Select => "terrain paste",
            Tool::Tint => "terrain tint paint",
            _ => "terrain height draw",
        }
    }

    pub fn uses_brush(self) -> bool {
        !matches!(self, Tool::ChunkManager | Tool::TerrainSettings | Tool::Stamp)
    }

    /// One of the Rust tools appended after the original nine.
    pub fn is_extra(self) -> bool {
        EXTRA_TOOLS.contains(&self)
    }
}

/// Identifiers of brush attributes (bound to UI controls), in the original global order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Setting {
    BrushType = 0,
    BrushMode,
    Size,
    EaseValue,
    Ceiling,
    Height,
    Floor,
    Strength,
    Flatten,
    Falloff,
    GridAligned,
    GridSize,
    MaskMode,
    Material,
    TextureName,
    TexturePreset,
    QuickPaint,
    PaintWalls,
    // ---- Rust additions (anisotropic brush, slope filter, scatter, stamp, select)
    Aspect,
    Rotation,
    Peak,
    SlopeMin,
    SlopeMax,
    ScatterItem,
    ScatterAdd,
    ScatterMode,
    EraseAll,
    ScatterFill,
    ScatterClear,
    StampImage,
    StampMode,
    StampSize,
    StampAspect,
    StampHeight,
    StampRotation,
    StampContrast,
    StampSnap,
    Copy,
    Paste,
    PasteRotation,
    PasteMirror,
    RelativeHeight,
    /// Stamp: clamp to Floor / Ceiling and use Height as the Flatten / Macro level.
    StampLimits,
    /// Tint tool: color (index into [`TINT_COLORS`]) and opacity.
    TintColor,
    TintOpacity,
    /// Scatter tool: removes the selected item from the list (its instances go, later items'
    /// instances are re-indexed).
    ScatterRemove,
}

const ALL: [Setting; 46] = {
    use Setting::*;
    [
        BrushType, BrushMode, Size, EaseValue, Ceiling, Height, Floor, Strength, Flatten, Falloff, GridAligned, GridSize, MaskMode, Material, TextureName, TexturePreset, QuickPaint, PaintWalls,
        Aspect, Rotation, Peak, SlopeMin, SlopeMax, ScatterItem, ScatterAdd, ScatterMode, EraseAll, ScatterFill, ScatterClear, StampImage, StampMode, StampSize, StampAspect, StampHeight,
        StampRotation, StampContrast, StampSnap, Copy, Paste, PasteRotation, PasteMirror, RelativeHeight, StampLimits, TintColor, TintOpacity, ScatterRemove,
    ]
};

impl Setting {
    pub fn from_i64(v: i64) -> Option<Setting> {
        ALL.into_iter().find(|s| *s as i64 == v)
    }
}

pub const BRUSH_TYPES: [&str; 6] = ["Round", "Square", "Hexagon", "Hexagon30", "Triangle", "Triangle180"];
pub const MERGE_MODES: [&str; 5] = ["Cubic", "Polyhedron", "Rounded Polyhedron", "Semi Round", "Spherical"];

/// Control used for an attribute.
pub enum Kind {
    /// `HSlider` (min, max, step).
    HSlider(f64, f64, f64),
    /// `EditorSpinSlider` (min, max, step), unbounded.
    Spin(f64, f64, f64),
    Check,
    /// Dropdown filled by the plugin (brush types, materials, presets, quick paints).
    Option,
    Text,
    /// Push button (text) that runs an action.
    Button(&'static str),
}

/// Tint tool colors (name, color).
pub const TINT_COLORS: [(&str, [f32; 3]); 10] = [
    ("Red", [0.9, 0.2, 0.18]),
    ("Orange", [0.95, 0.55, 0.15]),
    ("Yellow", [0.95, 0.85, 0.25]),
    ("Green", [0.3, 0.8, 0.3]),
    ("Teal", [0.15, 0.75, 0.7]),
    ("Blue", [0.25, 0.45, 0.95]),
    ("Purple", [0.6, 0.35, 0.85]),
    ("Pink", [0.95, 0.45, 0.7]),
    ("White", [1.0, 1.0, 1.0]),
    ("Black", [0.05, 0.05, 0.06]),
];

/// Items of the Scatter tool's mode dropdown.
pub const SCATTER_MODES: [&str; 2] = ["Paint", "Per Cell"];
/// Built-in items the "Add Item" dropdown creates (`PixelScatterItem.create_builtin`).
pub const SCATTER_ADD: [&str; 6] = ["Add Item...", "Rock", "Bush", "Tree", "Pine", "Flowers"];

pub fn describe(s: Setting, t: Tool) -> (&'static str, Kind) {
    use Setting::*;
    match s {
        BrushType => ("Brush Type", Kind::Option),
        BrushMode => ("Brush Mode", Kind::Option),
        Size => ("Size", Kind::HSlider(1.0, 50.0, 0.5)),
        EaseValue => ("Ease Value", Kind::Spin(-5.0, 5.0, 0.1)),
        Ceiling => ("Ceiling", Kind::Spin(-50.0, 50.0, 0.1)),
        Height if t == Tool::Stamp => ("Level", Kind::Spin(-50.0, 50.0, 0.1)),
        Height => ("Height", Kind::Spin(-50.0, 50.0, 0.1)),
        Floor => ("Floor", Kind::Spin(-50.0, 50.0, 0.1)),
        Strength if t == Tool::RaiseLower => ("Strength", Kind::HSlider(0.25, 30.0, 0.25)),
        Strength => ("Strength", Kind::HSlider(0.025, 0.3, 0.005)),
        Flatten => ("Flatten", Kind::Check),
        Falloff => ("Falloff", Kind::Check),
        GridAligned => ("Grid Aligned", Kind::Check),
        GridSize => ("Grid Size", Kind::HSlider(0.0, 20.0, 1.0)),
        MaskMode => ("Mask Mode", Kind::Check),
        Material => ("Material", Kind::Option),
        TextureName => ("Texture Name", Kind::Text),
        TexturePreset => ("Texture Preset", Kind::Option),
        QuickPaint => ("Quick Paint", Kind::Option),
        PaintWalls => ("Paint Walls", Kind::Check),
        Aspect => ("Aspect", Kind::HSlider(0.2, 5.0, 0.05)),
        Rotation => ("Rotation", Kind::Spin(-180.0, 180.0, 1.0)),
        Peak => ("Peak", Kind::HSlider(-1.0, 1.0, 0.05)),
        SlopeMin => ("Slope Min", Kind::Spin(0.0, 90.0, 0.5)),
        SlopeMax => ("Slope Max", Kind::Spin(0.0, 90.0, 0.5)),
        ScatterItem => ("Item", Kind::Option),
        ScatterAdd => ("Add Item", Kind::Option),
        ScatterMode => ("Scatter Mode", Kind::Option),
        EraseAll => ("Erase All", Kind::Check),
        ScatterFill => ("Fill Cells", Kind::Button("Fill All Cells")),
        ScatterClear => ("Clear Item", Kind::Button("Clear Item")),
        ScatterRemove => ("Remove Item", Kind::Button("Remove Item")),
        StampImage => ("Stamp", Kind::Option),
        StampMode => ("Mode", Kind::Option),
        StampSize => ("Size", Kind::HSlider(2.0, 200.0, 0.5)),
        StampAspect => ("Aspect", Kind::HSlider(0.2, 5.0, 0.05)),
        StampHeight => ("Height", Kind::Spin(-100.0, 100.0, 0.1)),
        StampRotation => ("Rotation", Kind::Spin(-360.0, 360.0, 1.0)),
        StampContrast => ("Contrast", Kind::HSlider(0.2, 4.0, 0.05)),
        StampSnap => ("Snap Rotation", Kind::Check),
        Copy => ("Copy", Kind::Button("Copy")),
        Paste => ("Paste", Kind::Button("Paste")),
        PasteRotation => ("Paste Rotation", Kind::Spin(-360.0, 360.0, 1.0)),
        PasteMirror => ("Mirror", Kind::Check),
        RelativeHeight => ("Relative Height", Kind::Check),
        StampLimits => ("Limits", Kind::Check),
        TintColor => ("Tint", Kind::Option),
        TintOpacity => ("Opacity", Kind::HSlider(0.05, 1.0, 0.05)),
    }
}

pub fn tooltip(s: Setting) -> &'static str {
    use Setting::*;
    match s {
        Size => "Brush diameter in world units (Shift+wheel).",
        EaseValue => "Bridge height curve, Godot ease(): 1 = linear, 2 = ease-in, 0.5 = ease-out, <0 = in-out. -1 = no easing.",
        Ceiling => "Raise never goes above this height (Ctrl+LMB picks it).",
        Height => "Target height (Ctrl+LMB picks it from the hovered cell).",
        Floor => "Lower never goes below this height (Ctrl+RMB picks it).",
        Strength => "Strength of the tool (Ctrl+wheel).",
        Flatten => "Flatten the dragged selection to the clicked height.",
        Falloff => "Weaken the brush towards its edge.",
        GridAligned => "Snap the brush to whole cells (cell grids, Hexagon brushes, no falloff).",
        GridSize => "Size of the grid-aligned stamp in cells.",
        MaskMode => "On: LMB removes grass. Off: LMB grows grass. RMB always removes.",
        Material => "Texture slot to paint.",
        TextureName => "Rename the selected material (Enter to apply).",
        TexturePreset => "Apply a texture preset to this terrain.",
        QuickPaint => "Paint textures while sculpting with the height tools.",
        PaintWalls => "Paint wall textures instead of the floor.",
        BrushType => "Brush shape.",
        BrushMode => "Drag: select, then drag the height (original). Raise / Lower: hold LMB/RMB to sculpt continuously.",
        Aspect => "Brush Z radius / X radius (ellipse / rectangle brushes).",
        Rotation => "Brush rotation in degrees (Alt+wheel).",
        Peak => "Falloff contrast: > 0 pointy peaks, < 0 flat-topped mesas.",
        SlopeMin => "Only affect terrain at least this steep (degrees).",
        SlopeMax => "Only affect terrain at most this steep (degrees).",
        ScatterItem => "Scatter item to paint (the terrain's Scatter Items list).",
        ScatterAdd => "Adds a built-in item (rock, bush, tree, pine, flowers) to the terrain's Scatter Items.",
        ScatterMode => "Paint: density under the brush. Per Cell: deterministic 0-N items per cell.",
        EraseAll => "RMB erases every item (off: only the selected item).",
        ScatterFill => "Per-cell scatter of the selected item over the whole terrain (replaces that item).",
        ScatterClear => "Removes every instance of the selected item.",
        ScatterRemove => "Removes the selected item from the terrain's Scatter Items, with its instances; instances of the items after it keep their item (undoable).",
        StampImage => "Stamp heightmap (built-ins + images in addons/MarchingSquaresTerrain/stamps/).",
        StampMode => "Add: raise. Max / Min: only raise / lower towards the stamp. Replace: set the area.\nMinor Deity (ElevationStampJob): Raise / Lower: add / remove height x value (Minor Deity contrast). Flatten: pull towards the click height. Macro: landmass heights from the click height up by Height, only ever raising.",
        StampSize => "Stamp width in world units (Shift+wheel).",
        StampAspect => "Stamp depth / width.",
        StampHeight => "Height of white in the stamp (Ctrl+wheel). Negative carves.",
        StampRotation => "Stamp rotation in degrees (Alt+wheel, Z / X).",
        StampContrast => "Gamma of the stamp values: > 1 sharper, < 1 fuller.",
        StampSnap => "Rotate in 60° steps on hex/triangle grids (15° on square grids).",
        Copy => "Copy the selected cells (C); the hovered cell is the anchor.",
        Paste => "Enter paste mode (V): LMB pastes, Z / X rotate, M mirrors.",
        PasteRotation => "Rotation of the pasted patch (60° steps on hex, 90° on square).",
        PasteMirror => "Mirror the pasted patch (M).",
        RelativeHeight => "Paste heights relative to the target cell (off: copied absolute heights).",
        StampLimits => "Stamp limits (Minor Deity ElevationStampJob): the stamp never raises above Ceiling or\nlowers below Floor (every mode), and Flatten / Macro use Level as their height.\nOff: no limits, Flatten / Macro level at the clicked height.",
        TintColor => "Color painted by LMB (RMB erases).",
        TintOpacity => "Strength of the painted tint (its alpha).",
    }
}

pub fn settings_for(t: Tool) -> &'static [Setting] {
    use Setting::*;
    match t {
        Tool::Brush => &[BrushType, BrushMode, Size, Aspect, Rotation, Flatten, Falloff, Peak, GridAligned, GridSize, QuickPaint],
        Tool::Level => &[BrushType, Size, Aspect, Rotation, Height, Falloff, Peak, GridAligned, GridSize, QuickPaint],
        Tool::Smooth => &[BrushType, Size, Aspect, Rotation, Strength, GridAligned, GridSize, QuickPaint],
        Tool::Bridge => &[BrushType, Size, EaseValue, GridAligned, GridSize, QuickPaint],
        Tool::GrassMask => &[BrushType, Size, Aspect, Rotation, MaskMode, SlopeMin, SlopeMax],
        Tool::VertexPaint => &[BrushType, Size, Aspect, Rotation, Material, TextureName, TexturePreset, PaintWalls, SlopeMin, SlopeMax],
        Tool::Debug => &[BrushType, Size],
        Tool::ChunkManager | Tool::TerrainSettings => &[],
        Tool::RaiseLower => &[BrushType, BrushMode, Size, Aspect, Rotation, Ceiling, Height, Floor, Strength, Falloff, Peak, GridAligned, GridSize, QuickPaint],
        Tool::Scatter => &[ScatterItem, ScatterAdd, ScatterMode, BrushType, Size, Aspect, Rotation, Falloff, SlopeMin, SlopeMax, EraseAll, ScatterFill, ScatterClear, ScatterRemove],
        Tool::Stamp => &[StampImage, StampMode, StampSize, StampAspect, StampHeight, StampRotation, StampSnap, StampContrast, StampLimits, Ceiling, Height, Floor],
        Tool::Gradient => &[BrushType, Size, Aspect, Rotation, Strength, Falloff, Peak],
        Tool::Select => &[BrushType, Size, GridAligned, GridSize, Copy, Paste, PasteRotation, PasteMirror, RelativeHeight],
        Tool::Tint => &[BrushType, Size, Aspect, Rotation, TintColor, TintOpacity, GridAligned, GridSize],
    }
}

/// Built-in quick paints (original `quick_paints/global/` + one bound to the Material slot).
pub struct QuickPaintDef {
    pub name: &'static str,
    /// `None` = use the selected Material.
    pub ground: Option<u8>,
    pub wall: Option<u8>,
    pub grass: Option<bool>,
}

pub const QUICK_PAINTS: [QuickPaintDef; 4] = [
    QuickPaintDef { name: "None", ground: None, wall: None, grass: None },
    QuickPaintDef { name: "Full Void", ground: Some(15), wall: Some(15), grass: Some(false) },
    QuickPaintDef { name: "Base Grass Masked", ground: Some(0), wall: Some(0), grass: Some(false) },
    QuickPaintDef { name: "Selected Material", ground: None, wall: None, grass: None },
];

/// Current values of every attribute (session-only, like the original).
#[derive(Clone, Debug)]
pub struct State {
    pub tool: Tool,
    pub brush: Brush,
    pub smooth_strength: f32,
    pub raise_strength: f32,
    pub flatten: bool,
    pub level: f32,
    pub ceiling: f32,
    pub floor: f32,
    pub paint_slot: u8,
    pub paint_walls: bool,
    pub mask_mode: bool,
    pub quick_paint: usize,
    pub bridge_ease: f32,
    pub merge_mode: i32,
    /// Brush tool is in "Raise / Lower" mode.
    pub raise_mode: bool,
    // ---- Rust additions
    pub slope_min: f32,
    pub slope_max: f32,
    pub scatter_item: usize,
    pub scatter_per_cell: bool,
    pub erase_all: bool,
    pub stamp: usize,
    pub stamp_mode: i64,
    pub stamp_size: f32,
    pub stamp_aspect: f32,
    pub stamp_height: f32,
    pub stamp_rotation: f32,
    pub stamp_contrast: f32,
    pub stamp_snap: bool,
    /// Stamp uses Floor / Ceiling / Height (level) limits.
    pub stamp_limits: bool,
    pub gradient_strength: f32,
    /// Paste rotation in degrees (multiples of the grid step) and mirror.
    pub paste_rotation: f32,
    pub paste_mirror: bool,
    pub relative_height: bool,
    pub tint_color: usize,
    pub tint_opacity: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
            tool: Tool::Brush,
            brush: Brush { shape: BrushShape::Round, radius: 7.5, falloff: true, falloff_ease: -1.6, grid_aligned: false, grid_size: 1, ..Default::default() },
            smooth_strength: 0.15,
            raise_strength: 4.0,
            flatten: true,
            level: 0.0,
            ceiling: 16.0,
            floor: -8.0,
            paint_slot: 0,
            paint_walls: false,
            mask_mode: false,
            quick_paint: 0,
            bridge_ease: -1.0,
            merge_mode: 1,
            raise_mode: false,
            slope_min: 0.0,
            slope_max: 90.0,
            scatter_item: 0,
            scatter_per_cell: false,
            erase_all: false,
            stamp: 0,
            stamp_mode: 0,
            stamp_size: 24.0,
            stamp_aspect: 1.0,
            stamp_height: 8.0,
            stamp_rotation: 0.0,
            stamp_contrast: 1.0,
            stamp_snap: true,
            stamp_limits: false,
            gradient_strength: 0.3,
            paste_rotation: 0.0,
            paste_mirror: false,
            relative_height: true,
            tint_color: 0,
            tint_opacity: 0.5,
        }
    }
}

impl State {
    /// Tint tool paint color (RGB of the selected color, alpha = opacity).
    pub fn tint_rgba(&self) -> [u8; 4] {
        let c = TINT_COLORS[self.tint_color.min(TINT_COLORS.len() - 1)].1;
        let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        [b(c[0]), b(c[1]), b(c[2]), b(self.tint_opacity).max(1)]
    }

    pub fn strength(&self) -> f32 {
        match self.tool {
            Tool::RaiseLower => self.raise_strength,
            Tool::Gradient => self.gradient_strength,
            _ => self.smooth_strength,
        }
    }

    /// Numeric value of an attribute (Size is the brush *diameter*).
    pub fn get(&self, s: Setting) -> f64 {
        use Setting::*;
        match s {
            BrushType => self.brush.shape as u8 as f64,
            BrushMode => self.raise_mode as i32 as f64,
            Size => self.brush.radius as f64 * 2.0,
            EaseValue => self.bridge_ease as f64,
            Ceiling => self.ceiling as f64,
            Height => self.level as f64,
            Floor => self.floor as f64,
            Strength => self.strength() as f64,
            Flatten => self.flatten as i32 as f64,
            Falloff => self.brush.falloff as i32 as f64,
            GridAligned => self.brush.grid_aligned as i32 as f64,
            GridSize => self.brush.grid_size as f64,
            MaskMode => self.mask_mode as i32 as f64,
            Material => self.paint_slot as f64,
            QuickPaint => self.quick_paint as f64,
            PaintWalls => self.paint_walls as i32 as f64,
            TextureName | TexturePreset => 0.0,
            Aspect => self.brush.aspect as f64,
            Rotation => self.brush.rotation as f64,
            Peak => self.brush.peak as f64,
            SlopeMin => self.slope_min as f64,
            SlopeMax => self.slope_max as f64,
            ScatterItem => self.scatter_item as f64,
            ScatterMode => self.scatter_per_cell as i32 as f64,
            EraseAll => self.erase_all as i32 as f64,
            StampImage => self.stamp as f64,
            StampMode => self.stamp_mode as f64,
            StampSize => self.stamp_size as f64,
            StampAspect => self.stamp_aspect as f64,
            StampHeight => self.stamp_height as f64,
            StampRotation => self.stamp_rotation as f64,
            StampContrast => self.stamp_contrast as f64,
            StampSnap => self.stamp_snap as i32 as f64,
            PasteRotation => self.paste_rotation as f64,
            PasteMirror => self.paste_mirror as i32 as f64,
            RelativeHeight => self.relative_height as i32 as f64,
            StampLimits => self.stamp_limits as i32 as f64,
            TintColor => self.tint_color as f64,
            TintOpacity => self.tint_opacity as f64,
            ScatterAdd | ScatterFill | ScatterClear | ScatterRemove | Copy | Paste => 0.0,
        }
    }

    pub fn set(&mut self, s: Setting, v: f64) {
        use Setting::*;
        match s {
            BrushType => self.brush.shape = BrushShape::from_i64(v as i64),
            BrushMode => {
                self.raise_mode = v != 0.0;
                if matches!(self.tool, Tool::Brush | Tool::RaiseLower) {
                    self.tool = if self.raise_mode { Tool::RaiseLower } else { Tool::Brush };
                }
            }
            Size => self.brush.radius = (v * 0.5).clamp(0.25, 128.0) as f32,
            EaseValue => self.bridge_ease = v as f32,
            Ceiling => self.ceiling = v as f32,
            Height => self.level = v as f32,
            Floor => self.floor = v as f32,
            Strength if self.tool == Tool::RaiseLower => self.raise_strength = (v as f32).clamp(0.05, 200.0),
            Strength if self.tool == Tool::Gradient => self.gradient_strength = (v as f32).clamp(0.005, 1.0),
            Strength => self.smooth_strength = (v as f32).clamp(0.005, 1.0),
            Flatten => self.flatten = v != 0.0,
            Falloff => self.brush.falloff = v != 0.0,
            GridAligned => self.brush.grid_aligned = v != 0.0,
            GridSize => self.brush.grid_size = (v as i32).clamp(0, 20),
            MaskMode => self.mask_mode = v != 0.0,
            Material => self.paint_slot = (v as i64).clamp(0, 15) as u8,
            QuickPaint => self.quick_paint = (v as usize).min(QUICK_PAINTS.len() - 1),
            PaintWalls => self.paint_walls = v != 0.0,
            TextureName | TexturePreset => {}
            Aspect => self.brush.aspect = (v as f32).clamp(0.05, 20.0),
            Rotation => self.brush.rotation = (v as f32 + 180.0).rem_euclid(360.0) - 180.0,
            Peak => self.brush.peak = (v as f32).clamp(-1.0, 1.0),
            SlopeMin => self.slope_min = (v as f32).clamp(0.0, 90.0),
            SlopeMax => self.slope_max = (v as f32).clamp(0.0, 90.0),
            ScatterItem => self.scatter_item = v.max(0.0) as usize,
            ScatterMode => self.scatter_per_cell = v != 0.0,
            EraseAll => self.erase_all = v != 0.0,
            StampImage => self.stamp = v.max(0.0) as usize,
            StampMode => self.stamp_mode = (v as i64).clamp(0, pt_core::stamp::STAMP_MODES_ALL.len() as i64 - 1),
            StampSize => self.stamp_size = (v as f32).clamp(0.5, 4096.0),
            StampAspect => self.stamp_aspect = (v as f32).clamp(0.05, 20.0),
            StampHeight => self.stamp_height = v as f32,
            StampRotation => self.stamp_rotation = (v as f32 + 360.0).rem_euclid(720.0) - 360.0,
            StampContrast => self.stamp_contrast = (v as f32).clamp(0.05, 20.0),
            StampSnap => self.stamp_snap = v != 0.0,
            PasteRotation => self.paste_rotation = (v as f32).rem_euclid(360.0),
            PasteMirror => self.paste_mirror = v != 0.0,
            RelativeHeight => self.relative_height = v != 0.0,
            StampLimits => self.stamp_limits = v != 0.0,
            TintColor => self.tint_color = (v.max(0.0) as usize).min(TINT_COLORS.len() - 1),
            TintOpacity => self.tint_opacity = (v as f32).clamp(0.0, 1.0),
            ScatterAdd | ScatterFill | ScatterClear | ScatterRemove | Copy | Paste => {}
        }
    }
}
