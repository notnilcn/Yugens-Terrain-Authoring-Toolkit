use godot::classes::{EditorInspector, EditorInspectorPlugin, IEditorInspectorPlugin, Object};
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyUsageFlags};
const LABELS: &[(&str, &str, &str)] = &[
    ("MarchingSquaresAutoRules", "filler_amount", "Amount"),
    ("MarchingSquaresAutoRules", "lip_threshold", "Lip Cutoff"),
    (
        "MarchingSquaresAutoRules",
        "rock_filler_slot",
        "Filler Slot",
    ),
    (
        "MarchingSquaresAutoRules",
        "shore_filler_slot",
        "Filler Slot",
    ),
    ("MarchingSquaresAutoRules", "snow_max_slope", "Max Slope"),
    (
        "MarchingSquaresAutoRules",
        "snow_noise_scale",
        "Noise Scale",
    ),
    ("MarchingSquaresAutoRules", "snow_slope_fade", "Slope Fade"),
    ("MarchingSquaresErosion", "carve_rate", "Carve"),
    ("MarchingSquaresErosion", "drop_rate", "Drop"),
    ("MarchingSquaresErosion", "droplet_steps", "Steps"),
    ("MarchingSquaresErosion", "dry_rate", "Dry"),
    ("MarchingSquaresErosion", "load_limit", "Load"),
    ("MarchingSquaresErosion", "min_load", "Min Load"),
    ("MarchingSquaresErosion", "min_plateau_cells", "Min Plateau"),
    ("MarchingSquaresErosion", "momentum", "Momentum"),
    ("MarchingSquaresErosion", "pixels_per_cell", "Pixels/Cell"),
    ("MarchingSquaresErosion", "splash_radius", "Radius"),
    ("MarchingSquaresErosion", "start_momentum", "Start Speed"),
    ("MarchingSquaresErosion", "start_volume", "Start Volume"),
    ("MarchingSquaresTerrain", "chunk_handles", "Handles"),
    (
        "MarchingSquaresTerrain",
        "grass_ledge_threshold",
        "Ledge Cutoff",
    ),
    ("MarchingSquaresTerrain", "grid_fade_distance", "Fade Range"),
    ("MarchingSquaresTerrain", "highlight_width", "Highlight"),
    ("MarchingSquaresTerrain", "lod_in_editor", "In Editor"),
    ("MarchingSquaresTerrain", "lod_distance", "Distance"),
    ("MarchingSquaresTerrain", "map_window_margin", "Margin"),
    ("MarchingSquaresTerrain", "map_window_radius", "Radius"),
    ("MarchingSquaresTerrain", "map_window_focus", "Focus"),
    (
        "MarchingSquaresTerrain",
        "publish_height_texture",
        "Height Tex",
    ),
    ("MarchingSquaresTerrain", "render_layers", "Layers"),
    ("MarchingSquaresTerrain", "scatter_items", "Items"),
    ("MarchingSquaresTerrain", "shader_cursor", "Cursor"),
    (
        "MarchingSquaresTerrain",
        "show_grid_in_editor",
        "Editor Grid",
    ),
    ("MarchingSquaresTerrain", "storage_mode", "Mode"),
    ("MarchingSquaresWildFlowers", "bloom_threshold", "Threshold"),
    (
        "MarchingSquaresWildFlowers",
        "ground_water_variation",
        "Variation",
    ),
    (
        "MarchingSquaresWildFlowers",
        "points_per_cell",
        "Points/Cell",
    ),
    (
        "MarchingSquaresWildFlowers",
        "shore_distance",
        "Shore Range",
    ),
    ("MarchingSquaresWildFlowers", "shore_wetness", "Wetness"),
    ("MarchingSquaresWildFlowers", "species_items", "Species"),
    ("PixelBirds", "camera_clearance", "Camera Gap"),
    ("PixelBirds", "land_interval", "Interval"),
    ("PixelBirds", "neighbor_distance", "Neighbors"),
    ("PixelBirds", "perch_seconds", "Perch (s)"),
    ("PixelBirds", "scare_distance", "Scare Range"),
    ("PixelBuilderMesh", "impact_stratum", "Layer"),
    ("PixelBuilderMesh", "lightmap_texel", "UV2 Texel"),
    ("PixelBuilderMeshData", "units_per_repeat", "Units/Repeat"),
    ("PixelDayNight", "ambient_gradient", "Ambient Ramp"),
    ("PixelDayNight", "day_length_seconds", "Day Length"),
    ("PixelDayNight", "fog_density_curve", "Density"),
    ("PixelDayNight", "fog_night_density", "Night"),
    ("PixelDayNight", "latitude_tilt_deg", "Latitude"),
    ("PixelDayNight", "azimuth_deg", "Azimuth"),
    ("PixelDayNight", "sun_energy_curve", "Energy Curve"),
    ("PixelDayNight", "sun_gradient", "Sun Ramp"),
    ("PixelDayNight", "sky_gradient", "Sky Ramp"),
    ("PixelDayNight", "time_override", "Override"),
    ("PixelDiagnostics", "sections_start_folded", "Folded"),
    ("PixelDiagnostics", "show_object_panel", "Object Panel"),
    ("PixelFireSystem", "ember_distance", "Ember Range"),
    ("PixelFireSystem", "glow_intensity", "Glow"),
    ("PixelFireSystem", "ignite_radius", "Ignite Range"),
    ("PixelFireSystem", "preview_in_editor", "Preview"),
    ("PixelFireSystem", "rain_override", "Rain"),
    ("PixelFireSystem", "wet_regrow_boost", "Wet Regrow"),
    ("PixelFireSystem", "wind_influence", "Influence"),
    ("PixelFireSystem", "wind_strength", "Strength"),
    ("PixelObject", "convert_materials", "Convert"),
    ("PixelObject", "contour_tint", "Outline"),
    ("PixelObject", "supersede_contour_tint", "Own Outline"),
    ("PixelObject", "snap_to_texel", "Texel Snap"),
    ("PixelObjectProfile", "contour_tint", "Outline"),
    (
        "PixelObjectProfile",
        "supersede_contour_tint",
        "Own Outline",
    ),
    ("PixelOutlineColors", "silhouette_colors", "Silhouette"),
    ("PixelOutlineColors", "edge_colors", "Edge"),
    ("PixelOutlineColors", "modulate_rows", "Modulate"),
    ("PixelPalette", "brightness_curve", "Brightness"),
    ("PixelParticleField", "particle_count", "Count"),
    ("PixelParticleField", "streak_length", "Streak"),
    ("PixelParticleField", "weather_response", "Weather"),
    ("PixelRenderer", "auto_id_first", "First Id"),
    ("PixelRenderer", "depth_threshold", "Depth Edge"),
    ("PixelRenderer", "dither_enabled", "Dither"),
    ("PixelRenderer", "dither_spread", "Spread"),
    ("PixelRenderer", "studio_prelook", "Preview"),
    ("PixelRenderer", "normal_threshold", "Normal Edge"),
    ("PixelRenderer", "object_outline", "Outline"),
    ("PixelRenderer", "object_pixel_size", "Pixel Size"),
    ("PixelRenderer", "palette_enabled", "Palette"),
    ("PixelRenderer", "cloud_softness", "Cloud Blur"),
    ("PixelRenderer", "sheet_falloff", "Falloff"),
    ("PixelRenderer", "sun_color", "Sun Color"),
    ("PixelRenderer", "crisp_edges", "Crisp Edges"),
    ("PixelRenderer", "subtexel_scroll", "Sub-texel Scroll"),
    ("PixelScatterItem", "align_to_normal", "Align Normal"),
    ("PixelScatterItem", "cluster_scale", "Cluster Size"),
    ("PixelScatterItem", "cluster_threshold", "Threshold"),
    ("PixelScatterItem", "impact_stratum", "Layer"),
    ("PixelScatterItem", "collision_min_scale", "Min Scale"),
    ("PixelScatterItem", "collision_size", "Size"),
    ("PixelSky", "cloud_camera_scope", "Cloud Cam"),
    ("PixelSky", "clouds_enabled", "Clouds"),
    ("PixelSky", "shadow_strength", "Shadows"),
    ("PixelSmoke", "glow_intensity", "Glow"),
    ("PixelSmoke", "wind_strength", "Wind"),
    ("PixelSmoke", "use_sky_wind", "Sky Wind"),
    ("PixelFireSystem", "use_sky_wind", "Sky Wind"),
    ("PixelWeather", "ground_seconds", "Ground (s)"),
    ("PixelWeather", "lightning_enabled", "Enabled"),
    ("PixelWeather", "rain_layer_spacing", "Spacing"),
    ("PixelWeather", "snow_accumulation", "Accumulate"),
    ("PixelWeather", "snow_big_flakes", "Big Flakes"),
    ("PixelWeather", "snow_intensity", "Intensity"),
    ("PixelWeather", "snow_max_cover", "Max Cover"),
    ("PixelWeather", "snow_enabled", "Enabled"),
    ("PixelWeather", "splash_radius", "Splash Size"),
    ("PixelWeather", "streak_texels", "Streak Size"),
    ("PixelWeather", "flake_texels", "Flake Size"),
    ("PixelWeather", "transition_seconds", "Blend (s)"),
    ("PixelWeatherPreset", "ambient_scale", "Ambient"),
    ("PixelWeatherPreset", "cloud_coverage", "Coverage"),
    ("PixelWeatherPreset", "cloud_softness", "Softness"),
    ("PixelWeatherPreset", "cloud_shadow", "Shadow"),
    ("PixelWeatherPreset", "deck_opacity", "Deck Opacity"),
    ("PixelWeatherPreset", "fog_color_mix", "Fog Mix"),
];
thread_local! {
    static IN_PARSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn short_label(class: &str, prop: &str) -> Option<&'static str> {
    super::pixel_ui::sections::strip_label(class, prop)
        .or_else(|| {
            if class == "MarchingSquaresTerrain" {
                super::terrain_ui::terrain_setting_label(prop)
            } else {
                None
            }
        })
        .or_else(|| {
            LABELS
                .iter()
                .find(|(c, p, _)| *c == class && *p == prop)
                .map(|(_, _, l)| *l)
        })
}
#[derive(GodotClass)]
#[class(tool, init, base = EditorInspectorPlugin, internal)]
pub struct MstInspectorLabels {
    base: Base<EditorInspectorPlugin>,
}
#[godot_api]
impl IEditorInspectorPlugin for MstInspectorLabels {
    fn can_handle(&self, entity: Option<Gd<Object>>) -> bool {
        let Some(o) = entity else { return false };
        let class = o.get_class().to_string();
        class == "MarchingSquaresTerrain"
            || super::pixel_ui::sections::has_strip_fields(&class)
            || LABELS.iter().any(|(c, _, _)| *c == class)
    }
    fn parse_property(
        &mut self,
        entity: Option<Gd<Object>>,
        type_: VariantType,
        moniker: GString,
        hint_type: PropertyHint,
        hint_string: GString,
        usage_flags: PropertyUsageFlags,
        wide: bool,
    ) -> bool {
        let Some(o) = entity else { return false };
        if IN_PARSE.get() {
            return false;
        }
        let prop = moniker.to_string();
        let Some(label) = short_label(&o.get_class().to_string(), &prop) else {
            return false;
        };
        let ed = {
            let _unbound = self.base_mut();
            IN_PARSE.set(true);
            let ed = EditorInspector::instantiate_property_editor_ex(
                &o,
                type_,
                &moniker,
                hint_type,
                &hint_string,
                usage_flags.ord() as u32,
            )
            .wide(wide)
            .done();
            IN_PARSE.set(false);
            ed
        };
        let Some(mut ed) = ed else { return false };
        let full: String = prop
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        ed.set_tooltip_text(&format!("{full} ({prop})"));
        self.base_mut()
            .add_property_editor_ex(&moniker, &ed)
            .label(label)
            .done();
        true
    }
}
