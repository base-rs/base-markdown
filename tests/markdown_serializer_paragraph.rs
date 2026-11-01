// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{Node, Paragraph, Text};

#[test]
fn paragraph() {
    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![],
            position: None
        }))
        .unwrap(),
        "",
        "should support an empty paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\nb"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\nb\n",
        "should support a paragraph"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("  a"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\n",
        "should trim spaces at the start of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a  "),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\n",
        "should trim spaces at the end of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("\t\ta"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\n",
        "should trim tabs at the start of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\t\t"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\n",
        "should trim tabs at the end of paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a  \n  b"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a &#x20;\n&#x20; b\n",
        "should encode spaces around line endings in paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("a\t\t\n\t\tb"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "a\t&#x9;\n&#x9;\tb\n",
        "should encode spaces around line endings in paragraphs"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: String::from("я_я"),
                position: None
            })],
            position: None
        }))
        .unwrap(),
        "я_я\n",
        "should not escape `_` between non-ASCII letters"
    );
}
