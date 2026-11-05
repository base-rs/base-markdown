// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// `Document::definition` and `Document::footnote_definition` find the first attached definition
// of a label as the document is edited (#121).

use base_markdown::{
    stream::{EventKind, Tag, Text},
    Document, NodeId, ParseOptions,
};

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
