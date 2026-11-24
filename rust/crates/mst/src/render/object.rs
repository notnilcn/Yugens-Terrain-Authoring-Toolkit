//! Marks a branch of a scene as pixel art and says how that branch should be pixelized.
//!
//! Intent: an author wraps a model, a prop or a character in one of these and chooses, in
//! the Inspector, how large its pixels are, whether it is outlined and in which outline
//! group, and whether the palette and dithering apply. Everything below it follows those
//! choices, nested ones refine them, and single meshes inside it can be given their own
//! settings without restructuring the model.
//!
//! Responsibility: it resolves the effective settings for every mesh beneath it, from its
//! own choices, its pixel-art ancestors and the renderer's defaults, and puts those meshes
//! into the form the pixel renderer understands, converting ordinary materials when asked.
//! It also brings them back to their original state when it is removed or while the editor
//! saves, so the author's materials are never overwritten. It does not draw anything and
//! does not decide the overall look; that belongs to the renderer.
//!
//! Why a node: what it governs is defined by where it sits in the tree, so it has to be in
//! the tree, above the meshes it affects, and it has to hear when meshes are added beneath
//! it or when the editor saves.
//!
//! Why a new type: no built-in node scopes rendering settings over a subtree, and putting
//! the rule on every mesh would mean editing each imported model by hand and losing the
//! changes whenever the model is reimported.
//!
//! Not exported: the hover or selection highlight that game code sets while playing; the
//! outline groups it was assigned automatically and registered; the converted materials
//! it caches; and the original state of each mesh it has taken over, kept so it can give
//! it back. All of that is either run-time state or derived from the exported choices.
use super::outline_colors;
use super::profile::PixelObjectProfile;
use crate::editor_detection::is_editor;
use crate::globals;
use crate::shader_materials::{SHADER_DIR, shader_material};
use godot::classes::base_material_3d::{CullMode, Feature, Flags, TextureParam, Transparency};
use godot::classes::object::ConnectFlags;
use godot::classes::{
    BaseMaterial3D, GeometryInstance3D, INode3D, Material, MeshInstance3D, MultiMeshInstance3D,
    Node, Node3D, ShaderMaterial,
};
use godot::prelude::*;
use pt_core::meta::{HOVER_ID, SELECT_ID, object_flags, with_outline_id};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub const IGNORE_GROUP: &str = "pixel_ignore";
pub const INHERIT: i32 = -1;
pub const AUTO: i32 = -2;
const META_BASE_FLAGS: &str = "_pt_base_flags";
const META_HIGHLIGHT: &str = "_pt_highlight";
static DEFAULTS: Mutex<(i32, bool)> = Mutex::new((1, true));
pub fn set_defaults(pixel_size: i32, outline: bool) -> bool {
    let mut d = DEFAULTS.lock().unwrap_or_else(|e| e.into_inner());
    let new = (
        pixel_size.clamp(1, pt_core::meta::MAX_PIXEL_SIZE as i32),
        outline,
    );
    let changed = *d != new;
    *d = new;
    changed
}
fn defaults() -> (i32, bool) {
    *DEFAULTS.lock().unwrap_or_else(|e| e.into_inner())
}
static LARGE_SIZES: Mutex<BTreeMap<(i64, u8), (i32, u64)>> = Mutex::new(BTreeMap::new());
static SIZE_SEQ: AtomicU64 = AtomicU64::new(0);
const META_SIZES: i32 = pt_core::meta::META_PIXEL_SIZES as i32;
const MAX_LARGE_SIZES: usize = MAX_BLOCK_SIZES - (META_SIZES as usize - 1);
pub const MAX_BLOCK_SIZES: usize = 16;
pub fn note_pixel_size(owner: InstanceId, slot: u8, pixel_size: i32) {
    let key = (owner.to_i64(), slot);
    let mut m = LARGE_SIZES.lock().unwrap_or_else(|e| e.into_inner());
    if pixel_size <= META_SIZES {
        m.remove(&key);
        return;
    }
    if m.get(&key).is_some_and(|(k, _)| *k == pixel_size) {
        return;
    }
    m.insert(key, (pixel_size, SIZE_SEQ.fetch_add(1, Ordering::Relaxed)));
}
pub fn block_sizes() -> Vec<i32> {
    let mut m = LARGE_SIZES.lock().unwrap_or_else(|e| e.into_inner());
    m.retain(|(id, _), _| {
        InstanceId::try_from_i64(*id).is_some_and(|i| Gd::<Object>::try_from_instance_id(i).is_ok())
    });
    pt_core::meta::block_sizes(m.values().copied(), MAX_LARGE_SIZES)
}
static EDITOR_PREVIEW: AtomicBool = AtomicBool::new(false);
pub fn assign_studio_prelook(on: bool) -> bool {
    EDITOR_PREVIEW.swap(on, Ordering::Relaxed) != on
}
pub fn studio_prelook() -> bool {
    EDITOR_PREVIEW.load(Ordering::Relaxed)
}
static AUTO_FIRST: AtomicU32 = AtomicU32::new(1);
static NEXT_AUTO: AtomicU32 = AtomicU32::new(0);
pub fn set_auto_first(first: u32) {
    AUTO_FIRST.store(
        first.clamp(1, pt_core::meta::MAX_USER_ID),
        Ordering::Relaxed,
    );
}
pub fn allocate_outline_id() -> u32 {
    let first = AUTO_FIRST.load(Ordering::Relaxed);
    let span = pt_core::meta::MAX_USER_ID + 1 - first;
    let n = NEXT_AUTO.fetch_add(1, Ordering::Relaxed);
    first + n % span
}
struct Saved {
    node: Gd<GeometryInstance3D>,
    layers: u32,
    bits: Variant,
    material_override: Option<Gd<Material>>,
    surfaces: Vec<Option<Gd<Material>>>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effective {
    pub pixel_size: i32,
    pub outline: bool,
    pub outline_id: i32,
    pub palette: bool,
    pub dither: bool,
    pub snap: bool,
    pub contour_tint: Option<Color>,
}
impl Effective {
    pub fn bits(&self) -> i32 {
        object_flags(
            self.pixel_size,
            self.outline,
            self.outline_id,
            self.snap,
            self.palette,
            self.dither,
        )
    }
}
#[derive(Clone, Copy)]
struct Raw {
    pixel_size: i32,
    outline: i32,
    outline_id: i32,
    palette: i32,
    dither: i32,
    contour_tint: Option<Color>,
}
impl Raw {
    fn over(self, parent: Raw) -> Raw {
        let pick = |v: i32, p: i32| if v < 0 { p } else { v };
        Raw {
            pixel_size: if self.pixel_size < 1 {
                parent.pixel_size
            } else {
                self.pixel_size
            },
            outline: pick(self.outline, parent.outline),
            outline_id: pick(self.outline_id, parent.outline_id),
            palette: pick(self.palette, parent.palette),
            dither: pick(self.dither, parent.dither),
            contour_tint: self.contour_tint.or(parent.contour_tint),
        }
    }
    fn settle(self, snap: bool) -> Effective {
        let (ps, outline) = defaults();
        Effective {
            pixel_size: if self.pixel_size < 1 {
                ps
            } else {
                self.pixel_size.min(pt_core::meta::MAX_PIXEL_SIZE as i32)
            },
            outline: if self.outline < 0 {
                outline
            } else {
                self.outline != 0
            },
            outline_id: self.outline_id.clamp(0, pt_core::meta::MAX_USER_ID as i32),
            palette: self.palette != 0,
            dither: self.dither != 0,
            snap,
            contour_tint: self.contour_tint,
        }
    }
    fn of_profile(p: &PixelObjectProfile, auto_id: impl FnOnce() -> u32) -> Raw {
        Raw {
            pixel_size: p.pixel_size,
            outline: p.outline,
            outline_id: if p.outline_id == AUTO {
                auto_id() as i32
            } else {
                p.outline_id
            },
            palette: p.palette,
            dither: p.dither,
            contour_tint: p.supersede_contour_tint.then_some(p.contour_tint),
        }
    }
}
#[derive(GodotClass)]
#[class(tool, base = Node3D)]
pub struct PixelObject {
    #[export(range = (-1.0, 16.0, 1.0, or_greater))]
    #[var(set = set_pixel_size)]
    pixel_size: i32,
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_outline)]
    outline: i32,
    #[export(range = (-2.0, 253.0, 1.0))]
    #[var(set = set_outline_id)]
    outline_id: i32,
    #[export]
    #[var(set = assign_supersede_contour_tint)]
    supersede_contour_tint: bool,
    #[export]
    #[var(set = assign_contour_tint)]
    contour_tint: Color,
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = assign_swatchset)]
    palette: i32,
    #[export(range = (-1.0, 1.0, 1.0))]
    #[var(set = set_dither)]
    dither: i32,
    #[export]
    #[var(set = set_snap_to_texel)]
    snap_to_texel: bool,
    #[export]
    #[var(set = set_profiles)]
    profiles: Array<Gd<PixelObjectProfile>>,
    #[export]
    #[var(set = set_profile_targets)]
    profile_targets: Array<Option<Gd<Node>>>,
    #[var(set = set_highlight)]
    highlight: i32,
    #[export]
    convert_materials: bool,
    #[export(range = (1.0, 8.0, 1.0))]
    bands: i32,
    #[export(range = (0.0, 1.0, 0.0105))]
    shade_floor: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    rim_strength: f32,
    #[export(range = (0.0, 1.0, 0.0105))]
    alpha_scissor_threshold: f32,
    cache: HashMap<i64, Gd<ShaderMaterial>>,
    applied_meta: u32,
    registered_ids: Vec<u32>,
    default_mat: Option<Gd<Material>>,
    refresh_queued: bool,
    warned: bool,
    auto_id: Cell<u32>,
    profile_ids: RefCell<HashMap<i64, u32>>,
    lineage: Option<(Raw, Gd<PixelObject>)>,
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
            supersede_contour_tint: false,
            contour_tint: Color::from_rgba(0.0405, 0.0505, 0.0905, 1.0),
            palette: INHERIT,
            dither: INHERIT,
            snap_to_texel: false,
            profiles: Array::new(),
            profile_targets: Array::new(),
            highlight: 0,
            convert_materials: true,
            bands: 3,
            shade_floor: 0.3005,
            rim_strength: 0.0,
            alpha_scissor_threshold: 0.5,
            cache: HashMap::new(),
            applied_meta: 0,
            registered_ids: Vec::new(),
            default_mat: None,
            refresh_queued: false,
            warned: false,
            auto_id: Cell::new(0),
            profile_ids: RefCell::new(HashMap::new()),
            lineage: None,
            saved: HashMap::new(),
            base,
        }
    }
    fn ready(&mut self) {
        let mut gd = self.to_gd();
        gd.add_to_group(globals::REFRESH_GROUP);
        let cb = Callable::from_object_method(&gd, "on_node_added");
        let mut tree = gd.get_tree();
        if !tree.is_connected("node_added", &cb) {
            tree.connect("node_added", &cb);
        }
        self.enact();
    }
    fn exit_tree(&mut self) {
        for id in self.registered_ids.drain(..) {
            outline_colors::clear_override(id);
        }
        self.lineage = None;
        self.unbind();
    }
    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        let targets = self.profile_targets.len();
        for (i, _) in self.profiles.iter_shared().enumerate() {
            let assigned = i < targets
                && self
                    .profile_targets
                    .at(i)
                    .is_some_and(|t| t.is_instance_valid());
            if !assigned {
                w.push(&format!(
                    "Assign entry {i} of Profile Targets: the profile at the same index has no node to apply to."
                ));
            }
        }
        w
    }
    fn on_notification(&mut self, what: godot::classes::notify::Node3DNotification) {
        use godot::classes::notify::Node3DNotification as N;
        if !is_editor() {
            return;
        }
        if what == N::EDITOR_PRE_SAVE {
            self.unbind();
        } else if what == N::EDITOR_POST_SAVE && studio_prelook() {
            self.queue_refresh();
        }
    }
}
fn is_pixel_shader(m: &Gd<ShaderMaterial>) -> bool {
    m.get_shader().map(|s| {
        let path = s.get_path().to_string();
        if path.starts_with(SHADER_DIR) {
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
        code.contains("pixel_common.gdshaderinc")
            || code.contains("object_body.gdshaderinc")
            || code.contains("pt_flags")
    }) == Some(true)
}
fn write_flags(g: &mut Gd<GeometryInstance3D>, flags: i32) {
    let kind = if g.has_meta(META_HIGHLIGHT) {
        g.get_meta(META_HIGHLIGHT).try_to::<i32>().unwrap_or(0)
    } else {
        0
    };
    if kind > 0 {
        g.set_meta(META_BASE_FLAGS, &flags.to_variant());
        let id = if kind == 1 { HOVER_ID } else { SELECT_ID };
        g.set_instance_shader_parameter("pt_flags", &with_outline_id(flags, id).to_variant());
    } else {
        g.set_instance_shader_parameter("pt_flags", &flags.to_variant());
    }
}
pub fn highlight(node: &Gd<Node>, kind: i32) {
    if is_editor() {
        return;
    }
    let meta = globals::meta_mask();
    let mut stack = vec![node.clone()];
    while let Some(n) = stack.pop() {
        stack.extend(n.get_children().iter_shared());
        let Ok(mut g) = n.try_cast::<GeometryInstance3D>() else {
            continue;
        };
        if g.get_layer_mask() & meta == 0 {
            continue;
        }
        let had = g.has_meta(META_BASE_FLAGS);
        let base = if had {
            g.get_meta(META_BASE_FLAGS).try_to::<i32>().unwrap_or(32)
        } else {
            g.get_instance_shader_parameter("pt_flags")
                .try_to::<i32>()
                .unwrap_or(32)
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
    pub(crate) fn refresh_settings_deferred(this: &Gd<Self>) {
        let mut this = this.clone();
        this.run_deferred_gd(|mut po| {
            if po.is_instance_valid() {
                po.bind_mut().refresh_pixel_settings();
            }
        });
    }
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
            outline_id: if self.outline_id == AUTO {
                self.auto_id() as i32
            } else {
                self.outline_id
            },
            palette: self.palette,
            dither: self.dither,
            contour_tint: self.supersede_contour_tint.then_some(self.contour_tint),
        }
    }
    fn inherited(&self) -> (Raw, Option<Gd<PixelObject>>) {
        match &self.lineage {
            Some((parent, top)) if top.is_instance_valid() => {
                (self.raw().over(*parent), Some(top.clone()))
            }
            _ => (self.raw(), None),
        }
    }
    fn chain(&self) -> Raw {
        let (raw, top) = self.inherited();
        let root_id = if raw.outline_id < 0 {
            match &top {
                Some(t) => t.bind().auto_id() as i32,
                None => self.auto_id() as i32,
            }
        } else {
            0
        };
        raw.over(Raw {
            pixel_size: -1,
            outline: -1,
            outline_id: root_id,
            palette: 1,
            dither: 1,
            contour_tint: None,
        })
    }
    fn unbind(&mut self) {
        for (_, sv) in self.saved.drain() {
            let mut g = sv.node;
            if !g.is_instance_valid() {
                continue;
            }
            g.set_layer_mask(sv.layers);
            g.set_instance_shader_parameter("pt_flags", &sv.bits);
            match &sv.material_override {
                Some(m) => g.set_material_override(m),
                None => g.set_material_override(Gd::<Material>::null_arg()),
            }
            if let Ok(mut mi) = g.try_cast::<MeshInstance3D>() {
                for (i, m) in sv.surfaces.iter().enumerate() {
                    match m {
                        Some(m) => mi.set_surface_override_material(i as i32, m),
                        None => {
                            mi.set_surface_override_material(i as i32, Gd::<Material>::null_arg())
                        }
                    }
                }
            }
        }
        self.applied_meta = 0;
    }
    fn recall(&mut self, g: &Gd<GeometryInstance3D>) {
        let key = g.instance_id().to_i64();
        if self.saved.contains_key(&key) {
            return;
        }
        let surfaces = match g.clone().try_cast::<MeshInstance3D>() {
            Ok(mi) => (0..mi.get_surface_override_material_count())
                .map(|i| mi.get_surface_override_material(i))
                .collect(),
            Err(_) => Vec::new(),
        };
        self.saved.insert(
            key,
            Saved {
                node: g.clone(),
                layers: g.get_layer_mask(),
                bits: g.get_instance_shader_parameter("pt_flags"),
                material_override: g.get_material_override(),
                surfaces,
            },
        );
    }
    pub fn actual(&self) -> Effective {
        self.chain().settle(self.snap_to_texel)
    }
    pub fn enact(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        if is_editor() && !studio_prelook() {
            self.unbind();
            return;
        }
        let chain = self.chain();
        let own = chain.settle(self.snap_to_texel);
        let me = self.to_gd();
        let mut targets: HashMap<i64, Effective> = HashMap::new();
        let refresh = Callable::from_object_method(&me, "queue_refresh");
        let profile_targets = self.profile_targets.len();
        for (i, p) in self.profiles.iter_shared().enumerate() {
            let mut pr = p.clone();
            if !pr.is_connected("changed", &refresh) {
                pr.connect_flags("changed", &refresh, ConnectFlags::DEFERRED);
            }
            let b = p.bind();
            if !b.active || i >= profile_targets {
                continue;
            }
            let Some(t) = self.profile_targets.at(i).filter(|t| t.is_instance_valid()) else {
                continue;
            };
            let key = t.instance_id().to_i64();
            let auto = || {
                *self
                    .profile_ids
                    .borrow_mut()
                    .entry(key)
                    .or_insert_with(allocate_outline_id)
            };
            targets.insert(
                key,
                Raw::of_profile(&b, auto)
                    .over(chain)
                    .settle(self.snap_to_texel),
            );
        }
        let mut ids: Vec<(u32, Color)> = Vec::new();
        if let Some(c) = own.contour_tint.filter(|_| self.supersede_contour_tint) {
            ids.push((own.outline_id as u32, c));
        }
        for e in targets.values() {
            if let Some(c) = e.contour_tint {
                ids.push((e.outline_id as u32, c));
            }
        }
        for id in self.registered_ids.drain(..).collect::<Vec<_>>() {
            if !ids.iter().any(|(i, _)| *i == id) {
                outline_colors::clear_override(id);
            }
        }
        for (id, c) in &ids {
            outline_colors::set_override(*id, *c, *c);
            self.registered_ids.push(*id);
        }
        let root = me.clone().upcast::<Node>();
        let mut stack: Vec<(Gd<Node>, Effective)> = root
            .get_children()
            .iter_shared()
            .map(|c| (c, own))
            .collect();
        let mut nested: Vec<Gd<PixelObject>> = Vec::new();
        while let Some((n, inherited)) = stack.pop() {
            if n.is_in_group(IGNORE_GROUP) {
                continue;
            }
            if let Ok(po) = n.clone().try_cast::<PixelObject>() {
                nested.push(po);
                continue;
            }
            let eff = targets
                .get(&n.instance_id().to_i64())
                .copied()
                .unwrap_or(inherited);
            if let Ok(g) = n.clone().try_cast::<GeometryInstance3D>() {
                note_pixel_size(g.instance_id(), 0, eff.pixel_size);
                self.enact_geom(g, eff.bits());
            }
            stack.extend(n.get_children().iter_shared().map(|c| (c, eff)));
        }
        self.applied_meta = globals::meta_mask();
        let (handed_down, top) = self.inherited();
        let top = top.unwrap_or_else(|| me.clone());
        for mut po in nested {
            po.bind_mut().lineage = Some((handed_down, top.clone()));
            po.call_deferred("queue_refresh", &[]);
        }
    }
    fn fallback_stuff(&mut self) -> Gd<Material> {
        self.default_mat
            .get_or_insert_with(|| godot::classes::StandardMaterial3D::new_gd().upcast())
            .clone()
    }
    fn transmute(&mut self, mat: &Gd<Material>) -> Option<Gd<ShaderMaterial>> {
        if let Ok(sm) = mat.clone().try_cast::<ShaderMaterial>() {
            return if is_pixel_shader(&sm) { Some(sm) } else { None };
        }
        let base = mat.clone().try_cast::<BaseMaterial3D>().ok()?;
        let key = base.instance_id().to_i64();
        if let Some(c) = self.cache.get(&key) {
            return Some(c.clone());
        }
        let double = base.get_cull_mode() == CullMode::DISABLED;
        let mut sm = shader_material(if double {
            "object_double.gdshader"
        } else {
            "object.gdshader"
        });
        sm.set_shader_parameter("albedo_color", &base.get_albedo().to_variant());
        if let Some(t) = base.get_texture(TextureParam::ALBEDO) {
            sm.set_shader_parameter("albedo_texture", &t.to_variant());
            sm.set_shader_parameter("use_albedo_texture", &true.to_variant());
        }
        sm.set_shader_parameter(
            "use_vertex_color",
            &base.get_flag(Flags::ALBEDO_FROM_VERTEX_COLOR).to_variant(),
        );
        sm.set_shader_parameter("uv1_scale", &base.get_uv1_scale().to_variant());
        sm.set_shader_parameter("uv1_offset", &base.get_uv1_offset().to_variant());
        let tr = base.get_transparency();
        if tr != Transparency::DISABLED {
            sm.set_shader_parameter("use_alpha_scissor", &true.to_variant());
            let th = if tr == Transparency::ALPHA_SCISSOR {
                base.get_alpha_scissor_threshold()
            } else {
                self.alpha_scissor_threshold
            };
            sm.set_shader_parameter("alpha_scissor_threshold", &th.to_variant());
        }
        if base.get_feature(Feature::EMISSION) {
            sm.set_shader_parameter("emission_color", &base.get_emission().to_variant());
            sm.set_shader_parameter(
                "emission_energy",
                &base.get_emission_energy_multiplier().to_variant(),
            );
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
    fn enact_geom(&mut self, mut g: Gd<GeometryInstance3D>, flags: i32) {
        if is_editor() {
            self.recall(&g);
        }
        let is_mesh = g.clone().try_cast::<MeshInstance3D>().is_ok()
            || g.clone().try_cast::<MultiMeshInstance3D>().is_ok();
        let mut ok = is_mesh || g.get_material_override().is_some();
        if !self.convert_materials {
            ok = false;
        } else {
            if let Some(m) = g.get_material_override() {
                match self.transmute(&m) {
                    Some(sm) => g.set_material_override(&sm),
                    None => ok = false,
                }
            } else if let Ok(mut mi) = g.clone().try_cast::<MeshInstance3D>() {
                if let Some(mesh) = mi.get_mesh() {
                    for i in 0..mesh.get_surface_count() {
                        let mat = mi
                            .get_surface_override_material(i)
                            .or_else(|| mesh.surface_get_material(i))
                            .unwrap_or_else(|| self.fallback_stuff());
                        match self.transmute(&mat) {
                            Some(sm) => mi.set_surface_override_material(i, &sm),
                            None => ok = false,
                        }
                    }
                }
            } else if let Ok(mmi) = g.clone().try_cast::<MultiMeshInstance3D>() {
                let mat = mmi
                    .get_multimesh()
                    .and_then(|mm| mm.get_mesh())
                    .and_then(|m| m.surface_get_material(0))
                    .unwrap_or_else(|| self.fallback_stuff());
                match self.transmute(&mat) {
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
        let relevant = node.clone().try_cast::<GeometryInstance3D>().is_ok()
            || node.clone().try_cast::<PixelObject>().is_ok();
        if self.refresh_queued || !relevant {
            return;
        }
        if self.base().is_ancestor_of(&node) {
            self.queue_refresh();
        }
    }
    #[func]
    fn queue_refresh(&mut self) {
        if self.refresh_queued {
            return;
        }
        self.refresh_queued = true;
        Self::refresh_settings_deferred(&self.to_gd());
    }
    #[func]
    fn refresh_pixel_settings(&mut self) {
        self.refresh_queued = false;
        self.enact();
        let h = self.highlight;
        if h > 0 {
            highlight(&self.to_gd().upcast(), h);
        }
    }
    #[func]
    fn set_pixel_size(&mut self, v: i32) {
        self.pixel_size = if v < 1 {
            INHERIT
        } else {
            v.min(pt_core::meta::MAX_PIXEL_SIZE as i32)
        };
        self.enact();
    }
    #[func]
    fn set_outline(&mut self, v: i32) {
        self.outline = v.clamp(-1, 1);
        self.enact();
    }
    #[func]
    fn set_outline_id(&mut self, v: i32) {
        self.outline_id = v.clamp(AUTO, pt_core::meta::MAX_USER_ID as i32);
        self.enact();
    }
    #[func]
    fn assign_supersede_contour_tint(&mut self, v: bool) {
        self.supersede_contour_tint = v;
        self.enact();
    }
    #[func]
    fn assign_contour_tint(&mut self, v: Color) {
        self.contour_tint = v;
        self.enact();
    }
    #[func]
    fn assign_swatchset(&mut self, v: i32) {
        self.palette = v.clamp(-1, 1);
        self.enact();
    }
    #[func]
    fn set_dither(&mut self, v: i32) {
        self.dither = v.clamp(-1, 1);
        self.enact();
    }
    #[func]
    fn set_snap_to_texel(&mut self, v: bool) {
        self.snap_to_texel = v;
        self.enact();
    }
    #[func]
    fn set_profiles(&mut self, v: Array<Gd<PixelObjectProfile>>) {
        self.profiles = v;
        self.enact();
        self.base_mut().update_configuration_warnings();
    }
    #[func]
    fn set_profile_targets(&mut self, v: Array<Option<Gd<Node>>>) {
        self.profile_targets = v;
        self.enact();
        self.base_mut().update_configuration_warnings();
    }
    fn profile_index(&self, node: &Gd<Node>) -> Option<usize> {
        self.profile_targets
            .iter_shared()
            .take(self.profiles.len())
            .position(|t| t.as_ref() == Some(node))
    }
    #[func]
    fn set_highlight(&mut self, v: i32) {
        self.highlight = v.clamp(0, 2);
        highlight(&self.to_gd().upcast(), self.highlight);
    }
    #[func]
    fn get_effective(&self) -> VarDictionary {
        let e = self.actual();
        let mut d = VarDictionary::new();
        d.set("pixel_size", e.pixel_size);
        d.set("outline", e.outline);
        d.set("outline_id", e.outline_id);
        d.set("palette", e.palette);
        d.set("dither", e.dither);
        d
    }
    #[func]
    pub fn get_profile_for(&self, node: Gd<Node>) -> Option<Gd<PixelObjectProfile>> {
        self.profile_index(&node).and_then(|i| self.profiles.get(i))
    }
    #[func]
    pub fn ensure_profile_for(&mut self, node: Gd<Node>) -> Gd<PixelObjectProfile> {
        if let Some(p) = self.get_profile_for(node.clone()) {
            return p;
        }
        let p = PixelObjectProfile::new_gd();
        let profiles = self.profiles.len();
        while self.profile_targets.len() > profiles {
            self.profile_targets.pop();
        }
        while self.profile_targets.len() < profiles {
            self.profile_targets.push(None::<&Gd<Node>>);
        }
        self.profiles.push(&p);
        self.profile_targets.push(Some(&node));
        self.enact();
        self.base_mut().update_configuration_warnings();
        p
    }
    #[func]
    pub fn remove_profile_for(&mut self, node: Gd<Node>) -> bool {
        let Some(i) = self.profile_index(&node) else {
            return false;
        };
        self.profiles.remove(i);
        self.profile_targets.remove(i);
        self.enact();
        self.base_mut().update_configuration_warnings();
        true
    }
}
