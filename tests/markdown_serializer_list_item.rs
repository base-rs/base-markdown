// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::{IndentOptions, Options};
use base_markdown::{
    stream::{EventKind, Tag, Text as Value},
    to_markdown_with_options, Document, ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::{canonical, List, ListItem, Node, Paragraph, Root, Text, ThematicBreak};
use support::fixture::{to_markdown as to, to_markdown_with_options as to_md_with_opts};
use support::linear::{assert_linear, timed, SCALE};

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

/// Assert that serializing the document `build` makes takes linear time with each of `options`.
fn assert_serializes_linear(
    label: &str,
    size: usize,
    options: &[(&str, Options)],
    build: impl Fn(usize) -> Document<String>,
) {
    let small = build(size);
    let large = build(SCALE * size);
    for (name, options) in options {
        assert_linear((label, name), size, |count| {
            let document = if count == size { &small } else { &large };
            timed(|| {
                to_markdown_with_options(document, options).unwrap();
            })
        });
    }
}

/// A loose list of `4 * size` separators and then `size` items of a paragraph.
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
    for _ in 0..4 * size {
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
    for _ in 0..size {
        let item = document
            .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
            .unwrap();
        let paragraph = document
            .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Value::Literal("a")))
            .unwrap();
    }
    document
}

/// A task item of `size` definitions and then `4 * size` paragraphs, each holding a task marker.
fn task_item_after_definitions(size: usize) -> Document<String> {
    let mut document = Document::parse(String::new(), &ParseOptions::gfm()).unwrap();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    let item = document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    for _ in 0..size {
        document
            .append_child(
                item,
                EventKind::Definition {
                    label: Value::Literal("a"),
                    destination: Value::Literal("/u"),
                    title: None,
                },
            )
            .unwrap();
    }
    for _ in 0..4 * size {
        let paragraph = document
            .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::TaskMarker(true))
            .unwrap();
    }
    document
}

#[test]
fn serializing_list_items_is_linear() {
    let default = [("default", Options::default())];
    type Build = fn(usize) -> String;
    let cases: [(&str, Build); 4] = [
        ("headings in one item", |size| {
            format!("- {}", "# a\n  ".repeat(size))
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
    ];
    let parse = ParseOptions::gfm();
    for (label, build) in cases {
        assert_serializes_linear(label, 1 << 8, &default, |size| {
            Document::parse(build(size), &parse).unwrap()
        });
    }
    assert_serializes_linear("items after separators", 1 << 8, &default, separated_items);
    // Finding the task marker does not depend on the options. With these, definitions are joined
    // without a blank line.
    let both = [
        ("default", Options::default()),
        (
            "combination",
            Options {
                fences: false,
                setext: true,
                list_item_indent: IndentOptions::Mixed,
                tight_definitions: true,
                ..Options::default()
            },
        ),
    ];
    assert_serializes_linear(
        "task markers after definitions",
        1 << 8,
        &both,
        task_item_after_definitions,
    );
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

/// A block of a tight item, before the block `next` builds after it and an item `q`.
enum Before {
    Definition(&'static str),
    Paragraph,
    /// A block quote holding a definition.
    Quote,
    /// A list of one item holding a definition.
    List,
}

/// Writes a tight list whose first item holds `before` and the block `next` adds to it, then
/// checks the Markdown reads back as the same tree, with the list tight, and writes the same.
fn assert_tight_join(
    before: Before,
    next: impl Fn(&mut Document<String>, base_markdown::NodeId),
    options: &Options,
    expected: &str,
) {
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    let item = document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    let definition = |destination| EventKind::Definition {
        label: Value::Literal("c"),
        destination: Value::Literal(destination),
        title: None,
    };
    match before {
        Before::Definition(destination) => {
            document
                .append_child(item, definition(destination))
                .unwrap();
        }
        Before::Paragraph => {
            let paragraph = document
                .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
                .unwrap();
            document
                .append_child(paragraph, EventKind::Text(Value::Literal("a")))
                .unwrap();
        }
        Before::Quote | Before::List => {
            let (tag, inner) = match before {
                Before::Quote => (Tag::BlockQuote, None),
                _ => (
                    Tag::List {
                        start: None,
                        tight: true,
                    },
                    Some(Tag::Item { block_end: false }),
                ),
            };
            let mut container = document.append_child(item, EventKind::Start(tag)).unwrap();
            if let Some(inner) = inner {
                container = document
                    .append_child(container, EventKind::Start(inner))
                    .unwrap();
            }
            document.append_child(container, definition("/u")).unwrap();
        }
    }
    next(&mut document, item);
    let item = document
        .append_child(list, EventKind::Start(Tag::Item { block_end: false }))
        .unwrap();
    let paragraph = document
        .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document
        .append_child(paragraph, EventKind::Text(Value::Literal("q")))
        .unwrap();
    document.normalize();
    let markdown = to_markdown_with_options(&document, options).unwrap();
    assert_eq!(markdown, expected);
    let mut reparsed = Document::parse(markdown.clone(), &ParseOptions::default()).unwrap();
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    assert_eq!(
        to_markdown_with_options(&reparsed, options).unwrap(),
        markdown
    );
}

fn add_block(
    tag: Tag<'static>,
    text: &'static str,
) -> impl Fn(&mut Document<String>, base_markdown::NodeId) {
    move |document, item| {
        let block = document
            .append_child(item, EventKind::Start(tag.clone()))
            .unwrap();
        document
            .append_child(block, EventKind::Text(Value::Literal(text)))
            .unwrap();
    }
}

#[test]
fn escapes_a_title_after_an_untitled_definition_in_a_tight_item() {
    // Joined to a definition without a title by one line ending, a paragraph or setext heading
    // starting with `"`, `'` or `(` would read back as its title (#173). A blank line would make
    // the list loose, so the character is escaped. Text that would not read as a title is not.
    let paragraph = Tag::Paragraph { tight: false };
    let setext = Options {
        setext: true,
        ..Options::default()
    };
    let tight_definitions = Options {
        tight_definitions: true,
        ..Options::default()
    };
    for (destination, text, expected) in [
        ("/u", "\"x\"", "- [c]: /u\n  \\\"x\"\n- q\n"),
        ("/u", "'x'", "- [c]: /u\n  \\'x'\n- q\n"),
        ("/u", "(x)", "- [c]: /u\n  \\(x)\n- q\n"),
        ("/u", "\"x\ny\"", "- [c]: /u\n  \\\"x\n  y\"\n- q\n"),
        ("", "\"x\"", "- [c]: <>\n  \\\"x\"\n- q\n"),
        ("/u", "\"x\" y", "- [c]: /u\n  \"x\" y\n- q\n"),
    ] {
        for options in [&Options::default(), &tight_definitions] {
            assert_tight_join(
                Before::Definition(destination),
                add_block(paragraph.clone(), text),
                options,
                expected,
            );
        }
    }
    assert_tight_join(
        Before::Definition("/u"),
        add_block(Tag::Heading(1), "\"x\""),
        &setext,
        "- [c]: /u\n  \\\"x\"\n  ====\n- q\n",
    );
}

#[test]
fn fences_code_joined_by_a_line_ending_to_text_in_a_tight_item() {
    // Indented code after a definition or paragraph, or a container ending in one, would continue
    // it, lazily through the container, and `<!---->` would make the list loose (#173).
    let options = Options {
        fences: false,
        ..Options::default()
    };
    let code = |document: &mut Document<String>, item| {
        document
            .append_child(
                item,
                EventKind::CodeBlock {
                    value: Value::Literal("x"),
                    language: None,
                    fence: None,
                    math: false,
                    unclosed: false,
                },
            )
            .unwrap();
    };
    for (before, expected) in [
        (
            Before::Definition("/u"),
            "- [c]: /u\n  ```\n  x\n  ```\n- q\n",
        ),
        (Before::Paragraph, "- a\n  ```\n  x\n  ```\n- q\n"),
        (Before::Quote, "- > [c]: /u\n  ```\n  x\n  ```\n- q\n"),
        (Before::List, "- - [c]: /u\n  ```\n  x\n  ```\n- q\n"),
    ] {
        assert_tight_join(before, code, &options, expected);
    }
}
