// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Declarative test inputs, converted to the public document AST before rendering.
//! These types describe fixtures only; there is no parser or renderer implementation here.
#![allow(dead_code)]
use base_markdown::stream::Text as EventText;
pub use base_markdown::{AlignKind, CodeFence, ReferenceKind};
use base_markdown::{Document, NodeId, NodeSyntax, ParseOptions};
use std::collections::BTreeMap;

use base_markdown::stream::{EventKind, Reference, Tag};
/// Historical source coordinates retained only as fixture descriptions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position([usize; 6]);

impl Position {
    pub fn new(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> Self {
        Self([a, b, c, d, e, f])
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Fragment(Fragment),
    Root(Root),
    Paragraph(Paragraph),
    Heading(Heading),
    ThematicBreak(ThematicBreak),
    Blockquote(Blockquote),
    List(List),
    ListItem(ListItem),
    Html(Html),
    Definition(Definition),
    Text(Text),
    Emphasis(Emphasis),
    Strong(Strong),
    InlineCode(InlineCode),
    Break(Break),
    Link(Link),
    Image(Image),
    LinkReference(LinkReference),
    ImageReference(ImageReference),
    FootnoteDefinition(FootnoteDefinition),
    FootnoteReference(FootnoteReference),
    Table(Table),
    TableRow(TableRow),
    TableCell(TableCell),
    Delete(Delete),
    Code(Code),
    Frontmatter(Frontmatter),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paragraph {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub depth: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThematicBreak {
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blockquote {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub ordered: bool,
    pub start: Option<u32>,
    pub spread: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub spread: bool,
    pub checked: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Html {
    pub value: String,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub position: Option<Position>,
    pub url: String,
    pub title: Option<String>,
    pub identifier: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub value: String,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emphasis {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strong {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineCode {
    pub value: String,
    pub position: Option<Position>,
    pub lang: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Break {
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub position: Option<Position>,
    pub alt: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkReference {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub reference_kind: ReferenceKind,
    pub identifier: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageReference {
    pub position: Option<Position>,
    pub alt: String,
    pub reference_kind: ReferenceKind,
    pub identifier: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootnoteDefinition {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub identifier: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootnoteReference {
    pub position: Option<Position>,
    pub identifier: String,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub children: Vec<Node>,
    pub position: Option<Position>,
    pub align: Vec<AlignKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delete {
    pub children: Vec<Node>,
    pub position: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub value: String,
    pub position: Option<Position>,
    pub lang: Option<String>,
    pub meta: Option<String>,
    pub fence: Option<CodeFence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frontmatter {
    pub value: String,
    pub info: String,
    pub fence: char,
    pub fence_length: usize,
    pub closing_fence_length: usize,
    pub position: Option<Position>,
}

impl Node {
    pub fn to_markdown(&self) -> String {
        to_markdown(self).unwrap()
    }
    pub fn children(&self) -> Option<&Vec<Node>> {
        match self {
            Self::Fragment(n) => Some(&n.children),
            Self::Root(n) => Some(&n.children),
            Self::Paragraph(n) => Some(&n.children),
            Self::Heading(n) => Some(&n.children),
            Self::Blockquote(n) => Some(&n.children),
            Self::List(n) => Some(&n.children),
            Self::ListItem(n) => Some(&n.children),
            Self::Emphasis(n) => Some(&n.children),
            Self::Strong(n) => Some(&n.children),
            Self::Link(n) => Some(&n.children),
            Self::LinkReference(n) => Some(&n.children),
            Self::FootnoteDefinition(n) => Some(&n.children),
            Self::Table(n) => Some(&n.children),
            Self::TableRow(n) => Some(&n.children),
            Self::TableCell(n) => Some(&n.children),
            Self::Delete(n) => Some(&n.children),
            _ => None,
        }
    }
    pub fn document(&self) -> Document {
        let mut definitions = BTreeMap::new();
        fn collect(node: &Node, definitions: &mut BTreeMap<String, (String, Option<String>)>) {
            if let Node::Definition(n) = node {
                definitions
                    .entry(normalize(&n.identifier))
                    .or_insert((n.url.clone(), n.title.clone()));
            }
            if let Some(children) = node.children() {
                for child in children {
                    collect(child, definitions);
                }
            }
        }
        collect(self, &mut definitions);
        let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
        let root = document.root();
        if matches!(self, Self::Root(_) | Self::Fragment(_)) {
            let children = self.children().unwrap();
            // Phrasing among blocks is put in paragraphs, as in any container of blocks.
            let blocks = !children.iter().all(phrasing);
            for child in children {
                let parent = if blocks && phrasing(child) {
                    document
                        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
                        .unwrap()
                } else {
                    root
                };
                append(
                    &mut document,
                    parent,
                    child,
                    &definitions,
                    false,
                    false,
                    &[],
                );
            }
        } else if let Self::ListItem(n) = self {
            // An item is only ever in a list, so a lone item is the only item of one, which is
            // tight unless the item records spread.
            let list = document
                .append_child(
                    root,
                    EventKind::Start(Tag::List {
                        start: None,
                        tight: !n.spread,
                    }),
                )
                .unwrap();
            append(
                &mut document,
                list,
                self,
                &definitions,
                !n.spread,
                false,
                &[],
            );
        } else if matches!(self, Self::TableRow(_) | Self::TableCell(_)) {
            // A row is only ever in a table, so a lone row is a table's head row, and a lone cell
            // its only cell.
            let mut parent = root;
            for tag in [Tag::Table, Tag::TableHead] {
                parent = document
                    .append_child(parent, EventKind::Start(tag))
                    .unwrap();
            }
            if matches!(self, Self::TableCell(_)) {
                parent = document
                    .append_child(parent, EventKind::Start(Tag::TableRow))
                    .unwrap();
            }
            append(&mut document, parent, self, &definitions, false, true, &[]);
        } else {
            append(&mut document, root, self, &definitions, false, false, &[]);
        }
        document
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// The stored value of a code block whose `mdast` value is `value`: each line ends in a line ending, so an empty body is `""` and a body of one blank line is `"\n"`.
fn block_value(value: &str) -> EventText<'static> {
    if value.is_empty() {
        self::value("")
    } else {
        format!("{value}\n").into()
    }
}

fn value(value: &str) -> EventText<'static> {
    value.to_string().into()
}
fn title(value: &Option<String>) -> Option<EventText<'static>> {
    value.as_ref().map(|s| self::value(s))
}
fn association(label: Option<&str>, identifier: &str) -> EventText<'static> {
    if let Some(label) = label {
        return value(label);
    }
    let pattern = regex::RegexBuilder::new(
        r"\\([!-/:-@\[-`{-~])|&(#(?:\d{1,7}|x[\da-f]{1,6})|[\da-z]{1,31});",
    )
    .case_insensitive(true)
    .build()
    .unwrap();
    let decoded = pattern.replace_all(identifier, |captures: &regex::Captures<'_>| {
        if let Some(escaped) = captures.get(1) {
            return escaped.as_str().to_owned();
        }
        let document = Document::parse(captures[0].to_owned(), &ParseOptions::default()).unwrap();
        document.text_content(document.root())
    });
    value(&decoded)
}
fn list_spread(list: &List) -> bool {
    list.spread
        || list
            .children
            .iter()
            .any(|child| matches!(child,Node::ListItem(item) if item.spread))
}

/// Whether `node` is phrasing, which in a tree of this crate is only ever in phrasing.
fn phrasing(node: &Node) -> bool {
    matches!(
        node,
        Node::Text(_)
            | Node::Emphasis(_)
            | Node::Strong(_)
            | Node::Delete(_)
            | Node::InlineCode(_)
            | Node::Break(_)
            | Node::Link(_)
            | Node::Image(_)
            | Node::LinkReference(_)
            | Node::ImageReference(_)
            | Node::FootnoteReference(_)
    )
}

fn append(
    document: &mut Document,
    parent: NodeId,
    node: &Node,
    definitions: &BTreeMap<String, (String, Option<String>)>,
    tight: bool,
    header: bool,
    align: &[AlignKind],
) -> NodeId {
    let mut syntax = None;
    let kind = match node {
        Node::Root(_) | Node::Fragment(_) => unreachable!("only the fixture root is transparent"),
        Node::Paragraph(_) => EventKind::Start(Tag::Paragraph { tight }),
        Node::Heading(n) => EventKind::Start(Tag::Heading(n.depth)),
        Node::ThematicBreak(_) => EventKind::Rule,
        Node::Blockquote(_) => EventKind::Start(Tag::BlockQuote),
        Node::List(n) => EventKind::Start(Tag::List {
            start: n.ordered.then_some(n.start.unwrap_or(1)),
            tight: !list_spread(n),
        }),
        Node::ListItem(n) => {
            syntax = Some(NodeSyntax::ListItem { spread: n.spread });
            EventKind::Start(Tag::Item {
                block_end: !tight && matches!(n.children.last(), Some(Node::Paragraph(_))),
                checked: n
                    .checked
                    .filter(|_| matches!(n.children.first(), Some(Node::Paragraph(_)))),
            })
        }
        Node::Html(n) => EventKind::Html {
            value: value(&n.value),
            block: !matches!(
                document.node(parent).unwrap().kind,
                EventKind::Start(
                    Tag::Paragraph { .. }
                        | Tag::Heading(_)
                        | Tag::Emphasis
                        | Tag::Strong
                        | Tag::Delete
                        | Tag::Link { .. }
                        | Tag::Image { .. }
                        | Tag::TableCell { .. }
                )
            ),
        },
        Node::Text(n) => EventKind::Text(value(&n.value)),
        Node::Code(n) => EventKind::CodeBlock {
            value: block_value(&n.value),
            info: n.lang.as_ref().map(|lang| {
                value(&match &n.meta {
                    Some(meta) => format!("{lang} {meta}"),
                    None => lang.clone(),
                })
            }),
            fence: n.fence,
            unclosed: false,
        },
        Node::Definition(n) => EventKind::Definition {
            label: association(n.label.as_deref(), &n.identifier),
            destination: value(&n.url),
            title: title(&n.title),
        },
        Node::Emphasis(_) => EventKind::Start(Tag::Emphasis),
        Node::Strong(_) => EventKind::Start(Tag::Strong),
        Node::Delete(_) => EventKind::Start(Tag::Delete),
        Node::InlineCode(n) => EventKind::Code {
            value: value(&n.value),
            language: title(&n.lang),
        },
        Node::Break(_) => EventKind::HardBreak,
        Node::Link(n) => EventKind::Start(Tag::Link {
            destination: value(&n.url),
            title: title(&n.title),
            reference: None,
        }),
        Node::Image(n) => EventKind::Start(Tag::Image {
            destination: value(&n.url),
            title: title(&n.title),
            reference: None,
        }),
        Node::LinkReference(n) => {
            let resource = definitions.get(&normalize(&n.identifier));
            EventKind::Start(Tag::Link {
                destination: value(resource.map_or("", |r| r.0.as_str())),
                title: resource.and_then(|r| title(&r.1)),
                reference: Some(Box::new(Reference {
                    kind: n.reference_kind,
                    label: association(n.label.as_deref(), &n.identifier),
                })),
            })
        }
        Node::ImageReference(n) => {
            let resource = definitions.get(&normalize(&n.identifier));
            EventKind::Start(Tag::Image {
                destination: value(resource.map_or("", |r| r.0.as_str())),
                title: resource.and_then(|r| title(&r.1)),
                reference: Some(Box::new(Reference {
                    kind: n.reference_kind,
                    label: association(n.label.as_deref(), &n.identifier),
                })),
            })
        }
        Node::FootnoteDefinition(n) => EventKind::Start(Tag::FootnoteDefinition {
            identifier: value(&normalize(&n.identifier)),
            label: Some(value(n.label.as_deref().unwrap_or(&n.identifier))),
        }),
        Node::FootnoteReference(n) => EventKind::FootnoteReference {
            identifier: value(&normalize(&n.identifier)),
            label: Some(value(n.label.as_deref().unwrap_or(&n.identifier))),
        },
        Node::Table(_) => EventKind::Start(Tag::Table),
        Node::TableRow(_) => EventKind::Start(Tag::TableRow),
        Node::TableCell(_) => EventKind::Start(Tag::TableCell {
            alignment: align.first().copied().unwrap_or(AlignKind::None),
            header,
        }),
        Node::Frontmatter(n) => {
            let root = document.root();
            let line_ending = match &document.node(root).unwrap().kind {
                EventKind::Start(Tag::Document { line_ending, .. }) => line_ending.clone(),
                _ => unreachable!("the root is a document"),
            };
            document
                .set_kind(
                    root,
                    EventKind::Start(Tag::Document {
                        line_ending,
                        frontmatter: Some(value(&n.value)),
                    }),
                )
                .unwrap();
            document
                .set_syntax(
                    root,
                    Some(NodeSyntax::Frontmatter {
                        info: value(&n.info),
                        fence: CodeFence {
                            character: n.fence,
                            length: n.fence_length,
                            indent: 0,
                        },
                        closing_fence_length: n.closing_fence_length,
                    }),
                )
                .unwrap();
            return root;
        }
    };
    let id = document
        .append_child(parent, kind.clone())
        .unwrap_or_else(|e| {
            panic!(
                "{e:?} {kind:?} in {:?}",
                document.node(parent).unwrap().kind
            )
        });
    if let Some(syntax) = syntax {
        document.set_syntax(id, Some(syntax)).unwrap();
    }
    if let Node::Image(n) = node {
        document
            .append_child(id, EventKind::Text(value(&n.alt)))
            .unwrap();
    }
    if let Node::ImageReference(n) = node {
        document
            .append_child(id, EventKind::Text(value(&n.alt)))
            .unwrap();
    }
    if let Node::Table(n) = node {
        let head = document
            .append_child(id, EventKind::Start(Tag::TableHead))
            .unwrap();
        if let Some(row) = n.children.first() {
            append(document, head, row, definitions, false, true, &n.align);
        }
        if n.children.len() > 1 {
            let body = document
                .append_child(id, EventKind::Start(Tag::TableBody))
                .unwrap();
            for row in &n.children[1..] {
                append(document, body, row, definitions, false, false, &n.align);
            }
        }
    } else if let Some(children) = node.children() {
        let child_tight = if let Node::List(n) = node {
            !list_spread(n)
        } else {
            tight
        };
        // mdast lets phrasing stand among blocks, where it is written as a block of its own, so
        // each such child is put in a paragraph of its own, as phrasing and blocks cannot share a
        // parent.
        let blocks = matches!(
            node,
            Node::Blockquote(_) | Node::ListItem(_) | Node::FootnoteDefinition(_)
        );
        for (index, child) in children.iter().enumerate() {
            let parent = if blocks && phrasing(child) {
                document
                    .append_child(id, EventKind::Start(Tag::Paragraph { tight: child_tight }))
                    .unwrap()
            } else {
                id
            };
            let child_id = append(
                document,
                parent,
                child,
                definitions,
                child_tight,
                header,
                if matches!(node, Node::TableRow(_)) {
                    align.get(index..).unwrap_or(&[])
                } else {
                    align
                },
            );
            if index == 0 {
                if let Node::ListItem(n) = node {
                    if n.checked.is_some() && matches!(child, Node::Paragraph(_)) {
                        let before = document.children(child_id).next();
                        let space = document
                            .append_child(child_id, EventKind::Text(EventText::Literal(" ")))
                            .unwrap();
                        document.attach(child_id, space, before).unwrap();
                    }
                }
            }
        }
    }
    id
}

pub trait MarkdownFixture {
    fn markdown(
        &self,
        options: &base_markdown::MarkdownOptions,
    ) -> Result<String, base_markdown::message::Message>;
}
impl MarkdownFixture for Node {
    fn markdown(
        &self,
        options: &base_markdown::MarkdownOptions,
    ) -> Result<String, base_markdown::message::Message> {
        // A constructed tree is normalized before serializing, as the serializer requires.
        let mut document = self.document();
        document.normalize();
        base_markdown::to_markdown_with_options(&document, options)
    }
}
impl<S: AsRef<str>> MarkdownFixture for Document<S> {
    fn markdown(
        &self,
        options: &base_markdown::MarkdownOptions,
    ) -> Result<String, base_markdown::message::Message> {
        base_markdown::to_markdown_with_options(self, options)
    }
}
/// The fixture's document with every link given `form`, as a parser records it.
pub fn with_link_form(node: &Node, form: base_markdown::LinkForm) -> Document {
    let mut document = node.document();
    let links: Vec<_> = document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Link { .. })
            )
        })
        .collect();
    for link in links {
        document
            .set_syntax(link, Some(NodeSyntax::Link { form }))
            .unwrap();
    }
    document
}
pub fn to_markdown(node: &impl MarkdownFixture) -> Result<String, base_markdown::message::Message> {
    node.markdown(&Default::default())
}
pub fn to_markdown_with_options(
    node: &impl MarkdownFixture,
    options: &base_markdown::MarkdownOptions,
) -> Result<String, base_markdown::message::Message> {
    node.markdown(options)
}

/// The parsed document retains its actual source and nodes; comparisons project both
/// parser output and declarative fixture into the same semantic event contract.
#[derive(Debug)]
pub struct ParsedDocument(pub Document);
pub fn parse_fixture(
    source: &str,
    options: &ParseOptions,
) -> Result<ParsedDocument, base_markdown::message::Message> {
    Ok(ParsedDocument(
        Document::parse(source.to_owned(), options).expect("valid parser stream"),
    ))
}
impl PartialEq<Node> for ParsedDocument {
    fn eq(&self, other: &Node) -> bool {
        let actual = fixture_canonical(&self.0);
        let mut fixture = other.document();
        normalize_table_fixture(&mut fixture);
        let expected = fixture_canonical(&fixture);
        if actual != expected {
            eprintln!("actual semantics: {actual:#?}\nexpected semantics: {expected:#?}");
        }
        actual == expected
    }
}
impl PartialEq<ParsedDocument> for Node {
    fn eq(&self, other: &ParsedDocument) -> bool {
        other == self
    }
}

// The semantic event contract normalizes body rows to the header width;
// original table spelling remains accessible through source spans.
fn normalize_table_fixture(document: &mut Document) {
    let tables: Vec<_> = document
        .descendants(document.root())
        .filter(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::Start(Tag::Table)
            )
        })
        .collect();
    for table in tables {
        let rows: Vec<_> = document
            .descendants(table)
            .filter(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::TableRow)
                )
            })
            .collect();
        let Some(first) = rows.first() else {
            continue;
        };
        let align: Vec<_> = document
            .children(*first)
            .map(|id| match document.node(id).unwrap().kind {
                EventKind::Start(Tag::TableCell { alignment, .. }) => alignment,
                _ => AlignKind::None,
            })
            .collect();
        for row in rows.into_iter().skip(1) {
            let cells: Vec<_> = document.children(row).collect();
            for extra in cells.iter().skip(align.len()) {
                document.detach(*extra).unwrap();
            }
            for alignment in align.iter().skip(cells.len()) {
                document
                    .append_child(
                        row,
                        EventKind::Start(Tag::TableCell {
                            alignment: *alignment,
                            header: false,
                        }),
                    )
                    .unwrap();
            }
        }
    }
}

/// Retain authored fields covered by construct fixtures without requiring syntax
/// choices to be equal during semantic Markdown round trips.
fn fixture_canonical<S: AsRef<str>>(document: &Document<S>) -> (Vec<String>, Vec<String>) {
    let mut syntax = Vec::new();
    for id in document.descendants(document.root()) {
        let node = document.node(id).unwrap();
        match &node.kind {
            EventKind::CodeBlock { fence, .. } => syntax.push(format!("fence:{fence:?}")),
            EventKind::Start(
                Tag::Link {
                    reference: Some(reference),
                    ..
                }
                | Tag::Image {
                    reference: Some(reference),
                    ..
                },
            ) => syntax.push(format!(
                "reference:{:?}:{:?}",
                reference.kind,
                normalize(reference.label.resolve(document.source()).unwrap())
            )),
            EventKind::Start(Tag::FootnoteDefinition {
                label: Some(label), ..
            })
            | EventKind::FootnoteReference {
                label: Some(label), ..
            } => syntax.push(format!(
                "footnote-label:{:?}",
                normalize(label.resolve(document.source()).unwrap())
            )),
            _ => {}
        }
        if let Some(NodeSyntax::Frontmatter {
            info,
            fence,
            closing_fence_length,
        }) = document.syntax(id)
        {
            syntax.push(format!(
                "frontmatter:{:?}:{fence:?}:{closing_fence_length}",
                info.resolve(document.source()).unwrap()
            ));
        }
    }
    (canonical(document), syntax)
}

/// Source extents and text chunking are deliberately independent of AST semantics.
/// Focused document tests assert spans directly, while this projection keeps the
/// long-established construct fixtures useful without retaining a second AST.
pub fn canonical<S: AsRef<str>>(document: &Document<S>) -> Vec<String> {
    let source = document.source();
    let mut output: Vec<String> = Vec::new();
    let text = |v: &EventText<'_>| v.resolve(source).unwrap().to_string();
    let mut pending = String::new();
    let mut after_hard_break = false;
    // Whether the last events start a task item, and then its first paragraph.
    let mut after_task: Option<bool> = None;
    // The signature of the last node of the document so far, when it is a code block without a final line ending, with that line ending added. The serializer closes every fence, so a last block that the parser stores without one, from an unclosed fence or indented code at the end of the source, reads back with one.
    let mut last_code: Option<(usize, String)> = None;
    for event in document.events() {
        let event = event.unwrap();
        if !matches!(event.kind, EventKind::End(_)) {
            last_code = None;
        }
        // A task's checkbox is followed by a space or tab, or by a line ending that a soft break
        // holds; mdast keeps neither, and a written task item reads back with a space.
        let task_paragraph = after_task == Some(true);
        match event.kind {
            EventKind::Separator { .. } => continue,
            EventKind::SoftBreak(_) if after_hard_break => {
                after_hard_break = false;
                continue;
            }
            EventKind::SoftBreak(_) if task_paragraph => {
                after_task = None;
                pending.push(' ');
                continue;
            }
            EventKind::Text(v) | EventKind::SoftBreak(v) => {
                after_hard_break = false;
                after_task = None;
                pending.push_str(&text(&v));
                continue;
            }
            _ => {}
        }
        after_hard_break = matches!(event.kind, EventKind::HardBreak);
        after_task = match event.kind {
            EventKind::Start(Tag::Item {
                checked: Some(_), ..
            }) => Some(false),
            EventKind::Definition { .. } if after_task == Some(false) => Some(false),
            EventKind::Start(Tag::Paragraph { .. }) if after_task == Some(false) => Some(true),
            _ => None,
        };
        if !pending.is_empty() {
            output.push(format!("text:{pending:?}"));
            pending.clear();
        }
        let signature = match event.kind {
            EventKind::Start(Tag::Paragraph { .. }) => "Paragraph".into(),
            EventKind::Start(Tag::Item { checked, .. }) => match checked {
                Some(checked) => format!("Item:{checked}"),
                None => "Item".into(),
            },
            EventKind::Start(Tag::Document { frontmatter, .. }) => match frontmatter {
                Some(value) => format!("Document:{:?}", text(&value)),
                None => "Document".into(),
            },
            EventKind::Start(Tag::Link {
                destination, title, ..
            }) => format!(
                "Link:{:?}:{:?}",
                text(&destination),
                title.as_ref().map(text)
            ),
            EventKind::Start(Tag::Image {
                destination, title, ..
            }) => format!(
                "Image:{:?}:{:?}",
                text(&destination),
                title.as_ref().map(text)
            ),
            EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) => {
                format!("FootnoteDefinition:{:?}", normalize(&text(&identifier)))
            }
            EventKind::Start(tag) => format!("{tag:?}"),
            EventKind::Code { value, language } => {
                format!("Code:{:?}:{:?}", text(&value), language.as_ref().map(text))
            }
            EventKind::CodeBlock { value, info, .. } => {
                let body = text(&value);
                let info = info.as_ref().map(text);
                if !body.is_empty() && !body.ends_with('\n') {
                    last_code = Some((
                        output.len(),
                        format!("CodeBlock:{:?}:{info:?}", format!("{body}\n")),
                    ));
                }
                format!("CodeBlock:{body:?}:{info:?}")
            }
            EventKind::Html { value, .. } => format!("Html:{:?}", text(&value)),
            EventKind::Definition {
                label,
                destination,
                title,
            } => format!(
                "Definition:{:?}:{:?}:{:?}",
                normalize(&text(&label)),
                text(&destination),
                title.as_ref().map(text)
            ),
            EventKind::FootnoteReference { identifier, .. } => {
                format!("FootnoteReference:{:?}", normalize(&text(&identifier)))
            }
            kind => format!("{kind:?}"),
        };
        output.push(signature);
    }
    if !pending.is_empty() {
        output.push(format!("text:{pending:?}"));
    }
    if let Some((index, signature)) = last_code {
        output[index] = signature;
    }
    output
}

impl Frontmatter {
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

/// Set link or image `id` of `document` to the reference `kind` with `label`.
///
/// # Errors
/// Returns what [`Document::set_kind`] returns.
pub fn set_reference<S: AsRef<str>>(
    document: &mut Document<S>,
    id: NodeId,
    kind: ReferenceKind,
    label: EventText<'static>,
) -> Result<(), base_markdown::stream::StreamError> {
    let reference = Some(Box::new(Reference { kind, label }));
    let kind = match document.node(id).unwrap().kind.clone() {
        EventKind::Start(Tag::Link {
            destination, title, ..
        }) => Tag::Link {
            destination,
            title,
            reference,
        },
        EventKind::Start(Tag::Image {
            destination, title, ..
        }) => Tag::Image {
            destination,
            title,
            reference,
        },
        kind => panic!("not a link or image: {:?}", kind),
    };
    document.set_kind(id, EventKind::Start(kind))
}
