// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{to_html, to_html_with_options, CompileOptions, LineEnding, Options};
use pretty_assertions::assert_eq;

#[test]
fn default_line_ending() {
    assert_eq!(
        to_html("> a"),
        "<blockquote>\n<p>a</p>\n</blockquote>",
        "should use `\\n` default"
    );

    assert_eq!(
        to_html("> a\n"),
        "<blockquote>\n<p>a</p>\n</blockquote>\n",
        "should use `\\n` for a line feed"
    );

    assert_eq!(
        to_html("> a\r"),
        "<blockquote>\n<p>a</p>\n</blockquote>\n",
        "should use `\\n` for a carriage return"
    );

    assert_eq!(
        to_html("> a\r\n"),
        "<blockquote>\n<p>a</p>\n</blockquote>\n",
        "should use `\\n` for a carriage return and line feed"
    );

    assert_eq!(
        to_html_with_options(
            "> a",
            &Options {
                compile: CompileOptions {
                    default_line_ending: LineEnding::CarriageReturn,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        "<blockquote>\r<p>a</p>\r</blockquote>",
        "should support the given line ending"
    );

    assert_eq!(
        to_html_with_options(
            "> a\n",
            &Options {
                compile: CompileOptions {
                    default_line_ending: LineEnding::CarriageReturn,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        // To do: is this a bug in `to_html.js` that it uses `\r` for earlier line endings?
        // "<blockquote>\r<p>a</p>\r</blockquote>\n",
        "<blockquote>\n<p>a</p>\n</blockquote>\n",
        "should support the given line ending, even if line endings exist"
    );
}
