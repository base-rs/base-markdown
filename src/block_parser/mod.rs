// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Copyright 2015-2018 Google Inc. and Google LLC.
// Copyright 2026 Samuel Williams.
// Block/container algorithms adapted from pulldown-cmark revision 38e4d08f14ec4bd9783270e9623db7681ebed968.

//! Compact block preparation with source fragments and deferred inline scanning.
mod definition;
mod frontmatter;
mod html;
mod scanners;
mod table;
use crate::definitions::Definitions;
use crate::stream::Text;
use crate::ParseOptions;
use crate::{AlignKind, CodeFence};
use alloc::vec;
use alloc::vec::Vec;
use core::{convert::TryFrom, ops::Range};
use scanners::{
    is_ascii_whitespace, scan_atx_heading, scan_blank_line, scan_closing_code_fence,
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
            table::line_end(source.as_bytes(), self.end)
        } else {
            self.end
        }
    }
}

pub(super) struct Code {
    pub info: Range<usize>,
    pub fence: Option<CodeFence>,
    pub unclosed: bool,
    pub math: bool,
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
    definition_context: bool,
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
            definitions: Definitions::default(),
            alignments: Vec::new(),
            definition_context: false,
        };
        builder.blocks.nodes.push(Node {
            start: 0,
            end: source.len(),
            child: 0,
            next: 0,
            parent: 0,
            body: Body::Root,
        });
        let mut index = usize::from(source.starts_with('\u{feff}')) * 3;
        if options.constructs.frontmatter {
            if let Some((end, next)) = frontmatter::scan(source.as_bytes(), index) {
                builder.append(index, end, Body::Opaque);
                index = next;
            }
        }
        while index < source.len() {
            index = builder.block(index);
        }
        while builder.open.len() > 1 {
            builder.pop(index);
        }
        builder.definitions.finish();
        (builder.blocks, builder.definitions)
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
        let content_context = self.definition_context;
        let mut continued = self.containers(&mut line);
        if content_context && continued < self.open.len() {
            let mut probe = line.clone();
            probe.scan_all_space();
            if !probe.is_at_eol()
                && !self.interrupts(&bytes[start + probe.bytes_scanned()..], false)
            {
                continued = self.open.len();
            }
        }
        self.definition_context = false;
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
                if content_context {
                    line.scan_all_space();
                }
                break (definition_start, indent);
            }
            let at = start + line.bytes_scanned();
            if bytes.get(at) == Some(&b'[') && self.options.constructs.footnote_definition {
                if let Some((label, end)) = footnote_prefix(bytes, at) {
                    self.finish_list(start);
                    line.advance(end - at);
                    line.scan_all_space();
                    let content = start + line.bytes_scanned();
                    let index = self
                        .definitions
                        .push_footnote(self.source, &Text::Source(label));
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
                self.list(at - indent, marker, number, start);
                self.append(at - indent, content, Body::Item(width));
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
        if indent == 4 && self.options.constructs.code_indented && !content_context {
            self.finish_list(start);
            return self.indented(at, line.remaining_space());
        }
        if content_context
            && self.options.constructs.heading_setext
            && scan_setext_heading(&bytes[at..]).is_some()
        {
            return self.paragraph(at, start);
        }
        if bytes[at] == b'[' && self.options.constructs.definition {
            if self.options.constructs.table && table_definition_header(&bytes[at..]) {
                if let Some(end) = self.table_head(definition_start) {
                    self.finish_list(start);
                    return self.table(definition_start, end);
                }
            }
            let metadata =
                definition::scan(self.source, self.definition_fragments(at), self.options);
            if let Some(metadata) = metadata {
                self.finish_list(start);
                let index = self.definitions.push(
                    self.source,
                    metadata.label,
                    metadata.destination,
                    metadata.title,
                );
                self.append(definition_start, metadata.end, Body::Definition(index));
                self.definition_context = true;
                return metadata.end;
            }
        }
        if indent == 4 && self.options.constructs.code_indented && content_context {
            return self.paragraph(at, start);
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
                    return at + length;
                }
            }
            b'#' if self.options.constructs.heading_atx => {
                if let Some(level) = scan_atx_heading(&bytes[at..]) {
                    self.finish_list(start);
                    return self.heading(at, level);
                }
            }
            b'`' | b'~' if self.options.constructs.code_fenced => {
                if let Some((length, marker)) = scan_code_fence(&bytes[at..]) {
                    self.finish_list(start);
                    return self.fenced(at, indent, marker, length);
                }
            }
            b'$' if self.options.constructs.math_flow => {
                if let Some(length) = math_fence(&bytes[at..]) {
                    self.finish_list(start);
                    return self.fenced(at, indent, b'$', length);
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
        self.paragraph(at, start)
    }
    fn definition_fragments(
        &self,
        start: usize,
    ) -> impl Iterator<Item = Range<usize>> + Clone + '_ {
        let mut at = Some((start, false));
        core::iter::from_fn(move || {
            let bytes = self.source.as_bytes();
            let (start, continuation) = at.take()?;
            let current = if continuation {
                let mut line = LineStart::new(&bytes[start..]);
                self.containers(&mut line);
                let candidate = start + line.bytes_scanned();
                let indented = line.scan_space(4) && self.options.constructs.code_indented;
                line.scan_all_space();
                if line.is_at_eol() {
                    return None;
                }
                let content = start + line.bytes_scanned();
                let interrupted = !indented
                    && ((self.options.constructs.heading_setext
                        && scan_setext_heading(&bytes[content..]).is_some())
                        || self.interrupts(&bytes[content..], false)
                        || (self.options.constructs.table
                            && self.table_delimiter(candidate).is_some()));
                if interrupted {
                    return None;
                }
                candidate
            } else {
                start
            };
            let end = current + scan_nextline(&bytes[current..]);
            if end < bytes.len() {
                at = Some((end, true));
            }
            Some(current..end)
        })
    }

    fn interrupts(&self, bytes: &[u8], current_container: bool) -> bool {
        let constructs = &self.options.constructs;
        match bytes.first().copied() {
            None | Some(b'\r' | b'\n') => true,
            Some(b'>') => constructs.block_quote,
            Some(b'<') => {
                constructs.html_flow
                    && html::Html::start(bytes, self.options.html_tag_namespaces, false).is_some()
            }
            Some(b'[') => constructs.footnote_definition && footnote_prefix(bytes, 0).is_some(),
            Some(b'#') => constructs.heading_atx && scan_atx_heading(bytes).is_some(),
            Some(b'`' | b'~') => constructs.code_fenced && scan_code_fence(bytes).is_some(),
            Some(b'$') => constructs.math_flow && math_fence(bytes).is_some(),
            Some(b'*' | b'_' | b'-') if constructs.thematic_break && scan_hrule(bytes).is_ok() => {
                true
            }
            Some(b'*' | b'-' | b'+' | b'0'..=b'9') if constructs.list_item => {
                self.list_interrupts(bytes, current_container)
            }
            _ => false,
        }
    }
    // List lookahead needs an aligned line cursor; ordinary interruption dispatch does not need its stack frame.
    #[inline(never)]
    fn list_interrupts(&self, bytes: &[u8], current_container: bool) -> bool {
        let mut line = LineStart::new(bytes);
        line.scan_list_marker_with_indent(0, self.options.constructs.thematic_break)
            .is_some_and(|(marker, number, _)| {
                !current_container
                    || ((matches!(marker, b'*' | b'-' | b'+') || number == 1)
                        && scan_blank_line(&bytes[line.bytes_scanned()..]).is_none())
            })
    }
    fn paragraph(&mut self, start: usize, line_start: usize) -> usize {
        self.finish_list(line_start);
        let paragraph = self.append(start, start, Body::Paragraph);
        self.push();
        let bytes = self.source.as_bytes();
        let mut at = start;
        loop {
            let next = at + scan_nextline(&bytes[at..]);
            self.fragment(at, next, 0);
            let mut line = LineStart::new(&bytes[next..]);
            let continued = self.containers(&mut line) == self.open.len();
            let content_start = next + line.bytes_scanned();
            let indented = line.scan_space(4) && self.options.constructs.code_indented;
            line.scan_all_space();
            if line.is_at_eol() {
                self.trim_fragment();
                self.pop(next);
                return next;
            }
            if !indented {
                let content = next + line.bytes_scanned();
                if self.options.constructs.table && self.table_delimiter(content_start).is_some() {
                    self.definition_context = true;
                    self.trim_fragment();
                    self.pop(next);
                    return next;
                }
                if matches!(bytes.get(content), Some(b'=' | b'-'))
                    && continued
                    && self.options.constructs.heading_setext
                {
                    if let Some((length, level)) = scan_setext_heading(&bytes[content..]) {
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
                    self.trim_fragment();
                    self.pop(next);
                    return next;
                }
            }
            at = content_start;
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
        let delimiter_end = table::line_end(bytes, end);
        let delimiter = &bytes[at..delimiter_end];
        if !delimiter.contains(&b'|') && !delimiter.contains(&b':') {
            return None;
        }
        let mut columns = 0;
        for (_, content) in table::Cells::new(bytes, at..delimiter_end) {
            table::alignment(&bytes[content])?;
            columns += 1;
        }
        let header_end = table::line_end(bytes, next);
        if &bytes[table::trim(bytes, start..header_end)] == b"|"
            || table::Cells::new(bytes, start..header_end).count() != columns
        {
            return None;
        }
        Some((end, at..delimiter_end))
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
        let end = table::line_end(bytes, next);
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
                let width = indent.scan_space_upto(usize::MAX) + spaces;
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
                return self.finish_html(
                    next,
                    retain_blanks || next == bytes.len() && !closed && !kind.blank_terminated(),
                );
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
                    let width = indentation.scan_space_upto(usize::MAX);
                    if !indentation.is_at_eol() {
                        if width == 0 || (minimum_indent > 0 && width < minimum_indent) {
                            return self.finish_html(end, true);
                        }
                        at = begin;
                        spaces = 0;
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
            if bytes.get(last.end.wrapping_sub(1)) == Some(&b'\n') {
                last.end -= 1;
            }
            if bytes.get(last.end.wrapping_sub(1)) == Some(&b'\r') {
                last.end -= 1;
            }
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
    fn fenced(&mut self, start: usize, indent: usize, marker: u8, length: usize) -> usize {
        let bytes = self.source.as_bytes();
        let mut info = start + length;
        info += scan_whitespace_no_nl(&bytes[info..]);
        let mut at = info + scan_nextline(&bytes[info..]);
        let info_end = info
            + bytes[info..at]
                .iter()
                .position(|byte| is_ascii_whitespace(*byte))
                .unwrap_or(at - info);
        let code = self.blocks.codes.len();
        self.blocks.codes.push(Code {
            info: if marker == b'$' {
                info..info
            } else {
                info..info_end
            },
            fence: Some(CodeFence {
                character: char::from(marker),
                length,
                indent,
            }),
            unclosed: true,
            math: marker == b'$',
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
            math: false,
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

/// Definition-looking table headers take precedence over definitions; other headers follow ordinary paragraph indentation rules.
fn table_definition_header(bytes: &[u8]) -> bool {
    let mut at = 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' if bytes.get(at + 1).is_some_and(u8::is_ascii_punctuation) => at += 1,
            b'[' => return false,
            b']' => return bytes.get(at + 1) == Some(&b':'),
            b'\r' | b'\n' => return true,
            _ => {}
        }
        at += 1;
    }
    false
}

fn math_fence(bytes: &[u8]) -> Option<usize> {
    let length = bytes.iter().take_while(|&&byte| byte == b'$').count();
    let end = scan_nextline(bytes);
    (length >= 2 && !bytes[length..end].contains(&b'$')).then_some(length)
}

fn footnote_prefix(bytes: &[u8], start: usize) -> Option<(Range<usize>, usize)> {
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
