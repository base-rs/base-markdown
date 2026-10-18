// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Traits for rendering Markdown AST nodes.
mod html_renderer;
mod markdown_renderer;

pub use html_renderer::HtmlRenderer;
pub use markdown_renderer::MarkdownRenderer;

use crate::mdast::Node;
use alloc::string::String;

/// A renderer for an AST node tree.
///
/// Implement [`render_node`][Self::render_node] to decide how each node is
/// emitted. Call [`render_children`][Self::render_children] for transparent
/// containers or when a node's children should be rendered recursively.
/// Renderers choose their output format and escaping rules.
///
/// This is separate from [`crate::to_html`], which parses Markdown source and
/// renders the parser's events with the built-in HTML compiler.
///
/// # Example
///
/// ```
/// use core_markdown::{mdast::Node, Renderer};
///
/// struct PlainText;
///
/// impl Renderer for PlainText {
///     fn render_node(&mut self, node: &Node) -> String {
///         match node {
///             Node::Text(text) => text.value.clone(),
///             Node::InlineCode(code) => code.value.clone(),
///             Node::Code(code) => code.value.clone(),
///             Node::Break(_) => "\n".into(),
///             Node::Image(image) => image.alt.clone(),
///             Node::ImageReference(image) => image.alt.clone(),
///             Node::Definition(_) | Node::FootnoteReference(_) | Node::ThematicBreak(_) => {
///                 String::new()
///             }
///             _ => self.render_children(node),
///         }
///     }
/// }
///
/// let node = Node::Fragment(core_markdown::mdast::Fragment {
///     children: vec![Node::Text(core_markdown::mdast::Text {
///         value: "hello".into(),
///         position: None,
///     })],
/// });
/// let mut renderer = PlainText;
/// let plain_text = node.render_with(&mut renderer);
/// assert_eq!(plain_text, "hello");
/// ```
pub trait Renderer {
    /// Render one AST node into this renderer's output.
    fn render_node(&mut self, node: &Node) -> String;

    /// Render a node and its descendants.
    fn render(&mut self, node: &Node) -> String {
        self.render_node(node)
    }

    /// Render the direct children of a parent node in order.
    fn render_children(&mut self, node: &Node) -> String {
        let mut output = String::new();

        if let Some(children) = node.children() {
            for child in children {
                output.push_str(&self.render(child));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::Renderer;
    use crate::mdast::{Node, Root, Text};
    use alloc::string::String;
    use alloc::vec;

    struct PlainTextRenderer;

    impl Renderer for PlainTextRenderer {
        fn render_node(&mut self, node: &Node) -> String {
            match node {
                Node::Text(text) => text.value.clone(),
                _ => self.render_children(node),
            }
        }
    }

    #[test]
    fn renders_nodes_and_their_children_by_default() {
        let node = Node::Root(Root {
            position: None,
            children: vec![
                Node::Text(Text {
                    value: "one".into(),
                    position: None,
                }),
                Node::Text(Text {
                    value: "two".into(),
                    position: None,
                }),
            ],
        });
        let mut renderer = PlainTextRenderer;

        assert_eq!(renderer.render(&node), "onetwo");
        assert_eq!(
            renderer.render_children(&Node::ThematicBreak(crate::mdast::ThematicBreak {
                position: None,
            })),
            ""
        );
    }
}
