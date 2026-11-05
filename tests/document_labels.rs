// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Labels of references, definitions and footnotes: every label the parser reads is one that can
//! be assigned, and normalizing gives each the shape that reads back wherever it is written.

use base_markdown::{
    markdown::{to_markdown, to_markdown_with_options},
    stream::{EventKind, Reference, Tag, Text},
    to_html, Document, LineWrapping, MarkdownOptions, NodeId, ParseOptions, ReferenceKind,
};

/// The label a node is written with, if it has one.
fn label_of<'a>(document: &'a Document<String>, kind: &'a EventKind<'_>) -> Option<&'a str> {
    let label = match kind {
        EventKind::Start(
            Tag::Link {
                reference: Some(reference),
                ..
            }
            | Tag::Image {
                reference: Some(reference),
                ..
            },
        ) => &reference.label,
        EventKind::Definition { label, .. } => label,
        EventKind::FootnoteReference { identifier, label }
        | EventKind::Start(Tag::FootnoteDefinition { identifier, label }) => {
            label.as_ref().unwrap_or(identifier)
        }
        _ => return None,
    };
    Some(label.resolve(document.source()).unwrap())
}

/// The nodes of `document` with a label, in document order.
fn labelled(document: &Document<String>) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|&id| label_of(document, &document.node(id).unwrap().kind).is_some())
        .collect()
}

/// The labels of `document`, in document order.
fn labels(document: &Document<String>) -> Vec<String> {
    labelled(document)
        .into_iter()
        .map(|id| {
            label_of(document, &document.node(id).unwrap().kind)
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn parse(source: &str) -> Document<String> {
    Document::parse(source.to_owned(), &ParseOptions::default()).unwrap()
}

/// Normalize `document`, which then reads back from its Markdown as the same labels and the same
/// Markdown, and return that Markdown.
fn normalized(mut document: Document<String>) -> String {
    document.normalize();
    assert!(document.is_normalized());
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = parse(&markdown);
    reparsed.normalize();
    assert_eq!(labels(&reparsed), labels(&document), "{:?}", markdown);
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
    markdown
}

/// Every label the parser reads can be assigned back to its node: the rule that rejects a label
/// at assignment admits each the parser produces (#192). Normalizing then gives labels that read
/// back as themselves.
#[test]
fn every_parsed_label_is_valid() {
    // Pieces of labels, of what starts a block on a line of one, and of what interrupts one.
    let pieces = [
        "a", "b", "ΐ", "\\", "\\]", "\\[", "\\\\", "]", "[", "\t", " ", "  ", "\n", "\r\n", "\n\n",
        "\n\t", "\n    ", "\n  \t", "# ", "#", "> ", "- ", "-", "+ ", "* ", "1. ", "2. ", "```",
        "~~~", "===", "---", "***", "<div>", "<span>", "<!--", "|", "| - |", "^", ":", "\t\t\t\t",
    ];
    // What the lines of a document start with.
    let containers = ["", "", "> ", "- ", "1. ", "  ", "\t", "   "];
    // A linear congruential generator, so each run tests the same documents.
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = |bound: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize % bound
    };
    let mut seen = 0;
    for _ in 0..2000 {
        let mut label = String::new();
        for _ in 0..1 + next(8) {
            label.push_str(pieces[next(pieces.len())]);
        }
        let mut source = String::new();
        for _ in 0..1 + next(6) {
            let construct = match next(7) {
                0 => format!("[{}]: /u", label),
                1 => format!("[x][{}]", label),
                2 => format!("[{}][]", label),
                3 => format!("[{}]", label),
                4 => format!("![x][{}]", label),
                5 => format!("[^{}]", label),
                _ => format!("[^{}]: x", label),
            };
            let container = containers[next(containers.len())];
            // Each line after the first continues the container or is lazy.
            for (index, line) in construct.split('\n').enumerate() {
                if index > 0 {
                    source.push('\n');
                }
                if index == 0 || next(2) == 0 {
                    source.push_str(container);
                }
                source.push_str(line);
            }
            source.push_str(["\n", "\n\n", " ", "\n===\n", "\n# a\n", "\n| - |\n"][next(6)]);
        }
        let mut document = parse(&source);
        for id in labelled(&document) {
            let kind = document.node(id).unwrap().kind.clone();
            document
                .set_kind(id, kind.clone())
                .unwrap_or_else(|error| panic!("{:?}: {:?}: {:?}", source, kind, error));
            seen += 1;
        }
        document.normalize();
        assert!(document.is_normalized(), "{:?}", source);
        // A definition whose label starts with `^` reads back as a footnote definition where it
        // starts a block, which it does unless it continues another definition lazily.
        let footnote_like = document.descendants(document.root()).any(|id| {
            matches!(
                &document.node(id).unwrap().kind,
                EventKind::Definition { label, .. }
                    if label.resolve(document.source()).unwrap().starts_with('^')
            )
        });
        if !footnote_like {
            let markdown = to_markdown(&document).unwrap();
            let mut reparsed = parse(&markdown);
            reparsed.normalize();
            assert_eq!(
                labels(&reparsed),
                labels(&document),
                "{:?} {:?}",
                source,
                markdown
            );
        }
        for id in labelled(&document) {
            let kind = document.node(id).unwrap().kind.clone();
            document
                .set_kind(id, kind.clone())
                .unwrap_or_else(|error| panic!("{:?}: {:?}: {:?}", source, kind, error));
        }
    }
    assert!(seen > 1000, "{}", seen);
}

/// The serializer writes a label as it is, as escaping it would change it: a line of one that
/// starts with what would start a block only where it is not, such as `#b`, reads back (#192).
#[test]
fn labels_are_written_as_they_are() {
    for source in [
        "[a #b]: /u\n\n[x][a\n#b]\n",
        "[a =b]: /u\n\n[x][a\n=b]\n",
        "[a <b]: /u\n\n[x][a\n<b]\n",
        "[a 2. b]: /u\n\n[x][a\n2. b]\n",
        "[a\n#b]: /u\n\n[x][a #b]\n",
        "[a\\[b]: /u\n\n[x][a\\[b]\n",
        "[^a]: x\n\n[^a\n]\n",
    ] {
        let document = parse(source);
        assert_eq!(to_markdown(&document).unwrap(), source);
        assert_eq!(normalized(document), source);
        assert_eq!(
            to_html(source),
            to_html(&to_markdown(&parse(source)).unwrap())
        );
    }
}

/// A tab in a label spans up to four columns, as many as its column needs, so normalizing makes
/// each a space in a label that would not fit everywhere, and keeps what it matches (#192).
#[test]
fn normalizing_spaces_the_tabs_of_labels_that_do_not_fit_everywhere() {
    let tabs = "\t".repeat(249);
    let spaces = " ".repeat(249);
    // Each label is read where its first tab spans fewer than four columns.
    let source = format!("[{tabs}aaaa]: /u\n\n[xx][{tabs}aaaa]\n\n[^{tabs}aaaa]\n\n[^aaaa]: x\n");
    let document = parse(&source);
    assert_eq!(
        labels(&document),
        [
            format!("{tabs}aaaa"),
            format!("{tabs}aaaa"),
            format!("{tabs}aaaa"),
            "aaaa".into(),
        ]
    );
    assert!(!document.is_normalized());
    let html = to_html(&source);
    let markdown = normalized(document);
    assert_eq!(
        markdown,
        format!("[{spaces}aaaa]: /u\n\n[xx][{spaces}aaaa]\n\n[^{spaces}aaaa]\n\n[^aaaa]: x\n")
    );
    assert_eq!(to_html(&markdown), html);
    // A label that fits everywhere keeps its tabs.
    let source = "[a\tb]: /u\n\n[x][a\tb]\n";
    assert!(parse(source).is_normalized());
    assert_eq!(normalized(parse(source)), source);
}

/// A line of a label after the first that starts with a tab starts a block where the tab spans
/// fewer than four columns, so normalizing makes its tabs four spaces each (#192).
#[test]
fn normalizing_indents_label_lines_that_start_a_block_at_some_columns() {
    let source = "[a\n\t# b]: /u\n[a #b]: /v\n\n[x][a\n\t# b] [y][a\n\t#b]\n";
    let document = parse(source);
    assert!(!document.is_normalized());
    let markdown = normalized(document);
    assert_eq!(
        markdown,
        "[a\n    # b]: /u\n\n[a #b]: /v\n\n[x][a\n    # b] [y][a\n\t#b]\n"
    );
    // Written in a list item, where the tab would span two columns, it still reads back.
    let mut document = parse(&format!("- {}", markdown.replace('\n', "\n  ")));
    document.normalize();
    assert_eq!(
        labels(&document),
        ["a\n    # b", "a #b", "a\n    # b", "a\n\t#b"]
    );
}

/// A label is written as it is, so a heading that holds one with a line ending is written setext,
/// even when soft breaks are unwrapped (#192).
#[test]
fn headings_with_labels_that_hold_line_endings_are_setext() {
    let source = "[a b]: /u\n\n[x][a\nb]\n===\n";
    let document = parse(source);
    let expected = "[a b]: /u\n\n[x][a\nb]\n==\n";
    assert_eq!(to_markdown(&document).unwrap(), expected);
    let unwrap = MarkdownOptions {
        line_wrapping: LineWrapping::Unwrap,
        ..MarkdownOptions::default()
    };
    assert_eq!(
        to_markdown_with_options(&document, &unwrap).unwrap(),
        expected
    );
    assert_eq!(to_html(expected), to_html(source));
    assert_eq!(normalized(document), expected);
    // A footnote call's label too.
    let mut document = parse("[^a]\n===\n\n[^a]: x\n");
    let call = labelled(&document)[0];
    document
        .set_kind(
            call,
            EventKind::FootnoteReference {
                identifier: Text::Literal("A"),
                label: Some(Text::Literal("a\n")),
            },
        )
        .unwrap();
    assert_eq!(normalized(document), "[^a\n]\n=\n\n[^a]: x\n");
    // In an item of a tight list, a setext heading after a paragraph needs a blank line, so
    // normalizing makes the list loose.
    let mut document = parse("- p\n\n  [x][a\n  b]\n  ===\n\n[a b]: /u\n");
    let list = document.children(document.root()).next().unwrap();
    document
        .set_kind(
            list,
            EventKind::Start(Tag::List {
                tight: true,
                start: None,
            }),
        )
        .unwrap();
    assert!(!document.is_normalized());
    assert_eq!(
        normalized(document),
        "- p\n\n  [x][a\n  b]\n  ==\n\n[a b]: /u\n"
    );
}

/// An ATX heading or a table cell is written on one line, so normalizing makes each run of
/// whitespace with a line ending in a label there one space, which matches the same (#192).
#[test]
fn normalizing_spaces_line_endings_in_labels_written_on_one_line() {
    let reference = |label: &str| {
        EventKind::Start(Tag::Link {
            destination: Text::Literal("/u"),
            title: None,
            reference: Some(Box::new(Reference {
                kind: ReferenceKind::Full,
                label: Text::from(label.to_owned()),
            })),
        })
    };
    let mut document = parse("### [x][a b]\n\n[a b]: /u\n");
    let link = labelled(&document)[0];
    document.set_kind(link, reference("a\n  b")).unwrap();
    assert!(!document.is_normalized());
    assert_eq!(normalized(document), "### [x][a b]\n\n[a b]: /u\n");
    // A label that is then too long cannot be written, so the link is a resource.
    let mut document = parse("### [x][a b]\n\n[a b]: /u\n");
    let long = format!("{}\n{}", "a".repeat(500), "a".repeat(499));
    document.set_kind(link, reference(&long)).unwrap();
    assert_eq!(normalized(document), "### [x](/u)\n\n[a b]: /u\n");
    // A footnote call in a table cell.
    let mut document = parse("| [^a] |\n| - |\n\n[^a]: x\n");
    let call = labelled(&document)[0];
    document
        .set_kind(
            call,
            EventKind::FootnoteReference {
                identifier: Text::Literal("A"),
                label: Some(Text::Literal("\na\n")),
            },
        )
        .unwrap();
    assert!(!document.is_normalized());
    assert_eq!(normalized(document), "| [^ a ] |\n| --- |\n\n[^a]: x\n");
}
