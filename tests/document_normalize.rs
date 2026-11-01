// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, AstBuilder, CompileOptions, Document, NodeId, ParseOptions,
};
use std::borrow::Cow;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

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
        r#"paragraph("a" emphasis(link "http://a.b"(" b")))"#
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
fn keeps_whitespace_around_inline_html_at_the_start_of_a_paragraph() {
    // Without the whitespace, the HTML would start a line, where it starts an HTML block, or be
    // alone on its line, where it is one. Only the breaks at the edges are removed.
    assert_eq!(
        settled(build(vec![
            soft(),
            text(" "),
            html("<b>"),
            text(" "),
            hard()
        ])),
        r#"paragraph(" " Html { value: Literal("<b>"), block: false }() " ")"#
    );
    // Whitespace moved out of attention before the HTML stays too.
    assert_eq!(
        settled(build(concat([
            emphasis(vec![text(" ")]),
            vec![html("<b>")]
        ]))),
        r#"paragraph(" " Html { value: Literal("<b>"), block: false }())"#
    );
    // A heading or table cell cannot start an HTML block, and is trimmed.
    assert_eq!(
        settled(build_blocks(heading(vec![
            text(" "),
            html("<b>"),
            text(" ")
        ]))),
        r#"heading(Html { value: Literal("<b>"), block: false }())"#
    );
    for (source, expected) in [
        ("&#x20;<b>", "&#x20;<b>\n"),
        ("<b>&#x20;", "<b>&#x20;\n"),
        ("&#x20;&#x20;<b>\n- a", "&#x20; <b>\n\n- a\n"),
        ("- <b>&#x20;", "- <b>&#x20;\n"),
        ("&#xA;<div>", "&#xA;<div>\n"),
    ] {
        let mut document = Document::parse(source.to_owned(), &ParseOptions::normalize()).unwrap();
        document.normalize();
        assert!(document.is_normalized(), "{}", source);
        let output = to_markdown(&document).unwrap();
        assert_eq!(output, expected, "{source}");
        let mut reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
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
fn trims_the_edges_of_blocks() {
    assert_eq!(
        settled(build(vec![soft(), text(" \t\na b\n "), soft()])),
        r#"paragraph("a b")"#
    );
    // Line endings in values are LF, so a CR is a character reference, which the parser does not
    // trim, and it stays.
    assert_eq!(
        settled(build(vec![text("\r a\r\n")])),
        r#"paragraph("\r a\r")"#
    );
    // A no-break space is not trimmed, and neither is a hard break at the start.
    assert_eq!(
        settled(build_blocks(concat([
            heading(vec![text(" \u{a0}a\u{a0} ")]),
            paragraph(vec![hard(), text(" a")]),
        ]))),
        r#"heading("\u{a0}a\u{a0}") paragraph(break " a")"#
    );
    // Whitespace that moved out of attention is trimmed too, and so is a cell.
    assert_eq!(
        settled(build_blocks(table(concat([
            emphasis(vec![text(" a")]),
            strong(vec![text("b ")]),
        ])))),
        r#"Start(Table)(Start(TableHead)(Start(TableRow)(cell(emphasis("a") strong("b")))))"#
    );
}

#[test]
fn removes_hard_breaks_at_the_end_of_a_block() {
    assert_eq!(
        settled(build(concat([
            vec![text("a"), hard(), text(" "), hard()],
            emphasis(vec![text("b"), hard(), soft()]),
            vec![hard(), text("\n")],
        ]))),
        r#"paragraph("a" break " " break emphasis("b"))"#
    );
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
        r#"Start(Table)(Start(TableHead)(Start(TableRow)(cell("a b " emphasis("c") " " delete(emphasis("d"))))))"#
    );
}

#[test]
fn removes_empty_paragraphs_with_one_separator() {
    let blank = || paragraph(vec![text(" ")]);
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
    let mut document = build(vec![text(" ")]);
    let paragraph = document.children(document.root()).next().unwrap();
    document.detach(paragraph).unwrap();
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(describe(&document, paragraph), "paragraph()");
}

#[test]
fn keeps_task_paragraphs_untrimmed() {
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
    let mut document = build_blocks(item(vec![task(), text(" ")]));
    assert!(document.is_normalized());
    document.normalize();
    assert_eq!(
        describe(&document, document.root()),
        r#"Start(List { start: None, tight: true })(item false(paragraph(task " ")))"#
    );
    // Attention in it still moves its whitespace out.
    assert_eq!(
        settled(build_blocks(item(concat([
            vec![task(), text(" ")],
            emphasis(vec![text("a ")]),
        ])))),
        r#"Start(List { start: None, tight: true })(item false(paragraph(task " " emphasis("a") " ")))"#
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
        build(vec![text("a ")]),
        build(vec![soft(), text("a")]),
        build(vec![text("a"), hard()]),
        build_blocks(heading(vec![text("\ta")])),
        build_blocks(table(vec![text("a"), hard(), text("b")])),
        build_blocks(table(vec![text("a\n")])),
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
        build(vec![hard(), text("a")]),
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
