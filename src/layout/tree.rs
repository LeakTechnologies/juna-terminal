use super::geom::{div_permille, offset, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn primary_extent(self, r: &Rect) -> i64 {
        match self {
            Self::Horizontal => r.w as i64,
            Self::Vertical => r.h as i64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ratio(u16);

impl Ratio {
    pub const HALF: Ratio = Ratio(500);

    pub fn new(permille: u16) -> Option<Self> {
        if permille == 0 || permille >= 1000 {
            None
        } else {
            Some(Ratio(permille))
        }
    }

    pub fn permille(self) -> u16 {
        self.0
    }
}

impl Default for Ratio {
    fn default() -> Self {
        Self::HALF
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinSize {
    pub width: u32,
    pub height: u32,
}

impl Default for MinSize {
    fn default() -> Self {
        Self { width: 1, height: 1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneRect {
    pub pane: PaneId,
    pub rect: Rect,
    pub below_minimum: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutReport {
    pub panes: Vec<PaneRect>,
    pub unsatisfied: Vec<PaneId>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Leaf(PaneId),
    Split {
        axis: Axis,
        ratio: Ratio,
        first: Box<Node>,
        second: Box<Node>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    root: Option<Node>,
    next_pane: u32,
    focus: Option<PaneId>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_single(pane: PaneId) -> Self {
        Self {
            root: Some(Node::Leaf(pane)),
            next_pane: pane.0 + 1,
            focus: Some(pane),
        }
    }

    pub fn root(&self) -> Option<&Node> {
        self.root.as_ref()
    }

    pub fn focus(&self) -> Option<PaneId> {
        self.focus
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map(count).unwrap_or(0)
    }

    pub fn panes(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        if let Some(r) = &self.root {
            collect(r, &mut out);
        }
        out
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        self.panes().contains(&pane)
    }

    pub fn insert_leaf(&mut self, pane: PaneId) -> bool {
        if self.contains(pane) {
            return false;
        }
        self.root = Some(match self.root.take() {
            None => Node::Leaf(pane),
            Some(existing) => Node::Split {
                axis: Axis::Horizontal,
                ratio: Ratio::HALF,
                first: Box::new(existing),
                second: Box::new(Node::Leaf(pane)),
            },
        });
        self.next_pane = self.next_pane.max(pane.0 + 1);
        if self.focus.is_none() {
            self.focus = Some(pane);
        }
        true
    }

    pub fn split_focused(&mut self, axis: Axis, ratio: Ratio) -> Option<PaneId> {
        let focus = self.focus?;
        let pane = PaneId(self.next_pane);
        let mut with = Some(Node::Split {
            axis,
            ratio,
            first: Box::new(Node::Leaf(focus)),
            second: Box::new(Node::Leaf(pane)),
        });
        if !self
            .root
            .as_mut()
            .is_some_and(|r| replace_leaf(r, focus, &mut with))
        {
            return None;
        }
        self.next_pane += 1;
        self.focus = Some(pane);
        Some(pane)
    }

    pub fn close(&mut self, pane: PaneId) -> bool {
        if !self.contains(pane) {
            return false;
        }
        let root = self.root.take();
        self.root = root.and_then(|n| remove_leaf(&n, pane));
        if self.focus == Some(pane) {
            self.focus = self
                .root
                .as_ref()
                .and_then(first_leaf)
                .or_else(|| self.root.as_ref().and_then(last_leaf));
        }
        true
    }

    pub fn set_focus(&mut self, pane: PaneId) -> bool {
        if self.contains(pane) {
            self.focus = Some(pane);
            true
        } else {
            false
        }
    }

    pub fn move_focus(&mut self, dir: Direction) -> Option<PaneId> {
        let target = self.pane_in_direction(self.focus?, dir)?;
        self.focus = Some(target);
        Some(target)
    }

    pub fn pane_in_direction(&self, from: PaneId, dir: Direction) -> Option<PaneId> {
        let root = self.root.as_ref()?;
        let path = path_to(root, from)?;
        let mut node = root;
        for step in path {
            let Node::Split {
                axis,
                first,
                second,
                ..
            } = node
            else {
                return None;
            };
            let sibling = match (dir, axis, step) {
                (Direction::Left, Axis::Horizontal, 1) => Some(&**first),
                (Direction::Right, Axis::Horizontal, 0) => Some(&**second),
                (Direction::Up, Axis::Vertical, 1) => Some(&**first),
                (Direction::Down, Axis::Vertical, 0) => Some(&**second),
                _ => None,
            };
            if let Some(sib) = sibling {
                return Some(extreme(sib, dir));
            }
            node = match step {
                0 => &**first,
                _ => &**second,
            };
        }
        None
    }

    pub fn layout(&self, area: Rect, min: MinSize) -> LayoutReport {
        let mut report = LayoutReport::default();
        if let Some(root) = &self.root {
            let mut depth = 0usize;
            emit(root, area, min, &mut report, &mut depth);
        }
        report
    }
}

fn emit(node: &Node, rect: Rect, min: MinSize, out: &mut LayoutReport, depth: &mut usize) {
    match node {
        Node::Leaf(pane) => {
            let below = (rect.w < min.width) || (rect.h < min.height);
            if below {
                out.unsatisfied.push(*pane);
            }
            out.panes.push(PaneRect {
                pane: *pane,
                rect,
                below_minimum: below,
            });
        }
        Node::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let total = axis.primary_extent(&rect);
            let floor = match axis {
                Axis::Horizontal => min.width as i64,
                Axis::Vertical => min.height as i64,
            };
            let mut a = div_permille(total, ratio.permille());
            if total >= 2 * floor && floor > 0 {
                if a < floor {
                    a = floor;
                }
                if total - a < floor {
                    a = total - floor;
                }
            } else {
                a = a.clamp(0, total);
            }
            let b = total - a;

            let (first_rect, second_rect) = match axis {
                Axis::Horizontal => (
                    Rect::new(rect.x, rect.y, a as u32, rect.h),
                    Rect::new(offset(rect.x, a), rect.y, b as u32, rect.h),
                ),
                Axis::Vertical => (
                    Rect::new(rect.x, rect.y, rect.w, a as u32),
                    Rect::new(rect.x, offset(rect.y, a), rect.w, b as u32),
                ),
            };

            let _ = axis;
            *depth += 1;
            emit(first, first_rect, min, out, depth);
            emit(second, second_rect, min, out, depth);
            *depth -= 1;
        }
    }
}

fn count(n: &Node) -> usize {
    match n {
        Node::Leaf(_) => 1,
        Node::Split { first, second, .. } => count(first) + count(second),
    }
}

fn collect(n: &Node, out: &mut Vec<PaneId>) {
    match n {
        Node::Leaf(p) => out.push(*p),
        Node::Split { first, second, .. } => {
            collect(first, out);
            collect(second, out);
        }
    }
}

fn first_leaf(n: &Node) -> Option<PaneId> {
    match n {
        Node::Leaf(p) => Some(*p),
        Node::Split { first, .. } => first_leaf(first),
    }
}

fn last_leaf(n: &Node) -> Option<PaneId> {
    match n {
        Node::Leaf(p) => Some(*p),
        Node::Split { second, .. } => last_leaf(second),
    }
}

fn path_to(n: &Node, target: PaneId) -> Option<Vec<u8>> {
    match n {
        Node::Leaf(p) if *p == target => Some(Vec::new()),
        Node::Leaf(_) => None,
        Node::Split { first, second, .. } => {
            if let Some(mut p) = path_to(first, target) {
                p.insert(0, 0);
                Some(p)
            } else {
                path_to(second, target).map(|mut p| {
                    p.insert(0, 1);
                    p
                })
            }
        }
    }
}

fn replace_leaf(n: &mut Node, target: PaneId, with: &mut Option<Node>) -> bool {
    match n {
        Node::Leaf(p) => {
            if *p != target {
                return false;
            }
            match with.take() {
                Some(w) => {
                    *n = w;
                    true
                }
                None => false,
            }
        }
        Node::Split { first, second, .. } => {
            replace_leaf(first, target, with) || replace_leaf(second, target, with)
        }
    }
}

fn split_axis(n: &Node) -> Axis {
    match n {
        Node::Split { axis, .. } => *axis,
        Node::Leaf(_) => Axis::Horizontal,
    }
}

fn split_ratio(n: &Node) -> Ratio {
    match n {
        Node::Split { ratio, .. } => *ratio,
        Node::Leaf(_) => Ratio::HALF,
    }
}

fn remove_leaf(n: &Node, target: PaneId) -> Option<Node> {
    match n {
        Node::Leaf(p) if *p == target => None,
        Node::Leaf(_) => Some(n.clone()),
        Node::Split { first, second, .. } => {
            let a = remove_leaf(first, target);
            let b = remove_leaf(second, target);
            match (a, b) {
                (Some(a), Some(b)) => Some(Node::Split {
                    axis: split_axis(n),
                    ratio: split_ratio(n),
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            }
        }
    }
}

fn extreme(n: &Node, dir: Direction) -> PaneId {
    let mut cur = n;
    loop {
        match cur {
            Node::Leaf(p) => return *p,
            Node::Split {
                axis,
                first,
                second,
                ..
            } => {
                let take_second = match dir {
                    Direction::Left => *axis == Axis::Horizontal,
                    Direction::Right => false,
                    Direction::Up => *axis == Axis::Vertical,
                    Direction::Down => false,
                };
                cur = if take_second { second } else { first };
            }
        }
    }
}