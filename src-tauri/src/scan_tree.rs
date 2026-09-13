//! Pure hierarchical scan navigation, shared by grid and item techniques.
#[derive(Debug, Clone, PartialEq)]
pub enum Node<T> {
    Branch(Vec<Node<T>>),
    Leaf(T),
}

impl<T> Node<T> {
    fn normalized(self) -> Option<Self> {
        match self {
            Self::Leaf(_) => Some(self),
            Self::Branch(children) => {
                let mut children: Vec<_> =
                    children.into_iter().filter_map(Self::normalized).collect();
                match children.len() {
                    0 => None,
                    1 => children.pop(),
                    _ => Some(Self::Branch(children)),
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Selection<T> {
    None,
    Entered,
    Escaped,
    Leaf(T),
}

pub struct Navigator<T> {
    roots: Vec<Node<T>>,
    path: Vec<usize>,
    index: usize,
    escaping: bool,
}

impl<T: Clone> Navigator<T> {
    pub fn new(roots: Vec<Node<T>>) -> Self {
        Self {
            roots: roots.into_iter().filter_map(Node::normalized).collect(),
            path: vec![],
            index: 0,
            escaping: false,
        }
    }
    fn siblings(&self) -> &[Node<T>] {
        let mut nodes = self.roots.as_slice();
        for index in &self.path {
            let Node::Branch(children) = &nodes[*index] else {
                unreachable!()
            };
            nodes = children;
        }
        nodes
    }
    pub fn path(&self) -> &[usize] {
        &self.path
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn escaping(&self) -> bool {
        self.escaping
    }
    pub fn reset(&mut self) {
        self.path.clear();
        self.index = 0;
        self.escaping = false;
    }
    /// Returns true only when a full traversal wraps, after any escape slot.
    pub fn step(&mut self, forward: bool) -> bool {
        let count = self.siblings().len();
        if count == 0 {
            return false;
        }
        if self.escaping {
            self.escaping = false;
            self.index = if forward { 0 } else { count - 1 };
            return true;
        }
        let at_edge = if forward {
            self.index == count - 1
        } else {
            self.index == 0
        };
        if at_edge {
            if self.path.is_empty() {
                self.index = if forward { 0 } else { count - 1 };
                return true;
            }
            self.escaping = true;
        } else if forward {
            self.index += 1;
        } else {
            self.index -= 1;
        }
        false
    }
    pub fn select(&mut self) -> Selection<T> {
        if self.escaping {
            self.index = self.path.pop().expect("escape has a parent");
            self.escaping = false;
            return Selection::Escaped;
        }
        match self.siblings().get(self.index) {
            Some(Node::Leaf(value)) => Selection::Leaf(value.clone()),
            Some(Node::Branch(_)) => {
                self.path.push(self.index);
                self.index = 0;
                Selection::Entered
            }
            None => Selection::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> Navigator<&'static str> {
        Navigator::new(vec![
            Node::Leaf("outside"),
            Node::Branch(vec![
                Node::Leaf("a"),
                Node::Branch(vec![Node::Leaf("b"), Node::Leaf("c")]),
            ]),
        ])
    }
    #[test]
    fn nested_escape_preserves_parent_and_never_selects_a_leaf() {
        let mut n = tree();
        n.step(true);
        assert_eq!(n.select(), Selection::Entered);
        n.step(true);
        assert_eq!(n.select(), Selection::Entered);
        assert_eq!(n.select(), Selection::Leaf("b"));
        assert!(!n.step(false));
        assert!(n.escaping());
        assert_eq!(n.select(), Selection::Escaped);
        assert_eq!(n.path(), &[1]);
        assert_eq!(n.index(), 1);
        assert!(!n.step(true));
        assert_eq!(n.select(), Selection::Escaped);
        assert!(n.path().is_empty());
        assert_eq!(n.index(), 1);
        assert!(n.step(true));
        assert!(!n.escaping());
        assert_eq!(n.select(), Selection::Leaf("outside"));
    }
    #[test]
    fn ignored_escape_wraps_in_either_direction_and_reset_clears_it() {
        for forward in [true, false] {
            let mut n = Navigator::new(vec![Node::Branch(vec![Node::Leaf(10), Node::Leaf(20)])]);
            n.select();
            if forward {
                n.step(true);
            }
            assert!(!n.step(forward));
            assert!(n.escaping());
            assert!(n.step(forward));
            assert_eq!(n.select(), Selection::Leaf(if forward { 10 } else { 20 }));
            n.step(forward);
            n.step(forward);
            n.reset();
            assert!(n.path().is_empty());
            assert!(!n.escaping());
            assert_eq!(n.index(), 0);
        }
    }
    #[test]
    fn empty_branches_are_removed_and_single_children_collapse() {
        let mut n = Navigator::new(vec![
            Node::Branch(vec![]),
            Node::Branch(vec![Node::Branch(vec![Node::Leaf(42)])]),
        ]);
        assert_eq!(n.select(), Selection::Leaf(42));
        assert!(n.step(false));
        let mut empty = Navigator::<i32>::new(vec![Node::Branch(vec![])]);
        assert_eq!(empty.select(), Selection::None);
        assert!(!empty.step(true));
    }
}
