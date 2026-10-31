// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Attention sequence selection and encoding, ported from the `attention`, `roundtrip attention` and `roundtrip underscores` tests of `mdast-util-to-markdown`.

mod support;
use base_markdown::{markdown::Options, Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{
    canonical, to_markdown as to, to_markdown_with_options as to_md_with_opts, Blockquote, Break,
    Delete, Emphasis, Node, Paragraph, Root, Strong, Text,
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

fn html(value: &str) -> String {
    base_markdown::to_html_with_options(value, &base_markdown::Options::gfm())
}

/// Serialize phrasing in a paragraph and check that parsing the result gives the same tree.
fn serialize(children: Vec<Node>, options: &Options) -> String {
    let tree = paragraph(children);
    let markdown = to_md_with_opts(&tree, options).unwrap();
    let parsed = Document::parse(&markdown, &ParseOptions::gfm()).unwrap();
    assert_eq!(
        canonical(&parsed),
        canonical(&tree.document()),
        "{markdown:?} should parse to the serialized tree"
    );
    markdown
}

/// Parse and serialize Markdown, checking the rendered HTML is unchanged.
fn roundtrip(source: &str, options: &Options) -> String {
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    let markdown = base_markdown::markdown::to_markdown_with_options(&document, options).unwrap();
    assert_eq!(
        html(&markdown).trim_end(),
        html(source).trim_end(),
        "{source:?} serialized as {markdown:?} should render the same"
    );
    markdown
}

#[test]
fn adjacent_attention_from_markdown() {
    for (source, expected) in [
        ("*a*_b_", "*a*_b_\n"),
        ("*a*_b_*c*", "*a*_b_*c*\n"),
        ("**a**__b__", "**a**__b__\n"),
        ("_a_*b*", "*a*_b_\n"),
        ("__a__**b**", "**a**__b__\n"),
        ("*a*__b__", "*a***b**\n"),
        ("**a***b*", "**a***b*\n"),
        ("[*a*](b)*c*", "[*a*](b)*c*\n"),
        ("> *a*_b_\n> - **c**__d__", "> *a*_b_\n>\n> - **c**__d__\n"),
        ("# *a*_b_", "# *a*_b_\n"),
        ("| *a*_b_ |\n| - |", "| *a*_b_ |\n| --- |\n"),
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
        "_a_*b*\n",
        "should keep the configured marker first"
    );
    assert_eq!(
        roundtrip("**a**__b__", &markers('_', '_')),
        "__a__**b**\n",
        "should keep the configured strong marker first"
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
        "*a*_b_\n",
        "should use the other marker for emphasis next to emphasis"
    );

    assert_eq!(
        to(&emphasis(vec![emphasis(text_a())])).unwrap(),
        "*_a_*\n",
        "should use the other marker for emphasis in emphasis"
    );

    assert_eq!(
        serialize(
            vec![strong(text_a()), strong(text_b())],
            &Options::default()
        ),
        "**a**__b__\n",
        "should use the other marker for strong next to strong"
    );

    assert_eq!(
        to(&strong(vec![emphasis(text_a())])).unwrap(),
        "**_a_**\n",
        "should use the other marker for emphasis in strong"
    );

    assert_eq!(
        to(&emphasis(vec![strong(text_a())])).unwrap(),
        "***a***\n",
        "should use the same marker for strong in emphasis, as that forms"
    );

    assert_eq!(
        to_md_with_opts(&emphasis(vec![strong(text_a())]), &markers('_', '_')).unwrap(),
        "___a___\n",
        "should use the same marker for strong in emphasis w/ `_`"
    );

    assert_eq!(
        serialize(
            vec![emphasis(text_a()), emphasis(text_b())],
            &markers('_', '*')
        ),
        "_a_*b*\n",
        "should keep the configured marker first"
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
        "*a*_b_*c*_d_\n",
        "should use other markers for several nodes"
    );

    assert_eq!(
        serialize(
            vec![strong(vec![text(".")]), strong(vec![text("*")])],
            &Options::default()
        ),
        "**.**__\\*__\n",
        "should use the other marker for strong if the rule of 3 would pair it wrongly"
    );

    assert_eq!(
        serialize(
            vec![strong(text_a()), emphasis(text_b())],
            &markers('_', '_')
        ),
        "__a__*b*\n",
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
        "**___.a__*b*_**\n",
        "should use other markers if sequences would pair partially"
    );

    let mut nested = emphasis(vec![text("x")]);
    for _ in 0..5 {
        nested = emphasis(vec![nested]);
    }
    assert_eq!(
        to(&nested).unwrap(),
        "******x******\n",
        "should keep the markers if other markers do not help"
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
        "a~~b~~c *d*_e_\n",
        "should support strikethrough inside words"
    );

    assert_eq!(
        to(&paragraph(vec![delete(text_a()), delete(text_b())])).unwrap(),
        "~~a~~~~b~~\n",
        "should keep the preferred sequences if nothing pairs, as adjacent tildes form one run"
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
            vec![text("a"), emphasis(vec![text("b c")]), text("d")],
            &markers('_', '*')
        ),
        "&#x61;_&#x62; &#x63;_&#x64;\n",
        "should encode letters around emphasis using `_` between letters"
    );

    // Empty attention cannot be represented, so these do not round-trip.
    assert_eq!(
        to_md_with_opts(
            &paragraph(vec![text("a"), emphasis(Vec::new()), text("b")]),
            &markers('_', '*')
        )
        .unwrap(),
        "&#x61;__&#x62;\n",
        "should not encode characters in empty emphasis"
    );

    assert_eq!(
        to_md_with_opts(
            &paragraph(vec![text("a"), strong(Vec::new()), text("b")]),
            &markers('*', '_')
        )
        .unwrap(),
        "&#x61;____&#x62;\n",
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
        serialize(
            vec![text("\\a"), emphasis(vec![text("!")])],
            &Options::default()
        ),
        "\\\\&#x61;*!*\n",
        "should escape a backslash before a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![text("d"), emphasis(vec![text("a_bc")])],
            &markers('_', '*')
        ),
        "&#x64;_&#x61;\\_bc_\n",
        "should escape `_` after a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![emphasis(vec![text("ab_c")]), text("d")],
            &markers('_', '*')
        ),
        "_ab\\_&#x63;_&#x64;\n",
        "should escape `_` before a letter that is encoded for attention"
    );

    assert_eq!(
        serialize(
            vec![text("a__b"), emphasis(vec![text("x")])],
            &markers('_', '*')
        ),
        "a\\_\\_&#x62;_&#x78;_\n",
        "should escape an underscore run before a letter encoded for attention"
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
        ("x*a_b*", "&#x78;_&#x61;\\_b_\n"),
        ("x*ab_c*d", "&#x78;_&#x61;b\\_&#x63;_&#x64;\n"),
        ("a_b*x*", "a\\_&#x62;_&#x78;_\n"),
        ("a\\_b*x*", "a\\_&#x62;_&#x78;_\n"),
        ("a\\\\_b*x*", "a\\\\\\_&#x62;_&#x78;_\n"),
    ] {
        assert_eq!(
            roundtrip(source, &markers('_', '*')),
            expected,
            "{source:?}"
        );
    }
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

    for source in ["~\\~~\"a@b.c<www.a.b*", "~~b~~~c", "~a\\~~", "a\\~~~b~~"] {
        roundtrip(source, &Options::default());
    }
}

#[test]
fn encode_literal_domain() {
    // Escaping the underscore before a reference would end the domain early and link it.
    for (source, expected) in [
        ("**www.a.bx_b*~b*", "\\*\\*www.a.bx_&#x62;_~b_\n"),
        ("**http://a.bx_b*~b*", "\\*\\*http://a.bx_&#x62;_~b_\n"),
        ("**https://a.bx_b*~b*", "\\*\\*https://a.bx_&#x62;_~b_\n"),
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

    for value in [
        "(____",
        "Once activated, a service worker ______, then transitions to idle…",
    ] {
        let once = roundtrip(value, &Options::default());
        assert_eq!(roundtrip(&once, &Options::default()), once, "{value:?}");
    }
}
