// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{normalize_markdown, to_html, to_html_with_options, Options};

#[test]
fn presets_enable_their_extensions() {
    let cases = [
        // (source, default, gfm, commonmark)
        (
            "~~a~~ ~b~",
            "<p><del>a</del> ~b~</p>",
            "<p><del>a</del> <del>b</del></p>",
            "<p>~~a~~ ~b~</p>",
        ),
        (
            "| a |\n| - |",
            "<table>\n<thead>\n<tr>\n<th>a</th>\n</tr>\n</thead>\n</table>",
            "<table>\n<thead>\n<tr>\n<th>a</th>\n</tr>\n</thead>\n</table>",
            "<p>| a |\n| - |</p>",
        ),
        (
            "www.a.com",
            "<p><a href=\"https://www.a.com\">www.a.com</a></p>",
            "<p><a href=\"https://www.a.com\">www.a.com</a></p>",
            "<p>www.a.com</p>",
        ),
        (
            "---\na: b\n---\nc",
            "<p>c</p>",
            "<hr />\n<h2>a: b</h2>\n<p>c</p>",
            "<hr />\n<h2>a: b</h2>\n<p>c</p>",
        ),
        ("$a$", "<p>$a$</p>", "<p>$a$</p>", "<p>$a$</p>"),
    ];

    for (source, default, gfm, commonmark) in cases {
        assert_eq!(to_html(source), default, "default: {source:?}");
        assert_eq!(
            to_html_with_options(source, &Options::gfm()),
            gfm,
            "gfm: {source:?}"
        );
        assert_eq!(
            to_html_with_options(source, &Options::commonmark()),
            commonmark,
            "commonmark: {source:?}"
        );
    }
}

#[test]
fn normalize_markdown_reads_the_default_options() {
    assert_eq!(
        normalize_markdown("costs $5 and ~5 min, see www.a.com").unwrap(),
        "costs $5 and ~5 min, see www.a.com\n"
    );
}
