use core_markdown::{render_events, AstBuilder, BuildProgress, Document, EventParser, Options};

#[test]
fn parser_and_document_use_the_same_html_consumer() {
    let cases = [
        "",
        "hello",
        "hello\n",
        "# Heading\n\nText *emphasis* and **bold**.\n",
        "Title\n=====\n",
        "> quote\n>\n> second\n",
        "1. First\n   - nested\n2. Second\n",
        "- a\n\n- b\n",
        "-\n",
        "a [link](https://example.com?q=x&n=2 \"title\") and ![alt *text*](a.png).\n",
        "[link][x]\n\n[x]: /path \"title\"\n",
        "<https://example.com> <me@example.com>\n",
        "a &amp; &#x1f600; \\*escaped\\*\n",
        "`code` and `` a ` b `` and `a\nb`\n",
        "```rust\nfn main() {}\n```\n",
        "    code\n    two\n",
        "<div>\nhello\n</div>\n",
        "line  \nbreak\\\nagain\n",
        "---\n",
        "| a | b |\n| :- | -: |\n| c | d |\n",
        "- [x] yes\n- [ ] no\n",
        "~~old~~ and www.example.com\n",
        "Note[^x].\n\n[^x]: Body.\n",
        "a\r\nb\r\n",
        "# hello\n\n***\n",
    ];
    for options in [Options::default(), Options::gfm()] {
        for source in cases {
            let expected = core_markdown::to_html_with_options(source, &options).unwrap();
            let mut actual = String::new();
            render_events(
                source,
                EventParser::new(source, &options.parse).unwrap(),
                &options.compile,
                &mut actual,
            )
            .unwrap();
            assert_eq!(actual, expected, "source: {source:?}");
            let document = Document::parse(source.to_string(), &options.parse).unwrap();
            let mut roundtrip = String::new();
            render_events(
                document.source(),
                document.events(),
                &options.compile,
                &mut roundtrip,
            )
            .unwrap();
            assert_eq!(roundtrip, actual, "AST roundtrip: {source:?}");
        }
    }
}

#[test]
fn construction_pulls_only_its_budget_and_detaches_edits() {
    let source = String::from("one **two** three");
    let options = Options::default();
    let mut builder = AstBuilder::with_capacity(16, 4);
    {
        let mut events = EventParser::new(&source, &options.parse).unwrap();
        assert_eq!(
            builder.consume(&mut events, 0).unwrap(),
            BuildProgress::Pending
        );
        assert!(builder.is_empty());
        assert_eq!(
            builder.consume(&mut events, 1).unwrap(),
            BuildProgress::Pending
        );
        assert_eq!(builder.len(), 1);
        while builder.consume(&mut events, 2).unwrap() == BuildProgress::Pending {}
    }
    let mut document = builder.finish(source).unwrap();
    let paragraph = document
        .node(document.root())
        .unwrap()
        .first_child()
        .unwrap();
    let text = document.node(paragraph).unwrap().first_child().unwrap();
    document.text_mut(text).unwrap().push_str("edited ");
    assert_eq!(document.source(), "one **two** three");
    let mut output = String::new();
    render_events(
        document.source(),
        document.events(),
        &options.compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, "<p>one edited <strong>two</strong> three</p>");
}

#[test]
fn extended_metadata_and_rendering_options_roundtrip() {
    let mut options = Options::gfm();
    options.parse.inline_code_info = true;
    options.parse.constructs.frontmatter = true;
    options.parse.constructs.math_flow = true;
    options.parse.constructs.math_text = true;
    options.compile.heading_ids = true;
    options.compile.default_line_ending = core_markdown::LineEnding::CarriageReturnLineFeed;
    options.compile.task_list_item_checkable = true;
    options.compile.footnote_clobber_prefix = Some("prefix-".into());
    options.compile.footnote_back_label = Some("Back & return".into());
    options.compile.footnote_label = Some("Notes & more".into());
    options.compile.footnote_label_attributes = Some("class=\"notes\"".into());
    options.compile.footnote_label_tag_name = Some("h3".into());
    for source in [
        "---yaml\nx: y\n---\n# Same\n\n# Same\n\nUse ruby:`puts 1`, $x$ and [note][a].\n\n[a]: /path \"title\"\n\n$$\nx^2\n$$",
        "# Image ![a *b*](url \"title\") and <b>c</b>\n",
        "[a][x] and [x]\n\n[x]: /one\n[x]: /two\n",
        "Note[^x], again[^x].\n\n[^x]: First.\n\n    > Second.\n",
        "[^x]: Definition before call.\n\nNote[^x].\n",
        "| a | b |\n| :- | -: |\n| c |\n| d | e | f |\n",
        "`  `, ` a `, `a\r\nb`, ``` a `` b ```\n",
        "> ```\n>\n>\n>\n\na", "```", "<script>x</script>\n",
    ] {
        let expected = core_markdown::to_html_with_options(source, &options).unwrap();
        let document = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(document.source(), document.events(), &options.compile, &mut output).unwrap();
        assert_eq!(output, expected, "{source:?}");
    }
    let options = Options {
        parse: core_markdown::ParseOptions::default(),
        ..Options::default()
    };
    for source in [
        "<Thing a=\"b\">Hello *there*</Thing>",
        "# Hi {name}\n\n{value}\n",
        "<>\n\n# Heading\n\n</>\n",
    ] {
        let expected = core_markdown::to_html_with_options(source, &options).unwrap();
        let document = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(
            document.source(),
            document.events(),
            &options.compile,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn consumers_reject_broken_streams_and_propagate_writer_errors() {
    use core_markdown::stream::{Event, EventKind, StreamError, Tag, TagKind, Text};
    fn event(kind: EventKind<'static>) -> Result<Event<'static>, StreamError> {
        Ok(Event { kind, span: 0..0 })
    }
    let options = Options::default();
    for input in [
        vec![],
        vec![event(EventKind::Text(Text::Literal("x")))],
        vec![event(EventKind::End(TagKind::Document))],
        vec![event(EventKind::Start(Tag::Document(None)))],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::End(TagKind::Paragraph)),
        ],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::End(TagKind::Document)),
            event(EventKind::Text(Text::Literal("x"))),
        ],
        vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::Start(Tag::Document(None))),
        ],
    ] {
        let mut output = String::new();
        assert!(render_events("", input, &options.compile, &mut output).is_err());
    }
    let mut builder = AstBuilder::new();
    assert!(builder.finish("").is_err());
    builder = AstBuilder::new();
    let mut malformed = vec![event(EventKind::End(TagKind::Paragraph))].into_iter();
    assert!(builder.consume(&mut malformed, 1).is_err());
    assert!(builder.consume(&mut malformed, 1).is_err());
    assert!(builder.finish("").is_err());
    let mut builder = AstBuilder::new();
    let mut malformed = vec![
        event(EventKind::Start(Tag::Document(None))),
        event(EventKind::Text(Text::Source(0..1))),
        event(EventKind::End(TagKind::Document)),
    ]
    .into_iter();
    builder.consume(&mut malformed, usize::MAX).unwrap();
    assert!(matches!(builder.finish(""), Err(StreamError::InvalidRange)));
    assert!(Text::Source(1..2).resolve("é").is_err());
    assert_eq!(Text::Literal("literal").resolve("").unwrap(), "literal");
    assert_eq!(
        Text::from("owned".to_string()).resolve("").unwrap(),
        "owned"
    );
    assert_eq!(
        StreamError::Unbalanced.to_string(),
        "unbalanced semantic event stream"
    );
    assert_eq!(
        StreamError::InvalidRange.to_string(),
        "invalid source range"
    );
    struct Failing;
    impl std::fmt::Write for Failing {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let error = render_events(
        "x",
        EventParser::new("x", &options.parse).unwrap(),
        &options.compile,
        &mut Failing,
    )
    .unwrap_err();
    assert!(matches!(error, StreamError::Write(_)));
    assert!(!error.to_string().is_empty());
}

#[test]
fn edits_and_consumers_validate_the_same_contract() {
    use core_markdown::stream::{Event, EventKind, Tag, TagKind, Text};
    let options = Options::default();
    let mut document = Document::parse("abc", &options.parse).unwrap();
    let root = document.root();
    let paragraph = document.node(root).unwrap().first_child().unwrap();
    let text = document.node(paragraph).unwrap().first_child().unwrap();
    for (id, kind) in [
        (text, EventKind::Text(Text::Source(999..1000))),
        (paragraph, EventKind::Text(Text::Literal("hidden"))),
        (root, EventKind::Start(Tag::Paragraph { tight: false })),
        (text, EventKind::Start(Tag::Document(None))),
        (text, EventKind::End(TagKind::Paragraph)),
        (paragraph, EventKind::Start(Tag::Heading(0))),
    ] {
        assert!(document.set_kind(id, kind).is_err());
    }
    document
        .set_kind(paragraph, EventKind::Start(Tag::Heading(2)))
        .unwrap();
    document
        .set_kind(text, EventKind::Text(Text::Literal("new")))
        .unwrap();
    document.text_mut(text).unwrap().push('!');
    let mut output = String::new();
    render_events(
        document.source(),
        document.events(),
        &options.compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, "<h2>new!</h2>");
    for kind in [
        EventKind::Opaque {
            value: Text::Source(1..2),
            block: true,
        },
        EventKind::Definition {
            label: Text::Literal("x"),
            destination: Text::Source(1..2),
            title: None,
        },
        EventKind::Separator {
            value: Text::Source(1..2),
            conditional: true,
        },
        EventKind::Start(Tag::Heading(0)),
    ] {
        let input = vec![
            Ok(Event {
                kind: EventKind::Start(Tag::Document(None)),
                span: 0..0,
            }),
            Ok(Event { kind, span: 0..0 }),
            Ok(Event {
                kind: EventKind::End(TagKind::Document),
                span: 0..0,
            }),
        ];
        let mut output = String::new();
        let mut builder = AstBuilder::new();
        let mut other = input.iter().map(|e| Ok(e.as_ref().unwrap().clone()));
        assert!(builder
            .consume(&mut other, usize::MAX)
            .and_then(|_| builder.finish(""))
            .is_err());
        assert!(render_events("", input, &options.compile, &mut output).is_err());
    }
    let mut builder = AstBuilder::new();
    let mut input = vec![
        Ok(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span: 0..999,
        }),
        Ok(Event {
            kind: EventKind::End(TagKind::Document),
            span: 0..0,
        }),
    ]
    .into_iter();
    assert!(builder.consume(&mut input, usize::MAX).is_err());
    let input = vec![
        Ok(Event {
            kind: EventKind::Start(Tag::Document(None)),
            span: 0..1,
        }),
        Ok(Event {
            kind: EventKind::End(TagKind::Document),
            span: 0..1,
        }),
    ];
    assert!(render_events("x", input, &options.compile, &mut String::new()).is_err());
}

#[test]
fn virtual_container_spaces_and_autolink_nulls_keep_authored_values() {
    let options = Options::gfm();
    for (source, expected) in [
        (
            "1.  \tx",
            "<ol>\n<li>\n<pre><code> x\n</code></pre>\n</li>\n</ol>",
        ),
        ("   ```\n\tx\n   ```", "<pre><code> x\n</code></pre>"),
        ("- `a\n\tb`", "<ul>\n<li><code>a b</code></li>\n</ul>"),
        ("- `\n\t`", "<ul>\n<li><code> </code></li>\n</ul>"),
        (">\t</b>", "<blockquote>\n&lt;/b&gt;\n</blockquote>"),
        (
            "https://x.org\0",
            "<p><a href=\"https://x.org%00\">https://x.org�</a></p>",
        ),
        (
            "[x](https://x.org\0)",
            "<p><a href=\"https://x.org%EF%BF%BD\">x</a></p>",
        ),
    ] {
        assert_eq!(
            core_markdown::to_html_with_options(source, &options).unwrap(),
            expected
        );
        let document = Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        render_events(
            document.source(),
            document.events(),
            &options.compile,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn unclosed_footnote_fence_does_not_consume_following_soft_break() {
    let options = Options::gfm();
    let source = "[^a]:~~~\n1. a\nb";
    let expected = "<ol>\n<li>a\nb</li>\n</ol>";
    assert_eq!(
        core_markdown::to_html_with_options(source, &options).unwrap(),
        expected
    );
    let document = Document::parse(source, &options.parse).unwrap();
    let mut output = String::new();
    render_events(source, document.events(), &options.compile, &mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn task_checkbox_attributes_preserve_checkability_and_state() {
    let source = "- [ ] open\n- [x] done";
    for (checkable, expected) in [
        (false, "<ul>\n<li><input type=\"checkbox\" disabled=\"\" /> open</li>\n<li><input type=\"checkbox\" disabled=\"\" checked=\"\" /> done</li>\n</ul>"),
        (true, "<ul>\n<li><input type=\"checkbox\" /> open</li>\n<li><input type=\"checkbox\" checked=\"\" /> done</li>\n</ul>"),
    ] {
        let mut options = Options::gfm();
        options.compile.task_list_item_checkable = checkable;
        assert_eq!(core_markdown::to_html_with_options(source, &options).unwrap(), expected);
        let mut checked = String::new();
        render_events(source, EventParser::new(source, &options.parse).unwrap(), &options.compile, &mut checked).unwrap();
        assert_eq!(checked, expected);
        let document = Document::parse(source, &options.parse).unwrap();
        let mut traversed = String::new();
        render_events(source, document.events(), &options.compile, &mut traversed).unwrap();
        assert_eq!(traversed, expected);
    }
}

#[test]
fn inline_workspace_switches_between_compact_and_grammar_regions() {
    let source = "before `a\nb` &amp; *first*.\n\n[compact][ref] **second**.\n\nraw <span>third</span>.\n\n| a | b |\n| - | - |\n| | |\n\n`x\ny` [last][ref].\n\n[ref]: /target \"title\"\n";
    for (mut options, table) in [
        (Options::default(), "<p>| a | b |\n| - | - |\n| | |</p>\n"),
        (Options::gfm(), "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td></td>\n<td></td>\n</tr>\n</tbody>\n</table>\n"),
    ] {
        options.compile.allow_dangerous_html = true;
        let expected = format!(
            "<p>before <code>a b</code> &amp; <em>first</em>.</p>\n<p><a href=\"/target\" title=\"title\">compact</a> <strong>second</strong>.</p>\n<p>raw <span>third</span>.</p>\n{table}<p><code>x y</code> <a href=\"/target\" title=\"title\">last</a>.</p>\n"
        );
        assert_eq!(core_markdown::to_html_with_options(source, &options).unwrap(), expected);
        let mut rendered = String::new();
        render_events(source, EventParser::new(source, &options.parse).unwrap(), &options.compile, &mut rendered).unwrap();
        assert_eq!(rendered, expected);
        let document = Document::parse(source, &options.parse).unwrap();
        rendered.clear();
        render_events(source, document.events(), &options.compile, &mut rendered).unwrap();
        assert_eq!(rendered, expected);
    }
}

#[test]
fn javascript_like_input_is_ordinary_markdown() {
    let cases = [
        ("{unclosed", "<p>{unclosed</p>"),
        (
            "import Component from 'component'\n\nexport {value}",
            "<p>import Component from 'component'</p>\n<p>export {value}</p>",
        ),
        (
            "text {value} *markdown*",
            "<p>text {value} <em>markdown</em></p>",
        ),
        (
            "text <Widget title=\"demo\" /> *tail*",
            "<p>text &lt;Widget title=&quot;demo&quot; /&gt; <em>tail</em></p>",
        ),
    ];
    for mut options in [Options::default(), Options::gfm()] {
        for grammar in [false, true] {
            // Disabling this unrelated construct selects grammar preparation.
            options.parse.constructs.autolink = !grammar;
            for (source, expected) in cases {
                assert_eq!(
                    core_markdown::to_html_with_options(source, &options).unwrap(),
                    expected
                );
                let document = Document::parse(source, &options.parse).unwrap();
                let mut output = String::new();
                render_events(
                    document.source(),
                    document.events(),
                    &options.compile,
                    &mut output,
                )
                .unwrap();
                assert_eq!(output, expected);
            }
        }
    }
}

#[test]
fn custom_producer_diagnostics_reach_both_consumers() {
    use core_markdown::{message::Message, stream::StreamError};
    let diagnostic = Message {
        span: Some(0..0),
        reason: "custom input failed".into(),
        rule_id: "example",
        source: "custom-producer",
    };
    let mut output = String::new();
    let error = render_events(
        "",
        [Err(StreamError::from(diagnostic.clone()))],
        &Options::default().compile,
        &mut output,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "0..0: custom input failed (custom-producer:example)"
    );
    let mut builder = AstBuilder::new();
    let mut events = core::iter::once(Err(StreamError::from(diagnostic.clone())));
    let error = builder.consume(&mut events, 1).unwrap_err();
    assert!(matches!(error, StreamError::Parse(message) if message == diagnostic));
    assert!(builder.finish("").is_err());
}
