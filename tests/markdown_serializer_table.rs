// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod support;
use base_markdown::{
    stream::{EventKind, Tag, Text},
    to_html_with_options, to_markdown, Document, Options, ParseOptions, ReferenceKind,
};
use pretty_assertions::assert_eq;
use support::linear::{assert_linear, timed};

#[test]
fn table_cells_escape_like_paragraphs() {
    for (source, expected) in [
        ("| \\_a\\_ |\n| - |\n", "| \\_a\\_ |\n| --- |\n"),
        ("| a \\*b\\* |\n| - |\n", "| a \\*b\\* |\n| --- |\n"),
        ("| \\[a\\](b) |\n| - |\n", "| \\[a]\\(b) |\n| --- |\n"),
        ("| \\<span> |\n| - |\n", "| \\<span> |\n| --- |\n"),
        ("| a_b |\n| - |\n", "| a_b |\n| --- |\n"),
        ("| a \\| b |\n| - |\n", "| a \\| b |\n| --- |\n"),
        ("| `a\\|b` |\n| - |\n", "| `a\\|b` |\n| --- |\n"),
        ("| ``a\\|`b`` |\n| - |\n", "| ``a\\|`b`` |\n| --- |\n"),
        (
            "| <span title=\"`a\\|b`\"> |\n| - |\n",
            "| <span title=\"`a\\|b`\"> |\n| --- |\n",
        ),
        (
            "| <!-- `a\\|b` --> |\n| - |\n",
            "| <!-- `a\\|b` --> |\n| --- |\n",
        ),
        ("| [a](b\\|c) |\n| - |\n", "| [a](b\\|c) |\n| --- |\n"),
        ("| a\\\\\\|b |\n| - |\n", "| a\\\\\\|b |\n| --- |\n"),
    ] {
        let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let html = |value: &str| to_html_with_options(value, &Options::gfm());
        assert_eq!(
            html(&markdown),
            html(source),
            "{source:?} should round-trip"
        );
    }
}

#[test]
fn table_cells_encode_whitespace_at_their_edges() {
    // #109: the parser trims spaces and tabs at the edges of a cell, so they are encoded there.
    for (source, expected) in [
        ("| a&#x20; |\n| - |\n", "| a&#x20; |\n| --- |\n"),
        ("| &#x20;a |\n| - |\n", "| &#x20;a |\n| --- |\n"),
        ("| &#x9;a&#x9; |\n| - |\n", "| &#x9;a&#x9; |\n| --- |\n"),
        ("| &#x20; |\n| - |\n", "| &#x20; |\n| --- |\n"),
        ("| a &#x20; b |\n| - |\n", "| a   b |\n| --- |\n"),
        (
            "| www.a.b.&#x20; |\n| - |\n",
            "| www.a.b.&#x20; |\n| --- |\n",
        ),
        ("| www.&#x20; |\n| - |\n", "| www.&#x20; |\n| --- |\n"),
        ("| www\\.w&#x20; |\n| - |\n", "| www\\.w&#x20; |\n| --- |\n"),
        ("| a\\\\&#x20; |\n| - |\n", "| a\\\\&#x20; |\n| --- |\n"),
        ("| *a*&#x20; |\n| - |\n", "| *a*&#x20; |\n| --- |\n"),
        ("| `a`&#x20; |\n| - |\n", "| `a`&#x20; |\n| --- |\n"),
        (
            "| a | &#x20;b |\n| - | - |\n",
            "| a | &#x20;b |\n| --- | --- |\n",
        ),
    ] {
        let mut document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        document.normalize();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let html = |value: &str| to_html_with_options(value, &Options::gfm());
        assert_eq!(
            html(&markdown),
            html(source),
            "{source:?} should round-trip"
        );
        let mut reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
        reparsed.normalize();
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{source:?}");
    }
}

#[test]
fn table_cells_escape_pipes_in_reference_labels() {
    // Labels match by their raw text, so they are written as they are and stay linked.
    for (source, expected, linked) in [
        (
            "| [x][a\\|b] |\n| - |\n\n[a\\|b]: c\n",
            "| [x][a\\|b] |\n| --- |\n\n[a\\|b]: c\n",
            true,
        ),
        (
            "| [x][a\\\\\\|b] |\n| - |\n\n[a\\\\\\|b]: c\n",
            "| [x][a\\\\\\|b] |\n| --- |\n\n[a\\\\\\|b]: c\n",
            true,
        ),
        (
            "| [a\\\\\\|b] |\n| - |\n\n[a\\\\\\|b]: c\n",
            "| [a\\\\\\|b] |\n| --- |\n\n[a\\\\\\|b]: c\n",
            true,
        ),
    ] {
        let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{:?}", source);
        let html = to_html_with_options(&markdown, &Options::gfm());
        assert!(
            html.starts_with("<table>"),
            "a pipe in the label should not split the cell: {:?}",
            html
        );
        assert_eq!(html.contains("<a href=\"c\">"), linked, "{:?}", html);
        let reparsed = Document::parse(&markdown, &ParseOptions::gfm()).unwrap();
        assert_eq!(to_markdown(&reparsed).unwrap(), expected, "{:?}", source);
    }
}

#[test]
fn table_cells_escape_bare_pipes_in_edited_reference_labels() {
    let mut document =
        Document::parse("| [x][a] |\n| - |\n\n[a]: c\n", &ParseOptions::gfm()).unwrap();
    let link = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Start(Tag::Link { .. })
            )
        })
        .unwrap();
    support::fixture::set_reference(
        &mut document,
        link,
        ReferenceKind::Full,
        Text::Literal("a|b"),
    )
    .unwrap();
    let definition = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Definition { .. }
            )
        })
        .unwrap();
    document
        .set_kind(
            definition,
            EventKind::Definition {
                label: Text::Literal("a|b"),
                destination: Text::Literal("c"),
                title: None,
            },
        )
        .unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "| [x][a\\|b] |\n| --- |\n\n[a|b]: c\n");
    let html = to_html_with_options(&markdown, &Options::gfm());
    assert!(html.starts_with("<table>"), "{:?}", html);
}

#[test]
fn labels_keep_their_raw_text() {
    for source in [
        "[a\\]b]\n\n[a\\]b]: c\n",
        "[x][a\\*b\\\\]\n\n[a\\*b\\\\]: c\n",
        "[a|b]\n\n[a|b]: c\n",
        "| [x][a|b] |\n| - |\n\n[a|b]: c\n",
    ] {
        let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, source);
    }
}

/// A label is written as it is, so an edited one with a bracket without a backslash before it, or
/// a final backslash, which would not read back, is rejected, and one that escapes them is written
/// as it is (#192).
#[test]
fn edited_labels_are_written_as_they_are() {
    let mut document = Document::parse("[a]: c\n", &ParseOptions::gfm()).unwrap();
    let definition = document
        .descendants(document.root())
        .find(|&id| {
            matches!(
                document.node(id).unwrap().kind,
                EventKind::Definition { .. }
            )
        })
        .unwrap();
    let definition_of = |label| EventKind::Definition {
        label: Text::Literal(label),
        destination: Text::Literal("c"),
        title: None,
    };
    assert!(matches!(
        document.set_kind(definition, definition_of("a[b\\")),
        Err(base_markdown::stream::StreamError::InvalidValue)
    ));
    document
        .set_kind(definition, definition_of("a\\[b\\\\"))
        .unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "[a\\[b\\\\]: c\n");
    let reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
}

/// A lazy line that would be a delimiter row reads as text after a paragraph in a container, but
/// once the container is written before it, it would start a table, so its first character is
/// escaped. This is also the case after a definition, whose last line is the header (#146).
#[test]
fn lazy_delimiter_rows_in_containers_stay_text() {
    for (source, expected) in [
        ("[^r]: u\n| - |", "[^r]: u\n    \\| - |\n"),
        ("- [R]: u\n| - |", "- [R]: u\n  \\| - |\n"),
        ("- u\n| - |", "- u\n  \\| - |\n"),
        ("> u\n| - |", "> u\n> \\| - |\n"),
        ("- a\n  b\n| - |", "- a\n  b\n  \\| - |\n"),
        ("- a\n| b\n| - |", "- a\n  | b\n  \\| - |\n"),
        ("- a\n|b|\n:-", "- a\n  |b|\n  \\:-\n"),
        ("- a\n  b | c\n| - | - |", "- a\n  b | c\n  \\| - | - |\n"),
        ("- a\n\\| - |", "- a\n  \\| - |\n"),
        // A row with other cells than its header, or a table, is written as it is.
        ("- a\n| - | - |", "- a\n  | - | - |\n"),
        ("- | a |\n  | - |", "- | a |\n  | --- |\n"),
        ("> a\n> | - |", "> | a |\n> | --- |\n"),
    ] {
        let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{source:?}");
        let html = |value: &str| to_html_with_options(value, &Options::gfm());
        assert_eq!(
            html(&markdown).trim_end(),
            html(source).trim_end(),
            "{source:?}"
        );
        let reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{source:?}");
    }
}

/// A paragraph built with a line that would be a delimiter row has it escaped, also outside a
/// container (#146).
#[test]
fn synthetic_delimiter_rows_in_paragraphs_stay_text() {
    for (value, expected) in [
        ("a\n| - |", "a\n\\| - |\n"),
        ("a\n- | -", "a\n\\- | -\n"),
        ("a\n:-:|", "a\n\\:-:|\n"),
        ("a\nb\n| - |\n| - |", "a\nb\n\\| - |\n\\| - |\n"),
    ] {
        let mut document = Document::parse("", &ParseOptions::gfm()).unwrap();
        let root = document.root();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::from(value.to_string())))
            .unwrap();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, expected, "{value:?}");
        let reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
        assert_eq!(to_markdown(&reparsed).unwrap(), markdown, "{value:?}");
        let html = to_html_with_options(&markdown, &Options::gfm());
        assert!(!html.contains("<table>"), "{}", html);
    }
}

/// Checking each line of a block for a delimiter row, and escaping the ones that are, takes time
/// linear in the block (#146).
#[test]
fn delimiter_row_escapes_scale_linearly() {
    type Build = fn(usize) -> String;
    let cases: [(&str, Build); 3] = [
        ("rows", |count| format!("- a\n{}", "| - |\n".repeat(count))),
        ("headers", |count| {
            format!("- a\n{}| - |\n", "| b\n".repeat(count))
        }),
        ("quoted", |count| {
            format!("> a\n{}", "| - |\n".repeat(count))
        }),
    ];
    for (label, build) in cases {
        assert_linear(label, 1 << 7, |count| {
            let document = Document::parse(build(count), &ParseOptions::gfm()).unwrap();
            timed(|| {
                to_markdown(&document).unwrap();
            })
        });
    }
}

/// Output is read with the default options, which have tables, so a table parsed as text without
/// them is written with its delimiter row escaped, as it would otherwise read back as a table.
#[test]
fn tables_parsed_as_text_stay_text() {
    let source = "| a | b |\n| --- | :-: |\n| c | d |\n";
    let document = Document::parse(source, &ParseOptions::commonmark()).unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "| a | b |\n\\| --- | :-: |\n| c | d |\n");
    let html = to_html_with_options(&markdown, &Options::default());
    assert!(!html.contains("<table>"), "{}", html);
    assert_eq!(html, to_html_with_options(source, &Options::commonmark()));
}
