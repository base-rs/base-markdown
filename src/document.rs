// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! A source-backed arena document and its incremental pull consumer.
use crate::stream::{Event, EventKind, Resolved, StreamError, Target, Text};
use crate::{
    bindings::Bindings,
    markdown::{angle_reads_back, heading_as_setext, needs_blank_line},
    stream::{Tag, TagKind},
    NodeSyntax,
};
use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec, vec::Vec};
use core::{num::NonZeroUsize, ops::Range};

mod insert;
pub use insert::{FootnotePolicy, InsertOptions, Inserted};

/// A stable index into a document's contiguous node arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeId(NonZeroUsize);

impl NodeId {
    pub(crate) fn new(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("an arena index cannot reach usize::MAX"))
    }
    pub(crate) fn index(self) -> usize {
        self.0.get() - 1
    }
}

/// A semantic AST node. Child and sibling lists use arena indices, not heap objects.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    /// Node meaning. Container nodes store their start tag.
    pub kind: EventKind<'static>,
    /// Source provenance, which remains unchanged when a value is edited.
    pub span: Range<usize>,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
}

impl Node {
    /// Parent node, if any.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }
    /// First child, if any.
    pub fn first_child(&self) -> Option<NodeId> {
        self.first_child
    }
    /// Next sibling, if any.
    pub fn next_sibling(&self) -> Option<NodeId> {
        self.next_sibling
    }
}

/// The result of one bounded AST-building operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProgress {
    /// The event budget was consumed; more input may remain.
    Pending,
    /// The iterator ended with a balanced document.
    Complete,
}

/// An incremental consumer that pulls events directly into an arena.
#[derive(Debug, Default)]
pub struct AstBuilder {
    nodes: Vec<Node>,
    open: Vec<NodeId>,
    // The open containers that restrict what is below them at any depth.
    reach: crate::stream::Reach,
    // Only the active container needs a tail. Opening a child resets it; closing that child makes the child the parent's new tail.
    last_child: Option<NodeId>,
    // The definitions and footnote definitions built, in the order they end, which the document
    // indexes.
    definitions: Vec<NodeId>,
    complete: bool,
    failed: bool,
}

impl AstBuilder {
    /// Create an empty builder. It does not borrow or own the parser's source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve arena and open-container capacity for allocation reuse.
    pub fn with_capacity(nodes: usize, depth: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(nodes),
            open: Vec::with_capacity(depth),
            ..Self::default()
        }
    }

    /// Pull up to `budget` events, retaining construction state between calls.
    /// A zero budget does no work. Reaching the exact budget does not probe the iterator for another event.
    ///
    /// # Errors
    /// Returns parser errors, [`StreamError::Unbalanced`] for malformed streams, and [`StreamError::InvalidNesting`] for a node that its parent cannot hold. A failed builder cannot be finished.
    pub fn consume<'a>(
        &mut self,
        events: &mut impl Iterator<Item = Result<Event<'a>, StreamError>>,
        budget: usize,
    ) -> Result<BuildProgress, StreamError> {
        if self.failed {
            return Err(StreamError::Unbalanced);
        }
        if self.complete {
            return Ok(BuildProgress::Complete);
        }
        for _ in 0..budget {
            let event = match events.next() {
                Some(Ok(event)) => event,
                Some(Err(error)) => {
                    self.failed = true;
                    return Err(error);
                }
                None => {
                    if !self.open.is_empty() || self.nodes.is_empty() {
                        self.failed = true;
                        return Err(StreamError::Unbalanced);
                    }
                    self.complete = true;
                    return Ok(BuildProgress::Complete);
                }
            };
            if let Err(error) = self.append(event) {
                self.failed = true;
                return Err(error);
            }
        }
        Ok(BuildProgress::Pending)
    }

    fn append(&mut self, event: Event<'_>) -> Result<(), StreamError> {
        event.validate(None)?;
        let event = Event {
            kind: event.kind.map_text(Text::owned),
            span: event.span,
        };
        if let EventKind::End(kind) = event.kind {
            let id = self.open.pop().ok_or(StreamError::Unbalanced)?;
            let node = &mut self.nodes[id.index()];
            if !matches!(&node.kind, EventKind::Start(tag) if tag.kind() == kind) {
                return Err(StreamError::Unbalanced);
            }
            self.reach.leave(kind);
            if event.span.end < node.span.end {
                return Err(StreamError::InvalidRange);
            }
            node.span.end = event.span.end;
            // A footnote definition is in the order definitions end, as the renderer shows the
            // first to end of an identifier's, which is one nested in another before it.
            if matches!(node.kind, EventKind::Start(Tag::FootnoteDefinition { .. })) {
                self.definitions.push(id);
            }
            self.last_child = Some(id);
            return Ok(());
        }
        if let Some(&parent) = self.open.last() {
            // A nested document start is a node of the root class, which nothing holds.
            let node = &self.nodes[parent.index()];
            let EventKind::Start(tag) = &node.kind else {
                unreachable!("only containers are open")
            };
            let first = node
                .first_child
                .map(|first| &self.nodes[first.index()].kind);
            if !event.kind.nests_in(tag, first) || !self.reach.allows(&event.kind) {
                return Err(StreamError::InvalidNesting);
            }
        } else if !self.nodes.is_empty()
            || !matches!(&event.kind, EventKind::Start(tag) if tag.kind() == TagKind::Document)
        {
            return Err(StreamError::Unbalanced);
        }
        let id = NodeId::new(self.nodes.len());
        let parent = self.open.last().copied();
        let start = matches!(event.kind, EventKind::Start(_));
        self.nodes.push(Node {
            kind: event.kind,
            span: event.span,
            parent,
            first_child: None,
            next_sibling: None,
        });
        if let Some(parent) = parent {
            if let Some(previous) = self.last_child {
                self.nodes[previous.index()].next_sibling = Some(id);
            } else {
                self.nodes[parent.index()].first_child = Some(id);
            }
        }
        self.last_child = if start { None } else { Some(id) };
        if matches!(self.nodes[id.index()].kind, EventKind::Definition { .. }) {
            self.definitions.push(id);
        }
        if let EventKind::Start(tag) = &self.nodes[id.index()].kind {
            self.reach.enter(tag.kind());
            self.open.push(id);
        }
        Ok(())
    }

    /// Attach the original source after the stream has ended. The buffer is moved, not copied.
    ///
    /// # Errors
    /// Returns [`StreamError::Unbalanced`] if consumption failed or has not reached EOF, or [`StreamError::InvalidRange`] for invalid source-backed values.
    pub fn finish<S: AsRef<str>>(self, source: S) -> Result<Document<S>, StreamError> {
        self.build(source, false)
    }

    // Finish a document from the stream of the built-in parser when `parsed` is set, which binds
    // every name it gives.
    fn build<S: AsRef<str>>(self, source: S, parsed: bool) -> Result<Document<S>, StreamError> {
        if self.failed || !self.complete {
            return Err(StreamError::Unbalanced);
        }
        let mut document = Document {
            source,
            nodes: self.nodes,
            syntax: BTreeMap::new(),
            append_hint: None,
            index: crate::bindings::Index::default(),
            settled: false,
            attached: Vec::new(),
        };
        // Public producers can supply arbitrary offsets. Validate their contract once before
        // exposing the document, node by node, as the events of the document would be.
        let source = document.source.as_ref();
        let mut resolved = Vec::new();
        // The references by name and footnote calls, which may be unbound.
        let mut names = Vec::new();
        for (index, node) in document.nodes.iter().enumerate() {
            if node.span.start > node.span.end || source.get(node.span.clone()).is_none() {
                return Err(StreamError::InvalidRange);
            }
            node.kind.validate(Some(source))?;
            if matches!(
                node.kind,
                EventKind::Start(Tag::Link(Target::Resolved(_)) | Tag::Image(Target::Resolved(_)))
            ) {
                resolved.push(index);
            }
            if matches!(
                node.kind,
                EventKind::Start(
                    Tag::Link(Target::Reference(_)) | Tag::Image(Target::Reference(_))
                ) | EventKind::FootnoteReference { .. }
            ) {
                names.push(index);
            }
        }
        document.index = crate::bindings::Index::new(source, &document.nodes, &self.definitions);
        document.attached = attached_nodes(&document.nodes, document.root());
        document.store_resolved(&resolved);
        // The parser gives a name only when a definition binds it.
        document.settled = parsed || document.bound(&names);
        Ok(document)
    }

    /// Number of nodes constructed so far; useful when selecting the next work budget.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    /// Whether no nodes have been constructed.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// An owned source buffer and a direct semantic AST.
///
/// [The contract](crate::contract) states what a document holds and what normalizing, writing and rendering it guarantee.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Document<S = String> {
    source: S,
    nodes: Vec<Node>,
    syntax: BTreeMap<NodeId, Option<NodeSyntax<'static>>>,
    // The child that `append_child` last appended. It is still its parent's last child while it has that parent and no next sibling, so a run of appends to one parent does not scan the parent's children each time.
    #[cfg_attr(feature = "serde", serde(skip))]
    append_hint: Option<NodeId>,
    // Every definition and footnote definition of the attached document, by key, kept as the
    // document is edited, so that a lookup does not walk the document.
    #[cfg_attr(feature = "serde", serde(skip))]
    index: crate::bindings::Index,
    // Whether every reference and footnote call, in every tree, is known to be bound: set for a
    // built document whose references all came resolved and bound, and by normalizing, and
    // cleared by any edit of the tree, so that normalizing an unedited document looks up none.
    #[cfg_attr(feature = "serde", serde(skip))]
    settled: bool,
    // Whether each node of the arena is in the attached document, so that finding which tree a
    // node is in takes constant time for an attached node. Kept by each edit that moves a
    // subtree into or out of the attached document, for the nodes it moves, and made again by
    // normalizing.
    #[cfg_attr(feature = "serde", serde(skip))]
    attached: Vec<bool>,
}

impl<S: AsRef<str>> Document<S> {
    /// Parse an owned or borrowed UTF-8 buffer into an arena document.
    ///
    /// # Errors
    /// Returns stream validation errors; built-in Markdown parsing has no syntax errors.
    pub fn parse(source: S, options: &crate::ParseOptions) -> Result<Self, StreamError> {
        let mut builder = AstBuilder::new();
        let mut events = crate::EventParser::new(source.as_ref(), options).names();
        if builder.consume(&mut events, usize::MAX).is_err() {
            unreachable!("the built-in Markdown producer yields a balanced semantic stream");
        }
        builder.build(source, true)
    }

    // Parse a fragment of a document whose references and footnote calls are bound by the
    // definitions `outside` it too.
    fn parse_fragment(
        source: S,
        options: &crate::ParseOptions,
        outside: &dyn crate::definitions::Outside,
    ) -> Self {
        let mut builder = AstBuilder::new();
        let text = source.as_ref();
        let definitions = crate::definitions::Definitions::outside(outside);
        let (blocks, definitions) =
            crate::block_parser::Blocks::parse_seeded(text, options, definitions);
        let raw = crate::util::line_ending::needs_normalizing(text);
        let mut events =
            crate::EventParser::from_blocks(text, options, blocks, definitions, raw).names();
        if builder.consume(&mut events, usize::MAX).is_err() {
            unreachable!("the built-in Markdown producer yields a balanced semantic stream");
        }
        builder
            .build(source, true)
            .expect("the built-in Markdown producer yields a valid document")
    }

    /// The immutable source buffer.
    pub fn source(&self) -> &str {
        self.source.as_ref()
    }
    /// All nodes in stable arena order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    /// The document root. A finished document always has exactly one root.
    pub fn root(&self) -> NodeId {
        NodeId::new(0)
    }
    /// Access a node by stable index.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// Replace a node's semantic value while preserving the arena's structure and provenance.
    ///
    /// The new value must hold the same content as the old one, so the node's children, and its
    /// place in its parent, stay valid without being checked: a container keeps its kind, except
    /// that emphasis, strong and strikethrough are interchangeable, and a leaf keeps its variant,
    /// with HTML staying a block or inline, and an inline root, whose children are phrasing,
    /// takes no frontmatter. So a heading can change its level, a link its
    /// destination, a code block its info and a list whether it is ordered, but a paragraph
    /// cannot become a heading, nor text inline code. Rendering hints that depend on the tree,
    /// such as [`Tag::Paragraph`]'s `tight` and [`Tag::Item`]'s `block_end`, are recomputed. The
    /// root keeps its syntax, such as its frontmatter fences; any other node's syntax is reset.
    /// Changing a node takes constant time, except that changing a paragraph or item refreshes
    /// the item that holds it or itself from that item's children, and changing whether a list is
    /// tight refreshes each of its items, so these take time linear in the children involved.
    ///
    /// # Errors
    /// Rejects invalid IDs and source ranges with [`StreamError::InvalidRange`], invalid values
    /// with [`StreamError::InvalidValue`], and any other change of kind with
    /// [`StreamError::InvalidNesting`]. Giving an attached definition or footnote definition a
    /// label or identifier that another definition of its kind in the document already has is
    /// rejected with [`StreamError::InvalidValue`] too, as an edit adds no copy of a binding.
    /// Nothing is changed when an error is returned.
    pub fn set_kind(&mut self, id: NodeId, kind: EventKind<'static>) -> Result<(), StreamError> {
        self.edit(|document| document.set_kind_unsettled(id, kind))
    }

    // [`Document::set_kind`], which `edit` marks unsettled.
    fn set_kind_unsettled(
        &mut self,
        id: NodeId,
        kind: EventKind<'static>,
    ) -> Result<(), StreamError> {
        let kind = stored(kind);
        kind.validate(Some(self.source()))?;
        let node = self
            .nodes
            .get(id.index())
            .ok_or(StreamError::InvalidRange)?;
        if !interchangeable(&node.kind, &kind) {
            return Err(StreamError::InvalidNesting);
        }
        // Frontmatter makes a root hold blocks, so an inline root cannot take it.
        if let (EventKind::Start(tag), Some(first)) = (&kind, node.first_child) {
            if !self.nodes[first.index()].kind.nests_in(tag, None) {
                return Err(StreamError::InvalidNesting);
            }
        }
        let block = matches!(kind, EventKind::Start(Tag::Paragraph { .. }));
        let loosened = match (&node.kind, &kind) {
            (
                EventKind::Start(Tag::List { tight: old, .. }),
                EventKind::Start(Tag::List { tight: new, .. }),
            ) => old != new,
            _ => false,
        };
        // A definition's key is indexed while it is attached, and an edit adds no copy of a
        // key. A definition stays one, as only interchangeable kinds replace each other.
        let rekeyed = defines(&kind) && self.attached(id) && {
            let source = self.source.as_ref();
            let new = crate::bindings::kind_key(&kind, source);
            if crate::bindings::kind_key(&node.kind, source) == new {
                false
            } else if self.index.admits(&[new], |_| false) {
                true
            } else {
                return Err(StreamError::InvalidValue);
            }
        };
        if rekeyed {
            self.index.remove(self.source.as_ref(), &self.nodes, id);
        }
        self.nodes[id.index()].kind = kind;
        if rekeyed {
            self.index.insert(self.source.as_ref(), &self.nodes, id);
        }
        if id != self.root() {
            self.syntax.insert(id, None);
        }
        if block {
            // A paragraph's tightness follows its list, and an item's ending its last block.
            self.refresh_paragraph(id);
            self.refresh_item(self.nodes[id.index()].parent);
        }
        match self.nodes[id.index()].kind {
            EventKind::Start(Tag::Item { .. }) => self.refresh_item(Some(id)),
            // Only a list's tightness reaches its items' paragraphs and endings.
            EventKind::Start(Tag::List { .. }) if loosened => {
                let mut items = self.nodes[id.index()].first_child;
                while let Some(item) = items {
                    let mut child = self.nodes[item.index()].first_child;
                    while let Some(paragraph) = child {
                        self.refresh_paragraph(paragraph);
                        child = self.nodes[paragraph.index()].next_sibling;
                    }
                    self.refresh_item(Some(item));
                    items = self.nodes[item.index()].next_sibling;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Edit the text of a text or soft break leaf. Source-backed values detach on first mutation.
    ///
    /// Only these kinds are edited here, as their values hold any text but NUL, which the parser
    /// stores as U+FFFD; write none, as the tree would then be invalid ([`EventKind::Text`]). The
    /// value of inline code, a code block, HTML or frontmatter must keep the form the parser
    /// stores, which an edit through a `String` could not check, so give it a new value with
    /// [`Document::set_kind`].
    ///
    /// # Errors
    /// Returns [`StreamError::InvalidRange`] for an invalid node ID or a node other than text or
    /// a soft break.
    pub fn text_mut(&mut self, id: NodeId) -> Result<&mut String, StreamError> {
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(StreamError::InvalidRange)?;
        let (EventKind::Text(text) | EventKind::SoftBreak(text)) = &mut node.kind else {
            return Err(StreamError::InvalidRange);
        };
        text.to_mut(self.source.as_ref())
    }

    /// The body of the document's frontmatter, without its fences, or `None` when it has none.
    /// The body is the lines between the fences, each with its line ending, so `---\na\n---` has
    /// the body `a\n`, `---\n\n---` the body `\n`, and `---\n---` an empty one.
    /// It is [`Tag::Document`]'s `frontmatter`; its fences and info are the root's
    /// [`NodeSyntax::Frontmatter`].
    pub fn frontmatter(&self) -> Option<&str> {
        match &self.nodes[self.root().index()].kind {
            EventKind::Start(Tag::Document {
                frontmatter: Some(value),
                ..
            }) => Some(
                value
                    .resolve(self.source())
                    .expect("document text is validated"),
            ),
            _ => None,
        }
    }

    /// Traverse without a heap-allocated traversal stack or cloned owned strings.
    pub fn events(&self) -> DocumentEvents<'_, S> {
        DocumentEvents::new(self, Some(self.root()), None)
    }

    /// The bindings of the attached document's references: which definition each label names.
    /// This takes time linear in the size of the document; see [`Bindings`], which covers the
    /// attached document only.
    pub fn bindings(&self) -> Bindings<'_> {
        Bindings::new(self.source(), &self.nodes, self.root())
    }

    /// The link reference definition that a reference labelled `label` leads to in the attached
    /// document: the first in document order whose label matches, as the parser matches labels.
    /// A document keeps an index of its definitions by label as it is edited, with the binding
    /// of each label first, filed by a hash of the label with an overflow ordered by label, so
    /// this takes constant time usually and at most time logarithmic in the number of labels,
    /// whatever they are and whatever copies of a binding there are.
    pub fn definition(&self, label: &str) -> Option<NodeId> {
        crate::bindings::Lookup::new(self.context(), self.root()).definition(
            self.context(),
            label,
            &mut crate::bindings::Scratch::default(),
        )
    }

    /// The footnote definition that a footnote call with the normalized identifier `identifier`
    /// calls in the attached document: the first with that identifier to end, which is the
    /// first in document order unless one is nested in another of its identifier, as that one
    /// is the one rendered. This takes time like [`Document::definition`].
    pub fn footnote_definition(&self, identifier: &str) -> Option<NodeId> {
        crate::bindings::Lookup::new(self.context(), self.root()).footnote_definition(
            self.context(),
            identifier,
            &mut crate::bindings::Scratch::default(),
        )
    }

    /// The destination and title that link or image `id` leads to: its own for a resource, and
    /// for a reference those of the definition it is bound to, in the tree that holds it, which
    /// is the document when it is attached and the detached subtree otherwise, as it renders.
    /// `None` for an unbound reference or a node that is not a link or image. This takes time
    /// like [`Document::definition`], whatever the depth of `id`, in the attached document, and
    /// time linear in the size of a detached tree, as its table is built for each call, so
    /// calling this on each node of a large detached tree takes quadratic time (#226).
    pub fn destination(&self, id: NodeId) -> Option<(&str, Option<&str>)> {
        let kind = &self.node(id)?.kind;
        let cx = self.context();
        crate::bindings::destination(self.source(), &self.nodes, kind, |label| {
            crate::bindings::Lookup::new(cx, self.tree_of(id)).definition(
                cx,
                label,
                &mut crate::bindings::Scratch::default(),
            )
        })
    }

    // What a lookup of this document reads.
    pub(crate) fn context(&self) -> crate::bindings::Context<'_> {
        crate::bindings::Context {
            source: self.source(),
            nodes: &self.nodes,
            index: &self.index,
            root: self.root(),
        }
    }

    // Whether `id` is in the attached document.
    fn attached(&self, id: NodeId) -> bool {
        self.attached[id.index()]
    }

    // The root of the tree that holds `id`: the document's root, at once, when it is attached,
    // and otherwise the root of its detached subtree, in time linear in its depth there.
    pub(crate) fn tree_of(&self, id: NodeId) -> NodeId {
        if self.attached(id) {
            self.root()
        } else {
            tree_root(&self.nodes, id)
        }
    }

    // Mark each node of the subtree under `id` attached, or not.
    fn mark_attached(&mut self, id: NodeId, attached: bool) {
        let nodes = &self.nodes;
        let mut current = Some(id);
        while let Some(node) = current {
            self.attached[node.index()] = attached;
            current = next_descendant(nodes, id, node);
        }
    }

    // Mark each node added to the arena since the last mark attached when its parent is, in
    // arena order, as a node is added after its parent.
    fn mark_added(&mut self) {
        for index in self.attached.len()..self.nodes.len() {
            let attached = self.nodes[index]
                .parent
                .is_some_and(|parent| self.attached[parent.index()]);
            self.attached.push(attached);
        }
    }

    // Whether each reference by name and footnote call at `names` in the arena, all attached, is bound.
    fn bound(&self, names: &[usize]) -> bool {
        let cx = self.context();
        let mut lookup = crate::bindings::Lookup::new(cx, cx.root);
        let mut scratch = crate::bindings::Scratch::default();
        names
            .iter()
            .all(|&index| !lookup.unbound(cx, &self.nodes[index].kind, &mut scratch))
    }

    // Store each resolved reference that a builder was given, at `resolved` in the arena, by its
    // name when its label binds to a definition in this document with the destination and title
    // it was resolved to, and otherwise as a resource to them, so that it keeps its meaning
    // whatever the definitions it was built with.
    fn store_resolved(&mut self, resolved: &[usize]) {
        if resolved.is_empty() {
            return;
        }
        let source = self.source.as_ref();
        let names: Vec<bool> = {
            let nodes = &self.nodes;
            let cx = self.context();
            let mut lookup = crate::bindings::Lookup::new(cx, self.root());
            let mut scratch = crate::bindings::Scratch::default();
            resolved
                .iter()
                .map(|&index| {
                    let EventKind::Start(
                        Tag::Link(Target::Resolved(target)) | Tag::Image(Target::Resolved(target)),
                    ) = &nodes[index].kind
                    else {
                        unreachable!("only resolved references are stored")
                    };
                    lookup
                        .definition(cx, text(&target.reference.label, source), &mut scratch)
                        .is_some_and(|definition| {
                            let (destination, title) =
                                crate::bindings::definition_target(nodes, definition);
                            text(destination, source) == text(&target.destination, source)
                                && title.map(|title| text(title, source))
                                    == target.title.as_ref().map(|title| text(title, source))
                        })
                })
                .collect()
        };
        for (&index, name) in resolved.iter().zip(names) {
            let EventKind::Start(Tag::Link(target) | Tag::Image(target)) =
                &mut self.nodes[index].kind
            else {
                unreachable!("only resolved references are stored")
            };
            let placeholder = Target::Resource {
                destination: Text::Literal(""),
                title: None,
            };
            *target = match core::mem::replace(target, placeholder) {
                Target::Resolved(resolved) if name => {
                    Target::Reference(alloc::boxed::Box::new(resolved.reference))
                }
                target => target.resource(),
            };
        }
    }
}

// Whether `kind` is a definition or footnote definition, which the index of a document holds.
fn defines(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
    )
}

// `kind` as a document stores it: a resolved reference as a resource to where it leads, as the
// definitions of the document it is put in may lead elsewhere, and anything else as it is.
fn stored(kind: EventKind<'static>) -> EventKind<'static> {
    match kind {
        EventKind::Start(Tag::Link(target)) => EventKind::Start(Tag::Link(target.resource())),
        EventKind::Start(Tag::Image(target)) => EventKind::Start(Tag::Image(target.resource())),
        kind => kind,
    }
}

// Whether a document can store `kind` as it is: not a resolved reference.
#[cfg(feature = "serde")]
fn storable(kind: &EventKind<'_>) -> Result<(), StreamError> {
    match kind {
        EventKind::Start(Tag::Link(Target::Resolved(_)) | Tag::Image(Target::Resolved(_))) => {
            Err(StreamError::InvalidValue)
        }
        _ => Ok(()),
    }
}

// Whether each node of `nodes` is in the tree under `root`.
fn attached_nodes(nodes: &[Node], root: NodeId) -> Vec<bool> {
    let mut attached = vec![false; nodes.len()];
    let mut current = Some(root);
    while let Some(id) = current {
        attached[id.index()] = true;
        current = next_descendant(nodes, root, id);
    }
    attached
}

// The root of the tree that holds `id`: the document's root when it is attached, or the root of
// its detached subtree.
pub(crate) fn tree_root(nodes: &[Node], mut id: NodeId) -> NodeId {
    while let Some(parent) = nodes[id.index()].parent {
        id = parent;
    }
    id
}

/// A pull traversal over the arena's parent/child/sibling links. It allocates nothing, except
/// that at the first reference or footnote call it meets it builds the bindings of the tree that
/// holds what it traverses, in one pass, to give each bound reference as
/// [`crate::stream::Target::Resolved`], with the destination and title of its definition, as the
/// parser's events do. A reference or footnote call that nothing binds is given as the text it
/// renders and reads back as, which [`Document::normalize`] makes it: an unbound reference as
/// text of its opening bracket, its children's events and text of what follows them, with its
/// label for a full reference, and an unbound footnote call as text of its call.
pub struct DocumentEvents<'a, S> {
    document: &'a Document<S>,
    current: Option<NodeId>,
    root: NodeId,
    closing: bool,
    names: Names,
}

// What a traversal knows of the references and footnote calls it meets.
#[derive(Default)]
struct Names {
    // Where the tree that holds the traversal looks bindings up, from the first reference or
    // footnote call.
    lookup: Option<crate::bindings::Lookup>,
    scratch: crate::bindings::Scratch,
    // Whether each reference open in the traversal is unbound, innermost last.
    open: Vec<bool>,
}

// Borrow values rather than cloning Cow::Owned strings. Structural metadata has no owned storage.
fn borrow_kind<'a>(kind: &'a EventKind<'_>) -> EventKind<'a> {
    match kind {
        EventKind::Start(tag) => {
            let tag = match tag {
                crate::stream::Tag::Link(target) => crate::stream::Tag::Link(target.borrowed()),
                crate::stream::Tag::Image(target) => crate::stream::Tag::Image(target.borrowed()),
                crate::stream::Tag::FootnoteDefinition { identifier, label } => {
                    crate::stream::Tag::FootnoteDefinition {
                        identifier: identifier.borrowed(),
                        label: label.as_ref().map(Text::borrowed),
                    }
                }
                crate::stream::Tag::Document {
                    line_ending,
                    frontmatter,
                } => crate::stream::Tag::Document {
                    line_ending: line_ending.clone(),
                    frontmatter: frontmatter.as_ref().map(Text::borrowed),
                },
                _ => tag.clone(),
            };
            EventKind::Start(tag)
        }
        EventKind::Text(value) => EventKind::Text(value.borrowed()),
        EventKind::SoftBreak(value) => EventKind::SoftBreak(value.borrowed()),
        EventKind::Separator { value, conditional } => EventKind::Separator {
            value: value.borrowed(),
            conditional: *conditional,
        },
        EventKind::Code { value, language } => EventKind::Code {
            value: value.borrowed(),
            language: language.as_ref().map(Text::borrowed),
        },
        EventKind::CodeBlock {
            value,
            info,
            fence,
            unclosed,
        } => EventKind::CodeBlock {
            value: value.borrowed(),
            info: info.as_ref().map(Text::borrowed),
            fence: *fence,
            unclosed: *unclosed,
        },
        EventKind::Html { value, block } => EventKind::Html {
            value: value.borrowed(),
            block: *block,
        },
        EventKind::Definition {
            label,
            destination,
            title,
        } => EventKind::Definition {
            label: label.borrowed(),
            destination: destination.borrowed(),
            title: title.as_ref().map(Text::borrowed),
        },
        EventKind::FootnoteReference { identifier, label } => EventKind::FootnoteReference {
            identifier: identifier.borrowed(),
            label: label.as_ref().map(Text::borrowed),
        },
        _ => kind.clone(),
    }
}

impl<'a, S: AsRef<str>> DocumentEvents<'a, S> {
    // The events of the subtree under `current`, if any, looking references up with `lookup`,
    // that of the tree that holds it, once made.
    fn new(
        document: &'a Document<S>,
        current: Option<NodeId>,
        lookup: Option<crate::bindings::Lookup>,
    ) -> Self {
        Self {
            document,
            current,
            root: current.unwrap_or(document.root()),
            closing: false,
            names: Names {
                lookup,
                ..Names::default()
            },
        }
    }

    // The binding of the label or identifier of the reference or footnote call `kind` in the
    // tree that holds `root`.
    fn binding(&mut self, kind: &EventKind<'_>) -> Option<NodeId> {
        let (document, root) = (self.document, self.root);
        let cx = document.context();
        let names = &mut self.names;
        let lookup = names
            .lookup
            .get_or_insert_with(|| crate::bindings::Lookup::new(cx, document.tree_of(root)));
        match kind {
            EventKind::FootnoteReference { identifier, .. } => {
                lookup.footnote_definition(cx, text(identifier, cx.source), &mut names.scratch)
            }
            kind => lookup.definition_of(cx, kind, &mut names.scratch),
        }
    }

    // The event that starts the reference or footnote call `node`: a bound reference resolved,
    // and an unbound reference or footnote call as the text it reads back as up to its
    // children, which starts where it does.
    fn named(&mut self, node: &'a Node) -> Event<'a> {
        let source = self.document.source();
        let definition = self.binding(&node.kind);
        let kind = match &node.kind {
            EventKind::FootnoteReference { .. } if definition.is_some() => borrow_kind(&node.kind),
            EventKind::FootnoteReference { identifier, label } => {
                EventKind::Text(Text::from(crate::bindings::unbound_footnote_text(text(
                    label.as_ref().unwrap_or(identifier),
                    source,
                ))))
            }
            EventKind::Start(
                tag @ (Tag::Link(Target::Reference(reference))
                | Tag::Image(Target::Reference(reference))),
            ) => {
                self.names.open.push(definition.is_none());
                let Some(definition) = definition else {
                    return Event {
                        kind: EventKind::Text(Text::Literal(unbound_open(&node.kind))),
                        span: node.span.start..node.span.start,
                    };
                };
                let (destination, title) =
                    crate::bindings::definition_target(&self.document.nodes, definition);
                let target = Target::Resolved(alloc::boxed::Box::new(Resolved {
                    reference: reference.borrowed(),
                    destination: destination.borrowed(),
                    title: title.map(Text::borrowed),
                }));
                EventKind::Start(if matches!(tag, Tag::Link(_)) {
                    Tag::Link(target)
                } else {
                    Tag::Image(target)
                })
            }
            _ => unreachable!("only a reference or footnote call is named"),
        };
        Event {
            kind,
            span: node.span.clone(),
        }
    }

    // The event that closes the reference `node`: its end, or for an unbound reference the text
    // of what follows its children.
    fn closed(&mut self, node: &'a Node) -> Event<'a> {
        let span = node.span.end..node.span.end;
        let EventKind::Start(
            tag @ (Tag::Link(Target::Reference(reference))
            | Tag::Image(Target::Reference(reference))),
        ) = &node.kind
        else {
            unreachable!("only a reference is closed by name")
        };
        if !self
            .names
            .open
            .pop()
            .expect("a reference is closed after it is started")
        {
            return Event {
                kind: EventKind::End(tag.kind()),
                span,
            };
        }
        let label = text(&reference.label, self.document.source());
        let (_, close) =
            crate::bindings::unbound_text(reference.kind, label, matches!(tag, Tag::Image(_)));
        Event {
            kind: EventKind::Text(Text::from(close)),
            span,
        }
    }

    // The event of a reference or footnote call, opened or closed, after which the traversal
    // advances. Kept out of line, so that other nodes take the path they take without names.
    #[cold]
    #[inline(never)]
    fn next_named(&mut self, id: NodeId) -> Event<'a> {
        let node = &self.document.nodes[id.index()];
        let event = if self.closing {
            self.closed(node)
        } else {
            self.named(node)
        };
        self.advance(id, node);
        event
    }

    // Move past node `id` to the next event, inline in each path of `next`, which is hot.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn advance(&mut self, id: NodeId, node: &Node) {
        if !self.closing && matches!(node.kind, EventKind::Start(_)) {
            if let Some(child) = node.first_child {
                self.current = Some(child);
            } else {
                self.closing = true;
            }
        } else if id == self.root {
            self.current = None;
        } else if let Some(sibling) = node.next_sibling {
            self.current = Some(sibling);
            self.closing = false;
        } else {
            self.current = node.parent;
            self.closing = true;
        }
    }

    // Where this traversal looked bindings up, to look up the next with.
    fn into_lookup(self) -> Option<crate::bindings::Lookup> {
        self.names.lookup
    }
}

fn text<'v>(value: &'v Text<'_>, source: &'v str) -> &'v str {
    value.resolve(source).expect("document text is validated")
}

// Whether `kind` is a reference or footnote call, which a definition may bind.
fn names(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(Tag::Link(Target::Reference(_)) | Tag::Image(Target::Reference(_)))
            | EventKind::FootnoteReference { .. }
    )
}

// The opening bracket of the reference `kind`, a link or image, as text.
fn unbound_open(kind: &EventKind<'_>) -> &'static str {
    if matches!(kind, EventKind::Start(Tag::Image(_))) {
        "!["
    } else {
        "["
    }
}

impl<'a, S: AsRef<str>> Iterator for DocumentEvents<'a, S> {
    type Item = Result<Event<'a>, StreamError>;
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.document.nodes[id.index()];
        if matches!(
            node.kind,
            EventKind::Start(Tag::Link(Target::Reference(_)) | Tag::Image(Target::Reference(_)))
        ) || (!self.closing && matches!(node.kind, EventKind::FootnoteReference { .. }))
        {
            return Some(Ok(self.next_named(id)));
        }
        let event = if self.closing {
            match &node.kind {
                EventKind::Start(tag) => Event {
                    kind: EventKind::End(tag.kind()),
                    span: node.span.end..node.span.end,
                },
                _ => unreachable!("only container nodes are closed"),
            }
        } else {
            Event {
                kind: borrow_kind(&node.kind),
                span: node.span.clone(),
            }
        };
        self.advance(id, node);
        Some(Ok(event))
    }
}

/// A stack-free iterator over a node's direct children.
#[derive(Clone)]
pub struct Children<'a> {
    nodes: &'a [Node],
    current: Option<NodeId>,
}
impl Iterator for Children<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.nodes[id.index()].next_sibling;
        Some(id)
    }
}

/// A stack-free preorder traversal, including its starting node.
#[derive(Clone)]
pub struct Descendants<'a> {
    nodes: &'a [Node],
    root: NodeId,
    current: Option<NodeId>,
}
impl Iterator for Descendants<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = next_descendant(self.nodes, self.root, id);
        Some(id)
    }
}

// Find the next preorder node using the existing links, bounded by the requested subtree.
pub(crate) fn next_descendant(nodes: &[Node], root: NodeId, id: NodeId) -> Option<NodeId> {
    if let Some(child) = nodes[id.index()].first_child {
        return Some(child);
    }
    let mut at = id;
    loop {
        if at == root {
            return None;
        }
        let node = &nodes[at.index()];
        if let Some(next) = node.next_sibling {
            return Some(next);
        }
        at = node
            .parent
            .expect("a descendant has a parent before its subtree root");
    }
}

impl<S: AsRef<str>> Document<S> {
    /// Direct child IDs in sibling order. Invalid IDs yield an empty iterator.
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            nodes: &self.nodes,
            current: self.node(id).and_then(Node::first_child),
        }
    }
    /// Preorder node IDs in a subtree, including `id`. Invalid IDs yield an empty iterator.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        Descendants {
            nodes: &self.nodes,
            root: id,
            current: self.node(id).map(|_| id),
        }
    }
    /// A subtree's events, without surrounding document events unless `id` is the root.
    pub fn subtree_events(&self, id: NodeId) -> DocumentEvents<'_, S> {
        DocumentEvents::new(self, self.node(id).map(|_| id), None)
    }
    /// Plain semantic text, including image alternative text, code and raw HTML, with each
    /// reference or footnote call that nothing binds as the text it renders as, as
    /// [`Document::subtree_events`] gives it.
    pub fn text_content(&self, id: NodeId) -> String {
        self.collect_text::<false>(id, &mut None)
    }
    // The text of the subtree under `id`, looking references up with `lookup`, that of the tree
    // that holds it, once made.
    fn collect_text<const HEADING: bool>(
        &self,
        id: NodeId,
        lookup: &mut Option<crate::bindings::Lookup>,
    ) -> String {
        let mut text = String::new();
        // In a settled document every reference is bound, so its text is that of its nodes, as
        // it would be given as an event, and no lookup is needed.
        if self.settled && self.node(id).is_some() && self.attached(id) {
            for child in self.descendants(id) {
                crate::headings::append_text::<HEADING>(
                    &self.nodes[child.index()].kind,
                    self.source(),
                    &mut text,
                );
            }
            return text;
        }
        let mut events = DocumentEvents::new(self, self.node(id).map(|_| id), lookup.take());
        for event in events.by_ref() {
            crate::headings::append_text::<HEADING>(
                &event.expect("document traversal cannot fail").kind,
                self.source(),
                &mut text,
            );
        }
        *lookup = events.into_lookup();
        text
    }
    /// Extract headings and anchors using the HTML writer's rules. Filtered headings still reserve their anchors.
    pub fn headings(&self, options: &crate::HeadingOptions) -> Vec<crate::HeadingEntry> {
        let mut anchors = crate::headings::AnchorGenerator::default();
        let mut entries = Vec::new();
        // Every heading is in the attached document, so they share its lookup.
        let mut lookup = None;
        for id in self.descendants(self.root()) {
            if let EventKind::Start(Tag::Heading(level)) = self.nodes[id.index()].kind {
                let mut text = self.collect_text::<true>(id, &mut lookup);
                text.truncate(crate::util::line_ending::trim_terminal_ending(&text).len());
                let anchor = anchors.anchor_for(&text);
                if (options.min_level..=options.max_level).contains(&level) {
                    entries.push(crate::HeadingEntry {
                        node: id,
                        level,
                        text,
                        anchor,
                    });
                }
            }
        }
        entries
    }
    /// Optional authored syntax, inferred lazily from the original source unless explicitly overridden.
    pub fn syntax(&self, id: NodeId) -> Option<NodeSyntax<'_>> {
        let node = self.node(id)?;
        if let Some(value) = self.syntax.get(&id) {
            return value.as_ref().map(NodeSyntax::borrowed);
        }
        if matches!(node.kind, EventKind::Start(Tag::Item { .. })) {
            return Some(NodeSyntax::ListItem {
                spread: self.inferred_spread(id),
            });
        }
        crate::node_syntax::infer(self.source(), node.span.clone(), &node.kind)
    }
    // Whether the source has a blank line between two blocks of the item `id`.
    fn inferred_spread(&self, id: NodeId) -> bool {
        let mut previous: Option<NodeId> = None;
        for child in self.flow_children(id) {
            if let Some(previous) = previous {
                let end = self.nodes[previous.index()].span.end;
                let start = self.nodes[child.index()].span.start;
                if self.blank_line_between(end, start) || self.ends_with_blank_line(previous) {
                    return true;
                }
            }
            previous = Some(child);
        }
        false
    }
    // The children of `id` other than separators.
    fn flow_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.children(id)
            .filter(move |id| !matches!(self.nodes[id.index()].kind, EventKind::Separator { .. }))
    }
    // Whether the source from `end` to `start` holds a whole line, which between two blocks of a container is a blank line in it. A block's span ends after its last line ending, or for a table, a definition or an HTML block, just before it.
    fn blank_line_between(&self, end: usize, start: usize) -> bool {
        let Some(gap) = self.source().get(end..start) else {
            return false;
        };
        let ended = self
            .source()
            .as_bytes()
            .get(end.wrapping_sub(1))
            .is_some_and(|byte| matches!(byte, b'\r' | b'\n'));
        crate::util::line_ending::line_endings(gap)
            .nth(usize::from(!ended))
            .is_some()
    }
    // Whether the block `id` ends with a blank line, as the parser counts one before the next block of the item that holds it, which makes the item spread and its list loose. A list, item or footnote definition takes in the blank lines after its last block, or after its first line when it has none, so they are in its span and not between it and the next block; CommonMark looks through the last child of lists and items for them in the same way. Those lines hold only whitespace and the `>` of block quotes around them, so lines of a block detached from its end do not count. A block quote's own lines of `>` alone are not blank lines in the item, though they end its span, so a block quote ends with a blank line only through its last block, such as a list whose last item takes in a line of `>` alone. Each block is on this path of at most one block that is not the last of its item, as the path goes through last children only, so all items together take time linear in the size of the tree.
    fn ends_with_blank_line(&self, mut id: NodeId) -> bool {
        loop {
            let node = &self.nodes[id.index()];
            let quote = match node.kind {
                EventKind::Start(Tag::BlockQuote) => true,
                EventKind::Start(
                    Tag::List { .. } | Tag::Item { .. } | Tag::FootnoteDefinition { .. },
                ) => false,
                _ => return false,
            };
            let last = self.flow_children(id).last();
            if !quote {
                let end = match last {
                    Some(last) => Some(self.nodes[last.index()].span.end),
                    None => self.source().get(node.span.clone()).and_then(|span| {
                        crate::util::line_ending::line_endings(span)
                            .next()
                            .map(|ending| node.span.start + ending.end)
                    }),
                };
                if end.is_some_and(|end| {
                    self.blank_line_between(end, node.span.end)
                        && self.source().as_bytes()[end..node.span.end]
                            .iter()
                            .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'>'))
                }) {
                    return true;
                }
            }
            let Some(last) = last else {
                return false;
            };
            id = last;
        }
    }
    /// Override authored syntax. `None` disconnects the node from inferred source syntax.
    ///
    /// # Errors
    /// Rejects invalid node IDs, source ranges and metadata for an incompatible node kind with
    /// [`StreamError::InvalidRange`].
    pub fn set_syntax(
        &mut self,
        id: NodeId,
        syntax: Option<NodeSyntax<'static>>,
    ) -> Result<(), StreamError> {
        let node = self.node(id).ok_or(StreamError::InvalidRange)?;
        if let Some(value) = &syntax {
            value.validate(&node.kind, self.source())?;
        }
        self.syntax.insert(id, syntax);
        Ok(())
    }
    /// Render the whole document through the shared event HTML writer.
    ///
    /// # Errors
    /// Propagates output writer errors.
    pub fn render_html(
        &self,
        options: &crate::CompileOptions,
        output: &mut impl core::fmt::Write,
    ) -> Result<(), StreamError> {
        crate::render_events(self.source(), self.events(), options, output)
    }
    /// Render one subtree through the shared event HTML writer.
    ///
    /// # Errors
    /// Rejects invalid node IDs and propagates output writer errors.
    pub fn render_node(
        &self,
        id: NodeId,
        options: &crate::CompileOptions,
        output: &mut impl core::fmt::Write,
    ) -> Result<(), StreamError> {
        if self.node(id).is_none() {
            return Err(StreamError::InvalidRange);
        }
        if id == self.root() {
            return self.render_html(options, output);
        }
        let start = Event {
            kind: EventKind::Start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            }),
            span: 0..0,
        };
        let end = Event {
            kind: EventKind::End(TagKind::Document),
            span: self.source().len()..self.source().len(),
        };
        crate::event_html_renderer::render_fragment(
            self.source(),
            core::iter::once(Ok(start))
                .chain(self.subtree_events(id))
                .chain(core::iter::once(Ok(end))),
            options,
            output,
        )
    }
    // Run the edit `edit`, after which an edit may have unbound a reference, or added one that
    // nothing binds, so the document is no longer settled, unless it is rejected, which changes
    // nothing at all.
    fn edit<T>(
        &mut self,
        edit: impl FnOnce(&mut Self) -> Result<T, StreamError>,
    ) -> Result<T, StreamError> {
        let settled = core::mem::replace(&mut self.settled, false);
        let result = edit(self);
        if result.is_err() {
            self.settled = settled;
        }
        result
    }
    fn tight_paragraph(&self, id: NodeId) -> bool {
        if !matches!(
            self.nodes[id.index()].kind,
            EventKind::Start(Tag::Paragraph { .. })
        ) {
            return false;
        }
        let Some(parent) = self.nodes[id.index()].parent else {
            return false;
        };
        if !matches!(
            self.nodes[parent.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) {
            return false;
        }
        self.nodes[parent.index()].parent.is_some_and(|list| {
            matches!(
                self.nodes[list.index()].kind,
                EventKind::Start(Tag::List { tight: true, .. })
            )
        })
    }
    fn refresh_item(&mut self, id: Option<NodeId>) {
        let Some(id) = id else {
            return;
        };
        if !matches!(
            self.nodes[id.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) {
            return;
        }
        let last = self
            .children(id)
            .filter(|child| ends_item(&self.nodes[child.index()].kind))
            .last();
        let block_end = last.is_some_and(|last| !self.tight_paragraph(last));
        set_block_end(&mut self.nodes[id.index()].kind, block_end);
    }
    fn refresh_paragraph(&mut self, id: NodeId) {
        let tight = self.tight_paragraph(id);
        if let EventKind::Start(Tag::Paragraph { tight: value }) = &mut self.nodes[id.index()].kind
        {
            *value = tight;
        }
    }
    // The last child of `parent`, if the last node appended shows it: when appending in document order, `parent` is that node or one of its ancestors, and the nodes passed on the way up hold complete subtrees, which are not passed again.
    #[allow(clippy::option_option)]
    fn appended_last_child(&self, parent: NodeId) -> Option<Option<NodeId>> {
        let hint = self.append_hint?;
        if hint == parent {
            return self.nodes[parent.index()]
                .first_child
                .is_none()
                .then_some(None);
        }
        let mut current = hint;
        loop {
            let node = &self.nodes[current.index()];
            if node.parent == Some(parent) {
                return node.next_sibling.is_none().then_some(Some(current));
            }
            current = node.parent?;
        }
    }
    // Refresh an item after `child` was appended as its last child, from that child alone.
    fn refresh_item_after_append(&mut self, parent: NodeId, child: NodeId) {
        if matches!(
            self.nodes[parent.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) && ends_item(&self.nodes[child.index()].kind)
        {
            let block_end = !self.tight_paragraph(child);
            set_block_end(&mut self.nodes[parent.index()].kind, block_end);
        }
    }
    // Edits can change the context of existing paragraphs and item endings. Refresh those rendering hints without adding caches or links to every arena node. Paragraphs and items are only in containers of blocks, so phrasing is not walked, and moving a run of phrasing takes constant time however much it holds.
    fn refresh_context(&mut self, root: NodeId) {
        let mut current = Some(root);
        while let Some(id) = current {
            current = crate::bindings::next_block(&self.nodes, root, id);
            self.refresh_paragraph(id);
            self.refresh_item(Some(id));
        }
    }
    // Check `kind` against the one content model as a child of `parent`, whose first child other
    // than the node being placed is `first`. See `TagKind::holds`; callers check the ancestors'
    // rule with `reach`.
    fn check_nesting(
        &self,
        parent: NodeId,
        first: Option<NodeId>,
        kind: &EventKind<'_>,
    ) -> Result<(), StreamError> {
        let EventKind::Start(tag) = &self.nodes[parent.index()].kind else {
            unreachable!("callers check that the parent is a container");
        };
        if !kind.nests_in(tag, first.map(|first| &self.nodes[first.index()].kind)) {
            return Err(StreamError::InvalidNesting);
        }
        Ok(())
    }
    // The containers that restrict what can be below them at any depth among `id`, a container,
    // and its ancestors. Links hold phrasing and table cells hold phrasing and are held by rows,
    // so the search stops at the first that is not phrasing.
    fn reach(&self, id: NodeId) -> crate::stream::Reach {
        let mut reach = crate::stream::Reach::default();
        for id in core::iter::successors(Some(id), |id| self.nodes[id.index()].parent) {
            let kind = &self.nodes[id.index()].kind;
            let EventKind::Start(tag) = kind else {
                unreachable!("a parent is a container");
            };
            reach.enter(tag.kind());
            if kind.class() != crate::stream::Class::Phrasing {
                break;
            }
        }
        reach
    }
    /// Append a new semantic node. Its source provenance is empty; values may still reference the source buffer.
    ///
    /// Appending in document order, each node after the subtree before it is complete, takes amortized constant time per node, except that a footnote reference or a break also walks up through the phrasing that holds it, to check that no link encloses the reference and no table cell the break, in time linear in the depth of the phrasing nest, so appending many of them deep in one nest takes quadratic time (#233). Appending to another parent scans its children.
    ///
    /// # Errors
    /// Rejects a parent that is not in the arena, and invalid source ranges, with
    /// [`StreamError::InvalidRange`], a leaf parent and end events with
    /// [`StreamError::Unbalanced`], invalid values with
    /// [`StreamError::InvalidValue`], and a node that the parent cannot hold, such as a document
    /// root, a block in a paragraph, an item outside a list, phrasing in a root that holds blocks,
    /// a footnote reference inside a link, or a soft or hard break inside a table cell, with
    /// [`StreamError::InvalidNesting`]. Appending a
    /// definition or footnote definition to the attached document whose label or identifier
    /// another definition of its kind there already has is rejected with
    /// [`StreamError::InvalidValue`], as an edit adds no copy of a binding. Nothing is changed
    /// when an error is returned.
    pub fn append_child(
        &mut self,
        parent: NodeId,
        kind: EventKind<'static>,
    ) -> Result<NodeId, StreamError> {
        self.edit(|document| document.append_child_unsettled(parent, kind))
    }

    // [`Document::append_child`], which `edit` marks unsettled.
    fn append_child_unsettled(
        &mut self,
        parent: NodeId,
        kind: EventKind<'static>,
    ) -> Result<NodeId, StreamError> {
        let Some(node) = self.node(parent) else {
            return Err(StreamError::InvalidRange);
        };
        let leaf = !matches!(node.kind, EventKind::Start(_));
        let kind = stored(kind);
        kind.validate(Some(self.source()))?;
        if leaf || matches!(kind, EventKind::End(_)) {
            return Err(StreamError::Unbalanced);
        }
        self.check_nesting(parent, self.nodes[parent.index()].first_child, &kind)?;
        if crate::stream::Reach::restricted(&kind) && !self.reach(parent).allows(&kind) {
            return Err(StreamError::InvalidNesting);
        }
        // An edit adds no copy of a binding.
        let indexed = defines(&kind) && self.attached(parent);
        if indexed
            && !self.index.admits(
                &[crate::bindings::kind_key(&kind, self.source.as_ref())],
                |_| false,
            )
        {
            return Err(StreamError::InvalidValue);
        }
        let previous = match self.appended_last_child(parent) {
            Some(previous) => previous,
            None => self.children(parent).last(),
        };
        let id = self.insert_child(parent, kind, previous, None);
        if indexed {
            self.index.insert(self.source.as_ref(), &self.nodes, id);
        }
        self.append_hint = Some(id);
        self.refresh_context(id);
        self.refresh_item_after_append(parent, id);
        Ok(id)
    }
    // Callers validate the semantic content and insertion position before changing links, then refresh context once the operation is complete.
    fn link_child(
        &mut self,
        parent: NodeId,
        id: NodeId,
        previous: Option<NodeId>,
        next: Option<NodeId>,
    ) {
        self.nodes[id.index()].parent = Some(parent);
        self.nodes[id.index()].next_sibling = next;
        if let Some(previous) = previous {
            self.nodes[previous.index()].next_sibling = Some(id);
        } else {
            self.nodes[parent.index()].first_child = Some(id);
        }
    }
    fn insert_child(
        &mut self,
        parent: NodeId,
        kind: EventKind<'static>,
        previous: Option<NodeId>,
        next: Option<NodeId>,
    ) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self.nodes.push(Node {
            kind,
            span: 0..0,
            parent: None,
            first_child: None,
            next_sibling: None,
        });
        self.link_child(parent, id, previous, next);
        self.mark_added();
        id
    }
    /// Detach a subtree without moving its arena slots or invalidating IDs. This takes time
    /// linear in the subtree when it was attached, to mark its nodes detached and remove its
    /// definitions from the document's index of them, besides its parent's children before it.
    ///
    /// # Errors
    /// Rejects invalid IDs and detaching the document root.
    pub fn detach(&mut self, id: NodeId) -> Result<(), StreamError> {
        self.edit(|document| document.detach_unsettled(id))
    }

    // [`Document::detach`], which `edit` marks unsettled.
    fn detach_unsettled(&mut self, id: NodeId) -> Result<(), StreamError> {
        if id == self.root() {
            return Err(StreamError::Unbalanced);
        }
        let node = self.node(id).ok_or(StreamError::InvalidRange)?;
        let Some(parent) = node.parent else {
            return Ok(());
        };
        if self.attached(parent) {
            self.index
                .remove_tree(self.source.as_ref(), &self.nodes, id);
        }
        self.unlink_child(id, parent);
        if self.attached(id) {
            self.mark_attached(id, false);
        }
        self.refresh_context(id);
        self.refresh_item(Some(parent));
        Ok(())
    }
    fn unlink_child(&mut self, id: NodeId, parent: NodeId) {
        let next = self.nodes[id.index()].next_sibling;
        if self.nodes[parent.index()].first_child == Some(id) {
            self.nodes[parent.index()].first_child = next;
        } else {
            let previous = self
                .children(parent)
                .find(|child| self.nodes[child.index()].next_sibling == Some(id))
                .expect("an attached node occurs in its parent's child chain");
            self.nodes[previous.index()].next_sibling = next;
        }
        self.nodes[id.index()].parent = None;
        self.nodes[id.index()].next_sibling = None;
    }
    /// Move an existing subtree into a container, optionally before one of its children. No node or text allocation is required. Moving a subtree into or out of the attached document takes time linear in the subtree too, to mark its nodes and to add its definitions to the document's index of them or remove them. Every move also walks up through the phrasing that holds the new parent, to find whether a link or table cell is above it, in time linear in the depth of the phrasing nest, so many moves deep in one nest take quadratic time (#233), and when one is, scans the subtree.
    ///
    /// # Errors
    /// Rejects IDs that are not in the arena and positions that are not children of `parent`
    /// with [`StreamError::InvalidRange`], cycles, a leaf parent and root moves with
    /// [`StreamError::Unbalanced`], and a subtree that the parent cannot
    /// hold, as for [`Document::append_child`], or that holds a footnote reference when the parent
    /// is in a link, or a soft or hard break when it is in a table cell, with
    /// [`StreamError::InvalidNesting`], before modifying the tree. As an edit
    /// adds no copy of a binding, it rejects with [`StreamError::InvalidValue`], before modifying
    /// the tree, attaching a detached subtree to the attached document when the subtree holds two
    /// definitions, or footnote definitions, of one label or identifier, or one whose label or
    /// identifier a definition of its kind in the document already has, and moving a subtree
    /// within the document when it holds a definition or footnote definition of a label or
    /// identifier that the document defines more than once, which [`Document::normalize`] makes
    /// once: the document's index keeps a label's definitions in document order, the first
    /// binding, so such a move could reorder them and change which one binds. Detaching a
    /// definition first unbinds it, so the only definition of its label can be detached and
    /// attached again, but one that has a copy elsewhere cannot. Moving a subtree out of its
    /// parent always leaves that parent valid, if not normalized.
    pub fn attach(
        &mut self,
        parent: NodeId,
        id: NodeId,
        before: Option<NodeId>,
    ) -> Result<(), StreamError> {
        self.edit(|document| document.attach_unsettled(parent, id, before))
    }

    // [`Document::attach`], which `edit` marks unsettled.
    fn attach_unsettled(
        &mut self,
        parent: NodeId,
        id: NodeId,
        before: Option<NodeId>,
    ) -> Result<(), StreamError> {
        let parent_node = self.node(parent).ok_or(StreamError::InvalidRange)?;
        if id == self.root() || !matches!(parent_node.kind, EventKind::Start(_)) {
            return Err(StreamError::Unbalanced);
        }
        self.node(id).ok_or(StreamError::InvalidRange)?;
        if before.is_some_and(|before| {
            self.node(before)
                .map_or(true, |node| node.parent != Some(parent))
        }) {
            return Err(StreamError::InvalidRange);
        }
        let mut ancestor = Some(parent);
        while let Some(current) = ancestor {
            if current == id {
                return Err(StreamError::Unbalanced);
            }
            ancestor = self.nodes[current.index()].parent;
        }
        let first = match self.nodes[parent.index()].first_child {
            Some(first) if first == id => self.nodes[id.index()].next_sibling,
            first => first,
        };
        self.check_nesting(parent, first, &self.nodes[id.index()].kind)?;
        let reach = self.reach(parent);
        if !reach.is_empty()
            && self
                .descendants(id)
                .any(|child| !reach.allows(&self.nodes[child.index()].kind))
        {
            return Err(StreamError::InvalidNesting);
        }
        if before == Some(id) {
            return Ok(());
        }
        let former_parent = self.nodes[id.index()].parent;
        // Its definitions are indexed while it is attached, wherever in the document it is, and
        // an edit adds no copy of a binding, nor moves one, which would reorder it.
        let (was_attached, attached) = (self.attached(id), self.attached(parent));
        if attached {
            let source = self.source.as_ref();
            let admitted = if was_attached {
                self.index.movable(source, &self.nodes, id)
            } else {
                crate::bindings::tree_keys(source, &self.nodes, id)
                    .is_some_and(|keys| self.index.admits(&keys, |_| false))
            };
            if !admitted {
                return Err(StreamError::InvalidValue);
            }
        }
        if was_attached && !attached {
            self.index
                .remove_tree(self.source.as_ref(), &self.nodes, id);
        }
        if let Some(parent) = former_parent {
            self.unlink_child(id, parent);
        }
        let previous = self
            .children(parent)
            .take_while(|child| Some(*child) != before)
            .last();
        self.link_child(parent, id, previous, before);
        if attached != was_attached {
            self.mark_attached(id, attached);
        }
        if attached && !was_attached {
            self.index
                .insert_tree(self.source.as_ref(), &self.nodes, id);
        }
        self.refresh_context(id);
        if former_parent != Some(parent) {
            self.refresh_item(former_parent);
        }
        self.refresh_item(Some(parent));
        Ok(())
    }
    /// Detach all direct children, returning their stable IDs in order, in time linear in their
    /// subtrees when `parent` is attached, to mark their nodes detached and remove their
    /// definitions from the document's index of them.
    ///
    /// # Errors
    /// Rejects invalid node IDs.
    pub fn extract_children(&mut self, parent: NodeId) -> Result<Vec<NodeId>, StreamError> {
        self.edit(|document| document.extract_children_unsettled(parent))
    }

    // [`Document::extract_children`], which `edit` marks unsettled.
    fn extract_children_unsettled(&mut self, parent: NodeId) -> Result<Vec<NodeId>, StreamError> {
        self.node(parent).ok_or(StreamError::InvalidRange)?;
        let children: Vec<_> = self.children(parent).collect();
        if self.attached(parent) {
            for &id in &children {
                self.index
                    .remove_tree(self.source.as_ref(), &self.nodes, id);
                self.mark_attached(id, false);
            }
        }
        self.nodes[parent.index()].first_child = None;
        for &id in &children {
            self.nodes[id.index()].parent = None;
            self.nodes[id.index()].next_sibling = None;
            self.refresh_context(id);
        }
        self.refresh_item(Some(parent));
        Ok(children)
    }
    /// Replace a heading section with another document's root children, materializing their source-backed values into this document.
    ///
    /// `heading_index` counts direct children, including separator nodes. With `remove_subsections`, the section extends through headings deeper than the selected heading. Detached original nodes retain their IDs and can still be inspected or reattached.
    ///
    /// The replacement's root children are copied as [`Document::insert_from`] copies them with
    /// the default [`InsertOptions`]: they render as they render in the replacement, so its link
    /// reference definitions are not copied, as a reference bound there becomes a resource to
    /// where it leads, and its footnote definitions are copied only when the copies call them,
    /// after them, with a fresh identifier when one outside the replaced section binds theirs.
    /// The section's own definitions no longer bind once it is replaced. Frontmatter is not
    /// copied.
    ///
    /// # Errors
    /// Rejects invalid parent IDs with [`StreamError::InvalidRange`], and an inline replacement
    /// document, whose root holds phrasing, with [`StreamError::InvalidNesting`], and a fresh
    /// footnote identifier too long to be one with [`StreamError::InvalidValue`]. Returns `false`
    /// when the selected child is not a heading. Nothing is changed when an error is returned.
    pub fn replace_heading_section<T: AsRef<str>>(
        &mut self,
        parent: NodeId,
        heading_index: usize,
        replacement: &Document<T>,
        replace_heading: bool,
        remove_subsections: bool,
    ) -> Result<bool, StreamError> {
        let settled = self.settled;
        self.edit(|document| {
            document.replace_heading_section_unsettled(
                parent,
                heading_index,
                replacement,
                (replace_heading, remove_subsections),
                settled,
            )
        })
    }

    // [`Document::replace_heading_section`], which `edit` marks unsettled. `settled` is whether
    // every reference was bound before it.
    fn replace_heading_section_unsettled<T: AsRef<str>>(
        &mut self,
        parent: NodeId,
        heading_index: usize,
        replacement: &Document<T>,
        (replace_heading, remove_subsections): (bool, bool),
        settled: bool,
    ) -> Result<bool, StreamError> {
        self.node(parent).ok_or(StreamError::InvalidRange)?;
        let mut children = self.children(parent);
        let mut previous = children.by_ref().take(heading_index).last();
        let Some(heading) = children.next() else {
            return Ok(false);
        };
        let EventKind::Start(Tag::Heading(level)) = self.nodes[heading.index()].kind else {
            return Ok(false);
        };
        let first = if replace_heading {
            Some(heading)
        } else {
            self.nodes[heading.index()].next_sibling
        };
        let mut end = self.nodes[heading.index()].next_sibling;
        while let Some(id) = end {
            if matches!(self.nodes[id.index()].kind,EventKind::Start(Tag::Heading(next)) if !remove_subsections || next<=level)
            {
                break;
            }
            end = self.nodes[id.index()].next_sibling;
        }
        // The section's definitions no longer bind once it is replaced, so a copied footnote
        // definition may take an identifier that only the section had.
        let attached = self.attached(parent);
        let mut leaving = Vec::new();
        let mut replacing = Vec::new();
        let mut at = first;
        while at != end {
            let id = at.expect("the section ends in the selected sibling chain");
            leaving.extend(crate::bindings::definitions(&self.nodes, id));
            replacing.push(id);
            at = self.nodes[id.index()].next_sibling;
        }
        leaving.sort_unstable();
        replacing.sort_unstable();
        if !replace_heading {
            previous = Some(heading);
        }
        let plan = self.plan(
            parent,
            (previous, end),
            replacement,
            replacement.children(replacement.root()),
            &InsertOptions::default(),
            (&replacing, &leaving),
            settled,
        )?;
        // A section is one contiguous sibling range: splice it out once, retaining stable IDs and independent detached subtrees.
        if first != end {
            if let Some(previous) = previous {
                self.nodes[previous.index()].next_sibling = end;
            } else {
                self.nodes[parent.index()].first_child = end;
            }
            let mut at = first;
            while at != end {
                let id = at.expect("the section ends in the selected sibling chain");
                at = self.nodes[id.index()].next_sibling;
                if attached {
                    self.index
                        .remove_tree(self.source.as_ref(), &self.nodes, id);
                    self.mark_attached(id, false);
                }
                self.nodes[id.index()].parent = None;
                self.nodes[id.index()].next_sibling = None;
                self.refresh_context(id);
            }
        }
        self.apply(&plan, parent, previous, end, replacement);
        // The section may have held the child that ended the item.
        self.refresh_item(Some(parent));
        Ok(true)
    }
    /// Parse Markdown into a node's children, replacing all of them.
    ///
    /// The parent decides how `markdown` is read, as it would be read written there. A paragraph,
    /// heading, table cell, emphasis, strong, strikethrough, link or image holds phrasing, so
    /// `markdown` is read as the content of one paragraph: it must parse into one paragraph, or
    /// into nothing, such as an empty or blank fragment, which leaves the parent empty. In a table
    /// cell, or phrasing inside one, it is read as the content of a cell, which is one line and
    /// ends at a `|` that is not escaped, and in a link, or phrasing inside one, as the text of a
    /// link, where literal autolinks do not form and whitespace at the edges stays, so `a\|b` in
    /// a cell is `a|b`, and `www.a.com` and `# h` in a link are text, as they read back. It must
    /// then parse into the content of one cell or link: a fragment such as `a | b` or `a\nb` in a
    /// cell, or one that holds a link, such as `[b](/u)` or a reference, or whose brackets would
    /// take the link's own, such as `[a` or `a\`, in a link, which the parser would read as ending
    /// the cell or the link, or as a link in place of the one around it, is rejected. The document
    /// root, a block quote, an item or a footnote definition holds blocks, so `markdown` is read as
    /// a document's blocks. The parser writes some separators only at the root, so in a container
    /// other than the root, the separators after the fragment's last block are left out, and so
    /// are those after a paragraph that an item of a tight list makes tight, except before
    /// another paragraph. An item is then spread as the fragment's blocks are, with a blank line
    /// between two of them, unless it is in a tight list.
    ///
    /// A reference or footnote call in `markdown` is a name, as in a parsed document, which the
    /// parser reads as one where a definition binds its label: one of the fragment's own, or one
    /// of this document's outside the replaced children, in the tree that holds `parent`, the
    /// attached document or a detached subtree, as it will when the document is written and read
    /// back. One that nothing binds stays text. Inside a link, which cannot hold a footnote
    /// reference, a footnote call stays text. A block fragment's own definitions become nodes of
    /// this document, binding their labels for the rest of it, and the definitions among the
    /// replaced children no longer bind theirs.
    ///
    /// `markdown` is read as though it were not at the start of a document, so frontmatter is
    /// never read: `---` fences read as thematic breaks or setext underlines, as they would
    /// anywhere else, and the document's own frontmatter is kept. Parsed values are owned, so
    /// `markdown` can be dropped afterwards. The replaced children are detached, keeping their
    /// IDs, as [`Document::extract_children`] leaves them.
    ///
    /// `markdown` is parsed once, in time linear in its size and in the size of the replaced
    /// children, plus, for each label of a reference or footnote call in it that the fragment
    /// does not define, one lookup in the document's index, which passes over the replaced
    /// children's definitions of that label. In a detached subtree there is no index, so the
    /// first such label costs one pass over the whole detached tree, and editing each node of a
    /// large detached tree this way takes quadratic time, the known gap of #226. Into a phrasing
    /// parent, finding whether a link or table cell is above, which decides how the fragment is
    /// read, walks the parent's phrasing ancestors, in time linear in the depth of the phrasing
    /// nest, so many edits deep in one nest take quadratic time, as for
    /// [`Document::append_child`] and [`Document::attach`] (#233).
    ///
    /// ```
    /// use base_markdown::{stream::{EventKind, Tag, Target}, to_markdown, Document, ParseOptions};
    ///
    /// let options = ParseOptions::gfm();
    /// let mut document = Document::parse("Old text.\n".to_owned(), &options).unwrap();
    /// let paragraph = document.children(document.root()).next().unwrap();
    /// document
    ///     .parse_children(paragraph, "Visit www.google.com for more details.", &options)
    ///     .unwrap();
    ///
    /// // The literal autolink is parsed into a link.
    /// let link = document.children(paragraph).nth(1).unwrap();
    /// assert!(matches!(
    ///     &document.node(link).unwrap().kind,
    ///     EventKind::Start(Tag::Link(Target::Resource { destination, .. }))
    ///         if destination.resolve(document.source()).ok() == Some("https://www.google.com")
    /// ));
    /// assert_eq!(
    ///     to_markdown(&document).unwrap(),
    ///     "Visit www.google.com for more details.\n"
    /// );
    /// ```
    ///
    /// # Errors
    /// Rejects an invalid parent ID with [`StreamError::InvalidRange`] and a leaf parent with
    /// [`StreamError::Unbalanced`]. Rejects with [`StreamError::InvalidNesting`] a list, table,
    /// table head, table body or table row parent, which holds no Markdown that parses on its own,
    /// and, for a parent that holds phrasing, a fragment that parses into anything other than one
    /// paragraph, such as `# h`, `a\n\nb`, `- x`, a setext heading or a definition, as its
    /// blocks cannot be in the parent, or, in a table cell or link, into anything other than the
    /// content of one cell or link, as above. As an edit adds no copy of a binding, it rejects with
    /// [`StreamError::InvalidValue`], into the attached document, a fragment that defines a label,
    /// or footnote identifier, twice, or one that a definition of its kind in the document outside
    /// the replaced children already has, such as `[a]: /x` into a block quote when `[a]: /y` is
    /// after it. Nothing is changed when an error is returned.
    pub fn parse_children(
        &mut self,
        parent: NodeId,
        markdown: &str,
        options: &crate::ParseOptions,
    ) -> Result<(), StreamError> {
        self.edit(|document| document.parse_children_unsettled(parent, markdown, options))
    }

    // [`Document::parse_children`], which `edit` marks unsettled.
    fn parse_children_unsettled(
        &mut self,
        parent: NodeId,
        markdown: &str,
        options: &crate::ParseOptions,
    ) -> Result<(), StreamError> {
        let EventKind::Start(tag) = &self.node(parent).ok_or(StreamError::InvalidRange)?.kind
        else {
            return Err(StreamError::Unbalanced);
        };
        let phrasing = match tag.kind().content() {
            crate::stream::Content::Phrasing => true,
            crate::stream::Content::Root | crate::stream::Content::Blocks => false,
            _ => return Err(StreamError::InvalidNesting),
        };
        let reach = self.reach(parent);
        // A blank fragment holds nothing, which needs no cell or link around it.
        let blank = markdown
            .bytes()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'));
        let (cell, link) = (
            phrasing && !blank && reach.cell(),
            phrasing && !blank && reach.link(),
        );
        let options = crate::ParseOptions {
            constructs: crate::Constructs {
                frontmatter: false,
                table: options.constructs.table || cell,
                ..options.constructs.clone()
            },
            inline_code_info: options.inline_code_info,
            html_block_blank_lines: options.html_block_blank_lines,
            html_tag_namespaces: options.html_tag_namespaces,
            strikethrough_single_tilde: options.strikethrough_single_tilde,
        };
        // Phrasing in a cell or link is read in one, which a lone head cell of a table and a
        // link without a destination are, with spaces in the cell, which it trims, so that a
        // backslash at the end of the fragment does not escape the `|` after it.
        let mut wrapped = String::new();
        let text = if cell || link {
            for (around, open) in [(cell, "| "), (link, "[")] {
                if around {
                    wrapped.push_str(open);
                }
            }
            wrapped.push_str(markdown);
            for (around, close) in [(link, "]()"), (cell, " |\n|-|\n")] {
                if around {
                    wrapped.push_str(close);
                }
            }
            wrapped.as_str()
        } else {
            markdown
        };
        // The definitions of the replaced children, which a parent that holds phrasing has none
        // of, no longer bind.
        let mut leaving = Vec::new();
        if !phrasing {
            for child in self.children(parent) {
                leaving.extend(crate::bindings::definitions(&self.nodes, child));
            }
            leaving.sort_unstable();
        }
        let attached = self.attached(parent);
        let fragment = {
            let outside = Around {
                cx: self.context(),
                parent,
                attached,
                leaving: &leaving,
                footnotes: !reach.link(),
                table: core::cell::OnceCell::new(),
            };
            Document::parse_fragment(text, &options, &outside)
        };
        // A block quote, an item and a footnote definition hold the blocks and separators that a
        // document's root holds, so a flow parent holds every child of the fragment's root.
        let from = if phrasing {
            fragment
                .phrasing_content(cell, link)
                .ok_or(StreamError::InvalidNesting)?
        } else {
            Some(fragment.root())
        };
        // An edit adds no copy of a binding: the fragment's definitions may have only keys that no
        // definition outside the replaced children has.
        if attached {
            let keys =
                crate::bindings::tree_keys(fragment.source(), &fragment.nodes, fragment.root())
                    .ok_or(StreamError::InvalidValue)?;
            if !self
                .index
                .admits(&keys, |id| leaving.binary_search(&id).is_ok())
            {
                return Err(StreamError::InvalidValue);
            }
        }
        self.extract_children_unsettled(parent)
            .expect("the parent is a node of this document");
        let nested = parent != self.root();
        let tight = nested && self.tight_item(parent);
        let last = from.and_then(|from| {
            fragment
                .children(from)
                .filter(|&id| !fragment.separator(id))
                .last()
        });
        let mut previous = None;
        let mut copying = last.is_some();
        let mut after_paragraph = false;
        for child in from.into_iter().flat_map(|from| fragment.children(from)) {
            let separator = fragment.separator(child);
            // Two paragraphs are apart only with a separator between them, as in a loose list.
            let skip = separator
                && nested
                && (!copying || tight && after_paragraph && !fragment.before_paragraph(child));
            copying &= Some(child) != last;
            after_paragraph = matches!(
                fragment.nodes[child.index()].kind,
                EventKind::Start(Tag::Paragraph { .. })
            );
            if skip {
                continue;
            }
            // The fragment's references keep their names, which bind here as they were read.
            let copy = self.copy_subtree(&fragment, child, parent, previous, None);
            if attached {
                self.index
                    .insert_tree(self.source.as_ref(), &self.nodes, copy);
            }
            previous = Some(copy);
        }
        self.refresh_item(Some(parent));
        // An item's spread is read from the source between its blocks, which the copies are not
        // in, so it is the fragment's, unless the item is in a tight list, whose items' blocks are
        // written without a blank line between them.
        if matches!(
            self.nodes[parent.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) {
            let spread = !tight && fragment.inferred_spread(fragment.root());
            self.syntax
                .insert(parent, Some(NodeSyntax::ListItem { spread }));
        }
        Ok(())
    }
    // Whether a node is a separator.
    fn separator(&self, id: NodeId) -> bool {
        matches!(self.nodes[id.index()].kind, EventKind::Separator { .. })
    }
    // Whether the next sibling of `id` other than separators is a paragraph.
    fn before_paragraph(&self, id: NodeId) -> bool {
        core::iter::successors(self.nodes[id.index()].next_sibling, |next| {
            self.nodes[next.index()].next_sibling
        })
        .find(|&next| !self.separator(next))
        .is_some_and(|next| {
            matches!(
                self.nodes[next.index()].kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
    }
    // Whether a node is an item of a tight list.
    fn tight_item(&self, id: NodeId) -> bool {
        matches!(
            self.nodes[id.index()].kind,
            EventKind::Start(Tag::Item { .. })
        ) && self.nodes[id.index()].parent.is_some_and(|list| {
            matches!(
                self.nodes[list.index()].kind,
                EventKind::Start(Tag::List { tight: true, .. })
            )
        })
    }
    // The node of a fragment parsed for a parent that holds phrasing whose children become the
    // parent's: its one paragraph, or none for a fragment of no blocks, or, for a fragment read in
    // a `cell` or `link`, the cell or link it was read in, if it parsed into the one cell or link.
    #[allow(clippy::option_option)]
    fn phrasing_content(&self, cell: bool, link: bool) -> Option<Option<NodeId>> {
        // The one child of `id` other than separators, if it is a container of `kind`.
        let sole = |id: NodeId, kind: TagKind| {
            let mut children = self.children(id).filter(|&child| !self.separator(child));
            match (children.next(), children.next()) {
                (Some(child), None)
                    if matches!(
                        &self.nodes[child.index()].kind,
                        EventKind::Start(tag) if tag.kind() == kind
                    ) =>
                {
                    Some(child)
                }
                _ => None,
            }
        };
        let root = self.root();
        let mut content = if cell {
            let table = sole(root, TagKind::Table)?;
            let head = sole(table, TagKind::TableHead)?;
            // The delimiter row has one column, so the head's one row has one cell.
            let row = self.flow_children(head).next();
            row.and_then(|row| self.flow_children(row).next())
                .expect("a table's head has a row of a cell")
        } else if self.children(root).all(|child| self.separator(child)) {
            return Some(None);
        } else {
            sole(root, TagKind::Paragraph)?
        };
        if link {
            content = sole(content, TagKind::Link)?;
        }
        Some(Some(content))
    }
    // Copy the subtree at `root` of `source` into `parent`, after `previous` and before `before`,
    // owning its values and syntax. Its references keep their names, which the caller has read
    // as this document binds them. The caller refreshes the item `parent` may be once its
    // children are complete.
    fn copy_subtree<T: AsRef<str>>(
        &mut self,
        source: &Document<T>,
        root: NodeId,
        mut parent: NodeId,
        mut previous: Option<NodeId>,
        before: Option<NodeId>,
    ) -> NodeId {
        let copied_root = NodeId::new(self.nodes.len());
        let mut original_parent = source.nodes[root.index()].parent;
        for original in source.descendants(root) {
            let node = &source.nodes[original.index()];
            // Source and destination parent links describe the same copied path; ascending them together needs no mapping stack.
            while original_parent != node.parent {
                previous = Some(parent);
                parent = self.nodes[parent.index()]
                    .parent
                    .expect("a copied descendant has a copied parent");
                original_parent = source.nodes[original_parent
                    .expect("subtree traversal stays below its original parent")
                    .index()]
                .parent;
            }
            let kind =
                borrow_kind(&node.kind).map_text(|value| value.materialized(source.source()));
            let next = if original == root { before } else { None };
            let id = self.insert_child(parent, kind, previous, next);
            // An item's spread and a link's form describe only the copied node, so they survive.
            // Frontmatter is on the root, which is never copied.
            let syntax = match source.syntax(original) {
                Some(NodeSyntax::ListItem { spread }) => Some(NodeSyntax::ListItem { spread }),
                Some(NodeSyntax::Link { form }) => Some(NodeSyntax::Link { form }),
                _ => None,
            };
            if let Some(syntax) = syntax {
                self.set_syntax(id, Some(syntax))
                    .expect("syntax copied from the same kind of node");
            }
            if node.first_child.is_some() {
                original_parent = Some(original);
                parent = id;
                previous = None;
            } else {
                previous = Some(id);
            }
        }
        self.mark_added();
        self.refresh_context(copied_root);
        copied_root
    }
}

// The definitions of a document outside the children of a parent, which a fragment parsed into
// it replaces, for the fragment's references and footnote calls: those of `tree`, the tree that
// holds the parent, but for `leaving`, the definitions of the replaced children, sorted, and but
// for footnote definitions when not `footnotes`, as the parent is in a link, which holds no
// footnote reference.
#[derive(Debug)]
struct Around<'d> {
    cx: crate::bindings::Context<'d>,
    parent: NodeId,
    attached: bool,
    leaving: &'d [NodeId],
    footnotes: bool,
    // The definitions of a detached tree, built at the first lookup.
    table: core::cell::OnceCell<crate::bindings::Table>,
}

impl crate::definitions::Outside for Around<'_> {
    fn binds(&self, footnote: bool, key: &str) -> bool {
        if footnote && !self.footnotes {
            return false;
        }
        let leaving = |id: NodeId| self.leaving.binary_search(&id).is_ok();
        if self.attached {
            self.cx.index.binds(footnote, key, leaving)
        } else {
            // A detached tree has no index, so its definitions are found with one pass over it,
            // as for the lookups of #226.
            self.table
                .get_or_init(|| {
                    crate::bindings::Table::without(
                        self.cx.source,
                        self.cx.nodes,
                        tree_root(self.cx.nodes, self.parent),
                        leaving,
                    )
                })
                .binds(footnote, key)
        }
    }
}

// Whether a child of an item decides how the item ends: other children write nothing after the item's last line.
fn ends_item(kind: &EventKind<'_>) -> bool {
    !matches!(
        kind,
        EventKind::Separator { .. }
            | EventKind::Definition { .. }
            | EventKind::Start(Tag::FootnoteDefinition { .. })
    )
}

// Whether `set_kind` can replace `old` with `new` without checking the node's children, its
// siblings or its ancestors: a container keeps its kind, except that emphasis, strong and
// strikethrough are interchangeable, and a leaf keeps its variant, with HTML staying a block or
// inline.
fn interchangeable(old: &EventKind<'_>, new: &EventKind<'_>) -> bool {
    let attention =
        |kind: TagKind| matches!(kind, TagKind::Emphasis | TagKind::Strong | TagKind::Delete);
    match (old, new) {
        (EventKind::Start(old), EventKind::Start(new)) => {
            let (old, new) = (old.kind(), new.kind());
            old == new || attention(old) && attention(new)
        }
        (EventKind::Html { block: old, .. }, EventKind::Html { block: new, .. }) => old == new,
        _ => core::mem::discriminant(old) == core::mem::discriminant(new),
    }
}

/// Whether two nodes are emphasis, strong or strikethrough of the same kind, which form one run when adjacent.
fn same_attention(left: &EventKind<'_>, right: &EventKind<'_>) -> bool {
    matches!(
        (left, right),
        (
            EventKind::Start(Tag::Emphasis),
            EventKind::Start(Tag::Emphasis)
        ) | (EventKind::Start(Tag::Strong), EventKind::Start(Tag::Strong))
            | (EventKind::Start(Tag::Delete), EventKind::Start(Tag::Delete))
    )
}

fn empty_text(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Text(value) if empty_value(value))
}

// Whether a value is empty, without resolving it against the source.
fn empty_value(value: &Text<'_>) -> bool {
    match value {
        Text::Source(range) => range.is_empty(),
        Text::Literal(value) => value.is_empty(),
        Text::Value(value) => value.is_empty(),
    }
}

// Whether a node is HTML without a value. In phrasing it writes nothing, so the nodes on either side meet as if it were not there, and a reference after it is written at the start of the line as if it came first.
fn empty_html(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Html { value, .. } if empty_value(value))
}

// Whether a node is inline code without a value, which Markdown cannot write, as a run of backticks with nothing between them is not code.
fn empty_code(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Code { value, .. } if empty_value(value))
}

// Whether a child of a container that does not hold phrasing is a block that Markdown cannot write: a list without items, not counting separators, which is written as nothing, or as a lone `-` where it is the only block of an item, and a table without a cell. An HTML block is one HTML construct ([`EventKind::Html`]), so it is never empty or blank.
fn unwritable_block(nodes: &[Node], node: &Node) -> bool {
    match &node.kind {
        EventKind::Start(Tag::List { .. }) => {
            core::iter::successors(node.first_child, |id| nodes[id.index()].next_sibling)
                .all(|id| matches!(nodes[id.index()].kind, EventKind::Separator { .. }))
        }
        EventKind::Start(Tag::Table) => {
            table_rows(nodes, node).all(|row| nodes[row.index()].first_child.is_none())
        }
        _ => false,
    }
}

// The rows of a table, through its heads and bodies, in order.
fn table_rows<'a>(nodes: &'a [Node], table: &'a Node) -> impl Iterator<Item = NodeId> + 'a {
    blocks(nodes, table.first_child)
        .flat_map(move |group| blocks(nodes, nodes[group.index()].first_child))
}

// Whether a table has the shape the parser gives one, so that it is written as it is: a head of
// one row, then at most one body of one row or more, with as many cells in each row, those in the
// head marked as header cells and those in the body not, and each body cell aligned as the head
// cell of its column.
fn table_shaped(nodes: &[Node], table: &Node) -> bool {
    let kind = |id: NodeId| &nodes[id.index()].kind;
    let mut groups = blocks(nodes, table.first_child);
    let (Some(head), body, None) = (groups.next(), groups.next(), groups.next()) else {
        return false;
    };
    let mut rows = blocks(nodes, nodes[head.index()].first_child);
    let (EventKind::Start(Tag::TableHead), Some(header), None) =
        (kind(head), rows.next(), rows.next())
    else {
        return false;
    };
    let cells = |row: NodeId, header: bool| {
        blocks(nodes, nodes[row.index()].first_child).map(move |cell| match kind(cell) {
            EventKind::Start(Tag::TableCell {
                alignment,
                header: flag,
            }) if *flag == header => Some(*alignment),
            _ => None,
        })
    };
    if cells(header, true).any(|cell| cell.is_none()) || cells(header, true).next().is_none() {
        return false;
    }
    let Some(body) = body else {
        return true;
    };
    matches!(kind(body), EventKind::Start(Tag::TableBody))
        && blocks(nodes, nodes[body.index()].first_child)
            .next()
            .is_some()
        && blocks(nodes, nodes[body.index()].first_child).all(|row| {
            let mut columns = cells(header, true);
            let mut row = cells(row, false);
            loop {
                match (columns.next(), row.next()) {
                    (None, None) => return true,
                    (Some(column), Some(Some(cell))) if column == Some(cell) => {}
                    _ => return false,
                }
            }
        })
}

// Whether a container holds phrasing, where separators, which are block-level, have no place.
fn phrasing_container(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(
            Tag::Paragraph { .. }
                | Tag::Heading(_)
                | Tag::TableCell { .. }
                | Tag::Emphasis
                | Tag::Strong
                | Tag::Delete
                | Tag::Link { .. }
                | Tag::Image { .. }
        )
    )
}

// Whether a node is emphasis, strong or strikethrough.
fn attention(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(Tag::Emphasis | Tag::Strong | Tag::Delete)
    )
}

// Whether a node is strikethrough, which cannot be written inside other strikethrough.
fn delete(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::Delete))
}

// Whether normalization removes a child, in a container that holds phrasing or not, without joining it into another: empty text, a separator, or HTML or code without a value, in phrasing, and attention without children.
fn removed(node: &Node, phrasing: bool) -> bool {
    empty_text(&node.kind)
        || (phrasing
            && (matches!(node.kind, EventKind::Separator { .. })
                || empty_html(&node.kind)
                || empty_code(&node.kind)))
        || (attention(&node.kind) && node.first_child.is_none())
}

// Whether a node is a paragraph without children, which Markdown cannot write.
fn empty_paragraph(node: &Node) -> bool {
    matches!(node.kind, EventKind::Start(Tag::Paragraph { .. })) && node.first_child.is_none()
}

// Whether `list` is loose with one item that holds fewer than two blocks, not counting separators or the empty paragraphs normalizing removes. No blank line can be written between its items or between the blocks of its item, so it reads back tight.
fn reads_back_tight(nodes: &[Node], list: &Node) -> bool {
    if !matches!(list.kind, EventKind::Start(Tag::List { tight: false, .. })) {
        return false;
    }
    let mut items = blocks(nodes, list.first_child);
    let (Some(item), None) = (items.next(), items.next()) else {
        return false;
    };
    blocks(nodes, nodes[item.index()].first_child)
        .nth(1)
        .is_none()
}

// The children from `first` on, without separators or the empty paragraphs normalizing removes.
fn blocks(nodes: &[Node], first: Option<NodeId>) -> impl Iterator<Item = NodeId> + '_ {
    core::iter::successors(first, move |id| nodes[id.index()].next_sibling).filter(move |id| {
        let node = &nodes[id.index()];
        !matches!(node.kind, EventKind::Separator { .. }) && !empty_paragraph(node)
    })
}

// Set whether `item`, an item, ends in a block rather than a tight paragraph.
fn set_block_end(item: &mut EventKind<'_>, value: bool) {
    let EventKind::Start(Tag::Item { block_end, .. }) = item else {
        unreachable!("only an item's ending is refreshed");
    };
    *block_end = value;
}

// Whether a child of a list, an item, is empty once normalizing removes its empty paragraphs.
fn empty_item(nodes: &[Node], node: &Node) -> bool {
    blocks(nodes, node.first_child).next().is_none()
}

// Whether a node is an item of a tight list, whose blocks are written without a blank line between them.
fn tight_item(nodes: &[Node], node: &Node) -> bool {
    matches!(node.kind, EventKind::Start(Tag::Item { .. }))
        && node.parent.is_some_and(|list| {
            matches!(
                nodes[list.index()].kind,
                EventKind::Start(Tag::List { tight: true, .. })
            )
        })
}

// Whether a node is a list whose first item, not counting separators, is empty. In an item of a tight list, such a list cannot be written after a paragraph: an empty item cannot interrupt a paragraph, and a `-` alone under one is a setext underline.
fn starts_empty(nodes: &[Node], node: &Node) -> bool {
    matches!(node.kind, EventKind::Start(Tag::List { .. }))
        && core::iter::successors(node.first_child, |id| nodes[id.index()].next_sibling)
            .find(|id| !matches!(nodes[id.index()].kind, EventKind::Separator { .. }))
            .is_some_and(|id| empty_item(nodes, &nodes[id.index()]))
}

// Whether a node is an ordered list that starts at a number other than 1. In an item of a tight list, such a list cannot be written after a paragraph: only an ordered list that starts at 1 can interrupt a paragraph.
fn misnumbered(node: &Node) -> bool {
    matches!(node.kind, EventKind::Start(Tag::List { start: Some(start), .. }) if start != 1)
}

// Whether the item at `node` is of a tight list and holds a list for which `unwritable` holds directly after a paragraph, not counting separators.
fn list_after_paragraph(nodes: &[Node], node: &Node, unwritable: impl Fn(&Node) -> bool) -> bool {
    if !tight_item(nodes, node) {
        return false;
    }
    let mut paragraph = false;
    for id in blocks(nodes, node.first_child) {
        let child = &nodes[id.index()];
        if paragraph && unwritable(child) {
            return true;
        }
        paragraph = matches!(child.kind, EventKind::Start(Tag::Paragraph { .. }));
    }
    false
}

// Whether the item at `node` is of a tight list and holds a list that starts with an empty item directly after a paragraph, not counting separators.
fn empty_item_after_paragraph(nodes: &[Node], node: &Node) -> bool {
    list_after_paragraph(nodes, node, |child| starts_empty(nodes, child))
}

// Whether the item at `node` is of a tight list and holds an ordered list that starts at a number other than 1 directly after a paragraph, not counting separators.
fn misnumbered_after_paragraph(nodes: &[Node], node: &Node) -> bool {
    list_after_paragraph(nodes, node, misnumbered)
}

// Whether the item at `node` is of a tight list and holds two blocks in a row, not counting separators, for which `pair` holds.
fn tight_item_pair(nodes: &[Node], node: &Node, pair: impl Fn(NodeId, NodeId) -> bool) -> bool {
    if !tight_item(nodes, node) {
        return false;
    }
    let mut left = None;
    for right in blocks(nodes, node.first_child) {
        if left.is_some_and(|left| pair(left, right)) {
            return true;
        }
        left = Some(right);
    }
    false
}

// Whether the item `item` is of a tight list and holds two blocks in a row, not counting separators, that can be written apart only with a blank line between them, which makes the list loose, as the serializer decides with `needs_blank_line`, other than two paragraphs, which normalizing joins instead. A heading counts as written setext only when it holds a soft or hard break: the serializer writes any other heading there ATX, whatever its options, and options that write one with a break ATX too, such as unwrapping, write the list loose once it is.
fn loosens<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> bool {
    let nodes = &document.nodes;
    let paragraph = |id: NodeId| {
        matches!(
            nodes[id.index()].kind,
            EventKind::Start(Tag::Paragraph { .. })
        )
    };
    tight_item_pair(nodes, &nodes[item.index()], |left, right| {
        !(paragraph(left) && paragraph(right))
            && needs_blank_line(document, left, right, |heading| {
                heading_as_setext(document, heading, false, true)
            })
    })
}

// Whether the item at `node` is of a tight list and holds two paragraphs in a row, not counting separators.
fn adjacent_paragraphs(nodes: &[Node], node: &Node) -> bool {
    let paragraph = |node: &Node| matches!(node.kind, EventKind::Start(Tag::Paragraph { .. }));
    tight_item_pair(nodes, node, |left, right| {
        paragraph(&nodes[left.index()]) && paragraph(&nodes[right.index()])
    })
}

// Whether a node is a separator between blocks.
fn separator(nodes: &[Node], id: Option<NodeId>) -> bool {
    id.is_some_and(|id| matches!(nodes[id.index()].kind, EventKind::Separator { .. }))
}

// The last child of the paragraph `id`, which has children once empty paragraphs are removed.
fn last_child(nodes: &[Node], id: NodeId) -> NodeId {
    core::iter::successors(nodes[id.index()].first_child, |id| {
        nodes[id.index()].next_sibling
    })
    .last()
    .expect("empty paragraphs are removed before paragraphs are joined")
}

// The first node of the subtree at `id` in post-order: its deepest first descendant.
fn first_leaf(nodes: &[Node], mut id: NodeId) -> NodeId {
    while let Some(child) = nodes[id.index()].first_child {
        id = child;
    }
    id
}

// The span of two joined nodes. Constructed nodes have an empty span, which joining does not extend.
fn joined_span(left: &Range<usize>, right: &Range<usize>) -> Range<usize> {
    if left.is_empty() {
        right.clone()
    } else if right.is_empty() {
        left.clone()
    } else {
        left.start.min(right.start)..left.end.max(right.end)
    }
}

// Whether normalization joins `kind` into the child kept before it: text after text, inline code after inline code that has no language of its own, and attention after attention of the same kind.
fn joined(kept: Option<&EventKind<'_>>, kind: &EventKind<'_>) -> bool {
    kept.is_some_and(|kept| {
        matches!(
            (kept, kind),
            (EventKind::Text(_), EventKind::Text(_))
                | (
                    EventKind::Code { .. },
                    EventKind::Code { language: None, .. }
                )
        ) || same_attention(kept, kind)
    })
}

// Whether a node is a table cell, where Markdown cannot write a hard break.
fn table_cell(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::TableCell { .. }))
}

// Whether a node is a heading of rank 3 or more, which only an ATX heading can write.
fn atx_heading(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::Heading(rank)) if *rank > 2)
}

// Whether a node is a paragraph, heading or table cell, whose phrasing normalization removes breaks at the edges of.
fn block_phrasing(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::TableCell { .. })
    )
}

// Whether a link is written as an autolink, `<…>`, which is the only link the parser reads inside
// another link's text: one stored in that form, without a title, whose only child is text that
// reads back as an autolink to its destination, and in a table cell, without a `|`, as the
// serializer writes it.
fn autolink(
    syntax: &BTreeMap<NodeId, Option<NodeSyntax<'static>>>,
    source: &str,
    nodes: &[Node],
    id: NodeId,
    cell: bool,
) -> bool {
    let node = &nodes[id.index()];
    let EventKind::Start(Tag::Link(Target::Resource {
        destination,
        title: None,
    })) = &node.kind
    else {
        return false;
    };
    let form = match syntax.get(&id) {
        Some(stored) => stored.clone(),
        None => crate::node_syntax::infer(source, node.span.clone(), &node.kind),
    };
    // Its text, joined as normalizing joins it.
    let mut text = Cow::Borrowed("");
    for child in core::iter::successors(node.first_child, |id| nodes[id.index()].next_sibling) {
        let EventKind::Text(value) = &nodes[child.index()].kind else {
            return false;
        };
        let value = value.resolve(source).expect("document text is validated");
        if text.is_empty() {
            text = Cow::Borrowed(value);
        } else {
            text.to_mut().push_str(value);
        }
    }
    matches!(
        form,
        Some(NodeSyntax::Link {
            form: crate::LinkForm::Angle
        })
    ) && angle_reads_back(
        &text,
        destination
            .resolve(source)
            .expect("document text is validated"),
    ) && !(cell && text.contains('|'))
}

// Whether normalizing unwraps a node, given the nodes that enclose it: strikethrough inside other
// strikethrough, and a link inside another link that is not written as an autolink.
fn unwrapped(
    syntax: &BTreeMap<NodeId, Option<NodeSyntax<'static>>>,
    source: &str,
    nodes: &[Node],
    id: NodeId,
    enclosing: Enclosing,
) -> bool {
    match nodes[id.index()].kind {
        EventKind::Start(Tag::Delete) => enclosing.deletes > 0,
        EventKind::Start(Tag::Link { .. }) => {
            enclosing.links > 0 && !autolink(syntax, source, nodes, id, enclosing.cells > 0)
        }
        _ => false,
    }
}

// The strikethrough, links and table cells that enclose a node.
#[derive(Clone, Copy, Default)]
struct Enclosing {
    deletes: usize,
    links: usize,
    cells: usize,
    // Headings of rank 3 or more, which are written ATX, on one line.
    headings: usize,
}

impl Enclosing {
    // Count a node as enclosing the nodes below it.
    fn enter(&mut self, kind: &EventKind<'_>) {
        self.deletes += usize::from(delete(kind));
        self.links += usize::from(matches!(kind, EventKind::Start(Tag::Link { .. })));
        self.cells += usize::from(table_cell(kind));
        self.headings += usize::from(atx_heading(kind));
    }
    // Stop counting a node once the nodes below it are passed.
    fn leave(&mut self, kind: &EventKind<'_>) {
        self.deletes -= usize::from(delete(kind));
        self.links -= usize::from(matches!(kind, EventKind::Start(Tag::Link { .. })));
        self.cells -= usize::from(table_cell(kind));
        self.headings -= usize::from(atx_heading(kind));
    }
    // Whether what this encloses is written on one line, where a line ending in a label cannot be.
    fn one_line(self) -> bool {
        self.cells > 0 || self.headings > 0
    }
}

// The node after `id` in document order within the subtree at `root`, keeping `enclosing` as the strikethrough, links and table cells that enclose the current node.
fn next_enclosed(
    nodes: &[Node],
    root: NodeId,
    id: NodeId,
    enclosing: &mut Enclosing,
) -> Option<NodeId> {
    let node = &nodes[id.index()];
    if let Some(child) = node.first_child {
        enclosing.enter(&node.kind);
        return Some(child);
    }
    let mut at = id;
    loop {
        if at == root {
            return None;
        }
        let node = &nodes[at.index()];
        if let Some(next) = node.next_sibling {
            return Some(next);
        }
        at = node
            .parent
            .expect("a descendant has a parent before its subtree root");
        enclosing.leave(&nodes[at.index()].kind);
    }
}

// How much of a child, at one of its edges, is whitespace that normalization moves out of attention.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    // The child does not start or end with whitespace.
    None,
    // The text has whitespace at this edge, up to or from this byte offset.
    Part(usize),
    // The whole child is whitespace.
    All,
}

// Whitespace at the edge of emphasis, strong or strikethrough, which Markdown cannot write there: Unicode whitespace, as CommonMark flanking classifies it.
fn unicode_whitespace(character: char) -> bool {
    character.is_whitespace()
}

// The whitespace at the start of a child: a soft break, or text that starts with it.
fn leading_edge(kind: &EventKind<'_>, source: &str) -> Edge {
    match kind {
        EventKind::SoftBreak(_) => Edge::All,
        EventKind::Text(text) => {
            let value = text.resolve(source).expect("document text is validated");
            let rest = value.trim_start_matches(unicode_whitespace);
            if rest.is_empty() {
                Edge::All
            } else if rest.len() == value.len() {
                Edge::None
            } else {
                Edge::Part(value.len() - rest.len())
            }
        }
        _ => Edge::None,
    }
}

// The whitespace at the end of a child: a soft or hard break, or text that ends with it.
fn trailing_edge(kind: &EventKind<'_>, source: &str) -> Edge {
    match kind {
        EventKind::SoftBreak(_) | EventKind::HardBreak => Edge::All,
        EventKind::Text(text) => {
            let value = text.resolve(source).expect("document text is validated");
            let rest = value.trim_end_matches(unicode_whitespace);
            if rest.is_empty() {
                Edge::All
            } else if rest.len() == value.len() {
                Edge::None
            } else {
                Edge::Part(rest.len())
            }
        }
        _ => Edge::None,
    }
}

// Whether a child of a task item's paragraph is a break, or the one space that separates the checkbox from its text, given whether `separator` has been met already, which it updates. A checkbox followed only by these cannot be written, as nothing would follow the space written after it, and the parser keeps that space in the text after the checkbox.
fn unwritten_after_checkbox(kind: &EventKind<'_>, source: &str, separator: &mut bool) -> bool {
    match kind {
        EventKind::SoftBreak(_) | EventKind::HardBreak => true,
        EventKind::Text(text)
            if !*separator
                && matches!(
                    text.resolve(source).expect("document text is validated"),
                    " " | "\t"
                ) =>
        {
            *separator = true;
            true
        }
        _ => false,
    }
}

// Whether the first and last children of `node` have no whitespace that normalization moves out of attached attention, and no break that it removes from the edges of a paragraph, heading or table cell. A soft break that starts the paragraph of a task item, after its checkbox, stays, as the parser reads it there; `task` is whether `node` is one.
fn edges_normalized(
    node: &Node,
    first: &EventKind<'_>,
    last: &EventKind<'_>,
    task: bool,
    source: &str,
) -> bool {
    if attention(&node.kind) && node.parent.is_some() {
        leading_edge(first, source) == Edge::None && trailing_edge(last, source) == Edge::None
    } else if block_phrasing(&node.kind) {
        (task || !matches!(first, EventKind::SoftBreak(_)))
            && !matches!(last, EventKind::SoftBreak(_) | EventKind::HardBreak)
    } else {
        true
    }
}

// Split `text` at byte `at`, keeping the part before it and returning the part after it, without copying source or literal text.
fn split_text(text: &mut Text<'static>, at: usize) -> Text<'static> {
    match text {
        Text::Source(range) => {
            let tail = range.start + at..range.end;
            range.end = tail.start;
            Text::Source(tail)
        }
        Text::Literal(value) => {
            let (head, tail) = value.split_at(at);
            *value = head;
            Text::Literal(tail)
        }
        Text::Value(Cow::Borrowed(value)) => {
            let (head, tail) = value.split_at(at);
            *value = head;
            Text::Value(Cow::Borrowed(tail))
        }
        Text::Value(Cow::Owned(value)) => Text::Value(Cow::Owned(value.split_off(at))),
    }
}

// Join the value of `right` onto `left`, extending a source range when the two are contiguous.
fn join_text(left: &mut Text<'static>, right: &Text<'_>, source: &str) {
    match (&*left, right) {
        (Text::Source(left_range), Text::Source(right_range))
            if left_range.end == right_range.start =>
        {
            *left = Text::Source(left_range.start..right_range.end);
        }
        _ => left
            .to_mut(source)
            .expect("document text is validated")
            .push_str(right.resolve(source).expect("document text is validated")),
    }
}

// Marks of the children kept when copies are removed: a copy went, or a node moved in, just
// before the child; the child is a separator that came just after a copy; and the child moved
// in from a copy.
const CHANGED: u8 = 1;
const AFTER_COPY: u8 = 2;
const MOVED: u8 = 4;

// Add a constructed node to the arena as a child of `parent`, with an empty span, for the caller to link.
fn push_node(nodes: &mut Vec<Node>, kind: EventKind<'static>, parent: NodeId) -> NodeId {
    let id = NodeId::new(nodes.len());
    nodes.push(Node {
        kind,
        span: 0..0,
        parent: Some(parent),
        first_child: None,
        next_sibling: None,
    });
    id
}

// Whether the value of a code block cannot be written where it is: it is not empty and has no final line ending, which only the last block of a document can lack, as the parser stores one for an unclosed fence or indented code that ends the source without one ([`EventKind::CodeBlock`]). `last` is whether the block is the last node of the document.
fn unwritten_block_value(value: &str, last: bool) -> bool {
    !(last || value.is_empty() || value.ends_with('\n'))
}

// Add a final line ending to the value of a code block that cannot be written without one, as `unwritten_block_value` says.
fn write_block_value(kind: &mut EventKind<'static>, source: &str, last: bool) {
    let EventKind::CodeBlock { value, .. } = kind else {
        return;
    };
    let resolved = value.resolve(source).expect("document text is validated");
    if !unwritten_block_value(resolved, last) {
        return;
    }
    let mut written = String::with_capacity(resolved.len() + 1);
    written.push_str(resolved);
    written.push('\n');
    *value = Text::from(written);
}

// How `normalize_label` changes a label.
enum LabelChange {
    // The label becomes this one.
    Set(String),
    // A link or image becomes a resource, or a footnote call is written as its identifier.
    Drop,
}

// How the label of a reference, definition or footnote call changes to get the shape that reads back wherever it is written, as `node_syntax::normalized_label` describes, where it is written on `one_line` or not, if it does. A link or image whose label still does not fit wherever it is written becomes a resource, and such a footnote call is written as its identifier, when that fits.
fn label_change(kind: &EventKind<'_>, source: &str, one_line: bool) -> Option<LabelChange> {
    use crate::node_syntax::{label_fits_anywhere, normalized_label, valid_label};
    fn resolved<'v>(value: &'v Text<'_>, source: &'v str) -> &'v str {
        value.resolve(source).expect("document text is validated")
    }
    let resolve = |value| resolved(value, source);
    match kind {
        EventKind::Start(Tag::Link(target) | Tag::Image(target)) => {
            let reference = target.reference()?;
            let label = resolve(&reference.label);
            let full = reference.kind == crate::ReferenceKind::Full;
            let normalized = normalized_label(label, one_line, full, false);
            if label_fits_anywhere(normalized.as_deref().unwrap_or(label)) {
                normalized.map(LabelChange::Set)
            } else {
                Some(LabelChange::Drop)
            }
        }
        EventKind::Definition { label, .. } => {
            normalized_label(resolve(label), false, true, true).map(LabelChange::Set)
        }
        EventKind::FootnoteReference { identifier, label } => {
            let identifier = resolve(identifier);
            let written = label.as_ref().map_or(identifier, resolve);
            let normalized = normalized_label(written, one_line, true, false);
            if label.is_some()
                && !label_fits_anywhere(normalized.as_deref().unwrap_or(written))
                && valid_label(identifier, true)
                && normalized_label(identifier, one_line, true, false).is_none()
            {
                Some(LabelChange::Drop)
            } else {
                normalized.map(LabelChange::Set)
            }
        }
        _ => None,
    }
}

// Apply `label_change` to node `id`. A reference that becomes a resource takes the destination
// and title of the definition it is bound to in the tree under `root`, looked up with `lookup`
// once the first such reference makes it, and `index`, that of the attached document.
fn normalize_label(
    nodes: &mut [Node],
    index: &crate::bindings::Index,
    root: NodeId,
    id: NodeId,
    lookup: &mut Option<crate::bindings::Lookup>,
    source: &str,
    one_line: bool,
) {
    let Some(change) = label_change(&nodes[id.index()].kind, source, one_line) else {
        return;
    };
    // A reference whose label is dropped becomes a resource to where it leads.
    let resource = match (&nodes[id.index()].kind, &change) {
        (kind @ EventKind::Start(Tag::Link(_) | Tag::Image(_)), LabelChange::Drop) => {
            let cx = crate::bindings::Context {
                source,
                nodes: &*nodes,
                index,
                root: NodeId::new(0),
            };
            let definition = lookup
                .get_or_insert_with(|| crate::bindings::Lookup::new(cx, root))
                .definition_of(cx, kind, &mut crate::bindings::Scratch::default())
                .expect("normalizing leaves no unbound reference");
            match &nodes[definition.index()].kind {
                EventKind::Definition {
                    destination, title, ..
                } => Some(Target::Resource {
                    destination: destination.clone(),
                    title: title.clone(),
                }),
                _ => unreachable!("a binding is a definition"),
            }
        }
        _ => None,
    };
    match (&mut nodes[id.index()].kind, change) {
        (EventKind::Start(Tag::Link(target) | Tag::Image(target)), LabelChange::Drop) => {
            *target = resource.expect("a dropped label leaves a resource");
        }
        (EventKind::Start(Tag::Link(target) | Tag::Image(target)), LabelChange::Set(label)) => {
            match target {
                Target::Reference(reference) => reference.label = Text::from(label),
                _ => unreachable!("a document stores a reference by name"),
            }
        }
        (EventKind::Definition { label, .. }, LabelChange::Set(value)) => {
            *label = Text::from(value);
        }
        (EventKind::FootnoteReference { label, .. }, LabelChange::Drop) => *label = None,
        (EventKind::FootnoteReference { identifier, label }, LabelChange::Set(value)) => {
            *label.as_mut().unwrap_or(identifier) = Text::from(value);
        }
        _ => unreachable!("only a link, image or footnote call drops its label"),
    }
}

// Whether `normalize_label` would change nothing.
fn label_normalized(kind: &EventKind<'_>, source: &str, one_line: bool) -> bool {
    label_change(kind, source, one_line).is_none()
}

// Scratch storage for normalizing, reused across trees so that normalizing allocates it once.
#[derive(Default)]
struct Scratch {
    trailing: Trailing,
    blocks: Vec<NodeId>,
    copies: Copies,
    // The lists made tight or loose.
    lists: Vec<NodeId>,
}

// Scratch storage for removing copies: which nodes are copies and which containers hold one at
// some depth, by index, and the indices set, which alone are cleared once a tree is done, so
// that each tree costs what it touches, not the size of the arena.
#[derive(Default)]
struct Copies {
    copy: Vec<bool>,
    above: Vec<bool>,
    // The containers a table at the end of a list or footnote definition was looked for through.
    passed: Vec<bool>,
    touched: Vec<usize>,
    // The nodes of a copy still to be dismantled.
    dismantled: Vec<NodeId>,
}

impl Copies {
    fn reserve(&mut self, len: usize) {
        if self.copy.len() < len {
            self.copy.resize(len, false);
            self.above.resize(len, false);
            self.passed.resize(len, false);
        }
    }

    fn clear(&mut self) {
        for index in self.touched.drain(..) {
            self.copy[index] = false;
            self.above[index] = false;
            self.passed[index] = false;
        }
    }
}

// Scratch storage for moving whitespace out of the ends of nested attention, reused across child lists so normalizing allocates it once.
#[derive(Default)]
struct Trailing {
    // The children of each level searched that stay when the whitespace after them moves out, with the child before each.
    kept: Vec<(NodeId, Option<NodeId>)>,
    // Each level searched, with the child before it and where its kept children start.
    levels: Vec<(NodeId, Option<NodeId>, usize)>,
}

// Move the whitespace at the start of `attention`, after `previous` in its parent, out of it and out of the attention nested at its start, to just before it, updating `previous` to the child now before it. Attention left empty is removed, and the whitespace takes its place. A hard break stays. Returns whether anything changed.
fn move_leading(
    nodes: &mut Vec<Node>,
    source: &str,
    previous: &mut Option<NodeId>,
    attention_id: NodeId,
) -> bool {
    let parent = nodes[attention_id.index()]
        .parent
        .expect("attention with movable edges has a parent");
    let mut changed = false;
    let mut level = attention_id;
    loop {
        let Some(first) = nodes[level.index()].first_child else {
            // Every child of the level moved out: unlink it from the level that holds it, where it was the first child, and continue there.
            let up = nodes[level.index()]
                .parent
                .expect("a level below the attention has a parent");
            let next = nodes[level.index()].next_sibling.take();
            match (level == attention_id).then_some(*previous).flatten() {
                Some(before) => nodes[before.index()].next_sibling = next,
                None => nodes[up.index()].first_child = next,
            }
            nodes[level.index()].parent = None;
            if level == attention_id {
                return true;
            }
            level = up;
            continue;
        };
        let edge = leading_edge(&nodes[first.index()].kind, source);
        let moved = match (&mut nodes[first.index()].kind, edge) {
            (_, Edge::All) => {
                nodes[level.index()].first_child = nodes[first.index()].next_sibling;
                first
            }
            (EventKind::Text(text), Edge::Part(at)) => {
                let rest = split_text(text, at);
                let head = core::mem::replace(text, rest);
                push_node(nodes, EventKind::Text(head), parent)
            }
            _ => {
                if attention(&nodes[first.index()].kind) {
                    level = first;
                    continue;
                }
                return changed;
            }
        };
        nodes[moved.index()].parent = Some(parent);
        nodes[moved.index()].next_sibling = Some(attention_id);
        match *previous {
            Some(before) => nodes[before.index()].next_sibling = Some(moved),
            None => nodes[parent.index()].first_child = Some(moved),
        }
        *previous = Some(moved);
        changed = true;
        if edge != Edge::All {
            return true;
        }
    }
}

// Move the whitespace at the end of `attention`, after `previous` in its parent, out of it and out of the attention nested at its end, to just after it, keeping its order. Attention left empty is removed, and the whitespace takes its place. Hard breaks move too. Returns whether anything changed.
fn move_trailing(
    nodes: &mut Vec<Node>,
    source: &str,
    previous: Option<NodeId>,
    attention_id: NodeId,
    scratch: &mut Trailing,
) -> bool {
    let parent = nodes[attention_id.index()]
        .parent
        .expect("attention with movable edges has a parent");
    let Trailing { kept, levels } = scratch;
    kept.clear();
    levels.clear();
    let mut changed = false;
    let mut level = Some((attention_id, previous));
    loop {
        if let Some((id, before)) = level.take() {
            // Search the level once, keeping every child that does not move, so that if the last of them is emptied the one before it is at hand.
            levels.push((id, before, kept.len()));
            let mut previous = None;
            let mut child = nodes[id.index()].first_child;
            while let Some(current) = child {
                if trailing_edge(&nodes[current.index()].kind, source) != Edge::All {
                    kept.push((current, previous));
                }
                previous = Some(current);
                child = nodes[current.index()].next_sibling;
            }
        }
        let &(id, before, start) = levels.last().expect("a level is being searched");
        let last = if kept.len() > start { kept.pop() } else { None };
        // Move the whitespace after the last child that stays out of the level, to just after the attention and before whitespace moved out of levels it holds.
        let run = match last {
            Some((child, _)) => nodes[child.index()].next_sibling.take(),
            None => nodes[id.index()].first_child.take(),
        };
        if let Some(first) = run {
            let mut end = first;
            loop {
                nodes[end.index()].parent = Some(parent);
                match nodes[end.index()].next_sibling {
                    Some(next) => end = next,
                    None => break,
                }
            }
            nodes[end.index()].next_sibling = nodes[attention_id.index()].next_sibling;
            nodes[attention_id.index()].next_sibling = Some(first);
            changed = true;
        }
        let Some((child, child_before)) = last else {
            // Every child of the level moved out: unlink it from the level that holds it, which now ends with it, and continue there.
            levels.pop();
            let up = nodes[id.index()]
                .parent
                .expect("a level below the attention has a parent");
            let next = nodes[id.index()].next_sibling.take();
            match before {
                Some(before) => nodes[before.index()].next_sibling = next,
                None => nodes[up.index()].first_child = next,
            }
            nodes[id.index()].parent = None;
            if levels.is_empty() {
                return true;
            }
            continue;
        };
        let edge = trailing_edge(&nodes[child.index()].kind, source);
        if let (EventKind::Text(text), Edge::Part(at)) = (&mut nodes[child.index()].kind, edge) {
            let tail = split_text(text, at);
            let moved = push_node(nodes, EventKind::Text(tail), parent);
            nodes[moved.index()].next_sibling = nodes[attention_id.index()].next_sibling;
            nodes[attention_id.index()].next_sibling = Some(moved);
            return true;
        }
        if !attention(&nodes[child.index()].kind) {
            return changed;
        }
        level = Some((child, child_before));
    }
}

// Where to continue normalizing the children of `parent` after whitespace moved out after `attention_id`, which followed `previous`: the last child linked and the child after it, from what moved.
fn resume(
    nodes: &[Node],
    parent: NodeId,
    attention_id: NodeId,
    previous: Option<NodeId>,
) -> (Option<NodeId>, Option<NodeId>) {
    let linked = if nodes[attention_id.index()].parent.is_some() {
        Some(attention_id)
    } else {
        previous
    };
    let current = match linked {
        Some(linked) => nodes[linked.index()].next_sibling,
        None => nodes[parent.index()].first_child,
    };
    (linked, current)
}

// Whether the phrasing of the task paragraph `paragraph` is only breaks and one text of the space or tab that separates the checkbox from its text, so that nothing would be written after the separator.
fn unwritten_task(nodes: &[Node], paragraph: NodeId, source: &str) -> bool {
    let mut separator = false;
    core::iter::successors(nodes[paragraph.index()].first_child, |&id| {
        nodes[id.index()].next_sibling
    })
    .all(|id| unwritten_after_checkbox(&nodes[id.index()].kind, source, &mut separator))
}

// Whether the task paragraph `paragraph` starts with what separates the checkbox from its text: a soft break, or text that starts with a space or tab, which the serializer writes after the checkbox.
fn separated_task(nodes: &[Node], paragraph: NodeId, source: &str) -> bool {
    nodes[paragraph.index()]
        .first_child
        .is_some_and(|first| match &nodes[first.index()].kind {
            EventKind::SoftBreak(_) => true,
            EventKind::Text(text) => text
                .resolve(source)
                .expect("document text is validated")
                .starts_with([' ', '\t']),
            _ => false,
        })
}

// The paragraph that the checkbox of the task item `item` is written before: its first child other than definitions and separators, if that is a paragraph, or `None` when `item` is not a task item.
fn task_paragraph(nodes: &[Node], item: NodeId) -> Option<NodeId> {
    if !matches!(
        nodes[item.index()].kind,
        EventKind::Start(Tag::Item {
            checked: Some(_),
            ..
        })
    ) {
        return None;
    }
    core::iter::successors(nodes[item.index()].first_child, |&id| {
        nodes[id.index()].next_sibling
    })
    .find(|&id| {
        !matches!(
            nodes[id.index()].kind,
            EventKind::Definition { .. } | EventKind::Separator { .. }
        )
    })
    .filter(|&id| {
        matches!(
            nodes[id.index()].kind,
            EventKind::Start(Tag::Paragraph { .. })
        )
    })
}

// Make `item` an ordinary item rather than a task item.
fn clear_task(nodes: &mut [Node], item: NodeId) {
    let EventKind::Start(Tag::Item { checked, .. }) = &mut nodes[item.index()].kind else {
        unreachable!("only task items are cleared");
    };
    *checked = None;
}

// Remove the soft breaks at either edge of the phrasing of `parent`, a paragraph, heading or table cell, and the hard breaks at its end, which the parser does not read there. `task` is whether `parent` is a task item's paragraph, whose checkbox is written before it: a soft break at its start stays, as the parser reads one there, unless nothing would be written after the checkbox, as the phrasing is only breaks and one text of the space or tab that separates the checkbox from its text, in which case the item stops being a task item first. Returns whether anything changed.
fn trim_breaks(nodes: &mut [Node], parent: NodeId, source: &str, task: bool) -> bool {
    let mut changed = false;
    let mut leading = true;
    if task {
        if unwritten_task(nodes, parent, source) {
            let item = nodes[parent.index()]
                .parent
                .expect("a task paragraph is in its item");
            clear_task(nodes, item);
            changed = true;
        } else {
            leading = false;
        }
    }
    while let Some(first) = nodes[parent.index()].first_child.filter(|_| leading) {
        if !matches!(nodes[first.index()].kind, EventKind::SoftBreak(_)) {
            break;
        }
        nodes[parent.index()].first_child = nodes[first.index()].next_sibling.take();
        nodes[first.index()].parent = None;
        changed = true;
    }
    let mut last = None;
    let mut child = nodes[parent.index()].first_child;
    while let Some(id) = child {
        if !matches!(
            nodes[id.index()].kind,
            EventKind::SoftBreak(_) | EventKind::HardBreak
        ) {
            last = Some(id);
        }
        child = nodes[id.index()].next_sibling;
    }
    let mut child = match last {
        Some(last) => nodes[last.index()].next_sibling.take(),
        None => nodes[parent.index()].first_child.take(),
    };
    while let Some(id) = child {
        child = nodes[id.index()].next_sibling.take();
        nodes[id.index()].parent = None;
        changed = true;
    }
    changed
}

impl<S: AsRef<str>> Document<S> {
    /// Normalize every child list in place, so that the tree is one the Markdown serializer can write correctly and stably, in time linear in the size of the arena.
    ///
    /// What this guarantees, idempotence, the round trip and which HTML changes, is stated in [the contract](crate::contract), sections 3 and 4; this says how each rule works.
    ///
    /// Empty text is removed, and so is emphasis, strong or strikethrough that has no children once its own children are normalized, such as emphasis of empty text or strong that holds only empty emphasis. Strikethrough inside other strikethrough, at any depth, is unwrapped: its children take its place, since strikethrough cannot be written inside strikethrough and renders the same once. A link inside another link's text, at any depth, is unwrapped the same way, first of all, so its text stays and is normalized with what is around it, and one without children is removed, as the parser reads no link in a link's text other than an autolink: only a link stored as an autolink, `<…>`, without a title, whose children are text that reads back as an autolink to its destination, and in a table cell holds no `|`, stays. The serializer's `resource_link` option, which writes every link as a resource, is not known here, so such a link stays even when that option would write it as a resource that does not read back. A table is given the shape the parser gives one, so that it is written as it is: its rows, in order through its heads and bodies, become a head of the first row and a body of the others, if any, each row is given empty cells up to as many as the widest row has, each cell is marked as a header cell in the head and not in the body, and aligned as the head cell of its column, an empty head cell added being unaligned, and its separators are replaced with those the parser gives a table. A table without a cell cannot be written, so it is removed with one separator beside it. Adjacent text is joined into the first text, adjacent inline code is joined into the first code when the second has no language of its own, as adjacent code spans cannot be written apart. Adjacent emphasis, strong or strikethrough of the same kind is merged by moving the children of the second to the end of the first.
    ///
    /// Markdown cannot write whitespace at the edges of emphasis, strong or strikethrough, so it moves out: Unicode whitespace in text and soft breaks at either edge, and hard breaks at the end, move out through any attention nested at that edge to just before or after the outermost attention whose edge they are. Attention left empty is removed, and what moved out takes its place. A hard break at the start of attention stays, and links and images keep whitespace at their edges. The parser reads no break at the edges of a paragraph, heading or table cell, so soft breaks at either edge of its phrasing are removed, and so are hard breaks at the end of a paragraph or heading, including those that moved out of attention; a hard break at the start stays.
    ///
    /// Children that meet where a node was removed, moved, unwrapped or merged are normalized in turn, so text, code or attention on either side is joined or merged too. A separator sits between blocks, where it stays and is skipped when looking for an adjacent child. A paragraph with no children left, such as one of breaks alone, cannot be written, so it is removed with one separator beside it, the one after it if there is one, as the blocks around it render the same without it. Inline HTML without a value writes nothing, and inline code without one could be written only as a run of backticks, which is not code, so each is removed, before anything above, and what was on either side meets: text joins, attention merges, and attention or a paragraph it leaves empty is removed, so a link reference after it starts its paragraph and is escaped as such, `[r]\: /u` rather than `[r]: /u`, which reads back as a definition. An HTML block is never empty or blank, as its value is one block construct ([`EventKind::Html`]). A list without items, not counting separators, is written as nothing, or as a lone `-` where it is the only block of an item, so it is removed with one separator beside it, like an empty paragraph, and also before anything above, so the rules below see the items they leave empty: an item of `p` and a list whose only item holds a list without items is written `- p`, where `- p\n  -` read back as a heading. The parser produces none of these. A loose list whose only item is left with fewer than two blocks, not counting separators, cannot be written loose, as no blank line can be written between its items or its item's blocks, so it is made tight, which changes only how that item's paragraph renders; a lone item with two blocks or more is written with a blank line between them instead, and a list with other items is written with blank lines between them. An empty item cannot interrupt a paragraph, and a `-` alone under one is a setext underline, so in an item of a tight list, where no blank line can come between them, a list directly after a paragraph, not counting separators, cannot start with an empty item, one with no blocks once its empty paragraphs are removed: such a list's leading empty items are removed, as each next one becomes the first, so items of `p` and a list of an empty item and `a` are written `- p\n  - a`, where `- p\n  -` read back as a heading. Later empty items stay. A list left with no item is removed with one separator beside it, the one after it if there is one, and a list after it then follows the paragraph in turn. Only an ordered list that starts at 1 can interrupt a paragraph, so in an item of a tight list, an ordered list directly after a paragraph, not counting separators, that starts at another number, including one whose leading empty items were removed, is made to start at 1: `- p\n  3. a` reads back as text, so it is written `- p\n  1. a`. Its items and text stay; only its numbering changes. In an item of a tight list, two paragraphs in a row, not counting separators, cannot be written either, as the blank line between them would make the list loose, so once the rules above have run, each run of them is joined into its first paragraph, with a soft break between each, and the separators between them are removed: a tight list has no `<p>`, so this renders almost the same, and items of `a` and `b` are written `- a\n  b`. Some blocks in a row in an item of a tight list can be written apart only with a blank line between them, which makes the list loose, and the serializer decides which with the same rule: two block quotes or two tables, which would otherwise read back as one; a link reference definition, a heading that only a setext heading can write, as it holds a soft or hard break, or an HTML block of kind 7, after a paragraph, which none of them can interrupt, and a paragraph, link reference definition or such a heading after a table, whose rows it would be read as, which the parser never gives but a definition hoisted out of a copy can be; any block after an HTML block of kind 6 or 7, which ends only at a blank line, or of kinds 1–5 whose last line does not hold its end condition, as it would take the block as more of its lines, so `- <div>\n  a` reads back as one HTML block; and a paragraph, table, link reference definition or such a heading after a list or footnote definition that ends in a paragraph or link reference definition, through the last blocks of any containers in it, which would otherwise continue it lazily, or after a block quote whose last block is such a list or definition, where the line of `>` that would end the quote's paragraph is a blank line in that list or definition. Such a list is made loose, with the paragraphs of all its items, and its paragraphs in a row are then left apart. Any other heading there is written as an ATX heading, which interrupts a paragraph and ends the containers around it, with a line ending in its text written as `&#xA;`, so it leaves the list tight, as a parsed heading there always does; the serializer's options are not known here, so a heading with a soft break makes the list loose even when unwrapping would write it as a space. A block after a list that does not end in a paragraph or definition, such as one whose last item is empty or a heading, is written on the next line, a table after a paragraph interrupts it, and a block after a block quote that ends in a paragraph in any other way is written after a line of `>`, so these stay in a tight list. A task item's checkbox is written before its first block other than definitions and separators, and the parser keeps the space or tab that separates the checkbox from its text at the start of that text. Markdown cannot write a checkbox with nothing after its separator, so when that paragraph is only breaks and at most one text that is a single space or tab, the item stops being a task item, and the rules above apply to what is left: its breaks are removed, the separator stays as text, and a paragraph left empty is removed. So an item of a checkbox alone, or of a checkbox and a hard break, is left empty, and removed if it starts a list after a paragraph as above, and one of a checkbox and text ` ` keeps the space, which is written `- &#x20;`. Attention holding only whitespace and breaks has none left by then, as they move out of it. Anything else after the checkbox keeps it, including whitespace after the separator, which the serializer encodes, so a checkbox and text `  ` is written `- [ ] &#x20;`, and only trailing breaks are removed. Once the rules above have run, a task item whose first block other than definitions and separators is not a paragraph, or whose paragraph would write nothing after the checkbox, stops being a task item, and the paragraph of any other gets a space at its start when it does not start with a space, a tab or a soft break, as the serializer writes the checkbox's separator there and the parser reads it back as text. No value is rewritten but as follows, as each already has the form the parser stores, so no character of one changes ([`EventKind`]). The parser stores the last line of a closed fence with its line ending, so a code block value that is not empty and has no final line ending gets LF added: `a` becomes `a\n`. An empty value stays, as `` ```\n``` `` reads back as one. The last node of the document, attached, keeps a value without a final line ending, as the parser stores one for an unclosed fence or indented code that ends the source without a line ending, such as `` ```\na ``, so normalizing a parsed document leaves it as it is. A label of a link, image, definition or footnote call is written as it is, so it is given the shape that reads back wherever it is written with the default parse options: a line of it after the first that would start a block there, such as `# b`, `> b`, a fence, a list item or an HTML block, which a parser with that construct turned off reads as part of the label, or whose leading tabs would start one at some columns and not at others, or that could be a table's delimiter row under whatever line is written before it, has its leading whitespace written as four spaces, which starts none; a label longer than the parser reads at some column, counting each tab as four, has its other tabs written as one space each. An ATX heading or table cell cannot hold a line ending, so in a label there each run of whitespace that holds one becomes one space. A link or image whose label is still longer than a full reference's can be at some column cannot be written as a reference, so it becomes a resource, with the destination and title of the definition it is bound to, which renders the same, and such a footnote call loses its label and is written as its identifier when that fits. Other lines are left as they are, so a parsed label changes only where one of these holds, such as a line `|-|` under a head of another number of cells, which a head of the right number, written before the label, would make a table. Nothing else changes: text keeps its value, as whitespace that moves out of attention is split off whole and keeps its characters, and the serializer encodes whitespace at the edges of a block, which the parser would otherwise trim. Detached subtrees are normalized too, though the root of a detached subtree is never removed or unwrapped, and a detached root that is attention keeps whitespace at its own edges.
    ///
    /// A reference is a name, and leads where the first definition of its label in document order does, so being normalized is a property of the whole document, not of each node: removing or relabelling a definition anywhere can leave references elsewhere unbound. A reference that no link reference definition in its tree binds, the document for an attached node and its own detached subtree for a detached one, reads back as text, so before anything above it becomes that text: the reference keeps its ID, as text of its opening bracket, `[`, or `![` for an image, followed by its children and then new text of `]`, `][]` or `][` with its label and `]` after, for a shortcut, collapsed or full reference, with the label's character escapes and references decoded and each run of whitespace in it that holds a line ending written as one space, as the text around it is then normalized with it. A footnote call that no footnote definition in its tree binds becomes text of `[^`, its label, or its identifier without one, decoded the same way, and `]`, with its ID. The root of a detached subtree stays, like any other node there. So `[b][x]` without a definition of `x` becomes text `[b][x]`, which the serializer writes `\[b]\[x]`. A tree may hold copies of a binding, later definitions or footnote definitions of a label or identifier, which a parsed, built or loaded document may have and no edit adds; they bind nothing and render nothing, so each is removed, before anything above, and each tree then defines each label once. Each place a copy leaves, or a definition hoisted out of one fills, is given the separators the parser puts there between the blocks then beside it, or at the end of the document, so that the output reads back as the tree, unless those already there could not be the parser's, as an edit leaves them; and an item above it keeps the spread it had, which is otherwise read from the source between its blocks, where the copy's lines would count as blank. A footnote definition binds its identifier when it is the first to end, so of one nested in another of its identifier, the nested one binds and the other is the copy. A definition or footnote definition in a footnote definition that is a copy, which is not a copy itself, binds its own label, so it takes the place of the copy, where it stays in document order, with what it holds, the copies in which go in turn. Everything else a copy holds goes with it, as removed nodes do. This is one pass from the root in document order, so the result depends on the tree alone, and it takes time linear in the copies, the containers above them and their children, and what the copies hold, however deeply copies nest in copies. The root of a detached subtree is never a copy, as it comes first in its tree, so a footnote definition detached from a copy of itself stays and loses the copy it holds. The HTML stays the same but for the five exceptions of [section 3.4 of the contract](crate::contract), such as footnote numbering when a copy holds calls (#225).
    ///
    /// Call this before serializing a parsed or edited tree to Markdown: the serializer assumes normalized input, and writes other trees without panicking, but possibly incorrectly or unstably. [`Document::parse`] does not normalize, because joining text across a backslash escape or character reference would copy it out of the source: parsed text is split there, adjacent attention of the same kind, such as `*a*_b_`, stays separate, and strikethrough inside strikethrough, such as `~a~~b~~c~`, stays nested. Serializing a parsed document without normalizing it usually gives the same output; [`crate::normalize_markdown`] normalizes before serializing.
    ///
    /// The IDs of removed text, HTML, code, breaks, attention, links, paragraphs, items, lists, tables, table heads and bodies, separators, and copies, with everything a copy holds other than the definitions and footnote definitions taken out of it, joined text, code and paragraphs, merged attention and unwrapped strikethrough and links no longer refer to nodes in the tree. Their arena slots remain, detached, but their contents are unspecified. Whitespace split off the edge of a text node is a new node with an empty span, and the text node keeps its ID and span with the rest of its value. The soft break between joined paragraphs is a new node with an empty span too, and the first paragraph keeps its ID, and so are the cells, heads, bodies and separators added to a table, and the space added before a task item's text. A table's rows and cells keep their IDs. An unbound reference keeps its ID and span as the text of its opening bracket, and an unbound footnote call as its text, unless joined into text before them, and the text after a reference's children is a new node with an empty span. Whitespace and breaks that move whole keep their IDs, and a code block given a final line ending keeps its ID and span. Other IDs stay valid.
    pub fn normalize(&mut self) {
        let mut scratch = Scratch::default();
        for index in 0..self.nodes.len() {
            if self.nodes[index].parent.is_some() {
                continue;
            }
            self.normalize_tree(NodeId::new(index), !self.settled, &mut scratch);
        }
        self.write_values();
        self.attached = attached_nodes(&self.nodes, self.root());
        // What normalizing leaves of a tree binds its references, and what it removes holds none.
        self.settled = true;
    }

    // Normalize the tree under `root`, making each reference in it that its tree does not bind
    // text when `unbind` holds.
    fn normalize_tree(&mut self, root: NodeId, unbind: bool, scratch: &mut Scratch) {
        let end = self.drop_copies(root, &mut scratch.copies);
        // What a tree binds is what it keeps of its definitions, the first of each key, so its
        // references are looked up once its copies are gone, and only in what it keeps.
        if unbind {
            self.unbind(root);
        }
        self.unwrap_links(root);
        self.remove_subtree(root, &mut scratch.blocks);
        self.normalize_subtree(root, &mut scratch.trailing);
        self.remove_empty_paragraphs(root, &mut scratch.blocks, &mut scratch.lists);
        self.join_tight_items(root, &mut scratch.lists);
        self.settle_list_tables(root, &mut scratch.lists);
        self.settle_tasks(root);
        // The last leaf is only known once the rules above have run.
        if let Some(parsed) = end {
            self.settle_final_separator(parsed);
        }
    }

    // Give code block values the final line ending they need where they are, and labels the shape that reads back wherever they are written, in every subtree, attached or detached, once its child lists are normalized.
    fn write_values(&mut self) {
        let source = self.source.as_ref();
        let (nodes, definitions) = (&mut self.nodes, &self.index);
        for index in 0..nodes.len() {
            if nodes[index].parent.is_some() {
                continue;
            }
            let root = NodeId::new(index);
            // Only a reference whose label is dropped looks up its binding, so the tree's
            // lookup is made at the first one.
            let mut lookup = None;
            let mut enclosing = Enclosing::default();
            let mut current = Some(root);
            while let Some(id) = current {
                normalize_label(
                    nodes,
                    definitions,
                    root,
                    id,
                    &mut lookup,
                    source,
                    enclosing.one_line(),
                );
                current = next_enclosed(nodes, root, id, &mut enclosing);
                let last = current.is_none() && index == 0;
                write_block_value(&mut nodes[id.index()].kind, source, last);
            }
        }
    }

    /// Whether [`Document::normalize`] would change nothing: no reference or footnote call, other than the root of a detached subtree, is unbound in its tree, which depends on the definitions anywhere in that tree; no tree holds two definitions, or two footnote definitions, of one label or identifier; no child list, attached or detached, has empty text, attention without children, adjacent text, adjacent inline code where the second has no language, adjacent attention of the same kind, HTML or code without a value in phrasing, or, outside phrasing, a list without items, not counting separators, or a table without a cell; no strikethrough is inside other strikethrough; no link is inside another link's text unless it is written as an autolink; no table with a cell has another shape than the one the parser gives a table; no attached emphasis, strong or strikethrough starts or ends with whitespace or a soft break, or ends with a hard break; no paragraph, heading or table cell starts or ends with a soft break, or ends with a hard break; no task item's checkbox is written before anything but a paragraph that starts with its separator and writes something after it; no label of a link, image, definition or footnote call has a line whose leading tabs start a block at some columns only, a tab where it is longer than the parser reads at some column, or a line ending in an ATX heading or table cell, and no shortcut or collapsed reference has a label longer than a full reference's can be at some column; no code block has a value that is not empty and has no final line ending, unless it is the last node of the document; no attached paragraph is empty; no loose list has only one item with fewer than two blocks; no item of a tight list holds a list that starts with an empty item, or an ordered list that starts at a number other than 1, directly after a paragraph; and no item of a tight list holds two blocks in a row, not counting separators, that can be written apart only with a blank line between them, as [`Document::normalize`] lists, or two paragraphs in a row. This takes time linear in the size of the arena; see [the contract](crate::contract), section 3.2.
    pub fn is_normalized(&self) -> bool {
        let cx = self.context();
        let source = self.source.as_ref();
        let nodes = &self.nodes;
        (0..nodes.len())
            .filter(|&index| nodes[index].parent.is_none())
            .all(|index| {
                let root = NodeId::new(index);
                // Each label is defined once.
                if root == self.root() {
                    if self.index.has_copies() {
                        return false;
                    }
                } else if !crate::bindings::Table::copies(source, nodes, root).is_empty() {
                    return false;
                }
                // Made at the first reference or footnote call, as a tree may have none.
                let mut lookup = None;
                let mut scratch = crate::bindings::Scratch::default();
                let mut enclosing = Enclosing::default();
                let mut current = Some(root);
                // The paragraph of the task item last met, as in `normalize_subtree`.
                let mut task = None;
                while let Some(id) = current {
                    let node = &nodes[id.index()];
                    if let EventKind::Start(Tag::Item {
                        checked: Some(_), ..
                    }) = node.kind
                    {
                        match task_paragraph(nodes, id) {
                            Some(paragraph)
                                if !unwritten_task(nodes, paragraph, source)
                                    && separated_task(nodes, paragraph, source) =>
                            {
                                task = Some(paragraph);
                            }
                            _ => return false,
                        }
                    }
                    if (!self.settled
                        && id != root
                        && names(&node.kind)
                        && lookup
                            .get_or_insert_with(|| crate::bindings::Lookup::new(cx, root))
                            .unbound(cx, &node.kind, &mut scratch))
                        || unwrapped(&self.syntax, source, nodes, id, enclosing)
                        || !label_normalized(&node.kind, source, enclosing.one_line())
                    {
                        return false;
                    }
                    if let EventKind::CodeBlock { value, .. } = &node.kind {
                        let mut after = enclosing;
                        let last =
                            index == 0 && next_enclosed(nodes, root, id, &mut after).is_none();
                        let value = value.resolve(source).expect("document text is validated");
                        if unwritten_block_value(value, last) {
                            return false;
                        }
                    }
                    let phrasing = phrasing_container(&node.kind);
                    let mut kept: Option<&EventKind<'_>> = None;
                    let mut last = None;
                    let mut child = node.first_child;
                    while let Some(id) = child {
                        let node = &nodes[id.index()];
                        if removed(node, phrasing)
                            || joined(kept, &node.kind)
                            || empty_paragraph(node)
                            || (!phrasing && unwritable_block(nodes, node))
                        {
                            return false;
                        }
                        if !matches!(node.kind, EventKind::Separator { .. }) {
                            kept = Some(&node.kind);
                        }
                        last = Some(&node.kind);
                        child = node.next_sibling;
                    }
                    if (matches!(node.kind, EventKind::Start(Tag::Table))
                        && !unwritable_block(nodes, node)
                        && !table_shaped(nodes, node))
                        || reads_back_tight(nodes, node)
                        || empty_item_after_paragraph(nodes, node)
                        || misnumbered_after_paragraph(nodes, node)
                        || loosens(self, id)
                        || adjacent_paragraphs(nodes, node)
                    {
                        return false;
                    }
                    if let (Some(first), Some(last)) = (node.first_child, last) {
                        if !edges_normalized(
                            node,
                            &nodes[first.index()].kind,
                            last,
                            task == Some(id),
                            source,
                        ) {
                            return false;
                        }
                    }
                    current = next_enclosed(nodes, root, id, &mut enclosing);
                }
                true
            })
    }

    // Remove the children that are dropped without joining from each child list, from the bottom up, so that attention whose children are all removed is removed in turn, and the blocks that Markdown cannot write from each child list that does not hold phrasing, each with one separator beside it, so that an item left empty is empty before the rules that look at items run. No later rule makes a list empty without removing it, or makes HTML blank, so nothing removed here is left to remove again. Each removed node has no children left, so it stays detached and normalized. `kept` is scratch storage for the children kept so far in a list.
    fn remove_subtree(&mut self, root: NodeId, kept: &mut Vec<NodeId>) {
        let mut id = first_leaf(&self.nodes, root);
        loop {
            if self.nodes[id.index()].first_child.is_some() {
                self.remove_children(id);
                if !phrasing_container(&self.nodes[id.index()].kind) {
                    self.remove_empty_children(id, kept, unwritable_block);
                }
            }
            let node = &self.nodes[id.index()];
            if matches!(node.kind, EventKind::Start(Tag::Table))
                && !unwritable_block(&self.nodes, node)
                && !table_shaped(&self.nodes, node)
            {
                self.shape_table(id, kept);
            }
            if id == root {
                return;
            }
            let node = &self.nodes[id.index()];
            id = match node.next_sibling {
                Some(next) => first_leaf(&self.nodes, next),
                None => node
                    .parent
                    .expect("a descendant has a parent before its subtree root"),
            };
        }
    }

    // Give a table with a cell the shape the parser gives one: its rows, in order through its heads and bodies, become a head of the first row and a body of the others, reusing the first head and body, and each row is given empty cells up to the widest row, each cell marked as a header cell in the head and aligned as the head cell of its column, which an empty cell added to the head is not. Separators in the table are replaced with the ones the parser gives a table. `rows` is scratch storage.
    fn shape_table(&mut self, table: NodeId, rows: &mut Vec<NodeId>) {
        let nodes = &mut self.nodes;
        rows.clear();
        rows.extend(table_rows(nodes, &nodes[table.index()]));
        let mut head = None;
        let mut body = None;
        let mut group = nodes[table.index()].first_child.take();
        while let Some(id) = group {
            group = nodes[id.index()].next_sibling.take();
            nodes[id.index()].parent = None;
            match nodes[id.index()].kind {
                EventKind::Start(Tag::TableHead) if head.is_none() => head = Some(id),
                EventKind::Start(Tag::TableBody) if body.is_none() => body = Some(id),
                _ => {}
            }
            let mut child = nodes[id.index()].first_child.take();
            while let Some(id) = child {
                child = nodes[id.index()].next_sibling.take();
                nodes[id.index()].parent = None;
            }
        }
        let width = rows
            .iter()
            .map(|&row| blocks(nodes, nodes[row.index()].first_child).count())
            .max()
            .expect("a table with a cell has a row");
        let mut columns = Vec::with_capacity(width);
        for (index, &row) in rows.iter().enumerate() {
            let header = index == 0;
            let mut count = 0;
            let mut last = None;
            let mut cell = nodes[row.index()].first_child;
            while let Some(id) = cell {
                cell = nodes[id.index()].next_sibling;
                let EventKind::Start(Tag::TableCell {
                    alignment,
                    header: flag,
                }) = &mut nodes[id.index()].kind
                else {
                    unreachable!("a row holds only cells");
                };
                if header {
                    columns.push(*alignment);
                }
                *alignment = columns[count];
                *flag = header;
                count += 1;
                last = Some(id);
            }
            for column in count..width {
                if header {
                    columns.push(crate::AlignKind::None);
                }
                let alignment = columns[column];
                let id = push_node(
                    nodes,
                    EventKind::Start(Tag::TableCell { alignment, header }),
                    row,
                );
                match last {
                    Some(last) => nodes[last.index()].next_sibling = Some(id),
                    None => nodes[row.index()].first_child = Some(id),
                }
                last = Some(id);
            }
        }
        // Link the head and body, with the separators the parser puts between rows and after the head, so they render alike.
        let line = || EventKind::Separator {
            value: Text::Literal("\n"),
            conditional: false,
        };
        let head =
            head.unwrap_or_else(|| push_node(nodes, EventKind::Start(Tag::TableHead), table));
        nodes[head.index()].parent = Some(table);
        nodes[table.index()].first_child = Some(head);
        let (&first, rest) = rows.split_first().expect("a table with a cell has a row");
        let after = push_node(nodes, line(), head);
        nodes[head.index()].first_child = Some(first);
        nodes[first.index()].parent = Some(head);
        nodes[first.index()].next_sibling = Some(after);
        let Some((&next, _)) = rest.split_first() else {
            return;
        };
        let between = push_node(nodes, line(), table);
        let body =
            body.unwrap_or_else(|| push_node(nodes, EventKind::Start(Tag::TableBody), table));
        nodes[body.index()].parent = Some(table);
        nodes[head.index()].next_sibling = Some(between);
        nodes[between.index()].next_sibling = Some(body);
        nodes[body.index()].first_child = Some(next);
        for pair in rest.windows(2) {
            let separator = push_node(nodes, line(), body);
            nodes[pair[0].index()].next_sibling = Some(separator);
            nodes[separator.index()].next_sibling = Some(pair[1]);
        }
        for &row in rest {
            nodes[row.index()].parent = Some(body);
        }
    }

    // Remove each paragraph without children, which is left once its phrasing is removed or its breaks are, from the top down, as removing one empties nothing else. A loose list that reads back tight once they are gone is made tight first. `kept` is scratch storage for the children kept so far in a list.
    fn remove_empty_paragraphs(
        &mut self,
        root: NodeId,
        kept: &mut Vec<NodeId>,
        lists: &mut Vec<NodeId>,
    ) {
        let mut current = Some(root);
        while let Some(id) = current {
            if reads_back_tight(&self.nodes, &self.nodes[id.index()]) {
                self.set_tight(id, true, lists);
            }
            if self.nodes[id.index()].first_child.is_some() {
                self.remove_empty_children(id, kept, |_, node| empty_paragraph(node));
                if empty_item_after_paragraph(&self.nodes, &self.nodes[id.index()]) {
                    self.remove_empty_items_after_paragraphs(id, kept);
                }
                if misnumbered_after_paragraph(&self.nodes, &self.nodes[id.index()]) {
                    self.renumber_lists_after_paragraphs(id);
                }
            }
            current = next_descendant(&self.nodes, root, id);
        }
    }

    // Once the rules above have run, make each task item whose checkbox cannot be written an ordinary item: one whose first block other than definitions is not a paragraph, or whose paragraph would write nothing after the checkbox. Give each other task item's paragraph what separates its checkbox from its text, a space, when it does not start with one, as the serializer writes one there and the parser reads it back as text.
    fn settle_tasks(&mut self, root: NodeId) {
        let source = self.source.as_ref();
        let nodes = &mut self.nodes;
        let mut current = Some(root);
        while let Some(id) = current {
            current = next_descendant(nodes, root, id);
            if !matches!(
                nodes[id.index()].kind,
                EventKind::Start(Tag::Item {
                    checked: Some(_),
                    ..
                })
            ) {
                continue;
            }
            let Some(paragraph) = task_paragraph(nodes, id)
                .filter(|&paragraph| !unwritten_task(nodes, paragraph, source))
            else {
                clear_task(nodes, id);
                continue;
            };
            if separated_task(nodes, paragraph, source) {
                continue;
            }
            let first = nodes[paragraph.index()]
                .first_child
                .expect("a written task paragraph has children");
            if let EventKind::Text(text) = &mut nodes[first.index()].kind {
                text.to_mut(source)
                    .expect("document text is validated")
                    .insert(0, ' ');
            } else {
                let space = push_node(nodes, EventKind::Text(Text::Literal(" ")), paragraph);
                nodes[space.index()].next_sibling = Some(first);
                nodes[paragraph.index()].first_child = Some(space);
            }
        }
    }

    // Once the rules above have run, make each tight list loose that has an item with two blocks in a row, not counting separators, that can be written apart only in a loose list, and in each other tight list join each run of paragraphs in a row in an item, not counting separators, into its first paragraph, with a soft break between each. Removing empty paragraphs and lists can leave paragraphs in a row, and a block quote that ends in a list in a paragraph only once those rules have run in it, so this is a pass of its own. Making a list loose and joining paragraphs change no block before or after them, so a list visited later is decided the same.
    fn join_tight_items(&mut self, root: NodeId, lists: &mut Vec<NodeId>) {
        let mut current = Some(root);
        while let Some(id) = current {
            if matches!(
                self.nodes[id.index()].kind,
                EventKind::Start(Tag::List { tight: true, .. })
            ) {
                let items = || {
                    core::iter::successors(self.nodes[id.index()].first_child, |id| {
                        self.nodes[id.index()].next_sibling
                    })
                };
                if items().any(|item| loosens(self, item)) {
                    self.set_tight(id, false, lists);
                } else {
                    let mut item = self.nodes[id.index()].first_child;
                    while let Some(item_id) = item {
                        item = self.nodes[item_id.index()].next_sibling;
                        if adjacent_paragraphs(&self.nodes, &self.nodes[item_id.index()]) {
                            self.join_paragraphs(item_id);
                        }
                    }
                }
            }
            current = next_descendant(&self.nodes, root, id);
        }
    }

    // In the item `item` of a tight list, without empty paragraphs, join each paragraph that follows another, not counting separators, into it: a soft break and its children are appended to the other's children, and it and the separators between them are detached. Each paragraph's children are walked once, when it starts a run or is joined into one.
    fn join_paragraphs(&mut self, item: NodeId) {
        let nodes = &mut self.nodes;
        // The paragraph that the next one joins, and its last child.
        let mut open: Option<(NodeId, NodeId)> = None;
        let mut current = nodes[item.index()].first_child;
        while let Some(id) = current {
            current = nodes[id.index()].next_sibling;
            match nodes[id.index()].kind {
                EventKind::Separator { .. } => continue,
                EventKind::Start(Tag::Paragraph { .. }) => {}
                _ => {
                    open = None;
                    continue;
                }
            }
            let Some((paragraph, last)) = open else {
                open = Some((id, last_child(nodes, id)));
                continue;
            };
            let mut between = nodes[paragraph.index()].next_sibling;
            while let Some(separator) = between {
                between = nodes[separator.index()].next_sibling.take();
                if separator == id {
                    break;
                }
                nodes[separator.index()].parent = None;
            }
            nodes[paragraph.index()].next_sibling = current;
            nodes[id.index()].parent = None;
            let soft_break = push_node(nodes, EventKind::SoftBreak(Text::Literal("\n")), paragraph);
            nodes[last.index()].next_sibling = Some(soft_break);
            let mut child = nodes[id.index()].first_child.take();
            nodes[soft_break.index()].next_sibling = child;
            let mut last = soft_break;
            while let Some(child_id) = child {
                nodes[child_id.index()].parent = Some(paragraph);
                last = child_id;
                child = nodes[child_id.index()].next_sibling;
            }
            open = Some((paragraph, last));
        }
        self.refresh_item(Some(item));
    }

    // In the item `item` of a tight list, whose empty paragraphs are removed, remove the leading empty items of each list that directly follows a paragraph, not counting separators, as each next item becomes the first. A list left with no item is removed with one separator beside it, the one after it if there is one, so a list after it follows the paragraph in turn. `kept` is scratch storage for the children of `item` kept so far.
    fn remove_empty_items_after_paragraphs(&mut self, item: NodeId, kept: &mut Vec<NodeId>) {
        kept.clear();
        let mut paragraph = false;
        let mut current = self.nodes[item.index()].first_child;
        while let Some(id) = current {
            current = self.nodes[id.index()].next_sibling;
            let node = &self.nodes[id.index()];
            if matches!(node.kind, EventKind::Separator { .. }) {
                kept.push(id);
                continue;
            }
            if !(paragraph && starts_empty(&self.nodes, node)) {
                paragraph = matches!(node.kind, EventKind::Start(Tag::Paragraph { .. }));
                kept.push(id);
                continue;
            }
            self.remove_leading_empty_items(id);
            let nodes = &mut self.nodes;
            if nodes[id.index()].first_child.is_some() {
                paragraph = false;
                kept.push(id);
                continue;
            }
            nodes[id.index()].parent = None;
            nodes[id.index()].next_sibling = None;
            let removed = if separator(nodes, current) {
                current
            } else if separator(nodes, kept.last().copied()) {
                kept.pop()
            } else {
                None
            };
            if let Some(removed) = removed {
                if Some(removed) == current {
                    current = nodes[removed.index()].next_sibling;
                }
                nodes[removed.index()].parent = None;
                nodes[removed.index()].next_sibling = None;
            }
            let last = *kept.last().expect("a paragraph comes before the list");
            nodes[last.index()].next_sibling = current;
        }
        self.refresh_item(Some(item));
    }

    // In the item `item` of a tight list, whose empty paragraphs and leading empty items of lists after paragraphs are removed, make each ordered list that directly follows a paragraph, not counting separators, start at 1, the only number at which it can interrupt the paragraph. Its nodes and text stay; only its numbering changes.
    fn renumber_lists_after_paragraphs(&mut self, item: NodeId) {
        let mut paragraph = false;
        let mut current = self.nodes[item.index()].first_child;
        while let Some(id) = current {
            let node = &mut self.nodes[id.index()];
            current = node.next_sibling;
            match &mut node.kind {
                EventKind::Separator { .. } => continue,
                EventKind::Start(Tag::List {
                    start: Some(start), ..
                }) if paragraph => *start = 1,
                _ => {}
            }
            paragraph = matches!(node.kind, EventKind::Start(Tag::Paragraph { .. }));
        }
    }

    // Remove the leading empty items of `list`, each with the separator after it if there is one, and any separators before the first, so that the list starts with an item that is not empty, or is left with no children. The empty paragraphs and separators of a removed item are detached from it, so each removed node has no children.
    fn remove_leading_empty_items(&mut self, list: NodeId) {
        let nodes = &mut self.nodes;
        let mut current = nodes[list.index()].first_child;
        while let Some(id) = current {
            let node = &nodes[id.index()];
            if !matches!(node.kind, EventKind::Separator { .. }) && !empty_item(nodes, node) {
                break;
            }
            current = nodes[id.index()].next_sibling;
            let mut child = nodes[id.index()].first_child.take();
            while let Some(child_id) = child {
                child = nodes[child_id.index()].next_sibling.take();
                nodes[child_id.index()].parent = None;
            }
            nodes[id.index()].parent = None;
            nodes[id.index()].next_sibling = None;
        }
        nodes[list.index()].first_child = current;
    }

    // Make `list` tight or loose, with the paragraphs of its items, which are its only paragraphs that render differently, and push it to `lists`. A separator after a paragraph made tight is removed, as the parser puts none there, and one is added after a paragraph made loose that a block follows, as the parser puts one there.
    fn set_tight(&mut self, list: NodeId, tight: bool, lists: &mut Vec<NodeId>) {
        let EventKind::Start(Tag::List { start, .. }) = self.nodes[list.index()].kind else {
            unreachable!("only a list is made tight or loose");
        };
        self.nodes[list.index()].kind = EventKind::Start(Tag::List { start, tight });
        lists.push(list);
        let mut item = self.nodes[list.index()].first_child;
        while let Some(item_id) = item {
            let mut child = self.nodes[item_id.index()].first_child;
            while let Some(id) = child {
                if matches!(
                    self.nodes[id.index()].kind,
                    EventKind::Start(Tag::Paragraph { .. })
                ) {
                    self.nodes[id.index()].kind = EventKind::Start(Tag::Paragraph { tight });
                    // The parser puts no separator after a tight paragraph, so one after a
                    // paragraph made tight goes, as it would render a line ending of its own, and
                    // one after a loose paragraph that a block follows.
                    let next = self.nodes[id.index()].next_sibling;
                    if tight && separator(&self.nodes, next) {
                        let gone = next.expect("a separator follows");
                        self.nodes[id.index()].next_sibling =
                            self.nodes[gone.index()].next_sibling.take();
                        self.nodes[gone.index()].parent = None;
                    } else if !tight && next.is_some() && !separator(&self.nodes, next) {
                        let added = self.new_separator(item_id, "\n", false);
                        self.nodes[added.index()].next_sibling = next;
                        self.nodes[id.index()].next_sibling = Some(added);
                    }
                }
                child = self.nodes[id.index()].next_sibling;
            }
            self.refresh_item(Some(item_id));
            item = self.nodes[item_id.index()].next_sibling;
        }
    }

    // Unlink each child of `parent` for which `unwritable` holds, with one separator next to it, the one after it if there is one, so that the blocks on either side stay separated once. The children of a removed child, which are separators of a list without items, are unlinked from it too.
    fn remove_empty_children(
        &mut self,
        parent: NodeId,
        kept: &mut Vec<NodeId>,
        unwritable: fn(&[Node], &Node) -> bool,
    ) {
        let nodes = &mut self.nodes;
        kept.clear();
        let mut changed = false;
        let mut current = nodes[parent.index()].first_child;
        while let Some(id) = current {
            current = nodes[id.index()].next_sibling.take();
            if !unwritable(nodes, &nodes[id.index()]) {
                nodes[id.index()].next_sibling = current;
                kept.push(id);
                continue;
            }
            changed = true;
            nodes[id.index()].parent = None;
            let mut child = nodes[id.index()].first_child.take();
            while let Some(child_id) = child {
                child = nodes[child_id.index()].next_sibling.take();
                nodes[child_id.index()].parent = None;
            }
            let removed = if separator(nodes, current) {
                current
            } else if separator(nodes, kept.last().copied()) {
                kept.pop()
            } else {
                None
            };
            if let Some(removed) = removed {
                if Some(removed) == current {
                    current = nodes[removed.index()].next_sibling;
                }
                nodes[removed.index()].parent = None;
                nodes[removed.index()].next_sibling = None;
            }
            match kept.last() {
                Some(&last) => nodes[last.index()].next_sibling = current,
                None => nodes[parent.index()].first_child = current,
            }
        }
        if changed {
            self.refresh_item(Some(parent));
        }
    }

    // Unlink each child of `parent` that is removed without joining.
    fn remove_children(&mut self, parent: NodeId) {
        let nodes = &mut self.nodes;
        let phrasing = phrasing_container(&nodes[parent.index()].kind);
        let mut changed = false;
        let mut linked: Option<NodeId> = None;
        let mut current = nodes[parent.index()].first_child;
        while let Some(id) = current {
            current = nodes[id.index()].next_sibling;
            if !removed(&nodes[id.index()], phrasing) {
                linked = Some(id);
                continue;
            }
            changed = true;
            match linked {
                Some(linked) => nodes[linked.index()].next_sibling = current,
                None => nodes[parent.index()].first_child = current,
            }
            nodes[id.index()].parent = None;
            nodes[id.index()].next_sibling = None;
        }
        if changed {
            self.refresh_item(Some(parent));
        }
    }

    // Unwrap each link inside another link that is not written as an autolink, from the top down, so that its children take its place and are checked in turn, before anything else is normalized, so that what it leaves, such as whitespace at the edge of attention or attention left empty, is normalized too.
    fn unwrap_links(&mut self, root: NodeId) {
        let mut enclosing = Enclosing::default();
        let mut current = Some(root);
        while let Some(id) = current {
            let mut inner = enclosing;
            inner.enter(&self.nodes[id.index()].kind);
            if inner.links > 0 {
                let mut linked: Option<NodeId> = None;
                let mut child = self.nodes[id.index()].first_child;
                while let Some(link) = child {
                    child = self.nodes[link.index()].next_sibling;
                    if !unwrapped(&self.syntax, self.source.as_ref(), &self.nodes, link, inner) {
                        linked = Some(link);
                        continue;
                    }
                    let nodes = &mut self.nodes;
                    nodes[link.index()].parent = None;
                    nodes[link.index()].next_sibling = None;
                    // Link its children in its place, and continue from the first of them.
                    let first = nodes[link.index()].first_child.take();
                    let mut last = first;
                    while let Some(node) = last {
                        nodes[node.index()].parent = Some(id);
                        last = nodes[node.index()].next_sibling;
                        if last.is_none() {
                            nodes[node.index()].next_sibling = child;
                        }
                    }
                    if first.is_some() {
                        child = first;
                    }
                    match linked {
                        Some(linked) => nodes[linked.index()].next_sibling = child,
                        None => nodes[id.index()].first_child = child,
                    }
                }
            }
            current = next_enclosed(&self.nodes, root, id, &mut enclosing);
        }
    }

    // Replace each reference and footnote call in the tree under `root`, other than `root` itself,
    // that no definition in that tree binds with the text it reads back as: a reference keeps its
    // ID, as text of its opening bracket, followed by its children and then new text of what
    // follows them, and a footnote call becomes text, with its ID too. One pass finds them and
    // another replaces, so this takes time linear in the size of the tree.
    fn unbind(&mut self, root: NodeId) {
        let cx = self.context();
        let nodes = &self.nodes;
        // Made at the first reference or footnote call, as a tree may have none.
        let mut lookup = None;
        let mut scratch = crate::bindings::Scratch::default();
        let mut unbound = Vec::new();
        let mut current = next_descendant(nodes, root, root);
        while let Some(id) = current {
            let kind = &nodes[id.index()].kind;
            if names(kind)
                && lookup
                    .get_or_insert_with(|| crate::bindings::Lookup::new(cx, root))
                    .unbound(cx, kind, &mut scratch)
            {
                unbound.push(id);
            }
            current = next_descendant(nodes, root, id);
        }
        for id in unbound {
            self.unbind_node(id);
        }
    }

    // Remove each definition and footnote definition of the tree under `root` that is a copy, one
    // after the first of its label or identifier there, which binds nothing and renders nothing.
    // Each place a copy goes from, or a definition moves into, is given the separators the
    // parser puts there, so that the output reads back as the tree, and the end of the document
    // once normalizing is done, which can add or remove a line ending in the HTML (exception 5
    // of `normalize`). A definition or footnote definition in a footnote definition that is a copy,
    // which is not a copy itself, binds, so it takes the place of the copy, in document order,
    // with what it holds. An item above a copy keeps the spread it has, which is inferred from
    // the source between its blocks, where a removed copy's lines would read as blank.
    //
    // One pass from the root, in document order, rebuilds the child list of each container that
    // holds a copy, once, and of each container of a copy that a definition leaves, so the result
    // depends on the tree alone. The copies are found in the index or one walk of the tree, and
    // what binds in each copy in a walk of the copy, which then dismantles what is left of it, so
    // this takes time linear in the copies, the containers above them and their children, and
    // what the copies hold.
    fn drop_copies(&mut self, root: NodeId, scratch: &mut Copies) -> Option<bool> {
        let attached = root == self.root();
        // Every copy is removed below, so the index can lose them all at once.
        let copies = if attached {
            self.index.take_copies()
        } else {
            crate::bindings::Table::copies(self.source.as_ref(), &self.nodes, root)
        };
        if copies.is_empty() {
            return None;
        }
        scratch.reserve(self.nodes.len());
        let Copies {
            copy,
            // The containers that hold a copy at some depth.
            above,
            passed,
            touched,
            dismantled,
        } = &mut *scratch;
        for &id in &copies {
            copy[id.index()] = true;
            touched.push(id.index());
            let mut at = self.nodes[id.index()].parent;
            while let Some(parent) = at {
                if core::mem::replace(&mut above[parent.index()], true) {
                    break;
                }
                touched.push(parent.index());
                at = self.nodes[parent.index()].parent;
            }
        }
        debug_assert!(!copy[root.index()], "a tree's root is the first of its key");
        let mut kept = Vec::new();
        let mut moved = Vec::new();
        let mut marks = Vec::new();
        let mut settled = Vec::new();
        let mut tails = Vec::new();
        let mut pending = vec![root];
        // The container whose last block ends the document, through block quotes, lists and
        // items, as the parser picks the separator that ends it, and whether a copy went from
        // the end of one of them.
        let mut end_chain = attached.then_some(root);
        let mut ends_changed = false;
        // Whether the separators at the end of the document could be the parser's.
        let mut end_parsed = true;
        while let Some(parent) = pending.pop() {
            kept.clear();
            // The item's blocks change below, so its spread is pinned first, as its descendants
            // are only changed after it.
            if matches!(
                self.nodes[parent.index()].kind,
                EventKind::Start(Tag::Item { .. })
            ) && !self.syntax.contains_key(&parent)
            {
                let spread = self.inferred_spread(parent);
                self.syntax
                    .insert(parent, Some(NodeSyntax::ListItem { spread }));
            }
            // Whether a copy went or a node moved in just before each node kept, and after the
            // last.
            marks.clear();
            let mut changed = false;
            // Whether the last node was a copy.
            let mut after = false;
            let mut current = self.nodes[parent.index()].first_child;
            while let Some(id) = current {
                current = self.nodes[id.index()].next_sibling.take();
                if !copy[id.index()] {
                    let after_copy = after && separator(&self.nodes, Some(id));
                    kept.push(id);
                    marks.push(
                        if core::mem::take(&mut changed) {
                            CHANGED
                        } else {
                            0
                        } | if after_copy { AFTER_COPY } else { 0 },
                    );
                    after = false;
                    continue;
                }
                changed = true;
                after = true;
                self.nodes[id.index()].parent = None;
                // What a copy holds that binds stays, and the rest of it goes.
                self.bound_in_copy(id, copy, &mut moved);
                self.dismantle(id, dismantled);
                for &inner in &moved {
                    self.nodes[inner.index()].parent = Some(parent);
                    kept.push(inner);
                    marks.push(CHANGED | MOVED);
                }
                after &= moved.is_empty();
                moved.clear();
            }
            // The arena grows only below, so the marks cover every node kept.
            pending.extend(kept.iter().rev().filter(|id| above[id.index()]));
            let document_end = Some(parent) == end_chain;
            let (end, parsed) =
                self.settle_separators(parent, &mut kept, &marks, changed, &mut settled);
            core::mem::swap(&mut kept, &mut settled);
            if parent == root && attached {
                end_parsed = parsed;
            }
            if document_end {
                ends_changed |= end;
                end_chain = kept
                    .iter()
                    .rev()
                    .find(|&&id| !separator(&self.nodes, Some(id)))
                    .copied()
                    .filter(|&id| {
                        matches!(
                            self.nodes[id.index()].kind,
                            EventKind::Start(Tag::BlockQuote | Tag::List { .. } | Tag::Item { .. })
                        )
                    });
            }
            // No rendering hint changes: a paragraph's context is its parent and theirs, which
            // stay for each node kept, and an item's ending is its last child other than a
            // definition, a footnote definition or a separator, which are all that go or move.
            self.set_children(parent, &kept);
            tails.extend_from_slice(&kept);
        }
        // What follows a list or footnote definition kept above may have changed, so the
        // separator after a table that ends it, in an item, is settled again, once the arena
        // the marks cover is no longer read.
        if passed.len() < self.nodes.len() {
            passed.resize(self.nodes.len(), false);
        }
        for &id in &tails {
            self.settle_table_tail(id, passed, touched);
        }
        scratch.clear();
        ends_changed.then_some(end_parsed)
    }

    // Give each place in `kept`, the children of `parent`, where a copy went or a node moved in
    // the separators the parser puts there, into `settled`, so that the written blocks read
    // back as they are. `marks` marks each node kept with `CHANGED` when such a place is just
    // before it, `AFTER_COPY` when it is a separator that came just after a copy and `MOVED`
    // when it moved in from a copy, and `tail` says whether a place is after the last. Between
    // two blocks, the parser puts one separator unless `parted` says otherwise; at the end of a
    // container, one only where `table_line_after` says. The separators there are left as they
    // are when the parser could not have given them, as only an edit leaves: two in one place,
    // or, except at the end of the document, one after a copy. The end of the document is
    // settled once normalizing is done, as it depends on its last leaf; this returns whether
    // what is after its last block changed or the block moved in, and whether the separators
    // after it could be the parser's.
    fn settle_separators(
        &mut self,
        parent: NodeId,
        kept: &mut Vec<NodeId>,
        marks: &[u8],
        tail: bool,
        settled: &mut Vec<NodeId>,
    ) -> (bool, bool) {
        settled.clear();
        let mut last = None;
        let mut last_moved = false;
        let mut from = 0;
        let mut touched = false;
        // Separators in the current place, and whether those since the last block could be
        // the parser's, but for one after a copy.
        let mut place = 0;
        let mut parsed = true;
        let mut after_copy = false;
        for (&id, &change) in kept.iter().zip(marks) {
            let changed = change & CHANGED != 0;
            touched |= changed;
            if separator(&self.nodes, Some(id)) {
                if changed {
                    place = 0;
                }
                place += 1;
                parsed &= place == 1;
                after_copy |= change & AFTER_COPY != 0;
                settled.push(id);
                continue;
            }
            if let Some(last) = last.filter(|_| touched && parsed && !after_copy) {
                let wanted = self.parted(last, id);
                self.settle(parent, settled, from, wanted);
            }
            last_moved = change & MOVED != 0;
            touched = false;
            place = 0;
            parsed = true;
            after_copy = false;
            settled.push(id);
            last = Some(id);
            from = settled.len();
        }
        let end = touched || tail;
        if parent != self.root() {
            if let Some(last) = last.filter(|_| end && parsed && !after_copy) {
                let wanted = self.table_line_after(parent, last);
                self.settle(parent, settled, from, wanted);
            }
        }
        kept.clear();
        (end || last_moved, parsed)
    }

    // Whether the parser puts a separator after `last`, the last block of `parent`: only after
    // a table in an item or footnote definition that a blank line follows in it, as the line is
    // after the table's span. The writer puts one there before the next item of a loose list,
    // and before a block after the footnote definition or the list, but in an item that
    // records no spread, or after an item or footnote definition that either ends, but not
    // after a block quote, as the line is outside it.
    fn table_line_after(&self, parent: NodeId, last: NodeId) -> bool {
        if !matches!(self.nodes[last.index()].kind, EventKind::Start(Tag::Table))
            || !matches!(
                self.nodes[parent.index()].kind,
                EventKind::Start(Tag::Item { .. } | Tag::FootnoteDefinition { .. })
            )
        {
            return false;
        }
        let mut at = parent;
        loop {
            let next = core::iter::successors(self.nodes[at.index()].next_sibling, |id| {
                self.nodes[id.index()].next_sibling
            })
            .find(|&id| !separator(&self.nodes, Some(id)));
            let container = self.nodes[at.index()].parent;
            if next.is_some() {
                return if matches!(
                    self.nodes[at.index()].kind,
                    EventKind::Start(Tag::Item { .. })
                ) {
                    container.is_some_and(|list| {
                        matches!(
                            self.nodes[list.index()].kind,
                            EventKind::Start(Tag::List { tight: false, .. })
                        )
                    })
                } else {
                    !container.is_some_and(|container| self.unspread_item(container))
                };
            }
            at = match container {
                Some(container)
                    if matches!(
                        self.nodes[container.index()].kind,
                        EventKind::Start(
                            Tag::List { .. } | Tag::Item { .. } | Tag::FootnoteDefinition { .. }
                        )
                    ) =>
                {
                    container
                }
                _ => return false,
            };
        }
    }

    // Whether `id` is an item that records no spread, whose blocks are written without a blank
    // line between them.
    fn unspread_item(&self, id: NodeId) -> bool {
        matches!(
            self.syntax(id),
            Some(NodeSyntax::ListItem { spread: false })
        )
    }

    // Give the table that ends `block`, a list or footnote definition, through the last items
    // and footnote definitions in it, the separator after it that `table_line_after` says the
    // parser puts there, unless what is after it could not be the parser's, two or more. Each
    // container gone through is marked in `passed`, its index pushed to `touched`, and the
    // search stops at one already passed, whose table is settled, so that the blocks of a
    // tree's containers are each searched once, however deeply they nest.
    fn settle_table_tail(&mut self, block: NodeId, passed: &mut [bool], touched: &mut Vec<usize>) {
        let mut at = block;
        loop {
            let kind = &self.nodes[at.index()].kind;
            if !matches!(
                kind,
                EventKind::Start(
                    Tag::List { .. } | Tag::Item { .. } | Tag::FootnoteDefinition { .. }
                )
            ) || (at == block && matches!(kind, EventKind::Start(Tag::Item { .. })))
            {
                return;
            }
            if core::mem::replace(&mut passed[at.index()], true) {
                return;
            }
            touched.push(at.index());
            let Some(last) = self.flow_children(at).last() else {
                return;
            };
            if !matches!(self.nodes[last.index()].kind, EventKind::Start(Tag::Table)) {
                at = last;
                continue;
            }
            self.settle_table_end(at, last);
            return;
        }
    }

    // Give `last`, the last block of `parent`, if it is a table, the separator after it that
    // `table_line_after` says the parser puts there, unless what is after it could not be the
    // parser's, two or more.
    fn settle_table_end(&mut self, parent: NodeId, last: NodeId) {
        let tail = self.nodes[last.index()].next_sibling;
        let wanted = self.table_line_after(parent, last);
        match tail {
            Some(one) if !wanted && self.nodes[one.index()].next_sibling.is_none() => {
                self.nodes[last.index()].next_sibling = None;
                self.nodes[one.index()].parent = None;
            }
            None if wanted => {
                let separator = self.new_separator(parent, "\n", false);
                self.nodes[last.index()].next_sibling = Some(separator);
            }
            _ => {}
        }
    }

    // Give each table under `root` that ends an item or footnote definition the separator after
    // it that `table_line_after` says the parser puts there, where one of `lists`, the lists made
    // tight or loose, decides whether a blank line follows it, and clear `lists`. Only the lists
    // that the table ends a block of, through last blocks, or that hold the item it then ends,
    // decide, so each container is passed once, from the one table below it that it ends with.
    fn settle_list_tables(&mut self, root: NodeId, lists: &mut Vec<NodeId>) {
        if lists.is_empty() {
            return;
        }
        lists.sort_unstable();
        let mut current = Some(root);
        while let Some(id) = current {
            current = next_descendant(&self.nodes, root, id);
            if !matches!(self.nodes[id.index()].kind, EventKind::Start(Tag::Table)) {
                continue;
            }
            let parent = self.nodes[id.index()].parent.filter(|parent| {
                matches!(
                    self.nodes[parent.index()].kind,
                    EventKind::Start(Tag::Item { .. } | Tag::FootnoteDefinition { .. })
                )
            });
            if let Some(parent) = parent {
                if !self.block_after(id) && self.ends_in_lists(parent, lists) {
                    self.settle_table_end(parent, id);
                }
            }
        }
        lists.clear();
    }

    // Whether a sibling other than a separator follows `id`.
    fn block_after(&self, id: NodeId) -> bool {
        core::iter::successors(self.nodes[id.index()].next_sibling, |id| {
            self.nodes[id.index()].next_sibling
        })
        .any(|id| !separator(&self.nodes, Some(id)))
    }

    // Whether one of `lists`, sorted, holds `block` or a block that `block` ends, through last
    // blocks of lists, items and footnote definitions, up to the first with a block after it.
    fn ends_in_lists(&self, block: NodeId, lists: &[NodeId]) -> bool {
        let mut at = block;
        while let Some(container) = self.nodes[at.index()].parent {
            if lists.binary_search(&container).is_ok() {
                return true;
            }
            if self.block_after(at)
                || !matches!(
                    self.nodes[container.index()].kind,
                    EventKind::Start(
                        Tag::List { .. } | Tag::Item { .. } | Tag::FootnoteDefinition { .. }
                    )
                )
            {
                break;
            }
            at = container;
        }
        false
    }

    // Leave one separator at the end of `settled`, after its index `from`, if `wanted`, and
    // none otherwise.
    fn settle(&mut self, parent: NodeId, settled: &mut Vec<NodeId>, from: usize, wanted: bool) {
        let keep = from + usize::from(wanted);
        if settled.len() < keep {
            let separator = self.new_separator(parent, "\n", false);
            settled.push(separator);
        }
        for gone in settled.drain(keep..) {
            self.nodes[gone.index()].parent = None;
            self.nodes[gone.index()].next_sibling = None;
        }
    }

    // Whether the parser puts a separator between the blocks `previous` and `next`: after every
    // block but a definition, a footnote definition, an item and a tight paragraph, and after a
    // list unless a definition follows.
    fn parted(&self, previous: NodeId, next: NodeId) -> bool {
        match self.nodes[previous.index()].kind {
            EventKind::Definition { .. }
            | EventKind::Start(
                Tag::FootnoteDefinition { .. } | Tag::Item { .. } | Tag::Paragraph { tight: true },
            ) => false,
            EventKind::Start(Tag::List { .. }) => {
                !matches!(self.nodes[next.index()].kind, EventKind::Definition { .. })
            }
            _ => true,
        }
    }

    // Give the document the separator the parser puts after its last block as it is written:
    // none after a definition, a line ending after a block whose last leaf, through block
    // quotes, lists and items, holds content, and otherwise one that only ends the output.
    // `parsed` says whether the separators there could be the parser's; if not, which only an
    // edit leaves, they stay.
    fn settle_final_separator(&mut self, parsed: bool) {
        let root = self.root();
        let mut last = None;
        let mut separators = Vec::new();
        for id in self.children(root) {
            if separator(&self.nodes, Some(id)) {
                separators.push(id);
            } else {
                last = Some(id);
                separators.clear();
            }
        }
        let last = last.expect("the binding of a copy removed stays");
        if !parsed {
            return;
        }
        let wanted = self.final_separator(last);
        let keep = usize::from(wanted.is_some()).min(separators.len());
        for &gone in &separators[keep..] {
            self.nodes[gone.index()].parent = None;
            self.nodes[gone.index()].next_sibling = None;
        }
        match (separators.first(), wanted) {
            (None, Some((value, conditional))) => {
                let separator = self.new_separator(root, value, conditional);
                self.nodes[last.index()].next_sibling = Some(separator);
            }
            (Some(_), None) => self.nodes[last.index()].next_sibling = None,
            (Some(&one), Some((value, conditional))) => {
                self.nodes[one.index()].next_sibling = None;
                let EventKind::Separator {
                    value: old,
                    conditional: was,
                } = &self.nodes[one.index()].kind
                else {
                    unreachable!("a separator was found");
                };
                let same = *was == conditional
                    && old
                        .resolve(self.source.as_ref())
                        .expect("document text is validated")
                        == value;
                if !same {
                    self.nodes[one.index()].kind = EventKind::Separator {
                        value: Text::Literal(value),
                        conditional,
                    };
                }
            }
            (None, None) => {}
        }
    }

    // The separator the parser puts after `last`, the last block of a document, as it is
    // written; see `settle_final_separator`.
    fn final_separator(&self, last: NodeId) -> Option<(&'static str, bool)> {
        if matches!(self.nodes[last.index()].kind, EventKind::Definition { .. }) {
            return None;
        }
        let mut leaf = last;
        while matches!(
            self.nodes[leaf.index()].kind,
            EventKind::Start(Tag::BlockQuote | Tag::List { .. } | Tag::Item { .. })
        ) {
            let Some(child) = self.flow_children(leaf).last() else {
                break;
            };
            leaf = child;
        }
        let populated = matches!(
            self.nodes[leaf.index()].kind,
            EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::Table)
                | EventKind::Rule
                | EventKind::CodeBlock { .. }
                | EventKind::Html { block: true, .. }
        );
        Some(if populated { ("\n", false) } else { ("", true) })
    }

    // A new separator of `value` in `parent`, to be linked by the caller.
    fn new_separator(&mut self, parent: NodeId, value: &'static str, conditional: bool) -> NodeId {
        push_node(
            &mut self.nodes,
            EventKind::Separator {
                value: Text::Literal(value),
                conditional,
            },
            parent,
        )
    }

    // Take out of the copy `copy` each definition and footnote definition in it that binds, the
    // outermost of each nesting, into `moved`, in document order: each that is not a copy, as
    // the copies in it go with it. The child list of each container it leaves is rebuilt once.
    fn bound_in_copy(&mut self, copy: NodeId, copies: &[bool], moved: &mut Vec<NodeId>) {
        let nodes = &self.nodes;
        if !crate::bindings::holds_blocks(&nodes[copy.index()].kind) {
            return;
        }
        let mut current = nodes[copy.index()].first_child;
        let mut holders = Vec::new();
        while let Some(id) = current {
            let kind = &nodes[id.index()].kind;
            let definition = matches!(
                kind,
                EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
            );
            if definition && !copies[id.index()] {
                let holder = nodes[id.index()]
                    .parent
                    .expect("a definition in a copy has a parent in it");
                if holders.last() != Some(&holder) {
                    holders.push(holder);
                }
                moved.push(id);
                current = crate::bindings::next_after(nodes, copy, id);
            } else {
                current = crate::bindings::next_block(nodes, copy, id);
            }
        }
        self.take_out_of(moved, holders);
    }

    // Take each of `moved` out of its parent, one of `holders`.
    fn take_out_of(&mut self, moved: &[NodeId], mut holders: Vec<NodeId>) {
        // A node that leaves is marked by having no parent until it is given its new one.
        for &id in moved {
            self.nodes[id.index()].parent = None;
        }
        holders.sort_unstable();
        holders.dedup();
        let mut children = Vec::new();
        for holder in holders {
            children.clear();
            let nodes = &self.nodes;
            children.extend(
                self.children(holder)
                    .filter(|id| nodes[id.index()].parent.is_some()),
            );
            self.set_children(holder, &children);
        }
    }

    // Dismantle `copy`, a copy removed from its tree, once what binds in it is taken out: each
    // node left in it becomes a detached root of empty text, which binds and refers to nothing,
    // is normalized and is in no index, as the copies in it already left the index. `stack` is
    // scratch storage, left empty.
    fn dismantle(&mut self, copy: NodeId, stack: &mut Vec<NodeId>) {
        stack.push(copy);
        while let Some(id) = stack.pop() {
            let node = &mut self.nodes[id.index()];
            let mut child = node.first_child.take();
            node.parent = None;
            node.next_sibling = None;
            node.kind = EventKind::Text(Text::Literal(""));
            while let Some(inner) = child {
                child = self.nodes[inner.index()].next_sibling;
                stack.push(inner);
            }
            self.syntax.remove(&id);
        }
    }

    // Make `children`, in order, the child list of `parent`.
    fn set_children(&mut self, parent: NodeId, children: &[NodeId]) {
        let mut next = None;
        for &id in children.iter().rev() {
            self.nodes[id.index()].next_sibling = next;
            next = Some(id);
        }
        self.nodes[parent.index()].first_child = next;
    }

    // Replace the reference or footnote call `id`, which has a parent, with the text it reads
    // back as, as `unbind` does.
    fn unbind_node(&mut self, id: NodeId) {
        let source = self.source.as_ref();
        let nodes = &mut self.nodes;
        let resolve = |value: &Text<'_>| -> String {
            String::from(value.resolve(source).expect("document text is validated"))
        };
        match &nodes[id.index()].kind {
            EventKind::FootnoteReference { identifier, label } => {
                let label = resolve(label.as_ref().unwrap_or(identifier));
                nodes[id.index()].kind =
                    EventKind::Text(Text::from(crate::bindings::unbound_footnote_text(&label)));
            }
            EventKind::Start(tag @ (Tag::Link(target) | Tag::Image(target))) => {
                let reference = target.reference().expect("only a reference is unbound");
                let (open, close) = crate::bindings::unbound_text(
                    reference.kind,
                    &resolve(&reference.label),
                    matches!(tag, Tag::Image(_)),
                );
                let parent = nodes[id.index()]
                    .parent
                    .expect("only the root has no parent");
                let close = push_node(nodes, EventKind::Text(Text::from(close)), parent);
                nodes[close.index()].next_sibling = nodes[id.index()].next_sibling;
                // Its children follow its opening bracket, and what follows them, them.
                let mut last = id;
                let mut child = nodes[id.index()].first_child.take();
                nodes[id.index()].next_sibling = child;
                while let Some(node) = child {
                    nodes[node.index()].parent = Some(parent);
                    last = node;
                    child = nodes[node.index()].next_sibling;
                }
                nodes[last.index()].next_sibling = Some(close);
                nodes[id.index()].kind = EventKind::Text(Text::Literal(open));
                self.syntax.remove(&id);
            }
            _ => unreachable!("only a reference or footnote call is unbound"),
        }
    }

    // Normalize each child list from the top down, so that children moved into attention are normalized with its own children, counting the strikethrough and table cells that enclose each node so that nested strikethrough is unwrapped without searching ancestors.
    fn normalize_subtree(&mut self, root: NodeId, scratch: &mut Trailing) {
        let mut enclosing = Enclosing::default();
        let mut current = Some(root);
        // The paragraph of the task item last met. It is the item's first child other than definitions and separators, so no other item is met before it.
        let mut task = None;
        while let Some(id) = current {
            if let Some(paragraph) = task_paragraph(&self.nodes, id) {
                task = Some(paragraph);
            }
            if self.nodes[id.index()].first_child.is_some() {
                let mut inner = enclosing;
                inner.enter(&self.nodes[id.index()].kind);
                self.normalize_children(id, inner, scratch, task == Some(id));
            }
            current = next_enclosed(&self.nodes, root, id, &mut enclosing);
        }
    }

    // Normalize the children of `parent`, enclosed by `enclosing` including itself, in one pass: unwrap each strikethrough child of strikethrough in place, so its children are normalized next; unlink each child joined into the last child kept as it is met, merging attention first; once no more attention merges into the last kept attention, move whitespace out of its end and of the attention nested there, and continue from what moved after it; and move whitespace out of the start of each attention child and of the attention nested there, and continue from what moved before it. Then remove the breaks at the edges of a paragraph, heading or table cell. Empty children have already been removed, and paragraphs emptied by removing breaks are removed afterwards.
    fn normalize_children(
        &mut self,
        parent: NodeId,
        enclosing: Enclosing,
        scratch: &mut Trailing,
        task: bool,
    ) {
        let source = self.source.as_ref();
        let nodes = &mut self.nodes;
        let enclosed = enclosing.deletes > 0;
        // Attached attention has had whitespace moved out of its edges, and out of the children at its edges, by the parent that holds it.
        let covered =
            attention(&nodes[parent.index()].kind) && nodes[parent.index()].parent.is_some();
        let mut changed = false;
        // The last kept child that is not a separator, and the last kept child.
        let mut kept: Option<NodeId> = None;
        let mut linked: Option<NodeId> = None;
        // Attention that children were moved into, and its last child, so that moving more children does not walk its children again.
        let mut tail: Option<(NodeId, NodeId)> = None;
        // The attention child whose leading edge was last moved out, so that it is not searched again when met after what moved before it.
        let mut extracted: Option<NodeId> = None;
        // The last kept child if it is attention, with the child before it, whose trailing edge is moved out once no more attention merges into it.
        let mut pending: Option<(NodeId, Option<NodeId>)> = None;
        let mut current = nodes[parent.index()].first_child;
        loop {
            let Some(id) = current else {
                // The trailing edges of the last child of covered attention have been moved out already.
                match pending.take() {
                    Some((attention_id, previous))
                        if !covered
                            && move_trailing(nodes, source, previous, attention_id, scratch) =>
                    {
                        changed = true;
                        (linked, current) = resume(nodes, parent, attention_id, previous);
                        continue;
                    }
                    _ => break,
                }
            };
            current = nodes[id.index()].next_sibling;
            if enclosed && delete(&nodes[id.index()].kind) {
                // Link the children of the strikethrough in its place, and continue from the first of them.
                changed = true;
                let first = nodes[id.index()]
                    .first_child
                    .take()
                    .expect("attention without children has been removed");
                let mut last = first;
                loop {
                    nodes[last.index()].parent = Some(parent);
                    match nodes[last.index()].next_sibling {
                        Some(next) => last = next,
                        None => break,
                    }
                }
                nodes[last.index()].next_sibling = current;
                match linked {
                    Some(linked) => nodes[linked.index()].next_sibling = Some(first),
                    None => nodes[parent.index()].first_child = Some(first),
                }
                nodes[id.index()].parent = None;
                nodes[id.index()].next_sibling = None;
                current = Some(first);
                continue;
            }
            if !joined(
                kept.map(|kept| &nodes[kept.index()].kind),
                &nodes[id.index()].kind,
            ) {
                // No more attention merges into the pending attention: move its trailing edge out, after it, and continue from what moved.
                if let Some((attention_id, previous)) = pending.take() {
                    if move_trailing(nodes, source, previous, attention_id, scratch) {
                        changed = true;
                        (linked, current) = resume(nodes, parent, attention_id, previous);
                        continue;
                    }
                }
                if attention(&nodes[id.index()].kind) {
                    // Move the leading edge out, before it, unless it is the first child of covered attention, and continue from what moved.
                    let mut previous = linked;
                    if extracted != Some(id)
                        && (!covered || linked.is_some())
                        && move_leading(nodes, source, &mut previous, id)
                    {
                        extracted = Some(id);
                        changed = true;
                        current = match linked {
                            Some(linked) => nodes[linked.index()].next_sibling,
                            None => nodes[parent.index()].first_child,
                        };
                        continue;
                    }
                    pending = Some((id, linked));
                }
                if !matches!(nodes[id.index()].kind, EventKind::Separator { .. }) {
                    kept = Some(id);
                }
                linked = Some(id);
                continue;
            }
            let kept = kept.expect("a joined child follows a kept child");
            let linked = linked.expect("a joined child follows a kept child");
            changed = true;
            nodes[linked.index()].next_sibling = current;
            nodes[id.index()].parent = None;
            nodes[id.index()].next_sibling = None;
            nodes[kept.index()].span =
                joined_span(&nodes[kept.index()].span, &nodes[id.index()].span);
            // Hold the removed node's value while joining it, so the kept node can be borrowed mutably.
            let removed = core::mem::replace(&mut nodes[id.index()].kind, EventKind::Rule);
            match (&mut nodes[kept.index()].kind, &removed) {
                (EventKind::Text(left), EventKind::Text(right))
                | (EventKind::Code { value: left, .. }, EventKind::Code { value: right, .. }) => {
                    join_text(left, right, source);
                }
                _ => {
                    // Move the children of the removed attention to the end of the kept attention.
                    let mut last = match tail {
                        Some((attention, last)) if attention == kept => last,
                        _ => core::iter::successors(nodes[kept.index()].first_child, |&child| {
                            nodes[child.index()].next_sibling
                        })
                        .last()
                        .expect("attention without children has been removed"),
                    };
                    let mut child = nodes[id.index()].first_child.take();
                    nodes[last.index()].next_sibling = child;
                    while let Some(moved) = child {
                        nodes[moved.index()].parent = Some(kept);
                        last = moved;
                        child = nodes[moved.index()].next_sibling;
                    }
                    tail = Some((kept, last));
                }
            }
            nodes[id.index()].kind = removed;
        }
        if block_phrasing(&nodes[parent.index()].kind) {
            changed |= trim_breaks(nodes, parent, source, task);
        }
        if changed {
            self.refresh_item(Some(parent));
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Document<String> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Stored {
            source: String,
            nodes: Vec<Node>,
            #[serde(default)]
            syntax: BTreeMap<NodeId, Option<NodeSyntax<'static>>>,
        }
        let stored = Stored::deserialize(deserializer)?;
        let mut document = Self {
            source: stored.source,
            nodes: stored.nodes,
            syntax: stored.syntax,
            append_hint: None,
            index: crate::bindings::Index::default(),
            settled: false,
            attached: Vec::new(),
        };
        document
            .validate_storage()
            .map_err(serde::de::Error::custom)?;
        document.attached = attached_nodes(&document.nodes, document.root());
        document.index =
            crate::bindings::Index::of_tree(&document.source, &document.nodes, document.root());
        Ok(document)
    }
}

#[cfg(feature = "serde")]
impl<S: AsRef<str>> Document<S> {
    // Validate the complete arena forest, including detached nodes, before public traversal can observe deserialized links. Each slot must be visited exactly once.
    fn validate_storage(&self) -> Result<(), StreamError> {
        if !matches!(
            self.nodes.first().map(|node| &node.kind),
            Some(EventKind::Start(Tag::Document { .. }))
        ) {
            return Err(StreamError::Unbalanced);
        }
        if self.nodes[0].parent.is_some() || self.nodes[0].next_sibling.is_some() {
            return Err(StreamError::Unbalanced);
        }
        let mut seen = alloc::vec![false;self.nodes.len()];
        let mut pending = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if node.parent.is_none() {
                if node.next_sibling.is_some() {
                    return Err(StreamError::Unbalanced);
                }
                pending.push((NodeId::new(index), None, crate::stream::Reach::default()));
            }
        }
        // Each pending node comes with its parent and the containers above it that restrict what
        // is below them.
        while let Some((id, parent, reach)) = pending.pop() {
            let node = self.node(id).ok_or(StreamError::InvalidRange)?;
            if seen[id.index()] || node.parent != parent {
                return Err(StreamError::Unbalanced);
            }
            seen[id.index()] = true;
            storable(&node.kind)?;
            Event {
                kind: borrow_kind(&node.kind),
                span: node.span.clone(),
            }
            .validate(Some(self.source()))?;
            let container = matches!(node.kind, EventKind::Start(_));
            if matches!(node.kind, EventKind::End(_))
                || (node.first_child.is_some() && !container)
                // The root is a document, checked above. Another detached document is a second
                // root; a nested one is a node that nothing holds, checked below.
                || (parent.is_none()
                    && id != self.root()
                    && matches!(node.kind, EventKind::Start(Tag::Document { .. })))
            {
                return Err(StreamError::Unbalanced);
            }
            if let Some(parent) = parent {
                let first = self.nodes[parent.index()].first_child;
                self.check_nesting(parent, first, &node.kind)?;
                if !reach.allows(&node.kind) {
                    return Err(StreamError::InvalidNesting);
                }
            }
            if let Some(next) = node.next_sibling {
                pending.push((next, parent, reach));
            }
            if let (Some(child), EventKind::Start(tag)) = (node.first_child, &node.kind) {
                let mut reach = reach;
                reach.enter(tag.kind());
                pending.push((child, Some(id), reach));
            }
        }
        if seen.iter().any(|visited| !*visited) {
            return Err(StreamError::Unbalanced);
        }
        for (&id, value) in &self.syntax {
            let node = self.node(id).ok_or(StreamError::InvalidRange)?;
            if let Some(value) = value {
                value.validate(&node.kind, self.source())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, NodeId};
    use crate::stream::{EventKind, StreamError, Tag, Text};
    use crate::ParseOptions;
    use alloc::borrow::ToOwned;

    // A rejected edit changes nothing at all, so a settled document stays settled, while an edit
    // that succeeds makes it unsettled.
    #[test]
    fn rejected_edits_keep_the_document_settled() {
        let mut document =
            Document::parse("[a]\n\n[a]: /1\n\n> q\n".to_owned(), &ParseOptions::gfm()).unwrap();
        let root = document.root();
        let paragraph = document.children(root).next().unwrap();
        let missing = NodeId::new(document.nodes.len());
        let definition = || EventKind::Definition {
            label: Text::Literal("a"),
            destination: Text::Literal("/2"),
            title: None,
        };
        assert!(document.settled);
        let rejected: [&dyn Fn(&mut Document) -> StreamError; 6] = [
            &|document| document.set_kind(paragraph, EventKind::Rule).unwrap_err(),
            &|document| document.append_child(root, definition()).unwrap_err(),
            &|document| document.detach(root).unwrap_err(),
            &|document| document.attach(root, root, None).unwrap_err(),
            &|document| document.extract_children(missing).unwrap_err(),
            &|document| {
                let replacement = document.clone();
                document
                    .replace_heading_section(missing, 0, &replacement, true, true)
                    .unwrap_err()
            },
        ];
        for edit in rejected {
            edit(&mut document);
            assert!(document.settled);
        }
        document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        assert!(!document.settled);
    }
}
