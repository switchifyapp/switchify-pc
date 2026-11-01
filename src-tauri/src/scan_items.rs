use crate::{
    scan_preferences::{Direction, NextScan, Pattern, Resolved, StartFrom},
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
    /// Suspended by the user's choice to wait after a selection.
    waiting: bool,
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
            waiting: false,
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
        let mut scanner = Self::new(vec![], policy);
        scanner.options = options;
        scanner.nav = Navigator::new(scanner.configured_nodes(rows));
        scanner.restart();
        scanner
    }
    /// Rows as this scanner's pattern scans them.
    pub fn configured_nodes(&self, rows: &[Vec<T>]) -> Vec<Node<T>> {
        if self.options.pattern == Pattern::Linear {
            return rows.iter().flatten().cloned().map(Node::Leaf).collect();
        }
        let mut nodes = Self::nodes(rows);
        if self.splits_rows() {
            for (index, node) in nodes.iter_mut().enumerate() {
                let Node::Group { children, .. } = node else {
                    continue;
                };
                let groups = row_groups(children.len());
                if groups.len() > 1 {
                    let mut leaves = std::mem::take(children).into_iter();
                    *children = groups
                        .into_iter()
                        .enumerate()
                        .map(|(group, range)| Node::Group {
                            id: format!("row-{index}-group-{group}"),
                            children: leaves.by_ref().take(range.len()).collect(),
                        })
                        .collect();
                }
            }
        }
        nodes
    }
    fn splits_rows(&self) -> bool {
        self.options.pattern == Pattern::Grouped && self.options.row_groups
    }
    fn groups(&self, rows: &[Vec<T>], row: usize) -> Option<Vec<std::ops::Range<usize>>> {
        let groups = row_groups(rows.get(row)?.len());
        (self.splits_rows() && groups.len() > 1).then_some(groups)
    }
    pub fn row_scan(&self) -> bool {
        self.options.pattern == Pattern::Grouped && self.nav.path().is_empty()
    }
    /// Choosing among the groups of a long row.
    pub fn group_scan(&self) -> bool {
        self.splits_rows()
            && self.nav.path().len() == 1
            && !self.nav.escaping()
            && matches!(self.nav.current(), Some(Node::Group { .. }))
    }
    /// Offering to leave a group for the rest of its row.
    #[cfg(test)]
    pub fn escaping_to_groups(&self) -> bool {
        self.nav.escaping() && self.nav.path().len() > 1
    }
    /// The highlighted row, and its single highlighted item if there is one.
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
            let (row, columns) = self.highlight(rows);
            (row, columns.filter(|c| c.len() == 1).map(|c| c.start))
        }
    }
    /// The highlighted row, and the highlighted items in it when they are not
    /// the whole row: one item, or a group of a long row.
    pub fn highlight(&self, rows: &[Vec<T>]) -> (usize, Option<std::ops::Range<usize>>) {
        if self.options.pattern == Pattern::Linear {
            let (row, column) = self.position(rows);
            return (row, column.map(|c| c..c + 1));
        }
        let index = self.nav.index();
        let path = self.nav.path();
        let Some(&row) = path.first() else {
            return (index, None);
        };
        let escaping = self.nav.escaping();
        let columns = match (path.get(1), self.groups(rows, row)) {
            (None, _) if escaping => None,
            (None, Some(groups)) => groups.get(index).cloned(),
            (None, None) => Some(index..index + 1),
            (Some(&group), Some(groups)) => groups.get(group).map(|group| {
                if escaping {
                    group.clone()
                } else {
                    group.start + index..group.start + index + 1
                }
            }),
            (Some(_), None) => None,
        };
        (row, columns)
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
        self.waiting = false;
    }
    pub fn waiting(&self) -> bool {
        self.waiting
    }
    /// Left alone, a menu stays where it is and a panel starts again.
    pub fn stays_at_selection(&self) -> bool {
        match self.options.start_from {
            StartFrom::Standard => !self.policy.resume_at_root,
            StartFrom::Beginning => false,
            StartFrom::Selection => true,
        }
    }
    pub fn waits_after_selection(&self) -> bool {
        self.options.next_scan == NextScan::Wait && self.options.automatic
    }
    /// Carries on after a selection that did something, as the user chose.
    pub fn continue_after_selection(&mut self) {
        if !self.stays_at_selection() {
            self.restart();
        } else if self.options.start_from == StartFrom::Standard {
            // A menu left alone also keeps the direction it was turned to.
            self.restart_interval();
        } else {
            self.restart_in_place();
        }
        if self.waits_after_selection() {
            self.wait();
        }
    }
    /// A panel that has just opened starts from the beginning, and waits for
    /// Select first if the user chose to wait after a selection.
    pub fn open(&mut self) {
        self.restart();
        if self.waits_after_selection() {
            self.wait();
        }
    }
    /// Starts again from the highlighted item, in the chosen direction.
    pub fn restart_in_place(&mut self) {
        self.pending = false;
        self.forward = self.options.direction == Direction::Forward;
        self.restart_interval();
    }
    pub fn wait(&mut self) {
        self.waiting = true;
        self.suspended = true;
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
                if self.waiting && self.stays_at_selection() {
                    self.restart_interval();
                } else if self.policy.resume_at_root {
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

/// Splits a row the way Switchify for Android's Group Scan does: a row of seven
/// or more items becomes about √n groups, two to four, of at least two items,
/// sized as evenly as possible with any larger groups first. Shorter rows stay
/// whole.
pub fn row_groups(len: usize) -> Vec<std::ops::Range<usize>> {
    if len < 7 {
        return std::iter::once(0..len).collect();
    }
    let count = ((len as f64).sqrt().round() as usize).clamp(2, 4.min(len / 2));
    let (size, larger) = (len / count, len % count);
    let mut start = 0;
    (0..count)
        .map(|group| {
            let end = start + size + usize::from(group < larger);
            let range = start..end;
            start = end;
            range
        })
        .collect()
}

impl<T: Clone + PartialEq> ItemScanner<T> {
    /// Replaces the rows, scanned as this scanner's pattern scans them.
    pub fn replace_rows(&mut self, rows: &[Vec<T>]) -> bool {
        self.replace(self.configured_nodes(rows))
    }
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
    fn after_a_selection_scanning_continues_as_the_user_chose() {
        let rows = vec![vec!["copy", "paste"], vec!["save", "close"]];
        for next_scan in [NextScan::Standard, NextScan::Automatic, NextScan::Wait] {
            for start_from in [
                StartFrom::Standard,
                StartFrom::Beginning,
                StartFrom::Selection,
            ] {
                for automatic in [false, true] {
                    let mut scan = ItemScanner::configured_rows(
                        &rows,
                        Policy::KEYBOARD,
                        Resolved {
                            automatic,
                            next_scan,
                            start_from,
                            ..Default::default()
                        },
                    );
                    scan.handle(Action::Next);
                    scan.handle(Action::Select);
                    scan.handle(Action::Next);
                    scan.advance(499, 500);
                    assert_eq!(scan.handle(Action::Select), Some("close"));
                    scan.continue_after_selection();
                    let stays = start_from == StartFrom::Selection;
                    let place = if stays { (1, Some(1)) } else { (0, None) };
                    assert_eq!(scan.position(&rows), place);
                    let waits = automatic && next_scan == NextScan::Wait;
                    assert_eq!((scan.suspended, scan.waiting()), (waits, waits));
                    if waits {
                        assert!(!scan.advance(500, 500));
                        assert_eq!(scan.handle(Action::Next), None);
                        assert_eq!(scan.handle(Action::Select), None);
                        assert!(!scan.suspended && !scan.waiting());
                        assert_eq!(scan.position(&rows), place);
                    }
                    assert!(!scan.advance(499, 500));
                    assert!(scan.advance(1, 500));
                }
            }
        }
    }
    #[test]
    fn a_menu_left_alone_stays_at_the_selection() {
        let rows = vec![vec!["copy", "paste"], vec!["save", "close"]];
        for (start_from, place) in [
            (StartFrom::Standard, (1, Some(1))),
            (StartFrom::Selection, (1, Some(1))),
            (StartFrom::Beginning, (0, None)),
        ] {
            for wait in [false, true] {
                let mut scan = ItemScanner::configured_rows(
                    &rows,
                    Policy::MENU,
                    Resolved {
                        start_from,
                        next_scan: if wait {
                            NextScan::Wait
                        } else {
                            NextScan::Standard
                        },
                        ..Default::default()
                    },
                );
                scan.handle(Action::Next);
                scan.handle(Action::Select);
                scan.handle(Action::Next);
                assert_eq!(scan.handle(Action::Select), Some("close"));
                scan.continue_after_selection();
                assert_eq!(scan.position(&rows), place);
                assert_eq!(scan.waiting(), wait);
                if wait {
                    assert_eq!(scan.handle(Action::Select), None);
                    assert_eq!(scan.position(&rows), place);
                }
                assert!(!scan.advance(499, 500));
                assert!(scan.advance(1, 500));
            }
        }
        // Left alone, it goes on in the direction it was turned to.
        for (start_from, turned) in [(StartFrom::Standard, true), (StartFrom::Selection, false)] {
            let mut scan = ItemScanner::configured_rows(
                &rows,
                Policy::MENU,
                Resolved {
                    start_from,
                    ..Default::default()
                },
            );
            scan.handle(Action::Select);
            scan.handle(Action::Next);
            scan.handle(Action::Reverse);
            assert_eq!(scan.handle(Action::Select), Some("paste"));
            scan.continue_after_selection();
            scan.advance(500, 500);
            assert_eq!(scan.position(&rows), (0, turned.then_some(0)));
        }
    }
    #[test]
    fn staying_at_a_selection_keeps_the_direction_and_pass_limit() {
        let rows = vec![vec!["copy", "paste", "undo"], vec!["save", "close"]];
        let mut scan = ItemScanner::configured_rows(
            &rows,
            Policy::KEYBOARD,
            Resolved {
                start_from: StartFrom::Selection,
                pass_limit: 1,
                ..Default::default()
            },
        );
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        scan.handle(Action::Next);
        scan.handle(Action::Back);
        assert_eq!(scan.handle(Action::Select), Some("paste"));
        scan.continue_after_selection();
        // A step back does not turn automatic scanning around.
        assert!(scan.advance(500, 500));
        assert_eq!(scan.position(&rows), (0, Some(2)));
        // Past the last item and the way out of the row is one pass.
        assert!(scan.advance(500, 500));
        assert!(scan.nav.escaping() && !scan.suspended);
        assert!(scan.advance(500, 500));
        assert!(scan.suspended && !scan.waiting());
    }
    #[test]
    fn a_pause_that_was_not_chosen_still_resumes_at_the_start() {
        let mut scan = ItemScanner::configured_rows(
            &[vec!["copy", "paste"], vec!["save", "close"]],
            Policy::KEYBOARD,
            Resolved {
                next_scan: NextScan::Wait,
                start_from: StartFrom::Selection,
                ..Default::default()
            },
        );
        scan.handle(Action::Next);
        scan.handle(Action::Select);
        scan.suspended = true;
        scan.handle(Action::Select);
        assert!(scan.nav.path().is_empty());
        assert_eq!(scan.nav.index(), 0);
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

    #[test]
    fn long_rows_split_into_balanced_groups_as_android_does() {
        // A short row stays whole.
        for len in [0, 6] {
            assert!(row_groups(len).into_iter().eq(std::iter::once(0..len)));
        }
        assert_eq!(row_groups(7), vec![0..3, 3..5, 5..7]);
        assert_eq!(row_groups(10), vec![0..4, 4..7, 7..10]);
        assert_eq!(row_groups(12), vec![0..4, 4..8, 8..12]);
        assert_eq!(row_groups(13), vec![0..4, 4..7, 7..10, 10..13]);
        assert_eq!(row_groups(40).len(), 4);
        for len in 7..=60 {
            let groups = row_groups(len);
            assert!((2..=4).contains(&groups.len()));
            assert!(groups.iter().all(|group| group.len() >= 2));
            assert_eq!(groups.first().unwrap().start, 0);
            assert_eq!(groups.last().unwrap().end, len);
            assert!(groups.windows(2).all(|pair| pair[0].end == pair[1].start
                && pair[0].len() >= pair[1].len()
                && pair[0].len() - pair[1].len() <= 1));
        }
    }

    fn row_groups_scan(
        rows: &[Vec<&'static str>],
        direction: Direction,
    ) -> ItemScanner<&'static str> {
        ItemScanner::configured_rows(
            rows,
            Policy::KEYBOARD,
            Resolved {
                row_groups: true,
                direction,
                ..Default::default()
            },
        )
    }

    #[test]
    fn row_groups_choose_a_row_then_a_group_then_an_item() {
        let rows = vec![
            vec!["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
            vec!["shift", "space"],
        ];
        let mut scan = row_groups_scan(&rows, Direction::Forward);
        assert!(scan.row_scan() && !scan.group_scan());
        assert_eq!(scan.highlight(&rows), (0, None));
        assert_eq!(scan.handle(Action::Select), None);
        assert!(scan.group_scan() && !scan.row_scan());
        assert_eq!(scan.highlight(&rows), (0, Some(0..4)));
        assert_eq!(scan.position(&rows), (0, None));
        scan.handle(Action::Next);
        assert_eq!(scan.highlight(&rows), (0, Some(4..7)));
        assert_eq!(scan.handle(Action::Select), None);
        assert!(!scan.group_scan());
        assert_eq!(scan.highlight(&rows), (0, Some(4..5)));
        assert_eq!(scan.position(&rows), (0, Some(4)));
        scan.handle(Action::Next);
        assert_eq!(scan.position(&rows), (0, Some(5)));
        assert_eq!(scan.handle(Action::Select), Some("y"));
        // The scanner starts again at the rows, as the keyboard always has.
        scan.continue_after_selection();
        assert_eq!(scan.highlight(&rows), (0, None));
        // A short row has no groups.
        scan.handle(Action::Next);
        scan.handle(Action::Select);
        assert!(!scan.group_scan());
        assert_eq!(scan.highlight(&rows), (1, Some(0..1)));
        assert_eq!(scan.handle(Action::Select), Some("shift"));
    }

    #[test]
    fn leaving_a_group_returns_to_its_row_and_leaving_the_row_returns_to_the_rows() {
        let rows = vec![vec!["a", "b", "c", "d", "e", "f", "g"], vec!["x"]];
        let mut scan = row_groups_scan(&rows, Direction::Forward);
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        scan.handle(Action::Select);
        assert_eq!(scan.highlight(&rows), (0, Some(3..4)));
        scan.handle(Action::Next);
        assert_eq!(scan.highlight(&rows), (0, Some(4..5)));
        scan.handle(Action::Next);
        assert!(scan.escaping_to_groups());
        // The whole group is highlighted while offering to leave it.
        assert_eq!(scan.highlight(&rows), (0, Some(3..5)));
        assert_eq!(scan.position(&rows), (0, None));
        assert_eq!(scan.handle(Action::Select), None);
        assert!(scan.group_scan());
        assert_eq!(scan.highlight(&rows), (0, Some(3..5)));
        scan.handle(Action::Next);
        scan.handle(Action::Next);
        assert!(scan.nav.escaping() && !scan.escaping_to_groups());
        assert_eq!(scan.highlight(&rows), (0, None));
        assert_eq!(scan.handle(Action::Select), None);
        assert!(scan.row_scan());
        assert_eq!(scan.highlight(&rows), (0, None));
        scan.handle(Action::Next);
        assert_eq!(scan.handle(Action::Select), Some("x"));
    }

    #[test]
    fn row_groups_start_at_the_end_in_reverse() {
        let rows = vec![vec!["a", "b", "c", "d", "e", "f", "g"], vec!["x"]];
        let mut scan = row_groups_scan(&rows, Direction::Reverse);
        assert_eq!(scan.highlight(&rows), (1, None));
        scan.handle(Action::Back);
        scan.handle(Action::Select);
        assert_eq!(scan.highlight(&rows), (0, Some(5..7)));
        scan.handle(Action::Select);
        assert_eq!(scan.handle(Action::Select), Some("g"));
    }

    #[test]
    fn row_groups_need_grouped_scanning_and_survive_replacing_the_rows() {
        let rows = vec![vec!["a", "b", "c", "d", "e", "f", "g"], vec!["x"]];
        let mut linear = ItemScanner::configured_rows(
            &rows,
            Policy::KEYBOARD,
            Resolved {
                pattern: Pattern::Linear,
                row_groups: true,
                ..Default::default()
            },
        );
        assert!(!linear.group_scan());
        assert_eq!(linear.handle(Action::Select), Some("a"));
        let mut plain = ItemScanner::configured_rows(&rows, Policy::KEYBOARD, Resolved::default());
        plain.handle(Action::Select);
        assert_eq!(plain.highlight(&rows), (0, Some(0..1)));

        let mut scan = row_groups_scan(&rows, Direction::Forward);
        scan.handle(Action::Select);
        scan.handle(Action::Next);
        assert!(!scan.replace_rows(&rows));
        assert_eq!(scan.highlight(&rows), (0, Some(3..5)));
        let longer = vec![vec!["a", "b", "c", "d", "e", "f", "g", "h"], vec!["x"]];
        assert!(scan.replace_rows(&longer));
        assert!(scan.group_scan());
        assert_eq!(scan.highlight(&longer), (0, Some(3..6)));
    }
}
