//! `PixelOrbitCamera` — an RTS / god-game style orbit rig tuned for pixel-art views.

use godot::classes::camera_3d::ProjectionType;
use godot::classes::{Camera3D, ICamera3D, Input, InputEvent, InputEventKey, InputEventMouseButton, InputEventMouseMotion};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use crate::util::*;

/// Orbit camera around a ground pivot (`target`). Orthographic by default, which is what
/// the `PixelRenderer` texel snapping is designed for.
///
/// Game controls (raw keys, no InputMap actions needed):
/// * WASD / arrow keys — pan along the ground, relative to the view
/// * Q / E — rotate (in `yaw_snap_deg` steps when set, keeping pixel art crisp)
/// * mouse wheel — zoom (`ortho_size` for orthographic, `distance` for perspective)
/// * middle drag — rotate and tilt, right drag — pan
///
/// In the editor the camera just places itself from the exported values.
#[derive(GodotClass)]
#[class(tool, base = Camera3D)]
pub struct PixelOrbitCamera {
    /// Point on the ground the camera orbits and looks at.
    #[export]
    target: Vector3,
    /// Distance from `target` (zoom for perspective; for orthographic it only needs to keep
    /// the scene in front of the near plane).
    #[export(range = (1.0, 1000.0, 0.1, or_greater))]
    distance: f32,
    /// Rotation around the vertical axis, in degrees.
    #[export(range = (-360.0, 360.0, 0.1))]
    #[var(set = set_yaw_deg)]
    yaw_deg: f32,
    /// Angle above the ground plane, in degrees (90 = straight down).
    #[export(range = (0.0, 90.0, 0.1))]
    pitch_deg: f32,
    #[export(range = (0.0, 90.0, 0.1))]
    pitch_min: f32,
    #[export(range = (0.0, 90.0, 0.1))]
    pitch_max: f32,
    /// Use an orthographic projection.
    #[export]
    orthographic: bool,
    /// Orthographic view height in world units (zoom for orthographic).
    #[export(range = (0.5, 500.0, 0.1, or_greater))]
    ortho_size: f32,
    /// Smallest `ortho_size` / `distance` reachable by zooming.
    #[export(range = (0.5, 500.0, 0.1, or_greater))]
    zoom_min: f32,
    /// Largest `ortho_size` / `distance` reachable by zooming.
    #[export(range = (0.5, 2000.0, 0.1, or_greater))]
    zoom_max: f32,
    /// Zoom factor per mouse-wheel notch.
    #[export(range = (1.01, 3.0, 0.01))]
    zoom_step: f32,

    #[export_group(name = "Controls")]
    /// Keyboard pan speed in view heights per second.
    #[export(range = (0.0, 10.0, 0.01))]
    pan_speed: f32,
    /// Free rotation speed (Q/E held with no snapping), degrees per second.
    #[export(range = (0.0, 720.0, 1.0))]
    rotate_speed_deg: f32,
    /// Mouse-drag rotation, degrees per pixel.
    #[export(range = (0.01, 2.0, 0.01))]
    mouse_rotate_deg: f32,
    /// Q/E rotation step in degrees (0 = free rotation). Middle-drag also settles on a step.
    #[export(range = (0.0, 180.0, 1.0))]
    yaw_snap_deg: f32,
    /// How quickly the yaw settles on its target (0 = instant).
    #[export(range = (0.0, 60.0, 0.1))]
    smoothing: f32,
    /// Pan when the mouse touches the window edge.
    #[export]
    edge_pan: bool,
    /// Edge-pan border width in pixels.
    #[export(range = (1.0, 64.0, 1.0))]
    edge_margin: f32,

    yaw_goal: f32,
    rotating: bool,
    /// Inputs of the last `place` (it skips while they are unchanged).
    placed: Option<[f32; 8]>,
    base: Base<Camera3D>,
}

#[godot_api]
impl ICamera3D for PixelOrbitCamera {
    fn init(base: Base<Camera3D>) -> Self {
        PixelOrbitCamera {
            target: Vector3::ZERO,
            distance: 40.0,
            yaw_deg: 45.0,
            pitch_deg: 50.0,
            pitch_min: 25.0,
            pitch_max: 89.0,
            orthographic: true,
            ortho_size: 24.0,
            zoom_min: 4.0,
            zoom_max: 200.0,
            zoom_step: 1.15,
            pan_speed: 0.8,
            rotate_speed_deg: 90.0,
            mouse_rotate_deg: 0.3,
            yaw_snap_deg: 45.0,
            smoothing: 14.0,
            edge_pan: false,
            edge_margin: 6.0,
            yaw_goal: 45.0,
            rotating: false,
            placed: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.yaw_goal = self.yaw_deg;
        self.base_mut().set_process(true);
        self.place();
    }

    fn process(&mut self, delta: f64) {
        if !is_editor() && self.base().is_current() {
            self.handle_keys(delta as f32);
        }
        // Ease the yaw towards its goal (Q/E steps animate instead of jumping).
        let diff = self.yaw_goal - self.yaw_deg;
        self.yaw_deg = if self.smoothing <= 0.0 || diff.abs() < 0.01 {
            self.yaw_goal
        } else {
            self.yaw_deg + diff * (1.0 - (-self.smoothing * delta as f32).exp())
        };
        self.place();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if is_editor() || !self.base().is_current() {
            return;
        }
        let handled = if let Ok(k) = event.clone().try_cast::<InputEventKey>() {
            self.on_key(&k)
        } else if let Ok(b) = event.clone().try_cast::<InputEventMouseButton>() {
            self.on_button(&b)
        } else if let Ok(m) = event.try_cast::<InputEventMouseMotion>() {
            self.on_motion(&m)
        } else {
            false
        };
        if handled && let Some(mut vp) = self.base().get_viewport() {
            vp.set_input_as_handled();
        }
    }
}

impl PixelOrbitCamera {
    /// Positions and orients the camera from target / yaw / pitch / distance.
    fn place(&mut self) {
        if !self.base().is_inside_tree() {
            return;
        }
        self.pitch_deg = self.pitch_deg.clamp(self.pitch_min.min(self.pitch_max), self.pitch_max.max(self.pitch_min)).clamp(0.0, 90.0);
        let (lo, hi) = (self.zoom_min.min(self.zoom_max), self.zoom_max.max(self.zoom_min));
        let basis = Basis::from_euler(EulerOrder::YXZ, Vector3::new(-self.pitch_deg.to_radians(), self.yaw_deg.to_radians(), 0.0));
        let origin = self.target + basis.col_c() * self.distance.max(0.01);
        let ortho = self.orthographic;
        let size = self.ortho_size.clamp(lo, hi);
        // Nothing to do while the inputs are unchanged: re-placing an unmoved camera every frame
        // kept the editor redrawing (the camera's gizmo and its RenderingServer state).
        let key = [self.target.x, self.target.y, self.target.z, self.yaw_deg, self.pitch_deg, self.distance, size, if ortho { 1.0 } else { 0.0 }];
        if self.placed == Some(key) {
            return;
        }
        self.placed = Some(key);
        let mut cam = self.base_mut();
        crate::writes::set_global_transform(&mut cam, Transform3D::new(basis, origin));
        if ortho {
            if cam.get_projection() != ProjectionType::ORTHOGONAL {
                cam.set_projection(ProjectionType::ORTHOGONAL);
            }
            if cam.get_size() != size {
                cam.set_size(size);
            }
        } else if cam.get_projection() != ProjectionType::PERSPECTIVE {
            cam.set_projection(ProjectionType::PERSPECTIVE);
        }
    }

    /// Ground-plane right and forward vectors for the current yaw.
    fn ground_axes(&self) -> (Vector3, Vector3) {
        let y = self.yaw_deg.to_radians();
        (Vector3::new(y.cos(), 0.0, -y.sin()), Vector3::new(-y.sin(), 0.0, -y.cos()))
    }

    /// World units covered by the view height (scales panning with zoom).
    fn view_height(&self) -> f32 {
        if self.orthographic {
            self.ortho_size
        } else {
            2.0 * self.distance * (self.base().get_fov().to_radians() * 0.5).tan()
        }
    }

    fn handle_keys(&mut self, dt: f32) {
        let input = Input::singleton();
        let down = |a: Key, b: Key| input.is_physical_key_pressed(a) || input.is_key_pressed(b);
        let mut mv = Vector2::ZERO;
        if down(Key::W, Key::UP) {
            mv.y += 1.0;
        }
        if down(Key::S, Key::DOWN) {
            mv.y -= 1.0;
        }
        if down(Key::D, Key::RIGHT) {
            mv.x += 1.0;
        }
        if down(Key::A, Key::LEFT) {
            mv.x -= 1.0;
        }
        if self.edge_pan
            && let Some(vp) = self.base().get_viewport()
        {
            let size = vp.get_visible_rect().size;
            let p = vp.get_mouse_position();
            let m = self.edge_margin;
            if p.x >= 0.0 && p.y >= 0.0 && p.x <= size.x && p.y <= size.y {
                if p.x < m {
                    mv.x -= 1.0;
                } else if p.x > size.x - m {
                    mv.x += 1.0;
                }
                if p.y < m {
                    mv.y += 1.0;
                } else if p.y > size.y - m {
                    mv.y -= 1.0;
                }
            }
        }
        if mv != Vector2::ZERO {
            let (right, fwd) = self.ground_axes();
            let step = self.pan_speed * self.view_height() * dt;
            self.target += (right * mv.x + fwd * mv.y).normalized() * step;
        }
        // Free rotation while Q/E are held (stepped rotation is handled in on_key).
        if self.yaw_snap_deg <= 0.0 {
            let mut r = 0.0;
            if input.is_physical_key_pressed(Key::Q) {
                r -= 1.0;
            }
            if input.is_physical_key_pressed(Key::E) {
                r += 1.0;
            }
            self.yaw_goal += r * self.rotate_speed_deg * dt;
        }
    }

    fn on_key(&mut self, k: &Gd<InputEventKey>) -> bool {
        if !k.is_pressed() || k.is_echo() || self.yaw_snap_deg <= 0.0 {
            return false;
        }
        let dir = match k.get_physical_keycode() {
            Key::Q => -1.0,
            Key::E => 1.0,
            _ => return false,
        };
        let step = self.yaw_snap_deg;
        self.yaw_goal = ((self.yaw_goal / step).round() + dir) * step;
        true
    }

    fn on_button(&mut self, b: &Gd<InputEventMouseButton>) -> bool {
        let pressed = b.is_pressed();
        match b.get_button_index() {
            MouseButton::WHEEL_UP | MouseButton::WHEEL_DOWN if pressed => {
                let f = if b.get_button_index() == MouseButton::WHEEL_UP { 1.0 / self.zoom_step } else { self.zoom_step };
                let (lo, hi) = (self.zoom_min.min(self.zoom_max), self.zoom_max.max(self.zoom_min));
                if self.orthographic {
                    self.ortho_size = (self.ortho_size * f).clamp(lo, hi);
                } else {
                    self.distance = (self.distance * f).clamp(lo, hi);
                }
                true
            }
            MouseButton::MIDDLE => {
                self.rotating = pressed;
                // Settle on the nearest step when the drag ends.
                if !pressed && self.yaw_snap_deg > 0.0 {
                    self.yaw_goal = (self.yaw_goal / self.yaw_snap_deg).round() * self.yaw_snap_deg;
                }
                true
            }
            _ => false,
        }
    }

    fn on_motion(&mut self, m: &Gd<InputEventMouseMotion>) -> bool {
        let rel = m.get_relative();
        let input = Input::singleton();
        if self.rotating || input.is_mouse_button_pressed(MouseButton::MIDDLE) {
            self.yaw_goal -= rel.x * self.mouse_rotate_deg;
            self.yaw_deg = self.yaw_goal;
            self.pitch_deg += rel.y * self.mouse_rotate_deg;
            return true;
        }
        if input.is_mouse_button_pressed(MouseButton::RIGHT) {
            let Some(vp) = self.base().get_viewport() else { return false };
            let h = vp.get_visible_rect().size.y.max(1.0);
            let per_px = self.view_height() / h;
            let (right, fwd) = self.ground_axes();
            // Drag the ground: moving the mouse right moves the view left. Vertical screen
            // motion covers more ground the flatter the view is.
            let tilt = self.pitch_deg.to_radians().sin().max(0.2);
            self.target += (-right * rel.x + fwd * (rel.y / tilt)) * per_px;
            return true;
        }
        false
    }
}

#[godot_api]
impl PixelOrbitCamera {
    /// Moves the pivot (e.g. to follow a unit). The view keeps its yaw, pitch and zoom.
    #[func]
    fn focus(&mut self, point: Vector3) {
        self.target = point;
        self.place();
    }

    /// Sets the rotation immediately (no easing).
    #[func]
    fn set_yaw_deg(&mut self, deg: f32) {
        self.yaw_deg = deg;
        self.yaw_goal = deg;
        self.place();
    }
}
