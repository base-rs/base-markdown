// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Source ranges for unescaped table rows; no per-cell temporary allocation.
use crate::AlignKind;
use core::ops::Range;

pub(super) fn trim(bytes: &[u8], mut range: Range<usize>) -> Range<usize> {
    while range.start < range.end && matches!(bytes[range.start], b' ' | b'\t') {
        range.start += 1;
    }
    while range.end > range.start && matches!(bytes[range.end - 1], b' ' | b'\t') {
        range.end -= 1;
    }
    range
}

pub(super) struct Cells<'a> {
    bytes: &'a [u8],
    start: usize,
    end: usize,
    span_start: usize,
    span_end: usize,
    finished: bool,
}

impl<'a> Cells<'a> {
    pub fn new(bytes: &'a [u8], range: Range<usize>) -> Self {
        let mut content = trim(bytes, range.clone());
        if bytes.get(content.start) == Some(&b'|') && content.start < content.end {
            content.start += 1;
        }
        if content.end > content.start
            && bytes[content.end - 1] == b'|'
            && !escaped(bytes, content.end - 1)
        {
            content.end -= 1;
        }
        Self {
            bytes,
            start: content.start,
            end: content.end,
            span_start: range.start,
            span_end: range.end,
            finished: false,
        }
    }
}

impl Iterator for Cells<'_> {
    type Item = (Range<usize>, Range<usize>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let mut cursor = self.start;
        let pipe = loop {
            let Some(offset) = memchr::memchr(b'|', &self.bytes[cursor..self.end]) else {
                break None;
            };
            let pipe = cursor + offset;
            if !escaped(self.bytes, pipe) {
                break Some(pipe);
            }
            cursor = pipe + 1;
        };
        if let Some(pipe) = pipe {
            let cell = (self.span_start..pipe, trim(self.bytes, self.start..pipe));
            self.span_start = pipe;
            self.start = pipe + 1;
            Some(cell)
        } else {
            self.finished = true;
            Some((
                self.span_start..self.span_end,
                trim(self.bytes, self.start..self.end),
            ))
        }
    }
}

pub(super) fn alignment(bytes: &[u8]) -> Option<AlignKind> {
    let left = bytes.first() == Some(&b':');
    let right = bytes.last() == Some(&b':');
    let begin = usize::from(left);
    let end = bytes.len().saturating_sub(usize::from(right));
    if begin >= end || !bytes[begin..end].iter().all(|&byte| byte == b'-') {
        return None;
    }
    Some(match (left, right) {
        (true, true) => AlignKind::Center,
        (true, false) => AlignKind::Left,
        (false, true) => AlignKind::Right,
        (false, false) => AlignKind::None,
    })
}

fn escaped(bytes: &[u8], at: usize) -> bool {
    let mut start = at;
    while start > 0 && bytes[start - 1] == b'\\' {
        start -= 1;
    }
    (at - start) % 2 == 1
}
