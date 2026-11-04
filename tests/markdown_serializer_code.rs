// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{to_markdown, to_markdown_with_options, Options};
use base_markdown::stream::{EventKind, Tag};
use base_markdown::{Document, NodeSyntax, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{canonical, Code, Math, Node};
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
        "```a&#x20;b\n```\n",
        "should encode a space in `lang`"
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

/// The stored value of each code and math block in `document`, with its terminal line ending.
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

#[test]
fn math_blank_lines_round_trip() {
    let mut options = ParseOptions::default();
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    for ending in ["\n", "\r\n", "\r"] {
        for (source, expected) in [
            ("$${n}{n}$$", "$$\n\n$$\n"),
            ("p{n}$${n}{n}$$", "p\n\n$$\n\n$$\n"),
            ("- $${n}{n}  $$", "- $$\n\n  $$\n"),
            ("> $${n}>{n}> $$", "> $$\n>\n> $$\n"),
            ("$${n}$$", "$$\n$$\n"),
            ("$${n}{n}{n}$$", "$$\n\n\n$$\n"),
        ] {
            assert_round_trip(&source.replace("{n}", ending), &options, expected);
        }
    }
}

/// The language and decoded info string of each code and math block in `document`, and the metadata the serializer writes after the language: the rest of the info after the spaces and tabs that separate it, or, when there is only whitespace after the language, all of it but the first space or tab.
fn fence_infos(document: &Document<String>) -> Vec<(Option<String>, String, String)> {
    let source = document.source();
    document
        .descendants(document.root())
        .filter_map(|id| {
            let EventKind::CodeBlock { language, .. } = &document.node(id).unwrap().kind else {
                return None;
            };
            let language = language
                .as_ref()
                .map(|language| language.resolve(source).unwrap().to_string());
            let Some(NodeSyntax::Code { info }) = document.syntax(id) else {
                return Some((language, String::new(), String::new()));
            };
            let info = info.resolve(source).unwrap().to_string();
            let meta = match &language {
                Some(language) => code_meta(&info, language),
                None => info.clone(),
            };
            Some((language, info, meta))
        })
        .collect()
}

/// The metadata of code with `info` and `language`, as the serializer writes it.
fn code_meta(info: &str, language: &str) -> String {
    let rest = info.strip_prefix(language).unwrap_or("");
    match rest.trim_start_matches([' ', '\t']) {
        "" => rest.get(1..).unwrap_or("").to_string(),
        metadata => metadata.to_string(),
    }
}

/// Parses `source`, normalizes and serializes it as `expected`, which reads back with the same fence language, info and metadata, and serializes the same.
fn assert_info_round_trip(source: &str, options: &ParseOptions, expected: &str) {
    let mut document = Document::parse(source.to_string(), options).unwrap();
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected, "{source:?}");
    let mut reparsed = Document::parse(markdown.clone(), options).unwrap();
    reparsed.normalize();
    assert_eq!(fence_infos(&reparsed), fence_infos(&document), "{source:?}");
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
        // Whitespace and line endings in the language, and line endings in the metadata.
        ("```a&#x20;b&#x9;c d\n```", "```a&#x20;b&#x9;c d\n```\n"),
        ("```a&#xA;b c&#xD;d\n```", "```a&#xA;b c&#xD;d\n```\n"),
    ] {
        assert_info_round_trip(source, &options, expected);
    }
}

#[test]
fn math_meta_round_trip() {
    let mut options = ParseOptions::default();
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    for (source, expected) in [
        ("$$ \\&amp; x\n$$", "$$\\&amp; x\n$$\n"),
        ("$$&amp;amp;#x9;\n$$", "$$\\&amp;#x9;\n$$\n"),
        ("$$ \\\\&amp; a\\\\!\n$$", "$$\\\\& a\\\\!\n$$\n"),
        // The parser trims the metadata, so whitespace at its edges is encoded.
        ("$$ &#x20;a&#x9;\n$$", "$$&#x20;a&#x9;\n$$\n"),
        ("$$ &#x9; a &#x20;\n$$", "$$&#x9; a &#x20;\n$$\n"),
        ("$$ &#x20;\n$$", "$$&#x20;\n$$\n"),
        ("$$ a&#xA;b\n$$", "$$a&#xA;b\n$$\n"),
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

    assert_eq!(
        to(&Node::Math(Math {
            value: String::new(),
            position: None,
            meta: Some(" &amp;\t".to_string())
        }))
        .unwrap(),
        "$$&#x20;\\&amp;&#x9;\n$$\n",
        "should escape `&` before a reference and encode whitespace at the edges of the meta of math"
    );
}

/// The language and metadata of each code and math block in `document`, which is what reads back:
/// the spaces and tabs between the language and the metadata are written as one space.
fn languages_and_meta(document: &Document<String>) -> Vec<(Option<String>, String)> {
    fence_infos(document)
        .into_iter()
        .map(|(language, _, meta)| (language, meta))
        .collect()
}

/// Serializes `document` with `options` as `expected`, which reads back with the same languages
/// and metadata and serializes the same.
fn assert_meta_round_trip(document: &Document<String>, options: &Options, expected: &str) {
    let markdown = to_markdown_with_options(document, options).unwrap();
    assert_eq!(markdown, expected);
    let mut parse = ParseOptions::default();
    parse.constructs.math_flow = true;
    let reparsed = Document::parse(markdown.clone(), &parse).unwrap();
    assert_eq!(
        languages_and_meta(&reparsed),
        languages_and_meta(document),
        "{:?}",
        markdown
    );
    assert_eq!(canonical(&reparsed), canonical(document), "{:?}", markdown);
    assert_eq!(
        to_markdown_with_options(&reparsed, options).unwrap(),
        markdown
    );
}

/// Hand-built code with `language` and info string `info`.
fn hand_built(language: &str, info: &str) -> Document<String> {
    use base_markdown::stream::Text;
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let code = document
        .append_child(
            document.root(),
            EventKind::CodeBlock {
                value: Text::Literal(""),
                language: Some(Text::from(language.to_string())),
                fence: None,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    document
        .set_syntax(
            code,
            Some(NodeSyntax::Code {
                info: Text::from(info.to_string()),
            }),
        )
        .unwrap();
    document
}

/// Whitespace at the edges of code metadata is encoded so it reads back, and the run of spaces
/// and tabs between the language and the metadata is written as one space (#144).
#[test]
fn code_meta_edges_round_trip() {
    let options = Options::default();
    for (language, info, expected) in [
        ("x", "x  \ta\t ", "```x a\t&#x20;\n```\n"),
        ("x", "x a ", "```x a&#x20;\n```\n"),
        ("x", "x\ta\t", "```x a&#x9;\n```\n"),
        ("x", "x a\n", "```x a&#xA;\n```\n"),
        // Metadata of only whitespace is what follows the first space or tab.
        ("x", "x   ", "```x &#x20;&#x20;\n```\n"),
        ("x", "x \t \t", "```x &#x9; &#x9;\n```\n"),
        ("x", "x  ", "```x &#x20;\n```\n"),
        ("x", "x ", "```x\n```\n"),
        ("x", "x", "```x\n```\n"),
        ("x", "x \t a", "```x a\n```\n"),
        (" ", "  a ", "```&#x20; a&#x20;\n```\n"),
        ("a b", "a b c", "```a&#x20;b c\n```\n"),
    ] {
        assert_meta_round_trip(&hand_built(language, info), &options, expected);
    }
    // Parsed metadata whose edges come from character references.
    let parse = ParseOptions::default();
    for (source, expected) in [
        ("```x &#x20;a&#x20;\n```", "```x a&#x20;\n```\n"),
        ("```x &#x9;&#x20;\n```", "```x &#x9;&#x20;\n```\n"),
        ("```x &#x20;\n```", "```x &#x20;\n```\n"),
        // Real documents are unchanged: two spaces before the metadata are written as one.
        ("```x  a  \n```", "```x a\n```\n"),
        ("```x\t a\n```", "```x a\n```\n"),
    ] {
        let document = Document::parse(source.to_string(), &parse).unwrap();
        assert_meta_round_trip(&document, &options, expected);
    }
    // NUL cannot be written, and is written as U+FFFD, as the parser reads it.
    assert_eq!(
        to_markdown(&hand_built("a\0", "a\0 b\0")).unwrap(),
        "```a\u{FFFD} b\u{FFFD}\n```\n"
    );
    // With character references off, the parsed language is the raw first word while the info
    // string is decoded, so the info does not start with the language and the metadata is not
    // written, as before.
    let mut raw = ParseOptions::default();
    raw.constructs.character_reference = false;
    let document = Document::parse("```x&amp; y\n```".to_string(), &raw).unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "```x\\&amp;\n```\n");
}

/// A `~` at the start of the language of a tilde fence is escaped, as it would lengthen the fence
/// (#144).
#[test]
fn tilde_at_the_start_of_a_tilde_fence_language_is_escaped() {
    let tilde = Options {
        fence: '~',
        ..Default::default()
    };
    assert_meta_round_trip(&hand_built("~x", "~x a"), &tilde, "~~~\\~x a\n~~~\n");
    assert_meta_round_trip(&hand_built("~~", "~~ ~"), &tilde, "~~~\\~~ ~\n~~~\n");
    assert_meta_round_trip(&hand_built("x~", "x~"), &tilde, "~~~x~\n~~~\n");
    let document = Document::parse("``` \\~x\n```".to_string(), &ParseOptions::default()).unwrap();
    assert_meta_round_trip(&document, &tilde, "~~~\\~x\n~~~\n");
    assert_meta_round_trip(&document, &Options::default(), "```~x\n```\n");
}

/// Random fences, info strings and containers, parsed with and without math: every parsed code
/// block passes the checks a hand-built one does (#144), its language and info string can be
/// assigned again, and it reads back with the same language and metadata.
#[test]
fn parsed_code_blocks_are_valid_and_read_back() {
    use base_markdown::stream::Text;
    const PIECES: &[&str] = &[
        "x", "a", " ", "  ", "\t", "&#x20;", "&#x9;", "&#xA;", "&amp;", "\\~", "~", "`", "\\`",
        "\\\\", "\\", "&", "$", "\u{c}", "é", "\0", "&#0;", "&#x60;", "&#126;",
    ];
    const OPENERS: &[&str] = &["```", "~~~", "````", "$$", "~~~~"];
    const PREFIXES: &[&str] = &["", "> ", "- ", "1. ", " ", "\n"];
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    let mut math = ParseOptions::default();
    math.constructs.math_flow = true;
    let parses = [ParseOptions::default(), ParseOptions::gfm(), math];
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
                let EventKind::CodeBlock { language, math, .. } = &document.node(id).unwrap().kind
                else {
                    continue;
                };
                blocks += 1;
                let own = |text: &Text<'_>| {
                    Text::from(text.resolve(document.source()).unwrap().to_string())
                };
                let code = copy
                    .append_child(
                        copy.root(),
                        EventKind::CodeBlock {
                            value: Text::Literal(""),
                            language: language.as_ref().map(own),
                            fence: None,
                            math: *math,
                            unclosed: false,
                        },
                    )
                    .unwrap_or_else(|error| panic!("{:?}: {:?}", source, error));
                if let Some(NodeSyntax::Code { info }) = document.syntax(id) {
                    copy.set_syntax(code, Some(NodeSyntax::Code { info: own(&info) }))
                        .unwrap_or_else(|error| panic!("{:?}: {:?}", source, error));
                }
            }
            // Text such as `$$` without math or a `~` after a fence that did not open is not what
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
            let reparsed = Document::parse(markdown.clone(), parse).unwrap();
            assert_eq!(
                languages_and_meta(&reparsed),
                languages_and_meta(&document),
                "{:?} -> {:?}",
                source,
                markdown
            );
            assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{:?}", source);
        }
    }
    assert!(blocks > 10_000 && written > 3000, "{} {}", blocks, written);
}
