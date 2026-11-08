// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::to_markdown;
use base_markdown::stream::Target;
use base_markdown::stream::{EventKind, Reference, Tag, Text as EventText};
use base_markdown::{Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::rejected;
use support::fixture::to_markdown as to;
use support::fixture::{
    canonical, Break, Definition, FootnoteDefinition, FootnoteReference, Fragment, Heading, Html,
    ImageReference, Link, LinkReference, Node, Paragraph, ReferenceKind, Root, Text,
};

#[test]
fn link_reference() {
    // An empty label does not read back as one, so it is rejected (#192).
    assert!(rejected(
        EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
            kind: base_markdown::ReferenceKind::Full,
            label: EventText::Literal(""),
        })))),
        true
    ));
    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("a"),
            label: None
        }))
        .unwrap(),
        "[][a]\n",
        "should support an `identifier` (nonsensical)"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: Some(String::from("a")),
        }))
        .unwrap(),
        "[][a]\n",
        "should support a `label` (nonsensical)"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Shortcut,
            identifier: String::from("A"),
            label: None
        }))
        .unwrap(),
        "[A]\n",
        "should support `reference_type: ReferenceKind::Shortcut`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("A"),
            label: Some("A".into())
        }))
        .unwrap(),
        "[A][]\n",
        "should support `reference_type: ReferenceKind::Collapsed`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("A"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("A"),
            label: Some("A".into())
        }))
        .unwrap(),
        "[A][A]\n",
        "should support `reference_type: ReferenceKind::Full`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("&"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
            label: Some("&".into())
        }))
        .unwrap(),
        "[&][&]\n",
        "should prefer label over identifier"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("&"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("&amp;"),
            label: None
        }))
        .unwrap(),
        "[&][&]\n",
        "should decode `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::Paragraph(Paragraph {
            children: vec![Node::LinkReference(LinkReference {
                position: None,
                label: None,
                children: vec![Node::Text(Text {
                    value: String::from("&a;"),
                    position: None
                })],
                reference_kind: ReferenceKind::Full,
                identifier: String::from("&b;"),
            })],
            position: None
        }))
        .unwrap(),
        "[\\&a;][&b;]\n",
        "should support incorrect character references"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("a![b](c*d_e[f_g`h<i</j"),
            label: None
        }))
        .unwrap(),
        "[][a!\\[b\\](c*d_e\\[f_g`h<i</j]\n",
        "should not escape unneeded characters in a `reference`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("+"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::from("\\+"),
            label: None
        }))
        .unwrap(),
        "[+][+]\n",
        "should unescape `identifier` if w/o `label`"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Collapsed,
            identifier: String::from("a"),
            label: Some("b".to_string())
        }))
        .unwrap(),
        "[a][b]\n",
        "should use `reference_type: ReferenceKind::Full` if the label doesn't match the reference"
    );
}

/// Serializes `document`, normalized, as `expected`, which reads back as the same tree and serializes the same.
fn assert_round_trip(mut document: Document<String>, options: &ParseOptions, expected: &str) {
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected);
    let mut reparsed = Document::parse(markdown.clone(), options).unwrap();
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{markdown:?}");
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.to_string(),
        position: None,
    })
}

fn definition(identifier: &str) -> Node {
    Node::Definition(Definition {
        position: None,
        url: String::from("/u"),
        title: None,
        identifier: identifier.to_string(),
        label: None,
    })
}

#[test]
fn reference_before_colon_round_trip() {
    for (source, expected) in [
        // A shortcut or footnote reference that starts a paragraph would read as a definition's
        // label before `:` (#122).
        ("[r]\\://a.b\n\n[r]: /u", "[r]\\://a.b\n\n[r]: /u\n"),
        ("[r]\\: x\n\n[r]: /u", "[r]\\: x\n\n[r]: /u\n"),
        ("[r]\\::x\n\n[r]: /u", "[r]\\::x\n\n[r]: /u\n"),
        ("[*r*]\\:x\n\n[*r*]: /u", "[*r*]\\:x\n\n[*r*]: /u\n"),
        ("[a\\]b]\\:x\n\n[a\\]b]: /u", "[a\\]b]\\:x\n\n[a\\]b]: /u\n"),
        // `[^1]:` always starts a footnote definition.
        ("[^1]\\: x\n\n[^1]: n", "[^1]\\: x\n\n[^1]: n\n"),
        ("[^1]\\:x\n\n[^1]: n", "[^1]\\:x\n\n[^1]: n\n"),
        ("[^1]\\: a b\n\n[^1]: n", "[^1]\\: a b\n\n[^1]: n\n"),
        // The destination or title can come from later children.
        (
            "[r]\\: <https://a.b>\n\n[r]: /u",
            "[r]\\: <https://a.b>\n\n[r]: /u\n",
        ),
        ("[r]\\: `x`\n\n[r]: /u", "[r]\\: `x`\n\n[r]: /u\n"),
        ("[r]\\: [a](u)\n\n[r]: /u", "[r]\\: [a](u)\n\n[r]: /u\n"),
        ("[r]\\: x [a](u)\n\n[r]: /u", "[r]: x [a](u)\n\n[r]: /u\n"),
        ("[r]\\: x \"t\"\n\n[r]: /u", "[r]\\: x \"t\"\n\n[r]: /u\n"),
        ("[r]\\:\nx\n\n[r]: /u", "[r]\\:\nx\n\n[r]: /u\n"),
        ("[r]\\: x\ny\n\n[r]: /u", "[r]\\: x\ny\n\n[r]: /u\n"),
        ("[r]\\: <a\n\n[r]: /u", "[r]\\: \\<a\n\n[r]: /u\n"),
        // A `[r]:` without a destination, followed on its line only by a title, is not a definition,
        // so it is written as it is.
        ("[r]\\: a b\n\n[r]: /u", "[r]: a b\n\n[r]: /u\n"),
        ("[r]\\: x \"t\" y\n\n[r]: /u", "[r]: x \"t\" y\n\n[r]: /u\n"),
        ("[r]\\: x *\"t\"*\n\n[r]: /u", "[r]: x *\"t\"*\n\n[r]: /u\n"),
        ("[r]\\:\n\n[r]: /u", "[r]:\n\n[r]: /u\n"),
        ("[r]\\: a)\n\n[r]: /u", "[r]: a)\n\n[r]: /u\n"),
        ("- [r]\\: a b\n\n[r]: /u", "- [r]: a b\n\n[r]: /u\n"),
        // Indentation is not written, and containers start a line.
        ("   [r]\\:x\n\n[r]: /u", "[r]\\:x\n\n[r]: /u\n"),
        ("- [r]\\:x\n\n[r]: /u", "- [r]\\:x\n\n[r]: /u\n"),
        ("> [r]\\:x\n\n[r]: /u", "> [r]\\:x\n\n[r]: /u\n"),
        ("[^1]: [r]\\:x\n\n[r]: /u", "[^1]: [r]\\:x\n\n[r]: /u\n"),
        (
            "[^1]: a\n\n    [r]\\:x\n\n[r]: /u",
            "[^1]: a\n\n    [r]\\:x\n\n[r]: /u\n",
        ),
        // Definitions are taken from the start of a setext heading's content too.
        ("[r]\\:x\nb\n===\n\n[r]: /u", "[r]\\:x\nb\n=\n\n[r]: /u\n"),
        // A task marker is written before the item's content, so a paragraph that starts with one
        // cannot start with a definition.
        ("- [ ] [r]\\:x\n\n[r]: /u", "- [ ] [r]:x\n\n[r]: /u\n"),
        ("- [ ] [r]: /u\n\n[r]: /u", "- [ ] [r]: /u\n\n[r]: /u\n"),
        ("- [ ] [r]: <u>\n\n[r]: /u", "- [ ] [r]: <u>\n\n[r]: /u\n"),
        ("- [x] [^1]: x\n\n[^1]: n", "- [x] [^1]: x\n\n[^1]: n\n"),
        // A footnote definition interrupts a paragraph, so a footnote reference starting any line
        // is escaped before `:`, after a soft or hard break, also in a container or attention.
        ("a\n[^1]\\: u\n\n[^1]: n", "a\n[^1]\\: u\n\n[^1]: n\n"),
        ("a\\\n[^1]\\: u\n\n[^1]: n", "a\\\n[^1]\\: u\n\n[^1]: n\n"),
        ("a  \n[^1]\\: u\n\n[^1]: n", "a\\\n[^1]\\: u\n\n[^1]: n\n"),
        ("> a\n[^1]\\: u\n\n[^1]: n", "> a\n> [^1]\\: u\n\n[^1]: n\n"),
        (
            "- a\n  [^1]\\: x\n\n[^1]: n",
            "- a\n  [^1]\\: x\n\n[^1]: n\n",
        ),
        (
            "- [ ] a\n  [^1]\\: x\n\n[^1]: n",
            "- [ ] a\n  [^1]\\: x\n\n[^1]: n\n",
        ),
        ("*a\n[^1]\\: x*\n\n[^1]: n", "*a\n[^1]\\: x*\n\n[^1]: n\n"),
        ("a&#xA;[^1]\\: x\n\n[^1]: n", "a\n[^1]\\: x\n\n[^1]: n\n"),
        (
            "a\n[^a\\]b]\\: x\n\n[^a\\]b]: n",
            "a\n[^a\\]b]\\: x\n\n[^a\\]b]: n\n",
        ),
        // Not at the start of a line, in a table cell, where line endings are references, or with a
        // label that a footnote definition cannot have, it is written as it is.
        ("a\nb [^1]\\: x\n\n[^1]: n", "a\nb [^1]: x\n\n[^1]: n\n"),
        ("a\n[^1] \\: x\n\n[^1]: n", "a\n[^1] : x\n\n[^1]: n\n"),
        (
            "| a |\n| - |\n| b<br>[^1]\\: x |\n\n[^1]: n",
            "| a |\n| --- |\n| b<br>[^1]: x |\n\n[^1]: n\n",
        ),
        ("a\n[^ 1]\\: x\n\n[^1]: n", "a\n[^ 1]: x\n\n[^1]: n\n"),
        // At the start of a paragraph, such a label can still start a link definition.
        ("[^ 1]\\: x\n\n[^1]: n", "[^ 1]\\: x\n\n[^1]: n\n"),
        ("[^ 1]\\: a b\n\n[^1]: n", "[^ 1]: a b\n\n[^1]: n\n"),
        // Only the start of a paragraph can hold a definition.
        (
            "[r]\\:x\n[s]\\:y\n\n[r]: /u\n[s]: /v",
            "[r]\\:x\n[s]:y\n\n[r]: /u\n\n[s]: /v\n",
        ),
        ("a\n[r]\\:x\n\n[r]: /u", "a\n[r]:x\n\n[r]: /u\n"),
        ("x [r]\\:x\n\n[r]: /u", "x [r]:x\n\n[r]: /u\n"),
        ("*[r]*\\:x\n\n[r]: /u", "*[r]*:x\n\n[r]: /u\n"),
        ("# [r]\\:x\n\n[r]: /u", "# [r]:x\n\n[r]: /u\n"),
        (
            "| a |\n| - |\n| [r]\\:x |\n\n[r]: /u",
            "| a |\n| --- |\n| [r]:x |\n\n[r]: /u\n",
        ),
        // A definition's label is followed by `:` directly, and is one label.
        ("[r] \\:x\n\n[r]: /u", "[r] :x\n\n[r]: /u\n"),
        ("[r][]\\:x\n\n[r]: /u", "[r][]:x\n\n[r]: /u\n"),
        ("[t][r]\\:x\n\n[r]: /u", "[t][r]:x\n\n[r]: /u\n"),
        ("![r]\\:x\n\n[r]: /u", "![r]:x\n\n[r]: /u\n"),
        ("[a](u)\\:x", "[a](u):x\n"),
        // Text brackets at the start of a line are escaped already.
        ("[r]\\:x", "\\[r]:x\n"),
    ] {
        for options in [ParseOptions::default(), ParseOptions::gfm()] {
            let document = Document::parse(source.to_string(), &options).unwrap();
            assert_round_trip(document, &options, expected);
        }
    }
}

#[test]
fn reference_before_colon_tree_round_trip() {
    let shortcut = Node::LinkReference(LinkReference {
        children: vec![text("r")],
        position: None,
        reference_kind: ReferenceKind::Shortcut,
        identifier: String::from("r"),
        label: None,
    });
    let footnote = Node::FootnoteReference(FootnoteReference {
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    let footnote_definition = Node::FootnoteDefinition(FootnoteDefinition {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![text("n")],
            position: None,
        })],
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    // Phrasing directly in the document is written as a paragraph.
    assert_eq!(
        to(&Node::Fragment(Fragment {
            children: vec![shortcut.clone(), text(":x")],
        }))
        .unwrap(),
        "[r]\\:x\n"
    );
    assert_eq!(
        to(&Node::Fragment(Fragment {
            children: vec![shortcut.clone(), text(": a b")],
        }))
        .unwrap(),
        "[r]: a b\n"
    );
    for (tree, expected) in [
        (
            vec![
                Node::Paragraph(Paragraph {
                    children: vec![shortcut.clone(), text("://a.b")],
                    position: None,
                }),
                definition("r"),
            ],
            "[r]\\://a.b\n\n[r]: /u\n",
        ),
        (
            vec![
                Node::Paragraph(Paragraph {
                    children: vec![shortcut.clone(), text(": a b")],
                    position: None,
                }),
                definition("r"),
            ],
            "[r]: a b\n\n[r]: /u\n",
        ),
        (
            vec![
                Node::Heading(Heading {
                    children: vec![shortcut.clone(), text(":a\nb")],
                    position: None,
                    depth: 2,
                }),
                definition("r"),
            ],
            "[r]\\:a\nb\n-\n\n[r]: /u\n",
        ),
        (
            vec![
                Node::Paragraph(Paragraph {
                    children: vec![footnote.clone(), text(": x")],
                    position: None,
                }),
                footnote_definition.clone(),
            ],
            "[^1]\\: x\n\n[^1]: n\n",
        ),
        (
            vec![
                Node::Paragraph(Paragraph {
                    children: vec![text("a\n"), footnote.clone(), text(": x")],
                    position: None,
                }),
                footnote_definition.clone(),
            ],
            "a\n[^1]\\: x\n\n[^1]: n\n",
        ),
        (
            vec![
                Node::Paragraph(Paragraph {
                    children: vec![
                        text("a"),
                        Node::Break(Break { position: None }),
                        footnote.clone(),
                        text(": x"),
                    ],
                    position: None,
                }),
                footnote_definition.clone(),
            ],
            "a\\\n[^1]\\: x\n\n[^1]: n\n",
        ),
    ] {
        let tree = Node::Root(Root {
            children: tree,
            position: None,
        });
        assert_round_trip(tree.document(), &ParseOptions::default(), expected);
    }
}

fn shortcut(label: &str) -> Node {
    Node::LinkReference(LinkReference {
        children: vec![text(label)],
        position: None,
        reference_kind: ReferenceKind::Shortcut,
        identifier: label.to_string(),
        label: None,
    })
}

fn root(children: Vec<Node>) -> Document<String> {
    Node::Root(Root {
        children,
        position: None,
    })
    .document()
}

/// Serializes `document`, normalized, as `expected`, which reads back as the same HTML without
/// a definition it did not have, and serializes the same. Inline HTML that does not start with
/// `<` reads back as text, so the tree is not compared.
fn assert_reads_back(mut document: Document<String>, expected: &str) {
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected);
    let options = base_markdown::Options::default();
    let mut reparsed = Document::parse(markdown.clone(), &options.parse).unwrap();
    reparsed.normalize();
    let html = |document: &Document<String>| {
        let mut html = String::new();
        base_markdown::render_events(
            document.source(),
            document.events(),
            &options.compile,
            &mut html,
        )
        .unwrap();
        html
    };
    assert_eq!(
        html(&reparsed).trim_end(),
        html(&document).trim_end(),
        "{markdown:?}"
    );
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{markdown:?}");
}

/// A label on several lines in a container is what the parser read, without the container
/// prefixes on its lines, which are written again before them, so it is written as authored and
/// does not grow. In a block quote it is written as a reference rather than a link (#146).
#[test]
fn multiline_labels_in_containers_round_trip() {
    for (source, expected) in [
        ("1. [ r\n] u\n\n[r]: /u", "1. [ r\n   ] u\n\n[r]: /u\n"),
        ("1. [ r\n   ] u\n\n[r]: /u", "1. [ r\n   ] u\n\n[r]: /u\n"),
        ("- [a\n  b]\n\n[a b]: /u", "- [a\n  b]\n\n[a b]: /u\n"),
        (
            "- [a][r\n      s]\n\n[r s]: /u",
            "- [a][r\n      s]\n\n[r s]: /u\n",
        ),
        // The item takes two of the tab's columns; the label keeps the other two.
        (
            "- [a][r\n\ts]\n\n[r s]: /u",
            "- [a][r\n    s]\n\n[r s]: /u\n",
        ),
        (
            "- ![a][r\n  s]\n\n[r s]: /u",
            "- ![a][r\n  s]\n\n[r s]: /u\n",
        ),
        (
            "1. [a\n   b][ r\n   ] u\n\n[r]: /u",
            "1. [a\n   b][ r\n   ] u\n\n[r]: /u\n",
        ),
        (
            "- - [a][r\n    s]\n\n[r s]: /u",
            "- - [a][r\n    s]\n\n[r s]: /u\n",
        ),
        (
            "[^1]: [a][r\n    s]\n\n[r s]: /u",
            "[^1]: [a][r\n    s]\n\n[r s]: /u\n",
        ),
        ("> [a][r\n> s]\n\n[r s]: /u", "> [a][r\n> s]\n\n[r s]: /u\n"),
        ("> [r\n> s]\n\n[r s]: /u", "> [r\n> s]\n\n[r s]: /u\n"),
        // The paragraph's span is the reference's.
        ("[r s]: /u\n\n> [a][r\n> s]", "[r s]: /u\n\n> [a][r\n> s]\n"),
        ("> [a][r\ns]\n\n[r s]: /u", "> [a][r\n> s]\n\n[r s]: /u\n"),
        // The quote takes one of the tab's three columns; the label keeps the other two.
        (
            ">\t[a][r\n>\ts]\n\n[r s]: /u",
            "> [a][r\n>   s]\n\n[r s]: /u\n",
        ),
        (
            "> > ![a][r\n> > s]\n\n[r s]: /u",
            "> > ![a][r\n> > s]\n\n[r s]: /u\n",
        ),
        (
            "- > [a][r\n  >   s]\n\n[r s]: /u",
            "- > [a][r\n  >   s]\n\n[r s]: /u\n",
        ),
        ("> [^1\n> ]\n\n[^1]: n", "> [^1\n> ]\n\n[^1]: n\n"),
        // Outside a container, a label is written as authored.
        (
            "[a][r\n      s]\n\n[r s]: /u",
            "[a][r\n      s]\n\n[r s]: /u\n",
        ),
    ] {
        for options in [ParseOptions::default(), ParseOptions::gfm()] {
            let document = Document::parse(source.to_string(), &options).unwrap();
            assert_round_trip(document, &options, expected);
        }
    }
}

/// Set the inferred label of the first link in `document` back, and return it.
fn set_label_back(document: &mut Document<String>) -> String {
    use base_markdown::stream::{EventKind, Reference, Tag, Text as Value};
    let link = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Link { .. })
            )
        })
        .expect("the reference is read");
    let EventKind::Start(Tag::Link(Target::Reference(reference))) =
        document.node(link).unwrap().kind.clone()
    else {
        panic!("a reference has its reference");
    };
    let Reference { kind, label } = *reference;
    let label = label.resolve(document.source()).unwrap().to_string();
    support::fixture::set_reference(document, link, kind, Value::from(label.clone())).unwrap();
    label
}

/// The label of a parsed reference in a block quote is what the parser read, without the `>`
/// prefixes of the lines it continues on, so one the parser read at its limit of 999 bytes can be
/// set back as it is (#146).
#[test]
fn parsed_labels_in_block_quotes_can_be_set_back() {
    for depth in 1..=3 {
        let prefix = "> ".repeat(depth);
        let source = format!(
            "{prefix}[x][{}\n{prefix}{}]\n\n[{}\n{}]: /u",
            "a".repeat(500),
            "b".repeat(499),
            "a".repeat(500),
            "b".repeat(499),
        );
        let mut document = Document::parse(source.clone(), &ParseOptions::default()).unwrap();
        let label = set_label_back(&mut document);
        assert_eq!(label, format!("{}\n{}", "a".repeat(500), "b".repeat(499)));
        assert_round_trip(document, &ParseOptions::default(), &format!("{source}\n"));
    }
    // Spread over many lines, the prefixes alone would pass the limit.
    let source = format!(
        "> [x][{}]\n\n[{}]: /u",
        vec!["a"; 400].join("\n> "),
        vec!["a"; 400].join(" ")
    );
    let mut document = Document::parse(source.clone(), &ParseOptions::default()).unwrap();
    assert_eq!(set_label_back(&mut document), vec!["a"; 400].join("\n"));
    assert_round_trip(document, &ParseOptions::default(), &format!("{source}\n"));
}

/// A shortcut reference followed by a node that starts with `[` would read back as one full
/// reference with what follows as its label, so it is written collapsed (#146).
#[test]
fn shortcut_references_before_a_bracket_round_trip() {
    let link = Node::Link(Link {
        children: vec![text("a")],
        position: None,
        url: String::from("/l"),
        title: None,
    });
    let footnote = Node::FootnoteReference(FootnoteReference {
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    let footnote_definition = Node::FootnoteDefinition(FootnoteDefinition {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![text("n")],
            position: None,
        })],
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    let image = Node::ImageReference(ImageReference {
        position: None,
        alt: String::from("r"),
        reference_kind: ReferenceKind::Shortcut,
        identifier: String::from("r"),
        label: None,
    });
    for (children, expected) in [
        (
            vec![shortcut("r"), shortcut("r"), text(": /u")],
            "[r][][r]: /u\n\n[r]: /u\n\n[s]: /u\n",
        ),
        (
            vec![shortcut("r"), shortcut("s")],
            "[r][][s]\n\n[r]: /u\n\n[s]: /u\n",
        ),
        (
            vec![shortcut("r"), shortcut("s"), shortcut("r")],
            "[r][][s][][r]\n\n[r]: /u\n\n[s]: /u\n",
        ),
        (
            vec![shortcut("r"), link],
            "[r][][a](/l)\n\n[r]: /u\n\n[s]: /u\n",
        ),
        (
            vec![shortcut("r"), footnote.clone()],
            "[r][][^1]\n\n[r]: /u\n\n[s]: /u\n\n[^1]: n\n",
        ),
        (
            vec![image, shortcut("r")],
            "![r][][r]\n\n[r]: /u\n\n[s]: /u\n",
        ),
        // Text escapes its `[`, and other nodes start with something else.
        (
            vec![shortcut("r"), text("[s]")],
            "[r]\\[s]\n\n[r]: /u\n\n[s]: /u\n",
        ),
        (
            vec![footnote, shortcut("r")],
            "[^1][r]\n\n[r]: /u\n\n[s]: /u\n\n[^1]: n\n",
        ),
    ] {
        let mut tree = vec![Node::Paragraph(Paragraph {
            children,
            position: None,
        })];
        tree.extend([definition("r"), definition("s")]);
        if expected.contains("[^1]: n") {
            tree.push(footnote_definition.clone());
        }
        assert_round_trip(root(tree), &ParseOptions::default(), expected);
    }
}

/// The `:` after a reference that starts a paragraph, or a footnote reference that starts a line,
/// is escaped whatever node writes it, such as inline HTML (#146).
#[test]
fn reference_before_colon_from_other_nodes_reads_back() {
    let html = |value: &str| {
        Node::Html(Html {
            value: value.to_string(),
            position: None,
        })
    };
    let footnote = Node::FootnoteReference(FootnoteReference {
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    let footnote_definition = Node::FootnoteDefinition(FootnoteDefinition {
        children: vec![Node::Paragraph(Paragraph {
            children: vec![text("n")],
            position: None,
        })],
        position: None,
        identifier: String::from("1"),
        label: None,
    });
    let paragraph = |children| {
        Node::Paragraph(Paragraph {
            children,
            position: None,
        })
    };
    for (tree, expected) in [
        (
            vec![
                paragraph(vec![shortcut("r"), html(": /u")]),
                definition("r"),
            ],
            "[r]\\: /u\n\n[r]: /u\n",
        ),
        // Not a definition, so written as it is.
        (
            vec![
                paragraph(vec![shortcut("r"), html(": a b")]),
                definition("r"),
            ],
            "[r]: a b\n\n[r]: /u\n",
        ),
        (
            vec![
                paragraph(vec![footnote.clone(), html(":x")]),
                footnote_definition.clone(),
            ],
            "[^1]\\:x\n\n[^1]: n\n",
        ),
        (
            vec![
                paragraph(vec![text("a\n"), footnote.clone(), html(":x")]),
                footnote_definition.clone(),
            ],
            "a\n[^1]\\:x\n\n[^1]: n\n",
        ),
        (
            vec![
                paragraph(vec![text("a "), footnote, html(":x")]),
                footnote_definition,
            ],
            "a [^1]:x\n\n[^1]: n\n",
        ),
    ] {
        assert_reads_back(root(tree), expected);
    }
}

/// A label is read up to 999 bytes, so a longer one cannot be written, and it is rejected where
/// it is assigned (#146). One of 999 bytes as written round-trips.
#[test]
fn labels_up_to_the_parser_limit_round_trip() {
    for label in [
        "a".repeat(999),
        format!("{}\nb", "a".repeat(998)),
        format!("{}\\]", "a".repeat(997)),
    ] {
        let source = format!("[{label}] x\n\n[{label}]: /u");
        let document = Document::parse(source, &ParseOptions::default()).unwrap();
        let expected = format!("[{label}] x\n\n[{label}]: /u\n");
        assert_round_trip(document, &ParseOptions::default(), &expected);
    }
    let source = format!("[^{0}] x\n\n[^{0}]: n", "a".repeat(999));
    let document = Document::parse(source.clone(), &ParseOptions::default()).unwrap();
    assert_round_trip(document, &ParseOptions::default(), &format!("{source}\n"));
}

#[test]
fn shortcut_labels_longer_than_a_full_label_round_trip() {
    use base_markdown::stream::{EventKind, Text as Value};
    // The parser reads a shortcut or collapsed label at any size, but not a full one.
    let tabs = "\t".repeat(251);
    for form in ["", "[]"] {
        let source = format!("[a{tabs}b]{form} x\n\n[a b]: /u");
        // The serializer writes one whose text matches its label as it is.
        let document = Document::parse(source.clone(), &ParseOptions::default()).unwrap();
        assert_eq!(to_markdown(&document).unwrap(), format!("{source}\n"));
        assert!(!document.is_normalized());
        // Normalizing cannot tell whether the text will match, as that depends on how the
        // serializer writes it, so it makes one that could not be written full a resource.
        assert_round_trip(
            document,
            &ParseOptions::default(),
            &format!("[a{tabs}b](/u) x\n\n[a b]: /u\n"),
        );
        // Once its text no longer matches its label, it cannot be written full, so the
        // serializer writes it as a resource too, normalized or not.
        let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
        let text = document
            .descendants(document.root())
            .find(|&id| matches!(document.node(id).unwrap().kind, EventKind::Text(_)))
            .unwrap();
        document
            .set_kind(text, EventKind::Text(Value::Literal("c")))
            .unwrap();
        assert_eq!(to_markdown(&document).unwrap(), "[c](/u) x\n\n[a b]: /u\n");
        assert_round_trip(
            document,
            &ParseOptions::default(),
            "[c](/u) x\n\n[a b]: /u\n",
        );
    }
    // An image is made a resource the same way.
    let source = format!("![a{tabs}b] x\n\n[a b]: /u");
    let document = Document::parse(source, &ParseOptions::default()).unwrap();
    assert!(!document.is_normalized());
    assert_round_trip(
        document,
        &ParseOptions::default(),
        &format!("![a{tabs}b](/u) x\n\n[a b]: /u\n"),
    );
}
