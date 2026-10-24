// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Closed, unindented frontmatter at the beginning of the document.
use super::scanners::{scan_blank_line, scan_nextline, LineStart};
use super::table::line_end;

/// Return the full authored extent and the next physical line. Failed lookahead
/// does not mutate block state or consume any part of an ordinary code fence.
pub(super) fn scan(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let marker = bytes[start];
    if !matches!(marker, b'-' | b'+' | b'`' | b'~') {
        return None;
    }
    let length = bytes[start..]
        .iter()
        .take_while(|&&byte| byte == marker)
        .count();
    if length < 3 || (matches!(marker, b'-' | b'+') && length != 3) {
        return None;
    }
    let first = start + scan_nextline(&bytes[start..]);
    let info = &bytes[start + length..line_end(bytes, first)];
    if first == line_end(bytes, first)
        || matches!(marker, b'`' | b'~') && info.iter().all(|byte| matches!(byte, b' ' | b'\t'))
        || marker == b'`' && info.contains(&b'`')
    {
        return None;
    }
    let mut at = first;
    while at < bytes.len() {
        let next = at + scan_nextline(&bytes[at..]);
        let mut line = LineStart::new(&bytes[at..]);
        if !matches!(marker, b'`' | b'~') || !line.scan_space(4) {
            let begin = at + line.bytes_scanned();
            let count = bytes[begin..]
                .iter()
                .take_while(|&&byte| byte == marker)
                .count();
            if count >= length
                && (matches!(marker, b'`' | b'~') || count == length)
                && scan_blank_line(&bytes[begin + count..]).is_some()
            {
                return Some((line_end(bytes, next), next));
            }
        }
        at = next;
    }
    None
}
