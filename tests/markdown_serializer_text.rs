// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{LineWrapping, Options};
use base_markdown::Document;
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::to_markdown_with_options as to_with_options;
use support::fixture::{
    Blockquote, Delete, Emphasis, List, ListItem, Node, Paragraph, Root, Strong, Text,
};

#[test]
fn text() {
    assert_eq!(
        to(&Node::Text(Text {
            value: String::new(),
            position: None,
        }))
        .unwrap(),
        "",
        "should support an empty text"
    );

    assert_eq!(
        to(&Node::Text(Text {
            value: String::from("a\nb"),
            position: None,
        }))
        .unwrap(),
        "a\nb\n",
        "should support text"
    );
}

#[test]
fn unwraps_soft_line_breaks_and_adjacent_indentation() {
    let node = Node::Text(Text {
        value: String::from("first  \n \tsecond\r\n third\rfourth"),
        position: None,
    });

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(output, "first second third fourth\n");
}

#[test]
fn unwrapping_preserves_hard_breaks_and_code_blocks() {
    let source = "first\nsecond  \nthird\n\n```text\ncode\nline\n```\n";
    let node = Document::parse(source, &Default::default()).unwrap();

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(
        output,
        "first second\\\nthird\n\n```text\ncode\nline\n```\n"
    );
}

#[test]
fn unwraps_soft_line_breaks_inside_emphasis() {
    let node = Document::parse("*first\nsecond*", &Default::default()).unwrap();

    let output = to_with_options(
        &node,
        &Options {
            line_wrapping: LineWrapping::Unwrap,
            ..Options::default()
        },
    )
    .unwrap();

    assert_eq!(output, "*first second*\n");
}

#[test]
fn intraword_underscores() {
    for (source, expected) in [
        ("io_uring is fast\n", "io_uring is fast\n"),
        ("a snake_case_name\n", "a snake_case_name\n"),
        ("x_1 and a__b\n", "x_1 and a__b\n"),
        ("é_ü\n", "é_ü\n"),
        ("*io_uring* [io_uring](x)\n", "*io_uring* [io_uring](x)\n"),
        ("a\\_\n", "a\\_\n"),
        ("\\_a\n", "\\_a\n"),
        ("a \\_ b\n", "a \\_ b\n"),
        ("**a**\\_b\n", "**a**\\_b\n"),
        ("a\\_**b**\n", "a\\_**b**\n"),
        ("a\\*_b\n", "a\\*\\_b\n"),
        ("a_\\*b\n", "a\\_\\*b\n"),
        ("x\\@y a_b.com\n", "x@y a_b.com\n"),
    ] {
        let document = Document::parse(source, &base_markdown::ParseOptions::gfm()).unwrap();
        let markdown = base_markdown::to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let html = |value: &str| {
            base_markdown::to_html_with_options(value, &base_markdown::Options::gfm())
        };
        assert_eq!(
            html(&markdown),
            html(source),
            "{source:?} should round-trip"
        );
    }

    for (value, expected, message) in [
        ("a_b", "a_b\n", "should not escape `_` between letters"),
        ("1_000", "1_000\n", "should not escape `_` between digits"),
        (
            "é_é",
            "é_é\n",
            "should not escape `_` between non-ASCII letters",
        ),
        (
            "a._.b",
            "a.\\_.b\n",
            "should escape `_` between punctuation",
        ),
        ("a_ b", "a\\_ b\n", "should escape `_` before whitespace"),
        ("a _b", "a \\_b\n", "should escape `_` after whitespace"),
        ("a__b", "a__b\n", "should not escape `__` between letters"),
        (
            "x@a_b.com",
            "x@a_b.com\n",
            "should not escape `_` in email domains",
        ),
        (
            "a__.b",
            "a\\_\\_.b\n",
            "should escape `__` before punctuation",
        ),
    ] {
        assert_eq!(
            to(&Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from(value),
                    position: None,
                })],
                position: None,
            }))
            .unwrap(),
            expected,
            "{message}"
        );
    }

    let emphasis = || {
        Node::Emphasis(Emphasis {
            children: vec![Node::Text(Text {
                value: String::from("x"),
                position: None,
            })],
            position: None,
        })
    };
    let text = |value: &str| {
        Node::Text(Text {
            value: String::from(value),
            position: None,
        })
    };
    let options = Options {
        emphasis: '_',
        ..Default::default()
    };

    assert_eq!(
        to_with_options(
            &Node::Paragraph(Paragraph {
                children: vec![emphasis(), text("__a")],
                position: None,
            }),
            &options
        )
        .unwrap(),
        "_x_\\_\\_a\n",
        "should escape `__` after emphasis"
    );

    assert_eq!(
        to_with_options(
            &Node::Paragraph(Paragraph {
                children: vec![text("a__"), emphasis()],
                position: None,
            }),
            &options
        )
        .unwrap(),
        "a\\_\\__x_\n",
        "should escape `__` before emphasis"
    );
}

/// Serialize `source`, parsed with GFM, and serialize the result again.
fn two_passes(source: &str) -> (String, String) {
    let options = base_markdown::ParseOptions::gfm();
    let first = base_markdown::to_markdown(&Document::parse(source, &options).unwrap()).unwrap();
    let second =
        base_markdown::to_markdown(&Document::parse(first.as_str(), &options).unwrap()).unwrap();
    (first, second)
}

#[test]
fn literal_autolinks_are_written_as_authored() {
    for source in [
        "www.example.com\n",
        "https://a.b/c\n",
        "a@b.co\n",
        "x@a_b.com www.a.b_c\n",
        "see www.a.b, then https://c.d/e?f and x@y.co.\n",
        "(www.a.b) - https://c.d\n",
        "*www.a.b* (www.c.d) ~~e@f.gh~~\n",
        "**www.a.b**.\n",
        "- www.a.b\n- https://c.d\n",
        "> www.a.b\n",
        "# www.a.b\n",
        "| www.a.b |\n| --- |\n| x@y.z |\n",
        "x@y.z~www.c.d\n",
        "\\<a@b.c>\n",
        // A `<` before a bare link stays escaped when `>` would close an autolink around it.
        "\\<1@a.b> x\n",
        "\\<1@a.b\n",
        "\\\\<1@a.b>\n",
        "\\x@y.z\n",
        "\\&x@y.z\n",
        "www.a.b. x\n",
        "[www.a.b](http://www.a.b) <https://a.b> <a@b.c>\n",
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, source, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn escapes_that_only_kept_text_from_linking_are_dropped() {
    // Text is not escaped to keep GFM from linking it, so it may link once it is written.
    for (source, expected) in [
        ("www\\.example.com\n", "www.example.com\n"),
        ("http\\://example.com\n", "http://example.com\n"),
        ("x\\@example.com\n", "x@example.com\n"),
        ("\\.x@y.z\n", ".x@y.z\n"),
        // Characters an email address would take keep their escape, which ends it.
        ("x@y.z\\.c\n", "x@y.z\\.c\n"),
        ("xx@y.z\\-\n", "xx@y.z\\-\n"),
        ("x@y.z\\_a.b\n", "x@y.z\\_a.b\n"),
        ("x@y.z&#x61;\n", "x@y.z&#x61;\n"),
        ("\\\\x@y.z\n", "\\x@y.z\n"),
        // Underscores in an email domain are not escaped either.
        ("x\\@a_b.com\n", "x@a_b.com\n"),
        ("x&#64;a_b.com\n", "x@a_b.com\n"),
        ("x\\@y.co1\\_www.\n", "x@y.co1_www.\n"),
        // Adjacent escaped backslashes each keep their escape.
        ("\\\\\\\\.\n", "\\\\\\\\.\n"),
        ("a\\\\\\\\*b*\n", "a\\\\\\\\*b*\n"),
        ("\\\\\\\\[a](b)\n", "\\\\\\\\[a](b)\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn literal_autolinks_are_written_as_their_text_before_escaped_neighbours() {
    // A bare link is written as its text, and the text after it is escaped as anywhere else. A
    // backslash ends a bare link, so these read back as the same link.
    for (source, expected) in [
        ("www.com_\n", "www.com\\_\n"),
        ("www.a.b* x\n", "www.a.b\\* x\n"),
        ("http://a.b/c_(d)_ x\n", "http://a.b/c_(d)\\_ x\n"),
        ("www.a.b www.com_ x@y.z\n", "www.a.b www.com\\_ x@y.z\n"),
        // The link would take a `&` that ends the text, so it is escaped.
        ("WWW.www&amp;\n", "WWW.www\\&\n"),
        ("https://a.b/c  \nb\n", "https://a.b/c\\\nb\n"),
        (
            "https://api.example.com/**\n",
            "https://api.example.com/\\*\\*\n",
        ),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
        let options = base_markdown::Options::gfm();
        assert_eq!(
            base_markdown::to_html_with_options(&first, &options),
            base_markdown::to_html_with_options(source, &options),
            "{source:?}"
        );
    }
    // An email ends at the backslash already.
    assert_eq!(
        two_passes("x@y.z_\n"),
        ("x@y.z\\_\n".into(), "x@y.z\\_\n".into())
    );
}

#[test]
fn text_that_becomes_a_literal_autolink_settles_after_one_pass() {
    for source in [
        "> - //&amp;x@y.z9\\_www\n",
        "mailto:<a@b.c>\\\\www&#64;\\.0\\_a.b\n",
        "&x&#64;y.z\n",
        "\\\\x&#64;y.z\n",
        "&amp;http\\://a.b\n",
        "\\\\http\\://a.b\n",
        "x\\\\x\\@y.z\n",
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn literal_autolinks_keep_escapes_their_context_needs() {
    for source in [
        "x@y.z\\ \na\n",
        "_a x@y.z* b_\n",
        "[www. \\]: /u\n",
        "[www. \\]\n\n[www. ]: /u\n",
        "a\n    >x@y.z\n",
        "~~a \\~~.a@b.c\n",
        "~~! \\~~!a@b.c\n",
        "~~# \\~~:a@b.c\n",
    ] {
        let (first, second) = two_passes(source);
        let html = |value: &str| {
            base_markdown::to_html_with_options(value, &base_markdown::Options::gfm())
        };
        assert_eq!(html(&first), html(source), "{source:?} gave {first:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn ordered_list_markers_are_escaped_by_their_place_in_the_line() {
    // How the parser splits text at an escape does not change whether the marker is escaped.
    for (source, expected) in [
        ("1)a\n", "1\\)a\n"),
        ("1\\)a\n", "1\\)a\n"),
        ("1\\) a\n", "1\\) a\n"),
        ("1\\. a\n", "1\\. a\n"),
        ("x\n1\\) a\n", "x\n1\\) a\n"),
        ("- 1\\)a\n", "- 1\\)a\n"),
        ("x 1)a\n", "x 1)a\n"),
        ("x 1\\)a\n", "x 1)a\n"),
        ("x 1\\. a\n", "x 1. a\n"),
        // Text after an opening emphasis marker does not start the line.
        ("**1.** a\n", "**1.** a\n"),
        ("*1)a* b\n", "*1)a* b\n"),
        ("~~1\\. a~~\n", "~~1. a~~\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn underscores_after_attention_markers_do_not_depend_on_text_splits() {
    // The parser splits this text at `$` but not at `_`, so the first underscore follows the
    // opening marker's neighbour in one pass and the text node's start in the next.
    for (source, expected) in [("*b_1=a$1.:]@\na~*\n", "*b_1=a\\$1.:]@\na~*\n")] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn whitespace_inside_attention_is_always_a_character_reference() {
    for (source, expected) in [
        ("*&#x20;a*\n", "*&#x20;a*\n"),
        ("**&#x20;a**\n", "**&#x20;a**\n"),
        ("~~&#x20;a~~\n", "~~&#x20;a~~\n"),
        ("*&#9;a*\n", "*&#x9;a*\n"),
        ("*a&#x20;*\n", "*a&#x20;*\n"),
        ("*&#xA0;a*\n", "*&#xA0;a*\n"),
        ("> *&#x20;a*\n", "> *&#x20;a*\n"),
        ("- *&#x20;a*\n", "- *&#x20;a*\n"),
        ("x *&#x20;a*\n", "x *&#x20;a*\n"),
        ("x *a&#9;* b\n", "x *a&#x9;* b\n"),
        ("x **&#x20;a&#x20;** b\n", "x **&#x20;a&#x20;** b\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn whitespace_inside_constructed_attention_is_always_a_character_reference() {
    fn text(value: &str) -> Node {
        Node::Text(Text {
            value: value.to_owned(),
            position: None,
        })
    }
    fn paragraph(children: Vec<Node>) -> Node {
        Node::Paragraph(Paragraph {
            children,
            position: None,
        })
    }
    type Wrap = fn(Vec<Node>) -> Node;
    let attention: [(Wrap, &str); 3] = [
        (
            |children| {
                Node::Emphasis(Emphasis {
                    children,
                    position: None,
                })
            },
            "*",
        ),
        (
            |children| {
                Node::Strong(Strong {
                    children,
                    position: None,
                })
            },
            "**",
        ),
        (
            |children| {
                Node::Delete(Delete {
                    children,
                    position: None,
                })
            },
            "~~",
        ),
    ];
    for (wrap, marker) in attention {
        for (value, reference) in [
            (" note", "&#x20;note"),
            ("\tnote", "&#x9;note"),
            ("\u{a0}note", "&#xA0;note"),
        ] {
            let inline = || wrap(vec![text(value)]);
            let trailing = || wrap(vec![text("note ")]);
            for (node, expected) in [
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![inline()])],
                        position: None,
                    }),
                    format!("{marker}{reference}{marker}\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x "), inline(), text(" y")])],
                        position: None,
                    }),
                    format!("x {marker}{reference}{marker} y\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x "), trailing(), text("y")])],
                        position: None,
                    }),
                    // A letter next to a marker after a reference is a reference too.
                    format!("x {marker}note&#x20;{marker}&#x79;\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x"), inline()])],
                        position: None,
                    }),
                    format!("&#x78;{marker}{reference}{marker}\n"),
                ),
                // Only the character right before the marker changes, even when the text before
                // it is split.
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x"), text(".y"), inline()])],
                        position: None,
                    }),
                    format!("x.&#x79;{marker}{reference}{marker}\n"),
                ),
                // An empty text before the whitespace writes nothing.
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![wrap(vec![text(""), text(value)])])],
                        position: None,
                    }),
                    format!("{marker}{reference}{marker}\n"),
                ),
                (
                    Node::Blockquote(Blockquote {
                        children: vec![paragraph(vec![inline()])],
                        position: None,
                    }),
                    format!("> {marker}{reference}{marker}\n"),
                ),
                (
                    Node::List(List {
                        children: vec![Node::ListItem(ListItem {
                            children: vec![paragraph(vec![inline()])],
                            position: None,
                            spread: false,
                            checked: None,
                        })],
                        position: None,
                        ordered: false,
                        start: None,
                        spread: false,
                    }),
                    format!("- {marker}{reference}{marker}\n"),
                ),
            ] {
                let markdown = to(&node).unwrap();
                assert_eq!(markdown, expected, "{value:?}");
                let (first, second) = two_passes(&markdown);
                assert_eq!(first, markdown, "{value:?}");
                assert_eq!(second, first, "{value:?}");
            }
        }
    }
}

#[test]
fn whitespace_inside_attention_keeps_the_text_before_it() {
    // Escapes and references split the text before the marker into several nodes. Looking ahead
    // at the attention from an earlier node must not change what that node wrote.
    let options = base_markdown::Options::gfm();
    for (open, close) in [("*", "*"), ("**", "**"), ("~~", "~~"), ("~", "~")] {
        for whitespace in ["&#x20;", "&#x9;", "&#xA0;"] {
            for prefix in [
                "see e.g.",
                "v1.2",
                "word\\*",
                "Caf&eacute;",
                "Hi!",
                "a.b",
                "x&amp;y",
            ] {
                for (start, end) in [("", ""), ("> ", ""), ("- ", ""), ("| a |\n| - |\n| ", " |")] {
                    for space in [" ", ""] {
                        let source =
                            format!("{start}{prefix}{space}{open}{whitespace}x{close}{end}\n");
                        let (first, second) = two_passes(&source);
                        assert_eq!(second, first, "{source:?}");
                        assert_eq!(
                            base_markdown::to_html_with_options(&first, &options),
                            base_markdown::to_html_with_options(&source, &options),
                            "{source:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn text_after_attention_around_a_bare_link_does_not_join_it() {
    let options = base_markdown::Options::gfm();
    for (source, expected) in [
        (
            "**www.example.com**&amp; more\n",
            "**www.example.com**\\& more\n",
        ),
        ("**bold www.x.y**\\-\n", "**bold www.x.y**\\-\n"),
        // The link reads on across every closing marker after it.
        ("~~*www.a.b*~~&amp;\n", "~~*www.a.b*~~\\&\n"),
        ("~~https://a.b/c~~&amp;\n", "~~https://a.b/c~~\\&\n"),
        // Text the link would leave out is written as it is.
        ("*www.a.b*.\n", "*www.a.b*.\n"),
        ("*www.a.b* x\n", "*www.a.b* x\n"),
        // An address cannot reach across a marker.
        ("*a@b.c*.d\n", "*a@b.c*.d\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
        let html = |value: &str| base_markdown::to_html_with_options(value, &options);
        assert_eq!(html(&first), html(source), "{source:?}");
    }
}

#[test]
fn characters_a_literal_autolink_would_take_end_it() {
    for (source, expected) in [
        // A backslash ends a web link, so text the link would take is escaped.
        ("www.a.b&amp; c\n", "www.a.b\\& c\n"),
        ("https://a.b/?x=1&amp;\n", "https://a.b/?x=1\\&\n"),
        ("www.a\\_b.com\n", "www.a\\_b.com\n"),
        ("www.a.b&amp;. c\n", "www.a.b\\&. c\n"),
        // Text the link would leave out is written as it is.
        ("https://a.b/c. x\n", "https://a.b/c. x\n"),
        ("(https://a.b/c) x\n", "(https://a.b/c) x\n"),
        ("a@b.c&#46;d\n", "a@b.c\\.d\n"),
        ("a@b.c&#46;\n", "a@b.c.\n"),
        ("a@b.c&#x2D;d\n", "a@b.c\\-d\n"),
        // Only an `xmpp:` address takes a `/resource`.
        ("xmpp:a@b.c&#x2F;x\n", "xmpp:a@b.c\\/x\n"),
        ("a@b.c&#x2F;x\n", "a@b.c/x\n"),
        // Only the text written right after the link changes, even when a later node is
        // rendered again from the same place.
        (
            "#/[$@--~_|;[a]/_|x@y.z(1~+\n",
            "\\#/\\[\\$@--~~*|;\\[a]/*|x@y.z(1~~+\n",
        ),
        // The node after the one next to the link is looked at without the link beside it.
        ("http://b\\<!/ @\n", "http://b\\<!/ @\n"),
        ("http://b<!/ @\n", "http://b\\<!/ @\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn text_that_gfm_would_link_is_written_verbatim() {
    for source in [
        "www.example.com\n",
        "https://a.b/c\n",
        "a@b.co\n",
        "x@a_b.com and http://y.z/w_v\n",
    ] {
        let document = Document::parse(source, &base_markdown::ParseOptions::default()).unwrap();
        assert_eq!(base_markdown::to_markdown(&document).unwrap(), source);
    }
}
