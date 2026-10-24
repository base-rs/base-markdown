// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use core_markdown::{to_html, to_html_with_options, Constructs, Document, Options, ParseOptions};

#[test]
fn syntax_extensions_can_be_enabled_without_a_preset() {
    for (source, constructs, expected) in [
        (
            "www.example.com",
            Constructs {
                autolink_literal: true,
                ..Constructs::default()
            },
            "<a href=\"http://www.example.com\">",
        ),
        (
            "~~removed~~",
            Constructs {
                strikethrough: true,
                ..Constructs::default()
            },
            "<del>removed</del>",
        ),
        (
            "| a |\n| - |\n| b |",
            Constructs {
                table: true,
                ..Constructs::default()
            },
            "<table>",
        ),
        (
            "- [x] done",
            Constructs {
                task_list_item: true,
                ..Constructs::default()
            },
            "<input type=\"checkbox\"",
        ),
        (
            "note[^a]\n\n[^a]: explanation",
            Constructs {
                footnote_definition: true,
                label_start_footnote: true,
                ..Constructs::default()
            },
            "data-footnote-ref",
        ),
    ] {
        let options = Options {
            parse: ParseOptions {
                constructs,
                ..ParseOptions::default()
            },
            ..Options::default()
        };
        let actual = to_html_with_options(source, &options).unwrap();
        assert!(actual.contains(expected), "{}: {}", source, actual);
        assert_eq!(
            actual,
            to_html_with_options(source, &Options::gfm()).unwrap()
        );
        assert_ne!(actual, to_html(source));

        let document = Document::parse(source, &options.parse).unwrap();
        let mut rendered = String::new();
        document
            .render_html(&options.compile, &mut rendered)
            .unwrap();
        assert_eq!(rendered, actual);
    }
}
