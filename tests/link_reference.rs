// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use core_markdown::{
    message, to_html, to_html_with_options, CompileOptions, Constructs, Options, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Definition, LinkReference, Node, Paragraph, ReferenceKind, Root, Text};

#[test]
fn reference_paragraphs_resolve_emphasis_like_other_compact_inlines() {
    for (label, html) in [("[a]", "<a href=\"/a\">a</a>"), ("[unknown]", "[unknown]")] {
        let source = format!("{label}*a**_\n\n[a]: /a");
        assert_eq!(to_html(&source), format!("<p>{html}<em>a</em>*_</p>\n"));
    }
}

#[test]
fn compact_references_preserve_resolution_and_event_roundtrips() {
    use core_markdown::{render_events, Document, EventParser};
    let options = Options::default();
    let grammar = Options {
        parse: ParseOptions {
            // These fixtures contain no language prefixes. This keeps the established
            // grammar inline resolver available as an independent comparison path.
            inline_code_info: true,
            ..ParseOptions::default()
        },
        ..Options::default()
    };
    let mut sources = vec![
        "[b][z] [a] ![b][z] [x][]\n\n[z]: /z \"title\"\n[a]: /a\n[x]: /x\n[z]: /ignored"
            .to_string(),
        "[SS] [ϴ] [a  b] [a\tb][]\n\n[ẞ]: /ss\n[θ]: /theta\n[a b]: /space".to_string(),
        "[*formatted*][r] [outer [inner]][r] ![outer [inner]][r]\n\n[r]: /r".to_string(),
        "[outer [inner]][r] ![outer [inner]][r]\n\n[r]: /r\n[inner]: /inside".to_string(),
        "[a][missing] [a][ ] [a][[b]] [a][b\n\n[a]: /a".to_string(),
        "[a](broken [a][] [a] \\[a] [x][\\]]\n\n[a]: /a\n[\\]]: /bracket".to_string(),
        "[a][x\\\\y] [x\\\\y]\n\n[x\\\\y]: /slash".to_string(),
        "[a][x\\*y] [x\\*y]\n\n[x\\*y]: /star".to_string(),
        "> [a\n> b] [x][a\n> b]\n\n[a b]: /ab".to_string(),
        "[x][a\r\nb]\n\n[a b]: /ab".to_string(),
        "[a\nb][]\n\n[a b]: /ab".to_string(),
        "[a] ![a]\n\n[a]: /escaped\\*path \"a &amp; b\"".to_string(),
        "[undefined] ![undefined] [a][]\n\n[a]: /a".to_string(),
        "[a] [a] [b] [a]\n\n[b]: /b\n[a]: /first\n[A]: /second".to_string(),
        "[[a]] [outer [unknown]][]\n\n[a]: /a".to_string(),
        "[a][\tlabel\t]\n\n[label]: /label".to_string(),
        "[a][unfinished\n\n[a]: /a".to_string(),
    ];
    for length in [998, 999, 1000] {
        let label = "x".repeat(length);
        sources.push(format!("[text][{label}]\n\n[{label}]: /length\n[a]: /a"));
    }
    for count in [248, 249, 250, 251] {
        let tabs = "\t".repeat(count);
        for prefix in ["", "!", "> ", "> !", "- ", "- !"] {
            sources.push(format!("{prefix}[text][a{tabs}b]\n\n[a b]: /tabs"));
        }
    }
    for source in sources {
        let expected = to_html_with_options(&source, &grammar).unwrap();
        assert_eq!(to_html(&source), expected, "{source:?}");
        let mut checked = String::new();
        render_events(
            &source,
            EventParser::new(&source, &options.parse).unwrap(),
            &options.compile,
            &mut checked,
        )
        .unwrap();
        assert_eq!(checked, expected, "checked: {source:?}");
        let document = Document::parse(&source, &options.parse).unwrap();
        let mut arena = String::new();
        render_events(&source, document.events(), &options.compile, &mut arena).unwrap();
        assert_eq!(arena, expected, "arena: {source:?}");
    }
}

#[test]
fn link_reference() -> Result<(), message::Message> {
    let danger = Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html("[bar]: /url \"title\"\n\n[foo][bar]"),
        "<p><a href=\"/url\" title=\"title\">foo</a></p>",
        "should support link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[link [foo [bar]]][ref]"),
        "<p><a href=\"/uri\">link [foo [bar]]</a></p>",
        "should support balanced brackets in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[link \\[bar][ref]"),
        "<p><a href=\"/uri\">link [bar</a></p>",
        "should support escaped brackets in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[link *foo **bar** `#`*][ref]"),
        "<p><a href=\"/uri\">link <em>foo <strong>bar</strong> <code>#</code></em></a></p>",
        "should support content in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[![moon](moon.jpg)][ref]"),
        "<p><a href=\"/uri\"><img src=\"moon.jpg\" alt=\"moon\" /></a></p>",
        "should support images in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[foo [bar](/uri)][ref]"),
        "<p>[foo <a href=\"/uri\">bar</a>]<a href=\"/uri\">ref</a></p>",
        "should not support links in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[foo *bar [baz][ref]*][ref]"),
        "<p>[foo <em>bar <a href=\"/uri\">baz</a></em>]<a href=\"/uri\">ref</a></p>",
        "should not support deep links in link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n*[foo*][ref]"),
        "<p>*<a href=\"/uri\">foo*</a></p>",
        "should prefer link references over emphasis (1)"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[foo *bar][ref]"),
        "<p><a href=\"/uri\">foo *bar</a></p>",
        "should prefer link references over emphasis (2)"
    );

    assert_eq!(
        to_html_with_options("[ref]: /uri\n\n[foo <bar attr=\"][ref]\">", &danger)?,
        "<p>[foo <bar attr=\"][ref]\"></p>",
        "should prefer HTML over link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[foo`][ref]`"),
        "<p>[foo<code>][ref]</code></p>",
        "should prefer code over link references"
    );

    assert_eq!(
        to_html("[ref]: /uri\n\n[foo<http://example.com/?search=][ref]>"),
        "<p>[foo<a href=\"http://example.com/?search=%5D%5Bref%5D\">http://example.com/?search=][ref]</a></p>",
        "should prefer autolinks over link references"
    );

    assert_eq!(
        to_html("[bar]: /url \"title\"\n\n[foo][BaR]"),
        "<p><a href=\"/url\" title=\"title\">foo</a></p>",
        "should match references to definitions case-insensitively"
    );

    assert_eq!(
        to_html("[ТОЛПОЙ]: /url\n\n[Толпой][Толпой] is a Russian word."),
        "<p><a href=\"/url\">Толпой</a> is a Russian word.</p>",
        "should match references to definitions w/ unicode case-folding"
    );

    assert_eq!(
        to_html("[Foo\n  bar]: /url\n\n[Baz][Foo bar]"),
        "<p><a href=\"/url\">Baz</a></p>",
        "should match references to definitions w/ collapsing"
    );

    assert_eq!(
        to_html("[bar]: /url \"title\"\n\n[foo] [bar]"),
        "<p>[foo] <a href=\"/url\" title=\"title\">bar</a></p>",
        "should not support whitespace between label and reference (1)"
    );

    assert_eq!(
        to_html("[bar]: /url \"title\"\n\n[foo]\n[bar]"),
        "<p>[foo]\n<a href=\"/url\" title=\"title\">bar</a></p>",
        "should not support whitespace between label and reference (2)"
    );

    assert_eq!(
        to_html("[foo]: /url1\n\n[foo]: /url2\n\n[bar][foo]"),
        "<p><a href=\"/url1\">bar</a></p>",
        "should prefer earlier definitions"
    );

    assert_eq!(
        to_html("[foo!]: /url\n\n[bar][foo\\!]"),
        "<p>[bar][foo!]</p>",
        "should not match references to definitions w/ escapes"
    );

    assert_eq!(
        to_html("[ref[]: /uri\n\n[foo][ref[]"),
        "<p>[ref[]: /uri</p>\n<p>[foo][ref[]</p>",
        "should not support references w/ brackets (1)"
    );

    assert_eq!(
        to_html("[ref[bar]]: /uri\n\n[foo][ref[bar]]"),
        "<p>[ref[bar]]: /uri</p>\n<p>[foo][ref[bar]]</p>",
        "should not support references w/ brackets (2)"
    );

    assert_eq!(
        to_html("[[[foo]]]: /url\n\n[[[foo]]]"),
        "<p>[[[foo]]]: /url</p>\n<p>[[[foo]]]</p>",
        "should not support references w/ brackets (3)"
    );

    assert_eq!(
        to_html("[ref\\[]: /uri\n\n[foo][ref\\[]"),
        "<p><a href=\"/uri\">foo</a></p>",
        "should match references to definitions w/ matching escapes"
    );

    assert_eq!(
        to_html("[bar\\\\]: /uri\n\n[bar\\\\]"),
        "<p><a href=\"/uri\">bar\\</a></p>",
        "should support escapes"
    );

    assert_eq!(
        to_html("[]: /uri\n\n[]"),
        "<p>[]: /uri</p>\n<p>[]</p>",
        "should not support empty references"
    );

    assert_eq!(
        to_html("[\n ]: /uri\n\n[\n ]"),
        "<p>[\n]: /uri</p>\n<p>[\n]</p>",
        "should not support blank references"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[foo][]"),
        "<p><a href=\"/url\" title=\"title\">foo</a></p>",
        "should support collaped references"
    );

    assert_eq!(
        to_html("[*foo* bar]: /url \"title\"\n\n[*foo* bar][]"),
        "<p><a href=\"/url\" title=\"title\"><em>foo</em> bar</a></p>",
        "should support content in collaped references"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[Foo][]"),
        "<p><a href=\"/url\" title=\"title\">Foo</a></p>",
        "should match references to definitions case-insensitively"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[foo] \n[]"),
        "<p><a href=\"/url\" title=\"title\">foo</a>\n[]</p>",
        "should not support whitespace between label and collaped reference"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[foo]"),
        "<p><a href=\"/url\" title=\"title\">foo</a></p>",
        "should support shortcut references"
    );

    assert_eq!(
        to_html("[*foo* bar]: /url \"title\"\n\n[*foo* bar]"),
        "<p><a href=\"/url\" title=\"title\"><em>foo</em> bar</a></p>",
        "should support content in shortcut references (1)"
    );

    assert_eq!(
        to_html("[*foo* bar]: /url \"title\"\n\n[[*foo* bar]]"),
        "<p>[<a href=\"/url\" title=\"title\"><em>foo</em> bar</a>]</p>",
        "should support content in shortcut references (2)"
    );

    assert_eq!(
        to_html("[foo]: /url\n\n[[bar [foo]"),
        "<p>[[bar <a href=\"/url\">foo</a></p>",
        "should support content in shortcut references (3)"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[Foo]"),
        "<p><a href=\"/url\" title=\"title\">Foo</a></p>",
        "should match shortcut references to definitions case-insensitively"
    );

    assert_eq!(
        to_html("[foo]: /url\n\n[foo] bar"),
        "<p><a href=\"/url\">foo</a> bar</p>",
        "should support whitespace after a shortcut reference"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n\\[foo]"),
        "<p>[foo]</p>",
        "should “support” an escaped shortcut reference"
    );

    assert_eq!(
        to_html("[foo*]: /url\n\n*[foo*]"),
        "<p>*<a href=\"/url\">foo*</a></p>",
        "should prefer shortcut references over emphasis"
    );

    assert_eq!(
        to_html("[foo]: /url1\n[bar]: /url2\n\n[foo][bar]"),
        "<p><a href=\"/url2\">foo</a></p>",
        "should prefer full references over shortcut references"
    );

    assert_eq!(
        to_html("[foo]: /url1\n\n[foo][]"),
        "<p><a href=\"/url1\">foo</a></p>",
        "should prefer collapsed references over shortcut references"
    );

    assert_eq!(
        to_html("[foo]: /url\n\n[foo]()"),
        "<p><a href=\"\">foo</a></p>",
        "should prefer resources over shortcut references (1)"
    );

    assert_eq!(
        to_html("[foo]: /url \"title\"\n\n[foo]()"),
        "<p><a href=\"\">foo</a></p>",
        "should prefer resources over shortcut references (2)"
    );

    assert_eq!(
        to_html("[foo]: /url1\n\n[foo](not a link)"),
        "<p><a href=\"/url1\">foo</a>(not a link)</p>",
        "should support shortcut references when followed by nonconforming resources"
    );

    assert_eq!(
        to_html("[baz]: /url\n\n[foo][bar][baz]"),
        "<p>[foo]<a href=\"/url\">bar</a></p>",
        "stable/unstable (1)"
    );

    assert_eq!(
        to_html("[baz]: /url1\n[bar]: /url2\n\n[foo][bar][baz]"),
        "<p><a href=\"/url2\">foo</a><a href=\"/url1\">baz</a></p>",
        "stable/unstable (2)"
    );

    assert_eq!(
        to_html("[baz]: /url1\n[foo]: /url2\n\n[foo][bar][baz]"),
        "<p>[foo]<a href=\"/url1\">bar</a></p>",
        "stable/unstable (3)"
    );

    // Extra
    // This matches most implimentations, but is not strictly according to spec.
    // See: <https://github.com/commonmark/commonmark-spec/issues/653>
    assert_eq!(
        to_html("[x]: /url\n\n[x][ ], [x][\t], [x][\n], [x][]"),
        "<p>[x][ ], [x][\t], [x][\n], <a href=\"/url\">x</a></p>",
        "should not support whitespace-only full references"
    );

    // See also: <https://github.com/commonmark/commonmark-spec/issues/616>
    assert_eq!(
        to_html("[+]: example.com\n[\\;]: example.com\n\nWill it link? [\\+], [;]"),
        "<p>Will it link? [+], [;]</p>",
        "should not support mismatched character escapes in shortcuts"
    );

    assert_eq!(
        to_html("[©]: example.com\n[&amp;]: example.com\n\nWill it link? [&copy;], [&]"),
        "<p>Will it link? [©], [&amp;]</p>",
        "should not support mismatched character references in shortcuts"
    );

    assert_eq!(
        to_html("[+]: example.com\n[\\;]: example.com\n\nWill it link? [\\+][], [;][]"),
        "<p>Will it link? [+][], [;][]</p>",
        "should not support mismatched character escapes in collapsed"
    );

    assert_eq!(
        to_html("[©]: example.com\n[&amp;]: example.com\n\nWill it link? [&copy;][], [&][]"),
        "<p>Will it link? [©][], [&amp;][]</p>",
        "should not support mismatched character references in collapsed"
    );

    assert_eq!(
        to_html("[+]: example.com\n[\\;]: example.com\n\nWill it link? [a][ \\+ ], [b][ ; ]"),
        "<p>Will it link? [a][ + ], [b][ ; ]</p>",
        "should not support mismatched character escapes in fulls"
    );

    assert_eq!(
        to_html("[©]: example.com\n[&amp;]: example.com\n\nWill it link? [a][ &copy; ], [b][ & ]"),
        "<p>Will it link? [a][ © ], [b][ &amp; ]</p>",
        "should not support mismatched character references in fulls"
    );

    assert_eq!(
        to_html(
            "[*f*][]
[&semi;][]
[\\;][]
[;][]
[*f*&semi;][]
[*f*\\;][]
[*f*;][]

[*f*]: alpha
[&semi;]: bravo
[\\;]: charlie
[;]: delta
[*f*&semi;]: echo
[*f*\\;]: foxtrot
[*f*;]: golf"
        ),
        "<p><a href=\"alpha\"><em>f</em></a>
<a href=\"bravo\">;</a>
<a href=\"charlie\">;</a>
<a href=\"delta\">;</a>
<a href=\"echo\"><em>f</em>;</a>
<a href=\"foxtrot\"><em>f</em>;</a>
<a href=\"golf\"><em>f</em>;</a></p>
",
        "should properly handle labels w/ character references and -escapes, and phrasing"
    );

    // 999 `x` characters.
    let max = "x".repeat(999);

    assert_eq!(
        to_html(format!("[{}]: a\n[y][{}]", max, max).as_str()),
        "<p><a href=\"a\">y</a></p>",
        "should support 999 characters in a reference"
    );

    assert_eq!(
        to_html(format!("[{}x]: a\n[y][{}x]", max, max).as_str()),
        format!("<p>[{}x]: a\n[y][{}x]</p>", max, max),
        "should not support 1000 characters in a reference"
    );

    assert_eq!(
        to_html("[x] missing-colon\n\nWill it link? [x]"),
        "<p>[x] missing-colon</p>\n<p>Will it link? [x]</p>",
        "should not fail on a missing colon in a definition"
    );

    assert_eq!(
        to_html_with_options(
            "[x]()",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        label_start_link: false,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        )?,
        "<p>[x]()</p>",
        "should support turning off label start (link)"
    );

    assert_eq!(
        to_html_with_options(
            "[x]()",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        label_end: false,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        )?,
        "<p>[x]()</p>",
        "should support turning off label end"
    );

    assert_eq!(
        parse_fixture("[x]: y\n\na [x] b [x][] c [d][x] e.", &Default::default())?,
        Node::Root(Root {
            children: vec![
                Node::Definition(Definition {
                    identifier: "x".into(),
                    label: Some("x".into()),
                    url: "y".into(),
                    title: None,
                    position: Some(Position::new(1, 1, 0, 1, 7, 6))
                }),
                Node::Paragraph(Paragraph {
                    children: vec![
                        Node::Text(Text {
                            value: "a ".into(),
                            position: Some(Position::new(3, 1, 8, 3, 3, 10))
                        }),
                        Node::LinkReference(LinkReference {
                            reference_kind: ReferenceKind::Shortcut,
                            identifier: "x".into(),
                            label: Some("x".into()),
                            children: vec![Node::Text(Text {
                                value: "x".into(),
                                position: Some(Position::new(3, 4, 11, 3, 5, 12))
                            }),],
                            position: Some(Position::new(3, 3, 10, 3, 6, 13))
                        }),
                        Node::Text(Text {
                            value: " b ".into(),
                            position: Some(Position::new(3, 6, 13, 3, 9, 16))
                        }),
                        Node::LinkReference(LinkReference {
                            reference_kind: ReferenceKind::Collapsed,
                            identifier: "x".into(),
                            label: Some("x".into()),
                            children: vec![Node::Text(Text {
                                value: "x".into(),
                                position: Some(Position::new(3, 10, 17, 3, 11, 18))
                            }),],
                            position: Some(Position::new(3, 9, 16, 3, 14, 21))
                        }),
                        Node::Text(Text {
                            value: " c ".into(),
                            position: Some(Position::new(3, 14, 21, 3, 17, 24))
                        }),
                        Node::LinkReference(LinkReference {
                            reference_kind: ReferenceKind::Full,
                            identifier: "x".into(),
                            label: Some("x".into()),
                            children: vec![Node::Text(Text {
                                value: "d".into(),
                                position: Some(Position::new(3, 18, 25, 3, 19, 26))
                            }),],
                            position: Some(Position::new(3, 17, 24, 3, 23, 30))
                        }),
                        Node::Text(Text {
                            value: " e.".into(),
                            position: Some(Position::new(3, 23, 30, 3, 26, 33))
                        }),
                    ],
                    position: Some(Position::new(3, 1, 8, 3, 26, 33))
                }),
            ],
            position: Some(Position::new(1, 1, 0, 3, 26, 33))
        }),
        "should support link (reference) as `LinkReference`s in mdast"
    );

    Ok(())
}
