// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{to_markdown, Options};
use base_markdown::stream::EventKind;
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

/// The language and decoded info string of each code and math block in `document`, and the metadata the serializer writes after the language: the rest of the info, trimmed.
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
                Some(language) => info
                    .strip_prefix(language.as_str())
                    .unwrap_or("")
                    .trim_matches([' ', '\t'])
                    .to_string(),
                None => info.clone(),
            };
            Some((language, info, meta))
        })
        .collect()
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
