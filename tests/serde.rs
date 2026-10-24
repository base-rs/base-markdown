// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#![cfg(feature = "serde")]

use core_markdown::{
    stream::{Event, EventKind, Tag, Text},
    CodeFence, Document, NodeSyntax, Options, ReferenceKind,
};
use serde_json::{json, Value};

fn html(document: &Document<String>) -> String {
    let mut output = String::new();
    document
        .render_html(&Options::gfm().compile, &mut output)
        .unwrap();
    output
}

#[test]
fn document_serialization_preserves_source_edits_metadata_and_detached_subtrees() {
    let source=String::from("# Title\n\nA [link][name] and **bold**.\n\n[name]: /url \"title\"\n\n```rust extra\nx\n```\n");
    let mut document = Document::parse(source.clone(), &Options::gfm().parse).unwrap();
    let paragraph = document
        .children(document.root())
        .find(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
        .unwrap();
    let text = document.children(paragraph).next().unwrap();
    document.text_mut(text).unwrap().push_str("edited ");
    let detached = document
        .append_child(
            document.root(),
            EventKind::Start(Tag::Paragraph { tight: false }),
        )
        .unwrap();
    document
        .append_child(detached, EventKind::Text(Text::from("detached".to_owned())))
        .unwrap();
    document.detach(detached).unwrap();
    let link = document
        .descendants(paragraph)
        .find(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::Start(Tag::Link { .. })
            )
        })
        .unwrap();
    document
        .set_syntax(
            link,
            Some(NodeSyntax::Reference {
                kind: ReferenceKind::Full,
                label: Text::Literal("name"),
            }),
        )
        .unwrap();
    let code = document
        .children(document.root())
        .find(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::CodeBlock { .. }
            )
        })
        .unwrap();
    document.set_syntax(code, None).unwrap();
    let stored = serde_json::to_string(&document).unwrap();
    let restored: Document = serde_json::from_str(&stored).unwrap();
    assert_eq!(restored.source(), source);
    assert_eq!(restored.nodes().len(), document.nodes().len());
    assert_eq!(html(&restored), html(&document));
    assert_eq!(restored.text_content(detached), "detached");
    assert_eq!(
        core_markdown::to_markdown(&restored).unwrap(),
        core_markdown::to_markdown(&document).unwrap()
    );
    assert_eq!(serde_json::to_string(&restored).unwrap(), stored);
    assert!(restored.syntax(code).is_none());
    let mut detached_html = String::new();
    restored
        .render_node(detached, &Options::default().compile, &mut detached_html)
        .unwrap();
    assert_eq!(detached_html, "<p>detached</p>");
    // An absent override map still permits derived source metadata.
    let mut wire: Value = serde_json::from_str(&stored).unwrap();
    wire.as_object_mut().unwrap().remove("syntax");
    serde_json::from_value::<Document>(wire).unwrap();
}

#[test]
fn events_text_and_syntax_have_owned_deserialization() {
    let kinds = vec![
        EventKind::Text(Text::Literal("literal")),
        EventKind::Text(Text::Source(0..1)),
        EventKind::Start(Tag::TableCell {
            alignment: core_markdown::AlignKind::Center,
            header: true,
        }),
        EventKind::CodeBlock {
            value: Text::from("x".to_owned()),
            language: None,
            fence: Some(CodeFence {
                character: '`',
                length: 3,
                indent: 2,
            }),
            math: false,
            unclosed: false,
        },
    ];
    for kind in kinds {
        let event = Event { kind, span: 0..1 };
        let encoded = serde_json::to_string(&event).unwrap();
        let decoded: Event<'static> = serde_json::from_str(&encoded).unwrap();
        // A static literal becomes an owned semantic value after deserialization.
        if let EventKind::Text(value) = decoded.kind {
            assert_eq!(
                value.resolve("x").unwrap(),
                if encoded.contains("literal") {
                    "literal"
                } else {
                    "x"
                }
            );
        } else {
            assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
        }
    }
}

#[test]
fn invalid_serialized_arena_links_ranges_and_metadata_are_rejected() {
    let document = Document::parse("# h\n\ntext".to_owned(), &Options::default().parse).unwrap();
    let base = serde_json::to_value(document).unwrap();
    let paragraph = base["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["kind"]["Start"].get("Paragraph").is_some())
        .unwrap();
    let heading = base["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["kind"]["Start"].get("Heading").is_some())
        .unwrap();
    let text = heading + 1;
    let mut cases = Vec::new();
    let mut alter = |index: usize, field: &str, value: Value| {
        let mut wire = base.clone();
        wire["nodes"][index][field] = value;
        cases.push(wire);
    };
    alter(0, "parent", json!(2));
    alter(0, "next_sibling", json!(2));
    alter(0, "kind", json!("Rule"));
    alter(0, "first_child", json!(999));
    alter(heading, "next_sibling", json!(heading + 1));
    alter(heading, "parent", Value::Null);
    alter(heading, "first_child", json!(heading + 1));
    alter(text, "first_child", json!(paragraph + 1));
    alter(text, "kind", json!({"End":"Paragraph"}));
    alter(paragraph, "kind", json!({"Start":{"Document":null}}));
    alter(paragraph, "kind", json!({"Start":{"Heading":0}}));
    alter(paragraph, "span", json!({"start":20,"end":21}));
    alter(paragraph, "span", json!({"start":5,"end":2}));
    alter(
        text,
        "kind",
        json!({"Text":{"Source":{"start":0,"end":999}}}),
    );
    alter(text, "kind", json!({"Start":{"Heading":2}}));
    alter(
        text,
        "kind",
        json!({"Start":{"FootnoteDefinition":{"Literal":"x"}}}),
    );
    let mut unseen = base.clone();
    unseen["nodes"][0]["first_child"] = Value::Null;
    cases.push(unseen);
    let mut empty = base.clone();
    empty["nodes"] = json!([]);
    cases.push(empty);
    let mut duplicate = base.clone();
    duplicate["nodes"][text]["next_sibling"] = json!(text + 1);
    cases.push(duplicate);
    let mut missing_syntax = base.clone();
    missing_syntax["syntax"] = json!({"999":null});
    cases.push(missing_syntax);
    let mut wrong_syntax = base.clone();
    wrong_syntax["syntax"] = json!({"1":{"Code":{"info":{"Value":"x"}}}});
    cases.push(wrong_syntax);
    let mut bad_zero = base.clone();
    bad_zero["nodes"][0]["first_child"] = json!(0);
    cases.push(bad_zero);
    for (index, wire) in cases.into_iter().enumerate() {
        assert!(
            serde_json::from_value::<Document>(wire.clone()).is_err(),
            "case {}: {}",
            index,
            wire
        );
    }
}

#[test]
fn alignment_and_configuration_serialization_remain_compatible() {
    use core_markdown::AlignKind;
    for (value, wire) in [
        (AlignKind::None, json!(null)),
        (AlignKind::Left, json!("left")),
        (AlignKind::Right, json!("right")),
        (AlignKind::Center, json!("center")),
    ] {
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        assert_eq!(serde_json::from_value::<AlignKind>(wire).unwrap(), value);
    }
    assert!(serde_json::from_str::<AlignKind>("\"invalid\"").is_err());
    let options = Options::gfm();
    let wire = serde_json::to_string(&options).unwrap();
    let restored: Options = serde_json::from_str(&wire).unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), wire);
}
