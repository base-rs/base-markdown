// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

/// Opening fence metadata, matching Markly's `Node::Fence` structure.
///
/// This describes the source opening fence. The closing fence can be longer
/// or absent for ordinary code blocks. Markdown serialization may use a
/// different fence according to its formatting options and content safety.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CodeFence {
    /// Fence marker: a backtick or tilde for code, or a dash/plus for frontmatter.
    pub character: char,
    /// Number of markers in the opening fence.
    pub length: usize,
    /// Opening indentation in columns, relative to the containing block.
    /// Container markers and their required whitespace are excluded.
    pub indent: usize,
}

impl CodeFence {
    pub(crate) fn frontmatter_open(bytes: &[u8]) -> Option<Self> {
        let marker = *bytes.first()?;
        if !matches!(marker, b'-' | b'+' | b'`' | b'~') {
            return None;
        }
        let length = bytes.iter().take_while(|&&byte| byte == marker).count();
        if length < 3 || (matches!(marker, b'-' | b'+') && length != 3) {
            return None;
        }
        Some(Self {
            character: char::from(marker),
            length,
            indent: 0,
        })
    }

    pub(crate) fn valid_frontmatter_info(self, info: &[u8]) -> bool {
        !info.iter().any(|byte| matches!(byte, b'\r' | b'\n'))
            && (!matches!(self.character, '`' | '~')
                || !info.iter().all(|byte| matches!(byte, b' ' | b'\t')))
            && (self.character != '`' || !info.contains(&b'`'))
    }

    // The caller strips only the indentation permitted in its source context.
    pub(crate) fn frontmatter_close(self, line: &[u8]) -> Option<usize> {
        let count = line
            .iter()
            .take_while(|&&byte| char::from(byte) == self.character)
            .count();
        (count >= self.length
            && (!matches!(self.character, '-' | '+') || count == self.length)
            && line[count..]
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t')))
        .then_some(count)
    }
}
