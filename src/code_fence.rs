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
