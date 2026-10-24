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
        ("> - a\n>\n> -\n>\nb", "<blockquote>\n<ul>\n<li>\n<p>a</p>\n</li>\n<li></li>\n</ul>\n</blockquote>\n<p>b</p>"),
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
fn compact_resources_and_fallback_regions_agree_with_arena_rendering() {
    use core_markdown::ParseOptions;
    for source in [
        "a [x]( b)",
        "Literal ! and !x.",
        "-\tfoo <x>",
        "***a** b*",
        "a***b**c*",
        "a **b*c** d*",
        "    a\n\n\n    b",
        "    a\n  \n    b",
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
        let expected = arena_html(source, &ParseOptions::default());
        assert_eq!(to_html(source), expected, "{source:?}");
    }
    let deep_destination = format!("a [x]({}x{})", "(".repeat(34), ")".repeat(34));
    let expected = arena_html(&deep_destination, &ParseOptions::default());
    assert_eq!(to_html(&deep_destination), expected);
    let options = Options {
        parse: ParseOptions {
            inline_code_info: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    for source in ["#", "\t<x>", "a\n\t<b>"] {
        let expected = arena_html(source, &options.parse);
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
fn arena_preserves_empty_container_structure() {
    use core_markdown::stream::{EventKind, Tag};
    for source in [
        "* >\n\n* a",
        "* > \n\n* a",
        "*  \n\n* a",
        "* >\n* a",
        "* > \n* a",
    ] {
        let document = Document::parse(source, &Default::default()).unwrap();
        let list = document
            .children(document.root())
            .find(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::List { .. })
                )
            })
            .unwrap();
        let items: Vec<_> = document.children(list).collect();
        assert_eq!(items.len(), 2, "{source:?}");
        assert_eq!(document.text_content(items[0]), "", "{source:?}");
        assert_eq!(document.text_content(items[1]), "a", "{source:?}");
    }
    let document = Document::parse("- a\n \n  b", &Default::default()).unwrap();
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    let blocks: Vec<_> = document
        .children(item)
        .filter(|id| {
            !matches!(
                document.node(*id).unwrap().kind,
                EventKind::Separator { .. }
            )
        })
        .collect();
    assert_eq!(blocks.len(), 2);
    assert_eq!(document.text_content(item), "ab");
    let document = Document::parse("- \n  \n  a", &Default::default()).unwrap();
    let children: Vec<_> = document
        .children(document.root())
        .filter(|id| {
            !matches!(
                document.node(*id).unwrap().kind,
                EventKind::Separator { .. }
            )
        })
        .collect();
    assert_eq!(document.children(children[0]).count(), 1);
    assert_eq!(document.text_content(children[0]), "");
    assert_eq!(document.text_content(children[1]), "a");
}

#[test]
fn leading_brackets_distinguish_links_from_possible_definitions() {
    use core_markdown::ParseOptions;
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
        let expected = arena_html(source, &ParseOptions::default());
        assert_eq!(to_html(source), expected, "{source:?}");
        let options = Options::default();
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, expected, "arena: {source:?}");
    }
}

#[test]
fn ordinary_gfm_regions_preserve_extension_boundaries() {
    use core_markdown::{render_events, Document, EventParser, Options};
    let sources = [
        "ordinary *emphasis*, **strong**, `code`, &amp; and \\*escaped*.",
        "[explicit](https://example.com?q=x&n=2) and ![image](image.png)",
        "`www.example.com user@example.com https://example.com ~~code~~`",
        "www.example.com WWW.example.com (www.example.com)",
        "https://example.com mailto:user@example.com xmpp:user@example.com",
        "user@example.com and user\\@example.com and user&commat;example.com",
        "www&#46;example.com and www\\.example.com and words. More words.",
        "www.\nexample.com\nhttps:\n//example.com",
        "![www.example.com](image) [www.example.com](link)",
        "[^a] ![^a](b) ![^a] ![^a][b]\n\n[b]: /b",
        "first[^a].\n\n[^a]: note",
        "~~old~~ ~single~ **strong ~~old~~**",
        "[a](path) *emphasis* [a]\n\n[a]: /a",
        "- ordinary\n- [x] checked\n- [ ] unchecked\n\n  [x] second paragraph",
        "- [a]: /a\n  [x] after definition\n\n[a]",
        "- [\t] tabs\n- [\n] multiline\n- [x]\n  next",
        "| a | b |\n| :- | -: |\n| [explicit](https://example.com) | ~~old~~ |",
        "| a |\n| - |\n| `a\\|b` |\n| \\| |",
    ];
    for source in sources {
        let options = Options::gfm();
        let mut grammar = Options::gfm();
        // These sources contain no math; this forces the independent grammar path.
        grammar.parse.constructs.math_flow = true;
        let direct = core_markdown::to_html_with_options(source, &options).unwrap();
        let expected = core_markdown::to_html_with_options(source, &grammar).unwrap();
        assert_eq!(direct, expected, "source: {source:?}");
        let mut checked = String::new();
        render_events(
            source,
            EventParser::new(source, &options.parse).unwrap(),
            &options.compile,
            &mut checked,
        )
        .unwrap();
        assert_eq!(checked, direct, "checked: {source:?}");
        let document = Document::parse(source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, direct, "arena: {source:?}");
    }
}

#[test]
fn gfm_uses_commonmark_attention_boundaries_when_no_extension_is_present() {
    for source in ["[a]*a**_\n\n[a]: /a", "[a](path) *a**_ [a]*a**_\n\n[a]: /a"] {
        assert_eq!(
            core_markdown::to_html_with_options(source, &core_markdown::Options::gfm()).unwrap(),
            core_markdown::to_html(source)
        );
    }
}

#[test]
fn gfm_preserves_the_same_literal_line_endings_as_commonmark() {
    for source in [
        "intro\n\n    code\r\n",
        "intro\r\n\r\n    code\n",
        "intro\n\n```\r\ncode\r\n```\n",
    ] {
        assert_eq!(
            core_markdown::to_html_with_options(source, &core_markdown::Options::gfm()).unwrap(),
            core_markdown::to_html(source)
        );
    }
}

#[test]
fn grammar_fallback_preserves_virtual_spaces_in_code() {
    let mut options = Options::default();
    options.parse.constructs.math_flow = true;
    for source in [
        "-\t\tcode",
        "> \t\tcode",
        "- a\n\n  \t\tcode",
        "- ```\n\tcode\n  ```",
    ] {
        let direct = core_markdown::to_html_with_options(source, &options).unwrap();
        // The compatibility producer retains its documented virtual-space interpretation;
        // materializing the arena must preserve those constructed code values.
        assert!(direct.contains("code"));
        let document = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(source, document.events(), &options.compile, &mut output).unwrap();
        assert_eq!(output, direct);
    }
}

fn arena_html(source: &str, options: &core_markdown::ParseOptions) -> String {
    let document = Document::parse(source, options).unwrap();
    let mut output = String::new();
    document
        .render_html(&Default::default(), &mut output)
        .unwrap();
    output
}

#[test]
fn inline_angles_share_grammar_rules_and_preserve_authored_ranges() {
    use core_markdown::{stream::EventKind, to_html_with_options};
    let mut cases = vec![
        "plain < 3 and **bold**".to_string(),
        "x <?a>b?> <![CDAx[broken]> <![CDATA[unfinished".to_string(),
        "x <a\rtitle=\"a\0b\"> y </a>".to_string(),
        "x <a title=\"a\0b\"> y <!--c\0d-->".to_string(),
        "text <i>HTML</i> [a]\n\n[a]: /a".to_string(),
        "before <x a='>' b=word c d=\"value\"/> after".to_string(),
        "x <!--> <!---> <!-- a--b --> <?a??> <![CDATA[x]]]> <!doctype x>".to_string(),
        "x <a\n  title='two\n  lines'> y </a\n >".to_string(),
        "> before <span\n> title='value'>after</span>\n\n[a] <i>x</i>\n\n[a]: /a".to_string(),
        "before <a q=\"bad\"x> **after**".to_string(),
        "before <a / > <a q=> <a q=/> <a q='unterminated".to_string(),
        "[<https://example.test?a=1&amp;b=2>](outer) ![<b>x</b>](image)".to_string(),
        "<https://example.test/é> <user@example.test> <a:x> <ab:x>".to_string(),
        "<user@a-b> <user@-bad> <user@bad-> <user@a..b> <@a>".to_string(),
    ];
    for size in [1, 2, 31, 32, 33] {
        cases.push(format!("text <{}:path> after", "a".repeat(size)));
    }
    for size in [62, 63, 64] {
        cases.push(format!("text <user@{}.test> after", "a".repeat(size)));
    }
    for source in cases {
        let options = Options::default();
        let mut grammar = Options::default();
        // Force grammar inline tokenization without changing these inputs' semantics.
        grammar.parse.inline_code_info = true;
        let expected = to_html_with_options(&source, &grammar).unwrap();
        assert_eq!(
            to_html_with_options(&source, &options).unwrap(),
            expected,
            "{source:?}"
        );
        let document = Document::parse(source.as_str(), &options.parse).unwrap();
        let mut output = String::new();
        document.render_html(&options.compile, &mut output).unwrap();
        assert_eq!(output, expected, "AST: {source:?}");
    }
    let source = "text <ab:é> and <i q='x'>";
    let document = Document::parse(source, &Default::default()).unwrap();
    for node in document.nodes() {
        if matches!(
            node.kind,
            EventKind::Start(core_markdown::stream::Tag::Link { .. })
        ) {
            assert_eq!(&source[node.span.clone()], "<ab:é>");
        }
        if matches!(node.kind, EventKind::Html { block: false, .. }) {
            assert_eq!(&source[node.span.clone()], "<i q='x'>");
        }
    }
}
