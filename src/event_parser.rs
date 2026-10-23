// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Pull semantic events from compact source-backed block records.
use crate::block_parser::{Blocks, Body};
use crate::grammar_parser::{GrammarParser, ValueBuilder};
use crate::stream::{Event, EventKind, StreamError, Tag, TagKind, Text};
use crate::{LineEnding, ParseOptions};
use alloc::boxed::Box;

/// A pull producer of resolved semantic events. Preparation records block structure; inline syntax is scanned only when its containing block is consumed.
pub struct EventParser<'a> {
    producer: Producer<'a>,
}
// Keep the usual producer inline rather than allocating a box for every parse.
#[allow(clippy::large_enum_variant)]
enum Producer<'a> {
    Compact(CompactParser<'a>),
    Compatibility(Box<GrammarParser<'a>>),
}
impl<'a> EventParser<'a> {
    /// Prepare Markdown for pull consumption.
    ///
    /// # Errors
    /// Returns syntax errors from enabled Markdown extensions.
    pub fn new(source: &'a str, options: &'a ParseOptions) -> Result<Self, StreamError> {
        let producer = if let Some((blocks, definitions)) = Blocks::parse(source, options) {
            Producer::Compact(CompactParser {
                blocks,
                source,
                inline: GrammarParser::empty(source, options, definitions),
                compact_inline: crate::inline_parser::InlineParser::default(),
                direct_inline: false,
                options,
                cursor: 0,
                closing: false,
                inlines: false,
                started: false,
                ended: false,
                final_separator: false,
                suppress_final_separator: false,
                separator: None,
            })
        } else {
            Producer::Compatibility(Box::new(GrammarParser::new(source, options)?))
        };
        Ok(Self { producer })
    }
}
impl Iterator for EventParser<'_> {
    type Item = Result<Event<'static>, StreamError>;
    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.producer {
            Producer::Compact(parser) => parser.next(),
            Producer::Compatibility(parser) => parser.next(),
        }
    }
}
#[allow(clippy::struct_excessive_bools)]
struct CompactParser<'a> {
    blocks: Blocks,
    source: &'a str,
    inline: GrammarParser<'a>,
    compact_inline: crate::inline_parser::InlineParser,
    direct_inline: bool,
    options: &'a ParseOptions,
    cursor: usize,
    closing: bool,
    inlines: bool,
    started: bool,
    ended: bool,
    final_separator: bool,
    suppress_final_separator: bool,
    separator: Option<usize>,
}
impl CompactParser<'_> {
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
        let mut child = self.blocks.nodes[index].child;
        while child != 0 {
            let node = self.blocks.nodes[child];
            let Body::Fragment(spaces) = node.body else {
                unreachable!("literal block children are source fragments");
            };
            if spaces > 0 {
                value.append(Text::Literal(&"   "[..spaces]));
            }
            value.append(Text::Source(node.start..node.end));
            child = node.next;
        }
        value.finish()
    }
    fn separator(&self, end: usize) -> Event<'static> {
        let bytes = self.source.as_bytes();
        // Block boundaries passed here end at a physical line ending, never at container padding or a trimmed inline fragment.
        let start = match bytes[end - 1] {
            b'\n' => end - 1 - usize::from(end > 1 && bytes[end - 2] == b'\r'),
            b'\r' => end - 1,
            _ => unreachable!("authored block separators end at a source newline"),
        };
        Event {
            kind: EventKind::Separator {
                value: Text::Source(start..end),
                conditional: false,
            },
            span: end..end,
        }
    }
    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn next(&mut self) -> Option<Result<Event<'static>, StreamError>> {
        if self.ended {
            return None;
        }
        if let Some(end) = self.separator.take() {
            return Some(Ok(self.separator(end)));
        }
        if self.inlines {
            if let Some(event) = if self.direct_inline {
                self.compact_inline
                    .next(self.source, self.inline.definitions())
                    .map(Ok)
            } else {
                self.inline.next()
            } {
                return Some(event);
            }
            self.inlines = false;
            self.closing = true;
        }
        if !self.started {
            self.started = true;
            self.cursor = self.blocks.nodes[0].child;
            self.closing = self.cursor == 0;
            let line_ending = self.source.find(['\r', '\n']).map(|index| {
                if self.source.as_bytes()[index] == b'\n' {
                    LineEnding::LineFeed
                } else if self.source.as_bytes().get(index + 1) == Some(&b'\n') {
                    LineEnding::CarriageReturnLineFeed
                } else {
                    LineEnding::CarriageReturn
                }
            });
            return Some(Ok(Event {
                kind: EventKind::Start(Tag::Document(line_ending)),
                span: 0..self.source.len(),
            }));
        }
        if self.cursor == 0 {
            let mut last = self.blocks.nodes[0].child;
            while self.blocks.nodes[last].next != 0 {
                last = self.blocks.nodes[last].next;
            }
            let last = self.blocks.nodes[last];
            let mut leaf = last;
            while matches!(leaf.body, Body::Quote | Body::List { .. } | Body::Item(_))
                && leaf.child != 0
            {
                let mut child = leaf.child;
                while self.blocks.nodes[child].next != 0 {
                    child = self.blocks.nodes[child].next;
                }
                leaf = self.blocks.nodes[child];
            }
            let populated = matches!(
                leaf.body,
                Body::Paragraph(_) | Body::Heading(_) | Body::Rule | Body::Code(_) | Body::Html(_)
            );
            if !self.final_separator
                && !self.suppress_final_separator
                && self.blocks.nodes[0].child != 0
                && (!matches!(last.body, Body::Definition(_)) || last.end < self.source.len())
                && (self.source.ends_with(['\r', '\n'])
                    || (matches!(
                        last.body,
                        Body::Paragraph(_) | Body::Heading(_) | Body::Rule
                    ) || last.body == Body::Quote && populated)
                        && self.source[..last.end]
                            .trim_end_matches([' ', '\t'])
                            .ends_with(['\r', '\n']))
            {
                self.final_separator = true;
                // Populated containers retain their final semantic line ending. Empty containers and lists followed by blank lines use the document style.
                return Some(Ok(
                    if !populated
                        || matches!(last.body, Body::List { .. }) && leaf.end != self.source.len()
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
            self.ended = true;
            return Some(Ok(Event {
                kind: EventKind::End(TagKind::Document),
                span: self.source.len()..self.source.len(),
            }));
        }
        let index = self.cursor;
        let node = self.blocks.nodes[index];
        if self.closing {
            let tag = match node.body {
                Body::Paragraph(_) => TagKind::Paragraph,
                Body::Heading(_) => TagKind::Heading,
                Body::Quote => TagKind::BlockQuote,
                Body::List { .. } => TagKind::List,
                Body::Item(_) => TagKind::Item,
                _ => unreachable!("only block containers yield closing events"),
            };
            // Definitions are invisible. The grammar omits a list's boundary newline before them; a following visible block terminates the output itself.
            if !matches!(node.body, Body::Paragraph(true) | Body::Item(_))
                && node.next != 0
                && !(matches!(node.body, Body::List { .. })
                    && matches!(self.blocks.nodes[node.next].body, Body::Definition(_)))
            {
                self.separator = Some(node.end);
            }
            self.advance();
            return Some(Ok(Event {
                kind: EventKind::End(tag),
                span: node.end..node.end,
            }));
        }
        let kind = match node.body {
            Body::Paragraph(_) | Body::Heading(_) => {
                let nodes = &self.blocks.nodes;
                let fragments = || {
                    let mut child = node.child;
                    core::iter::from_fn(move || {
                        if child == 0 {
                            None
                        } else {
                            let node = nodes[child];
                            child = node.next;
                            Some(node.start..node.end)
                        }
                    })
                };
                self.direct_inline = !self.options.inline_code_info
                    && if self.inline.definitions().is_empty() {
                        self.compact_inline.prepare::<false, false>(
                            self.source,
                            fragments(),
                            self.inline.definitions(),
                            self.options,
                        )
                    } else {
                        self.compact_inline.prepare::<true, false>(
                            self.source,
                            fragments(),
                            self.inline.definitions(),
                            self.options,
                        )
                    };
                if !self.direct_inline {
                    self.inline.region(fragments());
                }
                self.inlines = true;
                match node.body {
                    Body::Heading(level) => EventKind::Start(Tag::Heading(level)),
                    Body::Paragraph(tight) => EventKind::Start(Tag::Paragraph { tight }),
                    _ => unreachable!("this branch handles inline block containers"),
                }
            }
            Body::Quote => EventKind::Start(Tag::BlockQuote),
            Body::List {
                tight,
                marker,
                start,
            } => EventKind::Start(Tag::List {
                tight,
                start: matches!(marker, b'.' | b')').then_some(start),
            }),
            Body::Item(_) => {
                let mut child = node.child;
                let mut last = 0;
                while child != 0 {
                    if !matches!(self.blocks.nodes[child].body, Body::Definition(_)) {
                        last = child;
                    }
                    child = self.blocks.nodes[child].next;
                }
                let block_end = last != 0 && self.blocks.nodes[last].body != Body::Paragraph(true);
                EventKind::Start(Tag::Item { block_end })
            }
            Body::Rule => {
                if node.next != 0 {
                    self.separator = Some(node.end);
                }
                self.advance();
                EventKind::Rule
            }
            Body::Definition(index) => {
                let definition = &self.inline.definitions().entries[index];
                let kind = EventKind::Definition {
                    label: definition.label.clone(),
                    destination: definition.destination.clone(),
                    title: definition.title.clone(),
                };
                self.advance();
                let mut end = node.end;
                if self.source.as_bytes().get(end - 1) == Some(&b'\n') {
                    end -= 1;
                }
                if self.source.as_bytes().get(end - 1) == Some(&b'\r') {
                    end -= 1;
                }
                return Some(Ok(Event {
                    kind,
                    span: node.start..end,
                }));
            }
            Body::Html(end) => {
                if end == node.end && node.end == self.source.len() {
                    self.suppress_final_separator = true;
                }
                let kind = EventKind::Html {
                    value: self.value(index),
                    block: true,
                };
                if node.next != 0 {
                    self.separator = Some(node.end);
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
                if code.unclosed && node.parent == 0 && node.end == self.source.len()
                    || code.fence.is_none()
                        && node.parent == 0
                        && self.source[node.end..].trim().is_empty()
                        && !self.source.ends_with(['\r', '\n'])
                {
                    self.suppress_final_separator = true;
                }
                let kind = EventKind::CodeBlock {
                    value,
                    language: if code.info.is_empty() {
                        None
                    } else {
                        Some(crate::grammar_parser::decode_string(
                            self.source,
                            code.info.clone(),
                        ))
                    },
                    fence: code.fence,
                    math: false,
                    unclosed: code.unclosed,
                };
                if node.next != 0 {
                    self.separator = Some(node.end);
                }
                if code.unclosed {
                    self.separator = None;
                }
                self.advance();
                kind
            }
            _ => unreachable!("root and source fragments are not semantic blocks"),
        };
        if matches!(node.body, Body::Quote | Body::List { .. } | Body::Item(_)) {
            if node.child == 0 {
                self.closing = true;
            } else {
                self.cursor = node.child;
            }
        }
        Some(Ok(Event {
            kind,
            span: node.start..node.end,
        }))
    }
}
