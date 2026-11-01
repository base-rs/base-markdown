// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Round-trip fuzz harness for normalizing and serializing Markdown, with fixed seeds and a checked-in baseline.
//!
//! Each input is parsed with [`ParseOptions::normalize`], the syntax `normalize_markdown` reads, or built by hand as a tree. It is then normalized, serialized with the default Markdown options, parsed again with the same options and normalized again. An input fails when either document is not normalized after normalizing (`normalized`), when the HTML of the two documents differs (`html`), when serializing the second document gives different Markdown (`unstable`), or when serializing a parsed document returns an error (`error`). A panic is a hard failure whatever the baseline. A hand-built tree that the serializer rejects with an error is skipped and counted as rejected.
//!
//! Every corpus is generated from its own fixed seed by a small xorshift generator, so the inputs, and the counts, are the same on every platform, and the first inputs of a larger run are the inputs of a smaller one. At the default size, the failure count of each corpus is compared with `tests/roundtrip_fuzz_baseline.txt`: the test fails when a corpus has more failures than its baseline, and prints a hint to lower the baseline when it has fewer.
//!
//! Environment variables:
//!
//! - `FUZZ_INPUTS=1000000` sets the inputs per corpus. The baseline is only checked at the default size. Run large sizes in release.
//! - `FUZZ_OUTPUT=directory` writes `<corpus>.txt`, the sorted, deduplicated failing inputs of each corpus, one Rust string literal per line, and `<corpus>-details.txt`, each failure with its kind, class, output and both HTML renderings.
//! - `FUZZ_BLESS=1` rewrites the baseline with the counts of this run, at the default size only.
//!
//! See `context/roundtrip-fuzz.md` for comparing a branch with main.

mod support;

use base_markdown::{
    to_markdown_with_options, CompileOptions, Document, MarkdownOptions, ParseOptions,
};
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use support::fixture::{
    AlignKind, Break, Delete, Emphasis, Heading, InlineCode, Link, Node, Paragraph, Root, Strong,
    Table, TableCell, TableRow, Text,
};

/// Inputs per corpus when `FUZZ_INPUTS` is not set; the baseline records failures at this size.
const DEFAULT_INPUTS: usize = 10_000;

const BASELINE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/roundtrip_fuzz_baseline.txt"
);

/// A xorshift generator: deterministic, portable and dependency free.
struct Random(u64);

impl Random {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    fn concatenate(&mut self, items: &[&str], most: usize) -> String {
        let mut value = String::new();
        for _ in 0..1 + self.below(most) {
            value.push_str(self.pick(items));
        }
        value
    }
}

/// Prose-like pieces: words, markers, escapes, references, links, code, block starts, tables and literal autolinks.
const REALISTIC: &[&str] = &[
    "a",
    "foo",
    "bar",
    " ",
    " ",
    " ",
    "x",
    "5 min",
    "~",
    "~",
    "~~",
    "~5",
    "~/path",
    "\\~",
    "\\\\",
    "*",
    "_",
    "**",
    "`c~d`",
    "`",
    "[t](u)",
    "[t~](u~ \"t~\")",
    "[",
    "](u)",
    "\n",
    "\n- ",
    "\n> ",
    "\n| a | b |\n| - | - |\n| ",
    " | ",
    "&#x20;",
    "&#126;",
    "&amp;",
    "# ",
    "1. ",
    ".",
    ",",
    ":",
    "(",
    ")",
    "!",
    "$",
    "~~del~~",
    "~x~",
    "\\",
    "  \n",
    "<b>",
    "www.a.b",
    "x@y.z",
    "é",
    "\u{a0}",
];

/// Characters of attention, strikethrough, escapes, punctuation and line endings, one at a time.
const SOUP: &[&str] = &["*", "_", "~", "a", ".", " ", "\n", "\\"];

/// Markers next to encoded whitespace, encoded letters and escaped markers.
const EDGE: &[&str] = &[
    "*", "**", "_", "__", "~", "~", "~~", "\\~", "&#x20;", "&#xA;", "&#x9;", "&#x61;", "&#x62;",
    "a", "b", " ", "(", ".", "\\", "\u{a0}", "|", "x ~a ",
];

/// Block starts for the edge corpus, so edges also meet headings, table cells, items and quotes.
const EDGE_BLOCK: &[&str] = &["# ", "| ", "- ", "> "];

/// Tilde runs of every length among letters, digits, attention, escapes, code and links.
const STRIKE: &[&str] = &[
    "~", "~", "~~", "~~", "~~~", "a", "b", "5", " ", " ", "*", "_", "(", ")", ".", "\\~", "del",
    "\n", "&#x20;", "[t](u)", "`~`",
];

/// Text values in hand-built trees, which the serializer must escape or encode.
const TREE_TEXT: &[&str] = &[
    "~", "~", "~~", "a", " ", ".", "(", "\\", "*", "_", "&", "b", "~~~", "\u{a0}", "\n", "&#x20;",
];

fn realistic(random: &mut Random) -> Input {
    Input::Source(random.concatenate(REALISTIC, 12))
}

fn soup(random: &mut Random) -> Input {
    Input::Source(random.concatenate(SOUP, 12))
}

fn edge(random: &mut Random) -> Input {
    let mut source = String::new();
    if random.below(4) == 0 {
        source.push_str(random.pick(EDGE_BLOCK));
    }
    source.push_str(&random.concatenate(EDGE, 12));
    if source.starts_with("| ") {
        source.push_str(" |\n| - |");
    }
    Input::Source(source)
}

fn strike(random: &mut Random) -> Input {
    Input::Source(random.concatenate(STRIKE, 10))
}

fn text(value: String) -> Node {
    Node::Text(Text {
        value,
        position: None,
    })
}

/// Phrasing of one to four nodes. `outside` lists the attention and links that enclose it, which are not nested in themselves. A hard break is never the first or last child, as Markdown cannot write a break at the edge of a block or of attention.
fn phrasing(random: &mut Random, depth: usize, outside: &[&str]) -> Vec<Node> {
    let mut children = Vec::new();
    for _ in 0..1 + random.below(4) {
        let mut choice = if depth > 1 {
            ["text", "text", "text", "code", "break"][random.below(5)]
        } else {
            [
                "text", "text", "text", "text", "delete", "emphasis", "strong", "code", "break",
                "link",
            ][random.below(10)]
        };
        if outside.contains(&choice) {
            choice = "text";
        }
        let mut inside = outside.to_vec();
        inside.push(choice);
        children.push(match choice {
            "text" => text(random.concatenate(TREE_TEXT, 4)),
            "delete" => Node::Delete(Delete {
                children: phrasing(random, depth + 1, &inside),
                position: None,
            }),
            "emphasis" => Node::Emphasis(Emphasis {
                children: phrasing(random, depth + 1, &inside),
                position: None,
            }),
            "strong" => Node::Strong(Strong {
                children: phrasing(random, depth + 1, &inside),
                position: None,
            }),
            "code" => Node::InlineCode(InlineCode {
                value: random.pick(&["~", "a~", "~~"]).into(),
                lang: None,
                position: None,
            }),
            "break" => Node::Break(Break { position: None }),
            _ => Node::Link(Link {
                children: phrasing(random, depth + 1, &inside),
                position: None,
                url: random.pick(&["u", "u~", "http://www.a~b.c"]).into(),
                title: None,
            }),
        });
    }
    for index in [0, children.len() - 1] {
        if matches!(children[index], Node::Break(_)) {
            children[index] = text(random.concatenate(TREE_TEXT, 4));
        }
    }
    children
}

/// A paragraph, heading or one-cell table of random phrasing.
fn tree(random: &mut Random) -> Input {
    let children = phrasing(random, 0, &[]);
    let block = match random.below(4) {
        0 => Node::Heading(Heading {
            children,
            position: None,
            depth: 1,
        }),
        1 => Node::Table(Table {
            children: vec![Node::TableRow(TableRow {
                children: vec![Node::TableCell(TableCell {
                    children,
                    position: None,
                })],
                position: None,
            })],
            position: None,
            align: vec![AlignKind::None],
        }),
        _ => Node::Paragraph(Paragraph {
            children,
            position: None,
        }),
    };
    Input::Tree(Node::Root(Root {
        children: vec![block],
        position: None,
    }))
}

enum Input {
    Source(String),
    Tree(Node),
}

impl Input {
    fn describe(&self) -> String {
        match self {
            Self::Source(source) => format!("{source:?}"),
            Self::Tree(node) => format!("{:?}", format!("{node:?}")),
        }
    }
}

struct Corpus {
    name: &'static str,
    seed: u64,
    generate: fn(&mut Random) -> Input,
    /// Marker for both emphasis and strong.
    marker: char,
}

const CORPORA: &[Corpus] = &[
    Corpus {
        name: "realistic",
        seed: 0x9E37_79B9_7F4A_7C17,
        generate: realistic,
        marker: '*',
    },
    Corpus {
        name: "soup",
        seed: 0x9E37_79B9_7F4A_7C14,
        generate: soup,
        marker: '*',
    },
    Corpus {
        name: "edge",
        seed: 0x5EED,
        generate: edge,
        marker: '*',
    },
    Corpus {
        name: "strike",
        seed: 0x9E37_79B9_7F4A_7C16,
        generate: strike,
        marker: '*',
    },
    Corpus {
        name: "tree-asterisk",
        seed: 0xDEAD_BEEF,
        generate: tree,
        marker: '*',
    },
    Corpus {
        name: "tree-underscore",
        seed: 0xDEAD_BEEF,
        generate: tree,
        marker: '_',
    },
];

const KINDS: [&str; 4] = ["html", "unstable", "normalized", "error"];

struct Failure {
    kind: &'static str,
    output: String,
    before: String,
    after: String,
}

enum Outcome {
    Pass,
    Rejected,
    Fail(Failure),
}

/// The HTML of `document`, without what depends only on where line endings were in the source: the line ending at the end of the document, which a hand-built tree has no source for, and the one after a hard break, which is part of the break when parsed.
fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    output.trim_end().replace("<br />\n", "<br />")
}

fn fail(kind: &'static str, output: String, before: String, after: String) -> Outcome {
    Outcome::Fail(Failure {
        kind,
        output,
        before,
        after,
    })
}

/// Normalize, serialize, parse and normalize `document`, and compare. A serializer error is a failure unless `may_reject`.
fn round_trip<S: AsRef<str>>(
    mut document: Document<S>,
    options: &MarkdownOptions,
    may_reject: bool,
) -> Outcome {
    document.normalize();
    let before = html(&document);
    if !document.is_normalized() {
        return fail("normalized", String::new(), before, String::new());
    }
    let output = match to_markdown_with_options(&document, options) {
        Ok(output) => output,
        Err(_) if may_reject => return Outcome::Rejected,
        Err(error) => return fail("error", error.to_string(), before, String::new()),
    };
    let mut reparsed = Document::parse(output.as_str(), &ParseOptions::normalize()).unwrap();
    reparsed.normalize();
    let after = html(&reparsed);
    if !reparsed.is_normalized() {
        return fail("normalized", output, before, after);
    }
    if after != before {
        return fail("html", output, before, after);
    }
    match to_markdown_with_options(&reparsed, options) {
        Ok(again) if again == output => Outcome::Pass,
        Ok(again) => fail("unstable", output, before, again),
        Err(error) => fail("error", output, before, error.to_string()),
    }
}

fn check(input: &Input, options: &MarkdownOptions) -> Outcome {
    match input {
        Input::Source(source) => round_trip(
            Document::parse(source.as_str(), &ParseOptions::normalize()).unwrap(),
            options,
            false,
        ),
        Input::Tree(node) => round_trip(node.document(), options, true),
    }
}

/// Approximate buckets for known issues, by simple rules on the input, the output and both HTML renderings. The first matching rule wins.
struct Classifier {
    cell_edge: Regex,
}

impl Classifier {
    fn new() -> Self {
        Self {
            cell_edge: Regex::new(r"(<t[dh][^>]*>)\s+|\s+(</t[dh]>)").unwrap(),
        }
    }

    fn classify(&self, input: &str, failure: &Failure) -> &'static str {
        let (before, after, output) = (&failure.before, &failure.after, &failure.output);
        let differs =
            |pattern: &str| before.matches(pattern).count() != after.matches(pattern).count();
        let without_line_endings = |html: &str| html.replace("</p>\n<p>", "").replace('\n', "");
        if failure.kind != "html" {
            return failure.kind;
        }
        let break_at_edge = before.ends_with("<br />")
            || before.contains("<br /></")
            || [
                "<p>", "<h1>", "<em>", "<strong>", "<del>", "<th>", "<td>", "<li>",
            ]
            .iter()
            .any(|start| before.contains(&format!("{start}<br />")));
        if differs("<br />") || break_at_edge {
            return "hard break at an edge or before a line ending";
        }
        if before.contains("</code><code>") {
            return "adjacent code spans";
        }
        if before.contains("<table>") && differs("<p>") {
            return "line ending in a table cell";
        }
        if differs("<table>") {
            return "#108 lazy table line becomes a table";
        }
        if differs("<pre>") && (output.starts_with("```") || output.starts_with("~~~")) {
            return "#107 fence at document start reads as frontmatter";
        }
        if self.cell_edge.replace_all(before, "$1$2") == self.cell_edge.replace_all(after, "$1$2") {
            return "#109 whitespace at a table cell edge";
        }
        // A source writes an encoded line ending as `&#xA;`, and a tree's description as `\n`.
        let encoded_line_ending = input.contains("&#xA;") || input.contains("\\n");
        if without_line_endings(before) == without_line_endings(after)
            || (encoded_line_ending && (differs("<p>") || differs("<h1>") || differs("<li>")))
        {
            return "#112 encoded line ending written raw";
        }
        if differs("<a href")
            && ["www.", "@", "://"]
                .iter()
                .any(|literal| input.contains(literal))
        {
            if differs("<code>") {
                return "#97 literal autolink absorbs a code span";
            }
            return "literal autolink, not escaped by design";
        }
        if differs("<del>") {
            return "strikethrough";
        }
        if nesting(before) > 1 || nesting(after) > 1 {
            if output.contains("&#x") {
                return "#101 encoded edge in nested attention";
            }
            return "#110 or #111 nested attention";
        }
        "other"
    }
}

/// The deepest nesting of emphasis and strong in `html`.
fn nesting(html: &str) -> usize {
    let mut depth = 0usize;
    let mut deepest = 0;
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        rest = &rest[at..];
        if rest.starts_with("<em>") || rest.starts_with("<strong>") {
            depth += 1;
            deepest = deepest.max(depth);
        } else if rest.starts_with("</em>") || rest.starts_with("</strong>") {
            depth = depth.saturating_sub(1);
        }
        rest = &rest[1..];
    }
    deepest
}

#[derive(Default)]
struct Tally {
    failures: usize,
    rejected: usize,
    kinds: BTreeMap<&'static str, usize>,
    classes: BTreeMap<&'static str, usize>,
    panics: Vec<String>,
    inputs: Vec<String>,
    details: String,
}

fn run(corpus: &Corpus, count: usize, record: bool) -> Tally {
    let options = MarkdownOptions {
        emphasis: corpus.marker,
        strong: corpus.marker,
        ..MarkdownOptions::default()
    };
    let classifier = Classifier::new();
    let mut random = Random(corpus.seed);
    let mut tally = Tally::default();
    for _ in 0..count {
        let input = (corpus.generate)(&mut random);
        let failure = match catch_unwind(AssertUnwindSafe(|| check(&input, &options))) {
            Ok(Outcome::Pass) => continue,
            Ok(Outcome::Rejected) => {
                tally.rejected += 1;
                continue;
            }
            Ok(Outcome::Fail(failure)) => failure,
            Err(_) => {
                tally.panics.push(input.describe());
                continue;
            }
        };
        let class = match &input {
            Input::Source(source) => classifier.classify(source, &failure),
            Input::Tree(node) => classifier.classify(&format!("{node:?}"), &failure),
        };
        tally.failures += 1;
        *tally.kinds.entry(failure.kind).or_default() += 1;
        *tally.classes.entry(class).or_default() += 1;
        if record {
            tally.inputs.push(input.describe());
            writeln!(
                tally.details,
                "{}\t{}\t{}\t{:?}\t{:?}\t{:?}",
                failure.kind,
                class,
                input.describe(),
                failure.output,
                failure.before,
                failure.after
            )
            .unwrap();
        }
    }
    tally
}

fn parse_baseline(text: &str) -> BTreeMap<String, usize> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let (name, count) = line.split_once(' ').unwrap();
            (name.to_owned(), count.trim().parse().unwrap())
        })
        .collect()
}

fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[test]
fn round_trip_fuzz() {
    let count = environment("FUZZ_INPUTS").map_or(DEFAULT_INPUTS, |value| value.parse().unwrap());
    let output = environment("FUZZ_OUTPUT");
    let record = output.is_some();
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = CORPORA
            .iter()
            .map(|corpus| scope.spawn(move || run(corpus, count, record)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });

    let mut report = format!(
        "{:<16} {:>9} {:>9} {:>9} {:>9} {:>11} {:>6} {:>9}\n",
        "corpus", "inputs", "failures", "html", "unstable", "normalized", "error", "rejected"
    );
    for (corpus, tally) in CORPORA.iter().zip(&tallies) {
        let kind = |name: &str| tally.kinds.get(name).copied().unwrap_or(0);
        writeln!(
            report,
            "{:<16} {:>9} {:>9} {:>9} {:>9} {:>11} {:>6} {:>9}",
            corpus.name,
            count,
            tally.failures,
            kind(KINDS[0]),
            kind(KINDS[1]),
            kind(KINDS[2]),
            kind(KINDS[3]),
            tally.rejected
        )
        .unwrap();
    }
    for (corpus, tally) in CORPORA.iter().zip(&tallies) {
        let mut classes: Vec<_> = tally.classes.iter().collect();
        classes.sort_by(|left, right| right.1.cmp(left.1).then(left.0.cmp(right.0)));
        for (class, number) in classes {
            writeln!(report, "{:<16} {number:>9}  {class}", corpus.name).unwrap();
        }
    }
    println!("{report}");

    if let Some(directory) = &output {
        std::fs::create_dir_all(directory).unwrap();
        for (corpus, tally) in CORPORA.iter().zip(&tallies) {
            let mut inputs = tally.inputs.clone();
            inputs.sort();
            inputs.dedup();
            let mut lines = inputs.join("\n");
            if !lines.is_empty() {
                lines.push('\n');
            }
            std::fs::write(format!("{directory}/{}.txt", corpus.name), lines).unwrap();
            std::fs::write(
                format!("{directory}/{}-details.txt", corpus.name),
                &tally.details,
            )
            .unwrap();
        }
        println!("Wrote failing inputs to {directory}.");
    }

    let panics: Vec<_> = CORPORA
        .iter()
        .zip(&tallies)
        .flat_map(|(corpus, tally)| tally.panics.iter().map(move |input| (corpus.name, input)))
        .collect();
    assert!(panics.is_empty(), "inputs panicked: {:#?}", panics);

    if count != DEFAULT_INPUTS {
        println!("The baseline is checked only at the default size of {DEFAULT_INPUTS} inputs.");
        return;
    }
    if environment("FUZZ_BLESS").is_some() {
        let mut baseline = format!("# Round-trip fuzz failures per corpus at {DEFAULT_INPUTS} inputs, checked by tests/roundtrip_fuzz.rs. The test fails when a corpus exceeds its count. Lower a count when it drops, or run with FUZZ_BLESS=1 to rewrite this file.\n");
        for (corpus, tally) in CORPORA.iter().zip(&tallies) {
            writeln!(baseline, "{} {}", corpus.name, tally.failures).unwrap();
        }
        std::fs::write(BASELINE_PATH, baseline).unwrap();
        println!("Rewrote {BASELINE_PATH}.");
        return;
    }
    let baseline = parse_baseline(include_str!("roundtrip_fuzz_baseline.txt"));
    let mut exceeded = Vec::new();
    for (corpus, tally) in CORPORA.iter().zip(&tallies) {
        let allowed = baseline[corpus.name];
        if tally.failures > allowed {
            exceeded.push(format!(
                "{}: {} failures, baseline {allowed}",
                corpus.name, tally.failures
            ));
        } else if tally.failures < allowed {
            println!(
                "{}: {} failures, below the baseline of {allowed}. Lower it in tests/roundtrip_fuzz_baseline.txt, or run with FUZZ_BLESS=1.",
                corpus.name, tally.failures
            );
        }
    }
    assert!(
        exceeded.is_empty(),
        "round-trip failures exceed the baseline; run with FUZZ_OUTPUT=directory to list them:\n{}",
        exceeded.join("\n")
    );
}

#[test]
fn baseline_lists_every_corpus() {
    let baseline = parse_baseline(include_str!("roundtrip_fuzz_baseline.txt"));
    let names: Vec<_> = CORPORA.iter().map(|corpus| corpus.name).collect();
    assert_eq!(baseline.keys().map(String::as_str).collect::<Vec<_>>(), {
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted
    });
}

/// Assert that four times the input takes less than twelve times as long, after a warm-up, as in `tests/document_normalize.rs`.
///
/// Quadratic time would take sixteen times as long. Each size takes the fastest of five runs, alternating between the sizes, so that load slows both alike, and only one scaling test times at once.
fn assert_linear(unit: &str, mut time: impl FnMut(usize) -> Duration) {
    static TIMING: Mutex<()> = Mutex::new(());
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    time(1 << 7);
    let mut small = Duration::MAX;
    let mut large = Duration::MAX;
    for _ in 0..5 {
        small = small.min(time(1 << 10));
        large = large.min(time(1 << 12));
    }
    assert!(
        large < small * 12,
        "{:?}: 4x input took {:?} against {:?}",
        unit,
        large,
        small
    );
}

#[test]
fn parsing_normalizing_and_serializing_is_linear() {
    // Shapes from the fuzz corpora, repeated: encoded edges, escaped markers, adjacent and nested attention, and nested strikethrough.
    for unit in [
        "*&#x20;~a* ",
        "\\\\\\~b*&#x20;c* ",
        "*a (\\~&#xA;*b ",
        "*a*_b_",
        "**a*b*c** ",
        "~a~~b~~c~ ",
    ] {
        assert_linear(unit, |size| {
            let source = unit.repeat(size) + "\n";
            let started = Instant::now();
            let mut document =
                Document::parse(source.as_str(), &ParseOptions::normalize()).unwrap();
            document.normalize();
            to_markdown_with_options(&document, &MarkdownOptions::default()).unwrap();
            started.elapsed()
        });
    }
}
