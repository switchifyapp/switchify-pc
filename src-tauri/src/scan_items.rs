use crate::{
    scan_preferences::{Direction, Pattern, Resolved},
    scan_tree::{Navigator, Node, Selection},
    scanning::{Action, Interval},
};

#[derive(Clone, Copy)]
pub struct Policy {
    pub reset_passes_on_step: bool,
    pub resume_at_root: bool,
}
impl Policy {
    pub const MENU: Self = Self {
        reset_passes_on_step: false,
        resume_at_root: false,
    };
    pub const KEYBOARD: Self = Self {
        reset_passes_on_step: true,
        resume_at_root: true,
    };
}

pub struct ItemScanner<T> {
    pub nav: Navigator<T>,
    pub options: Resolved,
    pub suspended: bool,
    interval: Interval,
    cycles: usize,
    forward: bool,
    policy: Policy,
    pending: bool,
    revision: u64,
}
impl<T: Clone> ItemScanner<T> {
    pub fn new(nodes: Vec<Node<T>>, policy: Policy) -> Self {
        Self {
            nav: Navigator::new(nodes),
            options: Resolved::default(),
            suspended: false,
            interval: Interval::default(),
            cycles: 0,
            forward: true,
            policy,
            pending: false,
            revision: 0,
        }
    }
    pub fn nodes(rows: &[Vec<T>]) -> Vec<Node<T>> {
        rows.iter()
            .enumerate()
            .map(|(index, row)| {
                if let [only] = row.as_slice() {
                    Node::Leaf(only.clone())
                } else {
                    Node::Group {
                        id: format!("row-{index}"),
                        children: row.iter().cloned().map(Node::Leaf).collect(),
                    }
                }
            })
            .collect()
    }
    #[cfg(test)]
    pub fn rows(rows: &[Vec<T>], policy: Policy) -> Self {
        Self::new(Self::nodes(rows), policy)
    }
    pub fn configured_rows(rows: &[Vec<T>], policy: Policy, options: Resolved) -> Self {
        let nodes = if options.pattern == Pattern::Linear {
            rows.iter().flatten().cloned().map(Node::Leaf).collect()
        } else {
            Self::nodes(rows)
        };
        let mut scanner = Self::new(nodes, policy);
        scanner.options = options;
        scanner.restart();
        scanner
    }
    pub fn row_scan(&self) -> bool {
        self.options.pattern == Pattern::Grouped && self.nav.path().is_empty()
    }
    pub fn position(&self, rows: &[Vec<T>]) -> (usize, Option<usize>) {
        if self.options.pattern == Pattern::Linear {
            let mut index = self.nav.index();
            for (r, row) in rows.iter().enumerate() {
                if index < row.len() {
                    return (r, Some(index));
                }
                index -= row.len();
            }
            (0, None)
        } else {
            (
                self.nav.path().first().copied().unwrap_or(self.nav.index()),
                if self.nav.path().is_empty() || self.nav.escaping() {
                    None
                } else {
                    Some(self.nav.index())
                },
            )
        }
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn begin_activation(&mut self) -> Option<u64> {
        if self.pending || self.suspended {
            return None;
        }
        self.revision = self.revision.wrapping_add(1);
        self.pending = true;
        Some(self.revision)
    }
    pub fn complete_activation(&mut self, revision: u64) -> bool {
        if !self.pending || revision != self.revision {
            return false;
        }
        self.pending = false;
        true
    }
    pub fn restart_interval(&mut self) {
        self.interval.reset();
        self.cycles = 0;
        self.suspended = false;
    }
    pub fn reset_clock(&mut self) {
        self.interval.reset();
    }
    pub fn restart(&mut self) {
        self.pending = false;
        self.nav.reset();
        self.forward = self.options.direction == Direction::Forward;
        if !self.forward {
            self.nav.start_at_end();
        }
        self.restart_interval();
    }
    pub fn skip(&mut self) {
        self.nav.step(self.forward);
    }
    pub fn skip_automatic(&mut self) {
        if self.nav.step(self.forward) {
            self.cycles += 1;
            self.suspended = self.options.exhausted(self.cycles);
        }
    }
    pub fn advance(&mut self, ms: u64, period: u64) -> bool {
        if self.pending || self.suspended || !self.interval.elapsed(ms, period) {
            return false;
        }
        self.skip_automatic();
        true
    }
    pub fn handle(&mut self, action: Action) -> Option<T> {
        if self.pending {
            return None;
        }
        if self.suspended {
            if action == Action::Select {
                if self.policy.resume_at_root {
                    self.restart();
                } else {
                    self.restart_interval();
                }
            }
            return None;
        }
        match action {
            Action::Select => {
                self.restart_interval();
                match self.nav.select() {
                    Selection::Leaf(value) => return Some(value),
                    Selection::Entered => {
                        self.forward = self.options.direction == Direction::Forward;
                        if !self.forward {
                            self.nav.start_at_end();
                        }
                    }
                    Selection::Escaped => {
                        self.forward = self.options.direction == Direction::Forward
                    }
                    Selection::None => {}
                }
            }
            Action::Next | Action::Back => {
                self.reset_clock();
                if self.policy.reset_passes_on_step {
                    self.cycles = 0;
                }
                self.forward = action == Action::Next;
                self.skip();
            }
            Action::Reverse => {
                self.forward = !self.forward;
                self.restart_interval();
            }
            _ => {}
        }
        None
    }
}

impl<T: Clone + PartialEq> ItemScanner<T> {
    pub fn replace(&mut self, nodes: Vec<Node<T>>) -> bool {
        fn leaves<T>(nodes: Vec<Node<T>>, output: &mut Vec<Node<T>>) {
            for node in nodes {
                match node {
                    Node::Leaf(_) => output.push(node),
                    Node::Branch(children) | Node::Group { children, .. } => {
                        leaves(children, output)
                    }
                }
            }
        }
        let nodes = if self.options.pattern == Pattern::Linear {
            let mut flat = vec![];
            leaves(nodes, &mut flat);
            flat
        } else {
            nodes
        };
        let Some(preserved) = self.nav.replace(nodes) else {
            return false;
        };
        self.revision = self.revision.wrapping_add(1);
        self.pending = false;
        if !preserved {
            self.interval.reset();
            self.cycles = 0;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner(policy: Policy) -> ItemScanner<&'static str> {
        ItemScanner::rows(&[vec!["copy", "paste"], vec!["close"]], policy)
    }

    #[test]
    fn nested_back_restores_parent_and_gives_it_a_full_interval() {
        let mut scan = scanner(Policy::MENU);
        assert_eq!(scan.handle(Action::Select), None);
        scan.advance(499, 500);
        scan.handle(Action::Back);
        assert!(scan.nav.escaping());
        assert_eq!(scan.handle(Action::Select), None);
        assert!(scan.nav.path().is_empty());
        assert_eq!(scan.nav.index(), 0);
        assert!(!scan.advance(499, 500));
        assert!(scan.advance(1, 500));
        assert_eq!(scan.handle(Action::Select), Some("close"));
    }

    #[test]
    fn suspension_consumes_resume_without_selecting() {
        for policy in [Policy::MENU, Policy::KEYBOARD] {
            let mut scan = scanner(policy);
            for _ in 0..6 {
                scan.advance(500, 500);
            }
            assert!(scan.suspended);
            assert_eq!(scan.handle(Action::Next), None);
            assert!(scan.suspended);
            assert_eq!(scan.handle(Action::Select), None);
            assert!(!scan.suspended);
            assert!(!scan.advance(499, 500));
        }
    }

    #[test]
    fn reverse_and_manual_steps_restart_the_clock() {
        let mut scan = scanner(Policy::MENU);
        scan.advance(499, 500);
        scan.handle(Action::Reverse);
        assert!(!scan.advance(499, 500));
        assert!(scan.advance(1, 500));
        assert_eq!(scan.handle(Action::Select), Some("close"));
        scan.handle(Action::Next);
        assert_eq!(scan.nav.index(), 0);
        assert!(!scan.advance(499, 500));
    }

    #[test]
    fn adapters_preserve_their_existing_resume_policy() {
        for policy in [Policy::MENU, Policy::KEYBOARD] {
            let mut scan = scanner(policy);
            scan.handle(Action::Select);
            scan.handle(Action::Next);
            scan.suspended = true;
            scan.handle(Action::Select);
            assert_eq!(scan.nav.path().is_empty(), policy.resume_at_root);
            assert_eq!(scan.nav.index(), usize::from(!policy.resume_at_root));
        }
    }
    fn group(id: &str, keys: &[&'static str]) -> Node<&'static str> {
        Node::Group {
            id: id.into(),
            children: keys.iter().copied().map(Node::Leaf).collect(),
        }
    }

    #[test]
    fn reordered_groups_and_keys_keep_identity_and_elapsed_time() {
        let mut scan = ItemScanner::new(
            vec![
                group("edit", &["copy", "paste"]),
                group("window", &["close", "minimise"]),
            ],
            Policy::MENU,
        );
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        scan.advance(400, 500);
        assert!(scan.replace(vec![
            group("window", &["close", "minimise"]),
            group("edit", &["paste", "copy", "undo"])
        ]));
        assert_eq!(scan.nav.path(), &[1]);
        assert_eq!(scan.nav.index(), 0);
        assert!(!scan.advance(99, 500));
        assert!(scan.advance(1, 500));
        assert_eq!(scan.handle(Action::Select), Some("copy"));
    }

    #[test]
    fn missing_item_returns_to_safe_parent_with_a_fresh_interval() {
        let mut scan = ItemScanner::new(
            vec![group("edit", &["copy", "paste", "undo"])],
            Policy::MENU,
        );
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        scan.advance(499, 500);
        scan.replace(vec![group("edit", &["copy", "undo"])]);
        assert!(!scan.advance(499, 500));
        assert_eq!(scan.handle(Action::Select), Some("copy"));
        scan.replace(vec![]);
        assert_eq!(scan.handle(Action::Select), None);
    }

    #[test]
    fn unchanged_content_does_not_cancel_activation_or_reset_interval() {
        let nodes = vec![group("edit", &["copy", "paste"])];
        let mut scan = ItemScanner::new(nodes.clone(), Policy::MENU);
        scan.handle(Action::Select);
        scan.advance(499, 500);
        assert!(!scan.replace(nodes.clone()));
        assert!(scan.advance(1, 500));
        let token = scan.begin_activation().unwrap();
        assert!(!scan.replace(nodes));
        assert!(scan.complete_activation(token));
        assert!(!scan.complete_activation(token));
    }

    #[test]
    fn pending_activation_blocks_steps_and_selection_and_stale_completion() {
        let mut scan = scanner(Policy::KEYBOARD);
        let first = scan.begin_activation().unwrap();
        assert_eq!(scan.begin_activation(), None);
        assert_eq!(scan.handle(Action::Select), None);
        assert_eq!(scan.handle(Action::Next), None);
        assert!(!scan.advance(500, 500));
        scan.restart();
        assert!(!scan.complete_activation(first));
        let second = scan.begin_activation().unwrap();
        assert_ne!(first, second);
        assert!(!scan.complete_activation(first));
        scan.replace(vec![Node::Leaf("save")]);
        assert!(!scan.complete_activation(second));
        assert_eq!(scan.handle(Action::Select), Some("save"));
    }
    #[test]
    fn back_remains_back_when_the_hidden_edge_item_disappears() {
        for forward in [false, true] {
            let mut scan = ItemScanner::new(
                vec![group("edit", &["copy", "paste", "undo"])],
                Policy::MENU,
            );
            scan.handle(Action::Select);
            if forward {
                scan.handle(Action::Next);
                scan.handle(Action::Next);
            }
            scan.handle(if forward { Action::Next } else { Action::Back });
            assert!(scan.nav.escaping());
            scan.advance(400, 500);
            scan.replace(vec![
                group("other", &["save", "close"]),
                group("edit", &["paste", "cut"]),
            ]);
            assert!(scan.nav.escaping());
            assert_eq!(scan.nav.path(), &[1]);
            assert!(!scan.advance(99, 500));
            assert_eq!(scan.handle(Action::Select), None);
            assert!(scan.nav.path().is_empty());
            assert_eq!(scan.nav.index(), 1);
        }
    }
    #[test]
    fn changing_content_never_resumes_a_suspended_scanner() {
        let mut scan = scanner(Policy::MENU);
        scan.handle(Action::Next);
        scan.suspended = true;
        scan.replace(vec![Node::Leaf("save")]);
        assert!(scan.suspended);
        assert!(!scan.advance(500, 500));
        assert_eq!(scan.handle(Action::Select), None);
        assert!(!scan.suspended);
        assert_eq!(scan.handle(Action::Select), Some("save"));
    }
    #[test]
    fn identified_group_survives_shrinking_to_one_item_while_back_is_selected() {
        for forward in [false, true] {
            let mut scan = ItemScanner::new(vec![group("edit", &["copy", "paste"])], Policy::MENU);
            scan.handle(Action::Select);
            if forward {
                scan.handle(Action::Next);
            }
            scan.handle(if forward { Action::Next } else { Action::Back });
            scan.replace(vec![group("edit", &["paste"])]);
            assert!(scan.nav.escaping());
            assert_eq!(scan.handle(Action::Select), None);
            assert!(scan.nav.path().is_empty());
            assert_eq!(scan.handle(Action::Select), None);
            assert_eq!(scan.handle(Action::Select), Some("paste"));
        }
    }

    #[test]
    fn selected_leaf_survives_its_group_shrinking_to_one_child() {
        let mut scan = ItemScanner::new(vec![group("edit", &["copy", "paste"])], Policy::MENU);
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        scan.replace(vec![group("edit", &["paste"])]);
        assert_eq!(scan.nav.path(), &[0]);
        assert_eq!(scan.handle(Action::Select), Some("paste"));
    }

    #[test]
    fn fixed_single_item_rows_keep_direct_activation() {
        let mut scan = ItemScanner::rows(&[vec!["close"]], Policy::MENU);
        assert_eq!(scan.handle(Action::Select), Some("close"));
    }
    #[test]
    fn linear_scanning_visits_each_item_once_and_reports_its_visual_position() {
        let rows = vec![vec!["copy", "paste"], vec!["save"]];
        let mut scan = ItemScanner::configured_rows(
            &rows,
            Policy::MENU,
            Resolved {
                pattern: Pattern::Linear,
                ..Default::default()
            },
        );
        for (r, c, expected) in [(0, 0, "copy"), (0, 1, "paste"), (1, 0, "save")] {
            assert!(!scan.row_scan());
            assert_eq!(scan.position(&rows), (r, Some(c)));
            assert_eq!(scan.handle(Action::Select), Some(expected));
            scan.handle(Action::Next);
        }
        assert_eq!(scan.position(&rows), (0, Some(0)));
    }

    #[test]
    fn reverse_starts_at_the_last_group_and_last_item() {
        let rows = vec![vec!["copy", "paste"], vec!["save", "close"]];
        let mut scan = ItemScanner::configured_rows(
            &rows,
            Policy::KEYBOARD,
            Resolved {
                direction: Direction::Reverse,
                ..Default::default()
            },
        );
        assert_eq!(scan.position(&rows), (1, None));
        scan.handle(Action::Select);
        assert_eq!(scan.position(&rows), (1, Some(1)));
        assert_eq!(scan.handle(Action::Select), Some("close"));
        scan.advance(500, 500);
        assert_eq!(scan.handle(Action::Select), Some("save"));
        scan.restart();
        assert_eq!(scan.position(&rows), (1, None));
    }

    #[test]
    fn configurable_pass_limits_and_unlimited_do_not_change_resume_semantics() {
        for limit in [0, 1, 2, 3, 5] {
            let mut scan = ItemScanner::configured_rows(
                &[vec!["copy", "paste"]],
                Policy::MENU,
                Resolved {
                    pattern: Pattern::Linear,
                    pass_limit: limit,
                    ..Default::default()
                },
            );
            let ticks = if limit == 0 { 40 } else { limit * 2 };
            for _ in 0..ticks {
                scan.advance(500, 500);
            }
            assert_eq!(scan.suspended, limit != 0);
            if limit != 0 {
                assert_eq!(scan.handle(Action::Select), None);
                assert!(!scan.suspended);
            }
        }
    }
}
