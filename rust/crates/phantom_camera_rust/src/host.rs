#![allow(clippy::collapsible_if)]
use godot::builtin::math::ApproxEq;
use godot::builtin::{Quaternion, Transform2D, Transform3D, Variant, Vector2, Vector3};
use godot::classes::{
    Camera2D, Camera3D, CameraAttributes, CameraAttributesPhysical, CameraAttributesPractical,
    ClassDb, Engine, Node, Os,
};
use godot::prelude::*;
use crate::abi::InterpolationMode;
use crate::manager::PhantomCameraManager;
use crate::pcam2d::PhantomCamera2D;
use crate::pcam3d::PhantomCamera3D;
use crate::resources::TweenDirectorResource;
use crate::tween_director::PhantomCameraTweenDirector;
const INTERPOLATION_AUTO: i64 = InterpolationMode::Auto as i64;
const INTERPOLATION_IDLE: i64 = InterpolationMode::Idle as i64;
const INTERPOLATION_PHYSICS: i64 = InterpolationMode::Physics as i64;
const INTERPOLATION_MANUAL: i64 = InterpolationMode::Manual as i64;
const PROCESS_MODE_INHERIT: i64 = 0;
const PROCESS_MODE_DISABLED: i64 = 4;
const PHYSICS_INTERPOLATION_MODE_INHERIT: i64 = 0;
const PHYSICS_INTERPOLATION_MODE_ON: i64 = 1;
const PHYSICS_INTERPOLATION_MODE_OFF: i64 = 2;
const CAMERA2D_PROCESS_PHYSICS: i64 = 0;
const CAMERA2D_PROCESS_IDLE: i64 = 1;
enum AnyPcam {
    TwoD(Gd<PhantomCamera2D>),
    ThreeD(Gd<PhantomCamera3D>),
}
impl AnyPcam {
    fn from_node(pcam: &Gd<Node>) -> Option<Self> {
        if let Ok(two_d) = pcam.clone().try_cast::<PhantomCamera2D>() {
            return Some(Self::TwoD(two_d));
        }
        pcam.clone()
            .try_cast::<PhantomCamera3D>()
            .ok()
            .map(Self::ThreeD)
    }
    fn priority(&self) -> i64 {
        match self {
            Self::TwoD(pcam) => pcam.bind().current_priority(),
            Self::ThreeD(pcam) => pcam.bind().current_priority(),
        }
    }
    fn in_layer(&self, host_layers: i64) -> bool {
        match self {
            Self::TwoD(pcam) => pcam.bind().is_in_host_layer(host_layers),
            Self::ThreeD(pcam) => pcam.bind().is_in_host_layer(host_layers),
        }
    }
    fn visible(&self) -> bool {
        match self {
            Self::TwoD(pcam) => pcam.bind().pcam_is_visible(),
            Self::ThreeD(pcam) => pcam.bind().pcam_is_visible(),
        }
    }
    fn set_active(&self, value: bool) {
        match self {
            Self::TwoD(pcam) => pcam.clone().bind_mut().host_set_active(value),
            Self::ThreeD(pcam) => pcam.clone().bind_mut().host_set_active(value),
        }
    }
    fn set_tween_skip(&self, value: bool) {
        match self {
            Self::TwoD(pcam) => pcam.clone().bind_mut().host_set_tween_skip(value),
            Self::ThreeD(pcam) => pcam.clone().bind_mut().host_set_tween_skip(value),
        }
    }
    fn get_tween_skip(&self) -> bool {
        match self {
            Self::TwoD(pcam) => pcam.bind().get_tween_skip(),
            Self::ThreeD(pcam) => pcam.bind().get_tween_skip(),
        }
    }
    fn tween_duration(&self) -> f64 {
        match self {
            Self::TwoD(pcam) => pcam.bind().get_tween_duration(),
            Self::ThreeD(pcam) => pcam.bind().get_tween_duration(),
        }
    }
    fn tween_transition(&self) -> i64 {
        match self {
            Self::TwoD(pcam) => pcam.bind().get_tween_transition(),
            Self::ThreeD(pcam) => pcam.bind().get_tween_transition(),
        }
    }
    fn tween_ease(&self) -> i64 {
        match self {
            Self::TwoD(pcam) => pcam.bind().get_tween_ease(),
            Self::ThreeD(pcam) => pcam.bind().get_tween_ease(),
        }
    }
    fn tween_resource(&self) -> Option<Gd<crate::resources::PhantomCameraTween>> {
        match self {
            Self::TwoD(pcam) => pcam.bind().get_tween_resource(),
            Self::ThreeD(pcam) => pcam.bind().get_tween_resource(),
        }
    }
}
fn tween_interp_f64(
    from: f64,
    to: f64,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> f64 {
    crate::math::interpolate_value_f64(from, to - from, elapsed, duration, transition, ease)
}
fn tween_interp_v2(
    from: Vector2,
    to: Vector2,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Vector2 {
    crate::math::interpolate_value_v2(from, to - from, elapsed, duration, transition, ease)
}
fn tween_interp_v3(
    from: Vector3,
    to: Vector3,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Vector3 {
    crate::math::interpolate_value_v3(from, to - from, elapsed, duration, transition, ease)
}
fn tween_interp_quat(
    from: Quaternion,
    to: Quaternion,
    elapsed: f64,
    duration: f64,
    transition: i64,
    ease: i64,
) -> Quaternion {
    crate::math::interpolate_value_quat(
        from,
        from.inverse() * to,
        elapsed,
        duration,
        transition,
        ease,
    )
}
fn pget_f64(object: &Gd<godot::classes::Object>, property: &str) -> f64 {
    object.get(property).try_to::<f64>().unwrap_or_default()
}
fn pget_bool(object: &Gd<godot::classes::Object>, property: &str) -> bool {
    object.get(property).try_to::<bool>().unwrap_or_default()
}
fn pget_v2(object: &Gd<godot::classes::Object>, property: &str) -> Vector2 {
    object
        .get(property)
        .try_to::<Vector2>()
        .unwrap_or(Vector2::ZERO)
}
fn pset_f64(object: &mut Gd<godot::classes::Object>, property: &str, value: f64) {
    object.set(property, &value.to_variant());
}
fn pset_bool(object: &mut Gd<godot::classes::Object>, property: &str, value: bool) {
    object.set(property, &value.to_variant());
}
fn pset_v2(object: &mut Gd<godot::classes::Object>, property: &str, value: Vector2) {
    object.set(property, &value.to_variant());
}
fn pset_i64(object: &mut Gd<godot::classes::Object>, property: &str, value: i64) {
    object.set(property, &value.to_variant());
}
#[derive(GodotClass)]
#[class(base = Node, tool)]
pub struct PhantomCameraHost {
    base: Base<Node>,
    #[export(flags_2d_render)]
    #[var(set = set_host_layers, get = get_host_layers)]
    pub host_layers: i64,
    #[export]
    #[var(
        set = set_interpolation_mode,
        get = get_interpolation_mode,
        hint = ENUM,
        hint_string = "Auto:0,Idle:1,Physics:2,Manual:3"
    )]
    pub interpolation_mode: i64,
    active_pcam_2d: Option<Gd<PhantomCamera2D>>,
    active_pcam_3d: Option<Gd<PhantomCamera3D>>,
    active_pcam_priority: i64,
    active_pcam_missing: bool,
    active_pcam_has_damping: bool,
    follow_target_physics_based: bool,
    prev_active_pcam_2d_transform: Transform2D,
    prev_active_pcam_3d_transform: Transform3D,
    trigger_pcam_tween: bool,
    tween_elapsed_time: f64,
    tween_duration: f64,
    tween_transition: i64,
    tween_ease: i64,
    tween_is_instant: bool,
    #[allow(dead_code)]
    multiple_pcam_hosts: bool,
    is_child_of_camera: bool,
    is_2d: bool,
    camera_zoom: Vector2,
    active_pcam_2d_glob_transform: Transform2D,
    active_pcam_3d_glob_transform: Transform3D,
    has_noise_emitted: bool,
    reset_noise_offset_2d: bool,
    noise_emitted_output_2d: Transform2D,
    noise_emitted_output_3d: Transform3D,
    #[allow(dead_code)]
    show_warning: bool,
    pub camera_2d: Option<Gd<Camera2D>>,
    pub camera_3d: Option<Gd<Camera3D>>,
    pending_ready: bool,
    prev_cam_attributes: Option<Gd<CameraAttributes>>,
    cam_attribute_type: i64,
    cam_attribute_changed: bool,
    cam_attribute_assigned: bool,
    prev_cam_auto_exposure_scale: f64,
    cam_auto_exposure_scale_changed: bool,
    prev_cam_auto_exposure_speed: f64,
    cam_auto_exposure_speed_changed: bool,
    prev_cam_exposure_multiplier: f64,
    cam_exposure_multiplier_changed: bool,
    prev_cam_exposure_sensitivity: f64,
    cam_exposure_sensitivity_changed: bool,
    prev_cam_exposure_min_sensitivity: f64,
    cam_exposure_min_sensitivity_changed: bool,
    prev_cam_exposure_max_sensitivity: f64,
    cam_exposure_max_sensitivity_changed: bool,
    prev_cam_dof_blur_amount: f64,
    cam_dof_blur_amount_changed: bool,
    cam_dof_blur_far_distance_default: f64,
    prev_cam_dof_blur_far_distance: f64,
    cam_dof_blur_far_distance_changed: bool,
    cam_dof_blur_far_transition_default: f64,
    prev_cam_dof_blur_far_transition: f64,
    cam_dof_blur_far_transition_changed: bool,
    cam_dof_blur_near_distance_default: f64,
    prev_cam_dof_blur_near_distance: f64,
    cam_dof_blur_near_distance_changed: bool,
    cam_dof_blur_near_transition_default: f64,
    prev_cam_dof_blur_near_transition: f64,
    cam_dof_blur_near_transition_changed: bool,
    prev_cam_exposure_min_exposure_value: f64,
    cam_exposure_min_exposure_value_changed: bool,
    prev_cam_exposure_max_exposure_value: f64,
    cam_exposure_max_exposure_value_changed: bool,
    prev_cam_exposure_aperture: f64,
    cam_exposure_aperture_changed: bool,
    prev_cam_exposure_shutter_speed: f64,
    cam_exposure_shutter_speed_changed: bool,
    prev_cam_frustum_far: f64,
    cam_frustum_far_changed: bool,
    prev_cam_frustum_focal_length: f64,
    cam_frustum_focal_length_changed: bool,
    prev_cam_frustum_near: f64,
    cam_frustum_near_changed: bool,
    prev_cam_frustum_focus_distance: f64,
    cam_frustum_focus_distance_changed: bool,
    prev_cam_h_offset: f64,
    cam_h_offset_changed: bool,
    prev_cam_v_offset: f64,
    cam_v_offset_changed: bool,
    prev_cam_fov: f64,
    cam_fov_changed: bool,
    prev_cam_size: f64,
    cam_size_changed: bool,
    prev_cam_frustum_offset: Vector2,
    cam_frustum_offset_changed: bool,
    prev_cam_near: f64,
    cam_near_changed: bool,
    prev_cam_far: f64,
    cam_far_changed: bool,
}
#[godot_api]
impl PhantomCameraHost {
    #[signal]
    fn viewfinder_update(check_framed_view: bool);
    #[signal]
    fn viewfinder_disable_dead_zone();
    #[signal]
    fn has_error();
    #[signal]
    fn pcam_became_active(pcam: Gd<Node>);
    #[signal]
    fn pcam_became_inactive(pcam: Gd<Node>);
    fn enter_tree_impl(&mut self) {
        let Some(parent) = self.base().get_parent() else {
            return;
        };
        let is_camera_2d = parent.clone().try_cast::<Camera2D>().is_ok();
        let is_camera_3d = parent.clone().try_cast::<Camera3D>().is_ok();
        if !is_camera_2d && !is_camera_3d {
            return;
        }
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_host_added(&this);
        self.is_child_of_camera = true;
        if let Ok(camera) = parent.clone().try_cast::<Camera2D>() {
            self.is_2d = true;
            let mut camera = camera;
            camera.set_position_smoothing_enabled(false);
            self.camera_2d = Some(camera);
        } else if let Ok(camera) = parent.try_cast::<Camera3D>() {
            self.is_2d = false;
            self.camera_3d = Some(camera);
        }
        if !self.base().is_node_ready() {
            return;
        }
        if self.is_2d {
            for pcam in PhantomCameraManager::pcam_2ds().iter_shared() {
                self.pcam_added_to_scene(&pcam);
            }
        } else {
            for pcam in PhantomCameraManager::pcam_3ds().iter_shared() {
                self.pcam_added_to_scene(&pcam);
            }
        }
    }
    fn exit_tree_impl(&mut self) {
        let this = self.base().clone().upcast::<Node>();
        PhantomCameraManager::notify_host_removed(&this);
    }
    fn ready_impl(&mut self) {
        self.pending_ready = true;
    }
    fn complete_ready(&mut self) {
        self.base_mut().set_process_priority(300);
        self.base_mut().set_physics_process_priority(300);
        let Some(manager) = PhantomCameraManager::active_or_editor() else {
            godot_warn!("PhantomCameraHost: Could not find the Phantom Camera Manager singleton");
            return;
        };
        let mut manager = manager;
        let bindings: [(&str, &str); 8] = [
            ("pcam_host_layer_changed", "host_pcam_layer_changed"),
            ("pcam_added_to_scene", "host_pcam_added_to_scene"),
            ("pcam_removed_from_scene", "host_pcam_removed_from_scene"),
            ("pcam_priority_changed", "pcam_priority_updated"),
            ("pcam_priority_override", "host_pcam_priority_override"),
            ("pcam_visibility_changed", "host_pcam_visibility_changed"),
            ("pcam_teleport", "host_pcam_teleported"),
            ("limit_2d_changed", "host_update_limit_2d"),
        ];
        for (signal, method) in bindings {
            let callable = self.base().callable(method);
            if !manager.is_connected(signal, &callable) {
                manager.connect(signal, &callable);
            }
        }
        let callable = self.base().callable("host_draw_limit_2d");
        if !manager.is_connected("draw_limit_2d", &callable) {
            manager.connect("draw_limit_2d", &callable);
        }
        self.find_pcam_with_highest_priority();
        if self.is_2d {
            if let Some(mut camera) = self.camera_2d.clone() {
                camera.set_offset(Vector2::ZERO);
            }
            let Some(pcam) = self.active_pcam_2d.clone() else {
                return;
            };
            self.active_pcam_2d_glob_transform = pcam.bind().get_transform_output();
        } else {
            let Some(pcam) = self.active_pcam_3d.clone() else {
                return;
            };
            self.active_pcam_3d_glob_transform = pcam.bind().get_transform_output();
        }
    }
    fn process_impl(&mut self, delta: f64) {
        if self.pending_ready {
            self.pending_ready = false;
            self.complete_ready();
            return;
        }
        if self.active_pcam_missing {
            return;
        }
        if !self.follow_target_physics_based {
            self.tween_follow_checker(delta);
        }
    }
    fn physics_process_impl(&mut self, delta: f64) {
        if self.pending_ready {
            self.pending_ready = false;
            self.complete_ready();
            return;
        }
        if self.active_pcam_missing || !self.follow_target_physics_based {
            return;
        }
        self.tween_follow_checker(delta);
    }
    #[func]
    fn host_pcam_layer_changed(&mut self, pcam: Gd<Node>) {
        self.base_mut()
            .call_deferred("host_pcam_layer_changed_deferred", &[pcam.to_variant()]);
    }
    #[func]
    fn host_pcam_layer_changed_deferred(&mut self, pcam: Gd<Node>) {
        if self.pcam_is_in_host_layer(&pcam) {
            self.check_pcam_priority(&pcam);
            return;
        }
        if self.is_2d {
            let is_active = match (&self.active_pcam_2d, AnyPcam::from_node(&pcam)) {
                (Some(active), Some(AnyPcam::TwoD(candidate))) => {
                    active.instance_id() == candidate.instance_id()
                }
                _ => false,
            };
            if is_active {
                self.active_pcam_missing = true;
                self.active_pcam_2d = None;
                self.active_pcam_priority = -1;
                if let Some(candidate) = AnyPcam::from_node(&pcam) {
                    candidate.set_active(false);
                }
            }
        } else {
            let is_active = match (&self.active_pcam_3d, AnyPcam::from_node(&pcam)) {
                (Some(active), Some(AnyPcam::ThreeD(candidate))) => {
                    active.instance_id() == candidate.instance_id()
                }
                _ => false,
            };
            if is_active {
                self.active_pcam_missing = true;
                self.active_pcam_3d = None;
                self.active_pcam_priority = -1;
                if let Some(candidate) = AnyPcam::from_node(&pcam) {
                    candidate.set_active(false);
                }
            }
        }
        self.find_pcam_with_highest_priority();
    }
    fn pcam_is_in_host_layer(&self, pcam: &Gd<Node>) -> bool {
        match AnyPcam::from_node(pcam) {
            Some(candidate) => candidate.in_layer(self.host_layers),
            None => false,
        }
    }
    fn find_pcam_with_highest_priority(&mut self) {
        let pcam_list = if self.is_2d {
            PhantomCameraManager::pcam_2ds()
        } else {
            PhantomCameraManager::pcam_3ds()
        };
        let mut highest: Option<Gd<Node>> = None;
        let mut highest_priority = -1i64;
        for pcam in pcam_list.iter_shared() {
            let Some(candidate) = AnyPcam::from_node(&pcam) else {
                continue;
            };
            if !candidate.in_layer(self.host_layers) {
                continue;
            }
            if !candidate.visible() {
                continue;
            }
            if candidate.priority() >= highest_priority {
                highest_priority = candidate.priority();
                highest = Some(pcam.clone());
            }
        }
        let Some(highest) = highest else {
            return;
        };
        self.check_pcam_priority(&highest);
        for pcam in pcam_list.iter_shared() {
            if let Some(candidate) = AnyPcam::from_node(&pcam) {
                candidate.set_tween_skip(false);
            }
        }
    }
    fn check_pcam_priority(&mut self, pcam: &Gd<Node>) {
        let Some(candidate) = AnyPcam::from_node(pcam) else {
            return;
        };
        if !candidate.in_layer(self.host_layers) {
            return;
        }
        if !candidate.visible() {
            return;
        }
        if candidate.priority() >= self.active_pcam_priority {
            self.assign_new_active_pcam(pcam);
            self.active_pcam_missing = false;
            if self.is_2d {
                if let Some(mut camera) = self.camera_2d.clone() {
                    camera.set_as_top_level(true);
                }
            } else if let Some(mut camera) = self.camera_3d.clone() {
                camera.set_as_top_level(true);
            }
        }
        candidate.set_tween_skip(false);
    }
    fn assign_new_active_pcam(&mut self, pcam: &Gd<Node>) {
        if !self.base().is_inside_tree() {
            return;
        }
        let new_pcam = AnyPcam::from_node(pcam);
        let mut no_previous_pcam = false;
        let previous_exists = self.active_pcam_2d.is_some() || self.active_pcam_3d.is_some();
        if previous_exists {
            if self.debug_feature() {
                self.signals().viewfinder_disable_dead_zone().emit();
            }
            if self.is_2d {
                if let Some(mut active) = self.active_pcam_2d.clone() {
                    if let Some(camera) = self.camera_2d.clone() {
                        self.prev_active_pcam_2d_transform = camera.get_global_transform();
                    }
                    active.queue_redraw();
                    active.bind_mut().host_set_active(false);
                    active.bind_mut().emit_became_inactive();
                    self.signals()
                        .pcam_became_inactive()
                        .emit(&active.clone().upcast::<Node>());
                    if self.trigger_pcam_tween {
                        if let Some(AnyPcam::TwoD(new_pcam_2d)) = &new_pcam {
                            active.bind_mut().emit_tween_interrupted(new_pcam_2d);
                        }
                    }
                }
            } else {
                if let Some(active) = self.active_pcam_3d.clone() {
                    if let Some(camera) = self.camera_3d.clone() {
                        self.prev_active_pcam_3d_transform = camera.get_global_transform();
                    }
                    active.clone().bind_mut().host_set_active(false);
                    active.clone().bind_mut().emit_became_inactive();
                    self.signals()
                        .pcam_became_inactive()
                        .emit(&active.clone().upcast::<Node>());
                    if self.trigger_pcam_tween {
                        if let Some(AnyPcam::ThreeD(new_pcam_3d)) = &new_pcam {
                            active
                                .clone()
                                .bind_mut()
                                .emit_tween_interrupted(new_pcam_3d);
                        }
                    }
                    self.snapshot_camera_3d_properties();
                }
            }
        } else {
            no_previous_pcam = true;
        }
        if self.is_2d {
            if let Some(AnyPcam::TwoD(new_pcam_2d)) = &new_pcam {
                let current_tween = self
                    .active_pcam_2d
                    .as_ref()
                    .map(|active| active.clone().upcast::<Node>());
                self.tween_value_checker(
                    current_tween.as_ref(),
                    &new_pcam_2d.clone().upcast::<Node>(),
                );
                self.active_pcam_2d = Some(new_pcam_2d.clone());
                self.active_pcam_priority = new_pcam_2d.bind().current_priority();
                self.active_pcam_has_damping = new_pcam_2d.bind().has_follow_damping();
            }
        } else if let Some(AnyPcam::ThreeD(new_pcam_3d)) = &new_pcam {
            let current_tween = self
                .active_pcam_3d
                .as_ref()
                .map(|active| active.clone().upcast::<Node>());
            self.tween_value_checker(
                current_tween.as_ref(),
                &new_pcam_3d.clone().upcast::<Node>(),
            );
            self.active_pcam_3d = Some(new_pcam_3d.clone());
            self.active_pcam_priority = new_pcam_3d.bind().current_priority();
            self.active_pcam_has_damping = new_pcam_3d.bind().has_follow_damping();
            self.assign_default_spring_arm_shape(new_pcam_3d);
            self.diff_camera_3d_properties(new_pcam_3d);
        }
        if self.debug_feature() {
            self.signals().viewfinder_update().emit(false);
        }
        if self.is_2d {
            if let Some(mut active) = self.active_pcam_2d.clone() {
                active.bind_mut().host_set_active(true);
                active.bind_mut().emit_became_active();
                self.signals()
                    .pcam_became_active()
                    .emit(&active.clone().upcast::<Node>());
                if let Some(camera) = self.camera_2d.clone() {
                    self.camera_zoom = camera.get_zoom();
                }
            }
        } else if let Some(mut active) = self.active_pcam_3d.clone() {
            active.bind_mut().host_set_active(true);
            active.bind_mut().emit_became_active();
            self.signals()
                .pcam_became_active()
                .emit(&active.clone().upcast::<Node>());
            if let (Some(mut camera_object), Some(resource)) = (
                self.camera_3d
                    .clone()
                    .map(|camera| camera.upcast::<godot::classes::Object>()),
                active.bind().camera_resource(),
            ) {
                let resource = resource.bind();
                let keep_aspect = resource.read("keep_aspect").try_to::<i64>().unwrap_or(1);
                let cull_mask = resource.read("cull_mask").try_to::<i64>().unwrap_or(0);
                let projection = resource.read("projection").try_to::<i64>().unwrap_or(0);
                pset_i64(&mut camera_object, "keep_aspect", keep_aspect);
                pset_i64(&mut camera_object, "cull_mask", cull_mask);
                pset_i64(&mut camera_object, "projection", projection);
            }
        }
        self.check_viewfinder_in_play();
        if no_previous_pcam {
            if self.is_2d {
                if let Some(active) = self.active_pcam_2d.clone() {
                    self.prev_active_pcam_2d_transform = active.bind().get_transform_output();
                }
            } else if let Some(active) = self.active_pcam_3d.clone() {
                self.prev_active_pcam_3d_transform = active.bind().get_transform_output();
            }
        }
        let skip = new_pcam
            .as_ref()
            .map(AnyPcam::get_tween_skip)
            .unwrap_or(false);
        if skip || self.tween_duration == 0.0 {
            self.tween_elapsed_time = self.tween_duration;
            self.tween_is_instant = true;
        } else {
            self.tween_elapsed_time = 0.0;
        }
        self.check_pcam_physics();
        self.trigger_pcam_tween = true;
    }
    fn diff_camera_3d_properties(&mut self, active: &Gd<PhantomCamera3D>) {
        let resource = active.bind().camera_resource();
        let attributes = active.bind().attributes_resource();
        if resource.is_some() {
            let pcam = active.clone();
            let pcam = pcam.bind();
            let h_offset = variant_f64(&pcam.get_h_offset(), 0.0);
            let v_offset = variant_f64(&pcam.get_v_offset(), 0.0);
            let fov = variant_f64(&pcam.get_fov(), 0.0);
            let size = variant_f64(&pcam.get_size(), 0.0);
            let frustum_offset = pcam
                .get_frustum_offset()
                .try_to::<Vector2>()
                .unwrap_or_default();
            let near = variant_f64(&pcam.get_near(), 0.0);
            let far = variant_f64(&pcam.get_far(), 0.0);
            drop(pcam);
            if self.prev_cam_h_offset != h_offset {
                self.cam_h_offset_changed = true;
            }
            if self.prev_cam_v_offset != v_offset {
                self.cam_v_offset_changed = true;
            }
            if self.prev_cam_fov != fov {
                self.cam_fov_changed = true;
            }
            if self.prev_cam_size != size {
                self.cam_size_changed = true;
            }
            if self.prev_cam_frustum_offset != frustum_offset {
                self.cam_frustum_offset_changed = true;
            }
            if self.prev_cam_near != near {
                self.cam_near_changed = true;
            }
            if self.prev_cam_far != far {
                self.cam_far_changed = true;
            }
        } else {
            self.cam_h_offset_changed = false;
            self.cam_v_offset_changed = false;
            self.cam_fov_changed = false;
            self.cam_size_changed = false;
            self.cam_frustum_offset_changed = false;
            self.cam_near_changed = false;
            self.cam_far_changed = false;
            self.cam_attribute_changed = false;
        }
        let Some(attributes) = attributes else {
            self.cam_attribute_changed = false;
            return;
        };
        let same_attributes = match &self.prev_cam_attributes {
            Some(previous) => previous.instance_id() == attributes.instance_id(),
            None => false,
        };
        if same_attributes {
            return;
        }
        self.prev_cam_attributes = Some(attributes.clone());
        self.cam_attribute_changed = true;
        let object = attributes.clone().upcast::<godot::classes::Object>();
        if pget_f64(&object, "auto_exposure_scale") != self.prev_cam_auto_exposure_scale {
            self.cam_auto_exposure_scale_changed = true;
        }
        if pget_f64(&object, "auto_exposure_speed") != self.prev_cam_auto_exposure_speed {
            self.cam_auto_exposure_speed_changed = true;
        }
        if pget_f64(&object, "exposure_multiplier") != self.prev_cam_exposure_multiplier {
            self.cam_exposure_multiplier_changed = true;
        }
        if pget_f64(&object, "exposure_sensitivity") != self.prev_cam_exposure_sensitivity {
            self.cam_exposure_sensitivity_changed = true;
        }
        if let Ok(practical) = attributes.clone().try_cast::<CameraAttributesPractical>() {
            self.cam_attribute_type = 0;
            self.ensure_camera_attributes_type(0, &attributes);
            let practical_object = practical.clone().upcast::<godot::classes::Object>();
            if pget_f64(&practical_object, "auto_exposure_min_sensitivity")
                != self.prev_cam_exposure_min_sensitivity
            {
                self.cam_exposure_min_sensitivity_changed = true;
            }
            if pget_f64(&practical_object, "auto_exposure_max_sensitivity")
                != self.prev_cam_exposure_max_sensitivity
            {
                self.cam_exposure_max_sensitivity_changed = true;
            }
            if pget_f64(&practical_object, "dof_blur_amount") != self.prev_cam_dof_blur_amount {
                self.cam_dof_blur_amount_changed = true;
            }
            if pget_f64(&practical_object, "dof_blur_far_distance")
                != self.prev_cam_dof_blur_far_distance
            {
                self.cam_dof_blur_far_distance_changed = true;
                self.set_camera_3d_attribute_bool("dof_blur_far_enabled", true);
            }
            if pget_f64(&practical_object, "dof_blur_far_transition")
                != self.prev_cam_dof_blur_far_transition
            {
                self.cam_dof_blur_far_transition_changed = true;
                self.set_camera_3d_attribute_bool("dof_blur_far_enabled", true);
            }
            if pget_f64(&practical_object, "dof_blur_near_distance")
                != self.prev_cam_dof_blur_near_distance
            {
                self.cam_dof_blur_near_distance_changed = true;
                self.set_camera_3d_attribute_bool("dof_blur_near_enabled", true);
            }
            if pget_f64(&practical_object, "dof_blur_near_transition")
                != self.prev_cam_dof_blur_near_transition
            {
                self.cam_dof_blur_near_transition_changed = true;
                self.set_camera_3d_attribute_bool("dof_blur_near_enabled", true);
            }
        } else if let Ok(physical) = attributes.clone().try_cast::<CameraAttributesPhysical>() {
            self.cam_attribute_type = 1;
            self.ensure_camera_attributes_type(1, &attributes);
            let physical_object = physical.clone().upcast::<godot::classes::Object>();
            if pget_f64(&physical_object, "auto_exposure_min_exposure_value")
                != self.prev_cam_exposure_min_exposure_value
            {
                self.cam_exposure_min_exposure_value_changed = true;
            }
            if pget_f64(&physical_object, "auto_exposure_max_exposure_value")
                != self.prev_cam_exposure_max_exposure_value
            {
                self.cam_exposure_max_exposure_value_changed = true;
            }
            if pget_f64(&physical_object, "exposure_aperture") != self.prev_cam_exposure_aperture {
                self.cam_exposure_aperture_changed = true;
            }
            if pget_f64(&physical_object, "exposure_shutter_speed")
                != self.prev_cam_exposure_shutter_speed
            {
                self.cam_exposure_shutter_speed_changed = true;
            }
            if pget_f64(&physical_object, "frustum_far") != self.prev_cam_frustum_far {
                self.cam_frustum_far_changed = true;
            }
            if pget_f64(&physical_object, "frustum_focal_length")
                != self.prev_cam_frustum_focal_length
            {
                self.cam_frustum_focal_length_changed = true;
            }
            if pget_f64(&physical_object, "frustum_focus_distance")
                != self.prev_cam_frustum_focus_distance
            {
                self.cam_frustum_focus_distance_changed = true;
            }
            if pget_f64(&physical_object, "frustum_near") != self.prev_cam_frustum_near {
                self.cam_frustum_near_changed = true;
            }
        }
    }
    fn ensure_camera_attributes_type(
        &mut self,
        attribute_type: i64,
        source: &Gd<CameraAttributes>,
    ) {
        let Some(mut camera) = self.camera_3d.clone() else {
            return;
        };
        if camera
            .get("attributes")
            .try_to::<Gd<CameraAttributes>>()
            .is_err()
        {
            let class = if attribute_type == 0 {
                "CameraAttributesPractical"
            } else {
                "CameraAttributesPhysical"
            };
            let instance = ClassDb::singleton().instantiate(&StringName::from(class));
            camera.set("attributes", &instance);
        }
        let mut source_object = source.clone();
        if let Ok(duplicate) = source_object
            .call("duplicate", &[])
            .try_to::<Gd<CameraAttributes>>()
        {
            camera.set("attributes", &duplicate.to_variant());
        }
        self.cam_attribute_assigned = true;
    }
    fn set_camera_3d_attribute_bool(&mut self, property: &str, value: bool) {
        let Some(camera) = self.camera_3d.clone() else {
            return;
        };
        if let Ok(attributes) = camera
            .get("attributes")
            .try_to::<Gd<godot::classes::Object>>()
        {
            let mut attributes = attributes;
            pset_bool(&mut attributes, property, value);
        }
    }
    fn snapshot_camera_3d_properties(&mut self) {
        let Some(camera) = self.camera_3d.clone() else {
            return;
        };
        let attributes = camera
            .get("attributes")
            .try_to::<Gd<CameraAttributes>>()
            .ok();
        if let Some(attributes) = attributes {
            let object = attributes.clone().upcast::<godot::classes::Object>();
            self.prev_cam_exposure_multiplier = pget_f64(&object, "exposure_multiplier");
            self.prev_cam_auto_exposure_scale = pget_f64(&object, "auto_exposure_scale");
            self.prev_cam_auto_exposure_speed = pget_f64(&object, "auto_exposure_speed");
            if let Ok(practical) = attributes.clone().try_cast::<CameraAttributesPractical>() {
                let practical_object = practical.upcast::<godot::classes::Object>();
                self.prev_cam_dof_blur_amount = pget_f64(&practical_object, "dof_blur_amount");
                if pget_bool(&practical_object, "dof_blur_far_enabled") {
                    self.prev_cam_dof_blur_far_distance =
                        pget_f64(&practical_object, "dof_blur_far_distance");
                    self.prev_cam_dof_blur_far_transition =
                        pget_f64(&practical_object, "dof_blur_far_transition");
                } else {
                    self.prev_cam_dof_blur_far_distance = self.cam_dof_blur_far_distance_default;
                    self.prev_cam_dof_blur_far_transition =
                        self.cam_dof_blur_far_transition_default;
                }
                if pget_bool(&practical_object, "dof_blur_near_enabled") {
                    self.prev_cam_dof_blur_near_distance =
                        pget_f64(&practical_object, "dof_blur_near_distance");
                    self.prev_cam_dof_blur_near_transition =
                        pget_f64(&practical_object, "dof_blur_near_transition");
                } else {
                    self.prev_cam_dof_blur_near_distance = self.cam_dof_blur_near_distance_default;
                    self.prev_cam_dof_blur_near_transition =
                        self.cam_dof_blur_near_transition_default;
                }
                if pget_bool(&practical_object, "auto_exposure_enabled") {
                    self.prev_cam_exposure_max_sensitivity =
                        pget_f64(&practical_object, "auto_exposure_max_sensitivity");
                    self.prev_cam_exposure_min_sensitivity =
                        pget_f64(&practical_object, "auto_exposure_min_sensitivity");
                }
            } else if let Ok(physical) = attributes.clone().try_cast::<CameraAttributesPhysical>() {
                let physical_object = physical.upcast::<godot::classes::Object>();
                self.prev_cam_frustum_focus_distance =
                    pget_f64(&physical_object, "frustum_focus_distance");
                self.prev_cam_frustum_focal_length =
                    pget_f64(&physical_object, "frustum_focal_length");
                self.prev_cam_frustum_far = pget_f64(&physical_object, "frustum_far");
                self.prev_cam_frustum_near = pget_f64(&physical_object, "frustum_near");
                self.prev_cam_exposure_aperture = pget_f64(&physical_object, "exposure_aperture");
                self.prev_cam_exposure_shutter_speed =
                    pget_f64(&physical_object, "exposure_shutter_speed");
                if pget_bool(&physical_object, "auto_exposure_enabled") {
                    self.prev_cam_exposure_min_exposure_value =
                        pget_f64(&physical_object, "auto_exposure_min_exposure_value");
                    self.prev_cam_exposure_max_exposure_value =
                        pget_f64(&physical_object, "auto_exposure_max_exposure_value");
                }
            }
        }
        let object = camera.upcast::<godot::classes::Object>();
        self.prev_cam_h_offset = pget_f64(&object, "h_offset");
        self.prev_cam_v_offset = pget_f64(&object, "v_offset");
        self.prev_cam_fov = pget_f64(&object, "fov");
        self.prev_cam_size = pget_f64(&object, "size");
        self.prev_cam_frustum_offset = pget_v2(&object, "frustum_offset");
        self.prev_cam_near = pget_f64(&object, "near");
        self.prev_cam_far = pget_f64(&object, "far");
    }
    fn assign_default_spring_arm_shape(&mut self, active: &Gd<PhantomCamera3D>) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        if active.bind().get_follow_mode() != 6 {
            return;
        }
        if active.bind().get_shape().is_some() {
            return;
        }
        let Some(camera) = self.camera_3d.clone() else {
            return;
        };
        let rid = camera.get_pyramid_shape_rid();
        let Some(server) = Engine::singleton().get_singleton("PhysicsServer3D") else {
            return;
        };
        let mut server = server;
        let data = server.call("shape_get_data", &[rid.to_variant()]);
        let Ok(points) = data.try_to::<godot::builtin::PackedVector3Array>() else {
            return;
        };
        let mut expanded = godot::builtin::PackedVector3Array::new();
        for point in points.as_slice().iter() {
            expanded.push(Vector3::new(
                point.x + if point.x >= 0.0 { 0.0205 } else { -0.0205 },
                point.y + if point.y >= 0.0 { 0.0205 } else { -0.0205 },
                point.z + if point.z >= 0.0 { 0.0205 } else { -0.0205 },
            ));
        }
        let instance = ClassDb::singleton().instantiate(&StringName::from("ConvexPolygonShape3D"));
        if let Ok(mut shape) = instance.try_to::<Gd<godot::classes::ConvexPolygonShape3D>>() {
            shape.set("points", &expanded.to_variant());
            let shape = shape.upcast::<godot::classes::Shape3D>();
            {
                active.clone().bind_mut().set_shape(Some(shape));
            }
        }
    }
    fn tween_value_checker(&mut self, current_pcam: Option<&Gd<Node>>, new_pcam: &Gd<Node>) {
        if let Some(candidate) = AnyPcam::from_node(new_pcam) {
            self.tween_duration = candidate.tween_duration();
            self.tween_transition = candidate.tween_transition();
            self.tween_ease = candidate.tween_ease();
        }
        let Some(current_pcam) = current_pcam else {
            return;
        };
        let directors = PhantomCameraManager::tween_directors();
        if directors.is_empty() {
            return;
        }
        for director in directors.iter_shared() {
            let Ok(director) = director.try_cast::<PhantomCameraTweenDirector>() else {
                continue;
            };
            let resources = director.bind().director_resources();
            for resource in resources.iter_shared() {
                self.tween_director_resource_check(&director, &resource, current_pcam, new_pcam);
            }
        }
    }
    fn tween_director_resource_check(
        &mut self,
        director: &Gd<PhantomCameraTweenDirector>,
        resource: &Gd<TweenDirectorResource>,
        current_pcam: &Gd<Node>,
        new_pcam: &Gd<Node>,
    ) {
        let resource_bind = resource.bind();
        let Some(tween) = resource_bind.tween_resource.clone() else {
            return;
        };
        let from_type = resource_bind.from_type;
        let to_type = resource_bind.to_type;
        let from_paths = resource_bind.from_phantom_cameras.clone();
        let from_tweens = resource_bind.from_tween_resources.clone();
        let to_paths = resource_bind.to_phantom_cameras.clone();
        let to_tweens = resource_bind.to_tween_resources.clone();
        drop(resource_bind);
        match from_type {
            0 => {
                let mut has_valid_pcam = false;
                for path in from_paths.iter_shared() {
                    let from_pcam = director.clone().upcast::<Node>().get_node_or_null(&path);
                    let same = match (&from_pcam, current_pcam) {
                        (Some(from), current) => from.instance_id() == current.instance_id(),
                        (None, _) => false,
                    };
                    if !same {
                        has_valid_pcam = false;
                        continue;
                    }
                    has_valid_pcam = true;
                    break;
                }
                if !has_valid_pcam {
                    return;
                }
            }
            1 => {
                let mut has_valid_resource = false;
                let current_tween =
                    AnyPcam::from_node(current_pcam).and_then(|p| p.tween_resource());
                for candidate in from_tweens.iter_shared() {
                    let same = match (&current_tween, &candidate) {
                        (Some(current), candidate) => {
                            current.instance_id() == candidate.instance_id()
                        }
                        (None, _) => false,
                    };
                    if same {
                        has_valid_resource = true;
                        break;
                    }
                    has_valid_resource = false;
                }
                if !has_valid_resource {
                    return;
                }
            }
            _ => {}
        }
        match to_type {
            0 => {
                let mut has_valid_pcam = false;
                for path in to_paths.iter_shared() {
                    let to_pcam = director.clone().upcast::<Node>().get_node_or_null(&path);
                    let is_current = match (&to_pcam, current_pcam) {
                        (Some(to), current) => to.instance_id() == current.instance_id(),
                        (None, _) => false,
                    };
                    if is_current {
                        has_valid_pcam = false;
                        continue;
                    }
                    let is_new = match (&to_pcam, new_pcam) {
                        (Some(to), new) => to.instance_id() == new.instance_id(),
                        (None, _) => false,
                    };
                    if is_new {
                        has_valid_pcam = true;
                        break;
                    }
                    has_valid_pcam = false;
                }
                if !has_valid_pcam {
                    return;
                }
            }
            1 => {
                let mut has_valid_resource = false;
                let new_tween = AnyPcam::from_node(new_pcam).and_then(|p| p.tween_resource());
                for candidate in to_tweens.iter_shared() {
                    let same = match (&new_tween, &candidate) {
                        (Some(new), candidate) => new.instance_id() == candidate.instance_id(),
                        (None, _) => false,
                    };
                    if same {
                        has_valid_resource = true;
                        break;
                    }
                    has_valid_resource = false;
                }
                if !has_valid_resource {
                    return;
                }
            }
            _ => {}
        }
        let tween = tween.bind();
        self.tween_duration = tween.duration;
        self.tween_transition = tween.transition;
        self.tween_ease = tween.ease;
    }
    fn check_pcam_physics(&mut self) {
        if self.interpolation_mode == INTERPOLATION_MANUAL {
            return;
        }
        if self.is_2d {
            let physics_based = self
                .active_pcam_2d
                .as_ref()
                .map(|pcam| pcam.bind().get_follow_target_physics_based())
                .unwrap_or(false);
            let Some(mut camera) = self.camera_2d.clone() else {
                return;
            };
            if physics_based && self.interpolation_mode != INTERPOLATION_IDLE {
                self.follow_target_physics_based = true;
                camera.call("reset_physics_interpolation", &[]);
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_ON.to_variant(),
                );
                let interpolated = self.tree_physics_interpolation();
                camera.set(
                    "process_callback",
                    &(if interpolated {
                        CAMERA2D_PROCESS_PHYSICS
                    } else {
                        CAMERA2D_PROCESS_IDLE
                    })
                    .to_variant(),
                );
            } else {
                self.follow_target_physics_based = false;
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_INHERIT.to_variant(),
                );
                let interpolated = self.tree_physics_interpolation();
                camera.set(
                    "process_callback",
                    &(if interpolated {
                        CAMERA2D_PROCESS_PHYSICS
                    } else {
                        CAMERA2D_PROCESS_IDLE
                    })
                    .to_variant(),
                );
            }
        } else {
            let physics_based = self
                .active_pcam_3d
                .as_ref()
                .map(|pcam| pcam.bind().get_follow_target_physics_based())
                .unwrap_or(false);
            let Some(mut camera) = self.camera_3d.clone() else {
                return;
            };
            if (self.tree_physics_interpolation() || physics_based)
                && self.interpolation_mode != INTERPOLATION_IDLE
            {
                self.follow_target_physics_based = true;
                camera.call("reset_physics_interpolation", &[]);
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_ON.to_variant(),
                );
            } else {
                self.follow_target_physics_based = false;
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_INHERIT.to_variant(),
                );
            }
        }
    }
    fn tree_physics_interpolation(&self) -> bool {
        self.base()
            .get_tree()
            .get("physics_interpolation")
            .try_to::<bool>()
            .unwrap_or(false)
    }
    fn debug_feature(&self) -> bool {
        Os::singleton().has_feature("debug")
    }
    fn tween_follow_checker(&mut self, delta: f64) {
        if self.is_2d {
            let Some(active) = self.active_pcam_2d.clone() else {
                self.active_pcam_missing = true;
                return;
            };
            active.clone().bind_mut().process_logic(delta);
            self.active_pcam_2d_glob_transform = active.bind().get_transform_output();
            if self.reset_noise_offset_2d {
                if let Some(mut camera) = self.camera_2d.clone() {
                    camera.set_offset(Vector2::ZERO);
                }
                self.reset_noise_offset_2d = false;
            }
        } else {
            let Some(active) = self.active_pcam_3d.clone() else {
                self.active_pcam_missing = true;
                return;
            };
            active.clone().bind_mut().process_logic(delta);
            self.active_pcam_3d_glob_transform = active.clone().bind().get_transform_output();
        }
        if !self.trigger_pcam_tween {
            if self.tween_is_instant {
                self.check_pcam_physics();
                self.tween_is_instant = false;
            }
            self.pcam_follow();
        } else {
            self.pcam_tween(delta);
        }
        if self.is_2d {
            let Some(active) = self.active_pcam_2d.clone() else {
                return;
            };
            let has_noise = active.bind().has_noise_resource();
            let mut active_mut = active.clone();
            let emitted = active_mut.bind_mut().take_noise_output();
            if let Some(output) = emitted {
                self.noise_emitted_output_2d = output;
                self.has_noise_emitted = true;
            }
            if !self.has_noise_emitted && !has_noise {
                return;
            }
            let Some(mut camera) = self.camera_2d.clone() else {
                return;
            };
            let noise_origin =
                active.bind().get_noise_transform().origin + self.noise_emitted_output_2d.origin;
            let mut offset = camera.get_offset();
            offset += noise_origin;
            camera.set_offset(offset);
            let noise_rotation = active.bind().get_noise_transform().rotation()
                + self.noise_emitted_output_2d.rotation();
            if camera
                .get("ignore_rotation")
                .try_to::<bool>()
                .unwrap_or(false)
                && self.noise_emitted_output_2d.rotation() != 0.0
            {
                godot_warn!(
                    "PhantomCameraHost: the Camera2D has ignore_rotation enabled; rotational noise is skipped"
                );
            } else {
                let rotation = camera.get_rotation() + noise_rotation;
                camera.set_rotation(rotation);
            }
            self.has_noise_emitted = false;
            self.reset_noise_offset_2d = true;
        } else {
            let Some(active) = self.active_pcam_3d.clone() else {
                return;
            };
            let has_noise = active.bind().pcam_has_noise_resource();
            let emitted = active.clone().bind_mut().take_noise_output();
            if let Some(output) = emitted {
                self.noise_emitted_output_3d = output;
                self.has_noise_emitted = true;
            }
            if !self.has_noise_emitted && !has_noise {
                return;
            }
            let Some(mut camera) = self.camera_3d.clone() else {
                return;
            };
            let noise = active.bind().get_noise_transform() * self.noise_emitted_output_3d;
            let transform = camera.get_global_transform() * noise;
            camera.set_global_transform(transform);
            self.has_noise_emitted = false;
        }
    }
    fn pcam_follow(&mut self) {
        if self.active_pcam_missing || !self.is_child_of_camera {
            return;
        }
        if self.is_2d {
            let Some(active) = self.active_pcam_2d.clone() else {
                return;
            };
            let Some(mut camera) = self.camera_2d.clone() else {
                return;
            };
            if active.bind().get_snap_to_pixel() {
                let mut transform = self.active_pcam_2d_glob_transform;
                transform.origin = transform.origin.round();
                camera.set_global_transform(transform);
            } else {
                camera.set_global_transform(self.active_pcam_2d_glob_transform);
            }
            camera.set_zoom(active.bind().get_zoom());
        } else {
            let Some(active) = self.active_pcam_3d.clone() else {
                return;
            };
            let Some(mut camera) = self.camera_3d.clone() else {
                return;
            };
            if !camera
                .get_global_transform()
                .approx_eq(&self.active_pcam_3d_glob_transform)
            {
                camera.set_global_transform(self.active_pcam_3d_glob_transform);
            }
            let _ = active;
        }
    }
    fn pcam_tween(&mut self, delta: f64) {
        if self.tween_elapsed_time == 0.0 {
            if self.is_2d {
                if let Some(mut active) = self.active_pcam_2d.clone() {
                    active.bind_mut().emit_tween_started();
                    active.call_deferred("reset_limit", &[]);
                }
            } else if let Some(mut active) = self.active_pcam_3d.clone() {
                active.bind_mut().emit_tween_started();
            }
        }
        self.tween_elapsed_time = self.tween_duration.min(self.tween_elapsed_time + delta);
        if self.is_2d {
            self.tween_2d_frame();
        } else {
            self.tween_3d_frame();
        }
        if self.tween_is_instant {
            if let Some(mut camera) = self.camera_2d.clone() {
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_OFF.to_variant(),
                );
                camera.call("reset_physics_interpolation", &[]);
            }
            if let Some(mut camera) = self.camera_3d.clone() {
                camera.set(
                    "physics_interpolation_mode",
                    &PHYSICS_INTERPOLATION_MODE_OFF.to_variant(),
                );
                camera.call("reset_physics_interpolation", &[]);
            }
        }
        if self.tween_elapsed_time < self.tween_duration {
            return;
        }
        self.trigger_pcam_tween = false;
        self.tween_elapsed_time = 0.0;
        self.signals().viewfinder_update().emit(true);
        if self.is_2d {
            if let Some(mut active) = self.active_pcam_2d.clone() {
                active.call_deferred("update_limit_all_sides", &[]);
                active.bind_mut().emit_tween_completed();
                active.bind_mut().host_set_tween_skip(false);
            }
        } else {
            if let Some(active) = self.active_pcam_3d.clone() {
                if active.bind().camera_resource().is_some()
                    && active.bind().attributes_resource().is_some()
                {
                    if self.cam_attribute_type == 0 {
                        if !self.camera_3d_attribute_enabled("dof_blur_far_enabled") {
                            self.set_camera_3d_attribute_bool("dof_blur_far_enabled", false);
                        }
                        if !self.camera_3d_attribute_enabled("dof_blur_near_enabled") {
                            self.set_camera_3d_attribute_bool("dof_blur_near_enabled", false);
                        }
                    }
                }
                self.cam_h_offset_changed = false;
                self.cam_v_offset_changed = false;
                self.cam_fov_changed = false;
                self.cam_size_changed = false;
                self.cam_frustum_offset_changed = false;
                self.cam_near_changed = false;
                self.cam_far_changed = false;
                self.cam_attribute_changed = false;
                active.clone().bind_mut().host_set_tween_skip(false);
                active.clone().bind_mut().emit_tween_completed();
            }
        }
    }
    fn camera_3d_attribute_enabled(&self, property: &str) -> bool {
        let Some(camera) = self.camera_3d.clone() else {
            return false;
        };
        match camera
            .get("attributes")
            .try_to::<Gd<godot::classes::Object>>()
        {
            Ok(attributes) => pget_bool(&attributes, property),
            Err(_) => false,
        }
    }
    fn tween_2d_frame(&mut self) {
        let Some(active) = self.active_pcam_2d.clone() else {
            return;
        };
        let Some(mut camera) = self.camera_2d.clone() else {
            return;
        };
        active.clone().bind_mut().emit_is_tweening();
        let (elapsed, duration, transition, ease) = self.tween_params();
        let destination = tween_interp_v2(
            self.prev_active_pcam_2d_transform.origin,
            self.active_pcam_2d_glob_transform.origin,
            elapsed,
            duration,
            transition,
            ease,
        );
        if active.bind().get_snap_to_pixel() {
            camera.set_global_position(destination.round());
        } else {
            camera.set_global_position(destination);
        }
        let rotation = tween_interp_f64(
            f64_of(self.prev_active_pcam_2d_transform.rotation()),
            f64_of(self.active_pcam_2d_glob_transform.rotation()),
            elapsed,
            duration,
            transition,
            ease,
        );
        camera.set_rotation(rotation as real);
        let zoom = tween_interp_v2(
            self.camera_zoom,
            active.bind().get_zoom(),
            elapsed,
            duration,
            transition,
            ease,
        );
        camera.set_zoom(zoom);
    }
    fn tween_3d_frame(&mut self) {
        let Some(active) = self.active_pcam_3d.clone() else {
            return;
        };
        let Some(mut camera) = self.camera_3d.clone() else {
            return;
        };
        active.clone().bind_mut().emit_is_tweening();
        let (elapsed, duration, transition, ease) = self.tween_params();
        camera.set_global_position(tween_interp_v3(
            self.prev_active_pcam_3d_transform.origin,
            self.active_pcam_3d_glob_transform.origin,
            elapsed,
            duration,
            transition,
            ease,
        ));
        let previous_quat = self
            .prev_active_pcam_3d_transform
            .basis
            .orthonormalized()
            .get_quaternion();
        let quat = tween_interp_quat(
            previous_quat,
            self.active_pcam_3d_glob_transform
                .basis
                .orthonormalized()
                .get_quaternion(),
            elapsed,
            duration,
            transition,
            ease,
        );
        camera.set_quaternion(quat);
        if self.cam_attribute_changed {
            let attributes = self.camera_attribute_object();
            if let Some(mut attributes) = attributes {
                let pcam_attributes = active.bind().attributes_resource();
                if let Some(pcam_attributes) = pcam_attributes {
                    let source = pcam_attributes.upcast::<godot::classes::Object>();
                    if pget_bool(&source, "auto_exposure_enabled") {
                        if self.cam_auto_exposure_scale_changed {
                            let value = tween_interp_f64(
                                self.prev_cam_auto_exposure_scale,
                                pget_f64(&source, "auto_exposure_scale"),
                                elapsed,
                                duration,
                                transition,
                                ease,
                            );
                            pset_f64(&mut attributes, "auto_exposure_scale", value);
                        }
                        if self.cam_auto_exposure_speed_changed {
                            let value = tween_interp_f64(
                                self.prev_cam_auto_exposure_scale,
                                pget_f64(&source, "auto_exposure_scale"),
                                elapsed,
                                duration,
                                transition,
                                ease,
                            );
                            pset_f64(&mut attributes, "auto_exposure_speed", value);
                        }
                    }
                    if self.cam_attribute_type == 0 {
                        self.tween_attributes_practical(
                            &mut attributes,
                            &source,
                            elapsed,
                            duration,
                            transition,
                            ease,
                        );
                    } else if self.cam_attribute_type == 1 {
                        self.tween_attributes_physical(
                            &mut attributes,
                            &source,
                            elapsed,
                            duration,
                            transition,
                            ease,
                        );
                    }
                }
            }
        }
        let mut camera_object = camera.upcast::<godot::classes::Object>();
        if self.cam_h_offset_changed {
            let value = tween_interp_f64(
                self.prev_cam_h_offset,
                pget_f64(&camera_object, "h_offset"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "h_offset", value);
        }
        if self.cam_v_offset_changed {
            let value = tween_interp_f64(
                self.prev_cam_v_offset,
                pget_f64(&camera_object, "v_offset"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "v_offset", value);
        }
        if self.cam_fov_changed {
            let target = match active.bind().get_fov().try_to::<f64>() {
                Ok(value) => value,
                Err(_) => pget_f64(&camera_object, "fov"),
            };
            let value = tween_interp_f64(
                self.prev_cam_fov,
                target,
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "fov", value);
        }
        if self.cam_size_changed {
            let target = match active.bind().get_size().try_to::<f64>() {
                Ok(value) => value,
                Err(_) => pget_f64(&camera_object, "size"),
            };
            let value = tween_interp_f64(
                self.prev_cam_size,
                target,
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "size", value);
        }
        if self.cam_frustum_offset_changed {
            let target = match active.bind().get_frustum_offset().try_to::<Vector2>() {
                Ok(value) => value,
                Err(_) => pget_v2(&camera_object, "frustum_offset"),
            };
            let value = tween_interp_v2(
                self.prev_cam_frustum_offset,
                target,
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_v2(&mut camera_object, "frustum_offset", value);
        }
        if self.cam_near_changed {
            let target = match active.bind().get_near().try_to::<f64>() {
                Ok(value) => value,
                Err(_) => pget_f64(&camera_object, "near"),
            };
            let value = tween_interp_f64(
                self.prev_cam_near,
                target,
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "near", value);
        }
        if self.cam_far_changed {
            let target = match active.bind().get_far().try_to::<f64>() {
                Ok(value) => value,
                Err(_) => pget_f64(&camera_object, "far"),
            };
            let value = tween_interp_f64(
                self.prev_cam_far,
                target,
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(&mut camera_object, "far", value);
        }
    }
    fn tween_attributes_practical(
        &mut self,
        attributes: &mut Gd<godot::classes::Object>,
        source: &Gd<godot::classes::Object>,
        elapsed: f64,
        duration: f64,
        transition: i64,
        ease: i64,
    ) {
        let auto_exposure = pget_bool(source, "auto_exposure_enabled");
        if auto_exposure {
            if self.cam_exposure_min_sensitivity_changed {
                let value = tween_interp_f64(
                    self.prev_cam_exposure_min_sensitivity,
                    pget_f64(source, "auto_exposure_min_sensitivity"),
                    elapsed,
                    duration,
                    transition,
                    ease,
                );
                pset_f64(attributes, "auto_exposure_min_sensitivity", value);
            }
            if self.cam_exposure_max_sensitivity_changed {
                let value = tween_interp_f64(
                    self.prev_cam_exposure_max_sensitivity,
                    pget_f64(source, "auto_exposure_max_sensitivity"),
                    elapsed,
                    duration,
                    transition,
                    ease,
                );
                pset_f64(attributes, "auto_exposure_max_sensitivity", value);
            }
        }
        if self.cam_dof_blur_amount_changed {
            let value = tween_interp_f64(
                self.prev_cam_dof_blur_amount,
                pget_f64(source, "dof_blur_amount"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "dof_blur_amount", value);
        }
        if self.cam_dof_blur_far_distance_changed {
            let value = tween_interp_f64(
                self.prev_cam_dof_blur_far_distance,
                pget_f64(source, "dof_blur_far_distance"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "dof_blur_far_distance", value);
        }
        if self.cam_dof_blur_far_transition_changed {
            let value = tween_interp_f64(
                self.prev_cam_dof_blur_far_transition,
                pget_f64(source, "dof_blur_far_transition"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "dof_blur_far_transition", value);
        }
        if self.cam_dof_blur_near_distance_changed {
            let value = tween_interp_f64(
                self.prev_cam_dof_blur_near_distance,
                pget_f64(source, "dof_blur_near_distance"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "dof_blur_near_distance", value);
        }
        if self.cam_dof_blur_near_transition_changed {
            let value = tween_interp_f64(
                self.prev_cam_dof_blur_near_transition,
                pget_f64(source, "dof_blur_near_transition"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "dof_blur_near_transition", value);
        }
    }
    fn tween_attributes_physical(
        &mut self,
        attributes: &mut Gd<godot::classes::Object>,
        source: &Gd<godot::classes::Object>,
        elapsed: f64,
        duration: f64,
        transition: i64,
        ease: i64,
    ) {
        if self.cam_dof_blur_near_transition_changed {
            let value = tween_interp_f64(
                self.prev_cam_exposure_max_exposure_value,
                pget_f64(source, "auto_exposure_max_exposure_value"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "auto_exposure_max_exposure_value", value);
        }
        if self.cam_exposure_min_exposure_value_changed {
            let value = tween_interp_f64(
                self.prev_cam_exposure_min_exposure_value,
                pget_f64(source, "auto_exposure_min_exposure_value"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "auto_exposure_min_exposure_value", value);
        }
        if self.cam_exposure_aperture_changed {
            let value = tween_interp_f64(
                self.prev_cam_exposure_aperture,
                pget_f64(source, "exposure_aperture"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "exposure_aperture", value);
        }
        if self.cam_exposure_shutter_speed_changed {
            let value = tween_interp_f64(
                self.prev_cam_exposure_shutter_speed,
                pget_f64(source, "exposure_shutter_speed"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "exposure_shutter_speed", value);
        }
        if self.cam_frustum_far_changed {
            let value = tween_interp_f64(
                self.prev_cam_frustum_far,
                pget_f64(source, "frustum_far"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "frustum_far", value);
        }
        if self.cam_frustum_near_changed {
            let value = tween_interp_f64(
                self.prev_cam_frustum_far,
                pget_f64(source, "frustum_near"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "frustum_near", value);
        }
        if self.cam_frustum_focal_length_changed {
            let value = tween_interp_f64(
                self.prev_cam_frustum_focal_length,
                pget_f64(source, "frustum_focal_length"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "frustum_focal_length", value);
        }
        if self.cam_frustum_focus_distance_changed {
            let value = tween_interp_f64(
                self.prev_cam_frustum_focus_distance,
                pget_f64(source, "frustum_focus_distance"),
                elapsed,
                duration,
                transition,
                ease,
            );
            pset_f64(attributes, "frustum_focus_distance", value);
        }
    }
    fn tween_params(&self) -> (f64, f64, i64, i64) {
        (
            self.tween_elapsed_time,
            self.tween_duration,
            self.tween_transition,
            self.tween_ease,
        )
    }
    fn camera_attribute_object(&self) -> Option<Gd<godot::classes::Object>> {
        let camera = self.camera_3d.clone()?;
        camera
            .get("attributes")
            .try_to::<Gd<godot::classes::Object>>()
            .ok()
    }
    fn check_viewfinder_in_play(&mut self) {
    }
    #[func]
    fn host_update_limit_2d(&self, side: i64, limit: i64) {
        let Some(mut camera) = self.camera_2d.clone() else {
            return;
        };
        camera.call(
            "set_limit",
            &[side.to_variant(), (limit as i32).to_variant()],
        );
    }
    #[func]
    fn host_draw_limit_2d(&self, enabled: bool) {
        if let Some(mut camera) = self.camera_2d.clone() {
            camera.set_limit_drawing_enabled(enabled);
        }
    }
    #[func]
    fn host_pcam_added_to_scene(&mut self, pcam: Gd<Node>) {
        self.base_mut()
            .call_deferred("host_pcam_added_check", &[pcam.to_variant()]);
    }
    fn pcam_added_to_scene(&mut self, pcam: &Gd<Node>) {
        self.check_pcam_priority(pcam);
    }
    #[func]
    fn host_pcam_added_check(&mut self, pcam: Gd<Node>) {
        self.check_pcam_priority(&pcam);
    }
    #[func]
    fn host_pcam_removed_from_scene(&mut self, pcam: Gd<Node>) {
        if self.is_2d {
            let is_active = match (&self.active_pcam_2d, &pcam) {
                (Some(active), pcam) => active.instance_id() == pcam.instance_id(),
                _ => false,
            };
            if is_active {
                self.active_pcam_2d = None;
                self.active_pcam_missing = true;
                if let Some(mut camera) = self.camera_2d.clone() {
                    camera.set_as_top_level(false);
                }
                self.active_pcam_priority = -1;
                self.find_pcam_with_highest_priority();
            }
        } else {
            let is_active = match (&self.active_pcam_3d, &pcam) {
                (Some(active), pcam) => active.instance_id() == pcam.instance_id(),
                _ => false,
            };
            if is_active {
                self.active_pcam_3d = None;
                self.active_pcam_missing = true;
                if let Some(mut camera) = self.camera_3d.clone() {
                    camera.set_as_top_level(false);
                }
                self.active_pcam_priority = -1;
                self.find_pcam_with_highest_priority();
            }
        }
    }
    #[func]
    fn host_pcam_visibility_changed(&mut self, pcam: Gd<Node>) {
        self.base_mut().call_deferred(
            "host_pcam_visibility_changed_deferred",
            &[pcam.to_variant()],
        );
    }
    #[func]
    fn host_pcam_visibility_changed_deferred(&mut self, pcam: Gd<Node>) {
        let is_active = self.pcam_is_active_node(&pcam);
        if is_active {
            self.active_pcam_priority = -1;
            self.find_pcam_with_highest_priority();
            return;
        }
        self.check_pcam_priority(&pcam);
    }
    #[func]
    fn host_pcam_teleported(&mut self, pcam: Gd<Node>) {
        self.base_mut()
            .call_deferred("host_pcam_teleported_deferred", &[pcam.to_variant()]);
    }
    #[func]
    fn host_pcam_teleported_deferred(&mut self, pcam: Gd<Node>) {
        if self.is_2d {
            let is_active = match (&self.active_pcam_2d, &pcam) {
                (Some(active), pcam) => active.instance_id() == pcam.instance_id(),
                _ => false,
            };
            if !is_active {
                return;
            }
            let Some(active) = self.active_pcam_2d.clone() else {
                return;
            };
            let Some(mut camera) = self.camera_2d.clone() else {
                return;
            };
            camera.set_global_position(active.bind().get_transform_output().origin);
            camera.call("reset_physics_interpolation", &[]);
        } else {
            let is_active = match (&self.active_pcam_3d, &pcam) {
                (Some(active), pcam) => active.instance_id() == pcam.instance_id(),
                _ => false,
            };
            if !is_active {
                return;
            }
            let Some(active) = self.active_pcam_3d.clone() else {
                return;
            };
            let Some(mut camera) = self.camera_3d.clone() else {
                return;
            };
            camera.set_global_position(active.bind().get_transform_output().origin);
            camera.call("reset_physics_interpolation", &[]);
        }
    }
    fn pcam_is_active_node(&self, pcam: &Gd<Node>) -> bool {
        match (&self.active_pcam_2d, &self.active_pcam_3d, pcam) {
            (Some(active), _, pcam) => {
                active.instance_id() == pcam.instance_id() || self.three_d_is_active(pcam)
            }
            (_, Some(active), pcam) => active.instance_id() == pcam.instance_id(),
            _ => false,
        }
    }
    fn three_d_is_active(&self, pcam: &Gd<Node>) -> bool {
        match &self.active_pcam_3d {
            Some(active) => active.instance_id() == pcam.instance_id(),
            None => false,
        }
    }
    #[func]
    fn pcam_priority_updated(&mut self, pcam: Gd<Node>) {
        self.base_mut()
            .call_deferred("pcam_priority_updated_deferred", &[pcam.to_variant()]);
    }
    #[func]
    fn pcam_priority_updated_deferred(&mut self, pcam: Gd<Node>) {
        if !pcam.is_instance_valid() {
            return;
        }
        if !self.pcam_is_in_host_layer(&pcam) {
            return;
        }
        if Engine::singleton().is_editor_hint() {
            let override_active = if self.is_2d {
                match &self.active_pcam_2d {
                    Some(active) => active.bind().get_priority_override(),
                    None => return,
                }
            } else {
                match &self.active_pcam_3d {
                    Some(active) => active.bind().get_priority_override(),
                    None => return,
                }
            };
            if override_active {
                return;
            }
        }
        let is_active = self.pcam_is_active_node(&pcam);
        if is_active {
            let visible = AnyPcam::from_node(&pcam)
                .map(|p| p.visible())
                .unwrap_or(false);
            if !visible {
                self.refresh_pcam_list_priorty();
            } else {
                let priority = AnyPcam::from_node(&pcam)
                    .map(|p| p.priority())
                    .unwrap_or_default();
                if priority < self.active_pcam_priority {
                    self.active_pcam_priority = priority;
                    self.find_pcam_with_highest_priority();
                }
            }
        } else {
            let priority = AnyPcam::from_node(&pcam)
                .map(|p| p.priority())
                .unwrap_or_default();
            if priority >= self.active_pcam_priority {
                if self.is_2d {
                    if let Some(mut camera) = self.camera_2d.clone() {
                        camera.set_as_top_level(true);
                    }
                } else if let Some(mut camera) = self.camera_3d.clone() {
                    camera.set_as_top_level(true);
                }
                self.assign_new_active_pcam(&pcam);
                self.active_pcam_missing = false;
                if let Some(candidate) = AnyPcam::from_node(&pcam) {
                    candidate.set_tween_skip(false);
                }
            }
        }
    }
    #[func]
    fn host_pcam_priority_override(&mut self, pcam: Gd<Node>, should_override: bool) {
        self.base_mut().call_deferred(
            "host_pcam_priority_override_deferred",
            &[pcam.to_variant(), should_override.to_variant()],
        );
    }
    #[func]
    fn host_pcam_priority_override_deferred(&mut self, pcam: Gd<Node>, should_override: bool) {
        if !Engine::singleton().is_editor_hint() {
            return;
        }
        if !self.pcam_is_in_host_layer(&pcam) {
            return;
        }
        if should_override {
            if self.is_2d {
                if let Some(mut active) = self.active_pcam_2d.clone() {
                    if active.bind().get_priority_override() {
                        active.bind_mut().set_priority_override(false);
                    }
                }
            } else if let Some(mut active) = self.active_pcam_3d.clone() {
                if active.bind().get_priority_override() {
                    active.bind_mut().set_priority_override(false);
                }
            }
            self.assign_new_active_pcam(&pcam);
        } else {
            self.find_pcam_with_highest_priority();
        }
        self.signals().viewfinder_update().emit(false);
    }
    #[func]
    fn pcam_priority_override_disabled(&mut self) {
        self.signals().viewfinder_update().emit(false);
    }
    #[func]
    fn get_active_pcam(&self) -> Option<Gd<Node>> {
        if self.is_2d {
            self.active_pcam_2d
                .clone()
                .map(|active| active.upcast::<Node>())
        } else {
            self.active_pcam_3d
                .clone()
                .map(|active| active.upcast::<Node>())
        }
    }
    #[func]
    fn get_trigger_pcam_tween(&self) -> bool {
        self.trigger_pcam_tween
    }
    #[func]
    fn refresh_pcam_list_priorty(&mut self) {
        self.active_pcam_priority = -1;
        self.find_pcam_with_highest_priority();
    }
    #[func(rename = process)]
    fn manual_process(&mut self, delta: f64) {
        if self.pending_ready {
            self.pending_ready = false;
            self.complete_ready();
            return;
        }
        self.tween_follow_checker(delta);
    }
    #[func]
    fn is_physics_based(&self) -> bool {
        self.follow_target_physics_based
    }
    #[func]
    fn set_interpolation_mode(&mut self, value: i64) {
        self.interpolation_mode = value;
        if value == INTERPOLATION_MANUAL {
            self.base_mut()
                .set("process_mode", &PROCESS_MODE_DISABLED.to_variant());
            return;
        }
        self.base_mut()
            .set("process_mode", &PROCESS_MODE_INHERIT.to_variant());
        if self.base().is_inside_tree() {
            self.check_pcam_physics();
        }
        let _ = INTERPOLATION_AUTO;
        let _ = INTERPOLATION_PHYSICS;
    }
    #[func]
    fn get_interpolation_mode(&self) -> i64 {
        self.interpolation_mode
    }
    #[func]
    fn set_host_layers(&mut self, value: i64) {
        self.host_layers = value;
        if !self.is_child_of_camera {
            return;
        }
        if !self.active_pcam_missing {
            if self.is_2d {
                if let Some(active) = self.active_pcam_2d.clone() {
                    let node = active.upcast::<Node>();
                    self.host_pcam_layer_changed(node);
                }
            } else if let Some(active) = self.active_pcam_3d.clone() {
                let node = active.upcast::<Node>();
                self.host_pcam_layer_changed(node);
            }
        } else {
            self.find_pcam_with_highest_priority();
        }
    }
    #[func]
    fn set_host_layers_value(&mut self, layer: i64, value: bool) {
        self.host_layers = crate::abi::set_layer(self.host_layers, layer, value);
    }
    #[func]
    fn get_host_layers(&self) -> i64 {
        self.host_layers
    }
}
fn variant_f64(value: &Variant, fallback: f64) -> f64 {
    value.try_to::<f64>().unwrap_or(fallback)
}
fn f64_of(value: real) -> f64 {
    value as f64
}
#[godot_api]
impl INode for PhantomCameraHost {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            host_layers: 1,
            interpolation_mode: INTERPOLATION_AUTO,
            active_pcam_2d: None,
            active_pcam_3d: None,
            active_pcam_priority: -1,
            active_pcam_missing: true,
            active_pcam_has_damping: false,
            follow_target_physics_based: false,
            prev_active_pcam_2d_transform: Transform2D::IDENTITY,
            prev_active_pcam_3d_transform: Transform3D::IDENTITY,
            trigger_pcam_tween: false,
            tween_elapsed_time: 0.0,
            tween_duration: 0.0,
            tween_transition: 0,
            tween_ease: 0,
            tween_is_instant: false,
            multiple_pcam_hosts: false,
            is_child_of_camera: false,
            is_2d: false,
            camera_zoom: Vector2::ONE,
            active_pcam_2d_glob_transform: Transform2D::IDENTITY,
            active_pcam_3d_glob_transform: Transform3D::IDENTITY,
            has_noise_emitted: false,
            reset_noise_offset_2d: false,
            noise_emitted_output_2d: Transform2D::IDENTITY,
            noise_emitted_output_3d: Transform3D::IDENTITY,
            show_warning: false,
            camera_2d: None,
            camera_3d: None,
            pending_ready: false,
            prev_cam_attributes: None,
            cam_attribute_type: 0,
            cam_attribute_changed: false,
            cam_attribute_assigned: false,
            prev_cam_auto_exposure_scale: 0.4005,
            cam_auto_exposure_scale_changed: false,
            prev_cam_auto_exposure_speed: 0.5,
            cam_auto_exposure_speed_changed: false,
            prev_cam_exposure_multiplier: 1.0,
            cam_exposure_multiplier_changed: false,
            prev_cam_exposure_sensitivity: 100.0,
            cam_exposure_sensitivity_changed: false,
            prev_cam_exposure_min_sensitivity: 0.0,
            cam_exposure_min_sensitivity_changed: false,
            prev_cam_exposure_max_sensitivity: 800.0,
            cam_exposure_max_sensitivity_changed: false,
            prev_cam_dof_blur_amount: 0.1005,
            cam_dof_blur_amount_changed: false,
            cam_dof_blur_far_distance_default: 10.0,
            prev_cam_dof_blur_far_distance: 10.0,
            cam_dof_blur_far_distance_changed: false,
            cam_dof_blur_far_transition_default: 5.0,
            prev_cam_dof_blur_far_transition: 5.0,
            cam_dof_blur_far_transition_changed: false,
            cam_dof_blur_near_distance_default: 2.0,
            prev_cam_dof_blur_near_distance: 2.0,
            cam_dof_blur_near_distance_changed: false,
            cam_dof_blur_near_transition_default: 1.0,
            prev_cam_dof_blur_near_transition: 1.0,
            cam_dof_blur_near_transition_changed: false,
            prev_cam_exposure_min_exposure_value: 10.0,
            cam_exposure_min_exposure_value_changed: false,
            prev_cam_exposure_max_exposure_value: -8.0,
            cam_exposure_max_exposure_value_changed: false,
            prev_cam_exposure_aperture: 16.0,
            cam_exposure_aperture_changed: false,
            prev_cam_exposure_shutter_speed: 100.0,
            cam_exposure_shutter_speed_changed: false,
            prev_cam_frustum_far: 4000.0,
            cam_frustum_far_changed: false,
            prev_cam_frustum_focal_length: 35.0,
            cam_frustum_focal_length_changed: false,
            prev_cam_frustum_near: 0.0505,
            cam_frustum_near_changed: false,
            prev_cam_frustum_focus_distance: 10.0,
            cam_frustum_focus_distance_changed: false,
            prev_cam_h_offset: 0.0,
            cam_h_offset_changed: false,
            prev_cam_v_offset: 0.0,
            cam_v_offset_changed: false,
            prev_cam_fov: 75.0,
            cam_fov_changed: false,
            prev_cam_size: 1.0,
            cam_size_changed: false,
            prev_cam_frustum_offset: Vector2::ZERO,
            cam_frustum_offset_changed: false,
            prev_cam_near: 0.0505,
            cam_near_changed: false,
            prev_cam_far: 4000.0,
            cam_far_changed: false,
        }
    }
    fn enter_tree(&mut self) {
        self.enter_tree_impl();
    }
    fn exit_tree(&mut self) {
        self.exit_tree_impl();
    }
    fn ready(&mut self) {
        self.ready_impl();
    }
    fn process(&mut self, delta: f64) {
        self.process_impl(delta);
    }
    fn physics_process(&mut self, delta: f64) {
        self.physics_process_impl(delta);
    }
}
