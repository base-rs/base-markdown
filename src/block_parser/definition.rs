// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Scan ordinary definition metadata without constructing grammar tokens.
use crate::definitions::scan_label;
use core::ops::Range;

pub(super) struct Metadata {
    pub label: Range<usize>,
    pub destination: Range<usize>,
    pub title: Option<Range<usize>>,
}

/// Multiline metadata and invalid candidates retain the grammar's rollback rules.
pub(super) fn scan(bytes: &[u8], start: usize, end: usize) -> Option<Metadata> {
    let (mut at, label) = scan_label(bytes, start, end)?;
    if bytes.get(at) != Some(&b':') || bytes[label.clone()].contains(&0) {
        return None;
    }
    at += 1;
    while at < end && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    let destination;
    if bytes.get(at) == Some(&b'<') {
        at += 1;
        let begin = at;
        while at < end && bytes[at] != b'>' {
            match bytes[at] {
                0 | b'<' | b'\r' | b'\n' => return None,
                b'\\' if at + 1 < end && matches!(bytes[at + 1], b'<' | b'>' | b'\\') => at += 1,
                _ => {}
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
        let mut depth = 0usize;
        while at < end && !matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
            match bytes[at] {
                0..=0x1f | 0x7f => return None,
                b'\\' if at + 1 < end && matches!(bytes[at + 1], b'(' | b')' | b'\\') => at += 1,
                b'(' => depth += 1,
                b')' if depth == 0 => return None,
                b')' => depth -= 1,
                _ => {}
            }
            at += 1;
        }
        if at == begin || depth != 0 {
            return None;
        }
        destination = begin..at;
    }
    let whitespace = at;
    while at < end && matches!(bytes[at], b' ' | b'\t') {
        at += 1;
    }
    let mut title = None;
    if at > whitespace && at < end && matches!(bytes[at], b'"' | b'\'' | b'(') {
        let marker = if bytes[at] == b'(' { b')' } else { bytes[at] };
        at += 1;
        let begin = at;
        while at < end && bytes[at] != marker {
            match bytes[at] {
                0 | b'\r' | b'\n' => return None,
                b'\\' if at + 1 < end && matches!(bytes[at + 1], b'"' | b'\'' | b')' | b'\\') => {
                    at += 1
                }
                _ => {}
            }
            at += 1;
        }
        if at == end {
            return None;
        }
        title = (begin < at).then_some(begin..at);
        at += 1;
        while at < end && matches!(bytes[at], b' ' | b'\t') {
            at += 1;
        }
    }
    if at != end && !matches!(bytes[at], b'\r' | b'\n') {
        return None;
    }
    Some(Metadata {
        label,
        destination,
        title,
    })
}
