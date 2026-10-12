// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::{escape_table_pipes, has_matching_ticks, Info, Join, State};
use crate::{
    markdown::{r#unsafe::Unsafe, util::safe::SafeConfig, Options},
    mdast::{
        Code, FootnoteDefinition, Fragment, LinkReference, ListItem, MdxJsxFlowElement, Node,
        Paragraph, ReferenceKind, Text,
    },
};
use alloc::{string::String, vec};

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn paragraph(value: &str) -> Node {
    Node::Paragraph(Paragraph {
        children: vec![text(value)],
        position: None,
    })
}

#[test]
fn serializes_fragment_footnote_and_empty_jsx_paths() {
    let options = Options::default();
    let mut state = State::new(&options);
    let phrasing = Node::Fragment(Fragment {
        children: vec![text("inline")],
    });
    assert_eq!(
        state.handle(&phrasing, &Info::new("", ""), None).unwrap(),
        "inline"
    );

    let footnote = Node::FootnoteDefinition(FootnoteDefinition {
        children: vec![paragraph("first"), paragraph("second")],
        position: None,
        identifier: "note".into(),
        label: None,
    });
    let rendered = state.handle(&footnote, &Info::new("", ""), None).unwrap();
    assert!(rendered.contains("    second"), "{:?}", rendered);

    let empty_jsx = Node::MdxJsxFlowElement(MdxJsxFlowElement {
        children: vec![],
        position: None,
        name: Some("Empty".into()),
        attributes: vec![],
    });
    assert_eq!(
        state.handle(&empty_jsx, &Info::new("", ""), None).unwrap(),
        "<Empty />"
    );
}

#[test]
fn handles_adjacent_text_nodes_when_looking_ahead_in_phrasing() {
    let options = Options::default();
    let mut state = State::new(&options);
    let paragraph = Node::Paragraph(Paragraph {
        children: vec![text("first"), text("second")],
        position: None,
    });

    assert_eq!(
        state
            .container_phrasing(&paragraph, &Info::new("", ""))
            .unwrap(),
        "firstsecond"
    );
}

#[test]
fn propagates_errors_from_phrasing_lookahead() {
    let options = Options {
        bullet: 'x',
        ..Options::default()
    };
    let mut state = State::new(&options);
    let paragraph = Node::Paragraph(Paragraph {
        children: vec![
            text("first"),
            Node::ListItem(ListItem {
                children: vec![],
                position: None,
                spread: false,
                checked: None,
            }),
        ],
        position: None,
    });

    let error = state
        .container_phrasing(&paragraph, &Info::new("", ""))
        .expect_err("lookahead must report errors while serializing a child");

    assert_eq!(*error.rule_id, "unexpected-marker");
}

#[test]
fn joins_code_and_spread_list_children_consistently() {
    let options = Options {
        fences: false,
        ..Options::default()
    };
    let state = State::new(&options);
    let code = Node::Code(Code {
        fence: None,
        value: "indented".into(),
        position: None,
        lang: None,
        meta: None,
    });
    assert!(matches!(
        state.join_defaults(
            &code,
            &code,
            &Node::Root(crate::mdast::Root {
                children: vec![],
                position: None,
            })
        ),
        Join::HtmlComment
    ));

    let left = paragraph("one");
    let right = paragraph("two");
    let tight_item = Node::ListItem(ListItem {
        children: vec![],
        position: None,
        spread: false,
        checked: None,
    });
    let loose_item = Node::ListItem(ListItem {
        children: vec![],
        position: None,
        spread: true,
        checked: None,
    });
    assert!(matches!(
        state.join_defaults(&left, &text("other"), &tight_item),
        Join::Lines(0)
    ));
    assert!(matches!(
        state.join_defaults(&left, &text("other"), &loose_item),
        Join::Lines(1)
    ));
    assert!(matches!(
        state.join_defaults(&left, &right, &tight_item),
        Join::Break
    ));
}

#[test]
fn decodes_associations_and_escapes_at_unicode_byte_offsets() {
    let options = Options::default();
    let mut state = State::new(&options);
    let reference = LinkReference {
        children: vec![text("label")],
        position: None,
        reference_kind: ReferenceKind::Shortcut,
        identifier: "known&amp;".into(),
        label: None,
    };
    assert_eq!(state.association(&reference), "known&");
    let numeric_reference = LinkReference {
        identifier: "known&#35;".into(),
        ..reference.clone()
    };
    assert_eq!(state.association(&numeric_reference), "known#");

    state.r#unsafe = vec![Unsafe::new('#', None, None, vec![], vec![], true)];
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
