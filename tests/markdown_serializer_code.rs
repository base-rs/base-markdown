// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{to_markdown, to_markdown_with_options, Options};
use base_markdown::stream::{EventKind, Tag};
use base_markdown::{Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{canonical, Code, Node};
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};

#[test]
fn text() {
    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                fence: None,
                value: String::from("a"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "    a\n",
        "should support code w/ a value (indent)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("a"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n```\n",
        "should support code w/ a value (fences)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\n```\n",
        "should support code w/ a lang"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: None,
            meta: Some("a".to_string())
        }))
        .unwrap(),
        "```\n```\n",
        "should support (ignore) code w/ only a meta"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a".to_string()),
            meta: Some("b".to_string())
        }))
        .unwrap(),
        "```a b\n```\n",
        "should support code w/ lang and meta"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a b\n```\n",
        "should write a space in `lang` as it is: the info string is one value, whose first word is the language (#188)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a\nb".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a&#xA;b\n```\n",
        "should encode a line ending in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a`b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a&#x60;b\n```\n",
        "should encode a grave accent in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("a\\-b".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\\\\-b\n```\n",
        "should escape a backslash in `lang`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a b".to_string())
        }))
        .unwrap(),
        "```x a b\n```\n",
        "should not encode a space in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a\nb".to_string())
        }))
        .unwrap(),
        "```x a&#xA;b\n```\n",
        "should encode a line ending in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a`b".to_string())
        }))
        .unwrap(),
        "```x a&#x60;b\n```\n",
        "should encode a grave accent in `meta`"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("x".to_string()),
            meta: Some("a\\-b".to_string())
        }))
        .unwrap(),
        "```x a\\\\-b\n```\n",
        "should escape a backslash in `meta`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                fence: None,
                value: String::new(),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~\n~~~\n",
        "should support fenced code w/ tildes when `fence: \"~\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                fence: None,
                value: String::new(),
                position: None,
                lang: Some("a`b".to_string()),
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~a`b\n~~~\n",
        "should not encode a grave accent when using tildes for fences"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("```\nasd\n```"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "````\n```\nasd\n```\n````\n",
        "should use more grave accents for fences if there are streaks of grave accents in the value (fences)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                fence: None,
                value: String::from("~~~\nasd\n~~~"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fence: '~',
                ..Default::default()
            }
        )
        .unwrap(),
        "~~~~\n~~~\nasd\n~~~\n~~~~\n",
        "should use more tildes for fences if there are streaks of tildes in the value (fences)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("b"),
            position: None,
            lang: Some("a".to_string()),
            meta: None
        }))
        .unwrap(),
        "```a\nb\n```\n",
        "should use a fence if there is an info"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from(" "),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n \n```\n",
        "should use a fence if there is only whitespace"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("\na"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n\na\n```\n",
        "should use a fence if there first line is blank (void)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from(" \na"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\n \na\n```\n",
        "should use a fence if there first line is blank (filled)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("a\n"),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n\n```\n",
        "should use a fence if there last line is blank (void)"
    );

    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::from("a\n "),
            position: None,
            lang: None,
            meta: None
        }))
        .unwrap(),
        "```\na\n \n```\n",
        "should use a fence if there last line is blank (filled)"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Code(Code {
                fence: None,
                value: String::from("  a\n\n b"),
                position: None,
                lang: None,
                meta: None
            }),
            &Options {
                fences: false,
                ..Default::default()
            }
        )
        .unwrap(),
        "      a\n\n     b\n",
        "should use an indent if the value is indented"
    );
}

/// The stored value of each code block in `document`, with its terminal line ending.
fn block_values(document: &Document<String>) -> Vec<String> {
    document
        .events()
        .filter_map(|event| match event.unwrap().kind {
            EventKind::CodeBlock { value, .. } => {
                Some(value.resolve(document.source()).unwrap().to_string())
            }
            _ => None,
        })
        .collect()
}

/// Parses `source`, normalizes and serializes it as `expected`, which reads back as the same tree and serializes the same.
fn assert_round_trip(source: &str, options: &ParseOptions, expected: &str) {
    let mut document = Document::parse(source.to_string(), options).unwrap();
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected, "{source:?}");
    let mut reparsed = Document::parse(markdown.clone(), options).unwrap();
    reparsed.normalize();
    assert_eq!(
        block_values(&reparsed),
        block_values(&document),
        "{source:?}"
    );
    assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{source:?}");
}

#[test]
fn fenced_blank_lines_round_trip() {
    let options = ParseOptions::default();
    for ending in ["\n", "\r\n", "\r"] {
        for (source, expected) in [
            ("~~~x{n}{n}~~~", "```x\n\n```\n"),
            ("~~~{n}{n}~~~", "```\n\n```\n"),
            ("~~~{n}{n}", "```\n\n```\n"),
            ("p{n}~~~x{n}{n}~~~", "p\n\n```x\n\n```\n"),
            ("- ~~~{n}{n}  ~~~", "- ```\n\n  ```\n"),
            ("- ~~~{n}  {n}  ~~~", "- ```\n\n  ```\n"),
            ("> ~~~{n}>{n}> ~~~", "> ```\n>\n> ```\n"),
            ("~~~{n}~~~", "```\n```\n"),
            ("~~~{n}{n}{n}~~~", "```\n\n\n```\n"),
            ("~~~{n}  {n}~~~", "```\n  \n```\n"),
            ("~~~{n}a{n}{n}~~~", "```\na\n\n```\n"),
        ] {
            assert_round_trip(&source.replace("{n}", ending), &options, expected);
        }
    }
}

/// The info string of each code block in `document`.
fn infos<S: AsRef<str>>(document: &Document<S>) -> Vec<Option<String>> {
    let source = document.source();
    document
        .descendants(document.root())
        .filter_map(|id| {
            let EventKind::CodeBlock { info, .. } = &document.node(id).unwrap().kind else {
                return None;
            };
            Some(
                info.as_ref()
                    .map(|info| info.resolve(source).unwrap().to_string()),
            )
        })
        .collect()
}

/// Parses `source`, normalizes and serializes it as `expected`, which reads back with the same
/// info strings and serializes the same.
fn assert_info_round_trip(source: &str, options: &ParseOptions, expected: &str) {
    let mut document = Document::parse(source.to_string(), options).unwrap();
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected, "{source:?}");
    let mut reparsed = Document::parse(markdown.clone(), options).unwrap();
    reparsed.normalize();
    assert_eq!(infos(&reparsed), infos(&document), "{source:?}");
    assert_eq!(canonical(&reparsed), canonical(&document), "{source:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{source:?}");
}

#[test]
fn fence_info_round_trip() {
    let options = ParseOptions::default();
    for (source, expected) in [
        // A decoded `&` that would start a reference is escaped (#131).
        ("a\n\n```x \\&amp;\n```", "a\n\n```x \\&amp;\n```\n"),
        ("```x \\&#x9;\n```", "```x \\&#x9;\n```\n"),
        ("```\\&amp; m\n```", "```\\&amp; m\n```\n"),
        (
            "~~~&amp;amp; m &amp;#x9;\n~~~",
            "```\\&amp; m \\&#x9;\n```\n",
        ),
        ("```&a &b\n```", "```\\&a \\&b\n```\n"),
        ("```a & b\n```", "```a & b\n```\n"),
        // Backslashes before punctuation are escaped.
        ("```a\\\\&amp;b c\\\\!\n```", "```a\\\\\\&b c\\\\!\n```\n"),
        // Backticks cannot be written in the info of a backtick fence.
        ("~~~a`b c`d\n~~~", "```a&#x60;b c&#x60;d\n```\n"),
        // Decoded whitespace is whitespace: it separates the words of the info string (#188).
        ("```a&#x20;b&#x9;c d\n```", "```a b\tc d\n```\n"),
        // Line endings are encoded wherever they are.
        ("```a&#xA;b c&#xD;d\n```", "```a&#xA;b c&#xD;d\n```\n"),
        // The spaces and tabs between words are kept (#188).
        ("```x  a\t b\n```", "```x  a\t b\n```\n"),
        // Encoded whitespace at the edges is kept, and then the first word is empty.
        ("```&#x20;x\n```", "```&#x20;x\n```\n"),
    ] {
        assert_info_round_trip(source, &options, expected);
    }
}

#[test]
fn fence_info_references() {
    assert_eq!(
        to(&Node::Code(Code {
            fence: None,
            value: String::new(),
            position: None,
            lang: Some("&amp;".to_string()),
            meta: Some("&#x9; & \\&".to_string())
        }))
        .unwrap(),
        "```\\&amp; \\&#x9; & \\\\&\n```\n",
        "should escape `&` before a reference in the info of code"
    );
}

/// Serializes `document` with `options` as `expected`, which reads back with the same info
/// strings and serializes the same.
fn assert_written<S: AsRef<str>>(document: &Document<S>, options: &Options, expected: &str) {
    let markdown = to_markdown_with_options(document, options).unwrap();
    assert_eq!(markdown, expected);
    let reparsed = Document::parse(markdown.clone(), &ParseOptions::default()).unwrap();
    assert_eq!(infos(&reparsed), infos(document), "{:?}", markdown);
    assert_eq!(
        to_markdown_with_options(&reparsed, options).unwrap(),
        markdown
    );
}

/// Hand-built code with info string `info`.
fn hand_built(info: &str) -> Document<String> {
    use base_markdown::stream::Text;
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    document
        .append_child(
            document.root(),
            EventKind::CodeBlock {
                value: Text::Literal(""),
                info: Some(Text::from(info.to_string())),
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    document
}

/// The spaces and tabs at the edges of an info string are encoded so they read back, and those
/// between its words are written as they are (#144, #188).
#[test]
fn code_info_edges_round_trip() {
    let options = Options::default();
    for (info, expected) in [
        ("x  \ta\t ", "```x  \ta\t&#x20;\n```\n"),
        ("x a ", "```x a&#x20;\n```\n"),
        ("x\ta\t", "```x\ta&#x9;\n```\n"),
        ("x a\n", "```x a&#xA;\n```\n"),
        ("x   ", "```x  &#x20;\n```\n"),
        ("x \t \t", "```x \t &#x9;\n```\n"),
        ("x ", "```x&#x20;\n```\n"),
        ("x", "```x\n```\n"),
        (" ", "```&#x20;\n```\n"),
        ("\t", "```&#x9;\n```\n"),
        ("  a ", "```&#x20; a&#x20;\n```\n"),
        ("\ta", "```&#x9;a\n```\n"),
        ("a b c", "```a b c\n```\n"),
    ] {
        assert_written(&hand_built(info), &options, expected);
    }
    // Parsed info strings whose edges come from character references.
    let parse = ParseOptions::default();
    for (source, expected) in [
        ("```x &#x20;a&#x20;\n```", "```x  a&#x20;\n```\n"),
        ("```x &#x9;&#x20;\n```", "```x \t&#x20;\n```\n"),
        ("```x &#x20;\n```", "```x &#x20;\n```\n"),
        ("```x  a  \n```", "```x  a\n```\n"),
        ("```x\t a\n```", "```x\t a\n```\n"),
    ] {
        let document = Document::parse(source.to_string(), &parse).unwrap();
        assert_written(&document, &options, expected);
    }
    // The parser stores NUL as U+FFFD, so an info string with NUL is rejected (#257).
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    assert!(matches!(
        document.append_child(
            document.root(),
            EventKind::CodeBlock {
                value: base_markdown::stream::Text::Literal(""),
                info: Some(base_markdown::stream::Text::Literal("a\0 b")),
                fence: None,
                unclosed: false,
            },
        ),
        Err(base_markdown::stream::StreamError::InvalidValue)
    ));
    // With character references off, the info string is raw, and is written for a parser with
    // references.
    let mut raw = ParseOptions::default();
    raw.constructs.character_reference = false;
    let document = Document::parse("```x&amp; y\n```".to_string(), &raw).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "```x\\&amp; y\n```\n");
}

/// The language is the first word of the decoded info string, up to a space, tab or line ending,
/// and code whose info string starts with one has no language (#188).
#[test]
fn code_language_is_the_first_word_of_the_info_string() {
    use base_markdown::stream::code_language;
    for (info, language) in [
        ("x", Some("x")),
        ("x y", Some("x")),
        ("x\ty", Some("x")),
        ("x\ny", Some("x")),
        ("x\ry", Some("x")),
        ("x\u{c}y", Some("x\u{c}y")),
        ("x\u{a0}y", Some("x\u{a0}y")),
        (" x", None),
        ("\nx", None),
        ("", None),
    ] {
        assert_eq!(code_language(info), language, "{info:?}");
    }
    for (source, html) in [
        (
            "```a&#x20;b c\n```\n",
            "<pre><code class=\"language-a\"></code></pre>\n",
        ),
        (
            "```a&#9;b\n```\n",
            "<pre><code class=\"language-a\"></code></pre>\n",
        ),
        (
            "```a&#10;b\n```\n",
            "<pre><code class=\"language-a\"></code></pre>\n",
        ),
        ("```&#x20;b\n```\n", "<pre><code></code></pre>\n"),
        (
            "```a\\ b\n```\n",
            "<pre><code class=\"language-a\\\"></code></pre>\n",
        ),
    ] {
        assert_eq!(base_markdown::to_html(source), html, "{source:?}");
    }
    // Code whose info string has no language is still fenced, so its info string reads back.
    let options = Options {
        fences: false,
        ..Default::default()
    };
    let mut document = hand_built(" x");
    let code = document.children(document.root()).next().unwrap();
    document
        .set_kind(
            code,
            EventKind::CodeBlock {
                value: base_markdown::stream::Text::Literal("v\n"),
                info: Some(base_markdown::stream::Text::Literal(" x")),
                fence: None,
                unclosed: false,
            },
        )
        .unwrap();
    assert_written(&document, &options, "```&#x20;x\nv\n```\n");
    // So no `<!---->` is written before it after a list or indented code, as it is fenced.
    for (source, expected) in [
        ("- a\n\n```x\nv\n```\n", "- a\n\n```&#x20;x\nv\n```\n"),
        ("    i\n\n```x\nv\n```\n", "    i\n\n```&#x20;x\nv\n```\n"),
    ] {
        let mut document = Document::parse(source.to_string(), &ParseOptions::default()).unwrap();
        let code = document
            .children(document.root())
            .filter(|&id| matches!(document.node(id).unwrap().kind, EventKind::CodeBlock { .. }))
            .last()
            .unwrap();
        document
            .set_kind(
                code,
                EventKind::CodeBlock {
                    value: base_markdown::stream::Text::Literal("v\n"),
                    info: Some(base_markdown::stream::Text::Literal(" x")),
                    fence: None,
                    unclosed: false,
                },
            )
            .unwrap();
        assert_written(&document, &options, expected);
    }
}

/// A `~` at the start of the info string of a tilde fence is escaped, as it would lengthen the
/// fence (#144).
#[test]
fn tilde_at_the_start_of_a_tilde_fence_info_is_escaped() {
    let tilde = Options {
        fence: '~',
        ..Default::default()
    };
    assert_written(&hand_built("~x a"), &tilde, "~~~\\~x a\n~~~\n");
    assert_written(&hand_built("~~ ~"), &tilde, "~~~\\~~ ~\n~~~\n");
    assert_written(&hand_built("x~"), &tilde, "~~~x~\n~~~\n");
    let document = Document::parse("``` \\~x\n```".to_string(), &ParseOptions::default()).unwrap();
    assert_written(&document, &tilde, "~~~\\~x\n~~~\n");
    assert_written(&document, &Options::default(), "```~x\n```\n");
}

/// Random fences, info strings and containers, parsed with several options: every parsed code
/// block passes the checks a hand-built one does (#144), its info string can be assigned again,
/// and it reads back with the same info string, with escapes and references on (#188).
#[test]
fn parsed_code_blocks_are_valid_and_read_back() {
    use base_markdown::stream::Text;
    const PIECES: &[&str] = &[
        "x", "a", " ", "  ", "\t", "&#x20;", "&#x9;", "&#xA;", "&amp;", "\\~", "~", "`", "\\`",
        "\\\\", "\\", "&", "$", "\u{c}", "é", "\0", "&#0;", "&#x60;", "&#126;", "\\*", "&#13;",
    ];
    const OPENERS: &[&str] = &["```", "~~~", "````", "~~~~"];
    const PREFIXES: &[&str] = &["", "> ", "- ", "1. ", " ", "\n"];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    let mut references = ParseOptions::default();
    references.constructs.character_reference = false;
    let mut escapes = ParseOptions::default();
    escapes.constructs.character_escape = false;
    let mut neither = ParseOptions::default();
    neither.constructs.character_escape = false;
    neither.constructs.character_reference = false;
    // The serializer writes for a parser with escapes and references, so every block reads back
    // with the default options.
    let parses = [
        ParseOptions::default(),
        ParseOptions::gfm(),
        references,
        escapes,
        neither,
    ];
    let (mut blocks, mut written) = (0, 0);
    for _ in 0..3000 {
        let mut source = String::from(PREFIXES[next(PREFIXES.len())]);
        source.push_str(OPENERS[next(OPENERS.len())]);
        for _ in 0..next(7) {
            source.push_str(PIECES[next(PIECES.len())]);
        }
        source.push_str("\nv\n");
        source.push_str(OPENERS[next(OPENERS.len())]);
        for parse in &parses {
            let document = Document::parse(source.clone(), parse).unwrap();
            let mut copy = Document::parse(String::new(), parse).unwrap();
            for id in document.descendants(document.root()) {
                let EventKind::CodeBlock { info, .. } = &document.node(id).unwrap().kind else {
                    continue;
                };
                blocks += 1;
                let own = |text: &Text<'_>| {
                    Text::from(text.resolve(document.source()).unwrap().to_string())
                };
                copy.append_child(
                    copy.root(),
                    EventKind::CodeBlock {
                        value: Text::Literal(""),
                        info: info.as_ref().map(own),
                        fence: None,
                        unclosed: false,
                    },
                )
                .unwrap_or_else(|error| panic!("{:?}: {:?}", source, error));
            }
            // Text such as a `~` after a fence that did not open is not what
            // this checks.
            if document.descendants(document.root()).any(|id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Start(Tag::Paragraph { .. })
                )
            }) {
                continue;
            }
            written += 1;
            let markdown = to_markdown(&document).unwrap();
            let reparsed = Document::parse(markdown.clone(), &ParseOptions::default()).unwrap();
            assert_eq!(
                infos(&reparsed),
                infos(&document),
                "{:?} -> {:?}",
                source,
                markdown
            );
            assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{:?}", source);
        }
    }
    assert!(blocks > 10_000 && written > 3000, "{} {}", blocks, written);
}
