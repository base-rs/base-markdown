// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Markdown rendering for mdast trees.
use super::Renderer;
use crate::{markdown::Options, mdast::Node, message::Message};
use alloc::string::String;

/// Render an mdast tree or fragment as Markdown.
///
/// The default options produce standard Markdown. Use [`with_options`][Self::with_options]
/// to configure markers and other formatting choices, or [`try_render`][Self::try_render]
/// to handle invalid options without panicking.
///
/// # Example
///
/// ```
/// use core_markdown::{mdast::Node, to_mdast, MarkdownRenderer, ParseOptions};
///
/// let mut tree = to_mdast("# Hello *world*!", &ParseOptions::default()).unwrap();
/// if let Node::Heading(heading) = &mut tree.children_mut().unwrap()[0] {
///     heading.depth = 2;
/// }
///
/// let mut renderer = MarkdownRenderer::new();
/// let markdown = tree.render_with(&mut renderer);
/// assert_eq!(markdown, "## Hello *world*!\n");
/// ```
#[derive(Clone, Debug, Default)]
pub struct MarkdownRenderer {
    options: Options,
}

impl MarkdownRenderer {
    /// Create a Markdown renderer with default options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a Markdown renderer with custom serialization options.
    #[must_use]
    pub fn with_options(options: Options) -> Self {
        Self { options }
    }

    /// Return the options used by this renderer.
    #[must_use]
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Render a node, returning an error if the options are invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if an option contains an invalid marker or the AST
    /// contains a node that cannot be serialized.
    pub fn try_render(&mut self, node: &Node) -> Result<String, Message> {
        crate::markdown::to_markdown_with_options(node, &self.options)
    }
}

impl Renderer for MarkdownRenderer {
    fn render_node(&mut self, node: &Node) -> String {
        self.try_render(node)
            .expect("Markdown rendering failed; use try_render to handle errors")
    }
}
