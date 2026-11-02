// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2024, by Nokome Bentley.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{
    message, to_html, to_html_with_options, Constructs, Document, NodeSyntax, Options, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::parse_fixture;
use support::fixture::Position;
use support::fixture::{Frontmatter, Node, Root};

#[test]
fn frontmatter() -> Result<(), message::Message> {
    let frontmatter = Options {
        parse: ParseOptions {
            constructs: Constructs {
                frontmatter: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        to_html_with_options("---\ntitle: Jupyter\n---", &Options::commonmark()),
        "<hr />\n<h2>title: Jupyter</h2>",
        "should not support frontmatter in CommonMark"
    );

    assert_eq!(
        to_html("---\ntitle: Jupyter\n---"),
        "",
        "should support frontmatter by default"
    );

    assert_eq!(
        to_html_with_options("---\ntitle: Jupyter\n---", &frontmatter),
        "",
        "should support frontmatter (yaml)"
    );

    assert_eq!(
        to_html_with_options("+++\ntitle = \"Jupyter\"\n+++", &frontmatter),
        "",
        "should support frontmatter (toml)"
    );

    assert_eq!(
        to_html_with_options("---\n---", &frontmatter),
        "",
        "should support empty frontmatter"
    );

    assert_eq!(
        to_html_with_options("--\n---", &frontmatter),
        "<h2>--</h2>",
        "should not support 2 markers in an opening fence"
    );

    assert_eq!(
        to_html_with_options("----\n---", &frontmatter),
        "<hr />\n<hr />",
        "should not support 4 markers in an opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n--", &frontmatter),
        "<hr />\n<p>--</p>",
        "should not support 2 markers in a closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n--\n", &frontmatter),
        "<hr />\n<p>--</p>\n",
        "should not panic if newline after 2 marker closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n----", &frontmatter),
        "<hr />\n<hr />",
        "should not support 4 markers in a closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\n## Neptune", &frontmatter),
        "<h2>Neptune</h2>",
        "should support content after frontmatter"
    );

    assert_eq!(
        to_html_with_options("--- \t\n---", &frontmatter),
        "",
        "should support spaces and tabs after opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\t ", &frontmatter),
        "",
        "should support spaces and tabs after closing fence"
    );

    assert_eq!(
        to_html_with_options("---\n---\na\nb", &frontmatter),
        "<p>a\nb</p>",
        "should support line endings after frontmatter"
    );

    assert_eq!(
        to_html_with_options("--- a\n---", &frontmatter),
        "",
        "should support a format hint after the opening fence"
    );

    assert_eq!(
        to_html_with_options("---\n--- b", &frontmatter),
        "<hr />\n<p>--- b</p>",
        "should not support content after closing fence"
    );

    assert_eq!(
        to_html_with_options("## Neptune\n---\n---", &frontmatter),
        "<h2>Neptune</h2>\n<hr />\n<hr />",
        "should not support frontmatter after content"
    );

    assert_eq!(
        to_html_with_options("> ---\n> ---\n> ## Neptune", &frontmatter),
        "<blockquote>\n<hr />\n<hr />\n<h2>Neptune</h2>\n</blockquote>",
        "should not support frontmatter in a container"
    );

    assert_eq!(
        to_html_with_options("---", &frontmatter),
        "<hr />",
        "should not support just an opening fence"
    );

    assert_eq!(
        to_html_with_options("---\ntitle: Neptune", &frontmatter),
        "<hr />\n<p>title: Neptune</p>",
        "should not support a missing closing fence"
    );

    assert_eq!(
        to_html_with_options("---\na\n\nb\n \t\nc\n---", &frontmatter),
        "",
        "should support blank lines in frontmatter"
    );

    assert_eq!(
        parse_fixture("---\na: b\n---", &frontmatter.parse)?,
        Node::Root(Root {
            children: vec![Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '-',
                fence_length: 3,
                closing_fence_length: 3,
                value: "a: b\n".into(),
                position: Some(Position::new(1, 1, 0, 3, 4, 12))
            })],
            position: Some(Position::new(1, 1, 0, 3, 4, 12))
        }),
        "should support yaml as `Frontmatter` in mdast"
    );

    assert_eq!(
        parse_fixture("+++\ntitle = \"Jupyter\"\n+++", &frontmatter.parse)?,
        Node::Root(Root {
            children: vec![Node::Frontmatter(Frontmatter {
                info: String::new(),
                fence: '+',
                fence_length: 3,
                closing_fence_length: 3,
                value: "title = \"Jupyter\"\n".into(),
                position: Some(Position::new(1, 1, 0, 3, 4, 25))
            })],
            position: Some(Position::new(1, 1, 0, 3, 4, 25))
        }),
        "should support toml as `Frontmatter` in mdast"
    );

    Ok(())
}

fn parse_options() -> ParseOptions {
    ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    }
}

fn parse_document(source: &str, options: &ParseOptions) -> Document {
    Document::parse(source.to_owned(), options).unwrap()
}

fn parse_frontmatter(source: &str) -> Frontmatter {
    let document = parse_document(source, &parse_options());
    let id = document.children(document.root()).next().unwrap();
    let NodeSyntax::Frontmatter {
        info,
        fence,
        closing_fence_length,
    } = document.syntax(id).expect("frontmatter syntax")
    else {
        panic!("expected frontmatter for {:?}", source);
    };
    Frontmatter {
        value: document.frontmatter_value(id).unwrap().to_owned(),
        info: info.resolve(source).unwrap().to_owned(),
        fence: fence.character,
        fence_length: fence.length,
        closing_fence_length,
        position: None,
    }
}

#[test]
fn delimiter_defaults_keep_the_authored_info_and_body() {
    for (source, language, info, body) in [
        ("---\nname: Neptune\n---", "yaml", "", "name: Neptune\n"),
        (
            "+++\nname = 'Neptune'\n+++",
            "toml",
            "",
            "name = 'Neptune'\n",
        ),
        ("--- \t\n---", "yaml", "", ""),
        ("+++\n+++", "toml", "", ""),
        ("---\r\na: b\r\n---", "yaml", "", "a: b\n"),
        ("+++\r\na = 1\r\n+++", "toml", "", "a = 1\n"),
        ("+++\ra = 1\r+++", "toml", "", "a = 1\n"),
        ("---\n\na: b\n\n---", "yaml", "", "\na: b\n\n"),
        ("--- json title=x\n{}\n---", "json", "json title=x", "{}\n"),
        ("+++ yaml\na: b\n+++", "yaml", "yaml", "a: b\n"),
    ] {
        let node = parse_frontmatter(source);
        assert_eq!(node.language(), Some(language), "{source:?}");
        assert_eq!(node.info, info);
        assert_eq!(node.value, body);
        let ast = Node::Frontmatter(node);

        let output = ast.to_markdown();
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.language(), Some(language));
        assert_eq!(reparsed.info, info);
        assert_eq!(reparsed.value, body);
        let fence = reparsed.fence.to_string().repeat(3);
        let opening = if info.is_empty() {
            fence
        } else {
            format!("{fence} {info}")
        };
        assert_eq!(output.lines().next(), Some(opening.as_str()));
    }
}

#[test]
fn tagged_frontmatter_keeps_opaque_info_and_literal_body() {
    for (source, fence, info, body) in [
        (
            "---yaml\nname: Neptune\n---",
            '-',
            "yaml",
            "name: Neptune\n",
        ),
        ("--- json\n{}\n---", '-', "json", "{}\n"),
        (
            "+++ custom\nraw &amp; \\* text\n+++",
            '+',
            "custom",
            "raw &amp; \\* text\n",
        ),
        (
            "--- \tjson\\+ &amp; title=planet \t\n\na\n\n \t\n---",
            '-',
            "json\\+ &amp; title=planet",
            "\na\n\n \t\n",
        ),
        ("---\u{a0}\nraw\n---", '-', "\u{a0}", "raw\n"),
        ("\u{feff}--- json\n{}\n---", '-', "json", "{}\n"),
        ("+++\u{c}\nraw\n+++", '+', "\u{c}", "raw\n"),
        (
            "--- ~format `metadata`\nraw\n---",
            '-',
            "~format `metadata`",
            "raw\n",
        ),
        ("+++json\n+++", '+', "json", ""),
        ("--- json\r\n{}\r\n---", '-', "json", "{}\n"),
        ("+++json\r{}\r+++ \t", '+', "json", "{}\n"),
    ] {
        let node = parse_frontmatter(source);
        assert_eq!(node.info, info, "{source:?}");
        assert_eq!(node.value, body, "{source:?}");
        assert_eq!(
            (node.fence, node.fence_length, node.closing_fence_length),
            (fence, 3, 3)
        );
        assert_eq!(
            node.language(),
            info.split_ascii_whitespace()
                .next()
                .or(Some(if fence == '-' { "yaml" } else { "toml" }))
        );
        let document = parse_document(source, &parse_options());
        let output = base_markdown::to_markdown(&document).unwrap();
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.info, info);
        assert_eq!(reparsed.value, body);
        assert_eq!(
            (
                reparsed.fence,
                reparsed.fence_length,
                reparsed.closing_fence_length
            ),
            (fence, 3, 3)
        );
    }
}

#[test]
fn code_fences_at_document_start_are_code_blocks() {
    for source in [
        "```json\n{}\n```",
        "~~~toml\na = 1\n~~~",
        "~~~~custom title=x\nraw\n~~~~~",
        "\u{feff}```json\n{}\n```",
    ] {
        let document = parse_document(source, &parse_options());
        let id = document.children(document.root()).next().unwrap();
        assert!(
            matches!(
                document.node(id).unwrap().kind,
                base_markdown::stream::EventKind::CodeBlock { .. }
            ),
            "{:?}",
            source
        );
        assert_eq!(document.frontmatter_value(id), None);
    }
}

#[test]
fn only_closed_delimiter_fences_at_document_start_are_frontmatter() {
    for source in [
        "```\nbody\n```",
        "~~~ \t\nbody\n~~~",
        "```json",
        "~~~json\nbody",
        "```json\nbody\n~~~",
        "````json\nbody\n```",
        "```json`bad\nbody\n```",
        "``json\nbody\n``",
        "~~json\nbody\n~~",
        " ```json\nbody\n ```",
        "\n```json\nbody\n```",
        "text\n\n~~~json\nbody\n~~~",
        "> ```json\n> body\n> ```",
        "- ~~~json\n  body\n  ~~~",
        "~~~json\nbody\n    ~~~",
        "```json\n{}\n\t```",
        "---json",
        "+++json\nbody",
        "----json\nbody\n----",
    ] {
        let document = parse_document(source, &parse_options());
        assert!(
            !document
                .children(document.root())
                .any(|id| document.frontmatter_value(id).is_some()),
            "{:?}",
            source
        );
    }

    for source in [
        "```json\n{}\n```",
        "~~~toml\na = 1\n~~~",
        "---json\n{}\n---",
    ] {
        let document = parse_document(source, &ParseOptions::commonmark());
        assert!(!document
            .children(document.root())
            .any(|id| document.frontmatter_value(id).is_some()));
    }
}

#[test]
fn nonclosing_fence_lines_remain_in_the_body() {
    let source = "+++ json\n```\n~~~~\n++++\n+++ suffix\n +++\n+++";
    assert_eq!(
        parse_frontmatter(source).value,
        "```\n~~~~\n++++\n+++ suffix\n +++\n"
    );
    assert_eq!(
        parse_frontmatter("---custom\n----\n--- suffix\n---").value,
        "----\n--- suffix\n"
    );
    assert_eq!(
        parse_frontmatter("---json\nbody\n---\n\n```rust\ncode\n```").value,
        "body\n"
    );
}

#[test]
fn both_html_paths_omit_frontmatter_and_keep_later_code() {
    let options = Options {
        parse: parse_options(),
        ..Options::default()
    };
    let source = "--- json\n{}\n---\n\n# Neptune\n\n```rust\nlet x = 1;\n```";
    let expected = "<h1>Neptune</h1>\n<pre><code class=\"language-rust\">let x = 1;\n</code></pre>";
    assert_eq!(to_html_with_options(source, &options), expected);
    let document = parse_document(source, &options.parse);
    let mut output = String::new();
    document.render_html(&options.compile, &mut output).unwrap();
    assert_eq!(output, expected);
    let children: Vec<_> = document
        .children(document.root())
        .filter(|id| {
            !matches!(
                document.node(*id).unwrap().kind,
                base_markdown::stream::EventKind::Separator { .. }
            )
        })
        .collect();
    assert_eq!(children.len(), 3);
    assert!(matches!(
        document.node(children[2]).unwrap().kind,
        base_markdown::stream::EventKind::CodeBlock { .. }
    ));
}

#[test]
fn frontmatter_preserves_source_spans_and_body_after_detaching() {
    let source = "--- json title=x\n{}\n---";
    let mut document = parse_document(source, &parse_options());
    let node = document.children(document.root()).next().unwrap();
    assert_eq!(document.node(node).unwrap().span, 0..23);
    assert_eq!(document.frontmatter_value(node), Some("{}\n"));
    assert_eq!(document.children(node).count(), 0);
    assert_eq!(document.descendants(node).count(), 1);
    document.detach(node).unwrap();
    assert_eq!(document.frontmatter_value(node), Some("{}\n"));
    assert_eq!(
        base_markdown::to_markdown_node(&document, node, &Default::default()).unwrap(),
        "--- json title=x\n{}\n---\n"
    );
}

#[test]
fn an_edited_lf_body_equal_to_the_whole_block_stays_an_edit_after_rebuilding() {
    let source = "---\na: 1\n---\n\n# x\n";
    let mut document = Document::parse(source.to_string(), &ParseOptions::default()).unwrap();
    let node = document.children(document.root()).next().unwrap();
    *document.text_mut(node).unwrap() = "---\na: 1\n---".into();
    // Rebuilding from events keeps the values but not the record of edits.
    let mut builder = base_markdown::AstBuilder::new();
    builder.consume(&mut document.events(), usize::MAX).unwrap();
    let rebuilt = builder.finish(document.source().to_string()).unwrap();
    let node = rebuilt.children(rebuilt.root()).next().unwrap();
    assert_eq!(rebuilt.frontmatter_value(node), Some("---\na: 1\n---"));
    assert_eq!(
        base_markdown::to_markdown(&rebuilt).unwrap(),
        "+++ yaml\n---\na: 1\n---\n+++\n\n# x\n"
    );
}

#[test]
fn an_edited_body_equal_to_the_whole_block_stays_an_edit() {
    for ending in ["\n", "\r\n", "\r"] {
        let source = "---\na\n---\n\nx\n".replace('\n', ending);
        let mut document = parse_document(source.as_str(), &parse_options());
        let node = document.children(document.root()).next().unwrap();
        assert_eq!(document.frontmatter_value(node), Some("a\n"), "{source:?}");
        *document.text_mut(node).unwrap() = "---\na\n---".into();
        assert_eq!(
            document.frontmatter_value(node),
            Some("---\na\n---"),
            "{source:?}"
        );
        assert_eq!(
            base_markdown::to_markdown(&document).unwrap(),
            "+++ yaml\n---\na\n---\n+++\n\nx\n",
            "{source:?}"
        );
        // Replacing the node with the parsed block, as a value, is an edit too.
        let mut document = parse_document(source.as_str(), &parse_options());
        let node = document.children(document.root()).next().unwrap();
        let Some(NodeSyntax::Frontmatter {
            fence,
            closing_fence_length,
            ..
        }) = document.syntax(node)
        else {
            panic!("{:?}", source);
        };
        document
            .set_kind(
                node,
                base_markdown::stream::EventKind::Opaque {
                    value: base_markdown::stream::Text::Value("---\na\n---".to_string().into()),
                    block: true,
                },
            )
            .unwrap();
        document
            .set_syntax(
                node,
                Some(NodeSyntax::Frontmatter {
                    info: base_markdown::stream::Text::Literal(""),
                    fence,
                    closing_fence_length,
                }),
            )
            .unwrap();
        assert_eq!(
            document.frontmatter_value(node),
            Some("---\na\n---"),
            "{source:?}"
        );
    }
}

#[test]
fn serialization_protects_edited_frontmatter_bodies() {
    for (source, body, expected) in [
        ("--- json\n---", "--- \t\n", "+++ json\n--- \t\n+++\n"),
        ("+++ json\n+++", "+++\nbody\n", "--- json\n+++\nbody\n---\n"),
        ("--- json\n---", "value", "--- json\nvalue\n---\n"),
        (
            "--- json\n---",
            "----\n+++ x\n --- \n",
            "--- json\n----\n+++ x\n --- \n---\n",
        ),
        ("---\n---", "---\n", "+++ yaml\n---\n+++\n"),
        ("+++\n+++", "+++\n", "--- toml\n+++\n---\n"),
        ("---\n---", "value", "---\nvalue\n---\n"),
        ("+++\n+++", "value", "+++\nvalue\n+++\n"),
        ("+++\n+++", "", "+++\n+++\n"),
        ("---\u{c}\n---", "---\n", "+++ yaml\n---\n+++\n"),
    ] {
        let mut frontmatter = parse_frontmatter(source);
        let language = frontmatter.language().map(str::to_owned);
        frontmatter.value = body.into();
        let output = Node::Frontmatter(frontmatter).to_markdown();
        assert_eq!(output, expected);
        assert_eq!(parse_frontmatter(&output).language(), language.as_deref());
        assert_eq!(
            parse_frontmatter(&output).value,
            if body.is_empty() || body.ends_with('\n') {
                body.to_string()
            } else {
                format!("{body}\n")
            }
        );
    }
}

#[test]
fn serialization_finds_closing_lines_at_every_line_ending() {
    // The body's own line endings are written unchanged and read back as LF.
    for (source, body, expected, value) in [
        (
            "---\n---",
            "a\r---\rb",
            "+++ yaml\na\r---\rb\n+++\n",
            "a\n---\nb\n",
        ),
        (
            "---\n---",
            "a\r\n--- \t\r\nb\r\n",
            "+++ yaml\na\r\n--- \t\r\nb\r\n+++\n",
            "a\n--- \t\nb\n",
        ),
        ("+++\n+++", "+++\r", "--- toml\n+++\r---\n", "+++\n"),
        (
            "--- json\n---",
            "a\r----\r",
            "--- json\na\r----\r---\n",
            "a\n----\n",
        ),
    ] {
        let mut frontmatter = parse_frontmatter(source);
        let language = frontmatter.language().map(str::to_owned);
        frontmatter.value = body.into();
        let output = Node::Frontmatter(frontmatter).to_markdown();
        assert_eq!(output, expected);
        let document = parse_document(&output, &parse_options());
        let blocks = document.children(document.root()).filter(|&child| {
            !matches!(
                document.node(child).unwrap().kind,
                base_markdown::stream::EventKind::Separator { .. }
            )
        });
        assert_eq!(blocks.count(), 1, "{output:?}");
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.language(), language.as_deref());
        assert_eq!(reparsed.value, value);
    }
}

#[test]
fn code_fence_frontmatter_syntax_is_written_with_delimiters() {
    // A hand-built node with a code-fence character, which no parsed node has any more, keeps its info as the format hint.
    for (fence, info, body, expected, language) in [
        ('`', "json", "{}\n", "--- json\n{}\n---\n", "json"),
        (
            '~',
            "toml title=x",
            "a = 1",
            "--- toml title=x\na = 1\n---\n",
            "toml",
        ),
        ('`', "", "a: b\n", "---\na: b\n---\n", "yaml"),
        ('~', "json", "---\n", "+++ json\n---\n+++\n", "json"),
    ] {
        let output = Node::Frontmatter(Frontmatter {
            value: body.into(),
            info: info.into(),
            fence,
            fence_length: 4,
            closing_fence_length: 5,
            position: None,
        })
        .to_markdown();
        assert_eq!(output, expected);
        let reparsed = parse_frontmatter(&output);
        assert_eq!(reparsed.language(), Some(language));
        assert_eq!(reparsed.info, info);
        assert_eq!(
            reparsed.value,
            if body.ends_with('\n') {
                body.to_string()
            } else {
                format!("{body}\n")
            }
        );
    }
}

#[test]
fn serialization_rejects_invalid_edited_frontmatter_metadata() {
    for (fence, info, body) in [
        ('!', "json", "a\n"),
        ('-', "json\nother", "a\n"),
        ('~', "json\rother", "a\n"),
        ('-', "json", "---\n+++\n"),
        ('`', "", "+++ \n---\t\n"),
        ('-', "", "a\r---\r+++\rb"),
        ('+', "toml", "---\r\n+++ \r\n"),
    ] {
        let mut document = parse_document("--- json\n---", &parse_options());
        let id = document.children(document.root()).next().unwrap();
        *document.text_mut(id).unwrap() = body.into();
        let result = document.set_syntax(
            id,
            Some(NodeSyntax::Frontmatter {
                info: info.to_string().into(),
                fence: base_markdown::CodeFence {
                    character: fence,
                    length: 3,
                    indent: 0,
                },
                closing_fence_length: 3,
            }),
        );
        if fence == '!' {
            assert!(result.is_err());
        } else {
            result.unwrap();
            assert_eq!(
                base_markdown::to_markdown(&document).unwrap_err().rule_id,
                "invalid-frontmatter"
            );
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn frontmatter_serde_roundtrip_preserves_body_and_fences() {
    let document = parse_document("--- custom title=x\nraw\n---", &parse_options());
    let json = serde_json::to_string(&document).unwrap();
    let restored: Document = serde_json::from_str(&json).unwrap();
    let id = restored.children(restored.root()).next().unwrap();
    assert_eq!(restored.frontmatter_value(id), Some("raw\n"));
    let NodeSyntax::Frontmatter {
        info,
        fence,
        closing_fence_length,
    } = restored.syntax(id).unwrap()
    else {
        panic!("frontmatter syntax");
    };
    assert_eq!(info.resolve(restored.source()).unwrap(), "custom title=x");
    assert_eq!((fence.character, fence.length), ('-', 3));
    assert_eq!(closing_fence_length, 3);
    assert_eq!(
        base_markdown::to_markdown(&document).unwrap(),
        base_markdown::to_markdown(&restored).unwrap()
    );
}

#[test]
fn frontmatter_does_not_require_the_code_fenced_construct() {
    let mut options = parse_options();
    options.constructs.code_fenced = false;
    let document = parse_document("--- json\n{}\n---", &options);
    let id = document.children(document.root()).next().unwrap();
    assert!(document.frontmatter_value(id).is_some());
}

#[test]
fn rejected_multiline_frontmatter_restores_line_positions() {
    let mut options = Options::gfm();
    options.parse.constructs.frontmatter = true;
    for source in ["---\n\n>", "---\r\r>", "---x\n\n>x", "+++\n\n> x"] {
        let expected = to_html_with_options(source, &Options::gfm());
        assert_eq!(to_html_with_options(source, &options), expected);
        let document = base_markdown::Document::parse(source, &options.parse).unwrap();
        let mut output = String::new();
        base_markdown::render_events(source, document.events(), &options.compile, &mut output)
            .unwrap();
        assert_eq!(output, expected);
    }
}
