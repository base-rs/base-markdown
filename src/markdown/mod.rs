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
use crate::{message::Message, Document, NodeId};
use alloc::string::String;

extern crate alloc;
mod configure;
mod construct_name;
mod handle;
mod state;
mod r#unsafe;
mod util;

/// Serialize an arena document to Markdown.
///
/// # Errors
/// Returns invalid formatting options, unsupported syntax metadata, or unsupported content shapes.
pub fn to_markdown<S: AsRef<str>>(document: &Document<S>) -> Result<String, Message> {
    to_markdown_with_options(document, &Options::default())
}

/// Serialize an arena document with formatting options.
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
    let mut state = State::new(options);
    state.serialization_root = Some(node);
    let mut result = state.handle(document, node, &Info::new("\n", "\n"))?;
    if !result.is_empty() && !result.ends_with(['\r', '\n']) {
        result.push('\n');
    }
    Ok(result)
}
