// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// Each path that builds or edits a tree checks the one content model (#194).

use base_markdown::{
    render_events,
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, to_markdown_node, AlignKind, CompileOptions, Document, NodeId, Options,
    ParseOptions,
};

fn empty() -> Document {
    Document::parse(String::new(), &ParseOptions::default()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output
}

fn link() -> EventKind<'static> {
    EventKind::Start(Tag::Link {
        destination: Text::Literal("u"),
        title: None,
    })
}

fn note() -> EventKind<'static> {
    EventKind::FootnoteReference(Text::Literal("n"))
}

fn first(document: &Document, id: NodeId) -> NodeId {
    document.children(id).next().unwrap()
}

#[test]
fn a_root_that_holds_blocks_holds_no_phrasing() {
    let mut document = empty();
    let root = document.root();
    for _ in 0..2 {
        document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
    }
    // Nor can an empty one hold an item.
    let mut other = empty();
    let other_root = other.root();
    assert!(matches!(
        other.append_child(
            other_root,
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: None,
            }),
        ),
        Err(StreamError::InvalidNesting)
    ));
    assert!(matches!(
        document.append_child(root, EventKind::Text(Text::Literal("a"))),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn no_footnote_reference_is_inside_a_link_at_any_depth() {
    let mut document = empty();
    let root = document.root();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let outer = document.append_child(paragraph, link()).unwrap();
    let emphasis = document
        .append_child(outer, EventKind::Start(Tag::Emphasis))
        .unwrap();
    assert!(matches!(
        document.append_child(emphasis, note()),
        Err(StreamError::InvalidNesting)
    ));
    // Nor can a subtree that holds one be attached there.
    let strong = document
        .append_child(paragraph, EventKind::Start(Tag::Strong))
        .unwrap();
    document.append_child(strong, note()).unwrap();
    assert!(matches!(
        document.attach(emphasis, strong, None),
        Err(StreamError::InvalidNesting)
    ));
    document.attach(paragraph, strong, None).unwrap();
    // The HTML writer checks the same rule on events.
    let events = vec![
        EventKind::Start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        EventKind::Start(Tag::Paragraph { tight: false }),
        link(),
        EventKind::Start(Tag::Emphasis),
        note(),
    ]
    .into_iter()
    .map(|kind| Ok(Event { kind, span: 0..0 }));
    let mut output = String::new();
    assert!(matches!(
        render_events("", events, &Options::default().compile, &mut output),
        Err(StreamError::InvalidNesting)
    ));
}

#[cfg(feature = "serde")]
#[test]
fn a_serialized_footnote_reference_in_a_link_is_rejected() {
    let document = Document::parse("[*a*](b)".to_owned(), &ParseOptions::default()).unwrap();
    let mut wire = serde_json::to_value(&document).unwrap();
    let text = wire["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["kind"].get("Text").is_some())
        .unwrap();
    serde_json::from_value::<Document>(wire.clone()).unwrap();
    wire["nodes"][text]["kind"] = serde_json::json!({"FootnoteReference": {"Value": "n"}});
    assert!(serde_json::from_value::<Document>(wire).is_err());
}

#[test]
fn a_heading_section_is_not_replaced_with_phrasing() {
    let mut document = Document::parse("# a\n\nb".to_owned(), &ParseOptions::default()).unwrap();
    // An inline document, whose root holds phrasing.
    let mut inline = empty();
    let inline_root = inline.root();
    inline
        .append_child(inline_root, EventKind::Text(Text::Literal("c")))
        .unwrap();
    let root = document.root();
    assert!(matches!(
        document.replace_heading_section(root, 0, &inline, true, true),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn kinds_change_only_within_their_class() {
    let mut document = Document::parse(
        "- [ ] a\n\n*b* <i>\n\n<div>\n".to_owned(),
        &ParseOptions::gfm(),
    )
    .unwrap();
    let list = first(&document, document.root());
    let item = first(&document, list);
    document
        .set_kind(
            item,
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: Some(true),
            }),
        )
        .unwrap();
    assert!(html(&document).contains("checked"));
    let paragraph = document.children(document.root()).nth(2).unwrap();
    let emphasis = first(&document, paragraph);
    document
        .set_kind(emphasis, EventKind::Start(Tag::Strong))
        .unwrap();
    let inline = document.children(paragraph).last().unwrap();
    let block = document.children(document.root()).last().unwrap();
    let block = if matches!(
        document.node(block).unwrap().kind,
        EventKind::Separator { .. }
    ) {
        document.children(document.root()).nth(4).unwrap()
    } else {
        block
    };
    let html_kind = |block| EventKind::Html {
        value: Text::Literal("<p>"),
        block,
    };
    document.set_kind(inline, html_kind(false)).unwrap();
    document.set_kind(block, html_kind(true)).unwrap();
    assert!(matches!(
        document.set_kind(inline, html_kind(true)),
        Err(StreamError::InvalidNesting)
    ));
    assert!(matches!(
        document.set_kind(block, html_kind(false)),
        Err(StreamError::InvalidNesting)
    ));
    assert!(html(&document).contains("<strong>b</strong>"));
}

#[test]
fn a_task_item_without_a_first_paragraph_has_no_checkbox() {
    let mut document = empty();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    let task = EventKind::Start(Tag::Item {
        block_end: false,
        checked: Some(true),
    });
    let item = document.append_child(list, task.clone()).unwrap();
    document
        .append_child(
            item,
            EventKind::CodeBlock {
                value: Text::Literal("a\n"),
                info: None,
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    document.append_child(list, task).unwrap();
    assert!(!html(&document).contains("checkbox"));
    let markdown = to_markdown(&document).unwrap();
    assert!(!markdown.contains('['), "{:?}", markdown);
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.descendants(document.root()).all(|id| !matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Item {
            checked: Some(_),
            ..
        })
    )));
}

#[test]
fn table_parts_are_written_on_their_own() {
    let document =
        Document::parse("| a | b |\n| - | - |\n".to_owned(), &ParseOptions::gfm()).unwrap();
    let table = first(&document, document.root());
    let head = first(&document, table);
    let row = first(&document, head);
    let cell = first(&document, row);
    let options = base_markdown::markdown::Options::default();
    assert_eq!(
        to_markdown_node(&document, row, &options).unwrap(),
        "| a | b |\n"
    );
    assert_eq!(to_markdown_node(&document, cell, &options).unwrap(), "a\n");
    assert!(matches!(
        document.node(cell).unwrap().kind,
        EventKind::Start(Tag::TableCell {
            alignment: AlignKind::None,
            header: true
        })
    ));
}

#[test]
fn frontmatter_without_recorded_fences_is_written_with_dashes() {
    let mut builder = base_markdown::AstBuilder::new();
    let events = vec![
        EventKind::Start(Tag::Document {
            line_ending: None,
            frontmatter: Some(Text::Literal("a: 1")),
        }),
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::Text(Text::Literal("b")),
        EventKind::End(TagKind::Paragraph),
        EventKind::End(TagKind::Document),
    ];
    builder
        .consume(
            &mut events
                .into_iter()
                .map(|kind| Ok(Event { kind, span: 0..0 })),
            usize::MAX,
        )
        .unwrap();
    let document = builder.finish(String::new()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "---\na: 1\n---\n\nb\n");
}

#[test]
fn text_content_includes_code_blocks() {
    let document =
        Document::parse("```\ncode\n```\n".to_owned(), &ParseOptions::default()).unwrap();
    assert_eq!(document.text_content(document.root()), "code\n");
}

fn document_start(frontmatter: Option<&'static str>) -> EventKind<'static> {
    EventKind::Start(Tag::Document {
        line_ending: None,
        frontmatter: frontmatter.map(Text::Literal),
    })
}

// Build and render the events, returning what the builder and the HTML writer report.
fn build_and_render(
    kinds: &[EventKind<'static>],
) -> (Result<(), StreamError>, Result<(), StreamError>) {
    let events = || {
        kinds
            .iter()
            .cloned()
            .map(|kind| Ok(Event { kind, span: 0..0 }))
    };
    let mut builder = base_markdown::AstBuilder::new();
    let built = builder
        .consume(&mut events(), usize::MAX)
        .and_then(|_| builder.finish(String::new()).map(drop));
    let mut output = String::new();
    let rendered = render_events("", events(), &Options::default().compile, &mut output);
    (built, rendered)
}

fn nesting(result: &Result<(), StreamError>) -> bool {
    matches!(result, Err(StreamError::InvalidNesting))
}

#[cfg(feature = "serde")]
fn deserialize_fails_nesting(nodes: serde_json::Value) {
    let wire = serde_json::json!({"source": "", "nodes": nodes, "syntax": {}});
    let error = serde_json::from_value::<Document>(wire).unwrap_err();
    assert!(error.to_string().contains("invalid nesting"), "{}", error);
}

#[test]
fn no_footnote_reference_is_built_inside_emphasis_in_a_link() {
    let (built, rendered) = build_and_render(&[
        document_start(None),
        EventKind::Start(Tag::Paragraph { tight: false }),
        link(),
        EventKind::Start(Tag::Emphasis),
        note(),
        EventKind::End(TagKind::Emphasis),
        EventKind::End(TagKind::Link),
        EventKind::End(TagKind::Paragraph),
        EventKind::End(TagKind::Document),
    ]);
    assert!(nesting(&built), "{:?}", built);
    assert!(nesting(&rendered), "{:?}", rendered);
}

#[test]
fn an_inline_root_holds_no_block() {
    let (built, rendered) = build_and_render(&[
        document_start(None),
        EventKind::Text(Text::Literal("a")),
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::End(TagKind::Paragraph),
        EventKind::End(TagKind::Document),
    ]);
    assert!(nesting(&built), "{:?}", built);
    assert!(nesting(&rendered), "{:?}", rendered);
    let mut document = empty();
    let root = document.root();
    document
        .append_child(root, EventKind::Text(Text::Literal("a")))
        .unwrap();
    assert!(matches!(
        document.append_child(root, EventKind::Start(Tag::Paragraph { tight: false })),
        Err(StreamError::InvalidNesting)
    ));
    // Nor can a detached block be attached to it.
    let mut document = empty();
    let root = document.root();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document.detach(paragraph).unwrap();
    document
        .append_child(root, EventKind::Text(Text::Literal("a")))
        .unwrap();
    assert!(matches!(
        document.attach(root, paragraph, None),
        Err(StreamError::InvalidNesting)
    ));
}

#[cfg(feature = "serde")]
#[test]
fn a_serialized_inline_root_holds_no_block() {
    deserialize_fails_nesting(serde_json::json!([
        {"kind": {"Start": {"Document": {"line_ending": null, "frontmatter": null}}}, "span": {"start": 0, "end": 0}, "parent": null, "first_child": 2, "next_sibling": null},
        {"kind": {"Text": {"Value": "a"}}, "span": {"start": 0, "end": 0}, "parent": 1, "first_child": null, "next_sibling": 3},
        {"kind": {"Start": {"Paragraph": {"tight": false}}}, "span": {"start": 0, "end": 0}, "parent": 1, "first_child": null, "next_sibling": null},
    ]));
}

#[test]
fn an_inline_root_takes_no_frontmatter() {
    let (built, rendered) = build_and_render(&[
        document_start(Some("x")),
        EventKind::Text(Text::Literal("a")),
        EventKind::End(TagKind::Document),
    ]);
    assert!(nesting(&built), "{:?}", built);
    assert!(nesting(&rendered), "{:?}", rendered);
    // Frontmatter is not set on an inline root.
    let mut document = empty();
    let root = document.root();
    document
        .append_child(root, EventKind::Text(Text::Literal("a")))
        .unwrap();
    assert!(matches!(
        document.set_kind(root, document_start(Some("x"))),
        Err(StreamError::InvalidNesting)
    ));
    assert_eq!(document.frontmatter(), None);
    // Nor is phrasing appended to a root with frontmatter, though a block is.
    let mut document = empty();
    let root = document.root();
    document.set_kind(root, document_start(Some("x"))).unwrap();
    assert!(matches!(
        document.append_child(root, EventKind::Text(Text::Literal("a"))),
        Err(StreamError::InvalidNesting)
    ));
    document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn a_serialized_root_with_frontmatter_holds_no_phrasing() {
    deserialize_fails_nesting(serde_json::json!([
        {"kind": {"Start": {"Document": {"line_ending": null, "frontmatter": {"Value": "x"}}}}, "span": {"start": 0, "end": 0}, "parent": null, "first_child": 2, "next_sibling": null},
        {"kind": {"Text": {"Value": "a"}}, "span": {"start": 0, "end": 0}, "parent": 1, "first_child": null, "next_sibling": null},
    ]));
}

#[test]
fn no_document_starts_inside_another() {
    let (built, rendered) = build_and_render(&[
        document_start(None),
        document_start(None),
        EventKind::End(TagKind::Document),
        EventKind::End(TagKind::Document),
    ]);
    assert!(nesting(&built), "{:?}", built);
    assert!(nesting(&rendered), "{:?}", rendered);
    // A second document after the first is unbalanced instead.
    let (_, rendered) = build_and_render(&[
        document_start(None),
        EventKind::End(TagKind::Document),
        document_start(None),
        EventKind::End(TagKind::Document),
    ]);
    assert!(
        matches!(rendered, Err(StreamError::Unbalanced)),
        "{:?}",
        rendered
    );
    let mut document = empty();
    let root = document.root();
    assert!(matches!(
        document.append_child(root, document_start(None)),
        Err(StreamError::InvalidNesting)
    ));
}

#[cfg(feature = "serde")]
#[test]
fn a_serialized_document_starts_inside_no_other() {
    deserialize_fails_nesting(serde_json::json!([
        {"kind": {"Start": {"Document": {"line_ending": null, "frontmatter": null}}}, "span": {"start": 0, "end": 0}, "parent": null, "first_child": 2, "next_sibling": null},
        {"kind": {"Start": {"Document": {"line_ending": null, "frontmatter": null}}}, "span": {"start": 0, "end": 0}, "parent": 1, "first_child": null, "next_sibling": null},
    ]));
}

#[test]
fn normalizing_ends_frontmatter_with_a_line_ending() {
    // The parser stores each line of the body with its line ending, so a body without a final one
    // gets one, and an empty body stays.
    for (body, normal, written) in [
        ("a", "a\n", "---\na\n---\n\nb\n"),
        ("a\n", "a\n", "---\na\n---\n\nb\n"),
        ("\n", "\n", "---\n\n---\n\nb\n"),
        ("", "", "---\n---\n\nb\n"),
    ] {
        let mut document = empty();
        let root = document.root();
        document.set_kind(root, document_start(Some(body))).unwrap();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::Literal("b")))
            .unwrap();
        assert_eq!(document.is_normalized(), body == normal, "{:?}", body);
        document.normalize();
        assert!(document.is_normalized());
        assert_eq!(document.frontmatter(), Some(normal));
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, written);
        let reparsed = Document::parse(markdown, &ParseOptions::default()).unwrap();
        assert_eq!(reparsed.frontmatter(), Some(normal));
    }
}

#[test]
fn frontmatter_line_endings_are_normalized_to_line_feeds() {
    for (body, normal) in [("a\rb", "a\nb\n"), ("a\r\nb", "a\nb\n"), ("a\r", "a\n")] {
        let mut document = empty();
        let root = document.root();
        document.set_kind(root, document_start(Some(body))).unwrap();
        assert!(!document.is_normalized());
        document.normalize();
        assert!(document.is_normalized());
        assert_eq!(document.frontmatter(), Some(normal));
        document.normalize();
        assert_eq!(document.frontmatter(), Some(normal));
        let reparsed =
            Document::parse(to_markdown(&document).unwrap(), &ParseOptions::default()).unwrap();
        assert_eq!(reparsed.frontmatter(), Some(normal));
    }
}
