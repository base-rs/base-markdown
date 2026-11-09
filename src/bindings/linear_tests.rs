// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Looking up, resolving and settling bindings does work linear in the document, counted with [`crate::work`].

extern crate std;

use crate::markdown::{to_markdown, to_markdown_node, Options as MarkdownOptions};
use crate::stream::{EventKind, Tag, Text};
use crate::work::tests::{assert_against_control, counted, SCALE};
use crate::{CompileOptions, Document, HeadingOptions, NodeId, ParseOptions};
use core::fmt::Write as _;
use std::borrow::ToOwned;
use std::format;
use std::string::String;
use std::vec::Vec;

fn parse(source: &str) -> Document {
    Document::parse(source.to_owned(), &ParseOptions::gfm()).unwrap()
}

fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&crate::Options::gfm().compile, &mut output)
        .unwrap();
    output
}

/// Assert that the work of `run` grows over `SCALE` times the input no more than twice as much as the control's does.
fn growth_against_control(
    label: &str,
    make: impl Fn(usize, bool) -> Document,
    run: impl Fn(&Document),
) {
    growth_against_control_from(1 << 6, label, make, run);
}

/// [`growth_against_control`] from `small` items, for work whose quadratic part shows only at
/// larger sizes.
fn growth_against_control_from(
    small: usize,
    label: &str,
    make: impl Fn(usize, bool) -> Document,
    run: impl Fn(&Document),
) {
    let documents = [
        make(small, false),
        make(small * SCALE, false),
        make(small, true),
        make(small * SCALE, true),
    ];
    let large = |size: usize| usize::from(size != small);
    assert_against_control(
        label,
        small,
        |size| counted(|| run(&documents[large(size)])),
        |size| counted(|| run(&documents[2 + large(size)])),
    );
}

/// Resolving, rendering, writing and normalizing references takes time linear in the document,
/// however many references and definitions it has: each is counted against a control of as many resources, whose growth the references' would exceed
/// `SCALE` times if a lookup scanned the document.
#[test]
fn reference_work_is_linear() {
    // `count` references to as many definitions, or as many resources, in a block quote, with a
    // footnote.
    let bound = |count: usize, control: bool| {
        let mut source = String::from("> ");
        for index in 0..count {
            if control {
                write!(source, "[l](/{index}) ").unwrap();
            } else {
                write!(source, "[l][{index}] ").unwrap();
            }
        }
        source.push_str("[^n]\n\n");
        for index in 0..count {
            writeln!(source, "[{index}]: /{index}").unwrap();
        }
        source.push_str("\n[^n]: x\n");
        parse(&source)
    };
    growth_against_control("render", bound, |document| {
        html(document);
    });
    growth_against_control_from(1 << 5, "write", bound, |document| {
        to_markdown(document).unwrap();
    });
    growth_against_control_from(1 << 5, "write a subtree", bound, |document| {
        let quote = document.children(document.root()).next().unwrap();
        to_markdown_node(document, quote, &MarkdownOptions::default()).unwrap();
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
    growth_against_control_from(1 << 5, "write a deep subtree", deep, |document| {
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
        to_markdown_node(document, quote, &MarkdownOptions::default()).unwrap();
    });
}

/// The per-subtree operations look a reference up in the document's index of its definitions,
/// so calling one on each section of a document takes time linear in the document: each is
/// counted against a control of as many resources.
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
                    writeln!(source, "# h [a{index}](/u) [d](/d)\n").unwrap();
                } else {
                    writeln!(source, "# h [a{index}] [d]\n").unwrap();
                }
                if copies && index == count / 2 {
                    source.push_str("[d]: /2\n\n");
                }
            }
            if copies {
                source.push_str("[d]: /3\n\n");
            }
            for index in 0..count {
                writeln!(source, "[a{index}]: /u").unwrap();
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
                    .render_node(id, &CompileOptions::default(), &mut String::new())
                    .unwrap();
            });
        });
        growth_against_control_from(1 << 5, "to_markdown_node", make, |document| {
            each(document, &|id| {
                to_markdown_node(document, id, &MarkdownOptions::default()).unwrap();
            });
        });
        growth_against_control("subtree_events", make, |document| {
            each(document, &|id| {
                document.subtree_events(id).for_each(drop);
            });
        });
        growth_against_control("headings", make, |document| {
            drop(document.headings(&HeadingOptions::default()));
        });
    }
}

/// Writing a node deep in the attached document finds the tree that binds its references at
/// once, not by walking up to the root: on each of a few references in a paragraph `depth`
/// lists deep, against a control of as many in a paragraph one list deep, so the work of each
/// call stays the same however deep the node is.
#[test]
fn writing_deep_attached_nodes_is_linear() {
    let make = |depth: usize, control: bool| {
        let depth = if control { 1 } else { depth };
        parse(&format!(
            "[a]: /0\n\n{}{}\n",
            "- ".repeat(depth),
            "[a] ".repeat(64)
        ))
    };
    // The links are found before counting, as finding them walks the lists.
    let steps = |document: Document| {
        let links: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Start(Tag::Link(_))
                )
            })
            .collect();
        counted(|| {
            for &id in &links {
                to_markdown_node(&document, id, &MarkdownOptions::default()).unwrap();
            }
        })
    };
    assert_against_control(
        "to_markdown_node in deep lists",
        1 << 9,
        |size| steps(make(size, false)),
        |size| steps(make(size, true)),
    );
}

/// A per-node call on a node of the attached document takes time linear in the node's subtree,
/// not in its depth: on each reference, or each text, of `count` emphases nested in each
/// other, each holding one, against a control of as many emphases, each in a paragraph of its
/// own. Parsed inline nesting is as deep as the input is long.
#[test]
fn per_node_work_on_deep_nesting_is_linear() {
    let nested = |references: bool| {
        move |count: usize, control: bool| {
            let item = if references { "*a [x] " } else { "*a " };
            let mut source = String::new();
            if control {
                for _ in 0..count {
                    source.push_str(item);
                    source.push_str("b*\n\n");
                }
            } else {
                source.push_str(&item.repeat(count));
                source.push('b');
                source.push_str(&"*".repeat(count));
            }
            source.push_str("\n\n[x]: /u\n");
            parse(&source)
        }
    };
    let each = |document: &Document, kind: fn(&EventKind<'_>) -> bool, run: &dyn Fn(NodeId)| {
        let ids: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|&id| kind(&document.node(id).unwrap().kind))
            .collect();
        for id in ids {
            run(id);
        }
    };
    let link = |kind: &EventKind<'_>| matches!(kind, EventKind::Start(Tag::Link(_)));
    let text = |kind: &EventKind<'_>| matches!(kind, EventKind::Text(_));
    // An edit leaves the document unsettled, so its calls take the path that looks names up.
    let edited = |count: usize, control: bool| {
        let mut document = nested(true)(count, control);
        let paragraph = document.children(document.root()).next().unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::from(String::from("c"))))
            .unwrap();
        document
    };
    for (label, make) in [
        ("parsed", &nested(true) as &dyn Fn(usize, bool) -> Document),
        ("edited", &edited),
    ] {
        let make = |count, control| make(count, control);
        growth_against_control(&format!("{label} text_content"), make, |document| {
            each(document, link, &|id| drop(document.text_content(id)));
        });
        growth_against_control(&format!("{label} render_node"), make, |document| {
            each(document, link, &|id| {
                document
                    .render_node(id, &CompileOptions::default(), &mut String::new())
                    .unwrap();
            });
        });
        growth_against_control(&format!("{label} to_markdown_node"), make, |document| {
            each(document, link, &|id| {
                to_markdown_node(document, id, &MarkdownOptions::default()).unwrap();
            });
        });
        // The cheapest of the calls on a link shows a walk to the root, were there one, at
        // these sizes; the others take enough more per call to hide it.
        growth_against_control_from(
            1 << 9,
            &format!("{label} subtree_events"),
            make,
            |document| {
                each(document, link, &|id| {
                    document.subtree_events(id).for_each(drop);
                });
            },
        );
        growth_against_control(&format!("{label} destination"), make, |document| {
            each(document, link, &|id| {
                assert!(document.destination(id).is_some());
            });
        });
    }
    let plain = nested(false);
    growth_against_control("text_content of text", plain, |document| {
        each(document, text, &|id| drop(document.text_content(id)));
    });
    growth_against_control("detach each text", plain, |document| {
        let mut document = document.clone();
        let ids: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|&id| text(&document.node(id).unwrap().kind))
            .collect();
        for id in ids {
            document.detach(id).unwrap();
        }
    });
    // Detaching each emphasis from the outside in moves each out of the attached document
    // once.
    growth_against_control("detach outside in", plain, |document| {
        let mut document = document.clone();
        let ids: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|&id| {
                matches!(
                    document.node(id).unwrap().kind,
                    EventKind::Start(Tag::Emphasis)
                )
            })
            .collect();
        for id in ids {
            document.detach(id).unwrap();
        }
    });
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
            write!(source, "[a{index}] ").unwrap();
        }
        source.push_str("\n\n");
        for index in 0..count {
            writeln!(source, "[a{index}]: /1\n").unwrap();
        }
        let second = if control { "b" } else { "a" };
        for index in 0..count {
            writeln!(source, "[{second}{index}]: /2\n").unwrap();
        }
        source
    };
    let make = |count, control| parse(&source(count, control));
    growth_against_control_from(1 << 7, "parse", make, |document| {
        drop(parse(document.source()));
    });
    growth_against_control_from(1 << 7, "render_html", make, |document| {
        drop(html(document));
    });
    growth_against_control_from(1 << 5, "to_markdown", make, |document| {
        to_markdown(document).unwrap();
    });
    growth_against_control_from(1 << 7, "headings", make, |document| {
        drop(document.headings(&HeadingOptions::default()));
    });
    growth_against_control("normalize", make, |document| {
        document.clone().normalize();
    });
}

/// Removing many copies of one label, and detaching its binding again and again, takes time
/// linear in them: against a control of as many distinct labels. Footnote definitions are
/// indexed by the same code, keyed by identifier instead.
#[test]
fn copies_of_one_key_go_in_linear_time() {
    // The quadratic part of removing copies one at a time is small next to the rest of the work
    // until there are thousands of them.
    let small = 1 << 12;
    let source = |count: usize, control: bool| {
        let mut source = String::new();
        for index in 0..count {
            if control {
                writeln!(source, "[a{index}]: /u").unwrap();
            } else {
                source.push_str("[a]: /u\n");
            }
        }
        source
    };
    // Both checks take the same documents, parsed once.
    let documents: Vec<((usize, bool), Document)> = [small, small * SCALE]
        .iter()
        .flat_map(|&count| [false, true].map(|control| (count, control)))
        .map(|key| (key, parse(&source(key.0, key.1))))
        .collect();
    let make = |count: usize, control: bool| {
        documents
            .iter()
            .find(|(key, _)| *key == (count, control))
            .unwrap()
            .1
            .clone()
    };
    growth_against_control_from(small, "normalize copies", make, |document| {
        document.clone().normalize();
    });
    growth_against_control_from(small, "detach bindings", make, |document| {
        let mut document = document.clone();
        let root = document.root();
        while let Some(id) = document.children(root).next() {
            document.detach(id).unwrap();
        }
    });
}

/// Normalizing many trees that hold copies takes time linear in the document, not in the trees
/// times the arena: copies of a footnote definition that hold copies of a definition, which go
/// with them, and detached block quotes that hold copies, against controls whose trees hold no
/// copy.
#[test]
fn many_trees_with_copies_normalize_in_linear_time() {
    let copies = |count: usize, control: bool| {
        let second = if control { "b" } else { "a" };
        let mut source = String::from("[a]: /0\n\n[^x]: y\n\n");
        for _ in 0..count {
            writeln!(source, "[^x]:\n    [a]: /1\n\n    [{second}]: /2\n").unwrap();
        }
        source.push_str("[a] [^x]\n");
        parse(&source)
    };
    let quotes = |count: usize, control: bool| {
        let second = if control { "b" } else { "a" };
        let mut document = parse(&format!("> [a]: /1\n>\n> [{second}]: /2\n\n").repeat(count));
        let root = document.root();
        while let Some(id) = document.children(root).next() {
            document.detach(id).unwrap();
        }
        document
    };
    // Allocating the arena's size for each tree shows only from hundreds of them.
    for (label, make) in [
        (
            "normalize copies holding copies",
            &copies as &dyn Fn(usize, bool) -> Document,
        ),
        ("normalize detached quotes", &quotes),
    ] {
        growth_against_control_from(1 << 8, label, make, |document| {
            document.clone().normalize();
        });
    }
}

/// Normalizing copies nested in copies, each holding the next, and a copy at the bottom of
/// deep lists or footnote definitions, whose containers each settle what ends them, takes time
/// linear in the depth: against a control of a copy at the bottom of block quotes as deep,
/// which settle nothing below them.
#[test]
fn deeply_nested_copies_normalize_in_linear_time() {
    let identifiers = |depth: usize, cycle: &[&str]| {
        (0..depth)
            .map(|index| cycle[index % cycle.len()])
            .collect::<String>()
            + "y\n"
    };
    let chains: [(&str, &dyn Fn(usize) -> String); 4] = [
        ("one identifier", &|depth| identifiers(depth, &["[^x]: "])),
        ("two identifiers", &|depth| {
            identifiers(depth, &["[^a]: ", "[^b]: "])
        }),
        ("lists", &|depth| {
            format!("[a]: /0\n\n{}[a]: /1\n", "- ".repeat(depth))
        }),
        ("footnote definitions", &|depth| {
            let containers = (0..depth).map(|index| format!("[^x{index}]: "));
            format!("[a]: /0\n\n{}[a]: /1\n", containers.collect::<String>())
        }),
    ];
    let control = |depth: usize| format!("[a]: /0\n\n{}[a]: /1\n", "> ".repeat(depth));
    for (label, chain) in chains {
        let mut document = parse(&chain(5));
        document.normalize();
        assert!(document.is_normalized(), "{}", label);
        let make = |depth: usize, is_control: bool| {
            parse(&if is_control {
                control(depth)
            } else {
                chain(depth)
            })
        };
        growth_against_control_from(1 << 8, label, make, |document| {
            document.clone().normalize();
        });
    }
    // Only the innermost of one identifier binds, and every other is a copy of its own, empty.
    let mut document = parse(&identifiers(3, &["[^x]: "]));
    document.normalize();
    assert_eq!(to_markdown(&document).unwrap(), "[^x]: y\n");
    let roots = document
        .descendants(document.root())
        .filter(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::FootnoteDefinition { .. })
            )
        })
        .count();
    assert_eq!(roots, 1);
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
            writeln!(source, "\n    [l{index}]: /u\n\n    p").unwrap();
        }
        parse(&source)
    };
    growth_against_control_from(1 << 7, "normalize", make, |document| {
        document.clone().normalize();
    });
}
