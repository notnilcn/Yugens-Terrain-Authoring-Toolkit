//! The engine-facing names for the terrain's cell layout and hex edge look, so the integer a
//! scene stores, the Inspector's choices and the plain meshing types can only disagree as a
//! build or test failure.
use godot::prelude::*;
use pt_core::cells::EdgeStyle;
use pt_core::grid::GridType;

#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[godot(via = i32)]
pub enum TerrainGridType {
    #[default]
    Square,
    Triangle,
    Hex,
    HexRings,
}
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[godot(via = i32)]
pub enum HexEdgeStyle {
    #[default]
    FlatWalls,
    Terraces,
    Slopes,
    HexPrism,
}
impl From<TerrainGridType> for GridType {
    fn from(v: TerrainGridType) -> GridType {
        match v {
            TerrainGridType::Square => GridType::Square,
            TerrainGridType::Triangle => GridType::Triangle,
            TerrainGridType::Hex => GridType::Hex,
            TerrainGridType::HexRings => GridType::HexRings,
        }
    }
}
impl From<GridType> for TerrainGridType {
    fn from(v: GridType) -> TerrainGridType {
        match v {
            GridType::Square => TerrainGridType::Square,
            GridType::Triangle => TerrainGridType::Triangle,
            GridType::Hex => TerrainGridType::Hex,
            GridType::HexRings => TerrainGridType::HexRings,
        }
    }
}
impl From<HexEdgeStyle> for EdgeStyle {
    fn from(v: HexEdgeStyle) -> EdgeStyle {
        match v {
            HexEdgeStyle::FlatWalls => EdgeStyle::FlatWalls,
            HexEdgeStyle::Terraces => EdgeStyle::Terraces,
            HexEdgeStyle::Slopes => EdgeStyle::Slopes,
            HexEdgeStyle::HexPrism => EdgeStyle::HexPrism,
        }
    }
}
impl From<EdgeStyle> for HexEdgeStyle {
    fn from(v: EdgeStyle) -> HexEdgeStyle {
        match v {
            EdgeStyle::FlatWalls => HexEdgeStyle::FlatWalls,
            EdgeStyle::Terraces => HexEdgeStyle::Terraces,
            EdgeStyle::Slopes => HexEdgeStyle::Slopes,
            EdgeStyle::HexPrism => HexEdgeStyle::HexPrism,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_grid_integers_match_the_core_layout_numbers() {
        for g in [
            GridType::Square,
            GridType::Triangle,
            GridType::Hex,
            GridType::HexRings,
        ] {
            assert_eq!(TerrainGridType::from(g) as i32, g as i32, "{g:?}");
            assert_eq!(GridType::from(TerrainGridType::from(g)), g);
        }
    }
    #[test]
    fn stored_edge_integers_match_the_core_edge_numbers() {
        for e in [
            EdgeStyle::FlatWalls,
            EdgeStyle::Terraces,
            EdgeStyle::Slopes,
            EdgeStyle::HexPrism,
        ] {
            assert_eq!(HexEdgeStyle::from(e) as i32, e as i32, "{e:?}");
            assert_eq!(EdgeStyle::from(HexEdgeStyle::from(e)), e);
        }
    }
}
