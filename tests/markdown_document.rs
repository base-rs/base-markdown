// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    stream::{EventKind, Tag, Text},
    to_markdown, to_markdown_node, Document, ParseOptions,
};

fn html(document: &Document<&str>) -> String {
    let mut html = String::new();
    document
        .render_html(&Default::default(), &mut html)
        .unwrap();
    html
}

#[test]
fn parsed_line_breaks_and_code_preserve_one_terminal_ending() {
    let document = Document::parse(
        "first  \nsecond\\\nthird\n\n```rust ignore\nline\n\n```\n",
        &ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "first\\\nsecond\\\nthird\n\n```rust ignore\nline\n\n```\n"
    );
}

#[test]
fn parsed_reference_labels_preserve_authored_spelling() {
    let source = "[label][MiXeD] ![alt][mixed]\n\n[MiXeD]: /url \"Title\"\n\nCall[^MiXeD].\n\n[^MiXeD]: Details.\n";
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), source);
}

#[test]
fn reference_semantics_survive_definition_edits_and_removal() {
    let source = "[label][Ref] ![alt][ref]\n\n[REF]: /old \"Old\"\n\n[ref]: /ignored\n";
    for operation in 0..3 {
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let original_html = html(&document);
        let definition = document
            .descendants(document.root())
            .find(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. }
                )
            })
            .unwrap();
        match operation {
            0 => document
                .set_kind(
                    definition,
                    EventKind::Definition {
                        label: Text::Literal("REF"),
                        destination: Text::Literal("/new"),
                        title: Some(Text::Literal("Old")),
                    },
                )
                .unwrap(),
            1 => document
                .set_kind(
                    definition,
                    EventKind::Definition {
                        label: Text::Literal("REF"),
                        destination: Text::Literal("/old"),
                        title: Some(Text::Literal("New")),
                    },
                )
                .unwrap(),
            _ => document.detach(definition).unwrap(),
        }
        let serialized = to_markdown(&document).unwrap();
        assert!(
            serialized.starts_with("[label](/old \"Old\") ![alt](/old \"Old\")\n"),
            "{}",
            serialized
        );
        let reparsed = Document::parse(serialized.as_str(), &ParseOptions::default()).unwrap();
        assert_eq!(html(&reparsed), original_html);
    }
}

#[test]
fn reference_subtrees_include_semantic_resources_without_external_definitions() {
    let document =
        Document::parse("*[label][ref]*\n\n[ref]: /url\n", &ParseOptions::default()).unwrap();
    let paragraph = document
        .children(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
        .unwrap();
    assert_eq!(
        to_markdown_node(&document, paragraph, &Default::default()).unwrap(),
        "*[label](/url)*\n"
    );
}

#[test]
fn unmodified_duplicate_definitions_use_the_first_target() {
    let source = "[label][ref]\n\n[REF]: /first\n\n[ref]: /second\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), source);
}
