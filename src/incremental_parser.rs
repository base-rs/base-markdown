// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Emit semantic events for Markdown that arrives in pieces.
use crate::block_parser::{Blocks, TopLevel};
use crate::definitions::{Definitions, Seed};
use crate::event_parser::EventParser;
use crate::stream::{Event, EventKind, Tag, TagKind};
use crate::ParseOptions;
use alloc::string::String;
use alloc::vec::Vec;

/// Parse Markdown that arrives in pieces, emitting the events of each document-level block once no later text can change them.
///
/// Push text as it arrives; each call returns the events of blocks that have become stable. Call [`IncrementalParser::finish`] at the end of the input to receive the remaining events. Events are the document's body events; the document start and end events are not emitted. Text values refer to byte ranges of the accumulated [`IncrementalParser::source`], exactly as [`EventParser`] events refer to its source.
///
/// ```
/// use base_markdown::stream::{EventKind, Tag};
/// use base_markdown::{IncrementalParser, ParseOptions};
///
/// let options = ParseOptions::gfm();
/// let mut parser = IncrementalParser::new(&options);
///
/// // An ATX heading is complete at the end of its line.
/// let events: Vec<_> = parser.push("# Title\nFirst para").collect();
/// assert!(matches!(events[0].kind, EventKind::Start(Tag::Heading(1))));
/// assert_eq!(parser.stable_offset(), 8);
///
/// // A paragraph remains open until a blank line or another block follows it.
/// assert_eq!(parser.push("graph.\n").count(), 0);
/// let events: Vec<_> = parser.push("\nSecond").collect();
/// assert!(events.iter().any(|event| matches!(event.kind, EventKind::Start(Tag::Paragraph { .. }))));
/// assert_eq!(&parser.source()[..parser.stable_offset()], "# Title\nFirst paragraph.\n");
///
/// // Finishing emits the rest of the document.
/// let rest: Vec<_> = parser.finish().collect();
/// assert!(matches!(rest.last().unwrap().kind, EventKind::End(_)));
/// ```
///
/// # Stability
///
/// The events emitted across all pushes and the final [`IncrementalParser::finish`] are the events that [`EventParser`] yields for the accumulated source, without the document start and end events, provided that no link reference or footnote call is resolved by a definition in a later document-level block. Stability is defined by block structure: a block is emitted when it and every block before it are closed and no appended text can reinterpret them. Emitted events are never retracted.
///
/// Reference resolution is scoped to make the output independent of how the text is divided. Inline content in a document-level block resolves against the definitions that begin before the end of that block, including definitions inside the block itself. A reference to a definition in a later document-level block remains literal text, whereas [`EventParser`] resolves forward references across the whole document.
///
/// Container contents are not emitted separately: a list is emitted once it ends, because its tightness depends on every item. Frontmatter holds back all output until its closing fence arrives.
#[derive(Debug)]
pub struct IncrementalParser<'a> {
    source: String,
    options: &'a ParseOptions,
    // Owned definitions authored before `anchor`.
    seed: Seed,
    // The line of a clean document-level block from which preparation restarts.
    anchor: usize,
    // How many document-level blocks from `anchor` have been emitted; the separators after the last of them are not.
    emitted: usize,
    // The end of the complete lines already prepared.
    complete: usize,
    stable: usize,
    events: Vec<Event<'static>>,
    finished: bool,
    // Whether the accumulated source has a CR, tracked per push so that preparation does not rescan the source.
    carriage_returns: bool,
}

impl<'a> IncrementalParser<'a> {
    /// Create a parser for an empty document.
    pub fn new(options: &'a ParseOptions) -> Self {
        Self {
            source: String::new(),
            options,
            seed: Seed::default(),
            anchor: 0,
            emitted: 0,
            complete: 0,
            stable: 0,
            events: Vec::new(),
            finished: false,
            carriage_returns: false,
        }
    }

    /// The accumulated source, which emitted text values refer to.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The byte offset where the last emitted block ends. Appended text cannot change the events of the source before this offset.
    pub fn stable_offset(&self) -> usize {
        self.stable
    }

    /// Append text and return the events of blocks that have become stable.
    ///
    /// The text is parsed before this returns; dropping the iterator discards the events it has not yielded.
    ///
    /// Only complete lines are prepared, so text without a line ending costs no parsing. Each preparation restarts from the most recent clean document-level block that has been emitted.
    ///
    /// # Panics
    ///
    /// Panics if called after [`IncrementalParser::finish`].
    pub fn push(&mut self, text: &str) -> impl Iterator<Item = Event<'static>> + '_ {
        assert!(!self.finished, "cannot push after finishing");
        // Earlier text has no line ending after `complete`, except perhaps a final carriage return.
        let from = self.source.len().saturating_sub(1).max(self.complete);
        self.source.push_str(text);
        self.carriage_returns |= memchr::memchr(b'\r', text.as_bytes()).is_some();
        let complete = complete_lines(&self.source, from);
        if complete > self.complete {
            self.complete = complete;
            self.prepare();
        }
        self.events.drain(..)
    }

    /// End the input and return all remaining events.
    ///
    /// # Panics
    ///
    /// Panics if called more than once.
    pub fn finish(&mut self) -> impl Iterator<Item = Event<'static>> + '_ {
        assert!(!self.finished, "cannot finish twice");
        self.finished = true;
        let (blocks, definitions, outline) = Blocks::parse_from(
            &self.source,
            self.anchor,
            self.options,
            Definitions::seeded(&self.seed),
        );
        let parser = EventParser::from_blocks(
            &self.source,
            self.options,
            blocks,
            definitions,
            self.carriage_returns,
        );
        deliver(
            parser,
            &outline.blocks,
            self.emitted,
            None,
            &mut self.events,
        );
        self.stable = self.source.len();
        self.events.drain(..)
    }

    fn prepare(&mut self) {
        let source = &self.source[..self.complete];
        if self.anchor == 0
            && self.emitted == 0
            && Blocks::frontmatter_pending(source, self.options)
        {
            return;
        }
        let (blocks, definitions, outline) = Blocks::parse_from(
            source,
            self.anchor,
            self.options,
            Definitions::seeded(&self.seed),
        );
        let tops = &outline.blocks;
        // A clean block proves every earlier block closed; a trailing blank line at document level closes the last block too.
        let last = if outline.closed {
            tops.len().checked_sub(1)
        } else {
            (1..tops.len())
                .rev()
                .find(|&next| tops[next].clean)
                .map(|next| next - 1)
        };
        let Some(last) = last.filter(|&last| last >= self.emitted) else {
            return;
        };
        // The first prepared block always begins at the clean anchor.
        let anchor = (0..=last)
            .rev()
            .find(|&index| tops[index].clean)
            .unwrap_or(0);
        let authored = definitions.before(tops[anchor].line);
        let parser = EventParser::from_blocks(
            source,
            self.options,
            blocks,
            definitions,
            self.carriage_returns,
        );
        deliver(parser, tops, self.emitted, Some(last), &mut self.events);
        self.stable = tops[last].end;
        self.seed.append(authored);
        self.anchor = tops[anchor].line;
        self.emitted = last - anchor + 1;
    }
}

/// The end of the last complete line, searching for line endings from `from`, or zero if there is none. A trailing carriage return may become part of a CRLF ending, so its line is incomplete.
fn complete_lines(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let end = bytes.len() - usize::from(bytes.last() == Some(&b'\r'));
    bytes[from.min(end)..end]
        .iter()
        .rposition(|&byte| matches!(byte, b'\n' | b'\r'))
        .map_or(0, |index| from + index + 1)
}

/// Move events into `output`, skipping the first `skip` document-level blocks and stopping after block `until`. Separators between blocks are delivered only with the block that follows them, unless delivering to the end of the document.
fn deliver(
    mut parser: EventParser<'_>,
    tops: &[TopLevel],
    skip: usize,
    until: Option<usize>,
    output: &mut Vec<Event<'static>>,
) {
    let mut block = 0;
    let mut depth = 0usize;
    let mut separators = Vec::new();
    loop {
        if depth == 0 {
            if until.is_some_and(|until| block > until) {
                return;
            }
            parser.set_definition_limit(Some(tops.get(block).map_or(usize::MAX, |top| top.end)));
        }
        let Some(Ok(event)) = parser.next() else {
            break;
        };
        match &event.kind {
            EventKind::Start(Tag::Document(_)) | EventKind::End(TagKind::Document) => continue,
            EventKind::Separator { .. } if depth == 0 => {
                separators.push(event);
                continue;
            }
            _ => {}
        }
        if depth == 0 {
            if block >= skip {
                output.append(&mut separators);
            } else {
                separators.clear();
            }
        }
        match event.kind {
            EventKind::Start(_) => depth += 1,
            EventKind::End(_) => depth -= 1,
            _ => {}
        }
        if block >= skip {
            output.push(event);
        }
        if depth == 0 {
            block += 1;
        }
    }
    // Only delivery to the end of the document exhausts the parser, after every skipped block.
    output.append(&mut separators);
}

#[cfg(test)]
mod tests {
    use super::{complete_lines, deliver, IncrementalParser};
    use crate::block_parser::Blocks;
    use crate::definitions::Definitions;
    use crate::event_parser::EventParser;
    use crate::stream::Event;
    use crate::{Constructs, ParseOptions};
    use alloc::string::String;
    use alloc::vec::Vec;
    use core::convert::TryFrom;

    /// A one-shot parse that scopes references to each document-level block, independent of anchors, seeds and stability.
    fn scoped(source: &str, options: &ParseOptions) -> Vec<Event<'static>> {
        let (blocks, definitions, outline) =
            Blocks::parse_from(source, 0, options, Definitions::default());
        let carriage_returns = memchr::memchr(b'\r', source.as_bytes()).is_some();
        let parser =
            EventParser::from_blocks(source, options, blocks, definitions, carriage_returns);
        let mut events = Vec::new();
        deliver(parser, &outline.blocks, 0, None, &mut events);
        events
    }

    fn incremental<'a>(
        pieces: impl IntoIterator<Item = &'a str>,
        options: &ParseOptions,
    ) -> Vec<Event<'static>> {
        let mut parser = IncrementalParser::new(options);
        let mut events = Vec::new();
        for piece in pieces {
            events.extend(parser.push(piece));
        }
        events.extend(parser.finish());
        events
    }

    #[test]
    fn definitions_resolve_as_in_a_scoped_one_shot_parse() {
        const LINES: &[&str] = &[
            "",
            "[a] [b] [^n] [c][a] [A]",
            "text [b]",
            "[a]: /a",
            "[A]: /upper",
            "[b]: /b 'title",
            "continued'",
            "[c]:",
            "/c",
            "[^n]: note [a]",
            "    indented [b]",
            "- [a]: /item",
            "> [b]: /quote",
            "- item [a]",
            "> quote [b]",
            "---",
            "===",
            "# [a]",
            "|-|",
            "| [a] | [b] |",
            "```",
            "<div>",
        ];
        let mut options = ParseOptions::gfm();
        options.constructs = Constructs {
            frontmatter: true,
            ..Constructs::gfm()
        };
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut random = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            usize::from(u16::try_from(state >> 48).unwrap())
        };
        for _ in 0..3000 {
            let mut source = String::new();
            for _ in 0..(random() % 14) {
                source.push_str(LINES[random() % LINES.len()]);
                source.push_str(if random() % 4 == 0 { "\r\n" } else { "\n" });
            }
            let expected = scoped(&source, &options);
            let characters = source
                .char_indices()
                .map(|(index, character)| &source[index..index + character.len_utf8()]);
            assert_eq!(incremental(characters, &options), expected, "{source:?}");
            let mut cuts: Vec<usize> = (0..4).map(|_| random() % (source.len() + 1)).collect();
            cuts.sort_unstable();
            let mut previous = 0;
            let mut pieces = Vec::new();
            for cut in cuts.into_iter().chain([source.len()]) {
                pieces.push(&source[previous..cut]);
                previous = cut;
            }
            assert_eq!(incremental(pieces, &options), expected, "{source:?}");
        }
    }

    #[test]
    fn complete_lines_exclude_a_trailing_carriage_return() {
        assert_eq!(complete_lines("", 0), 0);
        assert_eq!(complete_lines("a", 0), 0);
        assert_eq!(complete_lines("a\n", 0), 2);
        assert_eq!(complete_lines("a\nb", 0), 2);
        assert_eq!(complete_lines("a\r", 0), 0);
        assert_eq!(complete_lines("a\r\r", 0), 2);
        assert_eq!(complete_lines("a\r\n", 0), 3);
        assert_eq!(complete_lines("a\rb", 0), 2);
        assert_eq!(complete_lines("a\nbc", 2), 0);
        assert_eq!(complete_lines("a\nb\r\n", 3), 5);
    }

    #[test]
    fn preparation_restarts_near_the_end_of_closed_blocks() {
        let options = ParseOptions::gfm();
        let mut parser = IncrementalParser::new(&options);
        for index in 0..200 {
            let block = match index % 4 {
                0 => "# Heading\n\n",
                1 => "A paragraph with *emphasis*.\n\n",
                2 => "- a list\n- of items\n\n",
                _ => "```\ncode\n```\n\n",
            };
            for character in block.split_inclusive(' ') {
                let _ = parser.push(character);
            }
            // Preparation restarts at most two of these short blocks before the end, independent of the document length.
            assert!(parser.anchor <= parser.stable_offset());
            assert!(parser.source.len() - parser.anchor <= 64);
        }
    }
}
