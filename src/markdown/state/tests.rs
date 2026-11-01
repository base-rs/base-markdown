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
            EventKind::Start(Tag::FootnoteDefinition(Text::Literal("note"))),
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

    document
        .append_child(node, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    let options = Options {
        bullet: 'x',
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
        ConstructName::CodeFencedLangGraveAccent,
    ];
    for line_wrapping in [LineWrapping::Preserve, LineWrapping::Unwrap] {
        let options = Options {
            line_wrapping,
            ..Options::default()
        };
        for scope in scopes {
            for first in (0..=127).map(char::from).chain(['é', 'α', '\u{a0}', '世']) {
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
                language: None,
                fence: None,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    assert!(matches!(
        state.default_separator(&document, code, code, root),
        "\n\n<!---->\n\n"
    ));
    let item = document
        .append_child(root, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    let left = paragraph(&mut document, item, "one");
    let right = document
        .append_child(item, EventKind::Text(Text::Literal("other")))
        .unwrap();
    assert!(matches!(
        state.default_separator(&document, left, right, item),
        "\n"
    ));
    document
        .set_syntax(item, Some(NodeSyntax::ListItem { spread: true }))
        .unwrap();
    assert!(matches!(
        state.default_separator(&document, left, right, item),
        "\n\n"
    ));
    let right = paragraph(&mut document, item, "two");
    assert!(matches!(
        state.default_separator(&document, left, right, item),
        "\n\n"
    ));
}

#[test]
fn inline_literals_normalize_block_openers_and_preserve_other_line_endings() {
    let options = Options::default();
    let mut state = State::new(&options);
    state.inline_code("one\r\n# two", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`one # two`");
    state.inline_math("one\r\n> two").unwrap();
    assert_eq!(core::mem::take(&mut state.output), "$one > two$");
    state.inline_code("one\r\ntwo", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`one\r\ntwo`");
    state.inline_code("é ` α", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "``é ` α``");
    state.inline_math(" $α$ ").unwrap();
    assert_eq!(core::mem::take(&mut state.output), "$$  $α$  $$");
    state.inline_code("one\n12. two", None).unwrap();
    assert_eq!(core::mem::take(&mut state.output), "`one 12. two`");
}

#[test]
fn serializes_table_cells_with_escaped_pipes() {
    let mut document = empty();
    let row = document
        .append_child(document.root(), EventKind::Start(Tag::TableRow))
        .unwrap();
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
