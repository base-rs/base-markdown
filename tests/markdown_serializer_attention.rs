// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection and encoding, ported from the `attention`, `roundtrip attention` and `roundtrip underscores` tests of `mdast-util-to-markdown`.

mod support;
use base_markdown::{markdown::Options, Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{
    canonical, to_markdown as to, to_markdown_with_options as to_md_with_opts, Blockquote, Break,
    Delete, Emphasis, InlineCode, Node, Paragraph, Root, Strong, Text,
};

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn emphasis(children: Vec<Node>) -> Node {
    Node::Emphasis(Emphasis {
        children,
        position: None,
    })
}

fn strong(children: Vec<Node>) -> Node {
    Node::Strong(Strong {
        children,
        position: None,
    })
}

fn delete(children: Vec<Node>) -> Node {
    Node::Delete(Delete {
        children,
        position: None,
    })
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Root(Root {
        children: vec![Node::Paragraph(Paragraph {
            children,
            position: None,
        })],
        position: None,
    })
}

fn markers(emphasis: char, strong: char) -> Options {
    Options {
        emphasis,
        strong,
        ..Default::default()
    }
}

/// Render HTML, counting adjacent attention of the same kind as one node, as the serializer writes it.
fn html(value: &str) -> String {
    let mut html = base_markdown::to_html_with_options(value, &base_markdown::Options::gfm())
        .trim_end()
        .to_string();
    while let Some(merged) = ["</em><em>", "</strong><strong>", "</del><del>"]
        .iter()
        .find(|boundary| html.contains(*boundary))
    {
        html = html.replace(merged, "");
    }
    html
}

/// The canonical tree, counting adjacent attention of the same kind as one node.
fn merged<S: AsRef<str>>(document: &Document<S>) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    for entry in canonical(document) {
        let boundary = output
            .last()
            .and_then(|last| last.strip_prefix("End(")?.strip_suffix(')'));
        if boundary == Some(entry.as_str()) {
            output.pop();
            continue;
        }
        let joined = output.last().and_then(|last| {
            let left = last.strip_prefix("text:\"")?.strip_suffix('"')?;
            let right = entry.strip_prefix("text:\"")?.strip_suffix('"')?;
            Some(format!("text:\"{left}{right}\""))
        });
        if let Some(joined) = joined {
            *output.last_mut().unwrap() = joined;
        } else {
            output.push(entry);
        }
    }
    output
}

fn to_markdown<S: AsRef<str>>(document: &Document<S>, options: &Options) -> String {
    base_markdown::markdown::to_markdown_with_options(document, options).unwrap()
}

/// Check that serializing `markdown` again changes nothing.
fn assert_stable(markdown: &str, options: &Options) {
    let parsed = Document::parse(markdown, &ParseOptions::gfm()).unwrap();
    assert_eq!(
        to_markdown(&parsed, options),
        markdown,
        "{markdown:?} should serialize to itself"
    );
}

/// Serialize phrasing in a paragraph and check that parsing the result gives the same tree.
fn serialize(children: Vec<Node>, options: &Options) -> String {
    let tree = paragraph(children);
    let markdown = to_md_with_opts(&tree, options).unwrap();
    let parsed = Document::parse(&markdown, &ParseOptions::gfm()).unwrap();
    assert_eq!(
        merged(&parsed),
        merged(&tree.document()),
        "{markdown:?} should parse to the serialized tree"
    );
    assert_stable(&markdown, options);
    markdown
}

/// Parse and serialize Markdown, checking the rendered HTML is unchanged.
fn roundtrip(source: &str, options: &Options) -> String {
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    let markdown = to_markdown(&document, options);
    assert_eq!(
        html(&markdown),
        html(source),
        "{source:?} serialized as {markdown:?} should render the same"
    );
    assert_stable(&markdown, options);
    markdown
}

#[test]
fn adjacent_attention_from_markdown() {
    for (source, expected) in [
        ("*a*_b_", "*ab*\n"),
        ("*a*_b_*c*", "*abc*\n"),
        ("**a**__b__", "**ab**\n"),
        ("_a_*b*", "*ab*\n"),
        ("__a__**b**", "**ab**\n"),
        ("*a*__b__", "*a*__b__\n"),
        ("**a***b*", "**a**_b_\n"),
        ("*a*_b_**c**", "*ab*__c__\n"),
        ("*a*_**b**_", "*a**b***\n"),
        ("[*a*](b)*c*", "[*a*](b)*c*\n"),
        ("> *a*_b_\n> - **c**__d__", "> *ab*\n>\n> - **cd**\n"),
        ("# *a*_b_", "# *ab*\n"),
        ("| *a*_b_ |\n| - |", "| *ab* |\n| --- |\n"),
        ("~~a~~ ~b~", "~~a~~ ~~b~~\n"),
    ] {
        assert_eq!(
            roundtrip(source, &Options::default()),
            expected,
            "{source:?}"
        );
    }

    assert_eq!(
        roundtrip("*a*_b_", &markers('_', '_')),
        "_ab_\n",
        "should merge with the configured marker"
    );
    assert_eq!(
        roundtrip("*a*__b__", &markers('_', '_')),
        "_a_**b**\n",
        "should keep the configured marker first"
    );
}

#[test]
fn merge_attention() {
    assert_eq!(
        serialize(
            vec![
                emphasis(vec![emphasis(vec![text("a")])]),
                emphasis(vec![emphasis(vec![text("b")])]),
            ],
            &Options::default()
        ),
        "*_ab_*\n",
        "should merge the children of merged attention too"
    );

    assert_eq!(
        to(&paragraph(vec![
            emphasis(vec![text("a")]),
            text(""),
            emphasis(vec![text("b")]),
        ]))
        .unwrap(),
        "*a*_b_\n",
        "should not merge attention separated by other nodes"
    );
}

#[test]
fn join_opening() {
    for (children, options, expected) in [
        (
            vec![emphasis(vec![emphasis(vec![text("a")]), text(" b")])],
            markers('*', '*'),
            "**a* b*\n",
        ),
        (
            vec![emphasis(vec![emphasis(vec![text("a")]), text(".")])],
            markers('*', '*'),
            "**a*.*\n",
        ),
        // The closing sequence could also open, so the rule of 3 keeps it from the run of 2 its opening sequence joins.
        (
            vec![emphasis(vec![emphasis(vec![text("a")]), text("b")])],
            markers('*', '*'),
            "*_&#x61;_&#x62;*\n",
        ),
        (
            vec![emphasis(vec![emphasis(vec![text(".")]), text("b")])],
            markers('*', '*'),
            "*_._&#x62;*\n",
        ),
        (
            vec![emphasis(vec![emphasis(vec![text(".")]), text(".")])],
            markers('_', '_'),
            "_*.*._\n",
        ),
        // Emphasis starting the inner emphasis joins the run too, so the closing sequences pair.
        (
            vec![emphasis(vec![
                emphasis(vec![emphasis(vec![text("[")]), text("(")]),
                text("-"),
            ])],
            markers('*', '*'),
            "***\\[*(*-*\n",
        ),
        (
            vec![strong(vec![strong(vec![text("a")]), text(" b")])],
            markers('*', '*'),
            "****a** b**\n",
        ),
    ] {
        assert_eq!(serialize(children, &options), expected);
    }

    assert_eq!(
        serialize(
            vec![emphasis(vec![
                Node::Break(Break { position: None }),
                text("a")
            ])],
            &Options::default()
        ),
        "*\\\na*\n",
        "should count a hard break starting attention as punctuation"
    );

    // Attention cannot end with a line ending, so this does not round-trip.
    assert_eq!(
        to(&paragraph(vec![emphasis(vec![
            text("a"),
            Node::Break(Break { position: None })
        ])]))
        .unwrap(),
        "*a\\\n*\n",
        "should count a hard break ending attention as whitespace"
    );
}

#[test]
fn attention() {
    let text_a = || vec![text("a")];
    let text_b = || vec![text("b")];

    assert_eq!(
        serialize(
            vec![emphasis(text_a()), emphasis(text_b())],
            &Options::default()
        ),
        "*ab*\n",
        "should merge emphasis next to emphasis"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![emphasis(text_a())])],
            &Options::default()
        ),
        "*_a_*\n",
        "should use the other marker for emphasis in emphasis"
    );

    assert_eq!(
        serialize(
            vec![strong(text_a()), strong(text_b())],
            &Options::default()
        ),
        "**ab**\n",
        "should merge strong next to strong"
    );

    assert_eq!(
        serialize(vec![strong(vec![emphasis(text_a())])], &Options::default()),
        "**_a_**\n",
        "should use the other marker for emphasis in strong"
    );

    assert_eq!(
        serialize(vec![emphasis(vec![strong(text_a())])], &Options::default()),
        "***a***\n",
        "should use the same marker for strong in emphasis, as that forms"
    );

    assert_eq!(
        serialize(vec![emphasis(vec![strong(text_a())])], &markers('_', '_')),
        "___a___\n",
        "should use the same marker for strong in emphasis w/ `_`"
    );

    assert_eq!(
        serialize(vec![strong(vec![strong(text_a())])], &Options::default()),
        "****a****\n",
        "should use the same marker for strong in strong, as that forms"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![emphasis(text_a()), text(" b")])],
            &Options::default()
        ),
        "**a* b*\n",
        "should join the opening sequence of enclosing attention if the closing sequences are apart"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![emphasis(text_a()), text("b")])],
            &Options::default()
        ),
        "*_&#x61;_&#x62;*\n",
        "should not join the opening sequence of enclosing attention if the rule of 3 keeps the runs apart"
    );

    assert_eq!(
        serialize(
            vec![emphasis(text_a()), emphasis(text_b())],
            &markers('_', '*')
        ),
        "_ab_\n",
        "should merge with the configured marker"
    );

    assert_eq!(
        serialize(
            vec![
                emphasis(text_a()),
                emphasis(text_b()),
                emphasis(vec![text("c")]),
                emphasis(vec![text("d")]),
            ],
            &Options::default()
        ),
        "*abcd*\n",
        "should merge several nodes"
    );

    assert_eq!(
        serialize(
            vec![strong(text_a()), emphasis(text_b())],
            &Options::default()
        ),
        "**a**_b_\n",
        "should use the other marker for emphasis after strong"
    );

    assert_eq!(
        serialize(
            vec![emphasis(text_a()), strong(text_b())],
            &markers('_', '_')
        ),
        "_a_**b**\n",
        "should use the other marker for strong after emphasis"
    );

    assert_eq!(
        serialize(
            vec![text("a"), emphasis(text_b()), text("c")],
            &markers('_', '_')
        ),
        "a*b*c\n",
        "should use the other marker if underscores cannot form between words"
    );

    assert_eq!(
        serialize(
            vec![strong(vec![emphasis(vec![
                strong(vec![text(".a")]),
                emphasis(text_b()),
            ])])],
            &markers('_', '_')
        ),
        "__***.a**_b_*__\n",
        "should use other markers if sequences would pair partially"
    );

    let mut nested = emphasis(vec![text("x")]);
    for _ in 0..5 {
        nested = emphasis(vec![nested]);
    }
    // Emphasis nested this deeply with nothing between cannot be written by looking at neighbours only, so this does not round-trip.
    assert_eq!(
        to(&nested).unwrap(),
        "*_***_x_***_*\n",
        "should keep the preferred marker if neither marker forms"
    );

    assert_eq!(
        serialize(
            vec![
                text("a "),
                delete(vec![text("b "), delete(vec![text("c")]), text(" d")]),
                text(" e"),
            ],
            &Options::default()
        ),
        "a ~~b ~~c~~ d~~ e\n",
        "should support strikethrough"
    );

    assert_eq!(
        serialize(
            vec![
                text("a"),
                delete(text_b()),
                text("c "),
                emphasis(vec![text("d")]),
                emphasis(vec![text("e")]),
            ],
            &Options::default()
        ),
        "a~~b~~c *de*\n",
        "should support strikethrough inside words"
    );

    assert_eq!(
        serialize(
            vec![delete(text_a()), delete(text_b())],
            &Options::default()
        ),
        "~~ab~~\n",
        "should merge strikethrough next to strikethrough"
    );
}

#[test]
fn encode() {
    assert_eq!(
        serialize(
            vec![text("a"), emphasis(vec![text("💡b")])],
            &markers('_', '*')
        ),
        "&#x61;_💡b_\n",
        "should keep astral characters whole when encoding next to emphasis"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![text("b💡")]), text("a")],
            &markers('_', '*')
        ),
        "_b💡_&#x61;\n",
        "should keep astral characters whole when encoding after emphasis"
    );

    assert_eq!(
        serialize(
            vec![text("a"), emphasis(vec![text(" b ")]), text("c")],
            &markers('_', '*')
        ),
        "&#x61;_&#x20;b&#x20;_&#x63;\n",
        "should encode around emphasis if neither marker forms otherwise"
    );

    assert_eq!(
        serialize(
            vec![text("a"), emphasis(vec![text("b c")]), text("d")],
            &markers('_', '*')
        ),
        "a*b c*d\n",
        "should use the other marker if it needs nothing encoded"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![
                text("a"),
                emphasis(vec![text("b c")]),
                text("d")
            ])],
            &Options::default()
        ),
        "*&#x61;_&#x62; &#x63;_&#x64;*\n",
        "should encode letters around emphasis using `_` between letters"
    );

    // Empty attention cannot be represented, so these do not round-trip.
    assert_eq!(
        to_md_with_opts(
            &paragraph(vec![text("a"), emphasis(Vec::new()), text("b")]),
            &markers('_', '*')
        )
        .unwrap(),
        "a**b\n",
        "should not encode characters in empty emphasis"
    );

    assert_eq!(
        to_md_with_opts(
            &paragraph(vec![text("a"), strong(Vec::new()), text("b")]),
            &markers('*', '_')
        )
        .unwrap(),
        "a****b\n",
        "should not encode characters in empty strong"
    );

    assert_eq!(
        serialize(
            vec![
                Node::Break(Break { position: None }),
                emphasis(vec![text(" x")])
            ],
            &markers('_', '*')
        ),
        "\\\n_&#x20;x_\n",
        "should encode after the opening of emphasis, not a line ending before it"
    );

    assert_eq!(
        serialize(
            vec![text("a"), text(""), strong(vec![text(" b")])],
            &Options::default()
        ),
        "&#x61;**&#x20;b**\n",
        "should encode the character before strong ignoring empty text"
    );

    assert_eq!(
        serialize(vec![strong(vec![text(" a ")])], &Options::default()),
        "**&#x20;a&#x20;**\n",
        "should encode whitespace inside strong"
    );

    assert_eq!(
        serialize(vec![emphasis(vec![text(" ")])], &Options::default()),
        "*&#x20;*\n",
        "should encode a character that starts and ends attention once"
    );

    assert_eq!(
        serialize(
            vec![text("\\a"), emphasis(vec![text("!")])],
            &Options::default()
        ),
        "\\\\&#x61;*!*\n",
        "should escape a backslash before a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![text("d"), emphasis(vec![text("a_bc")])])],
            &Options::default()
        ),
        "*&#x64;_&#x61;\\_bc_*\n",
        "should escape `_` after a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![emphasis(vec![text("ab_c")]), text("d")])],
            &Options::default()
        ),
        "*_ab\\_&#x63;_&#x64;*\n",
        "should escape `_` before a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![text("a__b"), emphasis(vec![text("x")])])],
            &Options::default()
        ),
        "*a\\_\\_&#x62;_&#x78;_*\n",
        "should escape an underscore run before a letter encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![text("a\\_b"), emphasis(vec![text("x")])])],
            &Options::default()
        ),
        "*a\\\\\\_&#x62;_&#x78;_*\n",
        "should not escape an escaped underscore before a letter encoded for attention again"
    );

    assert_eq!(
        to(&Node::Blockquote(Blockquote {
            children: vec![emphasis(vec![text("*")])],
            position: None
        }))
        .unwrap(),
        "> *\\**\n",
        "should serialize emphasis outside phrasing"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Blockquote(Blockquote {
                children: vec![delete(Vec::new())],
                position: None
            }),
            &Options::default()
        )
        .unwrap(),
        "> ~~~~\n",
        "should serialize empty strikethrough"
    );

    for options in [markers('x', '*'), markers('*', 'x')] {
        assert!(
            to_md_with_opts(
                &Node::Blockquote(Blockquote {
                    children: vec![emphasis(vec![strong(vec![text("a")])])],
                    position: None
                }),
                &options
            )
            .is_err(),
            "should fail on an invalid marker outside phrasing"
        );
    }
}

#[test]
fn encode_split_text() {
    // Unmatched delimiter runs split text nodes, so escaping when text is written cannot see the attention boundary.
    for (source, expected) in [
        ("_x*a_b*_", "*&#x78;_&#x61;\\_b_*\n"),
        ("_x*ab_c*d_", "*&#x78;_&#x61;b\\_&#x63;_&#x64;*\n"),
        ("_a_b*x*_", "*a\\_&#x62;_&#x78;_*\n"),
        ("_a\\_b*x*_", "*a\\_&#x62;_&#x78;_*\n"),
    ] {
        assert_eq!(
            roundtrip(source, &Options::default()),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn encode_after_empty_text() {
    // Empty text writes nothing, so the character attention is encoded against is that of the next node that writes something.
    fn inline_code(value: &str) -> Node {
        Node::InlineCode(InlineCode {
            value: value.into(),
            position: None,
            lang: None,
        })
    }
    type Wrap = fn(Vec<Node>) -> Node;
    let kinds: [(Wrap, &str); 3] = [(emphasis, "*"), (strong, "**"), (delete, "~~")];
    for (wrap, marker) in kinds {
        for (after, expected) in [
            (
                vec![text(""), text("b")],
                format!("{marker}a&#x20;{marker}&#x62;\n"),
            ),
            (
                vec![text(""), text(""), text(""), text("b")],
                format!("{marker}a&#x20;{marker}&#x62;\n"),
            ),
            (
                vec![text(""), inline_code("b")],
                format!("{marker}a&#x20;{marker}`b`\n"),
            ),
            (
                vec![text(""), text("")],
                format!("{marker}a&#x20;{marker}\n"),
            ),
        ] {
            let mut children = vec![wrap(vec![text("a ")])];
            children.extend(after);
            let markdown = to_md_with_opts(&paragraph(children), &Options::default()).unwrap();
            assert_eq!(markdown, expected, "{marker:?}");
            assert_stable(&markdown, &Options::default());
        }
    }
}

#[test]
fn empty_text_around_attention_stays_linear() {
    // Each empty text is skipped once when looking for what follows a node, so four times the input takes about four times as long.
    use base_markdown::{
        stream::{Event, EventKind, Tag, TagKind, Text},
        AstBuilder,
    };
    let time = |count: usize| {
        let event = |kind: EventKind<'static>| Ok(Event { kind, span: 0..0 });
        let empty = || event(EventKind::Text(Text::Literal("")));
        let mut events = vec![
            event(EventKind::Start(Tag::Document(None))),
            event(EventKind::Start(Tag::Paragraph { tight: false })),
        ];
        events.extend((0..count).map(|_| empty()));
        events.extend([
            event(EventKind::Start(Tag::Emphasis)),
            event(EventKind::Text(Text::Literal("a "))),
            event(EventKind::End(TagKind::Emphasis)),
        ]);
        events.extend((0..count).map(|_| empty()));
        events.extend([
            event(EventKind::Text(Text::Literal("b"))),
            event(EventKind::End(TagKind::Paragraph)),
            event(EventKind::End(TagKind::Document)),
        ]);
        let mut builder = AstBuilder::new();
        builder
            .consume(&mut events.into_iter(), usize::MAX)
            .unwrap();
        let document = builder.finish(String::new()).unwrap();
        let started = std::time::Instant::now();
        let markdown = to_markdown(&document, &Options::default());
        let elapsed = started.elapsed();
        assert_eq!(markdown, "*a&#x20;*&#x62;\n");
        elapsed
    };
    time(1 << 10);
    let small = time(25_000);
    let large = time(100_000);
    assert!(
        large < small * 16,
        "4x input took {:?} against {:?}",
        large,
        small
    );
}

#[test]
fn encode_after_cleared_text() {
    // Clearing the text a reference was split off keeps the reference for the text after it.
    let mut document = Document::parse(
        "see *old&#x20;*&#x6E;ew text\n".to_owned(),
        &ParseOptions::gfm(),
    )
    .unwrap();
    let cleared = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                &document.node(id).unwrap().kind,
                base_markdown::stream::EventKind::Text(value)
                    if value.resolve(document.source()).ok() == Some("n")
            )
        })
        .unwrap();
    document.text_mut(cleared).unwrap().clear();
    let markdown = to_markdown(&document, &Options::default());
    assert_eq!(markdown, "see *old&#x20;*&#x65;w text\n");
    assert_stable(&markdown, &Options::default());
}

#[test]
fn encode_tilde() {
    for (children, expected) in [
        (vec![delete(vec![text("~a~")])], "~~\\~a\\~~~\n"),
        (vec![text("a~"), delete(vec![text("b")])], "a\\~~~b~~\n"),
        (vec![delete(vec![text("a")]), text("~b")], "~~a~~\\~b\n"),
        (vec![delete(vec![text("a\\~")])], "~~a\\\\\\~~~\n"),
    ] {
        assert_eq!(serialize(children, &Options::default()), expected);
    }

    for (children, expected) in [
        (vec![emphasis(vec![text("~a~")])], "*\\~a\\~*\n"),
        (vec![delete(vec![text("~~a~~")])], "~~\\~\\~a\\~\\~~~\n"),
        (vec![delete(vec![text("a\\~~")])], "~~a\\\\\\~\\~~~\n"),
    ] {
        assert_eq!(serialize(children, &Options::default()), expected);
    }

    for source in [
        "~\\~~\"a@b.c<www.a.b*",
        "~~b~~~c",
        "~a\\~~",
        "a\\~~~b~~",
        "*\\~a\\~*",
        "**\\~w!\\~**",
        "__\"a~1-~~__~~",
        "`__:_*\\\"~:~~_💡",
    ] {
        roundtrip(source, &Options::default());
    }
}

#[test]
fn encode_literal_domain() {
    // Escaping the underscore before a reference would end the domain early and link it.
    for (source, expected) in [
        ("**www.a.bx_b*~b*", "\\*\\*www.a.bx_&#x62;_\\~b_\n"),
        ("**http://a.bx_b*~b*", "\\*\\*http://a.bx_&#x62;_\\~b_\n"),
        ("**https://a.bx_b*~b*", "\\*\\*https://a.bx_&#x62;_\\~b_\n"),
        ("*www\\.a\\_1*", "_www.a_1_\n"),
        ("*http\\://www\\.a\\_b*", "_http://www.a_b_\n"),
    ] {
        assert_eq!(
            roundtrip(source, &markers('_', '*')),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn roundtrip_attention() {
    for kind in ["emphasis", "strong"] {
        for marker in ['*', '_'] {
            for open in [true, false] {
                for inside in [".", " ", "a"] {
                    for outside in [".", " ", "a"] {
                        let value = if open {
                            format!("{inside}y")
                        } else {
                            format!("y{inside}")
                        };
                        let attention = if kind == "emphasis" {
                            emphasis(vec![text(&value)])
                        } else {
                            strong(vec![text(&value)])
                        };
                        let children = if open {
                            vec![text(&format!("x{outside}")), attention]
                        } else {
                            vec![attention, text(&format!("{outside}z"))]
                        };
                        serialize(children, &markers(marker, marker));
                    }
                }
            }
        }
    }
}

#[test]
fn roundtrip_underscores() {
    for place in ["text", "after", "before", "start", "end"] {
        for kind in ["emphasis", "strong"] {
            for marker in ['*', '_'] {
                // Attention does not matter for underscores in plain text.
                if place == "text" && (kind != "emphasis" || marker != '*') {
                    continue;
                }
                for before in [".", " ", "a"] {
                    for after in [".", " ", "a"] {
                        let value = format!(
                            "{}{before}_{after}{}",
                            if matches!(place, "after" | "start") {
                                ""
                            } else {
                                "x"
                            },
                            if matches!(place, "before" | "end") {
                                ""
                            } else {
                                "y"
                            }
                        );
                        let attention = |children| {
                            if kind == "emphasis" {
                                emphasis(children)
                            } else {
                                strong(children)
                            }
                        };
                        let mut children = Vec::new();
                        if place == "after" {
                            children.push(attention(vec![text("z")]));
                        }
                        if matches!(place, "start" | "end") {
                            children.push(attention(vec![text(&value)]));
                        } else {
                            children.push(text(&value));
                        }
                        if place == "before" {
                            children.push(attention(vec![text("z")]));
                        }
                        serialize(children, &markers(marker, marker));
                    }
                }
            }
        }
    }
}

#[test]
fn roundtrip_documents() {
    assert_eq!(
        roundtrip("&#x1F4A1;**&#x20;a&#x20;**&#x1F4A1;\n", &Options::default()),
        "💡**&#x20;a&#x20;**💡\n",
        "should roundtrip astral characters around attention"
    );

    for marker in ["*", "_"] {
        let value = [
            "Separate paragraphs:",
            "",
            "a * is this emphasis? *",
            "",
            "a ** is this emphasis? **",
            "",
            "a *** is this emphasis? ***",
            "",
            "a *\\* is this emphasis? *\\*",
            "",
            "a \\** is this emphasis? \\**",
            "",
            "a **\\* is this emphasis? **\\*",
            "",
            "a *\\** is this emphasis? *\\**",
            "",
            "One paragraph:",
            "",
            "a * is this emphasis? *",
            "a ** is this emphasis? **",
            "a *** is this emphasis? ***",
            "a *\\* is this emphasis? *\\*",
            "a \\** is this emphasis? \\**",
            "a **\\* is this emphasis? **\\*",
            "a *\\** is this emphasis? *\\**",
        ]
        .join("\n")
        .replace('*', marker);
        let document = Document::parse(&value, &ParseOptions::gfm()).unwrap();
        let markdown = base_markdown::markdown::to_markdown(&document).unwrap();
        assert_eq!(
            canonical(&Document::parse(&markdown, &ParseOptions::gfm()).unwrap()),
            canonical(&document),
            "should roundtrip `{marker}` (tree)"
        );
        roundtrip(&value, &Options::default());
    }

    assert_eq!(
        roundtrip("***x**__y__*", &Options::default()),
        "***xy***\n",
        "should merge strong in emphasis"
    );

    for value in [
        "**x_a\\@b.**",
        "**é\\_a\\@b.\\éwww\\.a:\\_`a`**",
        "(____",
        "Once activated, a service worker ______, then transitions to idle…",
    ] {
        let once = roundtrip(value, &Options::default());
        assert_eq!(roundtrip(&once, &Options::default()), once, "{value:?}");
    }
}
