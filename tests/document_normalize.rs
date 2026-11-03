// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod support;

use base_markdown::{
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, AstBuilder, CompileOptions, Constructs, Document, NodeId, NodeSyntax,
    ParseOptions,
};
use std::borrow::Cow;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use support::fixture::canonical;

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

/// Wrap phrasing events in a document and paragraph, and build a document over `source`.
fn build_in(source: &str, phrasing: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![
        start(Tag::Document(None)),
        start(Tag::Paragraph { tight: false }),
    ];
    events.extend(phrasing);
    events.push(end(TagKind::Paragraph));
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::new();
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
        EventKind::Start(Tag::Item { block_end }) => format!("item {block_end}({})", children()),
        EventKind::Start(Tag::Document(_)) => children(),
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
        EventKind::Math(value) => format!("math {:?}", value.resolve(document.source()).unwrap()),
        EventKind::TaskMarker(_) => "task".into(),
        EventKind::Start(Tag::Link { destination, .. }) => format!(
            "link {:?}({})",
            destination.resolve(document.source()).unwrap(),
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
            delete(vec![text(""), separator()]),
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
fn removes_separators_from_phrasing() {
    assert!(!build(vec![text("a"), separator()]).is_normalized());
    assert_eq!(
        normalized(build(vec![text("a"), separator(), text("b")])),
        r#"paragraph("ab")"#
    );
    let document = build(concat([
        emphasis(vec![text("a")]),
        vec![separator()],
        emphasis(vec![separator(), text("b")]),
    ]));
    assert!(!document.is_normalized());
    assert_eq!(normalized(document), r#"paragraph(emphasis("ab"))"#);
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
    // A separator between blocks does not stop text on either side from joining.
    let mut document = build(vec![]);
    let paragraph = document.children(document.root()).next().unwrap();
    let quote = document
        .append_child(document.root(), EventKind::Start(Tag::BlockQuote))
        .unwrap();
    document.detach(paragraph).unwrap();
    for kind in [
        EventKind::Text(Text::Literal("a")),
        EventKind::Separator {
            value: Text::Literal("\n"),
            conditional: false,
        },
        EventKind::Text(Text::Literal("b")),
    ] {
        document.append_child(quote, kind).unwrap();
    }
    assert!(!document.is_normalized());
    document.normalize();
    assert_eq!(
        describe(&document, document.root()),
        r#"Start(BlockQuote)("ab" separator)"#
    );
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
        start(Tag::Document(None)),
        start(Tag::List {
            start: None,
            tight: true,
        }),
        start(Tag::Item { block_end: true }),
        start(Tag::Paragraph { tight: true }),
        text("a"),
        end(TagKind::Paragraph),
        text(""),
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
        EventKind::Start(Tag::Item { block_end: true })
    ));
    document.normalize();
    assert!(matches!(
        document.node(item).unwrap().kind,
        EventKind::Start(Tag::Item { block_end: false })
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
            2 => events.push(separator()),
            3 => events.push(hard()),
            4 => events.push(soft()),
            5 => events.push(code(
                ["a", " "][random.below(2) as usize],
                [None, Some("x")][random.below(2) as usize],
            )),
            6 => events.push(math("m")),
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
            1 => table(phrasing),
            2 => attention(
                Tag::List {
                    start: None,
                    tight: true,
                },
                TagKind::List,
                attention(
                    Tag::Item { block_end: false },
                    TagKind::Item,
                    paragraph(concat([
                        vec![event(EventKind::TaskMarker(false), 0..0)],
                        phrasing,
                    ])),
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

#[test]
fn normalize_makes_any_tree_normalized_and_is_idempotent() {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let mut unnormalized = 0;
    for _ in 0..20_000 {
        let mut document = random_document(&mut random);
        let normal = document.is_normalized();
        unnormalized += usize::from(!normal);
        // Serializing a tree that is not normalized may be wrong, but it must not panic.
        to_markdown(&document).unwrap();
        let before = describe(&document, document.root());
        document.normalize();
        assert!(document.is_normalized(), "{}", before);
        let once = describe(&document, document.root());
        // A tree is reported as normalized exactly when normalizing changes nothing.
        assert_eq!(normal, once == before, "{before}");
        let markdown = to_markdown(&document).unwrap();
        document.normalize();
        assert_eq!(describe(&document, document.root()), once);
        assert_eq!(to_markdown(&document).unwrap(), markdown);
    }
    assert!(unnormalized > 10_000, "{}", unnormalized);
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
    for _ in 0..20_000 {
        let mut events = Vec::new();
        for _ in 0..1 + random.below(3) {
            let phrasing = random_phrasing(&mut random, 6);
            events.extend(match random.below(3) {
                0 => heading(phrasing),
                1 => table(phrasing),
                _ => paragraph(phrasing),
            });
        }
        let document = build_blocks(events);
        for options in &options {
            base_markdown::to_markdown_with_options(&document, options).unwrap();
        }
    }
}

/// Assert that four times the input takes less than twelve times as long, after a warm-up.
///
/// Quadratic time would take sixteen times as long, so the bound sits below that to catch it, with room above four times for noise from load and instrumentation.
///
/// Each size takes the fastest of five runs, alternating between the sizes, so that a pause, or a stretch of load from other tests or instrumentation, slows both sizes alike and does not count. Only one scaling test times at once, as large allocations in parallel tests contend with each other.
fn assert_linear(mut time: impl FnMut(usize) -> Duration) {
    static TIMING: Mutex<()> = Mutex::new(());
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    time(1 << 10);
    let mut small = Duration::MAX;
    let mut large = Duration::MAX;
    for _ in 0..5 {
        small = small.min(time(50_000));
        large = large.min(time(200_000));
    }
    assert!(
        large < small * 12,
        "4x input took {:?} against {:?}",
        large,
        small
    );
}

/// Time normalizing the document from `build` and serializing the result. Serializing the smallest tree before normalizing checks that it does not panic; that need not be linear, so it is not timed.
fn assert_normalize_linear(build: impl Fn(usize) -> Document<String>) {
    assert_linear(|size| {
        let mut document = build(size);
        if size == 1 << 10 {
            to_markdown(&document).unwrap();
        }
        let started = Instant::now();
        document.normalize();
        assert!(document.is_normalized());
        to_markdown(&document).unwrap();
        started.elapsed()
    });
}

#[test]
fn normalizing_empty_text_is_linear() {
    assert_normalize_linear(|size| {
        build(concat([
            emphasis(vec![text("a")]),
            (0..size).map(|_| text("")).collect(),
            emphasis(vec![text("b")]),
        ]))
    });
}

#[test]
fn joining_text_is_linear() {
    assert_normalize_linear(|size| build((0..size).map(|_| text("a")).collect()));
}

#[test]
fn merging_attention_is_linear() {
    assert_normalize_linear(|size| {
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
    assert_linear(|size| {
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
    assert_normalize_linear(|size| {
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
fn removing_nested_empty_attention_is_linear() {
    // The serializer recurses per level, so only normalizing is timed.
    assert_linear(|size| {
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
    // Deep strikethrough becomes one level, which is serialized too. The serializer recurses per level, so the tree is not serialized before normalizing.
    let deep = |size: usize| {
        let mut events = Vec::new();
        for index in 0..size {
            events.push(text("a"));
            events.push(start(Tag::Delete));
            if index % 2 == 0 {
                events.push(text("b"));
            }
        }
        events.push(text("c"));
        for _ in 0..size {
            events.push(end(TagKind::Delete));
            events.push(text("d"));
        }
        build(events)
    };
    assert_linear(|size| {
        let mut document = deep(size);
        let started = Instant::now();
        document.normalize();
        assert!(document.is_normalized());
        to_markdown(&document).unwrap();
        let elapsed = started.elapsed();
        // The root, the paragraph, and text, strikethrough of text, and text in it.
        assert_eq!(document.descendants(document.root()).count(), 6);
        elapsed
    });
    // Strikethrough alternating with emphasis keeps the emphasis, so only normalizing is timed.
    assert_linear(|size| {
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
fn appending_children_is_linear() {
    assert_linear(|size| {
        let mut document = build(vec![]);
        let paragraph = document.children(document.root()).next().unwrap();
        let started = Instant::now();
        for _ in 0..size {
            document
                .append_child(paragraph, EventKind::Text(Text::Literal("a")))
                .unwrap();
        }
        let elapsed = started.elapsed();
        assert_eq!(document.children(paragraph).count(), size);
        elapsed
    });
}

#[test]
fn appending_to_an_item_is_linear() {
    assert_linear(|size| {
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
            EventKind::Start(Tag::Item { block_end }) => block_end,
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
        Tag::Link {
            destination: Text::Literal("http://a.b"),
            title: None,
        },
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

fn math(value: &'static str) -> Result<Event<'static>, StreamError> {
    event(EventKind::Math(Text::Literal(value)), 0..0)
}

fn soft() -> Result<Event<'static>, StreamError> {
    event(EventKind::SoftBreak(Text::Literal("\n")), 0..0)
}

fn hard() -> Result<Event<'static>, StreamError> {
    event(EventKind::HardBreak, 0..0)
}

/// Build a document of block events.
fn build_blocks(blocks: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![start(Tag::Document(None))];
    events.extend(blocks);
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::new();
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
fn joins_adjacent_code_and_math() {
    assert_eq!(
        settled(build(vec![
            code("a", None),
            code("b", None),
            code("c", Some("rust")),
            math("x"),
            math("y"),
            text("z"),
            code("d", Some("rust")),
            code("e", None),
        ])),
        r#"paragraph(code "ab" code "c" "rust" math "xy" "z" code "de" "rust")"#
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

#[test]
fn replaces_hard_breaks_in_table_cells() {
    assert_eq!(
        settled(build_blocks(table(concat([
            vec![text("a"), hard(), text("b")],
            emphasis(vec![hard(), text("c")]),
            vec![hard()],
            delete(emphasis(vec![text("d"), hard()])),
            vec![hard()],
        ])))),
        r#"Start(Table)(Start(TableHead)(Start(TableRow)(cell("a b " emphasis("c") " " delete(emphasis("d")) "  "))))"#
    );
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
    let task = || event(EventKind::TaskMarker(false), 0..0);
    let item = |children| {
        attention(
            Tag::List {
                start: None,
                tight: true,
            },
            TagKind::List,
            attention(
                Tag::Item { block_end: false },
                TagKind::Item,
                paragraph(children),
            ),
        )
    };
    let list = |inner: &str| {
        format!(r#"Start(List {{ start: None, tight: true }})(item false(paragraph({inner})))"#)
    };
    // Breaks after text are trimmed, as after any paragraph's phrasing.
    assert_eq!(
        settled(build_blocks(item(vec![task(), text(" a"), hard(), soft()]))),
        list(r#"task " a""#)
    );
    // So is a hard break that moves out of attention at the end.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task(), text(" a")],
            emphasis(vec![text("b"), hard()]),
        ])))),
        list(r#"task " a" emphasis("b")"#)
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
        let mut children = vec![task()];
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
        (vec![text("  ")], r#"task "  ""#),
        (vec![text(" \t")], r#"task " \t""#),
        (vec![text(" \u{a0}"), hard()], r#"task " \u{a0}""#),
        (vec![text(" \u{3000}"), soft()], r#"task " \u{3000}""#),
        (vec![text(" \n")], r#"task " \n""#),
        (vec![text(" "), soft(), text(" ")], r#"task " " soft " ""#),
        (vec![text(" "), text(" ")], r#"task "  ""#),
        (vec![text("\u{a0}"), hard()], r#"task "\u{a0}""#),
    ] {
        let mut children = vec![task()];
        children.extend(after);
        assert_eq!(normal(build_blocks(item(children))), list(expected));
    }
    // Whitespace moved out of attention counts as text, and attention of breaks alone is removed.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task(), text(" ")],
            emphasis(vec![soft()]),
        ])))),
        list(r#"" ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task()],
            emphasis(vec![text(" ")]),
            vec![soft()],
        ])))),
        list(r#"" ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task(), text(" ")],
            emphasis(vec![text(" ")]),
        ])))),
        list(r#"task "  ""#)
    );
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task()],
            emphasis(vec![hard()]),
        ])))),
        "Start(List { start: None, tight: true })(item false())"
    );
    // The rule is the same in any block phrasing that starts with a marker.
    assert!(!build_blocks(heading(vec![task(), hard()])).is_normalized());
    assert_eq!(
        settled(build_blocks(heading(vec![task(), hard()]))),
        "heading()"
    );
    let cell = build_blocks(table(vec![text(" ")]));
    assert_eq!(
        settled(build_blocks(table(vec![task(), text(" ")]))),
        describe(&cell, cell.root())
    );
    assert_eq!(
        settled(build_blocks(heading(vec![task(), text(" a"), hard()]))),
        r#"heading(task " a")"#
    );
    // A marker followed by anything else stays, such as a marker, code or a link.
    for after in [
        vec![task()],
        vec![text(" "), code("c", None)],
        concat([vec![text(" ")], link(vec![text(" ")])]),
    ] {
        let mut children = vec![task()];
        children.extend(after);
        assert!(build_blocks(item(children)).is_normalized());
    }
    // A marker that does not start the paragraph stays.
    assert_eq!(
        settled(build_blocks(item(vec![text("a"), task(), hard()]))),
        list(r#""a" task"#)
    );
    // Attention in it still moves its whitespace out.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task(), text(" ")],
            emphasis(vec![text("a ")]),
        ])))),
        list(r#"task " " emphasis("a") " ""#)
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
                Tag::Item { block_end: false },
                TagKind::Item,
                paragraph(children),
            ),
        )
    };
    let task = || event(EventKind::TaskMarker(false), 0..0);
    serializes_stably(
        build_blocks(item(vec![task(), text(" a"), hard()])),
        "- [ ] a\n",
    );
    serializes_stably(
        build_blocks(item(concat([
            vec![task(), text(" a")],
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
        let mut children = vec![task()];
        children.extend(after);
        serializes_stably(build_blocks(item(children)), expected);
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
        let mut children = vec![task()];
        children.extend(after);
        serializes_stably(build_blocks(item(children)), expected);
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
    let mut document = build_blocks(table(concat([
        vec![text("a")],
        emphasis(vec![text("b"), soft(), hard()]),
        vec![text("c")],
    ])));
    let row = |document: &Document<String>| {
        let table = document.children(document.root()).next().unwrap();
        let head = document.children(table).next().unwrap();
        let row = document.children(head).next().unwrap();
        document.children(row).next().unwrap()
    };
    let cell = row(&document);
    let attention = document.children(cell).nth(1).unwrap();
    let breaks: Vec<_> = document.children(attention).skip(1).collect();
    document.normalize();
    assert_eq!(
        describe(&document, cell),
        r#"cell("a" emphasis("b") soft " c")"#
    );
    // The soft break moved whole; the hard break became a space and joined the text after it.
    assert_eq!(document.children(cell).nth(2), Some(breaks[0]));
}

#[test]
fn is_normalized_finds_each_edge_shape() {
    let shapes = [
        build(vec![code("a", None), code("b", None)]),
        build(vec![math("a"), math("b")]),
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
        build_blocks(table(vec![text("a"), hard(), text("b")])),
        build_blocks(table(vec![text("a"), soft()])),
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
        build(vec![code("a", None), math("b")]),
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
    build: impl Fn(usize) -> Document<String>,
    check: impl Fn(&Document<String>, usize),
) {
    assert_linear(|size| {
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
    // Each level starts and ends with whitespace, which moves out through every level outside it.
    assert_normalize_only_linear(
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
    // Whitespace alone inside: every level is emptied and removed.
    assert_normalize_only_linear(
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
    // Hard breaks at the end of each level move out to the end of the block and are removed.
    assert_normalize_only_linear(
        |size| {
            build(nested_edges(size / 2, Vec::new, vec![text("a")], || {
                vec![hard()]
            }))
        },
        |document, size| {
            assert_eq!(
                document.descendants(document.root()).count(),
                2 + size / 2 + 1
            );
        },
    );
}

#[test]
fn removing_break_chains_is_linear() {
    // At the end of a block.
    assert_normalize_linear(|size| {
        build(concat([
            vec![text("a")],
            (0..size).map(|_| hard()).collect(),
        ]))
    });
    // At the end of attention, followed by more text.
    assert_normalize_linear(|size| {
        build(concat([
            emphasis(concat([
                vec![text("a")],
                (0..size).map(|_| hard()).collect(),
            ])),
            vec![text("b")],
        ]))
    });
    // In a table cell, where each becomes a space.
    assert_normalize_linear(|size| {
        build_blocks(table(concat([
            vec![text("a")],
            (0..size).map(|_| hard()).collect(),
            vec![text("b")],
        ])))
    });
}

#[test]
fn joining_code_and_math_is_linear() {
    assert_normalize_linear(|size| build((0..size).map(|_| code("a", None)).collect()));
    assert_normalize_linear(|size| build((0..size).map(|_| math("a")).collect()));
}

#[test]
fn serializing_code_and_math_with_many_line_endings_is_linear() {
    // Each line ending in code or math is written as a space, and normalizing replaces it with one. Writing them once flattened only those before a block marker, rescanning the content from its start for each (#115). Runs of line endings, CRLF and ordered-list digits each took a different path, and math takes the same path as code. The trees are serialized without normalizing too, which must stay linear.
    let content = |unit: &str, size: usize| Text::from(unit.repeat(size) + "#");
    let check = |tree: &dyn Fn(usize) -> Document<String>| {
        assert_normalize_linear(tree);
        assert_linear(|size| {
            let document = tree(size);
            let started = Instant::now();
            to_markdown(&document).unwrap();
            started.elapsed()
        });
    };
    for unit in ["a\n#", "\n", "a\r\n#", " \n\t", "a\n1. ", "a\n\n-", "\r"] {
        check(&|size| {
            build(vec![event(
                EventKind::Code {
                    value: content(unit, size),
                    language: None,
                },
                0..0,
            )])
        });
    }
    check(&|size| build(vec![event(EventKind::Math(content("a\n#", size)), 0..0)]));
}

fn math_text() -> ParseOptions {
    ParseOptions {
        constructs: Constructs {
            math_text: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    }
}

#[test]
fn line_endings_in_code_and_math_become_spaces() {
    // The parser reads each line ending in inline code or math as one space, CRLF too, so one in the value cannot be written.
    assert_eq!(
        settled(build(vec![
            text("x "),
            code("a\r\nb\rc\nd", Some("rust")),
            text(" "),
            math("\n\r\n\r"),
            text(" "),
            code("\na\n", None),
        ])),
        r#"paragraph("x " code "a b c d" "rust" " " math "   " " " code " a ")"#
    );
    // Code joined from parts is spaced once it is joined.
    assert_eq!(
        settled(build(vec![code("a\r", None), code("\nb", None)])),
        r#"paragraph(code "a b")"#
    );
    // Flow code keeps its line endings.
    let mut document = build_blocks(vec![event(
        EventKind::CodeBlock {
            value: Text::Literal("a\nb\n"),
            language: None,
            fence: None,
            math: false,
            unclosed: false,
        },
        0..0,
    )]);
    assert!(document.is_normalized());
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "```\na\nb\n```\n");
}

#[test]
fn code_and_math_with_line_endings_read_back_and_are_stable() {
    let values = [
        // #136: a line ending read back as a space.
        "a\nb",
        "a\r\nb",
        "a\rb",
        // #135: a line that reads as a table delimiter row.
        "a\n|-|\nb",
        "a\n:-\nb",
        "a\n-|-\nb",
        // A block marker, an indented line and a blank line.
        "a\n# b",
        "a\n    b",
        "a\n\nb",
        "a\r\n\r\nb",
        // Line endings at the edges, and nothing else.
        "\na\n",
        "\r\na\r",
        "\n",
        "\r\n",
        "\n\r\n\r",
        " \n ",
        "\n`\n",
        "\n$\n",
        // A tab is not a space, so the parser strips the spaces around it.
        " \t ",
        "\n\t\n",
        "\t",
        " \u{a0} ",
    ];
    let mut trees = Vec::new();
    for value in values {
        for (prefix, literal) in [
            ("x ", code(value, None)),
            ("x | y ", code(value, None)),
            ("x ", math(value)),
            ("x | y ", math(value)),
        ] {
            trees.push(build_blocks(paragraph(vec![text(prefix), literal])));
        }
        trees.push(build_blocks(heading(vec![code(value, None)])));
        trees.push(build_blocks(heading(vec![text("x "), math(value)])));
        trees.push(build_blocks(table(vec![text("x "), code(value, None)])));
    }
    for tree in trees {
        let unnormalized = to_markdown(&tree).unwrap();
        let mut normal = tree;
        normal.normalize();
        assert!(normal.is_normalized());
        let markdown = to_markdown(&normal).unwrap();
        // The serializer writes each line ending as the space normalizing replaces it with.
        assert_eq!(unnormalized, markdown);
        let mut reparsed = Document::parse(markdown.as_str(), &math_text()).unwrap();
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

fn image(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(
        Tag::Image {
            destination: Text::Literal("u"),
            title: None,
        },
        TagKind::Image,
        children,
    )
}

#[test]
fn code_and_math_in_image_text_keep_line_endings() {
    // Image text writes code and math as alt text, where a line ending can be written and reads back, so normalizing leaves it, and the heading is still chosen as setext for it.
    let literals: [fn() -> Result<Event<'static>, StreamError>; 2] =
        [|| code("a\nb", None), || math("a\r\nb")];
    for (kind, literal) in literals.iter().enumerate() {
        let shapes = vec![
            build_blocks(paragraph(image(vec![literal()]))),
            build_blocks(heading(image(vec![literal()]))),
            build_blocks(attention(
                Tag::Heading(3),
                TagKind::Heading,
                image(vec![literal()]),
            )),
            build_blocks(table(image(vec![literal()]))),
            build_blocks(heading(concat([
                image(vec![literal()]),
                vec![text(" "), literal()],
            ]))),
        ];
        let expected = [
            "![a\nb](u)\n",
            "![a\nb](u)\n=====\n",
            "### ![a&#xA;b](u)\n",
            "| ![a&#xA;b](u) |\n| --- |\n",
            "![a\nb](u) `a b`\n===========\n",
        ];
        let last = shapes.len() - 1;
        for (index, mut tree) in shapes.into_iter().enumerate() {
            let markdown = to_markdown(&tree).unwrap();
            if kind == 0 {
                assert_eq!(markdown, expected[index]);
            }
            // Only the code or math outside the image changes.
            assert_eq!(tree.is_normalized(), index != last);
            tree.normalize();
            assert!(tree.is_normalized());
            assert_eq!(to_markdown(&tree).unwrap(), markdown);
            if index == last {
                let heading = tree.children(tree.root()).next().unwrap();
                let described = describe(&tree, heading);
                assert!(
                    described.ends_with(r#"" " code "a b")"#)
                        || described.ends_with(r#"" " math "a b")"#),
                    "{}",
                    described
                );
            }
            let mut reparsed = Document::parse(markdown.as_str(), &math_text()).unwrap();
            assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
            reparsed.normalize();
            assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
        }
    }
}

#[test]
fn code_and_math_of_spaces_around_a_tab_are_stable() {
    // The parser strips one space from each side of `  \t  `, as it is not only spaces, so the serializer pads it. Whitespace with a tab at an edge is not stripped, so it is written as it is.
    for source in [
        "x `  \t  `\n",
        "x $  \t  $\n",
        "x ` \t`\n",
        "x `\t`\n",
        "x ` \ta\t `\n",
    ] {
        let document = Document::parse(source, &math_text()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), source);
    }
}

/// A block of a list tree built through the document API, which keeps paragraph and item rendering hints in step with the list.
enum Block {
    Paragraph(Vec<EventKind<'static>>),
    /// A list's start and tightness, and each item's recorded spread and blocks.
    List(Option<u32>, bool, Vec<(bool, Vec<Block>)>),
    Code,
    Quote(Vec<Block>),
    Separator,
}

fn append_block(document: &mut Document<String>, parent: NodeId, block: &Block) {
    match block {
        Block::Paragraph(phrasing) => {
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
                let item = document
                    .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
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
                        language: None,
                        fence: None,
                        math: false,
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
    use Block::{Code, List, Paragraph};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let empties = || {
        [
            Paragraph(vec![]),
            Paragraph(vec![EventKind::SoftBreak(Text::Literal("\n"))]),
            Paragraph(vec![EventKind::TaskMarker(false)]),
            Paragraph(vec![EventKind::HardBreak]),
        ]
    };
    let nested = || List(None, true, vec![(false, vec![p("c")])]);
    // Each item records its spread both ways: as the parser infers it, from a blank line between its blocks, and as a hand-built tree may leave it, unset.
    for spread in [false, true] {
        let lone = |blocks: Vec<Block>| List(None, false, vec![(spread, blocks)]);
        for empty in empties() {
            // The only item of a loose list, left with one paragraph, cannot be written loose: no blank line can separate items or blocks, so normalizing makes the list tight (#128).
            let mut document = build_list(&lone(vec![empty, p("b")]));
            assert!(!document.is_normalized());
            assert_eq!(reads_back(document.clone()), "- b\n");
            document.normalize();
            assert!(tight_of_first_list(&document));
        }
        for (blocks, expected) in [
            (vec![p("b"), Paragraph(vec![])], "- b\n"),
            (vec![Paragraph(vec![])], "-\n"),
            (vec![Paragraph(vec![]), Code], "- ```\n  x\n  ```\n"),
            (vec![Paragraph(vec![]), nested()], "- - c\n"),
            // A task marker with text after it stays.
            (
                vec![
                    Paragraph(vec![EventKind::TaskMarker(false), text(" x")]),
                    Paragraph(vec![]),
                ],
                "- [ ] x\n",
            ),
            (
                vec![
                    Paragraph(vec![EventKind::TaskMarker(false)]),
                    Paragraph(vec![EventKind::TaskMarker(true), text(" x")]),
                ],
                "- [x] x\n",
            ),
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
                vec![
                    Paragraph(vec![EventKind::TaskMarker(false), text(" x")]),
                    Paragraph(vec![]),
                    p("b"),
                ],
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
    // A list whose one child is not an item is another shape, which is left as it is.
    let mut document = build_list(&List(None, false, vec![]));
    let list = document.children(document.root()).next().unwrap();
    document
        .append_child(list, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let paragraph = document.children(list).next().unwrap();
    document
        .append_child(paragraph, EventKind::Text(Text::from("a".to_owned())))
        .unwrap();
    assert!(document.is_normalized());
    // Separators between items and blocks are not counted.
    let mut document = build_list(&lone(vec![p()]));
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    for parent in [list, item] {
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
    // The parser infers an item's spread from a blank line between its blocks in the source, but a nested list or footnote definition takes in the blank line after it, so these items record no spread although their lists are loose. A blank line after a nested list is written anyway, as a paragraph cannot follow it directly, so that output is unchanged; one after a footnote definition was not, and the paragraph after it read back as part of the definition.
    for (source, expected) in [
        ("- a\n  - b\n\n  c\n", "- a\n  - b\n\n  c\n"),
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
    assert_linear(|size| {
        let mut events = Vec::new();
        for _ in 0..size {
            events.extend([
                start(Tag::List {
                    start: None,
                    tight: false,
                }),
                start(Tag::Item { block_end: false }),
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
    assert_normalize_linear(|size| {
        let mut blocks: Vec<Block> = (0..size).map(|_| Block::Paragraph(vec![])).collect();
        blocks.push(Block::Paragraph(vec![EventKind::Text(Text::from(
            "a".to_owned(),
        ))]));
        build_list(&Block::List(None, false, vec![(false, blocks)]))
    });
    // A lone item of many blocks, which is written spread.
    assert_normalize_linear(|size| {
        build_list(&Block::List(
            None,
            false,
            vec![(false, (0..size).map(|_| Block::Code).collect())],
        ))
    });
}

#[test]
fn an_empty_item_after_a_paragraph_in_a_tight_list_is_removed() {
    use Block::{List, Paragraph, Quote, Separator};
    let text = |value: &str| EventKind::Text(Text::from(value.to_owned()));
    let p = |value: &str| Paragraph(vec![text(value)]);
    let task = || EventKind::TaskMarker(false);
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
        (after_p(tight(vec![vec![Paragraph(vec![task()])]])), "- p\n"),
        (
            after_p(tight(vec![vec![Paragraph(vec![
                task(),
                EventKind::HardBreak,
            ])]])),
            "- p\n",
        ),
        (
            after_p(tight(vec![vec![Paragraph(vec![
                task(),
                EventKind::SoftBreak(Text::Literal("\n")),
            ])]])),
            "- p\n",
        ),
        (
            after_p(tight(vec![vec![Paragraph(vec![task()])], vec![p("a")]])),
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
        let block = after_p(tight(vec![vec![Paragraph(vec![task(), text(phrasing)])]]));
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
    // Separators between the items of the list are not counted either.
    let mut document = build_list(&after_p(tight(vec![])));
    let outer = document.children(document.root()).next().unwrap();
    let item = document.children(outer).next().unwrap();
    let list = document.children(item).nth(1).unwrap();
    let separator = || EventKind::Separator {
        value: Text::Literal("\n"),
        conditional: false,
    };
    document.append_child(list, separator()).unwrap();
    document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    document.append_child(list, separator()).unwrap();
    let nested = document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    append_block(&mut document, nested, &p("a"));
    assert!(!document.is_normalized());
    assert_eq!(reads_back(document), "- p\n  - a\n");
    // A list whose first child is not an item is another shape, which the serializer rejects, and is left as it is.
    let mut document = build_list(&after_p(tight(vec![])));
    let outer = document.children(document.root()).next().unwrap();
    let item = document.children(outer).next().unwrap();
    let list = document.children(item).nth(1).unwrap();
    append_block(&mut document, list, &p("a"));
    assert!(document.is_normalized());
    let before = canonical(&document);
    document.normalize();
    assert_eq!(canonical(&document), before);
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
