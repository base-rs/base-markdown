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
    match random.below(9) {
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

/// Random options: each construct and option is on or off with even odds, so every combination is
/// sampled.
///
/// Step 4 of #257 handles the dialects left out here, where some shapes have no spelling at all
/// and the writer will return `unwritable-node`:
/// - `character_escape` off, where no backslash escape is read, and the writer escapes with them
///   everywhere;
/// - `code_indented` off, where an indented line starts no code, and the writer indents lines to
///   keep them in a paragraph or label.
fn options(random: &mut Random) -> ParseOptions {
    let mut flag = || random.below(2) == 0;
    let constructs = Constructs {
        attention: flag(),
        autolink: flag(),
        block_quote: flag(),
        character_escape: true,
        character_reference: flag(),
        code_indented: true,
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

/// Whether `output` holds a spelling that the dialect of `options` does not read, which the writer
/// falls back to for a shape that has no other: step 4 of #257 makes those `unwritable-node`.
/// - Without `Constructs::character_reference`, what the writer encodes as a reference, such as
///   a space at the end of a line or a backtick in a backtick fence's info string.
/// - Without a label start or `Constructs::label_end`, a link that neither autolink can hold, such
///   as one with a title or a bare one next to text, written `[text](destination)`.
/// - Without `Constructs::code_fenced`, indented code after a list or other indented code, which
///   the writer separates with `<!---->`, or which needs a fence where indented code would
///   continue the block before it.
fn unwritable(options: &ParseOptions, output: &str) -> bool {
    let constructs = &options.constructs;
    !constructs.character_reference && output.contains("&#")
        || !(constructs.label_start_link && constructs.label_end) && output.contains("](")
        || !constructs.code_fenced && (output.contains("<!---->") || output.contains("```"))
}

/// Whether `document` has a shape that the writer gets wrong under any options, the default ones
/// too, which are gaps on main rather than in following the dialect:
/// - an HTML block indented with spaces or tabs after a list, footnote definition or block quote,
///   or in a block quote (#229): written after a list or footnote definition, the indented line
///   continues it, and in a block quote, a tab after spaces spans other columns after the quote's
///   prefix;
/// - an HTML block that ends an item before another, which, when the list is written loose and
///   the block has no end, takes the blank line between the items;
/// - a definition whose label starts with `^`, which the writer can join to a footnote
///   definition's line (#208), left to step 4 of #257.
fn known_gap<S: AsRef<str>>(document: &Document<S>) -> bool {
    use base_markdown::stream::{EventKind, Tag, TagKind};
    let mut quotes = 0;
    let mut previous = None;
    let mut html_item_end = false;
    for event in document.events() {
        let kind = event.unwrap().kind;
        if html_item_end && matches!(kind, EventKind::Start(Tag::Item { .. })) {
            return true;
        }
        html_item_end = match (&previous, &kind) {
            (Some(EventKind::Html { block: true, .. }), EventKind::End(TagKind::Item)) => true,
            (_, EventKind::Separator { .. }) => html_item_end,
            _ => false,
        };
        match &kind {
            EventKind::Definition { label, .. }
                if label.resolve(document.source()).unwrap().starts_with('^') =>
            {
                return true;
            }
            EventKind::Start(Tag::BlockQuote) => quotes += 1,
            EventKind::End(TagKind::BlockQuote) => quotes -= 1,
            EventKind::Separator { .. } => continue,
            EventKind::Html { value, block: true }
                if value
                    .resolve(document.source())
                    .unwrap()
                    .starts_with([' ', '\t'])
                    && (quotes > 0
                        || matches!(
                            previous,
                            Some(EventKind::End(
                                TagKind::List | TagKind::FootnoteDefinition | TagKind::BlockQuote
                            ))
                        )) =>
            {
                return true;
            }
            _ => {}
        }
        previous = Some(kind);
    }
    false
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

/// Whether `output` has an item marked `*` whose content is a thematic break of `*`, which reads
/// back as a break under any options (#227).
fn star_rule(output: &str) -> bool {
    output.lines().any(|line| {
        line.trim_start_matches(['>', ' ', '-', '+'])
            .starts_with("* ***")
    })
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

#[test]
fn round_trip_under_every_dialect() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let markdown = MarkdownOptions::default();
    let mut failures = Vec::new();
    for _ in 0..inputs() {
        let options = options(&mut random);
        let source = source(&mut random);
        let document = Document::parse(source.as_str(), &options).unwrap();
        let known = known_gap(&document);
        match round_trip(document, &markdown, false) {
            Outcome::Pass | Outcome::Rejected => {}
            Outcome::Fail(failure)
                if unwritable(&options, &failure.output)
                    || known
                    || nested_attention(&failure.before)
                    || star_rule(&failure.output) => {}
            Outcome::Fail(failure) => failures.push((source, options, failure)),
        }
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
