//! Short inspector labels for long property names.
//!
//! Godot labels a property with its capitalised name, which a narrow inspector dock cuts off
//! ("Sediment Capacity Fac…"). Where a group / subgroup prefix cannot shorten the name in a
//! readable way, this inspector plugin shows Godot's own editor for the property under a shorter
//! label. Saved property names are unchanged, and the tooltip names the full property.

use godot::classes::{EditorInspector, EditorInspectorPlugin, IEditorInspectorPlugin, Object};
use godot::register::info::{PropertyHint, PropertyUsageFlags};
use godot::prelude::*;

/// (class, property, label).
const LABELS: &[(&str, &str, &str)] = &[
    ("MarchingSquaresAutoRules", "filler_amount", "Amount"),
    ("MarchingSquaresAutoRules", "lip_threshold", "Lip Cutoff"),
    ("MarchingSquaresAutoRules", "rock_filler_slot", "Filler Slot"),
    ("MarchingSquaresAutoRules", "shore_filler_slot", "Filler Slot"),
    ("MarchingSquaresAutoRules", "snow_max_slope", "Max Slope"),
    ("MarchingSquaresAutoRules", "snow_noise_scale", "Noise Scale"),
    ("MarchingSquaresAutoRules", "snow_slope_fade", "Slope Fade"),
    ("MarchingSquaresErosion", "deposit_speed", "Deposit"),
    ("MarchingSquaresErosion", "erosion_radius", "Radius"),
    ("MarchingSquaresErosion", "evaporate_speed", "Evaporation"),
    ("MarchingSquaresErosion", "initial_speed", "Start Speed"),
    ("MarchingSquaresErosion", "initial_water_volume", "Start Water"),
    ("MarchingSquaresErosion", "max_droplet_lifetime", "Max Lifetime"),
    ("MarchingSquaresErosion", "min_plateau_cells", "Min Plateau"),
    ("MarchingSquaresErosion", "min_sediment_capacity", "Min Capacity"),
    ("MarchingSquaresErosion", "pixels_per_cell", "Pixels/Cell"),
    ("MarchingSquaresErosion", "sediment_capacity_factor", "Capacity"),
    ("MarchingSquaresTerrain", "chunk_handles", "Handles"),
    ("MarchingSquaresTerrain", "grass_ledge_threshold", "Ledge Cutoff"),
    ("MarchingSquaresTerrain", "grid_fade_distance", "Fade Range"),
    ("MarchingSquaresTerrain", "highlight_width", "Highlight"),
    ("MarchingSquaresTerrain", "lod_in_editor", "In Editor"),
    ("MarchingSquaresTerrain", "lod_distance", "Distance"),
    ("MarchingSquaresTerrain", "map_window_margin", "Margin"),
    ("MarchingSquaresTerrain", "map_window_radius", "Radius"),
    ("MarchingSquaresTerrain", "map_window_focus", "Focus"),
    ("MarchingSquaresTerrain", "publish_height_texture", "Height Tex"),
    ("MarchingSquaresTerrain", "render_layers", "Layers"),
    ("MarchingSquaresTerrain", "scatter_items", "Items"),
    ("MarchingSquaresTerrain", "shader_cursor", "Cursor"),
    ("MarchingSquaresTerrain", "show_grid_in_editor", "Editor Grid"),
    ("MarchingSquaresTerrain", "storage_mode", "Mode"),
    ("MarchingSquaresWildFlowers", "flower_threshold", "Threshold"),
    ("MarchingSquaresWildFlowers", "ground_water_variation", "Variation"),
    ("MarchingSquaresWildFlowers", "points_per_cell", "Points/Cell"),
    ("MarchingSquaresWildFlowers", "shore_distance", "Shore Range"),
    ("MarchingSquaresWildFlowers", "shore_wetness", "Wetness"),
    ("MarchingSquaresWildFlowers", "species_items", "Species"),
    ("PixelBirds", "camera_clearance", "Camera Gap"),
    ("PixelBirds", "land_interval", "Interval"),
    ("PixelBirds", "neighbor_distance", "Neighbors"),
    ("PixelBirds", "perch_seconds", "Perch (s)"),
    ("PixelBirds", "scare_distance", "Scare Range"),
    ("PixelBuilderMesh", "collision_layer", "Layer"),
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
    ("PixelDiagnostics", "start_visible", "Visible"),
    ("PixelFireSystem", "ember_distance", "Ember Range"),
    ("PixelFireSystem", "glow_intensity", "Glow"),
    ("PixelFireSystem", "ignite_radius", "Ignite Range"),
    ("PixelFireSystem", "preview_in_editor", "Preview"),
    ("PixelFireSystem", "rain_override", "Rain"),
    ("PixelFireSystem", "wet_regrow_boost", "Wet Regrow"),
    ("PixelFireSystem", "wind_influence", "Influence"),
    ("PixelFireSystem", "wind_strength", "Strength"),
    ("PixelObject", "convert_materials", "Convert"),
    ("PixelObject", "outline_color", "Outline"),
    ("PixelObject", "override_outline_color", "Own Outline"),
    ("PixelObject", "snap_to_texel", "Texel Snap"),
    ("PixelObjectProfile", "outline_color", "Outline"),
    ("PixelObjectProfile", "override_outline_color", "Own Outline"),
    ("PixelOrbitCamera", "mouse_rotate_deg", "Mouse Turn"),
    ("PixelOrbitCamera", "rotate_speed_deg", "Turn Speed"),
    ("PixelOrbitCamera", "yaw_snap_deg", "Yaw Snap"),
    ("PixelOutlinePalette", "exterior_colors", "Exterior"),
    ("PixelOutlinePalette", "interior_colors", "Interior"),
    ("PixelOutlinePalette", "multiply_colors", "Multiply"),
    ("PixelPalette", "brightness_curve", "Brightness"),
    ("PixelParticleField", "particle_count", "Count"),
    ("PixelParticleField", "streak_length", "Streak"),
    ("PixelParticleField", "weather_response", "Weather"),
    ("PixelRenderer", "auto_id_first", "First Id"),
    ("PixelRenderer", "depth_threshold", "Depth Edge"),
    ("PixelRenderer", "dither_enabled", "Dither"),
    ("PixelRenderer", "dither_spread", "Spread"),
    ("PixelRenderer", "normal_threshold", "Normal Edge"),
    ("PixelRenderer", "palette_enabled", "Palette"),
    ("PixelRenderer", "ray_sheet_cloud_softness", "Cloud Blur"),
    ("PixelRenderer", "ray_sheet_distance_falloff", "Falloff"),
    ("PixelRenderer", "ray_use_sun_color", "Sun Color"),
    ("PixelRenderer", "sharp_upscale", "Sharp Scale"),
    ("PixelRenderer", "smooth_scroll", "Smooth Pan"),
    ("PixelScatterItem", "align_to_normal", "Align Normal"),
    ("PixelScatterItem", "cluster_scale", "Cluster Size"),
    ("PixelScatterItem", "cluster_threshold", "Threshold"),
    ("PixelScatterItem", "collision_layer", "Layer"),
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
    /// Set while `parse_property` asks Godot for the stock editor of a property.
    static IN_PARSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Label of `class.prop`, if it has its own (exact class, not subclasses): the strip label
/// first (Pixel World sections, Terrain Settings), so each property has one name in both
/// places, then `LABELS`.
pub fn short_label(class: &str, prop: &str) -> Option<&'static str> {
    super::pixel_ui::sections::strip_label(class, prop)
        .or_else(|| if class == "MarchingSquaresTerrain" { super::terrain_ui::terrain_setting_label(prop) } else { None })
        .or_else(|| LABELS.iter().find(|(c, p, _)| *c == class && *p == prop).map(|(_, _, l)| *l))
}

#[derive(GodotClass)]
#[class(tool, init, base = EditorInspectorPlugin, internal)]
pub struct MstInspectorLabels {
    base: Base<EditorInspectorPlugin>,
}

#[godot_api]
impl IEditorInspectorPlugin for MstInspectorLabels {
    fn can_handle(&self, object: Option<Gd<Object>>) -> bool {
        let Some(o) = object else { return false };
        let class = o.get_class().to_string();
        class == "MarchingSquaresTerrain" || super::pixel_ui::sections::has_strip_fields(&class) || LABELS.iter().any(|(c, _, _)| *c == class)
    }

    fn parse_property(
        &mut self,
        object: Option<Gd<Object>>,
        type_: VariantType,
        name: GString,
        hint_type: PropertyHint,
        hint_string: GString,
        usage_flags: PropertyUsageFlags,
        wide: bool,
    ) -> bool {
        let Some(o) = object else { return false };
        // `instantiate_property_editor` asks every inspector plugin again, this one included:
        // the nested call must decline (else it recurses) and must find the instance unbound.
        if IN_PARSE.get() {
            return false;
        }
        let prop = name.to_string();
        let Some(label) = short_label(&o.get_class().to_string(), &prop) else { return false };
        let ed = {
            // `base_mut()` releases the `&mut self` borrow for the duration of the call, so
            // Godot's re-entrant `can_handle` / `parse_property` can bind this instance.
            let _unbound = self.base_mut();
            IN_PARSE.set(true);
            let ed = EditorInspector::instantiate_property_editor_ex(&o, type_, &name, hint_type, &hint_string, usage_flags.ord() as u32).wide(wide).done();
            IN_PARSE.set(false);
            ed
        };
        let Some(mut ed) = ed else { return false };
        let full: String = prop.split('_').map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }).collect::<Vec<_>>().join(" ");
        ed.set_tooltip_text(&format!("{full} ({prop})"));
        self.base_mut().add_property_editor_ex(&name, &ed).label(label).done();
        true
    }
}
