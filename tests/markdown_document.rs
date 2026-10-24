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

fn empty() -> Document<&'static str> {
    Document::parse("", &ParseOptions::default()).unwrap()
}

#[test]
fn synthetic_references_cannot_override_a_present_definition() {
    use core_markdown::{NodeSyntax, ReferenceKind};
    for target in ["/original", "/changed"] {
        let mut document = empty();
        let root = document.root();
        let link = document
            .append_child(
                root,
                EventKind::Start(Tag::Link {
                    destination: Text::Literal("/original"),
                    title: None,
                }),
            )
            .unwrap();
        document
            .append_child(link, EventKind::Text(Text::Literal("label")))
            .unwrap();
        document
            .set_syntax(
                link,
                Some(NodeSyntax::Reference {
                    kind: ReferenceKind::Full,
                    label: Text::Literal("ref"),
                }),
            )
            .unwrap();
        document
            .append_child(
                root,
                EventKind::Definition {
                    label: Text::Literal("ref"),
                    destination: Text::Literal(target),
                    title: None,
                },
            )
            .unwrap();
        let serialized = to_markdown(&document).unwrap();
        if target == "/original" {
            assert!(serialized.starts_with("[label][ref]"), "{}", serialized);
        } else {
            assert!(
                serialized.starts_with("[label](/original)"),
                "{}",
                serialized
            );
        }
    }
}

#[test]
fn detached_nodes_and_invalid_subtree_indices_are_handled() {
    let mut document = Document::parse("a\n\n- item\n", &ParseOptions::default()).unwrap();
    let text = document
        .descendants(document.root())
        .find(|&id| matches!(document.node(id).unwrap().kind, EventKind::Text(_)))
        .unwrap();
    document.detach(text).unwrap();
    assert_eq!(
        to_markdown_node(&document, text, &Default::default()).unwrap(),
        "a\n"
    );
    let item = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Item { .. })
            )
        })
        .unwrap();
    document.detach(item).unwrap();
    assert_eq!(
        to_markdown_node(&document, item, &Default::default()).unwrap(),
        "- item\n"
    );
    let other = empty();
    assert_eq!(
        to_markdown_node(&other, text, &Default::default())
            .unwrap_err()
            .rule_id,
        "invalid-node"
    );
}

#[test]
fn serialized_fragments_support_table_sections_and_nonrendered_metadata() {
    let document = Document::parse(
        "| A | B |\n| --- | --- |\n| x | y |\n",
        &ParseOptions::gfm(),
    )
    .unwrap();
    for (tag, expected) in [
        (Tag::TableHead, "| A | B |\n"),
        (Tag::TableBody, "| x | y |\n"),
    ] {
        let id = document
            .descendants(document.root())
            .find(|&id| document.node(id).unwrap().kind == EventKind::Start(tag.clone()))
            .unwrap();
        assert_eq!(
            to_markdown_node(&document, id, &Default::default()).unwrap(),
            expected
        );
    }
    let mut document = empty();
    for (kind, expected) in [
        (EventKind::TaskMarker(true), "[x]\n"),
        (EventKind::TaskMarker(false), "[ ]\n"),
        (
            EventKind::Opaque {
                value: Text::Literal("raw metadata"),
                block: true,
            },
            "raw metadata\n",
        ),
        (
            EventKind::FootnoteReference(Text::Literal("NOTE")),
            "[^NOTE]\n",
        ),
        (
            EventKind::Separator {
                value: Text::Literal("\n\n"),
                conditional: false,
            },
            "",
        ),
    ] {
        let id = document.append_child(document.root(), kind).unwrap();
        assert_eq!(
            to_markdown_node(&document, id, &Default::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn structured_image_alt_and_link_text_use_their_semantic_content() {
    let source = "![*one* two](/image) ![`code`](/code) ![](/empty) [one *two*](/link) [*emphasis*](/emphasis)";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "![one two](/image) ![code](/code) ![](/empty) [one *two*](/link) [*emphasis*](/emphasis)\n");
}

#[test]
fn synthetic_math_blocks_use_semantic_info_without_authored_syntax() {
    use core_markdown::NodeSyntax;
    let mut document = empty();
    let block = document
        .append_child(
            document.root(),
            EventKind::CodeBlock {
                value: Text::Literal("x\n"),
                language: Some(Text::Literal("latex")),
                fence: None,
                math: true,
                unclosed: false,
            },
        )
        .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "$$latex\nx\n$$\n");
    document
        .set_kind(
            block,
            EventKind::CodeBlock {
                value: Text::Literal("plain\n"),
                language: None,
                fence: None,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    document
        .set_syntax(
            block,
            Some(NodeSyntax::Code {
                info: Text::Literal("ignored metadata"),
            }),
        )
        .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "```\nplain\n```\n");
}

#[test]
fn multiline_inline_literals_choose_setext_heading_syntax() {
    for (kind, expected) in [
        (
            EventKind::Code {
                value: Text::Literal("one\ntwo"),
                language: None,
            },
            "`one\ntwo`\n====\n",
        ),
        (
            EventKind::Math(Text::Literal("one\ntwo")),
            "$one\ntwo$\n====\n",
        ),
    ] {
        let mut document = empty();
        let heading = document
            .append_child(document.root(), EventKind::Start(Tag::Heading(1)))
            .unwrap();
        document.append_child(heading, kind).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), expected);
    }
}

#[test]
fn block_leaves_inside_a_heading_are_not_representable_in_markdown() {
    for kind in [
        EventKind::CodeBlock {
            value: Text::Literal("code\n"),
            language: None,
            fence: None,
            math: false,
            unclosed: false,
        },
        EventKind::Opaque {
            value: Text::Literal("metadata"),
            block: true,
        },
    ] {
        let mut document = empty();
        let heading = document
            .append_child(document.root(), EventKind::Start(Tag::Heading(1)))
            .unwrap();
        document.append_child(heading, kind).unwrap();
        assert_eq!(
            to_markdown(&document).unwrap_err().rule_id,
            "unsupported-node"
        );
    }
}

#[test]
fn table_serialization_rejects_unrepresentable_structure() {
    for parent in [Tag::Table, Tag::TableHead, Tag::TableBody, Tag::TableRow] {
        let mut document = empty();
        let container = document
            .append_child(document.root(), EventKind::Start(parent))
            .unwrap();
        document
            .append_child(container, EventKind::Text(Text::Literal("unexpected")))
            .unwrap();
        assert_eq!(
            to_markdown(&document).unwrap_err().rule_id,
            "unsupported-node"
        );
    }
    let mut document = empty();
    let table = document
        .append_child(document.root(), EventKind::Start(Tag::Table))
        .unwrap();
    let group = document
        .append_child(table, EventKind::Start(Tag::TableBody))
        .unwrap();
    document
        .append_child(group, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unsupported-node"
    );
}

#[test]
fn table_separator_metadata_does_not_add_empty_rows_or_cells() {
    use core_markdown::AlignKind;
    let mut document = empty();
    let table = document
        .append_child(document.root(), EventKind::Start(Tag::Table))
        .unwrap();
    let group = document
        .append_child(table, EventKind::Start(Tag::TableHead))
        .unwrap();
    let row = document
        .append_child(group, EventKind::Start(Tag::TableRow))
        .unwrap();
    let cell = document
        .append_child(
            row,
            EventKind::Start(Tag::TableCell {
                alignment: AlignKind::Left,
                header: true,
            }),
        )
        .unwrap();
    document
        .append_child(cell, EventKind::Text(Text::Literal("value")))
        .unwrap();
    for parent in [table, group, row, cell] {
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
    assert_eq!(to_markdown(&document).unwrap(), "| value |\n| :--- |\n");
}

#[test]
fn empty_item_paragraphs_and_math_without_info_serialize() {
    let mut document = empty();
    let item = document
        .append_child(
            document.root(),
            EventKind::Start(Tag::Item { block_end: false }),
        )
        .unwrap();
    document
        .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "-\n");
    let mut document = empty();
    document
        .append_child(
            document.root(),
            EventKind::CodeBlock {
                value: Text::Literal("x\n"),
                language: None,
                fence: None,
                math: true,
                unclosed: false,
            },
        )
        .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "$$\nx\n$$\n");
}

#[test]
fn list_children_must_be_items() {
    let mut document = empty();
    let list = document
        .append_child(
            document.root(),
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    document
        .append_child(list, EventKind::Text(Text::Literal("missing an item")))
        .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap_err().rule_id,
        "unsupported-node"
    );
}

#[test]
fn nul_language_normalization_preserves_code_metadata_for_both_producers() {
    let source = "```r\0ust extra &amp;\nx\n```\n";
    for grammar in [false, true] {
        let options = ParseOptions {
            html_tag_namespaces: grammar,
            ..ParseOptions::default()
        };
        let document = Document::parse(source, &options).unwrap();
        assert_eq!(
            to_markdown(&document).unwrap(),
            "```r�ust extra &\nx\n```\n"
        );
    }
}
