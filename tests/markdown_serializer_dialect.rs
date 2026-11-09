// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The writer follows the dialect a document was parsed with, `Document::parse_options` (#257):
//! it writes what those options read, and its escapes are the ones they need.

mod support;

use base_markdown::{to_markdown, Document, MarkdownOptions, ParseOptions};
use pretty_assertions::assert_eq;
use support::round_trip::{round_trip, Outcome};

/// Options from a list of changes to the defaults: `!name` turns a construct off, and `name` turns
/// an option on.
fn options(changes: &[&str]) -> ParseOptions {
    let mut options = ParseOptions::default();
    for change in changes {
        let (on, name) = match change.strip_prefix('!') {
            Some(name) => (false, name),
            None => (true, *change),
        };
        let constructs = &mut options.constructs;
        match name {
            "autolink" => constructs.autolink = on,
            "autolink_literal" => constructs.autolink_literal = on,
            "code_fenced" => constructs.code_fenced = on,
            "hard_break_escape" => constructs.hard_break_escape = on,
            "hard_break_trailing" => constructs.hard_break_trailing = on,
            "heading_atx" => constructs.heading_atx = on,
            "heading_setext" => constructs.heading_setext = on,
            "label_end" => constructs.label_end = on,
            "label_start_footnote" => constructs.label_start_footnote = on,
            "label_start_link" => constructs.label_start_link = on,
            "table" => constructs.table = on,
            "html_block_blank_lines" => options.html_block_blank_lines = on,
            "inline_code_info" => options.inline_code_info = on,
            _ => panic!("unknown option {}", name),
        }
    }
    options
}

/// Each source, parsed under its options and normalized, is written as expected, and reads back
/// under the same options as the same document.
fn check(cases: &[(&[&str], &str, &str)]) {
    for &(changes, source, expected) in cases {
        let options = options(changes);
        let mut document = Document::parse(source, &options).unwrap();
        document.normalize();
        assert_eq!(
            to_markdown(&document).unwrap(),
            expected,
            "{:?} {:?}",
            changes,
            source
        );
        assert!(
            matches!(
                round_trip(document, &MarkdownOptions::default(), false),
                Outcome::Pass
            ),
            "{:?} {:?} does not round trip",
            changes,
            source
        );
    }
}

/// A heading is written setext where ATX headings are off, and ATX where setext ones are.
#[test]
fn headings() {
    check(&[
        (&[], "a\n=\n", "# a\n"),
        (&["!heading_atx"], "a\n=\n", "a\n=\n"),
        (&["!heading_atx"], "a\n-\n", "a\n-\n"),
        (&["!heading_setext"], "# a\n", "# a\n"),
    ]);
}

/// A hard break is written with trailing spaces where the escape is off, after the space or tab
/// the text ends in encoded, and after the backslash before it escaped.
#[test]
fn hard_breaks() {
    check(&[
        (&["!hard_break_escape"], "a  \nb", "a  \nb\n"),
        (&["!hard_break_escape"], "a&#x20;  \nb", "a&#x20;  \nb\n"),
        (&["!hard_break_escape"], "a&#x9;  \nb", "a&#x9;  \nb\n"),
        (
            &["!hard_break_escape"],
            "a\\\\&#x20;  \nb",
            "a\\\\&#x20;  \nb\n",
        ),
        (&["!hard_break_escape"], "*a* *b*  \nc", "*a* *b*  \nc\n"),
        (
            &["!hard_break_escape"],
            "[^b]: a *b*  \nc",
            "[^b]: a *b*  \n    c\n",
        ),
        (&["!hard_break_trailing"], "a\\\nb", "a\\\nb\n"),
    ]);
}

/// Code is indented where fenced code is off.
#[test]
fn code() {
    check(&[
        (&[], "    x\n", "```\nx\n```\n"),
        (&["!code_fenced"], "    x\n", "    x\n"),
    ]);
}

/// A link is written in a form the options read: the form it was read in, or an autolink where
/// links with text are off.
#[test]
fn links() {
    check(&[
        (&["!autolink"], "https://a.b", "https://a.b\n"),
        (&["!autolink_literal"], "<https://a.b>", "<https://a.b>\n"),
        (&["!label_start_link"], "<https://a.b>", "<https://a.b>\n"),
        (
            &["!label_end"],
            "x <https://a.b> www.a.b",
            "x <https://a.b> www.a.b\n",
        ),
    ]);
}

/// Link text that starts with `^` is escaped only where `[^` would start a footnote call (#215).
#[test]
fn caret_link_text() {
    check(&[
        (
            &[],
            "[\\^n](/l)\n\n[^n]: note\n",
            "[\\^n](/l)\n\n[^n]: note\n",
        ),
        (&["!label_start_footnote"], "[\\^n](/l)", "[^n](/l)\n"),
    ]);
}

/// With `ParseOptions::inline_code_info`, a `:` after language characters before code is
/// escaped, so the characters are not read as its language (#180), and attention next to a
/// language is written with a marker that keeps it whole (#185).
#[test]
fn inline_code_info() {
    check(&[
        (&[], "a\\_b:`c`", "a_b:`c`\n"),
        (&["inline_code_info"], "a\\_b:`c`", "a_b\\:`c`\n"),
        (&["inline_code_info"], "- a\\_b:`c`", "- a_b\\:`c`\n"),
        (
            &["inline_code_info"],
            "- [ ] a\\_b:`c`",
            "- [ ] a_b\\:`c`\n",
        ),
        (&["inline_code_info"], "_a_**a**x:`v`", "*a***a**x:`v`\n"),
    ]);
}

/// With `ParseOptions::html_block_blank_lines`, an HTML block of kind 6 or 7 holds the blank
/// lines after it, which are not written again (#265).
#[test]
fn html_block_blank_lines() {
    check(&[
        (&["html_block_blank_lines"], "<div>\n\nx\n", "<div>\n\nx\n"),
        (
            &["html_block_blank_lines"],
            "<div>\n\n\nx\n",
            "<div>\n\n\nx\n",
        ),
        (
            &["html_block_blank_lines"],
            "- <div>\n\n  x\n",
            "- <div>\n\n  x\n",
        ),
        (
            &["html_block_blank_lines"],
            "> <div>\n\n> x\n",
            "> <div>\n\n> x\n",
        ),
        (
            &["html_block_blank_lines"],
            "> <div>\n>\n> y\n\n> x\n",
            "> <div>\n>\n> y\n\n> x\n",
        ),
        (&["html_block_blank_lines"], "<div>\n", "<div>\n"),
    ]);
}

/// Shapes that the default options get wrong on main, found by the dialect round trip.
#[test]
fn default_dialect() {
    check(&[
        // A task item's checkbox before a line ending (#199).
        (&[], "- [ ] \n  b\n", "- [ ] \n  b\n"),
        // A task item's checkbox after definitions, where the parser reads it (#200).
        (&[], "- [r]: /u\n  [ ] a\n", "- [r]: /u\n  [ ] a\n"),
        (
            &[],
            "- [r]: /u\n  [x]\ta\n  b\n",
            "- [r]: /u\n  [x]\ta\n  b\n",
        ),
        // A line of inline HTML that would start a block or a table's delimiter row.
        (
            &[],
            "a <x y=\"\n    </div>\">",
            "a <x y=\"\n    </div>\">\n",
        ),
        (
            &[],
            "- a <!X\n| a |\n| - |\n-->",
            "- a <!X\n  | a |\n      | - |\n  -->\n",
        ),
        // An item that starts with an HTML block indented by spaces.
        (&[], "1.       \n      </div>\n", "1.\n      </div>\n"),
    ]);
}

/// A table parsed as text without `Constructs::table` is written as it is.
#[test]
fn tables() {
    check(&[
        (&["!table"], "| a |\n| - |\n", "| a |\n| - |\n"),
        (&[], "| a |\n\\| - |\n", "| a |\n\\| - |\n"),
    ]);
}

/// Attention next to a language is written with `*`, which no language holds, whatever marker the
/// options prefer (#185).
#[test]
fn attention_next_to_a_language() {
    let options = options(&["inline_code_info"]);
    let markdown = MarkdownOptions {
        emphasis: '_',
        ..MarkdownOptions::default()
    };
    for (source, expected) in [
        ("*a*rust:`v`", "*a*rust:`v`\n"),
        ("xa*rust:`v`*", "x&#x61;*rust:`v`*\n"),
    ] {
        let mut document = Document::parse(source, &options).unwrap();
        document.normalize();
        assert_eq!(
            base_markdown::to_markdown_with_options(&document, &markdown).unwrap(),
            expected,
            "{:?}",
            source
        );
        assert!(
            matches!(round_trip(document, &markdown, false), Outcome::Pass),
            "{:?} does not round trip",
            source
        );
    }
}

/// Code after an HTML block that holds the blank lines after it is fenced, even where the options
/// prefer indented code, as the block would take indented lines after its blank lines (#265).
#[test]
fn code_after_html_block_blank_lines() {
    let options = options(&["html_block_blank_lines"]);
    let markdown = MarkdownOptions {
        fences: false,
        ..MarkdownOptions::default()
    };
    let mut document = Document::parse("<div>\n\n```\nx\n```\n", &options).unwrap();
    document.normalize();
    let output = base_markdown::to_markdown_with_options(&document, &markdown).unwrap();
    assert_eq!(output, "<div>\n\n```\nx\n```\n");
    assert!(matches!(
        round_trip(document, &markdown, false),
        Outcome::Pass
    ));
}
