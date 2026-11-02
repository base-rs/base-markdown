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
        // Normalizing moves the whitespace out of the strikethrough, and the serializer encodes it
        // at the end of the paragraph.
        (
            "x ~a ~&#xA;&#xA;~&#x20;&#xA;~",
            "x ~~a~~&#x20;\n&#xA;&#x20;&#xA;\n",
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
        (
            "| a | b&#xA;&#xA;c |\n| - | - |",
            "| a | b&#xA;&#xA;c |\n| --- | --- |\n",
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
        ("&#xD;a&#xD;", "&#xD;a&#xD;\n"),
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

/// A paragraph built from `children`, where `None` is a soft break, which the fixture cannot build.
fn with_soft_breaks(children: &[Option<&str>]) -> Document<String> {
    let mut document = Node::Root(Root {
        children: vec![paragraph(vec![])],
        position: None,
    })
    .document();
    let paragraph = document.children(document.root()).next().unwrap();
    for child in children {
        let kind = match child {
            Some(value) => EventKind::Text(Value::from(value.to_string())),
            None => EventKind::SoftBreak(Value::Literal("\n")),
        };
        document.append_child(paragraph, kind).unwrap();
    }
    document
}

/// The phrasing of the paragraph of `document` moved into the one cell of a table that replaces it.
fn in_cell(mut document: Document<String>) -> Document<String> {
    let paragraph = document.children(document.root()).next().unwrap();
    let children: Vec<_> = document.children(paragraph).collect();
    let table = document
        .append_child(document.root(), EventKind::Start(Tag::Table))
        .unwrap();
    let head = document
        .append_child(table, EventKind::Start(Tag::TableHead))
        .unwrap();
    let row = document
        .append_child(head, EventKind::Start(Tag::TableRow))
        .unwrap();
    let cell = document
        .append_child(
            row,
            EventKind::Start(Tag::TableCell {
                alignment: base_markdown::AlignKind::None,
                header: true,
            }),
        )
        .unwrap();
    for child in children {
        document.attach(cell, child, None).unwrap();
    }
    document.detach(paragraph).unwrap();
    document
}

#[test]
fn whitespace_after_an_encoded_soft_break_is_encoded() {
    // A soft break after another is written as `&#xA;`, which reads back as text `\n`, after which
    // whitespace is judged as after a raw line ending, so it is encoded on the first write too.
    for (children, expected) in [
        (
            &[Some("a"), None, None, Some(" b")][..],
            "a\n&#xA;&#x20;b\n",
        ),
        (&[Some("a"), None, None, Some("\tb")], "a\n&#xA;&#x9;b\n"),
        (
            &[Some("a"), None, None, None, Some(" b")],
            "a\n&#xA;\n&#x20;b\n",
        ),
        (
            &[Some("a"), None, None, Some("\n b")],
            "a\n&#xA;\n&#x20;b\n",
        ),
    ] {
        let document = with_soft_breaks(children);
        // The second soft break reads back as text, so compare the HTML rather than the trees.
        let mut normalized = document;
        normalized.normalize();
        let output = to_markdown(&normalized).unwrap();
        assert_eq!(output, expected, "{children:?}");
        let mut reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
        reparsed.normalize();
        let render = |document: &Document<String>| {
            let mut html = String::new();
            document
                .render_html(&base_markdown::CompileOptions::default(), &mut html)
                .unwrap();
            html.trim_end().to_owned()
        };
        assert_eq!(render(&reparsed), render(&normalized), "{children:?}");
        assert_eq!(to_markdown(&reparsed).unwrap(), output, "{children:?}");
    }
    // In a table cell every line ending is a reference, which reads back as one, so whitespace
    // next to it stays as it is.
    for (children, expected) in [
        (&[Some("a"), None, Some(" b")][..], "| a&#xA; b |"),
        (&[Some("a"), None, Some("\tb")], "| a&#xA;\tb |"),
        (&[Some("a"), None, None, Some(" b")], "| a&#xA;&#xA; b |"),
        (&[Some("a\n b")], "| a&#xA; b |"),
        (&[Some("a "), None, Some("b")], "| a &#xA;b |"),
        (
            &[Some("a"), None, Some(" "), None, Some("b")],
            "| a&#xA; &#xA;b |",
        ),
        (&[Some("a\n \nb")], "| a&#xA; &#xA;b |"),
        (&[Some("a\t\n\tb")], "| a\t&#xA;\tb |"),
        (&[Some("a"), None, Some("~~~ b")], "| a&#xA;~~~ b |"),
        (&[Some("a\n# b")], "| a&#xA;# b |"),
    ] {
        let mut document = in_cell(with_soft_breaks(children));
        document.normalize();
        let output = to_markdown(&document).unwrap();
        assert_eq!(output, format!("{expected}\n| --- |\n"), "{children:?}");
        let mut reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
        reparsed.normalize();
        assert_eq!(to_markdown(&reparsed).unwrap(), output, "{children:?}");
    }
    // A soft break moved out of emphasis is written the same.
    assert_source("a\n&#xA;&#x20;*b*", "a\n&#xA;&#x20;*b*\n");
    // Literal text that reads `&#xA;` is judged the same, which only adds a reference.
    assert_source("\\&#xA; b", "\\&#xA;&#x20;b\n");
}

/// `document` with the phrasing of its paragraph moved into a heading, or its paragraph moved into
/// nested `containers`.
fn in_block(mut document: Document<String>, containers: &[Tag<'static>]) -> Document<String> {
    let paragraph = document.children(document.root()).next().unwrap();
    document.detach(paragraph).unwrap();
    let mut parent = document.root();
    for container in containers {
        parent = document
            .append_child(parent, EventKind::Start(container.clone()))
            .unwrap();
    }
    if matches!(containers.last(), Some(Tag::Heading(_))) {
        let children: Vec<_> = document.children(paragraph).collect();
        for child in children {
            document.attach(parent, child, None).unwrap();
        }
    } else {
        document.attach(parent, paragraph, None).unwrap();
    }
    document
}

#[test]
fn line_starts_after_an_encoded_soft_break_are_escaped() {
    // A soft break after another is written as `&#xA;`, which reads back as text `\n`, after which
    // text is judged as at the start of a line, so it is escaped on the first write too (#125).
    let list = Tag::List {
        start: None,
        tight: true,
    };
    let item = Tag::Item { block_end: false };
    let blocks: [(&[Tag<'static>], &str, &str); 4] = [
        (&[], "", ""),
        (&[Tag::Heading(1)], "# ", ""),
        (&[list, item], "- ", "  "),
        (&[Tag::BlockQuote], "> ", "> "),
    ];
    for (containers, first, rest) in blocks {
        for (value, written) in [
            ("#", "\\#"),
            (">", "\\>"),
            ("- x", "\\- x"),
            ("+ x", "\\+ x"),
            ("* x", "\\* x"),
            ("1. x", "1\\. x"),
            ("1) x", "1\\) x"),
            ("=", "\\="),
            ("---", "\\---"),
            ("~x", "\\~x"),
            ("~~~", "\\~\\~\\~"),
            ("<div>", "\\<div>"),
            ("  # x", "&#x20; \\# x"),
            ("\t12. x", "&#x9;12\\. x"),
            ("a#", "a#"),
        ] {
            let document = in_block(
                with_soft_breaks(&[Some("a"), None, None, Some(value)]),
                containers,
            );
            let mut normalized = document;
            normalized.normalize();
            let output = to_markdown(&normalized).unwrap();
            // A heading of more than one line is written as a setext heading.
            let lines = format!("{first}a\n{rest}&#xA;{written}\n");
            if first == "# " {
                assert!(
                    output.starts_with(&lines[2..]) && output[lines.len() - 2..].starts_with('='),
                    "{:?}",
                    output
                );
            } else {
                assert_eq!(output, lines, "{containers:?} {value:?}");
            }
            let mut reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
            reparsed.normalize();
            let render = |document: &Document<String>| {
                let mut html = String::new();
                document
                    .render_html(&base_markdown::CompileOptions::default(), &mut html)
                    .unwrap();
                html.trim_end().to_owned()
            };
            assert_eq!(
                render(&reparsed),
                render(&normalized),
                "{containers:?} {value:?}"
            );
            assert_eq!(
                to_markdown(&reparsed).unwrap(),
                output,
                "{containers:?} {value:?}"
            );
        }
    }
    // Literal text that reads `&#xA;` is judged the same, which only adds an escape.
    for (source, expected) in [
        ("\\&#xA;# a", "\\&#xA;\\# a\n"),
        ("a \\&#xA;1. b", "a \\&#xA;1\\. b\n"),
        ("a \\&#xA;- b", "a \\&#xA;\\- b\n"),
        ("&amp;#xA;~~~", "\\&#xA;\\~\\~\\~\n"),
    ] {
        assert_source(source, expected);
    }
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

/// A document with every kind of value that can span lines, one line per item.
const RICH: &[&str] = &[
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

#[test]
fn source_line_endings_are_stored_as_line_feeds() {
    let source = RICH;
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

#[test]
fn carriage_returns_in_constructed_labels_are_written_as_line_feeds() {
    use base_markdown::{NodeSyntax, ReferenceKind};
    // A label of only whitespace, such as ` \r`, or with a blank line cannot be written with any line ending.
    for label in ["l\rm", "l\r\nm", "\\\r", "\rl\r"] {
        let mut document = Document::parse(String::new(), &ParseOptions::normalize()).unwrap();
        let root = document.root();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        let link = document
            .append_child(
                paragraph,
                EventKind::Start(Tag::Link {
                    destination: Value::Literal("/u"),
                    title: None,
                }),
            )
            .unwrap();
        document
            .append_child(link, EventKind::Text(Value::Literal("text")))
            .unwrap();
        document
            .set_syntax(
                link,
                Some(NodeSyntax::Reference {
                    kind: ReferenceKind::Full,
                    label: Value::Value(label.to_string().into()),
                }),
            )
            .unwrap();
        document
            .append_child(
                root,
                EventKind::Definition {
                    label: Value::Value(label.to_string().into()),
                    destination: Value::Literal("/u"),
                    title: None,
                },
            )
            .unwrap();
        let output = to_markdown(&document).unwrap();
        assert!(!output.contains('\r'), "{:?}: {:?}", label, output);
        let reparsed = Document::parse(output.clone(), &ParseOptions::normalize()).unwrap();
        assert_eq!(to_markdown(&reparsed).unwrap(), output, "{label:?}");
        let mut html = String::new();
        reparsed
            .render_html(&Default::default(), &mut html)
            .unwrap();
        assert!(
            html.contains("href=\"/u\""),
            "{:?}: {:?} {:?}",
            label,
            output,
            html
        );
    }
}

#[test]
fn html_line_endings_are_line_feeds_whichever_the_source_used() {
    let mut options = base_markdown::Options::gfm();
    options.parse = ParseOptions::normalize();
    options.compile.allow_dangerous_html = true;
    // Also end without a final line ending, after indented code and after a paragraph.
    for lines in [
        RICH,
        &RICH[..RICH.len() - 1],
        &["    a", "    b"],
        &["a", "b"],
    ] {
        let render = |ending: &str| {
            let source = lines.join(ending);
            let html = base_markdown::to_html_with_options(&source, &options);
            let mut events = String::new();
            base_markdown::render_events(
                &source,
                base_markdown::EventParser::new(&source, &options.parse),
                &options.compile,
                &mut events,
            )
            .unwrap();
            assert_eq!(events, html, "{:?}", source);
            let mut arena = String::new();
            Document::parse(source.as_str(), &options.parse)
                .unwrap()
                .render_html(&options.compile, &mut arena)
                .unwrap();
            assert_eq!(arena, html, "{:?}", source);
            html
        };
        let expected = render("\n");
        assert!(!expected.contains('\r'), "{:?}", expected);
        for ending in ["\r\n", "\r"] {
            assert_eq!(render(ending), expected, "{:?}", ending);
        }
    }
}
