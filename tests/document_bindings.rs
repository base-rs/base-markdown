// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! References are names and definitions bindings: a document stores a reference by its label
//! alone, and it leads where the first definition of that label in document order does, which
//! is found when asked, so it follows every edit to the definitions.

mod support;
use base_markdown::{
    markdown::{to_markdown, to_markdown_node},
    stream::{Event, EventKind, Reference, Resolved, StreamError, Tag, Target, Text},
    to_html, AstBuilder, Document, EventParser, NodeId, ParseOptions, ReferenceKind,
};
use support::fixture::canonical;
use support::linear::timed;

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&base_markdown::Options::gfm().compile, &mut output)
        .unwrap();
    output
}

/// The first node of `document` for which `find` holds.
fn find(document: &Document, find: impl Fn(&EventKind<'_>) -> bool) -> NodeId {
    document
        .descendants(document.root())
        .find(|&id| find(&document.node(id).unwrap().kind))
        .unwrap()
}

/// The definition of `label`, as the parser reads its label.
fn definition(document: &Document, label: &str) -> NodeId {
    document.definition(label).unwrap()
}

/// Normalize `document`, write it, and check that it reads back as the same tree and renders the
/// same, and return what was written.
fn round_trip(mut document: Document) -> String {
    document.normalize();
    assert!(document.is_normalized());
    let written = to_markdown(&document).unwrap();
    let reparsed = parse(&written);
    assert_eq!(canonical(&reparsed), canonical(&document), "{:?}", written);
    assert_eq!(html(&reparsed), html(&document), "{:?}", written);
    assert_eq!(to_html(&written), html(&document), "{:?}", written);
    written
}

const SOURCE: &str =
    "[a] [b][x] ![c][] [^n]\n\n[a]: /1\n\n[x]: /2\n\n[c]: /3\n\n[A]: /4\n\n[^n]: note\n";

/// Each edit to the definitions changes where the references lead, and normalizing makes each
/// reference that no definition binds any longer the text it reads back as, so that the
/// document is written as a tree that reads back as itself.
#[test]
fn references_follow_edits_to_definitions() {
    // Nothing edited: everything is bound, and normalizing drops the copy of `a`.
    let document = parse(SOURCE);
    assert!(!document.is_normalized());
    assert_eq!(round_trip(document), SOURCE.replace("[A]: /4\n\n", ""));

    // Removing a definition unbinds its references.
    let mut document = parse(SOURCE);
    document.detach(definition(&document, "x")).unwrap();
    assert!(!document.is_normalized());
    let written = round_trip(document);
    assert!(
        written.starts_with("[a] \\[b]\\[x] ![c][] [^n]\n"),
        "{:?}",
        written
    );

    // Removing the first definition of a label binds the next one.
    let mut document = parse(SOURCE);
    document.detach(definition(&document, "a")).unwrap();
    assert!(document.is_normalized());
    assert!(html(&document).contains("<a href=\"/4\">a</a>"));
    let written = round_trip(document);
    assert!(written.starts_with("[a] [b][x]"), "{:?}", written);

    // Relabelling a definition unbinds the references to its old label and binds those to its new
    // one, which no other definition may have, as an edit adds no copy.
    let mut document = parse(SOURCE);
    let id = definition(&document, "c");
    let relabelled = |label| EventKind::Definition {
        label: Text::Literal(label),
        destination: Text::Literal("/5"),
        title: None,
    };
    let before = to_markdown(&document).unwrap();
    assert!(matches!(
        document.set_kind(id, relabelled("X")),
        Err(StreamError::InvalidValue)
    ));
    assert_eq!(to_markdown(&document).unwrap(), before);
    document.set_kind(id, relabelled("q")).unwrap();
    assert_eq!(document.definition("Q"), Some(id));
    assert!(!document.is_normalized());
    assert!(html(&document).contains("<a href=\"/2\">b</a>"));
    let written = round_trip(document);
    assert!(
        written.starts_with("[a] [b][x] !\\[c]\\[] [^n]\n"),
        "{:?}",
        written
    );

    // Moving a definition of a label defined more than once would reorder the binding, so it is
    // rejected, and once normalizing drops the copy, the one left moves.
    let mut document = parse(SOURCE);
    let first = document.children(document.root()).next().unwrap();
    let before = to_markdown(&document).unwrap();
    for id in [definition(&document, "a"), {
        let ids: Vec<_> = document.descendants(document.root()).collect();
        ids.into_iter()
            .rev()
            .find(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. }
                )
            })
            .unwrap()
    }] {
        assert!(matches!(
            document.attach(document.root(), id, Some(first)),
            Err(StreamError::InvalidValue)
        ));
    }
    assert_eq!(to_markdown(&document).unwrap(), before);
    document.normalize();
    let upper = document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Definition { .. }
            )
        })
        .last()
        .unwrap();
    let first = document.children(document.root()).next().unwrap();
    document
        .attach(document.root(), upper, Some(first))
        .unwrap();
    assert!(html(&document).contains("<img src=\"/3\" alt=\"c\" />"));
    let written = round_trip(document);
    assert!(
        written.starts_with("[c]: /3\n\n[a] [b][x]"),
        "{:?}",
        written
    );

    // Adding a definition binds a reference by name that was unbound, and one of a label already
    // bound is rejected, as an edit adds no copy.
    let mut document = parse(SOURCE);
    let root = document.root();
    let added = EventKind::Definition {
        label: Text::Literal("C"),
        destination: Text::Literal("/6"),
        title: None,
    };
    let before = to_markdown(&document).unwrap();
    assert!(matches!(
        document.append_child(root, added.clone()),
        Err(StreamError::InvalidValue)
    ));
    assert_eq!(to_markdown(&document).unwrap(), before);
    document.detach(definition(&document, "c")).unwrap();
    assert!(!document.is_normalized());
    document.append_child(root, added).unwrap();
    assert!(html(&document).contains("<img src=\"/6\" alt=\"c\" />"));
    let written = round_trip(document);
    assert!(written.ends_with("[C]: /6\n"), "{:?}", written);

    // Removing a footnote definition unbinds its calls.
    let mut document = parse(SOURCE);
    let note = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    });
    document.detach(note).unwrap();
    assert!(!document.is_normalized());
    let written = round_trip(document);
    assert!(
        written.starts_with("[a] [b][x] ![c][] \\[^n]\n"),
        "{:?}",
        written
    );
}

/// Spec example 564, `*[foo*]`, reads as a link only with a definition of `foo*`, so its text is
/// written so that it reads back as itself after a definition is added, and the link as text
/// after its definition is removed.
#[test]
fn bracketed_text_reads_back_as_itself_whatever_the_definitions() {
    // A definition added: the text stays text, as its brackets are escaped.
    let mut document = parse("*[foo*]\n");
    let root = document.root();
    document
        .append_child(
            root,
            EventKind::Definition {
                label: Text::Literal("foo*"),
                destination: Text::Literal("/url"),
                title: None,
            },
        )
        .unwrap();
    assert!(document.is_normalized());
    assert_eq!(round_trip(document), "*\\[foo*]\n\n[foo*]: /url\n");

    // The only definition removed: the link becomes its text.
    let mut document = parse("*[foo*]\n\n[foo*]: /url\n");
    assert!(html(&document).contains("<a href=\"/url\">foo*</a>"));
    document.detach(definition(&document, "foo*")).unwrap();
    assert!(!document.is_normalized());
    assert_eq!(html(&document), "<p>*[foo*]</p>\n");
    assert_eq!(round_trip(document), "\\*\\[foo\\*]\n");
}

/// An unbound reference renders as the text it reads back as, with its label decoded, and
/// normalizing makes it that text.
#[test]
fn unbound_references_render_as_their_text() {
    for (kind, label, expected) in [
        (ReferenceKind::Shortcut, "*b*", "<p>[<em>b</em>]</p>\n"),
        (ReferenceKind::Collapsed, "*b*", "<p>![<em>b</em>][]</p>\n"),
        (
            ReferenceKind::Full,
            "x\\*  \n  &amp;y",
            "<p>[<em>b</em>][x* &amp;y]</p>\n",
        ),
    ] {
        let mut document = parse("*b*\n");
        let paragraph = document.children(document.root()).next().unwrap();
        let emphasis = document.children(paragraph).next().unwrap();
        let target = Target::Reference(Box::new(Reference {
            kind,
            label: Text::Literal(label),
        }));
        let tag = if kind == ReferenceKind::Collapsed {
            Tag::Image(target)
        } else {
            Tag::Link(target)
        };
        let link = document
            .append_child(paragraph, EventKind::Start(tag))
            .unwrap();
        document.attach(link, emphasis, None).unwrap();
        assert_eq!(document.destination(link), None);
        assert_eq!(html(&document), expected);
        let mut normalized = document.clone();
        normalized.normalize();
        assert_eq!(html(&normalized), expected);
        round_trip(document);
    }
}

/// The lookups agree with the parser: labels match as the parser matches them, the first
/// definition wins, and a detached subtree binds its own references.
#[test]
fn lookups_find_the_first_definition_of_a_label() {
    let document = parse(
        "[a][Foo  Bar] [^n]\n\n> [FOO bar]: /1 \"t\"\n\n[foo bar]: /2\n\n[^n]: x\n\n[^n]: y\n",
    );
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let first = document.definition("foo\tbar").unwrap();
    assert!(matches!(
        document.node(first).unwrap().kind,
        EventKind::Definition { destination: Text::Source(ref range), .. } if &document.source()[range.clone()] == "/1"
    ));
    assert_eq!(document.destination(link), Some(("/1", Some("t"))));
    let bindings = document.bindings();
    assert_eq!(bindings.definition("FOO BAR"), Some(first));
    assert_eq!(bindings.destination(link), Some(("/1", Some("t"))));
    assert_eq!(bindings.definition("other"), None);
    let note = bindings.footnote_definition("N").unwrap();
    assert_eq!(document.footnote_definition("N"), Some(note));
    assert_eq!(document.text_content(note), "x");
    assert_eq!(bindings.destination(note), None);

    // A detached subtree is not part of the document, so it binds only its own references.
    let mut document = parse("> [a]\n>\n> [a]: /inner\n\n[a]: /outer\n");
    let quote = document.children(document.root()).next().unwrap();
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(document.destination(link), Some(("/inner", None)));
    document.detach(quote).unwrap();
    assert_eq!(document.destination(link), Some(("/inner", None)));
    assert_eq!(
        document.destination(definition(&document, "a")),
        None,
        "a definition is not a link"
    );
    let mut rendered = String::new();
    document
        .render_node(quote, &Default::default(), &mut rendered)
        .unwrap();
    assert!(rendered.contains("href=\"/inner\""), "{}", rendered);
    // Normalizing keeps it, as it is bound in its own tree.
    document.normalize();
    assert!(document.is_normalized());
    assert_eq!(document.destination(link), Some(("/inner", None)));
}

/// A parsed document's events are the parser's: each bound reference is given resolved, with
/// its definition's destination and title. A builder keeps only the name of one bound to the
/// same target, and a document's edits store a resolved reference as a resource, as its
/// destination would not follow its definitions.
#[test]
fn events_resolve_and_documents_store_names() {
    let source = "[a][x] ![b][x] [c]\n\n[x]: /u \"t\"\n";
    let document = parse(source);
    let events: Vec<Event<'_>> = document.events().map(Result::unwrap).collect();
    let parsed: Vec<Event<'_>> = EventParser::new(source, &ParseOptions::gfm())
        .map(Result::unwrap)
        .collect();
    let kinds = |events: &[Event<'_>]| {
        events
            .iter()
            .map(|event| format!("{:?}", event.kind.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds(&events), kinds(&parsed));
    let resolved = events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::Start(Tag::Link(Target::Resolved(resolved))) => Some(resolved.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(resolved.destination.resolve(source).unwrap(), "/u");
    for node in document.nodes() {
        assert!(!matches!(
            node.kind,
            EventKind::Start(Tag::Link(Target::Resolved(_)) | Tag::Image(Target::Resolved(_)))
        ));
    }
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut parsed.into_iter().map(Ok), usize::MAX)
        .unwrap();
    let built = builder.finish(source).unwrap();
    assert_eq!(html(&built), html(&document));

    let mut document = parse(source);
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let paragraph = document.node(link).unwrap().parent().unwrap();
    let resolved = EventKind::Start(Tag::Link(Target::Resolved(Box::new(Resolved {
        reference: Reference {
            kind: ReferenceKind::Full,
            label: Text::Literal("x"),
        },
        destination: Text::Literal("/v"),
        title: None,
    }))));
    // A resolved reference is stored as a resource to where it leads, whatever the document's
    // definitions of its label.
    let resource = EventKind::Start(Tag::Link(Target::Resource {
        destination: Text::Literal("/v"),
        title: None,
    }));
    document.set_kind(link, resolved.clone()).unwrap();
    assert_eq!(document.node(link).unwrap().kind, resource);
    let appended = document.append_child(paragraph, resolved).unwrap();
    assert_eq!(document.node(appended).unwrap().kind, resource);
    assert_eq!(document.destination(appended), Some(("/v", None)));
}

/// A subtree is written with references only to the definitions it holds that are the bindings
/// of its references in the document; any other bound reference is written as a resource to
/// where it leads, so the subtree reads back leading where it did.
#[test]
fn subtrees_write_references_bound_outside_as_resources() {
    let document = parse("> [a] [b]\n>\n> [b]: /inner\n>\n> [a]: /shadowed\n\n[a]: /outer\n");
    let quote = document.children(document.root()).next().unwrap();
    // `[a]: /outer` comes later, so `[a]: /shadowed` is the first `a` in document order.
    assert_eq!(
        to_markdown_node(&document, quote, &Default::default()).unwrap(),
        "> [a] [b]\n>\n> [b]: /inner\n>\n> [a]: /shadowed\n"
    );
    let document = parse("[a]: /outer\n\n> [a] [b]\n>\n> [b]: /inner\n>\n> [a]: /shadowed\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    assert_eq!(
        to_markdown_node(&document, quote, &Default::default()).unwrap(),
        "> [a](/outer) [b]\n>\n> [b]: /inner\n>\n> [a]: /shadowed\n"
    );
}

/// The edges of binding: footnote definitions with an edited identifier, labels that end with a
/// line ending, lookups of nodes that are not references, a renderer whose output fails, an
/// unbound shortcut whose text no longer fits its label, and a titled definition written outside
/// the subtree that uses it.
#[test]
fn binding_edges() {
    // A footnote definition cannot take an identifier another one has, as an edit adds no copy,
    // and once that one is gone it binds the calls of the identifier.
    let mut document = parse("[^n] [^m]\n\n[^n]: note\n\n[^m]: other\n");
    let note = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    });
    let renamed = EventKind::Start(Tag::FootnoteDefinition {
        identifier: Text::Value("M".into()),
        label: Some(Text::Value("m".into())),
    });
    let before = to_markdown(&document).unwrap();
    assert!(matches!(
        document.set_kind(note, renamed.clone()),
        Err(StreamError::InvalidValue)
    ));
    assert_eq!(to_markdown(&document).unwrap(), before);
    let other = document.footnote_definition("M").unwrap();
    document.detach(other).unwrap();
    document.set_kind(note, renamed).unwrap();
    assert_eq!(document.footnote_definition("M"), Some(note));
    assert!(!document.is_normalized());
    let written = round_trip(document);
    assert!(written.starts_with("\\[^n] [^m]\n"), "{:?}", written);

    // A label that ends with a line ending reads back with a space for it.
    let mut document = parse("[a][b\n]\n\n[b\n]: /u\n");
    document.detach(definition(&document, "b")).unwrap();
    assert_eq!(html(&document), "<p>[a][b ]</p>\n");
    let written = round_trip(document);
    assert_eq!(written, "\\[a]\\[b ]\n");

    // A resource leads where it says; a node that is not a link, or not in the document, leads
    // nowhere.
    let document = parse("[a](/r \"t\") b\n");
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(document.destination(link), Some(("/r", Some("t"))));
    assert_eq!(
        document.bindings().destination(link),
        Some(("/r", Some("t")))
    );
    let paragraph = document.children(document.root()).next().unwrap();
    assert_eq!(document.destination(paragraph), None);
    let larger = parse(&"a\n\n".repeat(16));
    let outside = larger.descendants(larger.root()).last().unwrap();
    assert_eq!(document.destination(outside), None);
    assert_eq!(document.bindings().destination(outside), None);

    // An unbound reference's text that fails to be written fails the render.
    struct Failing;
    impl std::fmt::Write for Failing {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            if value.contains('[') {
                Err(std::fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut document = parse("[a]\n\n[a]: /u\n");
    document.detach(definition(&document, "a")).unwrap();
    assert!(document
        .render_html(&base_markdown::Options::gfm().compile, &mut Failing)
        .is_err());

    // An unbound shortcut whose text is no longer its label is written as a full reference, which
    // reads back as the same text.
    let mut document = parse("[foo]\n\n[foo]: /u\n");
    document.detach(definition(&document, "foo")).unwrap();
    let text = find(&document, |kind| matches!(kind, EventKind::Text(_)));
    document
        .set_kind(text, EventKind::Text(Text::Literal("bar")))
        .unwrap();
    assert_eq!(html(&document), "<p>[bar]</p>\n");
    assert_eq!(to_markdown(&document).unwrap(), "[bar][foo]\n");

    // One whose label is too long for a full reference to read back is written as one all the
    // same, as text reads back as text. Its label is then part of that text, which the document
    // does not render, so a document is normalized before it is written to read back as itself.
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let label = "a".repeat(1000);
    document
        .set_kind(
            link,
            EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                kind: ReferenceKind::Shortcut,
                label: Text::Value(label.clone().into()),
            })))),
        )
        .unwrap();
    let written = to_markdown(&document).unwrap();
    assert_eq!(written, format!("[bar][{}]\n", label));
    assert_eq!(to_html(&written), format!("<p>[bar][{}]</p>\n", label));
    assert_eq!(html(&document), "<p>[bar]</p>\n");
    assert_eq!(round_trip(document), "\\[bar]\n");

    // A reference bound outside the written subtree is written as a resource with its title.
    let document = parse("[a]: /outer \"T\"\n\n> [a]\n");
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    assert_eq!(
        to_markdown_node(&document, quote, &Default::default()).unwrap(),
        "> [a](/outer \"T\")\n"
    );
}

/// How much more the time of `run` grows over `SCALE` times the input than the control's does.
/// Building the large input of each pair the same way as its small one, timed in the same run,
/// means a slower machine or build slows both alike.
fn growth_against_control(
    label: &str,
    make: impl Fn(usize, bool) -> Document,
    run: impl Fn(&Document),
) {
    growth_against_control_from(1 << 8, label, make, run);
}

/// [`growth_against_control`] from `small` items, for work whose quadratic part shows only at
/// larger sizes.
fn growth_against_control_from(
    small: usize,
    label: &str,
    make: impl Fn(usize, bool) -> Document,
    run: impl Fn(&Document),
) {
    let _timing = support::linear::TIMING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let large = small * support::linear::SCALE;
    let documents = [
        make(small, false),
        make(large, false),
        make(small, true),
        make(large, true),
    ];
    let mut ratios = Vec::new();
    for _ in 0..3 {
        let mut times = [f64::MAX; 4];
        for _ in 0..5 {
            for (time, document) in times.iter_mut().zip(&documents) {
                *time = time.min(timed(|| run(document)).as_secs_f64());
            }
        }
        let ratio = (times[1] / times[0]) / (times[3] / times[2]);
        eprintln!("linear {label}: {times:?}, {ratio:.2}x the control's growth");
        if ratio < 2.5 {
            return;
        }
        ratios.push(ratio);
    }
    panic!("{}: grew {:?} times the control's growth", label, ratios);
}

/// Resolving, rendering, writing and normalizing references takes time linear in the document,
/// up to the logarithm of the labels a lookup takes, however many references and definitions it
/// has: each is
/// timed against a control of as many resources, whose growth the references' would exceed
/// `SCALE` times if a lookup scanned the document.
#[test]
fn reference_work_is_linear() {
    // `count` references to as many definitions, or as many resources, in a block quote, with a
    // footnote.
    let bound = |count: usize, control: bool| {
        let mut source = String::from("> ");
        for index in 0..count {
            if control {
                source.push_str(&format!("[l](/{index}) "));
            } else {
                source.push_str(&format!("[l][{index}] "));
            }
        }
        source.push_str("[^n]\n\n");
        for index in 0..count {
            source.push_str(&format!("[{index}]: /{index}\n"));
        }
        source.push_str("\n[^n]: x\n");
        parse(&source)
    };
    growth_against_control("render", bound, |document| {
        html(document);
    });
    growth_against_control_from(1 << 7, "write", bound, |document| {
        to_markdown(document).unwrap();
    });
    growth_against_control_from(1 << 7, "write a subtree", bound, |document| {
        let quote = document.children(document.root()).next().unwrap();
        to_markdown_node(document, quote, &Default::default()).unwrap();
    });
    growth_against_control("bindings", bound, |document| {
        let bindings = document.bindings();
        for id in document.descendants(document.root()) {
            bindings.destination(id);
        }
    });
    growth_against_control("is_normalized", bound, |document| {
        assert!(document.is_normalized());
    });
    // With the definitions removed, normalizing makes each reference text.
    let unbound = |count: usize, control: bool| {
        let mut document = bound(count, control);
        let definitions: Vec<_> = document
            .children(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. }
                )
            })
            .collect();
        for id in definitions {
            document.detach(id).unwrap();
        }
        document
    };
    growth_against_control("normalize", unbound, |document| {
        let mut document = document.clone();
        document.normalize();
    });
    // `count` references, or as many resources, in a block quote nested `count` deep, bound
    // outside it: writing the deepest quote finds the tree it is in once, not once a reference.
    let deep = |count: usize, control: bool| {
        let mut source = "> ".repeat(count);
        for _ in 0..count {
            source.push_str(if control { "[a](/u) " } else { "[a] " });
        }
        source.push_str("\n\n[a]: /u\n");
        parse(&source)
    };
    growth_against_control_from(1 << 7, "write a deep subtree", deep, |document| {
        let quote = document
            .descendants(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Start(Tag::BlockQuote)
                )
            })
            .last()
            .unwrap();
        to_markdown_node(document, quote, &Default::default()).unwrap();
    });
}

/// The per-subtree operations look a reference up in the document's index of its definitions,
/// so calling one on each section of a document takes time linear in the document: each is
/// timed against a control of as many resources.
#[test]
fn per_section_work_is_linear() {
    // `count` sections, each a heading that holds a reference, or a resource, to its own
    // definition and another to `d`, whose definition comes before the sections and, with
    // `copies`, has copies spread among them, halfway and after them, with the sections'
    // definitions after them. A binding is found at once, wherever its copies are.
    let sections = |copies: bool| {
        move |count: usize, control: bool| {
            let mut source = String::from("[d]: /d\n\n");
            for index in 0..count {
                if control {
                    source.push_str(&format!("# h [a{index}](/u) [d](/d)\n\n"));
                } else {
                    source.push_str(&format!("# h [a{index}] [d]\n\n"));
                }
                if copies && index == count / 2 {
                    source.push_str("[d]: /2\n\n");
                }
            }
            if copies {
                source.push_str("[d]: /3\n\n");
            }
            for index in 0..count {
                source.push_str(&format!("[a{index}]: /u\n"));
            }
            parse(&source)
        }
    };
    let each = |document: &Document, run: &dyn Fn(NodeId)| {
        for id in document.children(document.root()) {
            run(id);
        }
    };
    {
        let make = sections(true);
        growth_against_control("text_content", make, |document| {
            each(document, &|id| {
                document.text_content(id);
            });
        });
        growth_against_control("render_node", make, |document| {
            each(document, &|id| {
                document
                    .render_node(id, &Default::default(), &mut String::new())
                    .unwrap();
            });
        });
        growth_against_control("to_markdown_node", make, |document| {
            each(document, &|id| {
                to_markdown_node(document, id, &Default::default()).unwrap();
            });
        });
        growth_against_control("subtree_events", make, |document| {
            each(document, &|id| {
                document.subtree_events(id).for_each(drop);
            });
        });
        growth_against_control("headings", make, |document| {
            drop(document.headings(&Default::default()));
        });
    }
}

/// A label defined twice, with the copies far apart from the first definitions, costs a
/// lookup no more than one defined once, in every pass over the whole document: `count`
/// references, then a definition of each, then a copy of each, against a control whose second
/// definitions have labels of their own.
#[test]
fn whole_document_work_with_copies_far_apart_is_linear() {
    let source = |count: usize, control: bool| {
        let mut source = String::new();
        for index in 0..count {
            source.push_str(&format!("[a{index}] "));
        }
        source.push_str("\n\n");
        for index in 0..count {
            source.push_str(&format!("[a{index}]: /1\n\n"));
        }
        let second = if control { "b" } else { "a" };
        for index in 0..count {
            source.push_str(&format!("[{second}{index}]: /2\n\n"));
        }
        source
    };
    let make = |count, control| parse(&source(count, control));
    growth_against_control_from(1 << 9, "parse", make, |document| {
        drop(parse(document.source()));
    });
    growth_against_control_from(1 << 9, "render_html", make, |document| {
        drop(html(document));
    });
    growth_against_control_from(1 << 7, "to_markdown", make, |document| {
        to_markdown(document).unwrap();
    });
    growth_against_control_from(1 << 9, "headings", make, |document| {
        drop(document.headings(&Default::default()));
    });
    growth_against_control("normalize", make, |document| {
        document.clone().normalize();
    });
}

/// The events of `source`, from the parser, with `edit` applied to each.
fn parsed_events(source: &str, edit: impl Fn(&mut EventKind<'_>)) -> Vec<Event<'_>> {
    EventParser::new(source, &ParseOptions::gfm())
        .map(Result::unwrap)
        .map(|mut event| {
            edit(&mut event.kind);
            event
        })
        .collect()
}

/// A document built from `events` of `source`.
fn build<'s>(events: Vec<Event<'s>>, source: &'s str) -> Document<&'s str> {
    let mut builder = AstBuilder::new();
    builder
        .consume(&mut events.into_iter().map(Ok), usize::MAX)
        .unwrap();
    builder.finish(source).unwrap()
}

fn rewrite(destination: &mut Text<'_>) {
    *destination = Text::Literal("/rewritten");
}

/// A resolved reference never loses its meaning: a builder keeps its name only when it binds to
/// a definition with the destination and title it was resolved to in the built document, and
/// otherwise stores it as a resource to them, whatever edited its events or left its definition
/// out.
#[test]
fn built_references_keep_their_targets() {
    let source = "[a][x] [b](/r)\n\n[x]: /u\n";
    let link = |document: &Document<&str>| {
        document
            .descendants(document.root())
            .find(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Start(Tag::Link(_))
                )
            })
            .unwrap()
    };
    // Each link's destination rewritten, but not the definition's: both lead to the rewritten
    // destination, so the reference is stored as a resource.
    let built = build(
        parsed_events(source, |kind| match kind {
            EventKind::Start(Tag::Link(Target::Resolved(resolved))) => {
                rewrite(&mut resolved.destination);
            }
            EventKind::Start(Tag::Link(Target::Resource { destination, .. })) => {
                rewrite(destination);
            }
            _ => {}
        }),
        source,
    );
    assert_eq!(
        html(&built),
        "<p><a href=\"/rewritten\">a</a> <a href=\"/rewritten\">b</a></p>\n"
    );
    assert_eq!(
        built.node(link(&built)).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Resource {
            destination: Text::Literal("/rewritten"),
            title: None,
        }))
    );
    // The definition rewritten too: the reference binds to it, so it keeps its name.
    let built = build(
        parsed_events(source, |kind| match kind {
            EventKind::Start(Tag::Link(Target::Resolved(resolved))) => {
                rewrite(&mut resolved.destination);
            }
            EventKind::Definition { destination, .. } => rewrite(destination),
            _ => {}
        }),
        source,
    );
    assert!(matches!(
        built.node(link(&built)).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(_)))
    ));
    assert_eq!(built.destination(link(&built)), Some(("/rewritten", None)));
    // A different title, or no definition, leaves a resource to where it led.
    for edit in [
        (|kind: &mut EventKind<'_>| {
            if let EventKind::Definition { title, .. } = kind {
                *title = Some(Text::Literal("t"));
            }
        }) as fn(&mut EventKind<'_>),
        |kind| {
            if let EventKind::Definition { label, .. } = kind {
                *label = Text::Literal("y");
            }
        },
    ] {
        let built = build(parsed_events(source, edit), source);
        assert!(matches!(
            built.node(link(&built)).unwrap().kind,
            EventKind::Start(Tag::Link(Target::Resource { .. }))
        ));
        assert_eq!(built.destination(link(&built)), Some(("/u", None)));
    }
    let events = parsed_events(source, |_| {})
        .into_iter()
        .filter(|event| !matches!(event.kind, EventKind::Definition { .. }))
        .collect();
    let built = build(events, source);
    assert_eq!(built.destination(link(&built)), Some(("/u", None)));

    // The events of a subtree bound to a definition outside it build a subtree that leads where
    // it did, and render as it does in the document.
    let source = "> [a] ![b][a]\n\n[a]: /u \"t\"\n";
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    let quote = document.children(document.root()).next().unwrap();
    let mut rendered = String::new();
    document
        .render_node(quote, &base_markdown::Options::gfm().compile, &mut rendered)
        .unwrap();
    assert_eq!(
        rendered,
        "<blockquote>\n<p><a href=\"/u\" title=\"t\">a</a> <img src=\"/u\" alt=\"b\" title=\"t\" /></p>\n</blockquote>"
    );
    let mut events = vec![Event {
        kind: EventKind::Start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        }),
        span: 0..0,
    }];
    events.extend(document.subtree_events(quote).map(Result::unwrap));
    events.push(Event {
        kind: EventKind::End(base_markdown::stream::TagKind::Document),
        span: source.len()..source.len(),
    });
    let built = build(events, source);
    assert_eq!(html(&built).trim_end(), rendered.trim_end());
}

/// The bindings of the attached document cover it alone: a detached subtree's references are
/// bound by its own definitions, as it renders, which [`Document::destination`] gives.
#[test]
fn detached_references_are_bound_in_their_own_tree() {
    let mut document = parse("> [a]\n>\n> [a]: /inner\n\n[a]: /outer\n");
    let quote = document.children(document.root()).next().unwrap();
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    assert_eq!(
        document.bindings().destination(link),
        Some(("/inner", None))
    );
    document.detach(quote).unwrap();
    assert_eq!(document.destination(link), Some(("/inner", None)));
    assert_eq!(document.bindings().destination(link), None);
    let mut rendered = String::new();
    document
        .render_node(quote, &base_markdown::Options::gfm().compile, &mut rendered)
        .unwrap();
    assert!(rendered.contains("href=\"/inner\""), "{}", rendered);
}

/// A reference or footnote call that nothing binds renders, reads and gives its heading the
/// text it reads back as, as it is once normalized.
#[test]
fn unbound_references_are_text_everywhere() {
    let mut document = parse("# [b] and ![c][b] x[^n]\n\n[b]: /u\n\n[^n]: note\n");
    document.detach(definition(&document, "b")).unwrap();
    let footnote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    });
    document.detach(footnote).unwrap();
    let heading = document.children(document.root()).next().unwrap();
    let expected = "[b] and ![c][b] x[^n]";
    assert_eq!(document.text_content(heading), expected);
    let headings = document.headings(&Default::default());
    assert_eq!(headings[0].text, expected);
    let mut normalized = document.clone();
    normalized.normalize();
    let written = to_markdown(&normalized).unwrap();
    let reparsed = parse(&written);
    let mut read = reparsed.headings(&Default::default());
    read[0].node = headings[0].node;
    assert_eq!(read, headings);
    // It renders as text, with the heading's id, and no footnote section.
    assert_eq!(html(&document), html(&normalized));
    assert_eq!(html(&document), to_html(&written));
    assert!(!html(&document).contains("footnote"), "{}", html(&document));
}

/// A footnote call whose definition is detached renders as the text it reads back as, without
/// an error or a footnote section.
#[test]
fn unbound_footnote_calls_render_as_text() {
    let mut document = parse("x [^n]\n\n[^n]: x\n");
    let footnote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    });
    document.detach(footnote).unwrap();
    assert_eq!(html(&document), "<p>x [^n]</p>\n");
    let call = find(&document, |kind| {
        matches!(kind, EventKind::FootnoteReference { .. })
    });
    let mut rendered = String::new();
    document
        .render_node(call, &base_markdown::Options::gfm().compile, &mut rendered)
        .unwrap();
    assert_eq!(rendered, "[^n]");
}

/// Normalizing leaves the root of a detached subtree as it is, even a reference nothing in its
/// tree binds, and forgets the syntax of a reference it makes text.
#[test]
fn normalizing_detached_references() {
    let mut document = parse("x [a]\n\n[a]: /u\n");
    assert!(document.is_normalized());
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    document.detach(link).unwrap();
    assert!(document.is_normalized());
    document.normalize();
    assert!(matches!(
        document.node(link).unwrap().kind,
        EventKind::Start(Tag::Link(Target::Reference(_)))
    ));

    let mut document = parse("x [a]\n\n[a]: /u\n");
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    document
        .set_syntax(
            link,
            Some(base_markdown::NodeSyntax::Link {
                form: base_markdown::LinkForm::Explicit,
            }),
        )
        .unwrap();
    document.detach(definition(&document, "a")).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(matches!(
        document.node(link).unwrap().kind,
        EventKind::Text(_)
    ));
    assert_eq!(document.syntax(link), None);
}

/// A stream that carries a reference by name, which no definition binds in it, renders it as
/// the text it reads back as.
#[test]
fn streamed_references_by_name_are_rejected() {
    use base_markdown::stream::TagKind;
    // A stream carries resolved targets only, as a reference by name leads nowhere without the
    // definitions of a document.
    let event = |kind| Event { kind, span: 0..0 };
    for image in [false, true] {
        let target = Target::Reference(Box::new(Reference {
            kind: ReferenceKind::Full,
            label: Text::Literal("x"),
        }));
        let (start, end) = if image {
            (Tag::Image(target), TagKind::Image)
        } else {
            (Tag::Link(target), TagKind::Link)
        };
        let events = vec![
            event(EventKind::Start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            })),
            event(EventKind::Start(Tag::Paragraph { tight: false })),
            event(EventKind::Start(start)),
            event(EventKind::Text(Text::Literal("a"))),
            event(EventKind::End(end)),
            event(EventKind::End(TagKind::Paragraph)),
            event(EventKind::End(TagKind::Document)),
        ];
        let mut output = String::new();
        assert!(matches!(
            base_markdown::render_events(
                "",
                events.into_iter().map(Ok),
                &base_markdown::Options::gfm().compile,
                &mut output,
            ),
            Err(StreamError::InvalidValue)
        ));
    }
}

/// A document keeps its index of definitions as it is edited: after each edit of a random
/// sequence, every lookup finds what a walk of the attached document finds.
#[test]
fn lookups_follow_every_edit() {
    let labels = ["a", "A", "a  b", "A\nB", "b", "ä", "Ä", "c"];
    let identifiers = ["N", "M", "O"];
    let source = "# s\n\n[a]: /1\n> [A]: /2\n> - [b]: /3\n>\n> [^N]: x [c] [^M]\n\n# t [b]\n\n[a  b]: /4\n[^m]: y [ä]\n[ä]: /5\n\n# u\n\n[a][] [^n] [x][c]\n\n[c]: /c\n";
    let replacement = parse("[c]: /6\n[A]: /7\n\n> [^o]: z\n> [b]: /8\n");
    let check = |document: &Document| {
        let bindings = document.bindings();
        for label in labels {
            assert_eq!(document.definition(label), bindings.definition(label));
        }
        for identifier in identifiers {
            assert_eq!(
                document.footnote_definition(identifier),
                bindings.footnote_definition(identifier)
            );
        }
    };
    let attached = |document: &Document, mut id: NodeId| {
        while let Some(parent) = document.node(id).unwrap().parent() {
            id = parent;
        }
        id == document.root()
    };
    for seed in 0..64_u64 {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            std::convert::TryFrom::try_from(state % bound as u64).unwrap()
        };
        let mut document = parse(source);
        check(&document);
        // Every node met, attached or detached.
        let mut known: Vec<NodeId> = Vec::new();
        for _ in 0..64 {
            for id in document.descendants(document.root()) {
                if !known.contains(&id) {
                    known.push(id);
                }
            }
            let id = known[next(known.len())];
            let other = known[next(known.len())];
            match next(7) {
                0 => {
                    let label = labels[next(labels.len())];
                    let kind = match &document.node(id).unwrap().kind {
                        EventKind::Definition { .. } => EventKind::Definition {
                            label: Text::from(label.to_owned()),
                            destination: Text::Literal("/e"),
                            title: None,
                        },
                        EventKind::Start(Tag::FootnoteDefinition { .. }) => {
                            EventKind::Start(Tag::FootnoteDefinition {
                                identifier: Text::Literal(identifiers[next(identifiers.len())]),
                                label: None,
                            })
                        }
                        _ => continue,
                    };
                    let _ = document.set_kind(id, kind);
                }
                1 => {
                    let label = labels[next(labels.len())];
                    let identifier = Text::Literal(identifiers[next(identifiers.len())]);
                    let kind = match next(4) {
                        0 => EventKind::Definition {
                            label: Text::Literal(label),
                            destination: Text::Literal("/n"),
                            title: None,
                        },
                        1 => EventKind::Start(Tag::FootnoteDefinition {
                            identifier,
                            label: None,
                        }),
                        2 => EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                            kind: ReferenceKind::Full,
                            label: Text::Literal(label),
                        })))),
                        _ => EventKind::FootnoteReference {
                            identifier,
                            label: None,
                        },
                    };
                    if let Ok(id) = document.append_child(id, kind) {
                        known.push(id);
                    }
                }
                2 => {
                    let _ = document.detach(id);
                }
                3 => {
                    let _ = document.attach(id, other, None);
                }
                4 => {
                    let _ = document.extract_children(id);
                }
                5 => {
                    let _ = document.replace_heading_section(
                        id,
                        next(8),
                        &replacement,
                        next(2) == 0,
                        next(2) == 0,
                    );
                }
                _ => {
                    document.normalize();
                    // Normalizing leaves no reference unbound in its tree, other than a
                    // detached root, and no footnote call in the attached document.
                    let bindings = document.bindings();
                    for &id in &known {
                        let node = document.node(id).unwrap();
                        match &node.kind {
                            EventKind::Start(Tag::Link(Target::Reference(_)))
                            | EventKind::Start(Tag::Image(Target::Reference(_)))
                                if node.parent().is_some() =>
                            {
                                assert!(document.destination(id).is_some(), "seed {}", seed);
                            }
                            EventKind::FootnoteReference { identifier, .. }
                                if attached(&document, id) =>
                            {
                                let identifier = identifier.resolve(document.source()).unwrap();
                                assert!(bindings.footnote_definition(identifier).is_some());
                            }
                            _ => {}
                        }
                    }
                }
            }
            check(&document);
        }
    }
}

/// The text an unbound reference opens with is an event at its start, with an empty span, as
/// its children follow it.
#[test]
fn unbound_reference_text_starts_where_it_does() {
    let source = "a [b][x] c\n\n[x]: /u\n";
    let mut document = parse(source);
    document.detach(definition(&document, "x")).unwrap();
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    let start = document.node(link).unwrap().span.start;
    let events: Vec<_> = document.subtree_events(link).map(Result::unwrap).collect();
    assert!(
        matches!(&events[0].kind, EventKind::Text(text) if text.resolve(source).ok() == Some("["))
    );
    assert_eq!(events[0].span, start..start);
}

/// Of several definitions of a label, the first in document order binds it, whichever was
/// indexed first, also where one holds another, and edits of a detached tree leave the
/// document's lookups as they are.
#[test]
fn lookups_order_nested_and_detached_definitions() {
    let footnote = |identifier| {
        EventKind::Start(Tag::FootnoteDefinition {
            identifier: Text::Literal(identifier),
            label: None,
        })
    };
    // A definition nested in one of its own identifier would be a copy, which no edit adds, in
    // either order.
    let mut document = parse("[^n]\n\n[^n]: x\n");
    let outer = document.footnote_definition("N").unwrap();
    assert!(matches!(
        document.append_child(outer, footnote("N")),
        Err(StreamError::InvalidValue)
    ));
    document.set_kind(outer, footnote("Z")).unwrap();
    let inner = document.append_child(outer, footnote("N")).unwrap();
    assert_eq!(document.footnote_definition("N"), Some(inner));
    assert!(matches!(
        document.set_kind(outer, footnote("N")),
        Err(StreamError::InvalidValue)
    ));
    assert_eq!(document.footnote_definition("Z"), Some(outer));
    document.detach(inner).unwrap();
    document.set_kind(outer, footnote("N")).unwrap();
    assert_eq!(document.footnote_definition("N"), Some(outer));
    assert_eq!(document.footnote_definition("Z"), None);

    let mut document = parse("> # a\n>\n> [x]: /1\n\n[r][x]\n");
    let quote = document.children(document.root()).next().unwrap();
    document.detach(quote).unwrap();
    let replacement = parse("[x]: /2\n");
    assert!(document
        .replace_heading_section(quote, 0, &replacement, true, true)
        .unwrap());
    assert_eq!(document.definition("x"), None);
    assert_eq!(document.bindings().definition("x"), None);
    document.attach(document.root(), quote, None).unwrap();
    let x = document.definition("x").unwrap();
    assert_eq!(document.bindings().definition("x"), Some(x));
    assert!(matches!(
        &document.node(x).unwrap().kind,
        EventKind::Definition { destination, .. } if destination.resolve(document.source()).ok() == Some("/2")
    ));
}

/// Normalizing a normalized document looks nothing up, but any edit after it can unbind a
/// reference or add one that nothing binds, which the next normalizing then makes text.
#[test]
fn edits_after_normalizing_unbind() {
    let is_text = |document: &Document, id: NodeId| {
        matches!(document.node(id).unwrap().kind, EventKind::Text(_))
    };
    // A reference appended without a definition.
    let mut document = parse("a\n");
    document.normalize();
    let paragraph = document.children(document.root()).next().unwrap();
    let link = document
        .append_child(
            paragraph,
            EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                kind: ReferenceKind::Full,
                label: Text::Literal("x"),
            })))),
        )
        .unwrap();
    document
        .append_child(link, EventKind::Text(Text::Literal("t")))
        .unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(is_text(&document, link));
    assert!(document.is_normalized());
    // A definition moved into a detached tree.
    let mut document = parse("> q\n\n[x][y]\n\n[y]: /u\n");
    let quote = document.children(document.root()).next().unwrap();
    document.detach(quote).unwrap();
    document.normalize();
    let link = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::Link(_)))
    });
    document
        .attach(quote, definition(&document, "y"), None)
        .unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(is_text(&document, link));
}

/// A built document whose stream gave a reference by name, or a footnote call without its
/// definition, has them unbound, so normalizing makes them text.
#[test]
fn built_names_without_definitions_are_unbound() {
    use base_markdown::stream::TagKind;
    let event = |kind| Event { kind, span: 0..0 };
    for (name, end) in [
        (
            EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
                kind: ReferenceKind::Shortcut,
                label: Text::Literal("x"),
            })))),
            Some(TagKind::Link),
        ),
        (
            EventKind::FootnoteReference {
                identifier: Text::Literal("X"),
                label: None,
            },
            None,
        ),
    ] {
        let mut events = vec![
            event(EventKind::Start(Tag::Document {
                line_ending: None,
                frontmatter: None,
            })),
            event(EventKind::Start(Tag::Paragraph { tight: false })),
            event(name),
        ];
        if let Some(end) = end {
            events.push(event(EventKind::Text(Text::Literal("x"))));
            events.push(event(EventKind::End(end)));
        }
        events.push(event(EventKind::End(TagKind::Paragraph)));
        events.push(event(EventKind::End(TagKind::Document)));
        let mut document = build(events, "");
        assert!(!document.is_normalized());
        document.normalize();
        assert!(document.is_normalized());
        let paragraph = document.children(document.root()).next().unwrap();
        assert!(document
            .children(paragraph)
            .all(|id| matches!(document.node(id).unwrap().kind, EventKind::Text(_))));
    }
}

/// A label is looked up by its key, in which a run of spaces is one space.
#[test]
fn labels_with_runs_of_spaces_bind_by_their_key() {
    let document = parse("[x][a  b]\n\n[a b]: /u\n");
    let id = definition(&document, "a b");
    assert_eq!(document.definition("a  b"), Some(id));
    assert_eq!(document.bindings().definition("a  b"), Some(id));
    assert_eq!(html(&document), "<p><a href=\"/u\">x</a></p>\n");
    let document = parse("[x][a b]\n\n[a  b]: /u\n");
    assert_eq!(document.definition("a b"), document.definition("a  b"));
    assert_eq!(html(&document), "<p><a href=\"/u\">x</a></p>\n");
}

/// What a test compares to check that a document is unchanged.
fn state(document: &Document) -> (String, String, String, usize) {
    (
        format!("{:?}", canonical(document)),
        html(document),
        to_markdown(document).unwrap(),
        document.nodes().len(),
    )
}

/// Check that `edit` is rejected as adding a copy of a binding, and leaves `document` as it was.
fn adds_a_copy<T>(
    document: &mut Document,
    edit: impl FnOnce(&mut Document) -> Result<T, StreamError>,
) {
    let before = state(document);
    assert!(matches!(edit(document), Err(StreamError::InvalidValue)));
    assert_eq!(state(document), before);
}

fn link_definition(label: &'static str) -> EventKind<'static> {
    EventKind::Definition {
        label: Text::Literal(label),
        destination: Text::Literal("/new"),
        title: None,
    }
}

fn footnote_definition(identifier: &'static str) -> EventKind<'static> {
    EventKind::Start(Tag::FootnoteDefinition {
        identifier: Text::Literal(identifier),
        label: None,
    })
}

/// An edit that would give the attached document a second definition of a label or identifier
/// is rejected before it changes anything, by every edit that adds definitions.
#[test]
fn edits_add_no_copy_of_a_binding() {
    let source = "[a] [^n]\n\n[a]: /1\n\n[^n]: note\n\n> quote\n";
    let mut document = parse(source);
    let root = document.root();
    let note = document.footnote_definition("N").unwrap();
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    // Appending, at the root or nested in a footnote definition or block quote.
    adds_a_copy(&mut document, |document| {
        document.append_child(root, link_definition("A"))
    });
    adds_a_copy(&mut document, |document| {
        document.append_child(note, link_definition("a"))
    });
    adds_a_copy(&mut document, |document| {
        document.append_child(quote, link_definition("a"))
    });
    adds_a_copy(&mut document, |document| {
        document.append_child(root, footnote_definition("N"))
    });
    // Relabelling into a bound label, while keeping its own key is an edit like any other.
    let b = document.append_child(root, link_definition("b")).unwrap();
    adds_a_copy(&mut document, |document| {
        document.set_kind(b, link_definition("a"))
    });
    adds_a_copy(&mut document, |document| {
        document.set_kind(b, link_definition("A"))
    });
    let other = document
        .append_child(root, footnote_definition("O"))
        .unwrap();
    adds_a_copy(&mut document, |document| {
        document.set_kind(other, footnote_definition("N"))
    });
    document.set_kind(b, link_definition("B")).unwrap();
    assert_eq!(document.definition("b"), Some(b));
    // Attaching a detached subtree with a definition of a bound label, or with two of one label.
    let detached = document
        .append_child(root, EventKind::Start(Tag::BlockQuote))
        .unwrap();
    document.detach(detached).unwrap();
    let inner = document
        .append_child(detached, link_definition("a"))
        .unwrap();
    adds_a_copy(&mut document, |document| {
        document.attach(root, detached, None)
    });
    document.set_kind(inner, link_definition("c")).unwrap();
    document
        .append_child(detached, link_definition("C"))
        .unwrap();
    adds_a_copy(&mut document, |document| {
        document.attach(root, detached, None)
    });
    adds_a_copy(&mut document, |document| {
        document.attach(quote, detached, None)
    });
    // A detached subtree is a tree of its own, which binds its own references.
    document.extract_children(detached).unwrap();
    document
        .append_child(detached, link_definition("c"))
        .unwrap();
    document.attach(root, detached, None).unwrap();
    assert!(document.definition("c").is_some());
    // The only definition of a label, detached, can be attached again.
    document.detach(note).unwrap();
    assert!(html(&document).contains("[^n]"));
    document.attach(root, note, None).unwrap();
    assert_eq!(document.footnote_definition("N"), Some(note));
    assert!(html(&document).contains("data-footnote-ref"));

    // Moving a definition of a label defined more than once would reorder its copies.
    let mut document = parse("[a]\n\n[a]: /1\n\np\n\n[a]: /2\n");
    let root = document.root();
    let first = definition(&document, "a");
    let copy = document.children(root).last().unwrap();
    adds_a_copy(&mut document, |document| document.attach(root, first, None));
    adds_a_copy(&mut document, |document| {
        document.attach(root, copy, Some(first))
    });
    // Detaching a copy unbinds it, so attaching it again adds a copy, as attaching the binding
    // again would once the copy has taken its place.
    document.detach(copy).unwrap();
    adds_a_copy(&mut document, |document| document.attach(root, copy, None));
    document.attach(root, first, None).unwrap();
    assert_eq!(document.definition("a"), Some(first));

    // Replacing a section with one that defines a label bound outside it, or defines one twice;
    // a label that only the replaced section defines can be defined again.
    let mut document = parse("[a] [b]\n\n[a]: /1\n\n# s\n\n[b]: /2\n\n# t\n");
    let root = document.root();
    let index = document
        .children(root)
        .position(|id| document.text_content(id) == "s")
        .unwrap();
    for replacement in [
        "[a]: /3\n",
        "[c]: /3\n\n[C]: /4\n",
        "> [^x]: a\n>\n> [^x]: b\n",
    ] {
        let replacement = parse(replacement);
        adds_a_copy(&mut document, |document| {
            document.replace_heading_section(root, index, &replacement, true, false)
        });
    }
    let replacement = parse("# s\n\n[b]: /5\n");
    assert!(document
        .replace_heading_section(root, index, &replacement, true, false)
        .unwrap());
    assert!(html(&document).contains("<a href=\"/5\">b</a>"));
    // A section that holds a binding whose copy is outside it cannot define the label again,
    // as the copy stays.
    for source in [
        "[x]\n\n# s\n\n[x]: /1\n\n# t\n\n[x]: /2\n",
        "[x]\n\n[x]: /0\n\n# s\n\n[x]: /1\n",
    ] {
        let mut document = parse(source);
        let root = document.root();
        let index = document
            .children(root)
            .position(|id| document.text_content(id) == "s")
            .unwrap();
        let replacement = parse("# s\n\n[x]: /3\n");
        adds_a_copy(&mut document, |document| {
            document.replace_heading_section(root, index, &replacement, true, false)
        });
    }
}

/// Removing the binding of a label makes its next copy in document order the binding, and
/// removing a copy leaves the binding as it is.
#[test]
fn removing_a_binding_binds_its_next_copy() {
    let source = "[x]\n\n[x]: /1\n\n> [x]: /2\n\n- [x]: /3\n\n[x]: /4\n";
    let destination = |document: &Document| {
        let link = find(document, |kind| {
            matches!(kind, EventKind::Start(Tag::Link(_)))
        });
        assert_eq!(
            document.destination(link),
            document.bindings().destination(link)
        );
        document
            .destination(link)
            .map(|(destination, _)| destination.to_owned())
    };
    let definitions = |document: &Document| -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Definition { .. }
                )
            })
            .collect()
    };
    let mut document = parse(source);
    let ids = definitions(&document);
    assert_eq!(destination(&document).as_deref(), Some("/1"));
    // A copy goes, and the binding stays.
    document.detach(ids[2]).unwrap();
    assert_eq!(destination(&document).as_deref(), Some("/1"));
    // The binding goes, by a new label, and the next copy binds, then by detaching.
    document.set_kind(ids[0], link_definition("y")).unwrap();
    assert_eq!(destination(&document).as_deref(), Some("/2"));
    let quote = document.node(ids[1]).unwrap().parent().unwrap();
    document.extract_children(quote).unwrap();
    assert_eq!(destination(&document).as_deref(), Some("/4"));
    assert!(html(&document).contains("<a href=\"/4\">x</a>"));
    document.detach(ids[3]).unwrap();
    assert_eq!(destination(&document), None);

    // Replacing the section that holds the binding binds the copy after it.
    let mut document = parse("[x]\n\n# s\n\n[x]: /1\n\n# t\n\n[x]: /2\n");
    let root = document.root();
    let index = document
        .children(root)
        .position(|id| document.text_content(id) == "s")
        .unwrap();
    assert!(document
        .replace_heading_section(root, index, &parse("# s\n"), true, false)
        .unwrap());
    assert_eq!(destination(&document).as_deref(), Some("/2"));
}

/// Detaching a definition leaves the others bound.
#[test]
fn detaching_a_definition_keeps_the_others_bound() {
    let labels = ["k0", "k1"];
    let source = format!(
        "[{0}] [{1}]\n\n[{0}]: /0\n\n[{1}]: /1\n",
        labels[0], labels[1]
    );
    let mut document = parse(&source);
    let links: Vec<NodeId> = document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Link(_))
            )
        })
        .collect();
    let first = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Definition { .. }
            )
        })
        .unwrap();
    document.detach(first).unwrap();
    let destination = |link| {
        document
            .destination(link)
            .map(|(destination, _)| destination.to_owned())
    };
    assert_eq!(destination(links[0]), None);
    assert_eq!(destination(links[1]).as_deref(), Some("/1"));
}

/// Normalizing removes each copy of a binding, which renders nothing, so every label is defined
/// once, the document renders as it did, and normalizing again changes nothing.
#[test]
fn normalizing_removes_copies() {
    for source in [
        "[a]\n\n[a]: /1\n\n[a]: /2\n\n[A]: /3\n",
        "[a]\n\n[a]: /1\n[a]: /2\n\np\n",
        "[x]\n\n> [x]: /1\n\n[x]: /2\n\n> [x]: /3\n",
        "[x]\n\n[x]: /1\n\n- a\n\n  [x]: /2\n- b\n",
        "[^a]\n\n[^a]: one\n\n[^a]: two\n",
        "[^a]\n\n[^a]: x\n\n    [^a]: y\n",
        // The separator before a copy that ends a document without a line ending ends the
        // output, so it stays, while in a container, the container's end ends it.
        "[a]: /1\n[a]\n\n[a]: /2",
        "> [a]: /1\n> [a]\n>\n> [a]: /2\n",
        // A definition in a copy of a footnote definition can be the binding of its own label,
        // which stays where it is in document order.
        "[^a] [x] [y]\n\n[^a]: one\n\n[^a]: two [x]\n\n    [x]: /in\n    [y]: /y\n    [x]: /no\n\n[x]: /out\n",
        "[^a] [x]\n\n[^a]: one\n\n[^a]: [x]: /in\n\n[x]: /out\n\n[^a]: > [x]: /3\n",
    ] {
        let mut document = parse(source);
        assert!(!document.is_normalized(), "{:?}", source);
        let rendered = html(&document);
        document.normalize();
        assert!(document.is_normalized(), "{:?}", source);
        assert_eq!(html(&document), rendered, "{:?}", source);
        let once = state(&document);
        document.normalize();
        assert_eq!(state(&document), once, "{:?}", source);
        let written = round_trip(document);
        assert_eq!(to_html(&written), rendered, "{:?}: {:?}", source, written);
        let reparsed = parse(&written);
        assert!(reparsed.is_normalized(), "{:?}", written);
    }
    // The renderer counts the calls in a copy of a footnote definition, though it shows nothing
    // of the copy, so they give back references to calls that are not in the output, which go
    // with the copy.
    let mut document = parse("[^a]: x\n\n[^a]: y [^a]\n");
    assert!(html(&document).contains("data-footnote-backref"));
    assert!(!html(&document).contains("id=\"user-content-fnref-a\""));
    document.normalize();
    assert_eq!(html(&document), "");
    // A detached tree's copies are its own, removed as well, so it can then be attached.
    let mut document = parse("[a]\n\n> [b]: /1\n>\n> [b]: /2\n");
    let root = document.root();
    let quote = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::BlockQuote))
    });
    document.detach(quote).unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    assert!(document.is_normalized());
    document.attach(root, quote, None).unwrap();
    assert_eq!(document.descendants(quote).count(), 2);
}

/// Whether each lookup of `document`, through its index, finds what a walk of it from scratch
/// does, for each label and identifier it has.
fn lookups_match_a_walk(document: &Document, context: &str) {
    let bindings = document.bindings();
    let mut labels = Vec::new();
    let mut identifiers = Vec::new();
    for node in document.nodes() {
        match &node.kind {
            EventKind::Definition { label, .. } => {
                labels.push(label.resolve(document.source()).unwrap().to_owned());
            }
            EventKind::Start(Tag::FootnoteDefinition { identifier, .. })
            | EventKind::FootnoteReference { identifier, .. } => {
                identifiers.push(identifier.resolve(document.source()).unwrap().to_owned());
            }
            EventKind::Start(Tag::Link(Target::Reference(reference))) => {
                labels.push(
                    reference
                        .label
                        .resolve(document.source())
                        .unwrap()
                        .to_owned(),
                );
            }
            _ => {}
        }
    }
    for label in &labels {
        assert_eq!(
            document.definition(label),
            bindings.definition(label),
            "{}: {:?}",
            context,
            label
        );
    }
    for identifier in &identifiers {
        assert_eq!(
            document.footnote_definition(identifier),
            bindings.footnote_definition(identifier),
            "{}: {:?}",
            context,
            identifier
        );
    }
}

/// Normalize `document`, and check that it then is normalized, that normalizing again changes
/// nothing, that its lookups find what a walk does, and that detaching any of its definitions
/// and normalizing again keeps all of that.
fn normalizes_soundly(document: &mut Document, context: &str) {
    document.normalize();
    assert!(document.is_normalized(), "{}", context);
    let once = state(document);
    let mut again = document.clone();
    again.normalize();
    assert_eq!(state(&again), once, "{}", context);
    lookups_match_a_walk(document, context);
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
        let mut detached = document.clone();
        detached.detach(id).unwrap();
        lookups_match_a_walk(&detached, context);
        detached.normalize();
        assert!(detached.is_normalized(), "{}: {:?}", context, id);
        lookups_match_a_walk(&detached, context);
    }
}

/// Footnote definitions that are copies, nested in each other and holding definitions that
/// bind, are removed in one pass whose result depends on the tree alone: what binds in a copy
/// takes its place, the copies in that go too, and a detached footnote definition is the first
/// of its identifier in its own tree, so a copy in it goes and it stays.
#[test]
fn normalizing_removes_nested_copies() {
    for (source, written) in [
        (
            "[^x]:\n    [^x]: inner\n[^x]:\n    [^x]: inner\n",
            "[^x]: inner\n",
        ),
        (
            "[l]: /1\n[l]: /3\n\n[^x]: a\n\n[^x]: b\n\n    [^y]: c\n\n        [l]: /2\n",
            "[l]: /1\n\n[^x]: a\n\n[^y]: c\n",
        ),
        (
            "[^b]: z1\n\n[^a]: x1\n\n[^a]: x2\n\n    [^y]: y1\n\n        [^b]: z2\n\n[^a] [^y] [^b]\n",
            "[^b]: z1\n\n[^a]: x1\n\n[^y]: y1\n\n[^a] [^y] [^b]\n",
        ),
        (
            "[a]: /q\n[c]: /qb\n[c]: /5\n[d]: /qb\n[d]: /5\n\n[^z]:\n    [^z]:\n        [a]: /n\n",
            "[a]: /q\n\n[c]: /qb\n\n[d]: /qb\n\n[^z]:\n",
        ),
    ] {
        let mut document = parse(source);
        let rendered = html(&document);
        normalizes_soundly(&mut document, source);
        assert_eq!(html(&document), rendered, "{:?}", source);
        assert_eq!(to_markdown(&document).unwrap(), written, "{:?}", source);
    }
    // With the copy of `b` gone, detaching its binding unbinds the call, which reads back as
    // text, and a new definition of `b` can be added.
    let mut document = parse(
        "[^b]: z1\n\n[^a]: x1\n\n[^a]: x2\n\n    [^y]: y1\n\n        [^b]: z2\n\n[^a] [^y] [^b]\n",
    );
    document.normalize();
    let root = document.root();
    let first = document.children(root).next().unwrap();
    document.detach(first).unwrap();
    assert_eq!(document.footnote_definition("B"), None);
    assert!(html(&document).contains("[^b]"));
    let reparsed = parse(&to_markdown(&document).unwrap());
    assert_eq!(reparsed.footnote_definition("B"), None);
    document
        .append_child(root, footnote_definition("B"))
        .unwrap();
    assert!(document.footnote_definition("B").is_some());
    lookups_match_a_walk(&document, "b");
    // Detaching any node of a footnote definition that holds a copy of itself leaves a tree
    // that normalizes, the outer one as a root that stays.
    let source = "[^x]:\n    [^x]: inner\n";
    let parsed = parse(source);
    for id in parsed.descendants(parsed.root()).skip(1) {
        let mut document = parsed.clone();
        document.detach(id).unwrap();
        normalizes_soundly(&mut document, source);
        assert!(document.node(id).is_some());
    }
}

/// An item whose only block is a copy is empty once the copy goes, and an empty item cannot
/// start a list directly after a paragraph in an item of a tight list, as `- p\n  -` reads
/// back as a heading, so it goes as well, as `normalize` removes such an item whatever left it
/// empty.
#[test]
fn an_item_emptied_of_a_copy_after_a_paragraph_goes() {
    let mut document = parse("[b]: /0\n\n- x\n  - [b]: /l\n  - item\n");
    assert!(html(&document).contains("<li></li>"));
    normalizes_soundly(&mut document, "item");
    assert_eq!(
        html(&document),
        "<ul>\n<li>x\n<ul>\n<li>item</li>\n</ul>\n</li>\n</ul>\n"
    );
    assert_eq!(round_trip(document), "[b]: /0\n\n- x\n  - item\n");
    // Elsewhere an item emptied of a copy stays, empty.
    let mut document = parse("[b]: /0\n\n- [b]: /l\n- item\n");
    normalizes_soundly(&mut document, "list");
    assert_eq!(html(&document), "<ul>\n<li></li>\n<li>item</li>\n</ul>\n");
}

/// Random documents of footnote definitions nested in each other, copies of each other or
/// not, holding definitions and calls, each normalize soundly, with some of their nodes
/// detached first.
#[test]
fn normalizing_random_nested_copies() {
    fn blocks(next: &mut dyn FnMut(usize) -> usize, depth: usize, out: &mut String) {
        let indent = "    ".repeat(depth);
        for _ in 0..1 + next(3) {
            match next(if depth < 3 { 6 } else { 4 }) {
                0 => out.push_str(&format!(
                    "{indent}[{}]: /{}\n\n",
                    ["a", "b"][next(2)],
                    next(9)
                )),
                1 => out.push_str(&format!(
                    "{indent}[{}] [^{}]\n\n",
                    ["a", "b"][next(2)],
                    ["x", "y"][next(2)]
                )),
                2 => out.push_str(&format!("{indent}> [{}]: /q\n\n", ["a", "b"][next(2)])),
                3 => out.push_str(&format!("{indent}- [{}]: /i\n\n", ["a", "b"][next(2)])),
                _ => {
                    out.push_str(&format!("{indent}[^{}]: t\n\n", ["x", "y", "z"][next(3)]));
                    blocks(next, depth + 1, out);
                }
            }
        }
    }
    for seed in 0..400_u64 {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = |bound: usize| -> usize {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            std::convert::TryFrom::try_from(state % bound as u64).unwrap()
        };
        let mut source = String::new();
        blocks(&mut next, 0, &mut source);
        blocks(&mut next, 0, &mut source);
        let mut document = parse(&source);
        let context = format!("seed {}: {:?}", seed, source);
        lookups_match_a_walk(&document, &context);
        let ids: Vec<NodeId> = document.descendants(document.root()).skip(1).collect();
        for _ in 0..next(3) {
            let _ = document.detach(ids[next(ids.len())]);
        }
        lookups_match_a_walk(&document, &context);
        normalizes_soundly(&mut document, &context);
    }
}

/// Parsing indexes definitions by their keys in time linear in them, whatever the keys are:
/// keys whose FNV-1a hashes agree in their low 16 bits, which pick a slot in a table of up to
/// 2^16 slots, built from blocks of three letters that collide in turn, against keys as long
/// that are random.
#[test]
fn indexing_colliding_keys_is_linear() {
    let large = (1 << 8) * support::linear::SCALE;
    let stages = large.trailing_zeros();
    let step = |hash: u64, block: &[u8]| {
        block.iter().fold(hash, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    };
    // Two blocks of each stage that leave the low 16 bits of the hash alike, from what the
    // stages before leave, which those bits alone decide.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut pairs = Vec::new();
    for _ in 0..stages {
        let mut seen = std::collections::HashMap::new();
        let pair = (0..26 * 26 * 26)
            .find_map(|index: u32| {
                let block = [
                    b'a' + (index / 676) as u8,
                    b'a' + (index / 26 % 26) as u8,
                    b'a' + (index % 26) as u8,
                ];
                let low = step(hash, &block) & 0xffff;
                seen.insert(low, block)
                    .filter(|other| *other != block)
                    .map(|other| [other, block])
            })
            .unwrap();
        hash = step(hash, &pair[0]);
        pairs.push(pair);
    }
    let make = |count: usize, control: bool| {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut source = String::new();
        for index in 0..count {
            source.push('[');
            for (stage, pair) in pairs.iter().enumerate() {
                let block = if control {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let letters = state.to_le_bytes();
                    [
                        b'a' + letters[0] % 26,
                        b'a' + letters[1] % 26,
                        b'a' + letters[2] % 26,
                    ]
                } else {
                    pair[index >> stage & 1]
                };
                source.push_str(std::str::from_utf8(&block).unwrap());
            }
            source.push_str("]: /u\n");
        }
        parse(&source)
    };
    growth_against_control("parse colliding keys", make, |document| {
        drop(parse(document.source()));
    });
}

/// Removing many copies of one label or identifier, and detaching its binding again and
/// again, takes time linear in them: against a control of as many distinct labels.
#[test]
fn copies_of_one_key_go_in_linear_time() {
    for footnote in [false, true] {
        let make = move |count: usize, control: bool| {
            let mut source = String::new();
            for index in 0..count {
                let label = if control {
                    format!("a{index}")
                } else {
                    String::from("a")
                };
                if footnote {
                    source.push_str(&format!("[^{label}]: u\n\n"));
                } else {
                    source.push_str(&format!("[{label}]: /u\n"));
                }
            }
            parse(&source)
        };
        growth_against_control_from(1 << 13, "normalize copies", make, |document| {
            document.clone().normalize();
        });
        growth_against_control_from(1 << 12, "detach bindings", make, |document| {
            let mut document = document.clone();
            let root = document.root();
            while let Some(id) = document.children(root).next() {
                document.detach(id).unwrap();
            }
        });
    }
}

/// Moving the definitions out of a copy of a footnote definition takes time linear in them:
/// against a control whose footnote definition is not a copy.
#[test]
fn definitions_move_out_of_a_copy_in_linear_time() {
    let make = |count: usize, control: bool| {
        let mut source = String::from(if control {
            "[^x]: a\n\n[^y]: b\n"
        } else {
            "[^x]: a\n\n[^x]: b\n"
        });
        for index in 0..count {
            source.push_str(&format!("\n    [l{index}]: /u\n\n    p\n"));
        }
        parse(&source)
    };
    growth_against_control_from(1 << 9, "normalize", make, |document| {
        document.clone().normalize();
    });
}
