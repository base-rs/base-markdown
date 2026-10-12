// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::unist::Position;
use alloc::string::String;

/// Language-agnostic frontmatter with an optional, opaque format hint.
///
/// The contents are raw text. An explicit info string identifies the language;
/// untagged `---` and `+++` delimiters imply YAML and TOML respectively.
/// The Markdown parser leaves decoding the contents to callers.
///
/// Markdown serialization retains the stored info string and body while
/// normalizing spacing around the info string. Edited content can require a
/// longer fence, a different marker, or a trailing body line ending to keep
/// the frontmatter valid.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Frontmatter {
    /// Raw content between the fence lines, including its trailing line ending.
    pub value: String,
    /// The whole info string, with surrounding spaces and tabs removed.
    /// Escapes and character references remain literal, as in Markly.
    /// Empty for an implicit language; use `language()` to resolve the default.
    pub info: String,
    /// Opening fence marker: a backtick, tilde, dash, or plus.
    pub fence: char,
    /// Number of opening fence markers. Serialization may lengthen the fence
    /// to keep edited content from closing it prematurely.
    pub fence_length: usize,
    /// Number of closing fence markers. Closing code fences can be longer
    /// than their opening fence.
    pub closing_fence_length: usize,
    /// Source position, including the fences.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

impl Frontmatter {
    /// Return the explicit language hint, or the delimiter's default language.
    /// Untagged dash and plus delimiters imply YAML and TOML respectively.
    /// The parser does not validate or decode the named language.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.info
            .split_ascii_whitespace()
            .next()
            .or(match self.fence {
                '-' => Some("yaml"),
                '+' => Some("toml"),
                _ => None,
            })
    }
}
