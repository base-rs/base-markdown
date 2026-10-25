// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{to_html, to_html_with_options, Constructs, Options, ParseOptions};
use pretty_assertions::assert_eq;

#[test]
fn text() {
    assert_eq!(
        to_html("hello $.;'there"),
        "<p>hello $.;'there</p>",
        "should support ascii text"
    );

    assert_eq!(
        to_html("Foo χρῆν"),
        "<p>Foo χρῆν</p>",
        "should support unicode text"
    );

    assert_eq!(
        to_html("Multiple     spaces"),
        "<p>Multiple     spaces</p>",
        "should preserve internal spaces verbatim"
    );
}

#[test]
fn ordinary_marker_at_chunk_start() {
    assert_eq!(to_html("]unmatched"), "<p>]unmatched</p>");
    assert_eq!(to_html("{plain}"), "<p>{plain}</p>");
    assert_eq!(
        to_html("first\n\n]unmatched\n\n{plain}"),
        "<p>first</p>\n<p>]unmatched</p>\n<p>{plain}</p>"
    );
    let options = Options {
        parse: ParseOptions {
            constructs: Constructs {
                label_end: false,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        to_html_with_options("]unmatched [link](/url)", &options),
        "<p>]unmatched [link](/url)</p>"
    );
}
