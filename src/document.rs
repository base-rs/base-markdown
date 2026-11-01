// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! A source-backed arena document and its incremental pull consumer.
use crate::stream::{Event, EventKind, StreamError, Text};
use crate::{
    stream::{Tag, TagKind},
    NodeSyntax,
};
use alloc::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::{num::NonZeroUsize, ops::Range};

/// A stable index into a document's contiguous node arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeId(NonZeroUsize);

impl NodeId {
    fn new(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("an arena index cannot reach usize::MAX"))
    }
    fn index(self) -> usize {
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
    // Only the active container needs a tail. Opening a child resets it; closing that child makes the child the parent's new tail.
    last_child: Option<NodeId>,
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
    /// Returns parser errors or [`StreamError::Unbalanced`] for malformed streams. A failed builder cannot be finished.
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
            if event.span.end < node.span.end {
                return Err(StreamError::InvalidRange);
            }
            node.span.end = event.span.end;
            self.last_child = Some(id);
            return Ok(());
        }
        if self.open.is_empty() {
            if !self.nodes.is_empty()
                || !matches!(&event.kind, EventKind::Start(tag) if tag.kind() == crate::stream::TagKind::Document)
            {
                return Err(StreamError::Unbalanced);
            }
        } else if matches!(&event.kind, EventKind::Start(tag) if tag.kind() == crate::stream::TagKind::Document)
        {
            return Err(StreamError::Unbalanced);
        }
        if let EventKind::Start(tag) = &event.kind {
            if matches!(tag.kind(), crate::stream::TagKind::Heading | crate::stream::TagKind::FootnoteDefinition) && self.open.iter().any(|id| {
                matches!(&self.nodes[id.index()].kind, EventKind::Start(parent) if !parent.kind().allows_descendant(tag.kind()))
            }) {
                return Err(StreamError::Unbalanced);
            }
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
        if start {
            self.open.push(id);
        }
        Ok(())
    }

    /// Attach the original source after the stream has ended. The buffer is moved, not copied.
    ///
    /// # Errors
    /// Returns [`StreamError::Unbalanced`] if consumption failed or has not reached EOF, or [`StreamError::InvalidRange`] for invalid source-backed values.
    pub fn finish<S: AsRef<str>>(self, source: S) -> Result<Document<S>, StreamError> {
        if self.failed || !self.complete {
            return Err(StreamError::Unbalanced);
        }
        let document = Document {
            source,
            nodes: self.nodes,
            syntax: BTreeMap::new(),
            edited: BTreeSet::new(),
            append_hint: None,
        };
        // Public producers can supply arbitrary offsets. Validate their contract once before exposing the document.
        for event in document.events() {
            event
                .expect("arena traversal cannot fail")
                .validate(Some(document.source()))?;
        }
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
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Document<S = String> {
    source: S,
    nodes: Vec<Node>,
    syntax: BTreeMap<NodeId, Option<NodeSyntax<'static>>>,
    // Opaque nodes whose value was replaced through `text_mut` or `set_kind`. A parsed frontmatter value
    // with LF in place of the source's CRs is the whole block only until it is edited, even if the
    // edit makes it equal again.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "BTreeSet::is_empty"))]
    edited: BTreeSet<NodeId>,
    // The child that `append_child` last appended. It is still its parent's last child while it has that parent and no next sibling, so a run of appends to one parent does not scan the parent's children each time.
    #[cfg_attr(feature = "serde", serde(skip))]
    append_hint: Option<NodeId>,
}

impl<S: AsRef<str>> Document<S> {
    /// Parse an owned or borrowed UTF-8 buffer into an arena document.
    ///
    /// # Errors
    /// Returns stream validation errors; built-in Markdown parsing has no syntax errors.
    pub fn parse(source: S, options: &crate::ParseOptions) -> Result<Self, StreamError> {
        let mut builder = AstBuilder::new();
        {
            let mut events = crate::EventParser::new(source.as_ref(), options);
            if builder.consume(&mut events, usize::MAX).is_err() {
                unreachable!("the built-in Markdown producer yields a balanced semantic stream");
            }
        }
        builder.finish(source)
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
    /// # Errors
    /// Rejects invalid source ranges, end events, changing the root identity,
    /// and replacing a container with children by a leaf.
    pub fn set_kind(&mut self, id: NodeId, kind: EventKind<'static>) -> Result<(), StreamError> {
        kind.validate(Some(self.source()))?;
        let node = self
            .nodes
            .get(id.index())
            .ok_or(StreamError::InvalidRange)?;
        let document = matches!(&kind, EventKind::Start(Tag::Document(_)));
        if matches!(&kind, EventKind::End(_))
            || document != (id == self.root())
            || (node.first_child.is_some() && !matches!(&kind, EventKind::Start(_)))
        {
            return Err(StreamError::Unbalanced);
        }
        if let EventKind::Start(tag) = &kind {
            self.check_capture(node.parent, tag.kind())?;
            if self.descendants(id).skip(1).any(|child| matches!(&self.nodes[child.index()].kind, EventKind::Start(child_tag) if !tag.kind().allows_descendant(child_tag.kind()))) {
                return Err(StreamError::Unbalanced);
            }
        }
        if matches!(kind, EventKind::Opaque { .. }) {
            self.edited.insert(id);
        } else {
            self.edited.remove(&id);
        }
        self.nodes[id.index()].kind = kind;
        self.syntax.insert(id, None);
        self.refresh_context(id);
        self.refresh_item(self.nodes[id.index()].parent);
        Ok(())
    }

    /// Edit the text of a text/code/HTML/opaque leaf. Source-backed values detach on first mutation.
    ///
    /// # Errors
    /// Returns [`StreamError::InvalidRange`] for an invalid node ID or a non-text node.
    pub fn text_mut(&mut self, id: NodeId) -> Result<&mut String, StreamError> {
        // Frontmatter's semantic body detaches independently of its retained fence metadata.
        let body = if self.whole_frontmatter(id) {
            self.frontmatter_value(id).map(String::from)
        } else {
            None
        };
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(StreamError::InvalidRange)?;
        if matches!(node.kind, EventKind::Opaque { .. }) {
            self.edited.insert(id);
        }
        let (EventKind::Text(text)
        | EventKind::SoftBreak(text)
        | EventKind::Math(text)
        | EventKind::Code { value: text, .. }
        | EventKind::CodeBlock { value: text, .. }
        | EventKind::Html { value: text, .. }
        | EventKind::Opaque { value: text, .. }) = &mut node.kind
        else {
            return Err(StreamError::InvalidRange);
        };
        if let Some(body) = body {
            *text = Text::from(body);
        }
        text.to_mut(self.source.as_ref())
    }

    /// Traverse without a heap-allocated traversal stack or cloned owned strings.
    pub fn events(&self) -> DocumentEvents<'_, S> {
        DocumentEvents {
            document: self,
            current: Some(self.root()),
            root: self.root(),
            closing: false,
        }
    }
}

/// A zero-allocation pull traversal over the arena's parent/child/sibling links.
pub struct DocumentEvents<'a, S> {
    document: &'a Document<S>,
    current: Option<NodeId>,
    root: NodeId,
    closing: bool,
}

// Borrow values rather than cloning Cow::Owned strings. Structural metadata has no owned storage.
fn borrow_kind<'a>(kind: &'a EventKind<'_>) -> EventKind<'a> {
    match kind {
        EventKind::Start(tag) => {
            let tag = match tag {
                crate::stream::Tag::Link { destination, title } => crate::stream::Tag::Link {
                    destination: destination.borrowed(),
                    title: title.as_ref().map(Text::borrowed),
                },
                crate::stream::Tag::Image { destination, title } => crate::stream::Tag::Image {
                    destination: destination.borrowed(),
                    title: title.as_ref().map(Text::borrowed),
                },
                crate::stream::Tag::FootnoteDefinition(value) => {
                    crate::stream::Tag::FootnoteDefinition(value.borrowed())
                }
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
            language,
            fence,
            math,
            unclosed,
        } => EventKind::CodeBlock {
            value: value.borrowed(),
            language: language.as_ref().map(Text::borrowed),
            fence: *fence,
            math: *math,
            unclosed: *unclosed,
        },
        EventKind::Math(value) => EventKind::Math(value.borrowed()),
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
        EventKind::Opaque { value, block } => EventKind::Opaque {
            value: value.borrowed(),
            block: *block,
        },
        EventKind::FootnoteReference(value) => EventKind::FootnoteReference(value.borrowed()),
        _ => kind.clone(),
    }
}

impl<'a, S: AsRef<str>> Iterator for DocumentEvents<'a, S> {
    type Item = Result<Event<'a>, StreamError>;
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = &self.document.nodes[id.index()];
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
fn next_descendant(nodes: &[Node], root: NodeId, id: NodeId) -> Option<NodeId> {
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
        DocumentEvents {
            document: self,
            current: self.node(id).map(|_| id),
            root: id,
            closing: false,
        }
    }
    /// Plain semantic text, including image alternative text, code and raw HTML.
    pub fn text_content(&self, id: NodeId) -> String {
        self.collect_text::<false>(id)
    }
    fn collect_text<const HEADING: bool>(&self, id: NodeId) -> String {
        let mut text = String::new();
        for child in self.descendants(id) {
            crate::headings::append_text::<HEADING>(
                &self.nodes[child.index()].kind,
                self.source(),
                &mut text,
            );
        }
        text
    }
    /// Extract headings and anchors using the HTML writer's rules. Filtered headings still reserve their anchors.
    pub fn headings(&self, options: &crate::HeadingOptions) -> Vec<crate::HeadingEntry> {
        let mut anchors = crate::headings::AnchorGenerator::default();
        let mut entries = Vec::new();
        for id in self.descendants(self.root()) {
            if let EventKind::Start(Tag::Heading(level)) = self.nodes[id.index()].kind {
                let mut text = self.collect_text::<true>(id);
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
            let mut previous: Option<NodeId> = None;
            for child in self
                .children(id)
                .filter(|id| !matches!(self.nodes[id.index()].kind, EventKind::Separator { .. }))
            {
                if let Some(previous) = previous {
                    let end = self.nodes[previous.index()].span.end;
                    let start = self.nodes[child.index()].span.start;
                    if let Some(gap) = self.source().get(end..start) {
                        let ended = self
                            .source()
                            .as_bytes()
                            .get(end.wrapping_sub(1))
                            .is_some_and(|byte| matches!(byte, b'\r' | b'\n'));
                        if crate::util::line_ending::line_endings(gap)
                            .nth(usize::from(!ended))
                            .is_some()
                        {
                            return Some(NodeSyntax::ListItem { spread: true });
                        }
                    }
                }
                previous = Some(child);
            }
            return Some(NodeSyntax::ListItem { spread: false });
        }
        crate::node_syntax::infer(self.source(), node.span.clone(), &node.kind)
    }
    /// Override authored syntax. `None` disconnects the node from inferred source syntax.
    ///
    /// # Errors
    /// Rejects invalid node IDs, source ranges and metadata for an incompatible node kind.
    pub fn set_syntax(
        &mut self,
        id: NodeId,
        syntax: Option<NodeSyntax<'static>>,
    ) -> Result<(), StreamError> {
        let node = self.node(id).ok_or(StreamError::InvalidRange)?;
        if syntax
            .as_ref()
            .is_some_and(|value| !value.valid_for(&node.kind, self.source()))
        {
            return Err(StreamError::InvalidRange);
        }
        self.syntax.insert(id, syntax);
        Ok(())
    }
    /// Frontmatter's body without its opening and closing fences. Edited bodies are returned directly.
    pub fn frontmatter_value(&self, id: NodeId) -> Option<&str> {
        let node = self.node(id)?;
        let EventKind::Opaque { value: text, .. } = &node.kind else {
            return None;
        };
        if !matches!(self.syntax(id), Some(NodeSyntax::Frontmatter { .. })) {
            return None;
        }
        let value = text
            .resolve(self.source())
            .expect("document text is validated");
        if self.whole_frontmatter(id) {
            if let Some((_, _, _, body)) = crate::node_syntax::frontmatter(value) {
                return value.get(body);
            }
        }
        Some(value)
    }
    /// Whether the value of an opaque node is still the whole block it was parsed from, with its
    /// fences, rather than an edited body. Its line endings are LF, whichever the source used, so
    /// a block with a CR in the source is a value rather than the source range until it is edited.
    /// A parsed block without a CR is always the source range, so a value for it is an edit, even
    /// in a document rebuilt from another's events, which does not keep the record of edits.
    fn whole_frontmatter(&self, id: NodeId) -> bool {
        let Some(Node {
            kind: EventKind::Opaque { value, .. },
            span,
            ..
        }) = self.node(id)
        else {
            return false;
        };
        match value {
            Text::Source(range) => range == span,
            Text::Value(value) => {
                !self.edited.contains(&id)
                    && self.source().get(span.clone()).is_some_and(|raw| {
                        raw.contains('\r') && crate::util::line_ending::normalizes_to(raw, value)
                    })
            }
            Text::Literal(_) => false,
        }
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
            kind: EventKind::Start(Tag::Document(None)),
            span: 0..0,
        };
        let end = Event {
            kind: EventKind::End(TagKind::Document),
            span: self.source().len()..self.source().len(),
        };
        crate::render_events(
            self.source(),
            core::iter::once(Ok(start))
                .chain(self.subtree_events(id))
                .chain(core::iter::once(Ok(end))),
            options,
            output,
        )
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
        self.nodes[id.index()].kind = EventKind::Start(Tag::Item { block_end });
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
            self.nodes[parent.index()].kind = EventKind::Start(Tag::Item { block_end });
        }
    }
    // Edits can change the context of existing paragraphs and item endings. Refresh those rendering hints without adding caches or links to every arena node.
    fn refresh_context(&mut self, root: NodeId) {
        let mut current = Some(root);
        while let Some(id) = current {
            current = next_descendant(&self.nodes, root, id);
            if matches!(
                self.nodes[id.index()].kind,
                EventKind::Start(Tag::Paragraph { .. })
            ) {
                self.nodes[id.index()].kind = EventKind::Start(Tag::Paragraph {
                    tight: self.tight_paragraph(id),
                });
            }
            self.refresh_item(Some(id));
        }
    }
    fn check_capture(&self, mut parent: Option<NodeId>, kind: TagKind) -> Result<(), StreamError> {
        while let Some(id) = parent {
            let node = &self.nodes[id.index()];
            if matches!(&node.kind,EventKind::Start(tag) if !tag.kind().allows_descendant(kind)) {
                return Err(StreamError::Unbalanced);
            }
            parent = node.parent;
        }
        Ok(())
    }
    /// Append a new semantic node. Its source provenance is empty; values may still reference the source buffer.
    ///
    /// Appending in document order, each node after the subtree before it is complete, takes amortized constant time per node. Appending to another parent scans its children.
    ///
    /// # Errors
    /// Rejects invalid parents, document/end kinds, source ranges, or forbidden nested capture containers.
    pub fn append_child(
        &mut self,
        parent: NodeId,
        kind: EventKind<'static>,
    ) -> Result<NodeId, StreamError> {
        kind.validate(Some(self.source()))?;
        if !matches!(
            self.node(parent).map(|node| &node.kind),
            Some(EventKind::Start(_))
        ) {
            return Err(StreamError::Unbalanced);
        }
        match &kind {
            EventKind::End(_) | EventKind::Start(Tag::Document(_)) => {
                return Err(StreamError::Unbalanced)
            }
            EventKind::Start(tag) => self.check_capture(Some(parent), tag.kind())?,
            _ => {}
        }
        let previous = match self.appended_last_child(parent) {
            Some(previous) => previous,
            None => self.children(parent).last(),
        };
        let id = self.insert_child(parent, kind, previous, None);
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
        id
    }
    /// Detach a subtree without moving its arena slots or invalidating IDs.
    ///
    /// # Errors
    /// Rejects invalid IDs and detaching the document root.
    pub fn detach(&mut self, id: NodeId) -> Result<(), StreamError> {
        if id == self.root() {
            return Err(StreamError::Unbalanced);
        }
        let node = self.node(id).ok_or(StreamError::InvalidRange)?;
        let Some(parent) = node.parent else {
            return Ok(());
        };
        self.unlink_child(id, parent);
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
    /// Move an existing subtree into a container, optionally before one of its children. No node or text allocation is required.
    ///
    /// # Errors
    /// Rejects cycles, invalid IDs/positions, root moves, and forbidden capture nesting before modifying the tree.
    pub fn attach(
        &mut self,
        parent: NodeId,
        id: NodeId,
        before: Option<NodeId>,
    ) -> Result<(), StreamError> {
        if id == self.root()
            || !matches!(
                self.node(parent).map(|node| &node.kind),
                Some(EventKind::Start(_))
            )
        {
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
        for child in self.descendants(id) {
            if let EventKind::Start(tag) = &self.nodes[child.index()].kind {
                self.check_capture(Some(parent), tag.kind())?;
            }
        }
        if before == Some(id) {
            return Ok(());
        }
        let former_parent = self.nodes[id.index()].parent;
        if let Some(parent) = former_parent {
            self.unlink_child(id, parent);
        }
        let previous = self
            .children(parent)
            .take_while(|child| Some(*child) != before)
            .last();
        self.link_child(parent, id, previous, before);
        self.refresh_context(id);
        if former_parent != Some(parent) {
            self.refresh_item(former_parent);
        }
        self.refresh_item(Some(parent));
        Ok(())
    }
    /// Detach all direct children, returning their stable IDs in order.
    ///
    /// # Errors
    /// Rejects invalid node IDs.
    pub fn extract_children(&mut self, parent: NodeId) -> Result<Vec<NodeId>, StreamError> {
        self.node(parent).ok_or(StreamError::InvalidRange)?;
        let children: Vec<_> = self.children(parent).collect();
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
    /// # Errors
    /// Rejects invalid parent IDs. Returns `false` when the selected child is not a heading.
    pub fn replace_heading_section<T: AsRef<str>>(
        &mut self,
        parent: NodeId,
        heading_index: usize,
        replacement: &Document<T>,
        replace_heading: bool,
        remove_subsections: bool,
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
        // Selecting an existing heading proves this parent and all its ancestors accept heading/footnote captures. Every copied subtree is already validated in its source document.
        if !replace_heading {
            previous = Some(heading);
        }
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
                self.nodes[id.index()].parent = None;
                self.nodes[id.index()].next_sibling = None;
                self.refresh_context(id);
            }
            self.refresh_item(Some(parent));
        }
        for child in replacement.children(replacement.root()) {
            previous = Some(self.copy_subtree(replacement, child, parent, previous, end));
        }
        Ok(true)
    }
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
            let body = source.frontmatter_value(original);
            let kind = borrow_kind(&node.kind).map_text(|value| {
                body.map_or_else(
                    || value.materialized(source.source()),
                    |body| Text::from(String::from(body)),
                )
            });
            let next = if original == root { before } else { None };
            let id = self.insert_child(parent, kind, previous, next);
            if let Some(syntax) = source
                .syntax(original)
                .and_then(|syntax| syntax.copied(source.source()))
            {
                self.set_syntax(id, Some(syntax))
                    .expect("materialized syntax describes the copied node kind");
            }
            if node.first_child.is_some() {
                original_parent = Some(original);
                parent = id;
                previous = None;
            } else {
                previous = Some(id);
            }
        }
        self.refresh_context(copied_root);
        self.refresh_item(self.nodes[copied_root.index()].parent);
        copied_root
    }
}

// Whether a child of an item decides how the item ends: other children write nothing after the item's last line.
fn ends_item(kind: &EventKind<'_>) -> bool {
    !matches!(
        kind,
        EventKind::Separator { .. }
            | EventKind::Definition { .. }
            | EventKind::Opaque { .. }
            | EventKind::Start(Tag::FootnoteDefinition(_))
    )
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
    match kind {
        EventKind::Text(Text::Source(range)) => range.is_empty(),
        EventKind::Text(Text::Literal(value)) => value.is_empty(),
        EventKind::Text(Text::Value(value)) => value.is_empty(),
        _ => false,
    }
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

// Whether normalization removes a child, in a container that holds phrasing or not, without joining it into another: empty text, a separator in phrasing, and attention without children.
fn removed(node: &Node, phrasing: bool) -> bool {
    empty_text(&node.kind)
        || (phrasing && matches!(node.kind, EventKind::Separator { .. }))
        || (attention(&node.kind) && node.first_child.is_none())
}

// Whether a node is a paragraph without children, which Markdown cannot write.
fn empty_paragraph(node: &Node) -> bool {
    matches!(node.kind, EventKind::Start(Tag::Paragraph { .. })) && node.first_child.is_none()
}

// Whether a node is a separator between blocks.
fn separator(nodes: &[Node], id: Option<NodeId>) -> bool {
    id.is_some_and(|id| matches!(nodes[id.index()].kind, EventKind::Separator { .. }))
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

// Whether normalization joins `kind` into the child kept before it: text after text, inline code after inline code that has no language of its own, inline math after inline math, and attention after attention of the same kind.
fn joined(kept: Option<&EventKind<'_>>, kind: &EventKind<'_>) -> bool {
    kept.is_some_and(|kept| {
        matches!(
            (kept, kind),
            (EventKind::Text(_), EventKind::Text(_))
                | (
                    EventKind::Code { .. },
                    EventKind::Code { language: None, .. }
                )
                | (EventKind::Math(_), EventKind::Math(_))
        ) || same_attention(kept, kind)
    })
}

// Whether a node is a table cell, where Markdown cannot write a hard break.
fn table_cell(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::TableCell { .. }))
}

// Whether a node is a paragraph, heading or table cell, whose phrasing normalization trims.
fn trimmed(kind: &EventKind<'_>) -> bool {
    matches!(
        kind,
        EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::TableCell { .. })
    )
}

// The strikethrough and table cells that enclose a node.
#[derive(Clone, Copy, Default)]
struct Enclosing {
    deletes: usize,
    cells: usize,
}

impl Enclosing {
    // Count a node as enclosing the nodes below it.
    fn enter(&mut self, kind: &EventKind<'_>) {
        self.deletes += usize::from(delete(kind));
        self.cells += usize::from(table_cell(kind));
    }
    // Stop counting a node once the nodes below it are passed.
    fn leave(&mut self, kind: &EventKind<'_>) {
        self.deletes -= usize::from(delete(kind));
        self.cells -= usize::from(table_cell(kind));
    }
}

// The node after `id` in document order within the subtree at `root`, keeping `enclosing` as the strikethrough and table cells that enclose the current node.
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

// How much of a child, at one of its edges, is whitespace that normalization moves out of attention or trims from a block.
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

// Whitespace at the edge of a paragraph, heading or table cell, which the parser trims: space, tab and line endings, which are LF in values, but not other Unicode whitespace such as a no-break space. A CR in a value is a character reference, which the parser does not trim, so it stays.
fn line_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n')
}

// No whitespace, for a paragraph that starts with inline HTML, which loses only the breaks at its edges.
fn no_whitespace(_: char) -> bool {
    false
}

// Whether `parent` is a paragraph whose phrasing starts with inline HTML once the whitespace and soft breaks at its start are skipped. Trimming the whitespace before the HTML would leave it at the start of a line, where it can start an HTML block, and trimming the whitespace after it could leave it alone on its line, where it is one, so the whitespace text of such a paragraph stays.
fn starts_with_html(nodes: &[Node], source: &str, parent: NodeId) -> bool {
    if !matches!(
        nodes[parent.index()].kind,
        EventKind::Start(Tag::Paragraph { .. })
    ) {
        return false;
    }
    let mut child = nodes[parent.index()].first_child;
    while let Some(id) = child {
        let kind = &nodes[id.index()].kind;
        if matches!(kind, EventKind::Html { .. }) {
            return true;
        }
        if leading_edge(kind, source, line_whitespace) != Edge::All {
            return false;
        }
        child = nodes[id.index()].next_sibling;
    }
    false
}

// The whitespace, matched by `space`, at the start of a child: a soft break, or text that starts with it.
fn leading_edge(kind: &EventKind<'_>, source: &str, space: fn(char) -> bool) -> Edge {
    match kind {
        EventKind::SoftBreak(_) => Edge::All,
        EventKind::Text(text) => {
            let value = text.resolve(source).expect("document text is validated");
            let rest = value.trim_start_matches(space);
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

// The whitespace, matched by `space`, at the end of a child: a soft or hard break, or text that ends with it.
fn trailing_edge(kind: &EventKind<'_>, source: &str, space: fn(char) -> bool) -> Edge {
    match kind {
        EventKind::SoftBreak(_) | EventKind::HardBreak => Edge::All,
        EventKind::Text(text) => {
            let value = text.resolve(source).expect("document text is validated");
            let rest = value.trim_end_matches(space);
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

// Whether the first and last children of `node` have no whitespace that normalization moves out of attached attention or trims from a paragraph, heading or table cell, other than a task item's paragraph.
fn edges_normalized(
    nodes: &[Node],
    id: NodeId,
    first: &EventKind<'_>,
    last: &EventKind<'_>,
    source: &str,
) -> bool {
    let node = &nodes[id.index()];
    let space: fn(char) -> bool = if attention(&node.kind) && node.parent.is_some() {
        unicode_whitespace
    } else if trimmed(&node.kind) && !matches!(first, EventKind::TaskMarker(_)) {
        if starts_with_html(nodes, source, id) {
            no_whitespace
        } else {
            line_whitespace
        }
    } else {
        return true;
    };
    leading_edge(first, source, space) == Edge::None
        && trailing_edge(last, source, space) == Edge::None
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

// Space a hard break in a table cell takes, as Markdown cannot write the break there.
fn break_to_space(node: &mut Node, cell: bool) {
    if cell && matches!(node.kind, EventKind::HardBreak) {
        node.kind = EventKind::Text(Text::Literal(" "));
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

// Move the whitespace at the start of `attention`, after `previous` in its parent, out of it and out of the attention nested at its start, to just before it, updating `previous` to the child now before it. Attention left empty is removed, and the whitespace takes its place. A hard break stays, except in a table cell, where it becomes a space and moves. Returns whether anything changed.
fn move_leading(
    nodes: &mut Vec<Node>,
    source: &str,
    previous: &mut Option<NodeId>,
    attention_id: NodeId,
    cell: bool,
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
        break_to_space(&mut nodes[first.index()], cell);
        let edge = leading_edge(&nodes[first.index()].kind, source, unicode_whitespace);
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

// Move the whitespace at the end of `attention`, after `previous` in its parent, out of it and out of the attention nested at its end, to just after it, keeping its order. Attention left empty is removed, and the whitespace takes its place. Hard breaks move too, and in a table cell become spaces first. Returns whether anything changed.
fn move_trailing(
    nodes: &mut Vec<Node>,
    source: &str,
    previous: Option<NodeId>,
    attention_id: NodeId,
    cell: bool,
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
                break_to_space(&mut nodes[current.index()], cell);
                if trailing_edge(&nodes[current.index()].kind, source, unicode_whitespace)
                    != Edge::All
                {
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
        let edge = trailing_edge(&nodes[child.index()].kind, source, unicode_whitespace);
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

// Trim space, tab and LF from the edges of the phrasing of `parent`, a paragraph, heading or table cell, removing soft breaks at either edge and hard breaks at the end. A paragraph that starts with inline HTML loses only the breaks. Returns whether anything changed.
fn trim_edges(nodes: &mut [Node], source: &str, parent: NodeId) -> bool {
    let space: fn(char) -> bool = if starts_with_html(nodes, source, parent) {
        no_whitespace
    } else {
        line_whitespace
    };
    let mut changed = false;
    while let Some(first) = nodes[parent.index()].first_child {
        let edge = leading_edge(&nodes[first.index()].kind, source, space);
        match (&mut nodes[first.index()].kind, edge) {
            (_, Edge::All) => {
                nodes[parent.index()].first_child = nodes[first.index()].next_sibling.take();
                nodes[first.index()].parent = None;
                changed = true;
            }
            (EventKind::Text(text), Edge::Part(at)) => {
                *text = split_text(text, at);
                changed = true;
                break;
            }
            _ => break,
        }
    }
    let mut last = None;
    let mut child = nodes[parent.index()].first_child;
    while let Some(id) = child {
        if trailing_edge(&nodes[id.index()].kind, source, space) != Edge::All {
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
    if let Some(last) = last {
        let edge = trailing_edge(&nodes[last.index()].kind, source, space);
        if let (EventKind::Text(text), Edge::Part(at)) = (&mut nodes[last.index()].kind, edge) {
            split_text(text, at);
            changed = true;
        }
    }
    changed
}

impl<S: AsRef<str>> Document<S> {
    /// Normalize every child list in place, so that the tree is one the Markdown serializer can write correctly and stably, in time linear in the size of the arena.
    ///
    /// Empty text is removed, and so is emphasis, strong or strikethrough that has no children once its own children are normalized, such as emphasis of empty text or strong that holds only empty emphasis. Strikethrough inside other strikethrough, at any depth, is unwrapped: its children take its place, since strikethrough cannot be written inside strikethrough and renders the same once. Adjacent text is joined into the first text, adjacent inline code is joined into the first code when the second has no language of its own, and adjacent inline math is joined into the first math, as adjacent code spans or math cannot be written apart. Adjacent emphasis, strong or strikethrough of the same kind is merged by moving the children of the second to the end of the first.
    ///
    /// Markdown cannot write whitespace at the edges of emphasis, strong or strikethrough, so it moves out: Unicode whitespace in text and soft breaks at either edge, and hard breaks at the end, move out through any attention nested at that edge to just before or after the outermost attention whose edge they are. Attention left empty is removed, and what moved out takes its place. A hard break at the start of attention stays, and links and images keep whitespace at their edges. The phrasing of a paragraph, heading or table cell is trimmed as the parser trims it: space, tab and LF at either edge are removed, with soft breaks there and hard breaks at the end, including those that moved out of attention, but other Unicode whitespace, such as a no-break space or a CR, which is a character reference as line endings are LF in values, stays, and so does a hard break at the start. Markdown cannot write a hard break in a table cell, so one there, at any depth, becomes a space.
    ///
    /// Children that meet where a node was removed, moved, unwrapped or merged are normalized in turn, so text, code, math or attention on either side is joined or merged too. Separators are block-level: one inside a paragraph, heading, table cell, emphasis, strong, strikethrough, link or image is removed, as the Markdown serializer writes nothing for it, and one between blocks stays where it is and is skipped when looking for an adjacent child. A paragraph with no children left, such as one of whitespace alone, cannot be written, so it is removed with one separator beside it, the one after it if there is one, as the blocks around it render the same without it. The paragraph of a task item is not trimmed, as the marker cannot be written without text after it. A paragraph that starts with inline HTML, once whitespace and soft breaks at its start are skipped, keeps its whitespace text and loses only the breaks at its edges, as the HTML would otherwise start a line, where it can start an HTML block, or be alone on its line, where it is one. Nothing else changes: text keeps its value apart from whitespace moved or trimmed at these edges. Detached subtrees are normalized too, though the root of a detached subtree is never removed or unwrapped, and a detached root that is attention keeps whitespace at its own edges.
    ///
    /// Call this before serializing a parsed or edited tree to Markdown: the serializer assumes normalized input, and writes other trees without panicking, but possibly incorrectly or unstably. [`Document::parse`] does not normalize, because joining text across a backslash escape or character reference would copy it out of the source: parsed text is split there, adjacent attention of the same kind, such as `*a*_b_`, stays separate, and strikethrough inside strikethrough, such as `~a~~b~~c~`, stays nested. Serializing a parsed document without normalizing it usually gives the same output; [`crate::normalize_markdown`] normalizes before serializing.
    ///
    /// The IDs of removed text, breaks, attention, paragraphs and separators, joined text, code and math, merged attention and unwrapped strikethrough no longer refer to nodes in the tree. Their arena slots remain, detached, but their contents are unspecified. Whitespace split off the edge of a text node is a new node with an empty span, and the text node keeps its ID and span with the rest of its value. Whitespace and breaks that move whole, and a hard break that becomes a space, keep their IDs. Other IDs stay valid.
    pub fn normalize(&mut self) {
        let mut scratch = Trailing::default();
        let mut blocks = Vec::new();
        for index in 0..self.nodes.len() {
            if self.nodes[index].parent.is_none() {
                self.remove_subtree(NodeId::new(index));
                self.normalize_subtree(NodeId::new(index), &mut scratch);
                self.remove_empty_paragraphs(NodeId::new(index), &mut blocks);
            }
        }
    }

    /// Whether [`Document::normalize`] would change nothing: no child list, attached or detached, has empty text, attention without children, adjacent text, adjacent inline code where the second has no language, adjacent inline math, adjacent attention of the same kind, or a separator in phrasing; no strikethrough is inside other strikethrough; no hard break is inside a table cell; no attached emphasis, strong or strikethrough starts or ends with whitespace or a soft break, or ends with a hard break; no paragraph, heading or table cell, other than the paragraph of a task item, starts or ends with space, tab, LF or a soft break, or ends with a hard break, though a paragraph that starts with inline HTML after whitespace may start or end with whitespace text; and no attached paragraph is empty. This takes time linear in the size of the arena.
    pub fn is_normalized(&self) -> bool {
        let source = self.source.as_ref();
        let nodes = &self.nodes;
        (0..nodes.len())
            .filter(|&index| nodes[index].parent.is_none())
            .all(|index| {
                let root = NodeId::new(index);
                let mut enclosing = Enclosing::default();
                let mut current = Some(root);
                while let Some(id) = current {
                    let node = &nodes[id.index()];
                    if (enclosing.deletes > 0 && delete(&node.kind))
                        || (enclosing.cells > 0 && matches!(node.kind, EventKind::HardBreak))
                    {
                        return false;
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
                        {
                            return false;
                        }
                        if !matches!(node.kind, EventKind::Separator { .. }) {
                            kept = Some(&node.kind);
                        }
                        last = Some(&node.kind);
                        child = node.next_sibling;
                    }
                    if let (Some(first), Some(last)) = (node.first_child, last) {
                        if !edges_normalized(nodes, id, &nodes[first.index()].kind, last, source) {
                            return false;
                        }
                    }
                    current = next_enclosed(nodes, root, id, &mut enclosing);
                }
                true
            })
    }

    // Remove the children that are dropped without joining from each child list, from the bottom up, so that attention whose children are all removed is removed in turn. Each removed node has no children left, so it stays detached and normalized.
    fn remove_subtree(&mut self, root: NodeId) {
        let mut id = first_leaf(&self.nodes, root);
        loop {
            if self.nodes[id.index()].first_child.is_some() {
                self.remove_children(id);
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

    // Remove each paragraph without children, which is left once its phrasing is removed or trimmed, from the top down, as removing one empties nothing else. `kept` is scratch storage for the children kept so far in a list.
    fn remove_empty_paragraphs(&mut self, root: NodeId, kept: &mut Vec<NodeId>) {
        let mut current = Some(root);
        while let Some(id) = current {
            if self.nodes[id.index()].first_child.is_some() {
                self.remove_empty_children(id, kept);
            }
            current = next_descendant(&self.nodes, root, id);
        }
    }

    // Unlink each paragraph of `parent` without children, with one separator next to it, the one after it if there is one, so that the blocks on either side stay separated once.
    fn remove_empty_children(&mut self, parent: NodeId, kept: &mut Vec<NodeId>) {
        let nodes = &mut self.nodes;
        kept.clear();
        let mut changed = false;
        let mut current = nodes[parent.index()].first_child;
        while let Some(id) = current {
            current = nodes[id.index()].next_sibling.take();
            if !empty_paragraph(&nodes[id.index()]) {
                nodes[id.index()].next_sibling = current;
                kept.push(id);
                continue;
            }
            changed = true;
            nodes[id.index()].parent = None;
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

    // Normalize each child list from the top down, so that children moved into attention are normalized with its own children, counting the strikethrough and table cells that enclose each node so that nested strikethrough is unwrapped and hard breaks in cells are replaced without searching ancestors.
    fn normalize_subtree(&mut self, root: NodeId, scratch: &mut Trailing) {
        let mut enclosing = Enclosing::default();
        let mut current = Some(root);
        while let Some(id) = current {
            if self.nodes[id.index()].first_child.is_some() {
                let mut inner = enclosing;
                inner.enter(&self.nodes[id.index()].kind);
                self.normalize_children(id, inner, scratch);
            }
            current = next_enclosed(&self.nodes, root, id, &mut enclosing);
        }
    }

    // Normalize the children of `parent`, enclosed by `enclosing` including itself, in one pass: unwrap each strikethrough child of strikethrough in place, so its children are normalized next; replace each hard break in a table cell with a space; unlink each child joined into the last child kept as it is met, merging attention first; once no more attention merges into the last kept attention, move whitespace out of its end and of the attention nested there, and continue from what moved after it; and move whitespace out of the start of each attention child and of the attention nested there, and continue from what moved before it. Then trim the edges of a paragraph, heading or table cell. Empty children have already been removed, and paragraphs emptied by trimming are removed afterwards.
    fn normalize_children(&mut self, parent: NodeId, enclosing: Enclosing, scratch: &mut Trailing) {
        let source = self.source.as_ref();
        let nodes = &mut self.nodes;
        let enclosed = enclosing.deletes > 0;
        let cell = enclosing.cells > 0;
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
                            && move_trailing(
                                nodes,
                                source,
                                previous,
                                attention_id,
                                cell,
                                scratch,
                            ) =>
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
            if cell && matches!(nodes[id.index()].kind, EventKind::HardBreak) {
                break_to_space(&mut nodes[id.index()], cell);
                changed = true;
            }
            if !joined(
                kept.map(|kept| &nodes[kept.index()].kind),
                &nodes[id.index()].kind,
            ) {
                // No more attention merges into the pending attention: move its trailing edge out, after it, and continue from what moved.
                if let Some((attention_id, previous)) = pending.take() {
                    if move_trailing(nodes, source, previous, attention_id, cell, scratch) {
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
                        && move_leading(nodes, source, &mut previous, id, cell)
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
                | (EventKind::Math(left), EventKind::Math(right))
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
        // A task item's paragraph is not trimmed, as the marker cannot be written without text after it.
        let task = nodes[parent.index()]
            .first_child
            .is_some_and(|first| matches!(nodes[first.index()].kind, EventKind::TaskMarker(_)));
        if trimmed(&nodes[parent.index()].kind) && !task {
            changed |= trim_edges(nodes, source, parent);
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
            #[serde(default)]
            edited: BTreeSet<NodeId>,
        }
        let stored = Stored::deserialize(deserializer)?;
        let document = Self {
            source: stored.source,
            nodes: stored.nodes,
            syntax: stored.syntax,
            edited: stored.edited,
            append_hint: None,
        };
        document
            .validate_storage()
            .map_err(serde::de::Error::custom)?;
        Ok(document)
    }
}

#[cfg(feature = "serde")]
impl<S: AsRef<str>> Document<S> {
    // Validate the complete arena forest, including detached nodes, before public traversal can observe deserialized links. Each slot must be visited exactly once.
    fn validate_storage(&self) -> Result<(), StreamError> {
        if !matches!(
            self.nodes.first().map(|node| &node.kind),
            Some(EventKind::Start(Tag::Document(_)))
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
                pending.push((NodeId::new(index), None, false));
            }
        }
        while let Some((id, parent, capture)) = pending.pop() {
            let node = self.node(id).ok_or(StreamError::InvalidRange)?;
            if seen[id.index()] || node.parent != parent {
                return Err(StreamError::Unbalanced);
            }
            seen[id.index()] = true;
            Event {
                kind: borrow_kind(&node.kind),
                span: node.span.clone(),
            }
            .validate(Some(self.source()))?;
            let container = matches!(node.kind, EventKind::Start(_));
            if matches!(node.kind, EventKind::End(_))
                || (node.first_child.is_some() && !container)
                || matches!(node.kind, EventKind::Start(Tag::Document(_))) != (id == self.root())
            {
                return Err(StreamError::Unbalanced);
            }
            if capture
                && matches!(
                    node.kind,
                    EventKind::Start(Tag::Heading(_) | Tag::FootnoteDefinition(_))
                )
            {
                return Err(StreamError::Unbalanced);
            }
            if let Some(next) = node.next_sibling {
                pending.push((next, parent, capture));
            }
            if let Some(child) = node.first_child {
                let capture = capture
                    || matches!(
                        node.kind,
                        EventKind::Start(Tag::Heading(_) | Tag::Image { .. })
                    );
                pending.push((child, Some(id), capture));
            }
        }
        if seen.iter().any(|visited| !*visited) {
            return Err(StreamError::Unbalanced);
        }
        for (&id, value) in &self.syntax {
            let node = self.node(id).ok_or(StreamError::InvalidRange)?;
            if value
                .as_ref()
                .is_some_and(|value| !value.valid_for(&node.kind, self.source()))
            {
                return Err(StreamError::InvalidRange);
            }
        }
        Ok(())
    }
}
