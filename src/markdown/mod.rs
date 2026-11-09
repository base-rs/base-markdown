// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! API.
//!
//! JS equivalent: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/index.js>.

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
#[cfg(test)]
mod linear_tests;
mod state;
pub(crate) use self::{
    state::needs_blank_line, util::format_heading_as_setext::heading_as_setext,
    util::format_link_as_auto_link::angle_reads_back,
};
mod util;

/// Normalize a Markdown document.
///
/// Parses with the default options ([`ParseOptions::default`]: `CommonMark`, tables, task list items, footnotes, strikethrough, literal autolinks and frontmatter), then serializes the result with default formatting and soft source line breaks unwrapped. Explicit hard breaks, code content, and block boundaries are preserved. The output uses LF line endings and ends with one when non-empty. Tools that generate Markdown can pass their output through this function to produce exactly what `bake-markdown`'s `markdown:normalize` task writes. Section 5.5 of [the contract](crate::contract) states what the output reads back as.
///
/// ```
/// use base_markdown::normalize_markdown;
///
/// assert_eq!(normalize_markdown("* A wrapped\r\nline.").unwrap(), "- A wrapped line.\n");
/// ```
///
/// # Errors
/// Returns content nested more than 128 levels deep, and list item numbers of more than 9 digits, as for [`to_markdown`].
pub fn normalize_markdown(source: &str) -> Result<String, Message> {
    normalize_markdown_with_options(source, &ParseOptions::default())
}

/// Normalize a Markdown document using explicit parser options.
///
/// Use this for syntax beyond the default options of [`normalize_markdown`], such as single-tilde strikethrough, or to disable individual constructs. The output is serialized as by [`normalize_markdown`].
///
/// # Errors
/// Returns content nested more than 128 levels deep, and list item numbers of more than 9 digits, as for [`to_markdown`].
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
/// The output is written for the options the document records, [`Document::parse_options`], and
/// reads back under them: where they turn a construct off, another that reads back the same is
/// written, such as a setext heading without [`crate::Constructs::heading_atx`], and escapes that
/// only constructs they turn off would need, such as a table's delimiter row without
/// [`crate::Constructs::table`], are left out, though some that they read back the same remain.
/// Output read with other options may read differently.
///
/// The output uses LF line endings. The parser stores every source line ending as LF in node values, so a CR in a value is a character: it is written as `&#xD;` where character references are decoded, such as in text and titles, and as it is in code, HTML and frontmatter, where a parsed value never has one.
///
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably, and other trees are written without panicking, but possibly incorrectly or unstably, as [the contract](crate::contract), section 5, states.
///
/// # Errors
/// Returns invalid formatting options, unsupported syntax metadata, content nested too deeply, or a list item number that is too long. Serializing recurses once per level of nesting, so content nested more than 128 levels below the serialized node is an error rather than a stack overflow. Every block and inline node counts one level, including those in an image's text. CommonMark limits an ordered list marker to 9 digits, so an item whose number would have more, which would read back as text, is an error with the rule id `list-number-too-long`. An item's number is its list's start plus, when [`Options::increment_list_marker`] is on, its position among the list's items, so with it off only the start must fit.
pub fn to_markdown<S: AsRef<str>>(document: &Document<S>) -> Result<String, Message> {
    to_markdown_with_options(document, &Options::default())
}

/// Serialize an arena document with formatting options.
///
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably, and other trees are written without panicking, but possibly incorrectly or unstably, as [the contract](crate::contract), section 5, states.
///
/// # Errors
/// Returns invalid formatting options, unsupported syntax metadata, content nested more than 128 levels deep, or list item numbers of more than 9 digits, as for [`to_markdown`].
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
/// Call [`Document::normalize`] on a parsed or edited tree first: a normalized tree serializes correctly and stably, and other trees are written without panicking, but possibly incorrectly or unstably, as [the contract](crate::contract), section 5, states.
///
/// # Errors
/// Returns an invalid node index, invalid formatting options, unsupported syntax metadata, content nested more than 128 levels below `node`, or list item numbers of more than 9 digits, as for [`to_markdown`]. An item serialized on its own is numbered by its position in its list.
pub fn to_markdown_node<S: AsRef<str>>(
    document: &Document<S>,
    node: NodeId,
    options: &Options,
) -> Result<String, Message> {
    if document.node(node).is_none() {
        return Err(state::invalid_node());
    }
    let kind = &document.node(node).unwrap().kind;
    let inline = state::is_phrasing(kind);
    let mut state = State::new(options);
    state.parse = document.parse_options().clone();
    state.serialization_root = Some(node);
    let mut result = state.handle(document, node, &Info::new("\n", "\n"))?;
    // Only block output is line-terminated, matching cmark's renderer. An HTML block whose last
    // line is empty ends with that line's line ending, which is not the final one.
    if !inline
        && !result.is_empty()
        && (!result.ends_with('\n')
            || ends_in_empty_line(document, node)
            || state::ends_in_kept_line_ending(document, node, true))
    {
        result.push('\n');
    }
    Ok(result)
}

/// Whether the last node written for `node`, its last descendant but for separators, is an HTML block whose last line
/// is empty, which an unclosed block of kinds 1–5 that takes a blank line at its end has.
fn ends_in_empty_line<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> bool {
    let mut last = node;
    while let Some(child) = document
        .children(last)
        .filter(|&child| {
            !matches!(
                document.node(child).unwrap().kind,
                crate::stream::EventKind::Separator { .. }
            )
        })
        .last()
    {
        last = child;
    }
    matches!(
        &document.node(last).unwrap().kind,
        crate::stream::EventKind::Html { value, block: true }
            if value.resolve(document.source()).is_ok_and(crate::block_parser::html_block_ends_in_empty_line)
    )
}
