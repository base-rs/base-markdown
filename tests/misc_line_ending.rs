// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{to_html_with_options, CompileOptions, Options};
use pretty_assertions::assert_eq;

// Line endings in content, such as text, code and HTML, are LF. Those the HTML renderer writes
// itself, between blocks and after code without a final line ending, are authored.
#[test]
fn line_ending() {
    let danger = &Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Options::commonmark()
    };

    assert_eq!(
        to_html_with_options("\n", &Options::commonmark()),
        "",
        "should support just a line feed"
    );

    assert_eq!(
        to_html_with_options("\r", &Options::commonmark()),
        "",
        "should support just a carriage return"
    );

    assert_eq!(
        to_html_with_options("\r\n", &Options::commonmark()),
        "",
        "should support just a carriage return + line feed"
    );

    assert_eq!(
        to_html_with_options("\n\n", &Options::commonmark()),
        "",
        "should support just two line feeds"
    );

    assert_eq!(
        to_html_with_options("\r\r", &Options::commonmark()),
        "",
        "should support just two carriage return"
    );

    assert_eq!(
        to_html_with_options("\r\n\r\n", &Options::commonmark()),
        "",
        "should support just two carriage return + line feeds"
    );

    assert_eq!(
        to_html_with_options("a\nb", &Options::commonmark()),
        "<p>a\nb</p>",
        "should support a line feed for a line ending inside a paragraph"
    );

    assert_eq!(
        to_html_with_options("a\rb", &Options::commonmark()),
        "<p>a\nb</p>",
        "should support a carriage return for a line ending inside a paragraph"
    );

    assert_eq!(
        to_html_with_options("a\r\nb", &Options::commonmark()),
        "<p>a\nb</p>",
        "should support a carriage return + line feed for a line ending inside a paragraph"
    );

    assert_eq!(
        to_html_with_options("\ta\n\tb", &Options::commonmark()),
        "<pre><code>a\nb\n</code></pre>",
        "should support a line feed in indented code (and prefer it)"
    );

    assert_eq!(
        to_html_with_options("\ta\r\tb", &Options::commonmark()),
        "<pre><code>a\nb\n</code></pre>",
        "should support a carriage return in indented code (and prefer it)"
    );

    assert_eq!(
        to_html_with_options("\ta\r\n\tb", &Options::commonmark()),
        "<pre><code>a\nb\n</code></pre>",
        "should support a carriage return + line feed in indented code (and prefer it)"
    );

    assert_eq!(
        to_html_with_options("***\n### Heading", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>",
        "should support a line feed between flow"
    );

    assert_eq!(
        to_html_with_options("***\r### Heading", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>",
        "should support a carriage return between flow"
    );

    assert_eq!(
        to_html_with_options("***\r\n### Heading", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>",
        "should support a carriage return + line feed between flow"
    );

    assert_eq!(
        to_html_with_options("***\n\n\n### Heading\n", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>\n",
        "should support several line feeds between flow"
    );

    assert_eq!(
        to_html_with_options("***\r\r\r### Heading\r", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>\n",
        "should support several carriage returns between flow"
    );

    assert_eq!(
        to_html_with_options("***\r\n\r\n\r\n### Heading\r\n", &Options::commonmark()),
        "<hr />\n<h3>Heading</h3>\n",
        "should support several carriage return + line feeds between flow"
    );

    assert_eq!(
        to_html_with_options("```x\n\n\ny\n\n\n```\n\n\n", &Options::commonmark()),
        "<pre><code class=\"language-x\">\n\ny\n\n\n</code></pre>\n",
        "should support several line feeds in fenced code"
    );

    assert_eq!(
        to_html_with_options("```x\r\r\ry\r\r\r```\r\r\r", &Options::commonmark()),
        "<pre><code class=\"language-x\">\n\ny\n\n\n</code></pre>\n",
        "should support several carriage returns in fenced code"
    );

    assert_eq!(
        to_html_with_options(
            "```x\r\n\r\n\r\ny\r\n\r\n\r\n```\r\n\r\n\r\n",
            &Options::commonmark()
        ),
        "<pre><code class=\"language-x\">\n\ny\n\n\n</code></pre>\n",
        "should support several carriage return + line feeds in fenced code"
    );

    assert_eq!(
        to_html_with_options("A\r\nB\r\n-\r\nC", &Options::commonmark()),
        "<h2>A\nB</h2>\n<p>C</p>",
        "should support a carriage return + line feed in content"
    );

    assert_eq!(
        to_html_with_options("<div\n", danger),
        "<div\n",
        "should support a line feed after html"
    );

    assert_eq!(
        to_html_with_options("<div\r", danger),
        "<div\n",
        "should support a carriage return after html"
    );

    assert_eq!(
        to_html_with_options("<div\r\n", danger),
        "<div\n",
        "should support a carriage return + line feed after html"
    );

    assert_eq!(
        to_html_with_options("<div>\n\nx", danger),
        "<div>\n<p>x</p>",
        "should support a blank line w/ line feeds after html"
    );

    assert_eq!(
        to_html_with_options("<div>\r\rx", danger),
        "<div>\n<p>x</p>",
        "should support a blank line w/ carriage returns after html"
    );

    assert_eq!(
        to_html_with_options("<div>\r\n\r\nx", danger),
        "<div>\n<p>x</p>",
        "should support a blank line w/ carriage return + line feeds after html"
    );

    assert_eq!(
        to_html_with_options("<div>\nx", danger),
        "<div>\nx",
        "should support a non-blank line w/ line feed in html"
    );

    assert_eq!(
        to_html_with_options("<div>\nx", danger),
        "<div>\nx",
        "should support a non-blank line w/ carriage return in html"
    );

    assert_eq!(
        to_html_with_options("<div>\nx", danger),
        "<div>\nx",
        "should support a non-blank line w/ carriage return + line feed in html"
    );
}
