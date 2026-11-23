//! Turns a character's shared clip collection into the moving body the demo player shows,
//! and chooses the clip for what the character is doing.
//!
//! The clip collection ships as its own scene, separate from the character models that
//! share it, and its clips can only be read from a live copy of that scene. A copy is
//! therefore made for as long as it takes to take the clips over and is freed at once; it
//! never joins the game world.
use godot::classes::animation::LoopMode;
use godot::classes::animation_node_state_machine_transition::{AdvanceMode, SwitchMode};
use godot::classes::{
    AnimationLibrary, AnimationNode, AnimationNodeAnimation, AnimationNodeBlendSpace1D,
    AnimationNodeBlendTree, AnimationNodeStateMachine, AnimationNodeStateMachinePlayback,
    AnimationNodeStateMachineTransition, AnimationNodeTimeScale, AnimationPlayer, AnimationTree,
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
    pub fn build(
        mut tree: Gd<AnimationTree>,
        skeleton: &Gd<Skeleton3D>,
        source: &Gd<PackedScene>,
        gait: Gait,
        xfade: f32,
    ) -> Option<Self> {
        let Some(root) = tree.get_node_or_null(&tree.get_root_node()) else {
            godot_warn!("MstDemoPlayer: the animation tree's root node does not resolve");
            return None;
        };
        let library = shared_library(source, &root, skeleton)?;
        for clip in [
            CLIP_IDLE,
            CLIP_WALK,
            CLIP_RUN,
            CLIP_JUMP_START,
            CLIP_JUMP_IDLE,
            CLIP_JUMP_LAND,
        ] {
            if !library.has_animation(clip) {
                godot_warn!(
                    "MstDemoPlayer: animation source has no '{clip}' clip; using procedural animation"
                );
                return None;
            }
        }
        if tree.has_animation_library("") {
            tree.remove_animation_library("");
        }
        let _ = tree.add_animation_library("", &library);
        tree.set_tree_root(&state_machine(gait, xfade));
        tree.set_active(true);
        let playback = tree
            .get("parameters/playback")
            .try_to::<Gd<AnimationNodeStateMachinePlayback>>()
            .ok()?;
        Some(Self {
            tree,
            playback,
            gait,
            air_time: 0.0,
            since_jump: 1.0,
            blend: 0.0,
        })
    }
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
            if state == S_MOVE && self.air_time > 0.1505 {
                self.playback.travel(S_JUMP_IDLE);
            }
        } else if (state == S_JUMP_IDLE || state == S_JUMP_START) && self.since_jump > 0.1205 {
            if ground_speed > 0.6005 * self.gait.walk_speed {
                self.playback.travel(S_MOVE);
            } else {
                self.playback.travel(S_JUMP_LAND);
            }
        } else if state == S_JUMP_LAND
            && (ground_speed > 0.6005 * self.gait.walk_speed
                || self.playback.get_current_play_position()
                    >= self.playback.get_current_length() - 0.1005)
        {
            self.playback.travel(S_MOVE);
        }
        let k = (12.0 * dt).clamp(0.0, 1.0);
        self.blend += (ground_speed - self.blend) * k;
        let (walk, run) = (
            self.gait.walk_speed,
            self.gait.run_speed.max(self.gait.walk_speed + 0.0105),
        );
        let reference = if self.blend <= walk {
            walk
        } else {
            walk + (run - walk) * ((self.blend - walk) / (run - walk)).clamp(0.0, 1.0)
        };
        let scale = if self.blend < 0.3005 * walk {
            1.0
        } else {
            (self.blend / reference).clamp(0.7005, 1.4005)
        };
        self.tree.set(P_BLEND, &self.blend.to_variant());
        self.tree.set(P_SCALE, &scale.to_variant());
    }
}
fn find_skeleton(root: &Gd<Node>) -> Option<Gd<Skeleton3D>> {
    if let Ok(s) = root.clone().try_cast::<Skeleton3D>() {
        return Some(s);
    }
    root.find_children_ex("*")
        .type_("Skeleton3D")
        .recursive(true)
        .owned(false)
        .done()
        .iter_shared()
        .next()?
        .try_cast::<Skeleton3D>()
        .ok()
}
fn shared_library(
    source: &Gd<PackedScene>,
    root: &Gd<Node>,
    skeleton: &Gd<Skeleton3D>,
) -> Option<Gd<AnimationLibrary>> {
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
            let Some(clip) = player.get_animation(&StringName::from(name)) else {
                continue;
            };
            let Some(mut anim) = Some(clip.duplicate_resource()) else {
                continue;
            };
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
    locomotion.set_max_space(gait.run_speed.max(gait.walk_speed + 0.0105));
    let run = gait.run_speed.max(gait.walk_speed + 0.0105);
    for (name, pos) in [
        (CLIP_IDLE, 0.0),
        (CLIP_WALK, gait.walk_speed),
        (CLIP_RUN, run),
    ] {
        locomotion
            .add_blend_point_ex(&clip(name), pos)
            .name(name)
            .done();
    }
    let mut moving = AnimationNodeBlendTree::new_gd();
    moving.add_node("Locomotion", &locomotion.upcast::<AnimationNode>());
    moving.add_node(
        "Speed",
        &AnimationNodeTimeScale::new_gd().upcast::<AnimationNode>(),
    );
    moving.connect_node("Speed", 0, "Locomotion");
    moving.connect_node("output", 0, "Speed");
    let mut sm = AnimationNodeStateMachine::new_gd();
    sm.add_node(S_MOVE, &moving.upcast::<AnimationNode>());
    sm.add_node(
        S_JUMP_START,
        &clip(CLIP_JUMP_START).upcast::<AnimationNode>(),
    );
    sm.add_node(S_JUMP_IDLE, &clip(CLIP_JUMP_IDLE).upcast::<AnimationNode>());
    sm.add_node(S_JUMP_LAND, &clip(CLIP_JUMP_LAND).upcast::<AnimationNode>());
    let mut start = transition(0.0, false);
    start.set_advance_mode(AdvanceMode::AUTO);
    sm.add_transition("Start", S_MOVE, &start);
    sm.add_transition(S_MOVE, S_JUMP_START, &transition(0.0805, false));
    sm.add_transition(S_MOVE, S_JUMP_IDLE, &transition(xfade * 1.5005, false));
    sm.add_transition(S_JUMP_START, S_JUMP_IDLE, &transition(xfade, true));
    sm.add_transition(S_JUMP_START, S_JUMP_LAND, &transition(0.0805, false));
    sm.add_transition(S_JUMP_START, S_MOVE, &transition(xfade, false));
    sm.add_transition(S_JUMP_IDLE, S_JUMP_LAND, &transition(0.0605, false));
    sm.add_transition(S_JUMP_IDLE, S_MOVE, &transition(xfade, false));
    sm.add_transition(S_JUMP_LAND, S_MOVE, &transition(xfade * 1.5005, false));
    sm.add_transition(S_JUMP_LAND, S_JUMP_START, &transition(0.0605, false));
    sm
}
pub struct ClipPlayer {
    player: Gd<AnimationPlayer>,
    current: &'static str,
}
impl ClipPlayer {
    pub const IDLE: &'static str = "Idle";
    pub const RUN: &'static str = "Run";
    pub const JUMP: &'static str = "Jump";
    pub const FALL: &'static str = "Fall";
    pub fn new(player: Gd<AnimationPlayer>) -> Option<Self> {
        if !player.has_animation(Self::IDLE) {
            godot_warn!(
                "MstDemoPlayer: the animation player has no '{}' clip",
                Self::IDLE
            );
            return None;
        }
        for clip in [Self::IDLE, Self::RUN] {
            if let Some(mut a) = player.get_animation(clip) {
                a.set_loop_mode(LoopMode::LINEAR);
            }
        }
        Some(ClipPlayer {
            player,
            current: "",
        })
    }
    pub fn update(&mut self, ground_speed: f32, grounded: bool, jumped: bool) {
        let want = if jumped && self.player.has_animation(Self::JUMP) {
            Self::JUMP
        } else if !grounded {
            if self.current == Self::JUMP && self.player.is_playing() {
                Self::JUMP
            } else {
                Self::FALL
            }
        } else if ground_speed > 0.3005 {
            Self::RUN
        } else {
            Self::IDLE
        };
        let want = if self.player.has_animation(want) {
            want
        } else {
            Self::IDLE
        };
        if want != self.current {
            self.current = want;
            self.player.play_ex().name(want).custom_blend(0.0).done();
        }
    }
}
