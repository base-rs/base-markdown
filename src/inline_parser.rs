// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Compact inline candidates, resolved in reusable storage for the active block.
use crate::definitions::{scan_label, Definitions};
use crate::grammar_parser::decode_string;
use crate::stream::{Event, EventKind, Tag, TagKind, Text};
use crate::util::char::{classify_opt, Kind as CharacterKind};
use crate::util::normalize_identifier::normalize_identifier_into;
use alloc::{string::String, vec::Vec};
use core::{convert::TryFrom, ops::Range};

static MARKERS: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(b"\0<\\\r\n&`*_[]!");
static GFM_MARKERS: crate::byte_scan::AsciiSet =
    crate::byte_scan::AsciiSet::new(b"\0<\\\r\n&`*_[]!~@.:");

/// CommonMark with independently enabled GFM constructs can share ordinary inline scanning.
pub(crate) fn supported_options(options: &crate::ParseOptions) -> bool {
    let mut common = options.constructs.clone();
    common.gfm_autolink_literal = false;
    common.gfm_strikethrough = false;
    common.gfm_table = false;
    common.gfm_task_list_item = false;
    common.gfm_footnote_definition = false;
    common.gfm_label_start_footnote = false;
    common == crate::Constructs::default()
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    Decoded(usize),
    Code(usize),
    SoftBreak,
    HardBreak,
    Task(bool),
    Start(TagKind, usize),
    Reference(TagKind, usize),
    End(TagKind),
}
#[derive(Clone, Copy)]
struct Node {
    start: usize,
    end: usize,
    next: usize,
    previous: usize,
    kind: Kind,
}
struct Delimiter {
    node: usize,
    marker: u8,
    run_modulo: u8,
    open: bool,
    close: bool,
}
struct Bracket {
    node: usize,
    delimiters: usize,
    image: bool,
    label: bool,
}
struct Tick {
    start: usize,
    end: usize,
    next: usize,
}
struct Link {
    destination: Range<usize>,
    title: Option<Range<usize>>,
}
#[derive(Default)]
pub(crate) struct InlineParser {
    nodes: Vec<Node>,
    delimiters: Vec<Delimiter>,
    brackets: Vec<Bracket>,
    attention_stack: Vec<usize>,
    links: Vec<Link>,
    tick_heads: Vec<usize>,
    ticks: Vec<Tick>,
    values: Vec<Text<'static>>,
    reference_key: String,
    last: usize,
    cursor: usize,
    link_bottom: usize,
}
impl InlineParser {
    fn append(&mut self, start: usize, end: usize, kind: Kind) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            start,
            end,
            kind,
            previous: self.last,
            next: 0,
        });
        self.nodes[self.last].next = index;
        self.last = index;
        index
    }
    fn insert_after(&mut self, previous: usize, start: usize, end: usize, kind: Kind) {
        let next = self.nodes[previous].next;
        let index = self.nodes.len();
        self.nodes.push(Node {
            start,
            end,
            next,
            previous,
            kind,
        });
        self.nodes[previous].next = index;
        self.nodes[next].previous = index;
    }
    fn attention<const GFM: bool>(&mut self, bottom: usize) {
        // A successful match truncates the active opener stack. Failed searches advance bounds for the marker and modulo-three class, following pulldown-cmark's InlineStack technique.
        let mut lower = [[0; 6]; 3];
        self.attention_stack.clear();
        for close in bottom..self.delimiters.len() {
            let closer = self.delimiters[close].node;
            let marker = self.delimiters[close].marker;
            let marker_index = if GFM && marker == b'~' {
                2
            } else {
                usize::from(marker == b'_')
            };
            if self.delimiters[close].close {
                while self.nodes[closer].start < self.nodes[closer].end {
                    let size = self.nodes[closer].end - self.nodes[closer].start;
                    let closer_modulo = self.delimiters[close].run_modulo;
                    let bucket =
                        usize::from(closer_modulo) + usize::from(self.delimiters[close].open) * 3;
                    let bound = lower[marker_index][bucket].min(self.attention_stack.len());
                    let mut found = None;
                    for slot in (bound..self.attention_stack.len()).rev() {
                        let open = self.attention_stack[slot];
                        let candidate = &self.delimiters[open];
                        if candidate.marker == marker
                            && (!GFM
                                || marker != b'~'
                                || self.nodes[candidate.node].end
                                    - self.nodes[candidate.node].start
                                    == size)
                            && !((candidate.close || self.delimiters[close].open)
                                && (candidate.run_modulo + closer_modulo) % 3 == 0
                                && (candidate.run_modulo != 0 || closer_modulo != 0))
                        {
                            found = Some((slot, open));
                            break;
                        }
                    }
                    let Some((slot, open)) = found else {
                        lower[marker_index][bucket] = self.attention_stack.len();
                        break;
                    };
                    let opener = self.delimiters[open].node;
                    let take = (self.nodes[opener].end - self.nodes[opener].start)
                        .min(size)
                        .min(2);
                    let tag = if GFM && marker == b'~' {
                        TagKind::Delete
                    } else if take == 2 {
                        TagKind::Strong
                    } else {
                        TagKind::Emphasis
                    };
                    let start = self.nodes[opener].end - take;
                    let end = self.nodes[closer].start + take;
                    self.nodes[opener].end -= take;
                    self.nodes[closer].start += take;
                    self.insert_after(opener, start, end, Kind::Start(tag, 0));
                    self.insert_after(self.nodes[closer].previous, end, end, Kind::End(tag));
                    let remaining =
                        slot + usize::from(self.nodes[opener].start < self.nodes[opener].end);
                    self.attention_stack.truncate(remaining);
                    for bound in lower.iter_mut().flatten() {
                        *bound = (*bound).min(remaining);
                    }
                }
            }
            if self.delimiters[close].open && self.nodes[closer].start < self.nodes[closer].end {
                self.attention_stack.push(close);
            }
        }
        self.delimiters.truncate(bottom);
    }
    fn index_ticks(&mut self, bytes: &[u8], range: Range<usize>) {
        self.tick_heads.clear();
        self.ticks.clear();
        let mut at = range.end;
        while let Some(index) = memchr::memrchr(b'`', &bytes[range.start..at]) {
            let end = range.start + index + 1;
            at = end - 1;
            while at > range.start && bytes[at - 1] == b'`' {
                at -= 1;
            }
            let count = end - at;
            if self.tick_heads.len() <= count {
                self.tick_heads.resize(count + 1, 0);
            }
            self.ticks.push(Tick {
                start: at,
                end,
                next: self.tick_heads[count],
            });
            self.tick_heads[count] = self.ticks.len();
        }
    }
    fn closing_tick(&mut self, opening: usize, count: usize) -> Option<usize> {
        // Every candidate run (including an escaped run's shorter suffix) is bounded by the longest indexed raw run.
        let mut index = self.tick_heads[count];
        while index != 0 {
            let tick = &self.ticks[index - 1];
            self.tick_heads[count] = tick.next;
            if tick.start > opening {
                return Some(tick.end);
            }
            index = tick.next;
        }
        None
    }
    /// Each fragment contains one physical source line. Returns false when a region needs a compatibility construct; no events have escaped yet.
    // Specialize reference-free and CommonMark regions so unused extension checks compile out.
    pub(crate) fn prepare<const REFERENCES: bool, const GFM: bool>(
        &mut self,
        source: &str,
        fragments: impl Iterator<Item = Range<usize>>,
        definitions: &Definitions,
        options: &crate::ParseOptions,
        task: bool,
    ) -> bool {
        self.nodes.clear();
        self.delimiters.clear();
        self.brackets.clear();
        self.links.clear();
        self.values.clear();
        self.last = 0;
        self.cursor = 0;
        self.link_bottom = 0;
        self.nodes.push(Node {
            start: 0,
            end: 0,
            next: 0,
            previous: 0,
            kind: Kind::Text,
        });
        let bytes = source.as_bytes();
        let markers = if GFM { &GFM_MARKERS } else { &MARKERS };
        let mut fragments = fragments.peekable();
        while let Some(mut range) = fragments.next() {
            let mut indexed_ticks = false;
            let mut at = range.start;
            // Grammar-backed text chunks include paragraph padding; compact block fragments usually already omit it. Neither padding nor container indentation is inline content.
            while at < range.end && matches!(bytes[at], b' ' | b'\t') {
                at += 1;
            }
            if fragments.peek().is_none() {
                while range.end > at && matches!(bytes[range.end - 1], b' ' | b'\t') {
                    range.end -= 1;
                }
            }
            let inline_start = at;
            if GFM && task && self.nodes.len() == 1 && bytes.get(at) == Some(&b'[') {
                // Keep multiline markers and tab-expanded whitespace on the grammar path.
                if matches!(bytes.get(at + 1), Some(b' ' | b'x' | b'X'))
                    && bytes.get(at + 2) == Some(&b']')
                    && bytes.get(at + 3) == Some(&b' ')
                    && bytes[at + 3..range.end]
                        .iter()
                        .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    self.append(at, at + 3, Kind::Task(bytes[at + 1] != b' '));
                    at += 3;
                } else {
                    return false;
                }
            }
            while at < range.end {
                let start = at;
                match bytes[at] {
                    b'<' | 0 => return false,
                    b'@' | b':' if GFM && options.constructs.gfm_autolink_literal => return false,
                    b'.' if GFM
                        && options.constructs.gfm_autolink_literal
                        && bytes[at.saturating_sub(3)..at].eq_ignore_ascii_case(b"www") =>
                    {
                        return false
                    }
                    b'\\' if at + 1 < range.end && bytes[at + 1].is_ascii_punctuation() => {
                        self.append(at + 1, at + 2, Kind::Text);
                        at += 2;
                    }
                    b'\\' if at + 1 < range.end && matches!(bytes[at + 1], b'\r' | b'\n') => {
                        self.append(at, at + 1, Kind::HardBreak);
                        at += 1;
                    }
                    b'\r' | b'\n' => {
                        at += 1 + usize::from(
                            bytes[start] == b'\r' && bytes.get(start + 1) == Some(&b'\n'),
                        );
                        if matches!(self.nodes[self.last].kind, Kind::Text) {
                            let node = &mut self.nodes[self.last];
                            let mut spaces = node.end;
                            while spaces > node.start && bytes[spaces - 1] == b' ' {
                                spaces -= 1;
                            }
                            let hard = node.end - spaces >= 2
                                && (spaces == node.start || bytes[spaces - 1] != b'\t');
                            while node.end > node.start
                                && matches!(bytes[node.end - 1], b' ' | b'\t')
                            {
                                node.end -= 1;
                            }
                            if hard {
                                self.append(spaces, start, Kind::HardBreak);
                            }
                        }
                        self.append(start, at, Kind::SoftBreak);
                    }
                    b'&' => {
                        if let Some((end, value)) = entity(source, at, range.end) {
                            let index = self.values.len();
                            self.values.push(value);
                            self.append(at, end, Kind::Decoded(index));
                            at = end;
                        } else {
                            self.append(at, at + 1, Kind::Text);
                            at += 1;
                        }
                    }
                    b'`' => {
                        while at < range.end && bytes[at] == b'`' {
                            at += 1;
                        }
                        let count = at - start;
                        if !indexed_ticks {
                            self.index_ticks(bytes, range.clone());
                            indexed_ticks = true;
                        }
                        let found = self.closing_tick(start, count);
                        if let Some(end) = found {
                            self.append(start, end, Kind::Code(count));
                            at = end;
                        } else {
                            // A code span can cross source fragments; let the compatibility engine resolve it.
                            if fragments.peek().is_some() {
                                return false;
                            }
                            self.append(start, at, Kind::Text);
                        }
                    }
                    marker
                        if matches!(marker, b'*' | b'_')
                            || (GFM && marker == b'~' && options.constructs.gfm_strikethrough) =>
                    {
                        let marker = bytes[at];
                        while at < range.end && bytes[at] == marker {
                            at += 1;
                        }
                        let before_char = source[inline_start..start].chars().next_back();
                        let after_char = source[at..range.end].chars().next();
                        let before = classify_opt(before_char);
                        let after = classify_opt(after_char);
                        let left = (GFM
                            && options.constructs.gfm_strikethrough
                            && marker != b'~'
                            && after_char == Some('~'))
                            || after != CharacterKind::Whitespace
                                && (after != CharacterKind::Punctuation
                                    || before != CharacterKind::Other);
                        let right = (GFM
                            && options.constructs.gfm_strikethrough
                            && marker != b'~'
                            && before_char == Some('~'))
                            || before != CharacterKind::Whitespace
                                && (before != CharacterKind::Punctuation
                                    || after != CharacterKind::Other);
                        let node = self.append(start, at, Kind::Text);
                        if GFM
                            && marker == b'~'
                            && (at - start > 2
                                || (at - start == 1 && !options.gfm_strikethrough_single_tilde))
                        {
                            continue;
                        }
                        self.delimiters.push(Delimiter {
                            node,
                            marker,
                            // Rule three uses the original run length, even after a partial match consumes delimiters.
                            run_modulo: u8::try_from((at - start) % 3).unwrap(),
                            open: left
                                && (marker != b'_'
                                    || !right
                                    || before == CharacterKind::Punctuation),
                            close: right
                                && (marker != b'_' || !left || after == CharacterKind::Punctuation),
                        });
                    }
                    b'[' | b'!' => {
                        if GFM
                            && options.constructs.gfm_label_start_footnote
                            && (bytes[at..].starts_with(b"[^") || bytes[at..].starts_with(b"![^"))
                        {
                            return false;
                        }
                        let image = bytes[at] == b'!';
                        if image && bytes.get(at + 1) != Some(&b'[') {
                            at += 1;
                            at += markers.find(&bytes[at..range.end]);
                            self.append(start, at, Kind::Text);
                            continue;
                        }
                        at += 1 + usize::from(image);
                        let node = self.append(start, at, Kind::Text);
                        if REFERENCES {
                            if let Some(parent) = self.brackets.last_mut() {
                                // An unescaped nested bracket cannot occur in a definition label. Mark its parent now instead of rescanning nested labels at every close.
                                parent.label = false;
                            }
                        }
                        self.brackets.push(Bracket {
                            node,
                            delimiters: self.delimiters.len(),
                            image,
                            label: true,
                        });
                    }
                    b']' => {
                        at += 1;
                        let Some(bracket) = self.brackets.pop() else {
                            self.append(start, at, Kind::Text);
                            continue;
                        };
                        if !bracket.image && bracket.node < self.link_bottom {
                            self.append(start, at, Kind::Text);
                            continue;
                        }
                        let tag = if bracket.image {
                            TagKind::Image
                        } else {
                            TagKind::Link
                        };
                        let resolved = if bytes.get(at) == Some(&b'(') {
                            let Some((end, link)) = resource(bytes, at, range.end) else {
                                return false;
                            };
                            let index = self.links.len();
                            self.links.push(link);
                            Some((end, Kind::Start(tag, index)))
                        } else if !REFERENCES || definitions.is_empty() {
                            None
                        } else {
                            let label_start =
                                self.nodes[bracket.node].start + 1 + usize::from(bracket.image);
                            let (end, label) = if bytes.get(at) == Some(&b'[') {
                                if bytes.get(at + 1) == Some(&b']') {
                                    (at + 2, bracket.label.then_some(label_start..start))
                                } else {
                                    let Some((end, label)) = scan_label(bytes, at, range.end)
                                    else {
                                        // Multiline labels and malformed full references retain the grammar's fallback rules.
                                        return false;
                                    };
                                    (end, Some(label))
                                }
                            } else {
                                (at, bracket.label.then_some(label_start..start))
                            };
                            if let Some(label) = label {
                                if label.start < range.start {
                                    // Earlier fragments can contain stripped container prefixes.
                                    return false;
                                }
                                normalize_identifier_into(&source[label], &mut self.reference_key);
                                definitions
                                    .find(&self.reference_key)
                                    .map(|index| (end, Kind::Reference(tag, index)))
                            } else {
                                None
                            }
                        };
                        if let Some((end, kind)) = resolved {
                            self.attention::<GFM>(bracket.delimiters);
                            self.nodes[bracket.node].kind = kind;
                            self.nodes[bracket.node].end = end;
                            self.append(end, end, Kind::End(tag));
                            at = end;
                            if !bracket.image {
                                // Earlier link openers cannot contain a link. A monotone node boundary invalidates them without walking a deeply nested bracket stack.
                                self.link_bottom = self.nodes.len();
                            }
                            continue;
                        }
                        self.append(start, at, Kind::Text);
                    }
                    _ => {
                        at += 1;
                        at += markers.find(&bytes[at..range.end]);
                        self.append(start, at, Kind::Text);
                    }
                }
            }
        }
        self.attention::<GFM>(0);
        self.cursor = self.nodes[0].next;
        true
    }
    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub(crate) fn next(
        &mut self,
        source: &str,
        definitions: &Definitions,
    ) -> Option<Event<'static>> {
        while self.cursor != 0 {
            let node = self.nodes[self.cursor];
            self.cursor = node.next;
            let kind = match node.kind {
                Kind::Text if node.start == node.end => continue,
                Kind::Text => EventKind::Text(Text::Source(node.start..node.end)),
                Kind::Decoded(index) => EventKind::Text(core::mem::replace(
                    &mut self.values[index],
                    Text::Literal(""),
                )),
                Kind::SoftBreak => EventKind::SoftBreak(Text::Source(node.start..node.end)),
                Kind::HardBreak => EventKind::HardBreak,
                Kind::Task(checked) => EventKind::TaskMarker(checked),
                Kind::Code(count) => {
                    let mut range = node.start + count..node.end - count;
                    let value = &source[range.clone()];
                    if value.starts_with(' ')
                        && value.ends_with(' ')
                        && value.bytes().any(|b| b != b' ')
                    {
                        range.start += 1;
                        range.end -= 1;
                    }
                    EventKind::Code {
                        value: Text::Source(range),
                        language: None,
                    }
                }
                Kind::Start(tag, index) => EventKind::Start(match tag {
                    TagKind::Emphasis => Tag::Emphasis,
                    TagKind::Strong => Tag::Strong,
                    TagKind::Delete => Tag::Delete,
                    TagKind::Link | TagKind::Image => {
                        let link = &self.links[index];
                        let destination = decode_string(source, link.destination.clone());
                        let title = link.title.clone().map(|range| decode_string(source, range));
                        if tag == TagKind::Link {
                            Tag::Link { destination, title }
                        } else {
                            Tag::Image { destination, title }
                        }
                    }
                    _ => unreachable!("inline candidates open only inline containers"),
                }),
                Kind::End(tag) => EventKind::End(tag),
                Kind::Reference(tag, index) => {
                    let definition = &definitions.entries[index];
                    let destination = definition.destination.clone();
                    let title = definition.title.clone();
                    EventKind::Start(if tag == TagKind::Link {
                        Tag::Link { destination, title }
                    } else {
                        Tag::Image { destination, title }
                    })
                }
            };
            return Some(Event {
                kind,
                span: node.start..node.end,
            });
        }
        None
    }
}

fn resource(bytes: &[u8], start: usize, end: usize) -> Option<(usize, Link)> {
    let mut at = start + 1;
    while at < end && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    let destination;
    if bytes.get(at) == Some(&b'<') {
        at += 1;
        let begin = at;
        while at < end && bytes[at] != b'>' {
            if matches!(bytes[at], 0 | b'<' | b'\r' | b'\n') {
                return None;
            }
            if bytes[at] == b'\\' && at + 1 < end && bytes[at + 1].is_ascii_punctuation() {
                at += 1;
            }
            at += 1;
        }
        if at == end {
            return None;
        }
        destination = begin..at;
        at += 1;
    } else {
        let begin = at;
        let mut depth = 0;
        while at < end && !bytes[at].is_ascii_whitespace() {
            match bytes[at] {
                b'\\' if at + 1 < end && bytes[at + 1].is_ascii_punctuation() => {
                    at += 1;
                }
                b'(' => {
                    depth += 1;
                    if depth > 32 {
                        return None;
                    }
                }
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                b'\x00'..=b'\x1f' | b'\x7f' => return None,
                _ => {}
            }
            at += 1;
        }
        if depth != 0 {
            return None;
        }
        destination = begin..at;
    }
    let whitespace = at;
    while at < end && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    let mut title = None;
    if at > whitespace && at < end && matches!(bytes[at], b'\'' | b'"' | b'(') {
        let marker = if bytes[at] == b'(' { b')' } else { bytes[at] };
        at += 1;
        let begin = at;
        while at < end && bytes[at] != marker {
            if matches!(bytes[at], 0 | b'\r' | b'\n') || (marker == b')' && bytes[at] == b'(') {
                return None;
            }
            if bytes[at] == b'\\' && at + 1 < end && bytes[at + 1].is_ascii_punctuation() {
                at += 1;
            }
            at += 1;
        }
        title = (begin < at).then_some(begin..at);
        at += 1;
        while at < end && matches!(bytes[at], b' ' | b'\t') {
            at += 1;
        }
    }
    if at < end && bytes[at] == b')' {
        Some((at + 1, Link { destination, title }))
    } else {
        None
    }
}

fn entity(source: &str, start: usize, end: usize) -> Option<(usize, Text<'static>)> {
    use crate::util::character_reference::{decode_numeric, value_max, value_test};
    let bytes = source.as_bytes();
    let mut begin = start + 1;
    let mut marker = b'&';
    if bytes.get(begin) == Some(&b'#') {
        marker = b'#';
        begin += 1;
        if matches!(bytes.get(begin), Some(b'x' | b'X')) {
            marker = b'x';
            begin += 1;
        }
    }
    let mut at = begin;
    while at < end && at - begin < value_max(marker) && value_test(marker)(&bytes[at]) {
        at += 1;
    }
    if at == begin || at == end || bytes[at] != b';' {
        return None;
    }
    let value = if marker == b'&' {
        let references = crate::util::constant::CHARACTER_REFERENCES.as_slice();
        let index = references
            .binary_search_by_key(&&source[begin..at], |entry| entry.0)
            .ok()?;
        Text::Literal(references[index].1)
    } else {
        Text::from(decode_numeric(
            &source[begin..at],
            if marker == b'#' { 10 } else { 16 },
        ))
    };
    Some((at + 1, value))
}
