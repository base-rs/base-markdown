// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// `Document::definition` and `Document::footnote_definition` find the first attached definition
// of a label as the document is edited (#121).

mod support;

use base_markdown::{
    stream::{EventKind, Tag, Text},
    to_markdown, Document, NodeId, ParseOptions,
};
use std::time::Duration;
use support::linear::{assert_against_control, timed};

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

/// The destination of the definition `label` finds.
fn destination(document: &Document, label: &str) -> Option<String> {
    let id = document.definition(label)?;
    match &document.node(id).unwrap().kind {
        EventKind::Definition { destination, .. } => {
            Some(destination.resolve(document.source()).unwrap().to_owned())
        }
        kind => panic!("not a definition: {:?}", kind),
    }
}

/// The text of the footnote definition `label` finds.
fn note(document: &Document, label: &str) -> Option<String> {
    let id = document.footnote_definition(label)?;
    assert!(matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::FootnoteDefinition { .. })
    ));
    Some(document.text_content(id))
}

fn definition(label: &'static str, destination: &'static str) -> EventKind<'static> {
    EventKind::Definition {
        label: Text::Literal(label),
        destination: Text::Literal(destination),
        title: None,
    }
}

fn footnote_definition(identifier: &'static str, label: &'static str) -> EventKind<'static> {
    EventKind::Start(Tag::FootnoteDefinition {
        identifier: Text::Literal(identifier),
        label: Some(Text::Literal(label)),
    })
}

/// The direct children of `parent` whose kind `matches` accepts.
fn children(
    document: &Document,
    parent: NodeId,
    matches: impl Fn(&EventKind<'_>) -> bool,
) -> Vec<NodeId> {
    document
        .children(parent)
        .filter(|id| matches(&document.node(*id).unwrap().kind))
        .collect()
}

fn quote(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::BlockQuote))
}

#[test]
fn a_label_finds_the_first_definition_in_document_order_however_deep() {
    let document = parse(
        "> - [A  b]: /nested\n\n[a b]: /root\n\n[^Note]: One.\n\n> [^note]: Two.\n\n[^other]: Three.\n",
    );
    // Labels match as the parser matches them.
    assert_eq!(destination(&document, "a b").as_deref(), Some("/nested"));
    assert_eq!(destination(&document, "A\tB").as_deref(), Some("/nested"));
    assert_eq!(note(&document, "NOTE").as_deref(), Some("One."));
    assert_eq!(note(&document, "Other").as_deref(), Some("Three."));
    assert_eq!(destination(&document, "missing"), None);
    assert_eq!(note(&document, "missing"), None);
    // The two kinds of definition do not share labels.
    assert_eq!(destination(&document, "note"), None);
    assert_eq!(note(&document, "a b"), None);
}

#[test]
fn appending_a_definition_finds_it_only_in_the_tree() {
    let mut document = parse("[a]: /first\n\n> Quote.\n");
    let root = document.root();
    document
        .append_child(root, definition("A", "/second"))
        .unwrap();
    let appended = document.append_child(root, definition("b", "/b")).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/first"));
    assert_eq!(document.definition("B"), Some(appended));
    document
        .append_child(root, footnote_definition("N", "n"))
        .unwrap();
    assert!(document.footnote_definition("n").is_some());
    // A definition appended to a detached subtree is not in the tree.
    let quote = children(&document, root, quote)[0];
    document.detach(quote).unwrap();
    document.append_child(quote, definition("c", "/c")).unwrap();
    assert_eq!(document.definition("c"), None);
    // Attaching the subtree brings its definitions into the tree.
    document.attach(root, quote, None).unwrap();
    assert_eq!(destination(&document, "c").as_deref(), Some("/c"));
}

#[test]
fn detaching_moving_and_removing_definitions_follow_the_tree() {
    let mut document = parse("> [a]: /quote\n\n[a]: /root\n\n- [a]: /item\n");
    let root = document.root();
    let quote = children(&document, root, quote)[0];
    assert_eq!(destination(&document, "a").as_deref(), Some("/quote"));
    document.detach(quote).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/root"));
    // Detaching a detached subtree, or a node in one, changes nothing.
    document.detach(quote).unwrap();
    let inner = document.children(quote).next().unwrap();
    document.detach(inner).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/root"));
    document.attach(quote, inner, None).unwrap();
    // A subtree attached last comes last, and one attached first comes first.
    document.attach(root, quote, None).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/root"));
    let first = document.children(root).next();
    document.attach(root, quote, first).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/quote"));
    // Moving a definition within the tree keeps it.
    let list = children(&document, root, |kind| {
        matches!(kind, EventKind::Start(Tag::List { .. }))
    })[0];
    let item = document.children(list).next().unwrap();
    document.attach(item, inner, None).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/root"));
    // Removing children removes their definitions.
    document.extract_children(root).unwrap();
    assert_eq!(document.definition("a"), None);
    // Removing the children of a detached node changes nothing in the tree.
    document.extract_children(item).unwrap();
    assert_eq!(document.definition("a"), None);
}

#[test]
fn changing_a_definitions_label_moves_it_to_its_new_label() {
    let mut document = parse("[a]: /a\n\n[b]: /b\n\n[^n]: Note.\n");
    let first = document.children(document.root()).next().unwrap();
    document.set_kind(first, definition("B", "/new")).unwrap();
    assert_eq!(document.definition("a"), None);
    // It now comes first of the two labelled `b`.
    assert_eq!(destination(&document, "b").as_deref(), Some("/new"));
    let footnote = document.footnote_definition("n").unwrap();
    document
        .set_kind(footnote, footnote_definition("M", "m"))
        .unwrap();
    assert_eq!(document.footnote_definition("n"), None);
    assert_eq!(document.footnote_definition("m"), Some(footnote));
    // A detached definition stays out of the tree when it changes.
    document.detach(first).unwrap();
    document.set_kind(first, definition("c", "/c")).unwrap();
    assert_eq!(document.definition("c"), None);
    assert_eq!(destination(&document, "b").as_deref(), Some("/b"));
    // Changing anything else changes no definition.
    let paragraph = document.descendants(footnote).nth(1).unwrap();
    document
        .set_kind(paragraph, EventKind::Start(Tag::Paragraph { tight: true }))
        .unwrap();
    assert_eq!(document.footnote_definition("m"), Some(footnote));
}

#[test]
fn a_footnote_definition_comes_before_those_inside_it_whichever_was_indexed_first() {
    let mut document = parse("[^c]: x\n\n    [^b]: y\n");
    let outer = document.footnote_definition("c").unwrap();
    let inner = document.footnote_definition("b").unwrap();
    // The inner definition takes the label first, so the outer is found second.
    document
        .set_kind(inner, footnote_definition("A", "a"))
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(inner));
    document
        .set_kind(outer, footnote_definition("A", "a"))
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(outer));
    document
        .set_kind(outer, footnote_definition("C", "c"))
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(inner));
}

/// Finding the first again, once the first is removed, counts a definition inside another of its
/// label as after it, as finding it directly does.
#[test]
fn the_first_found_again_is_the_outer_of_two_nested_definitions() {
    let mut document = parse("> [^a]: First.\n\n[^b]: x\n");
    let b = document.footnote_definition("b").unwrap();
    document
        .parse_children(b, "[^a]: Outer.\n", &ParseOptions::gfm())
        .unwrap();
    let outer = children(&document, b, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    })[0];
    document
        .parse_children(outer, "[^a]: Inner.\n", &ParseOptions::gfm())
        .unwrap();
    let quote = children(&document, document.root(), quote)[0];
    document
        .parse_children(quote, "y", &ParseOptions::gfm())
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(outer));
}

#[test]
fn a_definition_moved_before_another_is_found_first() {
    let mut document = parse("[^a]: x\n\n> [^b]: y\n");
    let root = document.root();
    let first = document.footnote_definition("a").unwrap();
    let quote = children(&document, root, quote)[0];
    let later = document
        .append_child(quote, footnote_definition("A", "a"))
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(first));
    // Moved out and back, the first is indexed after the later one, and still found first.
    document.detach(first).unwrap();
    assert_eq!(document.footnote_definition("a"), Some(later));
    document.attach(root, first, Some(quote)).unwrap();
    assert_eq!(document.footnote_definition("a"), Some(first));
    // Moved after it, the later one is found.
    document.attach(root, first, None).unwrap();
    assert_eq!(document.footnote_definition("a"), Some(later));
}

#[test]
fn replacing_a_heading_section_replaces_its_definitions() {
    let mut document = parse("# A\n\n[a]: /old\n\n# B\n\n[b]: /b\n");
    let replacement = parse("# A\n\n[a]: /new\n\n[b]: /replacement\n");
    let root = document.root();
    assert!(document
        .replace_heading_section(root, 0, &replacement, true, false)
        .unwrap());
    assert_eq!(destination(&document, "a").as_deref(), Some("/new"));
    assert_eq!(destination(&document, "b").as_deref(), Some("/replacement"));
    // A detached parent's section is not in the tree, before or after.
    let mut document = parse("> # A\n>\n> [a]: /old\n");
    let quote = children(&document, root, quote)[0];
    document.detach(quote).unwrap();
    assert!(document
        .replace_heading_section(quote, 0, &replacement, true, false)
        .unwrap());
    assert_eq!(document.definition("a"), None);
}

#[test]
fn parsing_children_adds_and_removes_definitions() {
    let options = ParseOptions::gfm();
    let mut document = parse("> [a]: /old\n\n[b]: /b\n");
    let root = document.root();
    let quote = children(&document, root, quote)[0];
    document
        .parse_children(quote, "[c]: /c\n\n[^n]: Note.\n", &options)
        .unwrap();
    assert_eq!(document.definition("a"), None);
    assert_eq!(destination(&document, "c").as_deref(), Some("/c"));
    assert_eq!(note(&document, "n").as_deref(), Some("Note."));
    // The fragment's definition comes before the one after its parent.
    document
        .parse_children(quote, "[b]: /new\n", &options)
        .unwrap();
    assert_eq!(destination(&document, "b").as_deref(), Some("/new"));
    assert_eq!(document.definition("c"), None);
    // A detached parent's definitions are not in the tree.
    document.detach(quote).unwrap();
    document
        .parse_children(quote, "[d]: /d\n", &options)
        .unwrap();
    assert_eq!(document.definition("d"), None);
    assert_eq!(destination(&document, "b").as_deref(), Some("/b"));
}

#[test]
fn normalizing_keeps_the_definitions_of_the_tree() {
    let mut document = parse("[a\n\tb]: /a\n\n> [^n]: Note.\n");
    document.normalize();
    assert_eq!(destination(&document, "a b").as_deref(), Some("/a"));
    assert_eq!(note(&document, "n").as_deref(), Some("Note."));
}

#[test]
fn a_cloned_document_keeps_its_definitions() {
    let document = parse("[a]: /a\n");
    let clone = document.clone();
    assert_eq!(destination(&clone, "a").as_deref(), Some("/a"));
}

#[cfg(feature = "serde")]
#[test]
fn a_loaded_document_finds_its_definitions() {
    let mut document = parse("[a]: /a\n\n> [a]: /quote\n\n[^n]: Note.\n");
    let quote = children(&document, document.root(), quote)[0];
    document.detach(quote).unwrap();
    let json = serde_json::to_string(&document).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(destination(&loaded, "a").as_deref(), Some("/a"));
    assert_eq!(note(&loaded, "n").as_deref(), Some("Note."));
    // A detached definition is not loaded into the tree.
    let mut loaded = loaded;
    loaded.extract_children(loaded.root()).unwrap();
    assert_eq!(loaded.definition("a"), None);
}

#[test]
fn removing_duplicates_keeps_the_rest_in_document_order() {
    let mut document = parse("[a]: /1\n\n[a]: /2\n\n[a]: /3\n\n[a]: /4\n");
    let root = document.root();
    let built: Vec<NodeId> = document.children(root).collect();
    let definitions: Vec<NodeId> = built
        .iter()
        .copied()
        .filter(|id| {
            matches!(
                document.node(*id).unwrap().kind,
                EventKind::Definition { .. }
            )
        })
        .collect();
    // Removing one after the first keeps the first; removing the first finds the next left.
    document.detach(definitions[1]).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/1"));
    document.detach(definitions[0]).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/3"));
    // Definitions added by edits count with those left, in document order.
    let added = document.append_child(root, definition("a", "/5")).unwrap();
    let before = document.append_child(root, definition("A", "/6")).unwrap();
    let first = document.children(root).next();
    document.attach(root, before, first).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/6"));
    document.detach(before).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/3"));
    document.detach(definitions[2]).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/4"));
    // Put back before the others, a removed one is first again.
    let first = document.children(root).next();
    document.attach(root, definitions[1], first).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/2"));
    document.detach(definitions[1]).unwrap();
    document.detach(definitions[3]).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/5"));
    document.detach(added).unwrap();
    assert_eq!(document.definition("a"), None);
}

#[test]
fn replacing_a_heading_section_in_an_item_refreshes_how_the_item_ends() {
    let mut document = parse("- # A\n  a\n- b\n");
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    let replacement = parse("# A\n\n```\nd\n```\n");
    assert!(document
        .replace_heading_section(item, 0, &replacement, true, false)
        .unwrap());
    assert!(matches!(
        document.node(item).unwrap().kind,
        EventKind::Start(Tag::Item {
            block_end: true,
            ..
        })
    ));
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = parse(&markdown);
    reparsed.normalize();
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
}

/// Look `label` up `count` times.
fn look_up(document: &Document, label: &str, count: usize) -> Duration {
    timed(|| {
        for _ in 0..count {
            assert!(document.definition(label).is_some());
        }
    })
}

/// Looking up a label shared by many definitions takes time logarithmic in their number, as for a
/// label of one, the control, whether they were parsed or added by edits: the first is kept, not
/// searched for (#121). Searching them grew 8 times the control's growth.
#[test]
fn looking_up_a_label_of_many_definitions_takes_logarithmic_time() {
    let duplicated = |count: usize| parse(&"[a]: /x\n".repeat(count));
    let distinct = |count: usize| {
        parse(
            &(0..count)
                .map(|index| format!("[a{index}]: /x\n"))
                .collect::<String>(),
        )
    };
    assert_against_control(
        "parsed",
        1 << 11,
        |count| look_up(&duplicated(count), "a", count),
        |count| look_up(&distinct(count), "a0", count),
    );
    // Added by edits, each after the last.
    let added = |count: usize, label: fn(usize) -> String| {
        let mut document = parse("");
        let root = document.root();
        for index in 0..count {
            document
                .append_child(
                    root,
                    EventKind::Definition {
                        label: Text::from(label(index)),
                        destination: Text::Literal("/x"),
                        title: None,
                    },
                )
                .unwrap();
        }
        document
    };
    assert_against_control(
        "added",
        1 << 11,
        |count| look_up(&added(count, |_| String::from("a")), "a", count),
        |count| look_up(&added(count, |index| format!("a{index}")), "a0", count),
    );
}

/// A lookup in nested block quotes, each holding a definition of the same label, takes time linear
/// in their depth and logarithmic in their number, as with a label for each, the control (#121).
#[test]
fn looking_up_a_label_defined_at_every_depth_takes_time_linear_in_the_depth() {
    let nested = |count: usize, label: fn(usize) -> String| {
        let mut source = String::new();
        for depth in 1..=count {
            let prefix = ">".repeat(depth);
            source.push_str(&format!("{prefix} [{}]: /x\n{prefix}\n", label(depth)));
        }
        parse(&source)
    };
    let time = |document: Document, count: usize| {
        let mut document = document;
        let paragraphs: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|id| {
                matches!(
                    document.node(*id).unwrap().kind,
                    EventKind::Start(Tag::BlockQuote)
                )
            })
            .collect();
        timed(|| {
            for &quote in &paragraphs {
                let paragraph = document
                    .append_child(quote, EventKind::Start(Tag::Paragraph { tight: false }))
                    .unwrap();
                document
                    .parse_children(paragraph, "[a]", &ParseOptions::gfm())
                    .unwrap();
            }
            for _ in 0..count {
                assert!(document.definition("a").is_some());
            }
        })
    };
    assert_against_control(
        "nested",
        1 << 8,
        |count| time(nested(count, |_| String::from("a")), count),
        |count| {
            time(
                nested(count, |depth| {
                    if depth == 1 {
                        String::from("a")
                    } else {
                        format!("a{depth}")
                    }
                }),
                count,
            )
        },
    );
}

#[test]
fn a_definition_moved_into_one_added_after_it_is_found_after_it() {
    let mut document = parse("[^a]: Inner.\n");
    let root = document.root();
    let inner = document.footnote_definition("a").unwrap();
    let outer = document
        .append_child(root, footnote_definition("A", "a"))
        .unwrap();
    assert_eq!(document.footnote_definition("a"), Some(inner));
    // The inner definition, built before the outer, is now inside it, so the outer comes first.
    document.attach(outer, inner, None).unwrap();
    assert_eq!(document.footnote_definition("a"), Some(outer));
}

#[test]
fn parsing_into_a_container_of_the_first_definition_finds_the_next_outside_it() {
    let options = ParseOptions::gfm();
    let mut document = parse("> [a]: /in\n\n[a]: /after\n");
    let root = document.root();
    document
        .append_child(root, definition("a", "/added"))
        .unwrap();
    let quote = children(&document, root, quote)[0];
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document.parse_children(quote, "[a]", &options).unwrap();
    assert_eq!(destination(&document, "a").as_deref(), Some("/after"));
    // The paragraph's reference resolves to it too.
    document.parse_children(paragraph, "[a]", &options).unwrap();
    let link = document.children(paragraph).next().unwrap();
    assert!(matches!(
        &document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link { destination, .. })
            if destination.resolve(document.source()).ok() == Some("/after")
    ));
}

#[cfg(feature = "serde")]
#[test]
fn a_loaded_document_whose_definitions_were_moved_finds_the_first() {
    let mut document = parse("[a]: /first\n\n[^n]: First.\n");
    let root = document.root();
    let added = document
        .append_child(root, definition("A", "/added"))
        .unwrap();
    let first = document.children(root).next();
    document.attach(root, added, first).unwrap();
    let json = serde_json::to_string(&document).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(destination(&loaded, "a").as_deref(), Some("/added"));
    assert_eq!(note(&loaded, "n").as_deref(), Some("First."));
    let mut loaded = loaded;
    loaded.detach(added).unwrap();
    assert_eq!(destination(&loaded, "a").as_deref(), Some("/first"));
}
