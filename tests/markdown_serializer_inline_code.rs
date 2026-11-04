// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{to_markdown, Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{canonical, InlineCode, Node, Paragraph, Text};

#[test]
fn text() {
    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::new(),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "``\n",
        "should support an empty code text"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a`\n",
        "should support a code text"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" "),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "` `\n",
        "should support a space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("\n"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "` `\n",
        "should write an eol as the space it reads back as"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("  "),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`  `\n",
        "should support several spaces"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a`b"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "``a`b``\n",
        "should use a fence of two grave accents if the value contains one"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a``b"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a``b`\n",
        "should use a fence of one grave accent if the value contains two"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a``b`c"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "```a``b`c```\n",
        "should use a fence of three grave accents if the value contains two and one"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("`a"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`` `a ``\n",
        "should pad w/ a space if the value starts w/ a grave accent"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a`"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`` a` ``\n",
        "should pad w/ a space if the value ends w/ a grave accent"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" a "),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`  a  `\n",
        "should pad w/ a space if the value starts and ends w/ a space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from(" a"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "` a`\n",
        "should not pad w/ spaces if the value ends w/ a non-space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a "),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a `\n",
        "should not pad w/ spaces if the value starts w/ a non-space"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n- b"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (-)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n#"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a #`\n",
        "should prevent breaking out of code (#)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\n1. "),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a 1. `\n",
        "should prevent breaking out of code (\\d\\.)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\r- b"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (cr)"
    );

    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: String::from("a\r\n- b"),
            position: None,
            lang: None,
        }))
        .unwrap(),
        "`a - b`\n",
        "should prevent breaking out of code (crlf)"
    );

    let prefix = String::from("a").repeat(50);
    assert_eq!(
        to(&Node::InlineCode(InlineCode {
            value: format!("{}\r\n- b", prefix),
            position: None,
            lang: None,
        }))
        .unwrap(),
        format!("`{} - b`\n", prefix),
        "should prevent breaking out of code (long crlf)"
    );
}

/// The parser reads a language as the longest run of language characters before its `:`, so the
/// character of text directly before one is encoded when it would join the run, and an escaped one
/// is encoded instead of its escape, which would start the run with punctuation (#138).
#[test]
fn language_after_text_reads_back_alone() {
    let options = ParseOptions {
        inline_code_info: true,
        ..ParseOptions::default()
    };
    for (before, expected) in [
        ("x ", "x rust:`v`\n"),
        ("x*", "x\\*rust:`v`\n"),
        ("xa", "x&#x61;rust:`v`\n"),
        ("x-", "x&#x2D;rust:`v`\n"),
        ("x.", "x&#x2E;rust:`v`\n"),
        ("x_", "x&#x5F;rust:`v`\n"),
        ("a__b", "a\\_\\_&#x62;rust:`v`\n"),
        ("1.", "1&#x2E;rust:`v`\n"),
        ("x\\a", "x\\\\&#x61;rust:`v`\n"),
        ("x\\", "x\\\\rust:`v`\n"),
        ("~~a", "\\~\\~&#x61;rust:`v`\n"),
        ("x+", "x&#x2B;rust:`v`\n"),
        ("x#", "x&#x23;rust:`v`\n"),
        ("+", "&#x2B;rust:`v`\n"),
        ("#", "&#x23;rust:`v`\n"),
    ] {
        let node = Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: before.into(),
                    position: None,
                }),
                Node::InlineCode(InlineCode {
                    value: "v".into(),
                    position: None,
                    lang: Some("rust".into()),
                }),
            ],
            position: None,
        });
        let markdown = to(&node).unwrap();
        assert_eq!(markdown, expected, "{before:?}");
        let mut document = Document::parse(markdown.as_str(), &options).unwrap();
        document.normalize();
        assert_eq!(
            canonical(&document),
            canonical(&node.document()),
            "{before:?}"
        );
        assert_eq!(to_markdown(&document).unwrap(), markdown, "{before:?}");
    }
}
