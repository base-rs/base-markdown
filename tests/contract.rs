// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The clauses of `context/normalize-serialize.md`, the `base_markdown::contract` module, one or more tests each, named `clause_<section>_<clause>_…`. The known gaps of section 8 have ignored repros, named after their issue, with the issue in the reason.

mod support;

use base_markdown::{
    markdown::to_markdown_node,
    normalize_markdown, render_events,
    stream::{Event, EventKind, Reference, StreamError, Tag, TagKind, Target, Text},
    to_html_with_options, to_markdown, to_markdown_with_options, CompileOptions, Document,
    FootnotePolicy, InsertOptions, LineWrapping, MarkdownOptions, NodeId, NodeSyntax, Options,
    ParseOptions, ReferenceKind,
};
use regex::Regex;
use support::corpus::{self, Random};
use support::fixture::canonical;
use support::linear::{assert_linear, timed};
use support::round_trip::{self, round_trip, Failure, Outcome};

/// How many inputs each corpus test takes from each corpus; the fuzz in `roundtrip_fuzz.rs` takes many more.
const SAMPLES: usize = 3000;

type Generator = fn(&mut Random) -> String;

const CORPORA: [(&str, u64, Generator); 5] = [
    ("realistic", 1, corpus::realistic),
    ("soup", 2, corpus::soup),
    ("edge", 3, corpus::edge),
    ("strike", 4, corpus::strike),
    ("blocky", 5, corpus::blocky),
];

/// Each input of each corpus.
fn each_input(mut check: impl FnMut(&str, &str)) {
    for (name, seed, generate) in CORPORA {
        let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for _ in 0..SAMPLES {
            check(name, &generate(&mut random));
        }
    }
}

fn parse(source: &str) -> Document<&str> {
    Document::parse(source, &ParseOptions::default()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output
}

fn normalized(source: &str) -> Document<&str> {
    let mut document = parse(source);
    document.normalize();
    document
}

/// Everything about a document, IDs, values, the index and whether it is settled, for comparing before and after.
fn snapshot<S: AsRef<str> + core::fmt::Debug>(document: &Document<S>) -> String {
    format!("{:?}", document)
}

fn settled<S: AsRef<str> + core::fmt::Debug>(document: &Document<S>) -> bool {
    snapshot(document).contains("settled: true")
}

fn first<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> NodeId {
    document.children(id).next().unwrap()
}

/// The first node in document order, attached, that `matches`.
fn find<S: AsRef<str>>(document: &Document<S>, matches: impl Fn(&EventKind) -> bool) -> NodeId {
    document
        .descendants(document.root())
        .find(|&id| matches(&document.node(id).unwrap().kind))
        .unwrap()
}

fn all<S: AsRef<str>>(document: &Document<S>, matches: impl Fn(&EventKind) -> bool) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|&id| matches(&document.node(id).unwrap().kind))
        .collect()
}

fn is_definition(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
    )
}

fn is_paragraph(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Start(Tag::Paragraph { .. }))
}

fn is_item(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Start(Tag::Item { .. }))
}

fn text(value: &'static str) -> EventKind<'static> {
    EventKind::Text(Text::Literal(value))
}

fn definition(label: &'static str, destination: &'static str) -> EventKind<'static> {
    EventKind::Definition {
        label: Text::Literal(label),
        destination: Text::Literal(destination),
        title: None,
    }
}

fn paragraph() -> EventKind<'static> {
    EventKind::Start(Tag::Paragraph { tight: false })
}

/// The tags of attention, which merging, unwrapping and moving whitespace out of attention move (3.4).
fn without_attention(html: &str) -> String {
    ["<em>", "</em>", "<strong>", "</strong>", "<del>", "</del>"]
        .iter()
        .fold(html.to_owned(), |html, tag| html.replace(tag, ""))
}

/// Assert that `edit` fails with `error` and changes nothing (6.1).
fn assert_rejected<S: AsRef<str> + core::fmt::Debug, T: core::fmt::Debug>(
    document: &mut Document<S>,
    error: StreamError,
    edit: impl FnOnce(&mut Document<S>) -> Result<T, StreamError>,
) {
    let before = snapshot(document);
    let result = edit(document);
    assert!(
        matches!(&result, Err(found) if core::mem::discriminant(found) == core::mem::discriminant(&error)),
        "expected {:?}, got {:?}",
        error,
        result
    );
    assert_eq!(
        snapshot(document),
        before,
        "a failed edit changed the document"
    );
}

// 1. The model.

#[test]
fn clause_1_1_detached_trees_render_write_and_attach_again() {
    let mut document = parse("a\n\n*b*\n\nc\n");
    let second = all(&document, is_paragraph)[1];
    let before = to_markdown(&document).unwrap();
    document.detach(second).unwrap();
    assert!(document.node(second).unwrap().parent().is_none());
    let mut fragment = String::new();
    document
        .render_node(second, &CompileOptions::default(), &mut fragment)
        .unwrap();
    assert_eq!(fragment, "<p><em>b</em></p>");
    assert_eq!(
        to_markdown_node(&document, second, &MarkdownOptions::default()).unwrap(),
        "*b*\n"
    );
    let third = all(&document, is_paragraph)[1];
    document
        .attach(document.root(), second, Some(third))
        .unwrap();
    assert_eq!(all(&document, is_paragraph)[1], second, "the ID stays");
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), before);
}

#[test]
fn clause_1_1_detached_trees_bind_their_own_definitions() {
    let mut document = parse("> [a]\n>\n> [a]: /inner\n\n[a]: /outer\n");
    let quote = first(&document, document.root());
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(document.destination(link).unwrap().0, "/inner");
    document.detach(quote).unwrap();
    assert_eq!(document.destination(link).unwrap().0, "/inner");
    let outer = parse("[a]\n\n[a]: /outer\n");
    let link = find(&outer, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(outer.destination(link).unwrap().0, "/outer");
}

#[test]
fn clause_1_2_content_model_rejects_invalid_trees() {
    let mut document = parse("a\n\n| x |\n| - |\n| y |\n\n[l](u)\n");
    let root = document.root();
    let paragraph_id = first(&document, root);
    let cell = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::TableCell { header: false, .. }))
    });
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    // A block in phrasing, phrasing in a root of blocks.
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(paragraph_id, EventKind::Start(Tag::BlockQuote))
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(root, text("x"))
    });
    // An item outside a list.
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(
            root,
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: None,
            }),
        )
    });
    // Reach: no break in a table cell, no footnote call in a link.
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(cell, EventKind::HardBreak)
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(
            link,
            EventKind::FootnoteReference {
                identifier: Text::Literal("N"),
                label: None,
            },
        )
    });
    // Values: a heading level and a label the parser cannot read.
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, EventKind::Start(Tag::Heading(7)))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, definition("", "/u"))
    });
}

#[test]
fn clause_1_3_valid_shapes_need_not_be_normalized() {
    let mut document = parse("a\n");
    let root = document.root();
    let paragraph_id = first(&document, root);
    // Adjacent text, empty text, an empty paragraph and a link in a link are valid.
    document.append_child(paragraph_id, text("b")).unwrap();
    document.append_child(paragraph_id, text("")).unwrap();
    let outer = document
        .append_child(
            paragraph_id,
            EventKind::Start(Tag::Link(Target::Resource {
                destination: Text::Literal("/o"),
                title: None,
            })),
        )
        .unwrap();
    let inner = document
        .append_child(
            outer,
            EventKind::Start(Tag::Link(Target::Resource {
                destination: Text::Literal("/i"),
                title: None,
            })),
        )
        .unwrap();
    document.append_child(inner, text("in")).unwrap();
    document.append_child(root, paragraph()).unwrap();
    assert!(!document.is_normalized());
    assert_eq!(
        html(&document),
        "<p>ab<a href=\"/o\"><a href=\"/i\">in</a></a></p>\n<p></p>"
    );
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(html(&document), "<p>ab<a href=\"/o\">in</a></p>");
}

#[test]
fn clause_1_4_values_are_source_ranges_with_lf_line_endings() {
    let source = "```\r\na\r\n```\r\n\r\nb\r\nc\r\n";
    let mut document = parse(source);
    let text_id = find(&document, |kind| matches!(kind, EventKind::Text(_)));
    document.text_mut(text_id).unwrap().push('!');
    assert_eq!(document.source(), source, "the source stays immutable");
    let output = to_markdown(&document).unwrap();
    assert!(!output.contains('\r'));
    assert_eq!(output, "```\na\n```\n\nb!\nc\n");
}

// 2. Names and bindings.

#[test]
fn clause_2_1_references_store_names_and_follow_edits() {
    let mut document = parse("[a]\n\n[a]: /1\n");
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let EventKind::Start(Tag::Link(Target::Reference(reference))) =
        &document.node(link).unwrap().kind
    else {
        panic!("a reference stores its kind and label");
    };
    assert_eq!(reference.kind, ReferenceKind::Shortcut);
    let definition_id = find(&document, is_definition);
    document
        .set_kind(definition_id, definition("a", "/2"))
        .unwrap();
    assert_eq!(document.destination(link).unwrap().0, "/2");
    assert_eq!(html(&document), "<p><a href=\"/2\">a</a></p>\n");
}

#[test]
fn clause_2_2_the_first_definition_binds() {
    let document = parse("[A]\n\n[a]: /1\n[A]: /2\n");
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(document.destination(link).unwrap().0, "/1");
}

#[test]
fn clause_2_2_the_first_footnote_definition_to_end_binds() {
    let source = "x[^a]\n\n[^a]: outer\n\n    [^a]: inner\n";
    let rendered = html(&parse(source));
    assert!(rendered.contains("inner"), "{}", rendered);
    assert!(!rendered.contains("outer"), "{}", rendered);
    let mut document = parse(source);
    document.normalize();
    assert_eq!(html(&document), rendered);
    assert!(!to_markdown(&document).unwrap().contains("outer"));
}

#[test]
fn clause_2_3_copies_are_accepted_and_render_nothing() {
    let with_copy = parse("[a]\n\n[a]: /1\n\n[a]: /2\n\n[^n]\n\n[^n]: one\n\n[^n]: two\n");
    assert_eq!(all(&with_copy, is_definition).len(), 4);
    let without = parse("[a]\n\n[a]: /1\n\n[^n]\n\n[^n]: one\n");
    assert_eq!(html(&with_copy), html(&without));
    let mut builder = base_markdown::AstBuilder::new();
    builder
        .consume(&mut with_copy.events(), usize::MAX)
        .unwrap();
    let built = builder.finish(with_copy.source()).unwrap();
    assert_eq!(all(&built, is_definition).len(), 4);
    assert_eq!(html(&built), html(&without));
}

fn note(identifier: &'static str, label: &'static str) -> EventKind<'static> {
    EventKind::Start(Tag::FootnoteDefinition {
        identifier: Text::Literal(identifier),
        label: Some(Text::Literal(label)),
    })
}

/// The definitions and footnote definitions that normalizing would remove as copies.
fn copies<S: AsRef<str> + Clone>(document: &Document<S>) -> usize {
    let mut normalized = document.clone();
    normalized.normalize();
    all(document, is_definition).len() - all(&normalized, is_definition).len()
}

#[test]
fn clause_2_4_no_edit_adds_a_copy_to_the_attached_document() {
    let mut document =
        parse("[a]: /1\n\n[b]: /3\n\n[^n]: one\n\n[^m]: two\n\nx [^n] [^m]\n\n> q\n");
    let root = document.root();
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let definitions = all(&document, is_definition);
    // append_child, of a definition and of a footnote definition.
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, definition("A", "/2"))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(quote, note("N", "n"))
    });
    // set_kind, giving a definition a label that another has.
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.set_kind(definitions[1], definition("A", "/3"))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.set_kind(definitions[3], note("N", "n"))
    });
    // parse_children, of a label defined outside the replaced children, or defined twice.
    for fragment in [
        "[a]: /2\n",
        "[^n]: two\n",
        "[c]: /1\n\n[c]: /2\n",
        "[^c]: 1\n\n[^c]: 2\n",
    ] {
        assert_rejected(&mut document, StreamError::InvalidValue, |document| {
            document.parse_children(quote, fragment, &ParseOptions::default())
        });
    }
    // attach, of a detached tree that defines what the document defines, or holds a copy.
    for fragment in ["[a]: /2\n", "[^n]: two\n", "[z]: /1\n\n[z]: /2\n"] {
        let tree = document
            .append_child(root, EventKind::Start(Tag::BlockQuote))
            .unwrap();
        document.detach(tree).unwrap();
        // A detached tree has no index, so edits in it are not checked (#226).
        document
            .parse_children(tree, fragment, &ParseOptions::default())
            .unwrap();
        assert_rejected(&mut document, StreamError::InvalidValue, |document| {
            document.attach(root, tree, None)
        });
    }
    // insert_from and replace_heading_section copy no link reference definition, and relabel a
    // footnote definition or, under Reject, reject it.
    let source = parse("s [r][a] [^n]\n\n[a]: /9\n\n[^n]: other\n");
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.insert_from(
            root,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions {
                footnotes: FootnotePolicy::Reject,
                ..InsertOptions::default()
            },
        )
    });
    document
        .insert_from(
            root,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    assert_eq!(copies(&document), 0);
    let mut sectioned = parse("[a]: /1\n\nx [^n]\n\n[^n]: one\n\n# h\n\ny\n");
    let root = sectioned.root();
    let heading = sectioned
        .children(root)
        .position(|id| {
            matches!(
                sectioned.node(id).unwrap().kind,
                EventKind::Start(Tag::Heading(_))
            )
        })
        .unwrap();
    let replacement = parse("# r\n\n[a]: /2\n\n[^n]\n\n[^n]: three\n");
    assert!(sectioned
        .replace_heading_section(root, heading, &replacement, true, true)
        .unwrap());
    assert_eq!(copies(&sectioned), 0);
}

#[test]
fn clause_2_4_a_detached_copy_cannot_be_attached() {
    let mut document = parse("[a]: /1\n\n[a]: /2\n");
    let root = document.root();
    let copy = all(&document, is_definition)[1];
    document.detach(copy).unwrap();
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.attach(root, copy, None)
    });
    // The only definition of a label can be detached and attached again.
    let mut document = parse("[a]: /1\n\nx\n");
    let root = document.root();
    let only = find(&document, is_definition);
    document.detach(only).unwrap();
    document.attach(root, only, None).unwrap();
}

#[test]
fn clause_2_4_a_definition_whose_label_has_a_copy_cannot_move() {
    let mut document = parse("[a]: /1\n\n> q\n\n[a]: /2\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let binding = find(&document, is_definition);
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.attach(quote, binding, None)
    });
    document.normalize();
    let binding = find(&document, is_definition);
    document.attach(quote, binding, None).unwrap();
}

#[test]
fn clause_2_5_unbound_references_render_as_text_until_normalized() {
    let mut document = parse("x [a] y\n\n[a]: /u\n");
    let definition_id = find(&document, is_definition);
    document.detach(definition_id).unwrap();
    assert_eq!(html(&document), "<p>x [a] y</p>\n");
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(html(&document), "<p>x [a] y</p>\n");
    let paragraph_id = first(&document, document.root());
    assert_eq!(document.children(paragraph_id).count(), 1, "one text");
    assert_eq!(to_markdown(&document).unwrap(), "x \\[a] y\n");
}

// 3. Normalize.

#[test]
fn clause_3_1_normalize_covers_detached_trees() {
    let mut document = parse("a\n\nb\n");
    let second = all(&document, is_paragraph)[1];
    document.detach(second).unwrap();
    document.append_child(second, text("c")).unwrap();
    document
        .append_child(second, EventKind::Start(Tag::Emphasis))
        .unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(document.children(second).count(), 1);
    assert_eq!(
        to_markdown_node(&document, second, &MarkdownOptions::default()).unwrap(),
        "bc\n"
    );
}

#[test]
fn clause_3_1_unbound_references_become_text_before_joining() {
    let mut document = parse("[a]: /u\n\nx [a][b] *y*\n");
    let definition_id = find(&document, is_definition);
    document.detach(definition_id).unwrap();
    document.normalize();
    let paragraph_id = first(&document, document.root());
    let kinds: Vec<_> = document
        .children(paragraph_id)
        .map(|id| core::mem::discriminant(&document.node(id).unwrap().kind))
        .collect();
    assert_eq!(kinds.len(), 2, "one text, then emphasis");
    assert_eq!(html(&document), "<p>x [a][b] <em>y</em></p>\n");
}

#[test]
fn clause_3_1_a_reference_that_roots_a_detached_tree_stays_a_reference() {
    let mut document = parse("x\n");
    let paragraph_id = first(&document, document.root());
    let link = document
        .append_child(
            paragraph_id,
            EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                kind: ReferenceKind::Shortcut,
                label: Text::Literal("a"),
            })))),
        )
        .unwrap();
    document.append_child(link, text("a")).unwrap();
    let call = document
        .append_child(
            paragraph_id,
            EventKind::FootnoteReference {
                identifier: Text::Literal("N"),
                label: Some(Text::Literal("n")),
            },
        )
        .unwrap();
    document.detach(link).unwrap();
    document.detach(call).unwrap();
    document.normalize();
    assert!(document.is_normalized());
    assert!(matches!(
        document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(_)))
    ));
    assert!(matches!(
        document.node(call).unwrap().kind,
        EventKind::FootnoteReference { .. }
    ));
}

#[test]
fn clause_3_1_normalize_is_linear() {
    assert_linear("normalize", 400, |size| {
        let source = "a\\*b *c* _d_ [e][f] [^g]\n\n- h\n\n  [f]: /i\n".repeat(size);
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        timed(|| document.normalize())
    });
}

#[test]
fn clause_3_2_normalize_is_idempotent() {
    each_input(|name, source| {
        let mut document = normalized(source);
        assert!(document.is_normalized(), "{} {:?}", name, source);
        let once = snapshot(&document);
        document.normalize();
        assert_eq!(snapshot(&document), once, "{} {:?}", name, source);
    });
}

/// Make one edit of a random kind at random nodes, which may fail.
fn random_edit(document: &mut Document<&str>, random: &mut Random) {
    let ids: Vec<NodeId> = document.descendants(document.root()).collect();
    let pick = |random: &mut Random| ids[random.below(ids.len())];
    let (id, other) = (pick(random), pick(random));
    let leaves = [
        text("*a* "),
        text(""),
        EventKind::Start(Tag::Emphasis),
        paragraph(),
        EventKind::SoftBreak(Text::Literal("\n")),
        EventKind::HardBreak,
        definition("X", "/x"),
    ];
    let fragments = [
        "a *b*",
        "- c\n- d\n",
        "[x]: /y\n",
        "> e\n\n[^f]: g\n",
        "h [x] [^f]",
    ];
    let _ = match random.below(6) {
        0 => document.detach(id),
        1 => document
            .detach(id)
            .and_then(|()| document.attach(other, id, None).map(|_| ())),
        2 => document
            .append_child(id, leaves[random.below(leaves.len())].clone())
            .map(|_| ()),
        3 => document.set_kind(id, text("b")),
        4 => document.extract_children(id).map(|_| ()),
        _ => document
            .parse_children(
                id,
                fragments[random.below(fragments.len())],
                &ParseOptions::default(),
            )
            .map(|_| ()),
    };
}

#[test]
fn clause_3_2_normalize_is_idempotent_after_edits() {
    let mut random = Random(0x5EED_5EED);
    each_input(|name, source| {
        let mut document = parse(source);
        for _ in 0..4 {
            random_edit(&mut document, &mut random);
        }
        document.normalize();
        assert!(document.is_normalized(), "{} {:?}", name, source);
        let once = snapshot(&document);
        document.normalize();
        assert_eq!(snapshot(&document), once, "{} {:?}", name, source);
    });
}

#[test]
fn clause_3_2_is_normalized_is_whether_normalize_changes_anything() {
    for source in ["a\\*b\n", "*a*_b_\n", "**a**__b__\n", "[a]: /1\n[a]: /2\n"] {
        let mut document = parse(source);
        assert!(!document.is_normalized(), "{:?}", source);
        let before = snapshot(&document);
        document.normalize();
        assert_ne!(snapshot(&document), before, "{:?}", source);
    }
    let mut document = parse("a *b*\n");
    assert!(document.is_normalized());
    let before = snapshot(&document);
    document.normalize();
    assert_eq!(snapshot(&document), before);
}

const ROUND_TRIP_SAMPLES: [&str; 12] = [
    "# Title\n\nSome *emphasis*, **strong**, ~~deleted~~ and `code`.\n",
    "- a\n- b\n  - c\n\n1. d\n2. e\n",
    "- [ ] task\n- [x] done\n",
    "> quote\n> > nested\n",
    "| a | b |\n| :- | -: |\n| c | d |\n",
    "[full][r], [collapsed][], [r], ![image][r]\n\n[r]: /u \"t\"\n",
    "note[^n]\n\n[^n]: body\n",
    "```rust\nfn main() {}\n```\n\n    indented\n",
    "<div>\nhtml\n</div>\n\ninline <b>html</b>\n",
    "line  \nbreak\\\nescape \\* &amp; &#x20;\n",
    "---\ntitle: x\n---\n\ntext\n",
    "Setext\n===\n\n***\n",
];

#[test]
fn clause_3_3_normalize_round_trips() {
    let underscore = MarkdownOptions {
        emphasis: '_',
        strong: '_',
        ..MarkdownOptions::default()
    };
    for source in ROUND_TRIP_SAMPLES
        .iter()
        .chain(&["x[^a]\n\n[^a]: z\n\n- p\n  [^a]: y\n  2. q\n"])
    {
        for options in [&MarkdownOptions::default(), &underscore] {
            assert_round_trips_with(source, options);
        }
    }
}

/// `document` with each soft break but the line ending of a hard break, and each run of line endings, spaces and tabs in a text value that holds a line ending, one space, as `LineWrapping::Unwrap` writes them (3.3).
fn unwrapped<'s>(document: &Document<&'s str>) -> Document<&'s str> {
    let line_endings = Regex::new(r"[ \t]*\n[ \t\n]*").unwrap();
    let mut unwrapped = document.clone();
    let mut after_hard_break = false;
    for id in document.descendants(document.root()) {
        let kind = &document.node(id).unwrap().kind;
        let after = core::mem::replace(&mut after_hard_break, matches!(kind, EventKind::HardBreak));
        match kind {
            // The line ending of a hard break is part of it.
            EventKind::SoftBreak(_) if after => {}
            EventKind::SoftBreak(_) => {
                let parent = document.node(id).unwrap().parent().unwrap();
                let space = unwrapped.append_child(parent, text(" ")).unwrap();
                unwrapped.attach(parent, space, Some(id)).unwrap();
                unwrapped.detach(id).unwrap();
            }
            EventKind::Text(value) => {
                let value = value.resolve(document.source()).unwrap();
                if value.contains('\n') {
                    *unwrapped.text_mut(id).unwrap() =
                        line_endings.replace_all(value, " ").into_owned();
                }
            }
            _ => {}
        }
    }
    unwrapped.normalize();
    unwrapped
}

/// The canonical tree of `document` without whether each reference is full, as a shortcut or collapsed reference whose text no longer matches its label once unwrapped is written full.
fn without_reference_forms<S: AsRef<str>>(document: &Document<S>) -> Vec<String> {
    let form = Regex::new(r"Some\(\((true|false), ").unwrap();
    canonical(document)
        .iter()
        .map(|entry| form.replace(entry, "Some((").into_owned())
        .collect()
}

#[test]
fn clause_3_3_unwrap_writes_soft_breaks_and_line_endings_in_text_as_spaces() {
    let options = MarkdownOptions {
        line_wrapping: LineWrapping::Unwrap,
        ..MarkdownOptions::default()
    };
    assert_eq!(normalize_markdown("a\nb\n").unwrap(), "a b\n");
    let soft = [
        "a\nb\n",
        "a  \nb\nc\n",
        "a&#xA;b \t&#xA;\tc\n",
        "Setext\nheading\n===\n",
        "- a\n  b\n- c\n",
        "> a\n> b\n",
        "*a\nb* **c\nd**\n",
        "[a\nb](/u) [c\nd]\n\n[c d]: /v\n",
        "| a |\n| - |\n| b&#xA;c |\n",
    ];
    let mut sources: Vec<&str> = soft.iter().chain(&ROUND_TRIP_SAMPLES).copied().collect();
    let mut generated = Vec::new();
    for (_, seed, generate) in &CORPORA[..4] {
        let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        generated.extend((0..SAMPLES / 3).map(|_| generate(&mut random)));
    }
    sources.extend(generated.iter().map(String::as_str));
    for source in sources {
        let document = normalized(source);
        let expected = unwrapped(&document);
        let output = to_markdown_with_options(&document, &options).unwrap();
        let read = normalized(&output);
        assert_eq!(
            round_trip::html(&read, false),
            round_trip::html(&expected, false),
            "{:?}: {:?}",
            source,
            output
        );
        assert_eq!(
            without_reference_forms(&read),
            without_reference_forms(&expected),
            "{:?}: {:?}",
            source,
            output
        );
        let again = to_markdown_with_options(&read, &options).unwrap();
        assert_eq!(again, output, "{:?}", source);
    }
}

/// A space or tab after a line ending is written as a character reference, so what follows it does not start a line, whether the two are one text or, as an unwrapped soft break leaves them, several, which read back as one (#237).
#[test]
fn clause_3_3_unwrapped_line_starts_are_written_stably() {
    let options = MarkdownOptions {
        line_wrapping: LineWrapping::Unwrap,
        ..MarkdownOptions::default()
    };
    for marker in [
        "\\~", "\\#", "\\>", "\\- a", "\\+ a", "\\=", "1\\. a", "1\\) a", "\\`",
    ] {
        let output =
            to_markdown_with_options(&normalized(&format!("&#x20;\n{marker}\n")), &options)
                .unwrap();
        assert_eq!(
            to_markdown_with_options(&normalized(&output), &options).unwrap(),
            output,
            "{:?}",
            marker
        );
    }
}

#[test]
fn clause_3_3_an_inline_document_is_written_as_a_paragraphs_content() {
    let mut document = parse("x\n");
    let root = document.root();
    document.extract_children(root).unwrap();
    document.append_child(root, text("a ")).unwrap();
    let emphasis = document
        .append_child(root, EventKind::Start(Tag::Emphasis))
        .unwrap();
    document.append_child(emphasis, text("b")).unwrap();
    document.normalize();
    let output = to_markdown(&document).unwrap();
    assert_eq!(output, "a *b*\n");
    let read = normalized(&output);
    let paragraph_id = first(&read, read.root());
    assert!(is_paragraph(&read.node(paragraph_id).unwrap().kind));
    assert_eq!(
        read.children(read.root())
            .filter(|&id| !matches!(read.node(id).unwrap().kind, EventKind::Separator { .. }))
            .count(),
        1
    );
    assert_eq!(
        html(&read),
        format!("<p>{}</p>\n", html(&document).trim_end())
    );
    assert_eq!(to_markdown(&read).unwrap(), output);
}

/// The deepest nesting of emphasis and strong in `html`.
fn nesting(html: &str) -> usize {
    let (mut depth, mut deepest) = (0usize, 0);
    for (at, _) in html.match_indices('<') {
        let rest = &html[at..];
        if rest.starts_with("<em>") || rest.starts_with("<strong>") {
            depth += 1;
            deepest = deepest.max(depth);
        } else if rest.starts_with("</em>") || rest.starts_with("</strong>") {
            depth = depth.saturating_sub(1);
        }
    }
    deepest
}

fn html_blocks<S: AsRef<str>>(document: &Document<S>) -> usize {
    all(document, |kind| {
        matches!(kind, EventKind::Html { block: true, .. })
    })
    .len()
}

/// What differs between `before` and `after`: each without the longest common prefix and suffix.
fn differing<'a>(before: &'a str, after: &'a str) -> (&'a str, &'a str) {
    let prefix = before
        .char_indices()
        .zip(after.chars())
        .find(|((_, one), other)| one != other)
        .map_or(before.len().min(after.len()), |((at, _), _)| at);
    let (before, after) = (&before[prefix..], &after[prefix..]);
    let suffix: usize = before
        .chars()
        .rev()
        .zip(after.chars().rev())
        .take_while(|(one, other)| one == other)
        .map(|(one, _)| one.len_utf8())
        .sum();
    (
        &before[..before.len() - suffix],
        &after[..after.len() - suffix],
    )
}

/// The issue of section 8 that explains why the normalized `document` does not round trip, or `None`. Each rule needs the document to have the issue's shape, the failure to be of a kind the issue makes, and what differs between the two renderings or trees to involve that shape; a failure to write or to normalize is never a known gap.
fn known_gap(
    document: &Document<&str>,
    failure: &Failure,
    rules: &GapRules,
) -> Option<&'static str> {
    let (output, before, after) = (&failure.output, &failure.before, &failure.after);
    let (html, tree) = (failure.kind == "html", failure.kind == "tree");
    if !html && !tree {
        return None;
    }
    let (one, other) = differing(before, after);
    let involves = |needles: &[&str]| {
        needles
            .iter()
            .any(|needle| one.contains(needle) || other.contains(needle))
    };
    let kinds: Vec<&EventKind> = document
        .descendants(document.root())
        .map(|id| &document.node(id).unwrap().kind)
        .collect();
    let source = document.source();
    let caret_label = kinds.iter().any(|kind| {
        matches!(kind, EventKind::Definition { label, .. } if label.resolve(source).unwrap().starts_with('^'))
    });
    if caret_label && involves(&["^", "footnote", "Footnote"]) {
        return Some("#208 a definition label starting with ^");
    }
    // Only the break, or the space before it, differs.
    let only_breaks = [one, other].iter().all(|side| {
        ["<br />", "\\n", "&#xA;", "&#x20;"]
            .iter()
            .fold((*side).to_owned(), |side, needle| side.replace(needle, ""))
            .trim()
            .is_empty()
    });
    if only_breaks
        && ((html && before.contains("<input") && rules.task_break.is_match(output))
            || (tree && rules.task_line_ending.is_match(output)))
    {
        return Some("#199 a task item starting with a break");
    }
    let leading_definition = all(document, is_item).into_iter().any(|item| {
        document
            .children(item)
            .find(|&id| !matches!(document.node(id).unwrap().kind, EventKind::Separator { .. }))
            .is_some_and(|id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. }
                )
            })
    });
    if leading_definition && involves(&["[", "<input", "<li", "Item", "Definition", "<a href"]) {
        return Some("#200 a leading definition in an item");
    }
    if html && rules.star_rule.is_match(output) && involves(&["<hr", "<li", "<ul"]) {
        return Some("#227 a * marker before a rule");
    }
    if html
        && (nesting(before) > 1 || nesting(after) > 1)
        && involves(&["<em>", "</em>", "<strong>", "</strong>", "*", "_"])
    {
        return Some("#110 nested attention");
    }
    let indented_html = kinds.iter().any(|kind| {
        matches!(kind, EventKind::Html { value, block: true } if value.resolve(source).unwrap().starts_with([' ', '\t']))
    });
    let whitespace = one.trim().is_empty() && other.trim().is_empty();
    if html && indented_html && (whitespace || involves(&["&lt;"])) {
        return Some("#229 an HTML block with an indented first line");
    }
    if html && before.replace("\n&lt;", " &lt;") == after.replace("\n&lt;", " &lt;") {
        return Some("#193 inline HTML after a soft break is written after a space");
    }
    if involves(&["&lt;", "Html"]) && html_blocks(&normalized(output)) > html_blocks(document) {
        return Some("#193 inline HTML at the start of a line is written as an HTML block");
    }
    None
}

struct GapRules {
    task_break: Regex,
    task_line_ending: Regex,
    star_rule: Regex,
}

fn gap_rules() -> GapRules {
    GapRules {
        task_break: Regex::new(r"(?m)\] (&#xA;|&#x20;)?\s*$|\] &#xA;|\]  +\n|\] \\\n").unwrap(),
        task_line_ending: Regex::new(r"\[[ x]\] &#xA;").unwrap(),
        star_rule: Regex::new(r"(?m)^(> |\s)*([-*+] |\d+[.)] )*\* \*\*\*").unwrap(),
    }
}

#[test]
fn clause_3_3_blocky_round_trip_failures_are_known_gaps() {
    let rules = gap_rules();
    let mut random = Random(5u64.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut failures = 0;
    for _ in 0..SAMPLES {
        let source = corpus::blocky(&mut random);
        let document = normalized(&source);
        if let Outcome::Fail(failure) =
            round_trip(document.clone(), &MarkdownOptions::default(), false)
        {
            failures += 1;
            assert!(
                known_gap(&document, &failure, &rules).is_some(),
                "not a known gap: {} {:?}: {:?}\n{}\n{}",
                failure.kind,
                source,
                failure.output,
                failure.before,
                failure.after
            );
        }
        // The rules attribute no other failure of the same document.
        let output = to_markdown(&document).unwrap();
        let before = round_trip::html(&document, false);
        for (kind, after) in [
            ("html", format!("{}\n<p>BUG</p>", before)),
            ("error", String::new()),
            ("normalized", before.clone()),
        ] {
            let fake = Failure {
                kind,
                output: output.clone(),
                before: before.clone(),
                after,
            };
            assert_eq!(
                known_gap(&document, &fake, &rules),
                None,
                "a {} failure attributed for {:?}",
                kind,
                source
            );
        }
    }
    assert!(failures > 0, "the corpus still finds the gaps");
}

#[test]
fn clause_3_4_parsed_html_changes_only_in_attention() {
    each_input(|name, source| {
        let mut document = parse(source);
        let before = html(&document);
        let definitions = all(&document, is_definition).len();
        document.normalize();
        if all(&document, is_definition).len() != definitions || before.contains("/><br />") {
            // Copies have the exceptions below; a task item starting with a hard break is #199.
            return;
        }
        assert_eq!(
            without_attention(&html(&document)),
            without_attention(&before),
            "{} {:?}",
            name,
            source
        );
    });
}

#[test]
fn clause_3_4_attention_changes_only_where_tags_go() {
    for (source, before, after) in [
        (
            "*a*_b_\n",
            "<p><em>a</em><em>b</em></p>\n",
            "<p><em>ab</em></p>\n",
        ),
        (
            "**a&#x20;**\n",
            "<p><strong>a </strong></p>\n",
            "<p><strong>a</strong> </p>\n",
        ),
        ("*&#xA;*\n", "<p><em>\n</em></p>\n", "<p>\n</p>\n"),
    ] {
        let mut document = parse(source);
        assert_eq!(html(&document), before, "{:?}", source);
        document.normalize();
        assert_eq!(html(&document), after, "{:?}", source);
    }
}

/// The HTML of `source` as parsed and once normalized.
fn html_change(source: &str) -> (String, String, String) {
    let mut document = parse(source);
    let before = html(&document);
    document.normalize();
    (before, html(&document), to_markdown(&document).unwrap())
}

fn is_list(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Start(Tag::List { .. }))
}

/// Whether `id` holds nothing but definitions, footnote definitions and separators, so that removing copies can empty it.
fn holds_only_definitions<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    document.children(id).all(|child| {
        let kind = &document.node(child).unwrap().kind;
        is_definition(kind)
            || matches!(kind, EventKind::Separator { .. })
            || (is_item(kind) && holds_only_definitions(document, child))
    })
}

/// `before`, as parsed, with the copies that `after`, the same document normalized, removed detached, and exceptions 1, 3 and 4 of 3.4 applied as `after` has them: each list whose tightness or start changed is given its new one, and each list or item that held only definitions and is gone is detached.
fn with_list_exceptions<'s>(
    before: &Document<&'s str>,
    after: &Document<&'s str>,
) -> Document<&'s str> {
    with_exceptions_but_2(before, after, false)
}

/// [`with_list_exceptions`], but when `calls` the removed footnote definition copies that hold calls are kept, rendering nothing but still numbering their calls as parsed, so that only exception 2 is left.
fn with_exceptions_but_2<'s>(
    before: &Document<&'s str>,
    after: &Document<&'s str>,
    calls: bool,
) -> Document<&'s str> {
    let attached: Vec<NodeId> = after.descendants(after.root()).collect();
    let mut expected = before.clone();
    let removed: Vec<NodeId> = all(before, is_definition)
        .into_iter()
        .filter(|id| !attached.contains(id))
        .collect();
    let mut places = Vec::new();
    for &copy in &removed {
        let mut ancestor = before.node(copy).unwrap().parent();
        let parent = ancestor.unwrap();
        while let Some(id) = ancestor {
            if removed.contains(&id) {
                break;
            }
            ancestor = before.node(id).unwrap().parent();
        }
        if ancestor.is_some() {
            // Inside a removed copy, which goes with it.
            continue;
        }
        let next = before
            .children(parent)
            .skip_while(|&id| id != copy)
            .find(|id| attached.contains(id));
        // The definitions in a copy that bind their own labels take its place (3.5).
        let hoisted: Vec<NodeId> = before
            .descendants(copy)
            .filter(|&id| {
                attached.contains(&id)
                    && !attached.contains(&before.node(id).unwrap().parent().unwrap())
            })
            .collect();
        for &id in &hoisted {
            expected.detach(id).unwrap();
        }
        let calling = matches!(
            before.node(copy).unwrap().kind,
            EventKind::Start(Tag::FootnoteDefinition { .. })
        ) && before.descendants(copy).any(|id| {
            !hoisted
                .iter()
                .any(|&kept| before.descendants(kept).any(|inner| inner == id))
                && matches!(
                    before.node(id).unwrap().kind,
                    EventKind::FootnoteReference { .. }
                )
        });
        if !(calls && calling) {
            expected.detach(copy).unwrap();
        }
        places.extend(hoisted.into_iter().map(|id| (parent, id, next)));
    }
    // Once every copy is gone, as attaching beside one is rejected (2.4).
    for (parent, id, next) in places {
        expected.attach(parent, id, next).unwrap();
    }
    // The separators where copies were are given again.
    for id in all(before, |kind| matches!(kind, EventKind::Separator { .. })) {
        if !attached.contains(&id) {
            expected.detach(id).unwrap();
        }
    }
    for id in all(before, |kind| is_list(kind) || is_item(kind)) {
        if !attached.contains(&id) {
            if holds_only_definitions(before, id) {
                expected.detach(id).unwrap();
            }
            continue;
        }
        let kind = &after.node(id).unwrap().kind;
        if let EventKind::Start(Tag::List { start, tight }) = *kind {
            if kind != &before.node(id).unwrap().kind {
                expected
                    .set_kind(id, EventKind::Start(Tag::List { start, tight }))
                    .unwrap();
            }
        }
    }
    expected
}

/// The footnote entries of `html` by their anchor, each without its back references, and the rest of `html`.
fn footnote_entries(html: &str) -> (String, std::collections::BTreeMap<String, String>) {
    let backreference =
        Regex::new(r##" ?<a href="#user-content-fnref-[^"]*" data-footnote-backref=""[^>]*>↩(<sup>\d+</sup>)?</a>"##)
            .unwrap();
    let mut entries = std::collections::BTreeMap::new();
    let Some(at) = html.find("<section data-footnotes") else {
        return (html.to_owned(), entries);
    };
    let section = &html[at..];
    let opening = "<li id=\"user-content-fn-";
    let mut rest = section;
    while let Some(start) = rest.find(opening) {
        let body = &rest[start + opening.len()..];
        let anchor_end = body.find('"').unwrap();
        let anchor = &body[..anchor_end];
        let content_start = anchor_end + 2;
        let mut depth = 1;
        let mut end = content_start;
        while depth > 0 {
            let next = body[end..].find("<li").map(|at| (at, true));
            let close = body[end..].find("</li>").map(|at| (at, false));
            let (offset, opens) = match (next, close) {
                (Some(open), Some(close)) if open.0 < close.0 => open,
                (_, Some(close)) => close,
                _ => panic!("an unclosed footnote entry in {}", html),
            };
            depth = if opens { depth + 1 } else { depth - 1 };
            end += offset + if opens { 3 } else { 5 };
        }
        let content = &body[content_start..end - 5];
        entries.insert(
            anchor.to_owned(),
            backreference.replace_all(content, "").into_owned(),
        );
        rest = &body[end..];
    }
    (html[..at].to_owned(), entries)
}

/// `html` with the numbers and anchors of footnote calls dropped, which exception 2 changes.
fn without_call_numbers(html: &str) -> String {
    let call =
        Regex::new(r##"<sup><a href="#user-content-fn-([^"]*)"[^>]*>\d+</a></sup>"##).unwrap();
    call.replace_all(html, "<sup>[^$1]</sup>").into_owned()
}

/// `html` without attention, which normalizing moves, and without the line ending after a list or at the end, which exception 5 adds or removes.
fn without_list_line_endings(html: &str) -> String {
    without_attention(html)
        .replace("</ul>\n", "</ul>")
        .replace("</ol>\n", "</ol>")
        .trim_end()
        .to_owned()
}

/// Assert that `after`, the HTML of a normalized document, is `expected`, the HTML of the document as parsed with exceptions 1, 3 and 4 applied, but for exception 5 and, when `footnotes`, exception 2: footnote numbers, anchors, back references and the order of entries, and entries no remaining call calls.
fn assert_html_but_for_exceptions(expected: &str, after: &str, footnotes: bool, context: &str) {
    if !footnotes {
        assert_eq!(
            without_list_line_endings(after),
            without_list_line_endings(expected),
            "{}",
            context
        );
        return;
    }
    let (expected_body, expected_entries) = footnote_entries(expected);
    let (after_body, after_entries) = footnote_entries(after);
    let after_body = without_call_numbers(&after_body);
    assert_eq!(
        without_list_line_endings(&after_body),
        without_list_line_endings(&without_call_numbers(&expected_body)),
        "{}",
        context
    );
    let after_entries: std::collections::BTreeMap<_, _> = after_entries
        .iter()
        .map(|(anchor, content)| (anchor, without_call_numbers(content)))
        .collect();
    for (anchor, content) in &expected_entries {
        match after_entries.get(anchor) {
            Some(kept) => assert_eq!(
                without_list_line_endings(kept),
                without_list_line_endings(&without_call_numbers(content)),
                "{}",
                context
            ),
            None => {
                let call = format!("<sup>[^{}]</sup>", anchor);
                assert!(
                    !after_body.contains(&call)
                        && after_entries.values().all(|kept| !kept.contains(&call)),
                    "entry {} gone but still called: {}",
                    anchor,
                    context
                );
            }
        }
    }
    assert!(
        after_entries
            .keys()
            .all(|anchor| expected_entries.contains_key(*anchor)),
        "{}",
        context
    );
}

#[test]
fn clause_3_4_copy_removal_keeps_html_but_for_five_exceptions() {
    let mut checked = [0, 0];
    let mut numbered = 0;
    each_input(|name, source| {
        let mut document = parse(source);
        let before = html(&document);
        let parsed = document.clone();
        let definitions = all(&document, is_definition).len();
        // Exception 2 applies where a removed copy is a footnote definition that holds a call.
        let calling: Vec<NodeId> = all(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
        })
        .into_iter()
        .filter(|&id| {
            document.descendants(id).any(|id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::FootnoteReference { .. }
                )
            })
        })
        .collect();
        document.normalize();
        if all(&document, is_definition).len() == definitions || before.contains("/><br />") {
            // No copy; a task item starting with a hard break is #199.
            return;
        }
        let attached: Vec<NodeId> = document.descendants(document.root()).collect();
        let footnotes = calling.iter().any(|id| !attached.contains(id));
        checked[usize::from(footnotes)] += 1;
        let expected = html(&with_exceptions_but_2(&parsed, &document, true));
        let after = html(&document);
        if footnotes && without_list_line_endings(&after) != without_list_line_endings(&expected) {
            numbered += 1;
        }
        assert_html_but_for_exceptions(
            &expected,
            &after,
            footnotes,
            &format!("{} {:?}", name, source),
        );
    });
    assert!(checked[0] > 0 && checked[1] > 0, "{:?}", checked);
    // Exception 2 is needed: some removed copy's calls changed the numbering.
    assert!(numbered > 0, "{:?}", checked);
}

#[test]
fn clause_3_4_the_exceptions_are_applied_exactly() {
    for source in [
        "[x]: /1\n\n- a\n\n  [x]: /2\n",
        "[x]: /1\n\n- p\n  - [x]: /2\n",
        "x[^a]\n\n[^a]: z\n\n- p\n  [^a]: y\n  2. q\n",
        "- [c]: /6\n\n[c]: /6\n",
    ] {
        let parsed = parse(source);
        let mut document = parsed.clone();
        document.normalize();
        assert_html_but_for_exceptions(
            &html(&with_list_exceptions(&parsed, &document)),
            &html(&document),
            false,
            source,
        );
    }
    let source = "[^a]\n\n[^a]: one\n\n[^b]\n\n[^a]: two [^b]\n\n[^b]: b\n";
    let parsed = parse(source);
    let mut document = parsed.clone();
    document.normalize();
    assert_html_but_for_exceptions(&html(&parsed), &html(&document), true, source);
}

#[test]
fn clause_3_4_exception_1_a_loose_single_item_becomes_tight() {
    let (before, after, _) = html_change("[x]: /1\n\n- a\n\n  [x]: /2\n");
    assert_eq!(before, "<ul>\n<li>\n<p>a</p>\n</li>\n</ul>\n");
    assert_eq!(after, "<ul>\n<li>a</li>\n</ul>\n");
}

#[test]
fn clause_3_4_exception_2_footnote_numbering_follows_visible_calls() {
    let (before, after, _) =
        html_change("[^a]\n\n[^a]: one\n\n[^b]\n\n[^a]: two [^b]\n\n[^b]: b\n");
    assert!(before.contains("fnref-b-2"), "{}", before);
    assert!(!after.contains("fnref-b-2"), "{}", after);
}

#[test]
fn clause_3_4_exception_3_an_emptied_item_after_a_paragraph_goes() {
    let (before, after, output) = html_change("[x]: /1\n\n- p\n  - [x]: /2\n");
    assert_eq!(
        before,
        "<ul>\n<li>p\n<ul>\n<li></li>\n</ul>\n</li>\n</ul>\n"
    );
    assert_eq!(after, "<ul>\n<li>p</li>\n</ul>\n");
    assert_eq!(output, "[x]: /1\n\n- p\n");
}

#[test]
fn clause_3_4_exception_4_unwritable_tight_blocks_are_rewritten() {
    let (before, after, output) = html_change("x[^a]\n\n[^a]: z\n\n- p\n  [^a]: y\n  2. q\n");
    assert!(before.contains("<ol start=\"2\">"), "{}", before);
    assert!(!after.contains("start="), "{}", after);
    assert!(output.contains("- p\n  1. q\n"), "{}", output);
}

#[test]
fn clause_3_4_exception_5_separators_after_a_list_follow_the_parser() {
    let (before, after, output) = html_change("- [c]: /6\n\n[c]: /6\n");
    assert_eq!(before, "<ul>\n<li></li>\n</ul>");
    assert_eq!(after, "<ul>\n<li></li>\n</ul>\n");
    assert_eq!(output, "- [c]: /6\n");
    assert_eq!(html(&parse(&output)), after);
}

#[test]
fn clause_3_4_edited_trees_join_paragraphs_in_tight_items() {
    let mut document = parse("- a\n- b\n");
    let source = parse("Para\n");
    let item = find(&document, is_item);
    let paragraph_id = first(&document, item);
    document
        .insert_from(
            item,
            Some(paragraph_id),
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    let before = html(&document);
    assert_eq!(before, "<ul>\n<li>a\nPara</li>\n<li>b</li>\n</ul>\n");
    document.normalize();
    assert_eq!(html(&document), before);
    assert_eq!(to_markdown(&document).unwrap(), "- a\n  Para\n- b\n");
}

/// The non-separator children of `id`.
fn blocks_of<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> Vec<NodeId> {
    document
        .children(id)
        .filter(|&id| !matches!(document.node(id).unwrap().kind, EventKind::Separator { .. }))
        .collect()
}

/// The first item of the tight list in `context` whose first block is a paragraph, and `document` with its children parsed from `fragment`.
fn parsed_into(context: &'static str, fragment: &str) -> (Document<&'static str>, NodeId) {
    let mut document = parse(context);
    let item = all(&document, is_item)
        .into_iter()
        .find(|&item| is_paragraph(&document.node(first(&document, item)).unwrap().kind))
        .unwrap();
    document
        .parse_children(item, fragment, &ParseOptions::default())
        .unwrap();
    (document, item)
}

/// The first item of the tight list in `context` whose first block is a paragraph, given `blocks` in a row, not counting separators, with the separators `parse_children` gives them. Each two are parsed apart by a blank line, or, where that would merge them, such as two lists of one kind, by an HTML comment, which is then removed with the separator after it, so that they are in a row as an edit can put them. `@` in a block is its label, unique to its place.
fn tight_item(context: &'static str, blocks: &[&str]) -> (Document<&'static str>, NodeId) {
    let blocks: Vec<String> = blocks
        .iter()
        .enumerate()
        .map(|(at, block)| block.replace('@', &format!("l{}", at)))
        .collect();
    let mut fragment = blocks[0].clone();
    for pair in blocks.windows(2) {
        let (document, item) = parsed_into(context, &format!("{}\n\n{}", pair[0], pair[1]));
        fragment.push_str(if blocks_of(&document, item).len() == 2 {
            "\n\n"
        } else {
            "\n\n<!---->\n\n"
        });
        fragment.push_str(&pair[1]);
    }
    let (mut document, item) = parsed_into(context, &fragment);
    let separator = |document: &Document<&str>, id: Option<&NodeId>| {
        id.is_some_and(|&id| matches!(document.node(id).unwrap().kind, EventKind::Separator { .. }))
    };
    for spacer in blocks_of(&document, item) {
        if !matches!(&document.node(spacer).unwrap().kind, EventKind::Html { value, .. } if value.resolve(document.source()).unwrap() == "<!---->")
        {
            continue;
        }
        let children: Vec<NodeId> = document.children(item).collect();
        let at = children.iter().position(|&id| id == spacer).unwrap();
        document.detach(spacer).unwrap();
        if separator(&document, children.get(at + 1)) {
            document.detach(children[at + 1]).unwrap();
        }
    }
    assert_eq!(
        blocks_of(&document, item).len(),
        blocks.len(),
        "{:?}",
        blocks
    );
    (document, item)
}

/// `document` with each list given the tightness and start it has in `normalized`, the same document normalized: the only change to the HTML that the tight-item rules make, as joining paragraphs in a tight list renders the same (3.4).
fn with_lists_of<'s>(
    document: &Document<&'s str>,
    normalized: &Document<&'s str>,
) -> Document<&'s str> {
    let mut expected = document.clone();
    for id in all(document, is_list) {
        let kind = &normalized.node(id).unwrap().kind;
        if let EventKind::Start(Tag::List { start, tight }) = *kind {
            if kind != &document.node(id).unwrap().kind {
                expected
                    .set_kind(id, EventKind::Start(Tag::List { start, tight }))
                    .unwrap();
            }
        }
    }
    expected
}

/// Assert that `blocks` in a row in an item of the tight list `context` normalize idempotently (3.2), change the HTML only where a list is made loose or renumbered (3.4) and round trip with `options` (3.3). With `fences: false`, indented code after a list is written after `<!---->`, which is #157.
fn assert_tight_item_settles(context: &'static str, blocks: &[&str], options: &MarkdownOptions) {
    let (document, _) = tight_item(context, blocks);
    let mut normalized = document.clone();
    normalized.normalize();
    assert!(normalized.is_normalized(), "{:?}", blocks);
    let once = snapshot(&normalized);
    let mut again = normalized.clone();
    again.normalize();
    assert_eq!(snapshot(&again), once, "{:?}", blocks);
    assert_eq!(
        html(&normalized),
        html(&with_lists_of(&document, &normalized)),
        "{:?} in {:?}",
        blocks,
        context
    );
    match round_trip(document, options, false) {
        Outcome::Pass => {}
        Outcome::Fail(failure) if !options.fences && failure.output.contains("<!---->") => {}
        Outcome::Rejected => panic!("rejected {:?}", blocks),
        Outcome::Fail(failure) => panic!(
            "{} {:?} in {:?}: {:?}\n{}\n{}",
            failure.kind, blocks, context, failure.output, failure.before, failure.after
        ),
    }
}

/// One block of each kind that can be in an item, with variants where what follows or precedes it matters: paragraphs that could be read as a title, headings ATX and setext, code fenced and indented, HTML blocks of each of the seven kinds, definitions with and without a title, a table, footnote definitions, and block quotes and lists ending in a paragraph or in something else. An HTML block of kinds 1–5 that its last line does not close, which takes every line after it, is not here.
const TIGHT_ITEM_BLOCKS: [&str; 39] = [
    "a",
    "\"t\"",
    "(t)",
    "# h",
    "#\t",
    "h\\\ni\n===",
    "***",
    "```\nx\n```",
    "    x",
    "<pre>\nx\n</pre>",
    "<pre>x</pre>",
    "<!-- c -->",
    "<?x?>",
    "<!X>",
    "<![CDATA[x]]>",
    "<div>",
    "</div>",
    "<b>",
    "</b>",
    "<x-y a=\"1\">",
    "[@]: /u",
    "[@]: /u \"t\"",
    "| a |\n| - |",
    "[^@]: x",
    "[^@]: # h",
    "> q",
    "> - q",
    "> # h",
    "> <div>",
    "> a\n>\n> b",
    "- l",
    "1. o",
    "2. o",
    "- # h",
    "- <b>",
    "-     c",
    "- [ ] t",
    "1. a\n2. b",
    "- a\n\n  b",
];

const TIGHT_LISTS: [&str; 4] = [
    "- p\n- q\n",
    "1. p\n1. q\n",
    "> - p\n> - q\n",
    "- - p\n  - q\n- r\n",
];

#[test]
fn clause_3_3_tight_items_round_trip_every_pair_of_blocks() {
    for left in TIGHT_ITEM_BLOCKS {
        for right in TIGHT_ITEM_BLOCKS {
            assert_tight_item_settles(TIGHT_LISTS[0], &[left, right], &MarkdownOptions::default());
        }
    }
}

#[test]
fn clause_3_3_tight_items_round_trip_random_blocks() {
    let options = [
        MarkdownOptions::default(),
        MarkdownOptions {
            fences: false,
            ..MarkdownOptions::default()
        },
        MarkdownOptions {
            tight_definitions: true,
            bullet: '*',
            ..MarkdownOptions::default()
        },
        MarkdownOptions {
            list_item_indent: base_markdown::IndentOptions::Tab,
            ..MarkdownOptions::default()
        },
    ];
    let mut random = Random(0x7167_4752);
    for _ in 0..SAMPLES {
        let blocks: Vec<&str> = (0..1 + random.below(4))
            .map(|_| TIGHT_ITEM_BLOCKS[random.below(TIGHT_ITEM_BLOCKS.len())])
            .collect();
        let context = TIGHT_LISTS[random.below(TIGHT_LISTS.len())];
        let options = &options[random.below(options.len())];
        // The parser reads indented code that starts an item on the line after a definition as a paragraph, `[a]: /u\n-     c`, a parser gap that only indented code written with `fences: false` reaches.
        if !options.fences
            && blocks
                .windows(2)
                .any(|pair| pair[0].starts_with("[@]") && pair[1] == "-     c")
        {
            continue;
        }
        assert_tight_item_settles(context, &blocks, options);
    }
}

#[test]
fn clause_3_4_tight_items_loosen_where_a_block_needs_a_blank_line() {
    for (fragment, output) in [
        // A paragraph and a kind-7 HTML block, which cannot interrupt it (#232, #219).
        ("a\n\n <b>", "- a\n\n   <b>\n\n- q\n"),
        // A kind-7 HTML block and a heading, which it would take as a line (#232, #191).
        ("<b>\n\n#\t", "- <b>\n\n  #\n\n- q\n"),
        // A paragraph and a definition, which cannot interrupt it (#232).
        ("a\n\n[a]: /x", "- a\n\n  [a]: /x\n\n- q\n"),
        // A kind-6 HTML block and a paragraph (#191).
        ("<div>\n\na", "- <div>\n\n  a\n\n- q\n"),
    ] {
        let mut document = parse("- p\n- q\n");
        let item = find(&document, is_item);
        document
            .parse_children(item, fragment, &ParseOptions::default())
            .unwrap();
        let mut normalized = document.clone();
        normalized.normalize();
        let loose = html(&with_lists_of(&document, &normalized));
        assert!(loose.contains("<li>\n"), "{:?}: {}", fragment, loose);
        assert_eq!(html(&normalized), loose, "{:?}", fragment);
        assert_eq!(to_markdown(&normalized).unwrap(), output, "{:?}", fragment);
        assert!(
            matches!(
                round_trip(document, &MarkdownOptions::default(), false),
                Outcome::Pass
            ),
            "{:?}",
            fragment
        );
    }
    // A kind 1–5 HTML block closed on its last line ends there, so the list stays tight.
    let (mut document, _) = tight_item("- p\n- q\n", &["<!-- c -->", "a"]);
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "- <!-- c -->\n  a\n- q\n");
    // A parsed HTML block that interrupts a paragraph after its leading spaces stays tight.
    for input in [
        "- a\n   <div>\n- q\n",
        "- a\n   <!-- c -->\n- q\n",
        "- p\n    <div>\n- q\n",
    ] {
        let document = parse(input);
        assert!(document.is_normalized(), "{:?}", input);
        assert_eq!(to_markdown(&document).unwrap(), input);
    }
    // Two tables, which would read back as one.
    let (mut document, _) = tight_item("- p\n- q\n", &["| a |\n| - |", "| b |\n| - |"]);
    document.normalize();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "- | a |\n  | --- |\n\n  | b |\n  | --- |\n\n- q\n"
    );
}

#[test]
fn clause_3_5_copies_are_dismantled() {
    let mut document = parse("x[^a]\n\n[^a]: one\n\n[^a]: *two*\n\n    [b]: /u\n\n[b]\n");
    let copy = all(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    })[1];
    let held: Vec<NodeId> = document.descendants(copy).skip(1).collect();
    let hoisted = find(&document, |kind| {
        matches!(kind, EventKind::Definition { .. })
    });
    document.normalize();
    assert_eq!(all(&document, is_definition).len(), 2);
    assert!(
        document.node(hoisted).unwrap().parent().is_some(),
        "a binding definition stays"
    );
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(document.destination(link).unwrap().0, "/u");
    for id in held.into_iter().filter(|&id| id != hoisted) {
        let node = document.node(id).unwrap();
        assert!(node.parent().is_none());
        assert!(
            matches!(&node.kind, EventKind::Text(value) if value.resolve(document.source()).unwrap().is_empty())
        );
        assert_eq!(document.children(id).count(), 0);
    }
}

#[test]
fn clause_3_6_other_ids_stay_valid() {
    let mut document = parse("a\\*b\n\n# c\n");
    let paragraph_id = first(&document, document.root());
    let heading = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Heading(_)))
    });
    document.normalize();
    assert_eq!(first(&document, document.root()), paragraph_id);
    assert!(matches!(
        document.node(heading).unwrap().kind,
        EventKind::Start(Tag::Heading(1))
    ));
}

// 4. Settled.

#[test]
fn clause_4_1_settled_is_not_normalized() {
    let mut document = parse("a\\*b\n");
    assert!(settled(&document));
    assert!(!document.is_normalized());
    let root = document.root();
    document.append_child(root, paragraph()).unwrap();
    assert!(!settled(&document));
    document.normalize();
    assert!(settled(&document));
    assert!(document.is_normalized());
}

#[test]
fn clause_4_2_what_keeps_and_clears_settled() {
    let source = parse("Para[^n]\n\n[^n]: note\n");
    let fresh = || parse("a\n\n- b\n");
    // Kept: insert_from, failed edits, text_mut, set_syntax, cloning and frontmatter_mut.
    let mut document = fresh();
    let root = document.root();
    document
        .insert_from(
            root,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    assert!(settled(&document));
    let _ = document.append_child(root, text("x"));
    assert!(settled(&document));
    let text_id = find(&document, |kind| matches!(kind, EventKind::Text(_)));
    document.text_mut(text_id).unwrap().push('!');
    assert!(settled(&document));
    let item = find(&document, is_item);
    document
        .set_syntax(item, Some(NodeSyntax::ListItem { spread: false }))
        .unwrap();
    assert!(settled(&document));
    assert!(settled(&document.clone()));
    let mut with_frontmatter = parse("---\na: b\n---\n\nx\n");
    with_frontmatter
        .frontmatter_mut()
        .unwrap()
        .push_str("\nc: d");
    assert!(settled(&with_frontmatter));
    // Cleared by every other successful edit.
    type Edit = fn(&mut Document<&'static str>);
    let edits: [(&str, Edit); 7] = [
        ("set_kind", |document| {
            let id = find(document, |kind| matches!(kind, EventKind::Text(_)));
            document.set_kind(id, text("z")).unwrap();
        }),
        ("append_child", |document| {
            let root = document.root();
            document.append_child(root, paragraph()).unwrap();
        }),
        ("detach", |document| {
            let id = find(document, is_paragraph);
            document.detach(id).unwrap();
        }),
        ("attach", |document| {
            let id = find(document, is_paragraph);
            document.detach(id).unwrap();
            document.normalize();
            let root = document.root();
            document.attach(root, id, None).unwrap();
        }),
        ("extract_children", |document| {
            let root = document.root();
            document.extract_children(root).unwrap();
        }),
        ("parse_children", |document| {
            let root = document.root();
            document
                .parse_children(root, "x", &ParseOptions::default())
                .unwrap();
        }),
        ("replace_heading_section", |document| {
            let root = document.root();
            document
                .parse_children(root, "# h\n\nx\n", &ParseOptions::default())
                .unwrap();
            document.normalize();
            let replacement = Document::parse("y\n", &ParseOptions::default()).unwrap();
            assert!(document
                .replace_heading_section(root, 0, &replacement, false, true)
                .unwrap());
        }),
    ];
    for (name, edit) in edits {
        let mut document = Document::parse("a\n\n- b\n", &ParseOptions::default()).unwrap();
        edit(&mut document);
        assert!(!settled(&document), "{}", name);
    }
}

#[cfg(feature = "serde")]
#[test]
fn clause_4_2_a_deserialized_document_is_not_settled() {
    let document = parse("a\n");
    let json = serde_json::to_string(&document).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert!(!settled(&loaded));
}

// 5. Output.

#[test]
fn clause_5_1_output_is_lf_terminated_and_deterministic() {
    each_input(|name, source| {
        let document = normalized(source);
        let output = to_markdown(&document).unwrap();
        assert!(!output.contains('\r'), "{} {:?}", name, source);
        // A parsed root holds blocks.
        assert!(
            output.is_empty() || output.ends_with('\n'),
            "{} {:?}",
            name,
            source
        );
        assert_eq!(to_markdown(&document).unwrap(), output);
    });
}

#[test]
fn clause_5_2_unnormalized_trees_are_written_as_they_are() {
    let mut document = parse("- a\n- b\n");
    let source = parse("Para\n");
    let item = find(&document, is_item);
    let paragraph_id = first(&document, item);
    document
        .insert_from(
            item,
            Some(paragraph_id),
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    assert_eq!(all(&document, is_paragraph).len(), 3, "two stay two");
    assert_eq!(to_markdown(&document).unwrap(), "- a\n\n  Para\n- b\n");
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "- a\n  Para\n- b\n");
}

#[test]
fn clause_5_3_writing_errors_are_errors() {
    let deep = "> ".repeat(129);
    let document = parse(&deep);
    let error = to_markdown(&document).unwrap_err();
    assert!(format!("{:?}", error).contains("nesting-too-deep"));
    let mut document = parse("1. a\n");
    let list = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::List { .. }))
    });
    document
        .set_kind(
            list,
            EventKind::Start(Tag::List {
                start: Some(1_000_000_000),
                tight: true,
            }),
        )
        .unwrap();
    let error = to_markdown(&document).unwrap_err();
    assert!(format!("{:?}", error).contains("list-number-too-long"));
}

#[test]
fn clause_5_4_parsed_html_is_the_parsers() {
    let options = Options::default();
    each_input(|name, source| {
        let document = parse(source);
        assert_eq!(
            html(&document),
            to_html_with_options(source, &options),
            "{} {:?}",
            name,
            source
        );
    });
}

#[test]
fn clause_5_4_render_events_rejects_references() {
    let events = [
        Event {
            kind: EventKind::Start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            }),
            span: 0..0,
        },
        Event {
            kind: EventKind::Start(Tag::Paragraph { tight: false }),
            span: 0..0,
        },
        Event {
            kind: EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                kind: ReferenceKind::Shortcut,
                label: Text::Literal("a"),
            })))),
            span: 0..0,
        },
        Event {
            kind: EventKind::End(TagKind::Link),
            span: 0..0,
        },
        Event {
            kind: EventKind::End(TagKind::Paragraph),
            span: 0..0,
        },
        Event {
            kind: EventKind::End(TagKind::Document),
            span: 0..0,
        },
    ];
    let mut output = String::new();
    assert!(matches!(
        render_events(
            "",
            IntoIterator::into_iter(events).map(Ok),
            &CompileOptions::default(),
            &mut output
        ),
        Err(StreamError::InvalidValue)
    ));
    let document = parse("[a]\n\n[a]: /u\n");
    let mut output = String::new();
    render_events(
        document.source(),
        document.events(),
        &CompileOptions::default(),
        &mut output,
    )
    .unwrap();
    assert_eq!(output, "<p><a href=\"/u\">a</a></p>\n");
}

#[test]
fn clause_5_4_render_node_renders_a_fragment_with_the_documents_bindings() {
    let document = parse("[a]\n\n[a]: /u\n");
    let mut output = String::new();
    document
        .render_node(
            first(&document, document.root()),
            &CompileOptions::default(),
            &mut output,
        )
        .unwrap();
    assert_eq!(output, "<p><a href=\"/u\">a</a></p>");
}

#[test]
fn clause_5_5_normalize_markdown_is_parse_normalize_unwrap() {
    let options = MarkdownOptions {
        line_wrapping: LineWrapping::Unwrap,
        ..MarkdownOptions::default()
    };
    each_input(|name, source| {
        let document = normalized(source);
        assert_eq!(
            normalize_markdown(source).unwrap(),
            to_markdown_with_options(&document, &options).unwrap(),
            "{} {:?}",
            name,
            source
        );
    });
}

// 6. Edits.

#[test]
fn clause_6_1_failed_edits_change_nothing() {
    let mut document = parse("# h\n\na *b*\n\n- c\n\n[d]: /u\n");
    let root = document.root();
    let heading = first(&document, root);
    let paragraph_id = all(&document, is_paragraph)[0];
    let item = find(&document, is_item);
    let source = parse("# x\n\n[^d]\n\n[^d]: e\n");
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.set_kind(heading, EventKind::Start(Tag::BlockQuote))
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.attach(item, root, None)
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.detach(root)
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.parse_children(paragraph_id, "# x", &ParseOptions::default())
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.insert_from(
            root,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions {
                heading_shift: 6,
                ..InsertOptions::default()
            },
        )
    });
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.text_mut(paragraph_id).map(|_| ())
    });
}

#[test]
fn clause_6_2_error_kinds() {
    let mut document = parse("a\n\n- b\n\n> q\n\n[d]: /u\n\n[d]: /v\n");
    let root = document.root();
    let paragraph_id = first(&document, root);
    let text_id = first(&document, paragraph_id);
    let list = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::List { .. }))
    });
    let item = first(&document, list);
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let binding = find(&document, is_definition);
    let loose = document.append_child(root, paragraph()).unwrap();
    document.detach(loose).unwrap();
    let larger_source = "a\n\n".repeat(100);
    let larger = parse(&larger_source);
    let outside = larger.descendants(larger.root()).last().unwrap();
    let source = parse("s[^n]\n\n[^n]: other\n\n# t\n");
    let nodes: Vec<NodeId> = source.children(source.root()).collect();
    let mut with_note = parse("x[^n]\n\n[^n]: mine\n");
    let note_root = with_note.root();
    let far = Text::Source(0..10_000);

    // InvalidRange: an ID that is not in the arena, as a node or a parent.
    type Edit = fn(&mut Document<&str>, NodeId, &Document<&str>) -> Result<(), StreamError>;
    let invalid_ids: [Edit; 11] = [
        |document, id, _| document.set_kind(id, text("x")),
        |document, id, _| document.append_child(id, text("x")).map(|_| ()),
        |document, id, _| document.detach(id),
        |document, id, _| document.attach(document.root(), id, None),
        |document, id, _| {
            let paragraph_id = first(document, document.root());
            document.attach(id, paragraph_id, None)
        },
        |document, id, _| document.extract_children(id).map(|_| ()),
        |document, id, _| document.text_mut(id).map(|_| ()),
        |document, id, _| document.set_syntax(id, None),
        |document, id, _| {
            document
                .parse_children(id, "x", &ParseOptions::default())
                .map(|_| ())
        },
        |document, id, source| {
            document
                .insert_from(
                    id,
                    None,
                    source,
                    source.children(source.root()),
                    &InsertOptions::default(),
                )
                .map(|_| ())
        },
        |document, id, source| {
            document
                .replace_heading_section(id, 0, source, true, true)
                .map(|_| ())
        },
    ];
    for edit in invalid_ids {
        assert_rejected(&mut document, StreamError::InvalidRange, |document| {
            edit(document, outside, &source)
        });
    }
    // A source range outside the source.
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.append_child(paragraph_id, EventKind::Text(far.clone()))
    });
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.set_kind(text_id, EventKind::Text(far.clone()))
    });
    // A position that is not a child of the parent.
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.attach(root, loose, Some(text_id))
    });
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.insert_from(
            root,
            Some(text_id),
            &source,
            nodes.iter().copied(),
            &InsertOptions::default(),
        )
    });
    // A position that is not in the arena.
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.attach(root, loose, Some(outside))
    });
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.insert_from(
            root,
            Some(outside),
            &source,
            nodes.iter().copied(),
            &InsertOptions::default(),
        )
    });
    // A source node that is not in the source's arena, as from another document's.
    assert!(source.node(outside).is_none());
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.insert_from(root, None, &source, [outside], &InsertOptions::default())
    });
    // A node outside the source's attached document.
    let mut detached_source = parse("s\n\nt\n");
    let detached = all(&detached_source, is_paragraph)[1];
    detached_source.detach(detached).unwrap();
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.insert_from(
            root,
            None,
            &detached_source,
            [detached],
            &InsertOptions::default(),
        )
    });
    // text_mut of a node that is not text, and syntax for an incompatible node.
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.text_mut(paragraph_id).map(|_| ())
    });
    assert_rejected(&mut document, StreamError::InvalidRange, |document| {
        document.set_syntax(paragraph_id, Some(NodeSyntax::ListItem { spread: false }))
    });

    // Unbalanced: a leaf parent, an end event appended, a cycle, moving or detaching the root.
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.append_child(text_id, text("x"))
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.attach(text_id, loose, None)
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.parse_children(text_id, "x", &ParseOptions::default())
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.append_child(paragraph_id, EventKind::End(TagKind::Paragraph))
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.attach(item, list, None)
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.attach(quote, root, None)
    });
    assert_rejected(&mut document, StreamError::Unbalanced, |document| {
        document.detach(root)
    });

    // InvalidNesting: what the content model does not allow there, a change of kind that
    // set_kind cannot make in place, a fragment that does not parse into what its parent holds.
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.append_child(root, text("x"))
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.attach(paragraph_id, loose, None)
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.set_kind(paragraph_id, EventKind::Start(Tag::Heading(1)))
    });
    assert_rejected(&mut document, StreamError::InvalidNesting, |document| {
        document.parse_children(paragraph_id, "# x", &ParseOptions::default())
    });

    // InvalidValue: an invalid value, a copy, a move of a definition whose label has a copy, a
    // footnote clash under Reject, a fresh identifier too long to be one.
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, EventKind::Start(Tag::Heading(7)))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, definition("", "/x"))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.append_child(root, definition("D", "/w"))
    });
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.attach(quote, binding, None)
    });
    assert_rejected(&mut with_note, StreamError::InvalidValue, |document| {
        document.insert_from(
            note_root,
            None,
            &source,
            nodes.iter().copied(),
            &InsertOptions {
                footnotes: FootnotePolicy::Reject,
                ..InsertOptions::default()
            },
        )
    });
    let long = "a".repeat(999);
    let long_source = format!("x[^{0}]\n\n[^{0}]: n\n", long);
    let mut long_note = parse(&long_source);
    let long_copy = parse(&long_source);
    let long_root = long_note.root();
    assert_rejected(&mut long_note, StreamError::InvalidValue, |document| {
        document.insert_from(
            long_root,
            None,
            &long_copy,
            long_copy.children(long_copy.root()),
            &InsertOptions::default(),
        )
    });
}

#[test]
fn clause_6_3_insert_from_copies_render_as_their_originals() {
    let source = parse("# s\n\n[l][r] x[^n]\n\n[r]: /r\n\n[^n]: note\n");
    let mut document = parse("a[^n]\n\n[^n]: mine\n");
    let root = document.root();
    let nodes: Vec<NodeId> = source.children(source.root()).collect();
    let inserted = document
        .insert_from(
            root,
            None,
            &source,
            nodes.iter().copied(),
            &InsertOptions {
                heading_shift: 1,
                ..InsertOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        inserted.footnote_definitions.len(),
        1,
        "the call brings its note"
    );
    document.normalize();
    let output = to_markdown(&document).unwrap();
    assert!(output.starts_with("## s\n"), "{}", output);
    assert!(
        output.contains("[l](/r)"),
        "a reference becomes a resource: {}",
        output
    );
    assert!(
        !output.contains("[r]:"),
        "no definition is copied: {}",
        output
    );
    assert!(output.contains("[^n-2]: note"), "relabelled: {}", output);
    assert!(output.contains("[^n]: mine"), "{}", output);
    let mut document = parse("a[^n]\n\n[^n]: mine\n");
    let root = document.root();
    assert_rejected(&mut document, StreamError::InvalidValue, |document| {
        document.insert_from(
            root,
            None,
            &source,
            nodes.iter().copied(),
            &InsertOptions {
                footnotes: FootnotePolicy::Reject,
                ..InsertOptions::default()
            },
        )
    });
}

#[test]
fn clause_6_3_same_document_moves_keep_names() {
    let mut document = parse("> [a]\n\n[a]: /1\n");
    let quote = first(&document, document.root());
    let paragraph_id = first(&document, quote);
    document.detach(paragraph_id).unwrap();
    let root = document.root();
    document.attach(root, paragraph_id, None).unwrap();
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert!(matches!(
        document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(_)))
    ));
    assert_eq!(document.destination(link).unwrap().0, "/1");
}

// 7. Costs.

#[test]
fn clause_7_1_write_and_render_are_linear() {
    assert_linear("write and render", 300, |size| {
        let source = "# h\n\na *b* [c][d] `e`\n\n- f\n  > g\n\n[d]: /u\n".repeat(size);
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        document.normalize();
        timed(|| {
            to_markdown(&document).unwrap();
            html(&document);
        })
    });
}

#[test]
#[ignore = "#226: lookups in a detached tree build a table each time"]
fn clause_7_2_issue_226_detached_lookups() {
    assert_linear("text_content in a detached tree", 500, |size| {
        let source = "> [a]\n>\n".repeat(size) + "\n[a]: /u\n";
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let quote = first(&document, document.root());
        document.detach(quote).unwrap();
        let paragraphs: Vec<NodeId> = document.children(quote).collect();
        timed(|| {
            for &paragraph_id in &paragraphs {
                document.text_content(paragraph_id);
            }
        })
    });
}

#[test]
#[ignore = "#233: edits deep in a phrasing nest walk every ancestor"]
fn clause_7_2_issue_233_phrasing_depth() {
    assert_linear("append_child deep in phrasing", 200, |size| {
        let mut document = parse("a\n");
        let mut parent = first(&document, document.root());
        for _ in 0..size {
            parent = document
                .append_child(parent, EventKind::Start(Tag::Emphasis))
                .unwrap();
        }
        timed(|| {
            for _ in 0..size {
                document.append_child(parent, EventKind::HardBreak).unwrap();
            }
        })
    });
}

// 8. Known gaps.

/// Assert that `source` round trips (3.3).
fn assert_round_trips(source: &str) {
    assert_round_trips_with(source, &MarkdownOptions::default());
}

/// Assert that `source` round trips with `options` (3.3).
fn assert_round_trips_with(source: &str, options: &MarkdownOptions) {
    match round_trip(parse(source), options, false) {
        Outcome::Pass => {}
        Outcome::Rejected => panic!("rejected {:?}", source),
        Outcome::Fail(failure) => panic!(
            "{} {:?}: {:?}\n{}\n{}",
            failure.kind, source, failure.output, failure.before, failure.after
        ),
    }
}

/// The repros of #89, #99 and #108 round trip on main, so those issues may be fixed; these keep them so.
#[test]
fn clause_3_3_issue_89_99_108_repros_round_trip() {
    for source in [
        "\\_.\\*a_a/xxmpp:(@é<",
        "http://~!~ ~~a~~",
        "- [ ] \t*<http://a.b>*em**",
        "- a\n| a | b |\n| - | - |\n",
        "> a\n| a | b |\n| - | - |\n",
        "~a~\\~(~",
    ] {
        assert_round_trips(source);
    }
}

#[test]
#[ignore = "#110: nested attention with a switched marker"]
fn gap_issue_110() {
    assert_round_trips("*a*__**c*.*****\\~**&#x61;**__");
}

#[test]
#[ignore = "#157: with fences: false, indented code after a list is written after <!---->"]
fn gap_issue_157() {
    let options = MarkdownOptions {
        fences: false,
        ..MarkdownOptions::default()
    };
    assert_round_trips_with("- a\n\n```\nx\n```\n", &options);
}

#[test]
#[ignore = "#199: a task item starting with a hard break gains a space"]
fn gap_issue_199() {
    let (before, after, _) = html_change("1. [ ]   \na");
    assert_eq!(after, before);
}

#[test]
#[ignore = "#208: a definition label starting with ^ reads back as a footnote definition"]
fn gap_issue_208() {
    assert_round_trips("[a]: /u\n    [^b]: #\n");
}

#[test]
#[ignore = "#211: a shortcut reference whose text needs escaping is written full"]
fn gap_issue_211() {
    let document = normalized("[a*b]\n\n[a*b]: /u\n");
    assert!(to_markdown(&document).unwrap().starts_with("[a\\*b]\n"));
}

#[test]
#[ignore = "#216: a hand-appended separator at the end of an item survives normalizing"]
fn gap_issue_216() {
    let mut document = parse("- a\n");
    let item = find(&document, is_item);
    document
        .append_child(
            item,
            EventKind::Separator {
                value: Text::Literal("\n"),
                conditional: false,
            },
        )
        .unwrap();
    document.normalize();
    assert!(document
        .children(item)
        .all(|id| !matches!(document.node(id).unwrap().kind, EventKind::Separator { .. })));
}

#[test]
#[ignore = "#223: inline-root text [b][c] writes non-idempotent output"]
fn gap_issue_223() {
    let mut document = parse("x\n");
    let root = document.root();
    document.extract_children(root).unwrap();
    document.append_child(root, text("[b][c]")).unwrap();
    document.normalize();
    let output = to_markdown(&document).unwrap();
    let mut again = Document::parse(output.as_str(), &ParseOptions::default()).unwrap();
    again.normalize();
    assert_eq!(to_markdown(&again).unwrap(), output);
}

#[test]
#[ignore = "#225: the renderer counts calls inside copies"]
fn gap_issue_225() {
    let (before, _, _) = html_change("[^a]\n\n[^a]: one\n\n[^b]\n\n[^a]: two [^b]\n\n[^b]: b\n");
    assert!(
        !before.contains("fnref-b-2"),
        "a back reference to a call not shown: {}",
        before
    );
}

#[test]
#[ignore = "#227: a * marker before thematic-break content writes a rule"]
fn gap_issue_227() {
    assert_round_trips("- a\n+ ***\n");
}

#[test]
#[ignore = "#228: a paragraph after a quote ending in an empty item leaves the outer list"]
fn gap_issue_228() {
    let mut document = parse("- > - a\n  >\n  > - b\n\n  q");
    let emptied = all(&document, is_paragraph)[1];
    document.detach(emptied).unwrap();
    assert!(matches!(
        round_trip(document, &MarkdownOptions::default(), false),
        Outcome::Pass
    ));
}

#[test]
#[ignore = "#229: an HTML block with an indented first line changes container"]
fn gap_issue_229() {
    let mut document = parse("- x\n\nb\n\n  <div>");
    let detached = all(&document, is_paragraph)[1];
    document.detach(detached).unwrap();
    assert!(matches!(
        round_trip(document, &MarkdownOptions::default(), false),
        Outcome::Pass
    ));
}

#[test]
#[ignore = "#231: attach puts phrasing directly under an emptied root"]
fn gap_issue_231() {
    let mut document = parse("a\n\nb\n");
    let root = document.root();
    let children = document.extract_children(root).unwrap();
    let text_id = first(&document, children[0]);
    document.detach(text_id).unwrap();
    assert!(document.attach(root, text_id, None).is_err());
}

#[test]
#[ignore = "#236: with rule: '-', a rule is written as a setext underline or frontmatter"]
fn gap_issue_236() {
    let options = MarkdownOptions {
        rule: '-',
        ..MarkdownOptions::default()
    };
    for source in ["- a\n  ***\n", "***\n\na\n\n***\n"] {
        assert_round_trips_with(source, &options);
    }
}
