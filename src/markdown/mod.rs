// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! API.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/index.js.

// Formatting rules are adapted from `mdast-util-to-markdown`; keep the parser's stricter lints on the surrounding API.
#![allow(clippy::pedantic)]

pub use self::configure::{IndentOptions, LineWrapping, Options};
use self::state::{Info, State};
use crate::{message::Message, Document, NodeId, ParseOptions};
use alloc::string::String;

extern crate alloc;
mod attention;
mod configure;
mod construct_name;
mod escape;
mod handle;
mod state;
mod util;

/// Normalize a Markdown document.
///
/// Parses `CommonMark`, GFM, frontmatter, and math ([`ParseOptions::normalize`]), then serializes the result with default formatting and soft source line breaks unwrapped. Explicit hard breaks, code content, and block boundaries are preserved. The output uses LF line endings and ends with one when non-empty. Tools that generate Markdown can pass their output through this function to produce exactly what `bake-markdown`'s `markdown:normalize` task writes.
///
/// ```
/// use base_markdown::normalize_markdown;
///
/// assert_eq!(normalize_markdown("* A wrapped\r\nline.").unwrap(), "- A wrapped line.\n");
/// ```
///
/// # Errors
/// Returns unsupported content shapes from serialization.
pub fn normalize_markdown(source: &str) -> Result<String, Message> {
    normalize_markdown_with_options(source, &ParseOptions::normalize())
}

/// Normalize a Markdown document using explicit parser options.
///
/// Use this for syntax beyond the GFM, frontmatter, and math support of [`normalize_markdown`], or to disable individual constructs. The output is serialized as by [`normalize_markdown`].
///
/// # Errors
/// Returns unsupported content shapes from serialization.
pub fn normalize_markdown_with_options(
    source: &str,
    parse_options: &ParseOptions,
) -> Result<String, Message> {
    let mut document = Document::parse(source, parse_options)
        .expect("the built-in Markdown parser produces a valid document");
    document.normalize();

    to_markdown_with_options(
        &document,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
}

/// Serialize an arena document to Markdown.
///
/// The output uses LF line endings. The parser stores every source line ending as LF in node values, so a CR in a value is a character: it is written as `&#xD;` where character references are decoded, such as in text and titles, and as it is in code, HTML and frontmatter, where a parsed value never has one.
///
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably. Other trees, such as those with empty text, attention or paragraphs, adjacent text, code spans or math, adjacent attention of the same kind, strikethrough inside strikethrough, whitespace at the edges of attention or blocks, or hard breaks in table cells or at the end of a block, are written without panicking, but possibly incorrectly or unstably.
///
/// # Errors
/// Returns invalid formatting options, unsupported syntax metadata, or unsupported content shapes.
pub fn to_markdown<S: AsRef<str>>(document: &Document<S>) -> Result<String, Message> {
    to_markdown_with_options(document, &Options::default())
}

/// Serialize an arena document with formatting options.
///
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably. Other trees, such as those with empty text, attention or paragraphs, adjacent text, code spans or math, adjacent attention of the same kind, strikethrough inside strikethrough, whitespace at the edges of attention or blocks, or hard breaks in table cells or at the end of a block, are written without panicking, but possibly incorrectly or unstably.
///
/// # Errors
/// Returns invalid formatting options, unsupported syntax metadata, or unsupported content shapes.
pub fn to_markdown_with_options<S: AsRef<str>>(
    document: &Document<S>,
    options: &Options,
) -> Result<String, Message> {
    to_markdown_node(document, document.root(), options)
}

/// Serialize a document subtree with formatting options.
///
/// The output always uses LF line endings. Non-empty block subtrees end with a line ending; inline (phrasing) subtrees, such as text, emphasis, or a link, are returned without an added line ending.
///
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably. Other trees, such as those with empty text, attention or paragraphs, adjacent text, code spans or math, adjacent attention of the same kind, strikethrough inside strikethrough, whitespace at the edges of attention or blocks, or hard breaks in table cells or at the end of a block, are written without panicking, but possibly incorrectly or unstably.
///
/// # Errors
/// Returns an invalid node index, invalid formatting options, unsupported syntax metadata, or unsupported content shapes.
pub fn to_markdown_node<S: AsRef<str>>(
    document: &Document<S>,
    node: NodeId,
    options: &Options,
) -> Result<String, Message> {
    if document.node(node).is_none() {
        return Err(state::invalid_node());
    }
    let kind = &document.node(node).unwrap().kind;
    let inline = state::is_phrasing(kind)
        || matches!(kind, crate::stream::EventKind::Opaque { block: false, .. });
    let mut state = State::new(options);
    state.serialization_root = Some(node);
    let mut result = state.handle(document, node, &Info::new("\n", "\n"))?;
    // Only block output is line-terminated, matching cmark's renderer.
    if !inline && !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}
