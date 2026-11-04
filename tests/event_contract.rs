// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    render_events,
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    AstBuilder, BuildProgress, Document, EventHtmlRenderer, Options,
};

fn event(kind: EventKind<'static>) -> Result<Event<'static>, StreamError> {
    Ok(Event { kind, span: 0..0 })
}
fn document(body: Vec<EventKind<'static>>) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = vec![event(EventKind::Start(Tag::Document(None)))];
    events.extend(body.into_iter().map(event));
    events.push(event(EventKind::End(TagKind::Document)));
    events
}

#[test]
fn footnotes_keep_call_order_and_the_first_completed_definition() {
    let definition = |key| EventKind::Start(Tag::FootnoteDefinition(Text::Literal(key)));
    let end = || EventKind::End(TagKind::FootnoteDefinition);
    let body = vec![
        definition("unused"),
        EventKind::Text(Text::Literal("hidden")),
        end(),
        EventKind::FootnoteReference(Text::Literal("B")),
        EventKind::FootnoteReference(Text::Literal("Z")),
        EventKind::FootnoteReference(Text::Literal("ΟΣ")),
        EventKind::FootnoteReference(Text::Literal("B")),
        definition("Z"),
        end(),
        definition("Z"),
        EventKind::Text(Text::Literal("ignored")),
        end(),
        definition("B"),
        EventKind::Text(Text::Literal("outer")),
        definition("B"),
        EventKind::Text(Text::Literal("inner")),
        end(),
        end(),
        definition("ΟΣ"),
        EventKind::Text(Text::Literal("unicode")),
        end(),
    ];
    let mut output = String::new();
    render_events("", document(body), &Options::default().compile, &mut output).unwrap();
    assert!(output.contains("id=\"user-content-fnref-b-2\""));
    assert!(output.contains("id=\"user-content-fn-%CE%BF%CF%82\""));
    let first = output.find("<li id=\"user-content-fn-b\"").unwrap();
    let second = output.find("<li id=\"user-content-fn-z\"").unwrap();
    let third = output
        .find("<li id=\"user-content-fn-%CE%BF%CF%82\"")
        .unwrap();
    assert!(first < second && second < third);
    assert!(output.contains("inner"));
    assert!(output.contains("unicode"));
    for omitted in ["hidden", "ignored", "outer"] {
        assert!(!output.contains(omitted));
    }
}

#[test]
fn footnote_ids_survive_a_call_writer_failure() {
    struct Writer {
        calls: usize,
        fail_at: usize,
        output: String,
    }
    impl std::fmt::Write for Writer {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail_at {
                return Err(std::fmt::Error);
            }
            self.output.push_str(value);
            Ok(())
        }
    }
    let mut writer = Writer {
        calls: 0,
        fail_at: usize::MAX,
        output: String::new(),
    };
    let options = Options::default();
    {
        let mut renderer = base_markdown::EventHtmlRenderer::new("", &options.compile, &mut writer);
        renderer
            .event(event(EventKind::Start(Tag::Document(None))).unwrap())
            .unwrap();
        renderer
            .event(event(EventKind::FootnoteReference(Text::Literal("É"))).unwrap())
            .unwrap();
    }
    let writes = writer.calls;
    for fail_at in 0..writes {
        let mut writer = Writer {
            calls: 0,
            fail_at,
            output: String::new(),
        };
        let mut renderer = base_markdown::EventHtmlRenderer::new("", &options.compile, &mut writer);
        renderer
            .event(event(EventKind::Start(Tag::Document(None))).unwrap())
            .unwrap();
        assert!(matches!(
            renderer.event(event(EventKind::FootnoteReference(Text::Literal("É"))).unwrap()),
            Err(StreamError::Write(_))
        ));
        for kind in [
            EventKind::FootnoteReference(Text::Literal("É")),
            EventKind::Start(Tag::FootnoteDefinition(Text::Literal("É"))),
            EventKind::Text(Text::Literal("body")),
            EventKind::End(TagKind::FootnoteDefinition),
            EventKind::End(TagKind::Document),
        ] {
            renderer.event(event(kind).unwrap()).unwrap();
        }
        assert!(writer.output.contains("id=\"user-content-fnref-%C3%A9-2\""));
        assert!(writer.output.contains("<li id=\"user-content-fn-%C3%A9\""));
    }
}

#[test]
fn all_invalid_metadata_is_rejected_before_rendering_or_editing() {
    let bad = || Text::Source(1..2);
    let kinds = vec![
        EventKind::Start(Tag::Link {
            destination: bad(),
            title: None,
        }),
        EventKind::Start(Tag::Image {
            destination: Text::Literal("x"),
            title: Some(bad()),
        }),
        EventKind::Start(Tag::FootnoteDefinition(bad())),
        EventKind::Code {
            value: bad(),
            language: None,
        },
        EventKind::Code {
            value: bad(),
            language: Some(Text::Literal("rust")),
        },
        EventKind::Code {
            value: Text::Literal("x"),
            language: Some(bad()),
        },
        EventKind::CodeBlock {
            value: Text::Literal("x"),
            language: Some(bad()),
            fence: None,
            unclosed: false,
        },
        EventKind::Definition {
            label: bad(),
            destination: Text::Literal("x"),
            title: None,
        },
        EventKind::Definition {
            label: Text::Literal("x"),
            destination: bad(),
            title: None,
        },
        EventKind::Definition {
            label: Text::Literal("x"),
            destination: Text::Literal("x"),
            title: Some(bad()),
        },
        EventKind::Start(Tag::Heading(7)),
    ];
    let kinds = kinds
        .into_iter()
        .chain(INVALID_LANGUAGES.iter().map(|&language| code(language)));
    let options = Options::default();
    let mut ast = Document::parse("x", &options.parse).unwrap();
    let leaf = ast.events().count();
    assert!(leaf > 0);
    let paragraph = ast.node(ast.root()).unwrap().first_child().unwrap();
    let text = ast.node(paragraph).unwrap().first_child().unwrap();
    for kind in kinds {
        assert!(ast.set_kind(text, kind.clone()).is_err());
        assert!(render_events(
            "x",
            document(vec![kind]),
            &options.compile,
            &mut String::new()
        )
        .is_err());
    }
    for span in [2..2, std::ops::Range { start: 1, end: 0 }] {
        let input = vec![Ok(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span,
        })];
        assert!(render_events("x", input, &options.compile, &mut String::new()).is_err());
    }
    let input = vec![
        Ok(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span: 0..1,
        }),
        event(EventKind::End(TagKind::Document)),
    ];
    assert!(matches!(
        render_events("x", input, &options.compile, &mut String::new()),
        Err(StreamError::InvalidRange)
    ));
}

/// Inline code languages the prefix grammar of `inline_code_info` rejects (#138).
const INVALID_LANGUAGES: [&str; 7] = ["a\nb", "a`b", "", "-a", "a b", "a:b", "é"];
const VALID_LANGUAGES: [&str; 5] = ["rust", "c++", "c#", "objective-c", "a.b_c"];

fn code(language: &'static str) -> EventKind<'static> {
    EventKind::Code {
        value: Text::Literal("v"),
        language: Some(Text::Literal(language)),
    }
}

fn paragraph(kind: EventKind<'static>) -> Vec<Result<Event<'static>, StreamError>> {
    document(vec![
        EventKind::Start(Tag::Paragraph { tight: false }),
        kind,
        EventKind::End(TagKind::Paragraph),
    ])
}

fn build(source: &str, kind: EventKind<'static>) -> Result<(), StreamError> {
    let mut builder = AstBuilder::new();
    builder.consume(&mut paragraph(kind).into_iter(), usize::MAX)?;
    builder.finish(source).map(drop)
}

/// An inline code language that would not read back as one is rejected wherever an event enters
/// a document or renderer, whether its text is a literal, an owned value or a source range (#138).
#[test]
fn inline_code_languages_outside_the_prefix_grammar_are_rejected() {
    let options = Options::default();
    let mut ast = Document::parse("x", &options.parse).unwrap();
    let paragraph_id = ast.node(ast.root()).unwrap().first_child().unwrap();
    let text = ast.node(paragraph_id).unwrap().first_child().unwrap();
    let invalid = |result: Result<(), StreamError>| {
        assert!(
            matches!(result, Err(StreamError::InvalidValue)),
            "{:?}",
            result
        );
    };
    for language in INVALID_LANGUAGES {
        let owned = EventKind::Code {
            value: Text::Literal("v"),
            language: Some(Text::from(String::from(language))),
        };
        for kind in [code(language), owned] {
            invalid(ast.set_kind(text, kind.clone()));
            invalid(ast.append_child(paragraph_id, kind.clone()).map(drop));
            invalid(render_events(
                "x",
                paragraph(kind.clone()),
                &options.compile,
                &mut String::new(),
            ));
            // A literal or owned language is checked as it is consumed.
            let mut builder = AstBuilder::new();
            invalid(
                builder
                    .consume(&mut paragraph(kind).into_iter(), usize::MAX)
                    .map(drop),
            );
        }
        // A source-backed one is checked once the source is known.
        let source = format!("{language}:`v`");
        let kind = EventKind::Code {
            value: Text::Literal("v"),
            language: Some(Text::Source(0..language.len())),
        };
        invalid(build(&source, kind.clone()));
        invalid(render_events(
            &source,
            paragraph(kind),
            &options.compile,
            &mut String::new(),
        ));
    }
    assert_eq!(StreamError::InvalidValue.to_string(), "invalid event value");
    for language in VALID_LANGUAGES {
        ast.set_kind(text, code(language)).unwrap();
        ast.append_child(paragraph_id, code(language)).unwrap();
        let mut html = String::new();
        render_events("x", paragraph(code(language)), &options.compile, &mut html).unwrap();
        assert_eq!(
            html,
            format!("<p><code class=\"language-{language}\">v</code></p>")
        );
        let source = format!("{language}:`v`");
        let kind = EventKind::Code {
            value: Text::Literal("v"),
            language: Some(Text::Source(0..language.len())),
        };
        build(&source, kind).unwrap();
    }
}

fn block(language: Option<Text<'static>>) -> EventKind<'static> {
    EventKind::CodeBlock {
        value: Text::Literal("v\n"),
        language,
        fence: None,
        unclosed: false,
    }
}

/// A code block language that cannot be written is rejected wherever an event enters a document or
/// renderer (#144): an empty one, which the parser reads as no language. Every other code language
/// is written with whitespace, line endings, backticks and a `~` at its start encoded, so it is
/// accepted.
#[test]
fn code_block_languages_that_cannot_be_written_are_rejected() {
    let options = Options::default();
    let source = "x\n\n```\n```\n";
    let mut ast = Document::parse(source, &options.parse).unwrap();
    let root = ast.root();
    let code = ast
        .descendants(root)
        .find(|&id| matches!(ast.node(id).unwrap().kind, EventKind::CodeBlock { .. }))
        .unwrap();
    let check = |ast: &mut Document<&str>, kind: EventKind<'static>, valid: bool| {
        let results = [
            ast.set_kind(code, kind.clone()),
            ast.append_child(root, kind.clone()).map(drop),
            render_events(
                source,
                document(vec![kind.clone()]),
                &options.compile,
                &mut String::new(),
            ),
            AstBuilder::new()
                .consume(&mut document(vec![kind.clone()]).into_iter(), usize::MAX)
                .map(drop),
        ];
        for result in results {
            if valid {
                assert!(result.is_ok(), "{:?}: {:?}", kind, result);
            } else {
                assert!(
                    matches!(result, Err(StreamError::InvalidValue)),
                    "{:?}: {:?}",
                    kind,
                    result
                );
            }
        }
    };
    for empty in [
        Text::Literal(""),
        Text::from(String::new()),
        Text::Source(1..1),
    ] {
        check(&mut ast, block(Some(empty)), false);
    }
    for language in [
        "x", "~x", "`x", "a b", " x\t", "a\nb", "\r", "&amp;", "\\", "math",
    ] {
        check(&mut ast, block(Some(Text::Literal(language))), true);
    }
    check(&mut ast, block(Some(Text::Source(0..1))), true);
    // Out-of-range text is reported as such, in the language or the value.
    let out_of_range = EventKind::CodeBlock {
        value: Text::Source(0..99),
        language: Some(Text::Literal("x")),
        fence: None,
        unclosed: false,
    };
    for kind in [block(Some(Text::Source(0..99))), out_of_range] {
        assert!(matches!(
            ast.set_kind(code, kind),
            Err(StreamError::InvalidRange)
        ));
    }
    check(&mut ast, block(None), true);
}

#[test]
fn builder_failures_are_permanent_and_completion_does_not_pull_again() {
    let cases = vec![
        vec![],
        vec![event(EventKind::Text(Text::Literal("x")))],
        vec![event(EventKind::Start(Tag::Document(None)))],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::Start(Tag::Document(None))),
        ],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::End(TagKind::Paragraph)),
        ],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::End(TagKind::Document)),
            event(EventKind::Start(Tag::Document(None))),
        ],
        vec![Ok(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span: std::ops::Range { start: 1, end: 0 },
        })],
        vec![Err(StreamError::InvalidRange)],
    ];
    for input in cases {
        let mut builder = AstBuilder::new();
        assert!(builder.consume(&mut input.into_iter(), usize::MAX).is_err());
        assert!(builder.consume(&mut std::iter::empty(), 0).is_err());
        assert!(builder.finish("").is_err());
    }
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut document(vec![]).into_iter(), usize::MAX)
        .unwrap();
    let mut forbidden = std::iter::from_fn(|| -> Option<Result<Event<'_>, StreamError>> {
        panic!("completed builder pulled again")
    });
    assert_eq!(
        builder.consume(&mut forbidden, 1).unwrap(),
        BuildProgress::Complete
    );
    assert_eq!(builder.finish("").unwrap().nodes().len(), 1);
}

#[test]
fn builder_links_siblings_across_nested_and_empty_containers() {
    let input = document(vec![
        EventKind::Start(Tag::BlockQuote),
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::Text(Text::Literal("a")),
        EventKind::Start(Tag::Strong),
        EventKind::Text(Text::Literal("b")),
        EventKind::End(TagKind::Strong),
        EventKind::Text(Text::Literal("c")),
        EventKind::End(TagKind::Paragraph),
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::End(TagKind::Paragraph),
        EventKind::End(TagKind::BlockQuote),
        EventKind::Rule,
        EventKind::Start(Tag::Paragraph { tight: false }),
        EventKind::Text(Text::Literal("tail")),
        EventKind::End(TagKind::Paragraph),
    ]);
    let input: Vec<_> = input.into_iter().map(Result::unwrap).collect();
    for budget in [1, 2, 3, 7, usize::MAX] {
        let mut builder = AstBuilder::new();
        let mut events = input.iter().cloned().map(Ok);
        while builder.consume(&mut events, budget).unwrap() == BuildProgress::Pending {}
        let ast = builder.finish("").unwrap();
        assert_eq!(
            ast.events().map(Result::unwrap).collect::<Vec<_>>(),
            input,
            "budget {budget}"
        );
        // Every child in each sibling chain keeps its own parent, including after closing nested and empty containers.
        let mut parents = vec![ast.root()];
        let mut visited = 0;
        while let Some(parent) = parents.pop() {
            visited += 1;
            let mut child = ast.node(parent).unwrap().first_child();
            while let Some(id) = child {
                let node = ast.node(id).unwrap();
                assert_eq!(node.parent(), Some(parent));
                parents.push(id);
                child = node.next_sibling();
            }
        }
        assert_eq!(visited, ast.nodes().len());
    }
}

#[test]
fn edits_detach_every_supported_leaf_and_preserve_arena_links() {
    use std::borrow::Cow;
    let kinds = vec![
        EventKind::Text(Text::Source(0..1)),
        EventKind::SoftBreak(Text::Literal("\n")),
        EventKind::Code {
            value: Text::Value(Cow::Borrowed("x")),
            language: None,
        },
        EventKind::CodeBlock {
            value: Text::Literal("x"),
            language: None,
            fence: None,
            unclosed: false,
        },
        EventKind::Html {
            value: Text::Literal("x"),
            block: false,
        },
        EventKind::Opaque {
            value: Text::Literal("x"),
            block: false,
        },
    ];
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut document(kinds).into_iter(), usize::MAX)
        .unwrap();
    let mut ast = builder.finish("x").unwrap();
    let root = ast.root();
    assert!(ast.node(root).unwrap().parent().is_none());
    assert!(ast.text_mut(root).is_err());
    let mut id = ast.node(root).unwrap().first_child();
    while let Some(current) = id {
        let node = ast.node(current).unwrap();
        assert_eq!(node.parent(), Some(root));
        id = node.next_sibling();
        ast.text_mut(current).unwrap().push('!');
        ast.text_mut(current).unwrap().push('?');
    }
    for event in ast.events() {
        if let EventKind::Text(value)
        | EventKind::SoftBreak(value)
        | EventKind::Code { value, .. }
        | EventKind::CodeBlock { value, .. }
        | EventKind::Html { value, .. }
        | EventKind::Opaque { value, .. } = event.unwrap().kind
        {
            assert!(value.resolve(ast.source()).unwrap().ends_with("!?"));
        }
    }
    // IDs from another arena are checked before indexing.
    let other = Document::parse("a *b* c", &Options::default().parse).unwrap();
    let mut id = other.node(other.root()).unwrap().first_child().unwrap();
    id = other.node(id).unwrap().first_child().unwrap();
    while let Some(next) = other.node(id).unwrap().next_sibling() {
        id = next;
    }
    let mut tiny = Document::parse("", &Options::default().parse).unwrap();
    assert!(tiny.node(id).is_none());
    assert!(tiny.text_mut(id).is_err());
    assert!(tiny.set_kind(id, EventKind::Rule).is_err());
}

#[test]
fn writer_failures_propagate_from_every_output_position() {
    use std::fmt::{self, Write};
    struct Writer {
        remaining: usize,
        writes: usize,
    }
    impl Write for Writer {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            if self.remaining == 0 {
                return Err(fmt::Error);
            }
            self.remaining -= 1;
            self.writes += 1;
            Ok(())
        }
    }
    let cases = [
        "# A &amp; `b` <i>c</i> ![d](x \"t\")\n\n> - *a* **b** ~~c~~\n\n42. next\n\n---",
        "| a | b | c | d |\n| :- | -: | :-: | - |\n| & | < | > | x |",
        "[a](x%20y/?a=b&c=d \"t&\\\"\") ![a *b*](x \"t&\")\n\n- [x] a\n- [ ] b\n\nline  \nbreak\\\nend",
        "ruby:`<&>` $x&y$\n\n```rust\n<&>\n```\n\n```math\nx&y\n```\n\n<script>x\0</script>",
        "Note[^x], again[^x], code[^y].\n\n[^x]: a &amp; b\n\n[^y]:\n    ```\n    a\n    ```",
        "1. ordinary\n\n9. numbered",
        "a [x](xé \"&\")\n\n```\nraw",
    ];
    for (heading_ids, checkable) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut options = Options::gfm();
        options.parse.inline_code_info = true;
        options.compile.heading_ids = heading_ids;
        options.compile.task_list_item_checkable = checkable;
        options.compile.allow_dangerous_html = true;
        for source in cases {
            let render = |writer: &mut Writer| {
                render_events(
                    source,
                    base_markdown::EventParser::new(source, &options.parse),
                    &options.compile,
                    writer,
                )
            };
            let mut writer = Writer {
                remaining: usize::MAX,
                writes: 0,
            };
            render(&mut writer).unwrap();
            for remaining in 0..writer.writes {
                assert!(
                    matches!(
                        render(&mut Writer {
                            remaining,
                            writes: 0
                        }),
                        Err(StreamError::Write(_))
                    ),
                    "{:?}, write {}",
                    source,
                    remaining
                );
            }
        }
    }
    let error = render_events(
        "",
        vec![Err(StreamError::InvalidRange)],
        &Options::default().compile,
        &mut String::new(),
    )
    .unwrap_err();
    assert!(matches!(error, StreamError::InvalidRange));
    let missing = document(vec![EventKind::FootnoteReference(Text::Literal("missing"))]);
    assert!(matches!(
        render_events("", missing, &Options::default().compile, &mut String::new()),
        Err(StreamError::Unbalanced)
    ));
}

#[test]
fn heading_ids_use_normalized_semantic_code_text() {
    let mut options = Options::default();
    options.compile.heading_ids = true;
    for (source, expected) in [
        ("`\nb `\n---", "<h2 id=\"b\"><code>b</code></h2>"),
        ("a`\nb `\n---", "<h2 id=\"ab\">a<code>b</code></h2>"),
    ] {
        assert_eq!(
            base_markdown::to_html_with_options(source, &options),
            expected
        );
        let ast = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(source, ast.events(), &options.compile, &mut output).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn document_completion_yields_one_terminal_separator_and_end() {
    let options = Options::default();
    for (source, expected_separator) in [
        ("", None),
        ("a", None),
        ("a\n", Some(("\n", false))),
        ("a\r\n", Some(("\n", false))),
        ("a\r\n\r\nb\n", Some(("\n", false))),
        ("- a\n\n", Some(("", true))),
        (">\n", Some(("", true))),
        ("a\n\n[x]: /a\n", None),
        ("```\ncode\n", None),
        ("> ```\n> code\n", Some(("\n", false))),
        ("    code\n ", None),
        ("<!--\nbody\n", None),
        ("> <!--\n> body\n", None),
        ("<!-- closed -->\r\n", Some(("\n", false))),
    ] {
        let mut parser = base_markdown::EventParser::new(source, &options.parse);
        assert!(matches!(
            parser.next().unwrap().unwrap().kind,
            EventKind::Start(Tag::Document(_))
        ));
        let mut terminal_separator = None;
        loop {
            let event = parser.next().expect("document end event").unwrap();
            if matches!(event.kind, EventKind::End(TagKind::Document)) {
                assert_eq!(event.span, source.len()..source.len());
                break;
            }
            terminal_separator = match event.kind {
                EventKind::Separator { value, conditional } => {
                    Some((value.resolve(source).unwrap().to_owned(), conditional))
                }
                _ => None,
            };
        }
        assert_eq!(
            terminal_separator
                .as_ref()
                .map(|(value, conditional)| (value.as_str(), *conditional)),
            expected_separator,
            "{source:?}"
        );
        assert!(parser.next().is_none(), "{:?}", source);
        assert!(parser.next().is_none(), "{:?}", source);
    }
}

#[test]
fn paragraph_tightness_follows_its_own_list() {
    let options = Options::default();
    for (source, expected) in [
        ("text", vec![false]),
        ("- a\n  > b\n  - c\n- d", vec![true, false, true, true]),
        (
            "- a\n\n  > b\n\n  - c\n- d",
            vec![false, false, true, false],
        ),
        ("- a\n\n  b\n- c", vec![false, false, false]),
    ] {
        let actual: Vec<_> = base_markdown::EventParser::new(source, &options.parse)
            .filter_map(|event| match event.unwrap().kind {
                EventKind::Start(Tag::Paragraph { tight }) => Some(tight),
                _ => None,
            })
            .collect();
        assert_eq!(actual, expected, "{source:?}");
    }
}

#[test]
fn mixed_list_kinds_after_unclosed_fences_are_siblings() {
    let output = base_markdown::to_html("1. ~~~\n-");
    assert_eq!(
        output,
        "<ol>\n<li>\n<pre><code></code></pre>\n</li>\n</ol>\n<ul>\n<li></li>\n</ul>"
    );
}

#[test]
fn capture_nesting_is_checked_during_construction_rendering_and_edits() {
    let options = Options::gfm();
    for parent in [
        Tag::Heading(1),
        Tag::Image {
            destination: Text::Literal("x"),
            title: None,
        },
    ] {
        for child in [Tag::Heading(2), Tag::FootnoteDefinition(Text::Literal("x"))] {
            // Directly inside the parent, and inside a non-capturing container in it.
            for wrap in [false, true] {
                let mut body = vec![EventKind::Start(parent.clone())];
                if wrap {
                    body.push(EventKind::Start(Tag::Emphasis));
                }
                body.extend([
                    EventKind::Start(child.clone()),
                    EventKind::Text(Text::Literal("text")),
                    EventKind::End(child.kind()),
                ]);
                if wrap {
                    body.push(EventKind::End(TagKind::Emphasis));
                }
                body.push(EventKind::End(parent.kind()));
                let input = document(body);
                let mut builder = AstBuilder::new();
                assert!(builder
                    .consume(
                        &mut input.iter().map(|e| Ok(e.as_ref().unwrap().clone())),
                        usize::MAX
                    )
                    .is_err());
                assert!(render_events("", input, &options.compile, &mut String::new()).is_err());
            }
        }
    }
    let mut ast = Document::parse("> # h", &options.parse).unwrap();
    let quote = ast.node(ast.root()).unwrap().first_child().unwrap();
    assert!(ast
        .set_kind(quote, EventKind::Start(Tag::Heading(1)))
        .is_err());
    let heading = ast.node(quote).unwrap().first_child().unwrap();
    let text = ast.node(heading).unwrap().first_child().unwrap();
    assert!(ast
        .set_kind(text, EventKind::Start(Tag::Heading(2)))
        .is_err());
    // Editing one subtree must not treat its following sibling as a descendant.
    let mut ast = Document::parse("> a\n\n# h", &options.parse).unwrap();
    let quote = ast.node(ast.root()).unwrap().first_child().unwrap();
    ast.set_kind(
        quote,
        EventKind::Start(Tag::Image {
            destination: Text::Literal("x"),
            title: None,
        }),
    )
    .unwrap();
    let mut output = String::new();
    render_events(ast.source(), ast.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, "<img src=\"x\" alt=\"a\" />\n<h1>h</h1>");
}

#[test]
fn custom_block_events_inside_images_contribute_only_alt_text() {
    let input = document(vec![
        EventKind::Start(Tag::Image {
            destination: Text::Literal("x"),
            title: None,
        }),
        EventKind::Start(Tag::Table),
        EventKind::Start(Tag::TableHead),
        EventKind::Start(Tag::TableRow),
        EventKind::Start(Tag::TableCell {
            alignment: base_markdown::AlignKind::Left,
            header: true,
        }),
        EventKind::Text(Text::Literal("<&")),
        EventKind::End(TagKind::TableCell),
        EventKind::End(TagKind::TableRow),
        EventKind::End(TagKind::TableHead),
        EventKind::Start(Tag::TableBody),
        EventKind::End(TagKind::TableBody),
        EventKind::End(TagKind::Table),
        EventKind::CodeBlock {
            value: Text::Literal("code\""),
            language: None,
            fence: None,
            unclosed: false,
        },
        EventKind::Rule,
        EventKind::HardBreak,
        EventKind::TaskMarker(false),
        EventKind::End(TagKind::Image),
    ]);
    let options = Options::default();
    let mut output = String::new();
    render_events("", input, &options.compile, &mut output).unwrap();
    assert_eq!(output, "<img src=\"x\" alt=\"&lt;&amp;code&quot;\" />");
}

#[test]
fn extensions_and_unclosed_container_fences_keep_their_boundaries() {
    let mut options = Options::gfm();
    options.parse.inline_code_info = true;
    options.parse.constructs.frontmatter = true;
    for source in [
        "> ```\r> a",
        "> ```\r\n> a",
        "\\ruby:`x`",
        "a\\-ruby:`x`",
        // A valid raw prefix can cross emphasis; its start is outside the final Data token.
        // Preserve the formatted text rather than creating a backwards source range.
        "a-_b_:`x`",
        "---\nx\n---\n\n>",
        "+++\nx\n+++\n\n>",
    ] {
        let output = base_markdown::to_html_with_options(source, &options);
        let ast = Document::parse(source, &options.parse).unwrap();
        let mut actual = String::new();
        render_events(source, ast.events(), &options.compile, &mut actual).unwrap();
        assert_eq!(actual, output, "{source:?}");
    }
}

#[test]
fn synthetic_consumers_terminate_blocks_and_propagate_write_errors() {
    use std::fmt;
    struct Writer(usize);
    impl fmt::Write for Writer {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            if self.0 == 0 {
                return Err(fmt::Error);
            }
            self.0 -= 1;
            Ok(())
        }
    }
    let text = || EventKind::Text(Text::Literal("x"));
    let mut body = vec![text()];
    for tag in [
        Tag::Heading(2),
        Tag::BlockQuote,
        Tag::List {
            start: None,
            tight: false,
        },
        Tag::Item { block_end: true },
        Tag::Table,
        Tag::TableHead,
    ] {
        let kind = tag.kind();
        body.push(EventKind::Start(tag));
        body.push(text());
        body.push(EventKind::End(kind));
        body.push(text());
    }
    body.extend([
        EventKind::CodeBlock {
            value: Text::Literal("x"),
            language: None,
            fence: None,
            unclosed: false,
        },
        text(),
        EventKind::Code {
            value: Text::Literal("x"),
            language: None,
        },
        text(),
        EventKind::Html {
            value: Text::Literal("<&>"),
            block: true,
        },
        EventKind::Start(Tag::Heading(1)),
        EventKind::SoftBreak(Text::Literal("\n")),
        EventKind::Code {
            value: Text::Literal("x"),
            language: None,
        },
        EventKind::HardBreak,
        EventKind::End(TagKind::Heading),
        EventKind::Start(Tag::FootnoteDefinition(Text::Literal("X"))),
        text(),
        EventKind::End(TagKind::FootnoteDefinition),
        EventKind::FootnoteReference(Text::Literal("X")),
    ]);
    let mut options = Options::default();
    options.compile.heading_ids = true;
    let input = document(body);
    let mut writer = Writer(usize::MAX);
    render_events(
        "",
        input.iter().map(|e| Ok(e.as_ref().unwrap().clone())),
        &options.compile,
        &mut writer,
    )
    .unwrap();
    let writes = usize::MAX - writer.0;
    for remaining in 0..writes {
        assert!(matches!(
            render_events(
                "",
                input.iter().map(|e| Ok(e.as_ref().unwrap().clone())),
                &options.compile,
                &mut Writer(remaining)
            ),
            Err(StreamError::Write(_))
        ));
    }
    // Every ordinary container in an image contributes only its textual children.
    let mut body = vec![EventKind::Start(Tag::Image {
        destination: Text::Literal("x"),
        title: None,
    })];
    for tag in [
        Tag::BlockQuote,
        Tag::List {
            start: None,
            tight: true,
        },
        Tag::Item { block_end: false },
        Tag::Emphasis,
        Tag::Strong,
        Tag::Delete,
    ] {
        let kind = tag.kind();
        body.extend([EventKind::Start(tag), text(), EventKind::End(kind)]);
    }
    body.extend([
        EventKind::Code {
            value: Text::Literal("m"),
            language: None,
        },
        EventKind::End(TagKind::Image),
    ]);
    let mut output = String::new();
    render_events("", document(body), &options.compile, &mut output).unwrap();
    assert_eq!(output, "<img src=\"x\" alt=\"xxxxxxm\" />");
    assert!(render_events(
        "",
        vec![event(EventKind::Start(Tag::Paragraph { tight: false }))],
        &options.compile,
        &mut String::new()
    )
    .is_err());
}

#[test]
fn source_detachment_checks_buffers_with_interior_mutability() {
    use std::{cell::Cell, rc::Rc};
    struct Source(Rc<Cell<bool>>);
    impl AsRef<str> for Source {
        fn as_ref(&self) -> &str {
            if self.0.get() {
                ""
            } else {
                "abc"
            }
        }
    }
    let changed = Rc::new(Cell::new(false));
    let mut ast = Document::parse(Source(changed.clone()), &Options::default().parse).unwrap();
    let paragraph = ast.node(ast.root()).unwrap().first_child().unwrap();
    let text = ast.node(paragraph).unwrap().first_child().unwrap();
    changed.set(true);
    assert!(matches!(ast.text_mut(text), Err(StreamError::InvalidRange)));
}

#[test]
fn reference_labels_keep_significant_collapsed_spaces() {
    let options = Options::default();
    let source = "[a b] [ab]\n\n[a b]: /spaced\n[ab]: /compact";
    let expected = "<p><a href=\"/spaced\">a b</a> <a href=\"/compact\">ab</a></p>\n";
    assert_eq!(base_markdown::to_html(source), expected);
    let ast = Document::parse(source, &options.parse).unwrap();
    let mut output = String::new();
    render_events(source, ast.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn individual_event_delivery_matches_whole_stream_rendering() {
    use base_markdown::{EventHtmlRenderer, EventParser};
    let source = "# Incremental *events*\n\nA [link](url).";
    let options = Options::default();
    let mut output = String::new();
    let mut renderer = EventHtmlRenderer::new(source, &options.compile, &mut output);
    for event in EventParser::new(source, &options.parse) {
        renderer.event(event.unwrap()).unwrap();
    }
    assert_eq!(output, base_markdown::to_html(source));
    let mut output = String::new();
    let mut renderer = EventHtmlRenderer::new(source, &options.compile, &mut output);
    assert!(renderer
        .event(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span: 0..source.len() + 1,
        })
        .is_err());
    assert!(output.is_empty());
}

#[test]
fn deferred_image_titles_retain_values_until_the_image_closes() {
    use std::borrow::Cow;
    let source = "<&\"\0é";
    let options = Options::default();
    for title in [
        Text::Source(0..source.len()),
        Text::Literal("<&\"\0é"),
        Text::from(source.to_string()),
    ] {
        let body = vec![
            EventKind::Start(Tag::Image {
                destination: Text::Literal("image"),
                title: Some(title),
            }),
            EventKind::Text(Text::Literal("outer ")),
            EventKind::Start(Tag::Image {
                destination: Text::Literal("nested"),
                title: Some(Text::Literal("ignored")),
            }),
            EventKind::Text(Text::Literal("inner")),
            EventKind::End(TagKind::Image),
            EventKind::End(TagKind::Image),
        ];
        let mut output = String::new();
        render_events(source, document(body), &options.compile, &mut output).unwrap();
        assert_eq!(
            output,
            "<img src=\"image\" alt=\"outer inner\" title=\"&lt;&amp;&quot;�é\" />"
        );
    }

    let mut output = String::new();
    let mut renderer = base_markdown::EventHtmlRenderer::new("", &options.compile, &mut output);
    renderer
        .event(event(EventKind::Start(Tag::Document(None))).unwrap())
        .unwrap();
    {
        // A custom producer may discard the borrowed title immediately after this call.
        let title = String::from("temporary & title");
        renderer
            .event(Event {
                span: 0..0,
                kind: EventKind::Start(Tag::Image {
                    destination: Text::Literal("image"),
                    title: Some(Text::Value(Cow::Borrowed(&title))),
                }),
            })
            .unwrap();
    }
    renderer
        .event(event(EventKind::End(TagKind::Image)).unwrap())
        .unwrap();
    renderer
        .event(event(EventKind::End(TagKind::Document)).unwrap())
        .unwrap();
    assert_eq!(
        output,
        "<img src=\"image\" alt=\"\" title=\"temporary &amp; title\" />"
    );
}

#[test]
fn a_mismatched_end_still_closes_a_capturing_frame() {
    // The renderer keeps accepting events after an error, so the popped heading must stop
    // capturing.
    let options = Options::gfm();
    let mut output = String::new();
    let mut renderer = EventHtmlRenderer::new("", &options.compile, &mut output);
    renderer
        .event(event(EventKind::Start(Tag::Document(None))).unwrap())
        .unwrap();
    renderer
        .event(event(EventKind::Start(Tag::Heading(1))).unwrap())
        .unwrap();
    assert!(renderer
        .event(event(EventKind::End(TagKind::Paragraph)).unwrap())
        .is_err());
    renderer
        .event(
            event(EventKind::Start(Tag::FootnoteDefinition(Text::Literal(
                "x",
            ))))
            .unwrap(),
        )
        .unwrap();
}

fn definition(label: Text<'static>) -> EventKind<'static> {
    EventKind::Definition {
        label,
        destination: Text::Literal("/u"),
        title: None,
    }
}

/// A definition label longer than the parser reads would be written as text, so it is rejected
/// wherever an event enters a document or renderer, counting the escapes it is written with but
/// not its line endings or the indentation after them (#146).
#[test]
fn definition_labels_longer_than_the_parser_reads_are_rejected() {
    let options = Options::default();
    let mut ast = Document::parse("[r]: /u", &options.parse).unwrap();
    let id = ast.node(ast.root()).unwrap().first_child().unwrap();
    let a = |count: usize| "a".repeat(count);
    let render = |source: &str, kind: EventKind<'static>| {
        render_events(
            source,
            document(vec![kind]),
            &options.compile,
            &mut String::new(),
        )
    };
    let fits = [
        a(999),
        format!("{}\n   \t{}", a(500), a(499)),
        format!("{}\\]", a(997)),
        format!("{}\\", a(997)),
    ];
    let long = [
        a(1000),
        format!("{}]", a(998)),
        format!("{}\\", a(998)),
        format!("{}\\\\\\", a(996)),
        format!("{}\n{}", a(500), a(500)),
    ];
    let mut check = |label: &String, valid: bool| {
        let kind = definition(Text::from(label.clone()));
        let root = ast.root();
        let results = [
            ast.set_kind(id, kind.clone()),
            ast.append_child(root, kind.clone()).map(drop),
            render("", kind.clone()),
            AstBuilder::new()
                .consume(&mut document(vec![kind]).into_iter(), usize::MAX)
                .map(drop),
            // A source-backed label is checked once the source is known.
            {
                let mut builder = AstBuilder::new();
                builder
                    .consume(
                        &mut document(vec![definition(Text::Source(0..label.len()))]).into_iter(),
                        usize::MAX,
                    )
                    .and_then(|_| builder.finish(label).map(drop))
            },
            render(label, definition(Text::Source(0..label.len()))),
        ];
        for result in results {
            if valid {
                assert!(result.is_ok(), "{:?}: {:?}", label, result);
            } else {
                assert!(
                    matches!(result, Err(StreamError::InvalidValue)),
                    "{:?}: {:?}",
                    label,
                    result
                );
            }
        }
    };
    for label in &fits {
        check(label, true);
    }
    for label in &long {
        check(label, false);
    }
}
