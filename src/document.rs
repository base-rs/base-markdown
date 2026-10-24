// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! A source-backed arena document and its incremental pull consumer.
use crate::stream::{Event, EventKind, StreamError, Text};
use crate::{
    stream::{Tag, TagKind},
    NodeSyntax,
};
use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec::Vec};
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
}

impl<S: AsRef<str>> Document<S> {
    /// Parse an owned or borrowed UTF-8 buffer into an arena document.
    ///
    /// # Errors
    /// Returns stream validation errors; built-in Markdown parsing has no syntax errors.
    pub fn parse(source: S, options: &crate::ParseOptions) -> Result<Self, StreamError> {
        let mut builder = AstBuilder::new();
        {
            let mut events = crate::EventParser::prepare(source.as_ref(), options);
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
        let body = if matches!(self.node(id), Some(Node {kind:EventKind::Opaque {value:Text::Source(range),..},span,..}) if range==span)
        {
            self.frontmatter_value(id).map(String::from)
        } else {
            None
        };
        let node = self
            .nodes
            .get_mut(id.index())
            .ok_or(StreamError::InvalidRange)?;
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
        if let Text::Source(range) = text {
            *text = Text::Value(Cow::Owned(
                self.source
                    .as_ref()
                    .get(range.clone())
                    .ok_or(StreamError::InvalidRange)?
                    .into(),
            ));
        }
        match text {
            Text::Value(value) => Ok(value.to_mut()),
            Text::Literal(value) => {
                *text = Text::Value(Cow::Owned((*value).into()));
                match text {
                    Text::Value(value) => Ok(value.to_mut()),
                    _ => unreachable!("literal was materialized above"),
                }
            }
            Text::Source(_) => unreachable!("source text was materialized above"),
        }
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
        let node = &self.nodes[id.index()];
        if let Some(child) = node.first_child {
            self.current = Some(child);
        } else {
            let mut at = id;
            loop {
                if at == self.root {
                    self.current = None;
                    break;
                }
                let node = &self.nodes[at.index()];
                if let Some(next) = node.next_sibling {
                    self.current = Some(next);
                    break;
                }
                at = node
                    .parent
                    .expect("a descendant has a parent before its subtree root");
            }
        }
        Some(id)
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
            match &self.nodes[child.index()].kind {
                EventKind::Text(value)
                | EventKind::SoftBreak(value)
                | EventKind::Math(value)
                | EventKind::Code { value, .. }
                | EventKind::Html { value, .. } => text.push_str(
                    value
                        .resolve(self.source())
                        .expect("document text is validated"),
                ),
                EventKind::CodeBlock { value, .. } if !HEADING => text.push_str(
                    value
                        .resolve(self.source())
                        .expect("document text is validated"),
                ),
                EventKind::HardBreak => text.push('\n'),
                _ => {}
            }
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
                if text.ends_with("\r\n") {
                    text.truncate(text.len() - 2);
                } else if text.ends_with(['\r', '\n']) {
                    text.pop();
                }
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
            let mut spread = false;
            for child in self
                .children(id)
                .filter(|id| !matches!(self.nodes[id.index()].kind, EventKind::Separator { .. }))
            {
                if let Some(previous) = previous {
                    let end = self.nodes[previous.index()].span.end;
                    let start = self.nodes[child.index()].span.start;
                    if let Some(gap) = self.source().get(end..start) {
                        let lines = gap.bytes().filter(|&byte| byte == b'\n').count()
                            + gap
                                .as_bytes()
                                .windows(2)
                                .filter(|pair| pair[0] == b'\r' && pair[1] != b'\n')
                                .count()
                            + usize::from(gap.ends_with('\r'));
                        let ended = self
                            .source()
                            .as_bytes()
                            .get(end.wrapping_sub(1))
                            .is_some_and(|byte| matches!(byte, b'\r' | b'\n'));
                        spread |= lines >= if ended { 1 } else { 2 };
                    }
                }
                previous = Some(child);
            }
            return Some(NodeSyntax::ListItem { spread });
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
        if let Text::Source(range) = text {
            if *range == node.span {
                if let Some((_, _, _, body)) = crate::node_syntax::frontmatter(value) {
                    return value.get(body);
                }
            }
        }
        Some(value)
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
            .filter(|child| {
                !matches!(
                    self.nodes[child.index()].kind,
                    EventKind::Separator { .. }
                        | EventKind::Definition { .. }
                        | EventKind::Opaque { .. }
                        | EventKind::Start(Tag::FootnoteDefinition(_))
                )
            })
            .last();
        let block_end = last.is_some_and(|last| !self.tight_paragraph(last));
        self.nodes[id.index()].kind = EventKind::Start(Tag::Item { block_end });
    }
    // Edits can change the context of existing paragraphs and item endings. Refresh those rendering hints without adding caches or links to every arena node.
    fn refresh_context(&mut self, root: NodeId) {
        let mut current = Some(root);
        while let Some(id) = current {
            let mut iterator = Descendants {
                nodes: &self.nodes,
                root,
                current,
            };
            iterator.next();
            current = iterator.current;
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
        let previous = self.children(parent).last();
        let id = self.insert_child(parent, kind, previous, None);
        self.refresh_context(id);
        self.refresh_item(Some(parent));
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
        let Some(heading) = self.children(parent).nth(heading_index) else {
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
        let mut at = first;
        while at != end {
            let id = at.expect("the section ends in the selected sibling chain");
            at = self.nodes[id.index()].next_sibling;
            self.detach(id)
                .expect("a selected section contains existing non-root siblings");
        }
        let mut previous = self
            .children(parent)
            .take_while(|child| Some(*child) != end)
            .last();
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
        let document = Self {
            source: stored.source,
            nodes: stored.nodes,
            syntax: stored.syntax,
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
