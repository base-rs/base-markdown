// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! How the writer plans the sequences of attention: parsed documents that read back as the same
//! tree, and the shapes that have no spelling under its invariants, which it rejects.

mod support;
use base_markdown::{Document, LineWrapping, MarkdownOptions, ParseOptions};
use support::fixture::{
    to_markdown, Break, Emphasis, Heading, Html, InlineCode, List, ListItem, Node, Paragraph, Root,
    Strong, Text,
};
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
fn whitespace_at_the_start_of_a_line() {
    // A space after a hard break starts a line, where it is written as a reference, which is
    // punctuation next to the runs after it.
    same_tree("*a\\\n&#x20;_._ b*");
    same_tree("*~~a\\\n&#x20;_._~~*");
}

/// Assert that `tree` reads back as itself, with either marker preferred.
fn writes(name: &str, tree: &Node) {
    for marker in ['*', '_'] {
        let options = MarkdownOptions {
            emphasis: marker,
            strong: marker,
            ..MarkdownOptions::default()
        };
        assert!(
            matches!(round_trip(tree.document(), &options, true), Outcome::Pass),
            "{} with {:?} should read back as itself",
            name,
            marker
        );
    }
}

/// Assert that `tree` reads back as itself under `options` and is written as `expected`.
fn writes_as(tree: &Node, options: &MarkdownOptions, expected: &str) {
    assert!(
        matches!(round_trip(tree.document(), options, true), Outcome::Pass),
        "{:?} should read back as the tree",
        expected
    );
    let mut document = tree.document();
    document.normalize();
    assert_eq!(
        base_markdown::to_markdown_with_options(&document, options).unwrap(),
        expected
    );
}

fn hard_break() -> Node {
    Node::Break(Break { position: None })
}

fn code(value: &str, language: &str) -> Node {
    Node::InlineCode(InlineCode {
        value: value.into(),
        lang: Some(language.into()),
        position: None,
    })
}

fn markers(emphasis: char, strong: char, line_wrapping: LineWrapping) -> MarkdownOptions {
    MarkdownOptions {
        emphasis,
        strong,
        line_wrapping,
        close_atx: true,
        ..MarkdownOptions::default()
    }
}

#[test]
fn text_is_planned_as_it_is_written() {
    // Unwrapped, a line ending and the spaces after it are one space, which after a hard break
    // starts a line and is encoded, so it is punctuation next to the runs after it.
    let unwrap = |emphasis, strong| markers(emphasis, strong, LineWrapping::Unwrap);
    writes_as(
        &root(vec![
            strong(vec![hard_break()]),
            strong(vec![strong(vec![text("\n .")]), text("c")]),
        ]),
        &unwrap('_', '*'),
        "**\\\n&#x20;__.__&#x63;**\n",
    );
    let tree = root(vec![emphasis(vec![strong(vec![
        hard_break(),
        emphasis(vec![text("\n "), text("`")]),
    ])])]);
    writes_as(&tree, &unwrap('_', '*'), "_**\\\n&#x20;*\\`***_\n");
    writes_as(&tree, &unwrap('*', '_'), "*__\\\n&#x20;_\\`___*\n");
    let item = Node::Root(Root {
        children: vec![Node::List(List {
            ordered: true,
            start: Some(1),
            spread: false,
            position: None,
            children: vec![Node::ListItem(ListItem {
                spread: false,
                checked: None,
                position: None,
                children: vec![Node::Paragraph(Paragraph {
                    position: None,
                    children: vec![strong(vec![
                        strong(vec![text(" \n"), strong(vec![text("_"), text("\n")])]),
                        Node::Html(Html {
                            value: "<!-- c -->".into(),
                            position: None,
                        }),
                    ])],
                })],
            })],
        })],
        position: None,
    });
    writes_as(
        &item,
        &unwrap('_', '_'),
        "1. &#x20;**____\\_____ <!-- c -->**\n",
    );
    // A space before the line ending at the end of the paragraph is written as it is, before the
    // line ending, which is encoded.
    writes_as(
        &root(vec![
            emphasis(vec![
                hard_break(),
                emphasis(vec![text("y"), emphasis(vec![hard_break(), text("b")])]),
            ]),
            text(" \n"),
        ]),
        &MarkdownOptions::default(),
        "*\\\n*&#x79;_\\\nb_** &#xA;\n",
    );
    // A language character before code with a language is encoded, which keeps it out of the
    // language, so it is punctuation next to the run before it.
    writes_as(
        &root(vec![text("a"), strong(vec![text("a"), code("*", "rust")])]),
        &MarkdownOptions::default(),
        "&#x61;**&#x61;rust:`*`**\n",
    );
    writes_as(
        &root(vec![
            text("b"),
            emphasis(vec![text("a"), code(" a ", "rust")]),
        ]),
        &MarkdownOptions::default(),
        "&#x62;*&#x61;rust:`  a  `*\n",
    );
    // A space at the start of the content of an ATX heading would be stripped, so it is encoded,
    // which makes it punctuation next to the runs after it.
    let heading = Node::Root(Root {
        children: vec![Node::Heading(Heading {
            depth: 2,
            position: None,
            children: vec![strong(vec![
                text(" \n"),
                strong(vec![strong(vec![strong(vec![
                    strong(vec![text("a"), text("(")]),
                    text(" \n"),
                ])])]),
                text("[."),
            ])],
        })],
        position: None,
    });
    writes_as(
        &heading,
        &unwrap('_', '_'),
        "## &#x20;__**____**a(**____** \\[.__ ##\n",
    );
}

#[test]
fn chains_of_any_depth() {
    // Each level holds the one inside it and then text, so each shares its opening run with
    // the levels inside it, as `***a* b** c`.
    for (inner, after) in [("a", [" b", " c"]), (".", [".", "."])] {
        for strong_first in [false, true] {
            let mut node = text(inner);
            for depth in 1..=30 {
                let level = depth % 2 == usize::from(strong_first);
                node = if level {
                    emphasis(vec![node, text(after[0])])
                } else {
                    strong(vec![node, text(after[1])])
                };
                writes(
                    &format!("a chain {depth} deep on {inner:?}"),
                    &root(vec![node.clone()]),
                );
            }
        }
    }
}

#[test]
fn only_children_alternating_around_strong() {
    // Six deep around strong, each the only child of the next, has a spelling, though six deep
    // around emphasis has none (`unwritable_shapes`).
    let mut node = text("a");
    for level in 0..6 {
        node = if level % 2 == 0 {
            strong(vec![node])
        } else {
            emphasis(vec![node])
        };
    }
    writes(
        "strong and emphasis alternating six deep",
        &root(vec![node]),
    );
}

#[test]
fn runs_shared_with_a_child_and_a_parent() {
    // The inner emphasis shares its opening run with the strong inside it and its closing run
    // with the emphasis around it.
    same_tree("*x ***a b**_\\*`a`a!_**");
    writes(
        "emphasis sharing a run with its child and its parent",
        &root(vec![emphasis(vec![
            text("x "),
            emphasis(vec![
                strong(vec![text("a b")]),
                emphasis(vec![
                    text("*"),
                    Node::InlineCode(support::fixture::InlineCode {
                        value: "a".into(),
                        lang: None,
                        position: None,
                    }),
                    text("a!"),
                ]),
            ]),
        ])]),
    );
    // Strong sharing its opening run with the emphasis inside it and its closing run with the
    // strong inside it.
    same_tree("a&***\\*b**_-\\*_**b***-");
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
            "emphasis and strong alternating six deep around emphasis, each the only child of the next",
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

/// Assert that `source` reads back as itself under `options`, and is written as `expected`.
fn parsed_writes_as(source: &str, options: &MarkdownOptions, expected: &str) {
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert!(
        matches!(round_trip(document.clone(), options, false), Outcome::Pass),
        "{:?} should read back as itself",
        source
    );
    let mut document = document;
    document.normalize();
    assert_eq!(
        base_markdown::to_markdown_with_options(&document, options).unwrap(),
        expected
    );
}

#[test]
fn the_next_child_is_judged_after_this_one() {
    // A soft break is written after the text before it, not at the start of the heading, so
    // unwrapped it is a space that the `\` before it does not escape.
    let unwrap = markers('*', '*', LineWrapping::Unwrap);
    let unwrap = MarkdownOptions {
        close_atx: false,
        ..unwrap
    };
    parsed_writes_as("\\\\\na\n===", &unwrap, "# \\ a\n");
    parsed_writes_as("_\\\\\n_\n===", &unwrap, "# \\_\\ \\_\n");
    let heading = |depth, value: &str| {
        Node::Heading(Heading {
            depth,
            position: None,
            children: vec![text(value)],
        })
    };
    let block = |child| {
        Node::Root(Root {
            children: vec![child],
            position: None,
        })
    };
    writes_as(&block(heading(1, "a\\\nb")), &unwrap, "# a\\ b\n");
    writes_as(&block(heading(2, "\\\n#")), &unwrap, "## \\ \\#\n");
    let item = block(Node::List(List {
        ordered: false,
        start: None,
        spread: false,
        position: None,
        children: vec![Node::ListItem(ListItem {
            spread: false,
            checked: None,
            position: None,
            children: vec![heading(1, "\\\na")],
        })],
    }));
    writes_as(&item, &unwrap, "- # \\ a\n");
}

#[test]
fn setext_headings_unwrapped_are_stable() {
    // Every heading of up to four of these characters, under each pair of markers.
    let alphabet = ['*', '_', 'a', '\\', ' ', '\n', '#'];
    for (emphasis, strong) in [('*', '*'), ('_', '_'), ('*', '_'), ('_', '*')] {
        let options = MarkdownOptions {
            emphasis,
            strong,
            line_wrapping: LineWrapping::Unwrap,
            ..MarkdownOptions::default()
        };
        for length in 0..=4u32 {
            for code in 0..alphabet.len().pow(length) {
                let mut rest = code;
                let mut source = String::new();
                for _ in 0..length {
                    source.push(alphabet[rest % alphabet.len()]);
                    rest /= alphabet.len();
                }
                source.push_str("\n===");
                let document = Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
                let outcome = round_trip(document, &options, false);
                assert!(
                    matches!(outcome, Outcome::Pass | Outcome::Unwritable(_)),
                    "{:?} with {:?} and {:?} should read back as itself",
                    source,
                    emphasis,
                    strong
                );
            }
        }
    }
}

/// Attention in link text next to a URL literal, which no longer starts in link text while its
/// opener is pending (#295), is written so that it reads back as the same tree.
#[test]
fn attention_next_to_urls_in_link_text() {
    for source in [
        "[*http://a.&#x62;_&#x61;_*](u)",
        "[**http://a.&#x62;__&#x61;__**](u)",
        "[*http://a.&#x62;*&#x61;](u)",
        "[x http://a.&#x62;*&#x61;*](u)",
    ] {
        same_tree(source);
    }
}
