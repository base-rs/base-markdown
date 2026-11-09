// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The writer follows the dialect a document was parsed with, `Document::parse_options` (#257):
//! it writes what those options read, and where no spelling reads back under them, returns
//! `unwritable-node`.

mod support;

use base_markdown::{
    stream::{EventKind, Tag, Text},
    to_markdown, to_markdown_with_options, Document, MarkdownOptions, ParseOptions,
};
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
            "character_escape" => constructs.character_escape = on,
            "character_reference" => constructs.character_reference = on,
            "code_fenced" => constructs.code_fenced = on,
            "code_indented" => constructs.code_indented = on,
            "footnote_definition" => constructs.footnote_definition = on,
            "hard_break_escape" => constructs.hard_break_escape = on,
            "hard_break_trailing" => constructs.hard_break_trailing = on,
            "heading_atx" => constructs.heading_atx = on,
            "heading_setext" => constructs.heading_setext = on,
            "html_flow" => constructs.html_flow = on,
            "label_end" => constructs.label_end = on,
            "label_start_footnote" => constructs.label_start_footnote = on,
            "label_start_link" => constructs.label_start_link = on,
            "table" => constructs.table = on,
            "html_block_blank_lines" => options.html_block_blank_lines = on,
            "inline_code_info" => options.inline_code_info = on,
            "strikethrough_single_tilde" => options.strikethrough_single_tilde = on,
            _ => panic!("unknown option {}", name),
        }
    }
    options
}

/// Each source, parsed under its options and normalized, is written as expected, and reads back
/// under the same options as the same document.
fn check(cases: &[(&[&str], &str, &str)]) {
    check_with(&MarkdownOptions::default(), cases);
}

/// As [`check`], writing with `markdown`.
fn check_with(markdown: &MarkdownOptions, cases: &[(&[&str], &str, &str)]) {
    for &(changes, source, expected) in cases {
        let options = options(changes);
        let mut document = Document::parse(source, &options).unwrap();
        document.normalize();
        assert_eq!(
            to_markdown_with_options(&document, markdown).unwrap(),
            expected,
            "{:?} {:?}",
            changes,
            source
        );
        assert!(
            matches!(round_trip(document, markdown, false), Outcome::Pass),
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
        // A backtick in the info string makes the fence tildes where references are off.
        (
            &["!character_reference"],
            "~~~[ `\nx\n~~~\n",
            "~~~[ `\nx\n~~~\n",
        ),
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

/// A list's bullet neither joins it to the list before nor makes a line of an item's indented
/// code, with the markers before it, a thematic break: it is the preferred bullet, else the one
/// that alternates with it, else the third.
#[test]
fn bullets_around_indented_code() {
    check(&[
        (&["!code_fenced"], "*     --\n", "*     --\n"),
        (&["!code_fenced"], "*     - -\n", "*     - -\n"),
        (&["!code_fenced"], "+     -\t-\n", "*     -\t-\n"),
        (
            &["!code_fenced"],
            "+     ++\n+     **\n",
            "-     ++\n-     **\n",
        ),
        (
            &["!code_fenced"],
            "+     **\n+     --\n",
            "+     **\n+     --\n",
        ),
        (
            &["!code_fenced"],
            "+     --\n+     **\n",
            "+     --\n+     **\n",
        ),
        (&["!code_fenced"], "* a\n+     **\n", "- a\n\n+     **\n"),
        (
            &["!code_fenced"],
            "+     **\n\n\n-     **\n",
            "-     **\n\n+     **\n",
        ),
        (&["!code_fenced"], "- *     -\n", "- *     -\n"),
        // The marker of an ordered item before is not part of a thematic break.
        (&["!code_fenced"], "1. +     --\n", "1. *     --\n"),
        (&["!code_fenced"], "1. +     **\n", "1. -     **\n"),
        // A list after one of `-` whose item starts with a rule of `*` takes the third, as `*`
        // would make the item a rule (#227).
        (&[], "- a\n+ ***\n", "- a\n\n+ ***\n"),
        // Fenced code makes no thematic break, so adjacent lists alternate as before.
        (
            &[],
            "- ```\n  --\n  ```\n* ```\n  --\n  ```\n",
            "- ```\n  --\n  ```\n\n* ```\n  --\n  ```\n",
        ),
        (
            &[],
            "- ```\n  ++\n  ```\n* ```\n  ++\n  ```\n",
            "- ```\n  ++\n  ```\n\n* ```\n  ++\n  ```\n",
        ),
        (
            &[],
            "- ```\n  - -\n  ```\n* ```\n  - -\n  ```\n",
            "- ```\n  - -\n  ```\n\n* ```\n  - -\n  ```\n",
        ),
    ]);
    let tildes = MarkdownOptions {
        fence: '~',
        ..MarkdownOptions::default()
    };
    check_with(
        &tildes,
        &[(
            &[],
            "- ```\n  --\n  ```\n* ```\n  --\n  ```\n",
            "- ~~~\n  --\n  ~~~\n\n* ~~~\n  --\n  ~~~\n",
        )],
    );
    // Without fences, the code is indented under the defaults too.
    let indented = MarkdownOptions {
        fences: false,
        ..MarkdownOptions::default()
    };
    check_with(
        &indented,
        &[(
            &[],
            "- ```\n  --\n  ```\n* ```\n  --\n  ```\n",
            "*     --\n\n+     --\n",
        )],
    );
    // A list whose item starts with a rule of its bullet's character, after a list of the bullet
    // that alternates with it, takes the third (#227).
    let dash_rule = MarkdownOptions {
        rule: '-',
        ..MarkdownOptions::default()
    };
    check_with(
        &dash_rule,
        &[(&[], "- a\n+ b\n- ***\n", "- a\n\n* b\n\n+ ---\n")],
    );
    let plus = MarkdownOptions {
        bullet: '+',
        ..MarkdownOptions::default()
    };
    check_with(
        &plus,
        &[
            (
                &["!code_fenced"],
                "+     ++\n+     **\n",
                "+     ++\n+     **\n",
            ),
            (
                &[],
                "- ```\n  ++\n  ```\n* ```\n  ++\n  ```\n",
                "+ ```\n  ++\n  ```\n\n* ```\n  ++\n  ```\n",
            ),
        ],
    );
}

/// Where every bullet would join the list before or make a line of indented code a thematic
/// break, the code is fenced, where the options read fences, whatever `MarkdownOptions::fences`
/// says, and otherwise the list has no spelling (#257).
#[test]
fn bullets_all_ruled_out() {
    let indented = MarkdownOptions {
        fences: false,
        ..MarkdownOptions::default()
    };
    let source = "- ```\n  --\n  ```\n- ```\n  **\n  ```\n* ```\n  --\n  ```\n* ```\n  **\n  ```\n";
    let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
    document.normalize();
    assert_eq!(
        to_markdown_with_options(&document, &indented).unwrap(),
        "- ```\n  --\n  ```\n- ```\n  **\n  ```\n\n+     --\n+     **\n"
    );
    document
        .set_parse_options(options(&["!code_fenced"]))
        .unwrap();
    let error = to_markdown_with_options(&document, &indented).unwrap_err();
    assert_eq!(error.rule_id, "unwritable-node");
}

/// Whether writing `document` returns `unwritable-node`.
fn unwritable(document: &Document<String>) -> bool {
    to_markdown(document).is_err_and(|error| error.rule_id == "unwritable-node")
}

/// A document of one heading of rank 1 holding `children`, under `options`.
fn heading(options: &ParseOptions, children: Vec<EventKind<'static>>) -> Document<String> {
    let mut document = Document::parse(String::new(), options).unwrap();
    let root = document.root();
    let heading = document
        .append_child(root, EventKind::Start(Tag::Heading(1)))
        .unwrap();
    for child in children {
        document.append_child(heading, child).unwrap();
    }
    document
}

/// Where no character can be escaped, the writer still finds a spelling where one reads back: a
/// `[^` that no footnote definition binds starts a link, a line that would start a block is
/// indented as a paragraph's continuation, a marker that the text holds is swapped for the other,
/// and a run of two tildes is struck through with one where the options read one.
#[test]
fn spellings_without_escapes() {
    let plain: &[&str] = &["!character_escape", "!character_reference"];
    let single: &[&str] = &[
        "!character_escape",
        "!character_reference",
        "strikethrough_single_tilde",
    ];
    check(&[
        (&["!label_start_link"], "[^t](u)\n", "[^t](u)\n"),
        (plain, "a\n    > b\n", "a\n    > b\n"),
        (plain, "_a*b_\n", "_a*b_\n"),
        (plain, "**a*b**\n", "__a*b__\n"),
        (single, "~a~~b~\n", "~a~~b~\n"),
        (single, "~~a~b~~\n", "~~a~b~~\n"),
        (
            &["!label_end", "!autolink"],
            "www.a.b\\.x\n",
            "www.a.b\\.x\n",
        ),
    ]);
}

/// Without fences, the bullet of each list is chosen so that the lists after it still have one
/// that makes no line of their indented code a thematic break.
#[test]
fn bullets_looking_ahead() {
    check(&[(
        &["!code_fenced"],
        "* a\n-     **\n+     --\n+     **\n",
        "* a\n\n-     **\n\n+     --\n+     **\n",
    )]);
    // A list whose indented code leaves it no bullet fences its code, so the list after it
    // keeps the bullet that it needs.
    let indented = MarkdownOptions {
        fences: false,
        ..MarkdownOptions::default()
    };
    let source =
        "* a\n+ ```\n  --\n  ```\n+ ```\n  **\n  ```\n- ```\n  --\n  ```\n- ```\n  **\n  ```\n";
    let mut document = Document::parse(source, &ParseOptions::default()).unwrap();
    document.normalize();
    assert_eq!(
        to_markdown_with_options(&document, &indented).unwrap(),
        "- a\n\n* ```\n  --\n  ```\n* ```\n  **\n  ```\n\n+     --\n+     **\n"
    );
}

/// A definition whose label starts with `^` is joined to the definition before it, indented so
/// it starts no footnote definition, and otherwise has no spelling, as without indented code no
/// indent keeps it from starting one (#208).
#[test]
fn caret_definitions() {
    check(&[
        (&[], "[a]: /u\n    [^b]: #\n", "[a]: /u\n    [^b]: #\n"),
        (
            &[],
            "- [a]: /u\n      [^b]: #\n      [^c]: d\n",
            "- [a]: /u\n      [^b]: #\n      [^c]: d\n",
        ),
        (&[], "[^a b]: /u\n", "[^a b]: /u\n"),
    ]);
    let mut document = Document::parse(
        String::from("[^b]: /u\n"),
        &options(&["!footnote_definition"]),
    )
    .unwrap();
    assert_eq!(to_markdown(&document).unwrap(), "[^b]: /u\n");
    document.set_parse_options(ParseOptions::default()).unwrap();
    assert!(unwritable(&document));
    let mut document = Document::parse(
        String::from("[a]: /u\n[^b]: #\n"),
        &options(&["!footnote_definition"]),
    )
    .unwrap();
    document
        .set_parse_options(options(&["!code_indented"]))
        .unwrap();
    assert!(unwritable(&document));
}

/// Without ATX headings, an empty heading has no spelling, as a setext heading needs content, and
/// `normalize` leaves it, as it removes no heading (#270). A heading that ends in a hard break has
/// none either, as the parser drops a break there, which `normalize` does too (#271).
#[test]
fn headings_without_spelling() {
    let mut document = heading(&options(&["!heading_atx"]), Vec::new());
    assert!(unwritable(&document));
    document.normalize();
    assert!(unwritable(&document));
    for changes in [&[][..], &["!heading_atx"]] {
        let options = options(changes);
        let mut document = heading(
            &options,
            vec![EventKind::Text(Text::Literal("ab")), EventKind::HardBreak],
        );
        assert!(unwritable(&document), "{:?}", changes);
        document.normalize();
        let markdown = to_markdown(&document).unwrap();
        assert_eq!(
            markdown,
            if changes.is_empty() {
                "# ab\n"
            } else {
                "ab\n==\n"
            }
        );
    }
    assert!(unwritable(&heading(
        &ParseOptions::default(),
        vec![EventKind::HardBreak]
    )));
}

/// An HTML block of kinds 1–5 that its last line does not close takes the blocks after it, so it
/// has no spelling there, and `normalize` closes it with a line of its end condition (#191).
#[test]
fn unclosed_html_blocks() {
    for (source, expected) in [
        ("<pre>\n", "<pre>\n</pre>\n\na\n"),
        ("- <PRE>\n- b\n", "- <PRE>\n  </PRE>\n  a\n- b\n"),
        ("> <!-- x\n", "> <!-- x\n> -->\n>\n> a\n"),
        ("<?\n", "<?\n?>\n\na\n"),
        ("<!A\n", "<!A\n>\n\na\n"),
        ("<![CDATA[\n", "<![CDATA[\n]]>\n\na\n"),
    ] {
        let mut document = Document::parse(String::from(source), &ParseOptions::default()).unwrap();
        let html = document
            .descendants(document.root())
            .find(|&id| matches!(document.node(id).unwrap().kind, EventKind::Html { .. }))
            .unwrap();
        let parent = document.node(html).unwrap().parent().unwrap();
        // The line ending a tight item renders between its blocks is a separator's.
        if matches!(
            document.node(parent).unwrap().kind,
            EventKind::Start(Tag::Item { .. })
        ) {
            document
                .append_child(
                    parent,
                    EventKind::Separator {
                        value: Text::Literal("\n"),
                        conditional: false,
                    },
                )
                .unwrap();
        }
        let paragraph = document
            .append_child(parent, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        document
            .append_child(paragraph, EventKind::Text(Text::Literal("a")))
            .unwrap();
        assert!(unwritable(&document), "{:?}", source);
        assert!(!document.is_normalized(), "{:?}", source);
        document.normalize();
        assert!(document.is_normalized(), "{:?}", source);
        assert_eq!(to_markdown(&document).unwrap(), expected, "{:?}", source);
        assert!(
            matches!(
                round_trip(document, &MarkdownOptions::default(), false),
                Outcome::Pass
            ),
            "{:?} does not round trip",
            source
        );
    }
}

/// Emphasis that ends in punctuation before code with a language has no spelling where
/// `inline_code_info` reads the language: `*` cannot close before the language, and `_` would
/// join it (#185).
#[test]
fn emphasis_before_a_language() {
    let mut document =
        Document::<String>::parse(String::from("*a*rust:`v`"), &options(&["inline_code_info"]))
            .unwrap();
    let text = document
        .descendants(document.root())
        .find(|&id| matches!(document.node(id).unwrap().kind, EventKind::Text(_)))
        .unwrap();
    document
        .set_kind(text, EventKind::Text(Text::from(String::from("a."))))
        .unwrap();
    assert!(unwritable(&document));
}

/// Without `\` hard breaks, trailing spaces alone on a line make it blank, so a hard break after
/// another, or at the start of a paragraph, has no spelling.
#[test]
fn hard_breaks_without_spelling() {
    let options = options(&["!hard_break_escape"]);
    for breaks in [
        vec![EventKind::Text(Text::Literal("a")), EventKind::HardBreak],
        vec![EventKind::HardBreak],
    ] {
        let mut document = Document::parse(String::new(), &options).unwrap();
        let root = document.root();
        let paragraph = document
            .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
            .unwrap();
        for child in breaks
            .into_iter()
            .chain([EventKind::HardBreak, EventKind::Text(Text::Literal("b"))])
        {
            document.append_child(paragraph, child).unwrap();
        }
        assert!(unwritable(&document));
    }
}

/// Under `html_block_blank_lines`, an HTML block of kind 6 or 7 takes the blocks after it with the
/// blank line before them, so one that keeps a final line ending without a blank line, which
/// only the end of the document gives, has no spelling before another block.
#[test]
fn html_block_line_ending_before_a_block() {
    let mut document = Document::parse(
        String::from("<div>\n"),
        &options(&["html_block_blank_lines"]),
    )
    .unwrap();
    let root = document.root();
    let paragraph = document
        .append_child(root, EventKind::Start(Tag::Paragraph { tight: false }))
        .unwrap();
    document
        .append_child(paragraph, EventKind::Text(Text::Literal("a")))
        .unwrap();
    document.normalize();
    assert!(unwritable(&document));
}

/// Whether `source`, read under `read` and normalized, has no spelling under `write`.
fn unwritable_under(read: &[&str], write: &[&str], source: &str) -> bool {
    let mut document = Document::parse(String::from(source), &options(read)).unwrap();
    document.normalize();
    document.set_parse_options(options(write)).unwrap();
    unwritable(&document)
}

/// Without indented code, an indented line starts the blocks it would start unindented, so
/// phrasing whose line would start one has no spelling: inline HTML after a definition, after a
/// line ending or in a tag's attribute.
#[test]
fn lines_without_indented_code() {
    for (read, source) in [
        (&[][..], "[a]: /u\n    <div>\n"),
        (&["!html_flow"], "x <a b='\n<div>'>\n"),
        (&["!html_flow"], "x\n<div>\n"),
    ] {
        assert!(
            unwritable_under(read, &["!code_indented"], source),
            "{:?}",
            source
        );
    }
}

/// Without escapes or references, inline HTML alone on a paragraph's first line starts an HTML
/// block, and text that forms a literal autolink reads back as one, so neither has a spelling,
/// with or without footnote calls in the paragraph.
#[test]
fn phrasing_without_escapes() {
    let plain: &[&str] = &["!character_escape", "!character_reference"];
    assert!(unwritable_under(&["!html_flow"], plain, "<b>\ny\n"));
    assert!(unwritable_under(
        &["!autolink_literal"],
        plain,
        "a www.a.b\n"
    ));
    assert!(unwritable_under(
        &["!autolink_literal"],
        plain,
        "[^1] www.a.b\n\n[^1]: n\n"
    ));
}

/// Without explicit links, a link is written bare, and in a paragraph with footnote calls, which
/// is read back with its definitions, it is written where its links read back the same.
#[test]
fn bare_links_with_footnote_calls() {
    let bare: &[&str] = &["!label_start_link", "!autolink"];
    check(&[(
        bare,
        "[^1] https://a.b.\n\n[^1]: n\n",
        "[^1] https://a.b\\.\n\n[^1]: n\n",
    )]);
    assert!(unwritable_under(
        &[],
        bare,
        "[^1] <https://a.b>x\n\n[^1]: n\n"
    ));
}
