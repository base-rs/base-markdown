// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use core_markdown::markdown::{LineWrapping, Options};
use core_markdown::Document;
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::to_markdown_with_options as to_with_options;
use support::fixture::{Node, Text};

#[test]
fn text() {
    assert_eq!(
        to(&Node::Text(Text {
            value: String::new(),
            position: None,
        }))
        .unwrap(),
        "",
        "should support an empty text"
    );

    assert_eq!(
        to(&Node::Text(Text {
            value: String::from("a\nb"),
            position: None,
        }))
        .unwrap(),
        "a\nb\n",
        "should support text"
    );
}

#[test]
fn unwraps_soft_line_breaks_and_adjacent_indentation() {
    let node = Node::Text(Text {
        value: String::from("first  \n \tsecond\r\n third\rfourth"),
        position: None,
    });

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(output, "first second third fourth\n");
}

#[test]
fn unwrapping_preserves_hard_breaks_and_code_blocks() {
    let source = "first\nsecond  \nthird\n\n```text\ncode\nline\n```\n";
    let node = Document::parse(source, &Default::default()).unwrap();

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(
        output,
        "first second\\\nthird\n\n```text\ncode\nline\n```\n"
    );
}

#[test]
fn unwraps_soft_line_breaks_inside_emphasis() {
    let node = Document::parse("*first\nsecond*", &Default::default()).unwrap();

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(output, "*first second*\n");
}
