//! A camera a reader of the tutorials can steer around the scene they are studying, so each
//! step can be looked at from any side without opening the editor.
//!
//! Intent: a tutorial scene places this where its camera would be, says which spot to look
//! at and from how far, and the reader can then circle that spot by dragging, zoom in and
//! out with the wheel, and slide the spot around and up and down with the keyboard.
//!
//! Responsibility: it owns where the camera sits and what it looks at while the scene runs,
//! and how the reader's input changes that. It does not own the input bindings, which the
//! project declares, and it does not change how the camera renders beyond its zoom.
//!
//! Why a node: it is the camera itself, so it needs a place in the world and the engine's
//! input and frame callbacks.
//!
//! Why a new type: a built-in camera placed in a scene stays where it was put, and the
//! project has no scripts, so the steering rules need a camera type of their own.
//!
//! Not exported: the current angles, distance and focus point the reader has steered to,
//! whether a drag is in progress, and which pan keys are held. They start from the exported
//! view each run and only mean something while it lasts.
use godot::classes::camera_3d::ProjectionType;
use godot::classes::{
    Camera3D, ICamera3D, Input, InputEvent, InputEventMouseButton, InputEventMouseMotion,
};
use godot::global::MouseButton;
use godot::prelude::*;
const ACT_PAN_LEFT: &str = "tutorial_camera_pan_left";
const ACT_PAN_RIGHT: &str = "tutorial_camera_pan_right";
const ACT_PAN_FORWARD: &str = "tutorial_camera_pan_forward";
const ACT_PAN_BACK: &str = "tutorial_camera_pan_back";
const ACT_LOWER: &str = "tutorial_camera_lower";
const ACT_RAISE: &str = "tutorial_camera_raise";
#[derive(GodotClass)]
#[class(base = Camera3D)]
pub struct MstOrbitCamera {
    #[export]
    target: Vector3,
    #[export]
    distance: f32,
    #[export]
    yaw_deg: f32,
    #[export]
    pitch_deg: f32,
    #[export]
    min_distance: f32,
    #[export]
    max_distance: f32,
    #[export]
    min_size: f32,
    #[export]
    max_size: f32,
    #[export]
    pitch_min_deg: f32,
    #[export]
    pitch_max_deg: f32,
    #[export]
    orbit_radians_per_pixel: f32,
    #[export]
    zoom_in_factor: f32,
    #[export]
    zoom_out_factor: f32,
    #[export]
    pan_base_speed: f32,
    #[export]
    pan_speed_per_distance: f32,
    yaw: f32,
    pitch: f32,
    current_distance: f32,
    focus: Vector3,
    dragging: bool,
    pan_input: Vector3,
    base: Base<Camera3D>,
}
#[godot_api]
impl ICamera3D for MstOrbitCamera {
    fn init(base: Base<Camera3D>) -> Self {
        Self {
            target: Vector3::ZERO,
            distance: 40.0,
            yaw_deg: 38.0,
            pitch_deg: 38.0,
            min_distance: 6.0,
            max_distance: 400.0,
            min_size: 4.0,
            max_size: 400.0,
            pitch_min_deg: 5.0,
            pitch_max_deg: 88.0,
            orbit_radians_per_pixel: 0.006,
            zoom_in_factor: 0.9,
            zoom_out_factor: 1.1,
            pan_base_speed: 12.0,
            pan_speed_per_distance: 0.25,
            yaw: 0.0,
            pitch: 0.0,
            current_distance: 0.0,
            focus: Vector3::ZERO,
            dragging: false,
            pan_input: Vector3::ZERO,
            base,
        }
    }
    fn ready(&mut self) {
        self.yaw = self.yaw_deg.to_radians();
        self.pitch = self.pitch_deg.to_radians();
        self.current_distance = self.distance;
        self.focus = self.target;
        self.apply();
    }
    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.is_action_type() {
            let input = Input::singleton();
            let planar =
                input.get_vector(ACT_PAN_LEFT, ACT_PAN_RIGHT, ACT_PAN_FORWARD, ACT_PAN_BACK);
            let vertical = input.get_axis(ACT_LOWER, ACT_RAISE);
            self.pan_input = Vector3::new(planar.x, vertical, planar.y);
        }
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            let button = mb.get_button_index();
            if button == MouseButton::RIGHT {
                self.dragging = mb.is_pressed();
            } else if button == MouseButton::WHEEL_UP && mb.is_pressed() {
                self.zoom(self.zoom_in_factor);
            } else if button == MouseButton::WHEEL_DOWN && mb.is_pressed() {
                self.zoom(self.zoom_out_factor);
            }
        } else if let Ok(mm) = event.try_cast::<InputEventMouseMotion>()
            && self.dragging
        {
            let relative = mm.get_relative();
            let lo = self.pitch_min_deg.min(self.pitch_max_deg).to_radians();
            let hi = self.pitch_max_deg.max(self.pitch_min_deg).to_radians();
            self.yaw -= relative.x * self.orbit_radians_per_pixel;
            self.pitch = (self.pitch - relative.y * self.orbit_radians_per_pixel).clamp(lo, hi);
            self.apply();
        }
    }
    fn process(&mut self, delta: f64) {
        let pan = self.pan_input;
        if pan == Vector3::ZERO {
            return;
        }
        let flat = Vector3::new(pan.x, 0.0, pan.z).rotated(Vector3::UP, self.yaw);
        let speed = self.pan_base_speed + self.current_distance * self.pan_speed_per_distance;
        self.focus += (flat + Vector3::UP * pan.y) * speed * delta as f32;
        self.apply();
    }
}
impl MstOrbitCamera {
    fn zoom(&mut self, factor: f32) {
        if self.base().get_projection() == ProjectionType::ORTHOGONAL {
            let lo = self.min_size.min(self.max_size);
            let hi = self.max_size.max(self.min_size);
            let size = (self.base().get_size() * factor).clamp(lo, hi);
            self.base_mut().set_size(size);
        } else {
            let lo = self.min_distance.min(self.max_distance);
            let hi = self.max_distance.max(self.min_distance);
            self.current_distance = (self.current_distance * factor).clamp(lo, hi);
            self.apply();
        }
    }
    fn apply(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        let offset = Vector3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        ) * self.current_distance;
        let focus = self.focus;
        let mut cam = self.base_mut();
        cam.set_global_position(focus + offset);
        cam.look_at(focus);
    }
}
