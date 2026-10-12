// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2023, by Kyle McCarthy.
// Copyright, 2023, by Mia.
// Copyright, 2023, by Rafael Bachmann.
// Copyright, 2024, by Harsha Teja Kanna.
// Copyright, 2026, by Samuel Williams.

//! markdown syntax tree: [mdast][].
//!
//! [mdast]: https://github.com/syntax-tree/mdast
use crate::unist::Position;
use alloc::{
    fmt,
    string::{String, ToString},
    vec::Vec,
};

mod headings;
pub use headings::{HeadingEntry, HeadingOptions, Headings};
mod code;
pub use code::Code;
mod code_fence;
pub use code_fence::CodeFence;
mod frontmatter;
pub use frontmatter::Frontmatter;

/// MDX: relative byte index into a string, to an absolute byte index into the
/// whole document.
pub type Stop = (usize, usize);

/// Explicitness of a reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum ReferenceKind {
    /// The reference is implicit, its identifier inferred from its content.
    Shortcut,
    /// The reference is explicit, its identifier inferred from its content.
    Collapsed,
    /// The reference is explicit, its identifier explicitly set.
    Full,
}

/// GFM: alignment of phrasing content.
///
/// Used to align the contents of table cells within a table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlignKind {
    /// Left alignment.
    ///
    /// See the `left` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | :-- |
    ///       ^^^
    /// ```
    Left,
    /// Right alignment.
    ///
    /// See the `right` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | --: |
    ///       ^^^
    /// ```
    Right,
    /// Center alignment.
    ///
    /// See the `center` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | :-: |
    ///       ^^^
    /// ```
    Center,
    /// No alignment.
    ///
    /// Phrasing content is aligned as defined by the host environment.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | --- |
    ///       ^^^
    /// ```
    None,
}

/// Implement serde according to <https://github.com/syntax-tree/mdast#aligntype>
#[cfg(feature = "serde")]
impl serde::ser::Serialize for AlignKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            AlignKind::Left => serializer.serialize_unit_variant("AlignKind", 0, "left"),
            AlignKind::Right => serializer.serialize_unit_variant("AlignKind", 1, "right"),
            AlignKind::Center => serializer.serialize_unit_variant("AlignKind", 2, "center"),
            AlignKind::None => serializer.serialize_none(),
        }
    }
}

#[cfg(feature = "serde")]
struct AlignKindVisitor;

#[cfg(feature = "serde")]
impl serde::de::Visitor<'_> for AlignKindVisitor {
    type Value = AlignKind;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("'left', 'right', 'center' or null")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            "left" => Ok(AlignKind::Left),
            "right" => Ok(AlignKind::Right),
            "center" => Ok(AlignKind::Center),
            &_ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Str(v),
                &self,
            )),
        }
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            b"left" => Ok(AlignKind::Left),
            b"right" => Ok(AlignKind::Right),
            b"center" => Ok(AlignKind::Center),
            &_ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Bytes(v),
                &self,
            )),
        }
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(AlignKind::None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(AlignKind::None)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for AlignKind {
    fn deserialize<D>(deserializer: D) -> Result<AlignKind, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(AlignKindVisitor)
    }
}

/// Nodes.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename_all = "camelCase")
)]
pub enum Node {
    // Document:
    /// Root.
    Root(Root),

    // Container:
    /// A transparent fragment containing a sequence of nodes.
    ///
    /// Fragments are useful for holding children extracted from another node.
    Fragment(Fragment),
    /// Block quote.
    Blockquote(Blockquote),
    /// Footnote definition.
    FootnoteDefinition(FootnoteDefinition),
    /// MDX: JSX element (container).
    MdxJsxFlowElement(MdxJsxFlowElement),
    /// List.
    List(List),

    // Frontmatter:
    /// Language-agnostic frontmatter with an optional format hint.
    Frontmatter(Frontmatter),
    /// MDX.js ESM.
    MdxjsEsm(MdxjsEsm),

    // Phrasing:
    /// Break.
    Break(Break),
    /// Code (phrasing).
    InlineCode(InlineCode),
    /// Math (phrasing).
    InlineMath(InlineMath),
    /// Delete.
    Delete(Delete),
    /// Emphasis.
    Emphasis(Emphasis),
    // MDX: expression (text).
    MdxTextExpression(MdxTextExpression),
    /// Footnote reference.
    FootnoteReference(FootnoteReference),
    /// Html (phrasing).
    Html(Html),
    /// Image.
    Image(Image),
    /// Image reference.
    ImageReference(ImageReference),
    // MDX: JSX element (text).
    MdxJsxTextElement(MdxJsxTextElement),
    /// Link.
    Link(Link),
    /// Link reference.
    LinkReference(LinkReference),
    /// Strong
    Strong(Strong),
    /// Text.
    Text(Text),

    // Flow:
    /// Code (flow).
    Code(Code),
    /// Math (flow).
    Math(Math),
    // MDX: expression (flow).
    MdxFlowExpression(MdxFlowExpression),
    /// Heading.
    Heading(Heading),
    /// Html (flow).
    // Html(Html),
    /// Table.
    Table(Table),
    /// Thematic break.
    ThematicBreak(ThematicBreak),

    // Table content.
    /// Table row.
    TableRow(TableRow),

    // Row content.
    /// Table cell.
    TableCell(TableCell),

    // List content.
    /// List item.
    ListItem(ListItem),

    // Content.
    /// Definition.
    Definition(Definition),
    /// Paragraph.
    Paragraph(Paragraph),
}

impl fmt::Debug for Node {
    // Debug the wrapped struct.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Root(x) => x.fmt(f),
            Node::Fragment(x) => x.fmt(f),
            Node::Blockquote(x) => x.fmt(f),
            Node::FootnoteDefinition(x) => x.fmt(f),
            Node::MdxJsxFlowElement(x) => x.fmt(f),
            Node::List(x) => x.fmt(f),
            Node::MdxjsEsm(x) => x.fmt(f),
            Node::Frontmatter(x) => x.fmt(f),
            Node::Break(x) => x.fmt(f),
            Node::InlineCode(x) => x.fmt(f),
            Node::InlineMath(x) => x.fmt(f),
            Node::Delete(x) => x.fmt(f),
            Node::Emphasis(x) => x.fmt(f),
            Node::MdxTextExpression(x) => x.fmt(f),
            Node::FootnoteReference(x) => x.fmt(f),
            Node::Html(x) => x.fmt(f),
            Node::Image(x) => x.fmt(f),
            Node::ImageReference(x) => x.fmt(f),
            Node::MdxJsxTextElement(x) => x.fmt(f),
            Node::Link(x) => x.fmt(f),
            Node::LinkReference(x) => x.fmt(f),
            Node::Strong(x) => x.fmt(f),
            Node::Text(x) => x.fmt(f),
            Node::Code(x) => x.fmt(f),
            Node::Math(x) => x.fmt(f),
            Node::MdxFlowExpression(x) => x.fmt(f),
            Node::Heading(x) => x.fmt(f),
            Node::Table(x) => x.fmt(f),
            Node::ThematicBreak(x) => x.fmt(f),
            Node::TableRow(x) => x.fmt(f),
            Node::TableCell(x) => x.fmt(f),
            Node::ListItem(x) => x.fmt(f),
            Node::Definition(x) => x.fmt(f),
            Node::Paragraph(x) => x.fmt(f),
        }
    }
}

fn children_to_string(children: &[Node]) -> String {
    children.iter().map(ToString::to_string).collect()
}

// To do: clippy may be right but that’s a breaking change.
#[allow(clippy::to_string_trait_impl)]
impl ToString for Node {
    fn to_string(&self) -> String {
        match self {
            // Parents.
            Node::Root(x) => children_to_string(&x.children),
            Node::Fragment(x) => children_to_string(&x.children),
            Node::Blockquote(x) => children_to_string(&x.children),
            Node::FootnoteDefinition(x) => children_to_string(&x.children),
            Node::MdxJsxFlowElement(x) => children_to_string(&x.children),
            Node::List(x) => children_to_string(&x.children),
            Node::Delete(x) => children_to_string(&x.children),
            Node::Emphasis(x) => children_to_string(&x.children),
            Node::MdxJsxTextElement(x) => children_to_string(&x.children),
            Node::Link(x) => children_to_string(&x.children),
            Node::LinkReference(x) => children_to_string(&x.children),
            Node::Strong(x) => children_to_string(&x.children),
            Node::Heading(x) => children_to_string(&x.children),
            Node::Table(x) => children_to_string(&x.children),
            Node::TableRow(x) => children_to_string(&x.children),
            Node::TableCell(x) => children_to_string(&x.children),
            Node::ListItem(x) => children_to_string(&x.children),
            Node::Paragraph(x) => children_to_string(&x.children),

            // Literals.
            Node::MdxjsEsm(x) => x.value.clone(),
            Node::Frontmatter(x) => x.value.clone(),
            Node::InlineCode(x) => x.value.clone(),
            Node::InlineMath(x) => x.value.clone(),
            Node::MdxTextExpression(x) => x.value.clone(),
            Node::Html(x) => x.value.clone(),
            Node::Text(x) => x.value.clone(),
            Node::Code(x) => x.value.clone(),
            Node::Math(x) => x.value.clone(),
            Node::MdxFlowExpression(x) => x.value.clone(),

            // Voids.
            Node::Break(_)
            | Node::FootnoteReference(_)
            | Node::Image(_)
            | Node::ImageReference(_)
            | Node::ThematicBreak(_)
            | Node::Definition(_) => String::new(),
        }
    }
}

impl Node {
    #[must_use]
    pub fn children(&self) -> Option<&Vec<Node>> {
        match self {
            // Parent.
            Node::Root(x) => Some(&x.children),
            Node::Fragment(x) => Some(&x.children),
            Node::Paragraph(x) => Some(&x.children),
            Node::Heading(x) => Some(&x.children),
            Node::Blockquote(x) => Some(&x.children),
            Node::List(x) => Some(&x.children),
            Node::ListItem(x) => Some(&x.children),
            Node::Emphasis(x) => Some(&x.children),
            Node::Strong(x) => Some(&x.children),
            Node::Link(x) => Some(&x.children),
            Node::LinkReference(x) => Some(&x.children),
            Node::FootnoteDefinition(x) => Some(&x.children),
            Node::Table(x) => Some(&x.children),
            Node::TableRow(x) => Some(&x.children),
            Node::TableCell(x) => Some(&x.children),
            Node::Delete(x) => Some(&x.children),
            Node::MdxJsxFlowElement(x) => Some(&x.children),
            Node::MdxJsxTextElement(x) => Some(&x.children),
            // Non-parent.
            _ => None,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            // Parent.
            Node::Root(x) => Some(&mut x.children),
            Node::Fragment(x) => Some(&mut x.children),
            Node::Paragraph(x) => Some(&mut x.children),
            Node::Heading(x) => Some(&mut x.children),
            Node::Blockquote(x) => Some(&mut x.children),
            Node::List(x) => Some(&mut x.children),
            Node::ListItem(x) => Some(&mut x.children),
            Node::Emphasis(x) => Some(&mut x.children),
            Node::Strong(x) => Some(&mut x.children),
            Node::Link(x) => Some(&mut x.children),
            Node::LinkReference(x) => Some(&mut x.children),
            Node::FootnoteDefinition(x) => Some(&mut x.children),
            Node::Table(x) => Some(&mut x.children),
            Node::TableRow(x) => Some(&mut x.children),
            Node::TableCell(x) => Some(&mut x.children),
            Node::Delete(x) => Some(&mut x.children),
            Node::MdxJsxFlowElement(x) => Some(&mut x.children),
            Node::MdxJsxTextElement(x) => Some(&mut x.children),
            // Non-parent.
            _ => None,
        }
    }

    #[must_use]
    pub fn position(&self) -> Option<&Position> {
        match self {
            Node::Root(x) => x.position.as_ref(),
            Node::Fragment(_) => None,
            Node::Blockquote(x) => x.position.as_ref(),
            Node::FootnoteDefinition(x) => x.position.as_ref(),
            Node::MdxJsxFlowElement(x) => x.position.as_ref(),
            Node::List(x) => x.position.as_ref(),
            Node::MdxjsEsm(x) => x.position.as_ref(),
            Node::Frontmatter(x) => x.position.as_ref(),
            Node::Break(x) => x.position.as_ref(),
            Node::InlineCode(x) => x.position.as_ref(),
            Node::InlineMath(x) => x.position.as_ref(),
            Node::Delete(x) => x.position.as_ref(),
            Node::Emphasis(x) => x.position.as_ref(),
            Node::MdxTextExpression(x) => x.position.as_ref(),
            Node::FootnoteReference(x) => x.position.as_ref(),
            Node::Html(x) => x.position.as_ref(),
            Node::Image(x) => x.position.as_ref(),
            Node::ImageReference(x) => x.position.as_ref(),
            Node::MdxJsxTextElement(x) => x.position.as_ref(),
            Node::Link(x) => x.position.as_ref(),
            Node::LinkReference(x) => x.position.as_ref(),
            Node::Strong(x) => x.position.as_ref(),
            Node::Text(x) => x.position.as_ref(),
            Node::Code(x) => x.position.as_ref(),
            Node::Math(x) => x.position.as_ref(),
            Node::MdxFlowExpression(x) => x.position.as_ref(),
            Node::Heading(x) => x.position.as_ref(),
            Node::Table(x) => x.position.as_ref(),
            Node::ThematicBreak(x) => x.position.as_ref(),
            Node::TableRow(x) => x.position.as_ref(),
            Node::TableCell(x) => x.position.as_ref(),
            Node::ListItem(x) => x.position.as_ref(),
            Node::Definition(x) => x.position.as_ref(),
            Node::Paragraph(x) => x.position.as_ref(),
        }
    }

    pub fn position_mut(&mut self) -> Option<&mut Position> {
        match self {
            Node::Root(x) => x.position.as_mut(),
            Node::Fragment(_) => None,
            Node::Blockquote(x) => x.position.as_mut(),
            Node::FootnoteDefinition(x) => x.position.as_mut(),
            Node::MdxJsxFlowElement(x) => x.position.as_mut(),
            Node::List(x) => x.position.as_mut(),
            Node::MdxjsEsm(x) => x.position.as_mut(),
            Node::Frontmatter(x) => x.position.as_mut(),
            Node::Break(x) => x.position.as_mut(),
            Node::InlineCode(x) => x.position.as_mut(),
            Node::InlineMath(x) => x.position.as_mut(),
            Node::Delete(x) => x.position.as_mut(),
            Node::Emphasis(x) => x.position.as_mut(),
            Node::MdxTextExpression(x) => x.position.as_mut(),
            Node::FootnoteReference(x) => x.position.as_mut(),
            Node::Html(x) => x.position.as_mut(),
            Node::Image(x) => x.position.as_mut(),
            Node::ImageReference(x) => x.position.as_mut(),
            Node::MdxJsxTextElement(x) => x.position.as_mut(),
            Node::Link(x) => x.position.as_mut(),
            Node::LinkReference(x) => x.position.as_mut(),
            Node::Strong(x) => x.position.as_mut(),
            Node::Text(x) => x.position.as_mut(),
            Node::Code(x) => x.position.as_mut(),
            Node::Math(x) => x.position.as_mut(),
            Node::MdxFlowExpression(x) => x.position.as_mut(),
            Node::Heading(x) => x.position.as_mut(),
            Node::Table(x) => x.position.as_mut(),
            Node::ThematicBreak(x) => x.position.as_mut(),
            Node::TableRow(x) => x.position.as_mut(),
            Node::TableCell(x) => x.position.as_mut(),
            Node::ListItem(x) => x.position.as_mut(),
            Node::Definition(x) => x.position.as_mut(),
            Node::Paragraph(x) => x.position.as_mut(),
        }
    }

    pub fn position_set(&mut self, position: Option<Position>) {
        match self {
            Node::Root(x) => x.position = position,
            Node::Fragment(_) => (),
            Node::Blockquote(x) => x.position = position,
            Node::FootnoteDefinition(x) => x.position = position,
            Node::MdxJsxFlowElement(x) => x.position = position,
            Node::List(x) => x.position = position,
            Node::MdxjsEsm(x) => x.position = position,
            Node::Frontmatter(x) => x.position = position,
            Node::Break(x) => x.position = position,
            Node::InlineCode(x) => x.position = position,
            Node::InlineMath(x) => x.position = position,
            Node::Delete(x) => x.position = position,
            Node::Emphasis(x) => x.position = position,
            Node::MdxTextExpression(x) => x.position = position,
            Node::FootnoteReference(x) => x.position = position,
            Node::Html(x) => x.position = position,
            Node::Image(x) => x.position = position,
            Node::ImageReference(x) => x.position = position,
            Node::MdxJsxTextElement(x) => x.position = position,
            Node::Link(x) => x.position = position,
            Node::LinkReference(x) => x.position = position,
            Node::Strong(x) => x.position = position,
            Node::Text(x) => x.position = position,
            Node::Code(x) => x.position = position,
            Node::Math(x) => x.position = position,
            Node::MdxFlowExpression(x) => x.position = position,
            Node::Heading(x) => x.position = position,
            Node::Table(x) => x.position = position,
            Node::ThematicBreak(x) => x.position = position,
            Node::TableRow(x) => x.position = position,
            Node::TableCell(x) => x.position = position,
            Node::ListItem(x) => x.position = position,
            Node::Definition(x) => x.position = position,
            Node::Paragraph(x) => x.position = position,
        }
    }

    /// Visit this node and its descendants in depth-first, pre-order.
    pub fn walk<'a>(&'a self, mut visitor: impl FnMut(&'a Node)) {
        fn visit<'a>(node: &'a Node, visitor: &mut impl FnMut(&'a Node)) {
            visitor(node);
            if let Some(children) = node.children() {
                for child in children {
                    visit(child, visitor);
                }
            }
        }

        visit(self, &mut visitor);
    }

    /// Mutably visit this node and its descendants in depth-first, pre-order.
    pub fn walk_mut(&mut self, mut visitor: impl FnMut(&mut Node)) {
        fn visit(node: &mut Node, visitor: &mut impl FnMut(&mut Node)) {
            visitor(node);
            if let Some(children) = node.children_mut() {
                for child in children {
                    visit(child, visitor);
                }
            }
        }

        visit(self, &mut visitor);
    }

    /// Return the concatenated text represented by this node.
    ///
    /// Image nodes contribute their alt text and hard breaks contribute a
    /// newline. Structural nodes concatenate the text of their children.
    #[must_use]
    pub fn text_content(&self) -> String {
        match self {
            Node::Image(node) => node.alt.clone(),
            Node::ImageReference(node) => node.alt.clone(),
            Node::Break(_) => "\n".into(),
            Node::Definition(_) | Node::FootnoteReference(_) | Node::ThematicBreak(_) => {
                String::new()
            }
            _ => {
                if let Some(children) = self.children() {
                    children.iter().map(Node::text_content).collect()
                } else {
                    self.to_string()
                }
            }
        }
    }

    /// Find a direct child heading with the requested text.
    #[must_use]
    pub fn find_heading(&self, title: &str) -> Option<&Node> {
        self.children()?
            .iter()
            .find(|child| matches!(child, Node::Heading(_)) && child.text_content() == title)
    }

    /// Return the language identifier of inline code, fenced code, or frontmatter.
    #[must_use]
    pub fn code_language(&self) -> Option<&str> {
        match self {
            Node::InlineCode(node) => node.lang.as_deref(),
            Node::Code(node) => node.lang.as_deref(),
            Node::Frontmatter(node) => node.language(),
            _ => None,
        }
    }

    /// Return opening fence details for code blocks and frontmatter.
    ///
    /// `indent` is measured in columns relative to the containing block, not
    /// the absolute source column. Inline code, indented code, and nodes without
    /// retained fence metadata return `None`. Frontmatter fences have no indentation.
    #[must_use]
    pub fn code_fence(&self) -> Option<CodeFence> {
        match self {
            Node::Code(node) => node.fence,
            Node::Frontmatter(node) => Some(CodeFence {
                character: node.fence,
                length: node.fence_length,
                indent: 0,
            }),
            _ => None,
        }
    }

    /// Return the info string for inline code, fenced code, or frontmatter.
    ///
    /// Inline code metadata is its language token. Fenced code metadata is
    /// reconstructed from `lang` and `meta` when either is present.
    /// Frontmatter returns its authored info string, empty for an implicit language.
    #[must_use]
    pub fn code_info(&self) -> Option<String> {
        match self {
            Node::InlineCode(node) => node.lang.clone(),
            Node::Frontmatter(node) => Some(node.info.clone()),
            Node::Code(node) => match (&node.lang, &node.meta) {
                (Some(lang), Some(meta)) => Some(alloc::format!("{lang} {meta}")),
                (Some(lang), None) => Some(lang.clone()),
                (None, Some(meta)) => Some(meta.clone()),
                (None, None) => None,
            },
            _ => None,
        }
    }

    /// Take all direct children from this node and return them in a fragment.
    ///
    /// Returns `None` for nodes that cannot contain children. The extracted
    /// child nodes retain their own positions; the new fragment has no source
    /// position of its own.
    pub fn extract_children(&mut self) -> Option<Node> {
        self.children_mut().map(|children| {
            Node::Fragment(Fragment {
                children: core::mem::take(children),
            })
        })
    }

    /// Render this node with a user-provided renderer.
    pub fn render_with<R: crate::renderer::Renderer>(&self, renderer: &mut R) -> String {
        renderer.render(self)
    }

    /// Serialize this AST node or fragment as Markdown.
    ///
    /// Markdown output ends with a newline. For custom formatting, use
    /// [`to_markdown_with_options`][crate::markdown::to_markdown_with_options].
    ///
    /// # Example
    ///
    /// ```ignore
    /// let markdown = node.to_markdown();
    /// ```
    #[must_use]
    pub fn to_markdown(&self) -> String {
        crate::markdown::to_markdown(self).expect("Markdown serialization failed for this AST node")
    }

    /// Serialize this AST node or fragment as Markdown with custom options.
    ///
    /// # Errors
    ///
    /// Returns an error if an option contains an invalid marker or the AST
    /// contains a node that cannot be serialized.
    pub fn to_markdown_with_options(
        &self,
        options: &crate::markdown::Options,
    ) -> Result<String, crate::message::Message> {
        crate::markdown::to_markdown_with_options(self, options)
    }

    /// Replace a heading and the section below it in this node's children.
    ///
    /// `heading_index` is the direct-child index of the heading. When
    /// `replace_heading` is false, the heading stays and only its section
    /// content is replaced. When `remove_subsections` is true, nested heading
    /// sections are included through the next heading of the same or higher
    /// level. Returns `false` if the index is not a heading.
    pub fn replace_heading_section(
        &mut self,
        heading_index: usize,
        replacement: Vec<Node>,
        replace_heading: bool,
        remove_subsections: bool,
    ) -> bool {
        let Some(children) = self.children_mut() else {
            return false;
        };
        let depth = match children.get(heading_index) {
            Some(Node::Heading(heading)) => heading.depth,
            _ => return false,
        };
        let end = children
            .iter()
            .enumerate()
            .skip(heading_index + 1)
            .find_map(|(index, node)| {
                let Node::Heading(next) = node else {
                    return None;
                };
                if !remove_subsections || next.depth <= depth {
                    Some(index)
                } else {
                    None
                }
            })
            .unwrap_or(children.len());
        let start = if replace_heading {
            heading_index
        } else {
            heading_index + 1
        };

        children.splice(start..end, replacement);
        true
    }
}

/// MDX: attribute content.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(untagged)
)]
pub enum AttributeContent {
    /// JSX expression.
    ///
    /// ```markdown
    /// > | <a {...b} />
    ///        ^^^^^^
    /// ```
    Expression(MdxJsxExpressionAttribute),
    /// JSX property.
    ///
    /// ```markdown
    /// > | <a b />
    ///        ^
    /// ```
    Property(MdxJsxAttribute),
}
//
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename = "mdxJsxAttributeValueExpression")
)]
pub struct AttributeValueExpression {
    pub value: String,
    #[cfg_attr(feature = "serde", serde(rename = "_markdownRsStops"))]
    pub stops: Vec<Stop>,
}

/// MDX: attribute value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(untagged)
)]
pub enum AttributeValue {
    /// Expression value.
    ///
    /// ```markdown
    /// > | <a b={c} />
    ///          ^^^
    /// ```
    Expression(AttributeValueExpression),
    /// Static value.
    ///
    /// ```markdown
    /// > | <a b="c" />
    ///          ^^^
    /// ```
    Literal(String),
}

/// A transparent container for a sequence of AST nodes.
///
/// Fragments have no source position of their own. They are useful for holding
/// the children extracted from another node.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fragment {
    /// Content model.
    pub children: Vec<Node>,
}

/// Document.
///
/// ```markdown
/// > | a
///     ^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Root {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Paragraph.
///
/// ```markdown
/// > | a
///     ^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Paragraph {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Heading.
///
/// ```markdown
/// > | # a
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Heading {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Extra.
    /// Rank (between `1` and `6`, both including).
    pub depth: u8,
}

/// Thematic break.
///
/// ```markdown
/// > | ***
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThematicBreak {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Block quote.
///
/// ```markdown
/// > | > a
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blockquote {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// List.
///
/// ```markdown
/// > | * a
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct List {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Extra.
    /// Ordered (`true`) or unordered (`false`).
    pub ordered: bool,
    /// Starting number of the list.
    /// `None` when unordered.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub start: Option<u32>,
    /// One or more of its children are separated with a blank line from its
    /// siblings (when `true`), or not (when `false`).
    pub spread: bool,
}

/// List item.
///
/// ```markdown
/// > | * a
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListItem {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Extra.
    /// The item contains two or more children separated by a blank line
    /// (when `true`), or not (when `false`).
    pub spread: bool,
    /// GFM: whether the item is done (when `true`), not done (when `false`),
    /// or indeterminate or not applicable (`None`).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub checked: Option<bool>,
}

/// Html (flow or phrasing).
///
/// ```markdown
/// > | <a>
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Html {
    // Text.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Math (flow).
///
/// ```markdown
/// > | $$
///     ^^
/// > | a
///     ^
/// > | $$
///     ^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Math {
    // Text.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Extra.
    /// Custom info relating to the node.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub meta: Option<String>,
}

/// Definition.
///
/// ```markdown
/// > | [a]: b
///     ^^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Definition {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Resource.
    /// URL to the referenced resource.
    pub url: String,
    /// Advisory info for the resource, such as something that would be
    /// appropriate for a tooltip.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
    // Association.
    /// Value that can match another node.
    /// `identifier` is a source value: character escapes and character references
    /// are *not* parsed.
    /// Its value must be normalized.
    pub identifier: String,
    /// `label` is a string value: it works just like `title` on a link or a
    /// `lang` on code: character escapes and character references are parsed.
    ///
    /// To normalize a value, collapse markdown whitespace (`[\t\n\r ]+`) to a
    /// space, trim the optional initial and/or final space, and perform
    /// case-folding.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

/// Text.
///
/// ```markdown
/// > | a
///     ^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Text {
    // Text.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Emphasis.
///
/// ```markdown
/// > | *a*
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Emphasis {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Strong.
///
/// ```markdown
/// > | **a**
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Strong {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Code (phrasing).
///
/// ```markdown
/// > | `a`
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InlineCode {
    // Text.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    /// Optional language identifier parsed from an inline code prefix.
    ///
    /// This is a Socketry Markdown extension; standard mdast inline code
    /// nodes do not have a language field.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub lang: Option<String>,
}

/// Math (phrasing).
///
/// ```markdown
/// > | $a$
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InlineMath {
    // Text.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Break.
///
/// ```markdown
/// > | a\
///      ^
///   | b
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Break {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// Link.
///
/// ```markdown
/// > | [a](b)
///     ^^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Link {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Resource.
    /// URL to the referenced resource.
    pub url: String,
    /// Advisory info for the resource, such as something that would be
    /// appropriate for a tooltip.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
}

/// Image.
///
/// ```markdown
/// > | ![a](b)
///     ^^^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Alternative.
    /// Equivalent content for environments that cannot represent the node as
    /// intended.
    pub alt: String,
    // Resource.
    /// URL to the referenced resource.
    pub url: String,
    /// Advisory info for the resource, such as something that would be
    /// appropriate for a tooltip.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub title: Option<String>,
}

/// Link reference.
///
/// ```markdown
/// > | [a]
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinkReference {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Reference.
    /// Explicitness of a reference.
    #[cfg_attr(feature = "serde", serde(rename = "referenceType"))]
    pub reference_kind: ReferenceKind,
    // Association.
    /// Value that can match another node.
    /// `identifier` is a source value: character escapes and character references
    /// are *not* parsed.
    /// Its value must be normalized.
    pub identifier: String,
    /// `label` is a string value: it works just like `title` on a link or a
    /// `lang` on code: character escapes and character references are parsed.
    ///
    /// To normalize a value, collapse markdown whitespace (`[\t\n\r ]+`) to a
    /// space, trim the optional initial and/or final space, and perform
    /// case-folding.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

/// Image reference.
///
/// ```markdown
/// > | ![a]
///     ^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageReference {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Alternative.
    /// Equivalent content for environments that cannot represent the node as
    /// intended.
    pub alt: String,
    // Reference.
    /// Explicitness of a reference.
    #[cfg_attr(feature = "serde", serde(rename = "referenceType"))]
    pub reference_kind: ReferenceKind,
    // Association.
    /// Value that can match another node.
    /// `identifier` is a source value: character escapes and character references
    /// are *not* parsed.
    /// Its value must be normalized.
    pub identifier: String,
    /// `label` is a string value: it works just like `title` on a link or a
    /// `lang` on code: character escapes and character references are parsed.
    ///
    /// To normalize a value, collapse markdown whitespace (`[\t\n\r ]+`) to a
    /// space, trim the optional initial and/or final space, and perform
    /// case-folding.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

/// GFM: footnote definition.
///
/// ```markdown
/// > | [^a]: b
///     ^^^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FootnoteDefinition {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Association.
    /// Value that can match another node.
    /// `identifier` is a source value: character escapes and character references
    /// are *not* parsed.
    /// Its value must be normalized.
    pub identifier: String,
    /// `label` is a string value: it works just like `title` on a link or a
    /// `lang` on code: character escapes and character references are parsed.
    ///
    /// To normalize a value, collapse markdown whitespace (`[\t\n\r ]+`) to a
    /// space, trim the optional initial and/or final space, and perform
    /// case-folding.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

/// GFM: footnote reference.
///
/// ```markdown
/// > | [^a]
///     ^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FootnoteReference {
    // Void.
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Association.
    /// Value that can match another node.
    /// `identifier` is a source value: character escapes and character references
    /// are *not* parsed.
    /// Its value must be normalized.
    pub identifier: String,
    /// `label` is a string value: it works just like `title` on a link or a
    /// `lang` on code: character escapes and character references are parsed.
    ///
    /// To normalize a value, collapse markdown whitespace (`[\t\n\r ]+`) to a
    /// space, trim the optional initial and/or final space, and perform
    /// case-folding.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

/// GFM: table.
///
/// ```markdown
/// > | | a |
///     ^^^^^
/// > | | - |
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // Extra.
    /// Represents how cells in columns are aligned.
    pub align: Vec<AlignKind>,
}

/// GFM: table row.
///
/// ```markdown
/// > | | a |
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableRow {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// GFM: table cell.
///
/// ```markdown
/// > | | a |
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCell {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// GFM: delete.
///
/// ```markdown
/// > | ~~a~~
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Delete {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// MDX: ESM.
///
/// ```markdown
/// > | import a from 'b'
///     ^^^^^^^^^^^^^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MdxjsEsm {
    // Literal.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,

    // Custom data on where each slice of `value` came from.
    #[cfg_attr(feature = "serde", serde(rename = "_markdownRsStops"))]
    pub stops: Vec<Stop>,
}

/// MDX: expression (flow).
///
/// ```markdown
/// > | {a}
///     ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MdxFlowExpression {
    // Literal.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,

    // Custom data on where each slice of `value` came from.
    #[cfg_attr(feature = "serde", serde(rename = "_markdownRsStops"))]
    pub stops: Vec<Stop>,
}

/// MDX: expression (text).
///
/// ```markdown
/// > | a {b}
///       ^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MdxTextExpression {
    // Literal.
    /// Content model.
    pub value: String,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,

    // Custom data on where each slice of `value` came from.
    #[cfg_attr(feature = "serde", serde(rename = "_markdownRsStops"))]
    pub stops: Vec<Stop>,
}

/// MDX: JSX element (container).
///
/// ```markdown
/// > | <a />
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct MdxJsxFlowElement {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // JSX element.
    /// Name.
    ///
    /// Fragments have no name.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    /// Attributes.
    pub attributes: Vec<AttributeContent>,
}

/// MDX: JSX element (text).
///
/// ```markdown
/// > | <a />.
///     ^^^^^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct MdxJsxTextElement {
    // Parent.
    /// Content model.
    pub children: Vec<Node>,
    /// Positional info.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    // JSX element.
    /// Name.
    ///
    /// Fragments have no name.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    /// Attributes.
    pub attributes: Vec<AttributeContent>,
}

/// MDX: JSX attribute.
///
/// ```markdown
/// > | <a b />
///        ^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename = "mdxJsxAttribute")
)]
pub struct MdxJsxAttribute {
    // Void.
    /// Positional info.
    // pub position: Option<Position>,
    /// Key.
    pub name: String,
    /// Value.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub value: Option<AttributeValue>,
}

/// MDX: JSX expression attribute.
///
/// ```markdown
/// > | <a {...b} />
///        ^
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", rename = "mdxJsxExpressionAttribute")
)]
pub struct MdxJsxExpressionAttribute {
    /// Value.
    pub value: String,
    /// Stops
    #[cfg_attr(feature = "serde", serde(rename = "_markdownRsStops"))]
    pub stops: Vec<Stop>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unist::Position;
    use alloc::{format, string::ToString, vec};

    // Literals.

    #[test]
    fn text() {
        let mut node = Node::Text(Text {
            value: "a".into(),
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Text { value: \"a\", position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Text { value: \"a\", position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn inline_code() {
        let mut node = Node::InlineCode(InlineCode {
            value: "a".into(),
            position: None,
            lang: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "InlineCode { value: \"a\", position: None, lang: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "InlineCode { value: \"a\", position: Some(1:1-1:2 (0-1)), lang: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn code() {
        let mut node = Node::Code(Code {
            fence: None,
            value: "a".into(),
            position: None,
            lang: None,
            meta: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Code { value: \"a\", position: None, lang: None, meta: None, fence: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Code { value: \"a\", position: Some(1:1-1:2 (0-1)), lang: None, meta: None, fence: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn inline_math() {
        let mut node = Node::InlineMath(InlineMath {
            value: "a".into(),
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "InlineMath { value: \"a\", position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "InlineMath { value: \"a\", position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn math() {
        let mut node = Node::Math(Math {
            value: "a".into(),
            position: None,
            meta: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Math { value: \"a\", position: None, meta: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Math { value: \"a\", position: Some(1:1-1:2 (0-1)), meta: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn html() {
        let mut node = Node::Html(Html {
            value: "a".into(),
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Html { value: \"a\", position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Html { value: \"a\", position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn mdx_text_expression() {
        let mut node = Node::MdxTextExpression(MdxTextExpression {
            value: "a".into(),
            stops: vec![],
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "MdxTextExpression { value: \"a\", position: None, stops: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "MdxTextExpression { value: \"a\", position: Some(1:1-1:2 (0-1)), stops: [] }",
            "should support `position_set`"
        );
    }

    #[test]
    fn mdx_flow_expression() {
        let mut node = Node::MdxFlowExpression(MdxFlowExpression {
            value: "a".into(),
            stops: vec![],
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "MdxFlowExpression { value: \"a\", position: None, stops: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "MdxFlowExpression { value: \"a\", position: Some(1:1-1:2 (0-1)), stops: [] }",
            "should support `position_set`"
        );
    }

    #[test]
    fn mdxjs_esm() {
        let mut node = Node::MdxjsEsm(MdxjsEsm {
            value: "a".into(),
            stops: vec![],
            position: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "MdxjsEsm { value: \"a\", position: None, stops: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "a", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "MdxjsEsm { value: \"a\", position: Some(1:1-1:2 (0-1)), stops: [] }",
            "should support `position_set`"
        );
    }

    // Voids.

    #[test]
    fn break_node() {
        let mut node = Node::Break(Break { position: None });

        assert_eq!(
            format!("{node:?}"),
            "Break { position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Break { position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn thematic_break() {
        let mut node = Node::ThematicBreak(ThematicBreak { position: None });

        assert_eq!(
            format!("{node:?}"),
            "ThematicBreak { position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "ThematicBreak { position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn footnote_reference() {
        let mut node = Node::FootnoteReference(FootnoteReference {
            position: None,
            identifier: "a".into(),
            label: Some("b".into()),
        });

        assert_eq!(
            format!("{node:?}"),
            "FootnoteReference { position: None, identifier: \"a\", label: Some(\"b\") }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "FootnoteReference { position: Some(1:1-1:2 (0-1)), identifier: \"a\", label: Some(\"b\") }",
            "should support `position_set`"
        );
    }

    #[test]
    fn image_reference() {
        let mut node = Node::ImageReference(ImageReference {
            position: None,
            alt: "a".into(),
            identifier: "b".into(),
            label: Some("c".into()),
            reference_kind: ReferenceKind::Full,
        });

        assert_eq!(
            format!("{node:?}"),
            "ImageReference { position: None, alt: \"a\", reference_kind: Full, identifier: \"b\", label: Some(\"c\") }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "ImageReference { position: Some(1:1-1:2 (0-1)), alt: \"a\", reference_kind: Full, identifier: \"b\", label: Some(\"c\") }",
            "should support `position_set`"
        );
    }

    #[test]
    fn image() {
        let mut node = Node::Image(Image {
            position: None,
            alt: "a".into(),
            url: "b".into(),
            title: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Image { position: None, alt: \"a\", url: \"b\", title: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Image { position: Some(1:1-1:2 (0-1)), alt: \"a\", url: \"b\", title: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn definition() {
        let mut node = Node::Definition(Definition {
            position: None,
            identifier: "a".into(),
            label: None,
            url: "b".into(),
            title: None,
        });

        assert_eq!(
            format!("{node:?}"),
            "Definition { position: None, url: \"b\", title: None, identifier: \"a\", label: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(node.children_mut(), None, "should support `children_mut`");
        assert_eq!(node.children(), None, "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Definition { position: Some(1:1-1:2 (0-1)), url: \"b\", title: None, identifier: \"a\", label: None }",
            "should support `position_set`"
        );
    }

    // Parents.

    #[test]
    fn root() {
        let mut node = Node::Root(Root {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Root { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Root { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn block_quote() {
        let mut node = Node::Blockquote(Blockquote {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Blockquote { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Blockquote { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn delete() {
        let mut node = Node::Delete(Delete {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Delete { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Delete { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn emphasis() {
        let mut node = Node::Emphasis(Emphasis {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Emphasis { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Emphasis { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn strong() {
        let mut node = Node::Strong(Strong {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Strong { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Strong { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn paragraph() {
        let mut node = Node::Paragraph(Paragraph {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Paragraph { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Paragraph { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn table_row() {
        let mut node = Node::TableRow(TableRow {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "TableRow { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "TableRow { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn table_cell() {
        let mut node = Node::TableCell(TableCell {
            position: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "TableCell { children: [], position: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "TableCell { children: [], position: Some(1:1-1:2 (0-1)) }",
            "should support `position_set`"
        );
    }

    #[test]
    fn heading() {
        let mut node = Node::Heading(Heading {
            position: None,
            depth: 1,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Heading { children: [], position: None, depth: 1 }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Heading { children: [], position: Some(1:1-1:2 (0-1)), depth: 1 }",
            "should support `position_set`"
        );
    }

    #[test]
    fn table() {
        let mut node = Node::Table(Table {
            position: None,
            align: vec![],
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Table { children: [], position: None, align: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Table { children: [], position: Some(1:1-1:2 (0-1)), align: [] }",
            "should support `position_set`"
        );
    }

    #[test]
    fn list_item() {
        let mut node = Node::ListItem(ListItem {
            position: None,
            spread: false,
            checked: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "ListItem { children: [], position: None, spread: false, checked: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "ListItem { children: [], position: Some(1:1-1:2 (0-1)), spread: false, checked: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn list() {
        let mut node = Node::List(List {
            position: None,
            spread: false,
            ordered: false,
            start: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "List { children: [], position: None, ordered: false, start: None, spread: false }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "List { children: [], position: Some(1:1-1:2 (0-1)), ordered: false, start: None, spread: false }",
            "should support `position_set`"
        );
    }

    #[test]
    fn link_reference() {
        let mut node = Node::LinkReference(LinkReference {
            position: None,
            identifier: "a".into(),
            label: None,
            reference_kind: ReferenceKind::Full,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "LinkReference { children: [], position: None, reference_kind: Full, identifier: \"a\", label: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "LinkReference { children: [], position: Some(1:1-1:2 (0-1)), reference_kind: Full, identifier: \"a\", label: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn link() {
        let mut node = Node::Link(Link {
            position: None,
            url: "a".into(),
            title: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "Link { children: [], position: None, url: \"a\", title: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "Link { children: [], position: Some(1:1-1:2 (0-1)), url: \"a\", title: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn footnote_definition() {
        let mut node = Node::FootnoteDefinition(FootnoteDefinition {
            position: None,
            identifier: "a".into(),
            label: None,
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "FootnoteDefinition { children: [], position: None, identifier: \"a\", label: None }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "FootnoteDefinition { children: [], position: Some(1:1-1:2 (0-1)), identifier: \"a\", label: None }",
            "should support `position_set`"
        );
    }

    #[test]
    fn mdx_jsx_flow_element() {
        let mut node = Node::MdxJsxFlowElement(MdxJsxFlowElement {
            position: None,
            name: None,
            attributes: vec![],
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "MdxJsxFlowElement { children: [], position: None, name: None, attributes: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "MdxJsxFlowElement { children: [], position: Some(1:1-1:2 (0-1)), name: None, attributes: [] }",
            "should support `position_set`"
        );
    }

    #[test]
    fn mdx_jsx_text_element() {
        let mut node = Node::MdxJsxTextElement(MdxJsxTextElement {
            position: None,
            name: None,
            attributes: vec![],
            children: vec![],
        });

        assert_eq!(
            format!("{node:?}"),
            "MdxJsxTextElement { children: [], position: None, name: None, attributes: [] }",
            "should support `Debug`"
        );
        assert_eq!(node.to_string(), "", "should support `ToString`");
        assert_eq!(
            node.children_mut(),
            Some(&mut vec![]),
            "should support `children_mut`"
        );
        assert_eq!(node.children(), Some(&vec![]), "should support `children`");
        assert_eq!(node.position(), None, "should support `position`");
        assert_eq!(node.position_mut(), None, "should support `position`");
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(
            format!("{node:?}"),
            "MdxJsxTextElement { children: [], position: Some(1:1-1:2 (0-1)), name: None, attributes: [] }",
            "should support `position_set`"
        );
    }

    #[test]
    fn fragment_node_methods() {
        let text = Node::Text(Text {
            value: "fragment".into(),
            position: None,
        });
        let mut node = Node::Fragment(Fragment {
            children: vec![text],
        });

        assert_eq!(
            format!("{node:?}"),
            "Fragment { children: [Text { value: \"fragment\", position: None }] }"
        );
        assert_eq!(node.to_string(), "fragment");
        assert_eq!(node.children().map(Vec::len), Some(1));
        assert_eq!(node.position(), None);
        assert_eq!(node.position_mut(), None);
        node.position_set(Some(Position::new(1, 1, 0, 1, 2, 1)));
        assert_eq!(node.position(), None);

        let extracted = node.extract_children().expect("extract fragment children");
        assert_eq!(extracted.to_string(), "fragment");
        assert_eq!(node.children().unwrap(), &[]);
        assert!(node.extract_children().is_some());
    }

    #[test]
    fn walks_and_mutates_nested_children() {
        let mut node = Node::Root(Root {
            position: None,
            children: vec![Node::Paragraph(Paragraph {
                position: None,
                children: vec![Node::Text(Text {
                    value: "before".into(),
                    position: None,
                })],
            })],
        });

        let mut visited = Vec::new();
        node.walk(|node| visited.push(node.to_string()));
        assert_eq!(visited, ["before", "before", "before"]);

        node.walk_mut(|node| {
            if let Node::Text(text) = node {
                text.value.push_str(" after");
            }
        });
        assert_eq!(node.text_content(), "before after");
    }

    #[test]
    fn node_text_and_heading_helpers() {
        let image = Node::Image(Image {
            position: None,
            alt: "image alt".into(),
            url: "image.png".into(),
            title: None,
        });
        let image_reference = Node::ImageReference(ImageReference {
            position: None,
            alt: "reference alt".into(),
            reference_kind: ReferenceKind::Full,
            identifier: "image".into(),
            label: None,
        });
        let heading = Node::Heading(Heading {
            position: None,
            depth: 2,
            children: vec![Node::Text(Text {
                value: "Target".into(),
                position: None,
            })],
        });
        let root = Node::Root(Root {
            position: None,
            children: vec![image.clone(), image_reference.clone(), heading.clone()],
        });

        assert_eq!(image.text_content(), "image alt");
        assert_eq!(image_reference.text_content(), "reference alt");
        assert_eq!(Node::Break(Break { position: None }).text_content(), "\n");
        assert_eq!(
            Node::ThematicBreak(ThematicBreak { position: None }).text_content(),
            ""
        );
        assert_eq!(
            Node::Definition(Definition {
                position: None,
                identifier: "x".into(),
                label: None,
                url: "x".into(),
                title: None
            })
            .text_content(),
            ""
        );
        assert_eq!(root.find_heading("Target"), Some(&heading));
        assert_eq!(root.find_heading("missing"), None);
        assert_eq!(image.find_heading("Target"), None);
        assert_eq!(
            Node::Text(Text {
                value: "x".into(),
                position: None
            })
            .code_language(),
            None
        );
    }

    #[test]
    fn code_accessors_cover_optional_metadata() {
        let inline = Node::InlineCode(InlineCode {
            value: "code".into(),
            position: None,
            lang: Some("rust".into()),
        });
        assert_eq!(inline.code_language(), Some("rust"));
        assert_eq!(inline.code_info().as_deref(), Some("rust"));

        for (lang, meta, expected) in [
            (Some("rust"), Some("ignore"), Some("rust ignore")),
            (Some("rust"), None, Some("rust")),
            (None, Some("ignore"), Some("ignore")),
            (None, None, None),
        ] {
            let code = Node::Code(Code {
                fence: None,
                value: "code".into(),
                position: None,
                lang: lang.map(Into::into),
                meta: meta.map(Into::into),
            });
            assert_eq!(code.code_language(), lang);
            assert_eq!(code.code_info().as_deref(), expected);
        }

        assert_eq!(
            Node::Text(Text {
                value: "plain".into(),
                position: None,
            })
            .code_info(),
            None
        );
    }

    #[test]
    fn node_serializes_markdown_with_custom_options() {
        let node = Node::Text(Text {
            value: "text".into(),
            position: None,
        });
        let options = crate::markdown::Options::default();

        assert_eq!(node.to_markdown_with_options(&options).unwrap(), "text\n");
    }

    #[test]
    fn replaces_heading_sections_with_all_options() {
        let heading = |depth, title: &str| {
            Node::Heading(Heading {
                position: None,
                depth,
                children: vec![Node::Text(Text {
                    value: title.into(),
                    position: None,
                })],
            })
        };
        let text = |value: &str| {
            Node::Text(Text {
                value: value.into(),
                position: None,
            })
        };
        let replacement = || vec![text("replacement")];

        let mut root = Node::Root(Root {
            position: None,
            children: vec![
                heading(1, "one"),
                text("body"),
                heading(2, "nested"),
                text("nested body"),
                heading(1, "two"),
            ],
        });
        assert!(!root.replace_heading_section(1, replacement(), true, false));
        assert!(!text("not a parent").replace_heading_section(0, replacement(), true, false));
        assert!(root.replace_heading_section(0, replacement(), false, true));
        assert_eq!(root.to_string(), "onereplacementtwo");

        let mut root = Node::Root(Root {
            position: None,
            children: vec![
                heading(1, "one"),
                text("body"),
                heading(2, "nested"),
                text("nested body"),
                heading(1, "two"),
            ],
        });
        assert!(root.replace_heading_section(0, replacement(), true, false));
        assert_eq!(root.to_string(), "replacementnestednested bodytwo");

        let mut root = Node::Root(Root {
            position: None,
            children: vec![
                heading(1, "one"),
                text("body"),
                heading(2, "nested"),
                text("nested body"),
                heading(1, "two"),
            ],
        });
        assert!(root.replace_heading_section(0, replacement(), true, true));
        assert_eq!(root.to_string(), "replacementtwo");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn align_kind_serialization_and_visitors() {
        use serde::de::Visitor;

        assert_eq!(serde_json::to_string(&AlignKind::Left).unwrap(), "\"left\"");
        assert_eq!(
            serde_json::to_string(&AlignKind::Right).unwrap(),
            "\"right\""
        );
        assert_eq!(
            serde_json::to_string(&AlignKind::Center).unwrap(),
            "\"center\""
        );
        assert_eq!(serde_json::to_string(&AlignKind::None).unwrap(), "null");
        assert_eq!(
            serde_json::from_str::<AlignKind>("\"left\"").unwrap(),
            AlignKind::Left
        );
        assert_eq!(
            serde_json::from_str::<AlignKind>("null").unwrap(),
            AlignKind::None
        );

        assert_eq!(
            AlignKindVisitor
                .visit_bytes::<serde_json::Error>(b"right")
                .unwrap(),
            AlignKind::Right
        );
        assert_eq!(
            AlignKindVisitor
                .visit_bytes::<serde_json::Error>(b"left")
                .unwrap(),
            AlignKind::Left
        );
        assert_eq!(
            AlignKindVisitor
                .visit_bytes::<serde_json::Error>(b"center")
                .unwrap(),
            AlignKind::Center
        );
        assert_eq!(
            AlignKindVisitor.visit_none::<serde_json::Error>().unwrap(),
            AlignKind::None
        );
        assert_eq!(
            AlignKindVisitor.visit_unit::<serde_json::Error>().unwrap(),
            AlignKind::None
        );
        assert!(AlignKindVisitor
            .visit_str::<serde_json::Error>("invalid")
            .is_err());
        assert!(AlignKindVisitor
            .visit_bytes::<serde_json::Error>(b"invalid")
            .is_err());
    }
}
