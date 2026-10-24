// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Declarative test inputs, converted to the public document AST before rendering.
//! These types describe fixtures only; there is no parser or renderer implementation here.
#![allow(dead_code)]
use core_markdown::stream::Text as EventText;
use core_markdown::unist::Position;
pub use core_markdown::{AlignKind, CodeFence, ReferenceKind};
use core_markdown::{Document, NodeId, NodeSyntax, ParseOptions};
use std::collections::BTreeMap;

use core_markdown::stream::{EventKind, Tag};
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
    Math(Math),
    Definition(Definition),
    Text(Text),
    Emphasis(Emphasis),
    Strong(Strong),
    InlineCode(InlineCode),
    InlineMath(InlineMath),
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
pub struct Math {
    pub value: String,
    pub position: Option<Position>,
    pub meta: Option<String>,
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
pub struct InlineMath {
    pub value: String,
    pub position: Option<Position>,
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
            for child in self.children().unwrap() {
                append(&mut document, root, child, &definitions, false, false, &[]);
            }
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
        Node::Code(n) => {
            let info = n
                .lang
                .as_ref()
                .map(|lang| {
                    format!(
                        "{lang}{}",
                        n.meta.as_ref().map_or(String::new(), |s| format!(" {s}"))
                    )
                })
                .unwrap_or_default();
            syntax = Some(NodeSyntax::Code { info: value(&info) });
            EventKind::CodeBlock {
                value: format!("{}\n", n.value).into(),
                language: title(&n.lang),
                fence: n.fence,
                math: false,
                unclosed: false,
            }
        }
        Node::Math(n) => {
            syntax = Some(NodeSyntax::Code {
                info: value(n.meta.as_deref().unwrap_or("")),
            });
            EventKind::CodeBlock {
                value: format!("{}\n", n.value).into(),
                language: None,
                fence: None,
                math: true,
                unclosed: false,
            }
        }
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
        Node::InlineMath(n) => EventKind::Math(value(&n.value)),
        Node::Break(_) => EventKind::HardBreak,
        Node::Link(n) => EventKind::Start(Tag::Link {
            destination: value(&n.url),
            title: title(&n.title),
        }),
        Node::Image(n) => EventKind::Start(Tag::Image {
            destination: value(&n.url),
            title: title(&n.title),
        }),
        Node::LinkReference(n) => {
            syntax = Some(NodeSyntax::Reference {
                kind: n.reference_kind,
                label: association(n.label.as_deref(), &n.identifier),
            });
            let resource = definitions.get(&normalize(&n.identifier));
            EventKind::Start(Tag::Link {
                destination: value(resource.map_or("", |r| r.0.as_str())),
                title: resource.and_then(|r| title(&r.1)),
            })
        }
        Node::ImageReference(n) => {
            syntax = Some(NodeSyntax::Reference {
                kind: n.reference_kind,
                label: association(n.label.as_deref(), &n.identifier),
            });
            let resource = definitions.get(&normalize(&n.identifier));
            EventKind::Start(Tag::Image {
                destination: value(resource.map_or("", |r| r.0.as_str())),
                title: resource.and_then(|r| title(&r.1)),
            })
        }
        Node::FootnoteDefinition(n) => {
            syntax = Some(NodeSyntax::Footnote {
                label: value(n.label.as_deref().unwrap_or(&n.identifier)),
            });
            EventKind::Start(Tag::FootnoteDefinition(value(&normalize(&n.identifier))))
        }
        Node::FootnoteReference(n) => {
            syntax = Some(NodeSyntax::Footnote {
                label: value(n.label.as_deref().unwrap_or(&n.identifier)),
            });
            EventKind::FootnoteReference(value(&normalize(&n.identifier)))
        }
        Node::Table(_) => EventKind::Start(Tag::Table),
        Node::TableRow(_) => EventKind::Start(Tag::TableRow),
        Node::TableCell(_) => EventKind::Start(Tag::TableCell {
            alignment: align.first().copied().unwrap_or(AlignKind::None),
            header,
        }),
        Node::Frontmatter(n) => {
            syntax = Some(NodeSyntax::Frontmatter {
                info: value(&n.info),
                fence: CodeFence {
                    character: n.fence,
                    length: n.fence_length,
                    indent: 0,
                },
                closing_fence_length: n.closing_fence_length,
            });
            EventKind::Opaque {
                value: value(&n.value),
                block: true,
            }
        }
    };
    let id = document.append_child(parent, kind).unwrap();
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
        for (index, child) in children.iter().enumerate() {
            let child_id = append(
                document,
                id,
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
                    if let Some(checked) = n.checked {
                        if matches!(child, Node::Paragraph(_)) {
                            let before = document.children(child_id).next();
                            let marker = document
                                .append_child(child_id, EventKind::TaskMarker(checked))
                                .unwrap();
                            document.attach(child_id, marker, before).unwrap();
                            let space = document
                                .append_child(child_id, EventKind::Text(EventText::Literal(" ")))
                                .unwrap();
                            document.attach(child_id, space, before).unwrap();
                        }
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
        options: &core_markdown::MarkdownOptions,
    ) -> Result<String, core_markdown::message::Message>;
}
impl MarkdownFixture for Node {
    fn markdown(
        &self,
        options: &core_markdown::MarkdownOptions,
    ) -> Result<String, core_markdown::message::Message> {
        core_markdown::to_markdown_with_options(&self.document(), options)
    }
}
impl<S: AsRef<str>> MarkdownFixture for Document<S> {
    fn markdown(
        &self,
        options: &core_markdown::MarkdownOptions,
    ) -> Result<String, core_markdown::message::Message> {
        core_markdown::to_markdown_with_options(self, options)
    }
}
pub fn to_markdown(node: &impl MarkdownFixture) -> Result<String, core_markdown::message::Message> {
    node.markdown(&Default::default())
}
pub fn to_markdown_with_options(
    node: &impl MarkdownFixture,
    options: &core_markdown::MarkdownOptions,
) -> Result<String, core_markdown::message::Message> {
    node.markdown(options)
}

/// The parsed document retains its actual source and nodes; comparisons project both
/// parser output and declarative fixture into the same semantic event contract.
#[derive(Debug)]
pub struct ParsedDocument(pub Document);
pub fn parse_fixture(
    source: &str,
    options: &ParseOptions,
) -> Result<ParsedDocument, core_markdown::message::Message> {
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
        if let EventKind::CodeBlock {
            fence, math: false, ..
        } = &node.kind
        {
            syntax.push(format!("fence:{fence:?}"));
        }
        match document.syntax(id) {
            Some(NodeSyntax::Reference { kind, label }) => syntax.push(format!(
                "reference:{kind:?}:{:?}",
                normalize(label.resolve(document.source()).unwrap())
            )),
            Some(NodeSyntax::Code { info }) => {
                let info = info.resolve(document.source()).unwrap();
                if !info.is_empty() {
                    syntax.push(format!("code-info:{info:?}"));
                }
            }
            Some(NodeSyntax::Frontmatter {
                info,
                fence,
                closing_fence_length,
            }) => syntax.push(format!(
                "frontmatter:{:?}:{fence:?}:{closing_fence_length}",
                info.resolve(document.source()).unwrap()
            )),
            Some(NodeSyntax::Footnote { label }) => syntax.push(format!(
                "footnote-label:{:?}",
                normalize(label.resolve(document.source()).unwrap())
            )),
            _ => {}
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
    let mut after_task_marker = false;
    for event in document.events() {
        let event = event.unwrap();
        match event.kind {
            EventKind::Separator { .. } => continue,
            EventKind::SoftBreak(_) if after_hard_break => {
                after_hard_break = false;
                continue;
            }
            EventKind::SoftBreak(_) if after_task_marker => {
                pending.push(' ');
                after_task_marker = false;
                continue;
            }
            EventKind::Text(v) | EventKind::SoftBreak(v) => {
                after_hard_break = false;
                after_task_marker = false;
                pending.push_str(&text(&v));
                continue;
            }
            _ => {}
        }
        after_hard_break = matches!(event.kind, EventKind::HardBreak);
        after_task_marker = matches!(event.kind, EventKind::TaskMarker(_));
        if !pending.is_empty() {
            output.push(format!("text:{pending:?}"));
            pending.clear();
        }
        let signature = match event.kind {
            EventKind::Start(Tag::Document(_)) => "Document".into(),
            EventKind::Start(Tag::Paragraph { .. }) => "Paragraph".into(),
            EventKind::Start(Tag::Item { .. }) => "Item".into(),
            EventKind::Start(Tag::Link { destination, title }) => format!(
                "Link:{:?}:{:?}",
                text(&destination),
                title.as_ref().map(text)
            ),
            EventKind::Start(Tag::Image { destination, title }) => format!(
                "Image:{:?}:{:?}",
                text(&destination),
                title.as_ref().map(text)
            ),
            EventKind::Start(Tag::FootnoteDefinition(label)) => {
                format!("FootnoteDefinition:{:?}", normalize(&text(&label)))
            }
            EventKind::Start(tag) => format!("{tag:?}"),
            EventKind::Code { value, language } => {
                format!("Code:{:?}:{:?}", text(&value), language.as_ref().map(text))
            }
            EventKind::CodeBlock {
                value,
                language,
                math,
                ..
            } => {
                let body = text(&value);
                let body = body
                    .strip_suffix("\r\n")
                    .or_else(|| body.strip_suffix(['\r', '\n']))
                    .unwrap_or(&body);
                format!(
                    "CodeBlock:{body:?}:{:?}:{math}",
                    language.as_ref().map(text)
                )
            }
            EventKind::Math(v) => format!("Math:{:?}", text(&v)),
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
            EventKind::Opaque { ref value, .. } => {
                let body = document
                    .descendants(document.root())
                    .find(|id| {
                        document.node(*id).unwrap().span == event.span
                            && matches!(document.node(*id).unwrap().kind, EventKind::Opaque { .. })
                    })
                    .and_then(|id| document.frontmatter_value(id));
                format!(
                    "Opaque:{:?}",
                    body.map_or_else(|| text(&value), str::to_owned)
                )
            }
            EventKind::FootnoteReference(v) => {
                format!("FootnoteReference:{:?}", normalize(&text(&v)))
            }
            kind => format!("{kind:?}"),
        };
        output.push(signature);
    }
    if !pending.is_empty() {
        output.push(format!("text:{pending:?}"));
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
