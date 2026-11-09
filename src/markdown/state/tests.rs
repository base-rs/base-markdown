// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{Info, State};
use crate::{
    markdown::Options,
    stream::{EventKind, Tag, Text},
    Document, NodeId, NodeSyntax, ParseOptions,
};
use alloc::string::String;

fn empty() -> Document<String> {
    Document::parse(String::new(), &ParseOptions::default()).unwrap()
}
fn paragraph(document: &mut Document<String>, parent: NodeId, value: &str) -> NodeId {
    let node = document
        .append_child(parent, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document
        .append_child(node, EventKind::Text(Text::from(String::from(value))))
        .unwrap();
    node
}

#[test]
fn serializes_inline_documents_and_footnote_subtrees() {
    let options = Options::default();
    let mut state = State::new(&options);
    let mut document = empty();
    let root = document.root();
    document
        .append_child(root, EventKind::Text(Text::Literal("inline")))
        .unwrap();
    assert_eq!(
        state.handle(&document, root, &Info::new("", "")).unwrap(),
        "inline"
    );

    let mut document = empty();
    let note = document
        .append_child(
            document.root(),
            EventKind::Start(Tag::FootnoteDefinition {
                identifier: Text::Literal("NOTE"),
                label: Some(Text::Literal("note")),
            }),
        )
        .unwrap();
    paragraph(&mut document, note, "first");
    paragraph(&mut document, note, "second");
    let rendered = state.handle(&document, note, &Info::new("", "")).unwrap();
    assert!(rendered.contains("    second"), "{:?}", rendered);
}

#[test]
fn handles_adjacent_text_nodes_and_phrasing_lookahead_errors() {
    let options = Options::default();
    let mut state = State::new(&options);
    let mut document = empty();
    let root = document.root();
    let node = paragraph(&mut document, root, "first");
    document
        .append_child(node, EventKind::Text(Text::Literal("second")))
        .unwrap();
    state
        .container_phrasing(&document, node, &Info::new("", ""))
        .unwrap();
    assert_eq!(core::mem::take(&mut state.output), "firstsecond");

    let emphasis = document
        .append_child(node, EventKind::Start(Tag::Emphasis))
        .unwrap();
    document
        .append_child(emphasis, EventKind::Text(Text::Literal("third")))
        .unwrap();
    let options = Options {
        emphasis: 'x',
        ..Options::default()
    };
    let error = State::new(&options)
        .container_phrasing(&document, node, &Info::new("", ""))
        .unwrap_err();
    assert_eq!(error.rule_id, "unexpected-marker");
}

#[test]
fn literal_lookahead_matches_rendering_across_scopes_and_wrapping_modes() {
    use crate::markdown::{construct_name::ConstructName, LineWrapping};
    let mut document = empty();
    let text = document
        .append_child(document.root(), EventKind::Text(Text::Literal("")))
        .unwrap();
    assert_eq!(
        State::new(&Options::default()).peek_node(&document, text, document.root()),
        None
    );
    let scopes = [
        ConstructName::Phrasing,
        ConstructName::Label,
        ConstructName::DestinationRaw,
        ConstructName::HeadingAtx,
        ConstructName::CodeFencedInfoGraveAccent,
    ];
    for line_wrapping in [LineWrapping::Preserve, LineWrapping::Unwrap] {
        let options = Options {
            line_wrapping,
            ..Options::default()
        };
        for scope in scopes {
            // Text holds no NUL, which the parser stores as U+FFFD.
            for first in (1..=127).map(char::from).chain(['é', 'α', '\u{a0}', '世']) {
                for after in ["", "ordinary", "*", "\\!", "\r\n# two", "\n12. two"] {
                    document
                        .set_kind(
                            text,
                            EventKind::Text(Text::from(alloc::format!("{first}{after}"))),
                        )
                        .unwrap();
                    let mut state = State::new(&options);
                    state.enter(scope);
                    if let Some(prefix) = state.peek_node(&document, text, document.root()) {
                        state
                            .write_node(&document, text, &Info::new("", ""))
                            .unwrap();
                        assert_eq!(state.output.chars().next(), Some(prefix));
                    }
                }
            }
        }
    }
}

#[test]
fn joins_code_and_spread_items_consistently() {
    let options = Options {
        fences: false,
        ..Options::default()
    };
    let state = State::new(&options);
    let mut document = empty();
    let root = document.root();
    let code = document
        .append_child(
            root,
            EventKind::CodeBlock {
                value: Text::Literal("indented\n"),
                info: None,
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    assert!(matches!(
        state.default_separator(&document, None, code, code, root, false),
        "\n\n<!---->\n\n"
    ));
    let list = document
        .append_child(
            root,
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
    let left = paragraph(&mut document, item, "one");
    let right = document.append_child(item, EventKind::Rule).unwrap();
    assert!(matches!(
        state.default_separator(
            &document,
            None,
            left,
            right,
            item,
            state.records_spread(&document, item)
        ),
        "\n"
    ));
    document
        .set_syntax(item, Some(NodeSyntax::ListItem { spread: true }))
        .unwrap();
    assert!(matches!(
        state.default_separator(
            &document,
            None,
            left,
            right,
            item,
            state.records_spread(&document, item)
        ),
        "\n\n"
    ));
    let right = paragraph(&mut document, item, "two");
    assert!(matches!(
        state.default_separator(
            &document,
            None,
            left,
            right,
            item,
            state.records_spread(&document, item)
        ),
        "\n\n"
    ));
}

#[test]
fn inline_code_is_padded_where_the_parser_strips_spaces() {
    let options = Options::default();
    let mut state = State::new(&options);
    state.inline_code(" one ", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`  one  `");
    state.inline_code(" ", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "` `");
    // A tab is not a space, so content of spaces and a tab is padded.
    state.inline_code(" \t ", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`  \t  `");
    state.inline_code("  ", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`  `");
    state.inline_code("é ` α", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "``é ` α``");
    state.inline_code(" `α` ", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "``  `α`  ``");
}

#[test]
fn serializes_table_cells_with_escaped_pipes() {
    let mut document = empty();
    let mut row = document.root();
    for tag in [Tag::Table, Tag::TableHead, Tag::TableRow] {
        row = document.append_child(row, EventKind::Start(tag)).unwrap();
    }
    for kind in [
        EventKind::Text(Text::Literal("plain α")),
        EventKind::Code {
            value: Text::Literal("a`b"),
            language: None,
        },
        EventKind::Text(Text::Literal("a|b")),
        EventKind::Code {
            value: Text::Literal("a|b"),
            language: None,
        },
        EventKind::Text(Text::Literal("")),
    ] {
        let cell = document
            .append_child(
                row,
                EventKind::Start(Tag::TableCell {
                    alignment: crate::AlignKind::None,
                    header: false,
                }),
            )
            .unwrap();
        document.append_child(cell, kind).unwrap();
    }
    let options = Options::default();
    let mut state = State::new(&options);
    state.render_table_row(&document, row).unwrap();
    assert_eq!(state.output, "| plain α | ``a`b`` | a\\|b | `a\\|b` |  |");
}

#[test]
fn html_block_shapes_record_the_columns_their_first_line_starts_at() {
    use crate::block_parser::HtmlBlockShape;
    let options = ParseOptions::default();
    let mut unindented = ParseOptions::default();
    unindented.constructs.code_indented = false;
    // Spaces alone fit every column up to three, and a tab fits only where it is at most three
    // columns wide, to the next multiple of four (#263, #266).
    for (value, residues) in [
        ("<div>", 0b1111),
        ("   <div>", 0b1111),
        ("    <div>", 0),
        ("\t<div>", 0b1110),
        (" \t<div>", 0b0110),
        ("  \t<div>", 0b0010),
        ("   \t<div>", 0),
        ("\t <div>", 0b1100),
    ] {
        let shape = HtmlBlockShape::of(value, &options).unwrap();
        assert_eq!(shape.residues, residues, "{:?}", value);
        assert_eq!(
            (0..8).filter(|&column| shape.fits(column)).count(),
            2 * residues.count_ones() as usize,
            "{:?}",
            value
        );
        assert_eq!(
            HtmlBlockShape::of(value, &unindented).unwrap().residues,
            0b1111
        );
    }
    assert!(HtmlBlockShape::of("  text", &options).is_none());
}
