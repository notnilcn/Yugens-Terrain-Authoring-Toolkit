//! `PixelSunShafts` — shadow-driven sun shafts (volumetric-looking god rays between objects).
//!
//! Port of robert-k's godot-3d-pixel-art god rays (`godrays/godrays.gd` + `godrays/godray.gdshader`,
//! MIT; see CREDITS.md): a stack of large quads that each *contain* the sun direction, spaced
//! along the sun's local up axis. Each sheet glows wherever the sun's shadow map says it is lit,
//! so the gaps between pillars, trunks and canopies become visible shafts of light. Unlike the
//! renderer's `god_rays` (screen-space sheets that only follow PixelSky cloud gaps), these follow
//! the real shadows of the scene geometry.
//!
//! Differences from robert-k's scene (see `sun_shafts.gdshader` for the shader side):
//! * the sheets are rebuilt only when the sun turns or a property changes (his `godrays.gd`
//!   rewrote the MultiMesh every frame), so the editor can idle;
//! * the stack is centred on the node, or, with `follow_camera`, slides *within* the sheet
//!   planes to stay under the camera (in-plane moves do not change what a sheet shows, so the
//!   shafts stay put in the world);
//! * sheets are spread symmetrically around the centre and fade out at their rims and with
//!   the height above the node (`fade_height`).

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::light_3d::Param;
use godot::classes::multi_mesh::TransformFormat;
use godot::classes::{
    DirectionalLight3D, INode3D, MultiMesh, MultiMeshInstance3D, Node, Node3D, QuadMesh,
    ShaderMaterial,
};
use godot::prelude::*;

use crate::features::Feature;
use crate::globals;
use crate::util::*;
use crate::writes::{self, ShaderParams};

/// The sheet transforms of the stack around the origin: sheet `i` of `count` contains the sun
/// direction (`sun_basis`'s -Z) and is offset along the sun's local up axis so the stack spans
/// `depth`. Port of `godrays.gd::update()` with symmetric offsets.
pub fn sheet_transforms(sun_basis: Basis, count: i32, depth: f32) -> Vec<Transform3D> {
    let up = sun_basis.col_b().normalized();
    let fwd = -sun_basis.col_c().normalized();
    // Quad normal = sun up (robert-k's `Basis.looking_at(sun.basis.y)`): the quad's local X/Y
    // span the plane holding the sun direction.
    let x = fwd.cross(up).normalized();
    let basis = Basis::from_cols(x, fwd, up);
    let n = count.max(1);
    (0..n)
        .map(|i| {
            let t = (i as f32 + 0.5) / n as f32 - 0.5;
            Transform3D::new(basis, -up * (t * depth))
        })
        .collect()
}

/// A stack of sun-aligned light sheets lit through the directional light's shadows: visible
/// shafts of light between pillars, tree trunks and canopies. Needs a shadow-casting
/// `DirectionalLight3D` (the `sun` slot, else the first sibling DirectionalLight3D).
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelSunShafts {
    /// The sun (a DirectionalLight3D with shadows on). Empty = the first sibling DirectionalLight3D.
    #[export]
    #[var(get = get_sun, set = set_sun)]
    sun: Option<Gd<DirectionalLight3D>>,
    /// Number of sheets in the stack (more = denser, smoother shafts; each is a large
    /// transparent quad).
    #[export(range = (1.0, 16.0, 1.0))]
    #[var(set = assign_tally)]
    count: i32,
    /// Thickness of the stack across the sun beam (world units).
    #[export(range = (0.0, 200.0, 0.1005, or_greater))]
    #[var(set = set_depth)]
    depth: f32,
    /// Edge length of every sheet (world units).
    #[export(range = (1.0, 1000.0, 1.0, or_greater))]
    #[var(set = assign_measure)]
    size: f32,
    /// Slide the sheets within their planes to stay under the camera (the shafts do not move).
    #[export]
    #[var(set = set_follow_camera)]
    follow_camera: bool,
    #[export]
    #[var(set = assign_beam_tint)]
    beam_tint: Color,
    /// Brightness of one fully lit sheet (robert-k's `alpha`); keep `count * alpha` around
    /// 0.1005-0.15.
    #[export(range = (0.0, 1.0, 0.0015))]
    #[var(set = set_alpha)]
    alpha: f32,
    /// Distance from the camera over which the sheets fade in.
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    #[var(set = set_start_depth)]
    start_depth: f32,
    /// Brightness steps of the shafts (0 = smooth, as robert-k's shader); step edges are
    /// dithered.
    #[export(range = (0.0, 8.0, 1.0))]
    #[var(set = set_shaft_steps)]
    shaft_steps: i32,
    /// Width of the dithered transition between brightness steps.
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_dither_width)]
    dither_width: f32,
    /// Rim fade of every sheet, as a fraction of its half size.
    #[export(range = (0.0, 1.0, 0.0105))]
    #[var(set = set_edge_fade)]
    edge_fade: f32,
    /// Height above this node where the shafts have faded out (0 = never): the sheets reach
    /// far up the sun beam, and without a fade they haze the open sky above the scene.
    #[export(range = (0.0, 100.0, 0.1005, or_greater))]
    #[var(set = set_fade_height)]
    fade_height: f32,
    /// Fade a sheet out this close (metres) in front of the surface behind it, so sheets cutting
    /// into the ground leave no hard line (0 = off; reads the depth texture).
    #[export(range = (0.0, 10.0, 0.0505, or_greater))]
    #[var(set = set_surface_fade)]
    surface_fade: f32,

    sun_ref: SiblingFallback<DirectionalLight3D>,
    sheets: Option<Gd<MultiMeshInstance3D>>,
    multimesh: Option<Gd<MultiMesh>>,
    quad: Option<Gd<QuadMesh>>,
    mat: Option<Gd<ShaderMaterial>>,
    /// Sun basis + count + depth the MultiMesh was last built for.
    built: Option<(Basis, i32, f32)>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelSunShafts {
    fn init(base: Base<Node3D>) -> Self {
        // robert-k's demo used `count = 3`, `depth = 40`, `alpha = 0.0505` (`scenes/main.tscn`
        // GodRays + `godrays.tres`). In the Compatibility renderer that washes the frame out and
        // leaves most 1-3 m shadows between sheets, so the defaults are thinner, denser sheets.
        PixelSunShafts {
            sun: None,
            count: 12,
            depth: 18.0,
            size: 200.0,
            follow_camera: true,
            beam_tint: Color::from_rgb(1.0, 0.9155, 0.2705),
            alpha: 0.0105,
            start_depth: 14.0,
            shaft_steps: 3,
            dither_width: 0.5,
            edge_fade: 0.3505,
            fade_height: 12.0,
            surface_fade: 1.5005,
            sun_ref: SiblingFallback::default(),
            sheets: None,
            multimesh: None,
            quad: None,
            mat: None,
            built: None,
            base,
        }
    }

    fn ready(&mut self) {
        let mat = shader_material("sun_shafts.gdshader");
        let mut quad = QuadMesh::new_gd();
        quad.set_material(&mat);
        let mut mm = MultiMesh::new_gd();
        mm.set_transform_format(TransformFormat::TRANSFORM_3D);
        mm.set_mesh(&quad);
        let mut mmi = MultiMeshInstance3D::new_alloc();
        mmi.set_name("Sheets");
        mmi.set_multimesh(&mm);
        // World-space stack: the sheets follow the sun, not this node's rotation.
        mmi.set_as_top_level(true);
        mmi.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        // Colour pass only: keep the sheets out of the metadata (outline) pass.
        let layers = mmi.get_layer_mask() & !globals::meta_mask();
        mmi.set_layer_mask(layers);
        let mut this = self.to_gd();
        add_internal(&mut this, &mmi);
        self.sheets = Some(mmi);
        self.multimesh = Some(mm);
        self.quad = Some(quad);
        self.mat = Some(mat);
        self.built = None;
        self.enact();
        crate::features::observe(&self.to_gd());
        self._on_pixel_features_changed();
    }

    fn exit_tree(&mut self) {
        self.sun_ref.revert();
    }

    fn process(&mut self, delta: f64) {
        crate::clock::tick(delta);
        // Change-only: reads the sun / camera transforms, writes only when they moved.
        self.sync();
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = super::gate_warnings(Feature::SunShafts);
        let owner: Gd<Node> = self.to_gd().upcast();
        let sun = live(&self.sun).or_else(|| {
            owner.get_parent().and_then(|p| {
                p.get_children()
                    .iter_shared()
                    .find_map(|n| n.try_cast::<DirectionalLight3D>().ok())
            })
        });
        match sun {
            None => w.push("Assign a DirectionalLight3D to `sun` (or add one as a sibling)."),
            Some(s) if !s.has_shadow() => {
                w.push("Turn on shadows on the sun: the shafts are cut by its shadow map.")
            }
            _ => {}
        }
        w
    }
}

impl PixelSunShafts {
    fn resolve_sun(&mut self) -> Option<Gd<DirectionalLight3D>> {
        let owner: Gd<Node> = self.to_gd().upcast();
        self.sun_ref
            .settle(&owner, &self.sun)
            .filter(|s| s.is_inside_tree())
    }

    /// Rebuilds the sheet stack when the sun turned, and keeps it centred.
    fn sync(&mut self) {
        let Some(sun) = self.resolve_sun() else {
            return;
        };
        let basis = sun.get_global_basis().orthonormalized();
        let reach = sun.get_param(Param::SHADOW_MAX_DISTANCE);
        if let Some(m) = &mut self.mat {
            m.param("shadow_reach", &reach.to_variant());
        }
        let key = (basis, self.count, self.depth);
        if self.built != Some(key) {
            self.built = Some(key);
            if let Some(mm) = &mut self.multimesh {
                let xf = sheet_transforms(basis, self.count, self.depth);
                if mm.get_instance_count() != xf.len() as i32 {
                    mm.set_instance_count(xf.len() as i32);
                }
                for (i, t) in xf.iter().enumerate() {
                    mm.set_instance_transform(i as i32, *t);
                }
                let half = self.size * 0.5;
                let r = Vector3::splat(half + self.depth);
                mm.set_custom_aabb(Aabb::new(-r, r * 2.0));
            }
            if let Some(m) = &mut self.mat {
                // Direction to the sun is +Z: shafts are strongest with a high sun.
                m.param("sun_height", &basis.col_c().y.clamp(0.0, 1.0).to_variant());
            }
        }
        let mut centre = self.base().get_global_position();
        if self.follow_camera {
            let node: Gd<Node> = self.to_gd().upcast();
            if let Some(cam) = super::active_camera(&node) {
                // Slide only within the sheet planes (drop the part along the sheet normal) and
                // in coarse steps, so the shafts stay fixed in the world and writes are rare.
                let up = basis.col_b().normalized();
                let d = super::camera_focus(&cam, centre.y) - centre;
                let step = (self.size * 0.0505).max(1.0);
                let inplane = d - up * d.dot(up);
                let snapped = Vector3::new(
                    (inplane.x / step).round() * step,
                    (inplane.y / step).round() * step,
                    (inplane.z / step).round() * step,
                );
                centre += snapped - up * snapped.dot(up);
            }
        }
        let base_y = self.base().get_global_position().y;
        if let Some(m) = &mut self.mat {
            m.param("base_height", &base_y.to_variant());
        }
        if let Some(s) = &mut self.sheets {
            writes::set_global_transform(s, Transform3D::new(Basis::IDENTITY, centre));
        }
    }

    /// Pushes the properties to the quad and material.
    fn enact(&mut self) {
        if let Some(q) = &mut self.quad
            && q.get_size() != Vector2::splat(self.size)
        {
            q.set_size(Vector2::splat(self.size));
        }
        if let Some(m) = &mut self.mat {
            m.param("ray_color", &self.beam_tint.to_variant());
            m.param("alpha", &self.alpha.to_variant());
            m.param("start_depth", &self.start_depth.to_variant());
            m.param("shaft_steps", &self.shaft_steps.to_variant());
            m.param("dither_width", &self.dither_width.to_variant());
            m.param("edge_fade", &self.edge_fade.to_variant());
            m.param("fade_height", &self.fade_height.to_variant());
            m.param("surface_fade", &self.surface_fade.to_variant());
        }
        self.built = None;
        if self.sheets.is_some() {
            self.sync();
        }
    }
}

#[godot_api]
impl PixelSunShafts {
    /// Sun Shafts feature gate (`features::CALLBACK`).
    #[func]
    fn _on_pixel_features_changed(&mut self) {
        if super::gate(&self.to_gd(), Feature::SunShafts) {
            self.enact();
        }
    }

    #[func]
    fn get_sun(&self) -> Option<Gd<DirectionalLight3D>> {
        live(&self.sun)
    }
    #[func]
    fn set_sun(&mut self, v: Option<Gd<DirectionalLight3D>>) {
        self.sun = v;
        self.sun_ref.revert();
        self.enact();
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn assign_tally(&mut self, v: i32) {
        self.count = v.clamp(1, 64);
        self.enact();
    }
    #[func]
    fn set_depth(&mut self, v: f32) {
        self.depth = v.max(0.0);
        self.enact();
    }
    #[func]
    fn assign_measure(&mut self, v: f32) {
        self.size = v.max(1.0);
        self.enact();
    }
    #[func]
    fn set_follow_camera(&mut self, v: bool) {
        self.follow_camera = v;
        self.enact();
    }
    #[func]
    fn assign_beam_tint(&mut self, v: Color) {
        self.beam_tint = v;
        self.enact();
    }
    #[func]
    fn set_alpha(&mut self, v: f32) {
        self.alpha = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_start_depth(&mut self, v: f32) {
        self.start_depth = v.max(0.0);
        self.enact();
    }
    #[func]
    fn set_shaft_steps(&mut self, v: i32) {
        self.shaft_steps = v.clamp(0, 8);
        self.enact();
    }
    #[func]
    fn set_dither_width(&mut self, v: f32) {
        self.dither_width = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_edge_fade(&mut self, v: f32) {
        self.edge_fade = v.clamp(0.0, 1.0);
        self.enact();
    }
    #[func]
    fn set_surface_fade(&mut self, v: f32) {
        self.surface_fade = v.max(0.0);
        self.enact();
    }
    #[func]
    fn set_fade_height(&mut self, v: f32) {
        self.fade_height = v.max(0.0);
        self.enact();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_contain_the_sun_direction_and_span_the_depth() {
        let sun = Basis::from_euler(EulerOrder::YXZ, Vector3::new(-0.9, 0.5, 0.0));
        let xf = sheet_transforms(sun, 4, 40.0);
        assert_eq!(xf.len(), 4);
        let fwd = -sun.col_c();
        let up = sun.col_b();
        for t in &xf {
            // Quad normal (+Z) is the sun's up; the sun direction lies in the sheet.
            assert!((t.basis.col_c() - up).length() < 1e-4);
            assert!(t.basis.col_c().dot(fwd).abs() < 1e-4);
        }
        // Symmetric spread: -15, -5, 5, 15 along -up.
        let offs: Vec<f32> = xf.iter().map(|t| -t.origin.dot(up)).collect();
        for (o, e) in offs.iter().zip([-15.0, -5.0, 5.0, 15.0]) {
            assert!((o - e).abs() < 1e-3, "{offs:?}");
        }
    }
}
