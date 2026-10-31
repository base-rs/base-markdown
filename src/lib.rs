// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by Rafael Bachmann.
// Copyright, 2024, by Bnchi.
// Copyright, 2026, by Samuel Williams.

//! A pull parser, source-backed document AST, and shared HTML writer.
//!
//! Use [`to_html`] for direct rendering, [`EventParser`] for custom pull consumers, and [`Document::parse`] when inspecting or editing a syntax tree. Documents traverse the same semantic event contract as the parser. [`to_markdown`] serializes documents, and [`normalize_markdown`] rewrites source into canonical Markdown; [`render_events`] accepts parser or document events.
//!
//! ## Pull events and build a source-backed AST
//!
//! [`EventParser`] yields resolved semantic events on demand. Direct HTML rendering and the source-backed [`Document`] AST use this same contract. A document can own the input buffer, while ordinary text values retain byte ranges into it:
//!
//! ```
//! use base_markdown::{render_events, CompileOptions, Document, ParseOptions};
//!
//! let source = String::from("# Introduction\n\nHello, *world*!");
//! let document = Document::parse(source, &ParseOptions::default()).unwrap();
//! let mut html = String::new();
//! render_events(document.source(), document.events(), &CompileOptions::default(), &mut html).unwrap();
//! assert_eq!(html, "<h1>Introduction</h1>\n<p>Hello, <em>world</em>!</p>");
//! ```
//!
//! [`AstBuilder::consume`] accepts an event budget so callers can build the AST over several steps without collecting the complete stream. Parsing accepts a complete UTF-8 buffer and prepares block structure before yielding events; inline resolution is deferred until consumption. [`IncrementalParser`] accepts text in pieces, such as a streamed reply, and emits the events of each document-level block once later text can no longer change it.
//!
//! ## Edit and serialize a document
//!
//! Nodes use stable [`NodeId`] indices. Editing text with [`Document::text_mut`] copies only that value; detaching or moving a subtree retains its IDs and source metadata, and detached arena slots remain available until the document is dropped:
//!
//! ```
//! use base_markdown::{to_markdown, Document, HeadingOptions, ParseOptions};
//!
//! let mut document = Document::parse(String::from("# Introduction\n\nHello"), &ParseOptions::default()).unwrap();
//! let heading = document.headings(&HeadingOptions::default())[0].node;
//! let text = document.children(heading).next().unwrap();
//! document.text_mut(text).unwrap().push_str(" updated");
//! assert!(to_markdown(&document).unwrap().starts_with("# Introduction updated"));
//! ```
//!
//! [`Document::append_child`], [`Document::detach`], and [`Document::attach`] edit structure without rebuilding the tree, and [`Document::replace_heading_section`] copies replacement content from another document. [`Document::render_node`] and [`to_markdown_node`] render individual subtrees. [`NodeSyntax`] carries optional authored formatting, such as reference style, code info strings, and frontmatter fences, so Markdown serialization can preserve it.
//!
//! ## Parsing options
//!
//! The parser supports `CommonMark` and the GFM, frontmatter, and math constructs inherited from `markdown-rs`; see [`Constructs`] and [`Options::gfm`]. HTML output escapes raw HTML by default. MDX, JSX, and JavaScript expression parsing are not supported. Three additional [`ParseOptions`] are disabled by default, so ordinary parsing retains `CommonMark` behavior apart from angle-bracket autolinks, which deliberately accept only `http`, `https` and `mailto` schemes and email addresses (see [`Constructs::autolink`]): [`ParseOptions::inline_code_info`] recognizes a language prefix before inline code, [`ParseOptions::html_block_blank_lines`] continues indented HTML blocks across blank lines, and [`ParseOptions::html_tag_namespaces`] accepts namespace-prefixed HTML tags. [`Constructs::frontmatter`] accepts language-agnostic frontmatter, and [`CodeFence`] describes authored code fences.
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
mod definitions;
mod document;
mod event_html_renderer;
mod event_parser;
mod headings;
mod incremental_parser;
mod inline_parser;
pub mod markdown;
mod node_syntax;
mod reference_kind;
pub mod stream;
mod util;
mod value_builder;

pub mod message; // To do: externalize.

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
pub use incremental_parser::IncrementalParser;
pub use markdown::to_markdown_node;
pub use markdown::{normalize_markdown, normalize_markdown_with_options};
pub use markdown::{to_markdown, to_markdown_with_options};
pub use markdown::{IndentOptions, LineWrapping, Options as MarkdownOptions};
pub use node_syntax::{LinkForm, NodeSyntax};
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
/// use base_markdown::to_html;
///
/// assert_eq!(to_html("# Hi Mercury!"), "<h1>Hi Mercury!</h1>");
/// ```
pub fn to_html(value: &str) -> String {
    to_html_with_options(value, &Options::default())
}

/// Turn markdown into HTML, with configuration.
///
/// Markdown has no syntax errors, so this returns the rendered string directly.
///
/// ## Examples
///
/// ```
/// use base_markdown::{to_html_with_options, CompileOptions, Options};
///
/// // Use GFM:
/// let result = to_html_with_options("~Venus~Mars!", &Options::gfm());
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
/// });
///
/// assert_eq!(result, "<div>\n<h1>Hi Jupiter!</h1>\n</div>");
/// ```
pub fn to_html_with_options(value: &str, options: &Options) -> String {
    // Skip early growth steps while bounding spare capacity for mostly invisible input.
    let mut output = String::with_capacity(value.len().min(4096));
    let result = EventHtmlRenderer::new(value, &options.compile, &mut output)
        .render_markdown(&options.parse);
    match result {
        Ok(()) => output,
        Err(_) => unreachable!(
            "parser events are balanced and source-valid; writing to String cannot fail"
        ),
    }
}
