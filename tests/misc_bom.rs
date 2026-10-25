// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::to_html;
use pretty_assertions::assert_eq;

#[test]
fn bom() {
    assert_eq!(to_html("\u{FEFF}"), "", "should ignore just a bom");

    assert_eq!(
        to_html("\u{FEFF}# hea\u{FEFF}ding"),
        "<h1>hea\u{FEFF}ding</h1>",
        "should ignore a bom"
    );
}

#[test]
fn options_preserve_bom_like_unicode() {
    let mut options = base_markdown::Options::default();
    options.parse.constructs.math_flow = true;
    for source in ["\u{fffd}", "\u{fec0}"] {
        assert_eq!(
            base_markdown::to_html_with_options(source, &options),
            format!("<p>{source}</p>")
        );
    }
}
