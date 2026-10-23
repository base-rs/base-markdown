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
        ("- a\n\n- \n\n> b", "<ul>\n<li>\n<p>a</p>\n</li>\n<li></li>\n</ul>\n<blockquote>\n<p>b</p>\n</blockquote>"),
        ("x\n\n> - a\r\n> 1. b", "<p>x</p>\n<blockquote>\n<ul>\n<li>a</li>\n</ul>\r\n<ol>\n<li>b</li>\n</ol>\n</blockquote>"),
        ("a\n1.     b", "<p>a</p>\n<ol>\n<li>\n<pre><code>b\n</code></pre>\n</li>\n</ol>"),
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

#[test]
fn compact_inline_resolution_keeps_escaped_markers_and_unmatched_code() {
    for (source, expected) in [
        ("\\**x**", "<p>*<em>x</em>*</p>"),
        ("**a*_*_**", "<p><em><em>a</em><em>*</em></em>*</p>"),
        ("a [x](<\0>)", "<p>a <a href=\"%EF%BF%BD\">x</a></p>"),
        ("![x](<\0>)", "<p><img src=\"%EF%BF%BD\" alt=\"x\" /></p>"),
        (
            "a [x](b \"\0\")",
            "<p>a <a href=\"b\" title=\"�\">x</a></p>",
        ),
        (
            "![x](b \"\0\")",
            "<p><img src=\"b\" alt=\"x\" title=\"�\" /></p>",
        ),
        (
            ">_) _\"__",
            "<blockquote>\n<p><em>) <em>&quot;</em></em></p>\n</blockquote>",
        ),
        (
            ">a\n>_) _\"__",
            "<blockquote>\n<p>a\n<em>) <em>&quot;</em></em></p>\n</blockquote>",
        ),
        ("a &](b &amp;)", "<p>a &amp;](b &amp;)</p>"),
        ("a `b``c```d``e`", "<p>a <code>b``c```d``e</code></p>"),
        ("a `x\n y`", "<p>a <code>x  y</code></p>"),
        ("a ``x`y", "<p>a ``x`y</p>"),
    ] {
        assert_eq!(to_html(source), expected, "{source:?}");
    }
}

#[test]
fn compact_resources_and_fallback_regions_agree_with_legacy_rendering() {
    use core_markdown::{to_mdast, HtmlRenderer, ParseOptions};
    for source in [
        "a [x]( b)",
        "Literal ! and !x.",
        "-\tfoo <x>",
        "***a** b*",
        "a***b**c*",
        "a **b*c** d*",
        "    a\n\n\n    b",
        "#a",
        "####### a",
        "#\t",
        "a\n10. b",
        "{a}",
        "a {b}",
        "~~~a~\nb\n~~~",
        "~~~a b~\nc\n~~~",
        "a [x](<b>)",
        "a [x](<b\\>c>)",
        "a [x](<b\\ c>)",
        "a [x](<b<c>)",
        "a [x](<b",
        "a [x](b\\(c)",
        "a [x](b\\ c)",
        "a [x](b(c))",
        "a [x](b(c)",
        "a [x](b\u{7f})",
        "a [x](b\u{1})",
        "a [x](b 'title')",
        "a [x](b (title))",
        "a [x](b (bad(title)))",
        "a [x](b \"ti\\\"tle\")",
        "a [x](b \"ti\\ tle\")",
        "a [x](b \"title\"  )",
        "a [x](b \"title\" x)",
        "a [x](b \"title",
        "a [x](b \"title\ntitle\")",
        "a [x](<b\nc>)",
        "a [x](b\n\"title\")",
        "a [x](b ",
        "a &copy; &invalid; &#9999999; &#X41; &#xZZ; &#; &;",
        "a \\*b*",
        "a `b``c",
        "a **b _c_** d*",
        ">_](x)\t\u{c}_\\*\"__\u{c}('",
        "a [b [c](d)](e)",
        "a [![b](c)](d)",
        "a [x](b)",
        "a\n    \nb",
        "a\n\t\nb",
        "-\n\n-",
        "*\n\n a",
        "a\n3",
        "```&#x61;\nb\n```",
        "```&#97;\nb\n```",
        "```&bad;\nb\n```",
        "```a\\-b\nb\n```",
    ] {
        let expected = to_mdast(source, &ParseOptions::default())
            .unwrap()
            .render_with(&mut HtmlRenderer::default());
        assert_eq!(to_html(source), expected, "{source:?}");
    }
    let deep_destination = format!("a [x]({}x{})", "(".repeat(34), ")".repeat(34));
    let expected = to_mdast(&deep_destination, &ParseOptions::default())
        .unwrap()
        .render_with(&mut HtmlRenderer::default());
    assert_eq!(to_html(&deep_destination), expected);
    let options = Options {
        parse: ParseOptions {
            inline_code_info: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    for source in ["#", "\t<x>", "a\n\t<b>"] {
        let expected = to_mdast(source, &options.parse)
            .unwrap()
            .render_with(&mut HtmlRenderer::default());
        assert_eq!(
            core_markdown::to_html_with_options(source, &options).unwrap(),
            expected
        );
    }
}

#[test]
fn semantic_endings_ignore_trailing_blanks_and_container_prefixes() {
    for (source, expected) in [
        ("a\r\r\n", "<p>a</p>\r"),
        ("x\n\n- a\r\n", "<p>x</p>\n<ul>\n<li>a</li>\n</ul>\r\n"),
        ("x\n\n> a\r\n", "<p>x</p>\n<blockquote>\n<p>a</p>\n</blockquote>\r\n"),
        ("> -\n ", "<blockquote>\n<ul>\n<li></li>\n</ul>\n</blockquote>"),
        ("x\n\n- a\r\r\n", "<p>x</p>\n<ul>\n<li>a</li>\n</ul>\n"),
        ("x\n\n> a\r\r\n", "<p>x</p>\n<blockquote>\n<p>a</p>\n</blockquote>\r"),
        (">     a\n ", "<blockquote>\n<pre><code>a\n</code></pre>\n</blockquote>\n"),
        ("x\n\n- # a\r\n ", "<p>x</p>\n<ul>\n<li>\n<h1>a</h1>\n</li>\n</ul>"),
        ("# a\n    ", "<h1>a</h1>\n"),
        ("---\n    ", "<hr />\n"),
        ("- a\r\r\n", "<ul>\r<li>a</li>\r</ul>\r"),
        ("---\n> \r", "<hr />\n<blockquote>\n</blockquote>\n"),
        ("> a\n ", "<blockquote>\n<p>a</p>\n</blockquote>\n"),
        (">\n ", "<blockquote>\n</blockquote>"),
        ("# a\n\r", "<h1>a</h1>\n"),
        ("+ \n  ", "<ul>\n<li></li>\n</ul>"),
        ("x\n\n> - - # a\r\n>   b", "<p>x</p>\n<blockquote>\n<ul>\n<li>\n<ul>\n<li>\n<h1>a</h1>\n</li>\n</ul>\r\nb</li>\n</ul>\n</blockquote>"),
    ] {
        assert_eq!(to_html(source), expected, "{source:?}");
    }
}

#[test]
fn compact_inlines_share_compatibility_document_structure() {
    for (source, expected) in [
        ("<div>\n</div>\n\n- a  \n\t``` ", "&lt;div&gt;\n&lt;/div&gt;\n<ul>\n<li>a\n<pre><code></code></pre>\n</li>\n</ul>"),
        ("<div>\n</div>\n\n- # a\n\n   b  ", "&lt;div&gt;\n&lt;/div&gt;\n<ul>\n<li>\n<h1>a</h1>\n<p>b</p>\n</li>\n</ul>"),
        ("<div>\n</div>\n\n- ```\n  ```\n\n   b  ", "&lt;div&gt;\n&lt;/div&gt;\n<ul>\n<li>\n<pre><code></code></pre>\n<p>b</p>\n</li>\n</ul>"),

        (" [a](u)", "<p><a href=\"u\">a</a></p>"),
        ("[a](u) ", "<p><a href=\"u\">a</a></p>"),
        ("[ ", "<p>[</p>"),
        ("<div>\n</div>\n\na ", "&lt;div&gt;\n&lt;/div&gt;\n<p>a</p>"),
        ("plain *emphasis* and `code`\n\n[id]: /path\n", "<p>plain <em>emphasis</em> and <code>code</code></p>\n"),
        ("plain *emphasis*\n\n[x][id]\n\n[id]: /path 'title'\n", "<p>plain <em>emphasis</em></p>\n<p><a href=\"/path\" title=\"title\">x</a></p>\n"),
        ("plain &copy;\n\n<div>\nraw\n</div>\n", "<p>plain ©</p>\n&lt;div&gt;\nraw\n&lt;/div&gt;\n"),
        ("[not a definition] *emphasis*\n\n-\t**bold**\n\n[id]: /path\n", "<p>[not a definition] <em>emphasis</em></p>\n<ul>\n<li><strong>bold</strong></li>\n</ul>"),
        ("[x](url)\n\n## *heading*\n\n> **a**\n> b\n", "<p><a href=\"url\">x</a></p>\n<h2><em>heading</em></h2>\n<blockquote>\n<p><strong>a</strong>\nb</p>\n</blockquote>\n"),
    ] {
        let options = Options::default();
        let output = to_html(source);
        assert_eq!(output, expected, "{source:?}");
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, output, "{source:?}");
    }
}

#[test]
fn legacy_ast_preserves_empty_container_structure() {
    use core_markdown::{mdast::Node, to_mdast, ParseOptions};
    for source in ["* >\n\n* a", "* > \n\n* a", "*  \n\n* a"] {
        let root = to_mdast(source, &ParseOptions::default()).unwrap();
        let Node::List(list) = &root.children().unwrap()[0] else {
            panic!("the document starts with a list");
        };
        assert_eq!(list.children.len(), 2, "{source:?}");
        assert_eq!(list.children[0].text_content(), "", "{source:?}");
        assert_eq!(list.children[1].text_content(), "a", "{source:?}");
    }
    let root = to_mdast("- a\n \n  b", &ParseOptions::default()).unwrap();
    let item = &root.children().unwrap()[0].children().unwrap()[0];
    assert_eq!(item.children().unwrap().len(), 2);
    assert_eq!(item.text_content(), "ab");
    let root = to_mdast("- \n  \n  a", &ParseOptions::default()).unwrap();
    let children = root.children().unwrap();
    assert!(matches!(&children[0], Node::List(list) if list.children.len() == 1));
    assert_eq!(children[0].text_content(), "");
    assert_eq!(children[1].text_content(), "a");
}

#[test]
fn leading_brackets_distinguish_links_from_possible_definitions() {
    use core_markdown::{to_mdast, HtmlRenderer, ParseOptions};
    for source in [
        "[text](url)",
        "[**strong**](url)",
        "[unclosed",
        "[unfinished\\",
        "[nested [label]](url)",
        "[escaped\\]label]: /url\n\n[escaped\\]label]",
        "[escaped\\[label]: /url\n\n[escaped\\[label]",
        "[multi\nline]: /url\n\n[multi line]",
        "> [multi\n> line]: /url\n>\n> [multi line]",
        "[label] : /url",
        "[label]: /url\n\n[label]",
    ] {
        let expected = to_mdast(source, &ParseOptions::default())
            .unwrap()
            .render_with(&mut HtmlRenderer::default());
        assert_eq!(to_html(source), expected, "{source:?}");
        let options = Options::default();
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, expected, "arena: {source:?}");
    }
}
