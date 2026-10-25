// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    render_events,
    stream::{EventKind, Tag, Text},
    to_html_with_options, Document, EventParser, Options, ParseOptions,
};

fn assert_rendering_paths(source: &str, options: &Options, expected: &str) {
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
    render_events(source, document.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected, "arena: {source:?}");
}

#[test]
fn block_indentation_preserves_line_endings_and_quote_continuations() {
    for options in [Options::default(), Options::gfm()] {
        for indent in ["", " ", "  ", "   "] {
            for ending in ["\n", "\r", "\r\n"] {
                let source = format!("{indent}# heading{ending}{indent}{ending}{indent}paragraph{ending}{ending}{indent}> quote{ending}{indent}> next");
                let expected = format!("<h1>heading</h1>{ending}<p>paragraph</p>{ending}<blockquote>{ending}<p>quote{ending}next</p>{ending}</blockquote>");
                assert_rendering_paths(&source, &options, &expected);
            }
        }
    }
}

#[test]
fn partial_tabs_and_blank_lines_keep_block_boundaries() {
    for mut options in [Options::default(), Options::gfm()] {
        options.compile.allow_dangerous_html = true;
        for (source, expected) in [
            (" \t\n   # a\n \t\n   ---\n  \t", "<h1>a</h1>\n<hr />\n"),
            (" - a\n \t\n - b", "<ul>\n<li>\n<p>a</p>\n</li>\n<li>\n<p>b</p>\n</li>\n</ul>"),
            ("  > \t# inside\n  > \t\n  > next\noutside", "<blockquote>\n<pre><code># inside\n</code></pre>\n<p>next\noutside</p>\n</blockquote>"),
            ("   <!-- x -->\n\ntext", "   <!-- x -->\n<p>text</p>"),
            ("   ```\n\t<x>\n   ```\n\n\t", "<pre><code> &lt;x&gt;\n</code></pre>"),
        ] {
            assert_rendering_paths(source, &options, expected);
        }
    }
}

#[test]
fn definition_offsets_include_the_authored_indentation() {
    let source = "   [ref]: /target \"title\"\n\n   [ref]";
    for options in [Options::default(), Options::gfm()] {
        assert_rendering_paths(
            source,
            &options,
            "<p><a href=\"/target\" title=\"title\">ref</a></p>",
        );
        let event = EventParser::new(source, &options.parse)
            .map(Result::unwrap)
            .find(|event| matches!(event.kind, EventKind::Definition { .. }))
            .unwrap();
        assert_eq!(event.span, 0..25);
        assert_eq!(
            event.kind,
            EventKind::Definition {
                label: Text::Source(4..7),
                destination: Text::Source(10..17),
                title: Some(Text::Source(19..24)),
            }
        );
    }
}

#[test]
fn paragraph_boundaries_distinguish_block_markers_from_ordinary_text() {
    for options in [Options::default(), Options::gfm()] {
        for (source, expected) in [
            ("first\nordinary", "<p>first\nordinary</p>"),
            ("first\né中", "<p>first\né中</p>"),
            ("first\n#not heading", "<p>first\n#not heading</p>"),
            ("first\n####### heading", "<p>first\n####### heading</p>"),
            ("first\n*not a list", "<p>first\n*not a list</p>"),
            ("first\n_not a rule", "<p>first\n_not a rule</p>"),
            ("first\n-not setext", "<p>first\n-not setext</p>"),
            ("first\n+not a list", "<p>first\n+not a list</p>"),
            (
                "first\n42. not an interrupt",
                "<p>first\n42. not an interrupt</p>",
            ),
            ("first\n[link](/x)", "<p>first\n<a href=\"/x\">link</a></p>"),
            ("first\n`code`", "<p>first\n<code>code</code></p>"),
            ("first\n~~not a fence", "<p>first\n~~not a fence</p>"),
            ("first\n# heading", "<p>first</p>\n<h1>heading</h1>"),
            ("first\n***", "<p>first</p>\n<hr />"),
            ("first\n___", "<p>first</p>\n<hr />"),
            ("first\n1. item", "<p>first</p>\n<ol>\n<li>item</li>\n</ol>"),
            (
                "first\n> quote",
                "<p>first</p>\n<blockquote>\n<p>quote</p>\n</blockquote>",
            ),
            (
                "first\n~~~\ncode\n~~~",
                "<p>first</p>\n<pre><code>code\n</code></pre>",
            ),
        ] {
            for ending in ["\n", "\r", "\r\n"] {
                assert_rendering_paths(
                    &source.replace('\n', ending),
                    &options,
                    &expected.replace('\n', ending),
                );
            }
        }
    }
}

#[test]
fn nested_empty_items_preserve_the_outer_boundary_before_a_blank_line() {
    for ending in ["\n", "\r", "\r\n"] {
        let source = "- > - a\n  >\n  > -\n\noutside\n".replace('\n', ending);
        let expected_end = source.find(&format!("{ending}{ending}")).unwrap() + ending.len();
        let document =
            core_markdown::Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
        let item = document
            .descendants(document.root())
            .find(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::Item { .. })
                )
            })
            .unwrap();
        assert_eq!(document.node(item).unwrap().span, 0..expected_end);
    }
}
