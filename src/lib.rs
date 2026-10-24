// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by Rafael Bachmann.
// Copyright, 2024, by Bnchi.
// Copyright, 2026, by Samuel Williams.

//! A pull parser, source-backed document AST, and shared HTML writer.
//!
//! Use [`to_html`] for direct rendering, [`EventParser`] for custom pull consumers,
//! and [`Document::parse`] when inspecting or editing a syntax tree. Documents
//! traverse the same semantic event contract as the parser. [`to_markdown`]
//! serializes documents; [`render_events`] accepts parser or document events.
#![no_std]
#![deny(clippy::pedantic)]
#![allow(clippy::doc_link_with_quotes)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::result_large_err)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/wooorm/markdown-rs/8924580/media/logo-monochromatic.svg?sanitize=true"
)]

extern crate alloc;
mod align_kind;
mod block_parser;
mod byte_scan;
mod code_fence;
mod configuration;
mod construct;
mod definitions;
mod document;
mod event;
mod event_html_renderer;
mod event_parser;
mod grammar_parser;
mod headings;
mod inline_parser;
mod inline_workspace;
pub mod markdown;
mod node_syntax;
mod parser;
mod reference_kind;
mod resolve;
mod state;
pub mod stream;
mod subtokenize;
mod tokenizer;
mod util;
mod value_builder;

pub mod message; // To do: externalize.

#[doc(hidden)]
pub use util::character_reference::{decode_named, decode_numeric};

#[doc(hidden)]
pub use util::sanitize_uri::sanitize;

pub use util::line_ending::LineEnding;

pub use align_kind::AlignKind;
pub use code_fence::CodeFence;
pub use configuration::{CompileOptions, Constructs, Options, ParseOptions};
pub use document::{
    AstBuilder, BuildProgress, Children, Descendants, Document, DocumentEvents,
    Node as DocumentNode, NodeId,
};
pub use event_html_renderer::{render_events, EventHtmlRenderer};
pub use event_parser::EventParser;
pub use headings::{HeadingEntry, HeadingOptions};
pub use markdown::to_markdown_node;
pub use markdown::{to_markdown, to_markdown_with_options};
pub use markdown::{IndentOptions, LineWrapping, Options as MarkdownOptions};
pub use node_syntax::NodeSyntax;
pub use reference_kind::ReferenceKind;

use alloc::string::String;

/// Turn markdown into HTML.
///
/// Compiles markdown to HTML according to `CommonMark`.
/// Use [`to_html_with_options()`][] to configure how markdown is turned into
/// HTML.
///
/// ## Examples
///
/// ```
/// use core_markdown::to_html;
///
/// assert_eq!(to_html("# Hi Mercury!"), "<h1>Hi Mercury!</h1>");
/// ```
pub fn to_html(value: &str) -> String {
    to_html_with_options(value, &Options::default()).unwrap()
}

/// Turn markdown into HTML, with configuration.
///
/// ## Errors
///
/// `to_html_with_options()` never errors with normal markdown because markdown
/// does not have syntax errors, so feel free to `unwrap()`.
///
/// ## Examples
///
/// ```
/// use core_markdown::{to_html_with_options, CompileOptions, Options};
/// # fn main() -> Result<(), core_markdown::message::Message> {
///
/// // Use GFM:
/// let result = to_html_with_options("~Venus~Mars!", &Options::gfm())?;
///
/// assert_eq!(result, "<p><del>Venus</del>Mars!</p>");
///
/// // Live dangerously / trust the author:
/// let result = to_html_with_options("<div>\n\n# Hi Jupiter!\n\n</div>", &Options {
///     compile: CompileOptions {
///       allow_dangerous_html: true,
///       allow_dangerous_protocol: true,
///       ..CompileOptions::default()
///     },
///     ..Options::default()
/// })?;
///
/// assert_eq!(result, "<div>\n<h1>Hi Jupiter!</h1>\n</div>");
/// # Ok(())
/// # }
/// ```
pub fn to_html_with_options(value: &str, options: &Options) -> Result<String, message::Message> {
    // Skip early growth steps while bounding spare capacity for mostly invisible input.
    let mut output = String::with_capacity(value.len().min(4096));
    let result = EventHtmlRenderer::new(value, &options.compile, &mut output)
        .render_markdown(&options.parse);
    match result {
        Ok(()) => Ok(output),
        Err(_) => unreachable!(
            "parser events are balanced and source-valid; writing to String cannot fail"
        ),
    }
}
