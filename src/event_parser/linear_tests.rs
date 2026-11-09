// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Parsing does work linear in the input, counted with [`crate::work`].

extern crate std;

use crate::work::tests::{assert_linear, counted};
use crate::{to_html, to_markdown, Document, IncrementalParser, ParseOptions};

#[test]
fn searching_for_openers_is_linear() {
    // Each `_` closer finds no `_` opener among the `*` openers left open before it, and the search starts where the last failed search for that marker stopped.
    assert_linear("*a b_ ", 1 << 9, |size| {
        let source = "*a b_ ".repeat(size);
        counted(|| {
            Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
        })
    });
}

#[test]
fn literal_autolinks_are_linear() {
    for unit in [
        // A `www` after `_` within a domain, which an `_` makes invalid, starts another domain
        // inside it (#123), with ASCII, non-ASCII and `-` before the `_`.
        "www.a_",
        "www.例_",
        "www.a-_",
        // Other literals: the cached trailing suffix of protocols and paths, an email with a
        // scheme and resource, whose scans are bounded by the `@` on either side, and the
        // parentheses of a path, counted once.
        "http://a_www.a_",
        "http://a_b.c/",
        "xmpp:a@b/c_",
        "a_b@c_",
        "https://a.b/(",
    ] {
        assert_linear(unit, 1 << 8, |size| {
            let source = unit.repeat(size) + "\n";
            counted(|| {
                let document =
                    crate::Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
                to_markdown(&document).unwrap();
            })
        });
    }
}

#[test]
fn definitions_read_at_paragraph_close_scale_linearly() {
    for (label, unit) in [
        ("definitions", "[a]: /u\n"),
        ("failing titles", "[a]: /u\n'x\n"),
        ("unclosed labels", "[a\n"),
        ("lists that cannot interrupt", "[a]: /u\n2. c\n"),
    ] {
        assert_linear(label, 1 << 11, |size| {
            let source = unit.repeat(size);
            counted(|| {
                to_html(&source);
            })
        });
    }
}

#[test]
fn nesting_footnote_definitions_is_linear() {
    // A footnote definition cannot start in a heading or an image. The document builder counts the open headings and images rather than looking through the nodes of every open container at each start, which made a deep nest of footnote definitions quadratic (#171). The HTML renderer counts them the same way rather than looking through its frames, which no count shows, so `to_html` is measured by the `nested footnote definitions` benchmark in `benches/bench.rs` instead, and `tests/footnote.rs` renders the nest under a watchdog.
    assert_linear("Document::parse", 1 << 9, |size| {
        let source = "- [^1]: ".repeat(size);
        counted(|| {
            Document::parse(source.as_str(), &ParseOptions::default()).unwrap();
        })
    });
}

#[test]
fn incremental_parsing_is_linear() {
    // Each push completes a line, so each prepares the source again from the last clean block, which must not scan the accumulated source for a CR. With CRLF and CR endings, that scan finds one at once, so only LF endings are counted.
    for unit in ["word\n\n"] {
        assert_linear(unit, 1 << 7, |size| {
            let options = ParseOptions::default();
            let mut events = 0;
            let steps = counted(|| {
                let mut parser = IncrementalParser::new(&options);
                for _ in 0..size * 16 {
                    events += parser.push(unit).count();
                }
                events += parser.finish().count();
            });
            assert_eq!(events, size * 16 * 4);
            steps
        });
    }
}
