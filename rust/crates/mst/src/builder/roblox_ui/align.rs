//! How far each selected object moves when the selection is aligned, decided from plain
//! bounding boxes so the rule is testable without a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignMode {
    Min,
    Center,
    Max,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeTo {
    SelectionBounds,
    Active(usize),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}
impl Bounds {
    pub fn union(boxes: &[Bounds]) -> Option<Bounds> {
        let first = *boxes.first()?;
        Some(boxes.iter().skip(1).fold(first, |a, b| Bounds {
            lo: std::array::from_fn(|k| a.lo[k].min(b.lo[k])),
            hi: std::array::from_fn(|k| a.hi[k].max(b.hi[k])),
        }))
    }
    fn anchor(&self, mode: AlignMode, k: usize) -> f32 {
        match mode {
            AlignMode::Min => self.lo[k],
            AlignMode::Center => 0.5 * (self.lo[k] + self.hi[k]),
            AlignMode::Max => self.hi[k],
        }
    }
}
pub type Frame = [[f32; 3]; 3];
pub const WORLD: Frame = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn frame_bounds(corners: &[[f32; 3]], frame: Frame) -> Option<Bounds> {
    let first = corners.first()?;
    let start: [f32; 3] = std::array::from_fn(|k| dot(*first, frame[k]));
    Some(corners.iter().fold(Bounds { lo: start, hi: start }, |b, c| Bounds {
        lo: std::array::from_fn(|k| b.lo[k].min(dot(*c, frame[k]))),
        hi: std::array::from_fn(|k| b.hi[k].max(dot(*c, frame[k]))),
    }))
}
pub fn offset_in_world(offset: [f32; 3], frame: Frame) -> [f32; 3] {
    std::array::from_fn(|i| (0..3).map(|k| offset[k] * frame[k][i]).sum())
}
pub fn align_offsets(
    boxes: &[Bounds],
    mode: AlignMode,
    axes: [bool; 3],
    relative: RelativeTo,
) -> Vec<[f32; 3]> {
    let reference = match relative {
        RelativeTo::SelectionBounds => Bounds::union(boxes),
        RelativeTo::Active(i) => boxes.get(i).copied(),
    };
    let Some(reference) = reference else {
        return vec![[0.0; 3]; boxes.len()];
    };
    boxes
        .iter()
        .map(|b| {
            std::array::from_fn(|k| {
                if axes[k] {
                    reference.anchor(mode, k) - b.anchor(mode, k)
                } else {
                    0.0
                }
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn b(lo: [f32; 3], hi: [f32; 3]) -> Bounds {
        Bounds { lo, hi }
    }
    fn two() -> Vec<Bounds> {
        vec![
            b([0.0, 0.0, 0.0], [2.0, 1.0, 1.0]),
            b([4.0, 3.0, -1.0], [8.0, 5.0, 1.0]),
        ]
    }
    #[test]
    fn center_on_x_meets_in_the_middle_of_the_selection() {
        let o = align_offsets(&two(), AlignMode::Center, [true, false, false], RelativeTo::SelectionBounds);
        assert_eq!(o, vec![[3.0, 0.0, 0.0], [-2.0, 0.0, 0.0]]);
    }
    #[test]
    fn min_and_max_line_up_the_matching_faces() {
        let lo = align_offsets(&two(), AlignMode::Min, [false, true, false], RelativeTo::SelectionBounds);
        assert_eq!(lo, vec![[0.0, 0.0, 0.0], [0.0, -3.0, 0.0]]);
        let hi = align_offsets(&two(), AlignMode::Max, [true, true, true], RelativeTo::SelectionBounds);
        assert_eq!(hi, vec![[6.0, 4.0, 0.0], [0.0, 0.0, 0.0]]);
    }
    #[test]
    fn the_active_object_stays_put() {
        let o = align_offsets(&two(), AlignMode::Center, [true, true, true], RelativeTo::Active(1));
        assert_eq!(o[1], [0.0, 0.0, 0.0]);
        assert_eq!(o[0], [5.0, 3.5, -0.5]);
    }
    #[test]
    fn world_frame_bounds_are_the_plain_box() {
        let corners = [[0.0, 0.0, 0.0], [2.0, 1.0, 3.0], [1.0, -1.0, 0.5]];
        assert_eq!(frame_bounds(&corners, WORLD), Some(b([0.0, -1.0, 0.0], [2.0, 1.0, 3.0])));
        assert_eq!(frame_bounds(&[], WORLD), None);
        assert_eq!(offset_in_world([1.0, 2.0, 3.0], WORLD), [1.0, 2.0, 3.0]);
    }
    #[test]
    fn local_frame_aligns_along_the_active_objects_turned_axes() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let turned: Frame = [[h, 0.0, -h], [0.0, 1.0, 0.0], [h, 0.0, h]];
        let a = frame_bounds(&[[0.0, 0.0, 0.0], [h, 0.0, -h]], turned).unwrap();
        let c = frame_bounds(&[[3.0 * h, 0.0, -3.0 * h], [4.0 * h, 0.0, -4.0 * h]], turned).unwrap();
        let o = align_offsets(&[a, c], AlignMode::Min, [true, false, false], RelativeTo::Active(0));
        assert_eq!(o[0], [0.0; 3]);
        let w = offset_in_world(o[1], turned);
        assert!((w[0] + 3.0 * h).abs() < 1e-5 && (w[2] - 3.0 * h).abs() < 1e-5 && w[1].abs() < 1e-6);
    }
    #[test]
    fn no_axes_or_a_missing_active_object_moves_nothing() {
        let none = align_offsets(&two(), AlignMode::Center, [false; 3], RelativeTo::SelectionBounds);
        assert!(none.iter().all(|o| *o == [0.0; 3]));
        let gone = align_offsets(&two(), AlignMode::Center, [true; 3], RelativeTo::Active(7));
        assert!(gone.iter().all(|o| *o == [0.0; 3]));
        assert!(align_offsets(&[], AlignMode::Min, [true; 3], RelativeTo::SelectionBounds).is_empty());
    }
}
