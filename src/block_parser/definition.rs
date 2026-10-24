// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Definition metadata scanned over source fragments, without grammar tokens.
use crate::stream::Text;
use crate::value_builder::{decode_string_with_options, ValueBuilder};
use crate::ParseOptions;
use core::ops::Range;

pub(super) struct Metadata {
    pub label: Text<'static>,
    pub destination: Text<'static>,
    pub title: Option<Text<'static>>,
    pub end: usize,
}

/// Lookahead retains physical offsets across stripped container prefixes.
#[derive(Clone)]
struct Cursor<'a, I> {
    source: &'a str,
    remaining: I,
    range: Range<usize>,
    at: usize,
    consumed: usize,
}

impl<'a, I: Iterator<Item = Range<usize>> + Clone> Cursor<'a, I> {
    fn new(source: &'a str, mut remaining: I) -> Self {
        let range = remaining.next().unwrap_or(0..0);
        let at = range.start;
        Self {
            source,
            remaining,
            range,
            at,
            consumed: at,
        }
    }
    fn byte(&self) -> Option<u8> {
        (self.at < self.range.end).then(|| self.source.as_bytes()[self.at])
    }
    fn bump(&mut self) {
        self.at += 1;
        self.consumed = self.at;
        while self.at == self.range.end {
            let Some(range) = self.remaining.next() else {
                break;
            };
            self.at = range.start;
            self.range = range;
        }
    }
    fn spaces(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }
    fn newline(&mut self) -> bool {
        match self.byte() {
            Some(b'\r') => {
                self.bump();
                if self.byte() == Some(b'\n') {
                    self.bump();
                }
            }
            Some(b'\n') => self.bump(),
            _ => return false,
        }
        true
    }
    fn whitespace(&mut self) {
        self.spaces();
        if self.newline() {
            self.spaces();
        }
    }
    fn line_end(&self) -> Option<usize> {
        let mut cursor = self.clone();
        cursor.spaces();
        if cursor.byte().is_none() || cursor.newline() {
            Some(cursor.consumed)
        } else {
            None
        }
    }
    fn value(&self, end: usize, options: Option<&ParseOptions>) -> Text<'static> {
        let mut result = ValueBuilder::new(self.source);
        let mut cursor = self.clone();
        let mut continuation = false;
        while cursor.at < end && cursor.at < cursor.range.end {
            let range = cursor.at..end.min(cursor.range.end);
            if let Some(options) = options {
                let mut start = range.start;
                // Each fragment is one physical line. Continuation indentation
                // is syntax padding, not part of a title's semantic value.
                if continuation {
                    while start < range.end && matches!(self.source.as_bytes()[start], b' ' | b'\t')
                    {
                        start += 1;
                    }
                }
                result.append(decode_string_with_options(
                    self.source,
                    start..range.end,
                    options,
                ));
            } else {
                result.append(Text::Source(range.clone()));
            }
            if range.end == end {
                break;
            }
            let range = cursor
                .remaining
                .next()
                .expect("value endpoint was scanned in these fragments");
            cursor.at = range.start;
            cursor.range = range;
            continuation = true;
        }
        result.finish()
    }
}

/// Content fragments stop at block interruptions. Failed next-line titles roll back to the destination-only checkpoint.
pub(super) fn scan(
    source: &str,
    fragments: impl Iterator<Item = Range<usize>> + Clone,
    options: &ParseOptions,
) -> Option<Metadata> {
    let mut cursor = Cursor::new(source, fragments);
    cursor.spaces();
    // Block dispatch only calls this scanner at an opening label bracket.
    cursor.bump();
    let label_start = cursor.clone();
    let mut content = false;
    let mut size = 0;
    let mut column = None;
    loop {
        if size > crate::util::constant::LINK_REFERENCE_SIZE_MAX {
            return None;
        }
        match cursor.byte()? {
            b'[' => return None,
            b']' => break,
            b'\\' => {
                content = true;
                cursor.bump();
                size += 1;
                if let Some(column) = &mut column {
                    *column += 1;
                }
                if matches!(cursor.byte(), Some(b'[' | b']' | b'\\')) {
                    cursor.bump();
                    size += 1;
                    if let Some(column) = &mut column {
                        *column += 1;
                    }
                }
            }
            b'\r' | b'\n' => {
                cursor.newline();
                column = None;
                cursor.spaces();
                if matches!(cursor.byte(), None | Some(b'\r' | b'\n')) {
                    return None;
                }
            }
            byte => {
                content |= !matches!(byte, b' ' | b'\t');
                let width = if byte == b'\t' {
                    let column = *column.get_or_insert_with(|| {
                        let bytes = source.as_bytes();
                        let start = memchr::memrchr2(b'\r', b'\n', &bytes[..cursor.at])
                            .map_or(0, |index| index + 1);
                        bytes[start..cursor.at].iter().fold(0, |column, byte| {
                            column + if *byte == b'\t' { 4 - column % 4 } else { 1 }
                        })
                    });
                    4 - column % 4
                } else {
                    1
                };
                size += width;
                if let Some(column) = &mut column {
                    *column += width;
                }
                cursor.bump();
            }
        }
    }
    if !content {
        return None;
    }
    let label_end = cursor.at;
    cursor.bump();
    if cursor.byte() != Some(b':') {
        return None;
    }
    cursor.bump();
    cursor.whitespace();
    let literal = cursor.byte() == Some(b'<');
    if literal {
        cursor.bump();
    }
    let destination_start = cursor.clone();
    let mut depth = 0usize;
    loop {
        match cursor.byte() {
            Some(b'>') if literal => break,
            None | Some(b'\r' | b'\n' | b'<') if literal => return None,
            None | Some(b' ' | b'\t' | b'\r' | b'\n') if !literal => break,
            Some(b'\\') => {
                cursor.bump();
                if cursor
                    .byte()
                    .is_some_and(|byte| byte.is_ascii_punctuation())
                {
                    cursor.bump();
                }
            }
            Some(b'(') if !literal => {
                depth += 1;
                cursor.bump();
            }
            Some(b')') if !literal => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                cursor.bump();
            }
            Some(1..=0x1f | 0x7f) if !literal => return None,
            Some(_) => cursor.bump(),
            None => unreachable!("both destination forms handle end of input above"),
        }
    }
    let destination_end = cursor.at;
    if depth != 0 || (!literal && destination_start.at == destination_end) {
        return None;
    }
    if literal {
        cursor.bump();
    }
    let checkpoint = cursor.line_end();
    let after_destination = cursor.consumed;
    cursor.whitespace();
    let mut title = None;
    let end = if cursor.consumed > after_destination
        && matches!(cursor.byte(), Some(b'"' | b'\'' | b'('))
    {
        let marker = match cursor.byte().unwrap() {
            b'(' => b')',
            byte => byte,
        };
        cursor.bump();
        let title_start = cursor.clone();
        let mut valid = true;
        loop {
            match cursor.byte() {
                Some(byte) if byte == marker => break,
                None => {
                    valid = false;
                    break;
                }
                Some(b'(') if marker == b')' => {
                    valid = false;
                    break;
                }
                Some(b'\\') => {
                    cursor.bump();
                    if cursor
                        .byte()
                        .is_some_and(|byte| byte.is_ascii_punctuation())
                    {
                        cursor.bump();
                    }
                }
                Some(b'\r' | b'\n') => {
                    cursor.newline();
                    cursor.spaces();
                    if matches!(cursor.byte(), None | Some(b'\r' | b'\n')) {
                        valid = false;
                        break;
                    }
                }
                Some(_) => cursor.bump(),
            }
        }
        if valid {
            let title_end = cursor.at;
            cursor.bump();
            if let Some(end) = cursor.line_end() {
                if title_end > title_start.at {
                    title = Some(title_start.value(title_end, Some(options)));
                }
                end
            } else {
                checkpoint?
            }
        } else {
            checkpoint?
        }
    } else {
        checkpoint?
    };
    Some(Metadata {
        label: label_start.value(label_end, None),
        destination: destination_start.value(destination_end, Some(options)),
        title,
        end,
    })
}
