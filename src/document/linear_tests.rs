// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Normalizing and editing documents does work linear in the input, counted with [`crate::work`].

extern crate std;

use crate::stream::{Event, EventKind, StreamError, Tag, TagKind, Text};
use crate::work::tests::{assert_against_control, assert_linear, counted};
use crate::{to_markdown, AstBuilder, Document, NodeId, NodeSyntax, ParseOptions};
use std::borrow::ToOwned;
use std::string::String;
use std::vec::Vec;
use std::{format, vec};

// The builder takes results, as the parser yields them.
#[allow(clippy::unnecessary_wraps)]
fn event(
    kind: EventKind<'static>,
    span: std::ops::Range<usize>,
) -> Result<Event<'static>, StreamError> {
    Ok(Event { kind, span })
}

fn text(value: &str) -> Result<Event<'static>, StreamError> {
    event(EventKind::Text(Text::from(value.to_owned())), 0..0)
}

fn start(tag: Tag<'static>) -> Result<Event<'static>, StreamError> {
    event(EventKind::Start(tag), 0..0)
}

fn end(kind: TagKind) -> Result<Event<'static>, StreamError> {
    event(EventKind::End(kind), 0..0)
}

fn separator() -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Separator {
            value: Text::Literal("\n"),
            conditional: false,
        },
        0..0,
    )
}

/// The default options, with inline code languages, which the built trees have.
fn code_info() -> ParseOptions {
    ParseOptions {
        inline_code_info: true,
        ..ParseOptions::default()
    }
}

/// Wrap phrasing events in a document and paragraph, and build a document over `source`.
fn build_in(source: &str, phrasing: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![
        start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        start(Tag::Paragraph { tight: false }),
    ];
    events.extend(phrasing);
    events.push(end(TagKind::Paragraph));
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::with_options(code_info());
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    builder.finish(source.to_owned()).unwrap()
}

fn build(phrasing: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    build_in("", phrasing)
}

fn attention(
    tag: Tag<'static>,
    kind: TagKind,
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = vec![start(tag)];
    events.extend(children);
    events.push(end(kind));
    events
}

fn emphasis(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Emphasis, TagKind::Emphasis, children)
}

fn strong(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Strong, TagKind::Strong, children)
}

fn delete(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(Tag::Delete, TagKind::Delete, children)
}

fn concat<T>(parts: impl IntoIterator<Item = Vec<T>>) -> Vec<T> {
    parts.into_iter().flatten().collect()
}

/// Count the steps of normalizing the document from `build` and serializing the result, with `size` against eight times as many units. Serializing a small tree before normalizing checks that it does not panic; that need not be linear, so it is not counted.
fn assert_normalize_linear(label: &str, size: usize, build: impl Fn(usize) -> Document<String>) {
    to_markdown(&build(size)).unwrap();
    assert_linear(label, size, |size| {
        let mut document = build(size);
        counted(|| {
            document.normalize();
            assert!(document.is_normalized());
            to_markdown(&document).unwrap();
        })
    });
}

#[test]
fn joining_text_is_linear() {
    // Joining copies each text once, rather than what was joined so far on each join, which allocates bytes that grow with the square of the input. The spaces that hard breaks in a table cell become are joined the same way.
    assert_normalize_linear("text", 1 << 14, |size| {
        build((0..size).map(|_| text("a")).collect())
    });
}

#[test]
fn joining_code_is_linear() {
    // Code joins like text, but takes its own arm of the join.
    assert_normalize_linear("code", 1 << 10, |size| {
        build((0..size).map(|_| code("a", None)).collect())
    });
}

#[test]
fn merging_attention_is_linear() {
    assert_normalize_linear("attention", 1 << 11, |size| {
        build(concat(
            (0..size).map(|_| emphasis(vec![text("a"), text("b")])),
        ))
    });
}

#[test]
fn merging_nested_attention_is_linear() {
    // Each merge exposes another pair to merge one level down. The serializer recurses per level, so only normalizing is counted.
    let nested = |depth: usize| {
        let mut events = Vec::new();
        for _ in 0..depth {
            events.push(start(Tag::Strong));
            events.push(start(Tag::Emphasis));
        }
        events.push(text("a"));
        for _ in 0..depth {
            events.push(end(TagKind::Emphasis));
            events.push(end(TagKind::Strong));
        }
        events
    };
    assert_linear("nested attention", 1 << 11, |size| {
        let mut document = build(concat([nested(size / 4), nested(size / 4)]));
        let steps = counted(|| document.normalize());
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), size / 2 + 3);
        steps
    });
}

#[test]
fn removing_empty_attention_is_linear() {
    assert_normalize_linear("empty attention", 1 << 11, |size| {
        build(concat([
            vec![text("a")],
            concat((0..size / 4).map(|_| {
                concat([
                    emphasis(vec![text("")]),
                    strong(vec![]),
                    delete(emphasis(vec![])),
                ])
            })),
            vec![text("b")],
        ]))
    });
}

#[test]
fn removing_empty_children_between_kept_ones_is_linear() {
    // Removing continues from the child kept before what was removed, not from the first child.
    assert_normalize_linear("empty text after text", 1 << 10, |size| {
        build(concat((0..size).map(|_| vec![text("a"), text("")])))
    });
    assert_normalize_linear("empty attention after text", 1 << 10, |size| {
        build(concat(
            (0..size).map(|_| concat([vec![text("a")], strong(vec![])])),
        ))
    });
}

#[test]
fn removing_nested_empty_attention_is_linear() {
    // The serializer recurses per level, so only normalizing is counted.
    assert_linear("nested empty attention", 1 << 11, |size| {
        let mut events = vec![text("a")];
        events.extend((0..size).map(|_| start(Tag::Emphasis)));
        events.push(text(""));
        events.extend((0..size).map(|_| end(TagKind::Emphasis)));
        events.push(text("b"));
        let mut document = build(events);
        let steps = counted(|| document.normalize());
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), 3);
        steps
    });
}

#[test]
fn unwrapping_nested_strikethrough_is_linear() {
    // Whether a node is inside strikethrough is counted on the way down, not found by searching its ancestors. Strikethrough alternating with emphasis keeps the emphasis, so it stays deep, and only normalizing is counted, as the serializer recurses per level.
    assert_linear("strikethrough alternating with emphasis", 1 << 11, |size| {
        let mut events = Vec::new();
        for _ in 0..size / 2 {
            events.push(start(Tag::Delete));
            events.push(start(Tag::Emphasis));
        }
        events.push(text("a"));
        for _ in 0..size / 2 {
            events.push(end(TagKind::Emphasis));
            events.push(end(TagKind::Delete));
        }
        let mut document = build(events);
        let steps = counted(|| document.normalize());
        assert!(document.is_normalized());
        assert_eq!(document.descendants(document.root()).count(), size / 2 + 4);
        steps
    });
}

#[test]
fn appending_to_an_item_is_linear() {
    // Appending finds the parent's last child from the child appended before, rather than walking its children, and refreshes an item's ending from the appended child alone. An item takes both paths.
    assert_linear("appending to an item", 1 << 11, |size| {
        let mut document = Document::parse("- a\n", &ParseOptions::default()).unwrap();
        let list = document.children(document.root()).next().unwrap();
        let item = document.children(list).next().unwrap();
        let steps = counted(|| {
            for _ in 0..size {
                let paragraph = document
                    .append_child(item, EventKind::Start(Tag::Paragraph { tight: true }))
                    .unwrap();
                document
                    .append_child(paragraph, EventKind::Text(Text::Literal("b")))
                    .unwrap();
            }
        });
        assert_eq!(document.children(item).count(), size + 1);
        steps
    });
}

fn code(
    value: &'static str,
    language: Option<&'static str>,
) -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Code {
            value: Text::Literal(value),
            language: language.map(Text::Literal),
        },
        0..0,
    )
}

fn soft() -> Result<Event<'static>, StreamError> {
    event(EventKind::SoftBreak(Text::Literal("\n")), 0..0)
}

fn hard() -> Result<Event<'static>, StreamError> {
    event(EventKind::HardBreak, 0..0)
}

/// Build a document of block events.
fn build_blocks(blocks: Vec<Result<Event<'static>, StreamError>>) -> Document<String> {
    let mut events = vec![start(Tag::Document {
        line_ending: None,
        frontmatter: None,
    })];
    events.extend(blocks);
    events.push(end(TagKind::Document));
    let mut builder = AstBuilder::with_options(code_info());
    builder
        .consume(&mut events.into_iter(), usize::MAX)
        .unwrap();
    builder.finish(String::new()).unwrap()
}

fn paragraph(
    children: Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    attention(
        Tag::Paragraph { tight: false },
        TagKind::Paragraph,
        children,
    )
}

fn html(value: &'static str) -> Result<Event<'static>, StreamError> {
    event(
        EventKind::Html {
            value: Text::Literal(value),
            block: false,
        },
        0..0,
    )
}

/// Count the steps of normalizing the document from `build`, without serializing, for trees nested deeper than the serializer recurses comfortably, and check what is left with `check`.
fn assert_normalize_only_linear(
    label: &str,
    size: usize,
    build: impl Fn(usize) -> Document<String>,
    check: impl Fn(&Document<String>, usize),
) {
    assert_linear(label, size, |size| {
        let mut document = build(size);
        let steps = counted(|| document.normalize());
        assert!(document.is_normalized());
        check(&document, size);
        steps
    });
}

/// Attention nested `depth` deep, alternating emphasis and strong, each level holding `before`, the next level and `after`, around `inner`.
fn nested_edges(
    depth: usize,
    before: impl Fn() -> Vec<Result<Event<'static>, StreamError>>,
    inner: Vec<Result<Event<'static>, StreamError>>,
    after: impl Fn() -> Vec<Result<Event<'static>, StreamError>>,
) -> Vec<Result<Event<'static>, StreamError>> {
    let mut events = Vec::new();
    for level in 0..depth {
        events.push(start(if level % 2 == 0 {
            Tag::Emphasis
        } else {
            Tag::Strong
        }));
        events.extend(before());
    }
    events.extend(inner);
    for level in (0..depth).rev() {
        events.extend(after());
        events.push(end(if level % 2 == 0 {
            TagKind::Emphasis
        } else {
            TagKind::Strong
        }));
    }
    events
}

#[test]
fn moving_whitespace_out_of_deep_attention_is_linear() {
    // Each level starts and ends with whitespace, which moves out through every level outside it. Attention already holds no whitespace at its edges once its parent has moved it out, so it is not searched again. Hard breaks at the end of each level take the same path.
    assert_normalize_only_linear(
        "whitespace at the edges of each level",
        1 << 9,
        |size| {
            build(concat([
                vec![text("x")],
                nested_edges(
                    size / 4,
                    || vec![text(" "), soft()],
                    vec![text("a")],
                    || vec![text(" ")],
                ),
                vec![text("y")],
            ]))
        },
        |document, size| {
            // Before the levels, the text that moved out stays apart between the soft breaks, the first joining "x"; after them, it all joins "y".
            assert_eq!(
                document.descendants(document.root()).count(),
                2 + size / 4 * 3 + 2
            );
        },
    );
    // Whitespace alone inside: every level is emptied and removed, and moving out continues from the level above, not from the outermost.
    assert_normalize_only_linear(
        "whitespace alone inside",
        1 << 9,
        |size| {
            build(concat([
                vec![text("x")],
                nested_edges(
                    size / 4,
                    || vec![text(" ")],
                    vec![soft()],
                    || vec![text("\t")],
                ),
                vec![text("y")],
            ]))
        },
        |document, _| {
            let paragraph = document.children(document.root()).next().unwrap();
            // The spaces join "x", the soft break stays apart, and the tabs join "y".
            assert_eq!(document.children(paragraph).count(), 3);
        },
    );
    // Whitespace at an edge of each of many sibling attention children: normalizing continues from what moved, not from the first child.
    let siblings = |label: &str, unit: &dyn Fn() -> Vec<Result<Event<'static>, StreamError>>| {
        assert_normalize_only_linear(
            label,
            1 << 9,
            |size| build(concat((0..size).map(|_| unit()))),
            |document, size| {
                let paragraph = document.children(document.root()).next().unwrap();
                assert_eq!(document.children(paragraph).count(), 2 * size);
            },
        );
    };
    siblings("whitespace leading each sibling", &|| {
        concat([vec![text("b")], emphasis(vec![text(" a")])])
    });
    siblings("whitespace trailing each sibling", &|| {
        concat([emphasis(vec![text("a ")]), vec![text("b")]])
    });
}

#[test]
fn removing_break_chains_is_linear() {
    // At the end of a block, where the whole run is cut off at once.
    assert_normalize_linear("breaks at the end of a block", 1 << 9, |size| {
        build(concat([
            vec![text("a")],
            (0..size).map(|_| hard()).collect(),
        ]))
    });
    // At the end of attention, followed by more text, where the whole run moves out at once. In a table cell, each becomes a space and is joined, as `joining_text_is_linear` checks.
    assert_normalize_linear("breaks at the end of attention", 1 << 9, |size| {
        build(concat([
            emphasis(concat([
                vec![text("a")],
                (0..size).map(|_| hard()).collect(),
            ])),
            vec![text("b")],
        ]))
    });
}

/// A block of a list tree built through the document API, which keeps paragraph and item rendering hints in step with the list.
enum Block {
    Paragraph(Vec<EventKind<'static>>),
    /// A list's start and tightness, and each item's recorded spread and blocks.
    List(Option<u32>, bool, Vec<(bool, Vec<Block>)>),
    Code,
    Quote(Vec<Block>),
    /// A heading of rank 1 and its phrasing.
    Heading(Vec<EventKind<'static>>),
    Separator,
}

fn append_block(document: &mut Document<String>, parent: NodeId, block: &Block) {
    match block {
        Block::Paragraph(phrasing) => {
            let id = document
                .append_child(parent, EventKind::Start(Tag::Paragraph { tight: false }))
                .unwrap();
            for kind in phrasing {
                document.append_child(id, kind.clone()).unwrap();
            }
        }
        Block::List(start, tight, items) => {
            let list = document
                .append_child(
                    parent,
                    EventKind::Start(Tag::List {
                        start: *start,
                        tight: *tight,
                    }),
                )
                .unwrap();
            for (spread, blocks) in items {
                let item = document
                    .append_child(
                        list,
                        EventKind::Start(Tag::Item {
                            block_end: false,
                            checked: None,
                        }),
                    )
                    .unwrap();
                document
                    .set_syntax(item, Some(NodeSyntax::ListItem { spread: *spread }))
                    .unwrap();
                for block in blocks {
                    append_block(document, item, block);
                }
            }
        }
        Block::Quote(blocks) => {
            let quote = document
                .append_child(parent, EventKind::Start(Tag::BlockQuote))
                .unwrap();
            for block in blocks {
                append_block(document, quote, block);
            }
        }
        Block::Heading(phrasing) => {
            let id = document
                .append_child(parent, EventKind::Start(Tag::Heading(1)))
                .unwrap();
            for kind in phrasing {
                document.append_child(id, kind.clone()).unwrap();
            }
        }
        Block::Separator => {
            document
                .append_child(
                    parent,
                    EventKind::Separator {
                        value: Text::Literal("\n"),
                        conditional: false,
                    },
                )
                .unwrap();
        }
        Block::Code => {
            document
                .append_child(
                    parent,
                    EventKind::CodeBlock {
                        value: Text::from("x\n".to_owned()),
                        info: None,
                        fence: None,
                        unclosed: false,
                    },
                )
                .unwrap();
        }
    }
}

fn build_list(block: &Block) -> Document<String> {
    let mut document = Document::parse(String::new(), &ParseOptions::default()).unwrap();
    let root = document.root();
    append_block(&mut document, root, block);
    document
}

#[test]
fn tightening_lone_loose_items_is_linear() {
    // Lone loose items nested in each other, each left with one block once its empty paragraph is removed. Appending nests in time linear in the depth, and serializing nests too deeply for the stack, so the tree is built from events and only normalizing is counted.
    assert_linear("nested lone loose items", 1 << 8, |size| {
        let mut events = Vec::new();
        for _ in 0..size {
            events.extend([
                start(Tag::List {
                    start: None,
                    tight: false,
                }),
                start(Tag::Item {
                    block_end: false,
                    checked: None,
                }),
            ]);
            events.extend(paragraph(vec![]));
        }
        events.extend(paragraph(vec![text("a")]));
        for _ in 0..size {
            events.extend([end(TagKind::Item), end(TagKind::List)]);
        }
        let mut document = build_blocks(events);
        let steps = counted(|| document.normalize());
        assert!(document.is_normalized());
        steps
    });
    // A lone item of many empty paragraphs and one that is not.
    assert_normalize_linear("a lone item of empty paragraphs", 1 << 8, |size| {
        let mut blocks: Vec<Block> = (0..size).map(|_| Block::Paragraph(vec![])).collect();
        blocks.push(Block::Paragraph(vec![EventKind::Text(Text::from(
            "a".to_owned(),
        ))]));
        build_list(&Block::List(None, false, vec![(false, blocks)]))
    });
    // A lone item of many blocks, which is written spread.
    assert_normalize_linear("a lone item of many blocks", 1 << 8, |size| {
        build_list(&Block::List(
            None,
            false,
            vec![(false, (0..size).map(|_| Block::Code).collect())],
        ))
    });
}

#[test]
fn joining_paragraphs_and_loosening_lists_in_tight_items_is_linear() {
    use Block::{List, Paragraph, Quote, Separator};
    let text = || EventKind::Text(Text::from("a".to_owned()));
    let tight = |items: Vec<(bool, Vec<Block>)>| List(None, true, items);
    // An item of many paragraphs of a few children each, joined into one, and a paragraph after many block quotes that end in a paragraph through block quotes in them. Serializing these does much more work than normalizing them, which would hide part of the growth of normalizing, so only normalizing is counted, at a size where joining that walks to the last child of the paragraph so far on each join already fails, and the joined paragraph is serialized once at a smaller size.
    let paragraphs = |size: usize| {
        let blocks = (0..size)
            .flat_map(|_| {
                [
                    Paragraph(vec![text(), EventKind::HardBreak, text()]),
                    Separator,
                ]
            })
            .collect();
        build_list(&tight(vec![(false, blocks)]))
    };
    assert_normalize_only_linear(
        "paragraphs in a row in a tight item",
        1 << 8,
        paragraphs,
        |_, _| {},
    );
    assert_normalize_linear("serializing joined paragraphs", 1 << 6, paragraphs);
    let quotes = |size: usize| {
        let blocks = (0..size)
            .flat_map(|_| {
                [
                    Quote(vec![Quote(vec![Paragraph(vec![text()])])]),
                    Separator,
                    Paragraph(vec![text()]),
                    Separator,
                ]
            })
            .collect();
        build_list(&tight(vec![(false, blocks)]))
    };
    assert_normalize_only_linear(
        "block quotes and paragraphs in a tight item",
        1 << 8,
        quotes,
        |_, _| {},
    );
    // Each paragraph after a block quote is written after a line of `>`, and each pair of block quotes in a row in a spread item of a loose list is checked for a blank line, which is linear only if each join looks at a bounded number of blocks.
    assert_normalize_linear("serializing block quotes and paragraphs", 1 << 8, quotes);
    assert_normalize_linear(
        "serializing block quotes in a row in a loose list",
        1 << 8,
        |size| {
            let blocks = (0..size)
                .flat_map(|_| [Quote(vec![Paragraph(vec![text()])]), Separator])
                .collect();
            build_list(&List(
                None,
                false,
                vec![(true, blocks), (false, vec![Paragraph(vec![text()])])],
            ))
        },
    );
    // Each heading with a line ending in its text after a paragraph is written ATX, which is linear only if each looks at the block before it alone.
    assert_linear(
        "serializing headings with line endings after paragraphs in a tight item",
        1 << 8,
        |size| {
            let blocks = (0..size)
                .flat_map(|_| {
                    [
                        Paragraph(vec![text()]),
                        Block::Heading(vec![EventKind::Text(Text::from("a\nb".to_owned()))]),
                    ]
                })
                .collect();
            let mut document = build_list(&tight(vec![(false, blocks)]));
            counted(|| {
                document.normalize();
                assert!(document.is_normalized());
                crate::to_markdown(&document).unwrap();
            })
        },
    );
    // A tight list of many items, of which only the last needs it to be loose.
    assert_normalize_only_linear(
        "a tight list made loose by its last item",
        1 << 8,
        |size| {
            let mut items: Vec<(bool, Vec<Block>)> = (0..size)
                .map(|_| (false, vec![Paragraph(vec![text()])]))
                .collect();
            items.push((false, vec![Quote(vec![]), Separator, Quote(vec![])]));
            build_list(&tight(items))
        },
        |_, _| {},
    );
}

#[test]
fn removing_unwritable_html_and_lists_is_linear() {
    // Each is removed with one separator in one pass over its parent's children, and inline HTML in the pass over its paragraph's.
    assert_linear("unwritable blocks", 1 << 10, |size| {
        let mut events = Vec::new();
        for _ in 0..size / 4 {
            events.extend(attention(
                Tag::List {
                    start: None,
                    tight: true,
                },
                TagKind::List,
                vec![],
            ));
            events.push(separator());
            events.extend(paragraph(vec![html(""), text("a"), html("")]));
        }
        let mut document = build_blocks(events);
        let steps = counted(|| {
            document.normalize();
            assert!(document.is_normalized());
            to_markdown(&document).unwrap();
        });
        assert_eq!(document.children(document.root()).count(), size / 4);
        steps
    });
}

/// The cells of a row, each with its alignment, whether it is a header cell, and its text.
type Row = Vec<(crate::AlignKind, bool, &'static str)>;

/// A table of `groups`, each a head or body of rows of cells, each with its alignment, whether it is a header cell, and its text.
fn shaped_table(groups: Vec<(TagKind, Vec<Row>)>) -> Document<String> {
    let mut events = Vec::new();
    for (kind, rows) in groups {
        let tag = match kind {
            TagKind::TableHead => Tag::TableHead,
            _ => Tag::TableBody,
        };
        let mut group = Vec::new();
        for row in rows {
            let mut cells = Vec::new();
            for (alignment, header, value) in row {
                let children = if value.is_empty() {
                    vec![]
                } else {
                    vec![text(value)]
                };
                cells.extend(attention(
                    Tag::TableCell { alignment, header },
                    TagKind::TableCell,
                    children,
                ));
            }
            group.extend(attention(Tag::TableRow, TagKind::TableRow, cells));
            group.push(separator());
        }
        events.extend(attention(tag, kind, group));
        events.push(separator());
    }
    build_blocks(attention(Tag::Table, TagKind::Table, events))
}

#[test]
fn shaping_tables_is_linear() {
    use crate::AlignKind::None as Plain;
    assert_normalize_linear("unshaped tables", 1 << 10, |size| {
        let rows = (0..size)
            .map(|index| vec![(Plain, true, "a"); index % 3])
            .collect();
        shaped_table(vec![(TagKind::TableBody, rows)])
    });
}

fn items<S: AsRef<str>>(document: &Document<S>, root: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    document.descendants(root).filter(move |&id| {
        matches!(
            document.node(id).unwrap().kind,
            EventKind::Start(Tag::Item { .. })
        )
    })
}

fn spread<S: AsRef<str>>(document: &Document<S>, item: NodeId) -> bool {
    matches!(
        document.syntax(item),
        Some(NodeSyntax::ListItem { spread: true })
    )
}

#[test]
fn item_spread_counts_a_blank_line_after_a_nested_list_in_linear_time() {
    // The blank line is found through the last blocks of nested lists, each looked through once.
    assert_linear("blank line after nested lists", 1 << 9, |size| {
        let source = format!("{}a\n\n  # h\n", "- ".repeat(size));
        let document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
        counted(|| {
            assert_eq!(
                items(&document, document.root())
                    .filter(|&item| spread(&document, item))
                    .count(),
                1
            );
        })
    });
}

fn gfm() -> ParseOptions {
    ParseOptions::gfm()
}

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &gfm()).unwrap()
}

/// The first node in document order whose kind `matches` accepts.
fn find(document: &Document, matches: impl Fn(&EventKind<'_>) -> bool) -> NodeId {
    document
        .descendants(document.root())
        .find(|id| matches(&document.node(*id).unwrap().kind))
        .unwrap()
}

/// Parsing into every paragraph of a document does work linear in the document: each parse
/// looks up the definitions its fragment refers to, without searching the document (#121). The
/// paragraphs of one large document are counted against the same paragraphs, one in each of as
/// many small documents, a control of the same work whose documents are short, so the ratio of
/// the two growths shows only work that depends on the size of the document, which searching the
/// document's blocks in each parse would make grow with it.
#[test]
fn parsing_into_every_paragraph_of_a_document_is_linear() {
    let definitions = "[r]: /u\n\n[^n]: Note.\n\n";
    let steps = |mut documents: Vec<Document>| {
        let paragraphs: Vec<Vec<NodeId>> = documents
            .iter()
            .map(|document| {
                document
                    .children(document.root())
                    .filter(|id| {
                        matches!(
                            document.node(*id).unwrap().kind,
                            EventKind::Start(Tag::Paragraph { .. })
                        )
                    })
                    .collect()
            })
            .collect();
        counted(|| {
            for (document, paragraphs) in documents.iter_mut().zip(&paragraphs) {
                for &paragraph in paragraphs {
                    document
                        .parse_children(paragraph, "q [r] *s* [^n]")
                        .unwrap();
                }
            }
        })
    };
    assert_against_control(
        "parse_children into each paragraph",
        1 << 8,
        |size| {
            steps(vec![parse(
                &(String::from(definitions) + &"p\n\n".repeat(size)),
            )])
        },
        |size| {
            steps(
                (0..size)
                    .map(|_| parse(&(String::from(definitions) + "p\n")))
                    .collect(),
            )
        },
    );
}

/// Parsing into each of many sections, each a block quote that defines a label of its own, which
/// the fragment defines again and refers to, takes time linear in the sections: each parse asks
/// the index about the labels its fragment uses and passes over the replaced children's own
/// definitions, without searching the document. Counted against the same sections, one in each of
/// as many small documents.
#[test]
fn parsing_into_every_section_of_a_document_is_linear() {
    let section = |index: usize| format!("> [d{index}]: /old\n>\n> [d{index}] [r]\n\n");
    let build = |sections: usize, first: usize| {
        let mut source = String::from("[r]: /u\n\n");
        for index in first..first + sections {
            source.push_str(&section(index));
        }
        parse(&source)
    };
    let steps = |mut documents: Vec<Document>| {
        let quotes: Vec<Vec<(NodeId, usize)>> = documents
            .iter()
            .map(|document| {
                document
                    .children(document.root())
                    .filter(|id| quote(&document.node(*id).unwrap().kind))
                    .map(|id| {
                        let definition = document.children(id).next().unwrap();
                        let label = match &document.node(definition).unwrap().kind {
                            EventKind::Definition { label, .. } => label
                                .resolve(document.source())
                                .unwrap()
                                .trim_start_matches('d')
                                .parse()
                                .unwrap(),
                            _ => unreachable!(),
                        };
                        (id, label)
                    })
                    .collect()
            })
            .collect();
        let fragments: Vec<Vec<String>> = quotes
            .iter()
            .map(|quotes| {
                quotes
                    .iter()
                    .map(|(_, label)| format!("[d{label}] [r] [^n]\n\n[d{label}]: /new\n"))
                    .collect()
            })
            .collect();
        counted(|| {
            for ((document, quotes), fragments) in documents.iter_mut().zip(&quotes).zip(&fragments)
            {
                for ((quote, _), fragment) in quotes.iter().zip(fragments) {
                    document.parse_children(*quote, fragment).unwrap();
                }
            }
        })
    };
    assert_against_control(
        "parse_children into each section",
        1 << 9,
        |size| steps(vec![build(size, 0)]),
        |size| steps((0..size).map(|index| build(1, index)).collect()),
    );
}

/// Parsing into the innermost paragraph and block quote of a deep nest of block quotes, again and
/// again, takes time linear in the parses whatever the depth: a block quote and a paragraph are
/// below no link or cell they could be in, so nothing looks at the quotes around them. Counted
/// against the same parses into a nest of one.
#[test]
fn parsing_deep_into_a_nest_is_linear() {
    let steps = |depth: usize, parses: usize| {
        let mut source = String::from("[r]: /u\n\n");
        source.push_str(&">".repeat(depth));
        source.push_str(" p\n");
        let mut document = parse(&source);
        let paragraph = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::Paragraph { .. }))
        });
        let quote = document.node(paragraph).unwrap().parent().unwrap();
        counted(|| {
            for _ in 0..parses {
                document
                    .parse_children(quote, "q [r] [d]\n\n[d]: /d\n")
                    .unwrap();
                let paragraph = document.children(quote).next().unwrap();
                document.parse_children(paragraph, "s [r] [d]").unwrap();
            }
        })
    };
    assert_against_control(
        "parse_children deep in a nest",
        1 << 8,
        |size| steps(size, size),
        |size| steps(1, size),
    );
}

fn quote(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::BlockQuote))
}
