// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The plan of the columns that containers write their content at.
//!
//! An HTML block's first line holds its indentation, which is a different block at another
//! column: a tab reaches the next multiple of four, so at some columns it is indented code, and
//! a block before it may continue over a line indented as far (`HtmlFit`). The containers that
//! an HTML block is in move it: an item by the spaces before its marker, its padding, and
//! whether it starts with a blank line; a block quote by the spaces before its `>` on its first
//! line, or by starting with a line of `>` alone; and a footnote definition by whether its first
//! block starts on its label's line. Each container chooses from these finitely many plans the
//! first, in order of preference, under which everything it holds can be written, and the blocks
//! after it in its container too, where its plan reaches them: an item's width bounds the
//! spaces before the marker of the item after it and the indentation of an HTML block after its
//! list (`Position::prev`). So the search backtracks over the nesting, and a plan that only a
//! joint choice of an item and the items in it fits is found (#288).
//!
//! The search depends only on the tree and the options, as does each plan, so a second write
//! gives the same output. Whether a node can be written at a position depends on its columns
//! only by their remainder mod 4, so the answers are kept by node and that key, and each is
//! worked out once. The flow children after a node are worked out last first, so that the
//! search recurses only into the nesting, not along a run of siblings.

use super::state::{
    flow_children, html_spaces, left_width, marker_spaces, writes_blank_line, After, Position,
    State, MAX_NESTING,
};
use crate::{
    stream::{EventKind, Tag},
    Document, NodeId,
};
use alloc::{collections::BTreeMap, vec, vec::Vec};

/// How an item is written: the spaces before its marker, the padding after it, and whether it
/// starts with a blank line, after which its content starts one column after the marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ItemPlan {
    pub(super) spaces: usize,
    pub(super) padding: usize,
    pub(super) blank: bool,
}

impl ItemPlan {
    /// The width of the item, with a marker `marker` wide.
    pub(super) fn width(self, marker: usize) -> usize {
        self.spaces + marker + self.padding
    }

    /// Where the first block of the item starts, which starts at `position`.
    pub(super) fn frame(self, position: Position, marker: usize) -> Position {
        let width = self.width(marker);
        Position {
            first: if self.blank {
                position.rest + width
            } else {
                position.first + width
            },
            rest: position.rest + width,
            after: if self.blank {
                After::Line
            } else {
                After::Marker
            },
            before: None,
            prev: None,
            shift: if self.blank {
                position.quoted
            } else {
                position.shift
            },
            quoted: position.quoted,
        }
    }
}

/// How a block quote's first line is written: with the spaces before its `>`, or as `>` alone,
/// after which its first block starts on the next line, whose `>` can take its own spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QuotePlan {
    Spaces(usize),
    Alone,
}

impl QuotePlan {
    /// Where the first block of the block quote starts, which starts at `position`.
    pub(super) fn frame(self, position: Position) -> Position {
        Position {
            first: match self {
                QuotePlan::Spaces(spaces) => position.first + spaces + 2,
                QuotePlan::Alone => position.rest + 2,
            },
            rest: position.rest + 2,
            after: After::Line,
            before: None,
            prev: None,
            shift: self == QuotePlan::Alone,
            quoted: true,
        }
    }
}

/// Where the first block of a footnote definition starts, which starts at `position` with a label
/// `width` wide with its `[^`, `]:` and space, on that line or the next.
pub(super) fn footnote_frame(position: Position, width: usize, next_line: bool) -> Position {
    Position {
        first: if next_line {
            position.rest + 4
        } else {
            position.first + width
        },
        rest: position.rest + 4,
        after: if next_line { After::Line } else { After::Label },
        before: None,
        prev: None,
        shift: if next_line {
            position.quoted
        } else {
            position.shift
        },
        quoted: position.quoted,
    }
}

/// The widest an item is: three spaces, a marker of nine digits and a delimiter, and a padding
/// of four.
const MAX_WIDTH: usize = 3 + 10 + 4;

/// What the search keeps for the write of one document.
#[derive(Default)]
pub(super) struct Plans {
    /// By node index, whether it or a flow sibling after it holds an HTML block.
    html: Option<Vec<bool>>,
    /// Whether a node and the flow siblings after it can be written, by node index and position
    /// (`key`).
    known: BTreeMap<(usize, u16), bool>,
}

/// The part of `position` that whether a node can be written there depends on.
fn key(position: Position) -> u16 {
    let after = match position.after {
        After::Line => 0,
        After::Marker => 1,
        After::Label => 2,
    };
    (position.first % 4) as u16
        | ((position.rest % 4) as u16) << 2
        | after << 4
        | u16::from(position.shift) << 6
        | u16::from(position.quoted) << 7
        | (position.prev.map_or(0, |prev| prev.min(MAX_WIDTH + 1) + 1) as u16) << 8
}

fn next_flow<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> Option<NodeId> {
    core::iter::successors(document.node(node).unwrap().next_sibling(), |&id| {
        document.node(id).unwrap().next_sibling()
    })
    .find(|&id| !matches!(document.node(id).unwrap().kind, EventKind::Separator { .. }))
}

/// Whether `node` is or ends in an item that a line indented as wide as it is would continue, so
/// that the position after it has a `Position::prev`.
fn leaves_width<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> bool {
    matches!(
        document.node(node).unwrap().kind,
        EventKind::Start(Tag::Item { .. } | Tag::List { .. })
    )
}

/// Whether `node` is an empty item or a list whose last item is empty, which a blank line ends.
fn ends_empty<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> bool {
    let last = match document.node(node).unwrap().kind {
        EventKind::Start(Tag::List { .. }) => flow_children(document, node).last(),
        _ => Some(node),
    };
    last.is_some_and(|last| flow_children(document, last).next().is_none())
}

impl State<'_> {
    /// Whether `node` or a flow sibling after it holds an HTML block, or, for an item, a block
    /// after its list does. Without one, the first plan of each container is written.
    fn holds_html<S: AsRef<str>>(&mut self, document: &Document<S>, node: NodeId) -> bool {
        let root = self.serialization_root.unwrap_or(document.root());
        let html = self.plans.html.get_or_insert_with(|| {
            let order: Vec<NodeId> = document.descendants(root).collect();
            let size = order.iter().map(|id| id.index() + 1).max().unwrap_or(0);
            let mut holds = vec![false; size];
            for &id in order.iter().rev() {
                let node = document.node(id).unwrap();
                if matches!(node.kind, EventKind::Html { block: true, .. }) {
                    holds[id.index()] = true;
                }
                if let (true, Some(parent)) = (holds[id.index()], node.parent()) {
                    holds[parent.index()] = true;
                }
            }
            let mut after = holds.clone();
            let mut children = Vec::new();
            for &id in &order {
                children.clear();
                children.extend(document.children(id));
                let mut any = false;
                for child in children.iter().rev() {
                    any |= holds[child.index()];
                    after[child.index()] = any;
                }
            }
            after
        });
        let holds = |id: NodeId| html.get(id.index()).copied().unwrap_or(false);
        holds(node)
            || matches!(
                document.node(node).unwrap().kind,
                EventKind::Start(Tag::Item { .. })
            ) && document
                .node(node)
                .unwrap()
                .parent()
                .and_then(|list| next_flow(document, list))
                .is_some_and(holds)
    }

    /// Whether flow child `node` and the flow siblings after it can be written, `node` at
    /// `position`, `depth` levels below the node being written.
    pub(super) fn writable<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        depth: usize,
    ) -> bool {
        // Content nested too deeply is an error of its own.
        if !self.holds_html(document, node) || self.depth + depth > MAX_NESTING {
            return true;
        }
        if let Some(&known) = self.plans.known.get(&(node.index(), key(position))) {
            return known;
        }
        // The siblings after `node` are worked out first, last first, at each `Position::prev`
        // they can be given, as far as one is not known yet.
        let mut after = Vec::new();
        let mut current = node;
        while let Some(next) = next_flow(document, current) {
            let rest = Position::after(position, current, None);
            if self.plans.known.contains_key(&(next.index(), key(rest))) {
                break;
            }
            after.push((current, next));
            current = next;
        }
        let index = self.item_index(document, node);
        for (offset, &(before, next)) in after.iter().enumerate().rev() {
            let index = index + offset + 1;
            self.fill(
                document,
                next,
                Position::after(position, before, None),
                index,
                depth,
            );
        }
        if position
            .before
            .is_some_and(|before| leaves_width(document, before))
        {
            self.fill(document, node, position, index, depth);
        } else {
            let result = self.writable_here(document, node, position, index, depth);
            self.plans
                .known
                .insert((node.index(), key(position)), result);
        }
        self.plans.known[&(node.index(), key(position))]
    }

    /// Work out whether `node` can be written at `position`, with each `Position::prev` that
    /// the block before it can leave.
    fn fill<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        index: usize,
        depth: usize,
    ) {
        let widths = position
            .before
            .filter(|&before| leaves_width(document, before))
            .map_or(0, |_| MAX_WIDTH);
        for prev in core::iter::once(None).chain((1..=widths).map(Some)) {
            let position = Position { prev, ..position };
            let result = self.writable_here(document, node, position, index, depth);
            self.plans
                .known
                .insert((node.index(), key(position)), result);
        }
    }

    /// The position of item `node` in its list, or 0 for another node.
    fn item_index<S: AsRef<str>>(&self, document: &Document<S>, node: NodeId) -> usize {
        let item = document.node(node).unwrap();
        match (&item.kind, item.parent()) {
            (EventKind::Start(Tag::Item { .. }), Some(list)) => flow_children(document, list)
                .position(|id| id == node)
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// Whether `node` can be written at `position`, with the siblings after it, whose answers are
    /// known.
    fn writable_here<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        index: usize,
        depth: usize,
    ) -> bool {
        if !self.holds_html(document, node) {
            return true;
        }
        match document.node(node).unwrap().kind {
            EventKind::Start(Tag::Item { .. }) => {
                let marker = self.item_marker(document, node, index);
                self.item_plans(document, node, position, marker)
                    .into_iter()
                    .any(|plan| self.item_writable(document, node, position, plan, marker, depth))
            }
            EventKind::Start(Tag::List { .. }) => match flow_children(document, node).next() {
                Some(item) => self.writable(document, item, position, depth + 1),
                None => self.after_writable(document, node, position, None, depth),
            },
            EventKind::Html { block: true, .. } => {
                html_spaces(document, node, position).is_some()
                    && self.after_writable(document, node, position, None, depth)
            }
            EventKind::Start(Tag::BlockQuote) => {
                self.quote_choice(document, node, position, depth + 1)
                    .is_some()
                    && self.after_writable(document, node, position, None, depth)
            }
            EventKind::Start(Tag::FootnoteDefinition {
                ref identifier,
                ref label,
            }) => {
                let label = label.as_ref().unwrap_or(identifier);
                let width = label.resolve(document.source()).unwrap().len() + 5;
                self.footnote_choice(document, node, position, width, depth + 1)
                    .is_some()
                    && self.after_writable(document, node, position, None, depth)
            }
            _ => self.after_writable(document, node, position, None, depth),
        }
    }

    /// Whether the flow siblings after `node`, which starts at `position` and, if it is or ends in
    /// an item, leaves that item's `width`, can be written.
    fn after_writable<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        width: Option<usize>,
        depth: usize,
    ) -> bool {
        if self.serialization_root == Some(node) {
            return true;
        }
        let Some(next) = next_flow(document, node) else {
            return true;
        };
        let blank = width.is_some()
            && ends_empty(document, node)
            && self.blank_between(document, node, next);
        let prev = left_width(document, node, width, blank);
        self.writable(document, next, Position::after(position, node, prev), depth)
    }

    /// Whether item `node` at `position`, with a marker `marker` wide, can be written with `plan`,
    /// with the items and blocks after it that its width reaches.
    pub(super) fn item_writable<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        plan: ItemPlan,
        marker: usize,
        depth: usize,
    ) -> bool {
        if let Some(first) = flow_children(document, node).next() {
            if !self.writable(document, first, plan.frame(position, marker), depth + 1) {
                return false;
            }
        }
        if self.serialization_root == Some(node) {
            return true;
        }
        // The last item's width reaches the blocks after its list.
        let after = match next_flow(document, node) {
            Some(_) => node,
            None => document.node(node).unwrap().parent().unwrap_or(node),
        };
        self.after_writable(document, after, position, Some(plan.width(marker)), depth)
    }

    /// The width of the marker of item `node`, the `index`th of its list, as `list_item` writes
    /// it.
    fn item_marker<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        node: NodeId,
        index: usize,
    ) -> usize {
        let list = document.node(node).unwrap().parent();
        match list.map(|list| &document.node(list).unwrap().kind) {
            Some(EventKind::Start(Tag::List {
                start: Some(start), ..
            })) => {
                let position = if self.options.increment_list_marker {
                    index
                } else {
                    0
                };
                let number = u64::from(*start).saturating_add(position as u64);
                number.checked_ilog10().unwrap_or(0) as usize + 2
            }
            _ => 1,
        }
    }

    /// The first plan, of the spaces before its `>` or a line of `>` alone, under which block
    /// quote `node` at `position` can be written, or `None` where none can.
    pub(super) fn quote_choice<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        depth: usize,
    ) -> Option<QuotePlan> {
        let Some(first) = flow_children(document, node).next() else {
            return Some(QuotePlan::Spaces(0));
        };
        let most = marker_spaces(document, position, false);
        (0..=most)
            .map(QuotePlan::Spaces)
            .chain([QuotePlan::Alone])
            .find(|&plan| self.writable(document, first, plan.frame(position), depth))
    }

    /// Whether the first block of footnote definition `node` at `position`, with a label `width`
    /// wide, starts on the next line: the first of the label's line and the next under which the
    /// definition can be written, or `None` where neither can. Indented code cannot start on the
    /// label's line, where it would read back as a paragraph.
    pub(super) fn footnote_choice<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        width: usize,
        depth: usize,
    ) -> Option<bool> {
        let Some(first) = flow_children(document, node).next() else {
            return Some(false);
        };
        let code = self.indented_code(document, node, false, None, first);
        [false, true]
            .iter()
            .copied()
            .filter(|&next_line| next_line || !code)
            .find(|&next_line| {
                self.writable(
                    document,
                    first,
                    footnote_frame(position, width, next_line),
                    depth,
                )
            })
    }

    /// Whether the first block of footnote definition `node`, written at `position` with a label
    /// `width` wide, starts on the next line (`footnote_choice`). Where neither line fits, one
    /// that starts with indented code or an HTML block starts on the next, and the block that does
    /// not fit is the error.
    pub(super) fn footnote_plan<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        node: NodeId,
        position: Position,
        width: usize,
    ) -> bool {
        self.footnote_choice(document, node, position, width, 0)
            .unwrap_or_else(|| {
                flow_children(document, node).next().is_some_and(|first| {
                    match document.node(first).unwrap().kind {
                        EventKind::Html { block: true, .. } => true,
                        _ => self.indented_code(document, node, false, None, first),
                    }
                })
            })
    }

    /// Whether the join between `left`, a list or item, and the flow sibling after it, `right`,
    /// is a blank line, which ends an empty item.
    fn blank_between<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        left: NodeId,
        right: NodeId,
    ) -> bool {
        let parent = document.node(left).unwrap().parent().unwrap();
        // The block before a list or item counts only for code after code.
        let before = None;
        let records = self.records_spread(document, parent);
        let spread = self.spreads_item(document, parent, records);
        writes_blank_line(self.default_separator(
            document,
            before,
            left,
            right,
            parent,
            records || spread,
        ))
    }
}
