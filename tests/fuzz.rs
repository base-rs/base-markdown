// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2023, by Mick van Gelderen.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{to_html, to_html_with_options, Document, Options};
use pretty_assertions::assert_eq;

#[test]
fn fuzz() {
    assert_eq!(
        to_html("[\n~\na\n-\n\n"),
        "<h2>[\n~\na</h2>\n",
        "1: label, blank lines, and code"
    );

    // The first link is stopped by the `+` (so it’s `a@b.c`), but the next
    // link overlaps it (`b.c+d@e.f`).
    assert_eq!(
        to_html_with_options("a@b.c+d@e.f", &Options::gfm()),
        "<p><a href=\"mailto:a@b.c\">a@b.c</a><a href=\"mailto:+d@e.f\">+d@e.f</a></p>",
        "2: gfm: email autolink literals running into each other"
    );

    assert_eq!(
        to_html("    x\n*    "),
        "<pre><code>x\n</code></pre>\n<ul>\n<li></li>\n</ul>",
        "3-a: containers should not pierce into indented code"
    );

    assert_eq!(
        to_html("    a\n*     b"),
        "<pre><code>a\n</code></pre>\n<ul>\n<li>\n<pre><code>b\n</code></pre>\n</li>\n</ul>",
        "3-b: containers should not pierce into indented code"
    );

    assert_eq!(
        to_html("a * "),
        "<p>a *</p>",
        "4-a: trailing whitespace and broken data"
    );

    assert_eq!(
        to_html("_  "),
        "<p>_</p>",
        "4-b: trailing whitespace and broken data (GH-13)"
    );

    assert_eq!(
        to_html_with_options("a ~ ", &Options::gfm()),
        "<p>a ~</p>",
        "4-c: trailing whitespace and broken data (GH-14)"
    );

    assert!(
        Document::parse("123456789. ok", &Default::default()).is_ok(),
        "5: lists should support high start numbers (GH-17)"
    );

    assert_eq!(
        to_html("> ```\n"),
        "<blockquote>\n<pre><code></code></pre>\n</blockquote>\n",
        "6-a: container close after unclosed fenced code, with eol (block quote, GH-16)"
    );

    assert_eq!(
        to_html("- ```\n"),
        "<ul>\n<li>\n<pre><code></code></pre>\n</li>\n</ul>\n",
        "6-b: container close after unclosed fenced code, with eol (list, GH-16)"
    );

    // An incomplete fence does not interrupt a lazy paragraph continuation.
    // GFM now shares the compact CommonMark behavior here (also matching Comrak).
    assert_eq!(
        to_html_with_options("> x\n``", &Options::gfm()),
        "<blockquote>\n<p>x\n``</p>\n</blockquote>",
        "7: lazy container lines almost starting fenced code (GH-19)"
    );

    assert_eq!(
        to_html_with_options("a\tb@c.d", &Options::gfm()),
        "<p>a\t<a href=\"mailto:b@c.d\">b@c.d</a></p>",
        "8-a: autolink literals after tabs (GH-18)"
    );

    assert_eq!(
        to_html_with_options("aa\tb@c.d", &Options::gfm()),
        "<p>aa\t<a href=\"mailto:b@c.d\">b@c.d</a></p>",
        "8-b: autolink literals after tabs (GH-18)"
    );

    assert_eq!(
        to_html_with_options("aaa\tb@c.d", &Options::gfm()),
        "<p>aaa\t<a href=\"mailto:b@c.d\">b@c.d</a></p>",
        "8-c: autolink literals after tabs (GH-18)"
    );

    assert_eq!(
        to_html_with_options("aaaa\tb@c.d", &Options::gfm()),
        "<p>aaaa\t<a href=\"mailto:b@c.d\">b@c.d</a></p>",
        "8-d: autolink literals after tabs (GH-18)"
    );

    assert_eq!(
        to_html_with_options("| a |\n| - |\n| www.a|", &Options::gfm()),
        "<table>\n<thead>\n<tr>\n<th>a</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td><a href=\"http://www.a\">www.a</a></td>\n</tr>\n</tbody>\n</table>",
        "9: autolink literals that end in table cell delimiter (GH-20)"
    );

    assert_eq!(
        to_html_with_options("[*]() [*]()", &Options::gfm()),
        "<p><a href=\"\">*</a> <a href=\"\">*</a></p>",
        "10: attention in different links (GH-21)"
    );

    assert!(
        Document::parse("* [ ]\na", &Default::default()).is_ok(),
        "11: gfm task list items followed by eols (GH-24)"
    );

    assert_eq!(
        core_markdown::to_html_with_options(
            "<",
            &core_markdown::Options {
                parse: core_markdown::ParseOptions::default(),
                ..Default::default()
            }
        ),
        "<p>&lt;</p>".to_string(),
        "12: incomplete HTML without panic (GH-26)"
    );
}
