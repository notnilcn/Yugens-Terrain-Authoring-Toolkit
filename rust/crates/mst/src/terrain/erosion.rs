//! `MarchingSquaresErosion` — hydraulic droplet erosion of the parent terrain (port of Minor
//! Deity's `Erosion`, see `pt_core::erosion`). Add it as a child of a `MarchingSquaresTerrain`,
//! tune the Minor Deity parameters in the inspector and press **Erode** (one undo step).
//! Scripts can call `erode()` on this node or `erode_terrain(iterations, seed)` on the terrain.
//!
//! In the editor a press runs on a background thread (the editor stays responsive; `progress`
//! and the `erosion_progress` signal report it) and lands as one undo step when it finishes;
//! outside the editor `erode()` runs synchronously. `mode` picks the faithful per-sample
//! change (Raw) or the level-step quantised one (Terraced; Auto = terraced on cell grids).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;

use godot::classes::{EditorInterface, INode, Node};
use godot::prelude::*;

use pt_core::brush::Touched;
use pt_core::data::TerrainStore;
use pt_core::erosion::{ErosionMode, ErosionParams, StoreErosion, erode_store, erode_store_with};
use pt_core::math::IVec2;

use super::MarchingSquaresTerrain;
use crate::util::*;

#[derive(GodotClass)]
#[class(tool, base = Node)]
pub struct MarchingSquaresErosion {
    /// Droplets per press (Minor Deity `numIterations`).
    #[export(range = (100.0, 500000.0, 100.0, or_greater))]
    pub iterations: i32,
    #[export]
    pub seed: i32,
    #[export_tool_button(fn = Self::erode_button, name = "Erode", icon = "Play")]
    erode_btn: PhantomVar<Callable>,

    #[export_group(name = "Droplet (Minor Deity defaults)")]
    /// Radius of the erosion brush in raster pixels.
    #[export(range = (2.0, 8.0, 1.0))]
    pub erosion_radius: i32,
    /// 0 = droplets follow the slope exactly, 1 = they keep their direction.
    #[export(range = (0.0, 1.0, 0.001))]
    pub inertia: f32,
    /// How much sediment a fast, full droplet can carry.
    #[export(range = (0.0, 32.0, 0.01, or_greater))]
    pub sediment_capacity_factor: f32,
    /// Capacity floor, so droplets on flat ground still erode a little.
    #[export(range = (0.0, 1.0, 0.001))]
    pub min_sediment_capacity: f32,
    #[export(range = (0.0, 1.0, 0.01))]
    pub erode_speed: f32,
    #[export(range = (0.0, 1.0, 0.01))]
    pub deposit_speed: f32,
    #[export(range = (0.0, 1.0, 0.001))]
    pub evaporate_speed: f32,
    #[export(range = (0.0, 32.0, 0.1, or_greater))]
    pub gravity: f32,
    #[export(range = (1.0, 256.0, 1.0))]
    pub max_droplet_lifetime: i32,
    #[export(range = (0.0, 8.0, 0.01))]
    pub initial_water_volume: f32,
    #[export(range = (0.0, 8.0, 0.01))]
    pub initial_speed: f32,

    #[export_group(name = "Terrain")]
    /// Raster pixels per cell (1 = one per square-grid vertex; 2–4 give hex grids smoother channels).
    #[export(range = (1.0, 8.0, 1.0))]
    pub pixels_per_cell: i32,
    /// World height mapped to 1.0 in the model (0 = the terrain's height range). Smaller =
    /// stronger erosion.
    #[export(range = (0.0, 256.0, 0.1, or_greater))]
    pub height_scale: f32,
    /// How the change is written back: Auto (terraced on hex / triangle grids, raw on square),
    /// Raw (the faithful port: continuous per-sample change) or Terraced (change smoothed over
    /// neighbours, quantised to whole level steps, one-cell pits / pillars removed).
    #[export(enum = (Auto = 0, Raw = 1, Terraced = 2))]
    pub mode: i32,
    /// Terraced step height (0 = the terrain's `level_height`).
    #[export(range = (0.0, 16.0, 0.05, or_greater))]
    pub level_step: f32,
    /// Terraced: connected columns / pits of fewer cells than this that the droplets isolated
    /// (carved around) are flattened to their surroundings; plateaus that stood out before the
    /// erosion are kept (0 = off).
    #[export(range = (0.0, 64.0, 1.0, or_greater))]
    pub min_plateau_cells: i32,
    /// Progress of the running background erosion (0..1), -1 when idle (`get_progress()`).
    progress: f32,
    job: Option<Job>,
    base: Base<Node>,
}

/// A background erosion run (editor): works on a copy of the terrain store.
struct Job {
    handle: JoinHandle<(TerrainStore, Touched)>,
    /// Progress in 1/10000.
    done: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    terrain: Gd<MarchingSquaresTerrain>,
}

#[godot_api]
impl INode for MarchingSquaresErosion {
    fn init(base: Base<Node>) -> Self {
        let d = ErosionParams::default();
        let s = StoreErosion::default();
        MarchingSquaresErosion {
            iterations: s.iterations as i32,
            seed: d.seed,
            erode_btn: PhantomVar::default(),
            erosion_radius: d.erosion_radius,
            inertia: d.inertia,
            sediment_capacity_factor: d.sediment_capacity_factor,
            min_sediment_capacity: d.min_sediment_capacity,
            erode_speed: d.erode_speed,
            deposit_speed: d.deposit_speed,
            evaporate_speed: d.evaporate_speed,
            gravity: d.gravity,
            max_droplet_lifetime: d.max_droplet_lifetime,
            initial_water_volume: d.initial_water_volume,
            initial_speed: d.initial_speed,
            pixels_per_cell: s.pixels_per_cell as i32,
            height_scale: s.height_scale,
            mode: s.mode as i32,
            level_step: s.level_step,
            min_plateau_cells: s.min_plateau_cells as i32,
            progress: -1.0,
            job: None,
            base,
        }
    }

    fn process(&mut self, _delta: f64) {
        let Some(job) = &self.job else { return };
        let p = job.done.load(Ordering::Relaxed) as f32 / 10000.0;
        if !job.handle.is_finished() {
            if (p - self.progress).abs() >= 0.01 {
                self.progress = p;
                self.signals().erosion_progress().emit(p);
            }
            return;
        }
        let job = self.job.take().unwrap();
        let cancelled = job.cancel.load(Ordering::Relaxed);
        self.progress = -1.0;
        let Ok((result, touched)) = job.handle.join() else {
            godot_error!("MarchingSquaresErosion: the erosion thread panicked");
            return;
        };
        let mut t = job.terrain;
        if !cancelled && t.is_instance_valid() && !touched.is_empty() {
            // Heights of the touched chunks from the eroded copy, as one undo step.
            edit_terrain_undoable(&mut t, "terrain erosion", |st| {
                for c in &touched {
                    if let (Some(dst), Some(src)) = (st.chunks.get_mut(c), result.chunks.get(c))
                        && dst.heights.len() == src.heights.len()
                    {
                        dst.heights.clone_from(&src.heights);
                    }
                }
                touched.clone()
            });
        }
        self.signals().erosion_progress().emit(1.0);
        self.signals().erosion_finished().emit(!cancelled);
    }

    fn get_configuration_warnings(&self) -> PackedStringArray {
        let mut w = PackedStringArray::new();
        if self.terrain().is_none() {
            w.push("MarchingSquaresErosion must be a child of a MarchingSquaresTerrain.");
        }
        w
    }
}

#[godot_api]
impl MarchingSquaresErosion {
    /// Background erosion progress (0..1), emitted about every percent.
    #[signal]
    fn erosion_progress(progress: f32);

    /// A background erosion finished (`applied` = false when it was cancelled).
    #[signal]
    fn erosion_finished(applied: bool);

    /// Erodes the parent terrain once with the current settings (undoable in the editor). In
    /// the editor this starts a background run (a press while one runs is ignored);
    /// elsewhere it runs synchronously.
    #[func]
    pub fn erode(&mut self) {
        let Some(mut t) = self.terrain() else {
            godot_warn!("MarchingSquaresErosion: parent is not a MarchingSquaresTerrain");
            return;
        };
        if self.job.is_some() {
            godot_warn!("MarchingSquaresErosion: an erosion is already running");
            return;
        }
        let settings = self.settings();
        if is_editor() {
            let mut store = t.bind().store().clone();
            let done = Arc::new(AtomicU32::new(0));
            let cancel = Arc::new(AtomicBool::new(false));
            let (d, c) = (done.clone(), cancel.clone());
            let handle = std::thread::spawn(move || {
                let touched = erode_store_with(&mut store, &settings, &mut |p| {
                    d.store((p * 10000.0) as u32, Ordering::Relaxed);
                    !c.load(Ordering::Relaxed)
                });
                (store, touched)
            });
            self.job = Some(Job { handle, done, cancel, terrain: t });
            self.progress = 0.0;
            self.signals().erosion_progress().emit(0.0);
        } else {
            edit_terrain_undoable(&mut t, "terrain erosion", |st| erode_store(st, &settings));
        }
        // Next press continues with new droplets.
        self.seed = self.seed.wrapping_add(1);
        self.base_mut().notify_property_list_changed();
    }

    /// Stops a running background erosion without changing the terrain.
    #[func]
    pub fn cancel(&mut self) {
        if let Some(j) = &self.job {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Progress of the running background erosion (0..1), -1 when idle.
    #[func]
    pub fn get_progress(&self) -> f32 {
        self.progress
    }

    /// Is a background erosion running?
    #[func]
    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    fn erode_button(&mut self) {
        self.erode();
    }
}

impl MarchingSquaresErosion {
    fn terrain(&self) -> Option<Gd<MarchingSquaresTerrain>> {
        self.base().get_parent()?.try_cast::<MarchingSquaresTerrain>().ok()
    }

    pub fn settings(&self) -> StoreErosion {
        StoreErosion {
            params: ErosionParams {
                seed: self.seed,
                erosion_radius: self.erosion_radius.clamp(2, 8),
                inertia: self.inertia,
                sediment_capacity_factor: self.sediment_capacity_factor,
                min_sediment_capacity: self.min_sediment_capacity,
                erode_speed: self.erode_speed,
                deposit_speed: self.deposit_speed,
                evaporate_speed: self.evaporate_speed,
                gravity: self.gravity,
                max_droplet_lifetime: self.max_droplet_lifetime.max(1),
                initial_water_volume: self.initial_water_volume,
                initial_speed: self.initial_speed,
            },
            iterations: self.iterations.max(0) as u32,
            pixels_per_cell: self.pixels_per_cell.clamp(1, 8) as u32,
            height_scale: self.height_scale.max(0.0),
            region: None,
            mode: ErosionMode::from_i32(self.mode),
            level_step: self.level_step.max(0.0),
            min_plateau_cells: self.min_plateau_cells.max(0) as u32,
        }
    }
}

/// Runs a height edit on `t`, writes it to the data resource and, in the editor, records the
/// changed chunks as one undo action (`set_chunks_data` before → after).
pub fn edit_terrain_undoable(t: &mut Gd<MarchingSquaresTerrain>, name: &str, f: impl FnOnce(&mut pt_core::data::TerrainStore) -> pt_core::brush::Touched) {
    let all: Vec<IVec2> = t.bind().store().chunk_coords();
    let before: Vec<(IVec2, Vec<u8>)> = all.iter().filter_map(|c| Some((*c, t.bind().chunk_bytes(*c)?))).collect();
    let touched = {
        let mut tb = t.bind_mut();
        let touched = tb.edit(f);
        tb.commit_to_data();
        tb.update();
        touched
    };
    if touched.is_empty() || !is_editor() {
        return;
    }
    let mut undo = VarDictionary::new();
    let mut redo = VarDictionary::new();
    for (c, b) in before {
        if !touched.contains(&c) {
            continue;
        }
        let k = from_iv(c);
        undo.set(k, &PackedByteArray::from(b.as_slice()).to_variant());
        redo.set(k, &PackedByteArray::from(t.bind().chunk_bytes(c).unwrap_or_default().as_slice()).to_variant());
    }
    let Some(mut ur) = EditorInterface::singleton().get_editor_undo_redo() else { return };
    let obj = t.clone().upcast::<Object>();
    ur.create_action(name);
    ur.add_do_method(&obj, "set_chunks_data", &[redo.to_variant()]);
    ur.add_undo_method(&obj, "set_chunks_data", &[undo.to_variant()]);
    ur.commit_action_ex().execute(false).done();
}

#[godot_api(secondary)]
impl MarchingSquaresTerrain {
    /// Hydraulic droplet erosion over the whole terrain with Minor Deity's default parameters
    /// (`iterations` droplets). For tuning, add a `MarchingSquaresErosion` child.
    #[func]
    pub fn erode_terrain(&mut self, iterations: i64, seed: i64) {
        let mut e = StoreErosion { iterations: iterations.clamp(0, 10_000_000) as u32, ..Default::default() };
        e.params.seed = seed as i32;
        self.edit(|st| erode_store(st, &e));
        self.commit_to_data();
    }
}
