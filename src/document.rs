// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! A source-backed arena document and its incremental pull consumer.
use crate::stream::{Event, EventKind, StreamError, Text};
use alloc::{borrow::Cow, string::String, vec::Vec};
use core::ops::Range;

/// A stable index into a document's contiguous node arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(usize);

/// A semantic AST node. Child and sibling lists use arena indices, not heap objects.
#[derive(Debug)]
pub struct Node {
    /// Node meaning. Container nodes store their start tag.
    pub kind: EventKind<'static>,
    /// Source provenance, which remains unchanged when a value is edited.
    pub span: Range<usize>,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
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
        if event.span.start > event.span.end {
            return Err(StreamError::InvalidRange);
        }
        if let EventKind::End(kind) = event.kind {
            if !event.span.is_empty() {
                return Err(StreamError::InvalidRange);
            }
            let id = self.open.pop().ok_or(StreamError::Unbalanced)?;
            let node = &mut self.nodes[id.0];
            if !matches!(&node.kind, EventKind::Start(tag) if tag.kind() == kind) {
                return Err(StreamError::Unbalanced);
            }
            node.span.end = event.span.end;
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
        let id = NodeId(self.nodes.len());
        let parent = self.open.last().copied();
        let start = matches!(event.kind, EventKind::Start(_));
        self.nodes.push(Node {
            kind: event.kind.map_text(Text::owned),
            span: event.span,
            parent,
            first_child: None,
            last_child: None,
            next_sibling: None,
        });
        if let Some(parent) = parent {
            if let Some(previous) = self.nodes[parent.0].last_child {
                self.nodes[previous.0].next_sibling = Some(id);
            } else {
                self.nodes[parent.0].first_child = Some(id);
            }
            self.nodes[parent.0].last_child = Some(id);
        }
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
        };
        // Public producers can supply arbitrary offsets. Validate their contract once before exposing the document.
        for event in document.events() {
            let event = event?;
            if document.source().get(event.span).is_none() {
                return Err(StreamError::InvalidRange);
            }
            let mut valid = true;
            event.kind.map_text(|text| {
                valid &= text.resolve(document.source()).is_ok();
                Text::Literal("")
            });
            if !valid {
                return Err(StreamError::InvalidRange);
            }
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
#[derive(Debug)]
pub struct Document<S = String> {
    source: S,
    nodes: Vec<Node>,
}

impl<S: AsRef<str>> Document<S> {
    /// Parse an owned or borrowed UTF-8 buffer into an arena document.
    ///
    /// # Errors
    /// Returns errors reported by Markdown extensions.
    pub fn parse(source: S, options: &crate::ParseOptions) -> Result<Self, StreamError> {
        let mut builder = AstBuilder::new();
        {
            let mut events = crate::EventParser::new(source.as_ref(), options)?;
            builder.consume(&mut events, usize::MAX)?;
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
    /// Root node, if the producer emitted one.
    pub fn root(&self) -> Option<NodeId> {
        if self.nodes.is_empty() {
            None
        } else {
            Some(NodeId(0))
        }
    }
    /// Access a node by stable index.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    /// Access node values and provenance for editing. Parent/child/sibling links remain private.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }

    /// Edit the text of a text/code/HTML/opaque leaf. Source-backed values detach on first mutation.
    ///
    /// # Errors
    /// Returns [`StreamError::InvalidRange`] for an invalid node ID or a non-text node.
    pub fn text_mut(&mut self, id: NodeId) -> Result<&mut String, StreamError> {
        let node = self.nodes.get_mut(id.0).ok_or(StreamError::InvalidRange)?;
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
        if let Text::Source(range) = text {
            *text = Text::Value(Cow::Owned(self.source.as_ref()[range.clone()].into()));
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
            current: self.root(),
            closing: false,
        }
    }
}

/// A zero-allocation pull traversal over the arena's parent/child/sibling links.
pub struct DocumentEvents<'a, S> {
    document: &'a Document<S>,
    current: Option<NodeId>,
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
        let node = &self.document.nodes[id.0];
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
