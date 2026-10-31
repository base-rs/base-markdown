// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::{to_html_with_options, to_markdown, Document, Options, ParseOptions};
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
fn table_cells_escape_pipes_in_reference_labels() {
    for (source, expected, linked) in [
        (
            "| [x][a\\|b] |\n| - |\n\n[a\\|b]: c\n",
            "| [x][a\\\\\\|b] |\n| --- |\n\n[a\\\\|b]: c\n",
            false,
        ),
        (
            "| [x][a\\\\\\|b] |\n| - |\n\n[a\\\\\\|b]: c\n",
            "| [x][a\\\\\\\\\\|b] |\n| --- |\n\n[a\\\\\\\\\\|b]: c\n",
            true,
        ),
        (
            "| [a\\\\\\|b] |\n| - |\n\n[a\\\\\\|b]: c\n",
            "| [a\\\\\\|b][a\\\\\\\\\\|b] |\n| --- |\n\n[a\\\\\\\\\\|b]: c\n",
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
    }
}
