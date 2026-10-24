// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use core_markdown::{message, to_html, to_html_with_options, Constructs, Options, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Heading, Node, Root, Text};

#[test]
fn heading_setext() -> Result<(), message::Message> {
    assert_eq!(
        to_html("Foo *bar*\n========="),
        "<h1>Foo <em>bar</em></h1>",
        "should support a heading w/ an equals to (rank of 1)"
    );

    assert_eq!(
        to_html("Foo *bar*\n---------"),
        "<h2>Foo <em>bar</em></h2>",
        "should support a heading w/ a dash (rank of 2)"
    );

    assert_eq!(
        to_html("Foo *bar\nbaz*\n===="),
        "<h1>Foo <em>bar\nbaz</em></h1>",
        "should support line endings in setext headings"
    );

    assert_eq!(
        to_html("  Foo *bar\nbaz*\t\n===="),
        "<h1>Foo <em>bar\nbaz</em></h1>",
        "should not include initial and final whitespace around content"
    );

    assert_eq!(
        to_html("Foo\n-------------------------"),
        "<h2>Foo</h2>",
        "should support long underlines"
    );

    assert_eq!(
        to_html("Foo\n="),
        "<h1>Foo</h1>",
        "should support short underlines"
    );

    assert_eq!(
        to_html(" Foo\n  ==="),
        "<h1>Foo</h1>",
        "should support indented content w/ 1 space"
    );

    assert_eq!(
        to_html("  Foo\n---"),
        "<h2>Foo</h2>",
        "should support indented content w/ 2 spaces"
    );

    assert_eq!(
        to_html("   Foo\n---"),
        "<h2>Foo</h2>",
        "should support indented content w/ 3 spaces"
    );

    assert_eq!(
        to_html("    Foo\n    ---"),
        "<pre><code>Foo\n---\n</code></pre>",
        "should not support too much indented content (1)"
    );

    assert_eq!(
        to_html("    Foo\n---"),
        "<pre><code>Foo\n</code></pre>\n<hr />",
        "should not support too much indented content (2)"
    );

    assert_eq!(
        to_html("Foo\n   ----      "),
        "<h2>Foo</h2>",
        "should support initial and final whitespace around the underline"
    );

    assert_eq!(
        to_html("Foo\n   ="),
        "<h1>Foo</h1>",
        "should support whitespace before underline"
    );

    assert_eq!(
        to_html("Foo\n    ="),
        "<p>Foo\n=</p>",
        "should not support too much whitespace before underline (1)"
    );

    assert_eq!(
        to_html("Foo\n\t="),
        "<p>Foo\n=</p>",
        "should not support too much whitespace before underline (2)"
    );

    assert_eq!(
        to_html("Foo\n= ="),
        "<p>Foo\n= =</p>",
        "should not support whitespace in the underline (1)"
    );

    assert_eq!(
        to_html("Foo\n--- -"),
        "<p>Foo</p>\n<hr />",
        "should not support whitespace in the underline (2)"
    );

    assert_eq!(
        to_html("Foo  \n-----"),
        "<h2>Foo</h2>",
        "should not support a hard break w/ spaces at the end"
    );

    assert_eq!(
        to_html("Foo\\\n-----"),
        "<h2>Foo\\</h2>",
        "should not support a hard break w/ backslash at the end"
    );

    assert_eq!(
        to_html("`Foo\n----\n`"),
        "<h2>`Foo</h2>\n<p>`</p>",
        "should precede over inline constructs (1)"
    );

    assert_eq!(
        to_html("<a title=\"a lot\n---\nof dashes\"/>"),
        "<h2>&lt;a title=&quot;a lot</h2>\n<p>of dashes&quot;/&gt;</p>",
        "should precede over inline constructs (2)"
    );

    assert_eq!(
        to_html("> Foo\n---"),
        "<blockquote>\n<p>Foo</p>\n</blockquote>\n<hr />",
        "should not allow underline to be lazy (1)"
    );

    assert_eq!(
        to_html("> foo\nbar\n==="),
        "<blockquote>\n<p>foo\nbar\n===</p>\n</blockquote>",
        "should not allow underline to be lazy (2)"
    );

    assert_eq!(
        to_html("- Foo\n---"),
        "<ul>\n<li>Foo</li>\n</ul>\n<hr />",
        "should not allow underline to be lazy (3)"
    );

    assert_eq!(
        to_html("Foo\nBar\n---"),
        "<h2>Foo\nBar</h2>",
        "should support line endings in setext headings"
    );

    assert_eq!(
        to_html("---\nFoo\n---\nBar\n---\nBaz"),
        "<hr />\n<h2>Foo</h2>\n<h2>Bar</h2>\n<p>Baz</p>",
        "should support adjacent setext headings"
    );

    assert_eq!(
        to_html("\n===="),
        "<p>====</p>",
        "should not support empty setext headings"
    );

    assert_eq!(
        to_html("---\n---"),
        "<hr />\n<hr />",
        "should prefer other constructs over setext headings (1)"
    );

    assert_eq!(
        to_html("- foo\n-----"),
        "<ul>\n<li>foo</li>\n</ul>\n<hr />",
        "should prefer other constructs over setext headings (2)"
    );

    assert_eq!(
        to_html("    foo\n---"),
        "<pre><code>foo\n</code></pre>\n<hr />",
        "should prefer other constructs over setext headings (3)"
    );

    assert_eq!(
        to_html("> foo\n-----"),
        "<blockquote>\n<p>foo</p>\n</blockquote>\n<hr />",
        "should prefer other constructs over setext headings (4)"
    );

    assert_eq!(
        to_html("\\> foo\n------"),
        "<h2>&gt; foo</h2>",
        "should support starting w/ character escapes"
    );

    assert_eq!(
        to_html("Foo\nbar\n---\nbaz"),
        "<h2>Foo\nbar</h2>\n<p>baz</p>",
        "paragraph and heading interplay (1)"
    );

    assert_eq!(
        to_html("Foo\n\nbar\n---\nbaz"),
        "<p>Foo</p>\n<h2>bar</h2>\n<p>baz</p>",
        "paragraph and heading interplay (2)"
    );

    assert_eq!(
        to_html("Foo\nbar\n\n---\n\nbaz"),
        "<p>Foo\nbar</p>\n<hr />\n<p>baz</p>",
        "paragraph and heading interplay (3)"
    );

    assert_eq!(
        to_html("Foo\nbar\n* * *\nbaz"),
        "<p>Foo\nbar</p>\n<hr />\n<p>baz</p>",
        "paragraph and heading interplay (4)"
    );

    assert_eq!(
        to_html("Foo\nbar\n\\---\nbaz"),
        "<p>Foo\nbar\n---\nbaz</p>",
        "paragraph and heading interplay (5)"
    );

    // Extra:
    assert_eq!(
        to_html("Foo  \nbar\n-----"),
        "<h2>Foo<br />\nbar</h2>",
        "should support a hard break w/ spaces in between"
    );

    assert_eq!(
        to_html("Foo\\\nbar\n-----"),
        "<h2>Foo<br />\nbar</h2>",
        "should support a hard break w/ backslash in between"
    );

    assert_eq!(
        to_html("a\n-\nb"),
        "<h2>a</h2>\n<p>b</p>",
        "should prefer a setext heading over an interrupting list"
    );

    assert_eq!(
        to_html("[a]: b\n=\n="),
        "<h1>=</h1>",
        "should support a two setext heading underlines after a definition, as a setext heading"
    );

    assert_eq!(
        to_html("> ===\na"),
        "<blockquote>\n<p>===\na</p>\n</blockquote>",
        "should not support lazyness (1)"
    );

    assert_eq!(
        to_html("> a\n==="),
        "<blockquote>\n<p>a\n===</p>\n</blockquote>",
        "should not support lazyness (2)"
    );

    assert_eq!(
        to_html("a\n- ==="),
        "<p>a</p>\n<ul>\n<li>===</li>\n</ul>",
        "should not support piercing (1)"
    );

    assert_eq!(
        to_html("a\n* ---"),
        "<p>a</p>\n<ul>\n<li>\n<hr />\n</li>\n</ul>",
        "should not support piercing (2)"
    );

    assert_eq!(
        to_html_with_options(
            "a\n-",
            &Options {
                parse: ParseOptions {
                    constructs: Constructs {
                        heading_setext: false,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }
        )?,
        "<p>a\n-</p>",
        "should support turning off setext underlines"
    );

    assert_eq!(
        parse_fixture("alpha\nbravo\n==", &Default::default())?,
        Node::Root(Root {
            children: vec![Node::Heading(Heading {
                depth: 1,
                children: vec![Node::Text(Text {
                    value: "alpha\nbravo".into(),
                    position: Some(Position::new(1, 1, 0, 2, 6, 11))
                }),],
                position: Some(Position::new(1, 1, 0, 3, 3, 14))
            })],
            position: Some(Position::new(1, 1, 0, 3, 3, 14))
        }),
        "should support heading (atx) as `Heading`s in mdast"
    );

    Ok(())
}

#[test]
fn literal_underlines_join_the_following_setext_heading() {
    use core_markdown::{render_events, Document};
    for (source, expected) in [
        ("a\n=\n=\na\n=", "<h1>a</h1>\n<h1>=\na</h1>"),
        ("a\n=\n=\nb\n\nc\n=", "<h1>a</h1>\n<p>=\nb</p>\n<h1>c</h1>"),
        (
            "a\n=\n=\na\n=\n=\nb\n=",
            "<h1>a</h1>\n<h1>=\na</h1>\n<h1>=\nb</h1>",
        ),
    ] {
        for options in [Options::default(), Options::gfm()] {
            // A definition forces the CommonMark compatibility block producer too.
            for (input, suffix) in [
                (source.to_string(), ""),
                (format!("{source}\n\n[id]: /url"), "\n"),
            ] {
                let expected = format!("{expected}{suffix}");
                assert_eq!(to_html_with_options(&input, &options).unwrap(), expected);
                let document = Document::parse(input.as_str(), &options.parse).unwrap();
                let mut output = String::new();
                render_events(&input, document.events(), &options.compile, &mut output).unwrap();
                assert_eq!(output, expected);
            }
        }
    }
}

#[test]
fn setext_underlines_accept_trailing_spaces_and_tabs() {
    for (underline, depth) in [("=== ", 1), ("===\t", 1), ("--- \t", 2)] {
        let source = format!("heading\n{underline}\n");
        let tree = core_markdown::Document::parse(&source, &core_markdown::ParseOptions::default())
            .unwrap();
        assert!(
            matches!(&tree.node(tree.children(tree.root()).next().unwrap()).unwrap().kind, core_markdown::stream::EventKind::Start(core_markdown::stream::Tag::Heading(level)) if *level == depth)
        );
        assert_eq!(
            core_markdown::to_html(&source),
            format!("<h{depth}>heading</h{depth}>\n")
        );
    }
}
