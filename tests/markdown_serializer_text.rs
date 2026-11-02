// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{LineWrapping, Options};
use base_markdown::Document;
use pretty_assertions::assert_eq;
use support::fixture::canonical;
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
        value: String::from("first  \n \tsecond\n third\nfourth"),
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
            "should not escape `_` in email domains, which keeps the email from linking",
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

/// Serialize `source`, parsed with GFM and normalized, and serialize the result again.
fn two_passes(source: &str) -> (String, String) {
    let options = base_markdown::ParseOptions::gfm();
    let pass = |value: &str| {
        let mut document = Document::parse(value, &options).unwrap();
        document.normalize();
        base_markdown::to_markdown(&document).unwrap()
    };
    let first = pass(source);
    let second = pass(&first);
    (first, second)
}

#[test]
fn literal_autolinks_between_boundaries_are_written_as_authored() {
    // A bare link is written bare when whitespace or a line boundary is on both sides of it.
    for source in [
        "www.example.com\n",
        "https://a.b/c\n",
        "a@b.co\n",
        "x@a-b.com www.a-b.c\n",
        "see www.a.b now\n",
        "https://a.b/c\nnext\n",
        "a\nhttps://a.b/c\n",
        "x\thttps://a.b/c\ty\n",
        "a\\\nhttps://a.b/c\n",
        "- www.a.b\n- https://c.d\n",
        "1. www.a.b\n",
        "- [ ] www.a.b\n",
        "> www.a.b\n",
        "# www.a.b\n",
        "## a www.a.b\n",
        "| www.a.b |\n| --- |\n| x@y.z |\n",
        "| a |\n| --- |\n| x https://a.b/c y |\n",
        "www.a.b *x*\n",
        "*x* www.a.b\n",
        "xmpp:a@b.c/x\n",
        "ssh://a.b\n",
        "\\\\<1@a.b>\n",
        "\\x@y.z\n",
        "[www.a.b](http://www.a.b) <https://a.b> <a@b.c>\n",
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, source, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn literal_autolinks_next_to_anything_else_are_written_as_autolinks_or_explicitly() {
    // No punctuation counts as a boundary. An address or a URL with a scheme is written between
    // angle brackets, and a `www.` link explicitly.
    for (source, expected) in [
        ("www.x.org.\n", "[www.x.org](http://www.x.org).\n"),
        ("a https://a.b/c, b\n", "a <https://a.b/c>, b\n"),
        ("(https://a.b/c)\n", "(<https://a.b/c>)\n"),
        (
            "(www.a.b) - https://c.d\n",
            "([www.a.b](http://www.a.b)) - https://c.d\n",
        ),
        ("https://a.b/c. x\n", "<https://a.b/c>. x\n"),
        ("www.a.b. x\n", "[www.a.b](http://www.a.b). x\n"),
        ("www.a.b* x\n", "[www.a.b](http://www.a.b)\\* x\n"),
        ("www.com_\n", "[www.com](http://www.com)\\_\n"),
        ("http://a.b/c_(d)_ x\n", "<http://a.b/c_(d)>\\_ x\n"),
        ("https://a.b/?x=1&amp;\n", "<https://a.b/?x=1>&\n"),
        ("www.a.b&amp;. c\n", "[www.a.b](http://www.a.b)&. c\n"),
        ("WWW.www&amp;\n", "[WWW.www](http://WWW.www)&\n"),
        ("xx@y.z\\-\n", "<xx@y.z>-\n"),
        ("a@b.c&#x2D;d\n", "<a@b.c>-d\n"),
        ("a@b.c&#46;d\n", "<a@b.c>.d\n"),
        ("x@y.z\\.c\n", "<x@y.z>.c\n"),
        ("x@y.z\\_a.b\n", "<x@y.z>\\_a.b\n"),
        ("x@y.z&#x61;\n", "<x@y.z>a\n"),
        ("x@y.co.\n", "<x@y.co>.\n"),
        ("xmpp:a@b.c&#x2F;x\n", "<xmpp:a@b.c>/x\n"),
        ("a@b.c\u{2014}\n", "<a@b.c>\u{2014}\n"),
        ("x\u{2014}https://a.b/c\n", "x\u{2014}<https://a.b/c>\n"),
        ("`x`https://a.b/c\n", "`x`<https://a.b/c>\n"),
        ("\\.x@y.z\n", ".<x@y.z>\n"),
        ("\\\\x@y.z\n", "\\\\<x@y.z>\n"),
        ("\\<a@b.c>\n", "<<a@b.c>>\n"),
        ("x@y.z~www.c.d\n", "<x@y.z>\\~[www.c.d](http://www.c.d)\n"),
        ("x@y.co1-www.\n", "<x@y.co1-www>.\n"),
        ("https://a.b/c  \nb\n", "<https://a.b/c>\\\nb\n"),
        (
            "https://api.example.com/**\n",
            "<https://api.example.com/>\\*\\*\n",
        ),
        // Inside attention, the marker is next to the link.
        ("*https://a.b/c*\n", "*<https://a.b/c>*\n"),
        ("**www.a.b**.\n", "**[www.a.b](http://www.a.b)**.\n"),
        ("~~e@f.gh~~\n", "~~<e@f.gh>~~\n"),
        (
            "**bold www.x.y**\\-\n",
            "**bold [www.x.y](http://www.x.y)**-\n",
        ),
        ("~~*www.a.b*~~&amp;\n", "~~*[www.a.b](http://www.a.b)*~~&\n"),
        ("*a@b.c*.d\n", "*<a@b.c>*.d\n"),
        // In a table cell, the cell's pipe is not next to the link, but punctuation is.
        (
            "| a |\n| - |\n| https://a.b/c. |\n",
            "| a |\n| --- |\n| <https://a.b/c>. |\n",
        ),
        (
            "| a |\n| - |\n| www.a\\|b |\n",
            "| a |\n| --- |\n| [www.a](http://www.a)\\|b |\n",
        ),
        // The node after the one next to the link is looked at without the link beside it.
        (
            "#/[$@--~_|;[a]/_|x@y.z(1~+\n",
            "\\#/\\[\\$@--~~*|;\\[a]/*|<x@y.z>(1~~+\n",
        ),
        ("http://b<!/ @\n", "<http://b>\\<!/ @\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
        let options = base_markdown::ParseOptions::gfm();
        assert_eq!(
            canonical(&Document::parse(first.as_str(), &options).unwrap()),
            canonical(&Document::parse(source, &options).unwrap()),
            "{source:?}"
        );
    }
}
#[test]
fn escapes_that_keep_text_from_linking_are_kept() {
    // Text is literal, so it is escaped where GFM would link it, with a backslash before the
    // colon, the dot after `www` or the `@`.
    for (source, expected) in [
        ("www\\.example.com\n", "www\\.example.com\n"),
        ("http\\://example.com\n", "http\\://example.com\n"),
        ("x\\@example.com\n", "x\\@example.com\n"),
        // After a backslash, the email has no local part.
        ("\\\\x&#64;y.z\n", "\\x@y.z\n"),
        // An email domain with `_` is no host name, so it does not link and needs no escape.
        ("x\\@a_b.com\n", "x@a_b.com\n"),
        ("x&#64;a_b.com\n", "x@a_b.com\n"),
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
fn text_that_would_become_a_literal_autolink_stays_text() {
    // Text that GFM would link is escaped, so it reads back as text on the first pass.
    for (source, expected) in [
        ("x\\\\x\\@y.z\n", "x\\x@y.z\n"),
        (
            "mailto:<a@b.c>\\\\www&#64;\\.0\\_a.b\n",
            "mailto:<a@b.c>\\www@.0_a.b\n",
        ),
        ("x\\@y.co1\\_www.\n", "x@y.co1_www.\n"),
        ("x\\@y.co1\\-www.\n", "x\\@y.co1-www.\n"),
        ("> - //&amp;x@y.z9\\_www\n", "> - //\\&x@y.z9_www\n"),
        ("&x&#64;y.z\n", "\\&x\\@y.z\n"),
        ("&amp;http\\://a.b\n", "\\&http\\://a.b\n"),
        ("\\\\http\\://a.b\n", "\\http\\://a.b\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
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
    // A `~` at the edge of attention is escaped so it cannot pair.
    for (source, expected) in [("*b_1=a$1.:]@\na~*\n", "*b_1=a\\$1.:]@\na\\~*\n")] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn whitespace_at_the_edges_of_attention_moves_out() {
    // Markdown cannot write whitespace at the edges of attention, so normalizing moves it out, and the serializer encodes it at the edges of the paragraph, other than a no-break space, which the parser does not trim.
    for (source, expected) in [
        ("*&#x20;a*\n", "&#x20;*a*\n"),
        ("**&#x20;a**\n", "&#x20;**a**\n"),
        ("~~&#x20;a~~\n", "&#x20;~~a~~\n"),
        ("*&#9;a*\n", "&#x9;*a*\n"),
        ("*a&#x20;*\n", "*a*&#x20;\n"),
        ("*&#xA0;a*\n", "\u{a0}*a*\n"),
        ("> *&#x20;a*\n", "> &#x20;*a*\n"),
        ("- *&#x20;a*\n", "- &#x20;*a*\n"),
        ("x *&#x20;a*\n", "x  *a*\n"),
        ("x *a&#9;* b\n", "x *a*\t b\n"),
        ("x **&#x20;a&#x20;** b\n", "x  **a**  b\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
    }
}

#[test]
fn whitespace_at_the_edges_of_constructed_attention_moves_out() {
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
        // The whitespace that moves out, and how it is written at the start of a paragraph.
        for (value, space, kept) in [
            (" note", " ", "&#x20;"),
            ("\tnote", "\t", "&#x9;"),
            ("\u{a0}note", "\u{a0}", "\u{a0}"),
        ] {
            let inline = || wrap(vec![text(value)]);
            let trailing = || wrap(vec![text("note ")]);
            for (node, expected) in [
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![inline()])],
                        position: None,
                    }),
                    format!("{kept}{marker}note{marker}\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x "), inline(), text(" y")])],
                        position: None,
                    }),
                    format!("x {space}{marker}note{marker} y\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x "), trailing(), text("y")])],
                        position: None,
                    }),
                    format!("x {marker}note{marker} y\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x"), inline()])],
                        position: None,
                    }),
                    format!("x{space}{marker}note{marker}\n"),
                ),
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![text("x"), text(".y"), inline()])],
                        position: None,
                    }),
                    format!("x.y{space}{marker}note{marker}\n"),
                ),
                // An empty text before the whitespace does not keep it inside.
                (
                    Node::Root(Root {
                        children: vec![paragraph(vec![wrap(vec![text(""), text(value)])])],
                        position: None,
                    }),
                    format!("{kept}{marker}note{marker}\n"),
                ),
                (
                    Node::Blockquote(Blockquote {
                        children: vec![paragraph(vec![inline()])],
                        position: None,
                    }),
                    format!("> {kept}{marker}note{marker}\n"),
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
                    format!("- {kept}{marker}note{marker}\n"),
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
fn whitespace_at_the_edge_of_attention_keeps_the_text_before_it() {
    // Escapes and references split the text before the marker into several nodes. Looking ahead
    // at the attention from an earlier node must not change what that node wrote, so the output
    // reads back as the normalized tree.
    let options = base_markdown::ParseOptions::gfm();
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
                        let mut document = Document::parse(source.as_str(), &options).unwrap();
                        document.normalize();
                        let mut reparsed = Document::parse(first.as_str(), &options).unwrap();
                        reparsed.normalize();
                        assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn text_that_gfm_would_link_is_escaped() {
    // Without GFM, these are text, which the serializer writes so GFM reads it as text too.
    for (source, expected) in [
        ("www.example.com\n", "www\\.example.com\n"),
        ("https://a.b/c\n", "https\\://a.b/c\n"),
        ("a@b.co\n", "a\\@b.co\n"),
        (
            "x@a-b.com and http://y.z/w_v\n",
            "x\\@a-b.com and http\\://y.z/w_v\n",
        ),
    ] {
        let document = Document::parse(source, &base_markdown::ParseOptions::default()).unwrap();
        let written = base_markdown::to_markdown(&document).unwrap();
        assert_eq!(written, expected);
        let mut reparsed =
            Document::parse(written.as_str(), &base_markdown::ParseOptions::gfm()).unwrap();
        reparsed.normalize();
        let mut document = document;
        document.normalize();
        assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
    }
}

#[test]
fn a_tilde_at_the_edges_of_strikethrough_keeps_its_escape() {
    // Unescaped, the `~` would lengthen a marker, and at the start of a line the opening one would
    // open a code fence.
    let options = base_markdown::ParseOptions::gfm();
    for (source, expected) in [
        ("~~\\~5 min~~\n", "~~\\~5 min~~\n"),
        // Strikethrough is always written with two markers.
        ("~\\~~\n", "~~\\~~~\n"),
        ("w\\\n~~\\~x~~\n", "w\\\n~~\\~x~~\n"),
        ("w\n~~\\~x~~\n", "w\n~~\\~x~~\n"),
        ("- ~~\\~~~\n", "- ~~\\~~~\n"),
        ("> ~~\\~~~\n", "> ~~\\~~~\n"),
        ("x ~~\\~a~~\n", "x ~~\\~a~~\n"),
        ("~~a\\~~~\n", "~~a\\~~~\n"),
        ("x ~~a\\~~~ y\n", "x ~~a\\~~~ y\n"),
        ("- ~~\\~a\\~~~\n", "- ~~\\~a\\~~~\n"),
        ("~~a\\\\\\~~~\n", "~~a\\\\\\~~~\n"),
        // A `~` inside that can close is escaped like any other.
        ("~~a~b~~\n", "~~a\\~b~~\n"),
    ] {
        let (first, second) = two_passes(source);
        assert_eq!(first, expected, "{source:?}");
        assert_eq!(second, first, "{source:?}");
        assert_eq!(
            canonical(&Document::parse(first.as_str(), &options).unwrap()),
            canonical(&Document::parse(source, &options).unwrap()),
            "{source:?}"
        );
    }
}

/// Parse `source` with GFM, normalize it, serialize it, and check that the output reads back as the
/// same tree and serializes to itself.
fn assert_roundtrip(source: &str, expected: &str) {
    let options = base_markdown::ParseOptions::gfm();
    let (first, second) = two_passes(source);
    assert_eq!(first, expected, "{source:?}");
    assert_eq!(second, first, "{source:?}");
    let mut document = Document::parse(source, &options).unwrap();
    document.normalize();
    let mut reparsed = Document::parse(first.as_str(), &options).unwrap();
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
}

#[test]
fn a_tilde_that_could_pair_as_strikethrough_is_escaped() {
    for (source, expected) in [
        // A tilde that can close is escaped, so the one before it, which can only open, need not be.
        ("x \\~a~\n", "x ~a\\~\n"),
        ("[\\~a~](u)\n", "[~a\\~](u)\n"),
        ("*\\~a~*\n", "*\\~a\\~*\n"),
        ("x *\\~a~*\n", "x *\\~a\\~*\n"),
        ("x \\~a \\*b\\~ c\\*\n", "x ~a \\*b\\~ c\\*\n"),
        // A run escapes whole, as what is left of a run can still pair.
        ("&#x20; ~~a~\n", "&#x20; \\~\\~a\\~\n"),
        // The run after an escaped backslash is judged by the backslash before it.
        ("~\\\\~~:~\n", "~~\\\\\\~\\~:~~\n"),
        ("a\\\\~b\n", "a\\\\~b\n"),
        ("a\\\\~ b\n", "a\\\\\\~ b\n"),
        ("a\\\\\\~b\n", "a\\\\~b\n"),
        // An escaped or encoded neighbour is punctuation, as written.
        ("\\*~ b\n", "\\*\\~ b\n"),
        ("x \\~~&#x20;\nb~~c~~\n", "x \\~\\~&#x20;\nb~~c~~\n"),
        // A tilde next to a strikethrough sequence escapes with it.
        ("a \\~~~b~~\n", "a \\~~~b~~\n"),
        ("~~a \\~~~\n", "~~a \\~~~\n"),
        // A character that attention writes as a reference is punctuation, so the tildes next to it
        // are escaped.
        ("x \\~&#x61;*.b* c\\~\n", "x \\~&#x61;*.b* c\\~\n"),
        ("x \\~&#x61;**.b** c\\~\n", "x \\~&#x61;**.b** c\\~\n"),
        ("*a.*&#x62;\\~ c~\n", "*a.*&#x62;\\~ c\\~\n"),
        ("**a.**&#x62;\\~\\~ c~\n", "**a.**&#x62;\\~\\~ c\\~\n"),
        // Two tildes that can open are escaped, as a strikethrough sequence can close them.
        ("x \\~~a\n", "x \\~\\~a\n"),
        ("\\~~x a~~b~~\n", "\\~\\~x a~~b~~\n"),
        ("a\\~b\n", "a\\~b\n"),
        ("5\\~10 and foo\\~\n", "5\\~10 and foo\\~\n"),
        ("(approx.\\~)\n", "(approx.\\~)\n"),
        // Tildes that cannot close a single tilde, or that are three or more, stay as they are.
        (
            "About ~5 min, ~/path, a ~ b, (~3 GB), home/~user\n",
            "About ~5 min, ~/path, a ~ b, (~3 GB), home/~user\n",
        ),
        ("*\\~5 min, ~3 GB*\n", "*\\~5 min, ~3 GB*\n"),
        ("a ~~~ b and a~~~b\n", "a ~~~ b and a~~~b\n"),
        // At the start of a line, the whole run is escaped, so no part of it opens a code fence or
        // pairs.
        ("\\~~~a\n", "\\~\\~\\~a\n"),
        ("a\n\\~\\~\\~b\\~\\~\n", "a\n\\~\\~\\~b\\~\\~\n"),
        ("a\n\\~~~b~~\n", "a\n\\~~~b~~\n"),
        ("- \\~~~a\n", "- \\~\\~\\~a\n"),
        ("> \\~~a\n", "> \\~\\~a\n"),
        ("\\~5 min\n", "\\~5 min\n"),
        // Code is written as it is.
        ("`~a~` and ``~~b~~``\n", "`~a~` and `~~b~~`\n"),
        // Destinations and titles are not phrasing.
        ("[a](u~ \"t~\")\n", "[a](u~ \"t~\")\n"),
        ("[a~](<u ~v~> 't~x~')\n", "[a\\~](<u ~v~> \"t~x~\")\n"),
        ("![a\\~b~](u~)\n", "![a\\~b\\~](u~)\n"),
        // Table cells are phrasing.
        (
            "| a\\~ | \\~~b |\n| - | - |\n| ~c~ | `~d~` |\n",
            "| a\\~ | \\~\\~b |\n| --- | --- |\n| ~~c~~ | `~d~` |\n",
        ),
    ] {
        assert_roundtrip(source, expected);
    }
}

#[test]
fn a_tilde_in_constructed_text_that_could_pair_is_escaped() {
    fn text(value: &str) -> Node {
        Node::Text(Text {
            value: value.to_owned(),
            position: None,
        })
    }
    fn delete(children: Vec<Node>) -> Node {
        Node::Delete(Delete {
            children,
            position: None,
        })
    }
    for (children, expected) in [
        // The backslash is literal, so it does not escape the `~` after it.
        (vec![text("\\~a~")], "\\\\~a\\~\n"),
        (vec![text("\\\\~a~")], "\\\\\\\\~a\\~\n"),
        (vec![delete(vec![text("a~~b")])], "~~a\\~\\~b~~\n"),
        (vec![delete(vec![text("x ~~a")])], "~~x \\~\\~a~~\n"),
        (
            vec![text("~~x a"), delete(vec![text("b")])],
            "\\~\\~x a~~b~~\n",
        ),
    ] {
        let node = Node::Root(Root {
            children: vec![Node::Paragraph(Paragraph {
                children,
                position: None,
            })],
            position: None,
        });
        let output = to(&node).unwrap();
        assert_eq!(output, expected);
        let options = base_markdown::ParseOptions::gfm();
        let mut reparsed = Document::parse(output.as_str(), &options).unwrap();
        reparsed.normalize();
        assert_eq!(
            canonical(&reparsed),
            canonical(&node.document()),
            "{expected:?}"
        );
        assert_eq!(base_markdown::to_markdown(&reparsed).unwrap(), output);
    }
}

#[test]
fn escaping_tildes_is_linear() {
    let options = base_markdown::ParseOptions::gfm();
    // Escapes of other characters, such as `*`, are already in the list each run is checked against.
    for unit in ["~", "a~", "\\~", "~~a ", "\\* ~~a"] {
        let time = |count: usize| {
            let mut document = Document::parse(unit.repeat(count) + "\n", &options).unwrap();
            document.normalize();
            let started = std::time::Instant::now();
            base_markdown::to_markdown(&document).unwrap();
            started.elapsed()
        };
        time(1 << 8);
        let mut small = std::time::Duration::MAX;
        let mut large = std::time::Duration::MAX;
        for _ in 0..5 {
            small = small.min(time(1 << 12));
            large = large.min(time(1 << 14));
        }
        assert!(
            large < small * 12,
            "{:?}: 4x input took {:?} against {:?}",
            unit,
            large,
            small
        );
    }
}
