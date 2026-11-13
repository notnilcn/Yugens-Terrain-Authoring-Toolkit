//! Clip-based animation for the demo knight: KayKit `Rig_Medium` clips (from
//! `demo/assets/characters/animations/Rig_Medium_Animations.glb`, CC0) shared onto the visual's
//! own `Skeleton3D` and driven by an `AnimationTree` built in code.
//!
//! Sharing the skeleton: both rigs are KayKit `Rig_Medium` with identical bone names and rest
//! poses, so the clips only need their track paths re-pointed from the source scene's skeleton
//! to the target skeleton. Tracks for bones the target lacks (the source's IK/control bones) or
//! for non-skeleton nodes (the source's prop nodes) are dropped.
//!
//! Tree: a state machine `Move` (blend tree: 1D blend space Idle / Walking_A / Running_A by ground
//! speed, then a time scale so the feet keep up with the body) ↔ `JumpStart` → `JumpIdle` →
//! `JumpLand` → `Move`.

use godot::classes::animation::LoopMode;
use godot::classes::animation_mixer::AnimationCallbackModeProcess;
use godot::classes::animation_node_state_machine_transition::{AdvanceMode, SwitchMode};
use godot::classes::{
    AnimationLibrary, AnimationNode, AnimationNodeAnimation, AnimationNodeBlendSpace1D,
    AnimationNodeBlendTree, AnimationNodeStateMachine, AnimationNodeStateMachinePlayback,
    AnimationNodeStateMachineTransition, AnimationNodeTimeScale, AnimationPlayer, AnimationTree, Node3D,
    PackedScene, Skeleton3D,
};
use godot::prelude::*;

pub const CLIP_IDLE: &str = "Idle";
pub const CLIP_WALK: &str = "Walking_A";
pub const CLIP_RUN: &str = "Running_A";
pub const CLIP_JUMP_START: &str = "Jump_Start";
pub const CLIP_JUMP_IDLE: &str = "Jump_Idle";
pub const CLIP_JUMP_LAND: &str = "Jump_Land";
const LOOPING: [&str; 4] = [CLIP_IDLE, CLIP_WALK, CLIP_RUN, CLIP_JUMP_IDLE];

const S_MOVE: &str = "Move";
const S_JUMP_START: &str = "JumpStart";
const S_JUMP_IDLE: &str = "JumpIdle";
const S_JUMP_LAND: &str = "JumpLand";
const P_BLEND: &str = "parameters/Move/Locomotion/blend_position";
const P_SCALE: &str = "parameters/Move/Speed/scale";

/// Ground speeds (m/s) at which the walk / run clips look right (no foot sliding) on the
/// KayKit knight at the demo's 0.78 scale.
#[derive(Clone, Copy)]
pub struct Gait {
    pub walk_speed: f32,
    pub run_speed: f32,
}

pub struct Animator {
    tree: Gd<AnimationTree>,
    playback: Gd<AnimationNodeStateMachinePlayback>,
    gait: Gait,
    air_time: f32,
    since_jump: f32,
    blend: f32,
}

impl Animator {
    /// Builds the tree under `visual`; `None` (with a warning) if the clips or skeleton are missing.
    pub fn build(visual: &Gd<Node3D>, source: &Gd<PackedScene>, gait: Gait, xfade: f32) -> Option<Self> {
        let skeleton = find_skeleton(&visual.clone().upcast())?;
        let library = shared_library(source, visual, &skeleton)?;
        for clip in [CLIP_IDLE, CLIP_WALK, CLIP_RUN, CLIP_JUMP_START, CLIP_JUMP_IDLE, CLIP_JUMP_LAND] {
            if !library.has_animation(clip) {
                godot_warn!("MstDemoPlayer: animation source has no '{clip}' clip; using procedural animation");
                return None;
            }
        }

        let mut tree = AnimationTree::new_alloc();
        tree.set_name("AnimationTree");
        let _ = tree.add_animation_library("", &library);
        tree.set_tree_root(&state_machine(gait, xfade));
        tree.set_callback_mode_process(AnimationCallbackModeProcess::PHYSICS);
        let mut parent = visual.clone();
        parent.add_child(&tree);
        // Default root_node "..": the visual, which owns the skeleton path the clips were remapped to.
        tree.set_active(true);
        let playback = tree.get("parameters/playback").try_to::<Gd<AnimationNodeStateMachinePlayback>>().ok()?;
        Some(Self { tree, playback, gait, air_time: 0.0, since_jump: 1.0, blend: 0.0 })
    }

    /// Call once per physics frame after `move_and_slide`.
    pub fn update(&mut self, dt: f32, ground_speed: f32, grounded: bool, jumped: bool) {
        self.since_jump += dt;
        if grounded {
            self.air_time = 0.0;
        } else {
            self.air_time += dt;
        }
        let state = self.playback.get_current_node().to_string();
        if jumped {
            self.since_jump = 0.0;
            self.playback.travel(S_JUMP_START);
        } else if !grounded {
            // Walked off a ledge: fall pose after a short grace (small steps stay in Move).
            if state == S_MOVE && self.air_time > 0.15 {
                self.playback.travel(S_JUMP_IDLE);
            }
        } else if (state == S_JUMP_IDLE || state == S_JUMP_START) && self.since_jump > 0.12 {
            // Touch-down: land in place, or straight back into the run when moving.
            if ground_speed > 0.6 * self.gait.walk_speed {
                self.playback.travel(S_MOVE);
            } else {
                self.playback.travel(S_JUMP_LAND);
            }
        } else if state == S_JUMP_LAND
            && (ground_speed > 0.6 * self.gait.walk_speed
                || self.playback.get_current_play_position() >= self.playback.get_current_length() - 0.1)
        {
            self.playback.travel(S_MOVE);
        }

        // Locomotion: ease the blend position (velocity already eases, this hides the walk/run
        // phase seam), then scale the clip rate so the feet match the actual ground speed.
        let k = (12.0 * dt).clamp(0.0, 1.0);
        self.blend += (ground_speed - self.blend) * k;
        let (walk, run) = (self.gait.walk_speed, self.gait.run_speed.max(self.gait.walk_speed + 0.01));
        let reference = if self.blend <= walk {
            walk
        } else {
            walk + (run - walk) * ((self.blend - walk) / (run - walk)).clamp(0.0, 1.0)
        };
        let scale = if self.blend < 0.3 * walk { 1.0 } else { (self.blend / reference).clamp(0.7, 1.4) };
        self.tree.set(P_BLEND, &self.blend.to_variant());
        self.tree.set(P_SCALE, &scale.to_variant());
    }
}

fn find_skeleton(root: &Gd<Node>) -> Option<Gd<Skeleton3D>> {
    if let Ok(s) = root.clone().try_cast::<Skeleton3D>() {
        return Some(s);
    }
    root.find_children_ex("*").type_("Skeleton3D").recursive(true).owned(false).done().iter_shared().next()?.try_cast::<Skeleton3D>().ok()
}

/// Copies every clip of the source scene's `AnimationPlayer`, re-pointing skeleton tracks at
/// `skeleton` (paths relative to `root`, the AnimationTree's root node).
fn shared_library(source: &Gd<PackedScene>, root: &Gd<Node3D>, skeleton: &Gd<Skeleton3D>) -> Option<Gd<AnimationLibrary>> {
    let Some(inst) = source.instantiate() else {
        godot_warn!("MstDemoPlayer: could not instantiate the animation source");
        return None;
    };
    let result = (|| {
        let player = inst
            .find_children_ex("*")
            .type_("AnimationPlayer")
            .recursive(true)
            .owned(false)
            .done()
            .iter_shared()
            .next()?
            .try_cast::<AnimationPlayer>()
            .ok()?;
        let src_skel = find_skeleton(&inst)?;
        let player_root = player.get_node_or_null(&player.get_root_node())?;
        let src_path = player_root.get_path_to(&src_skel).to_string();
        let dst_path = root.get_path_to(skeleton).to_string();
        let mut out = AnimationLibrary::new_gd();
        for name in player.get_animation_list().as_slice() {
            let Some(clip) = player.get_animation(&StringName::from(name)) else { continue };
            let Some(mut anim) = Some(clip.duplicate_resource()) else { continue };
            for t in (0..anim.get_track_count()).rev() {
                let path = anim.track_get_path(t);
                let node = path.get_concatenated_names().to_string();
                let bone = path.get_concatenated_subnames().to_string();
                if node == src_path && !bone.is_empty() && skeleton.find_bone(&bone) >= 0 {
                    anim.track_set_path(t, &NodePath::from(format!("{dst_path}:{bone}").as_str()));
                } else {
                    anim.remove_track(t);
                }
            }
            let short = name.to_string();
            if LOOPING.contains(&short.as_str()) {
                anim.set_loop_mode(LoopMode::LINEAR);
            }
            let _ = out.add_animation(&StringName::from(name), &anim);
        }
        Some(out)
    })();
    inst.free();
    if result.is_none() {
        godot_warn!("MstDemoPlayer: animation source has no AnimationPlayer / Skeleton3D");
    }
    result
}

fn clip(name: &str) -> Gd<AnimationNodeAnimation> {
    let mut n = AnimationNodeAnimation::new_gd();
    n.set_animation(name);
    n
}

fn transition(xfade: f32, auto_at_end: bool) -> Gd<AnimationNodeStateMachineTransition> {
    let mut t = AnimationNodeStateMachineTransition::new_gd();
    t.set_xfade_time(xfade);
    if auto_at_end {
        t.set_switch_mode(SwitchMode::AT_END);
        t.set_advance_mode(AdvanceMode::AUTO);
    }
    t
}

fn state_machine(gait: Gait, xfade: f32) -> Gd<AnimationNodeStateMachine> {
    let mut locomotion = AnimationNodeBlendSpace1D::new_gd();
    locomotion.set_min_space(0.0);
    locomotion.set_max_space(gait.run_speed.max(gait.walk_speed + 0.01));
    let run = gait.run_speed.max(gait.walk_speed + 0.01);
    for (name, pos) in [(CLIP_IDLE, 0.0), (CLIP_WALK, gait.walk_speed), (CLIP_RUN, run)] {
        locomotion.add_blend_point_ex(&clip(name), pos).name(name).done();
    }

    let mut moving = AnimationNodeBlendTree::new_gd();
    moving.add_node("Locomotion", &locomotion.upcast::<AnimationNode>());
    moving.add_node("Speed", &AnimationNodeTimeScale::new_gd().upcast::<AnimationNode>());
    moving.connect_node("Speed", 0, "Locomotion");
    moving.connect_node("output", 0, "Speed");

    let mut sm = AnimationNodeStateMachine::new_gd();
    sm.add_node(S_MOVE, &moving.upcast::<AnimationNode>());
    sm.add_node(S_JUMP_START, &clip(CLIP_JUMP_START).upcast::<AnimationNode>());
    sm.add_node(S_JUMP_IDLE, &clip(CLIP_JUMP_IDLE).upcast::<AnimationNode>());
    sm.add_node(S_JUMP_LAND, &clip(CLIP_JUMP_LAND).upcast::<AnimationNode>());

    let mut start = transition(0.0, false);
    start.set_advance_mode(AdvanceMode::AUTO);
    sm.add_transition("Start", S_MOVE, &start);
    sm.add_transition(S_MOVE, S_JUMP_START, &transition(0.08, false));
    sm.add_transition(S_MOVE, S_JUMP_IDLE, &transition(xfade * 1.5, false));
    sm.add_transition(S_JUMP_START, S_JUMP_IDLE, &transition(xfade, true));
    sm.add_transition(S_JUMP_START, S_JUMP_LAND, &transition(0.08, false));
    sm.add_transition(S_JUMP_START, S_MOVE, &transition(xfade, false));
    sm.add_transition(S_JUMP_IDLE, S_JUMP_LAND, &transition(0.06, false));
    sm.add_transition(S_JUMP_IDLE, S_MOVE, &transition(xfade, false));
    sm.add_transition(S_JUMP_LAND, S_MOVE, &transition(xfade * 1.5, false));
    sm.add_transition(S_JUMP_LAND, S_JUMP_START, &transition(0.06, false));
    sm
}
