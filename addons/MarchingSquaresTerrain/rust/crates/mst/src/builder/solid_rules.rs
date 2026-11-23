//! The rules of Roblox-style solid modelling, kept free of engine types so they can be
//! tested without a running editor: which parts take part, in what order they combine,
//! what the result is called, and how a split puts the parts back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SolidOp {
    Union,
    Intersect,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Union,
    Intersect,
    Subtract,
}
#[derive(PartialEq, Eq, Debug)]
pub struct SolidPlan {
    pub base: usize,
    pub steps: Vec<(usize, Step)>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PartRole {
    pub negated: bool,
    pub operand: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Candidate {
    pub role: PartRole,
    pub is_scene_root: bool,
    pub owned_by_scene: bool,
    pub has_shape: bool,
}
pub const NEED_TWO: &str = "Select two or more builder meshes.";
pub const NEED_POSITIVE: &str = "Select at least one part that is not negated.";
pub const NEED_WATERTIGHT: &str =
    "Every selected part must be watertight (Select holes shows the open edges).";
pub const NEED_COMBINED: &str = "Select a Union or Intersection to separate.";
pub const NEED_PART: &str = "Select one or more builder meshes.";
impl PartRole {
    pub fn takes_no_part_in_world(self) -> bool {
        self.negated || self.operand
    }
}
impl SolidOp {
    pub fn result_name(self) -> &'static str {
        match self {
            SolidOp::Union => "Union",
            SolidOp::Intersect => "Intersection",
        }
    }
}
pub fn is_selectable_part(c: Candidate) -> bool {
    !c.role.operand && !c.is_scene_root && c.owned_by_scene && c.has_shape
}
pub fn negate_turns_on(negated: &[bool]) -> bool {
    !negated.iter().all(|n| *n)
}
pub fn plan_solid(op: SolidOp, negated: &[bool]) -> Result<SolidPlan, &'static str> {
    if negated.len() < 2 {
        return Err(NEED_TWO);
    }
    let positives: Vec<usize> = (0..negated.len()).filter(|i| !negated[*i]).collect();
    let Some((&base, rest)) = positives.split_first() else {
        return Err(NEED_POSITIVE);
    };
    let join = match op {
        SolidOp::Union => Step::Union,
        SolidOp::Intersect => Step::Intersect,
    };
    let steps = rest
        .iter()
        .map(|i| (*i, join))
        .chain(
            (0..negated.len())
                .filter(|i| negated[*i])
                .map(|i| (i, Step::Subtract)),
        )
        .collect();
    Ok(SolidPlan { base, steps })
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UndoMove {
    ParkResultLast,
    Reinsert { part: usize, index: i32 },
    RemoveResult,
}
pub fn combine_undo_plan<K: Ord + Copy>(homes: &[(K, i32)]) -> Vec<UndoMove> {
    let mut order: Vec<usize> = (0..homes.len()).collect();
    order.sort_by_key(|i| homes[*i]);
    std::iter::once(UndoMove::ParkResultLast)
        .chain(order.into_iter().map(|part| UndoMove::Reinsert {
            part,
            index: homes[part].1,
        }))
        .chain(std::iter::once(UndoMove::RemoveResult))
        .collect()
}
pub fn separated_indices(combined_index: i32, parts: usize) -> Vec<i32> {
    (0..parts as i32).map(|k| combined_index + k).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn combine_then_undo(start: &[char], parts: &[(char, i32)], base: char) -> Vec<char> {
        let mut kids = start.to_vec();
        let at = kids.iter().position(|k| *k == base).unwrap();
        kids.insert(at, 'R');
        kids.retain(|k| !parts.iter().any(|p| p.0 == *k));
        let homes: Vec<(u8, i32)> = parts.iter().map(|p| (0, p.1)).collect();
        for m in combine_undo_plan(&homes) {
            match m {
                UndoMove::ParkResultLast => {
                    kids.retain(|k| *k != 'R');
                    kids.push('R');
                }
                UndoMove::Reinsert { part, index } => {
                    kids.insert(index as usize, parts[part].0);
                }
                UndoMove::RemoveResult => kids.retain(|k| *k != 'R'),
            }
        }
        kids
    }
    #[test]
    fn union_joins_positives_in_selection_order_then_subtracts_negatives() {
        let plan = plan_solid(SolidOp::Union, &[false, true, false, true, false]).unwrap();
        assert_eq!(plan.base, 0);
        assert_eq!(
            plan.steps,
            vec![
                (2, Step::Union),
                (4, Step::Union),
                (1, Step::Subtract),
                (3, Step::Subtract)
            ]
        );
    }
    #[test]
    fn base_is_first_positive_even_when_a_negated_part_was_picked_first() {
        let plan = plan_solid(SolidOp::Union, &[true, false]).unwrap();
        assert_eq!(plan.base, 1);
        assert_eq!(plan.steps, vec![(0, Step::Subtract)]);
    }
    #[test]
    fn intersect_intersects_positives_and_still_subtracts_negatives() {
        let plan = plan_solid(SolidOp::Intersect, &[false, false, true]).unwrap();
        assert_eq!(plan.base, 0);
        assert_eq!(plan.steps, vec![(1, Step::Intersect), (2, Step::Subtract)]);
    }
    #[test]
    fn every_part_appears_exactly_once() {
        let flags = [true, false, true, true, false, false];
        let plan = plan_solid(SolidOp::Union, &flags).unwrap();
        let mut seen: Vec<usize> = plan.steps.iter().map(|s| s.0).collect();
        seen.push(plan.base);
        seen.sort();
        assert_eq!(seen, (0..flags.len()).collect::<Vec<_>>());
    }
    #[test]
    fn fewer_than_two_parts_is_refused() {
        assert_eq!(plan_solid(SolidOp::Union, &[false]), Err(NEED_TWO));
        assert_eq!(plan_solid(SolidOp::Intersect, &[]), Err(NEED_TWO));
    }
    #[test]
    fn only_negated_parts_is_refused() {
        assert_eq!(plan_solid(SolidOp::Union, &[true, true]), Err(NEED_POSITIVE));
    }
    #[test]
    fn results_are_named_like_studio() {
        assert_eq!(SolidOp::Union.result_name(), "Union");
        assert_eq!(SolidOp::Intersect.result_name(), "Intersection");
    }
    #[test]
    fn negate_turns_on_unless_every_part_is_already_negated() {
        assert!(negate_turns_on(&[false, true]));
        assert!(negate_turns_on(&[false]));
        assert!(!negate_turns_on(&[true, true]));
    }
    #[test]
    fn cutters_and_hidden_operands_stay_out_of_the_world() {
        let r = |negated, operand| PartRole { negated, operand };
        assert!(!r(false, false).takes_no_part_in_world());
        assert!(r(true, false).takes_no_part_in_world());
        assert!(r(false, true).takes_no_part_in_world());
    }
    #[test]
    fn hidden_operands_root_and_unowned_nodes_are_not_selectable_parts() {
        let ok = Candidate {
            role: PartRole::default(),
            is_scene_root: false,
            owned_by_scene: true,
            has_shape: true,
        };
        assert!(is_selectable_part(ok));
        assert!(is_selectable_part(Candidate {
            role: PartRole {
                negated: true,
                operand: false
            },
            ..ok
        }));
        assert!(!is_selectable_part(Candidate {
            role: PartRole {
                negated: false,
                operand: true
            },
            ..ok
        }));
        assert!(!is_selectable_part(Candidate {
            is_scene_root: true,
            ..ok
        }));
        assert!(!is_selectable_part(Candidate {
            owned_by_scene: false,
            ..ok
        }));
        assert!(!is_selectable_part(Candidate {
            has_shape: false,
            ..ok
        }));
    }
    #[test]
    fn undo_puts_parts_back_at_their_old_positions_whatever_the_base() {
        let start = ['A', 'B', 'C', 'D'];
        for (parts, base) in [
            (vec![('A', 0), ('B', 1)], 'A'),
            (vec![('A', 0), ('C', 2)], 'C'),
            (vec![('C', 2), ('A', 0)], 'C'),
            (vec![('B', 1), ('D', 3)], 'B'),
            (vec![('D', 3), ('B', 1), ('A', 0)], 'D'),
        ] {
            assert_eq!(combine_then_undo(&start, &parts, base), start.to_vec());
        }
    }
    #[test]
    fn undo_parks_the_result_before_any_part_returns_and_removes_it_last() {
        let plan = combine_undo_plan(&[(0u8, 3), (0, 1)]);
        assert_eq!(plan.first(), Some(&UndoMove::ParkResultLast));
        assert_eq!(plan.last(), Some(&UndoMove::RemoveResult));
        assert_eq!(
            plan[1..3],
            [
                UndoMove::Reinsert { part: 1, index: 1 },
                UndoMove::Reinsert { part: 0, index: 3 }
            ]
        );
    }
    #[test]
    fn separated_parts_take_the_combined_solids_place_in_order() {
        assert_eq!(separated_indices(3, 3), vec![3, 4, 5]);
        assert!(separated_indices(0, 0).is_empty());
    }
}
