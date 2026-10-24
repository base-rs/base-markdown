// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use core_markdown::{message, to_html};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Node, Root};

#[test]
fn zero() -> Result<(), message::Message> {
    assert_eq!(to_html(""), "", "should support no markdown");

    assert_eq!(
        to_html("asd\0asd"),
        "<p>asd�asd</p>",
        "should replace `\\0` w/ a replacement characters (`�`)"
    );

    assert_eq!(
        to_html("&#0;"),
        "<p>�</p>",
        "should replace NUL in a character reference"
    );

    // This doesn’t make sense in markdown, as character escapes only work on
    // ascii punctuation, but it’s good to demonstrate the behavior.
    assert_eq!(
        to_html("\\0"),
        "<p>\\0</p>",
        "should not support NUL in a character escape"
    );

    assert_eq!(
        parse_fixture("", &Default::default())?,
        Node::Root(Root {
            children: vec![],
            position: Some(Position::new(1, 1, 0, 1, 1, 0))
        }),
        "should support no markdown (ast)"
    );

    Ok(())
}
