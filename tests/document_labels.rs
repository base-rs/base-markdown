// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Labels of references, definitions and footnotes: every label the parser reads is one that can
//! be assigned, and normalizing gives each the shape that reads back wherever it is written.

mod support;
use base_markdown::stream::Target;
use base_markdown::{
    markdown::{to_markdown, to_markdown_with_options},
    stream::{EventKind, Reference, Tag, Text},
    to_html, Document, LineWrapping, MarkdownOptions, NodeId, ParseOptions, ReferenceKind,
};
use support::linear::timed;

/// The label a node is written with, if it has one.
fn label_of<'a>(document: &'a Document<String>, kind: &'a EventKind<'_>) -> Option<&'a str> {
    let label = match kind {
        EventKind::Start(Tag::Link(target) | Tag::Image(target)) => &target.reference()?.label,
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

/// Documents dense with labels, from a linear congruential generator seeded with `seed`, so each
/// run tests the same documents: labels of pieces that start a block on a line of one, interrupt
/// one or are tabs, in definitions, references and footnotes, in containers whose prefixes, such
/// as `>` before a tab, can consume only part of a tab, with lazy lines.
fn sources(count: usize, seed: u64) -> Vec<String> {
    let pieces = [
        "a", "b", "ΐ", "\\", "\\]", "\\[", "\\\\", "]", "[", "\t", " ", "  ", "\n", "\r\n", "\n\n",
        "\n\t", "\n    ", "\n  \t", "\n\t  ", "# ", "#", "> ", "- ", "-", "+ ", "* ", "1. ", "2. ",
        "```", "~~~", "===", "---", "***", "<div>", "<span>", "<!--", "|", "| - |", "|-|", ":-:",
        "^", ":", "\t\t\t\t", "    ",
    ];
    // What the first line of each construct starts with, and what each line after it does,
    // unless it is lazy.
    let containers = [
        ("", ""),
        ("", ""),
        ("> ", "> "),
        (">", ">"),
        (">", ">\t"),
        ("> ", ">\t"),
        ("- ", "  "),
        ("- ", "\t"),
        ("1. ", "   "),
        ("1. ", "\t"),
        ("  ", "  "),
        ("\t", "\t"),
        ("   ", "   "),
    ];
    let mut state = seed;
    let mut next = |bound: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize % bound
    };
    (0..count)
        .map(|_| {
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
                let (first, rest) = containers[next(containers.len())];
                for (index, line) in construct.split('\n').enumerate() {
                    if index > 0 {
                        source.push('\n');
                        if next(3) > 0 {
                            source.push_str(rest);
                        }
                    } else {
                        source.push_str(first);
                    }
                    source.push_str(line);
                }
                source.push_str(["\n", "\n\n", " ", "\n===\n", "\n# a\n", "\n| - |\n"][next(6)]);
            }
            source
        })
        .collect()
}

/// Assign each label of `document` back to its node, which must accept it, and return how many.
fn reassign(document: &mut Document<String>, source: &str) -> usize {
    let ids = labelled(document);
    for &id in &ids {
        let kind = document.node(id).unwrap().kind.clone();
        document
            .set_kind(id, kind.clone())
            .unwrap_or_else(|error| panic!("{:?}: {:?}: {:?}", source, kind, error));
    }
    ids.len()
}

/// Every label the parser reads can be assigned back to its node: the rule that rejects a label
/// at assignment admits each the parser produces (#192). Normalizing then gives labels that read
/// back as themselves.
#[test]
fn every_parsed_label_is_valid() {
    let mut seen = 0;
    for source in sources(2000, 0x2545_f491_4f6c_dd1d) {
        let mut document = parse(&source);
        seen += reassign(&mut document, &source);
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
            let normalized = to_markdown(&document).unwrap();
            let mut reparsed = parse(&normalized);
            reparsed.normalize();
            assert_eq!(
                labels(&reparsed),
                labels(&document),
                "{:?} {:?}",
                source,
                normalized
            );
        }
        reassign(&mut document, &source);
    }
    assert!(seen > 1000, "{}", seen);
}

/// Every label the parser reads with any options can be assigned back, as each block a line of
/// a label could start can be turned off: under each preset, and the default options with each
/// construct off in turn, no document fails to parse or render, and every label is accepted,
/// before and after normalizing (#192).
#[test]
fn labels_parsed_with_any_options_are_valid() {
    let mut options = vec![
        ParseOptions::default(),
        ParseOptions::gfm(),
        ParseOptions::commonmark(),
    ];
    for index in 0..27 {
        let mut off = ParseOptions::default();
        let constructs = &mut off.constructs;
        *[
            &mut constructs.attention,
            &mut constructs.autolink,
            &mut constructs.block_quote,
            &mut constructs.character_escape,
            &mut constructs.character_reference,
            &mut constructs.code_indented,
            &mut constructs.code_fenced,
            &mut constructs.code_text,
            &mut constructs.definition,
            &mut constructs.frontmatter,
            &mut constructs.autolink_literal,
            &mut constructs.footnote_definition,
            &mut constructs.label_start_footnote,
            &mut constructs.strikethrough,
            &mut constructs.table,
            &mut constructs.task_list_item,
            &mut constructs.hard_break_escape,
            &mut constructs.hard_break_trailing,
            &mut constructs.heading_atx,
            &mut constructs.heading_setext,
            &mut constructs.html_flow,
            &mut constructs.html_text,
            &mut constructs.label_start_image,
            &mut constructs.label_start_link,
            &mut constructs.label_end,
            &mut constructs.list_item,
            &mut constructs.thematic_break,
        ][index] = false;
        options.push(off);
    }
    let mut seen = 0;
    for (index, options) in options.iter().enumerate() {
        for source in sources(150, 0x9e37_79b9_7f4a_7c15 ^ index as u64) {
            let mut document = Document::parse(source.clone(), options)
                .unwrap_or_else(|error| panic!("{:?} {:?}: {:?}", options, source, error));
            let compile = base_markdown::Options::default().compile;
            let mut rendered = String::new();
            let mut renderer =
                base_markdown::EventHtmlRenderer::new(&source, &compile, &mut rendered);
            for event in base_markdown::EventParser::new(&source, options) {
                renderer
                    .event(event.unwrap())
                    .unwrap_or_else(|error| panic!("{:?} {:?}: {:?}", options, source, error));
            }
            let mut html = String::new();
            document
                .render_html(&base_markdown::Options::default().compile, &mut html)
                .unwrap();
            seen += reassign(&mut document, &source);
            document.normalize();
            assert!(document.is_normalized(), "{:?} {:?}", options, source);
            reassign(&mut document, &source);
            to_markdown(&document).unwrap();
        }
    }
    assert!(seen > 2000, "{}", seen);
}

/// A container prefix can consume only part of a tab, and the columns it leaves are part of the
/// line, so the parser writes them into a label as spaces: a line indented four columns in the
/// source is indented four in its label, and continues it however it is written (#192).
#[test]
fn labels_keep_the_columns_of_tabs_that_containers_partly_consume() {
    for (source, label) in [
        (">[\n>\t  ```]:/", "\n    ```"),
        ("> [x][a\n>\t  ```]\n\n[a ```]: /u", "a\n    ```"),
        ("> [x][a\n>\t  # b]\n\n[a # b]: /u", "a\n    # b"),
        ("- [x][a\n\t  > b]\n\n[a > b]: /u", "a\n    > b"),
        (">[a\n>\t  - b]: /u", "a\n    - b"),
        ("- [a\n\t  # b]: /u", "a\n    # b"),
        ("> [^a\n>\t  b]\n\n[^a b]: x", "^a\n    b"),
        ("> [x][a\n>\t\tb]\n\n[a b]: /u", "a\n  \tb"),
    ] {
        let mut document = parse(source);
        assert_eq!(labels(&document)[0], label, "{:?}", source);
        reassign(&mut document, source);
        assert!(document.is_normalized(), "{:?}", source);
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(normalized(document), markdown);
        assert_eq!(
            to_html(&markdown).trim_end(),
            to_html(source).trim_end(),
            "{:?}",
            source
        );
    }
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
        EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
            kind: ReferenceKind::Full,
            label: Text::from(label.to_owned()),
        }))))
    };
    let mut document = parse("### [x][a b]\n\n[a b]: /u\n");
    let link = labelled(&document)[0];
    document.set_kind(link, reference("a\n  b")).unwrap();
    assert!(!document.is_normalized());
    assert_eq!(normalized(document), "### [x][a b]\n\n[a b]: /u\n");
    // A label that is then too long cannot be written, so the link is a resource to where its
    // definition leads.
    let mut document = parse("### [x][a b]\n\n[a b]: /u\n");
    let long = format!("{}\n{}", "a".repeat(500), "a".repeat(499));
    document.set_kind(link, reference(&long)).unwrap();
    let definition = labelled(&document)[1];
    document
        .set_kind(
            definition,
            EventKind::Definition {
                label: Text::from(long.clone()),
                destination: Text::Literal("/v"),
                title: None,
            },
        )
        .unwrap();
    assert_eq!(
        normalized(document),
        format!("### [x](/v)\n\n[{}]: /v\n", long)
    );
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

/// A line of a label can start a block where the parser reads it with that block turned off,
/// which it would start where the serializer writes it, so normalizing indents it four columns,
/// where it continues the label however it is read, and matches the same (#192).
#[test]
fn normalizing_indents_label_lines_that_start_a_block() {
    let off = |change: fn(&mut base_markdown::Constructs)| {
        let mut options = ParseOptions::gfm();
        change(&mut options.constructs);
        options
    };
    for (source, options, expected) in [
        (
            "[x][a\n# b]\n\n[a # b]: /u\n",
            off(|constructs| constructs.heading_atx = false),
            "[x][a\n    # b]\n\n[a # b]: /u\n",
        ),
        (
            "> [x][a\n> # b]\n\n[a # b]: /u\n",
            off(|constructs| constructs.heading_atx = false),
            "> [x][a\n>     # b]\n\n[a # b]: /u\n",
        ),
        (
            "[x][a\n- b]\n\n[a - b]: /u\n",
            off(|constructs| constructs.list_item = false),
            "[x][a\n    - b]\n\n[a - b]: /u\n",
        ),
        (
            "[a\n> b]: /u\n",
            off(|constructs| constructs.block_quote = false),
            "[a\n    > b]: /u\n",
        ),
        (
            "[x][a\n<div>]\n\n[a <div>]: /u\n",
            off(|constructs| constructs.html_flow = false),
            "[x][a\n    <div>]\n\n[a <div>]: /u\n",
        ),
        (
            "[a\n```]: /u\n",
            off(|constructs| constructs.code_fenced = false),
            "[a\n    ```]: /u\n",
        ),
        (
            "[a\n2. b]: /u\n",
            off(|constructs| constructs.list_item = false),
            "[a\n    2. b]: /u\n",
        ),
        (
            "[x][a\n2. b]\n\n[a 2. b]: /u\n",
            off(|constructs| constructs.list_item = false),
            "[x][a\n2. b]\n\n[a 2. b]: /u\n",
        ),
    ] {
        let mut document = Document::parse(source.to_owned(), &options).unwrap();
        let mut html = String::new();
        document
            .render_html(&base_markdown::Options::default().compile, &mut html)
            .unwrap();
        reassign(&mut document, source);
        let markdown = normalized(document);
        assert_eq!(markdown, expected, "{:?}", source);
        assert_eq!(to_html(&markdown), html, "{:?}", source);
    }
    // A hand-built label too, in a definition, a reference and a footnote call.
    let mut document = parse("[a # b]: /u\n\n[a > b]: /u\n\n[x][a > b] [^```]\n\n[^```]: x\n");
    let ids = labelled(&document);
    for (&id, kind) in [ids[0], ids[2], ids[3]].iter().zip([
        EventKind::Definition {
            label: Text::Literal("a\n# b"),
            destination: Text::Literal("/u"),
            title: None,
        },
        EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
            kind: ReferenceKind::Full,
            label: Text::Literal("a\n  > b"),
        })))),
        EventKind::FootnoteReference {
            identifier: Text::Literal("```"),
            label: Some(Text::Literal("\n ```")),
        },
    ]) {
        document.set_kind(id, kind).unwrap();
    }
    assert!(!document.is_normalized());
    assert_eq!(
        normalized(document),
        "[a\n    # b]: /u\n\n[a > b]: /u\n\n[x][a\n    > b] [^\n    ```]\n\n[^```]: x\n"
    );
}

/// A line of a label after its first that could be a table's delimiter row would start a table
/// under the right line before it, which for the second line is what is written before the
/// label, so normalizing indents every such line four columns, whatever the line before it, and
/// the reference reads back as itself (#192).
#[test]
fn label_lines_that_would_be_delimiter_rows_read_back() {
    let reference = |label: &'static str| {
        EventKind::Start(Tag::Link(Target::Reference(Box::new(Reference {
            kind: ReferenceKind::Full,
            label: Text::Literal(label),
        }))))
    };
    let mut document = parse(
        "[x][a b |-| c] [x][a b :-: c] [x][a | - | c] [^a]\n\n[a b |-| c]: /u\n\n[a b :-: c]: /u\n\n[a | - | c]: /u\n\n[d]: /u\n\n[^a]: x\n",
    );
    let ids = labelled(&document);
    for (&id, kind) in ids.iter().zip([
        reference("a\nb\n|-|\nc"),
        reference("a\nb\n:-:\nc"),
        reference("a\n| - |\nc"),
        EventKind::FootnoteReference {
            identifier: Text::Literal("A |-| C"),
            label: Some(Text::Literal("a\n|-|\nc")),
        },
    ]) {
        document.set_kind(id, kind).unwrap();
    }
    document
        .set_kind(
            ids[7],
            EventKind::Definition {
                label: Text::Literal("a\n|-|\nc"),
                destination: Text::Literal("/u"),
                title: None,
            },
        )
        .unwrap();
    assert!(!document.is_normalized());
    document.normalize();
    // No footnote definition's label holds whitespace, so a footnote call whose label does is
    // unbound, and becomes the text it reads back as, with each line ending a space.
    assert_eq!(
        to_markdown(&document).unwrap(),
        "[x][a\nb\n    |-|\nc] [x][a\nb\n    :-:\nc] [x][a\n    | - |\nc] \\[^a |-| c]\n\n[a b |-| c]: /u\n\n[a b :-: c]: /u\n\n[a | - | c]: /u\n\n[a\n    |-|\nc]: /u\n\n[^a]: x\n"
    );
    // A parsed label can hold such a line: as a lazy line in a block quote, under a head with
    // another number of cells, or parsed without tables. It is indented too, wherever the label
    // is, and then reads back as the same reference, which `normalized` checks.
    for (source, expected) in [
        (
            "> [x][a\n|-|\nb]\n\n[a |-| b]: /u",
            "> [x][a\n>     |-|\n> b]\n\n[a |-| b]: /u\n",
        ),
        (
            "> t [x][a\n|-|\nb]\n\n[a |-| b]: /u",
            "> t [x][a\n>     |-|\n> b]\n\n[a |-| b]: /u\n",
        ),
        (
            "- [x][a\n|-|\nb]\n\n[a |-| b]: /u",
            "- [x][a\n      |-|\n  b]\n\n[a |-| b]: /u\n",
        ),
        (
            "[x][a\nb|c\n|-|\nd]\n\n[a b|c |-| d]: /u",
            "[x][a\nb|c\n    |-|\nd]\n\n[a b|c |-| d]: /u\n",
        ),
        // Indented three columns, a line is still a delimiter row, so it takes four.
        (
            "> c | [x][a\n   |-|-|\nb]\n\n[a |-|-| b]: /u",
            "> c | [x][a\n>     |-|-|\n> b]\n\n[a |-|-| b]: /u\n",
        ),
    ] {
        let document = parse(source);
        assert!(!document.is_normalized(), "{:?}", source);
        assert_eq!(normalized(document), expected);
        assert_eq!(to_html(expected), to_html(source), "{:?}", source);
    }
    // The closing bracket after the last line keeps it from being a delimiter row, so it stays.
    let source = "[x][a\nb\n|-|]\n\n[a b |-|]: /u\n";
    assert_eq!(labels(&parse(source))[0], "a\nb\n|-|");
    assert!(parse(source).is_normalized());
    assert_eq!(normalized(parse(source)), source);
    let source = "[x][a\n|-|\nc]\n\n[a |-| c]: /u\n";
    let document = Document::parse(source.to_owned(), &ParseOptions::commonmark()).unwrap();
    assert_eq!(labels(&document)[0], "a\n|-|\nc");
    let markdown = normalized(document);
    assert_eq!(markdown, "[x][a\n    |-|\nc]\n\n[a |-| c]: /u\n");
    assert_eq!(
        to_html(&markdown),
        base_markdown::to_html_with_options(source, &base_markdown::Options::commonmark())
    );
}

/// A footnote call's label that is too long once its line endings are spaces, where it is
/// written on one line, is dropped, so that the call is written as its identifier (#192).
#[test]
fn footnote_calls_too_long_on_one_line_are_written_as_their_identifier() {
    let a = "a".repeat(999);
    for label in [format!("{}\n", a), format!("\n{}", a)] {
        let mut document = parse(&format!(
            "### [^{0}]\n\n| [^{0}] |\n| - |\n\n[^{0}]: x\n",
            a
        ));
        let ids = labelled(&document);
        for &id in &ids[..2] {
            document
                .set_kind(
                    id,
                    EventKind::FootnoteReference {
                        identifier: Text::from(a.to_uppercase()),
                        label: Some(Text::from(label.clone())),
                    },
                )
                .unwrap();
        }
        assert!(!document.is_normalized());
        document.normalize();
        assert!(document.is_normalized());
        let markdown = to_markdown(&document).unwrap();
        let upper = a.to_uppercase();
        assert_eq!(
            markdown,
            format!("### [^{0}]\n\n| [^{0}] |\n| --- |\n\n[^{1}]: x\n", upper, a)
        );
        assert_eq!(labels(&parse(&markdown)), [upper.clone(), upper, a.clone()]);
    }
}

/// Writing the labels of a line of many footnote calls takes time linear in the line, however
/// long the line is before each (#192). The line is timed against the same text with a line
/// ending for the space after each call, a control of the same size whose lines are short, so the
/// ratio of the two growths shows only work that depends on the length of the line: scanning
/// back over the line for each call grew 1.7 times the control's growth here.
#[test]
fn writing_many_labels_on_a_line_is_linear() {
    let _timing = support::linear::TIMING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let text = "a".repeat(64);
    let document = |count: usize, end: char| {
        parse(&format!(
            "{}\n\n[^a]: x\n",
            format!("[^a] {text}{end}").repeat(count)
        ))
    };
    let documents = [
        document(1 << 12, ' '),
        document(1 << 14, ' '),
        document(1 << 12, '\n'),
        document(1 << 14, '\n'),
    ];
    let mut ratios = Vec::new();
    // Noise makes linear work look slower only now and then, so a ratio over the bound is
    // measured again up to twice.
    for _ in 0..3 {
        let times: Vec<f64> = documents
            .iter()
            .map(|document| {
                timed(|| {
                    to_markdown(document).unwrap();
                })
                .as_secs_f64()
            })
            .collect();
        let ratio = (times[1] / times[0]) / (times[3] / times[2]);
        eprintln!("linear: {times:?}, {ratio:.2}x the control's growth");
        if ratio < 1.4 {
            return;
        }
        ratios.push(ratio);
    }
    panic!("the line grew {:?} times the control's growth", ratios);
}
