// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{
    message, to_html_with_options, CompileOptions, Constructs, Options, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Link, Node, Paragraph, Root, Text};

#[test]
fn autolink() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Options::commonmark()
    };

    assert_eq!(
        to_html_with_options("<http://foo.bar.baz>", &Options::commonmark()),
        "<p><a href=\"http://foo.bar.baz\">http://foo.bar.baz</a></p>",
        "should support protocol autolinks (1)"
    );

    assert_eq!(
    to_html_with_options("<http://foo.bar.baz/test?q=hello&id=22&boolean>", &Options::commonmark()),
    "<p><a href=\"http://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean\">http://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean</a></p>",
    "should support protocol autolinks (2)"
  );

    assert_eq!(
        to_html_with_options("<irc://foo.bar:2233/baz>", &Options::commonmark()),
        "<p><a href=\"irc://foo.bar:2233/baz\">irc://foo.bar:2233/baz</a></p>",
        "should support protocol autolinks w/ `irc`"
    );

    assert_eq!(
        to_html_with_options("<MAILTO:FOO@BAR.BAZ>", &Options::commonmark()),
        "<p><a href=\"MAILTO:FOO@BAR.BAZ\">MAILTO:FOO@BAR.BAZ</a></p>",
        "should support protocol autolinks in uppercase"
    );

    // Deliberate divergence from CommonMark: schemes outside the allowlist are text, even with dangerous protocols allowed.
    assert_eq!(
        to_html_with_options("<a+b+c:d>", &Options::commonmark()),
        "<p>&lt;a+b+c:d&gt;</p>",
        "should not support protocol autolinks w/ other schemes (1, default)"
    );

    // Deliberate divergence from CommonMark.
    assert_eq!(
        to_html_with_options("<a+b+c:d>", &danger),
        "<p>&lt;a+b+c:d&gt;</p>",
        "should not support protocol autolinks w/ other schemes (1, danger)"
    );

    // Deliberate divergence from CommonMark.
    assert_eq!(
        to_html_with_options("<made-up-scheme://foo,bar>", &Options::commonmark()),
        "<p>&lt;made-up-scheme://foo,bar&gt;</p>",
        "should not support protocol autolinks w/ other schemes (2, default)"
    );

    // Deliberate divergence from CommonMark.
    assert_eq!(
        to_html_with_options("<made-up-scheme://foo,bar>", &danger),
        "<p>&lt;made-up-scheme://foo,bar&gt;</p>",
        "should not support protocol autolinks w/ other schemes (2, danger)"
    );

    assert_eq!(
        to_html_with_options("<http://../>", &Options::commonmark()),
        "<p><a href=\"http://../\">http://../</a></p>",
        "should support protocol autolinks w/ incorrect URIs (3)"
    );

    // Deliberate divergence from CommonMark.
    assert_eq!(
        to_html_with_options("<localhost:5001/foo>", &danger),
        "<p>&lt;localhost:5001/foo&gt;</p>",
        "should not support protocol autolinks w/ other schemes (3)"
    );

    assert_eq!(
        to_html_with_options("<http://foo.bar/baz bim>", &Options::commonmark()),
        "<p>&lt;http://foo.bar/baz bim&gt;</p>",
        "should not support protocol autolinks w/ spaces"
    );

    assert_eq!(
        to_html_with_options("<http://example.com/\\[\\>", &Options::commonmark()),
        "<p><a href=\"http://example.com/%5C%5B%5C\">http://example.com/\\[\\</a></p>",
        "should not support character escapes in protocol autolinks"
    );

    assert_eq!(
        to_html_with_options("<foo@bar.example.com>", &Options::commonmark()),
        "<p><a href=\"mailto:foo@bar.example.com\">foo@bar.example.com</a></p>",
        "should support email autolinks (1)"
    );

    assert_eq!(
        to_html_with_options("<foo+special@Bar.baz-bar0.com>", &Options::commonmark()),
        "<p><a href=\"mailto:foo+special@Bar.baz-bar0.com\">foo+special@Bar.baz-bar0.com</a></p>",
        "should support email autolinks (2)"
    );

    assert_eq!(
        to_html_with_options("<a@b.c>", &Options::commonmark()),
        "<p><a href=\"mailto:a@b.c\">a@b.c</a></p>",
        "should support email autolinks (3)"
    );

    assert_eq!(
        to_html_with_options("<foo\\+@bar.example.com>", &Options::commonmark()),
        "<p>&lt;foo+@bar.example.com&gt;</p>",
        "should not support character escapes in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<>", &Options::commonmark()),
        "<p>&lt;&gt;</p>",
        "should not support empty autolinks"
    );

    assert_eq!(
        to_html_with_options("< http://foo.bar >", &Options::commonmark()),
        "<p>&lt; http://foo.bar &gt;</p>",
        "should not support autolinks w/ space"
    );

    assert_eq!(
        to_html_with_options("<m:abc>", &Options::commonmark()),
        "<p>&lt;m:abc&gt;</p>",
        "should not support autolinks w/ a single character for a scheme"
    );

    assert_eq!(
        to_html_with_options("<foo.bar.baz>", &Options::commonmark()),
        "<p>&lt;foo.bar.baz&gt;</p>",
        "should not support autolinks w/o a colon or at sign"
    );

    assert_eq!(
        to_html_with_options("http://example.com", &Options::commonmark()),
        "<p>http://example.com</p>",
        "should not support protocol autolinks w/o angle brackets"
    );

    assert_eq!(
        to_html_with_options("foo@bar.example.com", &Options::commonmark()),
        "<p>foo@bar.example.com</p>",
        "should not support email autolinks w/o angle brackets"
    );

    // Extra:
    assert_eq!(
        to_html_with_options("<https://a.b> <HTTP://A> <Https://a> <mailto:a@b>", &Options::commonmark()),
        "<p><a href=\"https://a.b\">https://a.b</a> <a href=\"HTTP://A\">HTTP://A</a> <a href=\"Https://a\">Https://a</a> <a href=\"mailto:a@b\">mailto:a@b</a></p>",
        "should support allowed schemes in any case"
    );

    for input in [
        "<httpx://a>",
        "<htt:a>",
        "<httpss://a>",
        "<mail:a>",
        "<mailtox:a>",
        "<xmailto:a>",
        "<tel:>",
        "<ftps://a>",
        "<javascript:alert(1)>",
        "<data:text/plain,a>",
        // Deliberate divergence from CommonMark: allowed schemes also need a target.
        "<http:>",
        "<http:a>",
        "<https:x>",
        "<http:/a>",
        "<http://>",
        "<https:///a>",
        "<http://?a>",
        "<http://#a>",
        "<http:// a>",
        "<mailto:>",
        "<mailto:?subject=a>",
        "<mailto: a>",
        "<mailto:a>",
        "<mailto:a@>",
    ] {
        assert_eq!(
            to_html_with_options(input, &danger),
            format!("<p>&lt;{}&gt;</p>", &input[1..input.len() - 1]),
            "should not support schemes outside the table, or w/o what their kind requires: {:?}",
            input
        );
    }

    assert_eq!(
        to_html_with_options("Option<std::io::Error>", &Options::commonmark()),
        "<p>Option&lt;std::io::Error&gt;</p>",
        "should not turn Rust paths into autolinks"
    );

    assert_eq!(
        to_html_with_options("<xs:element>", &danger),
        "<p>&lt;xs:element&gt;</p>",
        "should not turn namespaced tags into autolinks"
    );

    assert_eq!(
        to_html_with_options(
            "a <xs:element>",
            &Options {
                parse: ParseOptions {
                    html_tag_namespaces: true,
                    ..ParseOptions::commonmark()
                },
                ..danger
            }
        ),
        "<p>a <xs:element></p>",
        "should leave namespaced tags to raw HTML"
    );

    assert_eq!(
        to_html_with_options("<http@a.b> <https@a.b> <mailto@a.b> <h@a.b> <m@a.b>", &Options::commonmark()),
        "<p><a href=\"mailto:http@a.b\">http@a.b</a> <a href=\"mailto:https@a.b\">https@a.b</a> <a href=\"mailto:mailto@a.b\">mailto@a.b</a> <a href=\"mailto:h@a.b\">h@a.b</a> <a href=\"mailto:m@a.b\">m@a.b</a></p>",
        "should support email autolinks whose local part starts like a scheme"
    );

    assert_eq!(
        to_html_with_options("<http/a> <m/a>", &Options::commonmark()),
        "<p>&lt;http/a&gt; &lt;m/a&gt;</p>",
        "should not support scheme prefixes w/o a colon or at sign"
    );

    assert_eq!(
        to_html_with_options("<*@example.com>", &Options::commonmark()),
        "<p><a href=\"mailto:*@example.com\">*@example.com</a></p>",
        "should support autolinks w/ atext (1)"
    );
    assert_eq!(
        to_html_with_options("<a*@example.com>", &Options::commonmark()),
        "<p><a href=\"mailto:a*@example.com\">a*@example.com</a></p>",
        "should support autolinks w/ atext (2)"
    );
    assert_eq!(
        to_html_with_options("<aa*@example.com>", &Options::commonmark()),
        "<p><a href=\"mailto:aa*@example.com\">aa*@example.com</a></p>",
        "should support autolinks w/ atext (3)"
    );

    assert_eq!(
        to_html_with_options("<aaa©@example.com>", &Options::commonmark()),
        "<p>&lt;aaa©@example.com&gt;</p>",
        "should support non-atext in email autolinks local part (1)"
    );
    assert_eq!(
        to_html_with_options("<a*a©@example.com>", &Options::commonmark()),
        "<p>&lt;a*a©@example.com&gt;</p>",
        "should support non-atext in email autolinks local part (2)"
    );

    assert_eq!(
        to_html_with_options("<asd@.example.com>", &Options::commonmark()),
        "<p>&lt;asd@.example.com&gt;</p>",
        "should not support a dot after an at sign in email autolinks"
    );
    assert_eq!(
        to_html_with_options("<asd@e..xample.com>", &Options::commonmark()),
        "<p>&lt;asd@e..xample.com&gt;</p>",
        "should not support a dot after another dot in email autolinks"
    );

    assert_eq!(
        to_html_with_options(
        "<asd@012345678901234567890123456789012345678901234567890123456789012>", &Options::commonmark()),
        "<p><a href=\"mailto:asd@012345678901234567890123456789012345678901234567890123456789012\">asd@012345678901234567890123456789012345678901234567890123456789012</a></p>",
        "should support 63 character in email autolinks domains"
    );

    assert_eq!(
        to_html_with_options(
            "<asd@0123456789012345678901234567890123456789012345678901234567890123>",
            &Options::commonmark()
        ),
        "<p>&lt;asd@0123456789012345678901234567890123456789012345678901234567890123&gt;</p>",
        "should not support 64 character in email autolinks domains"
    );

    assert_eq!(
        to_html_with_options(
            "<asd@012345678901234567890123456789012345678901234567890123456789012.a>", &Options::commonmark()),
        "<p><a href=\"mailto:asd@012345678901234567890123456789012345678901234567890123456789012.a\">asd@012345678901234567890123456789012345678901234567890123456789012.a</a></p>",
        "should support a TLD after a 63 character domain in email autolinks"
    );

    assert_eq!(
        to_html_with_options(
            "<asd@0123456789012345678901234567890123456789012345678901234567890123.a>",
            &Options::commonmark()
        ),
        "<p>&lt;asd@0123456789012345678901234567890123456789012345678901234567890123.a&gt;</p>",
        "should not support a TLD after a 64 character domain in email autolinks"
    );

    assert_eq!(
        to_html_with_options(
            "<asd@a.012345678901234567890123456789012345678901234567890123456789012>", &Options::commonmark()),
        "<p><a href=\"mailto:asd@a.012345678901234567890123456789012345678901234567890123456789012\">asd@a.012345678901234567890123456789012345678901234567890123456789012</a></p>",
        "should support a 63 character TLD in email autolinks"
    );

    assert_eq!(
        to_html_with_options(
            "<asd@a.0123456789012345678901234567890123456789012345678901234567890123>",
            &Options::commonmark()
        ),
        "<p>&lt;asd@a.0123456789012345678901234567890123456789012345678901234567890123&gt;</p>",
        "should not support a 64 character TLD in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<asd@-example.com>", &Options::commonmark()),
        "<p>&lt;asd@-example.com&gt;</p>",
        "should not support a dash after `@` in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<asd@e-xample.com>", &Options::commonmark()),
        "<p><a href=\"mailto:asd@e-xample.com\">asd@e-xample.com</a></p>",
        "should support a dash after other domain characters in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<asd@e--xample.com>", &Options::commonmark()),
        "<p><a href=\"mailto:asd@e--xample.com\">asd@e--xample.com</a></p>",
        "should support a dash after another dash in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<asd@example-.com>", &Options::commonmark()),
        "<p>&lt;asd@example-.com&gt;</p>",
        "should not support a dash before a dot in email autolinks"
    );

    assert_eq!(
        to_html_with_options("<@example.com>", &Options::commonmark()),
        "<p>&lt;@example.com&gt;</p>",
        "should not support an at sign at the start of email autolinks"
    );

    assert_eq!(
        to_html_with_options(
            "<a@b.co>",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        autolink: false,
                        ..Constructs::commonmark()
                    },
                    ..ParseOptions::commonmark()
                },
                ..Options::commonmark()
            }
        ),
        "<p>&lt;a@b.co&gt;</p>",
        "should support turning off autolinks"
    );

    assert_eq!(
        parse_fixture(
            "a <https://alpha.com> b <bravo@charlie.com> c.",
            &Default::default()
        )?,
        Node::Root(Root {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![
                    Node::Text(Text {
                        value: "a ".into(),
                        position: Some(Position::new(1, 1, 0, 1, 3, 2))
                    }),
                    Node::Link(Link {
                        url: "https://alpha.com".into(),
                        title: None,
                        children: vec![Node::Text(Text {
                            value: "https://alpha.com".into(),
                            position: Some(Position::new(1, 4, 3, 1, 21, 20))
                        }),],
                        position: Some(Position::new(1, 3, 2, 1, 22, 21))
                    }),
                    Node::Text(Text {
                        value: " b ".into(),
                        position: Some(Position::new(1, 22, 21, 1, 25, 24))
                    }),
                    Node::Link(Link {
                        url: "mailto:bravo@charlie.com".into(),
                        title: None,
                        children: vec![Node::Text(Text {
                            value: "bravo@charlie.com".into(),
                            position: Some(Position::new(1, 26, 25, 1, 43, 42))
                        }),],
                        position: Some(Position::new(1, 25, 24, 1, 44, 43))
                    }),
                    Node::Text(Text {
                        value: " c.".into(),
                        position: Some(Position::new(1, 44, 43, 1, 47, 46))
                    })
                ],
                position: Some(Position::new(1, 1, 0, 1, 47, 46))
            })],
            position: Some(Position::new(1, 1, 0, 1, 47, 46))
        }),
        "should support autolinks as `Link`s in mdast"
    );

    Ok(())
}
