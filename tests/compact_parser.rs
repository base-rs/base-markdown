// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{render_events, to_html, Document, Options};

#[test]
fn block_boundaries_preserve_source_whitespace() {
    for (source, expected) in [
        ("> a\n\t> b", "<blockquote>\n<p>a\n&gt; b</p>\n</blockquote>"),
        (">\n\t>", "<blockquote>\n</blockquote>\n<pre><code>&gt;\n</code></pre>"),
        ("> ```\n\t> x", "<blockquote>\n<pre><code></code></pre>\n</blockquote>\n<pre><code>&gt; x\n</code></pre>"),
        ("> a`x\n  y`", "<blockquote>\n<p>a<code>x   y</code></p>\n</blockquote>"),
        ("- a`x\n b`", "<ul>\n<li>a<code>x  b</code></li>\n</ul>"),
        ("- - # a\n  b", "<ul>\n<li>\n<ul>\n<li>\n<h1>a</h1>\n</li>\n</ul>\nb</li>\n</ul>"),
        ("a\n ", "<p>a</p>\n"),
        ("a\r\n\n# b\n", "<p>a</p>\r\n<h1>b</h1>\n"),
        ("#\u{b}x", "<p>#\u{b}x</p>"),
        ("#\u{c}x", "<p>#\u{c}x</p>"),
        ("a\n---\u{b}\nb", "<p>a\n---\u{b}\nb</p>"),
        ("\u{c}", "<p>\u{c}</p>"),
        ("- - a\n  ```\n", "<ul>\n<li>\n<ul>\n<li>a</li>\n</ul>\n<pre><code></code></pre>\n</li>\n</ul>\n"),
    ] {
        let options = Options::default();
        assert_eq!(to_html(source), expected, "{source:?}");
        let document = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(source, document.events(), &options.compile, &mut output).unwrap();
        assert_eq!(output, expected, "arena: {source:?}");
    }
}
