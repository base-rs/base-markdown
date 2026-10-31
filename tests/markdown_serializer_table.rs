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
    let source = "| [x][a\\|b] |\n| - |\n\n[a\\|b]: c\n";
    let document = Document::parse(source, &ParseOptions::gfm()).unwrap();
    let markdown = to_markdown(&document).unwrap();
    assert_eq!(markdown, "| [x][a\\\\\\|b] |\n| --- |\n\n[a\\\\|b]: c\n");
    let html = to_html_with_options(&markdown, &Options::gfm());
    assert!(
        html.starts_with("<table>"),
        "a pipe in the label should not split the cell: {:?}",
        html
    );
}
