// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::to_html;
use pretty_assertions::assert_eq;

#[test]
fn soft_break() {
    assert_eq!(
        to_html("foo\nbaz"),
        "<p>foo\nbaz</p>",
        "should support line endings"
    );

    assert_eq!(
        to_html("foo \n baz"),
        "<p>foo\nbaz</p>",
        "should trim spaces around line endings"
    );
}
