// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{to_html_with_options, Document, Options};

fn assert_fallback_html(source: &str, expected: &str, gfm: bool) {
    let mut options = if gfm {
        Options::gfm()
    } else {
        Options::default()
    };
    // Namespaced HTML requires the grammar block producer; inline code metadata
    // also exercises its region tokenizer rather than compact inline scanning.
    options.parse.html_tag_namespaces = true;
    options.parse.inline_code_info = true;
    assert_eq!(
        to_html_with_options(source, &options).unwrap(),
        expected,
        "{source:?}"
    );
    let document = Document::parse(source, &options.parse).unwrap();
    let mut output = String::new();
    document.render_html(&options.compile, &mut output).unwrap();
    assert_eq!(output, expected, "arena: {source:?}");
}

#[test]
fn grammar_headings_preserve_closers_empty_headings_and_literal_underlines() {
    for (source, expected) in [
        ("#", "<h1></h1>"),
        ("####### a", "<p>####### a</p>"),
        ("#a", "<p>#a</p>"),
        ("# #", "<h1></h1>"),
        ("# a ##", "<h1>a</h1>"),
        ("# a # b ##", "<h1>a # b</h1>"),
        ("# a ##b", "<h1>a ##b</h1>"),
        ("# a \t## \t", "<h1>a</h1>"),
        ("heading\n=== \t", "<h1>heading</h1>"),
        ("a\n=\n=\na\n=", "<h1>a</h1>\n<h1>=\na</h1>"),
        (
            "a\n=\n=\na\n=\n=\nb\n=",
            "<h1>a</h1>\n<h1>=\na</h1>\n<h1>=\nb</h1>",
        ),
    ] {
        assert_fallback_html(source, expected, false);
    }
}

#[test]
fn grammar_lists_preserve_interruptions_empty_items_and_quote_boundaries() {
    for (source, expected) in [
        ("a\n10. b", "<p>a\n10. b</p>"),
        (
            "123456789. a",
            "<ol start=\"123456789\">\n<li>a</li>\n</ol>",
        ),
        ("-\n\n- a", "<ul>\n<li></li>\n<li>\n<p>a</p>\n</li>\n</ul>"),
        (
            "* >\n\n* a",
            "<ul>\n<li>\n<blockquote>\n</blockquote>\n</li>\n<li>\n<p>a</p>\n</li>\n</ul>",
        ),
        (
            "* > \n\n* a",
            "<ul>\n<li>\n<blockquote>\n</blockquote>\n</li>\n<li>\n<p>a</p>\n</li>\n</ul>",
        ),
        (
            "* >\n* a",
            "<ul>\n<li>\n<blockquote>\n</blockquote>\n</li>\n<li>a</li>\n</ul>",
        ),
        (
            "* > \n* a",
            "<ul>\n<li>\n<blockquote>\n</blockquote>\n</li>\n<li>a</li>\n</ul>",
        ),
        (
            "*  \n\n* a",
            "<ul>\n<li></li>\n<li>\n<p>a</p>\n</li>\n</ul>",
        ),
        ("- a\n\n  b", "<ul>\n<li>\n<p>a</p>\n<p>b</p>\n</li>\n</ul>"),
        ("- \n  \n  a", "<ul>\n<li></li>\n</ul>\n<p>a</p>"),
    ] {
        assert_fallback_html(source, expected, true);
    }
}

#[test]
fn grammar_fences_preserve_info_metadata_and_reject_embedded_markers() {
    for (source, expected) in [
        (
            "```rust \tmetadata\nx\n```",
            "<pre><code class=\"language-rust\">x\n</code></pre>",
        ),
        (
            "```rust \t",
            "<pre><code class=\"language-rust\"></code></pre>\n",
        ),
        ("```rust`bad", "<p>```rust`bad</p>"),
        ("```rust`bad\n# after", "<p>```rust`bad</p>\n<h1>after</h1>"),
        ("```rust meta`bad", "<p>```rust meta`bad</p>"),
        (
            "~~~rust`ok meta~\nx\n~~~",
            "<pre><code class=\"language-rust`ok\">x\n</code></pre>",
        ),
        (
            "~~~rust~\nx\n~~~",
            "<pre><code class=\"language-rust~\">x\n</code></pre>",
        ),
        (
            "~~~rust \tmetadata\nx\n~~~",
            "<pre><code class=\"language-rust\">x\n</code></pre>",
        ),
    ] {
        assert_fallback_html(source, expected, false);
    }
}

#[test]
fn repeated_inline_fallback_preserves_one_document_lifecycle() {
    use core_markdown::{
        stream::{EventKind, Tag, TagKind},
        EventParser,
    };
    let source =
        "first <i\n title='x'> [a]\n\ncompact [a]\n\nlast <b\n title='x'> [a]\n\n[a]: /a\n";
    for full_grammar in [false, true] {
        let mut options = Options::default();
        options.parse.html_tag_namespaces = full_grammar;
        options.parse.inline_code_info = full_grammar;
        let mut parser = EventParser::new(source, &options.parse).unwrap();
        let events: Vec<_> = parser.by_ref().map(Result::unwrap).collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, EventKind::Start(Tag::Document(_))))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, EventKind::End(TagKind::Document)))
                .count(),
            1
        );
        for _ in 0..3 {
            assert!(parser.next().is_none());
        }
        let mut checked = String::new();
        core_markdown::render_events(
            source,
            events.into_iter().map(Ok),
            &options.compile,
            &mut checked,
        )
        .unwrap();
        assert_eq!(checked, to_html_with_options(source, &options).unwrap());
    }
}
