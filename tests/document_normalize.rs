// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod support;

use base_markdown::stream::Target;
use base_markdown::{
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, AstBuilder, CompileOptions, Document, NodeId, NodeSyntax, ParseOptions,
};
use std::borrow::Cow;
use std::time::Instant;
use support::fixture::canonical;
use support::linear::{assert_linear, timed};

fn event(
    kind: EventKind<'static>,
    span: std::ops::Range<usize>,
) -> Result<Event<'static>, StreamError> {
    Ok(Event { kind, span })
}

fn text(value: &str) -> Result<Event<'static>, StreamError> {
    event(EventKind::Text(Text::from(value.to_owned())), 0..0)
}

fn start(tag: Tag<'static>) -> Result<Event<'static>, StreamError> {
    event(EventKind::Start(tag), 0..0)
}

fn end(kind: TagKind) -> Result<Event<'static>, StreamError> {
    event(EventKind::End(kind), 0..0)
}

fn separator() -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Separator {
            value: Text::Literal("\n"),
            conditional: false,
        },
        0..0,
    )
}

/// The default options, with inline code languages, which the built trees have.
fn code_info() -> ParseOptions {
    ParseOptions {
        inline_code_info: true,
        ..ParseOptions::default()
    }
}

/// Wrap phrasing events in a document and paragraph, and build a document over `source`.
fn build_in(source: &str, phrasing: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![
        start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        start(Tag::Paragraph { tight: false }),
    ];
    events.extend(phrasing);
    events.push(end(TagKind::Paragraph));
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::with_options(code_info());
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    builder.finish(source.to_owned()).unwrap()
}

fn build(phrasing: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    build_in("", phrasing)
}

fn attention(
    tag: Tag<'static>,
    kind: TagKind,
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = vec![start(tag)];
    events.extend(children);
    events.push(end(kind));
    events
}

fn emphasis(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Emphasis, TagKind::Emphasis, children)
}

fn strong(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Strong, TagKind::Strong, children)
}

fn delete(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Delete, TagKind::Delete, children)
}

/// A compact description of the subtree at `id`.
fn describe<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> String {
    let node = document.node(id).unwrap();
    let children = || {
        document
            .children(id)
            .map(|child| describe(document, child))
            .collect::<Vec<_>>()
            .join(" ")
    };
    match &node.kind {
        EventKind::Text(value) => format!("{:?}", value.resolve(document.source()).unwrap()),
        EventKind::Separator { .. } => "separator".into(),
        EventKind::HardBreak => "break".into(),
        EventKind::Start(Tag::Emphasis) => format!("emphasis({})", children()),
        EventKind::Start(Tag::Strong) => format!("strong({})", children()),
        EventKind::Start(Tag::Delete) => format!("delete({})", children()),
        EventKind::Start(Tag::Paragraph { .. }) => format!("paragraph({})", children()),
        EventKind::Start(Tag::Item { block_end, checked }) => format!(
            "item {block_end}{}({})",
            if checked.is_some() { " task" } else { "" },
            children()
        ),
        EventKind::Start(Tag::Document { .. }) => children(),
        EventKind::Start(Tag::Heading(_)) => format!("heading({})", children()),
        EventKind::Start(Tag::TableCell { .. }) => format!("cell({})", children()),
        EventKind::SoftBreak(_) => "soft".into(),
        EventKind::Code { value, language } => format!(
            "code {:?}{}",
            value.resolve(document.source()).unwrap(),
            language.as_ref().map_or(String::new(), |language| format!(
                " {:?}",
                language.resolve(document.source()).unwrap()
            ))
        ),
        EventKind::Start(Tag::Link(target)) => format!(
            "link {:?}({})",
            match target.destination() {
                Some((destination, _)) => destination.resolve(document.source()).unwrap(),
                None => document
                    .destination(id)
                    .map_or("", |(destination, _)| destination),
            },
            children()
        ),
        kind => format!("{kind:?}({})", children()),
    }
}

fn normalized(mut document: Document<String>) -> String {
    document.normalize();
    assert!(document.is_normalized());
    describe(&document, document.root())
}

fn concat<T>(parts: impl IntoIterator<Item = Vec<T>>) -> Vec<T> {
    parts.into_iter().flatten().collect()
}

#[test]
fn removes_empty_text() {
    assert_eq!(
        normalized(build(vec![text(""), text(""), text("a"), text("")])),
        r#"paragraph("a")"#
    );
    assert_eq!(
        normalized(build(concat([emphasis(vec![text("")]), vec![text("")]]))),
        ""
    );
}

#[test]
fn removes_empty_attention() {
    // Empty attention between text, which then joins.
    assert_eq!(
        normalized(build(concat([
            vec![text("a")],
            emphasis(vec![]),
            vec![text("b")],
        ]))),
        r#"paragraph("ab")"#
    );
    // Attention that is empty once its empty text is removed.
    assert_eq!(
        normalized(build(concat([
            vec![text("a")],
            strong(vec![text("")]),
            delete(vec![text("")]),
            vec![text("b")],
        ]))),
        r#"paragraph("ab")"#
    );
    // Empty attention between attention of the same kind, which then merges.
    assert_eq!(
        normalized(build(concat([
            emphasis(vec![text("a")]),
            strong(vec![]),
            emphasis(vec![text("b")]),
        ]))),
        r#"paragraph(emphasis("ab"))"#
    );
    assert_eq!(
        normalized(build(concat([
            delete(vec![text("a")]),
            emphasis(vec![text("")]),
            vec![text("")],
            delete(vec![text("b")]),
        ]))),
        r#"paragraph(delete("ab"))"#
    );
    // Empty attention of the same kind as its neighbours.
    assert_eq!(
        normalized(build(concat([
            strong(vec![text("a")]),
            strong(vec![]),
            strong(vec![text("b")]),
        ]))),
        r#"paragraph(strong("ab"))"#
    );
}

#[test]
fn removes_nested_empty_attention() {
    assert_eq!(
        normalized(build(concat([
            vec![text("a")],
            strong(concat([
                emphasis(delete(vec![text("")])),
                emphasis(vec![]),
                vec![text("")],
            ])),
            vec![text("b")],
        ]))),
        r#"paragraph("ab")"#
    );
    // Empty attention at the seam of merged attention, so the children on either side of it meet.
    assert_eq!(
        normalized(build(concat([
            emphasis(concat([
                vec![text("a")],
                strong(vec![text("b")]),
                delete(vec![])
            ])),
            emphasis(concat([
                strong(emphasis(vec![text("")])),
                strong(vec![text("c")]),
                vec![text("d")],
            ])),
        ]))),
        r#"paragraph(emphasis("a" strong("bc") "d"))"#
    );
    // Attention that holds other content stays.
    assert_eq!(
        normalized(build(concat([
            emphasis(vec![event(
                EventKind::Code {
                    value: Text::Literal("c"),
                    language: None,
                },
                0..0
            )]),
            strong(emphasis(vec![text("a")])),
        ]))),
        r#"paragraph(emphasis(code "c") strong(emphasis("a")))"#
    );
}

#[test]
fn keeps_the_root_of_a_detached_subtree() {
    let mut document = build(emphasis(vec![text(""), text("")]));
    let paragraph = document.children(document.root()).next().unwrap();
    let attention = document.children(paragraph).next().unwrap();
    document.detach(attention).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(describe(&document, attention), "emphasis()");
    assert_eq!(describe(&document, document.root()), "");
}

#[test]
fn unwraps_strikethrough_in_strikethrough() {
    assert_eq!(
        normalized(build(delete(concat([
            vec![text("a")],
            delete(vec![text("b")]),
            vec![text("c")],
        ])))),
        r#"paragraph(delete("abc"))"#
    );
    // At depth 2, through emphasis.
    assert_eq!(
        normalized(build(delete(concat([
            vec![text("a")],
            emphasis(concat([
                vec![text("b")],
                delete(vec![text("c")]),
                vec![text("d")]
            ])),
            vec![text("e")],
        ])))),
        r#"paragraph(delete("a" emphasis("bcd") "e"))"#
    );
    // Through a link, and several levels at once.
    assert_eq!(
        normalized(build(delete(link(delete(delete(delete(vec![text("a")]))))))),
        r#"paragraph(delete(link "http://a.b"("a")))"#
    );
    // Adjacent strikethrough inside strikethrough is unwrapped rather than merged.
    assert_eq!(
        normalized(build(delete(concat([
            delete(vec![text("a")]),
            delete(vec![text("b")]),
        ])))),
        r#"paragraph(delete("ab"))"#
    );
    // Strikethrough next to strikethrough with nested strikethrough is merged, and then unwrapped inside.
    assert_eq!(
        normalized(build(concat([
            delete(vec![text("a")]),
            delete(delete(vec![text("b")])),
        ]))),
        r#"paragraph(delete("ab"))"#
    );
    // Strikethrough outside strikethrough stays.
    assert_eq!(
        normalized(build(emphasis(delete(emphasis(vec![text("a")]))))),
        r#"paragraph(emphasis(delete(emphasis("a"))))"#
    );
}

#[test]
fn unwrapped_strikethrough_joins_its_neighbours() {
    // The first and last children of the unwrapped strikethrough meet the children around it.
    assert_eq!(
        normalized(build(delete(concat([
            emphasis(vec![text("a")]),
            delete(concat([
                emphasis(vec![text("b")]),
                strong(vec![text("c")]),
                vec![text("d")],
            ])),
            vec![text("e")],
            strong(vec![text("f")]),
        ])))),
        r#"paragraph(delete(emphasis("ab") strong("c") "de" strong("f")))"#
    );
    // Its first child is strikethrough too, and an empty one is removed.
    assert_eq!(
        normalized(build(delete(concat([
            vec![text("a")],
            delete(concat([delete(vec![text("b")]), vec![text("c")]])),
            delete(vec![text("")]),
            emphasis(vec![text("d")]),
            delete(emphasis(vec![text("e")])),
        ])))),
        r#"paragraph(delete("abc" emphasis("de")))"#
    );
}

#[test]
fn unwraps_strikethrough_in_detached_subtrees() {
    let mut document = build(delete(concat([vec![text("a")], delete(vec![text("b")])])));
    let paragraph = document.children(document.root()).next().unwrap();
    let attention = document.children(paragraph).next().unwrap();
    document.detach(attention).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(describe(&document, attention), r#"delete("ab")"#);
}

#[test]
fn unwraps_links_in_links_that_are_not_autolinks() {
    // A link in a link's text is written as an autolink or not at all, so any other is unwrapped, at any depth, and an empty one removed.
    assert_eq!(
        normalized(build(link(concat([
            vec![text("a")],
            link(vec![text("b")]),
            emphasis(link(vec![text("c")])),
            link(vec![]),
            vec![text("d")],
        ])))),
        r#"paragraph(link "http://a.b"("ab" emphasis("c") "d"))"#
    );
    assert!(!build(link(link(vec![text("x")]))).is_normalized());
    // A parsed autolink in a link stays, and reads back.
    for source in ["[<http://a.b>](m)\n", "[x <a@b.c> y](m)\n"] {
        let mut document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        document.normalize();
        assert_eq!(to_markdown(&document).unwrap(), source);
    }
    // Text that no longer reads back as its autolink is unwrapped, and so is a title, as an autolink cannot hold one.
    let mut document = Document::parse("[<http://a.b>](m)", &ParseOptions::gfm()).unwrap();
    let paragraph = document.children(document.root()).next().unwrap();
    let outer = document.children(paragraph).next().unwrap();
    let inner = document.children(outer).next().unwrap();
    let text_id = document.children(inner).next().unwrap();
    document.text_mut(text_id).unwrap().push('x');
    assert!(!document.is_normalized());
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "[http://a.bx](m)\n");
    let mut document = Document::parse("[<http://a.b>](m)", &ParseOptions::gfm()).unwrap();
    let paragraph = document.children(document.root()).next().unwrap();
    let outer = document.children(paragraph).next().unwrap();
    let inner = document.children(outer).next().unwrap();
    document
        .set_kind(
            inner,
            EventKind::Start(Tag::Link(Target::Resource {
                destination: Text::Literal("http://a.b"),
                title: Some(Text::Literal("t")),
            })),
        )
        .unwrap();
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "[http://a.b](m)\n");
    // An inner link whose recorded form is cleared is no longer an autolink, and one recorded as
    // an autolink is kept.
    let mut document = Document::parse("[<http://a.b>](m)", &ParseOptions::gfm()).unwrap();
    let paragraph = document.children(document.root()).next().unwrap();
    let outer = document.children(paragraph).next().unwrap();
    let inner = document.children(outer).next().unwrap();
    document.set_syntax(inner, None).unwrap();
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "[http://a.b](m)\n");
    let mut document = build(link(link(vec![text("http://a.b")])));
    let paragraph = document.children(document.root()).next().unwrap();
    let outer = document.children(paragraph).next().unwrap();
    let inner = document.children(outer).next().unwrap();
    document
        .set_syntax(
            inner,
            Some(NodeSyntax::Link {
                form: base_markdown::LinkForm::Angle,
            }),
        )
        .unwrap();
    assert!(document.is_normalized());
    assert_eq!(
        to_markdown(&document).unwrap(),
        "[<http://a.b>](http://a.b)\n"
    );
    // In a table cell, an autolink with a pipe is written as a resource, so it is unwrapped too.
    let mut document =
        Document::parse("|[<http://a.b/\\|>](m)|\n|-|\n", &ParseOptions::gfm()).unwrap();
    document.normalize();
    assert!(document.is_normalized());
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    // A link in a detached link is unwrapped too, though the detached link stays.
    let mut document = build(link(link(vec![text("b")])));
    let paragraph = document.children(document.root()).next().unwrap();
    let outer = document.children(paragraph).next().unwrap();
    document.detach(outer).unwrap();
    document.normalize();
    assert_eq!(describe(&document, outer), r#"link "http://a.b"("b")"#);
}

#[test]
fn nested_strikethrough_from_markdown_reads_back() {
    for (source, expected, tree) in [
        ("~a~~b~~c~", "~~abc~~\n", r#"paragraph(delete("abc"))"#),
        (
            "~5a~~del~~(*foo~",
            "~~5adel(\\*foo~~\n",
            r#"paragraph(delete("5adel(*foo"))"#,
        ),
    ] {
        let mut document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        assert!(!document.is_normalized());
        document.normalize();
        assert_eq!(describe(&document, document.root()), tree, "{source:?}");
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
        reparsed.normalize();
        // The serialized Markdown ends with a line ending, which is a separator after the paragraph.
        assert_eq!(
            describe(&reparsed, reparsed.root()),
            format!("{tree} separator"),
            "{markdown:?}"
        );
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{markdown:?}");
    }
}

#[test]
fn joins_adjacent_text() {
    assert_eq!(
        normalized(build(vec![text("a"), text(""), text("b"), text("c")])),
        r#"paragraph("abc")"#
    );
}

#[test]
fn joins_contiguous_source_text_without_copying() {
    let source = "abc";
    let mut document = build_in(
        source,
        vec![
            event(EventKind::Text(Text::Source(0..1)), 0..1),
            event(EventKind::Text(Text::Source(1..3)), 1..3),
        ],
    );
    document.normalize();
    let paragraph = document.children(document.root()).next().unwrap();
    let joined = document.children(paragraph).next().unwrap();
    let node = document.node(joined).unwrap();
    assert!(matches!(node.kind, EventKind::Text(Text::Source(ref range)) if *range == (0..3)));
    assert_eq!(node.span, 0..3);
    assert_eq!(document.children(paragraph).count(), 1);
}

#[test]
fn parsed_text_is_one_node_per_source_slice() {
    // Markers that stay literal do not split text.
    let document = Document::parse("a.b_c:d*e~f@g <h", &ParseOptions::gfm()).unwrap();
    assert!(document.is_normalized());
    assert_eq!(
        describe(&document, document.root()),
        r#"paragraph("a.b_c:d*e~f@g <h")"#
    );
}

#[test]
fn joins_text_split_by_parsing() {
    // An escape splits parsed text, because its value is not one slice of the source.
    let mut document = Document::parse("a\\*b&amp;c", &ParseOptions::default()).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(
        describe(&document, document.root()),
        r#"paragraph("a*b&c")"#
    );
    let paragraph = document.children(document.root()).next().unwrap();
    let joined = document.children(paragraph).next().unwrap();
    assert_eq!(document.node(joined).unwrap().span, 0..10);
}

#[test]
fn joined_span_keeps_the_nonempty_span() {
    let source = "abcd";
    let spans = |left: std::ops::Range<usize>, right: std::ops::Range<usize>| {
        let mut document = build_in(
            source,
            vec![
                event(EventKind::Text(Text::Literal("x")), left),
                event(EventKind::Text(Text::Literal("y")), right),
            ],
        );
        document.normalize();
        let paragraph = document.children(document.root()).next().unwrap();
        let joined = document.children(paragraph).next().unwrap();
        assert_eq!(document.text_content(paragraph), "xy");
        document.node(joined).unwrap().span.clone()
    };
    assert_eq!(spans(0..0, 1..3), 1..3);
    assert_eq!(spans(1..2, 0..0), 1..2);
    assert_eq!(spans(2..4, 0..1), 0..4);
}

#[test]
fn merges_adjacent_attention_of_the_same_kind() {
    assert_eq!(
        normalized(build(concat([
            emphasis(vec![text("a")]),
            emphasis(vec![text("b")]),
            emphasis(vec![text("c")]),
        ]))),
        r#"paragraph(emphasis("abc"))"#
    );
    assert_eq!(
        normalized(build(concat([
            strong(vec![text("a")]),
            vec![text("")],
            strong(vec![text("b")]),
        ]))),
        r#"paragraph(strong("ab"))"#
    );
    assert_eq!(
        normalized(build(concat([
            delete(vec![]),
            delete(vec![text("b")]),
            delete(vec![])
        ]))),
        r#"paragraph(delete("b"))"#
    );
    // Different kinds stay apart.
    assert_eq!(
        normalized(build(concat([
            emphasis(vec![text("a")]),
            strong(vec![text("b")]),
            emphasis(vec![text("c")]),
        ]))),
        r#"paragraph(emphasis("a") strong("b") emphasis("c"))"#
    );
}

#[test]
fn merging_applies_the_rules_again_at_the_join() {
    assert_eq!(
        normalized(build(concat([
            strong(concat([
                vec![text("a")],
                emphasis(vec![text("b")]),
                vec![text("")]
            ])),
            strong(concat([emphasis(vec![text("c")]), vec![text("d")]])),
        ]))),
        r#"paragraph(strong("a" emphasis("bc") "d"))"#
    );
}

#[test]
fn rejects_separators_in_phrasing() {
    let mut builder = AstBuilder::new();
    let events = vec![
        start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        start(Tag::Paragraph { tight: false }),
        text("a"),
        separator(),
    ];
    assert!(matches!(
        builder.consume(&mut events.into_iter(), usize::MAX),
        Err(StreamError::InvalidNesting)
    ));
    let mut document = build(concat([emphasis(vec![text("a")])]));
    let paragraph = document.children(document.root()).next().unwrap();
    let emphasis = document.children(paragraph).next().unwrap();
    for parent in [paragraph, emphasis] {
        assert!(matches!(
            document.append_child(
                parent,
                EventKind::Separator {
                    value: Text::Literal("\n"),
                    conditional: false,
                },
            ),
            Err(StreamError::InvalidNesting)
        ));
    }
}

#[test]
fn keeps_separators_between_blocks() {
    let mut document = Document::parse(
        "a\\

b

> c
",
        &ParseOptions::default(),
    )
    .unwrap();
    let events = |document: &Document<&str>| {
        document
            .events()
            .map(|event| format!("{:?}", event.unwrap()))
            .collect::<Vec<_>>()
    };
    let before = events(&document);
    assert!(before.iter().any(|event| event.contains("Separator")));
    document.normalize();
    assert_eq!(events(&document), before);
    // Text cannot sit beside a separator directly in a quote.
    let mut document = build(vec![]);
    let quote = document
        .append_child(document.root(), EventKind::Start(Tag::BlockQuote))
        .unwrap();
    assert!(matches!(
        document.append_child(quote, EventKind::Text(Text::Literal("a"))),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn normalizes_detached_subtrees() {
    let mut document = build(vec![text("a"), text("b")]);
    let paragraph = document.children(document.root()).next().unwrap();
    document.detach(paragraph).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(describe(&document, paragraph), r#"paragraph("ab")"#);
}

#[test]
fn refreshes_the_item_it_changes() {
    let events = vec![
        start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        start(Tag::List {
            start: None,
            tight: true,
        }),
        start(Tag::Item {
            block_end: true,
            checked: None,
        }),
        start(Tag::Paragraph { tight: true }),
        text("a"),
        end(TagKind::Paragraph),
        start(Tag::Paragraph { tight: true }),
        end(TagKind::Paragraph),
        end(TagKind::Item),
        end(TagKind::List),
        end(TagKind::Document),
    ];
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    let mut document = builder.finish(String::new()).unwrap();
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    assert!(matches!(
        document.node(item).unwrap().kind,
        EventKind::Start(Tag::Item {
            block_end: true,
            checked: None
        })
    ));
    document.normalize();
    assert!(matches!(
        document.node(item).unwrap().kind,
        EventKind::Start(Tag::Item {
            block_end: false,
            checked: None
        })
    ));
}

#[test]
fn is_normalized_finds_each_shape() {
    assert!(build(vec![text("a")]).is_normalized());
    assert!(build(concat([emphasis(vec![text("a")]), strong(vec![text("b")])])).is_normalized());
    assert!(!build(vec![text("")]).is_normalized());
    assert!(!build(vec![text("a"), text("b")]).is_normalized());
    assert!(!build(concat([delete(vec![text("a")]), delete(vec![text("b")])])).is_normalized());
    assert!(!build(emphasis(vec![text("a"), text("")])).is_normalized());
    assert!(!build(concat([vec![text("a")], strong(vec![])])).is_normalized());
    assert!(!build(delete(emphasis(delete(vec![text("a")])))).is_normalized());
    assert!(build(delete(emphasis(vec![text("a")]))).is_normalized());
    assert!(build(concat([
        delete(vec![text("a")]),
        emphasis(delete(vec![text("b")]))
    ]))
    .is_normalized());
}

#[test]
fn normalizing_a_normalized_document_changes_nothing() {
    let source =
        "# Title\n\nSome *emphasis*, **strong** and ~~struck~~ text.\n\n- [x] item\n- other\n";
    let mut document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    assert!(document.is_normalized());
    let events = |document: &Document<&str>| {
        document
            .events()
            .map(|event| format!("{:?}", event.unwrap()))
            .collect::<Vec<_>>()
    };
    let before = events(&document);
    document.normalize();
    assert_eq!(events(&document), before);
}

struct Random(u64);

impl Random {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
}

fn random_phrasing(random: &mut Random, depth: u32) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = Vec::new();
    for _ in 0..random.below(6) {
        let choice = random.below(if depth == 0 { 7 } else { 11 });
        match choice {
            0 => events.push(text("")),
            1 => events.push(text(
                ["a", "*", " ", "_", "\u{a0}", "\t", "\n", " a\t", "b\n "]
                    [random.below(9) as usize],
            )),
            2 => events.push(text("~")),
            3 => events.push(hard()),
            4 => events.push(soft()),
            5 => events.push(code(
                ["a", " "][random.below(2) as usize],
                [None, Some("x")][random.below(2) as usize],
            )),
            6 => events.push(code("m", None)),
            7 => events.extend(emphasis(random_phrasing(random, depth - 1))),
            8 => events.extend(strong(random_phrasing(random, depth - 1))),
            9 => events.extend(link(random_phrasing(random, depth - 1))),
            _ => events.extend(delete(random_phrasing(random, depth - 1))),
        }
    }
    events
}

/// A document of random blocks, each holding random phrasing: paragraphs, headings, table cells, task items and block quotes, with separators between some of them.
fn random_document(random: &mut Random) -> Document<String> {
    let mut events = Vec::new();
    for _ in 0..1 + random.below(3) {
        let phrasing = random_phrasing(random, 4);
        let block = match random.below(6) {
            0 => heading(phrasing),
            1 => table(without_breaks(phrasing)),
            2 => attention(
                Tag::List {
                    start: None,
                    tight: true,
                },
                TagKind::List,
                attention(
                    Tag::Item {
                        block_end: false,
                        checked: Some(false),
                    },
                    TagKind::Item,
                    paragraph(phrasing),
                ),
            ),
            3 => attention(Tag::BlockQuote, TagKind::BlockQuote, paragraph(phrasing)),
            _ => paragraph(phrasing),
        };
        events.extend(block);
        if random.below(2) == 0 {
            events.push(separator());
        }
    }
    build_blocks(events)
}

/// The Markdown for `result`, or `None` where it has a node with no spelling that reads back,
/// which is an answer too.
fn written(result: Result<String, base_markdown::message::Message>) -> Option<String> {
    match result {
        Ok(markdown) => Some(markdown),
        Err(error) if error.rule_id == "unwritable-node" => None,
        Err(error) => panic!("{}", error),
    }
}

#[test]
fn normalize_makes_any_tree_normalized_and_is_idempotent() {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let mut unnormalized = 0;
    // 500 trees reach what 20,000 did that no other test reaches, in a fraction of the time.
    for _ in 0..500 {
        let mut document = random_document(&mut random);
        let normal = document.is_normalized();
        unnormalized += usize::from(!normal);
        // Serializing a tree that is not normalized may be wrong, but it must not panic.
        written(to_markdown(&document));
        let before = describe(&document, document.root());
        document.normalize();
        assert!(document.is_normalized(), "{}", before);
        let once = describe(&document, document.root());
        // A tree is reported as normalized exactly when normalizing changes nothing.
        assert_eq!(normal, once == before, "{before}");
        let markdown = written(to_markdown(&document));
        document.normalize();
        assert_eq!(describe(&document, document.root()), once);
        assert_eq!(written(to_markdown(&document)), markdown);
    }
    assert!(unnormalized > 250, "{}", unnormalized);
}

// The serializer does nothing for trees that are not normalized beyond not panicking: write deeper random trees, unnormalized, across options.
#[test]
fn serializing_unnormalized_trees_does_not_panic() {
    let mut random = Random(0x2545_F491_4F6C_DD1D);
    let options = [
        base_markdown::MarkdownOptions::default(),
        base_markdown::MarkdownOptions {
            emphasis: '_',
            strong: '_',
            ..base_markdown::MarkdownOptions::default()
        },
    ];
    // As above, 500 trees cover what 20,000 did that no other test covers.
    for _ in 0..500 {
        let mut events = Vec::new();
        for _ in 0..1 + random.below(3) {
            let phrasing = random_phrasing(&mut random, 6);
            events.extend(match random.below(3) {
                0 => heading(phrasing),
                1 => table(without_breaks(phrasing)),
                _ => paragraph(phrasing),
            });
        }
        let document = build_blocks(events);
        for options in &options {
            written(base_markdown::to_markdown_with_options(&document, options));
        }
    }
}

/// Time normalizing the document from `build` and serializing the result, with `size` against eight times as many units. Serializing a small tree before normalizing checks that it does not panic, though it may return `unwritable-node`, as empty attention next to attention reads back as text (contract 5.3); that need not be linear, so it is not timed.
fn assert_normalize_linear(label: &str, size: usize, build: impl Fn(usize) -> Document<String>) {
    if let Err(error) = to_markdown(&build(size)) {
        assert_eq!(error.rule_id, "unwritable-node");
    }
    assert_linear(label, size, |size| {
        let mut document = build(size);
        timed(|| {
            document.normalize();
            assert!(document.is_normalized());
            to_markdown(&document).unwrap();
        })
    });
}

#[test]
fn joining_text_is_linear() {
    // Joining copies each text once. Copying what was joined so far on each join is cheap at small sizes, so this one is larger. The spaces that hard breaks in a table cell become are joined the same way.
    assert_normalize_linear("text", 1 << 14, |size| {
        build((0..size).map(|_| text("a")).collect())
    });
}

#[test]
fn joining_code_is_linear() {
    // Code joins like text, but takes its own arm of the join.
    assert_normalize_linear("code", 1 << 10, |size| {
        build((0..size).map(|_| code("a", None)).collect())
    });
}

#[test]
fn merging_attention_is_linear() {
    assert_normalize_linear("attention", 1 << 11, |size| {
        build(concat(
            (0..size).map(|_| emphasis(vec![text("a"), text("b")])),
        ))
    });
}

#[test]
fn merging_nested_attention_is_linear() {
    // Each merge exposes another pair to merge one level down. The serializer recurses per level, so only normalizing is timed.
    let nested = |depth: usize| {
        let mut events = Vec::new();
        for _ in 0..depth {
            events.push(start(Tag::Strong));
            events.push(start(Tag::Emphasis));
        }
        events.push(text("a"));
        for _ in 0..depth {
            events.push(end(TagKind::Emphasis));
            events.push(end(TagKind::Strong));
        }
        events
    };
    assert_linear("nested attention", 1 << 11, |size| {
        let mut document = build(concat([nested(size / 4), nested(size / 4)]));
        let started = Instant::now();
        document.normalize();
        let elapsed = started.elapsed();
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), size / 2 + 3);
        elapsed
    });
}

#[test]
fn removing_empty_attention_is_linear() {
    assert_normalize_linear("empty attention", 1 << 11, |size| {
        build(concat([
            vec![text("a")],
            concat((0..size / 4).map(|_| {
                concat([
                    emphasis(vec![text("")]),
                    strong(vec![]),
                    delete(emphasis(vec![])),
                ])
            })),
            vec![text("b")],
        ]))
    });
}

#[test]
fn removing_empty_children_between_kept_ones_is_linear() {
    // Removing continues from the child kept before what was removed, not from the first child.
    assert_normalize_linear("empty text after text", 1 << 10, |size| {
        build(concat((0..size).map(|_| vec![text("a"), text("")])))
    });
    assert_normalize_linear("empty attention after text", 1 << 10, |size| {
        build(concat(
            (0..size).map(|_| concat([vec![text("a")], strong(vec![])])),
        ))
    });
}

#[test]
fn removing_nested_empty_attention_is_linear() {
    // The serializer recurses per level, so only normalizing is timed.
    assert_linear("nested empty attention", 1 << 11, |size| {
        let mut events = vec![text("a")];
        events.extend((0..size).map(|_| start(Tag::Emphasis)));
        events.push(text(""));
        events.extend((0..size).map(|_| end(TagKind::Emphasis)));
        events.push(text("b"));
        let mut document = build(events);
        let started = Instant::now();
        document.normalize();
        let elapsed = started.elapsed();
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), 3);
        elapsed
    });
}

#[test]
fn unwrapping_nested_strikethrough_is_linear() {
    // Whether a node is inside strikethrough is counted on the way down, not found by searching its ancestors. Strikethrough alternating with emphasis keeps the emphasis, so it stays deep, and only normalizing is timed, as the serializer recurses per level.
    assert_linear("strikethrough alternating with emphasis", 1 << 11, |size| {
        let mut events = Vec::new();
        for _ in 0..size / 2 {
            events.push(start(Tag::Delete));
            events.push(start(Tag::Emphasis));
        }
        events.push(text("a"));
        for _ in 0..size / 2 {
            events.push(end(TagKind::Emphasis));
            events.push(end(TagKind::Delete));
        }
        let mut document = build(events);
        let started = Instant::now();
        document.normalize();
        let elapsed = started.elapsed();
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), size / 2 + 4);
        elapsed
    });
}

#[test]
fn appending_to_an_item_is_linear() {
    // Appending finds the parent's last child from the child appended before, rather than walking its children, and refreshes an item's ending from the appended child alone. An item takes both paths.
    assert_linear("appending to an item", 1 << 11, |size| {
        let mut document = Document::parse("- a\n", &ParseOptions::default()).unwrap();
        let list = document.children(document.root()).next().unwrap();
        let item = document.children(list).next().unwrap();
        let started = Instant::now();
        for _ in 0..size {
            let paragraph = document
                .append_child(item, EventKind::Start(Tag::Paragraph { tight: true }))
                .unwrap();
            document
                .append_child(paragraph, EventKind::Text(Text::Literal("b")))
                .unwrap();
        }
        let elapsed = started.elapsed();
        assert_eq!(document.children(item).count(), size + 1);
        elapsed
    });
}

#[test]
fn appending_to_an_item_refreshes_how_it_ends() {
    let block_end =
        |document: &Document<&str>, item: NodeId| match document.node(item).unwrap().kind {
            EventKind::Start(Tag::Item { block_end, .. }) => block_end,
            _ => unreachable!("the node is an item"),
        };
    let mut document = Document::parse("- a\n", &ParseOptions::default()).unwrap();
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    assert!(!block_end(&document, item));
    document.append_child(item, EventKind::Rule).unwrap();
    assert!(block_end(&document, item));
    // A definition writes nothing after the item's last line, so the item still ends as before.
    document
        .append_child(
            item,
            EventKind::Definition {
                label: Text::Literal("x"),
                destination: Text::Literal("y"),
                title: None,
            },
        )
        .unwrap();
    assert!(block_end(&document, item));
    let paragraph = document
        .append_child(item, EventKind::Start(Tag::Paragraph { tight: true }))
        .unwrap();
    assert!(!block_end(&document, item));
    document
        .append_child(paragraph, EventKind::Text(Text::Literal("b")))
        .unwrap();
    assert!(!block_end(&document, item));
    // Moving the paragraph in place refreshes the item from all of its children, with the same result.
    document.attach(item, paragraph, None).unwrap();
    assert!(!block_end(&document, item));
}

#[test]
fn appending_after_an_edit_elsewhere_finds_the_last_child() {
    let mut document = build(vec![]);
    let paragraph = document.children(document.root()).next().unwrap();
    let first = document
        .append_child(paragraph, EventKind::Start(Tag::Emphasis))
        .unwrap();
    document
        .append_child(first, EventKind::Text(Text::Literal("a")))
        .unwrap();
    // The last appended node is in another parent, so the paragraph's last child is found by walking.
    document
        .append_child(paragraph, EventKind::Text(Text::Literal("b")))
        .unwrap();
    // The last appended node is no longer last once a node is attached after it.
    let moved = document
        .append_child(first, EventKind::Text(Text::Literal("d")))
        .unwrap();
    let last = document
        .append_child(paragraph, EventKind::Text(Text::Literal("c")))
        .unwrap();
    document.attach(paragraph, moved, None).unwrap();
    assert!(document.node(last).unwrap().next_sibling().is_some());
    document
        .append_child(paragraph, EventKind::Text(Text::Literal("e")))
        .unwrap();
    assert_eq!(
        describe(&document, document.root()),
        r#"paragraph(emphasis("a") "b" "c" "d" "e")"#
    );
}

fn link(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(
        Tag::Link(Target::Resource {
            destination: Text::Literal("http://a.b"),
            title: None,
        }),
        TagKind::Link,
        children,
    )
}

// The serializer no longer skips empty text, reads across text nodes, or merges attention. Each shape it used to handle serializes, once normalized, exactly as the normal tree written directly.
#[test]
fn normalizing_replaces_serializer_coping() {
    let cases = [
        // Empty text between attention, which the serializer skipped when looking ahead.
        (
            concat([
                emphasis(vec![text("a ")]),
                vec![text(""), text("")],
                emphasis(vec![text("b")]),
            ]),
            emphasis(vec![text("a b")]),
        ),
        // Empty text at the edge of attention, which the serializer skipped to find the edge character.
        (
            concat([
                vec![text("a")],
                emphasis(vec![text(""), text("*b"), text("")]),
                vec![text("c")],
            ]),
            concat([vec![text("a")], emphasis(vec![text("*b")]), vec![text("c")]]),
        ),
        // Adjacent attention of the same kind, which the serializer wrote as one node.
        (
            concat([strong(vec![text("a")]), strong(vec![text("b")])]),
            strong(vec![text("ab")]),
        ),
        // Text split before a tilde run, whose start the serializer carried over to the next text.
        (
            concat([delete(vec![text("a")]), vec![text("~"), text("~b")]]),
            concat([delete(vec![text("a")]), vec![text("~~b")]]),
        ),
        // Spaces after a link split over several texts, which the serializer read in turn.
        (
            concat([
                link(vec![text("http://a.b")]),
                vec![text(" "), text(""), text(" x")],
            ]),
            concat([link(vec![text("http://a.b")]), vec![text("  x")]]),
        ),
    ];
    for (unnormalized, normal) in cases {
        let mut document = build(unnormalized);
        let normal = build(normal);
        assert!(!document.is_normalized());
        assert!(normal.is_normalized());
        document.normalize();
        assert_eq!(
            describe(&document, document.root()),
            describe(&normal, normal.root())
        );
        let markdown = to_markdown(&normal).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), markdown);
        // The normal tree reads back from its Markdown.
        let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
        reparsed.normalize();
        let paragraph = reparsed.children(reparsed.root()).next().unwrap();
        let expected = normal.children(normal.root()).next().unwrap();
        assert_eq!(
            describe(&reparsed, paragraph),
            describe(&normal, expected),
            "{markdown:?}"
        );
    }
}

fn code(
    value: &'static str,
    language: Option<&'static str>,
) -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Code {
            value: Text::Literal(value),
            language: language.map(Text::Literal),
        },
        0..0,
    )
}

fn soft() -> Result<Event<'static>, StreamError> {
    event(EventKind::SoftBreak(Text::Literal("\n")), 0..0)
}

fn hard() -> Result<Event<'static>, StreamError> {
    event(EventKind::HardBreak, 0..0)
}

/// Build a document of block events.
fn build_blocks(blocks: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![start(Tag::Document {
        line_ending: None,
        frontmatter: None,
    })];
    events.extend(blocks);
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::with_options(code_info());
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    builder.finish(String::new()).unwrap()
}

fn paragraph(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(
        Tag::Paragraph { tight: false },
        TagKind::Paragraph,
        children,
    )
}

fn heading(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Heading(1), TagKind::Heading, children)
}

/// A table with a head row of one cell holding `children`.
/// Phrasing without its soft and hard breaks, which a table cell cannot hold.
fn without_breaks(
    events: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    events
        .into_iter()
        .filter(|event| {
            !matches!(
                event,
                Ok(Event {
                    kind: EventKind::SoftBreak(_) | EventKind::HardBreak,
                    ..
                })
            )
        })
        .collect()
}

fn table(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(
        Tag::Table,
        TagKind::Table,
        attention(
            Tag::TableHead,
            TagKind::TableHead,
            attention(
                Tag::TableRow,
                TagKind::TableRow,
                attention(
                    Tag::TableCell {
                        alignment: base_markdown::AlignKind::None,
                        header: true,
                    },
                    TagKind::TableCell,
                    children,
                ),
            ),
        ),
    )
}

/// Normalize, check the result is normalized and that normalizing again changes nothing, and describe it.
fn settled(mut document: Document<String>) -> String {
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    let once = describe(&document, document.root());
    document.normalize();
    assert_eq!(describe(&document, document.root()), once);
    once
}

#[test]
fn joins_adjacent_code() {
    assert_eq!(
        settled(build(vec![
            code("a", None),
            code("b", None),
            code("c", Some("rust")),
            text("z"),
            code("d", Some("rust")),
            code("e", None),
        ])),
        r#"paragraph(code "ab" code "c" "rust" "z" code "de" "rust")"#
    );
    // Code joins across what is removed between it, and once a merge brings it together.
    assert_eq!(
        settled(build(concat([
            vec![code("a", None), text("")],
            emphasis(vec![]),
            vec![code("b", None)],
            emphasis(vec![code("c", None)]),
            emphasis(vec![code("d", None)]),
        ]))),
        r#"paragraph(code "ab" emphasis(code "cd"))"#
    );
}

#[test]
fn moves_whitespace_out_of_attention() {
    assert_eq!(
        settled(build(concat([
            vec![text("a")],
            emphasis(vec![text(" b ")]),
            vec![text("c")],
        ]))),
        r#"paragraph("a " emphasis("b") " c")"#
    );
    // Through nested attention, joining what moved to the text beside it.
    assert_eq!(
        settled(build(concat([
            vec![text("a")],
            strong(concat([
                emphasis(vec![text("\u{a0}\u{3000}b")]),
                vec![text("c")],
                delete(vec![text("d\t"), soft()]),
            ])),
            vec![text("e")],
        ]))),
        r#"paragraph("a\u{a0}\u{3000}" strong(emphasis("b") "c" delete("d")) "\t" soft "e")"#
    );
    // Attention of whitespace alone is removed, and the whitespace takes its place.
    assert_eq!(
        settled(build(concat([
            vec![text("a")],
            emphasis(strong(vec![text(" "), soft()])),
            vec![text("b")],
        ]))),
        r#"paragraph("a " soft "b")"#
    );
    // A hard break at the start stays; one at the end moves out.
    assert_eq!(
        settled(build(concat([
            vec![text("a")],
            emphasis(vec![hard(), text("b"), hard()]),
            vec![text("c")],
        ]))),
        r#"paragraph("a" emphasis(break "b") break "c")"#
    );
    // Links and images keep whitespace at their edges.
    assert_eq!(
        normalized(build(concat([
            vec![text("a")],
            link(vec![text(" b ")]),
            vec![text("c")],
        ]))),
        r#"paragraph("a" link "http://a.b"(" b ") "c")"#
    );
    // Whitespace inside a link at the edge of attention stays in the link.
    assert_eq!(
        settled(build(concat([
            vec![text("a")],
            emphasis(concat([link(vec![text(" b")]), vec![text(" ")]])),
        ]))),
        r#"paragraph("a" emphasis(link "http://a.b"(" b")) " ")"#
    );
}

#[test]
fn whitespace_moved_out_merges_what_it_separated() {
    // Once the whitespace moves out, the strong it left empty is removed and the emphasis on either side meets.
    assert_eq!(
        settled(build(concat([
            emphasis(vec![text("a")]),
            strong(vec![text(" ")]),
            emphasis(vec![text("b")]),
        ]))),
        r#"paragraph(emphasis("a") " " emphasis("b"))"#
    );
    // Attention that meets once empty attention is removed merges first, so whitespace between the two stays inside.
    assert_eq!(
        settled(build(concat([
            emphasis(vec![text("a")]),
            strong(vec![text("")]),
            emphasis(vec![text(" b")]),
        ]))),
        r#"paragraph(emphasis("a b"))"#
    );
}

fn html(value: &'static str) -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Html {
            value: Text::Literal(value),
            block: false,
        },
        0..0,
    )
}

#[test]
fn keeps_whitespace_at_the_edges_of_blocks() {
    // Whitespace is text, which normalizing never changes, so it stays at the edges of a paragraph, heading or table cell, where the serializer encodes it. Only soft breaks at either edge are removed.
    assert_eq!(
        settled(build(vec![soft(), text(" \t\na b\n "), soft()])),
        r#"paragraph(" \t\na b\n ")"#
    );
    for document in [
        build(vec![text("\r a\r\n")]),
        build(vec![text(" "), html("<b>"), text(" ")]),
        build(vec![hard(), text(" a")]),
        build_blocks(heading(vec![text(" \u{a0}a\u{a0} ")])),
        build_blocks(table(vec![text("\ta ")])),
    ] {
        assert!(
            document.is_normalized(),
            "{}",
            describe(&document, document.root())
        );
    }
    // Whitespace that moved out of attention stays too.
    assert_eq!(
        settled(build_blocks(table(concat([
            emphasis(vec![text(" a")]),
            strong(vec![text("b ")]),
        ])))),
        r#"Start(Table)(Start(TableHead)(Start(TableRow)(cell(" " emphasis("a") strong("b") " "))))"#
    );
    assert_eq!(
        settled(build(concat([
            emphasis(vec![text(" ")]),
            vec![html("<b>")]
        ]))),
        r#"paragraph(" " Html { value: Literal("<b>"), block: false }())"#
    );
    // The serializer writes the whitespace so that it reads back, including before inline HTML, which would otherwise start a line, where it can start an HTML block.
    for (source, expected) in [
        ("&#x20;hi&#x20;", "&#x20;hi&#x20;\n"),
        ("&#x9;", "&#x9;\n"),
        ("# &#x20;a&#x20;", "# &#x20;a&#x20;\n"),
        ("| &#x20;a&#x20; |\n| - |", "| &#x20;a&#x20; |\n| --- |\n"),
        ("*&#x20;a&#x20;*", "&#x20;*a*&#x20;\n"),
        ("&#x20;<b>", "&#x20;<b>\n"),
        ("<b>&#x20;", "<b>&#x20;\n"),
        ("&#x20;&#x20;<b>\n- a", "&#x20; <b>\n\n- a\n"),
        ("- <b>&#x20;", "- <b>&#x20;\n"),
        ("&#xA;<div>", "&#xA;<div>\n"),
    ] {
        let mut document = Document::parse(source.to_owned(), &ParseOptions::default()).unwrap();
        document.normalize();
        assert!(document.is_normalized(), "{}", source);
        let output = to_markdown(&document).unwrap();
        assert_eq!(output, expected, "{source}");
        let mut reparsed = Document::parse(output.clone(), &ParseOptions::default()).unwrap();
        reparsed.normalize();
        assert!(reparsed.is_normalized(), "{}", source);
        let html = |document: &Document<String>| {
            let mut html = String::new();
            document
                .render_html(&CompileOptions::default(), &mut html)
                .unwrap();
            // The final line ending follows the output's, and the source has none.
            html.trim_end().to_owned()
        };
        assert_eq!(html(&reparsed), html(&document), "{source}");
        assert_eq!(to_markdown(&reparsed).unwrap(), output, "{source}");
    }
}

#[test]
fn removes_hard_breaks_at_the_end_of_a_block() {
    assert_eq!(
        settled(build(concat([
            vec![text("a"), hard(), text(" "), hard()],
            emphasis(vec![text("b"), hard(), soft()]),
            vec![hard()],
        ]))),
        r#"paragraph("a" break " " break emphasis("b"))"#
    );
    // A line ending in text is not a break, so the breaks before it stay.
    assert!(build(vec![text("a"), hard(), soft(), text("\n")]).is_normalized());
    // Breaks alone are trimmed away, and the paragraph left empty with them.
    assert_eq!(
        settled(build_blocks(concat([
            paragraph(vec![text("a")]),
            vec![separator()],
            paragraph(concat([vec![hard()], emphasis(vec![hard()]), vec![hard()]])),
        ]))),
        r#"paragraph("a")"#
    );
}

// A table cell is one line, so it holds no break, as the parser never reads one there (#214).
#[test]
fn table_cells_hold_no_breaks() {
    for cell in [
        vec![text("a"), hard(), text("b")],
        emphasis(vec![text("a"), soft(), text("c")]),
        delete(emphasis(vec![text("d"), hard()])),
    ] {
        let mut events = vec![start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        })];
        events.extend(table(cell));
        events.push(end(TagKind::Document));
        let mut builder = AstBuilder::new();
        assert!(matches!(
            builder.consume(&mut events.into_iter(), usize::MAX),
            Err(StreamError::InvalidNesting)
        ));
    }
}

#[test]
fn removes_empty_paragraphs_with_one_separator() {
    let blank = || paragraph(vec![soft(), hard()]);
    assert_eq!(
        settled(build_blocks(concat([
            blank(),
            vec![separator()],
            paragraph(vec![text("a")]),
            vec![separator()],
            blank(),
            vec![separator()],
            paragraph(vec![]),
            vec![separator()],
            paragraph(vec![text("b")]),
            vec![separator()],
            blank(),
        ]))),
        r#"paragraph("a") separator paragraph("b")"#
    );
    // Adjacent separators lose only one each.
    assert_eq!(
        settled(build_blocks(concat([
            paragraph(vec![text("a")]),
            vec![separator(), separator()],
            blank(),
            blank(),
        ]))),
        r#"paragraph("a")"#
    );
    // A detached paragraph stays, as the root of a detached subtree is never removed.
    let mut document = build(vec![soft()]);
    let paragraph = document.children(document.root()).next().unwrap();
    document.detach(paragraph).unwrap();
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(describe(&document, paragraph), "paragraph()");
}

#[test]
fn removes_task_markers_with_nothing_written_after_them() {
    let item = |children| {
        attention(
            Tag::List {
                start: None,
                tight: true,
            },
            TagKind::List,
            attention(
                Tag::Item {
                    block_end: false,
                    checked: Some(false),
                },
                TagKind::Item,
                paragraph(children),
            ),
        )
    };
    let list = |inner: &str| {
        format!(r#"Start(List {{ start: None, tight: true }})(item false(paragraph({inner})))"#)
    };
    let task = |inner: &str| {
        format!(
            r#"Start(List {{ start: None, tight: true }})(item false task(paragraph({inner})))"#
        )
    };
    // Breaks after text are trimmed, as after any paragraph's phrasing.
    assert_eq!(
        settled(build_blocks(item(vec![text(" a"), hard(), soft()]))),
        task(r#"" a""#)
    );
    // So is a hard break that moves out of attention at the end.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![text(" a")],
            emphasis(vec![text("b"), hard()]),
        ])))),
        task(r#"" a" emphasis("b")"#)
    );
    // A marker followed by nothing but breaks and the one space or tab that separates it from its text, which the parser keeps in that text, cannot be written, so it is removed. Its breaks are then removed as at the edges of any paragraph, the separator stays as whitespace text, and a paragraph left empty is removed.
    for (after, expected) in [
        (vec![], "item false()"),
        (vec![soft()], "item false()"),
        (vec![hard(), soft()], "item false()"),
        (vec![text(" ")], r#"item false(paragraph(" "))"#),
        (vec![text("\t")], r#"item false(paragraph("\t"))"#),
        (vec![text(" "), soft()], r#"item false(paragraph(" "))"#),
        (vec![text("\t"), hard()], r#"item false(paragraph("\t"))"#),
        (
            vec![text(" "), hard(), soft(), soft()],
            r#"item false(paragraph(" "))"#,
        ),
        (
            vec![soft(), soft(), text(" ")],
            r#"item false(paragraph(" "))"#,
        ),
        (
            vec![hard(), text(" ")],
            r#"item false(paragraph(break " "))"#,
        ),
    ] {
        let mut children = vec![];
        children.extend(after);
        assert_eq!(
            settled(build_blocks(item(children))),
            format!("Start(List {{ start: None, tight: true }})({expected})")
        );
    }
    // Anything else after the separator is written after it, so the marker stays, including more whitespace, which the serializer encodes; trailing breaks are still removed.
    let normal = |mut document: Document<String>| {
        document.normalize();
        assert!(document.is_normalized());
        describe(&document, document.root())
    };
    for (after, expected) in [
        (vec![text("  ")], r#""  ""#),
        (vec![text(" \t")], r#"" \t""#),
        (vec![text(" \u{a0}"), hard()], r#"" \u{a0}""#),
        (vec![text(" \u{3000}"), soft()], r#"" \u{3000}""#),
        (vec![text(" \n")], r#"" \n""#),
        (vec![text(" "), soft(), text(" ")], r#"" " soft " ""#),
        (vec![text(" "), text(" ")], r#""  ""#),
        // The separator space the parser leaves after a marker is put back.
        (vec![text("\u{a0}"), hard()], r#"" \u{a0}""#),
    ] {
        let mut children = vec![];
        children.extend(after);
        assert_eq!(normal(build_blocks(item(children))), task(expected));
    }
    // Whitespace moved out of attention counts as text, and attention of breaks alone is removed.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![text(" ")],
            emphasis(vec![soft()]),
        ])))),
        list(r#"" ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![],
            emphasis(vec![text(" ")]),
            vec![soft()],
        ])))),
        list(r#"" ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![text(" ")],
            emphasis(vec![text(" ")]),
        ])))),
        task(r#""  ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat(
            [vec![], emphasis(vec![hard()]),]
        )))),
        "Start(List { start: None, tight: true })(item false())"
    );
    // A marker followed by anything else stays, such as code or a link.
    for after in [
        vec![text(" "), code("c", None)],
        concat([vec![text(" ")], link(vec![text(" ")])]),
    ] {
        let mut children = vec![];
        children.extend(after);
        assert!(build_blocks(item(children)).is_normalized());
    }
    // Attention in it still moves its whitespace out.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![text(" ")],
            emphasis(vec![text("a ")]),
        ])))),
        task(r#"" " emphasis("a") " ""#)
    );
}

/// Normalize `document`, serialize it, and check the output is `expected` and serializes the same once read back.
fn serializes_stably(mut document: Document<String>, expected: &str) {
    document.normalize();
    let output = to_markdown(&document).unwrap();
    assert_eq!(output, expected);
    let mut reparsed = Document::parse(output.clone(), &ParseOptions::default()).unwrap();
    reparsed.normalize();
    let html = |document: &Document<String>| {
        let mut html = String::new();
        document
            .render_html(&CompileOptions::default(), &mut html)
            .unwrap();
        html.trim_end()
            .replace("<li>\n<p>", "<li>")
            .replace("</p></li>", "</li>")
    };
    assert_eq!(html(&reparsed), html(&document));
    assert_eq!(to_markdown(&reparsed).unwrap(), output);
}

#[test]
fn trimmed_shapes_serialize_stably() {
    let item = |children| {
        attention(
            Tag::List {
                start: None,
                tight: true,
            },
            TagKind::List,
            attention(
                Tag::Item {
                    block_end: false,
                    checked: Some(false),
                },
                TagKind::Item,
                paragraph(children),
            ),
        )
    };
    serializes_stably(build_blocks(item(vec![text(" a"), hard()])), "- [ ] a\n");
    serializes_stably(
        build_blocks(item(concat([
            vec![text(" a")],
            emphasis(vec![text("b"), hard()]),
        ]))),
        "- [ ] a*b*\n",
    );
    // A marker with nothing written after its separator is removed, and the separator stays as whitespace text, which is encoded.
    for (after, expected) in [
        (vec![], "-\n"),
        (vec![soft()], "-\n"),
        (vec![text(" ")], "- &#x20;\n"),
        (vec![text(" "), soft()], "- &#x20;\n"),
        (vec![text(" "), hard()], "- &#x20;\n"),
        (vec![text("\t"), hard()], "- &#x9;\n"),
    ] {
        serializes_stably(build_blocks(item(after)), expected);
    }
    // Anything else after the separator keeps the marker, and is encoded where it would be trimmed. A tab separator is written as one.
    for (after, expected) in [
        (vec![text("  ")], "- [ ] &#x20;\n"),
        (vec![text(" \t")], "- [ ] &#x9;\n"),
        (vec![text(" \u{a0}"), hard()], "- [ ] \u{a0}\n"),
        (vec![text(" \u{3000}")], "- [ ] \u{3000}\n"),
        (vec![text(" \n")], "- [ ] &#xA;\n"),
        (vec![text(" "), soft(), text(" ")], "- [ ] &#xA;&#x20;\n"),
        (vec![text("\t ")], "- [ ]\t&#x20;\n"),
        (vec![text("\ta")], "- [ ]\ta\n"),
    ] {
        serializes_stably(build_blocks(item(after)), expected);
    }
    serializes_stably(
        build(concat([
            vec![text("a"), soft()],
            emphasis(vec![soft(), text(" b")]),
        ])),
        "a\n&#xA;&#x20;*b*\n",
    );
}

#[test]
fn detached_attention_keeps_its_own_edges() {
    let mut document = build(emphasis(concat([
        vec![text(" a")],
        strong(vec![text(" b ")]),
        vec![text(" ")],
    ])));
    let paragraph = document.children(document.root()).next().unwrap();
    let attention = document.children(paragraph).next().unwrap();
    document.detach(attention).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(
        describe(&document, attention),
        r#"emphasis(" a " strong("b") "  ")"#
    );
}

#[test]
fn splitting_text_keeps_the_node_and_its_span() {
    // Source text, a literal, and borrowed and owned values each split.
    let source = " a ";
    let texts = [
        Text::Source(0..3),
        Text::Literal(" a "),
        Text::Value(Cow::Borrowed(" a ")),
        Text::Value(Cow::Owned(" a ".to_owned())),
    ];
    for value in texts {
        let mut document = build_in(
            source,
            concat([
                vec![text("x")],
                emphasis(vec![event(EventKind::Text(Text::Literal("b")), 0..3)]),
                vec![text("y")],
            ]),
        );
        let paragraph = document.children(document.root()).next().unwrap();
        let attention = document.children(paragraph).nth(1).unwrap();
        let inner = document.children(attention).next().unwrap();
        // Building copies borrowed values, so set the value in place to keep it as it is.
        document.set_kind(inner, EventKind::Text(value)).unwrap();
        document.normalize();
        assert!(document.is_normalized());
        assert_eq!(
            describe(&document, document.root()),
            r#"paragraph("x " emphasis("a") " y")"#
        );
        // The text keeps its ID and span, and the whitespace split off joined the text beside it.
        assert_eq!(document.children(attention).next(), Some(inner));
        assert_eq!(document.node(inner).unwrap().span, 0..3);
    }
}

#[test]
fn moved_whole_nodes_keep_their_ids() {
    let mut document = build(concat([
        vec![text("a")],
        emphasis(vec![text("b"), soft()]),
        vec![text("c")],
    ]));
    let paragraph = document.children(document.root()).next().unwrap();
    let attention = document.children(paragraph).nth(1).unwrap();
    let soft = document.children(attention).nth(1).unwrap();
    document.normalize();
    assert_eq!(
        describe(&document, paragraph),
        r#"paragraph("a" emphasis("b") soft "c")"#
    );
    // The soft break moved whole.
    assert_eq!(document.children(paragraph).nth(2), Some(soft));
}

#[test]
fn is_normalized_finds_each_edge_shape() {
    let shapes = [
        build(vec![code("a", None), code("b", None)]),
        build(emphasis(vec![text(" a")])),
        build(concat([vec![text("a")], emphasis(vec![text("b\u{a0}")])])),
        build(concat([vec![text("a")], emphasis(vec![soft(), text("b")])])),
        build(concat([
            vec![text("a")],
            strong(vec![text("b"), hard()]),
            vec![text("c")],
        ])),
        build(vec![soft(), text("a")]),
        build(vec![text("a"), soft()]),
        build(vec![text("a"), hard()]),
        build_blocks(heading(vec![soft(), text("a")])),
        build_blocks(paragraph(vec![])),
    ];
    for document in shapes {
        assert!(
            !document.is_normalized(),
            "{}",
            describe(&document, document.root())
        );
    }
    let normal = [
        build(vec![code("a", None), code("b", Some("rust"))]),
        build(concat([
            vec![text("a ")],
            emphasis(vec![hard(), text("b")]),
            vec![text(" c")],
        ])),
        build(vec![text("\u{a0}a\u{a0}")]),
        build(vec![text(" a ")]),
        build(vec![hard(), text("a")]),
        build_blocks(heading(vec![text("\ta")])),
        build_blocks(table(vec![text("a\n")])),
        build(concat([
            vec![text("a")],
            link(vec![text(" b ")]),
            vec![text("c")],
        ])),
        build_blocks(heading(vec![])),
        build_blocks(table(vec![])),
    ];
    for document in normal {
        assert!(
            document.is_normalized(),
            "{}",
            describe(&document, document.root())
        );
    }
}

/// Time normalizing the document from `build`, without serializing, for trees nested deeper than the serializer recurses comfortably, and check what is left with `check`.
fn assert_normalize_only_linear(
    label: &str,
    size: usize,
    build: impl Fn(usize) -> Document<String>,
    check: impl Fn(&Document<String>, usize),
) {
    assert_linear(label, size, |size| {
        let mut document = build(size);
        let started = Instant::now();
        document.normalize();
        let elapsed = started.elapsed();
        assert!(document.is_normalized());
        check(&document, size);
        elapsed
    });
}

/// Attention nested `depth` deep, alternating emphasis and strong, each level holding `before`, the next level and `after`, around `inner`.
fn nested_edges(
    depth: usize,
    before: impl Fn() -> Vec<Result<Event<'static>, StreamError>>,
    inner: Vec<Result<Event<'static>, StreamError>>,
    after: impl Fn() -> Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = Vec::new();
    for level in 0..depth {
        events.push(start(if level % 2 == 0 {
            Tag::Emphasis
        } else {
            Tag::Strong
        }));
        events.extend(before());
    }
    events.extend(inner);
    for level in (0..depth).rev() {
        events.extend(after());
        events.push(end(if level % 2 == 0 {
            TagKind::Emphasis
        } else {
            TagKind::Strong
        }));
    }
    events
}

#[test]
fn moving_whitespace_out_of_deep_attention_is_linear() {
    // Each level starts and ends with whitespace, which moves out through every level outside it. Attention already holds no whitespace at its edges once its parent has moved it out, so it is not searched again. Hard breaks at the end of each level take the same path.
    assert_normalize_only_linear(
        "whitespace at the edges of each level",
        1 << 9,
        |size| {
            build(concat([
                vec![text("x")],
                nested_edges(
                    size / 4,
                    || vec![text(" "), soft()],
                    vec![text("a")],
                    || vec![text(" ")],
                ),
                vec![text("y")],
            ]))
        },
        |document, size| {
            // Before the levels, the text that moved out stays apart between the soft breaks, the first joining "x"; after them, it all joins "y".
            assert_eq!(
                document.descendants(document.root()).count(),
                2 + size / 4 * 3 + 2
            );
        },
    );
    // Whitespace alone inside: every level is emptied and removed, and moving out continues from the level above, not from the outermost.
    assert_normalize_only_linear(
        "whitespace alone inside",
        1 << 9,
        |size| {
            build(concat([
                vec![text("x")],
                nested_edges(
                    size / 4,
                    || vec![text(" ")],
                    vec![soft()],
                    || vec![text("\t")],
                ),
                vec![text("y")],
            ]))
        },
        |document, _| {
            let paragraph = document.children(document.root()).next().unwrap();
            // The spaces join "x", the soft break stays apart, and the tabs join "y".
            assert_eq!(document.children(paragraph).count(), 3);
        },
    );
    // Whitespace at an edge of each of many sibling attention children: normalizing continues from what moved, not from the first child.
    let siblings = |label: &str, unit: &dyn Fn() -> Vec<Result<Event<'static>, StreamError>>| {
        assert_normalize_only_linear(
            label,
            1 << 9,
            |size| build(concat((0..size).map(|_| unit()))),
            |document, size| {
                let paragraph = document.children(document.root()).next().unwrap();
                assert_eq!(document.children(paragraph).count(), 2 * size);
            },
        );
    };
    siblings("whitespace leading each sibling", &|| {
        concat([vec![text("b")], emphasis(vec![text(" a")])])
    });
    siblings("whitespace trailing each sibling", &|| {
        concat([emphasis(vec![text("a ")]), vec![text("b")]])
    });
}

#[test]
fn removing_break_chains_is_linear() {
    // At the end of a block, where the whole run is cut off at once.
    assert_normalize_linear("breaks at the end of a block", 1 << 9, |size| {
        build(concat([
            vec![text("a")],
            (0..size).map(|_| hard()).collect(),
        ]))
    });
    // At the end of attention, followed by more text, where the whole run moves out at once. In a table cell, each becomes a space and is joined, as `joining_text_is_linear` checks.
    assert_normalize_linear("breaks at the end of attention", 1 << 9, |size| {
        build(concat([
            emphasis(concat([
                vec![text("a")],
                (0..size).map(|_| hard()).collect(),
            ])),
            vec![text("b")],
        ]))
    });
}

#[test]
fn line_endings_in_code_are_rejected() {
    // The parser reads each line ending in inline code as one space, so a value with one is not one the parser stores, and normalizing rewrites no characters, so it is rejected where it is given (#257).
    for value in ["a\nb", "a\r\nb", "a\rb", "\n", "a\0b"] {
        let mut builder = AstBuilder::new();
        let events = vec![
            start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            }),
            start(Tag::Paragraph { tight: false }),
            code(value, None),
            end(TagKind::Paragraph),
            end(TagKind::Document),
        ];
        assert!(
            matches!(
                builder.consume(&mut events.into_iter(), usize::MAX),
                Err(StreamError::InvalidValue)
            ),
            "{:?}",
            value
        );
    }
    // Flow code keeps its line endings.
    let mut document = build_blocks(vec![event(
        EventKind::CodeBlock {
            value: Text::Literal("a\nb\n"),
            info: None,
            fence: None,
            unclosed: false,
        },
        0..0,
    )]);
    assert!(document.is_normalized());
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "```\na\nb\n```\n");
}

#[test]
fn empty_code_is_removed() {
    // A run of backticks with nothing between them is not code, so empty code cannot be written: it is valid, not normalized, and normalizing removes it, as it does inline HTML without a value, and what was on either side meets.
    assert_eq!(
        settled(build(vec![text("a"), code("", Some("rust")), text("b")])),
        r#"paragraph("ab")"#
    );
    assert_eq!(settled(build(vec![code("", None)])), "");
}

#[test]
fn code_reads_back_and_is_stable() {
    let values = [
        // Spaces at the edges, which the parser strips one of when the value is not only spaces.
        " a ", "  ", " ", " `", "`", "$",
        // A tab is not a space, so the parser strips the spaces around it.
        " \t ", "\t", " \u{a0} ",
    ];
    let mut trees = Vec::new();
    for value in values {
        for (prefix, literal) in [("x ", code(value, None)), ("x | y ", code(value, None))] {
            trees.push(build_blocks(paragraph(vec![text(prefix), literal])));
        }
        trees.push(build_blocks(heading(vec![code(value, None)])));
        trees.push(build_blocks(heading(vec![text("x "), code(value, None)])));
        trees.push(build_blocks(table(vec![text("x "), code(value, None)])));
    }
    for tree in trees {
        let unnormalized = to_markdown(&tree).unwrap();
        let mut normal = tree;
        normal.normalize();
        assert!(normal.is_normalized());
        let markdown = to_markdown(&normal).unwrap();
        assert_eq!(unnormalized, markdown);
        let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
        reparsed.normalize();
        let reparsed_block = reparsed.children(reparsed.root()).next().unwrap();
        let normal_block = normal.children(normal.root()).next().unwrap();
        // The parser keeps a separator after a table row, which the hand-built table has none of.
        assert_eq!(
            describe(&reparsed, reparsed_block).replace(" separator", ""),
            describe(&normal, normal_block),
            "{markdown:?}"
        );
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
    }
}

#[test]
fn code_of_spaces_around_a_tab_is_stable() {
    // The parser strips one space from each side of `  \t  `, as it is not only spaces, so the serializer pads it. Whitespace with a tab at an edge is not stripped, so it is written as it is.
    for source in ["x `  \t  `\n", "x ` \t`\n", "x `\t`\n", "x ` \ta\t `\n"] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), source);
    }
}

/// A block of a list tree built through the document API, which keeps paragraph and item rendering hints in step with the list.
enum Block {
    Paragraph(Vec<EventKind<'static>>),
    /// A paragraph that, first in an item, makes it an unchecked task.
    Task(Vec<EventKind<'static>>),
    /// A list's start and tightness, and each item's recorded spread and blocks.
    List(Option<u32>, bool, Vec<(bool, Vec<Block>)>),
    Code,
    Quote(Vec<Block>),
    Footnote(Vec<Block>),
    /// A heading of rank 1 and its phrasing.
    Heading(Vec<EventKind<'static>>),
    /// A table of a head row of one cell, `x`.
    Table,
    Leaf(EventKind<'static>),
    Separator,
}

fn append_block(document: &mut Document<String>, parent: NodeId, block: &Block) {
    match block {
        Block::Paragraph(phrasing) | Block::Task(phrasing) => {
            let id = document
                .append_child(parent, EventKind::Start(Tag::Paragraph { tight: false }))
                .unwrap();
            for kind in phrasing {
                document.append_child(id, kind.clone()).unwrap();
            }
        }
        Block::List(start, tight, items) => {
            let list = document
                .append_child(
                    parent,
                    EventKind::Start(Tag::List {
                        start: *start,
                        tight: *tight,
                    }),
                )
                .unwrap();
            for (spread, blocks) in items {
                let checked = matches!(blocks.first(), Some(Block::Task(_))).then_some(false);
                let item = document
                    .append_child(
                        list,
                        EventKind::Start(Tag::Item {
                            block_end: false,
                            checked,
                        }),
                    )
                    .unwrap();
                document
                    .set_syntax(item, Some(NodeSyntax::ListItem { spread: *spread }))
                    .unwrap();
                for block in blocks {
                    append_block(document, item, block);
                }
            }
        }
        Block::Quote(blocks) => {
            let quote = document
                .append_child(parent, EventKind::Start(Tag::BlockQuote))
                .unwrap();
            for block in blocks {
                append_block(document, quote, block);
            }
        }
        Block::Footnote(blocks) => {
            // Numbered in order, as an edit adds no second definition of an identifier.
            let number = 1 + document
                .nodes()
                .iter()
                .filter(|node| {
                    matches!(node.kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
                })
                .count();
            let definition = document
                .append_child(
                    parent,
                    EventKind::Start(Tag::FootnoteDefinition {
                        identifier: Text::from(number.to_string()),
                        label: None,
                    }),
                )
                .unwrap();
            for block in blocks {
                append_block(document, definition, block);
            }
        }
        Block::Heading(phrasing) => {
            let id = document
                .append_child(parent, EventKind::Start(Tag::Heading(1)))
                .unwrap();
            for kind in phrasing {
                document.append_child(id, kind.clone()).unwrap();
            }
        }
        Block::Table => {
            let mut id = parent;
            for tag in [
                Tag::Table,
                Tag::TableHead,
                Tag::TableRow,
                Tag::TableCell {
                    alignment: base_markdown::AlignKind::None,
                    header: true,
                },
            ] {
                id = document.append_child(id, EventKind::Start(tag)).unwrap();
            }
            document
                .append_child(id, EventKind::Text(Text::from("x".to_owned())))
                .unwrap();
        }
        Block::Leaf(kind) => {
            document.append_child(parent, kind.clone()).unwrap();
        }
        Block::Separator => {
            document
                .append_child(
                    parent,
                    EventKind::Separator {
                        value: Text::Literal("\n"),
                        conditional: false,
                    },
                )
                .unwrap();
        }
        Block::Code => {
            document
                .append_child(
                    parent,
                    EventKind::CodeBlock {
                        value: Text::from("x\n".to_owned()),
                        info: None,
                        fence: None,
                        unclosed: false,
                    },
                )
                .unwrap();
        }
    }
}

fn build_list(block: &Block) -> Document<String> {
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    append_block(&mut document, root, block);
    document
}

fn rendered<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output.trim_end().to_owned()
}

/// Normalize `document`, serialize it, and assert that it reads back as the same tree and HTML and is written the same again. Returns the Markdown.
fn reads_back<S: AsRef<str> + Clone>(mut document: Document<S>) -> String {
    document.normalize();
    assert!(document.is_normalized());
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::default()).unwrap();
    reparsed.normalize();
    assert!(reparsed.is_normalized());
    assert_eq!(rendered(&reparsed), rendered(&document), "{markdown:?}");
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
    markdown
}

fn tight_of_first_list<S: AsRef<str>>(document: &Document<S>) -> bool {
    let list = document.children(document.root()).next().unwrap();
    match document.node(list).unwrap().kind {
        EventKind::Start(Tag::List { tight, .. }) => tight,
        _ => unreachable!("the first block is a list"),
    }
}

#[test]
fn a_loose_list_reads_back_loose_after_empty_paragraphs_are_removed() {
    use Block::{Code, List, Paragraph, Task};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let empties = || {
        [
            Paragraph(vec![]),
            Paragraph(vec![EventKind::SoftBreak(Text::Literal("\n"))]),
            Task(vec![]),
            Paragraph(vec![EventKind::HardBreak]),
        ]
    };
    let nested = || List(None, true, vec![(false, vec![p("c")])]);
    // Each item records its spread both ways: as the parser infers it, from a blank line between its blocks, and as a hand-built tree may leave it, unset.
    for spread in [false, true] {
        let lone = |blocks: Vec<Block>| List(None, false, vec![(spread, blocks)]);
        for empty in empties() {
            // A task item keeps its checkbox, which moves to the paragraph that is now its first.
            let expected = if matches!(empty, Task(_)) {
                "- [ ] b\n"
            } else {
                "- b\n"
            };
            // The only item of a loose list, left with one paragraph, cannot be written loose: no blank line can separate items or blocks, so normalizing makes the list tight (#128).
            let mut document = build_list(&lone(vec![empty, p("b")]));
            assert!(!document.is_normalized());
            assert_eq!(reads_back(document.clone()), expected);
            document.normalize();
            assert!(tight_of_first_list(&document));
        }
        for (blocks, expected) in [
            (vec![p("b"), Paragraph(vec![])], "- b\n"),
            (vec![Paragraph(vec![])], "-\n"),
            (vec![Paragraph(vec![]), Code], "- ```\n  x\n  ```\n"),
            (vec![Paragraph(vec![]), nested()], "- - c\n"),
            // A task marker with text after it stays.
            (vec![Task(vec![text(" x")]), Paragraph(vec![])], "- [ ] x\n"),
            (vec![Task(vec![]), p(" x")], "- [ ] x\n"),
            // So is a lone item that was never emptied.
            (vec![p("b")], "- b\n"),
            (vec![], "-\n"),
            (vec![Code], "- ```\n  x\n  ```\n"),
        ] {
            assert_eq!(reads_back(build_list(&lone(blocks))), expected);
        }
        assert_eq!(
            reads_back(build_list(&List(
                Some(3),
                false,
                vec![(spread, vec![Paragraph(vec![]), p("b")])]
            ))),
            "3. b\n"
        );
        // An item left with two blocks or more is written with a blank line between them, so the list stays loose, whatever spread it records.
        for (blocks, expected) in [
            (vec![Paragraph(vec![]), p("b"), p("c")], "- b\n\n  c\n"),
            (vec![p("b"), Paragraph(vec![]), nested()], "- b\n\n  - c\n"),
            (
                vec![Paragraph(vec![]), p("b"), Code],
                "- b\n\n  ```\n  x\n  ```\n",
            ),
            (
                vec![Paragraph(vec![]), Code, Code],
                "- ```\n  x\n  ```\n\n  ```\n  x\n  ```\n",
            ),
            (vec![Paragraph(vec![]), nested(), p("b")], "- - c\n\n  b\n"),
            (
                vec![Task(vec![text(" x")]), Paragraph(vec![]), p("b")],
                "- [ ] x\n\n  b\n",
            ),
            (vec![p("a"), nested()], "- a\n\n  - c\n"),
        ] {
            let document = build_list(&lone(blocks));
            assert_eq!(reads_back(document.clone()), expected);
            let mut document = document;
            document.normalize();
            assert!(!tight_of_first_list(&document));
        }
        // With other items, the blank lines between items keep the list loose, wherever the emptied item is.
        for (items, expected) in [
            (
                vec![
                    (spread, vec![Paragraph(vec![]), p("b")]),
                    (false, vec![p("a")]),
                ],
                "- b\n\n- a\n",
            ),
            (
                vec![
                    (false, vec![p("a")]),
                    (spread, vec![Paragraph(vec![]), p("b")]),
                ],
                "- a\n\n- b\n",
            ),
            (
                vec![
                    (false, vec![p("a")]),
                    (spread, vec![Paragraph(vec![]), p("b")]),
                    (false, vec![p("c")]),
                ],
                "- a\n\n- b\n\n- c\n",
            ),
            (
                vec![(spread, vec![Paragraph(vec![])]), (false, vec![p("a")])],
                "-\n\n- a\n",
            ),
            (
                vec![
                    (spread, vec![Paragraph(vec![])]),
                    (spread, vec![Paragraph(vec![])]),
                ],
                "-\n\n-\n",
            ),
        ] {
            let document = build_list(&List(None, false, items));
            assert_eq!(reads_back(document.clone()), expected);
            let mut document = document;
            document.normalize();
            assert!(!tight_of_first_list(&document));
        }
        let ordered = List(
            Some(1),
            false,
            vec![
                (false, vec![p("a")]),
                (spread, vec![Paragraph(vec![]), p("b")]),
                (false, vec![p("c")]),
            ],
        );
        assert_eq!(reads_back(build_list(&ordered)), "1. a\n\n2. b\n\n3. c\n");
        // A nested list is made tight on its own, and a loose list around it stays loose while it has other items.
        let inner = || List(None, false, vec![(spread, vec![Paragraph(vec![]), p("b")])]);
        for (list, expected) in [
            (
                List(None, true, vec![(false, vec![p("a"), inner()])]),
                "- a\n  - b\n",
            ),
            (
                List(
                    None,
                    false,
                    vec![(false, vec![p("a")]), (false, vec![p("a"), inner()])],
                ),
                "- a\n\n- a\n  - b\n",
            ),
            (lone(vec![Paragraph(vec![]), inner()]), "- - b\n"),
        ] {
            assert_eq!(reads_back(build_list(&list)), expected);
        }
        // A tight list is left as it is.
        assert_eq!(
            reads_back(build_list(&List(
                None,
                true,
                vec![(spread, vec![Paragraph(vec![]), p("b")])]
            ))),
            "- b\n"
        );
    }
}

#[test]
fn is_normalized_finds_a_loose_list_that_reads_back_tight() {
    use Block::{Code, List, Paragraph};
    let p = || Paragraph(vec![EventKind::Text(Text::from("a".to_owned()))]);
    let lone = |blocks: Vec<Block>| List(None, false, vec![(true, blocks)]);
    assert!(!build_list(&lone(vec![p()])).is_normalized());
    assert!(!build_list(&lone(vec![])).is_normalized());
    assert!(!build_list(&lone(vec![Code])).is_normalized());
    assert!(build_list(&lone(vec![p(), Code])).is_normalized());
    assert!(build_list(&List(
        None,
        false,
        vec![(false, vec![p()]), (false, vec![p()])]
    ))
    .is_normalized());
    assert!(build_list(&List(None, true, vec![(false, vec![p()])])).is_normalized());
    // A list holds only items, and separators sit between blocks, never between items.
    let mut document = build_list(&List(None, false, vec![]));
    let list = document.children(document.root()).next().unwrap();
    for kind in [
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::Separator {
            value: Text::Literal("\n"),
            conditional: false,
        },
    ] {
        assert!(matches!(
            document.append_child(list, kind),
            Err(StreamError::InvalidNesting)
        ));
    }
    // Separators between the blocks of an item are not counted.
    let mut document = build_list(&lone(vec![p()]));
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    document
        .append_child(
            item,
            EventKind::Separator {
                value: Text::Literal("\n"),
                conditional: false,
            },
        )
        .unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert!(tight_of_first_list(&document));
    // A detached list is normalized too.
    let mut document = build_list(&lone(vec![p()]));
    let list = document.children(document.root()).next().unwrap();
    document.detach(list).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert!(matches!(
        document.node(list).unwrap().kind,
        EventKind::Start(Tag::List { tight: true, .. })
    ));
}

#[test]
fn parsed_lone_loose_items_keep_their_output() {
    // The parser infers an item's spread from a blank line between its blocks in the source, including one that a nested list or footnote definition takes in at its end, as CommonMark does. So an item with a blank line after a nested list records spread, as its loose list does, and is written with a blank line between each of its blocks (#157). One with no other joins is written unchanged.
    for (source, expected) in [
        ("- a\n  - b\n\n  c\n", "- a\n\n  - b\n\n  c\n"),
        ("- [^1]: x\n\n  y\n", "- [^1]: x\n\n  y\n"),
        ("- a\n\n  [x]: /u\n", "- a\n\n  [x]: /u\n"),
        ("- a\n\n  b\n", "- a\n\n  b\n"),
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized());
        assert!(!tight_of_first_list(&document));
        assert_eq!(reads_back(document), expected);
    }
}

#[test]
fn tightening_lone_loose_items_is_linear() {
    // Lone loose items nested in each other, each left with one block once its empty paragraph is removed. Appending nests in time linear in the depth, and serializing nests too deeply for the stack, so the tree is built from events and only normalizing is timed.
    assert_linear("nested lone loose items", 1 << 8, |size| {
        let mut events = Vec::new();
        for _ in 0..size {
            events.extend([
                start(Tag::List {
                    start: None,
                    tight: false,
                }),
                start(Tag::Item {
                    block_end: false,
                    checked: None,
                }),
            ]);
            events.extend(paragraph(vec![]));
        }
        events.extend(paragraph(vec![text("a")]));
        for _ in 0..size {
            events.extend([end(TagKind::Item), end(TagKind::List)]);
        }
        let mut document = build_blocks(events);
        let started = Instant::now();
        document.normalize();
        assert!(document.is_normalized());
        started.elapsed()
    });
    // A lone item of many empty paragraphs and one that is not.
    assert_normalize_linear("a lone item of empty paragraphs", 1 << 8, |size| {
        let mut blocks: Vec<Block> = (0..size).map(|_| Block::Paragraph(vec![])).collect();
        blocks.push(Block::Paragraph(vec![EventKind::Text(Text::from(
            "a".to_owned(),
        ))]));
        build_list(&Block::List(None, false, vec![(false, blocks)]))
    });
    // A lone item of many blocks, which is written spread.
    assert_normalize_linear("a lone item of many blocks", 1 << 8, |size| {
        build_list(&Block::List(
            None,
            false,
            vec![(false, (0..size).map(|_| Block::Code).collect())],
        ))
    });
}

#[test]
fn an_empty_item_after_a_paragraph_in_a_tight_list_is_removed() {
    use Block::{List, Paragraph, Quote, Separator, Task};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |items: Vec<Vec<Block>>| {
        List(
            None,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let ordered = |start: u32, items: Vec<Vec<Block>>| {
        List(
            Some(start),
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let after_p = |list: Block| tight(vec![vec![p("p"), list]]);
    // An empty item cannot interrupt a paragraph, and `-` alone under one is a setext underline, so in a tight list, where no blank line can come between them, a list after a paragraph cannot start with an empty item: `- p\n  -` reads back as a heading, and `- p\n  1.` and `- p\n  *` as text (#129). Normalizing removes the leading empty items of such a list, and the list itself when none is left.
    for (block, expected) in [
        (after_p(tight(vec![vec![]])), "- p\n"),
        (
            tight(vec![vec![p("p"), tight(vec![vec![]])], vec![p("q")]]),
            "- p\n- q\n",
        ),
        (after_p(ordered(1, vec![vec![]])), "- p\n"),
        (
            after_p(ordered(1, vec![vec![], vec![p("a")]])),
            "- p\n  1. a\n",
        ),
        (after_p(tight(vec![vec![], vec![p("a")]])), "- p\n  - a\n"),
        (after_p(tight(vec![vec![], vec![]])), "- p\n"),
        (
            after_p(tight(vec![vec![], vec![], vec![p("a")], vec![]])),
            "- p\n  - a\n  -\n",
        ),
        // A task marker with nothing after it is removed, which leaves its item empty (#126).
        (after_p(tight(vec![vec![Task(vec![])]])), "- p\n"),
        (
            after_p(tight(vec![vec![Task(vec![EventKind::HardBreak])]])),
            "- p\n",
        ),
        (
            after_p(tight(vec![vec![Task(vec![EventKind::SoftBreak(
                Text::Literal("\n"),
            )])]])),
            "- p\n",
        ),
        (
            after_p(tight(vec![vec![Task(vec![])], vec![p("a")]])),
            "- p\n  - a\n",
        ),
        // An empty paragraph between them is removed first.
        (
            tight(vec![vec![p("p"), Paragraph(vec![]), tight(vec![vec![]])]]),
            "- p\n",
        ),
        // Separators are not counted, and one goes with the list.
        (
            tight(vec![vec![p("p"), Separator, tight(vec![vec![Separator]])]]),
            "- p\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                Separator,
                tight(vec![vec![]]),
                Separator,
                Block::Code,
            ]]),
            "- p\n  ```\n  x\n  ```\n",
        ),
        // A list that then follows the paragraph is handled in turn.
        (
            tight(vec![vec![
                p("p"),
                tight(vec![vec![]]),
                ordered(1, vec![vec![]]),
            ]]),
            "- p\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                tight(vec![vec![]]),
                ordered(1, vec![vec![p("a")]]),
            ]]),
            "- p\n  1. a\n",
        ),
        // A loose nested list left with one item of one block reads back tight, so it is made tight.
        (
            after_p(List(
                None,
                false,
                vec![(true, vec![]), (true, vec![p("a")])],
            )),
            "- p\n  - a\n",
        ),
        // In a block quote, and at any depth.
        (Quote(vec![after_p(tight(vec![vec![]]))]), "> - p\n"),
        (
            after_p(tight(vec![vec![p("q"), tight(vec![vec![]])]])),
            "- p\n  - q\n",
        ),
        (
            tight(vec![vec![Quote(vec![after_p(tight(vec![
                vec![],
                vec![p("a")],
            ]))])]]),
            "- > - p\n  >   - a\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(!document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // Shapes that can be written are left as they are: an empty item after another item, an item whose only block is a list, an item of a task marker and its separator, which keeps the separator as text, a loose list, where a blank line comes before the nested list, and an empty item after a paragraph outside an item.
    for (block, expected) in [
        (
            after_p(tight(vec![vec![p("a")], vec![]])),
            "- p\n  - a\n  -\n",
        ),
        (
            after_p(tight(vec![vec![p("a")], vec![], vec![]])),
            "- p\n  - a\n  -\n  -\n",
        ),
        (
            after_p(tight(vec![vec![tight(vec![vec![]])]])),
            "- p\n  - -\n",
        ),
        (tight(vec![vec![p("p")], vec![]]), "- p\n-\n"),
        (
            List(None, false, vec![(true, vec![p("p"), tight(vec![vec![]])])]),
            "- p\n\n  -\n",
        ),
        (
            List(
                None,
                false,
                vec![
                    (true, vec![p("p"), tight(vec![vec![]])]),
                    (false, vec![p("q")]),
                ],
            ),
            "- p\n\n  -\n\n- q\n",
        ),
        (Quote(vec![p("p"), tight(vec![vec![]])]), "> p\n>\n> -\n"),
    ] {
        let document = build_list(&block);
        assert!(document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    for (phrasing, expected) in [(" ", "- p\n  - &#x20;\n"), ("\t", "- p\n  - &#x9;\n")] {
        let block = after_p(tight(vec![vec![Task(vec![text(phrasing)])]]));
        assert_eq!(reads_back(build_list(&block)), expected);
    }
    let mut document = build_list(&p("p"));
    let root = document.root();
    append_block(&mut document, root, &tight(vec![vec![]]));
    assert!(document.is_normalized());
    assert_eq!(reads_back(document), "p\n\n-\n");
    // An ordered list after a paragraph in a tight list cannot start at any number but 1 either, so once its leading empty items are removed, it starts at 1 (#152).
    let mut document = build_list(&after_p(ordered(3, vec![vec![], vec![p("a")]])));
    document.normalize();
    assert!(document.is_normalized());
    let outer = document.children(document.root()).next().unwrap();
    let item = document.children(outer).next().unwrap();
    let list = document.children(item).nth(1).unwrap();
    assert!(matches!(
        document.node(list).unwrap().kind,
        EventKind::Start(Tag::List {
            start: Some(1),
            tight: true
        })
    ));
    assert_eq!(document.children(list).count(), 1);
    // A detached list is normalized too.
    let mut document = build_list(&tight(vec![vec![after_p(tight(vec![vec![]]))]]));
    let outer = document.children(document.root()).next().unwrap();
    let item = document.children(outer).next().unwrap();
    let list = document.children(item).next().unwrap();
    document.detach(list).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(
        base_markdown::to_markdown_node(
            &document,
            list,
            &base_markdown::MarkdownOptions::default()
        )
        .unwrap(),
        "- p\n"
    );
}

#[test]
fn an_ordered_list_after_a_paragraph_in_a_tight_list_starts_at_1() {
    use Block::{List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |items: Vec<Vec<Block>>| {
        List(
            None,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let ordered = |start: u32, items: Vec<Vec<Block>>| {
        List(
            Some(start),
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let after_p = |list: Block| tight(vec![vec![p("p"), list]]);
    // Only an ordered list that starts at 1 can interrupt a paragraph, so in a tight list, where no blank line can come between them, `- p\n  3. a` reads back as text `p 3. a` (#152). Normalizing makes such a list start at 1, and keeps its items.
    for (block, expected) in [
        (after_p(ordered(3, vec![vec![p("a")]])), "- p\n  1. a\n"),
        (after_p(ordered(0, vec![vec![p("a")]])), "- p\n  1. a\n"),
        (after_p(ordered(2, vec![vec![p("a")]])), "- p\n  1. a\n"),
        (
            after_p(ordered(u32::MAX, vec![vec![p("a")]])),
            "- p\n  1. a\n",
        ),
        (
            after_p(ordered(3, vec![vec![p("a")], vec![p("b")], vec![]])),
            "- p\n  1. a\n  2. b\n  3.\n",
        ),
        (
            tight(vec![
                vec![p("p"), ordered(3, vec![vec![p("a")]])],
                vec![p("q")],
            ]),
            "- p\n  1. a\n- q\n",
        ),
        (
            ordered(5, vec![vec![p("p"), ordered(5, vec![vec![p("a")]])]]),
            "5. p\n   1. a\n",
        ),
        // Removing the leading empty items of a list after a paragraph can leave it starting at another number, so this runs after that (#150).
        (
            after_p(ordered(2, vec![vec![], vec![p("a")]])),
            "- p\n  1. a\n",
        ),
        (
            after_p(ordered(5, vec![vec![Paragraph(vec![])], vec![p("a")]])),
            "- p\n  1. a\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                tight(vec![vec![]]),
                Separator,
                ordered(4, vec![vec![p("a")]]),
            ]]),
            "- p\n  1. a\n",
        ),
        // Separators and empty paragraphs are not counted, and a list after another block keeps its start.
        (
            tight(vec![vec![
                p("p"),
                Separator,
                Paragraph(vec![]),
                ordered(3, vec![vec![p("a")]]),
                Separator,
                Block::Code,
                ordered(7, vec![vec![p("b")]]),
            ]]),
            "- p\n  1. a\n  ```\n  x\n  ```\n  7. b\n",
        ),
        // In a block quote, and at any depth.
        (
            Quote(vec![after_p(ordered(3, vec![vec![p("a")]]))]),
            "> - p\n>   1. a\n",
        ),
        (
            after_p(tight(vec![vec![p("q"), ordered(3, vec![vec![p("a")]])]])),
            "- p\n  - q\n    1. a\n",
        ),
        (
            tight(vec![vec![Quote(vec![after_p(ordered(
                3,
                vec![vec![p("a")]],
            ))])]]),
            "- > - p\n  >   1. a\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(!document.is_normalized(), "{:?}", expected);
        let mut once = document.clone();
        once.normalize();
        let mut twice = once.clone();
        twice.normalize();
        assert_eq!(canonical(&twice), canonical(&once), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // Shapes that can be written are left as they are: an ordered list that starts at 1, a bullet list, an ordered list after a block other than a paragraph or as an item's first block, and an ordered list after a paragraph in a loose list or outside an item, where a blank line comes between them.
    for (block, expected) in [
        (after_p(ordered(1, vec![vec![p("a")]])), "- p\n  1. a\n"),
        (after_p(tight(vec![vec![p("a")]])), "- p\n  - a\n"),
        (
            tight(vec![vec![ordered(3, vec![vec![p("a")]])]]),
            "- 3. a\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                Block::Code,
                ordered(3, vec![vec![p("a")]]),
            ]]),
            "- p\n  ```\n  x\n  ```\n  3. a\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                Quote(vec![p("q")]),
                ordered(3, vec![vec![p("a")]]),
            ]]),
            "- p\n  > q\n  3. a\n",
        ),
        (
            List(
                None,
                false,
                vec![(true, vec![p("p"), ordered(3, vec![vec![p("a")]])])],
            ),
            "- p\n\n  3. a\n",
        ),
        (
            List(
                None,
                false,
                vec![
                    (true, vec![p("p"), ordered(3, vec![vec![p("a")]])]),
                    (false, vec![p("q")]),
                ],
            ),
            "- p\n\n  3. a\n\n- q\n",
        ),
        (
            Quote(vec![p("p"), ordered(3, vec![vec![p("a")]])]),
            "> p\n>\n> 3. a\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // Outside a list, a blank line is written between a paragraph and an ordered list.
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    append_block(&mut document, root, &p("p"));
    append_block(&mut document, root, &ordered(3, vec![vec![p("a")]]));
    assert!(document.is_normalized());
    assert_eq!(reads_back(document), "p\n\n3. a\n");
    // Parsed ordered lists after a paragraph in a tight item start at 1, also when written `01.` or `1)`, and are left as they are; other numbers are read as text.
    for source in [
        "- p\n  1. a\n",
        "- p\n  01. a\n",
        "- p\n  1) a\n",
        "- p\n  3. a\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
    }
    // In a footnote definition, as in a block quote, a blank line is written between them.
    let source = "[^1]: p\n\n    3. a\n";
    let mut document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    assert!(document.is_normalized());
    let before = canonical(&document);
    document.normalize();
    assert_eq!(canonical(&document), before);
    assert_eq!(to_markdown(&document).unwrap(), source);
}

#[test]
fn a_footnote_definition_in_a_list_item_keeps_the_blank_line_before_a_paragraph() {
    // A footnote definition takes in the blank line after it. A parsed item of a definition and a paragraph records spread for that blank line, as its loose list does (#157), but the paragraph needs it whatever spread the item records, or it would read back as part of the definition (#148). An item with other blocks is then written with a blank line between each of them. The same holds when the definition ends in a block quote or list, or has several paragraphs, and before a definition or table, which cannot interrupt a paragraph either.
    for (source, expected) in [
        ("- [^1]: a\n\n  b\n- c", "- [^1]: a\n\n  b\n\n- c\n"),
        ("- c\n- [^1]: a\n\n  b", "- c\n\n- [^1]: a\n\n  b\n"),
        ("1. [^1]: a\n\n   b\n2. c", "1. [^1]: a\n\n   b\n\n2. c\n"),
        (
            "- x\n  [^1]: a\n\n  b\n- c",
            "- x\n\n  [^1]: a\n\n  b\n\n- c\n",
        ),
        (
            "- [^1]: a\n\n      b\n\n  c\n- d",
            "- [^1]: a\n\n      b\n\n  c\n\n- d\n",
        ),
        ("- [^1]: > q\n\n  b\n- c", "- [^1]: > q\n\n  b\n\n- c\n"),
        (
            "- [^1]: a\n\n      > q\n\n  b\n- c",
            "- [^1]: a\n\n      > q\n\n  b\n\n- c\n",
        ),
        ("- [^1]: - a\n\n  c\n- d", "- [^1]: - a\n\n  c\n\n- d\n"),
        (
            "- [^1]: a\n\n  [r]: /u\n- c",
            "- [^1]: a\n\n  [r]: /u\n\n- c\n",
        ),
        (
            "- [^1]: a\n\n  | x |\n  | - |\n- c",
            "- [^1]: a\n\n  | x |\n  | --- |\n\n- c\n",
        ),
        (
            "- a\n\n  - [^1]: x\n\n    y\n  - z",
            "- a\n\n  - [^1]: x\n\n    y\n\n  - z\n",
        ),
    ] {
        for source in [source, expected] {
            let document = Document::parse(source, &ParseOptions::default()).unwrap();
            assert!(!tight_of_first_list(&document), "{:?}", source);
            assert_eq!(reads_back(document), expected, "{:?}", source);
        }
    }
}

#[test]
fn a_footnote_definition_in_a_list_item_is_written_unchanged_otherwise() {
    // A line after a definition that does not end in a paragraph, or before a block that can interrupt a paragraph, ends the definition, so no blank line is added and a tight list stays tight. Outside a list, a blank line is always written between blocks.
    for (source, tight) in [
        ("- [^1]: # h\n  b\n- c\n", true),
        ("- [^1]:\n  b\n- c\n", true),
        ("- [^1]: | x |\n      | --- |\n  b\n- c\n", true),
        ("- [^1]: a\n  # h\n- c\n", true),
        ("- [^1]: a\n  [^2]: b\n- c\n", true),
        ("- [^1]: a\n  > b\n- c\n", true),
        ("- [^1]: a\n  # h\n\n- c\n", false),
        ("- [^1]: a\n  ```\n  x\n  ```\n\n- c\n", false),
        ("- [^1]: a\n  - b\n\n- c\n", false),
        ("- [^1]: a\n\n  b\n", false),
        ("- a\n\n  - b\n\n  c\n\n- d\n", false),
        ("> [^1]: a\n>\n> b\n", true),
        ("[^1]: a\n\nb\n", true),
        ("[^1]: a\n\n    b\n\nc\n", true),
        ("[^1]: a\n\n# h\n\n```\nx\n```\n\n- b\n\n[^2]: c\n", true),
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        if source.starts_with('-') {
            assert_eq!(tight_of_first_list(&document), tight, "{:?}", source);
        }
        assert_eq!(reads_back(document), source);
    }
}

#[test]
fn a_built_footnote_definition_in_a_list_item_keeps_a_paragraph_after_it() {
    use Block::{Code, Footnote, List, Paragraph, Quote};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    // Whatever spread the item records, a paragraph after a definition that ends in a paragraph is written after a blank line, so it reads back outside the definition.
    for spread in [false, true] {
        let list =
            |first: Vec<Block>| List(None, false, vec![(spread, first), (false, vec![p("c")])]);
        for (blocks, expected) in [
            (
                vec![Footnote(vec![p("a")]), p("b")],
                "- [^1]: a\n\n  b\n\n- c\n",
            ),
            (
                vec![Footnote(vec![p("a"), p("d")]), p("b")],
                "- [^1]: a\n\n      d\n\n  b\n\n- c\n",
            ),
            (
                vec![Footnote(vec![Quote(vec![p("a")])]), p("b")],
                "- [^1]: > a\n\n  b\n\n- c\n",
            ),
            (
                vec![
                    Footnote(vec![List(None, true, vec![(false, vec![p("a")])])]),
                    p("b"),
                ],
                "- [^1]: - a\n\n  b\n\n- c\n",
            ),
        ] {
            assert_eq!(reads_back(build_list(&list(blocks))), expected);
        }
    }
    // A definition that ends in a code block, or is empty, is ended by the paragraph's line, so no blank line is needed between them.
    for (blocks, expected) in [
        (
            vec![Footnote(vec![Code]), p("b")],
            "- [^1]: ```\n      x\n      ```\n  b\n\n- c\n",
        ),
        (vec![Footnote(vec![]), p("b")], "- [^1]:\n  b\n\n- c\n"),
    ] {
        let list = List(None, false, vec![(false, blocks), (false, vec![p("c")])]);
        assert_eq!(reads_back(build_list(&list)), expected);
    }
}

#[test]
fn a_footnote_definition_in_the_only_item_of_a_loose_list_keeps_the_item_spread() {
    use Block::{Footnote, Leaf, List, Paragraph};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let definition = || {
        Leaf(EventKind::Definition {
            label: Text::from("r".to_owned()),
            destination: Text::from("/u".to_owned()),
            title: None,
        })
    };
    // The blank line written after a footnote definition is taken in by the definition when read back, so it does not count as showing that the only item of a loose list is spread: the item's other blocks are still written spread. Without that, an ordered list not starting at 1 or a list with a leading empty item would read back as text in the paragraph, and blocks after an empty definition would be joined differently on the next write.
    for spread in [false, true] {
        for (blocks, expected) in [
            (
                vec![
                    Footnote(vec![p("a")]),
                    p("b"),
                    List(Some(3), true, vec![(false, vec![p("c")])]),
                ],
                "- [^1]: a\n\n  b\n\n  3. c\n",
            ),
            (
                vec![
                    Footnote(vec![p("a")]),
                    p("b"),
                    List(None, true, vec![(false, vec![]), (false, vec![p("c")])]),
                ],
                "- [^1]: a\n\n  b\n\n  -\n  - c\n",
            ),
            (
                vec![Footnote(vec![]), Footnote(vec![p("x")]), definition()],
                "- [^1]:\n\n  [^2]: x\n\n  [r]: /u\n",
            ),
        ] {
            assert_eq!(
                reads_back(build_list(&List(None, false, vec![(spread, blocks)]))),
                expected
            );
        }
    }
    // Parsed, these are written as they were.
    for source in [
        "- [^2]: q\n\n  [^1]: a\n\n  b\n",
        "- [^x]: q\n\n  a\n\n  | \\- |\n  | --- |\n",
        "- [^1]: a\n\n  b\n\n  3. c\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(!tight_of_first_list(&document), "{:?}", source);
        assert_eq!(reads_back(document), source);
    }
    // A table whose header row lazily continues the item is written after a blank line, so it does not join the paragraph before it.
    let source = "- [^x]: q\n\n  a\n| - |\n  | - |\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(
        reads_back(document),
        "- [^x]: q\n\n  a\n\n  | \\- |\n  | --- |\n"
    );
}

/// Clear the recorded spread of each item of the first list, as a hand-built tree may leave it.
fn unset_spread(mut document: Document<String>) -> Document<String> {
    let list = document.children(document.root()).next().unwrap();
    let items: Vec<NodeId> = document.children(list).collect();
    for item in items {
        document.set_syntax(item, None).unwrap();
    }
    document
}

#[test]
fn a_list_that_cannot_interrupt_a_paragraph_in_a_loose_item_is_written_after_a_blank_line() {
    use Block::{Code, List, Paragraph, Quote};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |start: Option<u32>, items: Vec<Vec<Block>>| {
        List(
            start,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    // A list that starts with an empty item, or is ordered and starts at a number other than 1, cannot interrupt a paragraph. In a loose list of more than one item whose items record no spread, nothing wrote a blank line between them, so `- p\n  -` read back as a heading and `- p\n  3. a` as text (#151). The list is loose, so the item is written spread, which renders the same and reads back spread.
    for spread in [None, Some(false), Some(true)] {
        let loose = |first: Vec<Block>| {
            List(
                None,
                false,
                vec![
                    (spread == Some(true), first),
                    (spread == Some(true), vec![p("q")]),
                ],
            )
        };
        for (block, expected) in [
            (
                loose(vec![p("p"), tight(None, vec![vec![]])]),
                "- p\n\n  -\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(Some(3), vec![vec![p("a")]])]),
                "- p\n\n  3. a\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(Some(0), vec![vec![p("a")]])]),
                "- p\n\n  0. a\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(Some(1), vec![vec![]])]),
                "- p\n\n  1.\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(Some(1), vec![vec![], vec![p("a")]])]),
                "- p\n\n  1.\n  2. a\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(None, vec![vec![], vec![p("a")]])]),
                "- p\n\n  -\n  - a\n\n- q\n",
            ),
            (
                loose(vec![p("p"), tight(None, vec![vec![Paragraph(vec![])]])]),
                "- p\n\n  -\n\n- q\n",
            ),
            // The other blocks of the item are written spread too, so it reads back the same.
            (
                loose(vec![p("p"), tight(Some(3), vec![vec![p("a")]]), Code]),
                "- p\n\n  3. a\n\n  ```\n  x\n  ```\n\n- q\n",
            ),
            (
                loose(vec![
                    Code,
                    p("p"),
                    tight(None, vec![vec![]]),
                    tight(Some(1), vec![vec![p("a")]]),
                ]),
                "- ```\n  x\n  ```\n\n  p\n\n  -\n\n  1. a\n\n- q\n",
            ),
            (
                List(
                    None,
                    false,
                    vec![
                        (spread == Some(true), vec![p("q")]),
                        (
                            spread == Some(true),
                            vec![p("p"), tight(None, vec![vec![]])],
                        ),
                    ],
                ),
                "- q\n\n- p\n\n  -\n",
            ),
            (
                List(
                    Some(1),
                    false,
                    vec![
                        (
                            spread == Some(true),
                            vec![p("p"), tight(Some(3), vec![vec![p("a")]])],
                        ),
                        (spread == Some(true), vec![p("q")]),
                    ],
                ),
                "1. p\n\n   3. a\n\n2. q\n",
            ),
            (
                Quote(vec![loose(vec![p("p"), tight(None, vec![vec![]])])]),
                "> - p\n>\n>   -\n>\n> - q\n",
            ),
            (
                Quote(vec![loose(vec![
                    p("p"),
                    tight(Some(3), vec![vec![p("a")]]),
                ])]),
                "> - p\n>\n>   3. a\n>\n> - q\n",
            ),
            // A list that can interrupt a paragraph is written as before.
            (
                loose(vec![p("p"), tight(None, vec![vec![p("a")]])]),
                if spread == Some(true) {
                    "- p\n\n  - a\n\n- q\n"
                } else {
                    "- p\n  - a\n\n- q\n"
                },
            ),
            (
                loose(vec![p("p"), tight(Some(1), vec![vec![p("a")], vec![]])]),
                if spread == Some(true) {
                    "- p\n\n  1. a\n  2.\n\n- q\n"
                } else {
                    "- p\n  1. a\n  2.\n\n- q\n"
                },
            ),
            (
                loose(vec![Code, tight(Some(3), vec![vec![p("a")]])]),
                if spread == Some(true) {
                    "- ```\n  x\n  ```\n\n  3. a\n\n- q\n"
                } else {
                    "- ```\n  x\n  ```\n  3. a\n\n- q\n"
                },
            ),
        ] {
            let mut document = build_list(&block);
            if spread.is_none() && matches!(block, Block::List(..)) {
                document = unset_spread(document);
            }
            assert_eq!(reads_back(document), expected, "{:?}", spread);
        }
        // The only item of a loose list is written spread, as before, also with a block after the list, which is already written after a blank line.
        for (blocks, expected) in [
            (
                vec![p("p"), tight(Some(3), vec![vec![p("a")]])],
                "- p\n\n  3. a\n",
            ),
            (vec![p("p"), tight(None, vec![vec![]])], "- p\n\n  -\n"),
            (
                vec![p("p"), tight(Some(3), vec![vec![p("a")]]), p("b")],
                "- p\n\n  3. a\n\n  b\n",
            ),
        ] {
            let mut document = build_list(&List(None, false, vec![(spread == Some(true), blocks)]));
            if spread.is_none() {
                document = unset_spread(document);
            }
            assert_eq!(reads_back(document), expected, "{:?}", spread);
        }
    }
    // Parsed, an item with a blank line before such a list records spread, and is written as it was. One where the list can interrupt the paragraph stays as it was too.
    for (source, expected) in [
        ("- p\n\n  -\n\n- q\n", "- p\n\n  -\n\n- q\n"),
        ("- p\n\n  3. a\n\n- q\n", "- p\n\n  3. a\n\n- q\n"),
        ("- p\n\n  3. a\n- q\n", "- p\n\n  3. a\n\n- q\n"),
        (
            "- p\n\n  1.\n  2. a\n\n- q\n",
            "- p\n\n  1.\n  2. a\n\n- q\n",
        ),
        ("> - p\n>\n>   -\n>\n> - q\n", "> - p\n>\n>   -\n>\n> - q\n"),
        ("- p\n  - a\n\n- q\n", "- p\n  - a\n\n- q\n"),
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        if !source.starts_with('>') {
            assert!(!tight_of_first_list(&document), "{:?}", source);
        }
        assert_eq!(reads_back(document), expected, "{:?}", source);
    }
}

#[test]
fn a_tight_list_after_a_paragraph_in_a_tight_item_stays_tight() {
    use Block::{List, Paragraph};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    // In a tight list, normalizing removes the leading empty items and renumbers the list, so no blank line is written and the list stays tight (#150, #153).
    for (start, items, expected) in [
        (None, vec![vec![], vec![p("a")]], "- p\n  - a\n- q\n"),
        (Some(3), vec![vec![p("a")]], "- p\n  1. a\n- q\n"),
    ] {
        let nested = List(
            start,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        );
        let list = List(
            None,
            true,
            vec![(false, vec![p("p"), nested]), (false, vec![p("q")])],
        );
        let document = reads_back(build_list(&list));
        assert_eq!(document, expected);
        assert!(tight_of_first_list(
            &Document::parse(document.as_str(), &ParseOptions::default()).unwrap()
        ));
    }
}

#[test]
fn a_spread_item_keeps_the_comment_before_indented_code() {
    // An item written spread because of a list that cannot interrupt a paragraph, here an ordered list that starts at 3, still writes `<!---->` between the list and indented code after it, so the code is not read back as part of the list's last item.
    // With `tight_definitions`, which does not change this join, it is the same (#160).
    for (source, expected, tight_definitions) in [false, true].iter().flat_map(|&tight| {
        [
            (
                "- p\n\n  3. a\n\n  ```\n  x\n  ```\n\n- q\n",
                "- p\n\n  3. a\n\n  <!---->\n\n      x\n\n- q\n",
                tight,
            ),
            (
                "- p\n\n  3. a\n\n  ```\n  x\n  ```\n",
                "- p\n\n  3. a\n\n  <!---->\n\n      x\n",
                tight,
            ),
        ]
    }) {
        let options = base_markdown::MarkdownOptions {
            fences: false,
            tight_definitions,
            ..base_markdown::MarkdownOptions::default()
        };
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        document.normalize();
        let markdown = base_markdown::to_markdown_with_options(&document, &options).unwrap();
        assert_eq!(markdown, expected, "{:?}", source);
        let reparsed = Document::parse(markdown.as_str(), &ParseOptions::default()).unwrap();
        // The comment reads back as an HTML block, which the default HTML escapes; apart from it, the code block and the nested list read back as they were.
        assert_eq!(
            rendered(&reparsed).replace("&lt;!----&gt;\n", ""),
            rendered(&document),
            "{:?}",
            source
        );
    }
}

#[test]
fn a_spread_item_keeps_tight_definitions() {
    // With `tight_definitions`, adjacent definitions in a spread item are joined by a line ending, and its other blocks by blank lines, which show its spread. When definitions are its only blocks, nothing else would, so they are joined by blank lines too.
    let options = base_markdown::MarkdownOptions {
        tight_definitions: true,
        ..base_markdown::MarkdownOptions::default()
    };
    for (source, expected) in [
        (
            "- p\n\n  3. a\n\n  [r]: /u\n  [s]: /v\n\n- q\n",
            "- p\n\n  3. a\n\n  [r]: /u\n  [s]: /v\n\n- q\n",
        ),
        (
            "- p\n\n    -\n  [r]: /u\n  [s]: /v",
            "- p\n\n  -\n\n  [r]: /u\n  [s]: /v\n",
        ),
        (
            "- [r]: /u\n\n  [s]: /v\n\n  p\n- q\n",
            "- [r]: /u\n  [s]: /v\n\n  p\n\n- q\n",
        ),
        (
            "- [r]: /u\n\n  [s]: /v\n- q\n",
            "- [r]: /u\n\n  [s]: /v\n\n- q\n",
        ),
        ("- [r]: /u\n\n  [s]: /v\n", "- [r]: /u\n\n  [s]: /v\n"),
        (
            "- [r]: /u\n  [s]: /v\n\n- q\n",
            "- [r]: /u\n  [s]: /v\n\n- q\n",
        ),
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(
            reads_back_with(document, &ParseOptions::default(), &options),
            expected,
            "{:?}",
            source
        );
    }
}

#[test]
fn tight_definitions_joins_other_blocks_as_without_it() {
    // Only adjacent definitions are joined differently, so a tight list stays tight, and with `fences = false`, `<!---->` is still written before indented code after a list (#160).
    let tight = base_markdown::MarkdownOptions {
        tight_definitions: true,
        ..base_markdown::MarkdownOptions::default()
    };
    for source in [
        "- c\n- c\n",
        "1. a\n2. b\n",
        "- a\n  # h\n",
        "- [r]: /u\n  [s]: /v\n- q\n",
        "> [r]: /u\n> [s]: /v\n\np\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(
            reads_back_with(document, &ParseOptions::default(), &tight),
            source
        );
    }
    // An item of a loose list that records no spread is written spread when it writes a blank line between two of its blocks, here two paragraphs, so the join after a definition is a blank line too and the item is written the same again (#157).
    use Block::{Leaf, List, Paragraph};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let definition = Leaf(EventKind::Definition {
        label: Text::from("c".to_owned()),
        destination: Text::from("/u".to_owned()),
        title: None,
    });
    let nested = List(None, true, vec![(false, vec![p("x")])]);
    for (blocks, expected) in [
        (
            vec![definition, p("p"), p("r")],
            "- [c]: /u\n\n  p\n\n  r\n\n- q\n",
        ),
        (vec![p("b"), p("b"), nested], "- b\n\n  b\n\n  - x\n\n- q\n"),
    ] {
        let list = List(None, false, vec![(false, blocks), (false, vec![p("q")])]);
        assert_eq!(
            reads_back_with(build_list(&list), &ParseOptions::default(), &tight),
            expected
        );
    }
    let source = "- p\n  - a\n\n  ```\n  x\n  ```\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    for options in [without_fences(), tight] {
        assert_eq!(
            base_markdown::to_markdown_with_options(
                &document,
                &base_markdown::MarkdownOptions {
                    fences: false,
                    ..options
                }
            )
            .unwrap(),
            "- p\n\n  - a\n\n  <!---->\n\n      x\n"
        );
    }
}

/// As `reads_back`, but serialized with `options` and read back with `parse`.
fn reads_back_with<S: AsRef<str> + Clone>(
    mut document: Document<S>,
    parse: &ParseOptions,
    options: &base_markdown::MarkdownOptions,
) -> String {
    document.normalize();
    assert!(document.is_normalized());
    let markdown = base_markdown::to_markdown_with_options(&document, options).unwrap();
    let mut reparsed = Document::parse(markdown.as_str(), parse).unwrap();
    reparsed.normalize();
    assert!(reparsed.is_normalized());
    assert_eq!(rendered(&reparsed), rendered(&document), "{markdown:?}");
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    assert_eq!(
        base_markdown::to_markdown_with_options(&reparsed, options).unwrap(),
        markdown
    );
    markdown
}

fn without_fences() -> base_markdown::MarkdownOptions {
    base_markdown::MarkdownOptions {
        fences: false,
        ..base_markdown::MarkdownOptions::default()
    }
}

#[test]
fn code_that_starts_a_footnote_definition_starts_on_the_next_line_without_fences() {
    // Indented code cannot start on a definition's first line, where `[^a]:     a` reads back as a paragraph, so it starts on the next line, indented like the lines after it.
    for parse in [ParseOptions::default(), ParseOptions::gfm()] {
        for (source, expected) in [
            // Issue #142.
            (
                "[^a]: ~~~\n    a\n    ~~~\n\nb[^a]",
                "[^a]:\n        a\n\nb[^a]\n",
            ),
            ("[^1]:\n        x\n", "[^1]:\n        x\n"),
            ("- [^1]:\n          x\n- a\n", "- [^1]:\n          x\n- a\n"),
            ("> [^1]:\n>         x\n", "> [^1]:\n>         x\n"),
            (
                "[^1]: [^2]:\n            x\n",
                "[^1]: [^2]:\n            x\n",
            ),
            ("[^1]:\n        x\n\n    a\n", "[^1]:\n        x\n\n    a\n"),
            (
                "- [^a]: ~~~\n      a\n      ~~~\n\n- b[^a]\n",
                "- [^a]:\n          a\n\n- b[^a]\n",
            ),
            (
                "[^a]: ~~~\n    a\n\n    b\n    ~~~\n",
                "[^a]:\n        a\n\n        b\n",
            ),
            // Code that indented code cannot write is fenced as before.
            ("[^a]: ~~~\n    ~~~\n", "[^a]: ```\n    ```\n"),
            (
                "[^a]: ~~~\n\n    a\n    ~~~\n",
                "[^a]: ```\n\n    a\n    ```\n",
            ),
            (
                "[^a]: ~~~ rs\n    a\n    ~~~\n",
                "[^a]: ```rs\n    a\n    ```\n",
            ),
        ] {
            let document = Document::parse(source, &parse).unwrap();
            assert_eq!(
                reads_back_with(document, &parse, &without_fences()),
                expected,
                "{source:?}"
            );
        }
    }
}

#[test]
fn two_code_blocks_that_start_a_footnote_definition_keep_the_comment_without_fences() {
    // Two indented code blocks in a row are separated by `<!---->`, also at the start of a definition. The comment reads back as an HTML block, which is the older `<!---->` behaviour; apart from it, the code blocks read back as they were.
    let parse = ParseOptions::default();
    let source = "[^1]:\n        x\n\n    ```\n    y\n    ```\n";
    let expected = "[^1]:\n        x\n\n    <!---->\n\n        y\n";
    let mut document = Document::parse(source, &parse).unwrap();
    document.normalize();
    let markdown = base_markdown::to_markdown_with_options(&document, &without_fences()).unwrap();
    assert_eq!(markdown, expected);
    let reparsed = Document::parse(markdown.as_str(), &parse).unwrap();
    assert_eq!(
        rendered(&reparsed).replace("&lt;!----&gt;\n", ""),
        rendered(&document)
    );
}

#[test]
fn code_after_a_footnote_definition_is_fenced_without_fences() {
    // Indented code after a footnote definition would continue the definition, which takes in indented lines after a blank line, so it is fenced whatever `fences` says.
    let parse = ParseOptions::default();
    for (source, expected) in [
        ("[^1]: f\n\n~~~\nx\n~~~\n", "[^1]: f\n\n```\nx\n```\n"),
        (
            "> [^1]: f\n>\n> ~~~\n> x\n> ~~~\n",
            "> [^1]: f\n>\n> ```\n> x\n> ```\n",
        ),
        (
            "- p\n\n  3. a\n\n  [^1]: f\n\n  ```\n  x\n  ```\n\n- q\n",
            "- p\n\n  3. a\n\n  [^1]: f\n\n  ```\n  x\n  ```\n\n- q\n",
        ),
        // Only the code directly after the definition is fenced, and indented code after the fenced code needs no comment between them.
        (
            "[^1]: f\n\n~~~\nx\n~~~\n~~~\ny\n~~~\n",
            "[^1]: f\n\n```\nx\n```\n\n    y\n",
        ),
    ] {
        let document = Document::parse(source, &parse).unwrap();
        assert_eq!(
            reads_back_with(document, &parse, &without_fences()),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn built_code_in_or_after_a_footnote_definition_reads_back_without_fences() {
    use Block::{Code, Footnote, List, Paragraph, Quote};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let parse = ParseOptions::default();
    let list = |spread: bool, first: Vec<Block>| {
        List(None, false, vec![(spread, first), (false, vec![p("c")])])
    };
    for (block, expected) in [
        // Issue #157: this was written `- [^1]: a\n      x\n\n- c\n`, where `x` joins the definition.
        (
            list(false, vec![Footnote(vec![p("a")]), Code]),
            "- [^1]: a\n  ```\n  x\n  ```\n\n- c\n",
        ),
        (
            list(false, vec![Footnote(vec![Code]), p("b")]),
            "- [^1]:\n          x\n  b\n\n- c\n",
        ),
        (
            list(false, vec![Footnote(vec![p("a")]), Code, Code]),
            "- [^1]: a\n  ```\n  x\n  ```\n      x\n\n- c\n",
        ),
        // The blank line between the two code blocks shows the item is spread, so it reads back spread.
        (
            list(true, vec![Footnote(vec![p("a")]), Code, Code]),
            "- [^1]: a\n\n  ```\n  x\n  ```\n\n      x\n\n- c\n",
        ),
    ] {
        assert_eq!(
            reads_back_with(build_list(&block), &parse, &without_fences()),
            expected
        );
    }
    for (block, expected) in [
        (Footnote(vec![Code]), "[^1]:\n        x\n"),
        (
            Quote(vec![Footnote(vec![p("a")]), Code]),
            "> [^1]: a\n>\n> ```\n> x\n> ```\n",
        ),
        (
            Quote(vec![Footnote(vec![Code, p("b")])]),
            "> [^1]:\n>         x\n>\n>     b\n",
        ),
    ] {
        assert_eq!(
            reads_back_with(build_list(&block), &parse, &without_fences()),
            expected
        );
    }
    // Code elsewhere is still indented.
    assert_eq!(
        reads_back_with(
            build_list(&Quote(vec![p("a"), Code])),
            &parse,
            &without_fences()
        ),
        "> a\n>\n>     x\n"
    );
}

#[test]
fn paragraphs_in_a_row_in_a_tight_item_are_joined() {
    use Block::{Code, List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let code = |value: &'static str| {
        Paragraph(vec![EventKind::Code {
            value: Text::Literal(value),
            language: None,
        }])
    };
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |items: Vec<Vec<Block>>| {
        List(
            None,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    // No blank line can come between two paragraphs in an item of a tight list, so `- a\n\n  b` read back loose (#154). Normalizing joins them into the first, with a soft break between them, which renders the same in a tight list, as it has no `<p>`.
    for (block, expected) in [
        (tight(vec![vec![p("a"), p("b")]]), "- a\n  b\n"),
        (tight(vec![vec![p("a"), Separator, p("b")]]), "- a\n  b\n"),
        (
            tight(vec![vec![
                p("a"),
                Separator,
                Paragraph(vec![]),
                Separator,
                p("b"),
            ]]),
            "- a\n  b\n",
        ),
        (
            tight(vec![vec![
                p("a"),
                Paragraph(vec![EventKind::SoftBreak(Text::Literal("\n"))]),
                p("b"),
            ]]),
            "- a\n  b\n",
        ),
        (
            tight(vec![vec![p("a"), p("b"), p("c")], vec![p("d"), p("e")]]),
            "- a\n  b\n  c\n- d\n  e\n",
        ),
        // A list removed after a paragraph leaves the paragraphs on either side of it in a row (#129).
        (
            tight(vec![vec![p("a"), tight(vec![vec![]]), Separator, p("b")]]),
            "- a\n  b\n",
        ),
        // Other blocks end a run.
        (
            tight(vec![vec![
                p("a"),
                p("b"),
                Code,
                Separator,
                p("c"),
                Separator,
                p("d"),
                tight(vec![vec![p("e")]]),
            ]]),
            "- a\n  b\n  ```\n  x\n  ```\n  c\n  d\n  - e\n",
        ),
        // The soft break keeps what meets apart: inline code is not joined.
        (tight(vec![vec![code("a"), code("b")]]), "- `a`\n  `b`\n"),
        // Whitespace that meets the soft break is encoded, as at the edges of a paragraph.
        (
            tight(vec![vec![p("a "), p(" b")]]),
            "- a&#x20;\n  &#x20;b\n",
        ),
        // In a block quote, and at any depth.
        (
            Quote(vec![tight(vec![vec![p("a"), p("b")]])]),
            "> - a\n>   b\n",
        ),
        (
            tight(vec![vec![p("x"), tight(vec![vec![p("a"), p("b")]])]]),
            "- x\n  - a\n    b\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(!document.is_normalized(), "{:?}", expected);
        let mut once = document.clone();
        once.normalize();
        let mut twice = once.clone();
        twice.normalize();
        assert_eq!(canonical(&twice), canonical(&once), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // The second paragraph's children move into the first after a new soft break, and it and the separator between them are detached.
    let mut document = build_list(&tight(vec![vec![p("a"), Separator, p("b")]]));
    let item = document.descendants(document.root()).nth(2).unwrap();
    let [first, between, second] = document.children(item).collect::<Vec<_>>()[..] else {
        unreachable!("the item has three children");
    };
    document.normalize();
    assert_eq!(document.children(item).collect::<Vec<_>>(), [first]);
    let kinds: Vec<_> = document
        .children(first)
        .map(|id| document.node(id).unwrap().kind.clone())
        .collect();
    assert!(matches!(
        &kinds[..],
        [
            EventKind::Text(_),
            EventKind::SoftBreak(Text::Literal("\n")),
            EventKind::Text(_)
        ]
    ));
    assert_eq!(document.node(between).unwrap().parent(), None);
    assert_eq!(document.node(second).unwrap().parent(), None);
    assert_eq!(document.children(second).next(), None);
    // Paragraphs in a row are left as they are where a blank line can be written between them: in a loose list, a block quote and the document.
    for (block, expected) in [
        (
            List(None, false, vec![(true, vec![p("a"), p("b")])]),
            "- a\n\n  b\n",
        ),
        (
            List(
                None,
                false,
                vec![(false, vec![p("a"), p("b")]), (false, vec![p("c")])],
            ),
            "- a\n\n  b\n\n- c\n",
        ),
        (Quote(vec![p("a"), p("b")]), "> a\n>\n> b\n"),
    ] {
        let document = build_list(&block);
        assert!(document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    append_block(&mut document, root, &p("a"));
    append_block(&mut document, root, &p("b"));
    assert!(document.is_normalized());
    assert_eq!(reads_back(document), "a\n\nb\n");
}

#[test]
fn blocks_that_need_a_blank_line_in_a_tight_item_make_the_list_loose() {
    use Block::{Code, Footnote, List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |items: Vec<Vec<Block>>| {
        List(
            None,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let quote = |value: &str| Quote(vec![p(value)]);
    // In an item of a tight list, a paragraph after a list or footnote definition that ends in a paragraph would continue that paragraph without a blank line, two block quotes in a row would read back as one, and a paragraph after a block quote that ends in such a list or definition would continue it, as the line of `>` that ends a quote's paragraph is a blank line in the list or definition. Each needs a blank line, which makes the list loose, so normalizing makes it loose (#154). Its other items' paragraphs in a row stay apart, as a loose list can write them.
    for (block, expected) in [
        (
            tight(vec![
                vec![tight(vec![vec![p("a")]]), Separator, p("p")],
                vec![p("q"), Separator, p("r")],
            ]),
            "- - a\n\n  p\n\n- q\n\n  r\n",
        ),
        (
            tight(vec![vec![quote("a"), Separator, quote("b")]]),
            "- > a\n\n  > b\n",
        ),
        (
            tight(vec![vec![Quote(vec![]), Separator, quote("b")]]),
            "- >\n\n  > b\n",
        ),
        (
            tight(vec![vec![quote("a"), Separator, quote("b")], vec![p("q")]]),
            "- > a\n\n  > b\n\n- q\n",
        ),
        (
            tight(vec![vec![p("q")], vec![quote("a"), Separator, quote("b")]]),
            "- q\n\n- > a\n\n  > b\n",
        ),
        (
            tight(vec![vec![
                tight(vec![vec![p("a")], vec![Quote(vec![p("b")])]]),
                Separator,
                p("p"),
            ]]),
            "- - a\n  - > b\n\n  p\n",
        ),
        (
            tight(vec![vec![Footnote(vec![p("a")]), Separator, p("p")]]),
            "- [^1]: a\n\n  p\n",
        ),
        (
            tight(vec![vec![
                Quote(vec![tight(vec![vec![p("a")]])]),
                Separator,
                p("p"),
            ]]),
            "- > - a\n\n  p\n",
        ),
        (
            tight(vec![vec![
                Quote(vec![Footnote(vec![p("a")])]),
                Separator,
                p("p"),
            ]]),
            "- > [^1]: a\n\n  p\n",
        ),
        // Separators and empty paragraphs are not counted.
        (
            tight(vec![vec![
                quote("a"),
                Separator,
                Paragraph(vec![]),
                Separator,
                quote("b"),
            ]]),
            "- > a\n\n  > b\n",
        ),
        // Only the list that holds the item is made loose.
        (
            tight(vec![vec![
                p("x"),
                Separator,
                tight(vec![vec![quote("a"), Separator, quote("b")]]),
            ]]),
            "- x\n  - > a\n\n    > b\n",
        ),
        (
            Quote(vec![tight(vec![vec![quote("a"), Separator, quote("b")]])]),
            "> - > a\n>\n>   > b\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(!document.is_normalized(), "{:?}", expected);
        let mut once = document.clone();
        once.normalize();
        let mut twice = once.clone();
        twice.normalize();
        assert_eq!(canonical(&twice), canonical(&once), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // The list and every paragraph of its items are made loose.
    let mut document = build_list(&tight(vec![
        vec![quote("a"), Separator, quote("b")],
        vec![p("q")],
    ]));
    document.normalize();
    let list = document.children(document.root()).next().unwrap();
    assert!(matches!(
        document.node(list).unwrap().kind,
        EventKind::Start(Tag::List { tight: false, .. })
    ));
    assert!(document.descendants(list).all(|id| !matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Paragraph { tight: true })
    )));
    // Shapes that can be written tight are left as they are: a paragraph after a list that does not end in a paragraph, such as one whose last item is empty, which the parser gives for `* *\n  p`, on the next line; a paragraph after a block quote that ends in a paragraph, through block quotes in it, which a line of `>` ends, or in a list in a block quote in it, which that line closes; blocks other than a paragraph after a list or block quote; and a paragraph after a block quote that ends in a list that does not end in a paragraph.
    for (block, expected) in [
        (
            tight(vec![vec![tight(vec![vec![]]), Separator, p("p")]]),
            "- -\n  p\n",
        ),
        (
            tight(vec![vec![
                tight(vec![vec![p("a")], vec![Code]]),
                Separator,
                p("p"),
            ]]),
            "- - a\n  - ```\n    x\n    ```\n  p\n",
        ),
        (
            tight(vec![vec![quote("a"), Separator, p("p")]]),
            "- > a\n  >\n  p\n",
        ),
        (
            tight(vec![vec![Quote(vec![quote("a")]), Separator, p("p")]]),
            "- > > a\n  >\n  p\n",
        ),
        (
            tight(vec![vec![
                Quote(vec![Quote(vec![tight(vec![vec![p("a")]])])]),
                Separator,
                p("p"),
            ]]),
            "- > > - a\n  >\n  p\n",
        ),
        (
            tight(vec![vec![tight(vec![vec![p("a")]]), Code]]),
            "- - a\n  ```\n  x\n  ```\n",
        ),
        (
            tight(vec![vec![quote("a"), Code]]),
            "- > a\n  ```\n  x\n  ```\n",
        ),
        (
            tight(vec![vec![
                Quote(vec![tight(vec![vec![Code]])]),
                Separator,
                p("p"),
            ]]),
            "- > - ```\n  >   x\n  >   ```\n  p\n",
        ),
        (
            tight(vec![vec![p("p"), Separator, quote("a"), Separator, p("q")]]),
            "- p\n  > a\n  >\n  q\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    for source in ["* *\n  p\n", "- 1.\n  p\n", "- - a\n  -\n  p\n"] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        assert!(tight_of_first_list(&document), "{:?}", source);
        reads_back(document);
    }
    // In a loose list, these are written with a blank line between them. An item that does not record spread is written spread when it holds two block quotes in a row, as the blank line between them makes it read back spread.
    for (block, expected) in [
        (
            List(
                None,
                false,
                vec![
                    (false, vec![quote("a"), Separator, quote("b"), Code]),
                    (false, vec![p("q")]),
                ],
            ),
            "- > a\n\n  > b\n\n  ```\n  x\n  ```\n\n- q\n",
        ),
        (
            List(
                None,
                false,
                vec![(false, vec![quote("a"), Separator, quote("b")])],
            ),
            "- > a\n\n  > b\n",
        ),
        (
            List(
                None,
                false,
                vec![
                    (false, vec![tight(vec![vec![p("a")]]), Separator, p("p")]),
                    (false, vec![p("q")]),
                ],
            ),
            "- - a\n\n  p\n\n- q\n",
        ),
        // A blank line is written after a list in a loose list whatever it ends in, as before.
        (
            List(
                None,
                false,
                vec![
                    (false, vec![tight(vec![vec![]]), Separator, p("p")]),
                    (false, vec![p("q")]),
                ],
            ),
            "- -\n\n  p\n\n- q\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
}

#[test]
fn a_paragraph_after_a_block_quote_in_an_item_is_written_after_a_line_of_quote_marker() {
    // In an item that is not written spread, a paragraph directly after a block quote that ends in a paragraph would continue it lazily: `- > a\n  p` reads back as one quote of `a p` (#154). A line of `>` alone ends the quote's paragraph without a blank line in the item, so a tight list stays tight. This holds for a table, a definition and a setext heading, which would continue it too, and for block quotes and lists in the item.
    for source in [
        "- > a\n  >\n  p\n",
        "- > a\n  >\n  p\n- q\n",
        "- > > a\n  >\n  p\n",
        "- > a\n  >\n  | a |\n  | --- |\n",
        "- > a\n  >\n  [x]: y\n",
        "> - > a\n>   >\n>   p\n",
        "- q\n  - > a\n    >\n    p\n",
        "1. > a\n   >\n   p\n",
        // In a loose list, an item that does not record spread is written the same.
        "- > a\n  >\n  p\n\n- q\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        assert_eq!(reads_back(document), source);
    }
    // A spread item and the only item of a loose list are written with a blank line instead. A line of `>` alone after a list in the quote is a blank line in that list's last item, which the parser counts as a blank line between the item's blocks, so that item records spread (#157).
    for source in [
        "- > a\n\n  p\n",
        "- > - a\n\n  p\n\n- q\n",
        "- > a\n\n  p\n\n  ```\n  x\n  ```\n\n- q\n",
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        assert_eq!(reads_back(document), source);
    }
    let document = Document::parse("- > - a\n  >\n  p\n\n- q\n", &ParseOptions::default()).unwrap();
    assert_eq!(reads_back(document), "- > - a\n\n  p\n\n- q\n");
    // A heading of one line is written ATX, which is not continued, so nothing is written between them.
    let source = "- > a\n  >\n  b\n  -\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(reads_back(document), "- > a\n  ## b\n");
    // The line is indented as the item's other lines are.
    for (indent, expected) in [
        (base_markdown::IndentOptions::Tab, "-   > a\n    >\n    p\n"),
        (base_markdown::IndentOptions::Mixed, "- > a\n  >\n  p\n"),
    ] {
        let options = base_markdown::MarkdownOptions {
            list_item_indent: indent,
            ..base_markdown::MarkdownOptions::default()
        };
        let document = Document::parse("- > a\n  >\n  p\n", &ParseOptions::default()).unwrap();
        assert_eq!(
            reads_back_with(document, &ParseOptions::default(), &options),
            expected
        );
    }
    // With `tight_definitions`, blocks other than adjacent definitions are joined as without it, here by a blank line, since the item is spread.
    let options = base_markdown::MarkdownOptions {
        tight_definitions: true,
        ..base_markdown::MarkdownOptions::default()
    };
    let document = Document::parse("- > a\n\n  p\n", &ParseOptions::default()).unwrap();
    assert_eq!(
        reads_back_with(document, &ParseOptions::default(), &options),
        "- > a\n\n  p\n"
    );
}

#[test]
fn a_link_reference_definition_ending_a_block_in_a_tight_item_is_continued_like_a_paragraph() {
    use Block::{Footnote, Leaf, List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |blocks: Vec<Block>| List(None, true, vec![(false, blocks)]);
    let definition = || {
        Leaf(EventKind::Definition {
            label: Text::from("d".to_owned()),
            destination: Text::from("u".to_owned()),
            title: None,
        })
    };
    // A line after a link reference definition continues it lazily like a paragraph: as its title, `- - [d]: u\n  "t"` reads back with the paragraph gone, and as a paragraph in the nested item, `- - [d]: u\n  p` moves `p` into it. So a paragraph after a list or footnote definition ending in a definition makes the list loose, and one after a block quote ending in a definition is written after a line of `>`.
    for (block, expected) in [
        (
            tight(vec![tight(vec![definition()]), Separator, p("p")]),
            "- - [d]: u\n\n  p\n",
        ),
        (
            tight(vec![tight(vec![definition()]), Separator, p("\"t\"")]),
            "- - [d]: u\n\n  \"t\"\n",
        ),
        (
            tight(vec![Footnote(vec![definition()]), Separator, p("p")]),
            "- [^1]: [d]: u\n\n  p\n",
        ),
        (
            tight(vec![
                Quote(vec![tight(vec![definition()])]),
                Separator,
                p("p"),
            ]),
            "- > - [d]: u\n\n  p\n",
        ),
        (
            tight(vec![Quote(vec![definition()]), Separator, p("p")]),
            "- > [d]: u\n  >\n  p\n",
        ),
        (
            tight(vec![Quote(vec![definition()]), Separator, p("\"t\"")]),
            "- > [d]: u\n  >\n  \"t\"\n",
        ),
    ] {
        assert_eq!(reads_back(build_list(&block)), expected);
    }
    for source in ["- > [d]: u\n  >\n  p\n", "- - [d]: u\n\n  p\n"] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert!(document.is_normalized(), "{:?}", source);
        assert_eq!(reads_back(document), source);
    }
}

/// The blocks before a block in a tight item for the tests of #168, each with how it is written there.
fn blocks_before() -> Vec<(Block, &'static str)> {
    use Block::{Footnote, List, Paragraph, Quote};
    let p = || Paragraph(vec![EventKind::Text(Text::from("a".to_owned()))]);
    let tight = |blocks: Vec<Block>| List(None, true, vec![(false, blocks)]);
    vec![
        (p(), "a"),
        (tight(vec![p()]), "- a"),
        (tight(vec![definition_block()]), "- [d]: u"),
        (Footnote(vec![p()]), "[^1]: a"),
        (Quote(vec![tight(vec![p()])]), "> - a"),
        (Quote(vec![Footnote(vec![p()])]), "> [^1]: a"),
    ]
}

fn definition_block() -> Block {
    Block::Leaf(EventKind::Definition {
        label: Text::from("d".to_owned()),
        destination: Text::from("u".to_owned()),
        title: None,
    })
}

/// A tight list of an item of `blocks` and an item `q`.
fn tight_before_q(blocks: Vec<Block>) -> Block {
    let q = Block::Paragraph(vec![EventKind::Text(Text::from("q".to_owned()))]);
    Block::List(None, true, vec![(false, blocks), (false, vec![q])])
}

#[test]
fn tables_definitions_and_setext_headings_that_need_a_blank_line_in_a_tight_item_make_the_list_loose(
) {
    use Block::{Heading, Leaf, Paragraph, Quote, Separator, Table};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    // Only a setext heading can hold a line ending.
    let setext_only = || {
        Heading(vec![
            text("x"),
            EventKind::SoftBreak(Text::Literal("\n")),
            text("y"),
        ])
    };
    let gfm = ParseOptions::gfm();
    // A definition or setext heading cannot interrupt a paragraph, and after a nested list or footnote definition that ends in one, through a block quote around it, a table, definition or setext heading would continue it lazily, as a paragraph would (#154). Without a blank line, which makes the item read back spread, each would read back as part of that paragraph, so the serializer writes one, and normalizing makes the list loose, as it does for a paragraph there, whatever the serializer's options (#168). The rule leads each item, so that one of only a footnote definition and a definition renders the same once read back.
    let afters = || {
        vec![
            (
                Leaf(EventKind::Definition {
                    label: Text::from("e".to_owned()),
                    destination: Text::from("u".to_owned()),
                    title: None,
                }),
                "[e]: u",
            ),
            (setext_only(), "x\n  y\n  ="),
            (Table, "| x |\n  | --- |"),
        ]
    };
    for (left, (_, before)) in blocks_before().into_iter().enumerate() {
        // A table interrupts a paragraph.
        for (right, (_, after)) in afters().into_iter().enumerate() {
            if left == 0 && right == 2 {
                continue;
            }
            let block = || {
                let left = blocks_before().swap_remove(left).0;
                let right = afters().swap_remove(right).0;
                tight_before_q(vec![Leaf(EventKind::Rule), left, Separator, right])
            };
            assert!(
                !build_list(&block()).is_normalized(),
                "{} {}",
                before,
                after
            );
            let mut document = build_list(&block());
            document.normalize();
            assert!(!tight_of_first_list(&document));
            assert_eq!(
                reads_back_with(document, &gfm, &base_markdown::MarkdownOptions::default()),
                format!("- ***\n\n  {}\n\n  {}\n\n- q\n", before, after)
            );
        }
    }
    // These can be written on the next line, so the list stays tight: a block after a list that ends in a heading, which no line continues (#157), a table after a paragraph, which it interrupts, and blocks after a block quote that ends in a paragraph, after a line of `>`, or after a definition.
    let h = || Heading(vec![text("h")]);
    for (blocks, expected) in [
        (
            vec![tight_list_of(h()), Separator, Paragraph(vec![text("b")])],
            "- - # h\n  b\n- q\n",
        ),
        (
            vec![tight_list_of(h()), Table],
            "- - # h\n  | x |\n  | --- |\n- q\n",
        ),
        (
            vec![tight_list_of(h()), definition_block()],
            "- - # h\n  [d]: u\n- q\n",
        ),
        (
            vec![Paragraph(vec![text("a")]), Table],
            "- a\n  | x |\n  | --- |\n- q\n",
        ),
        (
            vec![Quote(vec![Paragraph(vec![text("a")])]), Table],
            "- > a\n  >\n  | x |\n  | --- |\n- q\n",
        ),
        (
            vec![Quote(vec![Paragraph(vec![text("a")])]), setext_only()],
            "- > a\n  >\n  x\n  y\n  =\n- q\n",
        ),
        (
            vec![definition_block(), setext_only()],
            "- [d]: u\n  x\n  y\n  =\n- q\n",
        ),
    ] {
        let document = build_list(&tight_before_q(blocks));
        assert!(document.is_normalized(), "{:?}", expected);
        assert!(tight_of_first_list(&document));
        assert_eq!(
            reads_back_with(document, &gfm, &base_markdown::MarkdownOptions::default()),
            expected
        );
    }
    let source = "- a\n  - # h\n  b\n- c\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert!(document.is_normalized());
    assert_eq!(reads_back(document), source);
}

fn tight_list_of(block: Block) -> Block {
    Block::List(None, true, vec![(false, vec![block])])
}

#[test]
fn a_heading_is_written_atx_where_a_setext_heading_would_need_a_blank_line_in_a_tight_item() {
    use Block::{Heading, Paragraph, Quote, Separator};
    let x = || EventKind::Text(Text::from("x\ny".to_owned()));
    // A heading with a line ending in its text is written setext elsewhere, and a setext heading after a block it would continue needs a blank line, which would make the list read back loose and the output change on the next write (#184). An ATX heading interrupts a paragraph and ends the containers around it, so the heading is written ATX there, with the line ending written as `&#xA;`, and the list stays tight. Where no blank line is needed, after a line of `>` here, it stays setext.
    for (left, (_, before)) in blocks_before().into_iter().enumerate() {
        let block = || {
            tight_before_q(vec![
                blocks_before().swap_remove(left).0,
                Separator,
                Heading(vec![x()]),
            ])
        };
        let document = build_list(&block());
        assert!(document.is_normalized(), "{}", before);
        assert_eq!(
            reads_back_with(
                document,
                &ParseOptions::gfm(),
                &base_markdown::MarkdownOptions::default()
            ),
            format!("- {}\n  # x&#xA;y\n- q\n", before)
        );
    }
    let document = build_list(&tight_before_q(vec![
        Quote(vec![Paragraph(vec![EventKind::Text(Text::from(
            "a".to_owned(),
        ))])]),
        Heading(vec![x()]),
    ]));
    assert_eq!(reads_back(document), "- > a\n  >\n  x\n  y\n  =\n- q\n");
    // A line ending in inline HTML cannot be written as `&#xA;`, so such a heading stays setext, and normalizing makes the list loose.
    let document = build_list(&tight_before_q(vec![
        Paragraph(vec![EventKind::Text(Text::from("a".to_owned()))]),
        Heading(vec![
            x(),
            EventKind::Html {
                value: Text::from("<a\nb>".to_owned()),
                block: false,
            },
        ]),
    ]));
    assert_eq!(
        reads_back(document),
        "- a\n\n  x\n  y<a\n  b>\n  ==\n\n- q\n"
    );
    // A parsed ATX heading holds a line ending only from a character reference, which an ATX heading writes as `&#xA;`, so there it is not made setext, as elsewhere, and the list stays tight. It was written setext after a blank line, which read back loose and was written differently the next time.
    for (source, expected) in [
        ("- a\n  # x&#10;y\n- q\n", "- a\n  # x&#xA;y\n- q\n"),
        ("- - a\n  # x&#10;y\n- q\n", "- - a\n  # x&#xA;y\n- q\n"),
        ("- > - a\n  # x&#10;y\n- q\n", "- > - a\n  # x&#xA;y\n- q\n"),
        (
            "- a\n  # ![x&#10;y](u)\n- q\n",
            "- a\n  # ![x&#xA;y](u)\n- q\n",
        ),
    ] {
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        document.normalize();
        assert!(tight_of_first_list(&document), "{:?}", source);
        assert_eq!(reads_back(document), expected);
    }
    let document = Document::parse("# x&#10;y\n", &ParseOptions::default()).unwrap();
    assert_eq!(reads_back(document), "x\ny\n=\n");
}

#[test]
fn joining_paragraphs_and_loosening_lists_in_tight_items_is_linear() {
    use Block::{List, Paragraph, Quote, Separator};
    let text = || EventKind::Text(Text::from("a".to_owned()));
    let tight = |items: Vec<(bool, Vec<Block>)>| List(None, true, items);
    // An item of many paragraphs of a few children each, joined into one, and a paragraph after many block quotes that end in a paragraph through block quotes in them. Serializing these is slow next to normalizing them, so only normalizing is timed, at a size where joining that walks to the last child of the paragraph so far on each join already fails, and the joined paragraph is serialized once at a smaller size.
    let paragraphs = |size: usize| {
        let blocks = (0..size)
            .flat_map(|_| {
                [
                    Paragraph(vec![text(), EventKind::HardBreak, text()]),
                    Separator,
                ]
            })
            .collect();
        build_list(&tight(vec![(false, blocks)]))
    };
    assert_normalize_only_linear(
        "paragraphs in a row in a tight item",
        1 << 8,
        paragraphs,
        |_, _| {},
    );
    assert_normalize_linear("serializing joined paragraphs", 1 << 6, paragraphs);
    let quotes = |size: usize| {
        let blocks = (0..size)
            .flat_map(|_| {
                [
                    Quote(vec![Quote(vec![Paragraph(vec![text()])])]),
                    Separator,
                    Paragraph(vec![text()]),
                    Separator,
                ]
            })
            .collect();
        build_list(&tight(vec![(false, blocks)]))
    };
    assert_normalize_only_linear(
        "block quotes and paragraphs in a tight item",
        1 << 8,
        quotes,
        |_, _| {},
    );
    // Each paragraph after a block quote is written after a line of `>`, and each pair of block quotes in a row in a spread item of a loose list is checked for a blank line, which is linear only if each join looks at a bounded number of blocks.
    assert_normalize_linear("serializing block quotes and paragraphs", 1 << 8, quotes);
    assert_normalize_linear(
        "serializing block quotes in a row in a loose list",
        1 << 8,
        |size| {
            let blocks = (0..size)
                .flat_map(|_| [Quote(vec![Paragraph(vec![text()])]), Separator])
                .collect();
            build_list(&List(
                None,
                false,
                vec![(true, blocks), (false, vec![Paragraph(vec![text()])])],
            ))
        },
    );
    // Each heading with a line ending in its text after a paragraph is written ATX, which is linear only if each looks at the block before it alone.
    assert_linear(
        "serializing headings with line endings after paragraphs in a tight item",
        1 << 8,
        |size| {
            let blocks = (0..size)
                .flat_map(|_| {
                    [
                        Paragraph(vec![text()]),
                        Block::Heading(vec![EventKind::Text(Text::from("a\nb".to_owned()))]),
                    ]
                })
                .collect();
            let mut document = build_list(&tight(vec![(false, blocks)]));
            timed(|| {
                document.normalize();
                assert!(document.is_normalized());
                base_markdown::to_markdown(&document).unwrap();
            })
        },
    );
    // A tight list of many items, of which only the last needs it to be loose.
    assert_normalize_only_linear(
        "a tight list made loose by its last item",
        1 << 8,
        |size| {
            let mut items: Vec<(bool, Vec<Block>)> = (0..size)
                .map(|_| (false, vec![Paragraph(vec![text()])]))
                .collect();
            items.push((false, vec![Quote(vec![]), Separator, Quote(vec![])]));
            build_list(&tight(items))
        },
        |_, _| {},
    );
}

fn code_block(value: &str) -> EventKind<'static> {
    EventKind::CodeBlock {
        value: Text::from(value.to_owned()),
        info: None,
        fence: None,
        unclosed: false,
    }
}

fn code_block_value<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> String {
    let EventKind::CodeBlock { value, .. } = &document.node(id).unwrap().kind else {
        unreachable!("the node is a code block");
    };
    value.resolve(document.source()).unwrap().to_owned()
}

#[test]
fn canonical_ignores_a_missing_final_line_ending_only_on_the_last_node() {
    let parse = ParseOptions::default();
    let build = |value: &str, after: bool| {
        let mut document = Document::parse(String::new(), &parse).unwrap();
        let root = document.root();
        document.append_child(root, code_block(value)).unwrap();
        if after {
            let paragraph = document
                .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
                .unwrap();
            document
                .append_child(paragraph, EventKind::Text(Text::Literal("x")))
                .unwrap();
        }
        canonical(&document)
    };
    assert_eq!(build("a", false), build("a\n", false));
    assert_ne!(build("a", true), build("a\n", true));
}

#[test]
fn code_block_values_get_the_line_endings_the_parser_stores() {
    let parse = ParseOptions::default();
    let options = base_markdown::MarkdownOptions::default();
    // The value, then what it normalizes to before another block, and as the last node of the document. A closed fence stores its last line with a line ending.
    let values = [
        ("a", "a\n", "a"),
        ("\na", "\na\n", "\na"),
        (" ", " \n", " "),
        ("```", "```\n", "```"),
        ("a\nb", "a\nb\n", "a\nb"),
        // An empty value is written as a fence with no lines, and one blank line as a fence with one.
        ("", "", ""),
        ("\n", "\n", "\n"),
        ("a\n", "a\n", "a\n"),
    ];
    for (value, before, last) in values {
        {
            let mut document = Document::parse(String::new(), &parse).unwrap();
            let root = document.root();
            let code = document.append_child(root, code_block(value)).unwrap();
            document
                .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
                .unwrap();
            let at_end = document.clone();
            let paragraph = document.children(root).nth(1).unwrap();
            document
                .append_child(paragraph, EventKind::Text(Text::Literal("x")))
                .unwrap();
            assert_eq!(document.is_normalized(), value == before, "{value:?}");
            reads_back_with(document.clone(), &parse, &options);
            document.normalize();
            assert_eq!(code_block_value(&document, code), before, "{value:?}");

            // The paragraph is empty, so normalizing removes it, and the block is then the last node.
            let mut document = at_end;
            document.normalize();
            assert_eq!(code_block_value(&document, code), last, "{value:?} last");
            assert!(document.is_normalized());
        }
    }
    // The parser stores every line ending as LF, and NUL as U+FFFD, so a value with a CR or NUL is rejected rather than rewritten (#257).
    for value in ["\r", "\r\n", "a\rb\n", "a\0\n"] {
        let mut document = Document::parse(String::new(), &parse).unwrap();
        let root = document.root();
        assert!(
            matches!(
                document.append_child(root, code_block(value)),
                Err(StreamError::InvalidValue)
            ),
            "{:?}",
            value
        );
    }
}

#[test]
fn code_blocks_that_end_a_parsed_document_keep_their_values() {
    let parse = ParseOptions::default();
    // An unclosed fence or indented code at the end of the source stores its last line without a line ending.
    for source in [
        "```\na",
        "    a",
        "- ```\n  a",
        "> - ```\n>   a",
        "[^a]: x\n\n        b",
        "```\r\na\r\n```\r\n\r\n    b\r\n",
        "```\n```",
        "```\n\n```",
        "```",
    ] {
        let document = Document::parse(source, &parse).unwrap();
        let mut normalized = document.clone();
        normalized.normalize();
        assert!(normalized.is_normalized());
        for id in document.descendants(document.root()) {
            if let EventKind::CodeBlock { .. } = document.node(id).unwrap().kind {
                assert_eq!(
                    code_block_value(&normalized, id),
                    code_block_value(&document, id),
                    "{source:?}"
                );
            }
        }
    }
    // Only the last node of the document keeps a value without a final line ending: a block before it, nested or detached, gets one.
    let mut document = Document::parse("- ```\n  a", &ParseOptions::default()).unwrap();
    let root = document.root();
    let list = document.children(root).next().unwrap();
    let code = document.descendants(list).last().unwrap();
    let detached = document.append_child(root, code_block("b")).unwrap();
    document.detach(detached).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert_eq!(code_block_value(&document, code), "a");
    assert_eq!(code_block_value(&document, detached), "b\n");
    document.append_child(root, code_block("c")).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert_eq!(code_block_value(&document, code), "a\n");
}

fn html_block(value: &'static str) -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Html {
            value: Text::Literal(value),
            block: true,
        },
        0..0,
    )
}

#[test]
fn removes_html_that_cannot_be_written() {
    // Inline HTML without a value writes nothing, so it is removed and what was on either side meets: text joins, attention merges, and attention or a paragraph left empty is removed.
    assert_eq!(
        settled(build(vec![
            html(""),
            text("a"),
            html(""),
            text("b"),
            html("")
        ])),
        r#"paragraph("ab")"#
    );
    assert_eq!(
        settled(build(concat([
            emphasis(vec![text("a")]),
            vec![html("")],
            emphasis(vec![text("b")]),
            strong(vec![html("")]),
        ]))),
        r#"paragraph(emphasis("ab"))"#
    );
    assert_eq!(
        settled(build_blocks(concat([
            paragraph(vec![text("a")]),
            vec![separator()],
            paragraph(vec![html("")]),
            vec![separator()],
            heading(vec![html("")]),
        ]))),
        r#"paragraph("a") separator heading()"#
    );
    // A reference after inline HTML without a value is the first thing written in its paragraph, so it is escaped as one, and does not read back as a definition (#146).
    let mut document =
        Document::parse("[r]\n\n[r]: /u\n".to_owned(), &ParseOptions::default()).unwrap();
    let first = document.children(document.root()).next().unwrap();
    let reference = document.children(first).next();
    let empty = document
        .append_child(
            first,
            EventKind::Html {
                value: Text::Literal(""),
                block: false,
            },
        )
        .unwrap();
    document.attach(first, empty, reference).unwrap();
    document
        .append_child(first, EventKind::Text(Text::Literal(": /u")))
        .unwrap();
    assert!(!document.is_normalized());
    assert_eq!(reads_back(document), "[r]\\: /u\n\n[r]: /u\n");
    // An HTML block is one HTML block construct, so one of only spaces, tabs and line endings, which would be written as blank lines, is rejected rather than removed (#257).
    for value in ["", "\n", " ", "\t", "\r\n", " \n\t\n", "x"] {
        let mut builder = AstBuilder::new();
        let events = concat([
            vec![start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            })],
            vec![html_block(value)],
            vec![end(TagKind::Document)],
        ]);
        assert!(
            matches!(
                builder.consume(&mut events.into_iter(), usize::MAX),
                Err(StreamError::InvalidValue)
            ),
            "{:?}",
            value
        );
    }
    // HTML that can be written stays: an HTML block, and inline HTML with a value, which is one HTML construct (#193).
    for document in [
        build_blocks(concat([
            vec![html_block("<div>"), separator(), html_block(" <b>")],
            vec![separator(), html_block("<!-- x -->")],
        ])),
        build(vec![
            text("a"),
            html("<b>"),
            text("b"),
            html("<c\nd>"),
            text("c"),
        ]),
        build(vec![text("a"), html("<b>")]),
    ] {
        assert!(document.is_normalized());
    }
}

#[test]
fn removes_lists_without_items() {
    use Block::{List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let tight = |items: Vec<Vec<Block>>| {
        List(
            None,
            true,
            items.into_iter().map(|blocks| (false, blocks)).collect(),
        )
    };
    let empty = || List(None, true, vec![]);
    // A list without items is written as nothing, or as a lone `-` in an item, so it is removed, and the rules for what that leaves apply in turn: `- p\n  -` read back as a heading (#149).
    for (block, expected) in [
        (empty(), ""),
        (Quote(vec![empty()]), ">\n"),
        (
            Quote(vec![p("a"), Separator, empty(), Separator, p("b")]),
            "> a\n>\n> b\n",
        ),
        (tight(vec![vec![empty()]]), "-\n"),
        (
            tight(vec![vec![p("p"), tight(vec![vec![empty()]])]]),
            "- p\n",
        ),
        (
            tight(vec![vec![p("p"), tight(vec![vec![empty()], vec![p("a")]])]]),
            "- p\n  - a\n",
        ),
        (
            tight(vec![vec![
                p("p"),
                tight(vec![vec![p("q"), tight(vec![vec![empty()]])]]),
            ]]),
            "- p\n  - q\n",
        ),
        (tight(vec![vec![p("p"), Separator, empty()]]), "- p\n"),
        // Paragraphs that meet where it was are joined, and blocks that can be written apart only with a blank line make the list loose.
        (tight(vec![vec![p("a"), empty(), p("b")]]), "- a\n  b\n"),
        (
            tight(vec![vec![
                Quote(vec![p("a")]),
                empty(),
                Quote(vec![p("b")]),
            ]]),
            "- > a\n\n  > b\n",
        ),
        // A loose list whose only item is left with one block is made tight.
        (
            List(None, false, vec![(true, vec![p("a"), Separator, empty()])]),
            "- a\n",
        ),
        (
            List(None, false, vec![(true, vec![empty(), Separator, p("a")])]),
            "- a\n",
        ),
    ] {
        let document = build_list(&block);
        assert!(!document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // The root of a detached subtree is never removed.
    let mut document = build_list(&tight(vec![vec![p("p")]]));
    let root = document.root();
    let detached = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    document.detach(detached).unwrap();
    document.normalize();
    assert!(document.is_normalized());
    assert!(document.node(detached).unwrap().first_child().is_none());
}

/// The tree of `block` with what normalizing removes as unwritable taken out by hand: inline HTML without a value, and lists without items.
fn without_unwritable(block: &Block) -> Option<Block> {
    let blocks = |blocks: &[Block]| blocks.iter().filter_map(without_unwritable).collect();
    Some(match block {
        Block::Task(phrasing) => Block::Task(phrasing.clone()),
        Block::Paragraph(phrasing) => Block::Paragraph(
            phrasing
                .iter()
                .filter(|kind| {
                    !matches!(kind, EventKind::Html { value, .. } if value.resolve("").unwrap().is_empty())
                })
                .cloned()
                .collect(),
        ),
        Block::List(_, _, items) if items.is_empty() => return None,
        Block::List(start, tight, items) => Block::List(
            *start,
            *tight,
            items
                .iter()
                .map(|(spread, children)| (*spread, blocks(children)))
                .collect(),
        ),
        Block::Quote(children) => Block::Quote(blocks(children)),
        Block::Footnote(children) => Block::Footnote(blocks(children)),
        Block::Heading(phrasing) => Block::Heading(phrasing.clone()),
        Block::Table => Block::Table,
        Block::Leaf(kind) => Block::Leaf(kind.clone()),
        Block::Code => Block::Code,
        Block::Separator => Block::Separator,
    })
}

fn random_flow(random: &mut Random, depth: u32) -> Vec<Block> {
    // Inline HTML is empty or one construct (#193), and an HTML block one block construct (#257).
    let html = |random: &mut Random, block: bool| EventKind::Html {
        value: Text::Literal(if block {
            ["<b>", "<div>", "<!-- x -->"][random.below(3) as usize]
        } else {
            ["", "", "<b>", "</b>", "<!---->", "<?x?>"][random.below(6) as usize]
        }),
        block,
    };
    (0..random.below(4))
        .map(|_| match random.below(if depth == 0 { 2 } else { 5 }) {
            0 => Block::Paragraph(
                (0..1 + random.below(3))
                    .map(|_| match random.below(3) {
                        0 => html(random, false),
                        _ => EventKind::Text(Text::from(
                            ["a", " ", "b"][random.below(3) as usize].to_owned(),
                        )),
                    })
                    .collect(),
            ),
            1 => Block::Leaf(html(random, true)),
            2 => Block::Quote(random_flow(random, depth - 1)),
            _ => {
                let tight = random.below(2) == 0;
                Block::List(
                    None,
                    tight,
                    (0..random.below(3))
                        .map(|_| (!tight, random_flow(random, depth - 1)))
                        .collect(),
                )
            }
        })
        .collect()
}

#[test]
fn removing_unwritable_html_and_lists_is_removing_them_by_hand() {
    // Normalizing a tree gives the same tree as normalizing it with the unwritable nodes taken out first, so one pass removes them and everything their removal leaves to remove. Serializing the tree before normalizing must not panic.
    let mut random = Random(0x6A09_E667_F3BC_C908);
    let mut removed = 0;
    for _ in 0..5_000 {
        let blocks = Block::Quote(random_flow(&mut random, 3));
        let mut document = build_list(&blocks);
        to_markdown(&document).unwrap();
        let mut expected = build_list(&without_unwritable(&blocks).unwrap());
        let normal = document.is_normalized();
        let before = describe(&document, document.root());
        document.normalize();
        expected.normalize();
        let once = describe(&document, document.root());
        assert!(document.is_normalized(), "{}", before);
        assert_eq!(normal, once == before, "{before}");
        assert_eq!(once, describe(&expected, expected.root()), "{before}");
        removed += usize::from(once != describe(&build_list(&blocks), document.root()));
        document.normalize();
        assert_eq!(describe(&document, document.root()), once);
    }
    assert!(removed > 1_000, "{}", removed);
}

#[test]
fn serializing_html_without_a_value_after_a_line_ending_does_not_panic() {
    // Inline HTML at the start of a line is checked for whether it would start an HTML block, which an empty value cannot.
    for value in ["", "<b>"] {
        let document = build(vec![text("a"), soft(), html(value), text("b")]);
        assert_eq!(to_markdown(&document).unwrap(), format!("a\n{value}b\n"));
    }
}

#[test]
fn removing_unwritable_html_and_lists_is_linear() {
    // Each is removed with one separator in one pass over its parent's children, and inline HTML in the pass over its paragraph's.
    assert_linear("unwritable blocks", 1 << 10, |size| {
        let mut events = Vec::new();
        for _ in 0..size / 4 {
            events.extend(attention(
                Tag::List {
                    start: None,
                    tight: true,
                },
                TagKind::List,
                vec![],
            ));
            events.push(separator());
            events.extend(paragraph(vec![html(""), text("a"), html("")]));
        }
        let mut document = build_blocks(events);
        let elapsed = timed(|| {
            document.normalize();
            assert!(document.is_normalized());
            to_markdown(&document).unwrap();
        });
        assert_eq!(document.children(document.root()).count(), size / 4);
        elapsed
    });
}

/// The cells of a row, each with its alignment, whether it is a header cell, and its text.
type Row = Vec<(base_markdown::AlignKind, bool, &'static str)>;

/// A table of `groups`, each a head or body of rows of cells, each with its alignment, whether it is a header cell, and its text.
fn shaped_table(groups: Vec<(TagKind, Vec<Row>)>) -> Document<String> {
    let mut events = Vec::new();
    for (kind, rows) in groups {
        let tag = match kind {
            TagKind::TableHead => Tag::TableHead,
            _ => Tag::TableBody,
        };
        let mut group = Vec::new();
        for row in rows {
            let mut cells = Vec::new();
            for (alignment, header, value) in row {
                let children = if value.is_empty() {
                    vec![]
                } else {
                    vec![text(value)]
                };
                cells.extend(attention(
                    Tag::TableCell { alignment, header },
                    TagKind::TableCell,
                    children,
                ));
            }
            group.extend(attention(Tag::TableRow, TagKind::TableRow, cells));
            group.push(separator());
        }
        events.extend(attention(tag, kind, group));
        events.push(separator());
    }
    build_blocks(attention(Tag::Table, TagKind::Table, events))
}

#[test]
fn shapes_tables_as_the_parser_does() {
    use base_markdown::AlignKind::{Center, Left, None as Plain, Right};
    use TagKind::{TableBody as Body, TableHead as Head};
    // A parsed table is shaped already.
    let parsed = Document::parse(
        "| a | b |\n| :- | -: |\n| c |\n| d | e | f |\n",
        &ParseOptions::gfm(),
    )
    .unwrap();
    assert!(parsed.is_normalized());
    for (table, expected) in [
        // Rows get empty cells up to the widest row, which an empty head cell is added for, unaligned.
        (
            vec![
                (Head, vec![vec![(Left, true, "a")]]),
                (
                    Body,
                    vec![vec![(Left, false, "b"), (Right, false, "c")], vec![]],
                ),
            ],
            "| a |  |\n| :--- | --- |\n| b | c |\n|  |  |\n",
        ),
        // Header flags and alignments follow the head.
        (
            vec![
                (Head, vec![vec![(Center, false, "a"), (Plain, true, "b")]]),
                (Body, vec![vec![(Right, true, "c"), (Left, false, "d")]]),
            ],
            "| a | b |\n| :---: | --- |\n| c | d |\n",
        ),
        // A head of two rows keeps the first, and the second starts the body.
        (
            vec![
                (
                    Head,
                    vec![vec![(Plain, true, "a")], vec![(Plain, true, "b")]],
                ),
                (Body, vec![vec![(Plain, false, "c")]]),
            ],
            "| a |\n| --- |\n| b |\n| c |\n",
        ),
        // A body before the head, several bodies and empty parts: rows are taken in order.
        (
            vec![
                (Body, vec![vec![(Plain, false, "a")]]),
                (Head, vec![]),
                (Head, vec![vec![(Right, true, "b")]]),
                (Body, vec![]),
                (Body, vec![vec![(Plain, false, "c")]]),
            ],
            "| a |\n| --- |\n| b |\n| c |\n",
        ),
        // A head of two rows without a body, and two heads: a body is made for the rest.
        (
            vec![(
                Head,
                vec![vec![(Plain, true, "a")], vec![(Plain, true, "b")]],
            )],
            "| a |\n| --- |\n| b |\n",
        ),
        (
            vec![
                (Head, vec![vec![(Plain, true, "a")]]),
                (Head, vec![vec![(Plain, true, "b")]]),
            ],
            "| a |\n| --- |\n| b |\n",
        ),
        // A body alone becomes the head.
        (
            vec![(Body, vec![vec![(Plain, false, "a")]])],
            "| a |\n| --- |\n",
        ),
        // A head without a body loses an empty body.
        (
            vec![(Head, vec![vec![(Plain, true, "a")]]), (Body, vec![])],
            "| a |\n| --- |\n",
        ),
        // A table without a cell cannot be written, so it is removed.
        (vec![], ""),
        (vec![(Head, vec![vec![]]), (Body, vec![vec![], vec![]])], ""),
    ] {
        let document = shaped_table(table);
        assert!(!document.is_normalized(), "{:?}", expected);
        assert_eq!(reads_back(document), expected);
    }
    // A detached table is shaped too, and one without a cell is left as it is.
    for (table, expected) in [
        (
            vec![(Body, vec![vec![(Plain, false, "a")]])],
            r#"Start(Table)(Start(TableHead)(Start(TableRow)(cell("a")) separator))"#,
        ),
        (
            vec![(Head, vec![])],
            r#"Start(Table)(Start(TableHead)() separator)"#,
        ),
    ] {
        let mut document = shaped_table(table);
        let table = document.children(document.root()).next().unwrap();
        document.detach(table).unwrap();
        document.normalize();
        assert!(document.is_normalized());
        assert_eq!(describe(&document, table), expected);
    }
}

#[test]
fn shaping_tables_is_linear() {
    use base_markdown::AlignKind::None as Plain;
    assert_normalize_linear("unshaped tables", 1 << 10, |size| {
        let rows = (0..size)
            .map(|index| vec![(Plain, true, "a"); index % 3])
            .collect();
        shaped_table(vec![(TagKind::TableBody, rows)])
    });
}
