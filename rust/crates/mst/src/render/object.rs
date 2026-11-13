//! `PixelObject` — pixelizes every mesh in its subtree.
//!
//! For each `GeometryInstance3D` below it (stopping at nested `PixelObject`s, which own their
//! own subtree, and nodes in the `pixel_ignore` group) it:
//! * adds the metadata render layer,
//! * sets the per-instance `pt_flags` (pixel size, outline, outline id, texel snap, palette /
//!   dither opt-outs; layout in `pt_core::meta::object_flags`),
//! * converts `StandardMaterial3D`s to the pixel object shader (cached per source material).
//!   `ShaderMaterial`s that include `pixel_common.gdshaderinc` are kept as authored; other
//!   shaders are left alone and the mesh stays off the metadata layer (no outline/blocks).
//!
//! **Inherit chain** (port of the original `PixelizerApplier3D`): `pixel_size`, `outline`,
//! `outline_id`, `palette` and `dither` left at -1 take the nearest ancestor `PixelObject`'s
//! value, then the renderer's "Object Defaults", then the built-in default. A
//! `PixelObjectProfile` in `profiles` overrides the values for one target's subtree.
//!
//! **Outline ids** (port of the original outline registry): `outline_id` -2 (AUTO) hands the
//! object its own id from a cycling allocator (`allocate_outline_id`, 1..=253; 254/255 are the
//! highlight rows). A top-level PixelObject left at inherit also gets an automatic id, which
//! its nested inherit objects share, so separate objects get id edges where they touch.
//!
//! **Editor**: while the `PixelRenderer` editor preview is on, objects apply in the editor too
//! (the preview then shows their real ids/sizes). Everything they change (layers, `pt_flags`,
//! converted materials) is recorded and restored before the scene is saved and when the
//! preview is turned off, so converted state is never written to disk.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use godot::classes::base_material_3d::{CullMode, Feature, Flags, TextureParam, Transparency};
use godot::classes::object::ConnectFlags;
use godot::classes::{
    BaseMaterial3D, GeometryInstance3D, INode3D, Material, MeshInstance3D, MultiMeshInstance3D, Node, Node3D, ShaderMaterial,
};
use godot::prelude::*;
use pt_core::meta::{HOVER_ID, SELECT_ID, object_flags, with_outline_id};

use super::outline_palette;
use super::profile::PixelObjectProfile;
use crate::globals;
use crate::util::*;

pub const IGNORE_GROUP: &str = "pixel_ignore";
pub const INHERIT: i32 = -1;
/// `outline_id` sentinel: allocate this object's own id.
pub const AUTO: i32 = -2;

const META_BASE_FLAGS: &str = "_pt_base_flags";
const META_HIGHLIGHT: &str = "_pt_highlight";

/// Renderer-level fallbacks of the inherit chain (`PixelRenderer` "Object Defaults").
static DEFAULTS: Mutex<(i32, bool)> = Mutex::new((1, true));

/// Updates the fallbacks; true when they changed (callers then refresh the PixelObjects).
pub fn set_defaults(pixel_size: i32, outline: bool) -> bool {
    let mut d = DEFAULTS.lock().unwrap();
    let new = (pixel_size.clamp(1, 4), outline);
    let changed = *d != new;
    *d = new;
    changed
}

fn defaults() -> (i32, bool) {
    *DEFAULTS.lock().unwrap()
}

/// Whether the renderer's editor preview is shown (PixelObjects apply in the editor then).
static EDITOR_PREVIEW: AtomicBool = AtomicBool::new(false);

/// Sets the editor-preview state; true when it changed.
pub fn set_editor_preview(on: bool) -> bool {
    EDITOR_PREVIEW.swap(on, Ordering::Relaxed) != on
}

pub fn editor_preview() -> bool {
    EDITOR_PREVIEW.load(Ordering::Relaxed)
}

/// First id of the automatic outline-id cycle (`PixelRenderer.auto_id_first`).
static AUTO_FIRST: AtomicU32 = AtomicU32::new(1);
static NEXT_AUTO: AtomicU32 = AtomicU32::new(0);

pub fn set_auto_first(first: u32) {
    AUTO_FIRST.store(first.clamp(1, pt_core::meta::MAX_USER_ID), Ordering::Relaxed);
}

/// Next automatic outline id: cycles `auto_id_first..=253` (port of the original
/// `PixelizerOutlineRegistry.allocate_id`; 254/255 stay reserved for highlights).
pub fn allocate_outline_id() -> u32 {
    let first = AUTO_FIRST.load(Ordering::Relaxed);
    let span = pt_core::meta::MAX_USER_ID + 1 - first;
    let n = NEXT_AUTO.fetch_add(1, Ordering::Relaxed);
    first + n % span
}

/// What a geometry looked like before an editor-time apply (restored by `unbind`).
struct Saved {
    node: Gd<GeometryInstance3D>,
    layers: u32,
    flags: Variant,
    material_override: Option<Gd<Material>>,
    surfaces: Vec<Option<Gd<Material>>>,
}

/// Effective (resolved) settings of a subtree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effective {
    pub pixel_size: i32,
    pub outline: bool,
    pub outline_id: i32,
    pub palette: bool,
    pub dither: bool,
    pub snap: bool,
    pub outline_color: Option<Color>,
}

impl Effective {
    pub fn flags(&self) -> i32 {
        object_flags(self.pixel_size, self.outline, self.outline_id, self.snap, self.palette, self.dither)
    }
}

/// Raw (possibly inherit) values of one link of the chain.
#[derive(Clone, Copy)]
struct Raw {
    pixel_size: i32,
    outline: i32,
    outline_id: i32,
    palette: i32,
    dither: i32,
    outline_color: Option<Color>,
}

impl Raw {
    /// Fills this link's inherit values from `parent`.
    fn over(self, parent: Raw) -> Raw {
        let pick = |v: i32, p: i32| if v < 0 { p } else { v };
        Raw {
            pixel_size: if self.pixel_size < 1 { parent.pixel_size } else { self.pixel_size },
            outline: pick(self.outline, parent.outline),
            outline_id: pick(self.outline_id, parent.outline_id),
            palette: pick(self.palette, parent.palette),
            dither: pick(self.dither, parent.dither),
            outline_color: self.outline_color.or(parent.outline_color),
        }
    }

    fn resolve(self, snap: bool) -> Effective {
        let (ps, outline) = defaults();
        Effective {
            pixel_size: if self.pixel_size < 1 { ps } else { self.pixel_size.min(4) },
            outline: if self.outline < 0 { outline } else { self.outline != 0 },
            outline_id: self.outline_id.clamp(0, pt_core::meta::MAX_USER_ID as i32),
            palette: self.palette != 0,
            dither: self.dither != 0,
            snap,
            outline_color: self.outline_color,
        }
    }

    fn of_profile(p: &PixelObjectProfile, auto_id: impl FnOnce() -> u32) -> Raw {
        Raw {
            pixel_size: p.pixel_size,
            outline: p.outline,
            outline_id: if p.outline_id == AUTO { auto_id() as i32 } else { p.outline_id },
            palette: p.palette,
            dither: p.dither,
            outline_color: p.override_outline_color.then_some(p.outline_color),
        }
    }
}

#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelObject {
    /// Block size multiplier in the pixel renderer (1-4); -1 inherits (parent PixelObject,
    /// then the renderer's `object_pixel_size`).
    #[export(range = (-1.0, 4.0, 1.0))]
    #[var(set = set_pixel_size)]
    pixel_size: i32,
    /// -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_outline)]
    outline: i32,
    /// Outline palette id 0-253 (PixelRenderer.outline_palette); -1 inherits (a top-level
    /// object gets an automatic id), -2 always allocates this object's own id.
    #[export(range = (-2.0, 253.0, 1.0))]
    #[var(set = set_outline_id)]
    outline_id: i32,
    /// Register `outline_color` as this subtree's outline color (exterior + interior rows).
    #[export]
    #[var(set = set_override_outline_color)]
    override_outline_color: bool,
    /// Outline color used with `override_outline_color` (alpha = opacity, 0 = darken).
    #[export]
    #[var(set = set_outline_color)]
    outline_color: Color,
    /// Palette LUT grading: -1 inherit, 0 skip this subtree, 1 grade.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_palette)]
    palette: i32,
    /// Ordered dither: -1 inherit, 0 off, 1 on.
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_dither)]
    dither: i32,
    /// Snap mesh origins to the texel lattice (removes sub-texel swimming on movers).
    #[export]
    #[var(set = set_snap_to_texel)]
    snap_to_texel: bool,
    /// Per-target overrides (`PixelObjectProfile.target` is relative to this node).
    #[export]
    #[var(set = set_profiles)]
    profiles: Array<Gd<PixelObjectProfile>>,
    /// 0 none, 1 hover (outline id 254), 2 selected (id 255). Runtime only.
    #[var(set = set_highlight)]
    highlight: i32,
    /// Convert StandardMaterial3D to the pixel shader.
    #[export]
    convert_materials: bool,
    /// Toon bands for converted materials.
    #[export(range = (1.0, 8.0, 1.0))]
    bands: i32,
    #[export(range = (0.0, 1.0, 0.01))]
    shade_floor: f32,
    #[export(range = (0.0, 1.0, 0.01))]
    rim_strength: f32,

    cache: HashMap<i64, Gd<ShaderMaterial>>,
    /// Meta bit applied last time (removed again if the renderer's meta layer changes).
    applied_meta: u32,
    /// Outline ids this object registered color overrides for.
    registered_ids: Vec<u32>,
    default_mat: Option<Gd<Material>>,
    refresh_queued: bool,
    warned: bool,
    /// Automatic outline id (0 = not allocated yet); allocated once per object.
    auto_id: Cell<u32>,
    /// Automatic ids of AUTO profiles, by target path.
    profile_ids: RefCell<HashMap<String, u32>>,
    /// Editor only: original state of every geometry changed by an editor-time apply.
    saved: HashMap<i64, Saved>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PixelObject {
    fn init(base: Base<Node3D>) -> Self {
        PixelObject {
            pixel_size: INHERIT,
            outline: INHERIT,
            outline_id: INHERIT,
            override_outline_color: false,
            outline_color: Color::from_rgba(0.04, 0.05, 0.09, 1.0),
            palette: INHERIT,
            dither: INHERIT,
            snap_to_texel: false,
            profiles: Array::new(),
            highlight: 0,
            convert_materials: true,
            bands: 3,
            shade_floor: 0.3,
            rim_strength: 0.0,
            cache: HashMap::new(),
            applied_meta: 0,
            registered_ids: Vec::new(),
            default_mat: None,
            refresh_queued: false,
            warned: false,
            auto_id: Cell::new(0),
            profile_ids: RefCell::new(HashMap::new()),
            saved: HashMap::new(),
            base,
        }
    }

    fn ready(&mut self) {
        let mut gd = self.to_gd();
        gd.add_to_group(globals::REFRESH_GROUP);
        // Any node added anywhere below us (not just direct children) gets pixelized.
        let cb = Callable::from_object_method(&gd, "on_node_added");
        let mut tree = gd.get_tree();
        if !tree.is_connected("node_added", &cb) {
            tree.connect("node_added", &cb);
        }
        self.apply();
    }

    fn exit_tree(&mut self) {
        for id in self.registered_ids.drain(..) {
            outline_palette::clear_override(id);
        }
        self.unbind();
    }

    fn on_notification(&mut self, what: godot::classes::notify::Node3DNotification) {
        use godot::classes::notify::Node3DNotification as N;
        if !is_editor() {
            return;
        }
        // Restore the authored state before serialization; re-apply after the save.
        if what == N::EDITOR_PRE_SAVE {
            self.unbind();
        } else if what == N::EDITOR_POST_SAVE && editor_preview() {
            self.queue_refresh();
        }
    }
}

/// True for the addon's own shaders and for custom shaders that include the pixel header.
fn is_pixel_shader(m: &Gd<ShaderMaterial>) -> bool {
    m.get_shader().map(|s| {
        let path = s.get_path().to_string();
        if path.starts_with(SHADER_DIR) {
            // Only the addon shaders that write metadata (not clouds/light volumes/fireflies).
            return [
                "object.gdshader",
                "object_double.gdshader",
                "mst_terrain.gdshader",
                "mst_grass.gdshader",
                "water.gdshader",
                "tree_leaves.gdshader",
                "grass_billboard.gdshader",
                "screen_projection.gdshader",
            ]
            .iter()
            .any(|n| path.ends_with(n));
        }
        let code = s.get_code().to_string();
        code.contains("pixel_common.gdshaderinc") || code.contains("object_body.gdshaderinc") || code.contains("pt_flags")
    }) == Some(true)
}

/// Writes `flags` to a geometry, keeping an active hover/selection highlight on top.
fn write_flags(g: &mut Gd<GeometryInstance3D>, flags: i32) {
    let kind = if g.has_meta(META_HIGHLIGHT) { g.get_meta(META_HIGHLIGHT).try_to::<i32>().unwrap_or(0) } else { 0 };
    if kind > 0 {
        g.set_meta(META_BASE_FLAGS, &flags.to_variant());
        let id = if kind == 1 { HOVER_ID } else { SELECT_ID };
        g.set_instance_shader_parameter("pt_flags", &with_outline_id(flags, id).to_variant());
    } else {
        g.set_instance_shader_parameter("pt_flags", &flags.to_variant());
    }
}

/// Highlights (1 hover, 2 selection) or restores (0) every pixelized mesh in `node`'s subtree,
/// including `node` itself. Runtime only; the original flags are restored exactly.
pub fn highlight(node: &Gd<Node>, kind: i32) {
    if is_editor() {
        return;
    }
    let meta = globals::meta_mask();
    let mut stack = vec![node.clone()];
    while let Some(n) = stack.pop() {
        stack.extend(n.get_children().iter_shared());
        let Ok(mut g) = n.try_cast::<GeometryInstance3D>() else { continue };
        if g.get_layer_mask() & meta == 0 {
            continue;
        }
        let had = g.has_meta(META_BASE_FLAGS);
        let base = if had {
            g.get_meta(META_BASE_FLAGS).try_to::<i32>().unwrap_or(32)
        } else {
            g.get_instance_shader_parameter("pt_flags").try_to::<i32>().unwrap_or(32)
        };
        if kind > 0 {
            g.set_meta(META_BASE_FLAGS, &base.to_variant());
            g.set_meta(META_HIGHLIGHT, &kind.to_variant());
            let id = if kind == 1 { HOVER_ID } else { SELECT_ID };
            g.set_instance_shader_parameter("pt_flags", &with_outline_id(base, id).to_variant());
        } else if had {
            g.remove_meta(META_BASE_FLAGS);
            g.remove_meta(META_HIGHLIGHT);
            g.set_instance_shader_parameter("pt_flags", &base.to_variant());
        }
    }
}

impl PixelObject {
    /// This object's automatic outline id (allocated on first use).
    pub fn auto_id(&self) -> u32 {
        if self.auto_id.get() == 0 {
            self.auto_id.set(allocate_outline_id());
        }
        self.auto_id.get()
    }

    fn raw(&self) -> Raw {
        Raw {
            pixel_size: self.pixel_size,
            outline: self.outline,
            outline_id: if self.outline_id == AUTO { self.auto_id() as i32 } else { self.outline_id },
            palette: self.palette,
            dither: self.dither,
            outline_color: self.override_outline_color.then_some(self.outline_color),
        }
    }

    /// This object's raw values with every inherit filled from the ancestor PixelObjects and
    /// the built-in root (`outline_id` 0, palette and dither on).
    fn chain(&self) -> Raw {
        let mut raw = self.raw();
        let mut top: Option<Gd<PixelObject>> = None;
        let mut p = self.base().get_parent();
        while let Some(n) = p {
            if let Ok(po) = n.clone().try_cast::<PixelObject>() {
                raw = raw.over(po.bind().raw());
                top = Some(po);
            }
            p = n.get_parent();
        }
        // Inherit all the way up: the top-level object's automatic id.
        let root_id = if raw.outline_id < 0 {
            match &top {
                Some(t) => t.bind().auto_id() as i32,
                None => self.auto_id() as i32,
            }
        } else {
            0
        };
        raw.over(Raw { pixel_size: -1, outline: -1, outline_id: root_id, palette: 1, dither: 1, outline_color: None })
    }

    /// Editor only: restores every geometry an editor-time apply changed.
    fn unbind(&mut self) {
        for (_, sv) in self.saved.drain() {
            let mut g = sv.node;
            if !g.is_instance_valid() {
                continue;
            }
            g.set_layer_mask(sv.layers);
            g.set_instance_shader_parameter("pt_flags", &sv.flags);
            match &sv.material_override {
                Some(m) => g.set_material_override(m),
                None => g.set_material_override(Gd::<Material>::null_arg()),
            }
            if let Ok(mut mi) = g.try_cast::<MeshInstance3D>() {
                for (i, m) in sv.surfaces.iter().enumerate() {
                    match m {
                        Some(m) => mi.set_surface_override_material(i as i32, m),
                        None => mi.set_surface_override_material(i as i32, Gd::<Material>::null_arg()),
                    }
                }
            }
        }
        self.applied_meta = 0;
    }

    /// Editor only: remembers a geometry's authored state before it is first changed.
    fn remember(&mut self, g: &Gd<GeometryInstance3D>) {
        let key = g.instance_id().to_i64();
        if self.saved.contains_key(&key) {
            return;
        }
        let surfaces = match g.clone().try_cast::<MeshInstance3D>() {
            Ok(mi) => (0..mi.get_surface_override_material_count()).map(|i| mi.get_surface_override_material(i)).collect(),
            Err(_) => Vec::new(),
        };
        self.saved.insert(
            key,
            Saved {
                node: g.clone(),
                layers: g.get_layer_mask(),
                flags: g.get_instance_shader_parameter("pt_flags"),
                material_override: g.get_material_override(),
                surfaces,
            },
        );
    }

    /// Resolved settings of this object's subtree (no profile).
    pub fn effective(&self) -> Effective {
        self.chain().resolve(self.snap_to_texel)
    }

    /// Re-applies layers/flags/materials to the whole subtree.
    pub fn apply(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        if is_editor() && !editor_preview() {
            self.unbind();
            return;
        }
        let chain = self.chain();
        let own = chain.resolve(self.snap_to_texel);

        // Profiles: target instance id -> resolved settings for that subtree.
        let me = self.to_gd();
        let mut targets: HashMap<i64, Effective> = HashMap::new();
        let refresh = Callable::from_object_method(&me, "queue_refresh");
        for p in self.profiles.iter_shared() {
            let mut pr = p.clone();
            if !pr.is_connected("changed", &refresh) {
                pr.connect_flags("changed", &refresh, ConnectFlags::DEFERRED);
            }
            let b = p.bind();
            if !b.enabled || b.target.is_empty() {
                continue;
            }
            if let Some(t) = self.base().get_node_or_null(&b.target) {
                let key = b.target.to_string();
                let auto = || *self.profile_ids.borrow_mut().entry(key).or_insert_with(allocate_outline_id);
                targets.insert(t.instance_id().to_i64(), Raw::of_profile(&b, auto).over(chain).resolve(self.snap_to_texel));
            }
        }

        // Outline color overrides (object + profiles).
        let mut ids: Vec<(u32, Color)> = Vec::new();
        if let Some(c) = own.outline_color.filter(|_| self.override_outline_color) {
            ids.push((own.outline_id as u32, c));
        }
        for e in targets.values() {
            if let Some(c) = e.outline_color {
                ids.push((e.outline_id as u32, c));
            }
        }
        for id in self.registered_ids.drain(..).collect::<Vec<_>>() {
            if !ids.iter().any(|(i, _)| *i == id) {
                outline_palette::clear_override(id);
            }
        }
        for (id, c) in &ids {
            outline_palette::set_override(*id, *c, *c);
            self.registered_ids.push(*id);
        }

        let root = me.clone().upcast::<Node>();
        let mut stack: Vec<(Gd<Node>, Effective)> = root.get_children().iter_shared().map(|c| (c, own)).collect();
        let mut nested: Vec<Gd<PixelObject>> = Vec::new();
        while let Some((n, inherited)) = stack.pop() {
            if n.is_in_group(IGNORE_GROUP) {
                continue;
            }
            if let Ok(po) = n.clone().try_cast::<PixelObject>() {
                nested.push(po);
                continue;
            }
            let eff = targets.get(&n.instance_id().to_i64()).copied().unwrap_or(inherited);
            if let Ok(g) = n.clone().try_cast::<GeometryInstance3D>() {
                self.apply_geometry(g, eff.flags());
            }
            stack.extend(n.get_children().iter_shared().map(|c| (c, eff)));
        }
        self.applied_meta = globals::meta_mask();
        // Nested objects inherit from us: refresh them once we are no longer bound.
        for mut po in nested {
            po.call_deferred("queue_refresh", &[]);
        }
    }

    /// Stand-in for surfaces without a material (Godot draws those plain white).
    fn default_material(&mut self) -> Gd<Material> {
        self.default_mat.get_or_insert_with(|| godot::classes::StandardMaterial3D::new_gd().upcast()).clone()
    }

    fn convert(&mut self, mat: &Gd<Material>) -> Option<Gd<ShaderMaterial>> {
        if let Ok(sm) = mat.clone().try_cast::<ShaderMaterial>() {
            return if is_pixel_shader(&sm) { Some(sm) } else { None };
        }
        let base = mat.clone().try_cast::<BaseMaterial3D>().ok()?;
        let key = base.instance_id().to_i64();
        if let Some(c) = self.cache.get(&key) {
            return Some(c.clone());
        }
        let double = base.get_cull_mode() == CullMode::DISABLED;
        let mut sm = shader_material(if double { "object_double.gdshader" } else { "object.gdshader" });
        sm.set_shader_parameter("albedo_color", &base.get_albedo().to_variant());
        if let Some(t) = base.get_texture(TextureParam::ALBEDO) {
            sm.set_shader_parameter("albedo_texture", &t.to_variant());
            sm.set_shader_parameter("use_albedo_texture", &true.to_variant());
        }
        sm.set_shader_parameter("use_vertex_color", &base.get_flag(Flags::ALBEDO_FROM_VERTEX_COLOR).to_variant());
        sm.set_shader_parameter("uv1_scale", &base.get_uv1_scale().to_variant());
        sm.set_shader_parameter("uv1_offset", &base.get_uv1_offset().to_variant());
        let tr = base.get_transparency();
        if tr != Transparency::DISABLED {
            sm.set_shader_parameter("use_alpha_scissor", &true.to_variant());
            let th = if tr == Transparency::ALPHA_SCISSOR { base.get_alpha_scissor_threshold() } else { 0.5 };
            sm.set_shader_parameter("alpha_scissor_threshold", &th.to_variant());
        }
        if base.get_feature(Feature::EMISSION) {
            sm.set_shader_parameter("emission_color", &base.get_emission().to_variant());
            sm.set_shader_parameter("emission_energy", &base.get_emission_energy_multiplier().to_variant());
            if let Some(t) = base.get_texture(TextureParam::EMISSION) {
                sm.set_shader_parameter("emission_texture", &t.to_variant());
                sm.set_shader_parameter("use_emission_texture", &true.to_variant());
            }
        }
        sm.set_shader_parameter("bands", &self.bands.to_variant());
        sm.set_shader_parameter("shade_floor", &self.shade_floor.to_variant());
        sm.set_shader_parameter("rim_strength", &self.rim_strength.to_variant());
        self.cache.insert(key, sm.clone());
        Some(sm)
    }

    fn apply_geometry(&mut self, mut g: Gd<GeometryInstance3D>, flags: i32) {
        if is_editor() {
            self.remember(&g);
        }
        // Only geometry whose every material is (or becomes) a pixel shader may join the
        // metadata layer; anything else (sprites, labels, particles with stock materials)
        // would be decoded as garbage metadata.
        let is_mesh = g.clone().try_cast::<MeshInstance3D>().is_ok() || g.clone().try_cast::<MultiMeshInstance3D>().is_ok();
        let mut ok = is_mesh || g.get_material_override().is_some();
        if !self.convert_materials {
            ok = false;
        } else {
            if let Some(m) = g.get_material_override() {
                match self.convert(&m) {
                    Some(sm) => g.set_material_override(&sm),
                    None => ok = false,
                }
            } else if let Ok(mut mi) = g.clone().try_cast::<MeshInstance3D>() {
                if let Some(mesh) = mi.get_mesh() {
                    for i in 0..mesh.get_surface_count() {
                        let mat = mi.get_surface_override_material(i).or_else(|| mesh.surface_get_material(i)).unwrap_or_else(|| self.default_material());
                        match self.convert(&mat) {
                            Some(sm) => mi.set_surface_override_material(i, &sm),
                            None => ok = false,
                        }
                    }
                }
            } else if let Ok(mmi) = g.clone().try_cast::<MultiMeshInstance3D>() {
                let mat = mmi.get_multimesh().and_then(|mm| mm.get_mesh()).and_then(|m| m.surface_get_material(0)).unwrap_or_else(|| self.default_material());
                match self.convert(&mat) {
                    Some(sm) => g.set_material_override(&sm),
                    None => ok = false,
                }
            }
        }
        let meta = globals::meta_mask();
        let layers = g.get_layer_mask() & !self.applied_meta;
        if ok {
            g.set_layer_mask(layers | meta);
            write_flags(&mut g, flags);
        } else {
            g.set_layer_mask(layers & !meta);
            if !self.warned {
                self.warned = true;
                godot_warn!(
                    "PixelObject '{}': '{}' uses a material that is not a pixel shader; it renders without outlines/pixel sizes. Include pixel_common.gdshaderinc in custom shaders.",
                    self.base().get_name(),
                    g.get_name()
                );
            }
        }
    }
}

#[godot_api]
impl PixelObject {
    #[func]
    fn on_node_added(&mut self, node: Gd<Node>) {
        if self.refresh_queued || node.clone().try_cast::<GeometryInstance3D>().is_err() {
            return;
        }
        if self.base().is_ancestor_of(&node) {
            self.queue_refresh();
        }
    }

    /// Deferred, coalesced refresh (a spawned scene adds many nodes in one frame).
    #[func]
    fn queue_refresh(&mut self) {
        if self.refresh_queued {
            return;
        }
        self.refresh_queued = true;
        self.to_gd().call_deferred("refresh_pixel_settings", &[]);
    }

    /// Re-applies settings to the whole subtree.
    #[func]
    fn refresh_pixel_settings(&mut self) {
        self.refresh_queued = false;
        self.apply();
        let h = self.highlight;
        if h > 0 {
            highlight(&self.to_gd().upcast(), h);
        }
    }

    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = if v < 1 { INHERIT } else { v.min(4) };
        self.apply();
    }
    #[func]
    fn set_outline(&mut self, v: i32) {
        self.outline = v.clamp(-1, 1);
        self.apply();
    }
    #[func]
    fn set_outline_id(&mut self, v: i32) {
        self.outline_id = v.clamp(AUTO, pt_core::meta::MAX_USER_ID as i32);
        self.apply();
    }
    #[func]
    fn set_override_outline_color(&mut self, v: bool) {
        self.override_outline_color = v;
        self.apply();
    }
    #[func]
    fn set_outline_color(&mut self, v: Color) {
        self.outline_color = v;
        self.apply();
    }
    #[func]
    fn set_palette(&mut self, v: i32) {
        self.palette = v.clamp(-1, 1);
        self.apply();
    }
    #[func]
    fn set_dither(&mut self, v: i32) {
        self.dither = v.clamp(-1, 1);
        self.apply();
    }
    #[func]
    fn set_snap_to_texel(&mut self, v: bool) {
        self.snap_to_texel = v;
        self.apply();
    }
    #[func]
    fn set_profiles(&mut self, v: Array<Gd<PixelObjectProfile>>) {
        self.profiles = v;
        self.apply();
    }
    #[func]
    fn set_highlight(&mut self, v: i32) {
        self.highlight = v.clamp(0, 2);
        highlight(&self.to_gd().upcast(), self.highlight);
    }

    /// Resolved values after the inherit chain: {pixel_size, outline, outline_id, palette, dither}.
    #[func]
    fn get_effective(&self) -> VarDictionary {
        let e = self.effective();
        let mut d = VarDictionary::new();
        d.set("pixel_size", e.pixel_size);
        d.set("outline", e.outline);
        d.set("outline_id", e.outline_id);
        d.set("palette", e.palette);
        d.set("dither", e.dither);
        d
    }

    /// The enabled profile targeting `node` (path relative to this object), if any.
    #[func]
    pub fn get_profile_for(&self, node: Gd<Node>) -> Option<Gd<PixelObjectProfile>> {
        let path = self.base().get_path_to(&node);
        self.profiles.iter_shared().find(|p| p.bind().target == path)
    }

    /// Returns the profile for `node`, creating an all-inherit one when missing.
    #[func]
    pub fn ensure_profile_for(&mut self, node: Gd<Node>) -> Gd<PixelObjectProfile> {
        if let Some(p) = self.get_profile_for(node.clone()) {
            return p;
        }
        let mut p = PixelObjectProfile::new_gd();
        let path = self.base().get_path_to(&node);
        p.bind_mut().target = path;
        self.profiles.push(&p);
        self.apply();
        p
    }

    /// Removes the profile targeting `node`.
    #[func]
    pub fn remove_profile_for(&mut self, node: Gd<Node>) -> bool {
        let path = self.base().get_path_to(&node);
        let before = self.profiles.len();
        let kept: Array<Gd<PixelObjectProfile>> = self.profiles.iter_shared().filter(|p| p.bind().target != path).collect();
        self.profiles = kept;
        let removed = self.profiles.len() != before;
        if removed {
            self.apply();
        }
        removed
    }
}
