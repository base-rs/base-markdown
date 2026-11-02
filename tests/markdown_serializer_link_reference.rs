// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::markdown::to_markdown;
use base_markdown::{Document, ParseOptions};
use pretty_assertions::assert_eq;
use support::fixture::to_markdown as to;
use support::fixture::{
    canonical, Definition, FootnoteDefinition, FootnoteReference, Fragment, Heading, LinkReference,
    Node, Paragraph, ReferenceKind, Root, Text,
};

#[test]
fn link_reference() {
    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: Vec::new(),
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "[][]\n",
        "should support a link reference (nonsensical"
    );

    assert_eq!(
        to(&Node::LinkReference(LinkReference {
            children: vec![Node::Text(Text {
                value: String::from("a"),
                position: None
            })],
            position: None,
            reference_kind: ReferenceKind::Full,
            identifier: String::new(),
            label: None
        }))
        .unwrap(),
        "[a][]\n",
        "should support `children`"
    );

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
fn assert_round_trip(mut document: Document<String>, expected: &str) {
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, expected);
    let mut reparsed = Document::parse(markdown.clone(), &ParseOptions::default()).unwrap();
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
        ("- [ ] [r]\\:x\n\n[r]: /u", "- [ ] [r]\\:x\n\n[r]: /u\n"),
        ("> [r]\\:x\n\n[r]: /u", "> [r]\\:x\n\n[r]: /u\n"),
        ("[^1]: [r]\\:x\n\n[r]: /u", "[^1]: [r]\\:x\n\n[r]: /u\n"),
        (
            "[^1]: a\n\n    [r]\\:x\n\n[r]: /u",
            "[^1]: a\n\n    [r]\\:x\n\n[r]: /u\n",
        ),
        // Definitions are taken from the start of a setext heading's content too.
        ("[r]\\:x\nb\n===\n\n[r]: /u", "[r]\\:x\nb\n=\n\n[r]: /u\n"),
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
        let document = Document::parse(source.to_string(), &ParseOptions::default()).unwrap();
        assert_round_trip(document, expected);
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
    ] {
        let tree = Node::Root(Root {
            children: tree,
            position: None,
        });
        assert_round_trip(tree.document(), expected);
    }
}
