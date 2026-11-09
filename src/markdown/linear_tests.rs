// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Writing Markdown does work linear in the document, counted with [`crate::work`].

extern crate std;

use super::{to_markdown, to_markdown_with_options, IndentOptions, Options};
use crate::stream::{EventKind, Tag, Text};
use crate::work::tests::{assert_linear, counted, SCALE};
use crate::{Document, ParseOptions};
use std::borrow::ToOwned;
use std::string::String;
use std::{format, vec};

#[test]
fn literal_scans_stay_linear() {
    // Eight times the input does about eight times the work, whether the text is one node or,
    // without normalizing, many nodes split at escapes. A domain that starts inside one already
    // scanned is not scanned again (`www.a_`), and the markers in the siblings of a text split at
    // escapes are scanned from where the last scan stopped (`www\.a `, `x\@y.z`).
    let steps = |unit: &str, count: usize, normalize: bool| {
        let source = unit.repeat(count) + "\n";
        let mut document = Document::parse(source.as_str(), &ParseOptions::gfm()).unwrap();
        if normalize {
            document.normalize();
        }
        counted(|| {
            to_markdown(&document).unwrap();
        })
    };
    for (unit, normalize) in [("www.a_", true), ("www\\.a ", false), ("x\\@y.z", false)] {
        assert_linear(unit, 1 << 8, |count| steps(unit, count, normalize));
    }
}

#[test]
fn escaping_tildes_is_linear() {
    let options = crate::ParseOptions::gfm();
    // Escaped tildes are added to the list of escapes, and each run is checked against the list without searching it from the start: `\~` adds many escapes, and in `\* ~~a` escapes of `*` are already in the list. Runs of `~`, `a~` and `~~a ` take no path that rescans (a run of `~` is a code fence), so they are not counted.
    for unit in ["\\~", "\\* ~~a"] {
        assert_linear(unit, 1 << 9, |count| {
            let mut document = Document::parse(unit.repeat(count) + "\n", &options).unwrap();
            document.normalize();
            counted(|| {
                crate::to_markdown(&document).unwrap();
            })
        });
    }
}

#[test]
fn image_text_is_linear() {
    let options = ParseOptions::gfm();
    let check = |source: &str| {
        let mut document = Document::parse(source, &options).unwrap();
        document.normalize();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(markdown, format!("{source}\n"));
    };
    // A long chain of links, calls and images in one image's text.
    assert_linear("links in image text", 1 << 6, |size| {
        let source = format!(
            "![{}](/i)\n\n[^1]: x",
            "[a](/v) ![b](/w) [^1] *c* ".repeat(size)
        );
        counted(|| check(&source))
    });
    // Images nested in images, as deep as they can be written, each with a link in its text, side
    // by side. The size is how deep each nest is, so it grows up to the limit.
    assert_linear("images in image text", 15, |size| {
        let nest = format!("{}a{}", "![[l](/v) ".repeat(size), "](u)".repeat(size));
        let source = vec![nest; 8].join(" ");
        counted(|| check(&source))
    });
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
            counted(|| {
                to_markdown(&document).unwrap();
            })
        });
    }
}

#[test]
fn text_angle_bracket_email_scans_stay_linear() {
    // Each scan stops before the next `<`, so the work grows as the input does.
    let steps = |count: usize| {
        let input = "<1.2-3 ".repeat(count) + "x\n";
        let tree = Document::parse(&input, &Default::default()).unwrap();
        let mut output = String::new();
        let steps = counted(|| output = to_markdown(&tree).unwrap());
        assert_eq!(output, input);
        steps
    };
    assert_linear("<1.2-3 ", 1 << 9, steps);
}

/// Assert that serializing the document `build` makes does linear work with each of `options`.
fn assert_serializes_linear(
    label: &str,
    size: usize,
    options: &[(&str, Options)],
    build: impl Fn(usize) -> Document<String>,
) {
    let small = build(size);
    let large = build(SCALE * size);
    for (name, options) in options {
        assert_linear((label, name), size, |count| {
            let document = if count == size { &small } else { &large };
            counted(|| {
                to_markdown_with_options(document, options).unwrap();
            })
        });
    }
}

/// A task item of `size` definitions and then `4 * size` paragraphs, each holding a task marker.
fn task_item_after_definitions(size: usize) -> Document<String> {
    let mut document = Document::parse(String::new(), &ParseOptions::gfm()).unwrap();
    let root = document.root();
    let list = document
        .append_child(
            root,
            EventKind::Start(Tag::List {
                start: None,
                tight: true,
            }),
        )
        .unwrap();
    let item = document
        .append_child(
            list,
            EventKind::Start(Tag::Item {
                block_end: false,
                checked: Some(true),
            }),
        )
        .unwrap();
    for index in 0..size {
        document
            .append_child(
                item,
                EventKind::Definition {
                    // Each its own label, as an edit adds no second definition of one.
                    label: Text::from(format!("a{}", index)),
                    destination: Text::Literal("/u"),
                    title: None,
                },
            )
            .unwrap();
    }
    for _ in 0..4 * size {
        let paragraph = document
            .append_child(item, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::Literal(" a")))
            .unwrap();
    }
    document
}

#[test]
fn serializing_list_items_is_linear() {
    let default = [("default", Options::default())];
    type Build = fn(usize) -> String;
    let cases: [(&str, Build); 4] = [
        ("headings in one item", |size| {
            format!("- {}", "# a\n  ".repeat(size))
        }),
        ("footnote definitions in one item", |size| {
            format!("- {}", "[^a]: b\n  ".repeat(size))
        }),
        ("nested lists between paragraphs", |size| {
            format!("- a\n{}", "  - b\n\n  c\n".repeat(size))
        }),
        ("nested items with several blocks", |size| {
            format!("- a\n  - b\n{}", "    # c\n    d\n    - e\n".repeat(size))
        }),
    ];
    let parse = ParseOptions::gfm();
    for (label, build) in cases {
        assert_serializes_linear(label, 1 << 8, &default, |size| {
            Document::parse(build(size), &parse).unwrap()
        });
    }
    // Finding the task marker does not depend on the options. With these, definitions are joined
    // without a blank line.
    let both = [
        ("default", Options::default()),
        (
            "combination",
            Options {
                fences: false,
                list_item_indent: IndentOptions::Mixed,
                tight_definitions: true,
                ..Options::default()
            },
        ),
    ];
    assert_serializes_linear(
        "task markers after definitions",
        1 << 8,
        &both,
        task_item_after_definitions,
    );
}

#[test]
fn encoding_line_endings_is_linear() {
    let options = ParseOptions::default();
    type Build = fn(usize) -> String;
    // Whether a line ending is followed only by the end of the text is decided without scanning the rest, and each cell of a wide table is written without scanning back to the start of its line.
    let cases: [(&str, Build); 2] = [
        ("consecutive line endings", |count| {
            format!("a{}b\n", "&#xA;".repeat(count))
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
        assert_linear(label, 1 << 7, |count| {
            let mut document = Document::parse(build(count), &options).unwrap();
            document.normalize();
            counted(|| {
                to_markdown(&document).unwrap();
            })
        });
    }
}

fn parse(source: &str) -> Document<String> {
    Document::parse(source.to_owned(), &ParseOptions::default()).unwrap()
}

/// Writing the labels of a line of many footnote calls does work linear in the line, however
/// long the line is before each (#192). The line is counted against the same text with a line
/// ending for the space after each call, a control of the same size whose lines are short, so the
/// ratio of the two growths shows only work that depends on the length of the line, which
/// scanning back over the line for each call would make grow with it.
#[test]
fn writing_many_labels_on_a_line_is_linear() {
    let text = "a".repeat(64);
    let steps = |count: usize, end: char| {
        let document = parse(&format!(
            "{}\n\n[^a]: x\n",
            format!("[^a] {text}{end}").repeat(count)
        ));
        counted(|| {
            to_markdown(&document).unwrap();
        })
    };
    crate::work::tests::assert_against_control(
        "labels on a line",
        1 << 9,
        |count| steps(count, ' '),
        |count| steps(count, '\n'),
    );
}
