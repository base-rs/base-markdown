// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The serializer recurses once per nesting level, so it fails on content nested more than 128
//! levels below the serialized node, counting every block and inline node, rather than overflow
//! the stack. 128 levels fit in a test thread's 2 MiB stack in a debug build, so these run on test
//! threads.

use base_markdown::{
    message::Message,
    normalize_markdown,
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, to_markdown_node, AlignKind, AstBuilder, Document, NodeId, ParseOptions,
};

const LIMIT: usize = 128;

fn event(kind: EventKind<'static>) -> Result<Event<'static>, StreamError> {
    Ok(Event { kind, span: 0..0 })
}

/// A document of `tags` nested in each other, outermost first, around text `a`. The text is
/// `tags.len() + 1` levels below the root.
fn nest(tags: &[Tag<'static>]) -> Document<String> {
    let mut events = vec![event(EventKind::Start(Tag::Document(None)))];
    events.extend(tags.iter().map(|tag| event(EventKind::Start(tag.clone()))));
    events.push(event(EventKind::Text(Text::Literal("a"))));
    events.extend(
        tags.iter()
            .rev()
            .map(|tag| event(EventKind::End(tag.kind()))),
    );
    events.push(event(EventKind::End(TagKind::Document)));
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    builder.finish(String::new()).unwrap()
}

/// `outer`, then `unit` repeated, then `inner`, with text `depth` levels below the root. Levels
/// that do not make a whole `unit` are `pad`, after `outer`.
fn levels(
    depth: usize,
    outer: &[Tag<'static>],
    unit: &[Tag<'static>],
    inner: &[Tag<'static>],
    pad: &Tag<'static>,
) -> Vec<Tag<'static>> {
    let fill = depth - 1 - outer.len() - inner.len();
    let mut tags = outer.to_vec();
    tags.extend(core::iter::repeat(pad.clone()).take(fill % unit.len()));
    tags.extend(unit.iter().cycle().take(fill - fill % unit.len()).cloned());
    tags.extend_from_slice(inner);
    tags
}

fn link() -> Tag<'static> {
    Tag::Link {
        destination: Text::Literal("u"),
        title: None,
    }
}

fn image() -> Tag<'static> {
    Tag::Image {
        destination: Text::Literal("u"),
        title: None,
    }
}

fn list(ordered: bool) -> Tag<'static> {
    Tag::List {
        start: ordered.then_some(1),
        tight: !ordered,
    }
}

fn item() -> Tag<'static> {
    Tag::Item { block_end: false }
}

fn footnote() -> Tag<'static> {
    Tag::FootnoteDefinition(Text::Literal("a"))
}

fn paragraph() -> Tag<'static> {
    Tag::Paragraph { tight: false }
}

fn cell() -> Tag<'static> {
    Tag::TableCell {
        alignment: AlignKind::None,
        header: true,
    }
}

/// Each recursive shape, as the tags around text `depth` levels below the root.
fn shapes(depth: usize) -> Vec<(&'static str, Vec<Tag<'static>>)> {
    let quote = Tag::BlockQuote;
    let block = |unit: &[Tag<'static>]| levels(depth, &[], unit, &[paragraph()], &quote);
    let inline = |outer: &[Tag<'static>], unit: &[Tag<'static>]| {
        levels(depth, outer, unit, &[], &Tag::Emphasis)
    };
    let mut quoted = vec![quote.clone(); depth / 2];
    quoted.push(paragraph());
    vec![
        ("block quotes", block(&[Tag::BlockQuote])),
        ("lists", block(&[list(false), item()])),
        ("ordered loose lists", block(&[list(true), item()])),
        ("footnote definitions", block(&[footnote()])),
        (
            "quotes, lists and footnote definitions",
            block(&[quote.clone(), list(false), item(), footnote()]),
        ),
        ("links", inline(&[paragraph()], &[link()])),
        (
            "emphasis and strong",
            inline(&[paragraph()], &[Tag::Emphasis, Tag::Strong]),
        ),
        (
            "strikethrough and emphasis",
            inline(&[paragraph()], &[Tag::Delete, Tag::Emphasis]),
        ),
        (
            "links and attention",
            inline(
                &[paragraph()],
                &[link(), Tag::Emphasis, Tag::Delete, Tag::Strong],
            ),
        ),
        (
            "a heading",
            inline(&[Tag::Heading(1)], &[Tag::Emphasis, link()]),
        ),
        (
            "a table cell",
            inline(
                &[Tag::Table, Tag::TableHead, Tag::TableRow, cell()],
                &[Tag::Strong, link()],
            ),
        ),
        (
            "quotes around attention",
            inline(&quoted, &[Tag::Emphasis, link()]),
        ),
    ]
}

fn assert_too_deep(result: Result<String, Message>, label: &str) {
    let error = result.expect_err(label);
    assert_eq!(error.rule_id, "nesting-too-deep", "{label}");
    assert_eq!(
        error.reason,
        "Content nested more than 128 levels deep cannot be serialized"
    );
}

#[test]
fn content_at_the_limit_serializes_and_deeper_content_is_an_error() {
    let at_limit: Vec<_> = shapes(LIMIT)
        .into_iter()
        .map(|(label, tags)| (label, nest(&tags)))
        .collect();
    // The text is one level too deep, and then the node around it too.
    let deeper: Vec<_> = shapes(LIMIT + 1)
        .into_iter()
        .chain(shapes(LIMIT + 2))
        .map(|(label, tags)| (label, nest(&tags)))
        .collect();
    for (label, document) in &at_limit {
        assert!(to_markdown(document).is_ok(), "{}", label);
    }
    for (label, document) in &deeper {
        assert_too_deep(to_markdown(document), label);
    }
    let quotes = &at_limit[0].1;
    assert_eq!(
        to_markdown(quotes).unwrap(),
        format!("{}a\n", "> ".repeat(LIMIT - 2))
    );
}

#[test]
fn each_part_of_a_table_counts_one_level() {
    // A table `depth` levels below the root, in block quotes, with text in its one cell.
    let table = |depth: usize, group: bool| {
        let mut tags = vec![Tag::BlockQuote; depth - 1];
        tags.push(Tag::Table);
        if group {
            tags.push(Tag::TableHead);
        }
        tags.extend([Tag::TableRow, cell()]);
        nest(&tags)
    };
    assert!(to_markdown(&table(LIMIT - 4, true)).is_ok());
    // The text, the cell, the row and the head are each one level too deep.
    for (depth, group, label) in [
        (LIMIT - 3, true, "text"),
        (LIMIT - 2, true, "cell"),
        (LIMIT - 1, true, "row in a head"),
        (LIMIT, true, "head"),
        (LIMIT, false, "row"),
    ] {
        let document = table(depth, group);
        assert_too_deep(to_markdown(&document), label);
    }
}

#[test]
fn image_text_is_written_flat_and_does_not_count() {
    // The issue's reproducer: links and images nested 10,000 deep. An image's text is written
    // without descending into it, so only the link and image outside it count.
    for (unit, expected) in [
        ([link(), image()], "[![a](u)](u)\n"),
        ([image(), link()], "![a](u)\n"),
    ] {
        let document = nest(&levels(10_000, &[paragraph()], &unit, &[], &Tag::Emphasis));
        assert_eq!(to_markdown(&document).unwrap(), expected);
    }
}

/// The node `depth` levels below the root of `document`, through first children.
fn descend(document: &Document<String>, depth: usize) -> NodeId {
    let mut id = document.root();
    for _ in 0..depth {
        id = document.node(id).unwrap().first_child().unwrap();
    }
    id
}

#[test]
fn levels_count_from_the_serialized_node() {
    let options = Default::default();
    let attention = || vec![Tag::Emphasis, Tag::Strong].into_iter().cycle();
    // Text `2 * LIMIT` levels deep, under quotes, a paragraph and attention.
    let mut tags = vec![Tag::BlockQuote; LIMIT];
    tags.push(paragraph());
    tags.extend(attention().take(LIMIT - 2));
    let quotes = nest(&tags);
    // Text `2 * LIMIT + 1` levels deep, under a paragraph and attention.
    let mut tags = vec![paragraph()];
    tags.extend(attention().take(2 * LIMIT - 1));
    let attention = nest(&tags);
    assert_too_deep(to_markdown(&quotes), "document");
    // The last quote, with the text `LIMIT` levels below it, and the one around it.
    assert!(to_markdown_node(&quotes, descend(&quotes, LIMIT), &options).is_ok());
    let quote = descend(&quotes, LIMIT - 1);
    assert_too_deep(to_markdown_node(&quotes, quote, &options), "quote");
    // Attention, which is written on its own outside phrasing, counts once.
    let node = descend(&attention, LIMIT + 1);
    assert!(to_markdown_node(&attention, node, &options).is_ok());
    let node = descend(&attention, LIMIT);
    assert_too_deep(to_markdown_node(&attention, node, &options), "attention");
}

#[test]
fn deeply_nested_parsed_documents_are_an_error() {
    let marker = |level: usize| if level % 2 == 0 { "*" } else { "_" };
    let mut attention: String = (0..16_000)
        .map(|level| format!("{}a ", marker(level)))
        .collect();
    attention.extend(
        (0..16_000)
            .rev()
            .map(|level| format!("z{} ", marker(level))),
    );
    let sources = [
        // 16 KB of block quote markers.
        format!("{} a\n", ">".repeat(16_000)),
        // Lists nested 8,000 deep.
        format!("{}a\n", "- ".repeat(8_000)),
        format!("{}a\n", "> 1. ".repeat(4_000)),
        attention,
    ];
    for source in &sources {
        // Parsing, normalizing and dropping the document do not recurse, and serializing stops at
        // the limit, so all of it fits in a test thread's stack.
        let mut document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
        document.normalize();
        assert!(document.is_normalized());
        assert_too_deep(to_markdown(&document), &source[..10]);
        assert_too_deep(normalize_markdown(source), &source[..10]);
    }
    // Quotes, a paragraph and text: the text is two levels below the last quote.
    let source = format!("{} a\n", ">".repeat(LIMIT - 2));
    assert_eq!(
        normalize_markdown(&source).unwrap(),
        format!("{}a\n", "> ".repeat(LIMIT - 2))
    );
    let source = format!("{} a\n", ">".repeat(LIMIT - 1));
    assert_too_deep(normalize_markdown(&source), "quotes");
}
