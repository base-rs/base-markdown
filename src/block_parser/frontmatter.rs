// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Closed, unindented frontmatter at the beginning of the document.
use super::scanners::scan_nextline;
use crate::util::line_ending::terminal_ending_start;
use crate::CodeFence;

/// Return the full authored extent and the next physical line. Failed lookahead
/// does not mutate block state.
pub(crate) fn scan(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let (fence, first) = open(bytes, start)?;
    let mut at = first;
    while at < bytes.len() {
        let next = at + scan_nextline(&bytes[at..]);
        let end = terminal_ending_start(bytes, next);
        if fence.frontmatter_close(&bytes[at..end]).is_some() {
            return Some((end, next));
        }
        at = next;
    }
    None
}

/// Return the opening fence and the line after it when the first line opens frontmatter.
fn open(bytes: &[u8], start: usize) -> Option<(CodeFence, usize)> {
    let fence = CodeFence::frontmatter_open(&bytes[start..])?;
    let first = start + scan_nextline(&bytes[start..]);
    (first != terminal_ending_start(bytes, first)).then_some((fence, first))
}

/// Whether the source opens frontmatter that has not closed yet.
pub(super) fn pending(bytes: &[u8], start: usize) -> bool {
    open(bytes, start).is_some() && scan(bytes, start).is_none()
}
