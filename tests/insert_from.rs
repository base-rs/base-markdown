// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

// `Document::insert_from` copies content from another document into this one (#224).

mod support;

use base_markdown::{
    stream::{Event, EventKind, StreamError, Tag, TagKind, Text},
    to_markdown, CompileOptions, Document, FootnotePolicy, HeadingOptions, InsertOptions, NodeId,
    ParseOptions,
};
use support::fixture::canonical;

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::gfm(), &mut output)
        .unwrap();
    output
}

// The error `result` is, as written.
fn failed<T>(result: Result<T, StreamError>) -> String {
    match result {
        Ok(_) => String::from("Ok"),
        Err(error) => format!("{:?}", error),
    }
}

fn find(document: &Document, accept: impl Fn(&EventKind<'_>) -> bool) -> NodeId {
    document
        .descendants(document.root())
        .find(|&id| accept(&document.node(id).unwrap().kind))
        .unwrap()
}

fn heading(document: &Document, text: &str) -> NodeId {
    document
        .headings(&HeadingOptions::default())
        .into_iter()
        .find(|heading| heading.text == text)
        .unwrap()
        .node
}

// Insert all of `source`'s root children at the end of the section of the heading `text`.
fn insert_under(
    document: &mut Document,
    text: &str,
    source: &Document,
    options: &InsertOptions,
) -> Result<base_markdown::Inserted, StreamError> {
    let last = document
        .heading_section_last(heading(document, text), true)
        .unwrap();
    let root = document.root();
    document.insert_from(
        root,
        Some(last),
        source,
        source.children(source.root()),
        options,
    )
}

// Every footnote identifier and link label that `document` defines or names, attached or not.
fn names(document: &Document) -> (Vec<String>, Vec<String>) {
    let source = document.source();
    let mut labels = Vec::new();
    let mut identifiers = Vec::new();
    for node in document.nodes() {
        match &node.kind {
            EventKind::Definition { label, .. } => {
                labels.push(label.resolve(source).unwrap().to_owned());
            }
            EventKind::Start(Tag::FootnoteDefinition { identifier, .. })
            | EventKind::FootnoteReference { identifier, .. } => {
                identifiers.push(identifier.resolve(source).unwrap().to_owned());
            }
            _ => {}
        }
    }
    (labels, identifiers)
}

// Check the invariants an edit keeps: the index binds as a fresh walk does, and normalizing
// reads back as it is written and is idempotent.
fn check(document: &Document, context: &str) {
    let bindings = document.bindings();
    let (labels, identifiers) = names(document);
    for label in &labels {
        assert_eq!(
            document.definition(label),
            bindings.definition(label),
            "{}",
            context
        );
    }
    for identifier in &identifiers {
        assert_eq!(
            document.footnote_definition(identifier),
            bindings.footnote_definition(identifier),
            "{}",
            context
        );
    }
    let mut normalized = document.clone();
    normalized.normalize();
    assert!(normalized.is_normalized(), "{}", context);
    let written = to_markdown(&normalized).unwrap();
    let mut again = normalized.clone();
    again.normalize();
    assert_eq!(to_markdown(&again).unwrap(), written, "{}", context);
    let read = parse(&written);
    // A shortcut or collapsed reference whose text an edit changed is written full, as any edit
    // of its text leaves it, so whether a reference is full is not compared.
    let signature = |document: &Document| -> Vec<String> {
        canonical(document)
            .into_iter()
            .map(|line| line.replace(":Some((true, ", ":Some((false, "))
            .collect()
    };
    assert_eq!(
        signature(&read),
        signature(&normalized),
        "{}: {:?}",
        context,
        written
    );
    assert_eq!(
        words(&html(&read)),
        words(&html(&normalized)),
        "{}: {:?}",
        context,
        written
    );
}

// `html` with each run of whitespace one space, and none next to a tag. The renderer writes a
// separator at the root's edge as a line ending, which reads back as none, and the line endings
// around a tight item's blocks as they were met, which the text does not depend on, but two
// words apart stay apart.
fn words(html: &str) -> String {
    html.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("> ", ">")
        .replace(" <", "<")
}

#[test]
fn inserting_under_a_heading_relabels_clashing_footnotes() {
    let mut document = parse("# A\n\nText.[^n]\n\n# B\n\nMore.\n\n[^n]: Mine.\n");
    let other = parse("New.[^n] [^m]\n\n[^n]: Theirs.\n\n[^m]: Also.\n\n[^u]: Unused.\n");
    let inserted = insert_under(&mut document, "A", &other, &InsertOptions::default()).unwrap();
    assert_eq!(inserted.nodes.len(), 2);
    assert_eq!(inserted.footnote_definitions.len(), 2);
    assert_eq!(
        to_markdown(&document).unwrap(),
        "# A\n\nText.[^n]\n\nNew.[^n-2] [^m]\n\n[^n-2]: Theirs.\n\n[^m]: Also.\n\n# B\n\nMore.\n\n[^n]: Mine.\n"
    );
    assert_eq!(
        document.footnote_definition("N-2"),
        Some(inserted.footnote_definitions[0])
    );
    check(&document, "relabel");
    // Each insert of the same content takes the next fresh identifier.
    for expected in ["N-3", "N-4", "N-5"] {
        let inserted = insert_under(&mut document, "B", &other, &InsertOptions::default()).unwrap();
        let EventKind::Start(Tag::FootnoteDefinition { identifier, .. }) = &document
            .node(inserted.footnote_definitions[0])
            .unwrap()
            .kind
        else {
            panic!("a footnote definition")
        };
        assert_eq!(identifier.resolve(document.source()).unwrap(), expected);
    }
    // `m` clashes once it is here, and takes `M-2`, then `M-3`.
    assert!(document.footnote_definition("M-3").is_some());
    check(&document, "repeated");
}

#[test]
fn fresh_identifiers_skip_those_bound_and_those_copied() {
    // `a-2` is copied as it is, so `a` takes `a-3`; `b-2` is bound here, so `b` takes `b-3`.
    let mut document = parse("[^a] [^b] [^b-2]\n\n[^a]: 1\n\n[^b]: 2\n\n[^b-2]: 3\n");
    let other = parse("[^a] [^a-2] [^b]\n\n[^a]: x\n\n[^a-2]: y\n\n[^b]: z\n");
    let root = document.root();
    let last = document.children(root).last();
    document
        .insert_from(
            root,
            last,
            &other,
            other.children(other.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert!(
        markdown.ends_with("[^a-3] [^a-2] [^b-3]\n\n[^a-3]: x\n\n[^a-2]: y\n\n[^b-3]: z\n"),
        "{:?}",
        markdown
    );
    check(&document, "fresh");
}

#[test]
fn calls_bring_the_definitions_they_call_once_and_transitively() {
    let mut document = parse("# A\n");
    // `a` calls `b`, which is nested in a block quote and calls itself; `c` is never called.
    let other = parse(
        "[^a] [^a] [^z]\n\n[^a]: See [^b].\n\n> [^b]: Again [^b].\n>\n> [^c]: Never.\n\n[x]: /x\n",
    );
    let inserted = insert_under(&mut document, "A", &other, &InsertOptions::default()).unwrap();
    let markdown = to_markdown(&document).unwrap();
    // The block quote held only definitions, so it is empty, and the unbound call is text.
    assert_eq!(
        markdown,
        "# A\n\n[^a] [^a] \\[^z]\n\n>\n\n[^a]: See [^b].\n\n[^b]: Again [^b].\n"
    );
    assert_eq!(inserted.footnote_definitions.len(), 2);
    check(&document, "transitive");
}

#[test]
fn link_definitions_are_not_copied_and_bound_references_become_resources() {
    let mut document = parse("# A\n\n[x]: /mine\n");
    let other = parse("[x] [y][] [z]\n\n[x]: /theirs \"t\"\n\n[y]: /y\n");
    insert_under(&mut document, "A", &other, &InsertOptions::default()).unwrap();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "# A\n\n[x]: /mine\n\n[x](/theirs \"t\") [y](/y) \\[z]\n"
    );
    check(&document, "links");
}

#[test]
fn rejecting_clashes_changes_nothing() {
    let mut document = parse("# A\n\n[^n]\n\n[^n]: Mine.\n");
    let other = parse("[^n]\n\n[^n]: Theirs.\n");
    let before = format!("{:?}", document);
    let options = InsertOptions {
        footnotes: FootnotePolicy::Reject,
        ..InsertOptions::default()
    };
    assert_eq!(
        failed(insert_under(&mut document, "A", &other, &options)),
        "InvalidValue"
    );
    assert_eq!(format!("{:?}", document), before);
    // A settled document stays settled when an insert is rejected.
    document.normalize();
    let before = format!("{:?}", document);
    assert_eq!(
        failed(insert_under(&mut document, "A", &other, &options)),
        "InvalidValue"
    );
    assert_eq!(format!("{:?}", document), before);
    // Without a clash, it is copied.
    let other = parse("[^m]\n\n[^m]: Theirs.\n");
    insert_under(&mut document, "A", &other, &options).unwrap();
    assert!(document.footnote_definition("M").is_some());
}

#[test]
fn headings_shift_and_a_level_out_of_range_is_rejected() {
    let other = parse("# One\n\n## Two\n\n[^n]\n\n[^n]: ### Three\n");
    for (shift, result) in [
        (0, Some([1, 2, 3])),
        (2, Some([3, 4, 5])),
        (3, Some([4, 5, 6])),
        (4, None),
        (-1, None),
        (i8::MIN, None),
        (i8::MAX, None),
    ] {
        let mut document = parse("# A\n");
        let before = format!("{:?}", document);
        let options = InsertOptions {
            heading_shift: shift,
            ..InsertOptions::default()
        };
        let inserted = insert_under(&mut document, "A", &other, &options);
        match result {
            Some(levels) => {
                inserted.unwrap();
                let found: Vec<u8> = document
                    .headings(&HeadingOptions::default())
                    .into_iter()
                    .skip(1)
                    .map(|heading| heading.level)
                    .collect();
                assert_eq!(found, levels, "{}", shift);
            }
            None => {
                assert_eq!(failed(inserted), "InvalidValue", "{}", shift);
                assert_eq!(format!("{:?}", document), before);
            }
        }
    }
}

#[test]
fn copies_follow_the_content_model() {
    let other = parse("# h\n\nSome *text*[^n].\n\n[^n]: Note.\n");
    let paragraph = find(&other, |kind| {
        matches!(kind, EventKind::Start(Tag::Paragraph { .. }))
    });
    let mut document = parse("Before [link](/u) after.\n\n- item\n");
    let before = format!("{:?}", document);
    let target = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Paragraph { .. }))
    });
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let list = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::List { .. }))
    });
    let text = document.children(target).next().unwrap();
    let options = InsertOptions::default();
    for (parent, nodes, error) in [
        // A block in a paragraph.
        (target, vec![paragraph], StreamError::InvalidNesting),
        // A paragraph in a list.
        (list, vec![paragraph], StreamError::InvalidNesting),
        // A footnote call in a link.
        (
            link,
            other.children(paragraph).collect(),
            StreamError::InvalidNesting,
        ),
        // The root.
        (
            document.root(),
            vec![other.root()],
            StreamError::InvalidNesting,
        ),
        // Phrasing in the root.
        (
            document.root(),
            other.children(paragraph).collect(),
            StreamError::InvalidNesting,
        ),
        // A leaf parent.
        (text, vec![paragraph], StreamError::Unbalanced),
    ] {
        assert_eq!(
            failed(document.insert_from(parent, None, &other, nodes, &options)),
            format!("{:?}", error)
        );
        assert_eq!(format!("{:?}", document), before);
    }
    // `after` must be a child of the parent, and the nodes in the source's attached document.
    assert_eq!(
        failed(document.insert_from(target, Some(list), &other, vec![], &options)),
        "InvalidRange"
    );
    let mut detached = other.clone();
    detached.detach(paragraph).unwrap();
    let root = document.root();
    assert_eq!(
        failed(document.insert_from(root, None, &detached, vec![paragraph], &options)),
        "InvalidRange"
    );
    assert_eq!(format!("{:?}", document), before);
    // Phrasing goes into a paragraph, and its footnote definitions after the paragraph.
    let inserted = document
        .insert_from(
            target,
            Some(text),
            &other,
            other.children(paragraph),
            &options,
        )
        .unwrap();
    assert_eq!(
        document
            .node(inserted.footnote_definitions[0])
            .unwrap()
            .parent(),
        Some(document.root())
    );
    assert_eq!(
        to_markdown(&document).unwrap(),
        "Before Some *text*[^n].[link](/u) after.\n\n[^n]: Note.\n\n- item\n"
    );
    check(&document, "phrasing");
}

#[test]
fn copies_into_containers_keep_separators_and_spread() {
    let other = parse("One.\n\n[x]: /x\n\nTwo.\n\n[^n]: n\n");
    // Into an item of a loose list, the blank line between the copied blocks spreads the item.
    let mut document = parse("- a\n\n- b\n");
    let item = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Item { .. }))
    });
    let first = document.children(item).next();
    document
        .insert_from(
            item,
            first,
            &other,
            other.children(other.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    // The separators the definitions leave are dropped, and those after the last block, as the
    // parser writes none at the end of an item, and one goes between the item's paragraph and
    // the first copy, as the parser writes one there.
    let kinds: Vec<bool> = document
        .children(item)
        .map(|id| matches!(document.node(id).unwrap().kind, EventKind::Separator { .. }))
        .collect();
    assert_eq!(kinds, [false, true, false, true, false]);
    assert_eq!(
        to_markdown(&document).unwrap(),
        "- a\n\n  One.\n\n  Two.\n\n- b\n"
    );
    check(&document, "loose item");
    // Into an item of a tight list, paragraphs are joined only by a separator between them.
    let mut document = parse("- a\n- b\n");
    let item = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Item { .. }))
    });
    let first = document.children(item).next();
    let other = parse("> q\n\n> r\n");
    document
        .insert_from(
            item,
            first,
            &other,
            other.children(other.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    check(&document, "tight item");
    // Into a block quote, at its start.
    let mut document = parse("> a\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    let other = parse("# h\n\nb[^n]\n\n[^n]: c\n");
    document
        .insert_from(
            quote,
            None,
            &other,
            other.children(other.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "> # h\n>\n> b[^n]\n>\n> [^n]: c\n>\n> a\n"
    );
    check(&document, "quote");
}

#[test]
fn inserting_into_a_detached_tree_relabels_against_it() {
    let mut document = parse("> [^n]\n>\n> [^n]: Mine.\n\n[^m]\n\n[^m]: Other.\n");
    let quote = document.children(document.root()).next().unwrap();
    document.detach(quote).unwrap();
    let other = parse("[^n] [^m]\n\n[^n]: Theirs.\n\n[^m]: Theirs too.\n");
    let inserted = document
        .insert_from(
            quote,
            None,
            &other,
            other.children(other.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    // Only the detached tree's `n` clashes; the attached document's `m` is not in it.
    for (copy, identifier) in inserted.footnote_definitions.iter().zip(["N-2", "M"]) {
        let EventKind::Start(Tag::FootnoteDefinition {
            identifier: found, ..
        }) = &document.node(*copy).unwrap().kind
        else {
            panic!("a footnote definition")
        };
        assert_eq!(found.resolve(document.source()).unwrap(), identifier);
    }
    assert_eq!(document.footnote_definition("N-2"), None);
    document.attach(document.root(), quote, None).unwrap_err();
    check(&document, "detached");
}

#[test]
fn replacing_a_section_relabels_and_drops_definitions() {
    let mut document = parse("# A\n\nOld.[^n]\n\n[^n]: Old.\n\n# B\n\n[^m]\n\n[^m]: Keep.\n");
    let replacement = parse("# A\n\nNew.[^n] [^m]\n\n[^n]: New.\n\n[^m]: Clash.\n\n[r]: /r\n");
    let root = document.root();
    assert!(document
        .replace_heading_section(root, 0, &replacement, true, false)
        .unwrap());
    // `n` was only the section's, so it is kept; `m` is bound after it, so it is relabelled.
    assert_eq!(
        to_markdown(&document).unwrap(),
        "# A\n\nNew.[^n] [^m-2]\n\n[^n]: New.\n\n[^m-2]: Clash.\n\n# B\n\n[^m]\n\n[^m]: Keep.\n"
    );
    check(&document, "replace");
}

// The containers of `document`'s attached tree.
fn containers(document: &Document) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|&id| matches!(document.node(id).unwrap().kind, EventKind::Start(_)))
        .collect()
}

// `id` rendered alone, with each footnote call, whose number and anchor are the document's, as
// the text `<call>`, without the definitions and footnote definitions it holds, which a copy
// leaves out, and the footnote definition it is, if it is one, and without the paragraphs that a
// tight list leaves out, with whitespace collapsed.
fn rendered_alone(document: &Document, id: NodeId) -> String {
    let document_tag = |start: bool| Event {
        kind: if start {
            EventKind::Start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            })
        } else {
            EventKind::End(TagKind::Document)
        },
        span: 0..0,
    };
    // The root's events are a document's already.
    let root = id == document.root();
    let mut events = if root {
        vec![]
    } else {
        vec![document_tag(true)]
    };
    let mut skipped = 0;
    for (index, event) in document.subtree_events(id).enumerate() {
        let event = event.unwrap();
        match &event.kind {
            EventKind::Start(Tag::FootnoteDefinition { .. }) => {
                if index != 0 {
                    skipped += 1;
                }
            }
            EventKind::End(TagKind::FootnoteDefinition) => {
                skipped = usize::saturating_sub(skipped, 1);
            }
            _ if skipped > 0 => {}
            EventKind::Definition { .. } => {}
            EventKind::FootnoteReference { .. } => events.push(Event {
                kind: EventKind::Text(Text::Literal("<call>")),
                span: 0..0,
            }),
            _ => events.push(event),
        }
    }
    if !root {
        events.push(document_tag(false));
    }
    let mut output = String::new();
    if let Err(error) = base_markdown::render_events(
        document.source(),
        events.into_iter().map(Ok),
        &CompileOptions::gfm(),
        &mut output,
    ) {
        return format!("{:?}", error);
    }
    output
        .replace("<p>", " ")
        .replace("</p>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Random inserts of random runs of nodes from documents of footnotes, references, lists, quotes
/// and tables into random places of others: after each, the index binds as a fresh walk does,
/// normalizing reads back and is idempotent, each copy renders as its original does, but for
/// footnote identifiers and numbers, and a rejected insert changes nothing.
#[test]
fn random_inserts_render_as_their_sources() {
    let sources = [
        "# H\n\nPara [^a] and [r] and *em [^b]*.\n\n[^a]: A [^b].\n\n[^b]: B.\n\n[r]: /r\n",
        "> # Q\n>\n> q [^a] [x][]\n>\n> [^a]: qa\n\n[x]: /x \"t\"\n\n- l1 [^c]\n- l2\n\n[^c]: c\n",
        "1. one\n\n   two [^a]\n\n2. three\n\n[^a]: loose\n",
        "| a | b |\n|---|---|\n| [^t] | [r] |\n\n[^t]: in table\n\n[r]: /t\n\n## Two\n\ntext\n",
        "Line [^self]\n\n[^self]: Self [^self] and [^other]\n\n[^other]: O\n\n### Deep\n",
        "plain [^a-2] [^A]\n\n[^a-2]: dash\n\n[^a]: base\n",
        "- a\n- b [^z]\n  > quote\n\n[^z]: z\n",
        "Text with [link](/u) and ![img][r] [^nope] [nope]\n\n[r]: /i\n",
        "# A\n\nSome [^a].\n\n[^a]: mine\n\n## B\n\n> [^b]: nested\n>\n> b [^b]\n",
        "A [x][a] B ![alt *em*][a] [c] [^u]\n\n- [x][a]\n\n[a]: /u\n\n[c]: /c\n\n[^u]: U\n",
    ];
    let seeds: u64 = std::env::var("INSERT_SEEDS")
        .ok()
        .and_then(|seeds| seeds.parse().ok())
        .unwrap_or(48);
    let mut documents: Vec<Document> = sources.iter().map(|source| parse(source)).collect();
    // The same documents with their definitions detached, whose references and calls nothing
    // binds.
    for source in sources {
        let mut document = parse(source);
        let definitions: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
                )
            })
            .collect();
        for id in definitions {
            if document.node(id).unwrap().parent().is_some() {
                document.detach(id).unwrap();
            }
        }
        documents.push(document);
    }
    for seed in 0..seeds {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = |bound: usize| -> usize {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            std::convert::TryFrom::try_from(state % bound as u64).unwrap()
        };
        let mut document = documents[next(documents.len())].clone();
        for step in 0..24 {
            let context = format!("seed {} step {}", seed, step);
            let source = &documents[next(documents.len())];
            let source_parents = containers(source);
            // The root most often, as copying a document's content is the common case.
            let from = if next(2) == 0 {
                source.root()
            } else {
                source_parents[next(source_parents.len())]
            };
            let children: Vec<NodeId> = source.children(from).collect();
            if children.is_empty() {
                continue;
            }
            let start = next(children.len());
            let end = start + 1 + next(children.len() - start);
            let selected = &children[start..end];
            let parents = containers(&document);
            let parent = if next(2) == 0 {
                document.root()
            } else {
                parents[next(parents.len())]
            };
            let siblings: Vec<NodeId> = document.children(parent).collect();
            let after = match next(siblings.len() + 1) {
                0 => None,
                index => Some(siblings[index - 1]),
            };
            let options = InsertOptions {
                footnotes: if next(4) == 0 {
                    FootnotePolicy::Reject
                } else {
                    FootnotePolicy::Relabel
                },
                heading_shift: [0, 0, 0, 1, -1][next(5)],
            };
            let before = format!("{:?}", document);
            match document.insert_from(parent, after, source, selected.iter().copied(), &options) {
                Err(_) => assert_eq!(format!("{:?}", document), before, "{}", context),
                Ok(inserted) => {
                    let context = format!(
                        "{}: {:?} from {:?}",
                        context,
                        to_markdown(&document).unwrap(),
                        to_markdown(source).unwrap()
                    );
                    check(&document, &context);
                    if options.heading_shift != 0 {
                        continue;
                    }
                    let originals = selected.iter().copied().filter(|&id| {
                        !matches!(
                            source.node(id).unwrap().kind,
                            EventKind::Separator { .. }
                                | EventKind::Definition { .. }
                                | EventKind::Start(Tag::FootnoteDefinition { .. })
                        )
                    });
                    let copies = inserted.nodes.iter().copied().filter(|&id| {
                        !matches!(document.node(id).unwrap().kind, EventKind::Separator { .. })
                    });
                    // A reference that nothing binds is copied as the text around its children,
                    // which render as it does together.
                    let joined = |document: &Document, ids: &mut dyn Iterator<Item = NodeId>| {
                        ids.map(|id| rendered_alone(document, id).replace(' ', ""))
                            .collect::<String>()
                    };
                    assert_eq!(
                        joined(&document, &mut copies.clone()),
                        joined(source, &mut originals.clone()),
                        "{}",
                        context
                    );
                    // The parent renders as its children do apart: no copy runs into a child
                    // beside it.
                    if matches!(
                        document.node(parent).unwrap().kind,
                        EventKind::Start(Tag::Document { .. } | Tag::BlockQuote | Tag::Item { .. })
                    ) {
                        let text = |html: String| -> Vec<String> {
                            let mut text = String::new();
                            let mut tag = false;
                            for character in html.chars() {
                                match character {
                                    '<' => tag = true,
                                    '>' => {
                                        tag = false;
                                        text.push(' ');
                                    }
                                    _ if tag => {}
                                    _ => text.push(character),
                                }
                            }
                            text.split_whitespace().map(String::from).collect()
                        };
                        // An item renders in its list, and its siblings with it.
                        let item = document.node(parent).unwrap().parent().filter(|_| {
                            matches!(
                                document.node(parent).unwrap().kind,
                                EventKind::Start(Tag::Item { .. })
                            )
                        });
                        let container = item.unwrap_or(parent);
                        let parents: Vec<NodeId> = match item {
                            Some(list) => document.children(list).collect(),
                            None => vec![parent],
                        };
                        let apart: Vec<String> = parents
                            .iter()
                            .flat_map(|&parent| document.children(parent))
                            .filter(|&id| {
                                !matches!(
                                    document.node(id).unwrap().kind,
                                    EventKind::Definition { .. }
                                        | EventKind::Start(Tag::FootnoteDefinition { .. })
                                )
                            })
                            .flat_map(|id| text(rendered_alone(&document, id)))
                            .collect();
                        let whole = text(rendered_alone(&document, container));
                        assert_eq!(whole, apart, "{}", context);
                    }
                    // Each copied footnote definition renders as one of the source's.
                    let definitions: Vec<String> = source
                        .descendants(source.root())
                        .filter(|&id| {
                            matches!(
                                source.node(id).unwrap().kind,
                                EventKind::Start(Tag::FootnoteDefinition { .. })
                            )
                        })
                        .map(|id| rendered_alone(source, id))
                        .collect();
                    for &copy in &inserted.footnote_definitions {
                        let rendered = rendered_alone(&document, copy);
                        assert!(
                            definitions.contains(&rendered),
                            "{}: {:?} in {:?}",
                            context,
                            rendered,
                            definitions
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_targets_and_sources_are_rejected() {
    let large = parse("# a\n\nb\n\nc\n\nd\n");
    let far = large.descendants(large.root()).last().unwrap();
    let mut document = parse("x\n");
    let source = parse("y\n");
    let before = to_markdown(&document).unwrap();
    assert_eq!(
        failed(document.insert_from(
            far,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )),
        "InvalidRange"
    );
    assert_eq!(to_markdown(&document).unwrap(), before);
    // Only a heading has a section.
    assert_eq!(document.heading_section_last(far, true), None);
    let paragraph = document.children(document.root()).next().unwrap();
    assert_eq!(document.heading_section_last(paragraph, true), None);
}

#[test]
fn inserting_into_an_empty_document() {
    let mut document = parse("");
    let source = parse("# a\n\nb[^1]\n\n[^1]: c\n");
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
    assert_eq!(to_markdown(&document).unwrap(), "# a\n\nb[^1]\n\n[^1]: c\n");
    check(&document, "empty");
}

#[test]
fn a_fresh_identifier_too_long_to_be_one_is_rejected() {
    let long = "x".repeat(998);
    let text = format!("a[^{long}]\n\n[^{long}]: b\n");
    let mut document = parse(&text);
    let source = parse(&text);
    let before = to_markdown(&document).unwrap();
    let root = document.root();
    assert_eq!(
        failed(document.insert_from(
            root,
            None,
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )),
        "InvalidValue"
    );
    assert_eq!(to_markdown(&document).unwrap(), before);
}

#[test]
fn an_unbound_call_is_copied_as_written() {
    // A call that binds nothing in its own document renders as its text there, and so here.
    let mut source = parse("a\n");
    let paragraph = source.children(source.root()).next().unwrap();
    source
        .append_child(
            paragraph,
            EventKind::FootnoteReference {
                identifier: Text::Literal("NONE"),
                label: None,
            },
        )
        .unwrap();
    let mut document = parse("b\n");
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
    assert_eq!(html(&document), "<p>a[^NONE]</p>\n<p>b</p>\n");
}

#[test]
fn a_call_cannot_be_copied_where_its_definition_cannot_go() {
    // An inline document, whose root holds phrasing, holds no footnote definition.
    let mut document = parse("");
    let root = document.root();
    document
        .append_child(root, EventKind::Text(Text::Literal("c")))
        .unwrap();
    let source = parse("a[^1]\n\n[^1]: b\n");
    let paragraph = source.children(source.root()).next().unwrap();
    assert_eq!(
        failed(document.insert_from(
            root,
            None,
            &source,
            source.children(paragraph),
            &InsertOptions::default(),
        )),
        "InvalidNesting"
    );
    assert_eq!(html(&document), "c");
}

#[test]
fn a_separator_before_a_block_stays_apart_in_a_tight_item() {
    let mut document = parse("- # h\n  # i\n- b\n");
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    let heading = document.children(item).next().unwrap();
    let source = parse("x\n\ny\n");
    let selected: Vec<NodeId> = source.children(source.root()).take(2).collect();
    document
        .insert_from(
            item,
            Some(heading),
            &source,
            selected,
            &InsertOptions::default(),
        )
        .unwrap();
    check(&document, "tight");
    assert_eq!(to_markdown(&document).unwrap(), "- # h\n  x\n  # i\n- b\n");
}

#[test]
fn adjacent_blocks_keep_an_item_tight_in_a_loose_list() {
    let mut document = parse("- a\n\n- b\n");
    let list = document.children(document.root()).next().unwrap();
    let item = document.children(list).next().unwrap();
    let paragraph = document.children(item).next().unwrap();
    let source = parse("# h\ntext\n");
    document
        .insert_from(
            item,
            Some(paragraph),
            &source,
            source.children(source.root()),
            &InsertOptions::default(),
        )
        .unwrap();
    check(&document, "loose");
}

// `document` with its definitions and footnote definitions at the root detached, so that
// nothing binds the references and calls of them.
fn unbound(source: &str) -> Document {
    let mut document = parse(source);
    let definitions: Vec<NodeId> = document
        .children(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Definition { .. } | EventKind::Start(Tag::FootnoteDefinition { .. })
            )
        })
        .collect();
    for id in definitions {
        document.detach(id).unwrap();
    }
    document
}

/// A selected reference that nothing binds becomes the text around its children, all of which
/// are inserted, in order, and listed.
#[test]
fn an_unbound_reference_is_copied_whole() {
    for (text, twice, expected) in [
        ("A [x][a] B\n\n[a]: /u\n", false, "<p>TA [x][a] B</p>\n"),
        (
            "![alt *em*][a]\n\n[a]: /u\n",
            true,
            "<p>T![alt <em>em</em>][a]![alt <em>em</em>][a]</p>\n",
        ),
    ] {
        let source = unbound(text);
        let paragraph = source.children(source.root()).next().unwrap();
        let mut selected: Vec<NodeId> = source.children(paragraph).collect();
        if twice {
            selected.extend(selected.clone());
        }
        let mut document = parse("T\n");
        let target = document.children(document.root()).next().unwrap();
        let last = document.children(target).last();
        let inserted = document
            .insert_from(target, last, &source, selected, &InsertOptions::default())
            .unwrap();
        assert_eq!(html(&document), expected);
        // The listed nodes are the paragraph's children after the one that was there.
        let children: Vec<NodeId> = document.children(target).skip(1).collect();
        assert_eq!(inserted.nodes, children);
        check(&document, text);
    }
}

/// A copied footnote definition takes no identifier that a call here names that nothing binds,
/// as it would make the call call it: neither its own nor a fresh one, and with
/// `FootnotePolicy::Reject` such a call is a clash.
#[test]
fn copied_definitions_capture_no_unbound_call() {
    let target = |text: &str, unbound: &str| {
        let mut document = parse(text);
        let definition = find(&document, |kind| {
            matches!(kind, EventKind::Start(Tag::FootnoteDefinition { identifier, .. })
                if identifier.resolve("").ok() == Some(unbound))
        });
        document.detach(definition).unwrap();
        document
    };
    let insert = |document: &mut Document, text: &str, footnotes: FootnotePolicy| {
        let source = parse(text);
        let root = document.root();
        let last = document.children(root).last();
        let options = InsertOptions {
            footnotes,
            ..InsertOptions::default()
        };
        document
            .insert_from(
                root,
                last,
                &source,
                source.children(source.root()),
                &options,
            )
            .map(|_| ())
    };
    // A fresh identifier skips one that a call names unbound.
    let mut document = target(
        "Mine[^x] and [^x-2]\n\n[^x]: Mine.\n\n[^x-2]: Other.\n",
        "X-2",
    );
    insert(
        &mut document,
        "Theirs[^x]\n\n[^x]: Theirs.\n",
        FootnotePolicy::Relabel,
    )
    .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "Mine[^x] and [^x-2]\n\n[^x]: Mine.\n\nTheirs[^x-3]\n\n[^x-3]: Theirs.\n"
    );
    check(&document, "fresh");
    // So does a copy's own identifier.
    let mut document = target("Mine[^y]\n\n[^y]: Mine.\n", "Y");
    insert(
        &mut document,
        "Theirs[^y]\n\n[^y]: Theirs.\n",
        FootnotePolicy::Relabel,
    )
    .unwrap();
    assert_eq!(
        to_markdown(&document).unwrap(),
        "Mine[^y]\n\nTheirs[^y-2]\n\n[^y-2]: Theirs.\n"
    );
    // With `Reject`, a call that nothing binds clashes, and nothing changes.
    let mut document = target("Mine[^y]\n\n[^y]: Mine.\n", "Y");
    let before = format!("{:?}", document);
    assert_eq!(
        failed(insert(
            &mut document,
            "Theirs[^y]\n\n[^y]: Theirs.\n",
            FootnotePolicy::Reject
        )),
        "InvalidValue"
    );
    assert_eq!(format!("{:?}", document), before);
    // A call in the section that a replacement replaces is gone, so it clashes with nothing.
    let mut document = target("# s\n\nMine[^y]\n\n# t\n\n[^y]: Mine.\n", "Y");
    let root = document.root();
    let replacement = parse("# s\n\nTheirs[^y]\n\n[^y]: Theirs.\n");
    assert!(document
        .replace_heading_section(root, 0, &replacement, true, false)
        .unwrap());
    assert_eq!(
        to_markdown(&document).unwrap(),
        "# s\n\nTheirs[^y]\n\n[^y]: Theirs.\n\n# t\n"
    );
    // A call after the section bound only by a definition the section held calls the copy that
    // takes its identifier, as it called the definition replaced.
    let mut document = target(
        "# s\n\nMine[^y]\n\n[^y]: Mine.\n\n# t\n\nAfter[^y]\n\n[^z]: Z.\n",
        "Z",
    );
    let root = document.root();
    assert!(document
        .replace_heading_section(root, 0, &replacement, true, false)
        .unwrap());
    assert_eq!(
        to_markdown(&document).unwrap(),
        "# s\n\nTheirs[^y]\n\n[^y]: Theirs.\n\n# t\n\nAfter[^y]\n"
    );
}

/// Into an item of a tight list, a copied paragraph beside one of the item's gets the separator
/// between them that keeps them two paragraphs, before the first copy and after the last.
#[test]
fn copied_paragraphs_stay_apart_in_a_tight_item() {
    let source = parse("Para\n");
    for (first, expected) in [
        (true, "<ul>\n<li>a\nPara</li>\n<li>b</li>\n</ul>\n"),
        (false, "<ul>\n<li>Para\na</li>\n<li>b</li>\n</ul>\n"),
    ] {
        let mut document = parse("- a\n- b\n");
        let list = document.children(document.root()).next().unwrap();
        let item = document.children(list).next().unwrap();
        let after = first.then(|| document.children(item).next().unwrap());
        let inserted = document
            .insert_from(
                item,
                after,
                &source,
                source.children(source.root()),
                &InsertOptions::default(),
            )
            .unwrap();
        assert_eq!(html(&document), expected);
        assert_eq!(inserted.nodes.len(), 2);
        check(&document, expected);
    }
}

/// Replacing a section of an item writes what reads back as the same document, loose or tight.
#[test]
fn replacing_a_section_of_an_item_reads_back() {
    for (text, expected) in [
        ("- # A\n\n  old\n- x\n", "- # A\n\n  new\n\n- x\n"),
        ("- # A\n  old\n- x\n", "- # A\n  new\n- x\n"),
    ] {
        let mut document = parse(text);
        let list = document.children(document.root()).next().unwrap();
        let item = document.children(list).next().unwrap();
        assert!(document
            .replace_heading_section(item, 0, &parse("new"), false, true)
            .unwrap());
        let written = to_markdown(&document).unwrap();
        assert_eq!(written, expected);
        assert_eq!(html(&document), html(&parse(&written)));
        check(&document, text);
    }
}
