use pt_core::mst_import::{LegacyAsset, legacy_asset};
pub fn bundled_asset(legacy: LegacyAsset) -> &'static str {
    match legacy {
        LegacyAsset::NoiseTexture => "res://addons/MarchingSquaresTerrain/textures/ground/mst_noise.png",
        LegacyAsset::LeafSprite => "res://addons/MarchingSquaresTerrain/textures/grass_blade.png",
        LegacyAsset::VoidTexture => "res://addons/MarchingSquaresTerrain/textures/ground/void.png",
    }
}
pub fn bundled_replacement(original: &str) -> Option<&'static str> {
    legacy_asset(original).map(bundled_asset)
}
#[cfg(test)]
mod tests {
    use super::*;
    use pt_core::mst_import::{ORIG_GROUND_NOISE, ORIG_LEAF_SPRITE, ORIG_WALL_NOISE};
    #[test]
    fn legacy_materials_map_to_bundled_textures() {
        assert_eq!(
            bundled_replacement(ORIG_LEAF_SPRITE),
            Some("res://addons/MarchingSquaresTerrain/textures/grass_blade.png")
        );
        assert_eq!(bundled_replacement(ORIG_WALL_NOISE), bundled_replacement(ORIG_GROUND_NOISE));
        assert_eq!(bundled_replacement("res://art/rock.png"), None);
    }
}
