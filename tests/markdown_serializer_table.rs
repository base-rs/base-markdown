// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{
    stream::{EventKind, Tag, Text},
    to_html_with_options, to_markdown, Document, NodeSyntax, Options, ParseOptions, ReferenceKind,
};
use pretty_assertions::assert_eq;

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
    document
        .set_syntax(
            link,
            Some(NodeSyntax::Reference {
                kind: ReferenceKind::Full,
                label: Text::Literal("a|b"),
            }),
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

#[test]
fn edited_labels_escape_brackets_and_a_trailing_backslash() {
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
    document
        .set_kind(
            definition,
            EventKind::Definition {
                label: Text::Literal("a[b\\"),
                destination: Text::Literal("c"),
                title: None,
            },
        )
        .unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "[a\\[b\\\\]: c\n");
    let reparsed = Document::parse(markdown.as_str(), &ParseOptions::gfm()).unwrap();
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
}
