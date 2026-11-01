// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Line endings in text that a raw line ending would change: a second one in a row, which is a
//! blank line, one before the end of a block, one in a table cell, and a CR, which output line
//! endings would turn into LF. Parsed values store LF for every source line ending, so a CR in a
//! value comes from a character reference.

mod support;
use base_markdown::{
    markdown::to_markdown,
    stream::{EventKind, Tag, Text as Value},
    Document, ParseOptions,
};
use pretty_assertions::assert_eq;
use std::time::{Duration, Instant};
use support::fixture::{
    canonical, AlignKind, Break, Node, Paragraph, Root, Table, TableCell, TableRow, Text,
};

/// Serialize the normalized `document`, and check that the output is `expected`, that it parses
/// and normalizes back to the same tree, and that serializing that gives the same output.
fn assert_round_trip(mut document: Document<String>, expected: &str, label: &str) {
    document.normalize();
    let output = to_markdown(&document).unwrap();
    assert_eq!(output, expected, "{label}");
    let mut reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
    reparsed.normalize();
    assert!(reparsed.is_normalized(), "{}", label);
    assert_eq!(canonical(&reparsed), canonical(&document), "{label}");
    assert_eq!(to_markdown(&reparsed).unwrap(), output, "{label}");
}

fn assert_source(source: &str, expected: &str) {
    let document = Document::parse(source.to_string(), &ParseOptions::normalize()).unwrap();
    assert_round_trip(document, expected, source);
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn paragraph(children: Vec<Node>) -> Node {
    Node::Paragraph(Paragraph {
        children,
        position: None,
    })
}

fn hard_break() -> Node {
    Node::Break(Break { position: None })
}

fn table(rows: Vec<Vec<&str>>) -> Node {
    let columns = rows[0].len();
    Node::Table(Table {
        children: rows
            .into_iter()
            .map(|row| {
                Node::TableRow(TableRow {
                    children: row
                        .into_iter()
                        .map(|value| {
                            Node::TableCell(TableCell {
                                children: vec![text(value)],
                                position: None,
                            })
                        })
                        .collect(),
                    position: None,
                })
            })
            .collect(),
        position: None,
        align: vec![AlignKind::None; columns],
    })
}

fn assert_tree(node: Node, expected: &str) {
    let label = format!("{node:?}");
    assert_round_trip(node.document(), expected, &label);
}

#[test]
fn issue_repros_round_trip() {
    for (source, expected) in [
        // #112.
        ("a&#xA;&#xA;b", "a\n&#xA;b\n"),
        ("a &#xA;&#xA; b", "a&#x20;\n&#xA;&#x20;b\n"),
        ("x ~~a &#xA;&#xA; b~~", "x ~~a&#x20;\n&#xA;&#x20;b~~\n"),
        (
            "x ~a ~&#xA;&#xA;~&#x20;&#xA;~",
            "x ~~a&#x20;\n&#xA;&#x20;&#xA;~~\n",
        ),
        ("a&#xA;", "a&#xA;\n"),
        ("&#xA;a", "&#xA;a\n"),
        ("*a*&#xA;", "*a*&#xA;\n"),
        // A heading with a line ending is written as setext.
        ("# a&#xA;", "a&#xA;\n======\n"),
        ("- [ ] a&#xA;", "- [ ] a&#xA;\n"),
        // #116.
        ("| a&#xA;b |\n| - |", "| a&#xA;b |\n| --- |\n"),
        ("| a |\n| - |\n| a&#xA;b |", "| a |\n| --- |\n| a&#xA;b |\n"),
        ("a\\\n&#xA;b", "a\\\n&#xA;b\n"),
    ] {
        assert_source(source, expected);
    }
}

#[test]
fn consecutive_line_endings_encode_every_second_one() {
    for (source, expected) in [
        ("a&#xA;&#xA;&#xA;b", "a\n&#xA;\nb\n"),
        ("a&#xA;&#xA;&#xA;&#xA;b", "a\n&#xA;\n&#xA;b\n"),
        ("a&#xA;&#xA;&#xA;&#xA;&#xA;b", "a\n&#xA;\n&#xA;\nb\n"),
        ("a&#xA;&#xA;", "a\n&#xA;\n"),
        ("a&#xA;&#xA;&#xA;", "a\n&#xA;&#xA;\n"),
        // A raw line ending after an encoded one ends no paragraph.
        ("a\n&#xA;\nb", "a\n&#xA;\nb\n"),
        // A soft break and an encoded line ending.
        ("a&#xA;\nb", "a\n&#xA;b\n"),
        ("a\n&#xA;b", "a\n&#xA;b\n"),
        // A backslash before an encoded line ending is escaped once.
        ("a\\\\&#xA;", "a\\\\&#xA;\n"),
        ("a\\\\&#xA;&#xA;b", "a\\\\\n&#xA;b\n"),
    ] {
        assert_source(source, expected);
    }
}

#[test]
fn line_endings_in_containers_and_phrasing() {
    for (source, expected) in [
        // At the start of a paragraph after a block boundary.
        ("p\n\n&#xA;a", "p\n\n&#xA;a\n"),
        ("- &#xA;a", "- &#xA;a\n"),
        ("> &#xA;a", "> &#xA;a\n"),
        // Setext headings.
        ("a&#xA;&#xA;b\n===", "a\n&#xA;b\n======\n"),
        ("a&#xA;\n===", "a&#xA;\n======\n"),
        // Container prefixes are written between the lines.
        ("- a&#xA;&#xA;b", "- a\n  &#xA;b\n"),
        ("1. a&#xA;&#xA;&#xA;b", "1. a\n   &#xA;\n   b\n"),
        ("> a&#xA;&#xA;b", "> a\n> &#xA;b\n"),
        ("> - a&#xA;&#xA;b", "> - a\n>   &#xA;b\n"),
        ("- [ ] a&#xA;&#xA;", "- [ ] a\n  &#xA;\n"),
        // Links, image text and attention in the middle of a paragraph.
        ("x [a&#xA;&#xA;b](u) y", "x [a\n&#xA;b](u) y\n"),
        ("x ![a&#xA;&#xA;b](u) y", "x ![a\n&#xA;b](u) y\n"),
        ("x *a&#xA;&#xA;b* y", "x *a\n&#xA;b* y\n"),
        ("x **a&#xA;&#xA;b** y", "x **a\n&#xA;b** y\n"),
        ("x *a\\\n&#xA;b*", "x *a\\\n&#xA;b*\n"),
        ("*a*\\\n&#xA;b", "*a*\\\n&#xA;b\n"),
        ("a\\\n&#xA;&#xA;b", "a\\\n&#xA;\nb\n"),
        // Table cells.
        (
            "| a | b&#xA;&#xA; |\n| - | - |",
            "| a | b&#xA;&#xA; |\n| --- | --- |\n",
        ),
        ("| a\\\\&#xA;b |\n| - |", "| a\\\\&#xA;b |\n| --- |\n"),
    ] {
        assert_source(source, expected);
    }
}

#[test]
fn carriage_returns_are_encoded() {
    // Output line endings are LF, so a CR in text is written as a reference, and a CRLF keeps its
    // LF raw: the CR before it is never the one that ends the line.
    for (source, expected) in [
        ("a&#xD;&#xA;&#xD;&#xA;b", "a&#xD;\n&#xD;\nb\n"),
        ("a&#xD;&#xA;b", "a&#xD;\nb\n"),
        ("a&#xA;&#xD;&#xA;b", "a\n&#xD;\nb\n"),
        ("a&#xD;&#xD;b", "a&#xD;&#xD;b\n"),
        ("a&#xA;&#xD;b", "a\n&#xD;b\n"),
        ("a&#xD;&#xA;&#xA;b", "a&#xD;\n&#xA;b\n"),
        ("a&#xD;", "a&#xD;\n"),
        ("a&#xD;&#xA;", "a&#xD;&#xA;\n"),
        ("a\\\n&#xD;b", "a\\\n&#xD;b\n"),
        ("a\\\n&#xD;&#xA;b", "a\\\n&#xD;\nb\n"),
        ("| a&#xD;&#xA;b |\n| - |", "| a&#xD;&#xA;b |\n| --- |\n"),
        ("- a&#xD;&#xA;&#xD;&#xA;b", "- a&#xD;\n  &#xD;\n  b\n"),
    ] {
        assert_source(source, expected);
    }
}

#[test]
fn constructed_trees_round_trip() {
    // The hard break writes its own line ending, so the one after it is encoded.
    assert_tree(
        paragraph(vec![text("a"), hard_break(), text("\nb")]),
        "a\\\n&#xA;b\n",
    );
    assert_tree(
        paragraph(vec![text("a"), hard_break(), text("\r\nb")]),
        "a\\\n&#xD;\nb\n",
    );
    assert_tree(paragraph(vec![text("a\n\nb")]), "a\n&#xA;b\n");
    assert_tree(paragraph(vec![text("a\r\n\r\nb")]), "a&#xD;\n&#xD;\nb\n");
    assert_tree(
        Node::Root(Root {
            children: vec![table(vec![vec!["a\nb", "c"], vec!["d", "e\r\nf"]])],
            position: None,
        }),
        "| a&#xA;b | c |\n| --- | --- |\n| d | e&#xD;&#xA;f |\n",
    );
}

/// The values of `kind` that hold content, which excludes a separator, the line ending the HTML
/// renderer writes between blocks.
fn values<'a>(kind: &'a EventKind<'a>) -> Vec<&'a Value<'a>> {
    match kind {
        EventKind::Text(value)
        | EventKind::SoftBreak(value)
        | EventKind::Math(value)
        | EventKind::Html { value, .. }
        | EventKind::Opaque { value, .. }
        | EventKind::FootnoteReference(value)
        | EventKind::Start(Tag::FootnoteDefinition(value)) => vec![value],
        EventKind::Code { value, language }
        | EventKind::CodeBlock {
            value, language, ..
        } => IntoIterator::into_iter([Some(value), language.as_ref()])
            .flatten()
            .collect(),
        EventKind::Start(Tag::Link { destination, title } | Tag::Image { destination, title }) => {
            IntoIterator::into_iter([Some(destination), title.as_ref()])
                .flatten()
                .collect()
        }
        EventKind::Definition {
            label,
            destination,
            title,
        } => IntoIterator::into_iter([Some(label), Some(destination), title.as_ref()])
            .flatten()
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn source_line_endings_are_stored_as_line_feeds() {
    let source = [
        "---",
        "title: a",
        "---",
        "",
        "# a",
        "",
        "b",
        "c  ",
        "d\\",
        "e [f",
        "g] ![h",
        "i](u \"j",
        "k\") [l](u 'm",
        "n') `o",
        "p` <span",
        "q> $r",
        "s$ [^t",
        "u]",
        "",
        "> v",
        "> w",
        "",
        "- x",
        "  y",
        "",
        "```js",
        "z",
        "",
        "```",
        "",
        "    indented",
        "    code",
        "",
        "<div>",
        "html",
        "</div>",
        "",
        "$$",
        "math",
        "$$",
        "",
        "| a |",
        "| - |",
        "| b |",
        "",
        "[f",
        "g]: /d \"title",
        "line\"",
        "",
        "[^t",
        "u]: note",
        "",
    ];
    let options = ParseOptions::normalize();
    let parse = |ending: &str| {
        let mut document = Document::parse(source.join(ending), &options).unwrap();
        document.normalize();
        document
    };
    let expected = to_markdown(&parse("\n")).unwrap();
    // The shortcut reference, whose label has a line ending, stays a shortcut.
    assert!(expected.contains("e [f\ng] ![h"), "{}", expected);
    for ending in ["\r\n", "\r"] {
        let document = parse(ending);
        for event in document.events() {
            let event = event.unwrap();
            for value in values(&event.kind) {
                let value = value.resolve(document.source()).unwrap();
                assert!(
                    !value.contains('\r'),
                    "{:?}: {:?} in {:?}",
                    ending,
                    value,
                    event.kind
                );
            }
        }
        let output = to_markdown(&document).unwrap();
        assert_eq!(output, expected, "{:?}", ending);
        assert!(!output.contains("&#xD;"), "{:?}", ending);
        assert_eq!(
            canonical(&document),
            canonical(&parse("\n")),
            "{:?}",
            ending
        );
    }
}

#[test]
fn carriage_return_references_round_trip_in_text_and_titles() {
    for (source, expected) in [
        ("a&#xD;b", "a&#xD;b\n"),
        ("[a](u \"t&#xD;\")", "[a](u \"t&#xD;\")\n"),
        ("![a&#xD;b](u 't&#xD;')", "![a&#xD;b](u \"t&#xD;\")\n"),
        ("[a]: u \"t&#xD;\"\n\n[a]", "[a]: u \"t&#xD;\"\n\n[a]\n"),
        ("[a]: u \"t&#xD;\"\r\n\r\n[a]", "[a]: u \"t&#xD;\"\n\n[a]\n"),
        ("# a&#xD;b", "# a&#xD;b\n"),
    ] {
        assert_source(source, expected);
    }
    // Unwrapping soft breaks keeps a CR, which is not a line ending in a value.
    assert_eq!(
        base_markdown::normalize_markdown("a&#xD;\r\nb").unwrap(),
        "a&#xD; b\n"
    );
}

/// Assert that four times the input takes less than twelve times as long, after a warm-up, taking
/// the fastest of five runs of each size.
fn assert_linear(label: &str, mut time: impl FnMut(usize) -> Duration) {
    time(1 << 8);
    let mut small = Duration::MAX;
    let mut large = Duration::MAX;
    for _ in 0..5 {
        small = small.min(time(1 << 12));
        large = large.min(time(1 << 14));
    }
    assert!(
        large < small * 12,
        "{}: 4x input took {:?} against {:?}",
        label,
        large,
        small
    );
}

#[test]
fn encoding_line_endings_is_linear() {
    let options = ParseOptions::normalize();
    type Build = fn(usize) -> String;
    let cases: [(&str, Build); 5] = [
        ("consecutive line endings", |count| {
            format!("a{}b\n", "&#xA;".repeat(count))
        }),
        ("consecutive CRLFs", |count| {
            format!("a{}b\n", "&#xD;&#xA;".repeat(count))
        }),
        ("line endings between words", |count| {
            "a&#xA;&#xA;".repeat(count)
        }),
        ("table rows", |count| {
            format!("| a |\n| - |\n{}", "| a&#xA;&#xA;b |\n".repeat(count))
        }),
        ("table cells", |count| {
            format!(
                "{}|\n{}|\n{}|\n",
                "| a ".repeat(count),
                "| - ".repeat(count),
                "| &#xA;a&#xA;&#xA;b&#xA; ".repeat(count)
            )
        }),
    ];
    for (label, build) in cases {
        assert_linear(label, |count| {
            let mut document = Document::parse(build(count), &options).unwrap();
            document.normalize();
            let started = Instant::now();
            to_markdown(&document).unwrap();
            started.elapsed()
        });
    }
}
