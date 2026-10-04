//! Pure tab/split bookkeeping, with no rendering or sessions.
//!
//! Every tab lives in exactly one [`View`]. A view is a split tree of tabs, and a standalone
//! tab is a view with a single leaf. The sidebar lists `Workspace::order`. Tabs that share a
//! multi-pane view are kept next to each other, so they render as one unit.
//!
//! Tabs can also belong to a named [`Group`] (a sidebar folder). A group's tabs are kept
//! contiguous too, and a split view never straddles a group boundary.

use std::collections::HashMap;

use eframe::egui::{Rect, pos2};

/// Identifies a tab (terminal or preview) for its whole lifetime.
pub type TabId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Children side by side (split with a vertical divider).
    Horizontal,
    /// Children stacked (split with a horizontal divider).
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Leaf(TabId),
    Split {
        dir: Dir,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

/// A draggable divider: the path to its split node (false = a, true = b), its rect, direction.
#[derive(Clone, Debug)]
pub struct Splitter {
    pub path: Vec<bool>,
    pub rect: Rect,
    pub dir: Dir,
    /// Full rect of the split node, used to turn a pointer position into a ratio.
    pub parent: Rect,
}

pub const GAP: f32 = 4.0;

impl Node {
    pub fn contains(&self, id: TabId) -> bool {
        match self {
            Node::Leaf(t) => *t == id,
            Node::Split { a, b, .. } => a.contains(id) || b.contains(id),
        }
    }

    pub fn leaves(&self) -> Vec<TabId> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<TabId>) {
        match self {
            Node::Leaf(t) => out.push(*t),
            Node::Split { a, b, .. } => {
                a.collect(out);
                b.collect(out);
            }
        }
    }

    pub fn is_split(&self) -> bool {
        matches!(self, Node::Split { .. })
    }

    /// Split the `target` leaf, putting `new` on the given edge. Returns false if target is absent.
    pub fn insert(&mut self, target: TabId, new: TabId, edge: Edge) -> bool {
        match self {
            Node::Leaf(t) if *t == target => {
                let old = Box::new(Node::Leaf(target));
                let new = Box::new(Node::Leaf(new));
                let (dir, a, b) = match edge {
                    Edge::Left => (Dir::Horizontal, new, old),
                    Edge::Right => (Dir::Horizontal, old, new),
                    Edge::Top => (Dir::Vertical, new, old),
                    Edge::Bottom => (Dir::Vertical, old, new),
                };
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a,
                    b,
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => a.insert(target, new, edge) || b.insert(target, new, edge),
        }
    }

    /// Replace leaf `target` with `new`.
    pub fn replace(&mut self, target: TabId, new: TabId) -> bool {
        match self {
            Node::Leaf(t) if *t == target => {
                *t = new;
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => a.replace(target, new) || b.replace(target, new),
        }
    }

    /// Remove leaf `id`, collapsing its parent split into the sibling.
    /// Returns `None` if the whole tree was that single leaf (the caller must drop the tree).
    pub fn remove(self, id: TabId) -> Option<Node> {
        match self {
            Node::Leaf(t) if t == id => None,
            leaf @ Node::Leaf(_) => Some(leaf),
            Node::Split { dir, ratio, a, b } => match (a.remove(id), b.remove(id)) {
                (Some(a), Some(b)) => Some(Node::Split {
                    dir,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }

    fn at_path_mut(&mut self, path: &[bool]) -> Option<&mut Node> {
        match (path.split_first(), self) {
            (None, node) => Some(node),
            (Some((false, rest)), Node::Split { a, .. }) => a.at_path_mut(rest),
            (Some((true, rest)), Node::Split { b, .. }) => b.at_path_mut(rest),
            _ => None,
        }
    }

    pub fn set_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if let Some(Node::Split { ratio, .. }) = self.at_path_mut(path) {
            *ratio = new_ratio.clamp(0.1, 0.9);
        }
    }

    /// Compute the rect of every leaf, plus the splitters between them.
    pub fn layout(
        &self,
        rect: Rect,
        panes: &mut Vec<(TabId, Rect)>,
        splitters: &mut Vec<Splitter>,
    ) {
        self.layout_inner(rect, &mut Vec::new(), panes, splitters);
    }

    fn layout_inner(
        &self,
        rect: Rect,
        path: &mut Vec<bool>,
        panes: &mut Vec<(TabId, Rect)>,
        splitters: &mut Vec<Splitter>,
    ) {
        match self {
            Node::Leaf(t) => panes.push((*t, rect)),
            Node::Split { dir, ratio, a, b } => {
                let (ra, rs, rb) = match dir {
                    Dir::Horizontal => {
                        let x = rect.left() + rect.width() * ratio;
                        (
                            Rect::from_min_max(rect.min, pos2(x - GAP / 2.0, rect.bottom())),
                            Rect::from_min_max(
                                pos2(x - GAP / 2.0, rect.top()),
                                pos2(x + GAP / 2.0, rect.bottom()),
                            ),
                            Rect::from_min_max(pos2(x + GAP / 2.0, rect.top()), rect.max),
                        )
                    }
                    Dir::Vertical => {
                        let y = rect.top() + rect.height() * ratio;
                        (
                            Rect::from_min_max(rect.min, pos2(rect.right(), y - GAP / 2.0)),
                            Rect::from_min_max(
                                pos2(rect.left(), y - GAP / 2.0),
                                pos2(rect.right(), y + GAP / 2.0),
                            ),
                            Rect::from_min_max(pos2(rect.left(), y + GAP / 2.0), rect.max),
                        )
                    }
                };
                splitters.push(Splitter {
                    path: path.clone(),
                    rect: rs,
                    dir: *dir,
                    parent: rect,
                });
                path.push(false);
                a.layout_inner(ra, path, panes, splitters);
                path.pop();
                path.push(true);
                b.layout_inner(rb, path, panes, splitters);
                path.pop();
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub root: Node,
    /// The pane that receives keyboard input.
    pub focused: TabId,
}

impl View {
    fn single(id: TabId) -> Self {
        Self {
            root: Node::Leaf(id),
            focused: id,
        }
    }
}

/// Where a dragged tab was dropped relative to a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    Edge(Edge),
    /// Swap the dropped tab into the pane; the old occupant becomes standalone.
    Center,
}

pub type GroupId = u64;

/// A named folder of tabs in the sidebar (Zen style). Its tabs are contiguous in `order`.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    /// ANSI palette index used for the folder's color (None = accent).
    pub color: Option<u8>,
    /// Collapsed folders show only their header (and the focused tab, if it's inside).
    pub collapsed: bool,
}

#[derive(Default, Debug)]
pub struct Workspace {
    /// Sidebar order of tabs.
    pub order: Vec<TabId>,
    pub views: Vec<View>,
    pub active: usize,
    pub groups: Vec<Group>,
    group_of: HashMap<TabId, GroupId>,
    next_group: GroupId,
}

impl Workspace {
    pub fn view_of(&self, id: TabId) -> Option<usize> {
        self.views.iter().position(|v| v.root.contains(id))
    }

    pub fn active_view(&self) -> Option<&View> {
        self.views.get(self.active)
    }

    pub fn focused(&self) -> Option<TabId> {
        self.active_view().map(|v| v.focused)
    }

    /// Tabs visible right now (the leaves of the active view).
    pub fn visible(&self) -> Vec<TabId> {
        self.active_view()
            .map(|v| v.root.leaves())
            .unwrap_or_default()
    }

    /// Add a new standalone tab after `after` (or at the end) and activate it.
    pub fn add(&mut self, id: TabId, after: Option<TabId>) {
        let pos = after
            .and_then(|a| self.order.iter().position(|t| *t == a))
            .map(|i| i + 1)
            .unwrap_or(self.order.len());
        self.order.insert(pos, id);
        // A tab opened from inside a folder joins it.
        if let Some(g) = after.and_then(|a| self.group_of(a)) {
            self.group_of.insert(id, g);
        }
        self.views.push(View::single(id));
        self.active = self.views.len() - 1;
        self.normalize_order();
    }

    /// Show the view containing `id` and focus that tab.
    pub fn activate(&mut self, id: TabId) {
        if let Some(v) = self.view_of(id) {
            self.active = v;
            self.views[v].focused = id;
        }
    }

    /// Detach `id` from its view (collapsing splits). The tab stays in `order`, without a view.
    fn detach(&mut self, id: TabId) {
        let Some(vi) = self.view_of(id) else { return };
        let view = self.views.remove(vi);
        let focused = view.focused;
        if let Some(root) = view.root.remove(id) {
            let focused = if focused == id {
                root.leaves()[0]
            } else {
                focused
            };
            self.views.insert(vi, View { root, focused });
        } else if self.active > vi || (self.active == vi && self.active >= self.views.len()) {
            self.active = self.active.saturating_sub(1);
        }
    }

    /// Close a tab completely.
    pub fn close(&mut self, id: TabId) {
        let was_active = self.view_of(id) == Some(self.active);
        let idx = self.order.iter().position(|t| *t == id);
        self.detach(id);
        self.order.retain(|t| *t != id);
        self.group_of.remove(&id);
        self.groups
            .retain(|g| self.group_of.values().any(|v| *v == g.id));
        // If the closed tab's whole view went away, show its sidebar neighbour.
        if was_active
            && self.view_of(self.focused().unwrap_or(u64::MAX)).is_none()
            && let Some(i) = idx
            && let Some(&next) = self.order.get(i.min(self.order.len().saturating_sub(1)))
        {
            self.activate(next);
        }
        self.active = self.active.min(self.views.len().saturating_sub(1));
    }

    /// Pull a pane out of its split into its own standalone view, keeping the current view shown.
    pub fn minimize(&mut self, id: TabId) {
        let Some(vi) = self.view_of(id) else { return };
        if !self.views[vi].root.is_split() {
            return;
        }
        let shown = self.focused_view_anchor();
        self.detach(id);
        self.views.push(View::single(id));
        if let Some(anchor) = shown.filter(|a| *a != id) {
            self.active = self.view_of(anchor).unwrap_or(self.active);
        }
        self.normalize_order();
    }

    fn focused_view_anchor(&self) -> Option<TabId> {
        let view = self.active_view()?;
        // Pick a tab that will remain in the active view after a removal.
        view.root
            .leaves()
            .into_iter()
            .find(|t| *t != view.focused)
            .or(Some(view.focused))
    }

    /// Drop tab `dragged` onto pane `target` in the active view.
    pub fn drop_on(&mut self, dragged: TabId, target: TabId, drop: Drop) {
        if dragged == target {
            return;
        }
        let Some(target_view) = self.view_of(target) else {
            return;
        };
        if self.view_of(dragged).is_none() {
            return;
        }
        // Panes of one view always share a folder: the dragged tab joins the target's.
        match self.group_of(target) {
            Some(g) => self.group_of.insert(dragged, g),
            None => self.group_of.remove(&dragged),
        };
        match drop {
            Drop::Edge(edge) => {
                self.detach(dragged);
                let tv = self.view_of(target).unwrap_or(target_view);
                self.views[tv].root.insert(target, dragged, edge);
                self.views[tv].focused = dragged;
                self.active = tv;
            }
            Drop::Center => {
                self.detach(dragged);
                let tv = self.view_of(target).unwrap_or(target_view);
                self.views[tv].root.replace(target, dragged);
                self.views[tv].focused = dragged;
                self.views.push(View::single(target));
                self.active = tv;
            }
        }
        self.normalize_order();
    }

    /// Move a tab to a new sidebar position (index into `order` before the move).
    /// Moving one member of a group moves the whole group.
    pub fn reorder(&mut self, id: TabId, to: usize) {
        let Some(vi) = self.view_of(id) else { return };
        let members = self.views[vi].root.leaves();
        let before = self.order[..to.min(self.order.len())]
            .iter()
            .filter(|t| members.contains(t))
            .count();
        self.order.retain(|t| !members.contains(t));
        let at = (to - before).min(self.order.len());
        for (i, &m) in members.iter().enumerate() {
            self.order.insert(at + i, m);
        }
        self.normalize_order();
    }

    /// Move a tab (with its whole split group) one step up or down, past the neighbouring
    /// tab, split group or folder. A tab at the edge of its folder steps out of it.
    /// Returns false at either end of the list.
    pub fn move_tab(&mut self, id: TabId, down: bool) -> bool {
        let Some(vi) = self.view_of(id) else {
            return false;
        };
        let members = self.views[vi].root.leaves();
        let group = self.group_of(id);
        let positions = self.positions(|t| members.contains(&t));
        let (Some(&first), Some(&last)) = (positions.first(), positions.last()) else {
            return false;
        };

        let neighbour = if down {
            self.order.get(last + 1)
        } else {
            first.checked_sub(1).and_then(|i| self.order.get(i))
        }
        .copied();
        // Leaving the folder: stay in place, just outside it.
        if let Some(g) = group
            && neighbour.is_none_or(|n| self.group_of(n) != Some(g))
        {
            let block = self.positions(|t| self.group_of(t) == Some(g));
            for m in &members {
                self.group_of.remove(m);
            }
            let to = if down {
                block.last().unwrap() + 1
            } else {
                block[0]
            };
            self.reorder(id, to);
            self.prune_groups();
            return true;
        }
        let Some(neighbour) = neighbour else {
            return false;
        };
        // Step over the neighbour's whole folder (from outside) or split view.
        let unit: Vec<TabId> = match self.group_of(neighbour) {
            Some(ng) if group != Some(ng) => self.members(ng),
            _ => self
                .view_of(neighbour)
                .map(|v| self.views[v].root.leaves())
                .unwrap_or_else(|| vec![neighbour]),
        };
        let unit_pos = self.positions(|t| unit.contains(&t));
        let to = if down {
            unit_pos.last().unwrap() + 1
        } else {
            unit_pos[0]
        };
        self.reorder(id, to);
        true
    }

    /// Indices in `order` of the tabs matching `f`.
    fn positions(&self, f: impl Fn(TabId) -> bool) -> Vec<usize> {
        self.order
            .iter()
            .enumerate()
            .filter(|(_, t)| f(**t))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn group_of(&self, id: TabId) -> Option<GroupId> {
        self.group_of.get(&id).copied()
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn group_mut(&mut self, id: GroupId) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// A folder's tabs, in sidebar order.
    pub fn members(&self, group: GroupId) -> Vec<TabId> {
        self.order
            .iter()
            .copied()
            .filter(|t| self.group_of(*t) == Some(group))
            .collect()
    }

    /// Put `id` (with its split view) into a new folder named `name`, where it stands.
    pub fn new_group(&mut self, id: TabId, name: String) -> Option<GroupId> {
        let vi = self.view_of(id)?;
        self.next_group += 1;
        let gid = self.next_group;
        self.groups.push(Group {
            id: gid,
            name,
            color: None,
            collapsed: false,
        });
        for m in self.views[vi].root.leaves() {
            self.group_of.insert(m, gid);
        }
        self.prune_groups();
        self.normalize_order();
        Some(gid)
    }

    /// Move `id` (with its split view) into `group` at its end, or out of any folder (`None`),
    /// placed right after the folder it left.
    pub fn set_group(&mut self, id: TabId, group: Option<GroupId>) {
        let Some(vi) = self.view_of(id) else { return };
        if self.group_of(id) == group {
            return;
        }
        let members = self.views[vi].root.leaves();
        let old = self.group_of(id);
        let anchor = group
            .or(old)
            .map(|g| self.positions(|t| self.group_of(t) == Some(g)));
        for &m in &members {
            match group {
                Some(g) => self.group_of.insert(m, g),
                None => self.group_of.remove(&m),
            };
        }
        if let Some(block) = anchor.filter(|b| !b.is_empty()) {
            self.reorder(id, block.last().unwrap() + 1);
        }
        self.prune_groups();
        self.normalize_order();
    }

    /// Move `id` to sidebar position `to` (an index into `order` before the move), inside
    /// `group` or outside any folder.
    pub fn move_to(&mut self, id: TabId, to: usize, group: Option<GroupId>) {
        let Some(vi) = self.view_of(id) else { return };
        for m in self.views[vi].root.leaves() {
            match group {
                Some(g) => self.group_of.insert(m, g),
                None => self.group_of.remove(&m),
            };
        }
        self.reorder(id, to);
        self.prune_groups();
    }

    /// Dissolve a folder, leaving its tabs where they are.
    pub fn ungroup(&mut self, group: GroupId) {
        self.group_of.retain(|_, g| *g != group);
        self.groups.retain(|g| g.id != group);
    }

    /// Drop folders that no longer have tabs.
    fn prune_groups(&mut self) {
        let used: Vec<GroupId> = self.group_of.values().copied().collect();
        self.groups.retain(|g| used.contains(&g.id));
    }

    /// Keep each folder's tabs, and each view's tabs within it, contiguous in the sidebar.
    /// Both are anchored at their first member's position.
    fn normalize_order(&mut self) {
        // A view takes the folder of its first member, so it never straddles two.
        for view in &self.views {
            let leaves = view.root.leaves();
            let Some(first) = self.order.iter().find(|t| leaves.contains(t)) else {
                continue;
            };
            let group = self.group_of.get(first).copied();
            for m in leaves {
                match group {
                    Some(g) => self.group_of.insert(m, g),
                    None => self.group_of.remove(&m),
                };
            }
        }
        self.prune_groups();

        let mut out = Vec::with_capacity(self.order.len());
        let push_view = |out: &mut Vec<TabId>, t: TabId| match self.view_of(t) {
            Some(vi) => {
                for m in self.views[vi].root.leaves() {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
            }
            None => {
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        };
        for &t in &self.order {
            if out.contains(&t) {
                continue;
            }
            match self.group_of(t) {
                Some(g) => {
                    for &m in self.order.iter().filter(|m| self.group_of(**m) == Some(g)) {
                        push_view(&mut out, m);
                    }
                }
                None => push_view(&mut out, t),
            }
        }
        self.order = out;
    }

    /// Next/previous tab in sidebar order, wrapping.
    pub fn cycle(&mut self, forward: bool) {
        let Some(cur) = self.focused() else { return };
        let Some(i) = self.order.iter().position(|t| *t == cur) else {
            return;
        };
        let n = self.order.len();
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        let next = self.order[j];
        self.activate(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(n: u64) -> Workspace {
        let mut w = Workspace::default();
        for i in 1..=n {
            w.add(i, None);
        }
        w
    }

    #[test]
    fn insert_and_remove_collapses() {
        let mut root = Node::Leaf(1);
        assert!(root.insert(1, 2, Edge::Right));
        assert!(root.insert(2, 3, Edge::Bottom));
        assert_eq!(root.leaves(), vec![1, 2, 3]);
        let root = root.remove(2).unwrap();
        assert_eq!(root.leaves(), vec![1, 3]);
        let root = root.remove(1).unwrap();
        assert_eq!(root, Node::Leaf(3));
        assert!(root.remove(3).is_none());
    }

    #[test]
    fn layout_splits_rect() {
        let mut root = Node::Leaf(1);
        root.insert(1, 2, Edge::Left);
        let (mut panes, mut splitters) = (Vec::new(), Vec::new());
        root.layout(
            Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0)),
            &mut panes,
            &mut splitters,
        );
        assert_eq!(panes[0].0, 2);
        assert_eq!(panes[0].1.right(), 50.0 - GAP / 2.0);
        assert_eq!(panes[1].1.left(), 50.0 + GAP / 2.0);
        assert_eq!(splitters.len(), 1);
        root.set_ratio(&splitters[0].path, 0.99);
        assert!(matches!(root, Node::Split { ratio, .. } if ratio == 0.9));
    }

    #[test]
    fn drag_into_split_then_minimize() {
        let mut w = ws(3);
        w.activate(1);
        w.drop_on(3, 1, Drop::Edge(Edge::Right));
        assert_eq!(w.views.len(), 2);
        assert_eq!(w.visible(), vec![1, 3]);
        // Group members are adjacent in the sidebar.
        assert_eq!(w.order, vec![1, 3, 2]);

        w.minimize(3);
        assert_eq!(w.views.len(), 3);
        assert_eq!(w.visible(), vec![1]);
        assert!(w.view_of(3).is_some());
    }

    #[test]
    fn close_in_split_and_standalone() {
        let mut w = ws(3);
        w.activate(1);
        w.drop_on(2, 1, Drop::Edge(Edge::Bottom));
        w.close(2);
        assert_eq!(w.visible(), vec![1]);
        assert_eq!(w.order, vec![1, 3]);
        w.close(1);
        assert_eq!(w.visible(), vec![3]);
        w.close(3);
        assert!(w.views.is_empty() && w.order.is_empty());
    }

    #[test]
    fn center_drop_swaps() {
        let mut w = ws(3);
        w.activate(1);
        w.drop_on(2, 1, Drop::Edge(Edge::Right));
        w.drop_on(3, 1, Drop::Center);
        assert_eq!(w.visible(), vec![3, 2]);
        assert!(!w.views[w.view_of(1).unwrap()].root.is_split());
    }

    #[test]
    fn reorder_and_cycle() {
        let mut w = ws(3);
        w.reorder(3, 0);
        assert_eq!(w.order, vec![3, 1, 2]);
        w.activate(3);
        w.cycle(true);
        assert_eq!(w.focused(), Some(1));
        w.cycle(false);
        w.cycle(false);
        assert_eq!(w.focused(), Some(2));
    }

    #[test]
    fn move_tab_steps_over_groups() {
        let mut w = ws(4);
        w.activate(2);
        w.drop_on(3, 2, Drop::Edge(Edge::Right)); // order: 1 [2 3] 4
        assert_eq!(w.order, vec![1, 2, 3, 4]);
        assert!(w.move_tab(1, true)); // 1 jumps over the whole group
        assert_eq!(w.order, vec![2, 3, 1, 4]);
        assert!(w.move_tab(3, true)); // moving a member moves the group
        assert_eq!(w.order, vec![1, 2, 3, 4]);
        assert!(w.move_tab(4, false));
        assert_eq!(w.order, vec![1, 4, 2, 3]);
        assert!(!w.move_tab(1, false));
        assert!(!w.move_tab(2, true));
    }

    #[test]
    fn groups_stay_contiguous() {
        let mut w = ws(5);
        let g = w.new_group(2, "work".into()).unwrap();
        assert_eq!(w.members(g), vec![2]);
        w.set_group(4, Some(g));
        // 4 joins at the end of the folder, right after 2.
        assert_eq!(w.order, vec![1, 2, 4, 3, 5]);
        assert_eq!(w.members(g), vec![2, 4]);

        // A tab opened from inside the folder joins it.
        w.add(6, Some(2));
        assert_eq!(w.group_of(6), Some(g));
        assert_eq!(w.order, vec![1, 2, 6, 4, 3, 5]);

        // Leaving puts the tab right after the folder.
        w.set_group(2, None);
        assert_eq!(w.order, vec![1, 6, 4, 2, 3, 5]);

        // Closing the last members removes the folder.
        w.close(6);
        w.close(4);
        assert!(w.groups.is_empty());
    }

    #[test]
    fn split_view_shares_its_folder() {
        let mut w = ws(3);
        let g = w.new_group(1, "g".into()).unwrap();
        w.activate(1);
        // 3 is dropped into 1's pane: it joins the folder.
        w.drop_on(3, 1, Drop::Edge(Edge::Right));
        assert_eq!(w.group_of(3), Some(g));
        assert_eq!(w.order, vec![1, 3, 2]);
        // Grouping one pane of a split takes the whole view along.
        w.ungroup(g);
        let g2 = w.new_group(3, "h".into()).unwrap();
        assert_eq!(w.members(g2), vec![1, 3]);
    }

    #[test]
    fn move_tab_enters_steps_over_and_leaves_folders() {
        let mut w = ws(4);
        let g = w.new_group(2, "g".into()).unwrap();
        w.set_group(3, Some(g)); // 1 [2 3] 4
        // An outside tab steps over the whole folder.
        assert!(w.move_tab(1, true));
        assert_eq!(w.order, vec![2, 3, 1, 4]);
        // Inside, tabs swap; at the edge they step out.
        assert!(w.move_tab(3, false));
        assert_eq!(w.order, vec![3, 2, 1, 4]);
        assert!(w.move_tab(2, true));
        assert_eq!(w.group_of(2), None);
        assert_eq!(w.order, vec![3, 2, 1, 4]);
        assert!(w.move_tab(3, false));
        assert!(w.groups.is_empty());
        assert!(!w.move_tab(3, false));
    }

    #[test]
    fn move_to_places_into_folder() {
        let mut w = ws(4);
        let g = w.new_group(1, "g".into()).unwrap();
        w.set_group(2, Some(g)); // [1 2] 3 4
        w.move_to(4, 1, Some(g));
        assert_eq!(w.order, vec![1, 4, 2, 3]);
        assert_eq!(w.members(g), vec![1, 4, 2]);
        w.move_to(1, 4, None);
        assert_eq!(w.order, vec![4, 2, 3, 1]);
    }
}
