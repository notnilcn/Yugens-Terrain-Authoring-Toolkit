//! Projects world points to viewport pixels only when the engine can, so editor overlays never ask
//! the camera to divide by a zero depth.
use godot::classes::Camera3D;
use godot::classes::camera_3d::ProjectionType;
use godot::prelude::*;

pub fn depth_projectable(view_z: f32, near: f32, orthogonal: bool) -> bool {
    if !view_z.is_finite() {
        return false;
    }
    orthogonal || -view_z >= near.max(f32::EPSILON)
}

pub fn view_depth(camera_transform: Transform3D, world: Vector3) -> Option<f32> {
    let basis = camera_transform.basis;
    let det = basis.determinant();
    if !det.is_finite() || det.abs() <= f32::EPSILON {
        return None;
    }
    let z = (camera_transform.affine_inverse() * world).z;
    z.is_finite().then_some(z)
}

pub fn project(camera: &Gd<Camera3D>, world: Vector3) -> Option<Vector2> {
    if !camera.is_instance_valid() || !camera.is_inside_tree() {
        return None;
    }
    let z = view_depth(camera.get_camera_transform(), world)?;
    let orthogonal = camera.get_projection() == ProjectionType::ORTHOGONAL;
    depth_projectable(z, camera.get_near(), orthogonal).then(|| camera.unproject_position(world))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_on_the_camera_plane_is_rejected() {
        assert!(!depth_projectable(0.0, 0.05, false));
        assert!(!depth_projectable(-0.0, 0.05, false));
    }

    #[test]
    fn point_behind_or_inside_near_is_rejected_for_perspective() {
        assert!(!depth_projectable(3.0, 0.05, false));
        assert!(!depth_projectable(-0.01, 0.05, false));
        assert!(depth_projectable(-0.05, 0.05, false));
        assert!(depth_projectable(-10.0, 0.05, false));
    }

    #[test]
    fn zero_near_still_rejects_the_camera_plane() {
        assert!(!depth_projectable(0.0, 0.0, false));
        assert!(depth_projectable(-1.0, 0.0, false));
    }

    #[test]
    fn orthogonal_accepts_any_finite_depth() {
        assert!(depth_projectable(0.0, 0.05, true));
        assert!(depth_projectable(5.0, 0.05, true));
        assert!(!depth_projectable(f32::NAN, 0.05, true));
    }

    #[test]
    fn camera_at_the_point_has_zero_depth() {
        let cam = Transform3D::new(Basis::IDENTITY, Vector3::new(22.0, 5.0, 33.785));
        let z = view_depth(cam, Vector3::new(22.0, 5.0, 33.785));
        assert_eq!(z, Some(0.0));
        assert!(!depth_projectable(z.unwrap_or(1.0), 0.1, false));
    }

    #[test]
    fn collapsed_camera_basis_is_not_projectable() {
        let cam = Transform3D::new(
            Basis::from_cols(Vector3::ZERO, Vector3::ZERO, Vector3::ZERO),
            Vector3::ZERO,
        );
        assert_eq!(view_depth(cam, Vector3::new(1.0, 2.0, -3.0)), None);
        let nan = Transform3D::new(Basis::IDENTITY, Vector3::new(f32::NAN, 0.0, 0.0));
        assert_eq!(view_depth(nan, Vector3::new(1.0, 2.0, -3.0)), None);
    }

    #[test]
    fn point_in_front_has_negative_depth() {
        let cam = Transform3D::new(Basis::IDENTITY, Vector3::ZERO);
        assert_eq!(view_depth(cam, Vector3::new(0.0, 0.0, -4.0)), Some(-4.0));
    }
}
