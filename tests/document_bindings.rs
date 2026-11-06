// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! References are names and definitions bindings: a document stores a reference by its label
//! alone, and it leads where the first definition of that label in document order does, which
//! is found when asked, so it follows every edit to the definitions.

mod support;
use base_markdown::{
    markdown::{to_markdown, to_markdown_node},
    stream::{Event, EventKind, Reference, Resolved, Tag, Target, Text},
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
    // Nothing edited: everything is bound.
    let document = parse(SOURCE);
    assert!(document.is_normalized());
    assert_eq!(round_trip(document), SOURCE);

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
    // one.
    let mut document = parse(SOURCE);
    let id = definition(&document, "c");
    document
        .set_kind(
            id,
            EventKind::Definition {
                label: Text::Literal("x"),
                destination: Text::Literal("/5"),
                title: None,
            },
        )
        .unwrap();
    assert!(!document.is_normalized());
    // The first `x` is still the first.
    assert!(html(&document).contains("<a href=\"/2\">b</a>"));
    let written = round_trip(document);
    assert!(
        written.starts_with("[a] [b][x] !\\[c]\\[] [^n]\n"),
        "{:?}",
        written
    );

    // Moving a definition before the first of its label makes it the binding.
    let mut document = parse(SOURCE);
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
    assert!(html(&document).contains("<a href=\"/4\">a</a>"));
    let written = round_trip(document);
    assert!(
        written.starts_with("[A]: /4\n\n[a] [b][x]"),
        "{:?}",
        written
    );

    // Adding a definition binds a reference by name that was unbound.
    let mut document = parse(SOURCE);
    document.detach(definition(&document, "c")).unwrap();
    assert!(!document.is_normalized());
    let root = document.root();
    document
        .append_child(
            root,
            EventKind::Definition {
                label: Text::Literal("C"),
                destination: Text::Literal("/6"),
                title: None,
            },
        )
        .unwrap();
    assert!(document.is_normalized());
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
    // A footnote definition with an edited identifier binds the calls of that identifier, ahead
    // of a later definition of it.
    let mut document = parse("[^n] [^m]\n\n[^n]: note\n\n[^m]: other\n");
    let note = find(&document, |kind| {
        matches!(kind, EventKind::Start(Tag::FootnoteDefinition { .. }))
    });
    document
        .set_kind(
            note,
            EventKind::Start(Tag::FootnoteDefinition {
                identifier: Text::Value("M".into()),
                label: Some(Text::Value("m".into())),
            }),
        )
        .unwrap();
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
    let _timing = support::linear::TIMING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let small = 1 << 10;
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
/// as a lookup takes constant expected time, however many references and definitions it has: each is
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
    growth_against_control("write", bound, |document| {
        to_markdown(document).unwrap();
    });
    growth_against_control("write a subtree", bound, |document| {
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
    growth_against_control("write a deep subtree", deep, |document| {
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
fn streamed_references_by_name_render_as_text() {
    use base_markdown::stream::TagKind;
    let event = |kind| Event { kind, span: 0..0 };
    let reference = |kind, label| {
        Target::Reference(Box::new(Reference {
            kind,
            label: Text::Literal(label),
        }))
    };
    let events = vec![
        event(EventKind::Start(Tag::Document {
            line_ending: None,
            frontmatter: None,
        })),
        event(EventKind::Start(Tag::Paragraph { tight: false })),
        event(EventKind::Start(Tag::Link(reference(
            ReferenceKind::Full,
            "x",
        )))),
        event(EventKind::Text(Text::Literal("a"))),
        event(EventKind::End(TagKind::Link)),
        event(EventKind::Start(Tag::Image(reference(
            ReferenceKind::Shortcut,
            "b",
        )))),
        event(EventKind::Text(Text::Literal("b"))),
        event(EventKind::End(TagKind::Image)),
        event(EventKind::End(TagKind::Paragraph)),
        event(EventKind::End(TagKind::Document)),
    ];
    // A writer that fails at the opening bracket fails the render.
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
    assert!(base_markdown::render_events(
        "",
        events.clone().into_iter().map(Ok),
        &base_markdown::Options::gfm().compile,
        &mut Failing,
    )
    .is_err());
    let mut output = String::new();
    base_markdown::render_events(
        "",
        events.into_iter().map(Ok),
        &base_markdown::Options::gfm().compile,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, "<p>[a][x]![b]</p>");
}
