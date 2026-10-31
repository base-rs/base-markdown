// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    stream::{EventKind, Tag, Text},
    CompileOptions, Document, HeadingOptions, LinkForm, NodeSyntax, Options, ParseOptions,
    ReferenceKind,
};

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output
}
fn empty() -> Document {
    Document::parse(String::new(), &ParseOptions::default()).unwrap()
}

#[test]
fn text_edits_materialize_source_and_literals_once() {
    let source = "source";
    let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
    let paragraph = document.children(document.root()).next().unwrap();
    let parsed = document.children(paragraph).next().unwrap();
    let literal = document
        .append_child(paragraph, EventKind::Text(Text::Literal("literal")))
        .unwrap();
    let owned = document
        .append_child(
            paragraph,
            EventKind::Text(Text::from(String::from("owned"))),
        )
        .unwrap();
    for (id, original) in [(parsed, "source"), (literal, "literal"), (owned, "owned")] {
        let text = document.text_mut(id).unwrap();
        assert_eq!(text, original);
        text.reserve(16);
        let allocation = text.as_ptr();
        text.push('!');
        let text = document.text_mut(id).unwrap();
        assert_eq!(text.as_ptr(), allocation);
        text.push('?');
    }
    assert_eq!(document.source(), source);
    assert_eq!(html(&document), "<p>source!?literal!?owned!?</p>");
}

#[test]
fn node_ids_remain_stable_when_subtrees_are_reordered_detached_and_reattached() {
    let mut document = empty();
    let root = document.root();
    let first = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let a = document
        .append_child(first, EventKind::Text(Text::Literal("a")))
        .unwrap();
    let second = document
        .append_child(root, EventKind::Start(Tag::BlockQuote))
        .unwrap();
    let nested = document
        .append_child(second, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    let b = document
        .append_child(nested, EventKind::Text(Text::Literal("b")))
        .unwrap();
    let c = document
        .append_child(first, EventKind::Text(Text::Literal("c")))
        .unwrap();
    assert_eq!(
        html(&document),
        "<p>ac</p>\n<blockquote>\n<p>b</p>\n</blockquote>"
    );
    document.attach(first, b, Some(c)).unwrap();
    assert_eq!(document.children(first).collect::<Vec<_>>(), [a, b, c]);
    assert_eq!(document.text_content(first), "abc");
    document.attach(first, b, Some(a)).unwrap();
    document.attach(first, b, Some(b)).unwrap();
    document.attach(first, b, None).unwrap();
    assert_eq!(document.children(first).collect::<Vec<_>>(), [a, c, b]);
    document.detach(a).unwrap();
    document.detach(a).unwrap();
    assert_eq!(document.text_content(a), "a");
    assert_eq!(document.node(a).unwrap().parent(), None);
    let extracted = document.extract_children(first).unwrap();
    assert_eq!(extracted, [c, b]);
    for id in extracted {
        document.attach(nested, id, None).unwrap();
    }
    assert_eq!(document.text_content(second), "cb");
    document.attach(root, second, Some(first)).unwrap();
    assert_eq!(
        document.descendants(root).collect::<Vec<_>>(),
        [root, second, nested, c, b, first]
    );
    assert_eq!(document.subtree_events(second).count(), 6);
    let clone = document.clone();
    assert_eq!(html(&clone), html(&document));
}

#[test]
fn rejected_structural_edits_preserve_the_tree() {
    let mut document = empty();
    let root = document.root();
    let heading = document
        .append_child(root, EventKind::Start(Tag::Heading(1)))
        .unwrap();
    let text = document
        .append_child(heading, EventKind::Text(Text::Literal("title")))
        .unwrap();
    let footnote = document
        .append_child(
            root,
            EventKind::Start(Tag::FootnoteDefinition(Text::Literal("x"))),
        )
        .unwrap();
    let original = html(&document);
    assert!(document.detach(root).is_err());
    assert!(document.attach(heading, root, None).is_err());
    assert!(document.attach(heading, heading, None).is_err());
    assert!(document.attach(text, heading, None).is_err());
    assert!(document.attach(heading, footnote, None).is_err());
    assert!(document.attach(root, footnote, Some(text)).is_err());
    assert!(document.append_child(text, EventKind::Rule).is_err());
    assert!(document
        .append_child(root, EventKind::Start(Tag::Document(None)))
        .is_err());
    assert!(document
        .append_child(root, EventKind::Start(Tag::Heading(7)))
        .is_err());
    assert!(document
        .append_child(root, EventKind::Start(Tag::Heading(0)))
        .is_err());
    assert!(document
        .append_child(root, EventKind::Text(Text::Source(0..1)))
        .is_err());
    assert!(document
        .append_child(heading, EventKind::Start(Tag::Heading(2)))
        .is_err());
    assert_eq!(html(&document), original);
    // These IDs are outside this document's arena.
    let larger = Document::parse("a *b* c **d** e", &ParseOptions::default()).unwrap();
    let foreign = larger.descendants(larger.root()).last().unwrap();
    assert!(document.node(foreign).is_none());
    assert_eq!(document.children(foreign).count(), 0);
    assert_eq!(document.descendants(foreign).count(), 0);
    assert_eq!(document.subtree_events(foreign).count(), 0);
    assert!(document.syntax(foreign).is_none());
    assert!(document.detach(foreign).is_err());
    assert!(document.attach(root, foreign, None).is_err());
    assert!(document.attach(root, text, Some(foreign)).is_err());
    assert!(document.extract_children(foreign).is_err());
    assert!(document.set_syntax(foreign, None).is_err());
    assert!(document
        .render_node(foreign, &CompileOptions::default(), &mut String::new())
        .is_err());
    assert!(document
        .replace_heading_section(foreign, 0, &empty(), true, true)
        .is_err());
}

#[test]
fn source_syntax_is_lazy_and_explicit_edits_can_disconnect_it() {
    for (source, kind, label) in [
        ("[a][N]\n\n[N]: /u", ReferenceKind::Full, "N"),
        ("![a][]\n\n[a]: /u", ReferenceKind::Collapsed, "a"),
        ("[a]\n\n[a]: /u", ReferenceKind::Shortcut, "a"),
        ("[a [nested]][r]\n\n[r]: /u", ReferenceKind::Full, "r"),
        ("[a\\]]\n\n[a\\]]: /u", ReferenceKind::Shortcut, "a\\]"),
    ] {
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let id = document
            .descendants(document.root())
            .find(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::Link { .. } | Tag::Image { .. })
                )
            })
            .unwrap();
        let Some(NodeSyntax::Reference {
            kind: actual,
            label: value,
        }) = document.syntax(id)
        else {
            panic!("{:?}", source)
        };
        assert_eq!(actual, kind);
        assert_eq!(value.resolve(document.source()).unwrap(), label);
        document
            .set_syntax(
                id,
                Some(NodeSyntax::Reference {
                    kind: ReferenceKind::Full,
                    label: Text::Literal("Override"),
                }),
            )
            .unwrap();
        assert!(matches!(
            document.syntax(id),
            Some(NodeSyntax::Reference {
                kind: ReferenceKind::Full,
                ..
            })
        ));
        document.set_syntax(id, None).unwrap();
        assert!(document.syntax(id).is_none());
    }
    for (source, form) in [
        ("[a](/u)", LinkForm::Explicit),
        ("[a](/u 't')", LinkForm::Explicit),
        ("<https://example.com>", LinkForm::Angle),
        ("<a@example.com>", LinkForm::Angle),
        ("www.example.com", LinkForm::Bare),
        ("https://example.com", LinkForm::Bare),
        ("a@example.com", LinkForm::Bare),
    ] {
        let mut document = Document::parse(source, &Options::gfm().parse).unwrap();
        let mut forms = document
            .descendants(document.root())
            .filter_map(|id| document.syntax(id).map(|syntax| (id, syntax)));
        let (id, syntax) = forms.next().unwrap();
        assert_eq!(syntax, NodeSyntax::Link { form }, "{source:?}");
        assert!(forms.next().is_none());
        document
            .set_syntax(
                id,
                Some(NodeSyntax::Link {
                    form: LinkForm::Explicit,
                }),
            )
            .unwrap();
        assert_eq!(
            document.syntax(id),
            Some(NodeSyntax::Link {
                form: LinkForm::Explicit
            })
        );
        document.set_syntax(id, None).unwrap();
        assert!(document.syntax(id).is_none());
    }
    // Images are always written explicitly, so the form belongs to links only.
    let mut document = Document::parse("![a](/u)", &ParseOptions::default()).unwrap();
    let image = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Image { .. })
            )
        })
        .unwrap();
    assert!(document.syntax(image).is_none());
    assert!(document
        .set_syntax(
            image,
            Some(NodeSyntax::Link {
                form: LinkForm::Bare
            })
        )
        .is_err());
    let mut document =
        Document::parse("```rust key=&amp;\\!\nx\n```", &ParseOptions::default()).unwrap();
    let id = document.children(document.root()).next().unwrap();
    let Some(NodeSyntax::Code { info }) = document.syntax(id) else {
        panic!()
    };
    assert_eq!(info.resolve(document.source()).unwrap(), "rust key=&!");
    assert!(document
        .set_syntax(
            id,
            Some(NodeSyntax::Reference {
                kind: ReferenceKind::Full,
                label: Text::Literal("bad")
            })
        )
        .is_err());
    assert!(document
        .set_syntax(
            id,
            Some(NodeSyntax::Code {
                info: Text::Source(999..1000)
            })
        )
        .is_err());
    let kind = document.node(id).unwrap().kind.clone();
    document.set_kind(id, kind).unwrap();
    assert!(document.syntax(id).is_none());
}

#[test]
fn frontmatter_edits_detach_the_body_and_keep_fence_metadata() {
    let mut options = ParseOptions::default();
    options.constructs.frontmatter = true;
    for source in [
        "--- yaml meta\na: b\n---",
        "+++\nx = 1\n+++",
        "````json\n{}\n`````\n",
        "~~~json\n{}\n~~~\n",
    ] {
        let mut document = Document::parse(source, &options).unwrap();
        let id = document.children(document.root()).next().unwrap();
        let before = document.frontmatter_value(id).unwrap().to_owned();
        assert!(!before.contains("---"));
        assert!(matches!(
            document.syntax(id),
            Some(NodeSyntax::Frontmatter { .. })
        ));
        document.text_mut(id).unwrap().push_str("edited");
        document.text_mut(id).unwrap().push('!');
        assert_eq!(
            document.frontmatter_value(id),
            Some(format!("{before}edited!").as_str())
        );
        assert_eq!(html(&document), "");
        let output = base_markdown::to_markdown(&document).unwrap();
        let reparsed = Document::parse(output.as_str(), &options).unwrap();
        let id = reparsed.children(reparsed.root()).next().unwrap();
        assert_eq!(
            reparsed.frontmatter_value(id),
            Some(format!("{before}edited!\n").as_str())
        );
    }
    let mut document = empty();
    let root = document.root();
    assert!(document.frontmatter_value(root).is_none());
    let id = document
        .append_child(
            root,
            EventKind::Opaque {
                value: Text::Source(0..0),
                block: true,
            },
        )
        .unwrap();
    let syntax = NodeSyntax::Frontmatter {
        info: Text::Literal(""),
        fence: base_markdown::CodeFence {
            character: '-',
            length: 3,
            indent: 0,
        },
        closing_fence_length: 3,
    };
    document.set_syntax(id, Some(syntax)).unwrap();
    assert_eq!(document.frontmatter_value(id), Some(""));
}

#[test]
fn section_replacement_copies_values_without_retaining_another_source_buffer() {
    for (replace_heading, remove_subsections, expected) in [
        (false, false, "TitleNEWNestednestedOtherend"),
        (false, true, "TitleNEWOtherend"),
        (true, false, "NEWNestednestedOtherend"),
        (true, true, "NEWOtherend"),
    ] {
        let mut document = Document::parse(
            "# Title\n\nold\n\n## Nested\n\nnested\n\n# Other\n\nend".to_owned(),
            &ParseOptions::default(),
        )
        .unwrap();
        let root = document.root();
        let replacement = Document::parse("NEW".to_owned(), &ParseOptions::default()).unwrap();
        assert!(document
            .replace_heading_section(root, 0, &replacement, replace_heading, remove_subsections)
            .unwrap());
        drop(replacement);
        assert_eq!(document.text_content(root), expected);
        assert!(!document
            .replace_heading_section(root, 999, &empty(), true, true)
            .unwrap());
        let text = document
            .descendants(root)
            .find(|id| matches!(document.node(*id).unwrap().kind, EventKind::Text(_)))
            .unwrap();
        assert!(!document
            .replace_heading_section(text, 0, &empty(), true, true)
            .unwrap());
    }
    let mut document = Document::parse("# Last", &ParseOptions::default()).unwrap();
    let root = document.root();
    assert!(document
        .replace_heading_section(root, 0, &empty(), false, true)
        .unwrap());
    assert_eq!(document.text_content(root), "Last");
    let mut replacement = Document::parse(
        "## New\n\n[link][Ref]\n\n[Ref]: /u\n\n```rust meta\nx\n```",
        &ParseOptions::default(),
    )
    .unwrap();
    let paragraph = replacement
        .children(replacement.root())
        .find(|id| {
            matches!(
                replacement.node(*id).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
        .unwrap();
    replacement
        .attach(replacement.root(), paragraph, None)
        .unwrap();
    document
        .replace_heading_section(root, 0, &replacement, true, true)
        .unwrap();
    assert!(base_markdown::to_markdown(&document)
        .unwrap()
        .contains("[link](/u)"));
    assert!(base_markdown::to_markdown(&document)
        .unwrap()
        .contains("rust meta"));
}

#[test]
fn heading_anchors_follow_the_shared_html_writer_after_edits() {
    let mut document = Document::parse(
        "# Same\n\n## Same\n\n### Same-2\n\n# Same\n\n# ![alt *text*](u) and `code`\n\n## A<br>B",
        &ParseOptions::default(),
    )
    .unwrap();
    let headings = document.headings(&HeadingOptions::default());
    assert_eq!(
        headings
            .iter()
            .map(|entry| entry.anchor.as_str())
            .collect::<Vec<_>>(),
        [
            "same",
            "same-2",
            "same-2-2",
            "same-3",
            "alt-text-and-code",
            "a<br>b"
        ]
    );
    let filtered = document.headings(&HeadingOptions {
        min_level: 2,
        max_level: 3,
    });
    assert_eq!(filtered[0].anchor, "same-2");
    assert!(document
        .headings(&HeadingOptions {
            min_level: 5,
            max_level: 2
        })
        .is_empty());
    let options = CompileOptions {
        heading_ids: true,
        ..CompileOptions::default()
    };
    let mut output = String::new();
    document.render_html(&options, &mut output).unwrap();
    for entry in headings {
        assert!(output.contains(&format!(
            "id=\"{}\"",
            entry.anchor.replace('<', "&lt;").replace('>', "&gt;")
        )));
    }
    let heading = filtered[0].node;
    let text = document.children(heading).next().unwrap();
    document.text_mut(text).unwrap().push_str(" edited");
    assert_eq!(
        document.headings(&HeadingOptions::default())[1].anchor,
        "same-edited"
    );
}

#[test]
fn heading_anchors_reserve_authored_and_generated_suffixes() {
    let titles = [
        "A", "A-2", "A", "A-3", "A", "A-2", "A-2-2", "A-2", "", "", "-2", "",
    ];
    let source = titles.map(|title| format!("# {title}\n\n")).concat();
    let document = Document::parse(&source, &ParseOptions::default()).unwrap();
    let anchors: Vec<_> = document
        .headings(&HeadingOptions::default())
        .into_iter()
        .map(|heading| heading.anchor)
        .collect();
    assert_eq!(
        anchors,
        ["a", "a-2", "a-3", "a-3-2", "a-4", "a-2-2", "a-2-2-2", "a-2-3", "", "-2", "-2-2", "-3",]
    );
    let options = CompileOptions {
        heading_ids: true,
        ..CompileOptions::default()
    };
    let mut output = String::new();
    document.render_html(&options, &mut output).unwrap();
    for anchor in anchors {
        assert!(output.contains(&format!("id=\"{anchor}\"")));
    }
}

#[test]
fn structural_edits_refresh_list_tightness_and_item_endings() {
    let mut document =
        Document::parse("- first\n- second\n\noutside", &ParseOptions::default()).unwrap();
    let root = document.root();
    let list = document.children(root).next().unwrap();
    let first = document.children(list).next().unwrap();
    let paragraph = document.children(first).next().unwrap();
    assert!(matches!(
        document.node(paragraph).unwrap().kind,
        EventKind::Start(Tag::Paragraph { tight: true })
    ));
    document.attach(root, paragraph, None).unwrap();
    assert!(html(&document).ends_with("<p>first</p>"));
    document.attach(first, paragraph, None).unwrap();
    assert!(html(&document).contains("<li>first</li>"));
    let code = document
        .append_child(
            first,
            EventKind::CodeBlock {
                value: Text::Literal("x\n"),
                language: None,
                fence: None,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    assert!(html(&document).contains("</code></pre>\n</li>"));
    document.detach(code).unwrap();
    assert!(html(&document).contains("<li>first</li>"));
    document
        .set_kind(
            list,
            EventKind::Start(Tag::List {
                start: None,
                tight: false,
            }),
        )
        .unwrap();
    assert!(html(&document).contains("<li>\n<p>first</p>\n</li>"));
    document.detach(paragraph).unwrap();
    let mut output = String::new();
    document
        .render_node(paragraph, &CompileOptions::default(), &mut output)
        .unwrap();
    assert_eq!(output, "<p>first</p>");
}

#[test]
fn authored_metadata_handles_custom_provenance_without_assuming_markdown_syntax() {
    use base_markdown::{
        stream::{Event, TagKind},
        AstBuilder,
    };
    fn custom(raw: &str, kind: EventKind<'static>) -> Document<String> {
        let end = if let EventKind::Start(tag) = &kind {
            Some(EventKind::End(tag.kind()))
        } else {
            None
        };
        let mut events = vec![
            Ok(Event {
                kind: EventKind::Start(Tag::Document(None)),
                span: 0..raw.len(),
            }),
            Ok(Event {
                kind,
                span: 0..raw.len(),
            }),
        ];
        if let Some(end) = end {
            events.push(Ok(Event {
                kind: end,
                span: raw.len()..raw.len(),
            }));
        }
        events.push(Ok(Event {
            kind: EventKind::End(TagKind::Document),
            span: raw.len()..raw.len(),
        }));
        let mut builder = AstBuilder::new();
        builder
            .consume(&mut events.into_iter(), usize::MAX)
            .unwrap();
        builder.finish(raw.to_owned()).unwrap()
    }
    // An autolink's form needs a destination that derives from its text.
    for (raw, destination) in [
        ("plain", Text::Literal("u")),
        ("[text)", Text::Literal("u")),
        ("[x\\]", Text::Literal("u")),
        ("<x>", Text::Literal("u")),
    ] {
        let document = custom(
            raw,
            EventKind::Start(Tag::Link {
                destination,
                title: None,
            }),
        );
        let id = document.children(document.root()).next().unwrap();
        assert!(document.syntax(id).is_none(), "{:?}", raw);
    }
    for raw in ["plain", "[^missing", "[^a\\]"] {
        let document = custom(raw, EventKind::FootnoteReference(Text::Literal("x")));
        assert!(document
            .syntax(document.children(document.root()).next().unwrap())
            .is_none());
    }
    for raw in ["plain", "`x`", "``"] {
        let document = custom(
            raw,
            EventKind::CodeBlock {
                value: Text::Literal("x"),
                language: None,
                fence: Some(base_markdown::CodeFence {
                    character: '`',
                    length: 3,
                    indent: 0,
                }),
                math: false,
                unclosed: true,
            },
        );
        assert!(document
            .syntax(document.children(document.root()).next().unwrap())
            .is_none());
    }
    for raw in [
        "",
        "plain",
        "--\nx\n--",
        "----\nx\n----",
        "---",
        "---\n",
        "---\nno close",
        "---\nx\n--",
        "---\nx\n----",
        "~~~\nx\n~~~",
        "```a`b\nx\n```",
        "---\nx\n--- extra",
    ] {
        let document = custom(
            raw,
            EventKind::Opaque {
                value: Text::Source(0..raw.len()),
                block: true,
            },
        );
        let id = document.children(document.root()).next().unwrap();
        assert!(document.syntax(id).is_none(), "{:?}", raw);
        assert!(document.frontmatter_value(id).is_none());
    }
    // A custom producer can supply a body range and explicit fences without storing fenced source.
    let mut document = custom(
        "prebody",
        EventKind::Opaque {
            value: Text::Source(3..7),
            block: true,
        },
    );
    let id = document.children(document.root()).next().unwrap();
    document
        .set_syntax(
            id,
            Some(NodeSyntax::Frontmatter {
                info: Text::Literal(""),
                fence: base_markdown::CodeFence {
                    character: '-',
                    length: 3,
                    indent: 0,
                },
                closing_fence_length: 3,
            }),
        )
        .unwrap();
    assert_eq!(document.frontmatter_value(id), Some("body"));
}

#[test]
fn heading_anchors_collapse_adjacent_unicode_whitespace() {
    let document = Document::parse("# A \t\u{2003} B", &ParseOptions::default()).unwrap();
    let headings = document.headings(&HeadingOptions::default());
    assert_eq!(headings[0].anchor, "a-b");
    let options = CompileOptions {
        heading_ids: true,
        ..CompileOptions::default()
    };
    let mut output = String::new();
    document.render_html(&options, &mut output).unwrap();
    assert!(output.contains("id=\"a-b\""));
}

#[test]
fn escaped_footnote_labels_and_nul_code_info_roundtrip() {
    let options = Options::gfm();
    let source = "Call.[^a\\]b].\n\n[^a\\]b]: y\n";
    let document = Document::parse(source, &options.parse).unwrap();
    let markdown = base_markdown::to_markdown(&document).unwrap();
    assert!(markdown.contains("[^a\\]b]"));
    assert_eq!(
        html(&document),
        base_markdown::to_html_with_options(&markdown, &options)
    );
    let source = "```r\0ust extra &amp;\nx\n```\n";
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    let id = document.children(document.root()).next().unwrap();
    let Some(NodeSyntax::Code { info }) = document.syntax(id) else {
        panic!()
    };
    assert_eq!(info.resolve(source).unwrap(), "r\0ust extra &");
    let markdown = base_markdown::to_markdown(&document).unwrap();
    assert!(
        markdown.contains("extra &"),
        "{:?}; {:?}",
        markdown,
        document.node(id).unwrap().kind
    );
    assert_eq!(html(&document), base_markdown::to_html(&markdown));
}

#[test]
fn copied_references_preserve_targets_with_conflicting_or_missing_definitions() {
    for missing in [false, true] {
        let mut document = Document::parse(
            "# Keep\n\n[ref]: /old\n\n# Replace\n\nold\n",
            &ParseOptions::default(),
        )
        .unwrap();
        let mut replacement = Document::parse(
            "[x][ref]\n\n[ref]: /new \"title\"\n",
            &ParseOptions::default(),
        )
        .unwrap();
        if missing {
            let definitions: Vec<_> = replacement
                .children(replacement.root())
                .filter(|id| {
                    matches!(
                        replacement.node(*id).unwrap().kind,
                        EventKind::Definition { .. }
                    )
                })
                .collect();
            for id in definitions {
                replacement.detach(id).unwrap();
            }
        }
        let root = document.root();
        let index = document
            .children(root)
            .position(|id| document.text_content(id) == "Replace")
            .unwrap();
        document
            .replace_heading_section(root, index, &replacement, true, true)
            .unwrap();
        drop(replacement);
        let markdown = base_markdown::to_markdown(&document).unwrap();
        assert!(markdown.contains("[x](/new \"title\")"));
        assert_eq!(
            html(&document).trim_end(),
            base_markdown::to_html(&markdown).trim_end()
        );
    }
}

#[test]
fn copied_sections_materialize_frontmatter_footnotes_and_edited_values() {
    let mut options = Options::gfm();
    options.parse.constructs.frontmatter = true;
    let mut replacement=Document::parse("--- custom\nbody\n---\n\n- a\n\n  b\n\nNote[^Mixed] &amp;.\n\n[^Mixed]: Body.\n\n```rust meta\nx\n```\n".to_owned(),&options.parse).unwrap();
    let opaque = replacement.children(replacement.root()).next().unwrap();
    replacement.text_mut(opaque).unwrap().push_str("edited\n");
    let text = replacement
        .descendants(replacement.root())
        .find(|id| matches!(replacement.node(*id).unwrap().kind, EventKind::Text(_)))
        .unwrap();
    replacement.text_mut(text).unwrap().push('!');
    let mut document = Document::parse("# Old".to_owned(), &options.parse).unwrap();
    document
        .replace_heading_section(document.root(), 0, &replacement, true, true)
        .unwrap();
    let markdown = base_markdown::to_markdown(&document).unwrap();
    drop(replacement);
    assert!(markdown.contains("custom\nbody\nedited\n---"));
    assert!(markdown.contains("[^Mixed]"));
    assert!(markdown.contains("rust meta"));
    assert_eq!(
        document.frontmatter_value(document.children(document.root()).next().unwrap()),
        Some("body\nedited\n")
    );
    assert_eq!(
        html(&document).trim_end(),
        base_markdown::to_html_with_options(&markdown, &options).trim_end()
    );
}

#[test]
fn custom_heading_helpers_capture_the_same_leaf_set_and_line_endings_as_html() {
    let mut document = empty();
    let root = document.root();
    let title = document
        .append_child(root, EventKind::Start(Tag::Heading(1)))
        .unwrap();
    document
        .append_child(title, EventKind::Text(Text::Literal("Title")))
        .unwrap();
    document
        .append_child(
            title,
            EventKind::CodeBlock {
                value: Text::Literal("body\n"),
                language: None,
                fence: None,
                math: false,
                unclosed: false,
            },
        )
        .unwrap();
    for ending in ["Crème\r\n", "Other\r", "Final\n"] {
        let heading = document
            .append_child(root, EventKind::Start(Tag::Heading(2)))
            .unwrap();
        document
            .append_child(heading, EventKind::Text(Text::Literal(ending)))
            .unwrap();
    }
    let heading = document
        .append_child(root, EventKind::Start(Tag::Heading(3)))
        .unwrap();
    document
        .append_child(heading, EventKind::Math(Text::Literal("x")))
        .unwrap();
    document
        .append_child(heading, EventKind::HardBreak)
        .unwrap();
    assert_eq!(document.text_content(title), "Titlebody\n");
    let headings = document.headings(&HeadingOptions::default());
    assert_eq!(
        headings
            .iter()
            .map(|entry| entry.anchor.as_str())
            .collect::<Vec<_>>(),
        ["title", "crème", "other", "final", "x"]
    );
    let options = CompileOptions {
        heading_ids: true,
        ..CompileOptions::default()
    };
    let mut output = String::new();
    document.render_node(root, &options, &mut output).unwrap();
    for entry in headings {
        assert!(output.contains(&format!("id=\"{}\"", entry.anchor)));
    }
}

#[test]
fn item_source_spacing_preserves_crlf_boundaries_and_detached_hints() {
    use base_markdown::{
        stream::{Event, TagKind},
        AstBuilder,
    };

    // Custom producers may end sibling provenance before, within, or after CRLF.
    // A previous span ending in CR/LF still requires only one ending in the gap.
    for (source, previous_end, next_start, spread) in [
        ("a\r\nb", 1, 3, false),
        ("a\r\nb", 2, 3, true),
        ("a\r\nb", 3, 3, false),
        ("a\r\n\r\nb", 1, 5, true),
        ("a\r\n\r\nb", 3, 5, true),
        ("a\r\r\nb", 1, 4, true),
        ("a\rb", 1, 2, false),
        ("a\nb", 1, 2, false),
        ("a\n\nb", 2, 3, true),
    ] {
        let kinds = [
            (EventKind::Start(Tag::Document(None)), 0..0),
            (EventKind::Start(Tag::Item { block_end: false }), 0..0),
            (EventKind::Text(Text::Literal("a")), 0..previous_end),
            (
                EventKind::Text(Text::Literal("b")),
                next_start..source.len(),
            ),
            (EventKind::End(TagKind::Item), source.len()..source.len()),
            (
                EventKind::End(TagKind::Document),
                source.len()..source.len(),
            ),
        ];
        let mut events =
            IntoIterator::into_iter(kinds).map(|(kind, span)| Ok(Event { kind, span }));
        let mut builder = AstBuilder::new();
        builder.consume(&mut events, usize::MAX).unwrap();
        let mut document = builder.finish(source).unwrap();
        let item = document.children(document.root()).next().unwrap();
        for detached in [false, true] {
            if detached {
                document.detach(item).unwrap();
            }
            assert_eq!(
                document.syntax(item),
                Some(NodeSyntax::ListItem { spread }),
                "{source:?}, {previous_end}..{next_start}, detached={detached}"
            );
        }
    }
}

#[test]
fn item_source_spacing_handles_all_line_endings_and_reordered_provenance() {
    for ending in ["\n", "\r", "\r\n"] {
        let source = format!("- a{ending}{ending}  b{ending}");
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let list = document.children(document.root()).next().unwrap();
        let item = document.children(list).next().unwrap();
        assert!(matches!(
            document.syntax(item),
            Some(NodeSyntax::ListItem { spread: true })
        ));
        let blocks: Vec<_> = document
            .children(item)
            .filter(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::Paragraph { .. })
                )
            })
            .collect();
        document.attach(item, blocks[1], Some(blocks[0])).unwrap();
        assert!(matches!(
            document.syntax(item),
            Some(NodeSyntax::ListItem { spread: false })
        ));
    }
    let mut document = empty();
    let root = document.root();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    assert!(!document
        .replace_heading_section(root, 0, &empty(), true, true)
        .unwrap());
    assert!(document.frontmatter_value(paragraph).is_none());
    let larger = Document::parse("a *b* c **d** e", &ParseOptions::default()).unwrap();
    let foreign = larger.descendants(larger.root()).last().unwrap();
    assert!(document.frontmatter_value(foreign).is_none());
}

#[test]
fn section_copy_preserves_nested_siblings_empty_containers_and_insertion_boundaries() {
    let mut replacement = Document::parse(
        "a *b **c** d* e\n\n> q\n>\n> > r\n>\n> s\n\n- u\n  - v\n- w\n\nz".to_owned(),
        &ParseOptions::default(),
    )
    .unwrap();
    let root = replacement.root();
    let empty_quote = replacement
        .append_child(root, EventKind::Start(Tag::BlockQuote))
        .unwrap();
    replacement
        .append_child(
            empty_quote,
            EventKind::Start(Tag::Paragraph { tight: false }),
        )
        .unwrap();
    replacement
        .append_child(root, EventKind::Text(Text::Literal("tail")))
        .unwrap();
    let expected = html(&replacement);
    for (prefix, suffix) in [("", ""), ("# Keep\n\nprefix\n\n", "# End\n\nsuffix")] {
        let source = format!("{prefix}# Old\n\nold\n\n{suffix}");
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let root = document.root();
        let index = document
            .children(root)
            .position(|id| document.text_content(id) == "Old")
            .unwrap();
        let existing: Vec<_> = document.descendants(root).collect();
        document
            .replace_heading_section(root, index, &replacement, true, true)
            .unwrap();
        let prefix_html = base_markdown::to_html(prefix);
        let suffix_html = base_markdown::to_html(suffix);
        let expected = format!("{prefix_html}{expected}\n{suffix_html}");
        assert_eq!(html(&document).trim_end(), expected.trim_end());
        let copied: Vec<_> = document
            .descendants(root)
            .filter(|id| !existing.contains(id))
            .collect();
        assert_eq!(copied.len(), replacement.nodes().len() - 1);
        assert!(copied
            .iter()
            .all(|id| document.node(*id).unwrap().span.is_empty()));
    }
}

#[test]
fn section_removal_splices_item_children_and_retains_detached_subtrees() {
    for keep_suffix in [false, true] {
        let mut document = empty();
        let list = document
            .append_child(
                document.root(),
                EventKind::Start(Tag::List {
                    start: None,
                    tight: true,
                }),
            )
            .unwrap();
        let item = document
            .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
            .unwrap();
        let paragraph = document
            .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::Literal("prefix")))
            .unwrap();
        let heading = document
            .append_child(item, EventKind::Start(Tag::Heading(2)))
            .unwrap();
        let quote = document
            .append_child(item, EventKind::Start(Tag::BlockQuote))
            .unwrap();
        let body = document
            .append_child(quote, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(body, EventKind::Text(Text::Literal("detached")))
            .unwrap();
        let suffix = keep_suffix.then(|| {
            document
                .append_child(item, EventKind::Start(Tag::Heading(2)))
                .unwrap()
        });
        let original_subtree: Vec<_> = document.descendants(quote).collect();
        document
            .replace_heading_section(item, 1, &empty(), true, true)
            .unwrap();
        assert_eq!(
            document.children(item).collect::<Vec<_>>(),
            core::iter::once(paragraph)
                .chain(suffix)
                .collect::<Vec<_>>()
        );
        for id in [heading, quote] {
            assert_eq!(document.node(id).unwrap().parent(), None);
            assert_eq!(document.node(id).unwrap().next_sibling(), None);
        }
        assert_eq!(
            document.descendants(quote).collect::<Vec<_>>(),
            original_subtree
        );
        assert_eq!(document.text_content(quote), "detached");
        assert!(
            matches!(document.node(item).unwrap().kind, EventKind::Start(Tag::Item {block_end}) if block_end == keep_suffix)
        );
        assert!(matches!(
            document.node(paragraph).unwrap().kind,
            EventKind::Start(Tag::Paragraph { tight: true })
        ));
        document.attach(item, quote, suffix).unwrap();
        assert_eq!(document.text_content(item), "prefixdetached");
        #[cfg(feature = "serde")]
        {
            let restored: Document =
                serde_json::from_value(serde_json::to_value(&document).unwrap()).unwrap();
            assert_eq!(html(&restored), html(&document));
        }
    }
}

#[test]
fn reference_syntax_preserves_backslash_runs_and_authored_offsets() {
    for count in [0, 1, 2, 31, 32, 255, 256, 900] {
        let label = format!("é{}z", "\\".repeat(count));
        for (authored, kind) in [
            (format!("[{label}]"), ReferenceKind::Shortcut),
            (format!("![{label}][]"), ReferenceKind::Collapsed),
            (format!("[label][{label}]"), ReferenceKind::Full),
        ] {
            let source = format!("prefix {authored}\n\n[{label}]: /target");
            let document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
            let id = document
                .descendants(document.root())
                .find(|id| {
                    matches!(
                        document.node(*id).unwrap().kind,
                        EventKind::Start(Tag::Link { .. } | Tag::Image { .. })
                    )
                })
                .unwrap();
            let Some(NodeSyntax::Reference {
                kind: actual,
                label: actual_label,
            }) = document.syntax(id)
            else {
                panic!("missing syntax for {:?}", authored);
            };
            assert_eq!(actual, kind);
            assert_eq!(actual_label.resolve(document.source()).unwrap(), label);
            let Text::Source(range) = actual_label else {
                panic!("authored labels remain source-backed")
            };
            assert_eq!(&source[range], label);
        }
    }
}

#[test]
fn reference_syntax_distinguishes_escaped_brackets_after_backslash_runs() {
    for pairs in [0, 1, 16, 128] {
        let slashes = "\\".repeat(pairs * 2);
        for label in [
            format!("a{slashes}"),
            format!("a{slashes}\\]z"),
            format!("a{slashes}\\[z"),
        ] {
            for (authored, expected) in [
                (format!("[{label}]"), ReferenceKind::Shortcut),
                (format!("![{label}][]"), ReferenceKind::Collapsed),
                (format!("![image][{label}]"), ReferenceKind::Full),
            ] {
                let source = format!("{authored}\n\n[{label}]: /target");
                let document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
                let id = document
                    .descendants(document.root())
                    .find(|id| {
                        matches!(
                            document.node(*id).unwrap().kind,
                            EventKind::Start(Tag::Link { .. } | Tag::Image { .. })
                        )
                    })
                    .unwrap();
                let Some(NodeSyntax::Reference {
                    kind,
                    label: actual,
                }) = document.syntax(id)
                else {
                    panic!("missing reference syntax for {:?}", authored)
                };
                assert_eq!(kind, expected);
                assert_eq!(actual.resolve(document.source()).unwrap(), label);
            }
        }
    }
}
