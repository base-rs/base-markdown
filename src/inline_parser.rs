// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Compact inline candidates, resolved in reusable storage for the active block.
use crate::definitions::Definitions;
use crate::stream::{Event, EventKind, Tag, TagKind, Text};
use crate::util::char::{classify_opt, Kind as CharacterKind};
use crate::util::character_reference::{scan as scan_character_reference, Reference};
use crate::util::normalize_identifier::{
    normalize_identifier_into, normalize_reference_identifier_into,
};
use crate::value_builder::decode_string_with_options;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::{convert::TryFrom, ops::Range};

mod autolink;

// Inline destinations only need scalar handling at escapes, parentheses, whitespace, or controls.
// NUL remains ordinary destination data; value decoding replaces it later.
static DESTINATION_MARKERS: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(
    b"\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f ()\\\x7f",
);

static MARKERS: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(b"\0<\\\r\n&`*_[]!");
static EXTENSION_MARKERS: crate::byte_scan::AsciiSet =
    crate::byte_scan::AsciiSet::new(b"\0<\\\r\n&`*_[]!~@.:$");

// Keep fixed prefixes compact: a fat string pointer would enlarge every inline record.
#[derive(Clone, Copy)]
enum AutolinkPrefix {
    None,
    Http,
    Mailto,
}
impl AutolinkPrefix {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Http => "http://",
            Self::Mailto => "mailto:",
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Text,
    EscapedText,
    NamedEntity(usize),
    NumericEntity(char),
    Code(usize),
    CodeInfo(usize),
    Math(usize),
    Footnote(usize),
    Html,
    AngleLink(bool),
    SoftBreak,
    HardBreak,
    Task(bool),
    Autolink(AutolinkPrefix),
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
    mixed: bool,
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
struct CodeInfo {
    count: usize,
    language: Range<usize>,
}
struct Segment {
    logical: Range<usize>,
    source: Range<usize>,
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
    math_heads: Vec<usize>,
    ticks: Vec<Tick>,
    reference_key: String,
    logical: String,
    segments: Vec<Segment>,
    code_info: Vec<CodeInfo>,
    normalized: bool,
    table: bool,
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
    fn insert_after(&mut self, previous: usize, start: usize, end: usize, kind: Kind) -> usize {
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
        if next == 0 {
            self.last = index;
        } else {
            self.nodes[next].previous = index;
        }
        index
    }
    fn attention<const EXTENSIONS: bool>(&mut self, bottom: usize) {
        if bottom == self.delimiters.len() {
            return;
        }
        // A successful match truncates the active opener stack. Failed searches advance bounds for the marker and modulo-three class, following pulldown-cmark's InlineStack technique.
        let mut bounds = [[0; 6]; 3];
        // The CommonMark specialization never searches strikethrough openers.
        let lower = &mut bounds[..if EXTENSIONS { 3 } else { 2 }];
        self.attention_stack.clear();
        for close in bottom..self.delimiters.len() {
            let closer = self.delimiters[close].node;
            let marker = self.delimiters[close].marker;
            let marker_index = if EXTENSIONS && marker == b'~' {
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
                            && (!EXTENSIONS
                                || marker != b'~'
                                || self.nodes[candidate.node].end
                                    - self.nodes[candidate.node].start
                                    == size)
                            && ((EXTENSIONS && marker == b'~')
                                || !((candidate.close || self.delimiters[close].open)
                                    && (candidate.run_modulo + closer_modulo) % 3 == 0
                                    && (candidate.run_modulo != 0 || closer_modulo != 0)))
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
                    let tag = if EXTENSIONS && marker == b'~' {
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
                    // Mixed strike/emphasis resolution retries the remaining
                    // run, so its modulo changes after a partial match.
                    if EXTENSIONS && self.delimiters[open].mixed {
                        self.delimiters[open].run_modulo =
                            u8::try_from((self.nodes[opener].end - self.nodes[opener].start) % 3)
                                .unwrap();
                    }
                    if EXTENSIONS && self.delimiters[close].mixed {
                        self.delimiters[close].run_modulo =
                            u8::try_from((self.nodes[closer].end - self.nodes[closer].start) % 3)
                                .unwrap();
                    }
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
    fn index_ticks(&mut self, bytes: &[u8], range: Range<usize>, math: bool) {
        self.tick_heads.clear();
        self.math_heads.clear();
        self.ticks.clear();
        let mut at = range.end;
        while let Some(index) = if math {
            memchr::memrchr2(b'`', b'$', &bytes[range.start..at])
        } else {
            memchr::memrchr(b'`', &bytes[range.start..at])
        } {
            let end = range.start + index + 1;
            at = end - 1;
            let marker = bytes[at];
            while at > range.start && bytes[at - 1] == marker {
                at -= 1;
            }
            let count = end - at;
            let heads = if marker == b'$' {
                &mut self.math_heads
            } else {
                &mut self.tick_heads
            };
            if heads.len() <= count {
                heads.resize(count + 1, 0);
            }
            self.ticks.push(Tick {
                start: at,
                end,
                next: heads[count],
            });
            heads[count] = self.ticks.len();
        }
    }
    fn closing_tick(&mut self, opening: usize, count: usize, marker: u8) -> Option<usize> {
        // Every candidate run (including an escaped run's shorter suffix) is bounded by the longest indexed raw run.
        let heads = if marker == b'$' {
            &mut self.math_heads
        } else {
            &mut self.tick_heads
        };
        let mut index = heads[count];
        while index != 0 {
            let tick = &self.ticks[index - 1];
            heads[count] = tick.next;
            if tick.start > opening {
                return Some(tick.end);
            }
            index = tick.next;
        }
        None
    }
    /// Ordinary lines keep their original source ranges. Only a construct that
    /// crosses stripped container prefixes needs a logical view, scanned by the
    /// same parser and mapped back to source coordinates when events are pulled.
    pub(crate) fn prepare<const EXTENSIONS: bool>(
        &mut self,
        source: &str,
        fragments: impl Iterator<Item = Range<usize>> + Clone,
        definitions: &Definitions<'_>,
        options: &crate::ParseOptions,
        task: bool,
        table: bool,
    ) {
        if definitions.is_empty() {
            self.prepare_region::<false, EXTENSIONS>(
                source,
                fragments,
                definitions,
                options,
                task,
                table,
            );
        } else {
            self.prepare_region::<true, EXTENSIONS>(
                source,
                fragments,
                definitions,
                options,
                task,
                table,
            );
        }
    }

    fn prepare_region<const REFERENCES: bool, const EXTENSIONS: bool>(
        &mut self,
        source: &str,
        fragments: impl Iterator<Item = Range<usize>> + Clone,
        definitions: &Definitions<'_>,
        options: &crate::ParseOptions,
        task: bool,
        table: bool,
    ) {
        self.table = table;
        self.normalized = false;
        if self.scan::<REFERENCES, EXTENSIONS>(
            source,
            source,
            fragments.clone(),
            definitions,
            options,
            task,
        ) {
            return;
        }
        let mut combined: Option<Range<usize>> = None;
        let mut contiguous = true;
        for fragment in fragments.clone() {
            match &mut combined {
                None => combined = Some(fragment),
                Some(range) => {
                    contiguous &= range.end == fragment.start;
                    range.end = fragment.end;
                }
            }
        }
        if contiguous {
            let fragment = combined.unwrap_or(0..0);
            self.scan::<REFERENCES, EXTENSIONS>(
                source,
                source,
                core::iter::once(fragment),
                definitions,
                options,
                task,
            );
            return;
        }
        let mut logical = core::mem::take(&mut self.logical);
        logical.clear();
        self.segments.clear();
        for fragment in fragments {
            let start = logical.len();
            logical.push_str(&source[fragment.clone()]);
            self.segments.push(Segment {
                logical: start..logical.len(),
                source: fragment,
            });
        }
        self.normalized = true;
        self.scan::<REFERENCES, EXTENSIONS>(
            source,
            &logical,
            core::iter::once(0..logical.len()),
            definitions,
            options,
            task,
        );
        self.logical = logical;
    }

    fn scan<const REFERENCES: bool, const EXTENSIONS: bool>(
        &mut self,
        original_source: &str,
        source: &str,
        fragments: impl Iterator<Item = Range<usize>>,
        definitions: &Definitions<'_>,
        options: &crate::ParseOptions,
        task: bool,
    ) -> bool {
        self.nodes.clear();
        self.delimiters.clear();
        self.brackets.clear();
        self.links.clear();
        self.code_info.clear();
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
        let markers = if EXTENSIONS {
            &EXTENSION_MARKERS
        } else {
            &MARKERS
        };
        let mut fragments = fragments.peekable();
        let mut saw_email = false;
        while let Some(range) = fragments.next() {
            let mut autolink_trail = autolink::Trail::default();
            let mut indexed_ticks = false;
            let mut at = range.start;
            // Paragraph padding and stripped container prefixes are not inline data.
            while at < range.end && matches!(bytes[at], b' ' | b'\t') {
                at += 1;
            }
            // Block producers already trim trailing padding from the final range.
            let inline_start = at;
            if EXTENSIONS
                && task
                && options.constructs.task_list_item
                && self.nodes.len() == 1
                && bytes.get(at) == Some(&b'[')
            {
                if let Some((end, checked)) = task_marker(bytes, at, range.end) {
                    self.append(at, end, Kind::Task(checked));
                    at = end;
                } else if fragments.peek().is_some() && range.end.saturating_sub(at) < 5 {
                    return false;
                }
            }
            while at < range.end {
                let start = at;
                match bytes[at] {
                    0 => {
                        at += 1;
                        at += markers.find(&bytes[at..range.end]);
                        self.append(start, at, Kind::Text);
                    }
                    b'<' if !EXTENSIONS
                        || options.constructs.autolink
                        || options.constructs.html_text =>
                    {
                        let Some((end, kind)) = angle::<EXTENSIONS>(bytes, at, range.end, options)
                        else {
                            if fragments.peek().is_some() {
                                return false;
                            }
                            self.append(at, at + 1, Kind::Text);
                            at += 1;
                            continue;
                        };
                        self.append(at, end, kind);
                        if matches!(kind, Kind::AngleLink(_)) {
                            self.append(at + 1, end - 1, Kind::Text);
                            self.append(end, end, Kind::End(TagKind::Link));
                        }
                        at = end;
                    }
                    marker
                        if EXTENSIONS
                            && options.constructs.autolink_literal
                            && matches!(marker, b':' | b'.') =>
                    {
                        if let Some(found) =
                            autolink::web(source, inline_start, at, range.end, &mut autolink_trail)
                        {
                            // The prefix is ordinary source text immediately before this marker.
                            // Escapes and character references form separate runs and cannot be reinterpreted.
                            let previous = self.nodes[self.last];
                            if matches!(previous.kind, Kind::Text | Kind::EscapedText)
                                && found.start
                                    >= previous.start
                                        + usize::from(matches!(previous.kind, Kind::EscapedText))
                            {
                                self.nodes[self.last].end = found.start;
                                self.append(found.start, found.end, Kind::Autolink(found.prefix));
                                self.append(found.start, found.end, Kind::Text);
                                self.append(found.end, found.end, Kind::End(TagKind::Link));
                                at = found.end;
                                continue;
                            }
                        }
                        at += 1;
                        at += markers.find(&bytes[at..range.end]);
                        self.append(start, at, Kind::Text);
                    }
                    b'\\'
                        if at + 1 < range.end
                            && (((!EXTENSIONS || options.constructs.character_escape)
                                && bytes[at + 1].is_ascii_punctuation())
                                || (self.table && bytes[at + 1] == b'|')) =>
                    {
                        let text_start = at + 1;
                        at += 2;
                        // The escaped byte and the following ordinary run share one source slice. Stop before syntax candidates so delimiter and bracket nodes remain distinct.
                        at += markers.find(&bytes[at..range.end]);
                        self.append(text_start, at, Kind::EscapedText);
                    }
                    b'\\'
                        if (!EXTENSIONS || options.constructs.hard_break_escape)
                            && at + 1 < range.end
                            && matches!(bytes[at + 1], b'\r' | b'\n') =>
                    {
                        self.append(at, at + 1, Kind::HardBreak);
                        at += 1;
                    }
                    b'\r' | b'\n' => {
                        at += 1 + usize::from(
                            bytes[start] == b'\r' && bytes.get(start + 1) == Some(&b'\n'),
                        );
                        if matches!(self.nodes[self.last].kind, Kind::Text | Kind::EscapedText) {
                            let node = &mut self.nodes[self.last];
                            let mut spaces = node.end;
                            while spaces > node.start && bytes[spaces - 1] == b' ' {
                                spaces -= 1;
                            }
                            let hard = (!EXTENSIONS || options.constructs.hard_break_trailing)
                                && node.end - spaces >= 2
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
                        // Paragraph padding belongs to the line prefix, except inside
                        // raw spans and HTML, which scan their complete range at once.
                        while at < range.end && matches!(bytes[at], b' ' | b'\t') {
                            at += 1;
                        }
                    }
                    b'&' if !EXTENSIONS || options.constructs.character_reference => {
                        if let Some((end, value)) = scan_character_reference(source, at, range.end)
                        {
                            let kind = match value {
                                Reference::Named(index) => Kind::NamedEntity(index),
                                Reference::Numeric(character) => Kind::NumericEntity(character),
                            };
                            self.append(at, end, kind);
                            at = end;
                        } else {
                            self.append(at, at + 1, Kind::Text);
                            at += 1;
                        }
                    }
                    marker
                        if (marker == b'`' && (!EXTENSIONS || options.constructs.code_text))
                            || (EXTENSIONS && marker == b'$' && options.constructs.math_text) =>
                    {
                        while at < range.end && bytes[at] == marker {
                            at += 1;
                        }
                        let count = at - start;
                        if marker == b'$' && count == 1 && !options.math_text_single_dollar {
                            self.append(start, at, Kind::Text);
                            continue;
                        }
                        if !indexed_ticks {
                            self.index_ticks(
                                bytes,
                                range.clone(),
                                EXTENSIONS && options.constructs.math_text,
                            );
                            indexed_ticks = true;
                        }
                        if let Some(end) = self.closing_tick(start, count, marker) {
                            let mut kind = if marker == b'$' {
                                Kind::Math(count)
                            } else {
                                Kind::Code(count)
                            };
                            let mut event_start = start;
                            if marker == b'`' && options.inline_code_info {
                                if let Some((prefix_start, info)) =
                                    crate::util::inline_code_info::before(bytes, start)
                                {
                                    self.trim_text_suffix(prefix_start);
                                    let index = self.code_info.len();
                                    self.code_info.push(CodeInfo {
                                        count,
                                        language: prefix_start..prefix_start + info.len(),
                                    });
                                    kind = Kind::CodeInfo(index);
                                    event_start = prefix_start;
                                }
                            }
                            self.append(event_start, end, kind);
                            at = end;
                        } else {
                            if fragments.peek().is_some() {
                                return false;
                            }
                            self.append(start, at, Kind::Text);
                        }
                    }
                    marker
                        if ((!EXTENSIONS || options.constructs.attention)
                            && matches!(marker, b'*' | b'_'))
                            || (EXTENSIONS
                                && marker == b'~'
                                && options.constructs.strikethrough) =>
                    {
                        let marker = bytes[at];
                        while at < range.end && bytes[at] == marker {
                            at += 1;
                        }
                        let before_char = source[inline_start..start].chars().next_back();
                        let after_char = source[at..range.end].chars().next();
                        let before = classify_opt(before_char);
                        let after = classify_opt(after_char);
                        let left = after != CharacterKind::Whitespace
                            && (after != CharacterKind::Punctuation
                                || before != CharacterKind::Other)
                            || (marker != b'~'
                                && EXTENSIONS
                                && options.constructs.strikethrough
                                && after_char == Some('~'));
                        let right = before != CharacterKind::Whitespace
                            && (before != CharacterKind::Punctuation
                                || after != CharacterKind::Other)
                            || (marker != b'~'
                                && EXTENSIONS
                                && options.constructs.strikethrough
                                && before_char == Some('~'));
                        let node = self.append(start, at, Kind::Text);
                        if EXTENSIONS
                            && marker == b'~'
                            && (at - start > 2
                                || (at - start == 1 && !options.strikethrough_single_tilde))
                        {
                            continue;
                        }
                        self.delimiters.push(Delimiter {
                            node,
                            marker,
                            // Rule three uses the original run length, even after a partial match consumes delimiters.
                            run_modulo: u8::try_from((at - start) % 3).unwrap(),
                            mixed: EXTENSIONS
                                && options.constructs.strikethrough
                                && marker != b'~'
                                && (before_char == Some('~') || after_char == Some('~')),
                            open: left
                                && (marker != b'_' || !right || before != CharacterKind::Other),
                            close: right
                                && (marker != b'_' || !left || after != CharacterKind::Other),
                        });
                    }
                    b'[' | b'!' => {
                        let image = bytes[at] == b'!';
                        let footnote_start = at + usize::from(image);
                        let footnote = EXTENSIONS
                            && options.constructs.label_start_footnote
                            && bytes.get(footnote_start..footnote_start + 2) == Some(b"[^");
                        if footnote && options.constructs.label_end {
                            if let Some((end, label)) =
                                label(bytes, footnote_start + 1, range.end, |at| {
                                    self.column(original_source, at)
                                })
                            {
                                normalize_identifier_into(&source[label], &mut self.reference_key);
                                if let Some(index) = definitions.find_footnote(&self.reference_key)
                                {
                                    if image {
                                        self.append(at, at + 1, Kind::Text);
                                    }
                                    self.append(footnote_start, end, Kind::Footnote(index));
                                    at = end;
                                    self.link_bottom = self.nodes.len();
                                    continue;
                                }
                            } else if fragments.peek().is_some() {
                                return false;
                            }
                        }
                        if (image
                            && ((EXTENSIONS && !options.constructs.label_start_image)
                                || (EXTENSIONS
                                    && options.constructs.label_start_footnote
                                    && bytes.get(at + 2) == Some(&b'^'))
                                || bytes.get(at + 1) != Some(&b'[')))
                            || (!image
                                && EXTENSIONS
                                && !options.constructs.label_start_link
                                && !footnote)
                        {
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
                    b']' if !EXTENSIONS || options.constructs.label_end => {
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
                        let mut resolved = None;
                        if bytes.get(at) == Some(&b'(') {
                            if let Some((end, link)) = resource(bytes, at, range.end) {
                                let index = self.links.len();
                                self.links.push(link);
                                resolved = Some((end, Kind::Start(tag, index)));
                            } else if fragments.peek().is_some() {
                                return false;
                            }
                        }
                        if resolved.is_none() && REFERENCES && !definitions.is_empty() {
                            let label_start =
                                self.nodes[bracket.node].start + 1 + usize::from(bracket.image);
                            let mut reference = bracket.label.then_some((at, label_start..start));
                            if bytes.get(at) == Some(&b'[') {
                                if bytes.get(at + 1) == Some(&b']') {
                                    reference =
                                        bracket.label.then_some((at + 2, label_start..start));
                                } else {
                                    reference = label(bytes, at, range.end, |at| {
                                        self.column(original_source, at)
                                    });
                                    if reference.is_none() && fragments.peek().is_some() {
                                        return false;
                                    }
                                }
                            }
                            if let Some((end, label)) = reference {
                                if label.start < range.start {
                                    return false;
                                }
                                normalize_reference_identifier_into(
                                    &source[label],
                                    &mut self.reference_key,
                                );
                                resolved = definitions
                                    .find(&self.reference_key)
                                    .map(|index| (end, Kind::Reference(tag, index)));
                            }
                        }
                        if let Some((end, kind)) = resolved {
                            self.attention::<EXTENSIONS>(bracket.delimiters);
                            if EXTENSIONS && options.constructs.autolink_literal && !bracket.image {
                                self.suppress_autolinks(bracket.node);
                            }
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
                        if EXTENSIONS && bytes[at] == b'@' {
                            saw_email = true;
                        }
                        at += 1;
                        at += markers.find(&bytes[at..range.end]);
                        self.append(start, at, Kind::Text);
                    }
                }
            }
        }
        self.attention::<EXTENSIONS>(0);
        if EXTENSIONS && options.constructs.autolink_literal && saw_email {
            self.resolve_email(source);
        }
        self.cursor = self.nodes[0].next;
        true
    }
    /// Remove a language prefix from contiguous text candidates. Literal-link
    /// marker scanning may have split the prefix without changing its bytes.
    fn trim_text_suffix(&mut self, start: usize) {
        let mut cursor = self.last;
        // A language token consists only of ordinary text and unresolved `_`,
        // `.` and `:` candidates. Syntax that would have resolved these nodes
        // cannot occur within the accepted token alphabet.
        while self.nodes[cursor].start > start {
            self.nodes[cursor].end = self.nodes[cursor].start;
            cursor = self.nodes[cursor].previous;
        }
        self.nodes[cursor].end = start;
    }

    fn suppress_autolinks(&mut self, opener: usize) {
        let mut cursor = self.nodes[opener].next;
        while cursor != 0 {
            if matches!(self.nodes[cursor].kind, Kind::Autolink(_)) {
                self.nodes[cursor].end = self.nodes[cursor].start;
                self.nodes[cursor].kind = Kind::Text;
                let text = self.nodes[cursor].next;
                let close = self.nodes[text].next;
                self.nodes[close].kind = Kind::Text;
            }
            cursor = self.nodes[cursor].next;
        }
    }

    /// Email syntax is resolved after attention so unmatched underscores remain
    /// part of an address, while escapes, entities, and links bound each run.
    fn resolve_email(&mut self, source: &str) {
        let mut cursor = self.nodes[0].next;
        let mut links = 0;
        while cursor != 0 {
            let node = self.nodes[cursor];
            match node.kind {
                Kind::Start(TagKind::Link, _)
                | Kind::Reference(TagKind::Link, _)
                | Kind::AngleLink(_)
                | Kind::Autolink(_) => links += 1,
                Kind::End(TagKind::Link) => links -= 1,
                Kind::Text | Kind::EscapedText if links == 0 && node.start < node.end => {
                    let run_start =
                        node.start + usize::from(matches!(node.kind, Kind::EscapedText));
                    let mut end = node.end;
                    let mut next = node.next;
                    while next != 0
                        && matches!(self.nodes[next].kind, Kind::Text)
                        && self.nodes[next].start == end
                    {
                        end = self.nodes[next].end;
                        next = self.nodes[next].next;
                    }
                    if !source.as_bytes()[run_start..end].contains(&b'@') {
                        cursor = next;
                        continue;
                    }
                    // Keep the escaped punctuation outside the data run without
                    // adding another candidate for ordinary escaped prose.
                    if run_start != node.start {
                        self.nodes[cursor].end = run_start;
                        let escaped = cursor;
                        cursor = self.insert_after(escaped, run_start, end, Kind::Text);
                    }
                    // Coalesce candidate pieces without joining or allocating their text.
                    self.nodes[cursor].end = end;
                    self.nodes[cursor].next = next;
                    if next == 0 {
                        self.last = cursor;
                    } else {
                        self.nodes[next].previous = cursor;
                    }
                    let mut at = run_start;
                    let mut min = at;
                    let mut tail = cursor;
                    while let Some(offset) = memchr::memchr(b'@', &source.as_bytes()[at..end]) {
                        let marker = at + offset;
                        if let Some(found) = autolink::email(source, min, marker, end) {
                            self.nodes[tail].end = found.start;
                            let open = self.insert_after(
                                tail,
                                found.start,
                                found.end,
                                Kind::Autolink(found.prefix),
                            );
                            let text = self.insert_after(open, found.start, found.end, Kind::Text);
                            let close = self.insert_after(
                                text,
                                found.end,
                                found.end,
                                Kind::End(TagKind::Link),
                            );
                            tail = self.insert_after(close, found.end, end, Kind::Text);
                            min = found.end;
                            at = found.end;
                        } else {
                            at = marker + 1;
                        }
                    }
                    cursor = next;
                    continue;
                }
                _ => {}
            }
            cursor = node.next;
        }
    }

    fn column(&self, source: &str, at: usize) -> usize {
        let at = self.source_range(at..at).start;
        let bytes = source.as_bytes();
        let line_start = memchr::memrchr2(b'\r', b'\n', &bytes[..at]).map_or(0, |index| index + 1);
        bytes[line_start..at].iter().fold(1, |column, byte| {
            column
                + if *byte == b'\t' {
                    4 - (column - 1) % 4
                } else {
                    1
                }
        })
    }

    #[inline]
    fn source_range(&self, range: Range<usize>) -> Range<usize> {
        if self.normalized {
            self.mapped_source_range(range)
        } else {
            range
        }
    }

    #[inline(never)]
    fn mapped_source_range(&self, range: Range<usize>) -> Range<usize> {
        let start_segment = self
            .segments
            .partition_point(|segment| segment.logical.end <= range.start);
        let end_segment = self
            .segments
            .partition_point(|segment| segment.logical.end < range.end);
        let start = self.segments.get(start_segment).map_or_else(
            || self.segments.last().map_or(0, |segment| segment.source.end),
            |segment| {
                segment.source.start
                    + (range.start - segment.logical.start).min(segment.source.len())
            },
        );
        let end = self.segments.get(end_segment).map_or(start, |segment| {
            segment.source.start + (range.end - segment.logical.start).min(segment.source.len())
        });
        start..end.max(start)
    }

    #[inline]
    fn text(&self, source: &str, range: Range<usize>) -> Text<'static> {
        if self.normalized {
            self.mapped_text(source, range)
        } else {
            Text::Source(range)
        }
    }

    #[inline(never)]
    fn mapped_text(&self, source: &str, range: Range<usize>) -> Text<'static> {
        let mapped = self.mapped_source_range(range.clone());
        if mapped.len() == range.len() {
            return Text::Source(mapped);
        }
        Text::from(source[range].to_string())
    }

    fn decoded(
        &self,
        source: &str,
        range: Range<usize>,
        options: &crate::ParseOptions,
    ) -> Text<'static> {
        match decode_string_with_options(source, range, options) {
            Text::Source(range) => self.text(source, range),
            value => value,
        }
    }

    /// Inline HTML and title continuations omit paragraph prefix whitespace.
    /// This operates on the active source view before ranges map back to source.
    fn unpadded_text(source: &str, range: Range<usize>) -> Text<'static> {
        let bytes = source.as_bytes();
        let mut builder = crate::value_builder::ValueBuilder::new(source);
        let mut start = range.start;
        let mut at = start;
        while at < range.end {
            if bytes[at] == 0 {
                builder.append(Text::Source(start..at));
                builder.append(Text::Literal("�"));
                at += 1;
                start = at;
            } else if matches!(bytes[at], b'\r' | b'\n') {
                at += 1 + usize::from(bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n'));
                let end = at;
                while at < range.end && matches!(bytes[at], b' ' | b'\t') {
                    at += 1;
                }
                if end != at {
                    builder.append(Text::Source(start..end));
                    start = at;
                }
            } else {
                at += 1;
            }
        }
        builder.append(Text::Source(start..range.end));
        builder.finish()
    }

    fn html_text(&self, source: &str, range: Range<usize>) -> Text<'static> {
        match Self::unpadded_text(source, range) {
            Text::Source(range) => self.text(source, range),
            value => value,
        }
    }

    fn title(
        &self,
        source: &str,
        range: Range<usize>,
        options: &crate::ParseOptions,
    ) -> Text<'static> {
        match Self::unpadded_text(source, range) {
            Text::Source(range) => self.decoded(source, range, options),
            value => {
                let resolved = value.resolve(source).expect("normalized title is owned");
                match decode_string_with_options(resolved, 0..resolved.len(), options) {
                    Text::Source(_) => value,
                    decoded => decoded,
                }
            }
        }
    }

    #[inline]
    fn raw_text(&self, source: &str, mut range: Range<usize>) -> Text<'static> {
        let bytes = source.as_bytes();
        let needs_normalization = memchr::memchr3(0, b'\r', b'\n', &bytes[range.clone()]).is_some()
            || (self.table && source[range.clone()].contains("\\|"));
        if needs_normalization {
            return self.normalized_raw_text(source, range);
        }
        let value = &source[range.clone()];
        if value.starts_with(' ') && value.ends_with(' ') && value.bytes().any(|byte| byte != b' ')
        {
            range.start += 1;
            range.end -= 1;
        }
        self.text(source, range)
    }

    fn normalized_raw_text(&self, source: &str, range: Range<usize>) -> Text<'static> {
        let bytes = source.as_bytes();
        let mut value = String::with_capacity(range.len());
        let mut at = range.start;
        let mut start = at;
        while at < range.end {
            let replacement = match bytes[at] {
                0 => Some("�"),
                b'\r' | b'\n' => Some(" "),
                b'\\' if self.table && bytes.get(at + 1) == Some(&b'|') => Some(""),
                _ => None,
            };
            if let Some(replacement) = replacement {
                value.push_str(&source[start..at]);
                value.push_str(replacement);
                if bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
                    at += 1;
                }
                at += 1;
                start = at;
            } else {
                at += 1;
            }
        }
        value.push_str(&source[start..range.end]);
        if value.starts_with(' ') && value.ends_with(' ') && value.bytes().any(|byte| byte != b' ')
        {
            value.pop();
            value.remove(0);
        }
        Text::from(value)
    }

    // Paired mixed-document benchmarks justify inlining this event boundary.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub(crate) fn next(
        &mut self,
        source: &str,
        definitions: &Definitions<'_>,
        options: &crate::ParseOptions,
    ) -> Option<Event<'static>> {
        let source = if self.normalized {
            &self.logical
        } else {
            source
        };
        while self.cursor != 0 {
            let node = self.nodes[self.cursor];
            self.cursor = node.next;
            let kind = match node.kind {
                Kind::Text | Kind::EscapedText if node.start == node.end => continue,
                Kind::Text | Kind::EscapedText => {
                    EventKind::Text(self.text(source, node.start..node.end))
                }
                Kind::Footnote(index) => EventKind::FootnoteReference(Text::from(
                    definitions.footnote_key(index).to_string(),
                )),
                Kind::NamedEntity(index) => EventKind::Text(Text::Literal(
                    crate::util::constant::CHARACTER_REFERENCES[index].1,
                )),
                Kind::NumericEntity(character) => {
                    EventKind::Text(Text::from(String::from(character)))
                }
                Kind::SoftBreak => EventKind::SoftBreak(self.text(source, node.start..node.end)),
                Kind::HardBreak => EventKind::HardBreak,
                Kind::Task(checked) => EventKind::TaskMarker(checked),
                Kind::Autolink(prefix) => {
                    let prefix = prefix.as_str();
                    EventKind::Start(Tag::Link {
                        destination: if prefix.is_empty() {
                            self.text(source, node.start..node.end)
                        } else {
                            Text::from(alloc::format!("{prefix}{}", &source[node.start..node.end]))
                        },
                        title: None,
                    })
                }
                Kind::Html => EventKind::Html {
                    value: self.html_text(source, node.start..node.end),
                    block: false,
                },
                Kind::AngleLink(email) => EventKind::Start(Tag::Link {
                    destination: if email {
                        Text::from(alloc::format!(
                            "mailto:{}",
                            &source[node.start + 1..node.end - 1]
                        ))
                    } else {
                        self.text(source, node.start + 1..node.end - 1)
                    },
                    title: None,
                }),
                Kind::Code(_) | Kind::Math(_) | Kind::CodeInfo(_) => {
                    let (count, start, language) = match node.kind {
                        Kind::CodeInfo(index) => {
                            let info = &self.code_info[index];
                            (
                                info.count,
                                info.language.end + 1,
                                Some(self.text(source, info.language.clone())),
                            )
                        }
                        Kind::Code(count) | Kind::Math(count) => (count, node.start, None),
                        _ => unreachable!("raw inline candidates have raw metadata"),
                    };
                    let value = self.raw_text(source, start + count..node.end - count);
                    if matches!(node.kind, Kind::Math(_)) {
                        EventKind::Math(value)
                    } else {
                        EventKind::Code { value, language }
                    }
                }
                Kind::Start(tag, index) => EventKind::Start(match tag {
                    TagKind::Emphasis => Tag::Emphasis,
                    TagKind::Strong => Tag::Strong,
                    TagKind::Delete => Tag::Delete,
                    TagKind::Link | TagKind::Image => {
                        let link = &self.links[index];
                        let destination = self.decoded(source, link.destination.clone(), options);
                        let title = link
                            .title
                            .clone()
                            .map(|range| self.title(source, range, options));
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
                span: self.source_range(node.start..node.end),
            });
        }
        None
    }
}

fn resource(bytes: &[u8], start: usize, end: usize) -> Option<(usize, Link)> {
    let mut at = start + 1;
    while at < end && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
        at += 1;
    }
    let destination;
    if bytes.get(at) == Some(&b'<') {
        at += 1;
        let begin = at;
        while at < end && bytes[at] != b'>' {
            if matches!(bytes[at], b'<' | b'\r' | b'\n') {
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
        loop {
            at += DESTINATION_MARKERS.find(&bytes[at..end]);
            if at == end || bytes[at].is_ascii_whitespace() {
                break;
            }
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
                b'\x01'..=b'\x1f' | b'\x7f' => return None,
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
    while at < end && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
        at += 1;
    }
    let mut title = None;
    if at > whitespace && at < end && matches!(bytes[at], b'\'' | b'"' | b'(') {
        let marker = if bytes[at] == b'(' { b')' } else { bytes[at] };
        at += 1;
        let begin = at;
        while at < end && bytes[at] != marker {
            if bytes[at] == b'\\' && at + 1 < end && bytes[at + 1].is_ascii_punctuation() {
                at += 1;
            }
            at += 1;
        }
        if at == end {
            return None;
        }
        title = (begin < at).then_some(begin..at);
        at += 1;
        while at < end && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
            at += 1;
        }
    }
    if at < end && bytes[at] == b')' {
        Some((at + 1, Link { destination, title }))
    } else {
        None
    }
}

/// Keep angle recognition shared across the compact parser's inline configurations.
#[inline(never)]
fn angle<const CONFIGURED: bool>(
    bytes: &[u8],
    start: usize,
    limit: usize,
    options: &crate::ParseOptions,
) -> Option<(usize, Kind)> {
    if !CONFIGURED || options.constructs.autolink {
        let mut link = crate::util::autolink::Autolink::default();
        for (offset, &byte) in bytes[start + 1..limit].iter().enumerate() {
            match link.advance(byte) {
                None => break,
                Some(true) => return Some((start + offset + 2, Kind::AngleLink(link.email()))),
                Some(false) => {}
            }
        }
    }
    if !CONFIGURED || options.constructs.html_text {
        let mut html = crate::util::html_text::HtmlText::default();
        for (offset, &byte) in bytes[start + 1..limit].iter().enumerate() {
            let byte = if byte == b'\r' { b'\n' } else { byte };
            if html.advance(byte, options.html_tag_namespaces)? {
                return Some((start + offset + 2, Kind::Html));
            }
        }
    }
    None
}

fn task_marker(bytes: &[u8], start: usize, end: usize) -> Option<(usize, bool)> {
    let value = *bytes.get(start + 1)?;
    if !matches!(value, b' ' | b'\t' | b'\r' | b'\n' | b'x' | b'X') {
        return None;
    }
    let close = start + 2 + usize::from(value == b'\r' && bytes.get(start + 2) == Some(&b'\n'));
    if bytes.get(close) != Some(&b']') || close + 1 >= end {
        return None;
    }
    let after = bytes[close + 1];
    if !matches!(after, b' ' | b'\t' | b'\r' | b'\n') {
        return None;
    }
    Some((close + 1, matches!(value, b'x' | b'X')))
}

/// Labels count authored data, but not line endings or continuation padding.
fn label(
    bytes: &[u8],
    start: usize,
    end: usize,
    mut column_at: impl FnMut(usize) -> usize,
) -> Option<(usize, Range<usize>)> {
    let begin = start + 1;
    let mut at = begin;
    let mut size = 0;
    let mut content = false;
    let mut previous_tab = None;
    while at < end && size <= crate::util::constant::LINK_REFERENCE_SIZE_MAX {
        match bytes[at] {
            b']' => return content.then_some((at + 1, begin..at)),
            b'[' => return None,
            b'\r' | b'\n' => {
                at += 1 + usize::from(bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n'));
                while at < end && matches!(bytes[at], b' ' | b'\t') {
                    at += 1;
                }
                // Block regions never contain blank lines.
                if at == end {
                    return None;
                }
                previous_tab = None;
                continue;
            }
            b'\\' if at + 1 < end && matches!(bytes[at + 1], b'[' | b'\\' | b']') => {
                size += 1;
                at += 1;
                content = true;
            }
            b'\t' => {
                // Most labels have no tabs: only calculate the physical column
                // when expanded whitespace can affect the 999-character bound.
                let column = previous_tab.map_or_else(
                    || column_at(at),
                    |(position, column)| column + at - position,
                );
                let width = 4 - (column - 1) % 4;
                size += width - 1;
                previous_tab = Some((at + 1, column + width));
            }
            b' ' => {}
            _ => content = true,
        }
        size += 1;
        at += 1;
    }
    None
}
