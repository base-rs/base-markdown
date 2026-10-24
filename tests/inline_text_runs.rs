// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    render_events,
    stream::{EventKind, Tag, Text},
    to_html_with_options, AstBuilder, BuildProgress, Document, EventParser, Options,
};

fn assert_rendering_paths(source: &str, options: &Options, expected: &str) {
    assert_eq!(
        to_html_with_options(source, options).unwrap(),
        expected,
        "{source:?}"
    );
    let mut output = String::new();
    render_events(
        source,
        EventParser::new(source, &options.parse).unwrap(),
        &options.compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, expected, "checked: {source:?}");
    let document = Document::parse(source, &options.parse).unwrap();
    output.clear();
    render_events(source, document.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected, "arena: {source:?}");
}

#[test]
fn escaped_text_runs_retain_source_offsets_and_support_bounded_ast_edits() {
    let source = r"\*stars\* and \_世界\_";
    for options in [Options::default(), Options::gfm()] {
        let mut runs = Vec::new();
        for event in EventParser::new(source, &options.parse).unwrap() {
            let event = event.unwrap();
            if let EventKind::Text(value) = event.kind {
                let Text::Source(range) = value else {
                    panic!("ordinary text must remain source-backed")
                };
                assert_eq!(event.span, range);
                runs.push(range);
            }
        }
        assert_eq!(runs, [1..7, 8..14, 15..22, 23..24]);
        assert_eq!(
            runs.iter()
                .map(|range| &source[range.clone()])
                .collect::<Vec<_>>(),
            ["*stars", "* and ", "_世界", "_"]
        );

        let mut builder = AstBuilder::new();
        let mut events = EventParser::new(source, &options.parse).unwrap();
        while builder.consume(&mut events, 1).unwrap() == BuildProgress::Pending {}
        assert_eq!(builder.len(), 6); // Document, paragraph, and four source-backed text nodes.
        let mut document = builder.finish(source.to_owned()).unwrap();
        let paragraph = document
            .node(document.root())
            .unwrap()
            .first_child()
            .unwrap();
        let text = document.node(paragraph).unwrap().first_child().unwrap();
        *document.text_mut(text).unwrap() = "changed".into();
        assert_eq!(document.source(), source);
        assert_eq!(document.node(text).unwrap().span, 1..7);
        let mut output = String::new();
        render_events(
            document.source(),
            document.events(),
            &options.compile,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, "<p>changed* and _世界_</p>");
    }
}

#[test]
fn every_escaped_ascii_punctuation_can_start_a_unicode_text_run() {
    for byte in (0..=127_u8).filter(u8::is_ascii_punctuation) {
        let punctuation = char::from(byte);
        let source = format!("before \\{punctuation}世界 after");
        let encoded = match punctuation {
            '&' => "&amp;".into(),
            '"' => "&quot;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            _ => punctuation.to_string(),
        };
        for options in [Options::default(), Options::gfm()] {
            assert_rendering_paths(
                &source,
                &options,
                &format!("<p>before {encoded}世界 after</p>"),
            );
        }
    }
}

#[test]
fn coalescing_stops_before_semantic_boundaries() {
    for (source, expected) in [
        (r"\**x**", "<p>*<em>x</em>*</p>"),
        (r"\[label](url)", "<p>[label](url)</p>"),
        (r"\! [x](url)", "<p>! <a href=\"url\">x</a></p>"),
        (r"\&amp; &amp;", "<p>&amp;amp; &amp;</p>"),
        (r"\`prefix `code`", "<p>`prefix <code>code</code></p>"),
        ("\\*line  \r\nnext", "<p>*line<br />\r\nnext</p>"),
        (r"\*a \_b\_ **c**", "<p>*a _b_ <strong>c</strong></p>"),
        (
            r#"[\*text](url "title")"#,
            "<p><a href=\"url\" title=\"title\">*text</a></p>",
        ),
    ] {
        for options in [Options::default(), Options::gfm()] {
            assert_rendering_paths(source, &options, expected);
        }
    }
}

#[test]
fn structured_details_keep_their_own_source_ranges() {
    let source = r#"\*label [text](<https://example.com/p> "a title") and ` code `"#;
    let options = Options::default();
    let events = EventParser::new(source, &options.parse)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let link = events
        .iter()
        .find(|event| matches!(event.kind, EventKind::Start(Tag::Link { .. })))
        .unwrap();
    let EventKind::Start(Tag::Link { destination, title }) = &link.kind else {
        unreachable!()
    };
    for (value, expected) in [
        (destination, "https://example.com/p"),
        (title.as_ref().unwrap(), "a title"),
    ] {
        let start = source.find(expected).unwrap();
        assert_eq!(value, &Text::Source(start..start + expected.len()));
    }
    assert_eq!(
        &source[link.span.clone()],
        r#"[text](<https://example.com/p> "a title")"#
    );
    let code = events
        .iter()
        .find(|event| matches!(event.kind, EventKind::Code { .. }))
        .unwrap();
    let EventKind::Code { value, .. } = &code.kind else {
        unreachable!()
    };
    let start = source.find("code").unwrap();
    assert_eq!(value, &Text::Source(start..start + 4));
    assert_eq!(&source[code.span.clone()], "` code `");
}

#[test]
fn links_without_emphasis_preserve_delimiters_in_enclosing_and_following_regions() {
    for options in [Options::default(), Options::gfm()] {
        for (source, expected) in [
            ("*outside [plain](/one) after* [*inner*](/two) [plain](/three) _end_", "<p><em>outside <a href=\"/one\">plain</a> after</em> <a href=\"/two\"><em>inner</em></a> <a href=\"/three\">plain</a> <em>end</em></p>"),
            ("*unclosed\n\n[plain](/one)\n\n*after*", "<p>*unclosed</p>\n<p><a href=\"/one\">plain</a></p>\n<p><em>after</em></p>"),
            ("*outer [plain][ref] and ![alt](/img) tail*\n\n[ref]: /target", "<p><em>outer <a href=\"/target\">plain</a> and <img src=\"/img\" alt=\"alt\" /> tail</em></p>\n"),
        ] {
            assert_rendering_paths(source, &options, expected);
        }
    }
}
