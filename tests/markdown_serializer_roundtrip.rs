// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use base_markdown::markdown::{
    to_markdown as to, to_markdown_with_options as to_md_with_opts, Options,
};
mod support;
use base_markdown::{Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::canonical;

#[test]
fn normalizes_unordered_list_markers_by_default() {
    let document = "* first\n* second\n";
    let tree = Document::parse(document, &Default::default()).unwrap();

    assert_eq!(to(&tree).unwrap(), "- first\n- second\n");
}

#[test]
fn roundtrip() {
    let doc: String = document(vec![
        "> - Lorem ipsum dolor sit amet",
        ">",
        "> - consectetur adipisicing elit",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "- Lorem ipsum dolor sit amet",
        "",
        "  1. consectetur adipisicing elit",
        "",
        "  2. sed do eiusmod tempor incididunt",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "- 1. Lorem ipsum dolor sit amet",
        "",
        "  2. consectetur adipisicing elit",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "- hello",
        "  - world",
        "    how",
        "",
        "    are",
        "    you",
        "",
        "  - today",
        "- hi",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "An autolink: <http://example.com/?foo=1&bar=2>.\n".to_string();

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "A [primary][toString], [secondary][constructor], and [tertiary][__proto__] link.",
        "",
        "[toString]: http://primary.com",
        "",
        "[__proto__]: http://tertiary.com",
        "",
        "[constructor]: http://secondary.com",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "- foo",
        "",
        "-",
        "",
        "- bar",
        "",
        "- baz",
        "",
        "-",
        "",
        "- qux quux",
        "",
    ]);

    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "- a\n\n<!---->\n\n- b\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = document(vec![
        "    <h3>Header 3</h3>",
        "",
        "    <blockquote>",
        "        <p>This is a blockquote.</p>",
        "        ",
        "        <p>This is the second paragraph in the blockquote.</p>",
        "        ",
        "        <h2>This is an H2 in a blockquote</h2>",
        "    </blockquote>",
        "",
    ]);

    assert_eq!(
        to_md_with_opts(
            &Document::parse(&doc, &Default::default()).unwrap(),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        doc
    );

    let doc: String = "> a\n\n> b\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "[**https://unifiedjs.com/**](https://unifiedjs.com/)\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let step1 = "\\ \\\\ \\\\\\ \\\\\\\\";
    let step2 = "\\ \\ \\\\\\ \\\\\\\\\n";
    assert_eq!(
        to(&Document::parse(step1, &Default::default()).unwrap()).unwrap(),
        step2
    );
    assert_eq!(
        to(&Document::parse(step2, &Default::default()).unwrap()).unwrap(),
        step2
    );

    let doc = "\\\\\\*a\n";
    assert_eq!(
        to(&Document::parse(doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc = "\\\\*a\\\\\\*";
    assert_eq!(
        semantics(&Document::parse(doc, &Default::default()).unwrap()),
        semantics(
            &Document::parse(
                &to(&Document::parse(doc, &Default::default()).unwrap()).unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let doc = "```\n	\n```\n";
    assert_eq!(
        to(&Document::parse(doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc = "- - *\n";
    assert_eq!(
        to(&Document::parse(doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc = "- ***\n";
    assert_eq!(
        to(&Document::parse(doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let tree = Document::parse("* a\n- b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("* ---\n- - +\n+ b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("- - +\n* ---\n+ b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("- - +\n- -", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("* - +\n    *\n    -\n    +", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("- +\n- *\n  -\n  +", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(
            &Document::parse(
                &to_md_with_opts(
                    &tree,
                    &Options {
                        bullet: '*',
                        bullet_other: '-',
                        ..Default::default()
                    }
                )
                .unwrap(),
                &Default::default()
            )
            .unwrap()
        )
    );

    let tree = Document::parse("1. a\n1) b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree = Document::parse("1. ---\n1) 1. 1)\n1. b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree = Document::parse("1. 1. 1)\n1) ---\n1. b", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree = Document::parse("1. 1. 1)\n1. 1.", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree =
        Document::parse("1. 1) 1.\n      1.\n      1)\n    1.", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree =
        Document::parse("1. 1) 1.\n   1) 1.\n     1)\n     1.", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let tree = Document::parse("1. 1)\n1. 1.\n   1)\n   1.", &Default::default()).unwrap();
    assert_eq!(
        semantics(&tree),
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap())
    );

    let doc: String = "&#x20;\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "&#x9;\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "&#x20; a &#x20;\n&#x9;\tb\t&#x9;\n".to_string();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = "Separate paragraphs:

a * is this emphasis? *

a ** is this emphasis? **

a *** is this emphasis? ***

a *\\* is this emphasis? *\\*

a \\** is this emphasis? \\**

a **\\* is this emphasis? **\\*

a *\\** is this emphasis? *\\**

One paragraph:

a * is this emphasis? *
a ** is this emphasis? **
a *** is this emphasis? ***
a *\\* is this emphasis? *\\*
a \\** is this emphasis? \\**
a **\\* is this emphasis? **\\*
a *\\** is this emphasis? *\\**"
        .to_string();
    let tree = Document::parse(&doc, &Default::default()).unwrap();
    assert_eq!(
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap()),
        semantics(&tree),
    );

    let doc: String = "Separate paragraphs:

a _ is this emphasis? _

a __ is this emphasis? __

a ___ is this emphasis? ___

a _\\_ is this emphasis? _\\_

a \\__ is this emphasis? \\__

a __\\_ is this emphasis? __\\_

a _\\__ is this emphasis? _\\__

One paragraph:

a _ is this emphasis? _
a __ is this emphasis? __
a ___ is this emphasis? ___
a _\\_ is this emphasis? _\\_
a \\__ is this emphasis? \\__
a __\\_ is this emphasis? __\\_
a _\\__ is this emphasis? _\\__"
        .to_string();
    let tree = Document::parse(&doc, &Default::default()).unwrap();
    assert_eq!(
        semantics(&Document::parse(&to(&tree).unwrap(), &Default::default()).unwrap()),
        semantics(&tree),
    );

    let doc: String = to(&Document::parse("(____", &Default::default()).unwrap()).unwrap();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );

    let doc: String = to(&Document::parse(
        "Once activated, a service worker ______, then transitions to idle…",
        &Default::default(),
    )
    .unwrap())
    .unwrap();
    assert_eq!(
        to(&Document::parse(&doc, &Default::default()).unwrap()).unwrap(),
        doc
    );
}

#[test]
fn angle_bracket_autolinks_roundtrip() {
    for (input, expected) in [
        ("Option<std::io::Error>\n", "Option\\<std::io::Error>\n"),
        ("<xs:element>\n", "\\<xs:element>\n"),
        ("a <xs:element>\n", "a \\<xs:element>\n"),
        ("<irc://foo.bar>\n", "<irc://foo.bar>\n"),
        ("<irc:foo.bar>\n", "\\<irc:foo.bar>\n"),
        ("<httpx://a>\n", "\\<httpx://a>\n"),
        ("<https://a.b>\n", "<https://a.b>\n"),
        ("<HTTP://A>\n", "<HTTP://A>\n"),
        ("<MAILTO:a@b.c>\n", "<MAILTO:a@b.c>\n"),
        ("<a@b.co>\n", "<a@b.co>\n"),
        ("[a](mailto:a)\n", "[a](mailto:a)\n"),
        ("<tel:123>\n", "<tel:123>\n"),
        ("[tel:abc](tel:abc)\n", "[tel:abc](tel:abc)\n"),
        ("<http://a\\b>\n", "<http://a\\b>\n"),
        ("<http://a\\>\n", "<http://a\\>\n"),
        ("<http://a\\\\>\n", "<http://a\\\\>\n"),
        (
            "<http://a*b*_c_`d`[e]&amp;f>\n",
            "<http://a*b*_c_`d`[e]&amp;f>\n",
        ),
        ("<a#$%&*+=^_`{}~@b.c>\n", "<a#$%&*+=^_`{}~@b.c>\n"),
        ("<http://a|b>\n", "<http://a|b>\n"),
        ("<http:>\n", "\\<http:>\n"),
        ("<mailto:>\n", "\\<mailto:>\n"),
    ] {
        for options in [ParseOptions::default(), ParseOptions::gfm()] {
            let tree = Document::parse(input, &options).unwrap();
            let first = to(&tree).unwrap();
            assert_eq!(first, expected, "{:?}", input);
            let reparsed = Document::parse(&first, &options).unwrap();
            assert_eq!(semantics(&reparsed), semantics(&tree), "{:?}", input);
            assert_eq!(to(&reparsed).unwrap(), first, "{:?}", input);
        }
    }
}

#[test]
fn text_angle_brackets_escape_only_before_email_autolinks() {
    let namespaces = || ParseOptions {
        html_tag_namespaces: true,
        ..Default::default()
    };
    // Inputs and their CommonMark serialization, where one is fixed. Every input must keep its tree and serialize stably in each mode.
    // Only the rest of the same text node is scanned, so an email the parser splits across text
    // nodes, such as `<#\@b.c>`, `\<&@a.b>` or, with GFM, `\<#@a.b>`, is written unescaped, as on main.
    for (input, expected) in [
        // The `@` is escaped, since the email is a literal autolink with GFM, and that keeps the
        // email autolink from forming too.
        ("\\<1@a.b>\n", Some("<1\\@a.b>\n")),
        // Escaping `*` already breaks the email autolink, so `<` stays unescaped.
        ("\\<*@a.b>\n", Some("<\\*@a.b>\n")),
        ("a <= b\n", Some("a <= b\n")),
        ("<0.5%\n", Some("<0.5%\n")),
        ("<= 40\n", Some("<= 40\n")),
        ("-> Poll<...>\n", Some("-> Poll<...>\n")),
        ("-> Poll<T>\n", Some("-> Poll<T>\n")),
        ("<0.0001\n", Some("<0.0001\n")),
        ("a <1.b> c\n", Some("a <1.b> c\n")),
        ("<&x\n", Some("<\\&x\n")),
        ("a <&#1 b\n", Some("a <\\&#1 b\n")),
        ("<&amp;#|\\@b-c.d>\n", Some("<\\&#|@b-c.d>\n")),
        ("# <#\\:\n", Some("# <#:\n")),
        ("\\<1@b.c\n", Some("<1\\@b.c\n")),
    ] {
        for options in [ParseOptions::default(), ParseOptions::gfm(), namespaces()] {
            let tree = Document::parse(input, &options).unwrap();
            let first = to(&tree).unwrap();
            if let (Some(expected), false) = (expected, options.constructs.autolink_literal) {
                assert_eq!(first, expected, "{:?}", input);
            }
            let reparsed = Document::parse(&first, &options).unwrap();
            assert_eq!(semantics(&reparsed), semantics(&tree), "{:?}", input);
            assert_eq!(to(&reparsed).unwrap(), first, "{:?}", input);
        }
    }
}

#[test]
fn text_angle_bracket_email_scans_stay_linear() {
    // Each scan stops before the next `<`, so doubling the input roughly doubles the time.
    let time = |count: usize| {
        let input = "<1.2-3 ".repeat(count) + "x\n";
        let tree = Document::parse(&input, &Default::default()).unwrap();
        let started = std::time::Instant::now();
        let output = to(&tree).unwrap();
        assert_eq!(output, input);
        started.elapsed()
    };
    time(1 << 10);
    let small = time(1 << 13);
    let large = time(1 << 15);
    assert!(
        large < small * 16,
        "4x input took {:?} against {:?}",
        large,
        small
    );
}

#[test]
fn gfm_autolinks_with_pipes_outside_tables_roundtrip() {
    // A `|` only splits table cells, so these keep the form they were written in outside tables.
    for (input, expected) in [
        ("<http://a|`>\n", "<http://a|`>\n"),
        ("<www.`|@c>\n", "<www.`|@c>\n"),
        ("http://a$|\n", "http://a$|\n"),
    ] {
        let tree = Document::parse(input, &ParseOptions::gfm()).unwrap();
        let first = to(&tree).unwrap();
        assert_eq!(first, expected, "{:?}", input);
        let reparsed = Document::parse(&first, &ParseOptions::gfm()).unwrap();
        assert_eq!(semantics(&reparsed), semantics(&tree), "{:?}", input);
        assert_eq!(to(&reparsed).unwrap(), first, "{:?}", input);
    }
}

fn semantics<S: AsRef<str>>(document: &Document<S>) -> Vec<String> {
    canonical(document)
}

fn document(doc: Vec<&str>) -> String {
    doc.join("\n")
}

#[test]
fn text_angle_brackets_ignore_following_nodes() {
    // Emphasis or code after `<` is another node, so it never makes the `<` an email autolink.
    for (input, expected) in [
        ("[http://a<*b*](u)\n", "[http://a<*b*](u)\n"),
        ("[http://a<1`c`](u)\n", "[http://a<1`c`](u)\n"),
        ("[www.a.b<=*c*](u)\n", "[www.a.b<=*c*](u)\n"),
        // Accepted escapes keep the applied list sorted for later scans.
        (
            "\\<1@b.c> \\* \\* \\* \\* \\* \\* \\* \\<2\\*@d.e>\n",
            "<1\\@b.c> \\* \\* \\* \\* \\* \\* \\* <2\\*@d.e>\n",
        ),
    ] {
        for (options, gfm) in [
            (ParseOptions::default(), false),
            (ParseOptions::gfm(), true),
        ] {
            let tree = Document::parse(input, &options).unwrap();
            let first = to(&tree).unwrap();
            // GFM turns the bare email into a literal autolink, which serializes differently.
            if input.starts_with('[') || !gfm {
                assert_eq!(first, expected, "{:?}", input);
            }
            let reparsed = Document::parse(&first, &options).unwrap();
            assert_eq!(canonical(&reparsed), canonical(&tree), "{:?}", input);
            assert_eq!(to(&reparsed).unwrap(), first, "{:?}", input);
        }
    }
}

#[test]
fn text_angle_brackets_before_inline_code_stay_stable_with_gfm() {
    use support::fixture::{to_markdown, InlineCode, Node, Paragraph, Text};
    for value in ["www.a.b<&", "http://a<1", "www.a.b<="] {
        let first = to_markdown(&Node::Paragraph(Paragraph {
            children: vec![
                Node::Text(Text {
                    value: value.into(),
                    position: None,
                }),
                Node::InlineCode(InlineCode {
                    value: "c".into(),
                    position: None,
                    lang: None,
                }),
            ],
            position: None,
        }))
        .unwrap();
        assert!(!first.contains("\\<"), "{:?}", first);
        // The text is literal, so it is escaped where GFM would read a literal autolink, and
        // reads back as the same text once normalized.
        let mut reparsed = Document::parse(&first, &ParseOptions::gfm()).unwrap();
        reparsed.normalize();
        let second = to(&reparsed).unwrap();
        assert_eq!(second, first, "{:?}", value);
        assert_eq!(
            canonical(&reparsed)[2],
            format!("text:{:?}", value),
            "{:?}",
            value
        );
    }
}

#[test]
fn text_angle_brackets_scan_only_their_own_text() {
    use support::fixture::{to_markdown, Node, Paragraph, Text};
    // Parsed text usually ends at an unescaped `<`, so these shapes come from hand-built trees.
    for (value, expected) in [
        ("<1@b.c>", "<1\\@b.c>\n"),
        ("<1 x", "<1 x\n"),
        ("<*@a.b>", "<\\*@a.b>\n"),
        ("<1@b.c", "<1\\@b.c\n"),
    ] {
        let first = to_markdown(&Node::Paragraph(Paragraph {
            children: vec![Node::Text(Text {
                value: value.into(),
                position: None,
            })],
            position: None,
        }))
        .unwrap();
        assert_eq!(first, expected, "{:?}", value);
        let reparsed = Document::parse(&first, &Default::default()).unwrap();
        assert_eq!(to(&reparsed).unwrap(), first, "{:?}", value);
    }
}
