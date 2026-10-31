// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Emit semantic events for Markdown that arrives in pieces.
use crate::block_parser::{Blocks, TopLevel};
use crate::definitions::Definitions;
use crate::event_parser::EventParser;
use crate::stream::{Event, EventKind, Tag, TagKind};
use crate::ParseOptions;
use alloc::string::String;
use alloc::vec::{Drain, Vec};

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
pub struct IncrementalParser<'a> {
    source: String,
    options: &'a ParseOptions,
    // Owned definitions authored before `anchor`.
    seed: Definitions<'static>,
    // The line of a clean document-level block from which preparation restarts.
    anchor: usize,
    // How many document-level blocks from `anchor` have been emitted; the separators after the last of them are not.
    emitted: usize,
    // The end of the complete lines already prepared.
    complete: usize,
    stable: usize,
    events: Vec<Event<'static>>,
    finished: bool,
}

impl<'a> IncrementalParser<'a> {
    /// Create a parser for an empty document.
    pub fn new(options: &'a ParseOptions) -> Self {
        Self {
            source: String::new(),
            options,
            seed: Definitions::default(),
            anchor: 0,
            emitted: 0,
            complete: 0,
            stable: 0,
            events: Vec::new(),
            finished: false,
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
    /// Only complete lines are prepared, so text without a line ending costs no parsing. Each preparation restarts from the most recent clean document-level block that has been emitted.
    ///
    /// # Panics
    ///
    /// Panics if called after [`IncrementalParser::finish`].
    pub fn push(&mut self, text: &str) -> Drain<'_, Event<'static>> {
        assert!(!self.finished, "cannot push after finishing");
        self.source.push_str(text);
        let complete = complete_lines(&self.source);
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
    pub fn finish(&mut self) -> Drain<'_, Event<'static>> {
        assert!(!self.finished, "cannot finish twice");
        self.finished = true;
        let (blocks, definitions, outline) =
            Blocks::parse_from(&self.source, self.anchor, self.options, self.seed.clone());
        let parser = EventParser::from_blocks(&self.source, self.options, blocks, definitions);
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
        let (blocks, definitions, outline) =
            Blocks::parse_from(source, self.anchor, self.options, self.seed.clone());
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
        let seed = definitions.before(tops[anchor].line);
        let parser = EventParser::from_blocks(source, self.options, blocks, definitions);
        deliver(parser, tops, self.emitted, Some(last), &mut self.events);
        self.stable = tops[last].end;
        self.seed = seed;
        self.anchor = tops[anchor].line;
        self.emitted = last - anchor + 1;
    }
}

/// The end of the last complete line. A trailing carriage return may become part of a CRLF ending, so its line is incomplete.
fn complete_lines(source: &str) -> usize {
    let bytes = source.as_bytes();
    let end = bytes.len() - usize::from(bytes.last() == Some(&b'\r'));
    bytes[..end]
        .iter()
        .rposition(|&byte| matches!(byte, b'\n' | b'\r'))
        .map_or(0, |index| index + 1)
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
    use super::{complete_lines, IncrementalParser};
    use crate::ParseOptions;

    #[test]
    fn complete_lines_exclude_a_trailing_carriage_return() {
        assert_eq!(complete_lines(""), 0);
        assert_eq!(complete_lines("a"), 0);
        assert_eq!(complete_lines("a\n"), 2);
        assert_eq!(complete_lines("a\nb"), 2);
        assert_eq!(complete_lines("a\r"), 0);
        assert_eq!(complete_lines("a\r\r"), 2);
        assert_eq!(complete_lines("a\r\n"), 3);
        assert_eq!(complete_lines("a\rb"), 2);
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
