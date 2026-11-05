// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    stream::{EventKind, StreamError, Tag, Text},
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
    use base_markdown::{stream::Reference, ReferenceKind};
    for target in ["/original", "/changed"] {
        let mut document = empty();
        let root = document.root();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        let link = document
            .append_child(
                paragraph,
                EventKind::Start(Tag::Link {
                    destination: Text::Literal("/original"),
                    title: None,
                    reference: Some(Box::new(Reference {
                        kind: ReferenceKind::Full,
                        label: Text::Literal("ref"),
                    })),
                }),
            )
            .unwrap();
        document
            .append_child(link, EventKind::Text(Text::Literal("label")))
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

/// What a footnote label normalizes to, for an ASCII label: its runs of whitespace as one space,
/// trimmed, and uppercase.
fn identifier(label: &str) -> String {
    label
        .split([' ', '\t', '\r', '\n'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

/// A label is written as it is, so one that the parser would not read back as itself is rejected
/// wherever it is assigned, with one rule for every label: a definition's, a reference's, and a
/// footnote call's or definition's (#146, #192). A shortcut or collapsed reference's label is its
/// link text, which the parser reads at any size, and a footnote definition's holds no whitespace.
/// The parser counts a tab as the columns it spans, so a label fits when it fits at some column of
/// each of its lines.
#[test]
fn labels_the_parser_cannot_read_are_rejected() {
    use base_markdown::{
        stream::{Reference, StreamError},
        ReferenceKind,
    };
    let mut document = empty();
    let root = document.root();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let link = document
        .append_child(
            paragraph,
            EventKind::Start(Tag::Link {
                destination: Text::Literal("/u"),
                title: None,
                reference: None,
            }),
        )
        .unwrap();
    let footnote = document
        .append_child(
            paragraph,
            EventKind::FootnoteReference {
                identifier: Text::Literal("1"),
                label: None,
            },
        )
        .unwrap();
    let definition = document
        .append_child(
            root,
            EventKind::Definition {
                label: Text::Literal("1"),
                destination: Text::Literal("/u"),
                title: None,
            },
        )
        .unwrap();
    let footnote_definition = document
        .append_child(
            root,
            EventKind::Start(Tag::FootnoteDefinition {
                identifier: Text::Literal("1"),
                label: None,
            }),
        )
        .unwrap();
    let reference = |kind: ReferenceKind, label: &str| {
        EventKind::Start(Tag::Link {
            destination: Text::Literal("/u"),
            title: None,
            reference: Some(Box::new(Reference {
                kind,
                label: Text::from(label.to_string()),
            })),
        })
    };
    let check = |result: Result<(), StreamError>, valid: bool, name: &str| {
        if valid {
            assert!(result.is_ok(), "{:?}: {:?}", name, result);
        } else {
            assert!(
                matches!(result, Err(StreamError::InvalidValue)),
                "{:?}: {:?}",
                name,
                result
            );
        }
    };
    let a = |count: usize| "a".repeat(count);
    let tabs = |count: usize| "\t".repeat(count);
    // Each label, whether it reads back where its size is limited, as a definition's, a full
    // reference's and a footnote call's, as link text, and as a footnote definition's.
    for (label, limited, text, defined) in [
        (a(999), true, true, true),
        (format!("{}\n  {}", a(499), a(500)), true, true, false),
        (a(1000), false, true, false),
        (format!("{}\\]", a(997)), true, true, true),
        (format!("{}\\\\", a(997)), true, true, true),
        (format!("{}\\]", a(998)), false, true, false),
        // Each tab after the first of a run spans four columns, and the first at least one.
        (format!("{}a", tabs(300)), false, true, false),
        (format!("{}{}", tabs(249), a(6)), true, true, false),
        (format!("{}{}", tabs(249), a(7)), false, true, false),
        // After a letter, a tab spans one column only where the letter ends a column of three.
        (format!("{}\t{}", a(3), a(995)), true, true, false),
        (format!("{}\t{}", a(3), a(996)), false, true, false),
        // Each line starts at the column that suits it best, and the whitespace that starts a
        // line after the first does not count.
        (
            format!("{0}{1}\n{0}a{0}{1}", tabs(124), a(6)),
            true,
            true,
            false,
        ),
        (
            format!("{0}{1}\n{0}a{0}{1}a", tabs(124), a(6)),
            false,
            true,
            false,
        ),
        // A tab after a backslash is not escaped, and ends the escape, so a bracket after it
        // needs a backslash of its own, which counts.
        ("\\\t]".into(), false, false, false),
        (format!("{}\\\t]", a(997)), false, false, false),
        (format!("{}\\\t\\]", a(995)), true, true, false),
        (format!("{}\\\t\\]", a(996)), false, true, false),
        // Something other than whitespace, and no blank line, carriage return, bracket without a
        // backslash before it or backslash before the closing bracket.
        (String::new(), false, false, false),
        (" \n ".into(), false, false, false),
        ("a b".into(), true, true, false),
        ("a\tb".into(), true, true, false),
        ("a\nb".into(), true, true, false),
        ("a\n".into(), true, true, false),
        ("\na".into(), true, true, false),
        ("a\r\nb".into(), false, false, false),
        ("a\rb".into(), false, false, false),
        ("a\n\nb".into(), false, false, false),
        ("a\n \t\nb".into(), false, false, false),
        ("a]b".into(), false, false, false),
        ("a[b".into(), false, false, false),
        ("a\\[b".into(), true, true, true),
        ("a\\b".into(), true, true, true),
        ("a\\".into(), false, false, false),
        ("a\\\\\\".into(), false, false, false),
        ("a\\\nb".into(), true, true, false),
        // A line after the first may start a block where it would continue a paragraph, as the
        // parser reads one as part of the label where that block is turned off, so validity
        // does not depend on the options; normalizing gives such a line an indent.
        ("a\n#b".into(), true, true, false),
        ("a\n# b".into(), true, true, false),
        ("a\n   # b".into(), true, true, false),
        ("a\n    # b".into(), true, true, false),
        ("a\n\t# b".into(), true, true, false),
        ("a\n  \t# b".into(), true, true, false),
        ("a\n> b".into(), true, true, false),
        ("a\n```".into(), true, true, false),
        ("a\n```\nb".into(), true, true, false),
        ("a\n~~~ b".into(), true, true, false),
        ("a\n- b".into(), true, true, false),
        ("a\n-".into(), true, true, false),
        ("a\n-\nb".into(), true, true, false),
        ("a\n+ b".into(), true, true, false),
        ("a\n*b".into(), true, true, false),
        ("a\n1. b".into(), true, true, false),
        ("a\n2. b".into(), true, true, false),
        ("a\n===".into(), true, true, false),
        ("a\n===\nb".into(), true, true, false),
        ("a\n***\nb".into(), true, true, false),
        ("a\n<div>\nb".into(), true, true, false),
        ("a\n<!--".into(), true, true, false),
        ("a\n<span>\nb".into(), true, true, false),
    ] {
        check(
            document.set_kind(link, reference(ReferenceKind::Full, &label)),
            limited,
            &label,
        );
        for kind in [ReferenceKind::Collapsed, ReferenceKind::Shortcut] {
            check(
                document.set_kind(link, reference(kind, &label)),
                text,
                &label,
            );
        }
        check(
            document.set_kind(
                definition,
                EventKind::Definition {
                    label: Text::from(label.clone()),
                    destination: Text::Literal("/u"),
                    title: None,
                },
            ),
            limited,
            &label,
        );
        let identifier = identifier(&label);
        check(
            document.set_kind(
                footnote,
                EventKind::FootnoteReference {
                    identifier: Text::from(identifier.clone()),
                    label: Some(Text::from(label.clone())),
                },
            ),
            limited,
            &label,
        );
        check(
            document.set_kind(
                footnote_definition,
                EventKind::Start(Tag::FootnoteDefinition {
                    identifier: Text::from(identifier.clone()),
                    label: Some(Text::from(label.clone())),
                }),
            ),
            defined,
            &label,
        );
    }
    // A footnote's identifier is what its label normalizes to, and without a label, which its
    // identifier is then written as, it is normalized itself.
    for (identifier, label, valid) in [
        ("A", Some("a"), true),
        ("a", Some("a"), false),
        ("A B", Some(" a\n\tb "), true),
        ("A  B", Some("a  b"), false),
        ("B", Some("a"), false),
        ("A", None, true),
        ("A B", None, true),
        ("a", None, false),
        ("A  B", None, false),
        (" A", None, false),
    ] {
        let name = format!("{:?} {:?}", identifier, label);
        let label = label.map(Text::Literal);
        check(
            document.set_kind(
                footnote,
                EventKind::FootnoteReference {
                    identifier: Text::Literal(identifier),
                    label: label.clone(),
                },
            ),
            valid,
            &name,
        );
        // A definition's label holds no whitespace, nor then does its identifier.
        let defined = valid && !identifier.contains(' ');
        check(
            document.set_kind(
                footnote_definition,
                EventKind::Start(Tag::FootnoteDefinition {
                    identifier: Text::Literal(identifier),
                    label,
                }),
            ),
            defined,
            &name,
        );
    }
    // An identifier out of the source's range is reported as such, as is a label.
    for (identifier, label) in [
        (Text::Source(0..99), Some(Text::Literal("a"))),
        (Text::Literal("A"), Some(Text::Source(0..99))),
        (Text::Source(0..99), None),
    ] {
        let kind = EventKind::FootnoteReference {
            identifier: identifier.clone(),
            label: label.clone(),
        };
        assert!(matches!(
            document.set_kind(footnote, kind),
            Err(StreamError::InvalidRange)
        ));
    }
    // A parsed identifier can be longer than its label through case folding.
    let label = "ΐ".repeat(499);
    let source = format!("[^{label}]\n\n[^{label}]: x\n");
    let parsed = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
    let call = parsed
        .descendants(parsed.root())
        .find(|&id| {
            matches!(
                parsed.node(id).unwrap().kind,
                EventKind::FootnoteReference { .. }
            )
        })
        .unwrap();
    let EventKind::FootnoteReference { identifier, .. } = &parsed.node(call).unwrap().kind else {
        unreachable!("found a call");
    };
    assert!(identifier.resolve(parsed.source()).unwrap().len() > 999);
    let kind = parsed.node(call).unwrap().kind.clone();
    let mut parsed = parsed;
    parsed.set_kind(call, kind).unwrap();
    assert_eq!(to_markdown(&parsed).unwrap(), source);
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
        "a"
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
    for (kind, expected) in [
        (
            EventKind::FootnoteReference {
                identifier: Text::Literal("NOTE"),
                label: None,
            },
            "[^NOTE]",
        ),
        (
            EventKind::Separator {
                value: Text::Literal("\n\n"),
                conditional: false,
            },
            "",
        ),
    ] {
        let mut document = empty();
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
    assert_eq!(to_markdown(&document).unwrap(), "![*one* two](/image) ![`code`](/code) ![](/empty) [one *two*](/link) [*emphasis*](/emphasis)\n");
}

#[test]
fn multiline_inline_literals_write_line_endings_as_spaces_in_atx_headings() {
    for (kind, expected) in [
        (
            EventKind::Code {
                value: Text::Literal("one\ntwo"),
                language: None,
            },
            "# `one two`\n",
        ),
        (
            EventKind::Code {
                value: Text::Literal("one\r\ntwo"),
                language: None,
            },
            "# `one two`\n",
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
fn block_leaves_inside_a_heading_are_rejected() {
    let mut document = empty();
    let heading = document
        .append_child(document.root(), EventKind::Start(Tag::Heading(1)))
        .unwrap();
    assert!(matches!(
        document.append_child(
            heading,
            EventKind::CodeBlock {
                value: Text::Literal("code\n"),
                info: None,
                fence: None,
                unclosed: false,
            },
        ),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn table_parts_hold_only_their_own_parts() {
    for chain in [
        &[Tag::Table][..],
        &[Tag::Table, Tag::TableHead],
        &[Tag::Table, Tag::TableBody],
        &[Tag::Table, Tag::TableBody, Tag::TableRow],
    ] {
        let mut document = empty();
        let mut container = document.root();
        for tag in chain {
            container = document
                .append_child(container, EventKind::Start(tag.clone()))
                .unwrap();
        }
        assert!(matches!(
            document.append_child(container, EventKind::Text(Text::Literal("unexpected"))),
            Err(StreamError::InvalidNesting)
        ));
    }
    let mut document = empty();
    let table = document
        .append_child(document.root(), EventKind::Start(Tag::Table))
        .unwrap();
    let group = document
        .append_child(table, EventKind::Start(Tag::TableBody))
        .unwrap();
    assert!(matches!(
        document.append_child(group, EventKind::Start(Tag::Paragraph { tight: false })),
        Err(StreamError::InvalidNesting)
    ));
    // A row directly in a table is rejected too: it is in a head or body.
    assert!(matches!(
        document.append_child(table, EventKind::Start(Tag::TableRow)),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn table_separator_metadata_does_not_add_empty_rows_or_cells() {
    use base_markdown::AlignKind;
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
    let separator = || EventKind::Separator {
        value: Text::Literal("\n"),
        conditional: false,
    };
    // Separators sit between a table's parts and rows, where the parser writes them, and not in a
    // row or cell.
    for parent in [table, group] {
        document.append_child(parent, separator()).unwrap();
    }
    for parent in [row, cell] {
        assert!(matches!(
            document.append_child(parent, separator()),
            Err(StreamError::InvalidNesting)
        ));
    }
    assert_eq!(to_markdown(&document).unwrap(), "| value |\n| :--- |\n");
}

#[test]
fn empty_item_paragraphs_and_code_without_info_serialize() {
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
    let item = document
        .append_child(
            list,
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: None,
            }),
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
                info: None,
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "```\nx\n```\n");
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
    assert!(matches!(
        document.append_child(list, EventKind::Text(Text::Literal("missing an item"))),
        Err(StreamError::InvalidNesting)
    ));
    // And an item is only ever in a list.
    assert!(matches!(
        document.append_child(
            document.root(),
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: None,
            }),
        ),
        Err(StreamError::InvalidNesting)
    ));
}

#[test]
fn nul_language_normalization_preserves_code_metadata_for_both_producers() {
    let source = "```r\0ust extra &amp;\nx\n```\n";
    for configured in [false, true] {
        let options = ParseOptions {
            html_tag_namespaces: configured,
            ..ParseOptions::commonmark()
        };
        let document = Document::parse(source, &options).unwrap();
        assert_eq!(
            to_markdown(&document).unwrap(),
            "```r�ust extra &\nx\n```\n"
        );
    }
}

#[test]
fn ordered_marker_escaping_uses_markdown_ascii_digits() {
    for (source, expected) in [
        ("١. ordinary", "١. ordinary\n"),
        ("١) ordinary", "١) ordinary\n"),
        ("12. ordinary", "12. ordinary\n"),
    ] {
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), expected);
    }
}

#[test]
fn ordered_list_positions_survive_nested_and_direct_item_serialization() {
    let source = "9. outer\n   1. nested\n   2. nested again\n10. next\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), source);
    let list = document.children(document.root()).next().unwrap();
    let second = document.children(list).nth(1).unwrap();
    assert_eq!(
        to_markdown_node(&document, second, &Default::default()).unwrap(),
        "10. next\n"
    );
    let mut options = base_markdown::MarkdownOptions {
        bullet_ordered: ')',
        ..Default::default()
    };
    assert_eq!(
        to_markdown_node(&document, second, &options).unwrap(),
        "10) next\n"
    );
    options.increment_list_marker = false;
    assert_eq!(
        to_markdown_node(&document, second, &options).unwrap(),
        "9) next\n"
    );
    options.bullet_ordered = 'x';
    assert!(to_markdown_node(&document, second, &options).is_err());

    let source: String = (9..265)
        .map(|number| format!("{number}. value\n"))
        .collect();
    let document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), source);
    let options = base_markdown::MarkdownOptions {
        increment_list_marker: false,
        ..Default::default()
    };
    assert_eq!(
        base_markdown::to_markdown_with_options(&document, &options).unwrap(),
        "9. value\n".repeat(256)
    );
}
