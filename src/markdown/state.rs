// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! State.
//!
//! JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/types.js#L195>.
use crate::block_parser::{indentation_width, HtmlBlockShape};
use crate::markdown::{
    attention::{self, Edge, Surroundings},
    construct_name::ConstructName,
    util::{
        format_code_as_indented::format_code_as_indented,
        format_heading_as_setext::{format_heading_as_setext, heading_as_setext},
        format_link_as_auto_link::{ends_at_boundary, written_link, WrittenLink},
        safe::{EscapeInfos, Literal},
    },
    LineWrapping, Options,
};
use crate::{
    bindings::{Lookup, Scratch},
    message::Message,
    stream::{EventKind, Tag, TagKind, Target},
    util::line_ending::{line_endings, trim_terminal_ending},
    AlignKind, Document, NodeId, NodeSyntax, ParseOptions, ReferenceKind,
};
use alloc::{string::String, vec::Vec};

pub struct Info<'a> {
    pub after: &'a str,
    before: Before<'a>,
    /// Where the text siblings written directly before this text start in the output.
    text_run: Option<usize>,
}

enum Before<'a> {
    Literal(&'a str),
    /// Output, after the text that the construct the output belongs to was written after.
    Output(&'a str, core::ops::Range<usize>),
}

/// The bindings of the tree that holds what is being written, the document when it is
/// attached, and of the subtree being written when it is not that whole tree: a reference is
/// written as one only when the definition it is bound to is the one it would read back bound to.
#[derive(Debug)]
struct WrittenBindings {
    tree: Lookup,
    written: Option<Lookup>,
}

pub struct State<'a> {
    pub output: String,
    pub(super) scratch: String,
    escapes: Vec<(usize, EscapeInfos)>,
    /// Positions of `<` that need an escape only before an email autolink.
    email_candidates: Vec<usize>,
    /// Positions that receive an escape, in order.
    applied: Vec<usize>,
    /// Output offsets of the escapes written before `<` that would start an email autolink.
    email_escapes: Vec<usize>,
    /// Output offsets that make GFM literal autolinks of the text being written.
    literal_markers: Vec<usize>,
    /// Where the text of the image being written starts, when literal autolinks form in it. A
    /// bare link there reads back as one, as the parser reads the text after `![` like the
    /// start of a line.
    pub(super) alt_start: Option<usize>,
    /// Whether the text being written leaves literal autolinks to its caller, which edits its
    /// start first.
    defer_literals: bool,
    /// The attention nodes of the block being written again that are written in a variant other
    /// than the first (`respell`), and the variant.
    variants: Vec<(NodeId, u8)>,
    pub bullet_current: Option<char>,
    pub bullet_last_used: Option<char>,
    /// The bullets that each list of a run of adjacent lists can take, worked out at the run's
    /// first list, with the next to write last.
    pub(super) bullet_plan: Vec<(NodeId, u8)>,
    /// Where the last literal autolink written where it may not be followed by a boundary ends.
    pub(super) loose_end: Option<usize>,
    /// The lists being written whose items' code is fenced, as no bullet reads back with it
    /// indented.
    pub(super) fenced_lists: Vec<NodeId>,
    pub index_stack: Vec<usize>,
    pub options: &'a Options,
    /// The options of the document being written, which its output reads back under (contract
    /// 3.3): escapes and fallbacks follow the constructs they turn on.
    pub(super) parse: ParseOptions,
    pub stack: Vec<ConstructName>,
    pub serialization_root: Option<NodeId>,
    /// The bindings of what is being written, built at the first reference written.
    bindings: Option<WrittenBindings>,
    /// Room for a reference key that is not the label itself.
    key: Scratch,
    /// Where the phrasing of the paragraph, heading or table cell being written starts. A prefix
    /// added later, such as a list item's indent, puts it at the start of a line.
    pub(super) line_start: usize,
    /// Where the last label written as a whole reference ends, and whether it is a footnote
    /// reference, such as `[^1]`, rather than a shortcut reference, such as `[r]`. Followed by
    /// `:`, such a label at the start of a paragraph can read as a definition's.
    pub(super) label_end: Option<(usize, bool)>,
    /// Where the `:` after a shortcut reference that starts the paragraph being written is, to
    /// be escaped if the paragraph would otherwise start with a definition.
    definition_colon: Option<usize>,
    /// Whether the flow child being written is code that needs a fence whatever the options, as
    /// from `fences_code`. Set before each flow child is written.
    fence_code: bool,
    /// Whether the flow child being written is a heading written ATX whatever the options, as
    /// `setext_heading` decides. Set before each flow child is written.
    pub(super) atx_heading: bool,
    /// Whether the flow child being written is a rule on the line after a paragraph or
    /// definition, which a `---` rule would underline as a setext heading, so `thematic_break`
    /// writes it with `*` (#236). Set before each flow child is written.
    pub(super) rule_after_text: bool,
    /// Where the definition that the paragraph or setext heading being written follows across a
    /// single line ending starts, if it does, and whether it has no title. Without a title, a
    /// `"`, `'` or `(` starting the block could read back as the definition's title, and with or
    /// without one, a delimiter row starting the block could make the definition's last line a
    /// table's head. Set before each flow child is written.
    joined_definition: Option<(usize, bool)>,
    /// The innermost item `list_item` is writing, and its task marker, if any.
    pub(super) task_item: Option<(NodeId, Option<NodeId>)>,
    /// How many nodes are open, which is the depth of the next node written: the serialized node
    /// is at depth 0. See `MAX_NESTING`.
    pub(super) depth: usize,
    /// The node being written, which a spelling that does not read back is reported for.
    current: Option<NodeId>,
    /// The first node written with a spelling that does not read back under the document's
    /// options, as no other does, which `to_markdown` returns as `unwritable-node` (contract 3.3).
    pub(super) unwritable: Option<NodeId>,
    /// Whether a container has been written with its first plan as no plan fits it (`plan.rs`),
    /// which holds only where a block in it or after it does not fit, which the line writer
    /// records in `unwritable`, so that the search and the writer cannot disagree unseen.
    pub(super) misfit: bool,
    /// Whether the block being written has a link written bare next to what may change where it
    /// ends, as `WrittenLink::Loose`, which is checked once the block is written.
    pub(super) loose_links: bool,
    /// Whether the block being written has a character written as it is where the options read
    /// no escape for it, which is checked once the block is written.
    raw_literals: bool,
    /// Whether a paragraph, heading or table cell is being written, which `raw_literal` checks.
    verifying: bool,
    /// What the first line of each open list item, block quote and footnote definition starts
    /// with, which a block written first in them follows on its line, and what its other lines
    /// start with, innermost last.
    pub(super) prefixes: Vec<(String, String)>,
    /// Where the flow child being written starts. Set before each flow child is written, and
    /// read first by the child, as the children of a container overwrite it.
    pub(super) position: Position,
    /// Where the children of each open list, item, block quote and footnote definition start,
    /// innermost last. The root's start at column 0.
    pub(super) frames: Vec<Position>,
    /// The item written last, and the width of its content, which a later line indented that far
    /// would continue: an item or block quote written after it, at that line's start, has fewer
    /// spaces before its marker.
    pub(super) last_item: Option<(NodeId, usize)>,
    /// The lines of the block quotes being written that start an HTML block after their first,
    /// by where they are in the output, with the spaces to write before their `>` so that the
    /// block's first line starts at a column where it is no indented code, innermost last.
    pub(super) quote_lines: Vec<(usize, usize)>,
    /// What the plan of containers' columns has worked out (`plan.rs`).
    pub(super) plans: super::plan::Plans,
    /// By node index, the shape of each HTML block asked for (`html_parts`).
    shapes: core::cell::RefCell<Vec<Option<Option<HtmlBlockShape>>>>,
    /// For each list being written, whether one of its joins, or one of its items', is written
    /// as a blank line, and whether one that would be is not, as an HTML block of kinds 1–5 that
    /// no line closes before it would take it.
    pub(super) looseness: Vec<(bool, bool)>,
}

/// Where a flow child starts: the absolute columns of its content on its first line and on its
/// other lines, as the prefixes of the containers it is in reach them, and what is before it on
/// its first line. The writer and its plan (`plan.rs`) derive each child's from its container's
/// the same way, and a block's fit depends on its columns only by their remainder mod 4.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Position {
    pub(super) first: usize,
    pub(super) rest: usize,
    pub(super) after: After,
    /// The flow child before it, or for the first item of a list, the one before the list.
    pub(super) before: Option<NodeId>,
    /// The width of the item that `before` is or ends in, which a line indented that far would
    /// continue, or `None` where there is none, or it is empty and a blank line ended it.
    pub(super) prev: Option<usize>,
    /// Whether its first line can take up to three spaces before the `>` of the block quote it
    /// is in, as a line of a block quote other than its first can.
    pub(super) shift: bool,
    /// Whether a block quote holds it, so that its other lines can take those spaces.
    pub(super) quoted: bool,
}

impl Position {
    /// Where the flow child after `before`, which starts at `position`, starts, where `prev` is
    /// what `before` leaves for it.
    pub(super) fn after(position: Position, before: NodeId, prev: Option<usize>) -> Self {
        Position {
            first: position.rest,
            rest: position.rest,
            after: After::Line,
            before: Some(before),
            prev,
            shift: position.quoted,
            quoted: position.quoted,
        }
    }
}

/// What a flow child follows on its first line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum After {
    /// The start of a line, or a block quote's marker, after which spaces before a marker are
    /// indentation.
    #[default]
    Line,
    /// An item's marker, whose padding would take spaces before a marker as more padding, which
    /// widens the item.
    Marker,
    /// A footnote definition's label, which takes every space after it, so spaces before an
    /// item's marker would not count towards its width.
    Label,
}

impl<'a> Info<'a> {
    pub fn new(before: &'a str, after: &'a str) -> Self {
        Self {
            before: Before::Literal(before),
            after,
            text_run: None,
        }
    }
    pub fn at(start: usize, end: usize, after: &'a str) -> Self {
        Self {
            before: Before::Output("", start..end),
            after,
            text_run: None,
        }
    }
    /// The text before, in two parts that are read as one.
    pub fn before<'o>(&'o self, output: &'o str) -> [&'o str; 2] {
        match &self.before {
            Before::Literal(value) => ["", value],
            Before::Output(lead, range) => [lead, &output[range.clone()]],
        }
    }
}

impl<'a> State<'a> {
    /// Whether `parent`, if it is a list or item, records a blank line between its blocks. An
    /// item's spread is read from its syntax, which scans its blocks, so it is read once for all
    /// its joins (#161). Other parents give `false`, which their joins do not read.
    pub(super) fn records_spread<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
    ) -> bool {
        match document.node(parent).unwrap().kind {
            EventKind::Start(Tag::List { tight, .. }) => !tight,
            EventKind::Start(Tag::Item { .. }) => self.item_spread(document, parent),
            _ => false,
        }
    }

    /// Whether `item` is an item of a loose list that is written with a blank line between each of
    /// its blocks, whatever spread it records.
    ///
    /// An item with a paragraph or definition, which is read from a paragraph's text, directly
    /// followed by a list that cannot interrupt a paragraph,
    /// one that starts with an empty item or is ordered and starts at a number other than 1, needs
    /// a blank line between them, or the list would read back as text in the paragraph or, after
    /// `-`, as a setext heading. When the join would otherwise be a single line ending, the item is
    /// written spread: that blank line makes it read back spread, so each of its other joins that
    /// would be a single line ending is written as a blank line too, which the list being loose
    /// already renders.
    ///
    /// Any other blank line between two of its blocks, such as one between two paragraphs or one
    /// that `needs_blank_line` asks for, as between two block quotes or after an HTML block that
    /// ends only at a blank line, makes it read back spread too, and an item that
    /// records spread is written with a blank line between each of its blocks, so it is written
    /// spread now.
    ///
    /// The only item of a loose list is also written spread when no blank line would be written
    /// between its blocks. Nothing else can show the list is loose.
    ///
    /// `spread` is what `records_spread` gives for `item`.
    pub(super) fn spreads_item<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        item: NodeId,
        spread: bool,
    ) -> bool {
        let node = document.node(item).unwrap();
        if !matches!(node.kind, EventKind::Start(Tag::Item { .. })) {
            return false;
        }
        let Some(list) = node.parent() else {
            return false;
        };
        if !matches!(
            document.node(list).unwrap().kind,
            EventKind::Start(Tag::List { tight: false, .. })
        ) {
            return false;
        }
        // Whether `item` is the list's only child, as a list holds only items.
        let lone = node.next_sibling().is_none()
            && document.node(list).unwrap().first_child() == Some(item);
        let mut joined = false;
        let mut before = None;
        let mut left = None;
        for right in flow_children(document, item) {
            if let Some(left) = left {
                let apart = matches!(
                    document.node(left).unwrap().kind,
                    EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. }
                ) && !list_interrupts_paragraph(document, right);
                let written = writes_blank_line(
                    self.default_separator(document, before, left, right, item, spread),
                );
                if apart || written {
                    return true;
                }
                joined = true;
            }
            before = left;
            left = Some(right);
        }
        lone && joined
    }

    /// Whether `id` is a definition whose label starts with `^`, and holds no whitespace, which
    /// reads back as a footnote definition where the options read those.
    fn caret_definition<S: AsRef<str>>(&self, document: &Document<S>, id: NodeId) -> bool {
        matches!(
            &document.node(id).unwrap().kind,
            EventKind::Definition { label, .. }
                if crate::block_parser::caret_definition_label(
                    label.resolve(document.source()).unwrap(),
                    &self.parse,
                )
        )
    }

    pub fn container_flow<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
    ) -> Result<(), Message> {
        let mut children = flow_children(document, parent).peekable();
        let (records_spread, spread, tight_definitions) = self.joins(document, parent);
        self.index_stack.push(0);
        let mut before = None;
        // Whether the join before the child is a blank line, which ends an empty item.
        let mut blank = false;
        for index in 0.. {
            let Some(child) = children.next() else { break };
            *self.index_stack.last_mut().unwrap() = index;
            if !matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::List { .. })
            ) {
                self.bullet_last_used = None;
            }
            // Only code reads the flag, and only code walks into the block before it. The joins are
            // blank lines when the item records spread or is written spread.
            self.fence_code = matches!(
                document.node(child).unwrap().kind,
                EventKind::CodeBlock { .. }
            ) && self.fences(document, parent, before, records_spread || spread);
            self.atx_heading = matches!(
                document.node(child).unwrap().kind,
                EventKind::Start(Tag::Heading(_))
            ) && !self.setext_heading(document, parent, before, child);
            self.rule_after_text = matches!(document.node(child).unwrap().kind, EventKind::Rule)
                && before.is_some_and(|before| {
                    matches!(
                        document.node(before).unwrap().kind,
                        EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. }
                    )
                })
                && !self.output.ends_with("\n\n");
            // A definition whose label starts with `^` reads back as a footnote definition at the
            // start of a line, so it is written only as a continuation line of the definition
            // before it, indented so it starts no footnote definition, which under
            // `code_indented` off no indent does (#208).
            if self.caret_definition(document, child)
                && !(self.parse.constructs.code_indented
                    && before.is_some_and(|before| is_definition(document, before)))
            {
                self.no_spelling_of(child);
            }
            let start = self.output.len();
            let frame = self.frames.last().copied().unwrap_or_default();
            // The first child starts on its container's first line, after what is before it there,
            // and the first item of a list where the list does.
            self.position = match before {
                None => frame,
                Some(before) => {
                    let width = self.last_item.map(|(_, width)| width);
                    Position::after(frame, before, left_width(document, before, width, blank))
                }
            };
            self.write_node(document, child, &Info::new("\n", "\n"))?;
            self.joined_definition = None;
            if let Some(&next) = children.peek() {
                // An HTML block of kinds 1–5 that its last line does not close takes the blocks
                // after it, which `Document::normalize` closes for that (#191). Under
                // `html_block_blank_lines`, one of kind 6 or 7 takes them too, and keeps the blank
                // line before them, so one that keeps a final line ending without a blank line,
                // which only the end of the document gives, has none either.
                if let Some((value, shape)) = self.html_parts(document, child) {
                    let last_line = value.trim_end_matches('\n').rsplit('\n').next();
                    if shape.unclosed
                        || shape.keeps_line_ending
                            && !value.ends_with("\n\n")
                            && last_line
                                .is_some_and(|line| !line.trim_matches([' ', '\t']).is_empty())
                    {
                        self.no_spelling_of(child);
                    }
                }
                let separator = self.join(
                    document,
                    parent,
                    before,
                    child,
                    next,
                    records_spread || spread,
                    tight_definitions,
                );
                self.output.push_str(separator);
                blank = writes_blank_line(separator);
                // A kept blank line at the end of an HTML block shows that the list is loose
                // where a block follows it in the same item, as a blank line between them, though
                // not at the end of an item or nested list, where the block holds it
                // (`takes_blank_line`).
                if in_list(document, parent) {
                    let shown = writes_blank_line(separator)
                        || self
                            .shape(document, child)
                            .is_some_and(|shape| shape.keeps_line_ending);
                    let lost = !shown
                        && (records_spread || spread)
                        && self.takes_blank_line(document, child);
                    if let Some(looseness) = self.looseness.last_mut() {
                        looseness.0 |= shown;
                        looseness.1 |= lost;
                    }
                }
                // A list whose first item starts with a blank line before an HTML block with
                // indentation (`item_starts_blank`) cannot interrupt a paragraph, which
                // `Document::normalize` leaves, unlike an empty one.
                if matches!(
                    document.node(child).unwrap().kind,
                    EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. }
                ) && !writes_blank_line(separator)
                    && matches!(
                        document.node(next).unwrap().kind,
                        EventKind::Start(Tag::List { .. })
                    )
                    && flow_children(document, next).next().is_some_and(|item| {
                        item_starts_blank(document, item)
                            && flow_children(document, item).next().is_some()
                    })
                {
                    self.no_spelling_of(next);
                }
                // Without HTML blocks, the comment that ends a list or code before indented code
                // is a paragraph, and no other line ends them.
                if separator.contains("<!---->") && !self.parse.constructs.html_flow {
                    self.no_spelling_of(next);
                }
                if let EventKind::Definition { title, .. } = &document.node(child).unwrap().kind {
                    let joined = separator == "\n"
                        && match document.node(next).unwrap().kind {
                            EventKind::Start(Tag::Paragraph { .. }) => true,
                            EventKind::Start(Tag::Heading(_)) => {
                                self.setext_heading(document, parent, Some(child), next)
                            }
                            _ => false,
                        };
                    self.joined_definition = joined.then_some((start, title.is_none()));
                }
            }
            before = Some(child);
        }
        self.index_stack.pop();
        Ok(())
    }

    /// Whether adjacent definitions in `parent`, whose joins are blank lines where `spread`, are
    /// joined by a line ending. With `tight_definitions`, they are, and other blocks are joined as
    /// without it. In an item written spread, the other joins are blank lines, which show its
    /// spread. When definitions are its only blocks, nothing else would, so they are written
    /// spread too.
    pub(super) fn tight_definitions<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        spread: bool,
    ) -> bool {
        self.options.tight_definitions
            && !(spread
                && flow_children(document, parent).all(|child| is_definition(document, child)))
    }

    /// How the flow children of `parent` are joined: whether it records spread, whether it is
    /// written spread besides, and whether adjacent definitions are joined by a line ending.
    pub(super) fn joins<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
    ) -> (bool, bool, bool) {
        let records = self.records_spread(document, parent);
        let spread = self.spreads_item(document, parent, records);
        let tight_definitions = self.tight_definitions(document, parent, records || spread);
        (records, spread, tight_definitions)
    }

    /// The join that `container_flow` writes between flow children `child` and `next` of
    /// `parent`, after `before` (`joins`, `join`).
    pub(super) fn flow_join<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        before: Option<NodeId>,
        child: NodeId,
        next: NodeId,
    ) -> &'static str {
        let (records, spread, tight_definitions) = self.joins(document, parent);
        self.join(
            document,
            parent,
            before,
            child,
            next,
            records || spread,
            tight_definitions,
        )
    }

    /// The join between flow children `child` and `next` of `parent`, after `before`, where its
    /// joins are blank lines when `spread`, and adjacent definitions are joined by a line ending
    /// when `tight_definitions` (`tight_definitions`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn join<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        before: Option<NodeId>,
        child: NodeId,
        next: NodeId,
        spread: bool,
        tight_definitions: bool,
    ) -> &'static str {
        if self.caret_definition(document, next)
            && is_definition(document, child)
            && self.parse.constructs.code_indented
        {
            "\n    "
        } else if tight_definitions
            && is_definition(document, child)
            && is_definition(document, next)
        {
            "\n"
        } else {
            // In an item written spread, each of these joins is a blank line.
            self.default_separator(document, before, child, next, parent, spread)
        }
    }

    pub fn container_phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        parent: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let start = self.output.len();
        let kind = &document.node(parent).unwrap().kind;
        if matches!(
            kind,
            EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::TableCell { .. })
        ) {
            self.line_start = start;
        }
        // Phrasing directly in the document is written as a paragraph. Link text is not a block.
        let block = matches!(
            kind,
            EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::Document { .. })
        );
        let joined_definition = if block {
            self.joined_definition.take()
        } else {
            None
        };
        let children: Vec<NodeId> = flow_children(document, parent).collect();
        // Phrasing directly in the document, which only a built tree has, is not read back.
        let checked = matches!(
            kind,
            EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::TableCell { .. })
        );
        let (loose, raw) = if checked {
            (
                core::mem::replace(&mut self.loose_links, false),
                core::mem::replace(&mut self.raw_literals, false),
            )
        } else {
            (false, false)
        };
        let verifying = self.verifying;
        self.verifying |= checked;
        let marks = self.marks();
        let spelling = BlockSpelling {
            block: parent,
            children: &children,
            info,
            start,
            joined_definition,
            written: block,
        };
        self.block_phrasing(document, &spelling)?;
        // The block is read back as it is written, with the escapes above that keep it from
        // reading as another block. One with nested or adjacent attention is read back
        // whatever it holds, as which runs of markers pair depends on the whole block.
        if checked {
            let attention = nested_attention(document, parent) && !bound(document, parent);
            let raw = core::mem::replace(&mut self.raw_literals, raw);
            let loose = core::mem::replace(&mut self.loose_links, loose);
            if attention || loose || raw {
                let read = match self.divergence(document, parent, start, raw) {
                    Some(divergence) => {
                        attention && self.respell(document, &spelling, &marks, divergence)
                    }
                    None => true,
                };
                if !read {
                    self.no_spelling_of(parent);
                }
                self.variants.clear();
            }
        }
        self.verifying = verifying;
        Ok(())
    }

    /// Write the phrasing of `spelling.block`, with the escapes that keep it from reading as
    /// another block.
    fn block_phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        spelling: &BlockSpelling<'_, '_>,
    ) -> Result<(), Message> {
        let BlockSpelling {
            block: parent,
            children,
            info,
            start,
            joined_definition,
            written: block,
        } = *spelling;
        if block {
            self.definition_colon = None;
        }
        self.phrasing(document, children, info, &Edge::default())?;
        if block {
            if let Some(colon) = self.definition_colon.take() {
                if crate::block_parser::starts_definition(&self.output[start..], &self.parse) {
                    self.escape_at(colon);
                }
            }
            // Escaped, the character reads back as text that starts the block (#173).
            if let Some((definition, true)) = joined_definition {
                if self.output[start..].starts_with(['"', '\'', '('])
                    && crate::block_parser::definition_has_title(
                        &self.output[definition..],
                        &self.parse,
                    )
                {
                    self.escape_at(start);
                }
            }
            if !self.stack.contains(&ConstructName::HeadingAtx) {
                if joined_definition.is_some()
                    && starts_with_block_html(
                        document,
                        parent,
                        self.options.line_wrapping == LineWrapping::Unwrap,
                    )
                {
                    // Indented, the line continues the definition's paragraph (#193). Without
                    // indented code, an indented line starts the same blocks, so nothing does.
                    self.output.insert_str(start, "    ");
                    if !self.parse.constructs.code_indented {
                        self.no_spelling();
                    }
                } else if let Some(&first) = children.first() {
                    self.separate_html_start(document, first, start);
                }
                self.escape_delimiter_rows(start, joined_definition.is_some());
            } else if self.output[start..].starts_with([' ', '\t']) {
                // The parser strips the space or tab after an ATX heading's `#`, so it is a
                // reference, which nothing replaces.
                self.escape_at(start);
            }
        }
        Ok(())
    }

    /// What the writer records while writing a block, which it restores to write the block
    /// again.
    fn marks(&self) -> Marks {
        Marks {
            unwritable: self.unwritable,
            misfit: self.misfit,
            loose_links: self.loose_links,
            raw_literals: self.raw_literals,
            loose_end: self.loose_end,
            label_end: self.label_end,
            definition_colon: self.definition_colon,
        }
    }

    fn restore(&mut self, marks: &Marks) {
        self.unwritable = marks.unwritable;
        self.misfit = marks.misfit;
        self.loose_links = marks.loose_links;
        self.raw_literals = marks.raw_literals;
        self.loose_end = marks.loose_end;
        self.label_end = marks.label_end;
        self.definition_colon = marks.definition_colon;
    }

    /// Write the block of `spelling` again, where it does not read back, with other spellings
    /// of the attention in it, returning whether one reads back, which is then the output.
    ///
    /// Each attention node can be written in a few variants (`attention::vary`): with the other
    /// marker, with the characters outside its sequences encoded, with those inside encoded, or
    /// a combination. The search starts from the spelling the rules of `attention::choose` give
    /// and goes in rounds: each tries changing one node, nearest first to where the block first
    /// reads back differently, and stops at a spelling that reads back; failing that, it keeps
    /// the change that reads back furthest, if any reads back further, for the next round. It
    /// stops after a fixed number of tries. Where none reads back, the first spelling is
    /// written again, for `unwritable-node`.
    fn respell<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        spelling: &BlockSpelling<'_, '_>,
        marks: &Marks,
        mut reached: usize,
    ) -> bool {
        let block = spelling.block;
        let cell = matches!(
            document.node(block).unwrap().kind,
            EventKind::Start(Tag::TableCell { .. })
        );
        let nodes: Vec<NodeId> = document
            .descendants(block)
            .filter(|&id| is_attention(document, id))
            .collect();
        // Where each node starts and ends among the inline events the block should read back as.
        let mut spans = Vec::with_capacity(nodes.len());
        let mut open = Vec::new();
        for (index, kind) in inline_events(document, document.subtree_events(block), cell)
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            match kind {
                EventKind::Start(Tag::Emphasis | Tag::Strong | Tag::Delete) => {
                    open.push(spans.len());
                    spans.push((index, index));
                }
                EventKind::End(TagKind::Emphasis | TagKind::Strong | TagKind::Delete) => {
                    let node = open.pop().expect("the tree's events are balanced");
                    spans[node].1 = index;
                }
                _ => {}
            }
        }
        spans.resize(nodes.len(), (0, 0));
        let mut variants = alloc::vec![0u8; nodes.len()];
        let mut order: Vec<usize> = (0..nodes.len()).collect();
        let mut tries = 0;
        let found = 'search: loop {
            let distance = |node: usize| {
                let (start, end) = spans[node];
                start.abs_diff(reached).min(end.abs_diff(reached))
            };
            order.sort_by_key(|&node| distance(node));
            let mut best = None;
            for &node in &order {
                let current = variants[node];
                for variant in (0..attention::VARIANTS).filter(|&variant| variant != current) {
                    if tries == RESPELL_TRIES {
                        break 'search false;
                    }
                    tries += 1;
                    variants[node] = variant;
                    self.variants = nodes
                        .iter()
                        .zip(&variants)
                        .filter(|(_, &variant)| variant != 0)
                        .map(|(&id, &variant)| (id, variant))
                        .collect();
                    match self.spell_again(document, spelling, marks) {
                        None => break 'search true,
                        Some(divergence) if divergence > reached => {
                            reached = divergence;
                            best = Some((node, variant));
                        }
                        Some(_) => {}
                    }
                }
                variants[node] = current;
            }
            match best {
                Some((node, variant)) => variants[node] = variant,
                None => break false,
            }
        };
        if !found {
            self.variants.clear();
            self.spell_again(document, spelling, marks);
        }
        found
    }

    /// Write the block of `spelling` again from `marks`, returning where it first reads back
    /// differently, or `None` where it reads back. The block was written once without an error,
    /// and another spelling meets the same checks, so an error here counts as reading back
    /// differently from the start.
    fn spell_again<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        spelling: &BlockSpelling<'_, '_>,
        marks: &Marks,
    ) -> Option<usize> {
        let (loose, raw) = (self.loose_links, self.raw_literals);
        self.restore(marks);
        self.output.truncate(spelling.start);
        let phrased = self.block_phrasing(document, spelling);
        let written = core::mem::replace(&mut self.raw_literals, raw);
        self.loose_links = loose;
        phrased.map_or(Some(0), |()| {
            self.divergence(document, spelling.block, spelling.start, written)
        })
    }

    /// Where `block`, whose phrasing is written at `output[start..]`, first reads back
    /// differently from the tree, as an index into its inline events, or `None` where it reads
    /// back as written. It may have characters written as they are where the options read no
    /// escape for them (`raw`), or links written bare next to what may change where they end:
    /// the phrasing is read back alone, as a paragraph or a table's cell, which forms the same
    /// inline nodes as the block does. A block with references or footnote calls, which need
    /// definitions read with them, is read back for its links alone, and has no spelling where
    /// a character has none. The block is checked once the escapes that keep it from reading as
    /// another block are written.
    fn divergence<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        block: NodeId,
        start: usize,
        raw: bool,
    ) -> Option<usize> {
        let cell = matches!(
            document.node(block).unwrap().kind,
            EventKind::Start(Tag::TableCell { .. })
        );
        // The containers whose first line the block starts on, whose prefixes are written before
        // it there, such as a list item's marker, which can make a line of the block read as
        // another, such as `- --`, a rule.
        let mut first_in = 0;
        let mut node = block;
        while let Some(parent) = document.node(node).unwrap().parent() {
            if flow_children(document, parent).next() != Some(node) {
                break;
            }
            match document.node(parent).unwrap().kind {
                EventKind::Start(
                    Tag::Item { .. } | Tag::BlockQuote | Tag::FootnoteDefinition { .. },
                ) => {
                    first_in += 1;
                }
                EventKind::Start(Tag::List { .. }) => {}
                _ => break,
            }
            node = parent;
        }
        let (mut prefix, mut continuation) = (String::new(), String::new());
        // A table's cells are read back in a table of their own.
        if cell {
            first_in = 0;
        }
        for (first, rest) in &self.prefixes[self.prefixes.len().saturating_sub(first_in)..] {
            prefix.push_str(first);
            continuation.push_str(rest);
        }
        if let EventKind::Start(Tag::Heading(rank)) = document.node(block).unwrap().kind {
            if self.stack.contains(&ConstructName::HeadingAtx) {
                prefix.extend(core::iter::repeat_n('#', usize::from(rank)));
                prefix.push(' ');
            }
        }
        let written = self.output[start..].trim_start_matches([' ', '\t']);
        let bound = bound(document, block);
        if bound {
            // Only links are written bare, and none is a reference here, as without a link start
            // and end none is read.
            let same = !raw && {
                let read = Document::parse(written, &self.parse).ok();
                read.is_some_and(|read| {
                    links(document, document.subtree_events(block)).eq(links(&read, read.events()))
                })
            };
            return (!same).then_some(0);
        }
        let mut source = prefix;
        if cell {
            source.push_str("| ");
            source.push_str(written);
            source.push_str(" |\n| - |\n");
        } else {
            for (index, line) in written.split('\n').enumerate() {
                if index > 0 {
                    source.push('\n');
                    source.push_str(&continuation);
                }
                source.push_str(line);
            }
        }
        let unwrap = |events: Option<Vec<EventKind<'static>>>| {
            let mut events = events?;
            // Unwrapped, each line ending is written as a space.
            if self.options.line_wrapping == LineWrapping::Unwrap {
                for event in &mut events {
                    if let EventKind::Text(text) = event {
                        let value = text.resolve("").unwrap_or_default();
                        if let alloc::borrow::Cow::Owned(value) =
                            super::handle::unwrap_soft_line_breaks(value)
                        {
                            *text = crate::stream::Text::Value(alloc::borrow::Cow::Owned(value));
                        }
                    }
                }
            }
            Some(events)
        };
        let expected = unwrap(inline_events(
            document,
            document.subtree_events(block),
            cell,
        ));
        let actual = unwrap(
            Document::parse(source.as_str(), &self.parse)
                .ok()
                .and_then(|read| inline_events(&read, read.events(), cell)),
        );
        match (expected, actual) {
            (Some(expected), Some(actual)) if expected == actual => None,
            (Some(expected), Some(actual)) => Some(
                expected
                    .iter()
                    .zip(&actual)
                    .take_while(|(expected, actual)| expected == actual)
                    .count(),
            ),
            _ => Some(0),
        }
    }

    /// Write the line ending after inline HTML that starts the block written at `output[start..]`
    /// as a character reference if the block would otherwise start with an HTML block: a complete
    /// tag alone on its first line starts one (#146).
    fn separate_html_start<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        first: NodeId,
        start: usize,
    ) {
        let EventKind::Html { value, .. } = &document.node(first).unwrap().kind else {
            return;
        };
        let end = start + value.resolve(document.source()).unwrap().len();
        if self.output.as_bytes().get(end) == Some(&b'\n')
            && crate::block_parser::complete_tag_starts_html_block(
                &self.output.as_bytes()[start..],
                &self.parse,
            )
        {
            let mut written = String::new();
            if !self.literal().write(&mut written, '\n', false) {
                self.no_spelling();
            }
            self.output.replace_range(end..end + 1, &written);
        }
    }

    /// Escape the first character of each line of the block written at `output[start..]` that
    /// would read back as a table's delimiter row under the line before it, which can be the
    /// last line of a definition the block follows across a line ending when `after_definition`.
    /// Text writes such a line as it is, and it reads back as text where it is a lazy line in
    /// the source, but not where a container prefix is written before it (#146). An escaped
    /// line is the head the next line is judged under. Without `Constructs::table`, no line is.
    fn escape_delimiter_rows(&mut self, start: usize, after_definition: bool) {
        if !self.parse.constructs.table || !after_definition && !self.output[start..].contains('\n')
        {
            return;
        }
        let mut header = after_definition.then(|| {
            let end = start - 1;
            self.output[..end].rfind('\n').map_or(0, |index| index + 1)..end
        });
        let mut header_escaped = false;
        let mut escapes = Vec::new();
        let mut at = start;
        loop {
            let end = self.output[at..]
                .find('\n')
                .map_or(self.output.len(), |index| at + index);
            let line = &self.output[at..end];
            header_escaped = header.is_some_and(|header| {
                let header = &self.output[header];
                if header_escaped {
                    let mut written = String::with_capacity(header.len() + 1);
                    written.push('\\');
                    written.push_str(header);
                    crate::block_parser::starts_table(&written, line)
                } else {
                    crate::block_parser::starts_table(header, line)
                }
            });
            if header_escaped {
                escapes.push(at);
            }
            if end == self.output.len() {
                break;
            }
            header = Some(at..end);
            at = end + 1;
        }
        if escapes.is_empty() {
            return;
        }
        let tail = self.output.split_off(escapes[0]);
        let mut cursor = escapes[0];
        for &position in &escapes {
            self.output
                .push_str(&tail[cursor - escapes[0]..position - escapes[0]]);
            let character = tail[position - escapes[0]..]
                .chars()
                .next()
                .expect("a delimiter row starts with a character");
            self.write_literal(character, true);
            cursor = position + character.len_utf8();
        }
        self.output.push_str(&tail[cursor - escapes[0]..]);
    }

    fn attention_sequences<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        id: NodeId,
    ) -> Result<Option<&'static [&'static str]>, Message> {
        let kind = &document.node(id).unwrap().kind;
        // Where no character can be escaped, a marker that the text in the attention holds
        // would read as one, so the other is tried first.
        let literal = self.literal();
        let plain = !literal.backslash && !literal.reference;
        let holds = |run: &dyn Fn(&str) -> bool| {
            plain
                && document.descendants(id).any(|child| {
                    matches!(
                        &document.node(child).unwrap().kind,
                        EventKind::Text(value) if run(value.resolve(document.source()).unwrap())
                    )
                })
        };
        let holds_marker = |marker: char| holds(&|text: &str| text.contains(marker));
        let pick = |preferred: char, asterisk, underscore| {
            let other = if preferred == '*' { '_' } else { '*' };
            let marker = if holds_marker(preferred) && !holds_marker(other) {
                other
            } else {
                preferred
            };
            if marker == '*' {
                asterisk
            } else {
                underscore
            }
        };
        Ok(match kind {
            EventKind::Start(Tag::Emphasis) => Some(pick(
                self.options.check_emphasis()?,
                attention::EMPHASIS_ASTERISK,
                attention::EMPHASIS_UNDERSCORE,
            )),
            EventKind::Start(Tag::Strong) => Some(pick(
                self.options.check_strong()?,
                attention::STRONG_ASTERISK,
                attention::STRONG_UNDERSCORE,
            )),
            // A run of two tildes in the text closes `~~`, and a run of one closes `~` too where
            // the options read one, so a run of two alone is written in `~`.
            EventKind::Start(Tag::Delete) => Some(
                if self.parse.strikethrough_single_tilde
                    && holds(&|text: &str| tilde_runs(text).any(|run| run == 2))
                    && !holds(&|text: &str| tilde_runs(text).any(|run| run == 1))
                {
                    attention::STRIKETHROUGH_SINGLE
                } else {
                    attention::STRIKETHROUGH
                },
            ),
            _ => None,
        })
    }

    /// Write attention, returning the edits for the text after it.
    ///
    /// Text written directly before may be edited so the opening sequence forms.
    fn attention<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        sequences: &'static [&'static str],
        info: &Info,
        previous: &Previous,
        next: Option<NodeId>,
    ) -> Result<Edge, Message> {
        if !self.open() {
            return Err(nesting_too_deep());
        }
        let children: Vec<NodeId> = flow_children(document, id).collect();
        let [lead, before] = info.before(&self.output);
        let before = before
            .chars()
            .next_back()
            .or_else(|| lead.chars().next_back());
        let first = children
            .first()
            .and_then(|&child| edge_character(document, child, true));
        let last = children
            .last()
            .and_then(|&child| edge_character(document, child, false));
        let after = info.after.chars().next();
        let surroundings = Surroundings {
            before,
            before_closes: matches!(previous, Previous::Attention),
            before_opens: matches!(previous, Previous::Start),
            first,
            last,
            after,
            after_closes: next.is_none(),
            first_language: children
                .first()
                .is_some_and(|&child| language(document, child)),
            after_language: next.is_some_and(|next| language(document, next)),
        };
        let variant = self
            .variants
            .iter()
            .find(|&&(node, _)| node == id)
            .map_or(0, |&(_, variant)| variant);
        let choice = attention::choose(sequences, &surroundings)
            .map(|choice| attention::vary(choice, variant, sequences, &surroundings))
            .unwrap_or_else(|| {
                self.no_spelling_of(id);
                let sequence = sequences[0];
                attention::encoding(
                    sequence,
                    sequence.chars().next().expect("sequences are not empty"),
                    &surroundings,
                )
            });
        let sequence = choice.sequence;
        let tilde = sequence.starts_with('~');
        if let (Previous::Text(start), Some(character)) = (previous, before) {
            let start = *start;
            if choice.encode_before {
                self.encode_end(start, character);
            }
            if tilde {
                self.escape_tilde_end(start);
            }
        }
        self.output.push_str(sequence);
        let edge = Edge {
            encode: first.filter(|_| choice.encode_first),
            tilde: true,
        };
        let last_text =
            self.phrasing(document, &children, &Info::new(sequence, sequence), &edge)?;
        if let Some(start) = last_text {
            self.escape_tilde_end(start);
            if let Some(character) = last.filter(|_| choice.encode_last) {
                self.encode_end(start, character);
            }
        }
        self.output.push_str(sequence);
        self.depth -= 1;
        Ok(Edge {
            encode: after.filter(|_| choice.encode_after),
            tilde,
        })
    }

    /// Encode the last character of text written at `output[start..]` if it is `character`, as
    /// `attention::encode_end` does. The reference is punctuation, so tildes before it that were
    /// judged against the character could now pair, and they are escaped.
    fn encode_end(&mut self, start: usize, character: char) {
        let length = self.output.len();
        let literal = self.literal();
        if !attention::encode_end(&mut self.output, start, character, literal) {
            self.raw_literal();
            return;
        }
        // A reference is longer than the character it encodes.
        if self.output.len() == length {
            return;
        }
        let reference = self
            .output
            .rfind('&')
            .expect("an encoded character is written as a reference");
        let tail = self.output.split_off(reference);
        self.escape_tilde_end(start);
        self.output.push_str(&tail);
    }

    /// Escape tildes at the end of text written at `output[start..]`, as
    /// `attention::escape_tilde_end` does.
    fn escape_tilde_end(&mut self, start: usize) {
        if !self.escapes_for(self.parse.constructs.strikethrough) {
            return;
        }
        let literal = self.literal();
        if !attention::escape_tilde_end(&mut self.output, start, literal) {
            self.raw_literal();
        }
    }

    /// End text written at `output[start..]` so that a language prefix written after it reads
    /// back alone. The parser reads a language as the longest run of language characters before
    /// its `:`, so a last character that is one would join it, and it is encoded as a reference,
    /// which ends the run. An escaped one is encoded instead of its escape: the escape would
    /// start the run with punctuation, which no language starts with, so none would be read.
    fn separate_language(&mut self, start: usize) {
        let Some(last) = self.output[start..]
            .bytes()
            .next_back()
            .filter(|&byte| crate::util::inline_code_info::is_info_char(byte))
        else {
            return;
        };
        let end = self.output.len() - 1;
        if last.is_ascii_punctuation() && self.literal().escapes_next(&self.output[start..end]) {
            self.output.remove(end - 1);
        }
        self.encode_end(start, char::from(last));
    }

    /// Write inline HTML, indenting each line after a line ending in it that would otherwise start
    /// a block, such as `</div>` or `# h` in a tag's attribute, or make a table's delimiter row
    /// after the line before it, so that it continues the paragraph: the parser strips the
    /// indentation of a paragraph's continuation lines, and indented code cannot interrupt a
    /// paragraph. The escape `escape_delimiter_rows` writes before other rows would be content
    /// here.
    fn inline_html(&mut self, value: &str) {
        let mut lines = value.split('\n');
        self.output.push_str(lines.next().unwrap_or_default());
        let mut at = 0;
        for line in lines {
            at += value[at..].find('\n').unwrap() + 1;
            let header = self.output.rfind('\n').map_or(0, |index| index + 1);
            let row = self.parse.constructs.table
                && crate::block_parser::starts_table(&self.output[header..], line);
            self.output.push('\n');
            if row
                || crate::block_parser::continuation_starts_block(
                    &value.as_bytes()[at..],
                    false,
                    &self.parse,
                )
            {
                self.output.push_str("    ");
                // Without indented code, an indented line starts the same blocks.
                if !self.parse.constructs.code_indented {
                    self.no_spelling();
                }
            }
            self.output.push_str(line);
        }
    }

    /// Escape the `:` that text written last ends in when, with `ParseOptions::inline_code_info`,
    /// the code written next would read back with the language characters before it as its
    /// language (#180). Escaped, the `:` is still the character before the code, but the run
    /// before it ends at the backslash, which no language holds.
    ///
    /// The run is judged within the phrasing written at `output[start..]`: what is before it in
    /// the output, such as a list item's marker, is separated from it by whitespace once the
    /// output is complete.
    fn separate_colon(&mut self, start: usize) {
        let phrasing = &self.output.as_bytes()[start..];
        if self.parse.inline_code_info
            && crate::util::inline_code_info::before(phrasing, phrasing.len()).is_some()
        {
            self.escape_at(self.output.len() - 1);
        }
    }

    /// Apply `edge` to text written at `output[start..]`, as `attention::edit_start` does. When
    /// the first character becomes a reference, which is punctuation, tildes after it that were
    /// judged against the character could now pair, and they are escaped.
    fn edit_start(&mut self, start: usize, edge: &Edge) {
        // A character attention encodes is never `~`, so the edge escapes tildes or encodes.
        let encodes = edge
            .encode
            .is_some_and(|character| self.output[start..].starts_with(character));
        let literal = self.literal();
        let edge = &Edge {
            encode: edge.encode,
            tilde: edge.tilde && self.escapes_for(self.parse.constructs.strikethrough),
        };
        if !attention::edit_start(&mut self.output, start, edge, literal) {
            self.raw_literal();
            return;
        }
        if !encodes {
            return;
        }
        let reference = start
            + self.output[start..]
                .find(';')
                .expect("an encoded character is written as a reference")
            + 1;
        let run = self.output[reference..]
            .bytes()
            .take_while(|&byte| byte == b'~')
            .count();
        if run > 0 {
            let mut escaped = String::new();
            for _ in 0..run {
                // A character reference is written before the run, so one can be here too.
                if !literal.write(&mut escaped, '~', true) {
                    unreachable!("the options read character references");
                }
            }
            self.output
                .replace_range(reference..reference + run, &escaped);
        }
    }

    /// Serialize phrasing children, returning where the last one begins if it is text.
    ///
    /// `edge` edits the first child if it is text.
    fn phrasing<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        children: &[NodeId],
        info: &Info,
        edge: &Edge,
    ) -> Result<Option<usize>, Message> {
        let start = self.output.len();
        self.index_stack.push(0);
        let mut index = 0;
        let mut after_hard_break = false;
        let mut pending = Edge {
            encode: edge.encode,
            tilde: edge.tilde,
        };
        let mut previous = Previous::Start;
        // Where the text children written directly before the current child start.
        let mut run = None;
        // Where the previous child starts if it is a label that `:` can make a definition's,
        // whether it is a footnote reference, and whether it starts the block (#122).
        let mut definition_label = None;
        // A task item's checkbox is written before its paragraph, so the paragraph's first child
        // does not start the line.
        let task = children
            .first()
            .and_then(|&first| document.node(first).unwrap().parent())
            .is_some_and(|parent| self.is_task_paragraph(document, parent));
        for (position, &child) in children.iter().enumerate() {
            let kind = &document.node(child).unwrap().kind;
            let sequences = self.attention_sequences(document, child)?;
            if after_hard_break && matches!(kind, EventKind::SoftBreak(_)) {
                after_hard_break = false;
                continue;
            }
            after_hard_break = matches!(kind, EventKind::HardBreak);
            *self.index_stack.last_mut().unwrap() = index;
            let mut buffer = [0u8; 5];
            let next = children.get(position + 1).copied();
            let after = if let Some(next) = next {
                if let Some(first) = self.peek_node(document, next, child) {
                    first.encode_utf8(&mut buffer) as &str
                } else {
                    // Render nodes without a fixed prefix to determine the next character.
                    let checkpoint = self.output.len();
                    let marks = (self.unwritable, self.loose_links, self.raw_literals);
                    self.write_node(document, next, &Info::new("", ""))?;
                    let first = self.output[checkpoint..].chars().next().unwrap_or_default();
                    self.output.truncate(checkpoint);
                    (self.unwritable, self.loose_links, self.raw_literals) = marks;
                    // A line ending there is not the one that ends the block, so the NUL that
                    // follows it in the buffer stands for the rest of the next node and after.
                    let length = first.encode_utf8(&mut buffer).len();
                    core::str::from_utf8(&buffer[..length + 1]).expect("NUL is a character")
                }
            } else {
                info.after
            };
            let before = if self.output.len() == start {
                match &info.before {
                    Before::Literal(value) => Before::Literal(value),
                    Before::Output(lead, range) => Before::Output(lead, range.clone()),
                }
            } else if self.output.ends_with('\n')
                && matches!(kind, EventKind::Html { value, .. }
                if crate::block_parser::html_interrupts_paragraph(
                    value.resolve(document.source()).unwrap().as_bytes(),
                    &self.parse,
                ))
            {
                // Indent the line so it continues the paragraph, keeping the soft or hard break
                // before it, as indented code cannot interrupt a paragraph (#193). Without
                // indented code, an indented line starts the same blocks, so nothing does.
                self.output.push_str("    ");
                if !self.parse.constructs.code_indented {
                    self.no_spelling_of(child);
                }
                Before::Literal(" ")
            } else {
                // The children written so far follow what the container follows, such as the
                // start of a line, so where text sits in its line does not depend on how it is
                // split into nodes.
                let lead = match &info.before {
                    Before::Literal(value) => value,
                    Before::Output(..) => "",
                };
                Before::Output(lead, start..self.output.len())
            };
            let length = self.output.len();
            let text = matches!(kind, EventKind::Text(_));
            let info = Info {
                before,
                after,
                text_run: run.filter(|_| text),
            };
            if let Some(sequences) = sequences {
                pending = self.attention(document, child, sequences, &info, &previous, next)?;
                previous = Previous::Attention;
            } else {
                if let (
                    Previous::Text(start),
                    EventKind::Code {
                        language: Some(_), ..
                    },
                ) = (&previous, kind)
                {
                    self.separate_language(*start);
                } else if let (Previous::Text(_), EventKind::Code { language: None, .. }) =
                    (&previous, kind)
                {
                    self.separate_colon(start);
                }
                // Attention can encode the first character of text, which ends a literal
                // there, so literals are escaped once it has.
                self.defer_literals = text;
                if matches!(kind, EventKind::HardBreak) {
                    self.before_hard_break();
                }
                self.write_node(document, child, &info)?;
                self.defer_literals = false;
                if text {
                    self.edit_start(length, &pending);
                    if self.literals() {
                        self.escape_literals(length, &info);
                    }
                }
                // Any node can write the `:`, such as inline HTML (#146).
                if self.output[length..].starts_with(':') {
                    match definition_label {
                        // `[^1]:` starts a footnote definition whatever follows, when its
                        // label is one a footnote definition can have.
                        Some((start, true, _))
                            if crate::block_parser::starts_footnote_definition(
                                &self.output.as_bytes()[start..],
                            ) =>
                        {
                            self.escape_at(length);
                        }
                        // `[r]:` starts a definition only if a destination follows, which
                        // can be in later children, so the paragraph is judged once written.
                        // So does a footnote reference whose label, such as `^ 1`, only a
                        // definition can have.
                        Some((_, _, true)) => self.definition_colon = Some(length),
                        _ => {}
                    }
                }
                if text {
                    pending = Edge::default();
                    previous = Previous::Text(length);
                } else if self.output.len() != length {
                    pending = Edge::default();
                    previous = Previous::Other;
                }
            }
            definition_label = self
                .label_end
                .filter(|&(end, _)| {
                    end == self.output.len()
                        && end != length
                        && !self.stack.iter().any(|name| {
                            matches!(name, ConstructName::HeadingAtx | ConstructName::TableCell)
                        })
                })
                .and_then(|(_, footnote)| {
                    // The first child of a paragraph or setext heading starts it, unless the
                    // paragraph is a task item's, whose checkbox is written before it.
                    let block_start = position == 0 && !task && length == self.line_start;
                    // A footnote definition also interrupts a paragraph, so a footnote reference
                    // after a line ending in the block, such as a soft or hard break, can start
                    // one too.
                    let line_start = footnote
                        && length > self.line_start
                        && self.output[..length].ends_with('\n');
                    (block_start || line_start).then_some((length, footnote, block_start))
                });
            if text {
                run = run.or(Some(length));
            } else if self.output.len() != length {
                run = None;
            }
            index += 1;
        }
        self.index_stack.pop();
        Ok(match previous {
            Previous::Text(start) => Some(start),
            _ => None,
        })
    }

    pub fn enter(&mut self, name: ConstructName) {
        self.stack.push(name);
    }

    pub fn exit(&mut self) {
        self.stack.pop();
    }

    pub fn handle<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        info: &Info,
    ) -> Result<String, Message> {
        self.write_node(document, id, info)?;
        Ok(core::mem::take(&mut self.output))
    }

    /// Open a node at the current depth, unless it is deeper than `MAX_NESTING`, which the caller
    /// returns as an error before writing the node's children. The caller closes the node by
    /// decrementing `depth`; after an error, serializing stops, so it is not closed.
    fn open(&mut self) -> bool {
        let open = self.depth <= MAX_NESTING;
        self.depth += usize::from(open);
        open
    }

    fn write_node<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        let previous = self.current.replace(id);
        let result = self.write_node_kind(document, id, info);
        self.current = previous;
        result
    }

    /// Record that the node being written has no spelling that reads back under the document's
    /// options, so that `to_markdown` returns `unwritable-node` for the first such node.
    pub(super) fn no_spelling(&mut self) {
        let id = self
            .current
            .or(self.serialization_root)
            .expect("a node is being written");
        self.no_spelling_of(id);
    }

    /// Record that the node being written has a character written as it is, where the options
    /// read no escape for it: in a paragraph, heading or table cell, which is checked to read back
    /// once written, and otherwise as having no spelling.
    pub(super) fn raw_literal(&mut self) {
        if self.verifying {
            self.raw_literals = true;
        } else {
            self.no_spelling();
        }
    }

    /// Record that `id` has no spelling, as `no_spelling` does for the node being written.
    pub(super) fn no_spelling_of(&mut self, id: NodeId) {
        self.unwritable.get_or_insert(id);
    }

    /// How a character is written so that it reads back as text under the document's options.
    pub(super) fn literal(&self) -> Literal {
        Literal {
            backslash: self.parse.constructs.character_escape,
            reference: self.parse.constructs.character_reference,
        }
    }

    /// Write `character` so that it reads back as text, with a backslash where `backslash` and the
    /// options allow one and a character reference otherwise, or as it is, recording that the
    /// node has no spelling, where the options read neither.
    pub(super) fn write_literal(&mut self, character: char, backslash: bool) {
        let literal = self.literal();
        if !literal.write(&mut self.output, character, backslash) {
            self.raw_literal();
        }
    }

    /// Insert a backslash, or what replaces it, before the character at `position` of the output,
    /// as `write_literal` writes it.
    pub(super) fn escape_at(&mut self, position: usize) {
        let literal = self.literal();
        if !literal.escape_at(&mut self.output, position) {
            self.raw_literal();
        }
    }

    fn write_node_kind<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        info: &Info,
    ) -> Result<(), Message> {
        if !self.open() {
            return Err(nesting_too_deep());
        }
        let node = document
            .node(id)
            .expect("serialization uses existing arena nodes");
        let source = document.source();
        let result = match &node.kind {
            EventKind::Start(tag) => match tag {
                Tag::Document { frontmatter, .. } => {
                    if let Some(body) = frontmatter {
                        let syntax = document.syntax(id);
                        let (info, fence) = match &syntax {
                            // Without recorded fences, frontmatter is written with `---`.
                            Some(NodeSyntax::Frontmatter { info, fence, .. }) => {
                                (info.resolve(source).unwrap(), *fence)
                            }
                            _ => (
                                "",
                                crate::CodeFence {
                                    character: '-',
                                    length: 3,
                                    indent: 0,
                                },
                            ),
                        };
                        self.frontmatter(body.resolve(source).unwrap(), info, fence)?;
                        if flow_children(document, id).next().is_some() {
                            self.output.push_str("\n\n");
                        }
                    }
                    if flow_children(document, id)
                        .all(|child| is_phrasing(&document.node(child).unwrap().kind))
                    {
                        self.container_phrasing(document, id, info)
                    } else {
                        let start = self.output.len();
                        self.container_flow(document, id)?;
                        // A `---` rule starting the document opens frontmatter if a later line,
                        // such as another rule or a line of code, closes it, so it is written
                        // with `*` then (#236).
                        let first = flow_children(document, id).next().unwrap();
                        if frontmatter.is_none()
                            && matches!(document.node(first).unwrap().kind, EventKind::Rule)
                            && crate::block_parser::frontmatter::scan(self.output.as_bytes(), start)
                                .is_some()
                        {
                            self.output.replace_range(start..start + 3, "***");
                        }
                        Ok(())
                    }
                }
                Tag::Paragraph { .. } => self.paragraph(document, id, info),
                Tag::Heading(level) => self.heading(document, id, *level),
                Tag::BlockQuote => self.blockquote(document, id),
                Tag::List { start, .. } => self.list(document, id, start.is_some()),
                Tag::Item { .. } => self.list_item(document, id),
                Tag::Emphasis | Tag::Strong | Tag::Delete => {
                    // Attention outside a phrasing container is written on its own, and opened by
                    // `attention`, so it is not counted twice.
                    self.depth -= 1;
                    self.enter(ConstructName::Phrasing);
                    self.phrasing(document, &[id], info, &Edge::default())?;
                    self.exit();
                    self.depth += 1;
                    Ok(())
                }
                Tag::Link(target) | Tag::Image(target) => {
                    let image = matches!(tag, Tag::Image(_));
                    let (destination, title) = match self.written_target(document, id, target) {
                        Ok(written) => written,
                        Err(definition) => {
                            let reference = target
                                .reference()
                                .expect("only a reference has no destination");
                            let label = reference.label.resolve(source).unwrap();
                            // An unbound reference reads back as text however it is written, so
                            // one whose label does not fit its form is written as a full one.
                            let kind =
                                if definition.is_none() && !crate::node_syntax::label_fits(label) {
                                    ReferenceKind::Full
                                } else {
                                    reference.kind
                                };
                            if self.reference(document, id, kind, label, image, info.after)? {
                                self.depth -= 1;
                                return Ok(());
                            }
                            Self::definition_target(
                                document,
                                definition.expect("an unbound reference is written"),
                            )
                        }
                    };
                    self.resource(document, id, destination, title, image)
                }
                Tag::Table => self.render_table(document, id),
                Tag::TableRow => self.render_table_row(document, id),
                Tag::TableCell { .. } => self.container_phrasing(document, id, info),
                Tag::TableHead | Tag::TableBody => {
                    let rows = flow_children(document, id).map(|row| (row, false));
                    self.render_table_rows(document, rows, false)
                }
                Tag::FootnoteDefinition { identifier, label } => {
                    let label = label
                        .as_ref()
                        .unwrap_or(identifier)
                        .resolve(source)
                        .unwrap();
                    self.output.push_str("[^");
                    self.output.push_str(label);
                    self.output.push_str("]:");
                    if flow_children(document, id).next().is_some() {
                        let position = self.position;
                        let width = label.len() + 5;
                        let next_line = self.footnote_plan(document, id, position, width);
                        self.frames
                            .push(super::plan::footnote_frame(position, width, next_line));
                        self.output.push(if next_line { '\n' } else { ' ' });
                        let start = self.output.len();
                        let prefix = if next_line {
                            String::new()
                        } else {
                            alloc::format!("[^{label}]: ")
                        };
                        self.prefixes.push((prefix, "    ".into()));
                        let result = self.container_flow(document, id);
                        self.prefixes.pop();
                        self.frames.pop();
                        result?;
                        self.indent_lines(start, |line, index, blank, output| {
                            if (next_line || index > 0) && !blank {
                                output.push_str("    ");
                            }
                            output.push_str(line);
                        });
                    }
                    Ok(())
                }
            },
            EventKind::Text(text) => {
                let mut value = text.resolve(source).unwrap();
                if let Some(parent) = node.parent() {
                    if document.node(parent).unwrap().first_child() == Some(id)
                        && self.is_task_paragraph(document, parent)
                    {
                        // The checkbox is written with its separating space, so only the one the parser keeps after it is dropped. Other whitespace is content.
                        value = value.strip_prefix([' ', '\t']).unwrap_or(value);
                    }
                }
                self.text(value, info)
            }
            // A soft break is a line ending, which is LF whatever value a constructed one has.
            // First in a task item's paragraph, it follows the checkbox and its space, which the
            // parser reads as text before the line ending, so it is written as it is.
            EventKind::SoftBreak(_)
                if node.parent().is_some_and(|parent| {
                    document.node(parent).unwrap().first_child() == Some(id)
                        && self.is_task_paragraph(document, parent)
                }) =>
            {
                self.output.push('\n');
                Ok(())
            }
            EventKind::SoftBreak(_) => self.text("\n", info),
            EventKind::HardBreak => {
                self.hard_break();
                Ok(())
            }
            EventKind::Rule => self.thematic_break(),
            EventKind::Code { value, language } => self.inline_code(
                value.resolve(source).unwrap(),
                language
                    .as_ref()
                    .map(|language| language.resolve(source).unwrap()),
            ),
            EventKind::CodeBlock { value, info, .. } => {
                let value = value.resolve(source).unwrap();
                let info = info.as_ref().map(|info| info.resolve(source).unwrap());
                let fence = core::mem::take(&mut self.fence_code);
                // Code that only a fence keeps apart from the block before it, such as an HTML
                // block that keeps its blank lines (#265), has no spelling without fences.
                if fence && !self.parse.constructs.code_fenced {
                    self.no_spelling_of(id);
                }
                self.code(value, info, fence)
            }
            EventKind::Html { value, block: true } => {
                let value = value.resolve(source).unwrap();
                let shape = self
                    .shape(document, id)
                    .expect("a document's HTML block starts one");
                self.html_block(document, id);
                self.output.push_str(if shape.keeps_line_ending {
                    &value[..value.len() - 1]
                } else {
                    value
                });
                Ok(())
            }
            EventKind::Html { value, .. } => {
                self.inline_html(value.resolve(source).unwrap());
                Ok(())
            }
            EventKind::Definition {
                label,
                destination,
                title,
            } => self.definition(
                label.resolve(source).unwrap(),
                destination.resolve(source).unwrap(),
                title.as_ref().map(|value| value.resolve(source).unwrap()),
            ),
            EventKind::FootnoteReference { identifier, label } => {
                let label = label
                    .as_ref()
                    .unwrap_or(identifier)
                    .resolve(source)
                    .unwrap();
                self.output.push_str("[^");
                self.output.push_str(label);
                self.output.push(']');
                self.label_end = Some((self.output.len(), true));
                Ok(())
            }
            EventKind::Separator { .. } => Ok(()),
            EventKind::End(_) => unreachable!("arena nodes store starts and leaves only"),
        };
        self.depth -= 1;
        result
    }

    /// Check that HTML block `id` starts where it is written at a column it fits, and record the
    /// spaces before `>` that its line takes for that, which `blockquote` writes (`html_spaces`).
    fn html_block<S: AsRef<str>>(&mut self, document: &Document<S>, id: NodeId) {
        match self.html_spaces(document, id, self.position) {
            Some(0) => {}
            Some(spaces) => self.quote_lines.push((self.output.len(), spaces)),
            None => self.no_spelling_of(id),
        }
    }

    fn render_table<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
    ) -> Result<(), Message> {
        // Each row, in a head or body, a level between it and the table.
        let rows = flow_children(document, id)
            .flat_map(|group| flow_children(document, group).map(|row| (row, true)));
        self.render_table_rows(document, rows, true)
    }

    fn render_table_rows<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        rows: impl Iterator<Item = (NodeId, bool)>,
        header: bool,
    ) -> Result<(), Message> {
        for (index, (row, grouped)) in rows.enumerate() {
            if index > 0 {
                self.output.push('\n');
            }
            // A row in a head or body opens that first, a level between the row and the table.
            if grouped && !self.open() || !self.open() {
                return Err(nesting_too_deep());
            }
            let start = self.output.len();
            self.render_table_row(document, row)?;
            self.depth -= 1 + usize::from(grouped);
            if header && index == 0 {
                // A head row shaped like a delimiter row would read back as the delimiter row of
                // a table headed by the line before it, which a paragraph or definition can
                // continue to, so its first cell starts with an escape. Every cell of such a row
                // is `-` with optional colons, so the first starts after `| `.
                if crate::block_parser::delimiter_row(&self.output.as_bytes()[start..]) {
                    self.escape_at(start + 2);
                }
                self.output.push_str("\n| ");
                for (index, cell) in flow_children(document, row).enumerate() {
                    if index > 0 {
                        self.output.push_str(" | ");
                    }
                    let EventKind::Start(Tag::TableCell { alignment, .. }) =
                        document.node(cell).unwrap().kind
                    else {
                        unreachable!("rendered table rows contain only cells")
                    };
                    self.output.push_str(match alignment {
                        AlignKind::Left => ":---",
                        AlignKind::Right => "---:",
                        AlignKind::Center => ":---:",
                        AlignKind::None => "---",
                    });
                }
                self.output.push_str(" |");
            }
        }
        Ok(())
    }

    fn render_table_row<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        row: NodeId,
    ) -> Result<(), Message> {
        self.output.push_str("| ");
        for (index, cell) in flow_children(document, row).enumerate() {
            if index > 0 {
                self.output.push_str(" | ");
            }
            self.enter(ConstructName::TableCell);
            self.enter(ConstructName::Phrasing);
            if !self.open() {
                return Err(nesting_too_deep());
            }
            self.container_phrasing(document, cell, &Info::new("", ""))?;
            self.depth -= 1;
            self.exit();
            self.exit();
        }
        self.output.push_str(" |");
        Ok(())
    }

    // Transform only the newly written range; nested formatting reuses this buffer after its children finish.
    pub(super) fn rewrite(
        &mut self,
        start: usize,
        transform: impl FnOnce(&mut String, &mut String),
    ) {
        let mut scratch = core::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.push_str(&self.output[start..]);
        self.output.truncate(start);
        transform(&mut scratch, &mut self.output);
        self.scratch = scratch;
    }

    /// Rewrite each line of the output from `start` with `map`, which is given the line, its
    /// index, and whether it is empty, and writes the line last. The lines that a block quote
    /// being written records spaces for (`quote_lines`) are moved with their lines, so that it
    /// counts them where they end up.
    pub fn indent_lines(&mut self, start: usize, map: impl Fn(&str, usize, bool, &mut String)) {
        let moved = self
            .quote_lines
            .partition_point(|&(offset, _)| offset < start);
        let mut lines = core::mem::take(&mut self.quote_lines);
        let mut pending = lines[moved..].iter_mut().peekable();
        self.rewrite(start, |value, output| {
            let mut from = 0;
            let mut line = 0;
            // The last line counts as blank only where the whole value is empty, as a value ending
            // in a line ending has nothing after it that its container would indent.
            let mut write =
                |slice: &str, end: usize, line: usize, blank: bool, output: &mut String| {
                    map(slice, line, blank, output);
                    let content = output.len() - slice.len();
                    while let Some(entry) =
                        pending.next_if(|&&mut (offset, _)| offset - start <= end)
                    {
                        entry.0 = content + (entry.0 - start - (end - slice.len()));
                    }
                };
            for ending in line_endings(value) {
                let slice = &value[from..ending.start];
                write(slice, ending.start, line, slice.is_empty(), output);
                output.push_str(&value[ending.clone()]);
                from = ending.end;
                line += 1;
            }
            write(&value[from..], value.len(), line, value.is_empty(), output);
        });
        self.quote_lines = lines;
    }

    pub(super) fn default_separator<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        before: Option<NodeId>,
        left: NodeId,
        right: NodeId,
        parent: NodeId,
        spread: bool,
    ) -> &'static str {
        let left_kind = &document.node(left).unwrap().kind;
        let right_kind = &document.node(right).unwrap().kind;
        // The HTML block that `left` ends in, outside any block quote, which a line without `>`
        // ends, blank or not.
        let trailing = self
            .trailing_html(document, left)
            .filter(|&(_, quoted)| !quoted)
            .map(|(shape, _)| shape);
        // It holds the blank lines after it, which its containers continue over, and is written
        // without the line ending that this one gives back (#265).
        if trailing.is_some_and(|shape| shape.keeps_line_ending) {
            return "\n";
        }
        let unclosed = self.takes_blank_line(document, left);
        if self.indented_code(document, parent, spread, Some(left), right)
            && (matches!(left_kind, EventKind::Start(Tag::List { .. }))
                || self.indented_code(document, parent, spread, before, left))
        {
            return if unclosed {
                "\n<!---->\n\n"
            } else {
                "\n\n<!---->\n\n"
            };
        }
        if unclosed {
            return "\n";
        }
        // A paragraph or setext heading whose first line would start an HTML block has no other
        // spelling than on the line after a definition, which leaves the parser's paragraph open
        // to continue with it, indented so it starts no block (#193).
        if matches!(left_kind, EventKind::Definition { .. })
            && match right_kind {
                EventKind::Start(Tag::Paragraph { .. }) => true,
                EventKind::Start(Tag::Heading(_)) => {
                    self.setext_heading(document, parent, Some(left), right)
                }
                _ => false,
            }
            && starts_with_block_html(
                document,
                right,
                self.options.line_wrapping == LineWrapping::Unwrap,
            )
        {
            return "\n";
        }
        if in_list(document, parent) {
            // A block that would otherwise continue the one before it, or a block quote or table
            // after another, needs a blank line. That blank line makes the item read back spread, though
            // a list or footnote definition takes it into its span (#157), so `spreads_item`
            // writes the item's other blocks spread too, and `Document::normalize` makes a tight
            // list with such an item loose (#168).
            let setext = |heading| self.setext_heading(document, parent, Some(left), heading);
            if needs_blank_line(document, left, right, setext) {
                return "\n\n";
            }
            // A line of `>` alone ends a block quote's last paragraph without a blank line in the
            // item, so the item stays tight: `- > p\n  >\n  p` (#154). In a loose list, a blank
            // line is written after any list that a block which cannot interrupt a paragraph
            // follows.
            let continues = || {
                continues_text(document, right, setext)
                    || matches!(right_kind, EventKind::Start(Tag::Table))
            };
            match left_kind {
                EventKind::Start(Tag::List { .. })
                    if !in_tight_list(document, parent) && continues() =>
                {
                    return "\n\n";
                }
                EventKind::Start(Tag::BlockQuote)
                    if !spread && ends_in_paragraph(document, left) && continues() =>
                {
                    return QUOTE_END;
                }
                _ => {}
            }
            return if spread { "\n\n" } else { "\n" };
        }
        "\n\n"
    }

    /// Whether heading `heading`, a flow child of `parent` after flow child `before`, if any, is
    /// written as a setext heading, as only one with a line ending in its text must be
    /// (`format_heading_as_setext`). In an item of a tight list, a setext heading after a block it
    /// would otherwise continue, as `needs_blank_line` finds, could be written only after a blank
    /// line, which would make the list loose. An ATX heading interrupts a paragraph and ends the
    /// containers it would lazily continue, so a heading is written ATX there, with a line ending
    /// in its text written as `&#xA;`, unless it holds a soft or hard break, which only a setext
    /// heading can write, and for which `Document::normalize` makes the list loose instead:
    /// `- a\n  # h&#xA;i\n- q` (#168).
    fn setext_heading<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        before: Option<NodeId>,
        heading: NodeId,
    ) -> bool {
        format_heading_as_setext(document, heading, self)
            && !(in_tight_list(document, parent)
                && before
                    .is_some_and(|before| needs_blank_line(document, before, heading, |_| true))
                && !heading_as_setext(
                    document,
                    heading,
                    self.options.line_wrapping == LineWrapping::Unwrap,
                    true,
                ))
    }

    /// Whether code in `parent` after `before` needs a fence whatever the options: as
    /// [`fences_code`] says, or as the first block of an item whose list has no bullet that
    /// reads back with the code indented.
    fn fences<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        before: Option<NodeId>,
        spread: bool,
    ) -> bool {
        self.fences_code(document, parent, before, spread)
            || before.is_none()
                && document
                    .node(parent)
                    .unwrap()
                    .parent()
                    .is_some_and(|list| self.fenced_lists.contains(&list))
    }

    /// Whether flow child `id` of `parent`, after flow child `before`, if any, is code written
    /// indented. `spread` is whether `parent`'s blocks are joined by blank lines, as for
    /// `fences_code`.
    pub(super) fn indented_code<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        spread: bool,
        before: Option<NodeId>,
        id: NodeId,
    ) -> bool {
        if let EventKind::CodeBlock { value, info, .. } = &document.node(id).unwrap().kind {
            !self.fences(document, parent, before, spread)
                && format_code_as_indented(
                    trim_terminal_ending(value.resolve(document.source()).unwrap()),
                    info.is_none(),
                    self,
                )
        } else {
            false
        }
    }

    /// No JS equivalent.
    pub fn new(options: &'a Options) -> Self {
        State {
            output: String::new(),
            scratch: String::new(),
            escapes: Vec::new(),
            email_candidates: Vec::new(),
            applied: Vec::new(),
            email_escapes: Vec::new(),
            literal_markers: Vec::new(),
            alt_start: None,
            defer_literals: false,
            variants: Vec::new(),
            bullet_current: None,
            bullet_last_used: None,
            bullet_plan: Vec::new(),
            loose_end: None,
            fenced_lists: Vec::new(),
            index_stack: Vec::new(),
            options,
            parse: ParseOptions::default(),
            stack: Vec::new(),
            serialization_root: None,
            bindings: None,
            key: Scratch::default(),
            line_start: 0,
            label_end: None,
            definition_colon: None,
            fence_code: false,
            atx_heading: false,
            rule_after_text: false,
            joined_definition: None,
            task_item: None,
            depth: 0,
            current: None,
            unwritable: None,
            misfit: false,
            loose_links: false,
            raw_literals: false,
            verifying: false,
            prefixes: Vec::new(),
            position: Position::default(),
            frames: Vec::new(),
            last_item: None,
            quote_lines: Vec::new(),
            plans: super::plan::Plans::default(),
            shapes: core::cell::RefCell::new(Vec::new()),
            looseness: Vec::new(),
        }
    }

    /// The definition the reference `id` is bound to, and whether it is the one the reference
    /// reads back bound to once written: the first of its label in the subtree being written. A
    /// reference bound elsewhere is written as a resource to where it leads, and an unbound one,
    /// which reads back as text, as a reference.
    fn binding<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
    ) -> Option<(NodeId, bool)> {
        let cx = document.context();
        let root = self.serialization_root.unwrap_or(cx.root);
        let bindings = self.bindings.get_or_insert_with(|| {
            // At once for an attached subtree, so writing a deep one stays linear.
            let tree = document.tree_of(root);
            WrittenBindings {
                // Writing all of a tree looks up every reference in it.
                tree: Lookup::new(cx, tree),
                written: (root != tree).then(|| Lookup::new(cx, root)),
            }
        });
        let kind = &cx.nodes[id.index()].kind;
        let definition = bindings.tree.definition_of(cx, kind, &mut self.key)?;
        let Some(written) = &mut bindings.written else {
            return Some((definition, true));
        };
        let first = written.definition_of(cx, kind, &mut self.key);
        Some((definition, first == Some(definition)))
    }

    /// The destination and title of definition `id`.
    fn definition_target<S: AsRef<str>>(
        document: &Document<S>,
        id: NodeId,
    ) -> (&str, Option<&str>) {
        let source = document.source();
        let EventKind::Definition {
            destination, title, ..
        } = &document.node(id).unwrap().kind
        else {
            unreachable!("a binding is a definition")
        };
        (
            destination.resolve(source).unwrap(),
            title.as_ref().map(|value| value.resolve(source).unwrap()),
        )
    }

    /// The destination and title link or image `id`, of `target`, is written with as a resource:
    /// its own, or those of the definition it is bound to, or `None` for a reference written as
    /// one.
    fn written_target<'d, S: AsRef<str>>(
        &mut self,
        document: &'d Document<S>,
        id: NodeId,
        target: &'d Target<'static>,
    ) -> Result<(&'d str, Option<&'d str>), Option<NodeId>> {
        let source = document.source();
        if let Some((destination, title)) = target.destination() {
            return Ok((
                destination.resolve(source).unwrap(),
                title.map(|value| value.resolve(source).unwrap()),
            ));
        }
        match self.binding(document, id) {
            Some((definition, false)) => Ok(Self::definition_target(document, definition)),
            Some((definition, true)) => Err(Some(definition)),
            None => Err(None),
        }
    }

    /// The first character `id` writes, when it is known without writing it. `previous` is the node
    /// written before it.
    fn peek_node<S: AsRef<str>>(
        &mut self,
        document: &Document<S>,
        id: NodeId,
        previous: NodeId,
    ) -> Option<char> {
        match &document.node(id).unwrap().kind {
            // A soft break is written as a line ending, which is never a literal prefix.
            EventKind::Text(value) => {
                super::escape::literal_prefix(value.resolve(document.source()).unwrap())
            }
            EventKind::Start(Tag::Emphasis) => Some(self.options.emphasis),
            EventKind::Start(Tag::Strong) => Some(self.options.strong),
            EventKind::Start(Tag::Delete) => Some('~'),
            EventKind::Start(Tag::Image { .. }) => Some('!'),
            EventKind::Start(Tag::Link(target)) => Some(
                if let Ok((destination, title)) = self.written_target(document, id, target) {
                    match written_link(
                        document,
                        id,
                        destination,
                        title.is_some(),
                        ends_at_boundary(document, previous),
                        self,
                    ) {
                        WrittenLink::Bare(text) | WrittenLink::Loose(text) => {
                            return text.chars().next()
                        }
                        WrittenLink::Angle(_) => '<',
                        WrittenLink::Explicit => '[',
                    }
                } else {
                    '['
                },
            ),
            EventKind::Html { .. } => Some('<'),
            // Code with a language starts with it (#185), which attention next to it judges.
            EventKind::Code { language, .. } => Some(
                language
                    .as_ref()
                    .and_then(|language| {
                        language.resolve(document.source()).unwrap().chars().next()
                    })
                    .unwrap_or('`'),
            ),
            _ => None,
        }
    }

    pub fn write_safe(&mut self, input: &str, info: &Info, encode: Option<char>) {
        let before = info.before(&self.output);
        let start = before[0].len() + before[1].len();
        let end = start + input.len();
        self.escapes.clear();
        super::escape::collect(
            before,
            input,
            info.after,
            &self.stack,
            &self.parse.constructs,
            &mut self.escapes,
        );
        self.escapes.sort_unstable_by_key(|entry| entry.0);
        self.escapes.dedup_by(|right, left| {
            if right.0 == left.0 {
                left.1.before &= right.1.before;
                left.1.after &= right.1.after;
                left.1.email &= right.1.email;
                true
            } else {
                false
            }
        });
        self.email_candidates.clear();
        let candidates = &mut self.email_candidates;
        self.escapes.retain(|&(position, ref flags)| {
            if flags.email {
                candidates.push(position);
            }
            !flags.email
        });
        self.applied.clear();
        let mut skip_until = 0;
        let before_last = before[1]
            .chars()
            .next_back()
            .or_else(|| before[0].chars().next_back());
        for (index, &(position, ref flags)) in self.escapes.iter().enumerate() {
            if position < start || position >= end || position < skip_until {
                continue;
            }
            if let Some(run_end) =
                intraword_underscores(&self.escapes, index, start, before_last, input, info.after)
            {
                skip_until = run_end;
                continue;
            }
            if self.escapes.get(index + 1).is_some_and(|next| {
                position + 1 < end
                    && next.0 == position + 1
                    && flags.after
                    && !next.1.before
                    && !next.1.after
            }) || index > 0
                // An escape before the input is in output already written, where it was not
                // applied, so it cannot stand in for this one.
                && self.escapes[index - 1].0 >= start
                && self.escapes[index - 1].0 + 1 == position
                && flags.before
                && !self.escapes[index - 1].1.before
                && !self.escapes[index - 1].1.after
            {
                continue;
            }
            self.applied.push(position);
        }
        // Whether a `~` pairs depends on its neighbours as written, so decide them afterwards.
        if self.escapes_for(self.parse.constructs.strikethrough)
            && super::escape::strikethrough(&self.stack)
        {
            super::escape::tildes(
                input,
                start,
                [before_last, info.after.chars().next()],
                &mut self.applied,
            );
        }
        // Email-only escapes depend on which other escapes apply, so decide them afterwards.
        let applied = self.applied.len();
        for &position in &self.email_candidates {
            if position >= start
                && position < end
                && starts_email(
                    &input[position - start..],
                    position,
                    &self.applied[..applied],
                )
            {
                self.applied.push(position);
            }
        }
        if self.applied.len() > applied {
            self.applied.sort_unstable();
        }
        // A literal autolink written where it is not followed by a boundary would take the text
        // after it, so punctuation that starts that is escaped, which ends the link, as the
        // parser reads one in text that an escape splits. A reference does not end one: the
        // parser takes it into the link as written.
        if self.loose_end == Some(self.output.len())
            && self.parse.constructs.character_escape
            && input
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_punctuation())
            && self.applied.first() != Some(&start)
        {
            self.applied.insert(0, start);
        }
        let written = self.output.len();
        self.email_escapes.clear();
        let literal = self.literal();
        let mut unwritable = false;
        let mut cursor = start;
        for &position in &self.applied {
            super::util::safe::write_backslashes(
                &input[cursor - start..position - start],
                "\\",
                literal.backslash,
                &mut self.output,
            );
            cursor = position;
            let character = input[position - start..]
                .chars()
                .next()
                .expect("escape positions are character boundaries");
            if character.is_ascii_punctuation() && encode != Some(character) && literal.backslash {
                if self.email_candidates.binary_search(&position).is_ok() {
                    self.email_escapes.push(self.output.len() - written);
                }
                self.output.push('\\');
            } else {
                // Where no character can be escaped, a line of a paragraph or heading after its
                // first is indented instead, which keeps what it starts with from starting a
                // block, as no block but indented code starts indented 4 spaces. That reads back
                // unless the character means something in phrasing, which the block's check
                // finds.
                if !literal.backslash
                    && !literal.reference
                    && self.parse.constructs.code_indented
                    && self.verifying
                    && self.output.len() > self.line_start
                    && self.output.ends_with('\n')
                {
                    self.output.push_str("    ");
                }
                if !literal.write(&mut self.output, character, false) {
                    unwritable = true;
                }
                cursor += character.len_utf8();
            }
        }
        super::util::safe::write_backslashes(
            &input[cursor - start..],
            info.after,
            literal.backslash,
            &mut self.output,
        );
        if unwritable {
            self.raw_literal();
        }
        if !self.defer_literals && self.literals() {
            self.escape_literals(written, info);
        }
    }

    /// Whether text written now can form GFM literal autolinks, under the document's options.
    pub(super) fn literals(&self) -> bool {
        self.escapes_for(self.parse.constructs.autolink_literal)
            && super::escape::autolink_literals(&self.stack)
    }

    /// Whether what would start or end a construct that the document's options turn on or off,
    /// as `on` says, is escaped: always where backslash escapes are read, so the output reads
    /// the same under other options too, and otherwise, where an escape is a reference or
    /// cannot be written, only for a construct they read (contract 3.3).
    pub(super) fn escapes_for(&self, on: bool) -> bool {
        on || self.parse.constructs.character_escape
    }

    /// Escape what makes literal autolinks of the text written from `start`. Text is literal, so
    /// it never reads back as a link. A literal can start in text siblings written before it,
    /// which a parsed tree that is not normalized has at escapes, such as `www\\.a`.
    ///
    /// An escape in an email autolink keeps it from forming, so the escape before its `<` is
    /// dropped when one of these is in it.
    fn escape_literals(&mut self, start: usize, info: &Info) {
        let run = info.text_run.unwrap_or(start);
        let before = match &info.before {
            Before::Literal(value) if run == start => value.chars().next_back(),
            Before::Output(lead, range) if run == range.start => lead.chars().next_back(),
            _ => self.output[..run].chars().next_back(),
        };
        let mut markers = core::mem::take(&mut self.literal_markers);
        markers.clear();
        crate::inline_parser::literal_markers(
            &self.output[run..],
            start - run,
            before.is_none_or(|character| {
                character.is_ascii() && crate::inline_parser::www_previous(character as u8)
            }),
            &mut markers,
        );
        if !markers.is_empty() {
            let shift = start - run;
            for marker in &mut markers {
                *marker -= shift;
            }
            let output = &self.output[start..];
            let mut dropped = core::mem::take(&mut self.email_escapes);
            // Drop the escapes before autolinks that a marker is in. The autolink holds no other
            // escapes, so it runs to the next `>`.
            let mut next = 0;
            dropped.retain(|&escape| {
                let end = output[escape..]
                    .find('>')
                    .map_or(output.len(), |end| escape + end);
                while markers.get(next).is_some_and(|&marker| marker < escape) {
                    next += 1;
                }
                markers.get(next).is_some_and(|&marker| marker < end)
            });
            // A character reference ends the text a literal is found in as a backslash does.
            let literal = self.literal();
            let mut unwritable = false;
            self.rewrite(start, |value, output| {
                let mut dropped = dropped.iter().peekable();
                let mut cursor = 0;
                for &marker in &markers {
                    while let Some(&escape) = dropped.next_if(|&&escape| escape < marker) {
                        output.push_str(&value[cursor..escape]);
                        cursor = escape + 1;
                    }
                    output.push_str(&value[cursor..marker]);
                    let character = char::from(value.as_bytes()[marker]);
                    unwritable |= !literal.write(output, character, true);
                    cursor = marker + 1;
                }
                output.push_str(&value[cursor..]);
            });
            if unwritable {
                self.raw_literal();
            }
            self.email_escapes = dropped;
        }
        self.literal_markers = markers;
    }

    /// Write a label as it is: a label in a document is one the parser reads, as
    /// `node_syntax::valid_label` checks, a normalized one reads back as itself when written
    /// so, and an escape would change it.
    pub(super) fn write_label(&mut self, label: &str) {
        self.output.push_str(label);
    }
}

/// Whether the `<` that starts `rest`, at output `position`, opens an email autolink within this
/// text as written. An applied escape inserts a backslash, which ends the autolink, and the scan
/// stops at the next `<` because no autolink contains one.
fn starts_email(rest: &str, position: usize, applied: &[usize]) -> bool {
    let mut link = crate::util::autolink::Autolink::default();
    for (offset, &byte) in rest.as_bytes().iter().enumerate().skip(1) {
        if applied.binary_search(&(position + offset)).is_ok() {
            return false;
        }
        match link.advance(byte) {
            None => return false,
            Some(true) => return link.email(),
            Some(false) => {}
        }
    }
    false
}

/// How many times `State::respell` writes a block again at most, which keeps writing linear.
const RESPELL_TRIES: usize = 64;

/// A paragraph, heading or table cell being written, as `State::block_phrasing` writes it.
#[derive(Clone, Copy)]
struct BlockSpelling<'n, 'i> {
    block: NodeId,
    children: &'n [NodeId],
    info: &'i Info<'i>,
    /// Where its phrasing starts in the output.
    start: usize,
    joined_definition: Option<(usize, bool)>,
    /// Whether it is a paragraph or heading, which escapes what would start another block.
    written: bool,
}

/// What `State::marks` records.
struct Marks {
    unwritable: Option<NodeId>,
    misfit: bool,
    loose_links: bool,
    raw_literals: bool,
    loose_end: Option<usize>,
    label_end: Option<(usize, bool)>,
    definition_colon: Option<usize>,
}

/// Whether `id` is emphasis, strong or strikethrough.
fn is_attention<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Emphasis | Tag::Strong | Tag::Delete)
    )
}

/// Whether `block` holds attention in attention, or attention directly before other attention,
/// whose runs of markers can pair otherwise than the tree's, so that it is read back once
/// written.
fn nested_attention<S: AsRef<str>>(document: &Document<S>, block: NodeId) -> bool {
    document.descendants(block).any(|id| {
        is_attention(document, id)
            && (flow_children(document, id).any(|child| is_attention(document, child))
                || document
                    .node(id)
                    .unwrap()
                    .next_sibling()
                    .is_some_and(|next| is_attention(document, next)))
    })
}

/// Whether `block` holds a reference or footnote call, which reads back only with the
/// definitions it is bound to.
fn bound<S: AsRef<str>>(document: &Document<S>, block: NodeId) -> bool {
    document.subtree_events(block).any(|event| {
        matches!(
            event.map(|event| event.kind),
            Ok(EventKind::FootnoteReference { .. }
                | EventKind::Start(
                    Tag::Link(Target::Resolved(_) | Target::Reference(_))
                        | Tag::Image(Target::Resolved(_) | Target::Reference(_))
                ))
        )
    })
}

/// What a phrasing child was written after.
enum Previous {
    /// Attention, whose closing sequence ends the output.
    Attention,
    /// Text, which begins at this output offset.
    Text(usize),
    /// Nothing: the child starts its parent.
    Start,
    /// Anything else.
    Other,
}

/// The first or last character a phrasing node is written with, as far as attention next to it needs: text is approximated by its value and other nodes by punctuation.
/// Whether `id` is code with a language, which is written first, as it is.
fn language<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    matches!(
        document.node(id).unwrap().kind,
        EventKind::Code {
            language: Some(_),
            ..
        }
    )
}

fn edge_character<S: AsRef<str>>(document: &Document<S>, id: NodeId, first: bool) -> Option<char> {
    match &document.node(id).unwrap().kind {
        EventKind::Text(value) => {
            let value = value.resolve(document.source()).unwrap();
            if first {
                value.chars().next()
            } else {
                value.chars().next_back()
            }
        }
        // Other nodes start and end with punctuation, such as the backslash a hard break starts with. Normalized attention does not end with a hard break, or start or end with a soft break.
        _ => Some('.'),
    }
}

pub(crate) fn is_phrasing(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(
            Tag::Delete | Tag::Emphasis | Tag::Image { .. } | Tag::Link { .. } | Tag::Strong
        ) | EventKind::HardBreak
            | EventKind::SoftBreak(_)
            | EventKind::FootnoteReference { .. }
            | EventKind::Code { .. }
            | EventKind::Text(_)
            | EventKind::Html { block: false, .. }
    )
}

/// Whether `id`, if it is a list, can interrupt a paragraph: it starts with an item that is not
/// empty, not counting separators and empty paragraphs, and not written with a blank line first,
/// as `item_starts_blank` says, and, if ordered, starts at 1. A node that is not a list counts as
/// one that can.
fn list_interrupts_paragraph<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    let EventKind::Start(Tag::List { start, .. }) = document.node(id).unwrap().kind else {
        return true;
    };
    start.is_none_or(|start| start == 1)
        && flow_children(document, id).next().is_none_or(|item| {
            !item_starts_blank(document, item)
                && flow_children(document, item).any(|block| {
                    !matches!(
                        document.node(block).unwrap().kind,
                        EventKind::Start(Tag::Paragraph { .. })
                    ) || document.children(block).next().is_some()
                })
        })
}

/// Whether item `item` must start with a blank line: it is empty, or starts with an HTML block
/// with indentation that the marker's padding would take, which the parser reads after a padding
/// of one only where it is 4 columns or more, which only options without indented code allow. Its
/// content then starts one column after the marker. An item that starts with an HTML block
/// indented 4 columns or more at some column can start after its marker (`html_spaces`), or with
/// a blank line where that fits no column it can be written at (`plan.rs`).
pub(super) fn item_starts_blank<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> bool {
    let Some(first) = flow_children(document, item).next() else {
        return true;
    };
    match &document.node(first).unwrap().kind {
        EventKind::Html { value, block: true } => {
            let value = value.resolve(document.source()).unwrap().as_bytes();
            matches!(value.first(), Some(b' ' | b'\t'))
                && (document.parse_options().constructs.code_indented
                    || (0..4).all(|column| indentation_width(value, column) < 4))
        }
        _ => false,
    }
}

/// Whether `parent`, whose flow children are being joined, is a list or item, whose joins are a
/// single line ending when it records no blank line between its blocks.
pub(super) fn in_list<S: AsRef<str>>(document: &Document<S>, parent: NodeId) -> bool {
    matches!(
        document.node(parent).unwrap().kind,
        EventKind::Start(Tag::Item { .. } | Tag::List { .. })
    )
}

/// Whether paragraph or heading `id` would start an HTML block if written at the start of a line:
/// its first child is inline HTML of kinds 1–6, or a complete tag with nothing after it on its
/// line but whitespace, which starts one of kind 7. With `unwrap`, a soft break or a line ending
/// in text is written as a space, which does not end the line.
fn starts_with_block_html<S: AsRef<str>>(document: &Document<S>, id: NodeId, unwrap: bool) -> bool {
    let source = document.source();
    let mut children = flow_children(document, id);
    let Some(EventKind::Html { value, .. }) = children
        .next()
        .map(|first| &document.node(first).unwrap().kind)
    else {
        return false;
    };
    let value = value.resolve(source).unwrap().as_bytes();
    let options = document.parse_options();
    crate::block_parser::html_interrupts_paragraph(value, options)
        || crate::block_parser::complete_tag_starts_html_block(value, options)
            && match children
                .next()
                .map(|next| &document.node(next).unwrap().kind)
            {
                None => true,
                Some(EventKind::SoftBreak(_)) => !unwrap,
                Some(EventKind::Text(text)) => {
                    // Spaces or tabs before the line ending are encoded, so they
                    // stay on the tag's line and it is no longer alone there.
                    !unwrap && text.resolve(source).unwrap().starts_with('\n')
                }
                Some(_) => false,
            }
}

/// The lengths of the runs of tildes in `text`.
fn tilde_runs(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.split(|character| character != '~')
        .map(str::len)
        .filter(|&run| run > 0)
}

/// Whether flow child `id` is a link reference definition.
fn is_definition<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    matches!(
        document.node(id).unwrap().kind,
        EventKind::Definition { .. }
    )
}

/// The join after a block quote whose last paragraph the next block would lazily continue, in an
/// item whose blocks are not written spread: a line of `>` alone, which ends the paragraph inside
/// the quote rather than with a blank line in the item.
const QUOTE_END: &str = "\n>\n";

/// Whether a join between flow children writes a blank line, which an item records as spread.
/// `QUOTE_END` does not: its line of `>` is inside the block quote.
pub(super) fn writes_blank_line(separator: &str) -> bool {
    separator.starts_with("\n\n")
}

/// Whether `item`, an item or list whose blocks are being joined, is in a tight list. Only an item
/// holds a list as one of its blocks.
fn in_tight_list<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> bool {
    document.node(item).unwrap().parent().is_some_and(|list| {
        matches!(
            document.node(list).unwrap().kind,
            EventKind::Start(Tag::List { tight: true, .. })
        )
    })
}

/// Whether the last block of container `id`, through the last blocks of any containers in it, is a
/// paragraph or a link reference definition, which a following line could lazily continue. Only
/// last children are visited, which never come before another block, so each node is visited for
/// at most one container.
fn ends_in_paragraph<S: AsRef<str>>(document: &Document<S>, mut id: NodeId) -> bool {
    while let Some(last) = flow_children(document, id).last() {
        match document.node(last).unwrap().kind {
            EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. } => return true,
            EventKind::Start(
                Tag::BlockQuote
                | Tag::List { .. }
                | Tag::Item { .. }
                | Tag::FootnoteDefinition { .. },
            ) => id = last,
            _ => return false,
        }
    }
    false
}

/// Whether the last flow child of block quote `quote` is a list or footnote definition. A line of
/// `>` alone after it is a blank line in that list's last item or in the definition, which reads
/// back as a blank line at the end of the quote's item (#157), so the join is a blank line instead.
fn ends_in_list_or_footnote_definition<S: AsRef<str>>(
    document: &Document<S>,
    quote: NodeId,
) -> bool {
    flow_children(document, quote).last().is_some_and(|last| {
        matches!(
            document.node(last).unwrap().kind,
            EventKind::Start(Tag::List { .. } | Tag::FootnoteDefinition { .. })
        )
    })
}

/// Whether flow children `left` and `right` of an item, in a row, can be kept apart only by a blank
/// line between them, which makes the item read back spread and so its list loose. `setext` is
/// whether a heading is written as a setext heading after `left`.
///
/// This is the one rule for these joins: the serializer writes a blank line for them, and
/// `Document::normalize` makes a tight list loose when one of its items holds such a pair, other
/// than two paragraphs, which it joins (#154, #168). So the two cannot disagree.
///
/// - Two block quotes in a row would read back as one, and so would two tables, the second's head
///   and delimiter row read as rows of the first.
/// - A paragraph, link reference definition, setext heading or HTML block of kind 7 cannot
///   interrupt a paragraph: after one, it would continue it, as text, a title, the heading's
///   content or inline HTML (#232). After a table, each line of the first three would be a row,
///   but an HTML block can start there. The parser gives none of these after a table, but
///   normalizing can, when it hoists a definition out of a footnote definition that is a copy.
/// - A link reference definition is read from the front of a paragraph when the paragraph
///   closes, so the line after it continues that paragraph unless it interrupts one (#249). A
///   paragraph, definition or setext heading after it is read as before, but an HTML block of
///   kind 7 would be inline HTML in the paragraph's text. A table after it is read as before, as
///   the writer escapes a head row that would be read as a delimiter row under the line before.
/// - An HTML block of kind 6 or 7 ends only at a blank line, and one of kinds 1–5 at the line
///   that holds its end condition, so unless that is its last line, it would take any block
///   after it as more of its lines (#191).
/// - After a nested list or footnote definition that ends in a paragraph or definition, through
///   the last blocks of any containers in it, any of those or a table would continue that
///   paragraph lazily, as a table's rows cannot interrupt a lazy paragraph. So would one after a
///   block quote whose last block is such a list or definition, where the line of `>` alone that
///   ends a quote's paragraph (`QUOTE_END`) is a blank line in that list or definition.
///
/// A list or definition that ends in anything else, such as a heading, is ended by the next line,
/// so the block after it is written on that line: `- a\n  - # h\n  b` (#157).
pub(crate) fn needs_blank_line<S: AsRef<str>>(
    document: &Document<S>,
    left: NodeId,
    right: NodeId,
    setext: impl Fn(NodeId) -> bool,
) -> bool {
    let right_kind = &document.node(right).unwrap().kind;
    let lazy = || {
        (continues_text(document, right, &setext)
            || matches!(right_kind, EventKind::Start(Tag::Table)))
            && ends_in_paragraph(document, left)
    };
    match &document.node(left).unwrap().kind {
        EventKind::Start(Tag::Paragraph { .. }) => {
            continues_text(document, right, &setext)
                || matches!(right_kind, EventKind::Html { value, block: true }
                if !crate::block_parser::html_interrupts_paragraph(
                    value.resolve(document.source()).unwrap()
                        .trim_start_matches([' ', '\t']).as_bytes(),
                    document.parse_options(),
                ))
        }
        // A definition is read from the front of a paragraph, which the next line continues
        // unless it interrupts it, as an HTML block of kind 7 written there does not.
        EventKind::Definition { .. } => {
            matches!(right_kind, EventKind::Html { value, block: true }
            if !crate::block_parser::html_interrupts_paragraph(
                value.resolve(document.source()).unwrap()
                    .trim_start_matches([' ', '\t']).as_bytes(),
                document.parse_options(),
            ))
        }
        EventKind::Start(Tag::Table) => {
            continues_text(document, right, &setext)
                || matches!(right_kind, EventKind::Start(Tag::Table))
        }
        EventKind::Html { value, block: true } => !HtmlBlockShape::of(
            value.resolve(document.source()).unwrap(),
            document.parse_options(),
        )
        .is_some_and(HtmlBlockShape::ends_on_last_line),
        EventKind::Start(Tag::BlockQuote) => {
            matches!(right_kind, EventKind::Start(Tag::BlockQuote))
                || ends_in_list_or_footnote_definition(document, left) && lazy()
        }
        EventKind::Start(Tag::List { .. } | Tag::FootnoteDefinition { .. }) => lazy(),
        _ => false,
    }
}

/// The content columns an HTML block can start at where it is written: those its shape fits,
/// and, after a block that a line indented as far as its first line would continue, those where
/// that indentation is narrower (`below`). A footnote definition takes a line indented 4 columns
/// or more after a blank line, which only options without indented code give an HTML block
/// (#275), an HTML block that keeps its blank lines one indented as far as its own lines are, at
/// least one column, and an item one indented as wide as it is (`Position::prev`, #229, #262).
#[derive(Clone, Copy)]
pub(super) struct HtmlFit<'a> {
    pub(super) shape: HtmlBlockShape,
    value: &'a [u8],
    /// The indentation from which the block before it would continue over its first line.
    below: Option<usize>,
}

impl HtmlFit<'_> {
    /// The width of its first line's indentation at `column`.
    pub(super) fn indentation(self, column: usize) -> usize {
        indentation_width(self.value, column)
    }

    pub(super) fn fits(self, column: usize) -> bool {
        self.shape.fits(column)
            && self
                .below
                .is_none_or(|below| self.indentation(column) < below)
    }
}

/// What flow child `before`, whose last item, if it is or ends in one, is `width` wide, leaves
/// for the line after it (`Position::prev`): that width, unless that item is empty and the join
/// after it is a `blank` line, which ends it.
pub(super) fn left_width<S: AsRef<str>>(
    document: &Document<S>,
    before: NodeId,
    width: Option<usize>,
    blank: bool,
) -> Option<usize> {
    let last = match document.node(before).unwrap().kind {
        EventKind::Start(Tag::Item { .. }) => before,
        EventKind::Start(Tag::List { .. }) => flow_children(document, before).last()?,
        _ => return None,
    };
    width.filter(|_| !(blank && flow_children(document, last).next().is_none()))
}

/// Whether flow child `right` is a paragraph, link reference definition or a heading written as a
/// setext heading, as `setext` says, none of which can interrupt a paragraph.
fn continues_text<S: AsRef<str>>(
    document: &Document<S>,
    right: NodeId,
    setext: impl Fn(NodeId) -> bool,
) -> bool {
    match document.node(right).unwrap().kind {
        EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. } => true,
        EventKind::Start(Tag::Heading(_)) => setext(right),
        _ => false,
    }
}

pub(super) fn flow_children<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
) -> impl Iterator<Item = NodeId> + Clone + '_ {
    document.children(id).filter(move |child| {
        !matches!(
            document.node(*child).unwrap().kind,
            EventKind::Separator { .. }
        )
    })
}

/// The inline events of the first block in `events` that is a paragraph, or a table cell when
/// `cell`, with their values as strings and adjacent text joined, when it is the only block or
/// cell. Separators are left out, and a soft break is text of a line ending, as text holding
/// one renders the same, however the line ending is written, but for a soft break after a hard
/// break, which is the line ending the parser keeps after one, and which the writer leaves out.
fn inline_events<'d, S: AsRef<str>>(
    document: &'d Document<S>,
    events: impl Iterator<Item = Result<crate::stream::Event<'d>, crate::stream::StreamError>>,
    cell: bool,
) -> Option<Vec<EventKind<'static>>> {
    use crate::stream::Text;
    let source = document.source();
    let mut inner = Vec::new();
    let mut depth = 0usize;
    let mut blocks = 0;
    let mut after_hard_break = false;
    for event in events {
        let kind = event.expect("a document's events are read").kind;
        let outer = match &kind {
            EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_)) => !cell,
            EventKind::Start(
                Tag::List { .. }
                | Tag::Item { .. }
                | Tag::BlockQuote
                | Tag::FootnoteDefinition { .. },
            ) if depth == 0 => continue,
            EventKind::Start(Tag::TableCell { .. }) => cell,
            _ => false,
        };
        if depth == 0 {
            if outer {
                blocks += 1;
                depth = 1;
            } else if !matches!(
                kind,
                EventKind::Start(
                    Tag::Document { .. } | Tag::Table | Tag::TableHead | Tag::TableRow
                ) | EventKind::End(_)
                    | EventKind::Separator { .. }
            ) {
                return None;
            }
            continue;
        }
        match &kind {
            EventKind::Start(_) => depth += 1,
            EventKind::End(_) => {
                depth -= 1;
                if depth == 0 {
                    continue;
                }
            }
            _ => {}
        }
        // The writer writes no soft break after a hard break, whose line ending it is.
        let soft = matches!(kind, EventKind::SoftBreak(_));
        if core::mem::replace(&mut after_hard_break, matches!(kind, EventKind::HardBreak)) && soft {
            continue;
        }
        let kind = match kind {
            // A soft break renders as the line ending it is, which text can hold too.
            EventKind::SoftBreak(_) => {
                EventKind::Text(Text::Value(alloc::borrow::Cow::Borrowed("\n")))
            }
            kind => kind.map_text(|text| {
                Text::Value(alloc::borrow::Cow::Owned(String::from(
                    text.resolve(source).unwrap_or_default(),
                )))
            }),
        };
        if let (EventKind::Text(text), Some(EventKind::Text(last))) = (&kind, inner.last_mut()) {
            let joined = alloc::format!(
                "{}{}",
                last.resolve("").unwrap_or_default(),
                text.resolve("").unwrap_or_default()
            );
            *last = Text::Value(alloc::borrow::Cow::Owned(joined));
            continue;
        }
        inner.push(kind);
    }
    (blocks == 1).then_some(inner)
}

/// The destination and text of each link in `events`, in order.
fn links<'d, S: AsRef<str>>(
    document: &'d Document<S>,
    events: impl Iterator<Item = Result<crate::stream::Event<'d>, crate::stream::StreamError>> + 'd,
) -> impl Iterator<Item = (String, String)> + 'd {
    let source = document.source();
    let mut link: Option<(String, String)> = None;
    events.filter_map(
        move |event| match event.expect("a document's events are read").kind {
            EventKind::Start(Tag::Link(target)) => {
                let destination = target
                    .destination()
                    .map(|(destination, _)| {
                        String::from(destination.resolve(source).unwrap_or_default())
                    })
                    .unwrap_or_default();
                link = Some((destination, String::new()));
                None
            }
            EventKind::Text(text) => {
                if let Some((_, value)) = &mut link {
                    value.push_str(text.resolve(source).unwrap_or_default());
                }
                None
            }
            EventKind::End(crate::stream::TagKind::Link) => link.take(),
            _ => None,
        },
    )
}

/// How deep the serializer nests: a node more than this many levels below the serialized node,
/// counting every block and inline node between them, is an error rather than a deeper recursion
/// that could overflow the stack.
pub(super) const MAX_NESTING: usize = 128;

fn nesting_too_deep() -> Message {
    Message {
        span: None,
        reason: alloc::format!(
            "Content nested more than {MAX_NESTING} levels deep cannot be serialized"
        )
        .into(),
        rule_id: "nesting-too-deep",
        source: "markdown",
    }
}

/// The largest number an ordered list marker can have: CommonMark allows at most 9 digits.
pub(super) const MAX_LIST_NUMBER: u64 = 999_999_999;

/// A list item whose number, its list's start plus, when `increment_list_marker` is on, its
/// position among the list's items, would need more than 9 digits, which reads back as text.
pub(super) fn list_number_too_long(number: u64) -> Message {
    Message {
        span: None,
        reason: alloc::format!(
            "List item number {number} has more than 9 digits, so it cannot be written as a list marker"
        )
        .into(),
        rule_id: "list-number-too-long",
        source: "markdown",
    }
}

/// A node that no spelling writes so that it reads back under the document's options, as
/// contract 3.3 lists.
pub(super) fn unwritable_node<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> Message {
    let node = document.node(id).expect("an unwritable node exists");
    Message {
        span: Some(node.span.clone()),
        reason: alloc::format!(
            "{id:?} has no spelling that reads back under the document's parse options"
        )
        .into(),
        rule_id: "unwritable-node",
        source: "markdown",
    }
}

pub(super) fn invalid_node() -> Message {
    Message {
        span: None,
        reason: "Node does not belong to the document".into(),
        rule_id: "invalid-node",
        source: "markdown",
    }
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;

/// Returns the end of a run of underscores between word characters (`a_b`, `a__b`).
/// Such runs cannot form attention, so they stay unescaped, as in `mdast-util-to-markdown`.
fn intraword_underscores(
    escapes: &[(usize, EscapeInfos)],
    index: usize,
    start: usize,
    before_last: Option<char>,
    input: &str,
    after: &str,
) -> Option<usize> {
    use crate::util::char::{classify, Kind};
    let end = start + input.len();
    let position = escapes[index].0;
    if input.as_bytes()[position - start] != b'_' {
        return None;
    }
    let escaped = |at: usize| escapes.binary_search_by_key(&at, |entry| entry.0).is_ok();
    let previous = if position > start {
        input[..position - start].chars().next_back()
    } else {
        before_last
    }?;
    // Underscores next to characters that attention encodes as references are escaped by `attention::encode_end` and `attention::edit_start`.
    if classify(previous) != Kind::Other || escaped(position - previous.len_utf8()) {
        return None;
    }
    let run_end = position
        + input.as_bytes()[position - start..]
            .iter()
            .take_while(|&&byte| byte == b'_')
            .count();
    // A missing next character counts as whitespace, so the run stays escaped.
    let next = if run_end < end {
        input[run_end - start..].chars().next()
    } else {
        after.chars().next()
    }
    .unwrap_or(' ');
    let skip = run_end - position - 1;
    (escapes.get(index + skip).map(|entry| entry.0) == Some(run_end - 1)
        && classify(next) == Kind::Other
        && !escaped(run_end))
    .then_some(run_end)
}

impl State<'_> {
    /// The value of HTML block `id` and its shape under the options of `document`, or `None`
    /// where `id` is no HTML block or its value starts none, which only a hand-built tree has,
    /// kept for the write, as the separators and the plan ask for it often.
    pub(super) fn html_parts<'d, S: AsRef<str>>(
        &self,
        document: &'d Document<S>,
        id: NodeId,
    ) -> Option<(&'d str, HtmlBlockShape)> {
        let EventKind::Html { value, block: true } = &document.node(id).unwrap().kind else {
            return None;
        };
        let value = value.resolve(document.source()).unwrap();
        let mut shapes = self.shapes.borrow_mut();
        if shapes.len() <= id.index() {
            shapes.resize(id.index() + 1, None);
        }
        let shape = *shapes[id.index()]
            .get_or_insert_with(|| HtmlBlockShape::of(value, document.parse_options()));
        shape.map(|shape| (value, shape))
    }

    /// The shape of HTML block `id` (`html_parts`).
    pub(super) fn shape<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        id: NodeId,
    ) -> Option<HtmlBlockShape> {
        self.html_parts(document, id).map(|(_, shape)| shape)
    }

    /// Where HTML block `id`, starting at `position`, can start, or `None` where it is no HTML block.
    pub(super) fn html_fit<'d, S: AsRef<str>>(
        &self,
        document: &'d Document<S>,
        id: NodeId,
        position: Position,
    ) -> Option<HtmlFit<'d>> {
        self.html_parts(document, id).map(|(value, shape)| {
            let continued =
                position
                    .before
                    .and_then(|before| match document.node(before).unwrap().kind {
                        EventKind::Start(Tag::FootnoteDefinition { .. }) => Some(4),
                        _ => self.shape(document, before)?.continued_from,
                    });
            // Only a list or item before leaves a width, and neither continues from an indentation.
            HtmlFit {
                shape,
                value: value.as_bytes(),
                below: continued.or(position.prev),
            }
        })
    }

    /// The spaces before the `>` of the block quote it is in that HTML block `id` takes to start at a
    /// column it fits from `position` (`HtmlFit`), as few as can, or `None` where none does. Only a
    /// line that `Position::shift` says can take them has any. After a footnote definition's label,
    /// which would take the indentation, a block with indentation fits no column, nor after an
    /// item's marker, whose padding would take it, unless it is 4 columns or more, after which the
    /// parser reads a padding of one, which the item then has (`plan.rs`).
    pub(super) fn html_spaces<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        id: NodeId,
        position: Position,
    ) -> Option<usize> {
        self.html_fit(document, id, position).and_then(|fit| {
            let indented = matches!(fit.value.first(), Some(b' ' | b'\t'));
            if indented && position.after == After::Label {
                return None;
            }
            (0..=if position.shift { 3 } else { 0 }).find(|&spaces| {
                let column = position.first + spaces;
                fit.fits(column)
                    && !(indented && position.after == After::Marker && fit.indentation(column) < 4)
            })
        })
    }

    /// The indentation from which HTML block `before`, if it is one that keeps its blank lines,
    /// would continue over the line after it: the spaces before a marker or `>` there must be fewer.
    pub(super) fn continued_from<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        before: Option<NodeId>,
    ) -> Option<usize> {
        self.shape(document, before?)?.continued_from
    }

    /// The most spaces that an item or block quote starting at `position` can be written with
    /// before its marker: up to three, but none after an item's marker, whose padding would take
    /// them, or after a footnote definition's label for an item, which would not count them in its
    /// width (`After`), and fewer than the width of the item before it, which a line indented that
    /// far would continue (`Position::prev`), or than the indentation from which an HTML block before
    /// it that keeps its blank lines would continue over its line.
    pub(super) fn marker_spaces<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        position: Position,
        item: bool,
    ) -> usize {
        match position.after {
            After::Marker => return 0,
            After::Label if item => return 0,
            _ => {}
        }
        let most = self
            .continued_from(document, position.before)
            .map_or(3, |from| (from - 1).min(3));
        position
            .prev
            .map_or(most, |width| width.saturating_sub(1).min(most))
    }

    /// The shape of the HTML block that `node` is or ends in, through the last blocks of the
    /// containers in it, and whether a block quote holds it, which a line without `>` ends, blank or
    /// not. This is the one walk for what the end of a block leaves open: the blank lines an HTML
    /// block of kinds 1–5 that no line closes would take (#191, #273), the line ending that
    /// `ParseOptions::html_block_blank_lines` keeps in a block of kind 6 or 7 (#265), and the empty
    /// line a block ends in, which the final line ending must not take.
    pub(super) fn trailing_html<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        mut node: NodeId,
    ) -> Option<(HtmlBlockShape, bool)> {
        let mut quoted = false;
        while let Some(last) = flow_children(document, node).last() {
            quoted |= matches!(
                document.node(node).unwrap().kind,
                EventKind::Start(Tag::BlockQuote)
            );
            node = last;
        }
        self.shape(document, node).map(|shape| (shape, quoted))
    }

    /// Whether `node` is a list, item or footnote definition whose last block, outside any block
    /// quote, is an HTML block that takes a blank line after it: one of kinds 1–5 that no line closes,
    /// as more of that block, which a line indented less than that item ends instead, as the parser
    /// reads it: `- <pre>\nx`, `- - <pre>\n  x` in an item (#191), and `1. <!--x\n2. a` between items
    /// (#273); and, under `ParseOptions::html_block_blank_lines`, one of kind 6 or 7 that keeps its
    /// blank lines, `1. <div>\n\n2. a`. So the join after it is a single line ending, and a list it
    /// is in shows that it is loose only by another join.
    fn takes_blank_line<S: AsRef<str>>(&self, document: &Document<S>, node: NodeId) -> bool {
        matches!(
            document.node(node).unwrap().kind,
            EventKind::Start(Tag::List { .. } | Tag::Item { .. } | Tag::FootnoteDefinition { .. })
        ) && self
            .trailing_html(document, node)
            .is_some_and(|(shape, quoted)| (shape.unclosed || shape.keeps_line_ending) && !quoted)
    }

    /// Whether code as a flow child of `parent` after flow child `before`, if any, needs a fence
    /// whatever the options. After an HTML block that keeps its blank lines (#265), indented code
    /// would continue it. Directly after a footnote definition, indented code would continue the
    /// definition, which takes in indented lines after a blank line.
    ///
    /// In a list or item whose blocks are not joined by blank lines (`spread` is false: it records no
    /// blank line between them and, when it is being written, `spreads_item` does not write it
    /// spread), code after a paragraph or definition, or a container ending in one, is joined to it by
    /// a single line ending, where indented code would continue it as text, lazily through the
    /// container. A blank line or `<!---->` would make the list loose, so the code is fenced, which
    /// interrupts the paragraph and ends the containers (#173). In an item written spread, the blank
    /// line ends them and the code stays indented: that item reads back recording spread, so a fence
    /// would not be written again.
    fn fences_code<S: AsRef<str>>(
        &self,
        document: &Document<S>,
        parent: NodeId,
        before: Option<NodeId>,
        spread: bool,
    ) -> bool {
        before.is_some_and(|before| match document.node(before).unwrap().kind {
            // The HTML block that `before` ends in would take indented lines after its blank lines,
            // unless a block quote, which a blank line ends, holds it.
            _ if self
                .trailing_html(document, before)
                .is_some_and(|(shape, quoted)| shape.keeps_line_ending && !quoted) =>
            {
                true
            }
            EventKind::Start(Tag::FootnoteDefinition { .. }) => true,
            EventKind::Start(Tag::Paragraph { .. }) | EventKind::Definition { .. } => {
                !spread && in_list(document, parent)
            }
            _ => !spread && in_list(document, parent) && ends_in_paragraph(document, before),
        })
    }
}
