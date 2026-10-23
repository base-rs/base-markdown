// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Compatibility producer: turn resolved grammar tokens into semantic events on demand.
use crate::event::{Content, Event as Token, Kind, Name};
use crate::parser::ParseState;
use crate::state::{Name as StateName, State};
use crate::stream::{Event, EventKind, StreamError, Tag, TagKind, Text};
use crate::tokenizer::{Tokenizer, TokenizerBuffers};
use crate::util::{
    constant::CHARACTER_REFERENCES,
    infer,
    normalize_identifier::normalize_identifier,
    slice::{Position, Slice},
};
use crate::{
    mdast::{AlignKind, CodeFence},
    LineEnding, ParseOptions,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{convert::TryFrom, ops::Range};

#[derive(Debug)]
struct Definition {
    key: String,
    destination: Text<'static>,
    title: Option<Text<'static>>,
}

/// A pull producer of resolved semantic events.
///
/// The constructor prepares block structure and definitions. Inline regions are resolved on demand as events are pulled, reusing scratch allocations without materializing document-wide inline tokens. Ordinary text stays source-backed. The semantic stream is never collected internally.
#[allow(clippy::struct_excessive_bools)]
pub struct EventParser<'a> {
    source: &'a str,
    parse_state: ParseState<'a>,
    block_tokens: Option<(Vec<Token>, Vec<usize>, usize)>,
    buffers: TokenizerBuffers,
    inline_matching: Vec<usize>,
    matching_stack: Vec<usize>,
    options: &'a ParseOptions,
    tokens: Vec<Token>,
    matching: Vec<usize>,
    definitions: Vec<Definition>,
    index: usize,
    started: bool,
    ended: bool,
    slurp: bool,
    tight: Vec<bool>,
    containers: Vec<TagKind>,
    align: Vec<AlignKind>,
    column: usize,
    header: bool,
    line_ending: Option<LineEnding>,
    pending: Option<Event<'static>>,
    synthetic_end: Option<usize>,
    padding_end: Option<usize>,
}

impl<'a> EventParser<'a> {
    /// Prepare Markdown for pull consumption.
    ///
    /// # Errors
    /// Returns syntax errors from enabled Markdown extensions.
    pub fn new(source: &'a str, options: &'a ParseOptions) -> Result<Self, StreamError> {
        let (mut tokens, parse_state) = crate::parser::prepare(source, options)?;
        // Definition destinations and code-info strings are small and needed before their opening semantic events.
        crate::subtokenize::subtokenize(&mut tokens, &parse_state, Some(&Content::String))?;
        let mut matching = Vec::new();
        let mut stack = Vec::new();
        pair_tokens(&tokens, &mut matching, &mut stack)?;
        let line_ending = source.find(['\r', '\n']).map(|index| {
            if source.as_bytes()[index] == b'\n' {
                LineEnding::LineFeed
            } else if source.as_bytes().get(index + 1) == Some(&b'\n') {
                LineEnding::CarriageReturnLineFeed
            } else {
                LineEnding::CarriageReturn
            }
        });
        let mut parser = Self {
            source,
            parse_state,
            block_tokens: None,
            buffers: TokenizerBuffers::default(),
            inline_matching: Vec::new(),
            matching_stack: stack,
            options,
            tokens,
            matching,
            definitions: Vec::new(),
            index: 0,
            started: false,
            ended: false,
            slurp: false,
            tight: Vec::new(),
            containers: Vec::new(),
            align: Vec::new(),
            column: 0,
            header: false,
            line_ending,
            pending: None,
            synthetic_end: None,
            padding_end: None,
        };
        for index in 0..parser.tokens.len() {
            if parser.tokens[index].kind == Kind::Enter
                && parser.tokens[index].name == Name::Definition
            {
                let (label, destination, title) = parser.definition(index);
                let key = normalize_identifier(label.resolve(source)?);
                if !parser
                    .definitions
                    .iter()
                    .any(|definition| definition.key == key)
                {
                    parser.definitions.push(Definition {
                        key,
                        destination,
                        title,
                    });
                }
            }
        }
        Ok(parser)
    }

    fn inline(&mut self, index: usize) -> Result<(), StreamError> {
        let mut task = false;
        if self.options.constructs.gfm_task_list_item
            && index > 2
            && self.tokens[index - 1].kind == Kind::Enter
            && self.tokens[index - 1].name == Name::Paragraph
        {
            let before = crate::util::skip::opt_back(
                &self.tokens,
                index - 2,
                &[
                    Name::BlankLineEnding,
                    Name::Definition,
                    Name::LineEnding,
                    Name::SpaceOrTab,
                ],
            );
            task = self.tokens[before].kind == Kind::Exit
                && self.tokens[before].name == Name::ListItemPrefix;
        }
        let mut tokenizer = Tokenizer::with_buffers(
            self.tokens[index].point.clone(),
            &self.parse_state,
            core::mem::take(&mut self.buffers),
        );
        tokenizer
            .tokenize_state
            .document_at_first_paragraph_of_list_item = task;
        let mut state = State::Next(StateName::TextStart);
        let mut current = Some(index);
        while let Some(current_index) = current {
            let enter = &self.tokens[current_index];
            let link = enter.link.as_ref().unwrap();
            if link.previous().is_some() {
                tokenizer.define_skip(enter.point.clone());
            }
            let end = &self.tokens[current_index + 1].point;
            state = tokenizer.push(
                (enter.point.index, enter.point.vs),
                (end.index, end.vs),
                state,
            );
            current = link.next();
        }
        tokenizer.flush(state, true)?;
        // Resolve only the active inline region, never splice it into the block buffer.
        crate::subtokenize::subtokenize(&mut tokenizer.events, &self.parse_state, None)?;
        self.buffers = tokenizer.into_buffers();
        let tokens = core::mem::take(&mut self.buffers.events);
        let mut matching = core::mem::take(&mut self.inline_matching);
        pair_tokens(&tokens, &mut matching, &mut self.matching_stack)?;
        let blocks = core::mem::replace(&mut self.tokens, tokens);
        let block_matching = core::mem::replace(&mut self.matching, matching);
        self.block_tokens = Some((blocks, block_matching, self.index));
        self.index = 0;
        Ok(())
    }

    fn span(&self, index: usize) -> Range<usize> {
        let end = self.matching[index];
        self.tokens[index].point.index..self.tokens[end].point.index
    }

    fn named(&self, start: usize, name: &Name) -> Option<usize> {
        let mut index = start + 1;
        while index < self.matching[start] {
            if self.tokens[index].kind == Kind::Enter {
                if &self.tokens[index].name == name {
                    return Some(index);
                }
                if matches!(self.tokens[index].name, Name::Image | Name::Link) {
                    index = self.matching[index] + 1;
                    continue;
                }
            }
            index += 1;
        }
        None
    }

    fn text(&self, start: usize) -> Text<'static> {
        let end = self.matching[start];
        let token = &self.tokens[start];
        let start = &token.point;
        let end = &self.tokens[end].point;
        if matches!(token.name, Name::CodeFlowChunk | Name::MathFlowChunk)
            && (start.vs > 0 || end.vs > 0)
        {
            return Text::from(
                Slice::from_position(self.source.as_bytes(), &Position { start, end }).serialize(),
            );
        }
        // Virtual spaces are consumed container indentation, not authored text.
        Text::Source(start.index + usize::from(start.vs > 0)..end.index + usize::from(end.vs > 0))
    }

    fn entity(&self, range: Range<usize>) -> Text<'static> {
        let value = &self.source[range];
        let raw = &value[1..value.len() - 1];
        if let Some(number) = raw.strip_prefix('#') {
            let (number, radix) = if let Some(hex) = number
                .strip_prefix('x')
                .or_else(|| number.strip_prefix('X'))
            {
                (hex, 16)
            } else {
                (number, 10)
            };
            Text::from(crate::util::character_reference::decode_numeric(
                number, radix,
            ))
        } else {
            let index = CHARACTER_REFERENCES
                .binary_search_by_key(&raw, |entry| entry.0)
                .expect("resolved named reference exists in the entity table");
            Text::Literal(CHARACTER_REFERENCES[index].1)
        }
    }

    fn value(&self, start: usize, data: &[Name], line_endings: bool) -> Text<'static> {
        let mut value = ValueBuilder::new(self.source);
        let mut index = start + 1;
        while index < self.matching[start] {
            if self.tokens[index].kind == Kind::Enter {
                let name = &self.tokens[index].name;
                if *name == Name::CharacterReference {
                    value.append(self.entity(self.span(index)));
                    index = self.matching[index] + 1;
                    continue;
                }
                if data.contains(name) || (line_endings && *name == Name::LineEnding) {
                    value.append(self.text(index));
                }
            }
            index += 1;
        }
        value.finish()
    }

    fn string(&self, index: usize) -> Text<'static> {
        let value = self.value(index, &[Name::Data, Name::CharacterEscapeValue], true);
        if value.resolve(self.source).unwrap().contains('\0') {
            Text::from(value.resolve(self.source).unwrap().replace('\0', "�"))
        } else {
            value
        }
    }

    fn definition(&self, index: usize) -> (Text<'static>, Text<'static>, Option<Text<'static>>) {
        let label = self
            .named(index, &Name::DefinitionLabelString)
            .map_or(Text::Literal(""), |i| self.text(i));
        let destination = self
            .named(index, &Name::DefinitionDestinationString)
            .map_or(Text::Literal(""), |i| self.string(i));
        let title = self
            .named(index, &Name::DefinitionTitleString)
            .map(|i| self.string(i));
        (label, destination, title)
    }

    fn media(&self, index: usize) -> (Text<'static>, Option<Text<'static>>) {
        if let Some(resource) = self.named(index, &Name::Resource) {
            let destination = self
                .named(resource, &Name::ResourceDestinationString)
                .map_or(Text::Literal(""), |i| self.string(i));
            let title = self
                .named(resource, &Name::ResourceTitleString)
                .map(|i| self.string(i));
            (destination, title)
        } else {
            let label = self
                .named(index, &Name::ReferenceString)
                .or_else(|| self.named(index, &Name::LabelText))
                .expect("resolved reference has a label");
            let key = normalize_identifier(&self.source[self.span(label)]);
            let definition = self
                .definitions
                .iter()
                .find(|definition| definition.key == key)
                .expect("resolved link has a definition");
            (definition.destination.clone(), definition.title.clone())
        }
    }

    fn code(&self, index: usize, block: bool, math: bool) -> EventKind<'static> {
        let data = if block {
            if math {
                Name::MathFlowChunk
            } else {
                Name::CodeFlowChunk
            }
        } else if math {
            Name::MathTextData
        } else {
            Name::CodeTextData
        };
        let mut value = self.value(index, &[data], true);
        let mut language = if block {
            self.named(index, &Name::CodeFencedFenceInfo)
                .map(|i| self.string(i))
        } else {
            None
        };
        if !block {
            let string = value.resolve(self.source).unwrap();
            let trim = string.len() > 2
                && string.starts_with(' ')
                && string.ends_with(' ')
                && string.bytes().any(|byte| byte != b' ');
            let normalize =
                string.contains(['\r', '\n']) || (!self.align.is_empty() && string.contains("\\|"));
            if normalize {
                let mut normalized = string.replace("\r\n", " ").replace(['\r', '\n'], " ");
                if !self.align.is_empty() {
                    normalized = normalized.replace("\\|", "|");
                }
                if normalized.len() > 2
                    && normalized.starts_with(' ')
                    && normalized.ends_with(' ')
                    && normalized.bytes().any(|byte| byte != b' ')
                {
                    normalized.remove(0);
                    normalized.pop();
                }
                value = Text::from(normalized);
            } else if trim {
                value = match value {
                    Text::Source(range) => Text::Source(range.start + 1..range.end - 1),
                    _ => Text::from(string[1..string.len() - 1].to_string()),
                };
            }
            if self.options.inline_code_info {
                language = crate::util::inline_code_info::before(
                    self.source.as_bytes(),
                    self.tokens[index].point.index,
                )
                .map(|(start, info)| Text::Source(start..start + info.len()));
            }
            return if math {
                EventKind::Math(value)
            } else {
                EventKind::Code { value, language }
            };
        }
        let name = &self.tokens[index].name;
        let fenced = matches!(name, Name::CodeFenced | Name::MathFlow);
        let fence = if fenced {
            let start = self.tokens[index].point.index;
            let byte = self.source.as_bytes()[start];
            let indent = if index > 0 && self.tokens[index - 1].name == Name::SpaceOrTab {
                self.tokens[index - 1].point.column - self.tokens[index - 2].point.column
            } else {
                0
            };
            Some(CodeFence {
                character: char::from(byte),
                length: self.source.as_bytes()[start..]
                    .iter()
                    .take_while(|&&b| b == byte)
                    .count(),
                indent,
            })
        } else {
            None
        };
        let fence_count = (index + 1..self.matching[index])
            .filter(|&i| {
                self.tokens[i].kind == Kind::Enter
                    && matches!(
                        self.tokens[i].name,
                        Name::CodeFencedFence | Name::MathFlowFence
                    )
            })
            .count();
        // Opening and closing fence line endings are syntax, not code content.
        if fenced {
            let mut body = ValueBuilder::new(self.source);
            let mut seen = false;
            let mut i = index + 1;
            while i < self.matching[index] {
                if self.tokens[i].kind == Kind::Enter {
                    if matches!(
                        self.tokens[i].name,
                        Name::CodeFencedFence | Name::MathFlowFence
                    ) {
                        i = self.matching[i] + 1;
                        if !seen {
                            seen = true;
                            if i < self.matching[index] && self.tokens[i].name == Name::LineEnding {
                                i += 2;
                            }
                        }
                        continue;
                    }
                    if matches!(
                        self.tokens[i].name,
                        Name::CodeFlowChunk | Name::MathFlowChunk | Name::LineEnding
                    ) {
                        body.append(self.text(i));
                    }
                }
                i += 1;
            }
            value = body.finish();
        }
        if fenced
            && fence_count < 2
            && !self.tight.is_empty()
            && !matches!(
                self.tokens[self.matching[index] - 1].name,
                Name::CodeFencedFence | Name::MathFlowFence
            )
        {
            let mut body = ValueBuilder::new(self.source);
            body.append(value);
            body.append(Text::Literal(match self.line_ending {
                Some(LineEnding::CarriageReturn) => "\r",
                Some(LineEnding::CarriageReturnLineFeed) => "\r\n",
                _ => "\n",
            }));
            value = body.finish();
        }
        EventKind::CodeBlock {
            value,
            language,
            fence,
            math,
            unclosed: fenced && fence_count < 2,
        }
    }

    fn start(&mut self, index: usize) -> Option<Tag<'static>> {
        match self.tokens[index].name {
            Name::Paragraph => Some(Tag::Paragraph {
                tight: self.tight.last().copied().unwrap_or(false),
            }),
            Name::HeadingAtx => Some(Tag::Heading(
                self.named(index, &Name::HeadingAtxSequence)
                    .map_or(1, |i| u8::try_from(self.span(i).len()).unwrap()),
            )),
            Name::HeadingSetext => Some(Tag::Heading(
                if self
                    .named(index, &Name::HeadingSetextUnderlineSequence)
                    .is_some_and(|i| self.source.as_bytes()[self.tokens[i].point.index] == b'-')
                {
                    2
                } else {
                    1
                },
            )),
            Name::BlockQuote => {
                self.tight.push(false);
                Some(Tag::BlockQuote)
            }
            Name::ListOrdered | Name::ListUnordered => {
                let tight = !infer::list_loose(&self.tokens, index, true);
                self.tight.push(tight);
                let start = if self.tokens[index].name == Name::ListOrdered {
                    Some(
                        self.named(index, &Name::ListItemValue)
                            .map_or(1, |i| self.source[self.span(i)].parse().unwrap()),
                    )
                } else {
                    None
                };
                Some(Tag::List { start, tight })
            }
            Name::ListItem => {
                let mut before = self.matching[index] - 1;
                while before > index
                    && matches!(
                        self.tokens[before].name,
                        Name::BlankLineEnding
                            | Name::BlockQuotePrefix
                            | Name::LineEnding
                            | Name::SpaceOrTab
                            | Name::Definition
                            | Name::GfmFootnoteDefinition
                    )
                {
                    before = if self.tokens[before].kind == Kind::Exit {
                        self.matching[before].saturating_sub(1)
                    } else {
                        before - 1
                    };
                }
                let tight_paragraph =
                    self.tight.last() == Some(&true) && self.tokens[before].name == Name::Paragraph;
                Some(Tag::Item {
                    block_end: !tight_paragraph && self.tokens[before].name != Name::ListItemPrefix,
                })
            }
            Name::Emphasis => Some(Tag::Emphasis),
            Name::Strong => Some(Tag::Strong),
            Name::GfmStrikethrough => Some(Tag::Delete),
            Name::Link => {
                let (destination, title) = self.media(index);
                Some(Tag::Link { destination, title })
            }
            Name::Image => {
                let (destination, title) = self.media(index);
                Some(Tag::Image { destination, title })
            }
            Name::GfmTable => {
                self.align = infer::gfm_table_align(&self.tokens, index);
                Some(Tag::Table)
            }
            Name::GfmTableHead => {
                self.header = true;
                Some(Tag::TableHead)
            }
            Name::GfmTableBody => {
                self.header = false;
                Some(Tag::TableBody)
            }
            Name::GfmTableRow => {
                self.column = 0;
                Some(Tag::TableRow)
            }
            Name::GfmTableCell => {
                let alignment = self.align.get(self.column).copied();
                self.column += 1;
                if let Some(alignment) = alignment {
                    Some(Tag::TableCell {
                        alignment,
                        header: self.header,
                    })
                } else {
                    self.index = self.matching[index] + 1;
                    None
                }
            }
            Name::GfmFootnoteDefinition => {
                self.tight.push(false);
                let label = self
                    .named(index, &Name::GfmFootnoteDefinitionLabelString)
                    .expect("footnote has a label");
                Some(Tag::FootnoteDefinition(Text::from(normalize_identifier(
                    &self.source[self.span(label)],
                ))))
            }
            _ => None,
        }
    }

    fn end(&mut self, index: usize) -> Option<TagKind> {
        let kind = match self.tokens[index].name {
            Name::Paragraph => {
                self.slurp = self.tight.last() == Some(&true);
                TagKind::Paragraph
            }
            Name::HeadingAtx | Name::HeadingSetext => TagKind::Heading,
            Name::BlockQuote => {
                self.tight.pop();
                self.slurp = false;
                TagKind::BlockQuote
            }
            Name::ListOrdered | Name::ListUnordered => {
                self.tight.pop();
                TagKind::List
            }
            Name::ListItem => {
                self.slurp = false;
                TagKind::Item
            }
            Name::Emphasis => TagKind::Emphasis,
            Name::Strong => TagKind::Strong,
            Name::GfmStrikethrough => TagKind::Delete,
            Name::Link => TagKind::Link,
            Name::Image => TagKind::Image,
            Name::GfmTable => {
                self.align.clear();
                TagKind::Table
            }
            Name::GfmTableHead => TagKind::TableHead,
            Name::GfmTableBody => TagKind::TableBody,
            Name::GfmTableRow => TagKind::TableRow,
            Name::GfmTableCell => TagKind::TableCell,
            Name::GfmFootnoteDefinition => {
                self.tight.pop();
                TagKind::FootnoteDefinition
            }
            _ => return None,
        };
        self.containers.pop();
        Some(kind)
    }

    fn leaf(&mut self, index: usize) -> Option<EventKind<'static>> {
        let name = self.tokens[index].name.clone();
        match name {
            Name::Data | Name::CharacterEscapeValue => {
                let mut text = self.text(index);
                if self.options.inline_code_info && name == Name::Data {
                    let range = self.span(index);
                    let next = self.matching[index] + 1;
                    if next < self.tokens.len() && self.tokens[next].name == Name::CodeText {
                        if let Some((start, _)) = crate::util::inline_code_info::before(
                            self.source.as_bytes(),
                            self.tokens[next].point.index,
                        ) {
                            if start >= range.start {
                                text = Text::Source(range.start..start);
                            }
                        }
                    }
                }
                Some(EventKind::Text(text))
            }
            Name::CharacterReference => {
                self.index = self.matching[index] + 1;
                Some(EventKind::Text(self.entity(self.span(index))))
            }
            Name::CodeText
            | Name::MathText
            | Name::CodeFenced
            | Name::CodeIndented
            | Name::MathFlow => {
                let block = matches!(name, Name::CodeFenced | Name::CodeIndented | Name::MathFlow);
                let kind = self.code(
                    index,
                    block,
                    matches!(name, Name::MathFlow | Name::MathText),
                );
                self.index = self.matching[index] + 1;
                self.slurp = false;
                Some(kind)
            }
            Name::HtmlFlow | Name::HtmlText => {
                let value = self.value(index, &[Name::HtmlFlowData, Name::HtmlTextData], true);
                self.index = self.matching[index] + 1;
                Some(EventKind::Html {
                    value,
                    block: name == Name::HtmlFlow,
                })
            }
            Name::HardBreakEscape | Name::HardBreakTrailing => {
                self.index = self.matching[index] + 1;
                Some(EventKind::HardBreak)
            }
            Name::ThematicBreak => {
                self.index = self.matching[index] + 1;
                Some(EventKind::Rule)
            }
            Name::LineEnding => {
                if self.slurp {
                    self.slurp = false;
                    None
                } else if index > 0
                    && matches!(
                        self.tokens[index - 1].name,
                        Name::Definition | Name::GfmFootnoteDefinition | Name::HeadingSetextText
                    )
                {
                    None
                } else if self.containers.last().is_some_and(|tag| {
                    matches!(
                        tag,
                        TagKind::Paragraph
                            | TagKind::Heading
                            | TagKind::Emphasis
                            | TagKind::Strong
                            | TagKind::Delete
                            | TagKind::Link
                            | TagKind::Image
                            | TagKind::TableCell
                    )
                }) {
                    Some(EventKind::SoftBreak(self.text(index)))
                } else {
                    Some(EventKind::Separator {
                        value: self.text(index),
                        conditional: false,
                    })
                }
            }
            Name::BlankLineEnding => {
                self.slurp = false;
                if self.matching[index] == self.tokens.len() - 1 {
                    Some(EventKind::Separator {
                        value: self.text(index),
                        conditional: true,
                    })
                } else {
                    None
                }
            }
            Name::Resource
            | Name::Reference
            | Name::GfmFootnoteDefinitionLabelString
            | Name::GfmFootnoteDefinitionPrefix => {
                self.index = self.matching[index] + 1;
                None
            }
            Name::Definition => {
                let (label, destination, title) = self.definition(index);
                self.index = self.matching[index] + 1;
                self.slurp = true;
                Some(EventKind::Definition {
                    label,
                    destination,
                    title,
                })
            }
            Name::Frontmatter
            | Name::MdxEsm
            | Name::MdxFlowExpression
            | Name::MdxTextExpression
            | Name::MdxJsxFlowTag
            | Name::MdxJsxTextTag => {
                let block = !matches!(name, Name::MdxTextExpression | Name::MdxJsxTextTag);
                let value = self.text(index);
                self.index = self.matching[index] + 1;
                self.slurp = block;
                Some(EventKind::Opaque { value, block })
            }
            Name::GfmTaskListItemCheck => {
                let checked = self
                    .named(index, &Name::GfmTaskListItemValueChecked)
                    .is_some();
                self.index = self.matching[index] + 1;
                Some(EventKind::TaskMarker(checked))
            }
            Name::GfmFootnoteCall => {
                let value = self
                    .named(index, &Name::LabelText)
                    .expect("footnote call has a label");
                self.index = self.matching[index] + 1;
                Some(EventKind::FootnoteReference(Text::from(
                    normalize_identifier(&self.source[self.span(value)]),
                )))
            }
            Name::Autolink
            | Name::GfmAutolinkLiteralEmail
            | Name::GfmAutolinkLiteralMailto
            | Name::GfmAutolinkLiteralProtocol
            | Name::GfmAutolinkLiteralWww
            | Name::GfmAutolinkLiteralXmpp => {
                let inner = if name == Name::Autolink {
                    self.named(index, &Name::AutolinkProtocol)
                        .or_else(|| self.named(index, &Name::AutolinkEmail))
                        .unwrap()
                } else {
                    index
                };
                let label = self.span(inner);
                if name != Name::Autolink && self.containers.contains(&TagKind::Link) {
                    self.index = self.matching[index] + 1;
                    return Some(EventKind::Text(Text::Source(label)));
                }
                let prefix = if name == Name::GfmAutolinkLiteralWww {
                    "http://"
                } else if name == Name::GfmAutolinkLiteralEmail
                    || self.tokens[inner].name == Name::AutolinkEmail
                {
                    "mailto:"
                } else {
                    ""
                };
                let destination = if prefix.is_empty() {
                    Text::Source(label.clone())
                } else {
                    Text::from(alloc::format!("{prefix}{}", &self.source[label.clone()]))
                };
                // A tiny synthetic sequence, delivered by reusing the pending slot and cursor state.
                self.index = self.matching[index] + 1;
                self.pending = Some(Event {
                    kind: EventKind::Text(Text::Source(label.clone())),
                    span: label,
                });
                self.containers.push(TagKind::Link);
                // The matching synthetic end is generated before the next grammar token.
                self.synthetic_end = Some(self.span(index).end);
                Some(EventKind::Start(Tag::Link {
                    destination,
                    title: None,
                }))
            }
            _ => None,
        }
    }
}

impl Iterator for EventParser<'_> {
    type Item = Result<Event<'static>, StreamError>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(end) = self.padding_end.take() {
            return Some(Ok(Event {
                kind: EventKind::End(TagKind::TableCell),
                span: end..end,
            }));
        }
        if let Some(event) = self.pending.take() {
            return Some(Ok(event));
        }
        if let Some(end) = self.synthetic_end.take() {
            self.containers.pop();
            return Some(Ok(Event {
                kind: EventKind::End(TagKind::Link),
                span: end..end,
            }));
        }
        if !self.started {
            self.started = true;
            return Some(Ok(Event {
                kind: EventKind::Start(Tag::Document(self.line_ending.clone())),
                span: 0..self.source.len(),
            }));
        }
        loop {
            if self.index == self.tokens.len() {
                if let Some((tokens, matching, index)) = self.block_tokens.take() {
                    self.buffers.events = core::mem::replace(&mut self.tokens, tokens);
                    self.inline_matching = core::mem::replace(&mut self.matching, matching);
                    self.index = index;
                    continue;
                }
                break;
            }
            if self.tokens[self.index].kind == Kind::Exit
                && self.tokens[self.index].name == Name::GfmTableRow
                && self.column < self.align.len()
            {
                let end = self.tokens[self.index].point.index;
                let alignment = self.align[self.column];
                self.column += 1;
                self.padding_end = Some(end);
                return Some(Ok(Event {
                    kind: EventKind::Start(Tag::TableCell {
                        alignment,
                        header: self.header,
                    }),
                    span: end..end,
                }));
            }
            let index = self.index;
            self.index += 1;
            if self.tokens[index].kind == Kind::Enter {
                if let Some(link) = &self.tokens[index].link {
                    if link.content == Content::Text {
                        let first = link.previous().is_none();
                        self.index = self.matching[index] + 1;
                        if first {
                            if let Err(error) = self.inline(index) {
                                self.ended = true;
                                self.index = self.tokens.len();
                                return Some(Err(error));
                            }
                        }
                        continue;
                    }
                }
                let span = self.span(index);
                if let Some(tag) = self.start(index) {
                    self.containers.push(tag.kind());
                    return Some(Ok(Event {
                        kind: EventKind::Start(tag),
                        span,
                    }));
                }
                if let Some(kind) = self.leaf(index) {
                    return Some(Ok(Event { kind, span }));
                }
            } else if let Some(kind) = self.end(index) {
                let end = self.tokens[index].point.index;
                return Some(Ok(Event {
                    kind: EventKind::End(kind),
                    span: end..end,
                }));
            }
        }
        if self.ended {
            None
        } else {
            self.ended = true;
            let end = self.source.len();
            Some(Ok(Event {
                kind: EventKind::End(TagKind::Document),
                span: end..end,
            }))
        }
    }
}

/// Keep a contiguous source range until normalization actually requires an owned string.
struct ValueBuilder<'a> {
    source: &'a str,
    range: Option<Range<usize>>,
    owned: Option<String>,
}
impl<'a> ValueBuilder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            range: None,
            owned: None,
        }
    }
    fn append(&mut self, text: Text<'_>) {
        if self.range.is_none() && self.owned.is_none() {
            if let Text::Value(alloc::borrow::Cow::Owned(value)) = text {
                self.owned = Some(value);
                return;
            }
        }
        if let Some(owned) = &mut self.owned {
            owned.push_str(text.resolve(self.source).unwrap());
            return;
        }
        if let Text::Source(range) = &text {
            if let Some(previous) = &mut self.range {
                if previous.end == range.start {
                    previous.end = range.end;
                    return;
                }
            } else {
                self.range = Some(range.clone());
                return;
            }
        }
        let mut owned = self
            .range
            .take()
            .map_or_else(String::new, |range| self.source[range].to_string());
        owned.push_str(text.resolve(self.source).unwrap());
        self.owned = Some(owned);
    }
    fn finish(self) -> Text<'static> {
        match self.owned {
            Some(owned) => Text::from(owned),
            None => self.range.map_or(Text::Literal(""), Text::Source),
        }
    }
}

fn pair_tokens(
    tokens: &[Token],
    matching: &mut Vec<usize>,
    stack: &mut Vec<usize>,
) -> Result<(), StreamError> {
    matching.resize(tokens.len(), 0);
    stack.clear();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Enter {
            stack.push(index);
        } else {
            let start = stack.pop().ok_or(StreamError::Unbalanced)?;
            matching[start] = index;
            matching[index] = start;
        }
    }
    if stack.is_empty() {
        Ok(())
    } else {
        Err(StreamError::Unbalanced)
    }
}
