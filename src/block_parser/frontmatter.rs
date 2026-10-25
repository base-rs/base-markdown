// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Closed, unindented frontmatter at the beginning of the document.
use super::scanners::{scan_nextline, LineStart};
use super::table::line_end;
use crate::CodeFence;

/// Return the full authored extent and the next physical line. Failed lookahead
/// does not mutate block state or consume any part of an ordinary code fence.
pub(super) fn scan(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let fence = CodeFence::frontmatter_open(&bytes[start..])?;
    let first = start + scan_nextline(&bytes[start..]);
    let info = &bytes[start + fence.length..line_end(bytes, first)];
    if first == line_end(bytes, first) || !fence.valid_frontmatter_info(info) {
        return None;
    }
    let mut at = first;
    while at < bytes.len() {
        let next = at + scan_nextline(&bytes[at..]);
        let mut line = LineStart::new(&bytes[at..]);
        if !matches!(fence.character, '`' | '~') || !line.scan_space(4) {
            let begin = at + line.bytes_scanned();
            let end = line_end(bytes, next);
            if fence.frontmatter_close(&bytes[begin..end]).is_some() {
                return Some((end, next));
            }
        }
        at = next;
    }
    None
}
