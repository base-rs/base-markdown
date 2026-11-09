// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Copyright 2015-2018 Google Inc. and Google LLC.
// Copyright 2026 Samuel Williams.
// Block/container algorithms adapted from pulldown-cmark revision 38e4d08f14ec4bd9783270e9623db7681ebed968.

//! Compact block preparation with source fragments and deferred inline scanning.
mod definition;
pub(crate) mod frontmatter;
mod html;
mod scanners;
mod table;
use crate::definitions::Definitions;
use crate::stream::Text;
use crate::util::line_ending::terminal_ending_start;
use crate::ParseOptions;
use crate::{AlignKind, CodeFence};
use alloc::vec;
use alloc::vec::Vec;
use core::{convert::TryFrom, ops::Range};
use scanners::{
    is_ascii_whitespace_no_nl, scan_atx_heading, scan_blank_line, scan_closing_code_fence,
    scan_code_fence, scan_hrule, scan_nextline, scan_setext_heading, scan_whitespace_no_nl,
    LineStart,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Body {
    Root,
    Paragraph,
    Heading(u8),
    Quote,
    Footnote(usize),
    Opaque,
    List {
        tight: bool,
        marker: u8,
        start: u32,
    },
    Item(usize),
    Code(usize),
    Definition(usize),
    Html(usize),
    Table,
    TableHead,
    #[allow(clippy::enum_variant_names)] // This is the semantic HTML table body.
    TableBody,
    TableRow,
    TableCell {
        alignment: AlignKind,
        header: bool,
    },
    Rule,
    Fragment(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub start: usize,
    pub end: usize,
    pub child: usize,
    pub next: usize,
    pub parent: usize,
    pub body: Body,
}

impl Node {
    #[inline]
    pub fn span_end(&self, source: &str) -> usize {
        if matches!(
            self.body,
            Body::Table | Body::TableHead | Body::TableBody | Body::TableRow
        ) {
            terminal_ending_start(source.as_bytes(), self.end)
        } else {
            self.end
        }
    }
}

pub(super) struct Code {
    pub info: Range<usize>,
    pub fence: Option<CodeFence>,
    pub unclosed: bool,
}

pub(super) struct Blocks {
    pub nodes: Vec<Node>,
    pub codes: Vec<Code>,
}

struct Builder<'a> {
    source: &'a str,
    options: &'a ParseOptions,
    blocks: Blocks,
    open: Vec<usize>,
    current: usize,
    blank: bool,
    empty_item: Option<usize>,
    definitions: Definitions<'a>,
    alignments: Vec<AlignKind>,
    /// The lines of a paragraph whose definitions are being read, reused between paragraphs.
    lines: Vec<(Range<usize>, usize)>,
    /// Whether the paragraph before this line closed because this line heads a table, which then continues that paragraph's containers.
    table_header: bool,
    recorder: Option<Recorder>,
}

/// Outline state, maintained only when preparing for incremental parsing.
#[derive(Default)]
struct Recorder {
    outline: Outline,
    line: usize,
    clean: bool,
    interrupted: bool,
}

/// A document-level block and the line where its preparation began.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TopLevel {
    /// The node index in the prepared blocks.
    pub node: usize,
    /// The start of the physical line whose preparation created the block.
    pub line: usize,
    /// The end of the block's source extent.
    pub end: usize,
    /// Whether no state from earlier lines influenced the preparation of this line, and no earlier block's lookahead can reinterpret it. Every earlier block is then closed, and preparing the source from `line` with only the earlier definitions reproduces this block and everything after it.
    pub clean: bool,
}

/// Document-level structure recorded for incremental parsing.
#[derive(Debug, Default)]
pub(crate) struct Outline {
    pub blocks: Vec<TopLevel>,
    /// Whether every block closed on the prepared lines, so no later line can extend or reinterpret them: the last line was blank and closed every container, or a clean document-level ATX heading, thematic break or closed fence ended it.
    pub closed: bool,
}

impl Blocks {
    /// Walk authored children in source order without collecting their indexes.
    pub fn children(&self, index: usize) -> impl Iterator<Item = (usize, &Node)> + Clone + '_ {
        let mut next = self.nodes[index].child;
        core::iter::from_fn(move || {
            if next == 0 {
                None
            } else {
                let index = next;
                let node = &self.nodes[index];
                next = node.next;
                Some((index, node))
            }
        })
    }

    /// Only paragraphs directly inside a tight list item omit their own tags.
    #[inline]
    pub fn tight_paragraph(&self, node: &Node) -> bool {
        if node.body != Body::Paragraph {
            return false;
        }
        let parent = &self.nodes[node.parent];
        matches!(parent.body, Body::Item(_))
            && matches!(
                self.nodes[parent.parent].body,
                Body::List { tight: true, .. }
            )
    }
    pub fn parse<'a>(source: &'a str, options: &'a ParseOptions) -> (Self, Definitions<'a>) {
        let (blocks, definitions, _) =
            Self::prepare(source, 0, options, Definitions::default(), false);
        (blocks, definitions)
    }

    /// Prepare `source` with `seed` holding the definitions of another document, which precede
    /// its own.
    pub fn parse_seeded<'a>(
        source: &'a str,
        options: &'a ParseOptions,
        seed: Definitions<'a>,
    ) -> (Self, Definitions<'a>) {
        let (blocks, definitions, _) = Self::prepare(source, 0, options, seed, false);
        (blocks, definitions)
    }

    /// Prepare `source[start..]`, where `start` is the line of a clean document-level block, with `seed` holding the definitions authored before it. Node offsets remain relative to the whole source.
    pub fn parse_from<'a>(
        source: &'a str,
        start: usize,
        options: &'a ParseOptions,
        seed: Definitions<'a>,
    ) -> (Self, Definitions<'a>, Outline) {
        let (blocks, definitions, outline) = Self::prepare(source, start, options, seed, true);
        (
            blocks,
            definitions,
            outline.expect("recorded preparation returns an outline"),
        )
    }

    fn prepare<'a>(
        source: &'a str,
        start: usize,
        options: &'a ParseOptions,
        seed: Definitions<'a>,
        record: bool,
    ) -> (Self, Definitions<'a>, Option<Outline>) {
        let mut builder = Builder {
            source,
            options,
            blocks: Self {
                nodes: Vec::with_capacity(source.len() / 64 + 1),
                codes: Vec::new(),
            },
            open: vec![0],
            current: 0,
            blank: false,
            empty_item: None,
            definitions: seed,
            alignments: Vec::new(),
            lines: Vec::new(),
            table_header: false,
            recorder: record.then(|| Recorder {
                line: start,
                clean: true,
                ..Recorder::default()
            }),
        };
        builder.blocks.nodes.push(Node {
            start,
            end: source.len(),
            child: 0,
            next: 0,
            parent: 0,
            body: Body::Root,
        });
        let mut index = start;
        if start == 0 {
            index = usize::from(source.starts_with('\u{feff}')) * 3;
            if options.constructs.frontmatter {
                if let Some((end, next)) = frontmatter::scan(source.as_bytes(), index) {
                    builder.append(index, end, Body::Opaque);
                    index = next;
                }
            }
        }
        while index < source.len() {
            index = builder.block(index);
        }
        while builder.open.len() > 1 {
            builder.pop(index);
        }
        builder.definitions.finish();
        let nodes = &builder.blocks.nodes;
        let outline = builder.recorder.map(|recorder| {
            let mut outline = recorder.outline;
            for block in &mut outline.blocks {
                block.end = nodes[block.node].end;
            }
            outline
        });
        (builder.blocks, builder.definitions, outline)
    }

    /// Whether the complete lines of `source` begin frontmatter that has not yet closed. Its opening fence would otherwise prepare as ordinary blocks that the closing fence later reinterprets.
    pub fn frontmatter_pending(source: &str, options: &ParseOptions) -> bool {
        let bytes = source.as_bytes();
        let start = usize::from(source.starts_with('\u{feff}')) * 3;
        options.constructs.frontmatter && frontmatter::pending(bytes, start)
    }
}

impl Builder<'_> {
    fn append(&mut self, start: usize, end: usize, body: Body) -> usize {
        let parent = *self.open.last().unwrap();
        let index = self.blocks.nodes.len();
        self.blocks.nodes.push(Node {
            start,
            end,
            body,
            parent,
            child: 0,
            next: 0,
        });
        if parent == 0 {
            if let Some(recorder) = &mut self.recorder {
                recorder.outline.blocks.push(TopLevel {
                    node: index,
                    line: recorder.line,
                    end,
                    clean: recorder.clean,
                });
            }
        }
        if self.current == 0 {
            self.blocks.nodes[parent].child = index;
        } else {
            self.blocks.nodes[self.current].next = index;
        }
        self.current = index;
        index
    }
    fn push(&mut self) {
        self.open.push(self.current);
        self.current = 0;
    }
    fn pop(&mut self, end: usize) {
        let index = self.open.pop().unwrap();
        self.blocks.nodes[index].end = end;
        self.current = index;
        if matches!(
            self.blocks.nodes[index].body,
            Body::List { tight: true, .. }
        ) {
            self.empty_item = None;
        }
    }
    fn containers(&self, line: &mut LineStart<'_>) -> usize {
        // The document root is always open and never consumes a line prefix.
        let mut count = 1;
        for &index in &self.open[1..] {
            let indent = match self.blocks.nodes[index].body {
                Body::Quote => {
                    let save = line.clone();
                    line.scan_space(3);
                    if !line.scan_blockquote_marker() {
                        *line = save;
                        break;
                    }
                    count += 1;
                    continue;
                }
                Body::Footnote(_) => 4,
                Body::Item(indent) => indent,
                _ => {
                    count += 1;
                    continue;
                }
            };
            let save = line.clone();
            if !line.scan_space(indent) && !line.is_at_eol() {
                *line = save;
                break;
            }
            count += 1;
        }
        count
    }
    fn finish_empty(&mut self) {
        if let Some(end) = self.empty_item.take() {
            if self.blank
                && matches!(
                    self.blocks.nodes[*self.open.last().unwrap()].body,
                    Body::Item(_)
                )
            {
                self.pop(end);
            }
        }
    }
    fn finish_list(&mut self, end: usize) {
        self.finish_empty();
        if matches!(
            self.blocks.nodes[*self.open.last().unwrap()].body,
            Body::List { .. }
        ) {
            self.pop(end);
        }
        if self.blank {
            if self.open.len() > 1 {
                let grandparent = self.open[self.open.len() - 2];
                if let Body::List { tight, .. } = &mut self.blocks.nodes[grandparent].body {
                    *tight = false;
                }
            }
            self.blank = false;
        }
    }
    fn list(&mut self, start: usize, marker: u8, number: u64, line_start: usize) {
        self.finish_empty();
        let parent = *self.open.last().unwrap();
        if let Body::List {
            marker: previous,
            tight,
            ..
        } = &mut self.blocks.nodes[parent].body
        {
            if *previous == marker {
                if self.blank {
                    *tight = false;
                    self.blank = false;
                }
                return;
            }
        }
        self.finish_list(line_start);
        self.append(
            start,
            start,
            Body::List {
                tight: true,
                marker,
                start: u32::try_from(number).expect("ordered markers have at most nine digits"),
            },
        );
        self.push();
        self.blank = false;
    }
    fn fragment(&mut self, start: usize, end: usize, spaces: usize) {
        if start < end || spaces > 0 {
            self.append(start, end, Body::Fragment(spaces));
        }
    }
    fn block(&mut self, start: usize) -> usize {
        let bytes = self.source.as_bytes();
        let mut line = LineStart::new(&bytes[start..]);
        let header = core::mem::take(&mut self.table_header);
        if let Some(recorder) = &mut self.recorder {
            recorder.line = start;
            // A line that interrupted a paragraph was classified before the following line, which can still make it a table header.
            recorder.clean = !header && !core::mem::take(&mut recorder.interrupted);
            recorder.outline.closed = false;
        }
        let mut continued = self.containers(&mut line);
        // The paragraph before found that this line would continue it, lazily or not.
        if header {
            continued = self.open.len();
        }
        while self.open.len() > continued {
            self.pop(start);
        }
        let (definition_start, indent) = loop {
            let definition_start =
                start + line.bytes_scanned() - usize::from(line.remaining_space() > 0);
            let mut indent = line.scan_space_upto(4);
            if !self.options.constructs.code_indented {
                indent += line.scan_space_upto(usize::MAX);
            }
            if indent == 4 && self.options.constructs.code_indented {
                break (definition_start, indent);
            }
            let at = start + line.bytes_scanned();
            if bytes.get(at) == Some(&b'[') && self.options.constructs.footnote_definition {
                if let Some((label, end)) = footnote_prefix(bytes, at) {
                    self.finish_list(start);
                    line.advance(end - at);
                    line.scan_all_space();
                    let content = start + line.bytes_scanned();
                    let index =
                        self.definitions
                            .push_footnote(self.source, at, &Text::Source(label));
                    self.append(at, content, Body::Footnote(index));
                    self.push();
                    if let Some(length) = scan_blank_line(&bytes[content..]) {
                        return content + length;
                    }
                    continue;
                }
            }
            let marker = if matches!(bytes.get(at), Some(b'*' | b'-' | b'+' | b'0'..=b'9'))
                && self.options.constructs.list_item
            {
                line.scan_list_marker_with_indent(indent, self.options.constructs.thematic_break)
            } else {
                None
            };
            if let Some((marker, number, width)) = marker {
                let content = start + line.bytes_scanned();
                // The item starts where its indent does, at `definition_start`: `indent` counts columns, and a tab is one byte of up to four.
                self.list(definition_start, marker, number, start);
                self.append(definition_start, content, Body::Item(width));
                self.push();
                if let Some(length) = scan_blank_line(&bytes[content..]) {
                    self.empty_item = Some(content + length);
                    return content + length;
                }
            } else if self.options.constructs.block_quote && line.scan_blockquote_marker() {
                self.finish_list(start);
                self.append(at, at, Body::Quote);
                self.push();
            } else {
                // Failed marker scans leave the cursor after indentation. Keep that work for the block classifiers below.
                break (definition_start, indent);
            }
        };
        let at = start + line.bytes_scanned();
        if let Some(length) = scan_blank_line(&bytes[at..]) {
            if let Some(recorder) = &mut self.recorder {
                recorder.outline.closed = self.open.len() == 1;
            }
            let parent = *self.open.last().unwrap();
            match &mut self.blocks.nodes[parent].body {
                Body::Quote => {}
                Body::Item(indent) if self.empty_item.is_some() => {
                    *indent = 0;
                    self.blank = true;
                }
                _ => self.blank = true,
            }
            return at + length;
        }
        if indent == 4 && self.options.constructs.code_indented {
            self.finish_list(start);
            return self.indented(at, line.remaining_space());
        }
        if bytes[at] == b'<' && self.options.constructs.html_flow {
            if let Some(html) =
                html::Html::start(&bytes[at..], self.options.html_tag_namespaces, true)
            {
                self.finish_list(start);
                let html_start = if bytes.get(definition_start) == Some(&b'\t') {
                    at
                } else {
                    definition_start
                };
                return self.html(html_start, html);
            }
        }
        match bytes[at] {
            b'*' | b'_' | b'-' if self.options.constructs.thematic_break => {
                if let Ok(length) = scan_hrule(&bytes[at..]) {
                    self.finish_list(start);
                    self.append(at, at + length, Body::Rule);
                    self.settle();
                    return at + length;
                }
            }
            b'#' if self.options.constructs.heading_atx => {
                if let Some(level) = scan_atx_heading(&bytes[at..]) {
                    self.finish_list(start);
                    let end = self.heading(at, level);
                    self.settle();
                    return end;
                }
            }
            b'`' | b'~' if self.options.constructs.code_fenced => {
                if let Some((length, marker)) = scan_code_fence(&bytes[at..]) {
                    self.finish_list(start);
                    return self.fenced_block(at, indent, marker, length);
                }
            }
            _ => {}
        }
        if self.options.constructs.table {
            if let Some(delimiter_end) = self.table_head(definition_start) {
                self.finish_list(start);
                return self.table(definition_start, delimiter_end);
            }
        }
        self.paragraph(at, start, definition_start)
    }
    fn interrupts(&self, bytes: &[u8], current_container: bool) -> bool {
        interrupts(self.options, bytes, current_container)
    }
    /// Prepare the paragraph whose first line has its text at `start`, after indentation that
    /// starts at `indent`. A paragraph that starts with `[` may start with link reference
    /// definitions, which are read from the front of its text when it closes, or when an underline
    /// would make it a heading, as in `CommonMark`: its lines are judged as paragraph lines first.
    fn paragraph(&mut self, start: usize, line_start: usize, indent: usize) -> usize {
        self.finish_list(line_start);
        let bytes = self.source.as_bytes();
        let mut definitions = (bytes[start] == b'[' && self.options.constructs.definition)
            .then_some((self.current, indent));
        let mut paragraph = self.append(start, start, Body::Paragraph);
        self.push();
        let mut at = start;
        // The columns of a tab that a container prefix only partly consumed, before `at`.
        let mut spaces = 0;
        loop {
            let next = at + scan_nextline(&bytes[at..]);
            self.fragment(at, next, spaces);
            let mut line = LineStart::new(&bytes[next..]);
            let continued = self.containers(&mut line) == self.open.len();
            let content_start = next + line.bytes_scanned();
            spaces = line.remaining_space();
            let indented = line.scan_space(4) && self.options.constructs.code_indented;
            line.scan_all_space();
            if line.is_at_eol() {
                self.close_paragraph(paragraph, definitions, next);
                return next;
            }
            if !indented {
                let content = next + line.bytes_scanned();
                if matches!(bytes.get(content), Some(b'=' | b'-'))
                    && continued
                    && self.options.constructs.heading_setext
                {
                    if let Some((length, level)) = scan_setext_heading(&bytes[content..]) {
                        if let Some((before, indent)) = definitions.take() {
                            if let Some(rest) = self.definitions(paragraph, before, indent) {
                                paragraph = rest;
                            } else {
                                // With no text left, the underline is not one: it heads a table or starts the paragraph's text.
                                if let Some(recorder) = &mut self.recorder {
                                    recorder.line = next;
                                    recorder.clean = false;
                                }
                                let header = content_start - usize::from(spaces > 0);
                                if self.options.constructs.table {
                                    if let Some(end) = self.table_head(header) {
                                        return self.table(header, end);
                                    }
                                }
                                paragraph = self.append(content, content, Body::Paragraph);
                                self.push();
                                at = content;
                                spaces = 0;
                                continue;
                            }
                        }
                        self.trim_fragment();
                        self.blocks.nodes[paragraph].body = Body::Heading(level);
                        self.pop(content + length);
                        return content + length;
                    }
                }
                if self.interrupts(&bytes[content..], continued)
                    || !continued
                        && bytes.get(content) == Some(&b'<')
                        && self.options.constructs.html_flow
                        && html::Html::start(
                            &bytes[content..],
                            self.options.html_tag_namespaces,
                            true,
                        )
                        .is_some()
                {
                    if let Some(recorder) = &mut self.recorder {
                        recorder.interrupted = true;
                    }
                    self.close_paragraph(paragraph, definitions, next);
                    return next;
                }
                // As in cmark-gfm, a delimiter row only claims a line that would otherwise continue the paragraph.
                if self.options.constructs.table && self.table_delimiter(content_start).is_some() {
                    self.table_header = true;
                    self.close_paragraph(paragraph, definitions, next);
                    return next;
                }
            }
            at = content_start;
        }
    }
    /// Close `paragraph` before the line at `next`, after reading the definitions it starts with
    /// when `definitions` holds its previous sibling and the start of its indentation.
    fn close_paragraph(
        &mut self,
        paragraph: usize,
        definitions: Option<(usize, usize)>,
        next: usize,
    ) {
        let paragraph = match definitions {
            Some((before, indent)) => self.definitions(paragraph, before, indent),
            None => Some(paragraph),
        };
        if paragraph.is_some() {
            self.trim_fragment();
            self.pop(next);
        }
    }
    /// Read the link reference definitions at the front of `paragraph`, the open block whose
    /// fragments are the last nodes, whose previous sibling is `before`, and whose first line's
    /// indentation starts at `indent`. Each definition becomes a block before the paragraph, which
    /// keeps the lines after them: the paragraph, rebuilt when any definition was read, stays open,
    /// unless no line is left, when it is removed.
    fn definitions(&mut self, paragraph: usize, before: usize, indent: usize) -> Option<usize> {
        let source = self.source;
        let bytes = source.as_bytes();
        let mut lines = core::mem::take(&mut self.lines);
        lines.clear();
        lines.extend(self.blocks.nodes[paragraph + 1..].iter().map(|node| {
            let Body::Fragment(spaces) = node.body else {
                unreachable!("a paragraph holds only fragments")
            };
            (node.start..node.end, spaces)
        }));
        let mut first = 0;
        let mut found = Vec::new();
        while first < lines.len() {
            let range = &lines[first].0;
            let text = range.start
                + bytes[range.clone()]
                    .iter()
                    .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
                    .count();
            if bytes.get(text) != Some(&b'[') {
                break;
            }
            let Some(metadata) =
                definition::scan(source, lines[first..].iter().cloned(), self.options)
            else {
                break;
            };
            let start = if first == 0 {
                indent
            } else {
                range.start - usize::from(lines[first].1 > 0)
            };
            let line = range.start;
            while first < lines.len() && lines[first].0.end <= metadata.end {
                first += 1;
            }
            found.push((start, line, metadata));
        }
        if found.is_empty() {
            self.lines = lines;
            return Some(paragraph);
        }
        // Rebuild the blocks from the paragraph's place: its first block keeps the paragraph's outline entry.
        self.blocks.nodes.truncate(paragraph);
        self.open.pop();
        self.current = before;
        let mut first_line = None;
        if let Some(recorder) = &mut self.recorder {
            if let Some(block) = recorder
                .outline
                .blocks
                .pop_if(|block| block.node == paragraph)
            {
                first_line = Some((block.line, block.clean));
            }
        }
        for (count, (start, line, metadata)) in found.into_iter().enumerate() {
            self.record(count == 0, first_line, line);
            let index = self.definitions.push(
                source,
                start,
                metadata.label,
                metadata.destination,
                metadata.title,
            );
            self.append(start, metadata.end, Body::Definition(index));
        }
        let rest = (first < lines.len()).then(|| {
            let (range, _) = lines[first].clone();
            self.record(false, first_line, range.start);
            let text = range.start
                + bytes[range.clone()]
                    .iter()
                    .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
                    .count();
            let paragraph = self.append(text, text, Body::Paragraph);
            self.push();
            self.fragment(text, range.end, 0);
            for (range, spaces) in &lines[first + 1..] {
                self.fragment(range.start, range.end, *spaces);
            }
            paragraph
        });
        self.lines = lines;
        rest
    }
    /// Set the outline line of the next document-level block that rebuilding a paragraph appends: the paragraph's own for its first block, and otherwise that of `line`, after the first, which is not clean.
    fn record(&mut self, first: bool, paragraph: Option<(usize, bool)>, line: usize) {
        if let Some(recorder) = &mut self.recorder {
            match paragraph {
                Some((start, clean)) if first => {
                    recorder.line = start;
                    recorder.clean = clean;
                }
                _ => {
                    recorder.line = line;
                    recorder.clean = false;
                }
            }
        }
    }
    fn trim_fragment(&mut self) {
        let node = &mut self.blocks.nodes[self.current];
        while node.end > node.start
            && matches!(
                self.source.as_bytes()[node.end - 1],
                b' ' | b'\t' | b'\r' | b'\n'
            )
        {
            node.end -= 1;
        }
    }
    /// Recognize the delimiter row before allocating any table nodes.
    fn table_delimiter(&self, start: usize) -> Option<(usize, Range<usize>)> {
        let bytes = self.source.as_bytes();
        let next = start + scan_nextline(&bytes[start..]);
        let mut line = LineStart::new(&bytes[next..]);
        if self.containers(&mut line) != self.open.len()
            || (self.options.constructs.code_indented && line.scan_space(4))
        {
            return None;
        }
        if !self.options.constructs.code_indented {
            line.scan_all_space();
        }
        let at = next + line.bytes_scanned();
        if !matches!(bytes.get(at), Some(b'|' | b'-' | b':'))
            || LineStart::new(&bytes[at..])
                .scan_list_marker_with_indent(0, self.options.constructs.thematic_break)
                .is_some()
        {
            return None;
        }
        let end = at + scan_nextline(&bytes[at..]);
        let delimiter_end = terminal_ending_start(bytes, end);
        let header_end = terminal_ending_start(bytes, next);
        table_start(&bytes[start..header_end], &bytes[at..delimiter_end])
            .then_some((end, at..delimiter_end))
    }
    fn table_head(&mut self, start: usize) -> Option<usize> {
        let (end, delimiter) = self.table_delimiter(start)?;
        self.alignments.clear();
        for (_, content) in table::Cells::new(self.source.as_bytes(), delimiter) {
            self.alignments.push(
                table::alignment(&self.source.as_bytes()[content])
                    .expect("delimiter alignment was validated"),
            );
        }
        Some(end)
    }
    fn table_row(&mut self, start: usize, next: usize, header: bool) {
        let bytes = self.source.as_bytes();
        let end = terminal_ending_start(bytes, next);
        self.append(start, next, Body::TableRow);
        self.push();
        let mut cells = table::Cells::new(bytes, start..end);
        for index in 0..self.alignments.len() {
            let (span, content) = cells.next().unwrap_or((end..end, end..end));
            self.append(
                span.start,
                span.end,
                Body::TableCell {
                    alignment: self.alignments[index],
                    header,
                },
            );
            self.push();
            self.fragment(content.start, content.end, 0);
            self.pop(span.end);
        }
        self.pop(next);
    }
    fn table(&mut self, start: usize, delimiter_end: usize) -> usize {
        let bytes = self.source.as_bytes();
        let header_end = start + scan_nextline(&bytes[start..]);
        self.append(start, delimiter_end, Body::Table);
        self.push();
        self.append(start, delimiter_end, Body::TableHead);
        self.push();
        self.table_row(start, header_end, true);
        self.pop(delimiter_end);
        let mut at = delimiter_end;
        let mut body = false;
        while at < bytes.len() {
            let mut line = LineStart::new(&bytes[at..]);
            if self.containers(&mut line) != self.open.len() {
                break;
            }
            let row = at + line.bytes_scanned();
            if self.options.constructs.code_indented && line.scan_space(4) {
                break;
            }
            if !self.options.constructs.code_indented {
                line.scan_all_space();
            }
            let content = at + line.bytes_scanned();
            if self.interrupts(&bytes[content..], false)
                || bytes.get(content) == Some(&b'<')
                    && self.options.constructs.html_flow
                    && html::Html::start(&bytes[content..], self.options.html_tag_namespaces, true)
                        .is_some()
            {
                break;
            }
            let next = content + scan_nextline(&bytes[content..]);
            if !body {
                self.append(row, row, Body::TableBody);
                self.push();
                body = true;
            }
            self.table_row(row, next, false);
            at = next;
        }
        if body {
            self.pop(at);
        }
        self.pop(at);
        at
    }
    fn html(&mut self, start: usize, kind: html::Html) -> usize {
        self.append(start, start, Body::Html(start));
        self.push();
        let bytes = self.source.as_bytes();
        let retain_blanks = kind.blank_terminated() && self.options.html_block_blank_lines;
        let mut minimum_indent = 0;
        let mut at = start;
        let mut spaces = 0;
        loop {
            let next = at + scan_nextline(&bytes[at..]);
            if retain_blanks {
                let mut indent = LineStart::new(&bytes[at..]);
                indent.scan_all_space();
                // Widths are those of the line as the value stores it, which `html_block_value`
                // measures the same way.
                let width = html_indent_width(spaces, &bytes[at..]);
                if !indent.is_at_eol()
                    && width > 0
                    && (minimum_indent == 0 || width < minimum_indent)
                {
                    minimum_indent = width;
                }
            }
            self.fragment(at, next, spaces);
            let closed = kind.closed(&bytes[at..next]);
            let mut line = LineStart::new(&bytes[next..]);
            let continued = self.containers(&mut line) == self.open.len();
            let content = next + line.bytes_scanned();
            if closed || next == bytes.len() || !continued {
                return self.finish_html(next, retain_blanks);
            }
            if kind.blank_terminated() && scan_blank_line(&bytes[content..]).is_some() {
                if !retain_blanks {
                    return self.finish_html(next, false);
                }
                let mut blank = next;
                let mut prefix = line;
                loop {
                    let begin = blank + prefix.bytes_scanned();
                    let end = begin + scan_nextline(&bytes[begin..]);
                    self.fragment(begin, end, prefix.remaining_space());
                    if end == bytes.len() {
                        return self.finish_html(end, true);
                    }
                    prefix = LineStart::new(&bytes[end..]);
                    if self.containers(&mut prefix) != self.open.len() {
                        return self.finish_html(end, true);
                    }
                    let begin = end + prefix.bytes_scanned();
                    let mut indentation = prefix.clone();
                    indentation.scan_all_space();
                    if !indentation.is_at_eol() {
                        // The line keeps what is left of a tab its containers took part of.
                        let width = html_indent_width(prefix.remaining_space(), &bytes[begin..]);
                        if width == 0 || (minimum_indent > 0 && width < minimum_indent) {
                            return self.finish_html(end, true);
                        }
                        at = begin;
                        spaces = prefix.remaining_space();
                        break;
                    }
                    blank = end;
                }
            } else {
                at = content;
                spaces = 0;
            }
        }
    }
    fn finish_html(&mut self, next: usize, retain_ending: bool) -> usize {
        let last = &mut self.blocks.nodes[self.current];
        let bytes = self.source.as_bytes();
        if !retain_ending {
            last.end = terminal_ending_start(bytes, last.end);
        }
        let end = last.end;
        self.pop(next);
        self.blocks.nodes[self.current].body = Body::Html(end);
        next
    }
    fn heading(&mut self, start: usize, level: u8) -> usize {
        let bytes = self.source.as_bytes();
        let end = start + scan_nextline(&bytes[start..]);
        let mut content = start + usize::from(level);
        content += scan_whitespace_no_nl(&bytes[content..end]);
        let mut last = end;
        while last > content && matches!(bytes[last - 1], b' ' | b'\t' | b'\r' | b'\n') {
            last -= 1;
        }
        let mut closer = last;
        while closer > content && bytes[closer - 1] == b'#' {
            closer -= 1;
        }
        if closer == content || (closer < last && matches!(bytes[closer - 1], b' ' | b'\t')) {
            last = closer;
            while last > content && matches!(bytes[last - 1], b' ' | b'\t') {
                last -= 1;
            }
        }
        self.append(start, end, Body::Heading(level));
        self.push();
        self.fragment(content, last, 0);
        self.pop(end);
        end
    }
    /// Record that a clean document-level block closed on its own lines. Later lines are prepared as if the document began after it.
    fn settle(&mut self) {
        if let Some(recorder) = &mut self.recorder {
            recorder.outline.closed = recorder.clean && self.open.len() == 1;
        }
    }
    fn fenced_block(&mut self, start: usize, indent: usize, marker: u8, length: usize) -> usize {
        let end = self.fenced(start, indent, marker, length);
        if !self
            .blocks
            .codes
            .last()
            .expect("fences record code")
            .unclosed
        {
            self.settle();
        }
        end
    }
    fn fenced(&mut self, start: usize, indent: usize, marker: u8, length: usize) -> usize {
        let bytes = self.source.as_bytes();
        let mut info = start + length;
        info += scan_whitespace_no_nl(&bytes[info..]);
        let mut at = info + scan_nextline(&bytes[info..]);
        // The info string is the rest of the line, trimmed of spaces and tabs at both edges.
        let mut info_end = info
            + bytes[info..at]
                .iter()
                .position(|byte| matches!(byte, b'\r' | b'\n'))
                .unwrap_or(at - info);
        while info_end > info && is_ascii_whitespace_no_nl(bytes[info_end - 1]) {
            info_end -= 1;
        }
        let code = self.blocks.codes.len();
        self.blocks.codes.push(Code {
            info: info..info_end,
            fence: Some(CodeFence {
                character: char::from(marker),
                length,
                indent,
            }),
            unclosed: true,
        });
        self.append(start, start, Body::Code(code));
        self.push();
        loop {
            if at == bytes.len() {
                self.pop(at);
                return at;
            }
            let mut line = LineStart::new(&bytes[at..]);
            if self.containers(&mut line) != self.open.len() {
                self.pop(at);
                return at;
            }
            line.scan_space(indent);
            let mut closer = line.clone();
            let closing_allowed = if self.options.constructs.code_indented {
                !closer.scan_space(4usize.saturating_sub(indent))
            } else {
                closer.scan_all_space();
                true
            };
            if closing_allowed {
                let close = at + closer.bytes_scanned();
                if let Some(length) = scan_closing_code_fence(&bytes[close..], marker, length) {
                    let end = close + length;
                    let next = end + scan_blank_line(&bytes[end..]).unwrap_or(0);
                    self.blocks.codes[code].unclosed = false;
                    self.pop(next);
                    return next;
                }
            }
            let content = at + line.bytes_scanned();
            let next = content + scan_nextline(&bytes[content..]);
            self.fragment(content, next, line.remaining_space());
            at = next;
            if at == bytes.len() {
                self.pop(at);
                return at;
            }
        }
    }
    fn indented(&mut self, start: usize, mut spaces: usize) -> usize {
        let code = self.blocks.codes.len();
        self.blocks.codes.push(Code {
            info: start..start,
            fence: None,
            unclosed: false,
        });
        self.append(start, start, Body::Code(code));
        self.push();
        let bytes = self.source.as_bytes();
        let mut at = start;
        let mut last_content = 0;
        let mut end = start;
        self.blank = false;
        loop {
            let next = at + scan_nextline(&bytes[at..]);
            self.fragment(at, next, spaces);
            if !self.blank {
                last_content = self.current;
                end = next;
            }
            let mut line = LineStart::new(&bytes[next..]);
            if self.containers(&mut line) != self.open.len()
                || !(line.scan_space(4) || line.is_at_eol())
            {
                at = next;
                break;
            }
            at = next + line.bytes_scanned();
            if at == bytes.len() {
                break;
            }
            spaces = line.remaining_space();
            self.blank = scan_blank_line(&bytes[at..]).is_some();
        }
        // The first line is nonblank, so every indented block has a content fragment.
        self.blocks.nodes[last_content].next = 0;
        self.pop(end);
        at
    }
}

/// The extent of the frontmatter that `source` starts with, after a byte order mark, from its
/// opening fence through its closing fence, not counting the line ending after that.
pub(crate) fn frontmatter_extent(source: &str) -> Option<core::ops::Range<usize>> {
    let start = usize::from(source.starts_with('\u{feff}')) * 3;
    frontmatter::scan(source.as_bytes(), start).map(|(end, _)| start..end)
}

/// Whether `source`, which starts with `[` at the start of a paragraph and has no container
/// prefixes, starts with a definition.
pub(crate) fn starts_definition(source: &str) -> bool {
    definition::scan(
        source,
        core::iter::once((0..source.len(), 0)),
        &ParseOptions::default(),
    )
    .is_some()
}

/// Whether `source`, a definition without a title on its first line and the lines of a block
/// after it, with no container prefixes, reads back as a definition with a title from those lines.
pub(crate) fn definition_has_title(source: &str) -> bool {
    let lines = source.split_inclusive('\n').scan(0, |at, line| {
        let range = *at..*at + line.len();
        *at = range.end;
        Some((range, 0))
    });
    definition::scan(source, lines, &ParseOptions::default())
        .is_some_and(|definition| definition.title.is_some())
}

/// Whether `bytes`, at the start of a line, starts a footnote definition, which can interrupt a
/// paragraph.
pub(crate) fn starts_footnote_definition(bytes: &[u8]) -> bool {
    footnote_prefix(bytes, 0).is_some()
}

/// Whether `delimiter`, the content of a line after its indentation that starts with `|`, `-` or
/// `:` and is not a list item, is a delimiter row with as many cells as the row `header` on the
/// line before it, so that the two lines start a table. Neither includes its line ending.
fn table_start(header: &[u8], delimiter: &[u8]) -> bool {
    delimiter_columns(delimiter).is_some_and(|columns| {
        &header[table::trim(header, 0..header.len())] != b"|"
            && table::Cells::new(header, 0..header.len()).count() == columns
    })
}

/// The number of columns of the table whose delimiter row `delimiter` would be, if its cells are
/// one's.
fn delimiter_columns(delimiter: &[u8]) -> Option<usize> {
    if !delimiter.contains(&b'|') && !delimiter.contains(&b':') {
        return None;
    }
    let mut columns = 0;
    for (_, content) in table::Cells::new(delimiter, 0..delimiter.len()) {
        table::alignment(&delimiter[content])?;
        columns += 1;
    }
    Some(columns)
}

/// Whether the line `delimiter`, without container prefixes, indentation or line ending, would be
/// a table's delimiter row under a header with as many cells, with the default options.
pub(crate) fn delimiter_row(delimiter: &[u8]) -> bool {
    matches!(delimiter.first(), Some(b'|' | b'-' | b':'))
        && LineStart::new(delimiter)
            .scan_list_marker_with_indent(0, true)
            .is_none()
        && delimiter_columns(delimiter).is_some()
}

/// Whether the line `delimiter` after the line `header`, both without container prefixes or line
/// endings, would make the two lines start a table, which can interrupt a paragraph.
pub(crate) fn starts_table(header: &str, delimiter: &str) -> bool {
    let delimiter = delimiter.as_bytes();
    matches!(delimiter.first(), Some(b'|' | b'-' | b':'))
        && LineStart::new(delimiter)
            .scan_list_marker_with_indent(0, true)
            .is_none()
        && table_start(header.as_bytes(), delimiter)
}

/// Whether `bytes`, at the start of a block, would start an HTML block only because the rest
/// of its first line is blank (kind 7), so that a line ending written as a character reference
/// keeps it from starting one.
pub(crate) fn complete_tag_starts_html_block(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'<')
        && html::Html::start(bytes, false, false).is_none()
        && html::Html::start(bytes, false, true).is_some()
}

/// Whether HTML at the start of a line would start an HTML block inside a paragraph (kinds 1–6).
/// Only a value that starts with `<` can: a hand-built value need not, and an empty one has no
/// name to read.
pub(crate) fn html_interrupts_paragraph(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'<') && html::Html::start(bytes, false, false).is_some()
}

/// Whether an HTML block whose value is `value` ends with its last line, so that the line after
/// it can start another block: one of kinds 1–5 whose end condition is on its last line. A block
/// of kind 6 or 7 ends only at a blank line, one of kinds 1–5 that its last line does not close
/// takes the lines after it until one does, and a value that starts no HTML block, which only a
/// hand-built tree has, is written as a paragraph, which the next block may continue.
pub(crate) fn html_block_ends_on_last_line(value: &str) -> bool {
    let bytes = value.trim_start_matches([' ', '\t']).as_bytes();
    let last = memchr::memrchr(b'\n', bytes).map_or(0, |at| at + 1);
    bytes.first() == Some(&b'<')
        && html::Html::start(bytes, false, true)
            .is_some_and(|kind| !kind.blank_terminated() && kind.closed(&bytes[last..]))
}

/// Whether an HTML block whose value is `value` is one of kinds 1–5 that its last line does not
/// close, so that it takes every line after it in its container, blank or not, until one does.
pub(crate) fn html_block_unclosed(value: &str) -> bool {
    let bytes = value.trim_start_matches([' ', '\t']).as_bytes();
    let last = memchr::memrchr(b'\n', bytes).map_or(0, |at| at + 1);
    // Namespaces change only which tags start a block of kind 7, which takes no such lines.
    bytes.first() == Some(&b'<')
        && html::Html::start(bytes, false, true)
            .is_some_and(|kind| !kind.blank_terminated() && !kind.closed(&bytes[last..]))
}

/// Whether `value`, an HTML block's, ends in an empty line whose line ending is not in it: one of
/// kinds 1–5 that ends in a blank line it took. One of kind 6 or 7 ends in a line feed only where
/// `ParseOptions::html_block_blank_lines` keeps its final line ending in it.
pub(crate) fn html_block_ends_in_empty_line(value: &str) -> bool {
    let bytes = value.trim_start_matches([' ', '\t']).as_bytes();
    // Namespaces change only which tags start a block of kind 7.
    value.ends_with('\n')
        && html::Html::start(bytes, false, true).is_some_and(|kind| !kind.blank_terminated())
}

/// The width of the spaces and tabs that start `bytes` after `spaces` spaces, with a tab to the
/// next multiple of four counted from where the spaces start.
fn html_indent_width(spaces: usize, bytes: &[u8]) -> usize {
    let mut width = spaces;
    for &byte in bytes {
        match byte {
            b' ' => width += 1,
            b'\t' => width += 4 - width % 4,
            _ => break,
        }
    }
    width
}

/// Whether `value`, which holds no CR, is the value of one HTML block as the parser stores it.
///
/// It starts with the indentation the parser keeps before a block: up to three spaces, or one or
/// two spaces then a tab and spaces, where those spaces and the tab, counted as one column, are at
/// most three, as a tab first is not kept, or, without `Constructs::code_indented`, which makes
/// more indentation code, any spaces and tabs. HTML that starts a block follows, then the lines the block takes, with no line
/// ending after the last. A block of kinds 1–5 ends at the first line that holds its end
/// condition, so no line before its last holds it; one that no line closes takes every line to
/// the end of its container, blank or not, so its last line may be blank. A block of kind 6 or 7
/// ends before a blank line, so it holds none, except where `blank_lines`, as
/// `ParseOptions::html_block_blank_lines` continues it: a run of blank lines before a line
/// indented by at least one column, and by at least the least positive indentation of the lines
/// before it that are not blank, or at its end, which that option keeps with the line ending
/// after it. Namespace prefixes in tag names start a block of kind 7 when `namespaces`, as
/// `ParseOptions::html_tag_namespaces` reads them. Without `options`, each of these is read as
/// some options read it.
pub(crate) fn html_block_value(value: &str, options: Option<&crate::ParseOptions>) -> bool {
    let namespaces = options.map_or(true, |options| options.html_tag_namespaces);
    let blank_lines = options.map_or(true, |options| options.html_block_blank_lines);
    let limited = options.is_some_and(|options| options.constructs.code_indented);
    let bytes = value.as_bytes();
    let spaces = bytes.iter().take_while(|&&byte| byte == b' ').count();
    let mut indent = spaces;
    if !limited {
        indent = bytes
            .iter()
            .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
            .count();
    } else if spaces > 0 && bytes.get(spaces) == Some(&b'\t') {
        indent += 1;
        let after = bytes[indent..]
            .iter()
            .take_while(|&&byte| byte == b' ')
            .count();
        if spaces + 1 + after > 3 {
            return false;
        }
        indent += after;
    }
    let html = &bytes[indent..];
    if (limited && spaces > 3) || html.first() != Some(&b'<') {
        return false;
    }
    let Some(kind) = html::Html::start(html, namespaces, true) else {
        return false;
    };
    let mut lines = bytes.split(|&byte| byte == b'\n').peekable();
    let mut blank = false;
    let mut minimum_indent = 0;
    while let Some(line) = lines.next() {
        let after_blank = blank;
        blank = line.iter().all(|byte| matches!(byte, b' ' | b'\t'));
        let last = lines.peek().is_none();
        if kind.blank_terminated() {
            if blank {
                if !blank_lines {
                    return false;
                }
                continue;
            }
            let width = html_indent_width(0, line);
            if after_blank && (width == 0 || width < minimum_indent) {
                return false;
            }
            if width > 0 && (minimum_indent == 0 || width < minimum_indent) {
                minimum_indent = width;
            }
        } else if !last && kind.closed(line) {
            return false;
        }
    }
    true
}

/// Whether `value`, a code block's, is one that indented code gives: not empty, and starting and
/// ending with a line that is not blank, as the blank lines around indented code are not in it.
pub(crate) fn indented_code_value(value: &str) -> bool {
    let content = |line: &str| !line.trim_matches([' ', '\t']).is_empty();
    let lines = value.strip_suffix('\n').unwrap_or(value);
    lines.split('\n').next().is_some_and(content) && lines.rsplit('\n').next().is_some_and(content)
}

pub(crate) fn footnote_prefix(bytes: &[u8], start: usize) -> Option<(Range<usize>, usize)> {
    if !bytes[start..].starts_with(b"[^") {
        return None;
    }
    let begin = start + 2;
    let mut at = begin;
    while at < bytes.len() && at - begin <= crate::util::constant::LINK_REFERENCE_SIZE_MAX {
        match bytes[at] {
            b']' if at > begin && bytes.get(at + 1) == Some(&b':') => {
                return Some((begin..at, at + 2))
            }
            b']' | b'[' | b' ' | b'\t' | b'\r' | b'\n' => return None,
            b'\\' if matches!(bytes.get(at + 1), Some(b'[' | b']' | b'\\')) => at += 1,
            _ => {}
        }
        at += 1;
    }
    None
}

fn interrupts(options: &ParseOptions, bytes: &[u8], current_container: bool) -> bool {
    let constructs = &options.constructs;
    match bytes.first().copied() {
        None | Some(b'\r' | b'\n') => true,
        Some(b'>') => constructs.block_quote,
        Some(b'<') => {
            constructs.html_flow
                && html::Html::start(bytes, options.html_tag_namespaces, false).is_some()
        }
        Some(b'[') => constructs.footnote_definition && footnote_prefix(bytes, 0).is_some(),
        Some(b'#') => constructs.heading_atx && scan_atx_heading(bytes).is_some(),
        Some(b'`' | b'~') => constructs.code_fenced && scan_code_fence(bytes).is_some(),
        Some(b'*' | b'_' | b'-') if constructs.thematic_break && scan_hrule(bytes).is_ok() => true,
        Some(b'*' | b'-' | b'+' | b'0'..=b'9') if constructs.list_item => {
            list_interrupts(options, bytes, current_container)
        }
        _ => false,
    }
}
// List lookahead needs an aligned line cursor; ordinary interruption dispatch does not need its stack frame.
#[inline(never)]
fn list_interrupts(options: &ParseOptions, bytes: &[u8], current_container: bool) -> bool {
    let mut line = LineStart::new(bytes);
    line.scan_list_marker_with_indent(0, options.constructs.thematic_break)
        .is_some_and(|(marker, number, _)| {
            !current_container
                || ((matches!(marker, b'*' | b'-' | b'+') || number == 1)
                    && scan_blank_line(&bytes[line.bytes_scanned()..]).is_none())
        })
}

/// Whether a line that continues a paragraph in the same containers, from its first byte other
/// than a space or tab, starts a block instead, as `paragraph` decides with the default
/// constructs: a setext underline, or a block that interrupts a paragraph. A line that continues
/// a `definition` counts any list item as interrupting it. The parser reads a definition from a
/// paragraph's text, where only a list item that may interrupt a paragraph does (#249), so this
/// is the stricter reading, which rejects a label that either would end. Indentation, which keeps
/// any of these from starting, is not counted here.
pub(crate) fn continuation_starts_block(bytes: &[u8], definition: bool) -> bool {
    let options = ParseOptions::default();
    (matches!(bytes.first(), Some(b'=' | b'-')) && scan_setext_heading(bytes).is_some())
        || interrupts(&options, bytes, !definition)
}
