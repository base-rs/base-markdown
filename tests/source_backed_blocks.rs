// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    render_events, to_html_with_options, Constructs, Document, EventParser, Options,
};

fn assert_html(source: &str, options: &Options, expected: &str) {
    assert_eq!(
        to_html_with_options(source, options),
        expected,
        "{source:?}"
    );
    let mut output = String::new();
    render_events(
        source,
        EventParser::new(source, &options.parse),
        &options.compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, expected, "checked events: {source:?}");
    let document = Document::parse(source, &options.parse).unwrap();
    output.clear();
    document.render_html(&options.compile, &mut output).unwrap();
    assert_eq!(output, expected, "document: {source:?}");
}

#[test]
fn disabled_block_constructs_do_not_disable_other_classifiers() {
    type Configure = fn(&mut Constructs);
    let cases: &[(Configure, &str, &str)] = &[
        (
            |c| c.block_quote = false,
            "> a\n# b",
            "<p>&gt; a</p>\n<h1>b</h1>",
        ),
        (|c| c.list_item = false, "- a\n\n***", "<p>- a</p>\n<hr />"),
        (
            |c| c.code_fenced = false,
            "~~~\na\n~~~",
            "<p>~~~\na\n~~~</p>",
        ),
        (
            |c| c.code_indented = false,
            "    # a\n    > b",
            "<h1>a</h1>\n<blockquote>\n<p>b</p>\n</blockquote>",
        ),
        (|c| c.heading_atx = false, "# a\n***", "<p># a</p>\n<hr />"),
        (|c| c.heading_setext = false, "a\n===", "<p>a\n===</p>"),
        (|c| c.thematic_break = false, "***", "<p>***</p>"),
        (
            |c| c.definition = false,
            "[a]: /a\n\n[a]",
            "<p>[a]: /a</p>\n<p>[a]</p>",
        ),
        (|c| c.html_flow = false, "<div>\ntext", "<p><div>\ntext</p>"),
    ];
    for &(configure, source, expected) in cases {
        let mut options = Options::default();
        options.compile.allow_dangerous_html = true;
        configure(&mut options.parse.constructs);
        assert_html(source, &options, expected);
    }
    let mut options = Options::gfm();
    options.parse.constructs.table = false;
    assert_html("a|b\n-|-\nc|d", &options, "<p>a|b\n-|-\nc|d</p>");
}

#[test]
fn disabling_indented_code_allows_indented_interruptions_and_fence_closers() {
    let mut options = Options::gfm();
    options.parse.constructs.code_indented = false;
    for (source, expected) in [
        ("a\n        # b", "<p>a</p>\n<h1>b</h1>"),
        ("    ```\n    a\n        ```", "<pre><code>a\n</code></pre>"),
        ("a|b\n        -|-\n        c|d", "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>c</td>\n<td>d</td>\n</tr>\n</tbody>\n</table>"),
    ] {
        assert_html(source, &options, expected);
    }
}

#[test]
fn definitions_preserve_following_content_and_container_continuation() {
    for (source, expected) in [
        ("[a]: /a\n    [b]: /b\n[b]", "<p><a href=\"/b\">b</a></p>"),
        ("[a]: /a\n    text [a]", "<p>text <a href=\"/a\">a</a></p>"),
        ("[a]: /a\n---", "<p>---</p>"),
        (
            "> [a]: /a\nlazy [a]",
            "<blockquote>\n<p>lazy <a href=\"/a\">a</a></p>\n</blockquote>",
        ),
        (
            "> [a\n> b]: /ab\n>\n> [a b]",
            "<blockquote>\n<p><a href=\"/ab\">a b</a></p>\n</blockquote>",
        ),
    ] {
        assert_html(source, &Options::default(), expected);
    }
}

#[test]
fn lazy_table_headers_keep_the_enclosing_container() {
    for (source, expected) in [
        ("- x\na|b\n  --|--", "<ul>\n<li>x\n<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n</table>\n</li>\n</ul>"),
        ("> --|--\n--|--\n> --|--", "<blockquote>\n<p>--|--</p>\n<table>\n<thead>\n<tr>\n<th>--</th>\n<th>--</th>\n</tr>\n</thead>\n</table>\n</blockquote>"),
    ] {
        assert_html(source, &Options::gfm(), expected);
    }
}

#[test]
fn tables_accept_inline_syntax_and_ordinary_header_opening_bytes() {
    for (header, row, expected_header, expected_row) in [
        ("item", "text", "item", "text"),
        ("`code`", "~~old~~", "<code>code</code>", "<del>old</del>"),
        ("~~old~~", "`code`", "<del>old</del>", "<code>code</code>"),
    ] {
        assert_html(
            &format!("{header}|b\n--|--\n{row}|d"),
            &Options::gfm(),
            &format!("<table>\n<thead>\n<tr>\n<th>{expected_header}</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>{expected_row}</td>\n<td>d</td>\n</tr>\n</tbody>\n</table>"),
        );
    }
}

#[test]
fn indented_table_headers_after_definitions_keep_paragraph_precedence() {
    for (header, expected) in [
        ("[x]|y", "<p>[x]|y\n-|-</p>\n"),
        ("x|y", "<p>x|y\n-|-</p>\n"),
        (
            "[x]: /x|y",
            "<table>\n<thead>\n<tr>\n<th>[x]: /x</th>\n<th>y</th>\n</tr>\n</thead>\n</table>\n",
        ),
    ] {
        assert_html(
            &format!("[d]: /d\n    {header}\n-|-\n"),
            &Options::gfm(),
            expected,
        );
    }
}

#[test]
fn complete_html_ends_a_lazy_container_but_not_an_ordinary_paragraph() {
    let mut options = Options::default();
    options.compile.allow_dangerous_html = true;
    assert_html(
        "> a\n<a>",
        &options,
        "<blockquote>\n<p>a</p>\n</blockquote>\n<a>",
    );
    assert_html("a\n<a>", &options, "<p>a\n<a></p>");
}

#[test]
fn frontmatter_and_math_are_independent_of_code_fences() {
    let mut options = Options::default();
    options.parse.constructs.code_fenced = false;
    options.parse.constructs.frontmatter = true;
    options.parse.constructs.math_flow = true;
    for source in ["", "\u{feff}"] {
        assert_html(source, &options, "");
    }
    assert_html(
        "```yaml\na: b\n```\n\n$$\nx\n$$",
        &options,
        "<pre><code class=\"language-math math-display\">x\n</code></pre>",
    );
}

#[test]
fn definition_titles_strip_continuation_padding_and_preserve_line_endings() {
    for ending in ["\r", "\n", "\r\n"] {
        for first in ["first", "first&amp;"] {
            let source = format!("[a]: /d \"{first}{ending}  second\"{ending}{ending}[a]");
            assert_html(
                &source,
                &Options::default(),
                &format!("<p><a href=\"/d\" title=\"{first}{ending}second\">a</a></p>"),
            );
        }
    }
}

#[test]
fn invalid_definitions_leave_their_authored_text_visible() {
    for source in [
        "[a[b]: /d",
        "[a\nb[c]: /d",
        "[a]: /d\u{b}",
        "[a]: /d (a(b))",
    ] {
        assert_html(source, &Options::default(), &format!("<p>{source}</p>"));
    }
    for (control, encoded) in [("\u{1}", "%01"), ("\u{b}", "%0B"), ("\u{7f}", "%7F")] {
        assert_html(
            &format!("[a]: <b{control}c>\n\n[a]"),
            &Options::default(),
            &format!("<p><a href=\"b{encoded}c\">a</a></p>"),
        );
        let source = format!("[a]: /d{control}");
        assert_html(&source, &Options::default(), &format!("<p>{source}</p>"));
    }
}

#[test]
fn definition_label_limits_include_expanded_tabs_and_escaped_brackets() {
    let mut options = Options::default();
    options.parse.constructs.code_indented = false;
    for prefix in ["", "\t", "> \t"] {
        let label = "a\t\\]\\\\b";
        let source = format!("{prefix}[{label}]: /d\n\n{prefix}[{label}]");
        let expected = if prefix.starts_with('>') {
            "<blockquote>\n</blockquote>\n<blockquote>\n<p><a href=\"/d\">a\t]\\b</a></p>\n</blockquote>"
        } else {
            "<p><a href=\"/d\">a\t]\\b</a></p>"
        };
        assert_html(&source, &options, expected);
    }
    for count in [998, 999, 1000] {
        let label = "a".repeat(count);
        let source = format!("[{label}]: /d\n\n[{label}]");
        let output = to_html_with_options(&source, &Options::default());
        assert_eq!(output.contains("href=\"/d\""), count <= 999);
    }
}
