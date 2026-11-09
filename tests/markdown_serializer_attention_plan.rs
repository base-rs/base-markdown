// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! How the writer plans the sequences of attention: parsed documents that read back as the same
//! tree, and the shapes that have no spelling under its invariants, which it rejects.

mod support;
use base_markdown::{Document, MarkdownOptions, ParseOptions};
use support::fixture::{to_markdown, Emphasis, Node, Paragraph, Root, Strong, Text};
use support::round_trip::{round_trip, Outcome};

fn check(source: &str, options: &ParseOptions, marker: char) -> Outcome {
    let markdown = MarkdownOptions {
        emphasis: marker,
        strong: marker,
        ..MarkdownOptions::default()
    };
    round_trip(Document::parse(source, options).unwrap(), &markdown, false)
}

/// Assert that `source` reads back as the same tree, with either marker preferred.
fn same_tree(source: &str) {
    for options in [ParseOptions::default(), ParseOptions::gfm()] {
        for marker in ['*', '_'] {
            match check(source, &options, marker) {
                Outcome::Pass => {}
                Outcome::Unwritable(error) => {
                    panic!(
                        "{:?} with {:?} should be writable: {}",
                        source, marker, error
                    )
                }
                Outcome::Fail(failure) => panic!(
                    "{source:?} with {marker:?} should read back the same, as {:?}: {} / {}",
                    failure.output, failure.before, failure.after
                ),
                Outcome::Rejected => unreachable!("a parsed document is not rejected"),
            }
        }
    }
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn emphasis(children: Vec<Node>) -> Node {
    Node::Emphasis(Emphasis {
        children,
        position: None,
    })
}

fn strong(children: Vec<Node>) -> Node {
    Node::Strong(Strong {
        children,
        position: None,
    })
}

fn root(children: Vec<Node>) -> Node {
    Node::Root(Root {
        children: vec![Node::Paragraph(Paragraph {
            children,
            position: None,
        })],
        position: None,
    })
}

#[test]
fn issue_110_closing_run_next_to_a_sibling_opener() {
    same_tree("*a*__**c*.*****\\~**&#x61;**__");
    same_tree("**__a_._____.__&#x61;__.a**c");
}

#[test]
fn issue_111_strong_that_can_close_inside_emphasis() {
    // The `**` between letters can open and close, so it must not close the enclosing `*`.
    same_tree("*a**b**c*");
    same_tree("_a**b**c_");
    same_tree("*a __b__ c*");
    same_tree("**a*b*c**");
}

#[test]
fn issue_101_encoded_edges() {
    same_tree("_*a*b_&#97;");
    same_tree("&#97;_a*b*_.");
    same_tree("*_a_ b*");
    same_tree("_*.*._");
}

#[test]
fn pull_292_inputs() {
    for source in [
        "_*a*b_",
        "**a*.**",
        "***.___.___***",
        "***`c`___`c`___***",
        "*_a_ b*",
        "*_._&#x62;*",
        "***\\[*(*-*",
        "**__a__ b**",
    ] {
        same_tree(source);
    }
}

#[test]
fn references_and_footnotes() {
    for source in [
        "_*a*b_ [r]\n\n[r]: u",
        "_*a*b_ [^1]\n\n[^1]: x",
        "[_*a*b_][r]\n\n[r]: u",
        "- _*a*b_ [r]\n\n[r]: u",
        "[^x]: _*a*b_ [^x]",
    ] {
        same_tree(source);
    }
}

#[test]
fn repeated_shapes_share_no_budget() {
    // Each paragraph, run of text or line is planned on its own merits.
    for source in [
        "_*a*b_ _*a*b_ _*a*b_",
        "**a*.** **a*.** **a*.**",
        "- _*a*b_ _*a*b_\n  _*a*b_",
        &"_*a*b_ ".repeat(200),
        &"***.___.___***\n\n".repeat(50),
    ] {
        same_tree(source);
    }
}

#[test]
fn unwritable_shapes() {
    let nest = |depth: usize, inner: Node, wrap: &dyn Fn(usize, Node) -> Node| {
        (0..depth).fold(inner, |node, level| wrap(level, node))
    };
    let em = |_: usize, node: Node| emphasis(vec![node]);
    let alternate = |level: usize, node: Node| {
        if level.is_multiple_of(2) {
            emphasis(vec![node])
        } else {
            strong(vec![node])
        }
    };
    let shapes = [
        // Emphasis four deep has only two markers for three adjacent runs on each side.
        ("emphasis four deep", root(vec![nest(4, text("a"), &em)])),
        (
            "emphasis three deep on punctuation",
            root(vec![nest(3, text("."), &em)]),
        ),
        (
            "emphasis three deep on punctuation, then punctuation",
            root(vec![emphasis(vec![nest(2, text("|"), &em), text("(")])]),
        ),
        (
            "emphasis three deep on punctuation, then a letter",
            root(vec![emphasis(vec![nest(2, text("."), &em), text("a")])]),
        ),
        (
            "emphasis and strong alternating six deep",
            root(vec![nest(6, text("a"), &alternate)]),
        ),
    ];
    for (name, tree) in shapes {
        assert_eq!(
            to_markdown(&tree).map_err(|error| error.rule_id),
            Err("unwritable-node"),
            "{name} should be unwritable"
        );
    }
    // Parsed documents with such shapes, which the writer rejects rather than misspell. The
    // first reads as strong holding emphasis that holds strong, text and strong: the last strong
    // opens between punctuation, so it can close, and with either marker it pairs with an
    // opening run of the strong or emphasis around it. Its source writes the emphasis as two
    // nodes, `_…_*…*`, which normalizing joins, and the writer writes each node once.
    for (source, marker) in [
        ("___**~~a.~~**_*&#x62;**\\|\\#***__", '*'),
        ("___**~~a.~~**_*&#x62;**\\|\\#***__", '_'),
        ("***|*&#x20;&#xA;*(**", '*'),
        ("***|*&#x20;&#xA;*(**", '_'),
    ] {
        assert!(
            matches!(
                check(source, &ParseOptions::default(), marker),
                Outcome::Unwritable(_)
            ),
            "{:?} should be unwritable with {:?}",
            source,
            marker
        );
    }
}

#[test]
fn attention_written_on_its_own_checks_its_options() {
    let document = Document::parse("*a*", &ParseOptions::default()).unwrap();
    let emphasis = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                base_markdown::stream::EventKind::Start(base_markdown::stream::Tag::Emphasis)
            )
        })
        .unwrap();
    let options = MarkdownOptions {
        emphasis: 'x',
        ..MarkdownOptions::default()
    };
    assert!(base_markdown::to_markdown_node(&document, emphasis, &options).is_err());
    assert_eq!(
        base_markdown::to_markdown_node(&document, emphasis, &MarkdownOptions::default()).unwrap(),
        "*a*"
    );
}
