//! Pure, bounded edits on the workshop's nested step tree.
use crate::model::Step;

#[derive(Clone, Debug)]
pub enum Edit {
    Duplicate(Vec<usize>),
    Move(Vec<usize>, usize),  // destination index in the same sibling list
    Insert(Vec<usize>, Step), // insertion path ends in an index, possibly len
    Toggle(Vec<usize>),
}

fn children<'a>(steps: &'a mut Vec<Step>, parent: &[usize]) -> Option<&'a mut Vec<Step>> {
    if parent.is_empty() {
        return Some(steps);
    }
    let step = steps.get_mut(parent[0])?.active_mut();
    let (&branch, rest) = parent[1..].split_first()?;
    let list = match step {
        Step::Repeat { steps, .. } if branch == 0 => steps,
        Step::Toggle { a, b } => match branch {
            0 => a,
            1 => b,
            _ => return None,
        },
        Step::IfCondition {
            then_steps,
            else_steps,
            ..
        }
        | Step::IfWindow {
            then_steps,
            else_steps,
            ..
        }
        | Step::IfVariable {
            then_steps,
            else_steps,
            ..
        } => match branch {
            0 => then_steps,
            1 => else_steps,
            _ => return None,
        },
        _ => return None,
    };
    children(list, rest)
}

pub fn get<'a>(steps: &'a mut Vec<Step>, path: &[usize]) -> Option<&'a mut Step> {
    let (&index, parent) = path.split_last()?;
    children(steps, parent)?.get_mut(index)
}

/// Returns the new selection only if the entire edit succeeded; failures leave the tree intact.
pub fn apply(steps: &mut Vec<Step>, edit: Edit) -> Option<Vec<usize>> {
    let (path, kind) = match &edit {
        Edit::Duplicate(p) => (p, 0),
        Edit::Move(p, _) => (p, 2),
        Edit::Insert(p, _) => (p, 3),
        Edit::Toggle(p) => (p, 4),
    };
    let (&index, parent) = path.split_last()?;
    let list = children(steps, parent)?;
    if kind == 3 {
        if index > list.len() || list.len() >= 256 {
            return None;
        }
    } else if index >= list.len() {
        return None;
    }
    let mut selected = path.to_vec();
    match edit {
        Edit::Duplicate(_) if list.len() < 256 => {
            list.insert(index + 1, list[index].clone());
            selected.pop();
            selected.push(index + 1);
        }
        Edit::Move(_, target) if target < list.len() && target != index => {
            let step = list.remove(index);
            list.insert(target, step);
            selected.pop();
            selected.push(target);
        }
        Edit::Insert(_, step) => list.insert(index, step),
        Edit::Toggle(_) => {
            let old = list.remove(index);
            list.insert(
                index,
                match old {
                    Step::Disabled { step } => *step,
                    step => Step::Disabled {
                        step: Box::new(step),
                    },
                },
            );
        }
        _ => return None,
    }
    Some(selected)
}

pub fn remove(steps: &mut Vec<Step>, path: &[usize]) -> bool {
    let Some((&index, parent)) = path.split_last() else {
        return false;
    };
    let Some(list) = children(steps, parent) else {
        return false;
    };
    if index >= list.len() {
        return false;
    }
    list.remove(index);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn delay(n: u64) -> Step {
        Step::Delay { ms: n }
    }
    #[test]
    fn nested_edits_and_bounds() {
        let mut tree = vec![
            Step::Repeat {
                count: 2,
                steps: vec![delay(1), delay(2)],
            },
            delay(3),
        ];
        assert_eq!(
            apply(&mut tree, Edit::Duplicate(vec![0, 0, 0])),
            Some(vec![0, 0, 1])
        );
        assert_eq!(
            apply(&mut tree, Edit::Move(vec![0, 0, 1], 2)),
            Some(vec![0, 0, 2])
        );
        assert_eq!(
            apply(&mut tree, Edit::Toggle(vec![0, 0, 2])),
            Some(vec![0, 0, 2])
        );
        assert!(get(&mut tree, &[0, 0, 2]).unwrap().is_disabled());
        assert!(remove(&mut tree, &[0, 0, 2]));
        assert_eq!(
            apply(&mut tree, Edit::Insert(vec![0, 0, 2], delay(4))),
            Some(vec![0, 0, 2])
        );
        let before = tree.clone();
        for path in [
            vec![],
            vec![8],
            vec![0, 9, 0],
            vec![1, 0, 0],
            vec![0, 0, 99],
        ] {
            assert!(apply(&mut tree, Edit::Duplicate(path.clone())).is_none());
            assert!(!remove(&mut tree, &path));
        }
        assert!(apply(&mut tree, Edit::Move(vec![0, 0, 0], 99)).is_none());
        assert!(apply(&mut tree, Edit::Insert(vec![0, 0, 99], delay(9))).is_none());
        assert_eq!(tree, before);
    }
    #[test]
    fn branches_are_distinct() {
        let mut tree = vec![Step::IfVariable {
            name: "x".into(),
            value: "1".into(),
            then_steps: vec![delay(1)],
            else_steps: vec![delay(2)],
        }];
        assert!(remove(&mut tree, &[0, 1, 0]));
        assert!(get(&mut tree, &[0, 0, 0]).is_some());
        assert!(get(&mut tree, &[0, 1, 0]).is_none());
    }
}
