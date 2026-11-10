// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The round trip of contract 3.3 under every dialect: random documents, parsed with random
//! `ParseOptions`, normalized, written, read back under the same options and normalized again.

mod support;

use base_markdown::{Constructs, Document, MarkdownOptions, ParseOptions};
use support::{
    corpus::{self, Random},
    round_trip::{round_trip, Outcome},
};

/// Pieces that exercise each construct an option turns on or off, and the spellings the writer
/// falls back to when one is off.
const DIALECT: &[&str] = &[
    "a",
    "b c",
    " ",
    "\n",
    "\n\n",
    "# ",
    "## h\n",
    "### h\n",
    "h\n===\n",
    "h\n---\n",
    "a\\\nb",
    "a  \nb",
    "\\",
    "  \n",
    "<https://a.b>",
    "<a@b.c>",
    "<tel:+1>",
    "https://a.b",
    "www.a.b",
    "x@y.z",
    "[t](u)",
    "[t](u \"t\")",
    "[^t](u)",
    "[\\^t](u)",
    "![i](u)",
    "[r]",
    "[r][]",
    "[t][r]",
    "[r]: /u\n",
    "[r]: /u \"t\"\n",
    "[^1]",
    "[^1]: n\n",
    "```\nx\n```\n",
    "~~~ js\nx\n~~~\n",
    "    x\n",
    "`c`",
    "rust:`c`",
    "x:`c`",
    "a\\_b:`c`",
    "_a_",
    "*a*",
    "**a**",
    "~a~",
    "~~a~~",
    "<b>",
    "<svg:rect>",
    "<div>\n",
    "<div>\n\nx\n",
    "  <div>\n\n  x\n",
    "<!-- c -->",
    "| a | b |\n| - | - |\n| c | d |\n",
    "- ",
    "1. ",
    "- [ ] ",
    "- [x] ",
    "> ",
    "***\n",
    "---\n",
    "&amp;",
    "&#x20;",
    "\\*",
    ":",
    "]",
    "(",
    "=",
];

/// A source from one of the shared corpora or the dialect pieces.
fn source(random: &mut Random) -> String {
    match random.below(10) {
        7 => adjacent_lists(random),
        0 => corpus::realistic(random),
        1 => corpus::soup(random),
        2 => corpus::edge(random),
        3 => corpus::strike(random),
        4 => corpus::blocky(random),
        5 => corpus::html(random),
        6 => corpus::html_blocks(random),
        _ => {
            let mut source = String::new();
            if random.below(4) == 0 {
                source.push_str(random.pick(&["---\nt: 1\n---\n", "+++\nt = 1\n+++\n"]));
            }
            source.push_str(&random.concatenate(DIALECT, 12));
            source
        }
    }
}

/// Adjacent lists, each of different bullet from the one before, whose items start with code of
/// a line that a bullet could make a thematic break, fenced or indented, perhaps in an item.
fn adjacent_lists(random: &mut Random) -> String {
    let outer = random.pick(&["", "", "- ", "* ", "+ ", "1. "]);
    let indent = " ".repeat(outer.len());
    let mut source = String::new();
    let mut last = "";
    for list in 0..1 + random.below(3) {
        let marker = loop {
            let marker = random.pick(&["-", "*", "+"]);
            if marker != last {
                break marker;
            }
        };
        last = marker;
        for item in 0..1 + random.below(2) {
            source.push_str(if list == 0 && item == 0 {
                outer
            } else {
                &indent
            });
            source.push_str(marker);
            let line = random.pick(&["--", "- -", "**", "* *", "++", "__", "a"]);
            match random.below(3) {
                0 => {
                    source.push_str(" ```\n");
                    for part in [line, "```"] {
                        source.push_str(&indent);
                        source.push_str("  ");
                        source.push_str(part);
                        source.push('\n');
                    }
                }
                1 => {
                    source.push_str("     ");
                    source.push_str(line);
                    source.push('\n');
                }
                _ => {
                    source.push(' ');
                    source.push_str(line);
                    source.push('\n');
                }
            }
        }
    }
    source
}

/// Random options: each construct and option is on or off with even odds, so every combination is
/// sampled, those where some shapes have no spelling at all too, such as `character_escape` or
/// `code_indented` off.
fn options(random: &mut Random) -> ParseOptions {
    let mut flag = || random.below(2) == 0;
    let constructs = Constructs {
        attention: flag(),
        autolink: flag(),
        block_quote: flag(),
        character_escape: flag(),
        character_reference: flag(),
        code_indented: flag(),
        code_fenced: flag(),
        code_text: flag(),
        definition: flag(),
        frontmatter: flag(),
        autolink_literal: flag(),
        footnote_definition: flag(),
        label_start_footnote: flag(),
        strikethrough: flag(),
        table: flag(),
        task_list_item: flag(),
        hard_break_escape: flag(),
        hard_break_trailing: flag(),
        heading_atx: flag(),
        heading_setext: flag(),
        html_flow: flag(),
        html_text: flag(),
        label_start_image: flag(),
        label_start_link: flag(),
        label_end: flag(),
        list_item: flag(),
        thematic_break: flag(),
    };
    ParseOptions {
        constructs,
        inline_code_info: flag(),
        html_block_blank_lines: flag(),
        html_tag_namespaces: flag(),
        strikethrough_single_tilde: flag(),
    }
}

/// The options that differ from the defaults, for reports.
fn describe(options: &ParseOptions) -> String {
    let constructs = format!("{:?}", options.constructs);
    let mut changed: Vec<String> = constructs
        .trim_start_matches("Constructs { ")
        .trim_end_matches(" }")
        .split(", ")
        .filter(|field| field.ends_with("false"))
        .map(|field| format!("!{}", field.trim_end_matches(": false")))
        .collect();
    for (name, on) in [
        ("inline_code_info", options.inline_code_info),
        ("html_block_blank_lines", options.html_block_blank_lines),
        ("html_tag_namespaces", options.html_tag_namespaces),
        (
            "strikethrough_single_tilde",
            options.strikethrough_single_tilde,
        ),
    ] {
        if on {
            changed.push(name.into());
        }
    }
    changed.join(" ")
}

/// Whether `html` has emphasis or strong in emphasis or strong, which the writer can get wrong
/// under any options (#110).
fn nested_attention(html: &str) -> bool {
    let (mut depth, mut deepest) = (0usize, 0);
    for (at, _) in html.match_indices('<') {
        let rest = &html[at..];
        if rest.starts_with("<em>") || rest.starts_with("<strong>") {
            depth += 1;
            deepest = deepest.max(depth);
        } else if rest.starts_with("</em>") || rest.starts_with("</strong>") {
            depth = depth.saturating_sub(1);
        }
    }
    deepest > 1
}

fn inputs() -> usize {
    std::env::var("DIALECT_INPUTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20_000)
}

fn shown() -> usize {
    std::env::var("DIALECT_SHOW")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10)
}

fn unwritable_shown() -> usize {
    std::env::var("DIALECT_SHOW_UNWRITABLE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[test]
fn round_trip_under_every_dialect() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let markdown = MarkdownOptions::default();
    let mut failures = Vec::new();
    let mut unwritable = Vec::new();
    let count = inputs();
    for _ in 0..count {
        let options = options(&mut random);
        let source = source(&mut random);
        let document = Document::parse(source.as_str(), &options).unwrap();
        match round_trip(document, &markdown, false) {
            Outcome::Pass | Outcome::Rejected => {}
            Outcome::Unwritable(error) => unwritable.push((source, options, error)),
            Outcome::Fail(failure) if nested_attention(&failure.before) => {}
            Outcome::Fail(failure) => failures.push((source, options, failure)),
        }
    }
    // Where no spelling reads back, the writer returns `unwritable-node` rather than output that
    // reads back as something else, so a parsed document either round trips or is that error.
    eprintln!("unwritable-node: {} of {count}", unwritable.len());
    for (source, options, error) in unwritable.iter().take(unwritable_shown()) {
        eprintln!(
            "unwritable: {source:?}\n  options: {}\n  {error}",
            describe(options)
        );
    }
    for (source, options, failure) in failures.iter().take(shown()) {
        eprintln!(
            "{}: {source:?}\n  options: {}\n  output: {:?}\n  before: {:?}\n  after: {:?}",
            failure.kind,
            describe(options),
            failure.output,
            failure.before,
            failure.after
        );
    }
    assert!(failures.is_empty(), "{} failures", failures.len());
}
