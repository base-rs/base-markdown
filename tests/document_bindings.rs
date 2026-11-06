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

/// The per-subtree operations look a reference up in the document's index of its definitions,
/// so calling one on each section of a document takes time linear in the document: each is
/// timed against a control of as many resources.
#[test]
fn per_section_work_is_linear() {
    // `count` sections, each a heading that holds a reference, or a resource, to its own
    // definition and another to a label of `duplicates` definitions, which come before the
    // sections, and the sections' definitions after them. Several definitions of a label are
    // found among themselves, in time linear in the siblings between them, not in the document.
    let sections = |duplicates: usize| {
        move |count: usize, control: bool| {
            let mut source = String::new();
            for _ in 0..duplicates {
                source.push_str("[d]: /d\n");
            }
            for index in 0..count {
                if control {
                    source.push_str(&format!("# h [a{index}](/u) [d](/d)\n\n"));
                } else {
                    source.push_str(&format!("# h [a{index}] [d]\n\n"));
                }
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
    for duplicates in [1, 4] {
        let make = sections(duplicates);
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
    }
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
    let mut document = parse("[^n]\n\n[^n]: x\n");
    let outer = document.footnote_definition("N").unwrap();
    let inner = document.append_child(outer, footnote("N")).unwrap();
    assert_eq!(document.footnote_definition("N"), Some(outer));
    // Relabelled away and back, the outer definition is indexed after the inner one.
    document.set_kind(outer, footnote("Z")).unwrap();
    assert_eq!(document.footnote_definition("N"), Some(inner));
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
