// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::Options;
use base_markdown::{
    to_html_with_options, AlignKind, Document, Options as HtmlOptions, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::fixture::{Link, Node, Table, TableCell, TableRow, Text};

#[test]
fn text() {
    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "[]()\n",
        "should support a link"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            url: String::new(),
            title: None
        }))
        .unwrap(),
        "[a]()\n",
        "should support children"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("a"),
            title: None
        }))
        .unwrap(),
        "[](a)\n",
        "should support a url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: Some(String::from("a"))
        }))
        .unwrap(),
        "[](<> \"a\")\n",
        "should support a title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("a"),
            title: Some(String::from("b"))
        }))
        .unwrap(),
        "[](a \"b\")\n",
        "should support a url and title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b c"),
            title: None
        }))
        .unwrap(),
        "[](<b c>)\n",
        "should support a link w/ enclosed url w/ whitespace in url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b <c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\<c>)\n",
        "should escape an opening angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b >c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\>c>)\n",
        "should escape a closing angle bracket in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b \\+c"),
            title: None
        }))
        .unwrap(),
        "[](<b \\\\+c>)\n",
        "should escape a backslash in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b\nc"),
            title: None
        }))
        .unwrap(),
        "[](<b&#xA;c>)\n",
        "should encode a line ending in `url` in an enclosed url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b(c"),
            title: None
        }))
        .unwrap(),
        "[](b\\(c)\n",
        "should escape an opening paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b)c"),
            title: None
        }))
        .unwrap(),
        "[](b\\)c)\n",
        "should escape a closing paren in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("b\\.c"),
            title: None
        }))
        .unwrap(),
        "[](b\\\\.c)\n",
        "should escape a backslash in `url` in a raw url"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("\x0C"),
            title: None
        }))
        .unwrap(),
        "[](<\x0C>)\n",
        "should support control characters in links"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::new(),
            title: Some(String::from("b\\-c"))
        }))
        .unwrap(),
        "[](<> \"b\\\\-c\")\n",
        "should escape a backslash in `title`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: None
        }))
        .unwrap(),
        "[tel:123](tel:123)\n",
        "should use a resource link for schemes the parser does not autolink"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: vec![Node::Text(Text {
                    value: String::from("tel:123"),
                    position: None
                })],
                position: None,
                url: String::from("tel:123"),
                title: None
            }),
            &Options {
                resource_link: true,
                ..Default::default()
            }
        )
        .unwrap(),
        "[tel:123](tel:123)\n",
        "should use a resource link (`resourceLink: true`)"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            url: String::from("a"),
            title: None
        }),)
        .unwrap(),
        "[a](a)\n",
        "should use a normal link for nodes w/ a value similar to the url w/o a protocol"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: None
        }),)
        .unwrap(),
        "[tel:123](tel:123)\n",
        "should use a resource link for schemes the parser does not autolink"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("tel:123"),
                position: None
            })],
            position: None,
            url: String::from("tel:123"),
            title: Some(String::from("a"))
        }),)
        .unwrap(),
        "[tel:123](tel:123 \"a\")\n",
        "should use a normal link for nodes w/ a value similar to the url w/ a title"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a@b.c"),
                position: None
            })],
            position: None,
            url: String::from("mailto:a@b.c"),
            title: None
        }),)
        .unwrap(),
        "<a@b.c>\n",
        "should use an autolink for nodes w/ a value similar to the url and a protocol (email)"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: vec![Node::Text(Text {
                value: String::from("a.b-c_d@a.b"),
                position: None
            })],
            position: None,
            url: String::from("mailto:a.b-c_d@a.b"),
            title: None
        }),)
        .unwrap(),
        "<a.b-c_d@a.b>\n",
        "should not escape in autolinks"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::new(),
                title: Some("b".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](<> 'b')\n",
        "should support a link w/ title when `quote: \"\'\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::new(),
                title: Some("'".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](<> '\\'')\n",
        "should escape a quote in `title` in a title when `quote: \"\'\"`'"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: "a b![c](d*e_f[g_h`i".to_string(),
            title: None
        }))
        .unwrap(),
        "[](<a b![c](d*e_f[g_h`i>)\n",
        "should not escape unneeded characters in a `DestinationLiteral`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: "a![b](c*d_e[f_g`h<i</j".to_string(),
            title: None
        }))
        .unwrap(),
        "[](a![b]\\(c*d_e[f_g`h<i</j)\n",
        "should not escape unneeded characters in a `DestinationRaw`"
    );

    assert_eq!(
        to(&Node::Link(Link {
            children: Vec::new(),
            position: None,
            url: String::from("#"),
            title: Some("a![b](c*d_e[f_g`h<i</j".to_string())
        }))
        .unwrap(),
        "[](# \"a![b](c*d_e[f_g`h<i</j\")\n",
        "should not escape unneeded characters in a `title` (double quotes)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Link(Link {
                children: Vec::new(),
                position: None,
                url: String::from("#"),
                title: Some("a![b](c*d_e[f_g`h<i</j".to_string())
            }),
            &Options {
                quote: '\'',
                ..Default::default()
            }
        )
        .unwrap(),
        "[](# 'a![b](c*d_e[f_g`h<i</j')\n",
        "should not escape unneeded characters in a `title` (single quotes)"
    );
}

#[test]
fn autolink_schemes_require_the_allowlist() {
    for (destination, expected) in [
        ("HTTP://example.test", "<HTTP://example.test>\n"),
        ("Https://example.test", "<Https://example.test>\n"),
        ("MAILTO:a@b.c", "<MAILTO:a@b.c>\n"),
        ("http:", "[http:](http:)\n"),
        ("https:x", "[https:x](https:x)\n"),
        ("http://", "[http://](http://)\n"),
        ("mailto:", "[mailto:](mailto:)\n"),
        ("httpx://a", "[httpx://a](httpx://a)\n"),
        ("a+.-:value", "[a+.-:value](a+.-:value)\n"),
        ("irc://foo.bar", "[irc://foo.bar](irc://foo.bar)\n"),
        ("tel:123", "[tel:123](tel:123)\n"),
        ("Kk:value", "[Kk:value](Kk:value)\n"),
        ("ſs:value", "[ſs:value](ſs:value)\n"),
        ("hK:value", "[hK:value](hK:value)\n"),
        ("http://a>b", "[http://a>b](http://a>b)\n"),
        ("http://a\\b", "<http://a\\b>\n"),
        ("http://a\\\\", "<http://a\\\\>\n"),
        ("http://a|b", "<http://a|b>\n"),
        ("http://a\u{a0}b", "[http://a\u{a0}b](<http://a\u{a0}b>)\n"),
        ("http://a\u{85}b", "[http://a\u{85}b](<http://a\u{85}b>)\n"),
    ] {
        assert_eq!(
            to(&Node::Link(Link {
                children: vec![Node::Text(Text {
                    value: destination.into(),
                    position: None,
                })],
                position: None,
                url: destination.into(),
                title: None,
            }))
            .unwrap(),
            expected,
            "{:?}",
            destination
        );
    }
}

#[test]
fn email_autolinks_require_the_email_form() {
    for (text, destination, expected) in [
        ("a@b.co", "mailto:a@b.co", "<a@b.co>\n"),
        ("a", "mailto:a", "[a](mailto:a)\n"),
        ("a@b", "mailto:a@b", "<a@b>\n"),
        ("a@-b", "mailto:a@-b", "[a@-b](mailto:a@-b)\n"),
        ("a@b.co", "MAILTO:a@b.co", "[a@b.co](MAILTO:a@b.co)\n"),
        ("a@b.co", "a@b.co", "[a@b.co](a@b.co)\n"),
        ("mailto:a@b.co", "mailto:a@b.co", "<mailto:a@b.co>\n"),
    ] {
        assert_eq!(
            to(&Node::Link(Link {
                children: vec![Node::Text(Text {
                    value: text.into(),
                    position: None,
                })],
                position: None,
                url: destination.into(),
                title: None,
            }))
            .unwrap(),
            expected,
            "{:?} {:?}",
            text,
            destination
        );
    }
}

#[test]
fn table_cells_keep_links_with_pipes_out_of_autolinks() {
    for (text, destination) in [
        ("http://a|b", "http://a|b"),
        ("a|b@c.d", "mailto:a|b@c.d"),
        ("mailto:a|b", "mailto:a|b"),
    ] {
        let cell = |children: Vec<Node>| {
            Node::TableRow(TableRow {
                children: vec![Node::TableCell(TableCell {
                    children,
                    position: None,
                })],
                position: None,
            })
        };
        let table = Node::Table(Table {
            align: vec![AlignKind::None],
            children: vec![
                cell(vec![Node::Text(Text {
                    value: "h".into(),
                    position: None,
                })]),
                cell(vec![Node::Link(Link {
                    children: vec![Node::Text(Text {
                        value: text.into(),
                        position: None,
                    })],
                    position: None,
                    url: destination.into(),
                    title: None,
                })]),
            ],
            position: None,
        });
        let first = to(&table).unwrap();
        let escaped = text.replace('|', "\\|");
        assert_eq!(
            first,
            format!(
                "| h |\n| --- |\n| [{}]({}) |\n",
                escaped,
                destination.replace('|', "\\|")
            ),
            "{:?}",
            text
        );
        // The whole link stays in one cell.
        let html = to_html_with_options(&first, &HtmlOptions::gfm());
        assert!(
            html.contains(&format!(
                "<td><a href=\"{}\">",
                destination.replace('|', "%7C")
            )) && html.matches("<td>").count() == 1,
            "{:?} {:?}",
            text,
            html
        );
        // A label that is itself a GFM literal URL gains an escape on each serialization; that belongs to literal autolink escaping.
        if text.starts_with("http") {
            continue;
        }
        let reparsed = Document::parse(&first, &ParseOptions::gfm()).unwrap();
        assert_eq!(
            base_markdown::to_markdown(&reparsed).unwrap(),
            first,
            "{:?}",
            text
        );
    }
}
