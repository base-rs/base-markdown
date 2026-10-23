// Copyright 2015-2018 Google Inc. and Google LLC.
// Copyright 2026 Samuel Williams.
// Released under the MIT License; see license.md.
// Block/container algorithms adapted from pulldown-cmark revision 38e4d08f14ec4bd9783270e9623db7681ebed968.

//! Compact block preparation with source fragments and deferred inline scanning.
mod scanners;
use crate::mdast::CodeFence;
use crate::ParseOptions;
use alloc::vec;
use alloc::vec::Vec;
use core::{convert::TryFrom, ops::Range};
use scanners::{
    is_ascii_whitespace, scan_atx_heading, scan_blank_line, scan_closing_code_fence,
    scan_code_fence, scan_eol, scan_hrule, scan_nextline, scan_setext_heading,
    scan_whitespace_no_nl, LineStart,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Body {
    Root,
    Paragraph(bool),
    Heading(u8),
    Quote,
    List { tight: bool, marker: u8, start: u32 },
    Item(usize),
    Code(usize),
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
    blocks: Blocks,
    open: Vec<usize>,
    current: usize,
    blank: bool,
    empty_item: Option<usize>,
    unsupported: bool,
}

impl Blocks {
    pub fn parse(source: &str, options: &ParseOptions) -> Option<Self> {
        // The compatibility producer handles extension grammars and individually disabled CommonMark constructs during migration.
        if options.constructs != crate::Constructs::default()
            || options.html_block_blank_lines
            || options.html_tag_namespaces
        {
            return None;
        }
        let mut builder = Builder {
            source,
            blocks: Self {
                nodes: Vec::with_capacity(source.len() / 64 + 1),
                codes: Vec::new(),
            },
            open: vec![0],
            current: 0,
            blank: false,
            empty_item: None,
            unsupported: false,
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
        while index < source.len() && !builder.unsupported {
            index = builder.block(index);
        }
        if builder.unsupported {
            return None;
        }
        while builder.open.len() > 1 {
            builder.pop(index);
        }
        Some(builder.blocks)
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
            let mut item = self.blocks.nodes[index].child;
            while item != 0 {
                let mut child = self.blocks.nodes[item].child;
                while child != 0 {
                    if let Body::Paragraph(ref mut tight) = self.blocks.nodes[child].body {
                        *tight = true;
                    }
                    child = self.blocks.nodes[child].next;
                }
                item = self.blocks.nodes[item].next;
            }
            self.empty_item = None;
        }
    }
    fn containers(&self, line: &mut LineStart<'_>) -> usize {
        let mut count = 0;
        for &index in &self.open {
            let save = line.clone();
            match self.blocks.nodes[index].body {
                Body::Quote => {
                    line.scan_space(3);
                    if !line.scan_blockquote_marker() {
                        *line = save;
                        break;
                    }
                }
                Body::Item(indent) if !line.scan_space(indent) && !line.is_at_eol() => {
                    *line = save;
                    break;
                }
                _ => {}
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
        let continued = self.containers(&mut line);
        while self.open.len() > continued {
            self.pop(start);
        }
        loop {
            let save = line.clone();
            let indent = line.scan_space_upto(4);
            if indent == 4 {
                line = save;
                break;
            }
            let at = start + line.bytes_scanned();
            if let Some((marker, number, width)) = line.scan_list_marker_with_indent(indent) {
                let content = start + line.bytes_scanned();
                self.list(at - indent, marker, number, start);
                self.append(at - indent, content, Body::Item(width));
                self.push();
                if let Some(length) = scan_blank_line(&bytes[content..]) {
                    self.empty_item = Some(content + length);
                    return content + length;
                }
            } else if line.scan_blockquote_marker() {
                self.finish_list(start);
                self.append(at, at, Body::Quote);
                self.push();
            } else {
                line = save;
                break;
            }
        }
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
        let indent = line.scan_space_upto(4);
        let at = start + line.bytes_scanned();
        if indent == 4 {
            self.finish_list(start);
            return self.indented(at, line.remaining_space());
        }
        // References and raw HTML retain their existing grammar until their metadata is recorded directly.
        if matches!(bytes.get(at), Some(b'[' | b'<')) {
            self.unsupported = true;
            return bytes.len();
        }
        if let Ok(length) = scan_hrule(&bytes[at..]) {
            self.finish_list(start);
            self.append(at, at + length, Body::Rule);
            return at + length;
        }
        if let Some(level) = scan_atx_heading(&bytes[at..]) {
            self.finish_list(start);
            return self.heading(at, level);
        }
        if let Some((length, marker)) = scan_code_fence(&bytes[at..]) {
            self.finish_list(start);
            return self.fenced(at, indent, marker, length);
        }
        self.paragraph(at, start)
    }
    fn interrupts(&mut self, bytes: &[u8], current_container: bool) -> bool {
        // HTML can interrupt paragraphs even after lazy container continuations.
        if bytes.starts_with(b"<") {
            self.unsupported = true;
            return true;
        }
        scan_eol(bytes).is_some()
            || scan_hrule(bytes).is_ok()
            || scan_atx_heading(bytes).is_some()
            || scan_code_fence(bytes).is_some()
            || bytes.starts_with(b">")
            || {
                let mut line = LineStart::new(bytes);
                line.scan_list_marker_with_indent(0)
                    .is_some_and(|(marker, number, _)| {
                        !current_container
                            || ((matches!(marker, b'*' | b'-' | b'+') || number == 1)
                                && scan_blank_line(&bytes[line.bytes_scanned()..]).is_none())
                    })
            }
    }
    fn paragraph(&mut self, start: usize, line_start: usize) -> usize {
        self.finish_list(line_start);
        let paragraph = self.append(start, start, Body::Paragraph(false));
        self.push();
        let bytes = self.source.as_bytes();
        let mut at = start;
        loop {
            let next = at + scan_nextline(&bytes[at..]);
            self.fragment(at, next, 0);
            let mut line = LineStart::new(&bytes[next..]);
            let continued = self.containers(&mut line) == self.open.len();
            let content_start = next + line.bytes_scanned();
            if !line.scan_space(4) {
                let content = next + line.bytes_scanned();
                if continued {
                    if let Some((length, level)) = scan_setext_heading(&bytes[content..]) {
                        self.trim_fragment();
                        self.blocks.nodes[paragraph].body = Body::Heading(level);
                        self.pop(content + length);
                        return content + length;
                    }
                }
                if self.interrupts(&bytes[content..], continued) {
                    self.trim_fragment();
                    self.pop(next);
                    return next;
                }
            }
            line.scan_all_space();
            if line.is_at_eol() {
                self.trim_fragment();
                self.pop(next);
                return next;
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
            if !closer.scan_space(4 - indent) {
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
