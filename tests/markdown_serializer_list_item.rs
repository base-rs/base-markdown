// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{IndentOptions, Options};
use base_markdown::{
    stream::{EventKind, Tag, Text as Value},
    to_markdown_with_options, Document, NodeSyntax, ParseOptions,
};
use pretty_assertions::assert_eq;
use std::time::{Duration, Instant};
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::fixture::{List, ListItem, Node, Paragraph, Root, Text, ThematicBreak};

#[test]
fn list_item() {
    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "-\n",
        "should support a list item"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: Vec::new(),
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                bullet: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "+\n",
        "should serialize an item w/ a plus as bullet when `bullet: \" + \"`"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None
            })],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "- a\n",
        "should support a list item w/ a child"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::ThematicBreak(ThematicBreak { position: None }),
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("b"),
                        position: None
                    })],
                    position: None
                }),
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "- a\n  ***\n  b\n",
        "should support a list item w/ children"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![
                    Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("a"),
                            position: None
                        })],
                        position: None
                    }),
                    Node::ThematicBreak(ThematicBreak { position: None })
                ],
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::One,
                ..Default::default()
            }
        )
        .unwrap(),
        "- a\n  ***\n",
        "should use one space after the bullet for `list_item_indent: \"IndentOptions::One\"`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),],
                position: None,
                spread: false,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::Mixed,
                ..Default::default()
            }
        )
        .unwrap(),
        "- a\n",
        "should use one space after the bullet for `list_item_indent: \"IndentOptions::Mixed\"`, when the item is not spread"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::ListItem(ListItem {
                children: vec![Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::ThematicBreak(ThematicBreak { position: None })],
                position: None,
                spread: true,
                checked: None
            }),
            &Options {
                list_item_indent: IndentOptions::Mixed,
                ..Default::default()
            }
        )
        .unwrap(),
        "-   a\n\n    ***\n",
        "should use a tab stop of spaces after the bullet for `list_item_indent: \"IndentOptions::Mixed\"`, when the item is spread"
    );

    assert_eq!(
        to(&Node::ListItem(ListItem {
            children: vec![
                Node::Paragraph(Paragraph {
                    children: vec![Node::Text(Text {
                        value: String::from("a"),
                        position: None
                    })],
                    position: None
                }),
                Node::ThematicBreak(ThematicBreak { position: None }),
            ],
            position: None,
            spread: false,
            checked: None
        }))
        .unwrap(),
        "- a\n  ***\n",
        "should not use blank lines between child blocks for items w/ `spread: false`"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '*',
                bullet_other: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "* * +\n",
        "should support `bullet_other`"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '-',
                ..Default::default()
            }
        )
        .unwrap(),
        "- - *\n",
        "should default to an `bullet_other` different from `bullet` (1)"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '*',
                ..Default::default()
            }
        )
        .unwrap(),
        "* * -\n",
        "should default to an `bullet_other` different from `bullet` (2)"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
                    children: vec![Node::Paragraph(Paragraph {
                        children: vec![Node::Text(Text {
                            value: String::from("a"),
                            position: None
                        })],
                        position: None
                    }),],
                    position: None,
                    spread: false,
                    checked: None
                }),
                Node::ListItem(ListItem {
                    children: vec![Node::ThematicBreak(ThematicBreak { position: None })],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "- a\n- ***\n",
        "should use a different bullet than a thematic rule marker, if the first child of a list item is a thematic break (2)"
    );

    assert_eq!(
        to(&create_list(create_list::<Option<Node>>(None))).unwrap(),
        "- -\n",
        "should *not* use a different bullet for an empty list item in two lists"
    );

    assert_eq!(
        to(&create_list(create_list(create_list::<Option<Node>>(None)))).unwrap(),
        "- - *\n",
        "should use a different bullet for an empty list item in three lists (1)"
    );

    assert_eq!(
        to(&Node::List(List {
            children: vec![
                Node::ListItem(ListItem {
                    children: vec![],
                    position: None,
                    spread: false,
                    checked: None
                }),
                Node::ListItem(ListItem {
                    children: vec![create_list(create_list::<Option<Node>>(None))],
                    position: None,
                    spread: false,
                    checked: None
                })
            ],
            position: None,
            ordered: false,
            start: None,
            spread: false
        }))
        .unwrap(),
        "-\n- - *\n",
        "should use a different bullet for an empty list item in three lists (2)"
    );

    assert_eq!(
        to_md_with_opts(
            &create_list(create_list(create_list::<Option<Node>>(None))),
            &Options {
                bullet: '+',
                ..Default::default()
            }
        )
        .unwrap(),
        "+ + +\n",
        "should not use a different bullet for an empty list item in three lists if `bullet` isn’t a thematic rule marker"
    );

    assert_eq!(
        to(&create_list(create_list(create_list(create_list::<
            Option<Node>,
        >(None)))))
        .unwrap(),
        "- - - *\n",
        "should use a different bullet for an empty list item in four lists"
    );

    assert_eq!(
        to(&create_list(create_list(create_list(create_list(
            create_list::<Option<Node>>(None)
        )))))
        .unwrap(),
        "- - - - *\n",
        "should use a different bullet for an empty list item in five lists"
    );

    assert_eq!(
        to(&create_list(create_list(vec![
            create_list(Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a"),
                    position: None
                })],
                position: None
            })),
            create_list::<Option<Node>>(None)
        ])))
        .unwrap(),
        "- - - a\n    *\n",
        "should not use a different bullet for an empty list item at non-head in two lists"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::List(List {
                children: vec![Node::ListItem(ListItem {
                    children: vec![],
                    position: None,
                    spread: false,
                    checked: None
                })],
                position: None,
                ordered: true,
                start: None,
                spread: false
            }),
            &Options {
                bullet_ordered: ')',
                ..Default::default()
            }
        )
        .unwrap(),
        "1)\n",
        "should support `bullet_ordered`"
    );

    assert_eq!(
        to_md_with_opts(
            &Node::Root(Root {
                children: vec![
                    Node::List(List {
                        children: vec![Node::ListItem(ListItem {
                            children: vec![],
                            position: None,
                            spread: false,
                            checked: None
                        })],
                        position: None,
                        ordered: true,
                        start: None,
                        spread: false
                    }),
                    Node::List(List {
                        children: vec![Node::ListItem(ListItem {
                            children: vec![],
                            position: None,
                            spread: false,
                            checked: None
                        })],
                        position: None,
                        ordered: true,
                        start: None,
                        spread: false
                    }),
                ],
                position: None
            }),
            &Options {
                bullet_ordered: ')',
                ..Default::default()
            }
        )
        .unwrap(),
        "1)\n\n1.\n",
        "should use a different bullet for adjacent ordered lists"
    );
}

#[test]
fn serializes_checked_and_unchecked_task_list_items() {
    let item = |checked| {
        Node::ListItem(ListItem {
            children: vec![Node::Paragraph(Paragraph {
                children: vec![Node::Text(Text {
                    value: String::from("a task"),
                    position: None,
                })],
                position: None,
            })],
            position: None,
            spread: false,
            checked: Some(checked),
        })
    };

    assert_eq!(to(&item(true)).unwrap(), "- [x] a task\n");
    assert_eq!(to(&item(false)).unwrap(), "- [ ] a task\n");
}

#[test]
fn rejects_an_invalid_bullet_marker() {
    let item = Node::ListItem(ListItem {
        children: vec![],
        position: None,
        spread: false,
        checked: None,
    });
    let error = to_md_with_opts(
        &item,
        &Options {
            bullet: 'x',
            ..Options::default()
        },
    )
    .expect_err("unsupported list markers should be rejected");

    assert_eq!(error.rule_id, "unexpected-marker");
}

trait IntoVecNode {
    fn into_vec(self) -> Vec<Node>;
}

impl IntoVecNode for Node {
    fn into_vec(self) -> Vec<Node> {
        vec![self]
    }
}

impl IntoVecNode for Option<Node> {
    fn into_vec(self) -> Vec<Node> {
        self.map(|n| vec![n]).unwrap_or_default()
    }
}

impl IntoVecNode for Vec<Node> {
    fn into_vec(self) -> Vec<Node> {
        self
    }
}

fn create_list<T>(d: T) -> Node
where
    T: IntoVecNode,
{
    Node::List(List {
        children: vec![Node::ListItem(ListItem {
            children: d.into_vec(),
            position: None,
            spread: false,
            checked: None,
        })],
        position: None,
        ordered: false,
        start: None,
        spread: false,
    })
}

/// Assert that serializing the document `build` makes takes linear time with each of `options`:
/// 4x input within 12x the time, taking the fastest of five runs of each size.
fn assert_linear(label: &str, options: &[Options], build: impl Fn(usize) -> Document<String>) {
    let time = |document: &Document<String>, options: &Options| {
        let started = Instant::now();
        to_markdown_with_options(document, options).unwrap();
        started.elapsed()
    };
    let small_document = build(1 << 11);
    let large_document = build(1 << 13);
    for options in options {
        time(&small_document, options);
        let mut small = Duration::MAX;
        let mut large = Duration::MAX;
        for _ in 0..5 {
            small = small.min(time(&small_document, options));
            large = large.min(time(&large_document, options));
        }
        assert!(
            large < small * 12,
            "{}: 4x input took {:?} against {:?} with {:?}",
            label,
            large,
            small,
            options
        );
    }
}

/// A loose list of `size` separators and then `size` items, each with a paragraph and a heading,
/// whose recorded spread is unset, false or true in turn.
fn separated_items(size: usize) -> Document<String> {
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: false,
            }),
        )
        .unwrap();
    for _ in 0..size {
        document
            .append_child(
                list,
                EventKind::Separator {
                    value: Value::Literal("\n"),
                    conditional: false,
                },
            )
            .unwrap();
    }
    for index in 0..size {
        let item = document
            .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
            .unwrap();
        if index % 3 > 0 {
            let spread = index % 3 == 2;
            document
                .set_syntax(item, Some(NodeSyntax::ListItem { spread }))
                .unwrap();
        }
        for tag in [Tag::Paragraph { tight: false }, Tag::Heading(1)] {
            let block = document.append_child(item, EventKind::Start(tag)).unwrap();
            document
                .append_child(block, EventKind::Text(Value::Literal("a")))
                .unwrap();
        }
    }
    document
}

#[test]
fn serializing_list_items_is_linear() {
    type Build = fn(usize) -> String;
    let cases: [(&str, Build); 8] = [
        ("headings in one item", |size| {
            format!("- {}", "# a\n  ".repeat(size))
        }),
        ("paragraphs in one spread item", |size| {
            format!("- a\n\n{}", "  b\n\n".repeat(size))
        }),
        ("paragraphs and quotes in one spread item", |size| {
            format!("- a\n\n{}", "  > b\n\n  c\n\n".repeat(size))
        }),
        ("footnote definitions in one item", |size| {
            format!("- {}", "[^a]: b\n  ".repeat(size))
        }),
        ("nested lists between paragraphs", |size| {
            format!("- a\n{}", "  - b\n\n  c\n".repeat(size))
        }),
        ("nested items with several blocks", |size| {
            format!("- a\n  - b\n{}", "    # c\n    d\n    - e\n".repeat(size))
        }),
        ("loose list of items with several blocks", |size| {
            "- # a\n\n  b\n\n  > c\n\n".repeat(size)
        }),
        ("paragraphs after definitions in one item", |size| {
            format!(
                "- [a]: b\n{}\n{}",
                "  [a]: b\n".repeat(size),
                "  *c* d\n\n".repeat(size)
            )
        }),
    ];
    let option_sets = [
        Options::default(),
        // Definitions joined without a blank line, which separates the other blocks with one.
        Options {
            fences: false,
            setext: true,
            list_item_indent: IndentOptions::Mixed,
            tight_definitions: true,
            ..Options::default()
        },
    ];
    let parse = ParseOptions::gfm();
    for (label, build) in cases {
        assert_linear(label, &option_sets, |size| {
            Document::parse(build(size), &parse).unwrap()
        });
    }
    assert_linear("items after separators", &option_sets, separated_items);
}

#[test]
fn writes_task_markers_outside_the_item_being_written() {
    let parse = ParseOptions::gfm();
    let options = Options::default();
    // The marker of a task item belongs to the item, which writes it, so its paragraph written on
    // its own leaves the marker out, whether or not the item is being written.
    let document = Document::parse("- [x] a\n", &parse).unwrap();
    let paragraph = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. })
            )
        })
        .unwrap();
    assert_eq!(
        base_markdown::to_markdown_node(&document, paragraph, &options).unwrap(),
        "a\n"
    );
    // A marker in a paragraph that is not in an item is written as text.
    let mut document = Document::parse(String::new(), &parse).unwrap();
    let root = document.root();
    let quote = document
        .append_child(root, EventKind::Start(Tag::BlockQuote))
        .unwrap();
    let paragraph = document
        .append_child(quote, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document
        .append_child(paragraph, EventKind::TaskMarker(true))
        .unwrap();
    document
        .append_child(paragraph, EventKind::Text(Value::Literal(" a")))
        .unwrap();
    assert_eq!(
        to_markdown_with_options(&document, &options).unwrap(),
        "> [x] a\n"
    );
}

#[test]
fn writes_the_only_item_of_a_loose_list_before_separators_spread() {
    // The separator after the item is not a block, so the item is still the list's only one.
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: false,
            }),
        )
        .unwrap();
    let item = document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    for tag in [Tag::Paragraph { tight: false }, Tag::Heading(1)] {
        let block = document.append_child(item, EventKind::Start(tag)).unwrap();
        document
            .append_child(block, EventKind::Text(Value::Literal("a")))
            .unwrap();
    }
    document
        .append_child(
            list,
            EventKind::Separator {
                value: Value::Literal("\n"),
                conditional: false,
            },
        )
        .unwrap();
    assert_eq!(
        to_markdown_with_options(&document, &Options::default()).unwrap(),
        "- a\n\n  # a\n"
    );
}
