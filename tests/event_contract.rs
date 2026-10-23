// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{
    render_events,
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    AstBuilder, BuildProgress, Document, Options,
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
        EventKind::CodeBlock {
            value: Text::Literal("x"),
            language: Some(bad()),
            fence: None,
            math: false,
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
fn edits_detach_every_supported_leaf_and_preserve_arena_links() {
    use std::borrow::Cow;
    let kinds = vec![
        EventKind::Text(Text::Source(0..1)),
        EventKind::SoftBreak(Text::Literal("\n")),
        EventKind::Math(Text::Value(Cow::Borrowed("x"))),
        EventKind::Code {
            value: Text::Literal("x"),
            language: None,
        },
        EventKind::CodeBlock {
            value: Text::Literal("x"),
            language: None,
            fence: None,
            math: false,
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
        | EventKind::Math(value)
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
        "ruby:`<&>` $x&y$\n\n```rust\n<&>\n```\n\n$$\nx&y\n$$\n\n<script>x\0</script>",
        "Note[^x], again[^x], code[^y].\n\n[^x]: a &amp; b\n\n[^y]:\n    ```\n    a\n    ```",
        "a [x](é \"&\")\n\n```\nraw",
    ];
    for heading_ids in [false, true] {
        let mut options = Options::gfm();
        options.parse.inline_code_info = true;
        options.parse.constructs.math_text = true;
        options.parse.constructs.math_flow = true;
        options.compile.heading_ids = heading_ids;
        options.compile.allow_dangerous_html = true;
        for source in cases {
            let render = |writer: &mut Writer| {
                render_events(
                    source,
                    core_markdown::EventParser::new(source, &options.parse).unwrap(),
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
            core_markdown::to_html_with_options(source, &options).unwrap(),
            expected
        );
        let ast = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(source, ast.events(), &options.compile, &mut output).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn mixed_list_kinds_close_their_semantic_opening_tags() {
    let output = core_markdown::to_html("1. ~~~\n-");
    assert_eq!(
        output,
        "<ol>\n<li>\n<pre><code>\n</code></pre>\n</li>\n<ul>\n<li></li>\n</ul>\n</ol>"
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
            let input = document(vec![
                EventKind::Start(parent.clone()),
                EventKind::Start(child.clone()),
                EventKind::Text(Text::Literal("text")),
                EventKind::End(child.kind()),
                EventKind::End(parent.kind()),
            ]);
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
            alignment: core_markdown::mdast::AlignKind::Left,
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
            math: false,
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
fn extension_errors_and_unclosed_container_fences_keep_their_boundaries() {
    let options = core_markdown::ParseOptions::mdx();
    assert!(core_markdown::EventParser::new("{b", &options).is_err());
    assert!(Document::parse("{b", &options).is_err());
    assert!(Document::parse("a {", &options).is_err());
    let mut options = Options::gfm();
    options.parse.inline_code_info = true;
    options.parse.constructs.frontmatter = true;
    for source in [
        "> ```\r> a",
        "> ```\r\n> a",
        "\\ruby:`x`",
        "---\nx\n---\n\n>",
        "+++\nx\n+++\n\n>",
    ] {
        let output = core_markdown::to_html_with_options(source, &options).unwrap();
        let ast = Document::parse(source, &options.parse).unwrap();
        let mut actual = String::new();
        render_events(source, ast.events(), &options.compile, &mut actual).unwrap();
        assert_eq!(actual, output, "{source:?}");
    }
}
