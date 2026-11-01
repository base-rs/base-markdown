// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Pull semantic events from compact source-backed block records.
use crate::block_parser::{Blocks, Body};
use crate::definitions::Definitions;
use crate::inline_parser::InlineParser;
use crate::stream::{Event, EventKind, StreamError, Tag, TagKind, Text};
use crate::util::line_ending::terminal_ending_start;
use crate::value_builder::ValueBuilder;
use crate::{LineEnding, ParseOptions};
use alloc::string::ToString;

/// A pull producer of resolved semantic events. Preparation records block structure; inline syntax is scanned only when its containing block is consumed.
pub struct EventParser<'a> {
    blocks: Blocks,
    source: &'a str,
    options: &'a ParseOptions,
    definitions: Definitions<'a>,
    inline: InlineParser,
    extensions: bool,
    cursor: usize,
    closing: bool,
    phase: Phase,
    // Whether the source has a CR, so that source line endings in values need writing as LF.
    carriage_returns: bool,
}
impl<'a> EventParser<'a> {
    /// Prepare Markdown for pull consumption.
    /// Built-in Markdown preparation has no syntax errors.
    pub fn new(source: &'a str, options: &'a ParseOptions) -> Self {
        let (blocks, definitions) = Blocks::parse(source, options);
        let carriage_returns = memchr::memchr(b'\r', source.as_bytes()).is_some();
        Self::from_blocks(source, options, blocks, definitions, carriage_returns)
    }

    /// Pull events from blocks that have already been prepared from `source`. `carriage_returns` must be true if `source` has a CR; it may be true when it has none, which only costs a check of each value.
    pub(crate) fn from_blocks(
        source: &'a str,
        options: &'a ParseOptions,
        blocks: Blocks,
        definitions: Definitions<'a>,
        carriage_returns: bool,
    ) -> Self {
        Self {
            blocks,
            source,
            options,
            definitions,
            inline: InlineParser::default(),
            extensions: options.constructs != crate::Constructs::default(),
            cursor: 0,
            closing: false,
            phase: Phase::Start,
            carriage_returns,
        }
    }
}
impl EventParser<'_> {
    /// Resolve references and footnote calls in subsequently consumed blocks only against definitions starting before `limit`.
    pub(crate) fn set_definition_limit(&mut self, limit: Option<usize>) {
        self.definitions.set_limit(limit);
    }
}
impl Iterator for EventParser<'_> {
    type Item = Result<Event<'static>, StreamError>;
    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let event = self.next_event();
        if !self.carriage_returns {
            return event;
        }
        // Line endings are structure, so every value stores LF, whichever the source used.
        // Separators are already LF when yielded.
        let source = self.source;
        event.map(|event| {
            event.map(|event| Event {
                kind: event.kind.map_text(|text| text.normalized(source)),
                span: event.span,
            })
        })
    }
}
// Pending separators are delivery states, mutually exclusive with inline consumption.
enum Phase {
    Start,
    Body,
    Inline,
    Separator(usize),
    DocumentEnd,
    Done,
}
impl EventParser<'_> {
    fn prepare_inline<const EXTENSIONS: bool>(&mut self, index: usize, task: bool, table: bool) {
        let fragments = self
            .blocks
            .children(index)
            .map(|(_, node)| node.start..node.end);
        if self.definitions.is_empty() {
            self.inline.prepare::<false, EXTENSIONS>(
                self.source,
                fragments,
                &self.definitions,
                self.options,
                task,
                table,
            );
        } else {
            self.inline.prepare::<true, EXTENSIONS>(
                self.source,
                fragments,
                &self.definitions,
                self.options,
                task,
                table,
            );
        }
    }
    fn advance(&mut self) {
        let node = self.blocks.nodes[self.cursor];
        if node.next != 0 {
            self.cursor = node.next;
            self.closing = false;
        } else {
            self.cursor = node.parent;
            self.closing = true;
        }
    }
    fn value(&self, index: usize) -> Text<'static> {
        let mut value = ValueBuilder::new(self.source);
        for (_, node) in self.blocks.children(index) {
            let Body::Fragment(spaces) = node.body else {
                unreachable!("literal block children are source fragments");
            };
            if spaces > 0 {
                value.append(Text::Literal(&"   "[..spaces]));
            }
            value.append(Text::Source(node.start..node.end));
        }
        value.finish()
    }
    fn separator(&self, end: usize) -> Event<'static> {
        let bytes = self.source.as_bytes();
        // Block boundaries passed here end at a physical line ending, never at container padding or a trimmed inline fragment.
        // Like every value, the separator stores LF, so it points at the source only when the source has LF.
        let value = match bytes[end - 1] {
            b'\n' if end == 1 || bytes[end - 2] != b'\r' => Text::Source(end - 1..end),
            b'\n' | b'\r' => Text::Literal("\n"),
            _ => unreachable!("authored block separators end at a source newline"),
        };
        Event {
            kind: EventKind::Separator {
                value,
                conditional: false,
            },
            span: end..end,
        }
    }
    fn end_document(&mut self) -> Event<'static> {
        self.phase = Phase::Done;
        Event {
            kind: EventKind::End(TagKind::Document),
            span: self.source.len()..self.source.len(),
        }
    }
    // Document bookends are infrequent; keep their state transitions out of the hot body loop.
    #[cold]
    fn document_boundary(&mut self) -> Option<Result<Event<'static>, StreamError>> {
        match self.phase {
            Phase::Start => {
                self.phase = Phase::Body;
                self.cursor = self.blocks.nodes[0].child;
                self.closing = self.cursor == 0;
                // Parsed values store every line ending as LF, so the output line ending is LF too, and
                // only a document without line endings leaves it to the renderer's default.
                let line_ending = memchr::memchr2(b'\r', b'\n', self.source.as_bytes())
                    .map(|_| LineEnding::LineFeed);
                Some(Ok(Event {
                    kind: EventKind::Start(Tag::Document(line_ending)),
                    span: 0..self.source.len(),
                }))
            }
            Phase::DocumentEnd => Some(Ok(self.end_document())),
            Phase::Done => None,
            Phase::Body => {
                self.phase = Phase::DocumentEnd;
                let last = self.blocks.children(0).last().map_or(0, |(index, _)| index);
                let mut leaf = last;
                while matches!(
                    self.blocks.nodes[leaf].body,
                    Body::Quote | Body::List { .. } | Body::Item(_)
                ) {
                    let Some((child, _)) = self.blocks.children(leaf).last() else {
                        break;
                    };
                    leaf = child;
                }
                let last = self.blocks.nodes[last];
                let leaf = self.blocks.nodes[leaf];
                let populated = matches!(
                    leaf.body,
                    Body::Paragraph
                        | Body::Heading(_)
                        | Body::Rule
                        | Body::Code(_)
                        | Body::Html(_)
                        | Body::Table
                );
                let suppress_separator = match leaf.body {
                    Body::Html(end) => end == leaf.end && leaf.end == self.source.len(),
                    Body::Code(metadata) if leaf.parent == 0 => {
                        let code = &self.blocks.codes[metadata];
                        code.unclosed && leaf.end == self.source.len()
                            || code.fence.is_none()
                                && self.source[leaf.end..].trim().is_empty()
                                && !self.source.ends_with(['\r', '\n'])
                    }
                    _ => false,
                };
                if !suppress_separator
                    && self.blocks.nodes[0].child != 0
                    && (!matches!(last.body, Body::Definition(_)) || last.end < self.source.len())
                    && (self.source.ends_with(['\r', '\n'])
                        || (matches!(
                            last.body,
                            Body::Paragraph | Body::Heading(_) | Body::Rule | Body::Table
                        ) || last.body == Body::Quote && populated)
                            && self.source[..last.end]
                                .trim_end_matches([' ', '\t'])
                                .ends_with(['\r', '\n']))
                {
                    // Populated containers retain their final semantic line ending. Empty containers and lists followed by blank lines use the document style.
                    return Some(Ok(
                        if !populated
                            || matches!(last.body, Body::List { .. })
                                && leaf.end != self.source.len()
                            || leaf.body == Body::Table && last.end > leaf.end
                        {
                            Event {
                                kind: EventKind::Separator {
                                    value: Text::Literal(""),
                                    conditional: true,
                                },
                                span: last.end..last.end,
                            }
                        } else {
                            self.separator(leaf.end)
                        },
                    ));
                }
                Some(Ok(self.end_document()))
            }
            Phase::Inline | Phase::Separator(_) => {
                unreachable!("active body delivery bypasses document boundary handling")
            }
        }
    }
    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn next_event(&mut self) -> Option<Result<Event<'static>, StreamError>> {
        match self.phase {
            Phase::Inline => {
                if let Some(event) = self
                    .inline
                    .next(self.source, &self.definitions, self.options)
                    .map(Ok)
                {
                    return Some(event);
                }
                self.phase = Phase::Body;
                self.closing = true;
            }
            Phase::Body => {}
            Phase::Separator(end) => {
                self.phase = Phase::Body;
                return Some(Ok(self.separator(end)));
            }
            _ => return self.document_boundary(),
        }
        if self.cursor == 0 {
            return self.document_boundary();
        }
        let index = self.cursor;
        let node = self.blocks.nodes[index];
        if self.closing {
            let tag = match node.body {
                Body::Paragraph => TagKind::Paragraph,
                Body::Heading(_) => TagKind::Heading,
                Body::Quote => TagKind::BlockQuote,
                Body::Footnote(_) => TagKind::FootnoteDefinition,
                Body::List { .. } => TagKind::List,
                Body::Item(_) => TagKind::Item,
                Body::Table => TagKind::Table,
                Body::TableHead => TagKind::TableHead,
                Body::TableBody => TagKind::TableBody,
                Body::TableRow => TagKind::TableRow,
                Body::TableCell { .. } => TagKind::TableCell,
                _ => unreachable!("only block containers yield closing events"),
            };
            // Definitions are invisible; a following visible block terminates the output itself.
            if !self.blocks.tight_paragraph(&node)
                && !matches!(
                    node.body,
                    Body::Item(_) | Body::Footnote(_) | Body::TableCell { .. }
                )
                && (node.next != 0
                    || node.body == Body::TableRow
                        && self.blocks.nodes[node.parent].body == Body::TableHead
                    || node.body == Body::Table
                        && node.parent != 0
                        && self.blocks.nodes[node.parent].end > node.end)
                && !(matches!(node.body, Body::List { .. })
                    && matches!(self.blocks.nodes[node.next].body, Body::Definition(_)))
            {
                self.phase = Phase::Separator(node.end);
            }
            self.advance();
            let end = node.span_end(self.source);
            return Some(Ok(Event {
                kind: EventKind::End(tag),
                span: end..end,
            }));
        }
        let kind = match node.body {
            Body::Paragraph | Body::Heading(_) | Body::TableCell { .. } => {
                let nodes = &self.blocks.nodes;
                let task = self.options.constructs.task_list_item
                    && matches!(node.body, Body::Paragraph)
                    && matches!(nodes[node.parent].body, Body::Item(_))
                    && self.source[node.start..node.end].starts_with('[')
                    && self
                        .blocks
                        .children(node.parent)
                        .take_while(|&(before, _)| before != index)
                        .all(|(_, child)| matches!(child.body, Body::Definition(_)));
                let table = matches!(node.body, Body::TableCell { .. });
                if self.extensions {
                    self.prepare_inline::<true>(index, task, table);
                } else {
                    self.prepare_inline::<false>(index, task, table);
                }
                self.phase = Phase::Inline;
                match node.body {
                    Body::Heading(level) => EventKind::Start(Tag::Heading(level)),
                    Body::Paragraph => EventKind::Start(Tag::Paragraph {
                        tight: self.blocks.tight_paragraph(&node),
                    }),
                    Body::TableCell { alignment, header } => {
                        EventKind::Start(Tag::TableCell { alignment, header })
                    }
                    _ => unreachable!("this branch handles inline block containers"),
                }
            }
            Body::Quote => EventKind::Start(Tag::BlockQuote),
            Body::Footnote(index) => EventKind::Start(Tag::FootnoteDefinition(Text::from(
                self.definitions.footnote_key(index).to_string(),
            ))),
            Body::Opaque => {
                self.advance();
                EventKind::Opaque {
                    value: Text::Source(node.start..node.end),
                    block: true,
                }
            }
            Body::Table => EventKind::Start(Tag::Table),
            Body::TableHead => EventKind::Start(Tag::TableHead),
            Body::TableBody => EventKind::Start(Tag::TableBody),
            Body::TableRow => EventKind::Start(Tag::TableRow),
            Body::List {
                tight,
                marker,
                start,
            } => EventKind::Start(Tag::List {
                tight,
                start: matches!(marker, b'.' | b')').then_some(start),
            }),
            Body::Item(_) => {
                let block_end = self
                    .blocks
                    .children(index)
                    .filter(|(_, child)| {
                        !matches!(child.body, Body::Definition(_) | Body::Footnote(_))
                    })
                    .last()
                    .is_some_and(|(_, child)| !self.blocks.tight_paragraph(child));
                EventKind::Start(Tag::Item { block_end })
            }
            Body::Rule => {
                if node.next != 0 {
                    self.phase = Phase::Separator(node.end);
                }
                self.advance();
                EventKind::Rule
            }
            Body::Definition(index) => {
                let definition = self.definitions.entry(index);
                let kind = EventKind::Definition {
                    label: definition.label.clone(),
                    destination: definition.destination.clone(),
                    title: definition.title.clone(),
                };
                self.advance();
                let end = terminal_ending_start(self.source.as_bytes(), node.end);
                return Some(Ok(Event {
                    kind,
                    span: node.start..end,
                }));
            }
            Body::Html(end) => {
                let kind = EventKind::Html {
                    value: self.value(index),
                    block: true,
                };
                if node.next != 0 && end < node.end {
                    self.phase = Phase::Separator(node.end);
                }
                self.advance();
                return Some(Ok(Event {
                    kind,
                    span: node.start..end,
                }));
            }
            Body::Code(metadata) => {
                let code = &self.blocks.codes[metadata];
                let value = self.value(index);
                let kind = EventKind::CodeBlock {
                    value,
                    language: if code.info.is_empty() {
                        None
                    } else {
                        Some(crate::value_builder::decode_string_with_options(
                            self.source,
                            code.info.clone(),
                            self.options,
                        ))
                    },
                    fence: code.fence,
                    math: code.math,
                    unclosed: code.unclosed,
                };
                if node.next != 0 && !code.unclosed {
                    self.phase = Phase::Separator(node.end);
                }
                self.advance();
                kind
            }
            _ => unreachable!("root and source fragments are not semantic blocks"),
        };
        if matches!(
            node.body,
            Body::Quote
                | Body::Footnote(_)
                | Body::List { .. }
                | Body::Item(_)
                | Body::Table
                | Body::TableHead
                | Body::TableBody
                | Body::TableRow
        ) {
            if node.child == 0 {
                self.closing = true;
            } else {
                self.cursor = node.child;
            }
        }
        Some(Ok(Event {
            kind,
            span: node.start..node.span_end(self.source),
        }))
    }
}
