// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Round-trip fuzz harness for normalizing and serializing Markdown, with fixed seeds and a checked-in baseline.
//!
//! Each input is parsed with [`ParseOptions::normalize`], the syntax `normalize_markdown` reads, or built by hand as a tree. It is then normalized, serialized with the default Markdown options, parsed again with the same options and normalized again. An input fails when either document is not normalized after normalizing (`normalized`), when the HTML of the two documents differs (`html`), when the HTML is the same but the canonical trees of `support::fixture::canonical` differ (`tree`), when serializing the second document gives different Markdown (`unstable`), or when serializing a parsed document returns an error (`error`). A panic is a hard failure whatever the baseline. A hand-built tree that the serializer rejects with an error is skipped and counted as rejected.
//!
//! Every corpus is generated from its own fixed seed by a small xorshift generator, so the inputs, and the counts, are the same on every platform, and the first inputs of a larger run are the inputs of a smaller one. At the default size, each corpus's failure count, rejected count and a digest of its sorted failing inputs must equal its line in `tests/roundtrip_fuzz_baseline.txt`, so a fix that hides a new failure in the same corpus still changes the digest. Any difference, up or down, fails the test.
//!
//! Environment variables:
//!
//! - `FUZZ_INPUTS=1000000` sets the inputs per corpus. The baseline is only checked at the default size. Run large sizes in release.
//! - `FUZZ_OUTPUT=directory` writes `<corpus>.txt`, the sorted, deduplicated failing inputs of each corpus, one Rust string literal per line, and `<corpus>-details.txt`, each failure with its kind, class, output and both HTML renderings or canonical trees. The files are large for the tree corpora at 1,000,000 inputs: about 220 MB of details and 140 MB of inputs each.
//! - `FUZZ_BLESS=1` rewrites the baseline from this run and prints what changed. It fails at any other size.
//!
//! See `context/roundtrip-fuzz.md` for comparing a branch with main.

mod support;

use base_markdown::{
    to_markdown_with_options, CompileOptions, Document, IncrementalParser, MarkdownOptions,
    ParseOptions,
};
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use support::fixture::{
    canonical, AlignKind, Break, Delete, Emphasis, Heading, InlineCode, Link, Node, Paragraph,
    Root, Strong, Table, TableCell, TableRow, Text,
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

/// Phrasing of one to four nodes. `outside` lists the attention and links that enclose it, which are not nested in themselves, and `break` in a table cell, which GFM cannot write. A hard break is never the last child, as Markdown has no source for a break at the end of a block or of attention.
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
    if let Some(last @ Node::Break(_)) = children.last_mut() {
        *last = text(random.concatenate(TREE_TEXT, 4));
    }
    children
}

/// A paragraph, heading or one-cell table of random phrasing.
fn tree(random: &mut Random) -> Input {
    let kind = random.below(4);
    let children = phrasing(random, 0, if kind == 1 { &["break"] } else { &[] });
    let block = match kind {
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

const KINDS: [&str; 5] = ["html", "tree", "unstable", "normalized", "error"];

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
    // The HTML ignores line endings after hard breaks and at the end; the canonical trees do not, so dropping one is not a fix.
    let (canonical_before, canonical_after) = (canonical(&document), canonical(&reparsed));
    if canonical_after != canonical_before {
        return fail(
            "tree",
            output,
            canonical_before.join("\n"),
            canonical_after.join("\n"),
        );
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
        if failure.kind == "tree" {
            return "tree differs with the same HTML, such as a line ending after a hard break";
        }
        if failure.kind != "html" {
            return failure.kind;
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
        if differs("<br />") || before.ends_with("<br />") || before.contains("<br /></") {
            return "hard break at the end of a block or attention";
        }
        if differs("<a href")
            && ["www.", "@", "://"]
                .iter()
                .any(|literal| input.contains(literal))
        {
            // Text is escaped where GFM would read a literal autolink, so this should stay empty.
            return "literal autolink formed from text";
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
    /// The failing inputs, described as Rust string literals, sorted and deduplicated once the run ends.
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
        tally.inputs.push(input.describe());
        if record {
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
    tally.inputs.sort();
    tally.inputs.dedup();
    tally
}

/// The 64-bit FNV-1a hash of the sorted, deduplicated failing inputs, one per line as in `<corpus>.txt`, so any change to which inputs fail changes it.
fn digest(inputs: &[String]) -> String {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for input in inputs {
        for byte in input.bytes().chain(Some(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01B3);
        }
    }
    format!("{hash:016x}")
}

/// One corpus's line in the baseline: `<corpus> <failures> <rejected> <digest>`.
#[derive(Debug, PartialEq)]
struct Expected {
    failures: usize,
    rejected: usize,
    digest: String,
}

impl Expected {
    fn of(tally: &Tally) -> Self {
        Self {
            failures: tally.failures,
            rejected: tally.rejected,
            digest: digest(&tally.inputs),
        }
    }
}

/// Parse the baseline, rejecting malformed and duplicate lines.
fn parse_baseline(text: &str) -> Result<BTreeMap<String, Expected>, String> {
    let mut baseline = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let number = index + 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        let [name, failures, rejected, digest] = fields[..] else {
            return Err(format!(
                "baseline line {number} should be `<corpus> <failures> <rejected> <digest>`: {line:?}"
            ));
        };
        let count = |field: &str, what: &str| {
            field
                .parse::<usize>()
                .map_err(|_| format!("baseline line {number}: {what} {field:?} is not a count"))
        };
        if digest.len() != 16 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "baseline line {number}: digest {digest:?} is not 16 hexadecimal digits"
            ));
        }
        let expected = Expected {
            failures: count(failures, "failures")?,
            rejected: count(rejected, "rejected")?,
            digest: digest.to_owned(),
        };
        if baseline.insert(name.to_owned(), expected).is_some() {
            return Err(format!(
                "baseline line {number}: corpus {name:?} is listed twice"
            ));
        }
    }
    Ok(baseline)
}

fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The difference between `now` and `before`, signed.
fn change(now: usize, before: usize) -> String {
    if now >= before {
        format!("+{}", now - before)
    } else {
        format!("-{}", before - now)
    }
}

const BLESS: &str = "FUZZ_BLESS=1 cargo test --test roundtrip_fuzz round_trip_fuzz";

/// Taken by the fuzz run and by each scaling test, so that the fuzz threads never load the machine while a scaling test times.
static TIMING: Mutex<()> = Mutex::new(());

#[test]
fn round_trip_fuzz() {
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    let count = environment("FUZZ_INPUTS").map_or(DEFAULT_INPUTS, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("FUZZ_INPUTS={:?} is not a count", value))
    });
    let bless = match environment("FUZZ_BLESS").as_deref() {
        None | Some("0") => false,
        Some("1") => true,
        Some(other) => panic!(
            "FUZZ_BLESS={:?}: set it to 1 to rewrite the baseline",
            other
        ),
    };
    assert!(
        !bless || count == DEFAULT_INPUTS,
        "FUZZ_BLESS=1 rewrites the baseline only at the default size of {} inputs, not FUZZ_INPUTS={}",
        DEFAULT_INPUTS,
        count
    );
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
        "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>11} {:>6} {:>9}\n",
        "corpus",
        "inputs",
        "failures",
        "html",
        "tree",
        "unstable",
        "normalized",
        "error",
        "rejected"
    );
    for (corpus, tally) in CORPORA.iter().zip(&tallies) {
        let kind = |name: &str| tally.kinds.get(name).copied().unwrap_or(0);
        writeln!(
            report,
            "{:<16} {:>9} {:>9} {:>9} {:>9} {:>9} {:>11} {:>6} {:>9}",
            corpus.name,
            count,
            tally.failures,
            kind(KINDS[0]),
            kind(KINDS[1]),
            kind(KINDS[2]),
            kind(KINDS[3]),
            kind(KINDS[4]),
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
            let mut lines = tally.inputs.join("\n");
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
    let baseline = std::fs::read_to_string(BASELINE_PATH)
        .map_err(|error| format!("cannot read {BASELINE_PATH}: {error}"))
        .and_then(|text| parse_baseline(&text));
    // Blessing replaces a baseline that cannot be read; checking needs one.
    let baseline = match baseline {
        Ok(baseline) => baseline,
        Err(error) if bless => {
            println!("Replacing the baseline: {error}");
            BTreeMap::new()
        }
        Err(error) => panic!("{}", error),
    };
    let mut changes = Vec::new();
    for (corpus, tally) in CORPORA.iter().zip(&tallies) {
        let now = Expected::of(tally);
        match baseline.get(corpus.name) {
            Some(before) if *before == now => {}
            Some(before) => changes.push(format!(
                "{}: failures {} ({} from {}), rejected {} ({} from {}), digest {} (was {})",
                corpus.name,
                now.failures,
                change(now.failures, before.failures),
                before.failures,
                now.rejected,
                change(now.rejected, before.rejected),
                before.rejected,
                now.digest,
                before.digest
            )),
            None => changes.push(format!(
                "{}: failures {}, rejected {}, digest {} (not in the baseline)",
                corpus.name, now.failures, now.rejected, now.digest
            )),
        }
    }
    for name in baseline.keys() {
        if !CORPORA.iter().any(|corpus| corpus.name == name) {
            changes.push(format!("{name}: in the baseline but not a corpus"));
        }
    }

    if bless {
        let mut text = format!("# Round-trip fuzz results per corpus at {DEFAULT_INPUTS} inputs, checked exactly by tests/roundtrip_fuzz.rs: <corpus> <failures> <rejected> <digest of the sorted failing inputs>. Rewrite with {BLESS}.\n");
        for (corpus, tally) in CORPORA.iter().zip(&tallies) {
            let now = Expected::of(tally);
            writeln!(
                text,
                "{} {} {} {}",
                corpus.name, now.failures, now.rejected, now.digest
            )
            .unwrap();
        }
        std::fs::write(BASELINE_PATH, text)
            .unwrap_or_else(|error| panic!("cannot write {}: {}", BASELINE_PATH, error));
        if changes.is_empty() {
            println!("The baseline is unchanged.");
        } else {
            println!("Rewrote {BASELINE_PATH}:\n{}", changes.join("\n"));
        }
        return;
    }
    assert!(
        changes.is_empty(),
        "the round-trip results differ from tests/roundtrip_fuzz_baseline.txt:\n{}\nRun with FUZZ_OUTPUT=directory to list the failing inputs, and compare with main as in context/roundtrip-fuzz.md. If the change is intended, rewrite the baseline with `{}` and commit it.",
        changes.join("\n"),
        BLESS
    );
}

#[test]
fn baseline_lists_every_corpus() {
    let baseline = parse_baseline(include_str!("roundtrip_fuzz_baseline.txt")).unwrap();
    let mut names: Vec<_> = CORPORA.iter().map(|corpus| corpus.name).collect();
    names.sort_unstable();
    assert_eq!(
        baseline.keys().map(String::as_str).collect::<Vec<_>>(),
        names
    );
}

#[test]
fn baseline_rejects_malformed_lines() {
    let parse = |text: &str| parse_baseline(text).is_err();
    assert!(!parse("# comment\n\nsoup 1 0 0123456789abcdef\n"));
    assert!(parse(
        "soup 1 0 0123456789abcdef\nsoup 2 0 0123456789abcdef\n"
    ));
    assert!(parse("soup 1\n"));
    assert!(parse("soup x 0 0123456789abcdef\n"));
    assert!(parse("soup 1 0 digest\n"));
}

/// Assert that four times the input takes less than twelve times as long, after a warm-up, as in `tests/document_normalize.rs`.
///
/// Quadratic time would take sixteen times as long. Each size takes the fastest of five runs, alternating between the sizes, so that load slows both alike, and only one scaling test, or the fuzz run, runs at once.
fn assert_linear(unit: &str, mut time: impl FnMut(usize) -> Duration) {
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
fn incremental_parsing_is_linear() {
    // Each push completes a line, so each prepares the source again from the last clean block. With LF, CRLF and CR endings.
    for unit in ["word\n\n", "word\r\n\r\n", "word\r\r"] {
        assert_linear(unit, |size| {
            let options = ParseOptions::normalize();
            let started = Instant::now();
            let mut parser = IncrementalParser::new(&options);
            let mut events = 0;
            for _ in 0..size * 16 {
                events += parser.push(unit).count();
            }
            events += parser.finish().count();
            let elapsed = started.elapsed();
            assert_eq!(events, size * 16 * 4);
            elapsed
        });
    }
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
