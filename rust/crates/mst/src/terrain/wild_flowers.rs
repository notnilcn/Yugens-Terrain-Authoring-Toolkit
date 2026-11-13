//! `MarchingSquaresWildFlowers` — Minor Deity's clustered wild-flower placement
//! (`pt_core::wildflowers`, ported from `GetClusteredWildFlower` and `UpdateDoodadsJob`) on the
//! parent terrain's scatter layer.
//!
//! Add it as a child of a `MarchingSquaresTerrain` and press **Scatter Wild Flowers**: one
//! `PixelScatterItem` per species (8 flowers + bilberry bushes) is created in `species_items`
//! and appended to the terrain's `scatter_items`, then every cell gets Minor Deity's flower
//! count and species from the cell noise and its ground water. Re-pressing replaces the
//! previous wild flowers (one undo step); the species items can be restyled freely.

use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::{ArrayMesh, EditorInterface, INode, Node, Node3D, StandardMaterial3D};
use godot::classes::base_material_3d::Flags as BmFlags;
use godot::prelude::*;

use std::collections::HashMap;

use pt_core::grid::GridType;
use pt_core::math::{IVec2, Vec2 as PVec2, iv2};
use pt_core::scatter::ScatterSet;
use pt_core::wildflowers::{CellClimate, WILD_PICKS, WildFlowerRules, area_density, cell_center, cell_flowers, cell_noise, cell_points, slope01, species_mesh};

use super::MarchingSquaresTerrain;
use super::scatter::PixelScatterItem;
use crate::util::*;

#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct MarchingSquaresWildFlowers {
    #[export_tool_button(fn = Self::scatter_button, name = "Scatter Wild Flowers", icon = "Play")]
    scatter_btn: PhantomVar<Callable>,
    #[export_tool_button(fn = Self::clear_button, name = "Clear Wild Flowers", icon = "Remove")]
    clear_btn: PhantomVar<Callable>,
    /// One item per species: Bouncing Betty, Knapweed, Chamomile, Chicory, Poppy,
    /// St John's Wort, Cornflower, Goldenrod, Bilberry (created on the first scatter).
    #[export]
    pub species_items: Array<Option<Gd<PixelScatterItem>>>,
    #[export]
    pub seed: i32,

    #[export_group(name = "Placement (Minor Deity defaults)")]
    /// Ground water a cell needs for flowers (`flowerThreshold`); twice this adds the wet pass.
    #[export(range = (0.0, 1.0, 0.01))]
    pub flower_threshold: f32,
    /// Steepest ground for a flower (`slope01 = 0.5 · tan(angle)`; 0.376 ≈ 37°).
    #[export(range = (0.0, 1.0, 0.005))]
    pub max_slope: f32,
    /// Map points across one cell (Minor Deity hexes ≈ 8).
    #[export(range = (2.0, 32.0, 1.0))]
    pub points_per_cell: i32,
    /// Position jitter in map-point spacings.
    #[export(range = (0.0, 1.0, 0.01))]
    pub jitter: f32,
    /// Multiplies the flower counts (1 = Minor Deity's 8–24 + 12–24 wet flowers per 4-unit hex, scaled to the cell area).
    #[export(range = (0.0, 2.0, 0.01, or_greater))]
    pub density: f32,
    /// Multiplies the size of the species patches (1 = Minor Deity's noise periods ≈ 4–31 units).
    #[export(range = (0.05, 8.0, 0.01, or_greater))]
    pub patch_scale: f32,

    #[export_group(name = "Ground water")]
    /// Ground water everywhere (Minor Deity's climate `y`; its climate simulation is not ported).
    #[export(range = (0.0, 1.0, 0.01))]
    pub ground_water: f32,
    /// ± ground water from the fourth noise channel (wet and dry meadows).
    #[export(range = (0.0, 1.0, 0.01))]
    pub ground_water_variation: f32,
    /// Extra ground water at the water line, fading out `shore_distance` units above it.
    #[export(range = (0.0, 1.0, 0.01))]
    pub shore_wetness: f32,
    #[export]
    pub water_level: f32,
    #[export(range = (0.01, 64.0, 0.01, or_greater))]
    pub shore_distance: f32,
    /// Fewer flowers in cells with trees (scatter items with the Tree / Pine shape).
    #[export]
    pub trees_shade: bool,
    base: Base<Node>,
}

#[godot_api]
impl INode for MarchingSquaresWildFlowers {
    fn init(base: Base<Node>) -> Self {
        let r = WildFlowerRules::default();
        MarchingSquaresWildFlowers {
            scatter_btn: PhantomVar::default(),
            clear_btn: PhantomVar::default(),
            species_items: Array::new(),
            seed: 0,
            flower_threshold: r.flower_threshold,
            max_slope: r.max_slope01,
            points_per_cell: r.points_per_cell as i32,
            jitter: r.jitter,
            density: r.density,
            patch_scale: 1.0,
            ground_water: 0.25,
            ground_water_variation: 0.25,
            shore_wetness: 0.3,
            water_level: 0.0,
            shore_distance: 3.0,
            trees_shade: true,
            base,
        }
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.terrain().is_none() {
            w.push("MarchingSquaresWildFlowers must be a child of a MarchingSquaresTerrain.");
        }
        w
    }
}

/// The water surfaces of the scene, seen from one terrain: the terrain's stored cell water
/// levels, every visible `PixelWater` plane (sea / lake, a rectangle at its height) and its
/// raised water cells (pools, rivers).
struct WaterCover {
    /// Terrain-local -> water-local transform and the plane's half size.
    planes: Vec<(Transform3D, Vector2)>,
    /// Raised water surface per cell (terrain-local height).
    cells: HashMap<IVec2, f32>,
}

impl WaterCover {
    fn none() -> WaterCover {
        WaterCover { planes: Vec::new(), cells: HashMap::new() }
    }

    fn find(t: &Gd<MarchingSquaresTerrain>) -> WaterCover {
        let mut cover = WaterCover::none();
        // Water levels stored in the terrain data (`set_cell_water`, PTC2 blobs) first.
        for (c, level) in t.bind().store().water_cells() {
            cover.cells.insert(c, level);
        }
        let tn = t.clone().upcast::<Node>();
        let t3 = t.clone().upcast::<Node3D>();
        if !t3.is_inside_tree() {
            return cover;
        }
        // Search the terrain's scene: its owner; without one, the terrain is the edited scene's
        // root (editor) or was added from code (search the whole tree).
        let scene = match tn.get_owner() {
            Some(o) => o,
            None if is_editor() => tn.clone(),
            None => tn.get_tree().get_root().upcast::<Node>(),
        };
        let to_global = t3.get_global_transform();
        let mut waters: Vec<Gd<Node>> = scene.find_children_ex("*").type_("PixelWater").recursive(true).owned(false).done().iter_shared().collect();
        if scene.is_class("PixelWater") {
            waters.push(scene.clone());
        }
        for w in waters {
            let Ok(m) = w.clone().try_cast::<Node3D>() else { continue };
            if !m.is_visible_in_tree() {
                continue;
            }
            let size = w.get("size").try_to::<Vector2>().unwrap_or(Vector2::ZERO);
            cover.planes.push((m.get_global_transform().affine_inverse() * to_global, size * 0.5));
            // Raised water belongs to the water's `terrain` (empty = the first terrain).
            let own = w.get("terrain").try_to::<Gd<Node>>().ok();
            if own.is_some_and(|o| o != tn) {
                continue;
            }
            if let Ok(cells) = w.get("water_cells").try_to::<PackedVector3Array>() {
                for v in cells.as_slice() {
                    let c = iv2(v.x.round() as i32, v.y.round() as i32);
                    let e = cover.cells.entry(c).or_insert(v.z);
                    *e = e.max(v.z);
                }
            }
        }
        cover
    }

    /// Is the terrain-local ground point (`p`, height `h`) under water?
    fn covers(&self, cfg: &pt_core::grid::GridConfig, p: PVec2, h: f32) -> bool {
        if cfg.grid.is_cell_mode()
            && let Some(level) = self.cells.get(&cfg.world_to_sample(p))
            && *level > h
        {
            return true;
        }
        self.planes.iter().any(|(xf, half)| {
            let l = *xf * Vector3::new(p.x, h, p.y);
            l.y < 0.0 && l.x.abs() <= half.x && l.z.abs() <= half.y
        })
    }
}

/// Single-species flower tuft mesh (vertex colors via a StandardMaterial3D surface material).
fn tuft_mesh(index: usize) -> Gd<godot::classes::Mesh> {
    let m = species_mesh(WILD_PICKS[index], index as u32 * 7 + 3);
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let pos: PackedVector3Array = m.positions.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect();
    let nrm: PackedVector3Array = m.normals.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect();
    let col: PackedColorArray = m.colors.iter().map(|c| Color::from_rgba(c[0], c[1], c[2], c[3])).collect();
    arrays.set(ArrayType::VERTEX.ord() as usize, &pos.to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &nrm.to_variant());
    arrays.set(ArrayType::COLOR.ord() as usize, &col.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_flag(BmFlags::ALBEDO_FROM_VERTEX_COLOR, true);
    mesh.surface_set_material(0, &mat);
    mesh.upcast()
}

/// A built-in item for one species (flower prop tinted with the species color, or a small bush).
fn species_item(index: usize) -> Gd<PixelScatterItem> {
    let pick = WILD_PICKS[index];
    let bush = index == 8;
    let mut it = PixelScatterItem::create_builtin(if bush { "bush" } else { "flowers" }.into());
    {
        let mut b = it.bind_mut();
        b.display_name = format!("Wild {}", pick.name()).as_str().into();
        if bush {
            let [r, g, bl] = pick.color();
            b.color = Color::from_rgb(r * 1.6, g * 1.6, bl * 1.6);
        } else {
            b.mesh = Some(tuft_mesh(index));
        }
        b.scale_min = if bush { 0.35 } else { 0.7 };
        b.scale_max = if bush { 0.6 } else { 1.1 };
        b.hue_jitter = 0.015;
        b.seed = index as i32 + 1;
    }
    it
}

#[godot_api]
impl MarchingSquaresWildFlowers {
    /// Replaces the wild flowers on the parent terrain (undoable in the editor). Returns the
    /// number of flowers placed.
    #[func]
    pub fn scatter_flowers(&mut self) -> i64 {
        self.run(true)
    }

    /// Removes every instance of the species items.
    #[func]
    pub fn clear_flowers(&mut self) {
        self.run(false);
    }

    fn scatter_button(&mut self) {
        let n = self.scatter_flowers();
        godot_print!("MarchingSquaresWildFlowers: placed {n} wild flowers");
    }

    fn clear_button(&mut self) {
        self.clear_flowers();
    }
}

impl MarchingSquaresWildFlowers {
    fn terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.base().get_parent()?.try_cast::<MarchingSquaresTerrain>().ok()
    }

    pub fn rules(&self) -> WildFlowerRules {
        WildFlowerRules {
            flower_threshold: self.flower_threshold,
            max_slope01: self.max_slope,
            points_per_cell: self.points_per_cell.clamp(2, 32) as u32,
            jitter: self.jitter,
            density: self.density.max(0.0),
            seed: self.seed as u32,
            ..Default::default()
        }
    }

    fn ensure_species(&mut self) {
        while self.species_items.len() < WILD_PICKS.len() {
            self.species_items.push(None::<&Gd<PixelScatterItem>>);
        }
        for i in 0..WILD_PICKS.len() {
            if self.species_items.at(i).is_none() {
                self.species_items.set(i, Some(&species_item(i)));
            }
        }
    }

    fn run(&mut self, place: bool) -> i64 {
        let Some(mut t) = self.terrain() else {
            godot_warn!("MarchingSquaresWildFlowers: parent is not a MarchingSquaresTerrain");
            return 0;
        };
        if place {
            self.ensure_species();
        }
        let items_before = t.bind().scatter_items.clone();
        let scatter_before = t.bind().scatter_bytes();
        // Map species → terrain scatter item index (appending missing ones).
        let mut ids: Vec<Option<u16>> = Vec::new();
        {
            let mut tb = t.bind_mut();
            let mut items = tb.scatter_items.duplicate_shallow();
            for sp in self.species_items.iter_shared() {
                let Some(sp) = sp else {
                    ids.push(None);
                    continue;
                };
                let found = items.iter_shared().position(|x| x.is_some_and(|x| x == sp));
                let idx = match found {
                    Some(i) => i,
                    None if place => {
                        items.push(Some(&sp));
                        items.len() - 1
                    }
                    None => {
                        ids.push(None);
                        continue;
                    }
                };
                ids.push(Some(idx as u16));
            }
            tb.scatter_items = items;
        }
        let species: Vec<u16> = ids.iter().flatten().copied().collect();
        let water = if place { WaterCover::find(&t) } else { WaterCover::none() };
        let mut count = 0i64;
        {
            let mut tb = t.bind_mut();
            let mut set = std::mem::take(&mut tb.scatter.set);
            for v in set.tiles.values_mut() {
                v.retain(|i| !species.contains(&i.item));
            }
            set.tiles.retain(|_, v| !v.is_empty());
            if place {
                count = self.place(&tb, &mut set, &ids, &water);
            }
            tb.scatter.set = set;
            tb.scatter.mark_all();
            tb.scatter_write();
            tb.update();
        }
        if is_editor()
            && let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo()
        {
            let obj = t.clone().upcast::<Object>();
            let items_after = t.bind().scatter_items.clone();
            let scatter_after = t.bind().scatter_bytes();
            ur.create_action(if place { "scatter wild flowers" } else { "clear wild flowers" });
            ur.add_do_property(&obj, "scatter_items", &items_after.to_variant());
            ur.add_do_method(&obj, "set_scatter_data", &[PackedByteArray::from(scatter_after.as_slice()).to_variant()]);
            ur.add_undo_property(&obj, "scatter_items", &items_before.to_variant());
            ur.add_undo_method(&obj, "set_scatter_data", &[PackedByteArray::from(scatter_before.as_slice()).to_variant()]);
            ur.commit_action_ex().execute(false).done();
        }
        count
    }

    /// Minor Deity's per-cell flower pass over every cell of the terrain. Nothing grows under
    /// `water_level` or under any PixelWater surface (sea plane or raised pool cells).
    fn place(&self, t: &MarchingSquaresTerrain, set: &mut ScatterSet, ids: &[Option<u16>], water: &WaterCover) -> i64 {
        let cfg = t.store().config.clone();
        let mut rules = self.rules();
        // Minor Deity's counts are per 4-unit hex: keep its flowers per square unit.
        rules.density *= area_density(&cfg);
        let cells: Vec<IVec2> = if cfg.grid == GridType::Square {
            t.store()
                .chunk_coords()
                .into_iter()
                .flat_map(|c| {
                    let o = c * cfg.chunk_stride();
                    let n = cfg.chunk_size;
                    (0..n).flat_map(move |z| (0..n).map(move |x| o + iv2(x, z)))
                })
                .collect()
        } else {
            t.store().chunk_coords().into_iter().flat_map(|c| cfg.chunk_cells(c)).collect()
        };
        let trees: Vec<u16> = (0..t.scatter_items.len())
            .filter(|i| t.scatter_item(*i).is_some_and(|it| matches!(it.bind().shape, 3 | 4)))
            .map(|i| i as u16)
            .collect();
        let item_rules: Vec<Option<pt_core::scatter::ScatterRules>> = ids.iter().map(|id| id.and_then(|i| t.scatter_item(i as usize)).map(|it| it.bind().rules())).collect();
        let radius = cfg.cell_size_safe().x.max(cfg.cell_size_safe().y) * 0.5;
        let slope_at = |p: PVec2| t.surface_at(p).map(|(_, n, _)| slope01(n));
        let mut count = 0i64;
        for c in cells {
            let centre = cell_center(&cfg, c);
            let Some(h) = t.local_height_at(centre) else { continue };
            if water.covers(&cfg, centre, h) {
                continue;
            }
            let noise = cell_noise(centre, self.seed as u32, self.patch_scale);
            let shore = 1.0 - ((h - self.water_level) / self.shore_distance.max(0.01)).clamp(0.0, 1.0);
            let shore = if h < self.water_level { 0.0 } else { shore };
            let gw = (self.ground_water + (noise[3] - 0.5) * 2.0 * self.ground_water_variation + self.shore_wetness * shore).clamp(0.0, 1.0);
            let tree_cover = if self.trees_shade && trees.iter().any(|it| set.any_within(*it, centre, radius)) { 1.0 } else { 0.0 };
            let climate = CellClimate { noise, ground_water: gw, tree_cover, river: 0.0 };
            let (points, spacing) = cell_points(&cfg, c, rules.points_per_cell);
            let index = (c.x as u32).wrapping_mul(73_856_093) ^ (c.y as u32).wrapping_mul(19_349_663);
            for (k, spot) in cell_flowers(&rules, index, &points, spacing, &climate, &slope_at).into_iter().enumerate() {
                let si = spot.pick.index();
                let (Some(Some(item)), Some(Some(r))) = (ids.get(si), item_rules.get(si)) else { continue };
                if t.local_height_at(spot.pos).is_none_or(|hh| hh < self.water_level || water.covers(&cfg, spot.pos, hh)) {
                    continue;
                }
                set.insert(r.make_instance(*item, spot.pos, c, k as u32 + 1));
                count += 1;
            }
        }
        count
    }
}
