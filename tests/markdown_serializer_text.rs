// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{LineWrapping, Options};
use base_markdown::Document;
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::to_markdown_with_options as to_with_options;
use support::fixture::{Emphasis, Node, Paragraph, Text};

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

#[test]
fn intraword_underscores() {
    for (source, expected) in [
        ("io_uring is fast\n", "io_uring is fast\n"),
        ("a snake_case_name\n", "a snake_case_name\n"),
        ("x_1 and a__b\n", "x_1 and a__b\n"),
        ("é_ü\n", "é_ü\n"),
        ("*io_uring* [io_uring](x)\n", "*io_uring* [io_uring](x)\n"),
        ("a\\_\n", "a\\_\n"),
        ("\\_a\n", "\\_a\n"),
        ("a \\_ b\n", "a \\_ b\n"),
        ("**a**\\_b\n", "**a**\\_b\n"),
        ("a\\_**b**\n", "a\\_**b**\n"),
        ("a\\*_b\n", "a\\*\\_b\n"),
        ("a_\\*b\n", "a\\_\\*b\n"),
        ("x\\@a_b.com\n", "x@a\\_b.com\n"),
        ("x&#64;a_b.com\n", "x@a\\_b.com\n"),
        ("x\\@y.co1\\_www.\n", "x@y.co1\\_www.\n"),
        ("x\\@y a_b.com\n", "x@y a_b.com\n"),
    ] {
        let document = Document::parse(source, &base_markdown::ParseOptions::gfm()).unwrap();
        let markdown = base_markdown::to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let html = |value: &str| {
            base_markdown::to_html_with_options(value, &base_markdown::Options::gfm())
        };
        assert_eq!(
            html(&markdown),
            html(source),
            "{source:?} should round-trip"
        );
    }

    for (value, expected, message) in [
        ("a_b", "a_b\n", "should not escape `_` between letters"),
        ("1_000", "1_000\n", "should not escape `_` between digits"),
        (
            "é_é",
            "é_é\n",
            "should not escape `_` between non-ASCII letters",
        ),
        (
            "a._.b",
            "a.\\_.b\n",
            "should escape `_` between punctuation",
        ),
        ("a_ b", "a\\_ b\n", "should escape `_` before whitespace"),
        ("a _b", "a \\_b\n", "should escape `_` after whitespace"),
        ("a__b", "a__b\n", "should not escape `__` between letters"),
        (
            "x@a_b.com",
            "x@a\\_b.com\n",
            "should escape `_` in email domains",
        ),
        (
            "a__.b",
            "a\\_\\_.b\n",
            "should escape `__` before punctuation",
        ),
    ] {
        assert_eq!(
            to(&Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from(value),
                    position: None,
                })],
                position: None,
            }))
            .unwrap(),
            expected,
            "{message}"
        );
    }

    let emphasis = || {
        Node::Emphasis(Emphasis {
            children: vec![Node::Text(Text {
                value: String::from("x"),
                position: None,
            })],
            position: None,
        })
    };
    let text = |value: &str| {
        Node::Text(Text {
            value: String::from(value),
            position: None,
        })
    };
    let options = Options {
        emphasis: '_',
        ..Default::default()
    };

    assert_eq!(
        to_with_options(
            &Node::Paragraph(Paragraph {
                children: vec![emphasis(), text("__a")],
                position: None,
            }),
            &options
        )
        .unwrap(),
        "_x_\\_\\_a\n",
        "should escape `__` after emphasis"
    );

    assert_eq!(
        to_with_options(
            &Node::Paragraph(Paragraph {
                children: vec![text("a__"), emphasis()],
                position: None,
            }),
            &options
        )
        .unwrap(),
        "a\\_\\__x_\n",
        "should escape `__` before emphasis"
    );
}
