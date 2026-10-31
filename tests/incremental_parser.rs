// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use base_markdown::stream::{Event, EventKind, Tag, TagKind};
use base_markdown::{Constructs, EventParser, IncrementalParser, ParseOptions};
use pretty_assertions::assert_eq;

/// Documents in which every reference follows its definition.
const CORPUS: &[&str] = &[
    "",
    "hello",
    "hello\n",
    "# Heading\n\nText *emphasis* and **bold**.\n",
    "# One\n## Two\nParagraph\n### Three\n",
    "Title\n=====\n\nSubtitle\n--------\nBody\n",
    "Title\n\n---\n\n***\n- - -\nafter\n",
    "a\n-\n",
    "a\n\n-\nb\n",
    "> quote\n>\n> second\n\nafter\n",
    "> quote\nlazy continuation\n> more\n\n> another quote\n\nend",
    "> # heading\n> - item\nlazy\n",
    "1. First\n   - nested\n2. Second\n\nAfter the list.\n",
    "- a\n- b\n\n- c\n\nparagraph\n\n* d\n+ e\n",
    "- a\n  - b\n    - c\n\n      continued\n  - d\n- e\n",
    "1. one\n1) two\n3. three\n\n10. ten\n",
    "-\n\n  foo\n- \n-\n\nbar\n",
    "- a\n\n\n- b\n",
    "a [link](https://example.com?q=x&n=2 \"title\") and ![alt *text*](a.png).\n",
    "[x]: /path \"title\"\n[y]: /other\n'multi\nline title'\n\n[link][x] and [y] and [x][] and [y]\n",
    "[x]:\n/path\n\"title\"\nparagraph after [x]\n",
    "[x]: /path 'title\n\nnot a title' [x]\n",
    "<https://example.com> <me@example.com>\n",
    "a &amp; &#x1f600; \\*escaped\\*\n",
    "`code` and `` a ` b `` and `a\nb`\n",
    "```rust\nfn main() {}\n```\nafter\n```\nunclosed\n",
    "~~~\ncode\n~~~\n\n````\n```\n````\n",
    "    code\n    two\n\n\n    three\nparagraph\n",
    "    code\n\n\n",
    "<div>\nhello\n</div>\n\n<!-- comment\n\nstill -->\nafter\n",
    "<div>\n\n*emphasis*\n\n</div>\n",
    "line  \nbreak\\\nagain\n",
    "| a | b |\n| :- | -: |\n| c | d |\n\nafter\n",
    "paragraph\n| a | b |\n| - | - |\n| c | d |\nmore row\n\n",
    "| a |\n| - |\n# heading\n| b |\n| - |\n",
    "para\n# h\n|-|\n",
    "para\n***\n|-|\nrow\n",
    "a\n---\n|-|\n",
    "- [x] yes\n- [ ] no\n",
    "~~old~~ and www.example.com\n",
    "[^x]: Body.\n\n    indented\n\nNote[^x].\n",
    "a\r\nb\r\n\r\nc\r\n",
    "a\rb\r\r# c\r- d\r",
    "\u{feff}# bom\n\ntext\n",
    "# hello\n\n***\n",
    "tabs\tand\t\n\t- list\n\t\tcode\n",
    "$$\nmath\n$$\n\ntext $x$\n",
    "Para\n\n    \n\nPara\n",
];

fn options() -> Vec<ParseOptions> {
    let mut frontmatter = ParseOptions::gfm();
    frontmatter.constructs = Constructs {
        frontmatter: true,
        math_flow: true,
        math_text: true,
        ..Constructs::gfm()
    };
    let mut blank_lines = ParseOptions::gfm();
    blank_lines.html_block_blank_lines = true;
    vec![
        ParseOptions::default(),
        ParseOptions::gfm(),
        frontmatter,
        blank_lines,
    ]
}

fn one_shot(source: &str, options: &ParseOptions) -> Vec<Event<'static>> {
    EventParser::new(source, options)
        .map(Result::unwrap)
        .filter(|event| {
            !matches!(
                event.kind,
                EventKind::Start(Tag::Document(_)) | EventKind::End(TagKind::Document)
            )
        })
        .collect()
}

/// Push the pieces, checking that each delivery lies within the stable prefix.
fn incremental<'a>(
    pieces: impl IntoIterator<Item = &'a str>,
    options: &ParseOptions,
) -> Vec<Event<'static>> {
    let mut parser = IncrementalParser::new(options);
    let mut events = Vec::new();
    let mut stable = 0;
    for piece in pieces {
        let delivered: Vec<_> = parser.push(piece).collect();
        assert!(
            parser.stable_offset() >= stable,
            "the stable offset is monotonic"
        );
        stable = parser.stable_offset();
        assert!(stable <= parser.source().len());
        for event in &delivered {
            assert!(
                event.span.end <= stable,
                "{event:?} lies beyond the stable offset {stable} of {:?}",
                parser.source()
            );
        }
        events.extend(delivered);
    }
    events.extend(parser.finish());
    assert_eq!(parser.stable_offset(), parser.source().len());
    events
}

fn boundaries(source: &str) -> Vec<usize> {
    (0..=source.len())
        .filter(|&index| source.is_char_boundary(index))
        .collect()
}

/// A small deterministic generator, so failures are reproducible without a dependency.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Check that every division of the source produces the events of a single push, and return them.
fn check_divisions(
    source: &str,
    options: &ParseOptions,
    random: &mut Random,
) -> Vec<Event<'static>> {
    let whole = incremental([source], options);
    let points = boundaries(source);
    for &point in &points {
        let divided = incremental([&source[..point], &source[point..]], options);
        assert_eq!(divided, whole, "split at {point}: {source:?}");
    }
    let characters = points.windows(2).map(|pair| &source[pair[0]..pair[1]]);
    assert_eq!(
        incremental(characters, options),
        whole,
        "characters: {source:?}"
    );
    for _ in 0..16 {
        let mut cuts: Vec<usize> = points
            .iter()
            .copied()
            .filter(|_| random.next() % 4 == 0)
            .collect();
        cuts.insert(0, 0);
        cuts.push(source.len());
        let pieces = cuts.windows(2).map(|pair| &source[pair[0]..pair[1]]);
        assert_eq!(
            incremental(pieces, options),
            whole,
            "cuts {cuts:?}: {source:?}"
        );
    }
    whole
}

#[test]
fn every_division_matches_a_one_shot_parse() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    for options in options() {
        for source in CORPUS {
            let whole = check_divisions(source, &options, &mut random);
            assert_eq!(whole, one_shot(source, &options), "source: {source:?}");
        }
    }
}

#[test]
fn most_corpus_events_are_delivered_before_finishing() {
    let options = ParseOptions::gfm();
    let (mut early, mut total) = (0, 0);
    for source in CORPUS {
        let mut parser = IncrementalParser::new(&options);
        for (index, character) in source.char_indices() {
            let count = parser
                .push(&source[index..index + character.len_utf8()])
                .count();
            early += count;
            total += count;
        }
        total += parser.finish().count();
    }
    assert!(
        early * 3 > total,
        "only {} of {} events were delivered early",
        early,
        total
    );
}

#[test]
fn frontmatter_holds_output_until_it_closes() {
    let mut options = ParseOptions::default();
    options.constructs.frontmatter = true;
    let mut random = Random(7);
    for source in [
        "---\ntitle: x\n---\n# Heading\n\ntext\n",
        "---\n\ntext\n\nmore\n",
        "+++\na = 1\n+++\n\ntext\n",
        "# no frontmatter\n\n---\n",
    ] {
        let whole = check_divisions(source, &options, &mut random);
        assert_eq!(whole, one_shot(source, &options), "source: {source:?}");
    }
    let mut parser = IncrementalParser::new(&options);
    assert_eq!(parser.push("---\ntitle: x\n\nbody\n\n").count(), 0);
    assert!(parser.push("---\n\n").next().is_some());
    // Fences that cannot open frontmatter are ordinary blocks.
    for opening in ["```\n", "~~~\n", "```a`b\n"] {
        let mut parser = IncrementalParser::new(&options);
        let source = format!("{opening}code\n{}\n\npara\n\n", opening.trim_end());
        assert!(parser.push(&source).next().is_some(), "{:?}", source);
    }
}

/// Extract the inputs of the generated `CommonMark` specification test.
fn specification() -> Vec<String> {
    let source = include_str!("commonmark.rs");
    let pattern =
        regex::Regex::new(r####"(?s)to_html_with_options\(\s*r###"(.*?)"###"####).unwrap();
    pattern
        .captures_iter(source)
        .map(|capture| capture[1].to_string())
        .collect()
}

#[test]
fn specification_examples_are_independent_of_division() {
    let examples = specification();
    assert!(examples.len() > 600);
    let mut random = Random(0x2545_f491_4f6c_dd1d);
    for options in [ParseOptions::default(), ParseOptions::gfm()] {
        for source in &examples {
            let whole = check_divisions(source, &options, &mut random);
            // Forward references resolve only in a one-shot parse.
            if !source.contains("]:") {
                assert_eq!(whole, one_shot(source, &options), "source: {source:?}");
            }
        }
    }
}

fn delivered(parser: &mut IncrementalParser<'_>, text: &str) -> Vec<Event<'static>> {
    parser.push(text).collect()
}

fn starts(events: &[Event<'static>]) -> Vec<TagKind> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Start(tag) => Some(tag.kind()),
            _ => None,
        })
        .collect()
}

#[test]
fn blocks_are_delivered_once_closed() {
    let options = ParseOptions::gfm();
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "First paragraph").is_empty());
    assert!(delivered(&mut parser, " continues.\n").is_empty());
    let events = delivered(&mut parser, "\n");
    assert_eq!(starts(&events), [TagKind::Paragraph]);
    assert_eq!(parser.stable_offset(), "First paragraph continues.\n".len());

    // An ATX heading after a blank line is complete at its line ending.
    assert!(delivered(&mut parser, "## Heading").is_empty());
    let events = delivered(&mut parser, "\n");
    assert!(matches!(events[0].kind, EventKind::Separator { .. }));
    assert_eq!(starts(&events), [TagKind::Heading]);

    // A closed fence is complete at its closing line.
    assert!(delivered(&mut parser, "```\ncode\n").is_empty());
    let events = delivered(&mut parser, "```\n");
    assert!(events.iter().any(|event| matches!(
        event.kind,
        EventKind::CodeBlock {
            unclosed: false,
            ..
        }
    )));

    // A list waits for its end, since later items determine its tightness.
    assert!(delivered(&mut parser, "- one\n- two\n\n").is_empty());
    assert!(delivered(&mut parser, "- three\n\n").is_empty());
    let events = delivered(&mut parser, "After.\n");
    assert_eq!(
        starts(&events),
        [
            TagKind::List,
            TagKind::Item,
            TagKind::Paragraph,
            TagKind::Item,
            TagKind::Paragraph,
            TagKind::Item,
            TagKind::Paragraph
        ]
    );
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event.kind, EventKind::Start(_)))
            .unwrap()
            .kind,
        EventKind::Start(Tag::List { tight: false, .. })
    ));
}

#[test]
fn reinterpreting_lines_are_held_back() {
    let options = ParseOptions::gfm();

    // A setext underline turns the preceding paragraph into a heading.
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "Title\n").is_empty());
    assert!(delivered(&mut parser, "===\n").is_empty());
    assert_eq!(starts(&delivered(&mut parser, "\n")), [TagKind::Heading]);

    // A delimiter row turns the preceding line into a table header.
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "Intro\n| a | b |\n").is_empty());
    assert!(delivered(&mut parser, "| - | - |\n| 1 | 2 |\n").is_empty());
    let events = delivered(&mut parser, "\n");
    assert_eq!(starts(&events)[..2], [TagKind::Paragraph, TagKind::Table]);

    // An incomplete line may still become a heading marker or continuation.
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "a\n#").is_empty());
    assert!(delivered(&mut parser, "b\n").is_empty());

    // A lazy continuation line extends the block quote.
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "> quote\n").is_empty());
    assert!(delivered(&mut parser, "lazy\n").is_empty());
    assert_eq!(
        starts(&delivered(&mut parser, "\n")),
        [TagKind::BlockQuote, TagKind::Paragraph]
    );

    // A trailing carriage return may begin a CRLF line ending.
    let mut parser = IncrementalParser::new(&options);
    assert!(delivered(&mut parser, "a\r\n\r").is_empty());
    assert_eq!(starts(&delivered(&mut parser, "\n")), [TagKind::Paragraph]);
}

#[test]
fn references_resolve_against_earlier_definitions() {
    let options = ParseOptions::gfm();
    let link = |events: &[Event<'static>]| {
        events
            .iter()
            .any(|event| matches!(event.kind, EventKind::Start(Tag::Link { .. })))
    };

    let backward = "[x]: /url\n\n[x]\n";
    assert!(link(&incremental([backward], &options)));

    // A forward reference to a later document-level block stays literal, however the text is divided.
    let forward = "[x]\n\n[x]: /url\n";
    assert!(link(&one_shot(forward, &options)));
    assert!(!link(&incremental([forward], &options)));
    assert!(!link(&incremental(["[x]\n\n", "[x]: /url\n"], &options)));

    // Definitions inside the same document-level block resolve in either order.
    let nested = "> [x]\n>\n> [x]: /url\n";
    assert!(link(&incremental([nested], &options)));
    assert_eq!(incremental([nested], &options), one_shot(nested, &options));

    let footnote = "Note[^a].\n\n[^a]: Body.\n";
    let footnote_call = |events: &[Event<'static>]| {
        events
            .iter()
            .any(|event| matches!(event.kind, EventKind::FootnoteReference(..)))
    };
    assert!(footnote_call(&one_shot(footnote, &options)));
    assert!(!footnote_call(&incremental([footnote], &options)));
}

#[test]
fn pushing_after_finish_panics() {
    let options = ParseOptions::default();
    let mut parser = IncrementalParser::new(&options);
    assert_eq!(parser.finish().count(), 0);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parser.push("text");
    }))
    .is_err());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parser.finish();
    }))
    .is_err());
}

/// Lines that open, continue, interrupt or reinterpret blocks, combined at random.
const LINES: &[&str] = &[
    "",
    "text",
    "text [a] *em",
    "phasis* `co",
    "de`",
    "- item",
    "  continued",
    "    indented",
    "* star",
    "1. one",
    "2) two",
    "> quote",
    ">",
    "> - quoted item",
    "---",
    "===",
    "***",
    "# heading",
    "## h ##",
    "```",
    "~~~",
    "```rust",
    "$$",
    "| a | b |",
    "|-|-|",
    "| - |",
    "a | b",
    "[a]: /url",
    "[a]:",
    "'title'",
    "\"open title",
    "[^n]: note",
    "<div>",
    "</div>",
    "<!--",
    "-->",
    "<a href=\"x\">",
    "\t- tab item",
    "\t\tcode",
    "-",
    "1.",
    "[a]",
    "Setext",
    "  - nested",
    "      deep",
    "> > nested",
    ">     code",
    "> ```",
    "- > quote",
    "- ```",
    "  ```",
    "   - three",
    "- [a]: /url",
    "> | a |",
    "> |-|",
    "- | a |",
    "  |-|",
    "> # h",
    "> ---",
    "+++",
    "  # h",
    "- # h",
    "<pre>",
    "</pre>",
    "> <div>",
    "- [ ] task",
    "www.example.com",
    "~~strike~~",
    "\\",
    "  ",
    "\t",
];

#[test]
fn random_documents_are_independent_of_division() {
    let mut random = Random(0x1234_5678_9abc_def1);
    let endings = ["\n", "\n", "\n", "\r\n", "\r"];
    let options = options();
    for _ in 0..2000 {
        let mut source = String::new();
        for _ in 0..(random.next() % 16) {
            source.push_str(LINES[(random.next() % LINES.len() as u64) as usize]);
            source.push_str(endings[(random.next() % endings.len() as u64) as usize]);
        }
        if random.next() % 3 == 0 {
            source.push_str(LINES[(random.next() % LINES.len() as u64) as usize]);
        }
        let options = &options[(random.next() % options.len() as u64) as usize];
        let whole = check_divisions(&source, options, &mut random);
        if !source.contains("]:") {
            assert_eq!(whole, one_shot(&source, options), "source: {source:?}");
        }
    }
}
