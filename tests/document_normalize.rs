// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, AstBuilder, Document, NodeId, ParseOptions,
};
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
        "paragraph(emphasis())"
    );
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
        let choice = random.below(if depth == 0 { 3 } else { 7 });
        match choice {
            0 => events.push(text("")),
            1 => events.push(text(["a", "*", " ", "_"][random.below(4) as usize])),
            2 => events.push(separator()),
            3 => events.push(event(EventKind::HardBreak, 0..0)),
            4 => events.extend(emphasis(random_phrasing(random, depth - 1))),
            5 => events.extend(strong(random_phrasing(random, depth - 1))),
            _ => events.extend(delete(random_phrasing(random, depth - 1))),
        }
    }
    events
}

#[test]
fn normalize_makes_any_tree_normalized_and_is_idempotent() {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let mut unnormalized = 0;
    for _ in 0..5_000 {
        let mut document = build(random_phrasing(&mut random, 4));
        unnormalized += usize::from(!document.is_normalized());
        // Serializing a tree that is not normalized may be wrong, but it must not panic.
        to_markdown(&document).unwrap();
        document.normalize();
        assert!(document.is_normalized());
        let once = describe(&document, document.root());
        let markdown = to_markdown(&document).unwrap();
        document.normalize();
        assert_eq!(describe(&document, document.root()), once);
        assert_eq!(to_markdown(&document).unwrap(), markdown);
    }
    assert!(unnormalized > 1_000, "{}", unnormalized);
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
