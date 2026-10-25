// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::Options;
use pretty_assertions::assert_eq;
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::fixture::{Definition, Node};

#[test]
fn defintion() {
    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::new(),
            position: None,
            label: None
        }))
        .unwrap(),
        "[]: <>\n",
        "should support a definition w/o label"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::new(),
            position: None,
            label: Some(String::from("a"))
        }))
        .unwrap(),
        "[a]: <>\n",
        "should support a definition w/ label"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::new(),
            position: None,
            label: Some(String::from("\\"))
        }))
        .unwrap(),
        "[\\\\]: <>\n",
        "should escape a backslash in `label`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::new(),
            position: None,
            label: Some(String::from("["))
        }))
        .unwrap(),
        "[\\[]: <>\n",
        "should escape an opening bracket in `label`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::new(),
            position: None,
            label: Some(String::from("]"))
        }))
        .unwrap(),
        "[\\]]: <>\n",
        "should escape a closing bracket in `label`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <>\n",
        "should support a definition w/ identifier"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::from(r"\\"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[\\\\]: <>\n",
        "should escape a backslash in `identifier`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::from("["),
            position: None,
            label: None
        }))
        .unwrap(),
        "[\\[]: <>\n",
        "should escape an opening bracket in `identifier`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: None,
            identifier: String::from("]"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[\\]]: <>\n",
        "should escape a closing bracket in `identifier`"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: b\n",
        "should support a definition w/ url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <b c>\n",
        "should support a definition w/ enclosed url w/ whitespace in url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b <c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <b \\<c>\n",
        "should escape an opening angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b >c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <b \\>c>\n",
        "should escape a closing angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b \\.c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <b \\\\.c>\n",
        "should escape a backslash in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b\nc"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <b&#xA;c>\n",
        "should encode a line ending in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("\x0C"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <\x0C>\n",
        "should encode a line ending in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b(c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: b\\(c\n",
        "should escape an opening paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b)c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: b\\)c\n",
        "should escape a closing paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b\\?c"),
            title: None,
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: b\\\\?c\n",
        "should escape a backslash in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: String::from("b").into(),
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <> \"b\"\n",
        "should support a definition w/ title"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::from("b"),
            title: String::from("c").into(),
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: b \"c\"\n",
        "should support a definition w/ url & title"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: String::from("\"").into(),
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <> \"\\\"\"\n",
        "should escape a quote in `title` in a title"
    );

    assert_eq!(
        to(&Node::Definition(Definition {
            url: String::new(),
            title: String::from("\\").into(),
            identifier: String::from("a"),
            position: None,
            label: None
        }))
        .unwrap(),
        "[a]: <> \"\\\\\"\n",
        "should escape a backslash in `title` in a title"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Definition(Definition {
                url: String::new(),
                title: String::from("b").into(),
                identifier: String::from("a"),
                position: None,
                label: None
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[a]: <> 'b'\n",
        "should support a definition w/ title when `quote: \"\'\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Definition(Definition {
                url: String::new(),
                title: String::from("'").into(),
                identifier: String::from("a"),
                position: None,
                label: None
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[a]: <> '\\''\n",
        "should escape a quote in `title` in a title when `quote: \"\'\"`"
    );
}
