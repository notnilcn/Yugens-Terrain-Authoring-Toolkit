use godot::classes::fast_noise_lite::NoiseType;
use godot::classes::{FastNoiseLite, IResource, Resource};
use godot::global::randi;
use godot::prelude::*;
use crate::abi;
#[inline]
fn real_of(value: f64) -> real {
    value as real
}
#[derive(GodotClass)]
#[class(base=Resource)]
pub struct PhantomCameraTween {
    base: Base<Resource>,
    #[export]
    pub duration: f64,
    #[export]
    #[var(hint = ENUM, hint_string = "Linear:0,Sine:1,Quint:2,Quart:3,Quad:4,Expo:5,Elastic:6,Cubic:7,Circ:8,Bounce:9,Back:10")]
    pub transition: i64,
    #[export]
    #[var(hint = ENUM, hint_string = "Ease In:0,Ease Out:1,Ease In Out:2,Ease Out In:3")]
    pub ease: i64,
}
#[godot_api]
impl IResource for PhantomCameraTween {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            duration: abi::TWEEN_DEFAULT_DURATION,
            transition: abi::TWEEN_DEFAULT_TRANSITION,
            ease: abi::TWEEN_DEFAULT_EASE,
        }
    }
}
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct Camera3DResource {
    base: Base<Resource>,
    #[export]
    #[var(set = set_keep_aspect, hint = ENUM, hint_string = "Keep Width:0,Keep Height:1")]
    pub keep_aspect: i64,
    #[export(flags_3d_render)]
    #[var(set = set_cull_mask)]
    pub cull_mask: i64,
    #[export]
    #[var(set = set_h_offset, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater,or_less,hide_slider,suffix:m")]
    pub h_offset: f64,
    #[export]
    #[var(set = set_v_offset, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater,or_less,hide_slider,suffix:m")]
    pub v_offset: f64,
    #[export]
    #[var(set = set_projection, hint = ENUM, hint_string = "Perspective:0,Orthogonal:1,Frustum:2")]
    pub projection: i64,
    #[export]
    #[var(set = set_fov, hint = RANGE, hint_string = "1.0,179.0,0.1,degrees")]
    pub fov: f64,
    #[export]
    #[var(set = set_size, hint = RANGE, hint_string = "0.001,100.0,0.001,suffix:m,or_greater")]
    pub size: f64,
    #[export]
    #[var(set = set_frustum_offset)]
    pub frustum_offset: Vector2,
    #[export]
    #[var(set = set_near, hint = RANGE, hint_string = "0.001,10.0,0.001,suffix:m,or_greater")]
    pub near: f64,
    #[export]
    #[var(set = set_far, hint = RANGE, hint_string = "0.01,4000.0,0.001,suffix:m,or_greater")]
    pub far: f64,
}
#[godot_api]
impl IResource for Camera3DResource {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            keep_aspect: abi::KeepAspect::KeepHeight as i64,
            cull_mask: abi::CAMERA_3D_DEFAULT_CULL_MASK,
            h_offset: 0.0,
            v_offset: 0.0,
            projection: abi::ProjectionType::Perspective as i64,
            fov: 75.0,
            size: 1.0,
            frustum_offset: Vector2::ZERO,
            near: 0.0505,
            far: 4000.0,
        }
    }
}
#[godot_api]
impl Camera3DResource {
    #[func]
    pub fn set_keep_aspect(&mut self, value: i64) {
        self.keep_aspect = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_cull_mask(&mut self, value: i64) {
        self.cull_mask = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_h_offset(&mut self, value: f64) {
        self.h_offset = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_v_offset(&mut self, value: f64) {
        self.v_offset = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_projection(&mut self, value: i64) {
        self.projection = value;
        self.base_mut().notify_property_list_changed();
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_fov(&mut self, value: f64) {
        self.fov = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_size(&mut self, value: f64) {
        self.size = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_frustum_offset(&mut self, value: Vector2) {
        self.frustum_offset = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_near(&mut self, value: f64) {
        self.near = value;
        self.base_mut().emit_changed();
    }
    #[func]
    pub fn set_far(&mut self, value: f64) {
        self.far = value;
        self.base_mut().emit_changed();
    }
    pub fn read(&self, property: &str) -> Variant {
        match property {
            "keep_aspect" => self.keep_aspect.to_variant(),
            "cull_mask" => self.cull_mask.to_variant(),
            "h_offset" => self.h_offset.to_variant(),
            "v_offset" => self.v_offset.to_variant(),
            "projection" => self.projection.to_variant(),
            "fov" => self.fov.to_variant(),
            "size" => self.size.to_variant(),
            "frustum_offset" => self.frustum_offset.to_variant(),
            "near" => self.near.to_variant(),
            "far" => self.far.to_variant(),
            _ => Variant::nil(),
        }
    }
    pub fn persist(&mut self, property: &str, value: Variant) {
        match property {
            "keep_aspect" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_keep_aspect(v);
                }
            }
            "cull_mask" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_cull_mask(v);
                }
            }
            "h_offset" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_h_offset(v);
                }
            }
            "v_offset" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_v_offset(v);
                }
            }
            "projection" => {
                if let Ok(v) = value.try_to::<i64>() {
                    self.set_projection(v);
                }
            }
            "fov" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_fov(v);
                }
            }
            "size" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_size(v);
                }
            }
            "frustum_offset" => {
                if let Ok(v) = value.try_to::<Vector2>() {
                    self.set_frustum_offset(v);
                }
            }
            "near" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_near(v);
                }
            }
            "far" => {
                if let Ok(v) = value.try_to::<f64>() {
                    self.set_far(v);
                }
            }
            _ => {}
        }
    }
}
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct PhantomCameraNoise2D {
    base: Base<Resource>,
    #[export]
    #[var(get = get_amplitude, set = set_amplitude, hint = RANGE, hint_string = "0.0,1000.0,0.001,or_greater")]
    amplitude: f64,
    #[export]
    #[var(get = get_frequency, set = set_frequency, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater")]
    frequency: f64,
    #[export]
    #[var(get = get_randomize_noise_seed, set = set_randomize_noise_seed)]
    randomize_noise_seed: bool,
    #[export]
    #[var(get = get_noise_seed, set = set_noise_seed)]
    noise_seed: i64,
    #[export]
    #[var(get = get_positional_noise, set = set_positional_noise)]
    positional_noise: bool,
    #[export]
    #[var(get = get_rotational_noise, set = set_rotational_noise)]
    rotational_noise: bool,
    #[export]
    #[var(get = get_positional_multiplier_x, set = set_positional_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_x: f64,
    #[export]
    #[var(get = get_positional_multiplier_y, set = set_positional_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_y: f64,
    #[export]
    #[var(get = get_rotational_multiplier, set = set_rotational_multiplier, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier: f64,
    noise_algorithm: Option<Gd<FastNoiseLite>>,
    noise_positional_multiplier: Vector2,
    trauma: f64,
    noise_time: f64,
}
#[godot_api]
impl IResource for PhantomCameraNoise2D {
    fn init(base: Base<Resource>) -> Self {
        let mut noise_algorithm = FastNoiseLite::new_gd();
        noise_algorithm.set_noise_type(NoiseType::PERLIN);
        noise_algorithm.set_seed(truncate_seed(randi()));
        noise_algorithm.set_frequency(0.5);
        Self {
            base,
            amplitude: 10.0,
            frequency: 0.5,
            randomize_noise_seed: true,
            noise_seed: 0,
            positional_noise: true,
            rotational_noise: false,
            positional_multiplier_x: 1.0,
            positional_multiplier_y: 1.0,
            rotational_multiplier: 1.0,
            noise_algorithm: Some(noise_algorithm),
            noise_positional_multiplier: Vector2::new(1.0, 1.0),
            trauma: 0.0,
            noise_time: 0.0,
        }
    }
}
#[godot_api]
impl PhantomCameraNoise2D {
    #[func]
    fn _get_noise_from_seed(&self, noise_seed: i64) -> f64 {
        self.noise_from_seed(noise_seed)
    }
    fn noise_from_seed(&self, noise_seed: i64) -> f64 {
        let Some(algorithm) = self.noise_algorithm.as_ref() else {
            return 0.0;
        };
        let sample = algorithm.get_noise_2d(noise_seed as f32, self.noise_time as f32);
        f64::from(sample) * self.amplitude
    }
    fn write_trauma(&mut self, value: f64) {
        self.trauma = value;
    }
    #[func]
    pub fn get_noise_transform(&mut self, delta: f64) -> Transform2D {
        let mut output_position = Vector2::ZERO;
        let mut output_rotation = 0.0_f64;
        self.noise_time += delta;
        self.write_trauma(self.trauma.max(0.0));
        let trauma_squared = self.trauma.powi(2);
        if self.positional_noise {
            for axis in 0..2_i64 {
                let multiplier = if axis == 0 {
                    self.noise_positional_multiplier.x
                } else {
                    self.noise_positional_multiplier.y
                };
                let value = f64::from(multiplier)
                    * trauma_squared
                    * self.noise_from_seed(axis + self.noise_seed);
                if axis == 0 {
                    output_position.x = real_of(value);
                } else {
                    output_position.y = real_of(value);
                }
            }
        }
        if self.rotational_noise {
            output_rotation = self.rotational_multiplier / 100.0
                * trauma_squared
                * self.noise_from_seed(self.noise_seed);
        }
        Transform2D::from_angle_origin(real_of(output_rotation), output_position)
    }
    #[func]
    pub fn reset_noise_time(&mut self) {
        self.noise_time = 0.0;
    }
    #[func]
    pub fn set_trauma(&mut self, value: f64) {
        self.write_trauma(value);
    }
    #[func]
    pub fn set_amplitude(&mut self, value: f64) {
        self.amplitude = value;
    }
    #[func]
    pub fn get_amplitude(&self) -> f64 {
        self.amplitude
    }
    #[func]
    pub fn set_frequency(&mut self, value: f64) {
        self.frequency = value;
        if let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_frequency(value as f32);
        }
    }
    #[func]
    pub fn get_frequency(&self) -> f64 {
        self.frequency
    }
    #[func]
    pub fn set_randomize_noise_seed(&mut self, value: bool) {
        self.randomize_noise_seed = value;
        if value && let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_seed(truncate_seed(randi()));
        }
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_randomize_noise_seed(&self) -> bool {
        self.randomize_noise_seed
    }
    #[func]
    pub fn set_noise_seed(&mut self, value: i64) {
        self.noise_seed = value;
    }
    #[func]
    pub fn get_noise_seed(&self) -> i64 {
        self.noise_seed
    }
    #[func]
    pub fn set_positional_noise(&mut self, value: bool) {
        self.positional_noise = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_positional_noise(&self) -> bool {
        self.positional_noise
    }
    #[func]
    pub fn set_rotational_noise(&mut self, value: bool) {
        self.rotational_noise = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_rotational_noise(&self) -> bool {
        self.rotational_noise
    }
    #[func]
    pub fn set_positional_multiplier_x(&mut self, value: f64) {
        self.positional_multiplier_x = value;
        self.noise_positional_multiplier.x = real_of(value);
    }
    #[func]
    pub fn get_positional_multiplier_x(&self) -> f64 {
        self.positional_multiplier_x
    }
    #[func]
    pub fn set_positional_multiplier_y(&mut self, value: f64) {
        self.positional_multiplier_y = value;
        self.noise_positional_multiplier.y = real_of(value);
    }
    #[func]
    pub fn get_positional_multiplier_y(&self) -> f64 {
        self.positional_multiplier_y
    }
    #[func]
    pub fn set_rotational_multiplier(&mut self, value: f64) {
        self.rotational_multiplier = value;
    }
    #[func]
    pub fn get_rotational_multiplier(&self) -> f64 {
        self.rotational_multiplier
    }
}
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct PhantomCameraNoise3D {
    base: Base<Resource>,
    #[export]
    #[var(get = get_amplitude, set = set_amplitude, hint = RANGE, hint_string = "0.0,100.0,0.001,or_greater")]
    amplitude: f64,
    #[export]
    #[var(get = get_frequency, set = set_frequency, hint = RANGE, hint_string = "0.0,10.0,0.001,or_greater")]
    frequency: f64,
    #[export]
    #[var(get = get_randomize_noise_seed, set = set_randomize_noise_seed)]
    randomize_noise_seed: bool,
    #[export]
    #[var(get = get_noise_seed, set = set_noise_seed)]
    noise_seed: i64,
    #[export]
    #[var(get = get_rotational_noise, set = set_rotational_noise)]
    rotational_noise: bool,
    #[export]
    #[var(get = get_positional_noise, set = set_positional_noise)]
    positional_noise: bool,
    #[export]
    #[var(get = get_rotational_multiplier_x, set = set_rotational_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_x: f64,
    #[export]
    #[var(get = get_rotational_multiplier_y, set = set_rotational_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_y: f64,
    #[export]
    #[var(get = get_rotational_multiplier_z, set = set_rotational_multiplier_z, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    rotational_multiplier_z: f64,
    #[export]
    #[var(get = get_positional_multiplier_x, set = set_positional_multiplier_x, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_x: f64,
    #[export]
    #[var(get = get_positional_multiplier_y, set = set_positional_multiplier_y, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_y: f64,
    #[export]
    #[var(get = get_positional_multiplier_z, set = set_positional_multiplier_z, hint = RANGE, hint_string = "0.0,1.0,0.001,or_greater")]
    positional_multiplier_z: f64,
    noise_algorithm: Option<Gd<FastNoiseLite>>,
    noise_rotational_multiplier: Vector3,
    noise_positional_multiplier: Vector3,
    trauma: f64,
    noise_time: f64,
}
#[godot_api]
impl IResource for PhantomCameraNoise3D {
    fn init(base: Base<Resource>) -> Self {
        let mut noise_algorithm = FastNoiseLite::new_gd();
        noise_algorithm.set_noise_type(NoiseType::PERLIN);
        noise_algorithm.set_seed(truncate_seed(randi()));
        noise_algorithm.set_frequency(0.2005);
        Self {
            base,
            amplitude: 10.0,
            frequency: 0.2005,
            randomize_noise_seed: true,
            noise_seed: 0,
            rotational_noise: true,
            positional_noise: false,
            rotational_multiplier_x: 1.0,
            rotational_multiplier_y: 1.0,
            rotational_multiplier_z: 1.0,
            positional_multiplier_x: 1.0,
            positional_multiplier_y: 1.0,
            positional_multiplier_z: 1.0,
            noise_algorithm: Some(noise_algorithm),
            noise_rotational_multiplier: Vector3::ONE,
            noise_positional_multiplier: Vector3::ONE,
            trauma: 0.0,
            noise_time: 0.0,
        }
    }
}
#[godot_api]
impl PhantomCameraNoise3D {
    #[func]
    fn _get_noise_from_seed(&self, noise_seed: i64) -> f64 {
        self.noise_from_seed(noise_seed)
    }
    fn noise_from_seed(&self, noise_seed: i64) -> f64 {
        let Some(algorithm) = self.noise_algorithm.as_ref() else {
            return 0.0;
        };
        let sample = algorithm.get_noise_2d(noise_seed as f32, self.noise_time as f32);
        f64::from(sample) * self.amplitude
    }
    fn write_trauma(&mut self, value: f64) {
        self.trauma = value;
        if self.trauma == 0.0 {
            self.noise_time = 0.0;
        }
    }
    #[func]
    pub fn get_noise_transform(&mut self, delta: f64) -> Transform3D {
        let mut output_rotation = Vector3::ZERO;
        let mut output_position = Vector3::ZERO;
        self.noise_time += delta;
        self.write_trauma(self.trauma.max(0.0));
        let trauma_squared = self.trauma.powi(2);
        let rotational = [
            f64::from(self.noise_rotational_multiplier.x),
            f64::from(self.noise_rotational_multiplier.y),
            f64::from(self.noise_rotational_multiplier.z),
        ];
        let positional = [
            f64::from(self.noise_positional_multiplier.x),
            f64::from(self.noise_positional_multiplier.y),
            f64::from(self.noise_positional_multiplier.z),
        ];
        let mut rotation = [0.0_f64; 3];
        let mut position = [0.0_f64; 3];
        for axis in 0..3_usize {
            let seed = axis as i64 + self.noise_seed;
            if self.rotational_noise {
                rotation[axis] = godot::global::deg_to_rad(
                    rotational[axis] * trauma_squared * self.noise_from_seed(seed),
                );
            }
            if self.positional_noise {
                position[axis] +=
                    positional[axis] / 10.0 * trauma_squared * self.noise_from_seed(seed);
            }
        }
        output_rotation.x = real_of(rotation[0]);
        output_rotation.y = real_of(rotation[1]);
        output_rotation.z = real_of(rotation[2]);
        output_position.x = real_of(position[0]);
        output_position.y = real_of(position[1]);
        output_position.z = real_of(position[2]);
        Transform3D::new(
            Basis::from_quaternion(Quaternion::from_euler(output_rotation)),
            output_position,
        )
    }
    #[func]
    pub fn reset_noise_time(&mut self) {
        self.noise_time = 0.0;
    }
    #[func]
    pub fn set_trauma(&mut self, value: f64) {
        self.write_trauma(value);
    }
    #[func]
    pub fn set_amplitude(&mut self, value: f64) {
        self.amplitude = value;
    }
    #[func]
    pub fn get_amplitude(&self) -> f64 {
        self.amplitude
    }
    #[func]
    pub fn set_frequency(&mut self, value: f64) {
        self.frequency = value;
        if let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_frequency(value as f32);
        }
    }
    #[func]
    pub fn get_frequency(&self) -> f64 {
        self.frequency
    }
    #[func]
    pub fn set_randomize_noise_seed(&mut self, value: bool) {
        self.randomize_noise_seed = value;
        if value && let Some(mut algorithm) = self.noise_algorithm.clone() {
            algorithm.set_seed(truncate_seed(randi()));
        }
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_randomize_noise_seed(&self) -> bool {
        self.randomize_noise_seed
    }
    #[func]
    pub fn set_noise_seed(&mut self, value: i64) {
        self.noise_seed = value;
    }
    #[func]
    pub fn get_noise_seed(&self) -> i64 {
        self.noise_seed
    }
    #[func]
    pub fn set_positional_noise(&mut self, value: bool) {
        self.positional_noise = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_positional_noise(&self) -> bool {
        self.positional_noise
    }
    #[func]
    pub fn set_rotational_noise(&mut self, value: bool) {
        self.rotational_noise = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_rotational_noise(&self) -> bool {
        self.rotational_noise
    }
    #[func]
    pub fn set_positional_multiplier_x(&mut self, value: f64) {
        self.positional_multiplier_x = value;
        self.noise_positional_multiplier.x = real_of(value);
    }
    #[func]
    pub fn get_positional_multiplier_x(&self) -> f64 {
        self.positional_multiplier_x
    }
    #[func]
    pub fn set_positional_multiplier_y(&mut self, value: f64) {
        self.positional_multiplier_y = value;
        self.noise_positional_multiplier.y = real_of(value);
    }
    #[func]
    pub fn get_positional_multiplier_y(&self) -> f64 {
        self.positional_multiplier_y
    }
    #[func]
    pub fn set_positional_multiplier_z(&mut self, value: f64) {
        self.positional_multiplier_z = value;
        self.noise_positional_multiplier.z = real_of(value);
    }
    #[func]
    pub fn get_positional_multiplier_z(&self) -> f64 {
        self.positional_multiplier_z
    }
    #[func]
    pub fn set_rotational_multiplier_x(&mut self, value: f64) {
        self.rotational_multiplier_x = value;
        self.noise_rotational_multiplier.x = real_of(value);
    }
    #[func]
    pub fn get_rotational_multiplier_x(&self) -> f64 {
        self.rotational_multiplier_x
    }
    #[func]
    pub fn set_rotational_multiplier_y(&mut self, value: f64) {
        self.rotational_multiplier_y = value;
        self.noise_rotational_multiplier.y = real_of(value);
    }
    #[func]
    pub fn get_rotational_multiplier_y(&self) -> f64 {
        self.rotational_multiplier_y
    }
    #[func]
    pub fn set_rotational_multiplier_z(&mut self, value: f64) {
        self.rotational_multiplier_z = value;
        self.noise_rotational_multiplier.z = real_of(value);
    }
    #[func]
    pub fn get_rotational_multiplier_z(&self) -> f64 {
        self.rotational_multiplier_z
    }
}
#[inline]
fn truncate_seed(seed: i64) -> i32 {
    seed as i32
}
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct TweenDirectorResource {
    base: Base<Resource>,
    #[export]
    #[var(get = get_tween_resource, set = set_tween_resource)]
    pub tween_resource: Option<Gd<PhantomCameraTween>>,
    #[export]
    #[var(get = get_from_type, set = set_from_type, hint = ENUM, hint_string = "Phantom Camera:0,Tween Resource:1,Any:2")]
    pub from_type: i64,
    #[export(node_path = ("PhantomCamera2D", "PhantomCamera3D"))]
    #[var(get = get_from_phantom_cameras, set = set_from_phantom_cameras)]
    pub from_phantom_cameras: Array<NodePath>,
    #[export]
    #[var(get = get_from_tween_resources, set = set_from_tween_resources)]
    pub from_tween_resources: Array<Gd<PhantomCameraTween>>,
    #[export]
    #[var(get = get_to_type, set = set_to_type, hint = ENUM, hint_string = "Phantom Camera:0,Tween Resource:1,Any:2")]
    pub to_type: i64,
    #[export(node_path = ("PhantomCamera2D", "PhantomCamera3D"))]
    #[var(get = get_to_phantom_cameras, set = set_to_phantom_cameras)]
    pub to_phantom_cameras: Array<NodePath>,
    #[export]
    #[var(get = get_to_tween_resources, set = set_to_tween_resources)]
    pub to_tween_resources: Array<Gd<PhantomCameraTween>>,
}
#[godot_api]
impl IResource for TweenDirectorResource {
    fn init(base: Base<Resource>) -> Self {
        Self {
            base,
            tween_resource: None,
            from_type: abi::TweenDirectorMatch::PhantomCamera as i64,
            from_phantom_cameras: Array::new(),
            from_tween_resources: Array::new(),
            to_type: abi::TweenDirectorMatch::PhantomCamera as i64,
            to_phantom_cameras: Array::new(),
            to_tween_resources: Array::new(),
        }
    }
}
#[godot_api]
impl TweenDirectorResource {
    #[func]
    pub fn set_tween_resource(&mut self, value: Option<Gd<PhantomCameraTween>>) {
        self.tween_resource = value;
    }
    #[func]
    pub fn get_tween_resource(&self) -> Option<Gd<PhantomCameraTween>> {
        self.tween_resource.clone()
    }
    #[func]
    pub fn set_from_type(&mut self, value: i64) {
        self.from_type = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_from_type(&self) -> i64 {
        self.from_type
    }
    #[func]
    pub fn set_from_phantom_cameras(&mut self, value: Array<NodePath>) {
        self.from_phantom_cameras = value;
    }
    #[func]
    pub fn get_from_phantom_cameras(&self) -> Array<NodePath> {
        self.from_phantom_cameras.clone()
    }
    #[func]
    pub fn set_from_tween_resources(&mut self, value: Array<Gd<PhantomCameraTween>>) {
        self.from_tween_resources = value;
    }
    #[func]
    pub fn get_from_tween_resources(&self) -> Array<Gd<PhantomCameraTween>> {
        self.from_tween_resources.clone()
    }
    #[func]
    pub fn set_to_type(&mut self, value: i64) {
        self.to_type = value;
        self.base_mut().notify_property_list_changed();
    }
    #[func]
    pub fn get_to_type(&self) -> i64 {
        self.to_type
    }
    #[func]
    pub fn set_to_phantom_cameras(&mut self, value: Array<NodePath>) {
        self.to_phantom_cameras = value;
    }
    #[func]
    pub fn get_to_phantom_cameras(&self) -> Array<NodePath> {
        self.to_phantom_cameras.clone()
    }
    #[func]
    pub fn set_to_tween_resources(&mut self, value: Array<Gd<PhantomCameraTween>>) {
        self.to_tween_resources = value;
    }
    #[func]
    pub fn get_to_tween_resources(&self) -> Array<Gd<PhantomCameraTween>> {
        self.to_tween_resources.clone()
    }
}
