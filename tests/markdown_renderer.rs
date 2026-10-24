// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod support;
use core_markdown::{Constructs, Document, MarkdownOptions, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::{to_markdown as markdown, to_markdown_with_options};
use support::fixture::{
    AlignKind, Blockquote, Break, Code, Definition, Delete, Emphasis, FootnoteDefinition,
    FootnoteReference, Frontmatter, Heading, Html, Image, ImageReference, InlineCode, InlineMath,
    Link, LinkReference, List, ListItem, Math, Node, Paragraph, ReferenceKind, Root, Table,
    TableCell, TableRow, Text, ThematicBreak,
};

fn parse(
    source: &str,
    options: &ParseOptions,
) -> Result<Document, core_markdown::message::Message> {
    Ok(Document::parse(source.to_owned(), options).unwrap())
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        children,
        position: None,
    })
}

#[test]
fn serializes_a_markdown_document() -> Result<(), core_markdown::message::Message> {
    let node = parse(
        "# Hello *world*!\n\nA [link](/url).",
        &ParseOptions::default(),
    )?;

    assert_eq!(
        markdown(&node).unwrap(),
        "# Hello *world*!\n\nA [link](/url).\n"
    );

    Ok(())
}

#[test]
fn serializes_extracted_fragments() -> Result<(), core_markdown::message::Message> {
    let mut node = parse("**hello**", &ParseOptions::default())?;
    let fragment = node
        .extract_children(node.root())
        .expect("root has children");

    assert_eq!(
        core_markdown::to_markdown_node(&node, fragment[0], &Default::default()).unwrap(),
        "**hello**\n"
    );

    Ok(())
}

#[test]
fn renders_with_custom_markdown_options() -> Result<(), core_markdown::message::Message> {
    let node = parse("*hello*", &ParseOptions::default())?;
    let options = MarkdownOptions {
        emphasis: '_',
        ..Default::default()
    };
    assert_eq!(
        to_markdown_with_options(&node, &options).unwrap(),
        "_hello_\n"
    );

    Ok(())
}

#[test]
fn serializes_gfm_constructs() -> Result<(), core_markdown::message::Message> {
    let node = parse(
        "~~old~~\n\n| A | B |\n| :- | -: |\n| x | y |\n\nCall[^note].\n\n[^note]: Details.",
        &ParseOptions::gfm(),
    )?;

    assert_eq!(
        markdown(&node).unwrap(),
        "~~old~~\n\n| A | B |\n| :--- | ---: |\n| x | y |\n\nCall[^note].\n\n[^note]: Details.\n"
    );

    Ok(())
}

#[test]
fn serializes_frontmatter() -> Result<(), core_markdown::message::Message> {
    let options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    };
    let node = parse("---\ntitle: Example\n---\n\nHello", &options)?;

    assert_eq!(
        markdown(&node).unwrap(),
        "---\ntitle: Example\n---\n\nHello\n"
    );

    Ok(())
}

#[test]
fn serializes_a_node_directly() {
    let node = Node::Text(support::fixture::Text {
        value: "hello".into(),
        position: None,
    });

    assert_eq!(markdown(&node).unwrap(), "hello\n");
}

#[test]
fn exposes_default_options_and_fallible_rendering() {
    let node = Node::Text(support::fixture::Text {
        value: "hello".into(),
        position: None,
    });
    assert_eq!(MarkdownOptions::default().bullet, '-');
    assert_eq!(markdown(&node).unwrap(), "hello\n");
}

#[test]
fn serialization_reports_invalid_options() {
    let node = parse("*hello*", &ParseOptions::default()).unwrap();
    let options = MarkdownOptions {
        emphasis: 'x',
        ..Default::default()
    };
    assert!(to_markdown_with_options(&node, &options).is_err());
}

#[test]
fn fallible_rendering_reports_invalid_markers() {
    let cases = [
        (
            "- item",
            MarkdownOptions {
                bullet: 'x',
                ..Default::default()
            },
        ),
        (
            "1. item",
            MarkdownOptions {
                bullet_ordered: 'x',
                ..Default::default()
            },
        ),
        (
            "- item",
            MarkdownOptions {
                bullet: '*',
                bullet_other: 'x',
                ..Default::default()
            },
        ),
        (
            "*emphasis*",
            MarkdownOptions {
                emphasis: 'x',
                ..Default::default()
            },
        ),
        (
            "**strong**",
            MarkdownOptions {
                strong: 'x',
                ..Default::default()
            },
        ),
        (
            "```rust\ncode\n```",
            MarkdownOptions {
                fence: 'x',
                ..Default::default()
            },
        ),
        (
            "[link](url \"title\")",
            MarkdownOptions {
                quote: 'x',
                ..Default::default()
            },
        ),
        (
            "---",
            MarkdownOptions {
                rule: 'x',
                ..Default::default()
            },
        ),
        (
            "---",
            MarkdownOptions {
                rule_repetition: 2,
                ..Default::default()
            },
        ),
    ];

    for (source, options) in cases {
        let node = parse(source, &ParseOptions::default()).unwrap();
        assert!(
            to_markdown_with_options(&node, &options).is_err(),
            "{}",
            source
        );
    }
}

#[test]
fn serializes_ast_variants_and_custom_markdown_options() {
    let variants = vec![
        Node::Blockquote(Blockquote {
            children: vec![paragraph(vec![text("quote")])],
            position: None,
        }),
        Node::Code(Code {
            fence: None,
            value: "code".into(),
            position: None,
            lang: Some("rust".into()),
            meta: Some("ignore".into()),
        }),
        Node::Definition(Definition {
            position: None,
            url: "/guide".into(),
            title: Some("Guide".into()),
            identifier: "guide".into(),
            label: Some("Guide".into()),
        }),
        Node::Delete(Delete {
            children: vec![text("deleted")],
            position: None,
        }),
        Node::FootnoteDefinition(FootnoteDefinition {
            children: vec![paragraph(vec![text("details")])],
            position: None,
            identifier: "note".into(),
            label: Some("Note".into()),
        }),
        Node::FootnoteDefinition(FootnoteDefinition {
            children: vec![],
            position: None,
            identifier: "empty".into(),
            label: None,
        }),
        Node::FootnoteReference(FootnoteReference {
            position: None,
            identifier: "note".into(),
            label: Some("Note".into()),
        }),
        Node::Heading(Heading {
            children: vec![text("setext heading")],
            position: None,
            depth: 2,
        }),
        Node::Html(Html {
            value: "<b>raw</b>".into(),
            position: None,
        }),
        Node::Image(Image {
            position: None,
            alt: "image".into(),
            url: "/image.png".into(),
            title: Some("Image".into()),
        }),
        Node::ImageReference(ImageReference {
            position: None,
            alt: "reference image".into(),
            reference_kind: ReferenceKind::Full,
            identifier: "image".into(),
            label: Some("Image".into()),
        }),
        Node::InlineCode(InlineCode {
            value: " has ` ticks ` ".into(),
            position: None,
            lang: Some("ruby".into()),
        }),
        Node::InlineMath(InlineMath {
            value: "$x$".into(),
            position: None,
        }),
        Node::Link(Link {
            children: vec![text("https://example.com")],
            position: None,
            url: "https://example.com".into(),
            title: None,
        }),
        Node::Link(Link {
            children: vec![text("two"), text(" children")],
            position: None,
            url: "/path with spaces".into(),
            title: Some("A \"title\"".into()),
        }),
        Node::LinkReference(LinkReference {
            children: vec![text("label")],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: "one &#32; two".into(),
            label: None,
        }),
        Node::LinkReference(LinkReference {
            children: vec![text("hex")],
            position: None,
            reference_kind: ReferenceKind::Shortcut,
            identifier: "one &#X20; two".into(),
            label: None,
        }),
        Node::List(List {
            children: vec![Node::ListItem(ListItem {
                children: vec![paragraph(vec![text("item")])],
                position: None,
                spread: false,
                checked: Some(true),
            })],
            position: None,
            ordered: true,
            start: Some(7),
            spread: true,
        }),
        Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
                    children: vec![paragraph(vec![text("first")])],
                    position: None,
                    spread: false,
                    checked: None,
                }),
                Node::ListItem(ListItem {
                    children: vec![Node::ThematicBreak(ThematicBreak { position: None })],
                    position: None,
                    spread: false,
                    checked: None,
                }),
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false,
        }),
        Node::ListItem(ListItem {
            children: vec![paragraph(vec![
                text("loose item"),
                Node::Break(Break { position: None }),
            ])],
            position: None,
            spread: true,
            checked: Some(false),
        }),
        Node::Math(Math {
            value: "x^2".into(),
            position: None,
            meta: Some("latex".into()),
        }),
        Node::Table(Table {
            children: vec![
                Node::TableRow(TableRow {
                    children: vec![
                        Node::TableCell(TableCell {
                            children: vec![text("plain | pipe")],
                            position: None,
                        }),
                        Node::TableCell(TableCell {
                            children: vec![Node::InlineCode(InlineCode {
                                value: "inside|code".into(),
                                position: None,
                                lang: None,
                            })],
                            position: None,
                        }),
                        Node::TableCell(TableCell {
                            children: vec![text("escaped \\| pipe")],
                            position: None,
                        }),
                    ],
                    position: None,
                }),
                Node::TableRow(TableRow {
                    children: vec![Node::TableCell(TableCell {
                        children: vec![text("row")],
                        position: None,
                    })],
                    position: None,
                }),
            ],
            position: None,
            align: vec![AlignKind::Left, AlignKind::Center, AlignKind::None],
        }),
        Node::TableRow(TableRow {
            children: vec![Node::TableCell(TableCell {
                children: vec![text("standalone")],
                position: None,
            })],
            position: None,
        }),
        Node::TableCell(TableCell {
            children: vec![text("cell")],
            position: None,
        }),
        Node::ThematicBreak(ThematicBreak { position: None }),
        Node::Frontmatter(Frontmatter {
            info: String::new(),
            fence: '+',
            fence_length: 3,
            closing_fence_length: 3,
            value: "title = 'example'".into(),
            position: None,
        }),
        Node::Frontmatter(Frontmatter {
            info: String::new(),
            fence: '-',
            fence_length: 3,
            closing_fence_length: 3,
            value: "title: example".into(),
            position: None,
        }),
    ];

    for node in variants {
        let markdown = markdown(&node).unwrap();
        assert!(!markdown.is_empty(), "failed to serialize {:?}", node);
    }

    let custom_options = MarkdownOptions {
        fence: '~',
        fences: true,
        setext: true,
        close_atx: true,
        rule: '_',
        rule_repetition: 4,
        rule_spaces: true,
        quote: '\'',
        single_dollar_text_math: false,
        ..MarkdownOptions::default()
    };
    let custom = Node::Root(Root {
        position: None,
        children: vec![
            Node::Heading(Heading {
                children: vec![Node::Emphasis(Emphasis {
                    children: vec![text("a\nb")],
                    position: None,
                })],
                position: None,
                depth: 2,
            }),
            Node::Code(Code {
                fence: None,
                value: "~~~\ncode".into(),
                position: None,
                lang: Some("rust".into()),
                meta: Some("a ` b".into()),
            }),
            Node::InlineMath(InlineMath {
                value: "$x$\ny".into(),
                position: None,
            }),
            Node::ThematicBreak(ThematicBreak { position: None }),
        ],
    });
    let output =
        to_markdown_with_options(&custom, &custom_options).expect("custom options serialize");
    assert_eq!(
        output,
        "*a\nb*\n--~~~~rust a ` b\n~~~\ncode\n~~~~$$ $x$\ny $$_ _ _ _\n"
    );

    let inline_break = Node::InlineCode(InlineCode {
        value: "a\r\n#b".into(),
        position: None,
        lang: None,
    });
    assert_eq!(markdown(&inline_break).unwrap(), "`a #b`\n");

    let inline_math_break = Node::InlineMath(InlineMath {
        value: "a\r\n#b".into(),
        position: None,
    });
    assert_eq!(markdown(&inline_math_break).unwrap(), "$a #b$\n");

    let nested_heading = Node::Heading(Heading {
        children: vec![Node::Emphasis(Emphasis {
            children: vec![text("no line break")],
            position: None,
        })],
        position: None,
        depth: 2,
    });
    assert!(markdown(&nested_heading).unwrap().starts_with("## "));
}

#[test]
fn propagates_errors_from_nested_markdown_nodes() {
    let invalid_emphasis = Node::Emphasis(Emphasis {
        children: vec![text("nested")],
        position: None,
    });
    let paragraph_with_invalid_emphasis = paragraph(vec![invalid_emphasis.clone()]);
    let list_item = Node::ListItem(ListItem {
        children: vec![paragraph_with_invalid_emphasis.clone()],
        position: None,
        spread: false,
        checked: None,
    });
    let table_cell = Node::TableCell(TableCell {
        children: vec![invalid_emphasis.clone()],
        position: None,
    });
    let table_row = Node::TableRow(TableRow {
        children: vec![table_cell.clone()],
        position: None,
    });

    let nodes = vec![
        Node::Blockquote(Blockquote {
            children: vec![paragraph_with_invalid_emphasis.clone()],
            position: None,
        }),
        Node::Delete(Delete {
            children: vec![invalid_emphasis.clone()],
            position: None,
        }),
        Node::FootnoteDefinition(FootnoteDefinition {
            children: vec![paragraph_with_invalid_emphasis.clone()],
            position: None,
            identifier: "note".into(),
            label: None,
        }),
        Node::Heading(Heading {
            children: vec![invalid_emphasis.clone()],
            position: None,
            depth: 2,
        }),
        Node::Link(Link {
            children: vec![invalid_emphasis.clone()],
            position: None,
            url: "/resource".into(),
            title: None,
        }),
        Node::LinkReference(LinkReference {
            children: vec![invalid_emphasis.clone()],
            position: None,
            reference_kind: ReferenceKind::Shortcut,
            identifier: "reference".into(),
            label: None,
        }),
        Node::List(List {
            children: vec![list_item.clone()],
            position: None,
            ordered: false,
            start: None,
            spread: false,
        }),
        list_item,
        paragraph_with_invalid_emphasis,
        Node::Strong(support::fixture::Strong {
            children: vec![invalid_emphasis.clone()],
            position: None,
        }),
        Node::Table(Table {
            children: vec![table_row.clone()],
            position: None,
            align: vec![],
        }),
        table_row,
        table_cell,
    ];

    let invalid_emphasis_options = MarkdownOptions {
        emphasis: 'x',
        ..MarkdownOptions::default()
    };

    for node in nodes {
        assert!(
            to_markdown_with_options(&node, &invalid_emphasis_options).is_err(),
            "expected nested emphasis to fail for {:?}",
            node
        );
    }

    let nested_strong = Node::Emphasis(Emphasis {
        children: vec![Node::Strong(support::fixture::Strong {
            children: vec![text("nested")],
            position: None,
        })],
        position: None,
    });
    let invalid_strong_options = MarkdownOptions {
        strong: 'x',
        ..MarkdownOptions::default()
    };
    for setext in [false, true] {
        let heading = Node::Heading(Heading {
            children: vec![nested_strong.clone()],
            position: None,
            depth: 2,
        });
        let options = MarkdownOptions {
            setext,
            ..invalid_strong_options.clone()
        };
        assert!(
            to_markdown_with_options(&heading, &options).is_err(),
            "expected nested strong to fail for setext={}",
            setext
        );
    }

    for node in [
        Node::Image(Image {
            position: None,
            alt: "image".into(),
            url: "/image.png".into(),
            title: None,
        }),
        Node::Definition(Definition {
            position: None,
            url: "/resource".into(),
            title: None,
            identifier: "reference".into(),
            label: None,
        }),
    ] {
        let options = MarkdownOptions {
            quote: 'x',
            ..MarkdownOptions::default()
        };
        assert!(to_markdown_with_options(&node, &options).is_err());
    }

    let list = Node::List(List {
        children: vec![Node::ListItem(ListItem {
            children: vec![paragraph(vec![text("item")])],
            position: None,
            spread: false,
            checked: None,
        })],
        position: None,
        ordered: false,
        start: None,
        spread: false,
    });
    let invalid_rule_options = MarkdownOptions {
        rule: 'x',
        ..MarkdownOptions::default()
    };
    assert!(to_markdown_with_options(&list, &invalid_rule_options).is_err());
}
