// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{escape_table_pipes, has_matching_ticks, Info, Join, State};
use crate::{
    markdown::{r#unsafe::Unsafe, util::safe::SafeConfig, Options},
    stream::{EventKind, Tag, Text},
    Document, NodeId, NodeSyntax, ParseOptions,
};
use alloc::{string::String, vec};
use regex::Regex;

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
    assert_eq!(
        state
            .container_phrasing(&document, node, &Info::new("", ""))
            .unwrap(),
        "firstsecond"
    );

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
    assert_eq!(*error.rule_id, "unexpected-marker");
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
fn inline_literal_normalization_preserves_crlf_and_nonmatches() {
    let options = Options::default();
    let mut state = State::new(&options);
    let mut ending = Unsafe::new('\n', None, None, vec![], vec![], true);
    ending.set_compiled(Regex::new("\\n").unwrap());
    state.r#unsafe = vec![ending];
    assert_eq!(state.inline_code("one\r\ntwo", None).unwrap(), "`one two`");
    assert_eq!(state.inline_math("one\r\ntwo").unwrap(), "$one two$");
    state.r#unsafe = vec![Unsafe::new('#', None, None, vec![], vec![], true)];
    assert_eq!(
        state.inline_code("ordinary code", None).unwrap(),
        "`ordinary code`"
    );
    assert_eq!(
        state.inline_math("ordinary math").unwrap(),
        "$ordinary math$"
    );
    assert_eq!(
        state.safe("é\n# heading", &SafeConfig::new("", "", None)),
        "é\n\\# heading"
    );
}

#[test]
fn merges_duplicate_unsafe_positions_and_encodes_other_markers() {
    let options = Options::default();
    let mut state = State::new(&options);
    state.r#unsafe = vec![
        Unsafe::new('*', Some(""), Some(""), vec![], vec![], false),
        Unsafe::new('*', None, None, vec![], vec![], false),
    ];
    assert_eq!(state.safe("*", &SafeConfig::new("", "", None)), "\\*");

    state.r#unsafe = vec![Unsafe::new('*', None, None, vec![], vec![], false)];
    assert_eq!(state.safe("*", &SafeConfig::new("", "", Some('!'))), "\\*");
}

#[test]
fn leaves_text_unchanged_when_no_unsafe_patterns_are_configured() {
    let options = Options::default();
    let mut state = State::new(&options);
    state.r#unsafe.clear();

    assert_eq!(
        state.safe("ordinary text", &SafeConfig::new("", "", None)),
        "ordinary text"
    );
}

#[test]
fn escapes_punctuation_unsafe_at_the_end_of_the_ascii_range() {
    let options = Options::default();
    let mut state = State::new(&options);
    state.r#unsafe = vec![Unsafe::new('}', None, None, vec![], vec![], false)];

    assert_eq!(state.safe("}", &SafeConfig::new("", "", None)), "\\}");
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
