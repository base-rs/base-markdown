// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by Rafael Bachmann.
// Copyright, 2024, by Bnchi.
// Copyright, 2026, by Samuel Williams.

//! Public API of `core-markdown`.
//!
//! This module exposes primarily [`to_html()`][].
//! It also exposes [`to_html_with_options()`][] and [`to_mdast()`][].
//!
//! * [`to_html()`][]
//!   — safe way to transform (untrusted?) markdown into HTML
//! * [`to_html_with_options()`][]
//!   — like `to_html` but lets you configure how markdown is turned into
//!   HTML, such as allowing dangerous HTML or turning on/off different
//!   constructs (GFM, MDX, and the like)
//! * [`to_mdast()`][]
//!   — turn markdown into a syntax tree
//! * [`Renderer`][]
//!   — render AST nodes and fragments with a user-defined renderer
//! * [`HtmlRenderer`][]
//!   — render an AST node or fragment directly as HTML
//! * [`MarkdownRenderer`][]
//!   — render an AST node or fragment back to Markdown
//!
//! ## Features
//!
//! * **`default`**
//!   — nothing is enabled by default
//! * **`log`**
//!   — enable logging (includes `dep:log`);
//!   you can show logs with `RUST_LOG=debug`
//! * **`serde`**
//!   — enable serde to serialize ASTs and configuration (includes `dep:serde`)
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
mod block_parser;
mod byte_scan;
mod configuration;
mod construct;
mod document;
mod event;
mod event_html_renderer;
mod event_parser;
mod grammar_parser;
mod inline_parser;
pub mod markdown;
mod parser;
pub mod renderer;
mod resolve;
mod state;
pub mod stream;
mod subtokenize;
mod to_mdast;
mod tokenizer;
mod util;

pub mod mdast; // To do: externalize?
pub mod message; // To do: externalize.
pub mod unist; // To do: externalize.

#[doc(hidden)]
pub use util::character_reference::{decode_named, decode_numeric};

#[doc(hidden)]
pub use util::identifier::{id_cont, id_start};

#[doc(hidden)]
pub use util::sanitize_uri::sanitize;

#[doc(hidden)]
pub use util::location::Location;

pub use util::line_ending::LineEnding;

pub use util::mdx::{
    EsmParse as MdxEsmParse, ExpressionKind as MdxExpressionKind,
    ExpressionParse as MdxExpressionParse, Signal as MdxSignal,
};

pub use configuration::{CompileOptions, Constructs, Options, ParseOptions};
pub use document::{
    AstBuilder, BuildProgress, Document, DocumentEvents, Node as DocumentNode, NodeId,
};
pub use event_html_renderer::{render_events, EventHtmlRenderer};
pub use event_parser::EventParser;
pub use markdown::{to_markdown, to_markdown_with_options};
pub use markdown::{IndentOptions, LineWrapping, Options as MarkdownOptions};
pub use renderer::{HtmlRenderer, MarkdownRenderer, Renderer};

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
/// However, MDX does have syntax errors.
/// When MDX is turned on, there are several errors that can occur with how
/// expressions, ESM, and JSX are written.
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
    let mut output = String::new();
    let result = EventHtmlRenderer::new(value, &options.compile, &mut output)
        .render_markdown(&options.parse);
    match result {
        Ok(()) => Ok(output),
        Err(stream::StreamError::Parse(error)) => Err(error),
        Err(_) => unreachable!(
            "parser events are balanced and source-valid; writing to String cannot fail"
        ),
    }
}

/// Turn markdown into a syntax tree.
///
/// ## Errors
///
/// `to_mdast()` never errors with normal markdown because markdown does not
/// have syntax errors, so feel free to `unwrap()`.
/// However, MDX does have syntax errors.
/// When MDX is turned on, there are several errors that can occur with how
/// JSX, expressions, or ESM are written.
///
/// ## Examples
///
/// ```
/// use core_markdown::{to_mdast, ParseOptions};
/// # fn main() -> Result<(), core_markdown::message::Message> {
///
/// let tree = to_mdast("# Hi *Earth*!", &ParseOptions::default())?;
///
/// println!("{:?}", tree);
/// // => Root { children: [Heading { children: [Text { value: "Hi ", position: Some(1:3-1:6 (2-5)) }, Emphasis { children: [Text { value: "Earth", position: Some(1:7-1:12 (6-11)) }], position: Some(1:6-1:13 (5-12)) }, Text { value: "!", position: Some(1:13-1:14 (12-13)) }], position: Some(1:1-1:14 (0-13)), depth: 1 }], position: Some(1:1-1:14 (0-13)) }
/// # Ok(())
/// # }
/// ```
pub fn to_mdast(value: &str, options: &ParseOptions) -> Result<mdast::Node, message::Message> {
    let (events, parse_state) = parser::parse(value, options)?;
    let node = to_mdast::compile(&events, parse_state.bytes, options.inline_code_info)?;
    Ok(node)
}
