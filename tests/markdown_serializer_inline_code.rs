// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{InlineCode, Node};

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
        "`\n`\n",
        "should support an eol"
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
