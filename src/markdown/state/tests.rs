// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{escape_table_pipes, has_matching_ticks, Info, Join, State};
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
        state.join_defaults(&document, code, code, root),
        Join::HtmlComment
    ));
    let item = document
        .append_child(root, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    let left = paragraph(&mut document, item, "one");
    let right = document
        .append_child(item, EventKind::Text(Text::Literal("other")))
        .unwrap();
    assert!(matches!(
        state.join_defaults(&document, left, right, item),
        Join::Lines(0)
    ));
    document
        .set_syntax(item, Some(NodeSyntax::ListItem { spread: true }))
        .unwrap();
    assert!(matches!(
        state.join_defaults(&document, left, right, item),
        Join::Lines(1)
    ));
    let right = paragraph(&mut document, item, "two");
    assert!(matches!(
        state.join_defaults(&document, left, right, item),
        Join::Break
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
}

#[test]
fn detects_matching_and_nonmatching_code_fences() {
    assert!(has_matching_ticks(b"`code`", 1, 1));
    assert!(!has_matching_ticks(b"plain", 0, 1));
    assert!(!has_matching_ticks(b"``", 0, 1));
}

#[test]
fn joins_flow_nodes_with_each_supported_separator() {
    let cases = [
        (Join::Break, "\n\n"),
        (Join::Lines(0), "\n"),
        (Join::Lines(1), "\n\n"),
        (Join::Lines(2), "\n\n\n"),
        (Join::HtmlComment, "\n\n<!---->\n\n"),
    ];

    for (join, expected) in cases {
        let mut output = String::new();
        State::set_between(&join, &mut output);
        assert_eq!(output, expected);
    }
}

#[test]
fn escapes_pipes_outside_unmatched_code_spans_and_after_even_backslashes() {
    assert_eq!(escape_table_pipes(r"``code`| value"), r"``code`\| value");
    assert_eq!(escape_table_pipes(r"two \\|"), r"two \\\|");
    assert_eq!(escape_table_pipes(r"one \|"), r"one \|");
}
