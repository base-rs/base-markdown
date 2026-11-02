// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Text is literal: whatever a text node holds reads back as that text, never as a GFM literal
//! autolink, so the serializer escapes what would start one.
use base_markdown::stream::EventKind;
use base_markdown::{to_markdown, Document, NodeId, ParseOptions};
use pretty_assertions::assert_eq;

mod support;
use support::fixture::canonical;

fn parse(source: &str) -> Document<String> {
    Document::parse(source.to_owned(), &ParseOptions::normalize()).unwrap()
}

/// The text nodes of `document`, in order.
fn texts(document: &Document<String>) -> Vec<NodeId> {
    document
        .descendants(document.root())
        .filter(|&id| matches!(document.node(id).unwrap().kind, EventKind::Text(_)))
        .collect()
}

/// Parse `source`, set the text that reads `placeholder` to `value` and normalize.
fn edit(source: &str, placeholder: &str, value: &str) -> Document<String> {
    let mut document = parse(source);
    let id = texts(&document)
        .into_iter()
        .find(|&id| matches!(&document.node(id).unwrap().kind, EventKind::Text(text) if text.resolve(document.source()).ok() == Some(placeholder)))
        .unwrap_or_else(|| panic!("{:?} has the text {:?}", source, placeholder));
    *document.text_mut(id).unwrap() = value.to_owned();
    document.normalize();
    document
}

/// `document` serializes to `expected`, which reads back as the same tree and serializes back to
/// itself.
fn assert_round_trip(document: &Document<String>, expected: &str) {
    let markdown = to_markdown(document).unwrap();
    assert_eq!(markdown, expected);
    let mut reparsed = parse(&markdown);
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(document), "{markdown:?}");
    assert_eq!(to_markdown(&reparsed).unwrap(), markdown);
}

/// Each `(value, expected)` as the whole text of a paragraph.
fn assert_paragraphs(cases: &[(&str, &str)]) {
    for (value, expected) in cases {
        assert_round_trip(&edit("x\n", "x", value), &format!("{expected}\n"));
    }
}

#[test]
fn www_literals_escape_the_dot_after_www() {
    assert_paragraphs(&[
        (
            "Visit www.google.com for more details.",
            "Visit www\\.google.com for more details.",
        ),
        ("www.a.b", "www\\.a.b"),
        ("WWW.a.b", "WWW\\.a.b"),
        ("Www.a.b/c?d=e", "Www\\.a.b/c?d=e"),
        ("www.a", "www\\.a"),
        ("www.a.b.", "www\\.a.b."),
        ("www.點看.com", "www\\.點看.com"),
    ]);
}

#[test]
fn scheme_literals_escape_the_colon() {
    assert_paragraphs(&[
        ("see https://x.org/a", "see https\\://x.org/a"),
        ("http://x.org", "http\\://x.org"),
        ("HTTPS://X.ORG", "HTTPS\\://X.ORG"),
        ("ftp://x.org", "ftp\\://x.org"),
        ("sftp://x.org", "sftp\\://x.org"),
        ("ssh://x.org", "ssh\\://x.org"),
        ("irc://x.org", "irc\\://x.org"),
        ("ircs://x.org", "ircs\\://x.org"),
        // A scheme starts after any byte that is not a letter.
        ("1http://x.org", "1http\\://x.org"),
        ("a.http://x.org", "a.http\\://x.org"),
    ]);
}

#[test]
fn email_literals_escape_the_at_sign() {
    assert_paragraphs(&[
        ("mail a@b.co today", "mail a\\@b.co today"),
        ("a.b-c_d+e@f.gh", "a.b-c_d+e\\@f.gh"),
        // The address protocols are part of the literal, and still read as one when their colon
        // is escaped, so the `@` is.
        ("mailto:a@b.co", "mailto:a\\@b.co"),
        ("xmpp:a@b.co/r", "xmpp:a\\@b.co/r"),
        // Each `@` that would form an email once the one before it is escaped is escaped too.
        ("a@b.c.d@e.f", "a\\@b.c.d\\@e.f"),
    ]);
}

#[test]
fn text_that_forms_no_literal_is_unchanged() {
    for value in [
        "awww.a.b",
        "a.www.b.c",
        "!www.a.b",
        "www.",
        // `www.` needs a domain after it.
        "www. a",
        "www.\na",
        "www.)",
        "www.#a",
        "www.-",
        "www.a_b.c_d",
        "ahttp://x.org",
        "gopher://x.org",
        "tel://123",
        "http:/x.org",
        "http://",
        "a@b",
        "a@b.",
        "@b.co",
        "/a@b.co",
        "mailto:a",
        "x.org",
        "a: b. c",
    ] {
        assert_round_trip(&edit("x\n", "x", value), &format!("{value}\n"));
    }
}

#[test]
fn literals_that_form_once_another_is_escaped_are_escaped() {
    assert_paragraphs(&[
        // The `www` would be inside the scheme literal.
        ("http://a.b/(www.c.d", "http\\://a.b/(www\\.c.d"),
        // The email would be inside the `www` literal.
        ("(www.a@b.cd", "(www\\.a\\@b.cd"),
        // The escaped `_` ends the `www` literal, which leaves out the dot before it, and starts
        // the run of text the email would be found in.
        ("www.a._@b.cd", "www\\.a.\\_@b.cd"),
        // The second `www` is in the domain of the first, which an escape would end.
        ("www.x_www.y", "www.x_www\\.y"),
        (
            "several www.a.b, c@d.ef and http://g.h",
            "several www\\.a.b, c\\@d.ef and http\\://g.h",
        ),
    ]);
}

#[test]
fn an_escaped_at_sign_keeps_an_email_autolink_from_forming() {
    // The `<` needs no escape once the `@` of the email autolink has one.
    assert_paragraphs(&[
        ("<1@b.c>", "<1\\@b.c>"),
        // An email autolink with no literal in it keeps the escape before its `<`.
        ("a@b.co <x@y> <1@b.c>", "a\\@b.co \\<x@y> <1\\@b.c>"),
        ("www.a.b <1@b.c> <x@y>", "www\\.a.b <1\\@b.c> \\<x@y>"),
    ]);
}

#[test]
fn www_starts_a_literal_only_after_whitespace_or_some_punctuation() {
    assert_paragraphs(&[
        ("(www.a.b)", "(www\\.a.b)"),
        ("x*www.a.b", "x\\*www\\.a.b"),
        ("x_www.a.b", "x_www\\.a.b"),
        ("x~www.a.b", "x\\~www\\.a.b"),
        ("x]www.a.b", "x]www\\.a.b"),
        ("x[www.a.b", "x\\[www\\.a.b"),
        ("x\twww.a.b", "x\twww\\.a.b"),
    ]);
    // After a line ending, in the same text or after a break.
    assert_round_trip(&edit("x\n", "x", "a\nwww.b.c"), "a\nwww\\.b.c\n");
    assert_round_trip(&edit("a\\\nx\n", "x", "www.b.c"), "a\\\nwww\\.b.c\n");
}

#[test]
fn literals_next_to_attention() {
    // After the closing sequence, `www` can start a literal; inside, the text starts after the
    // opening one.
    assert_round_trip(&edit("*a*x\n", "x", "www.b.c"), "*a*www\\.b.c\n");
    assert_round_trip(&edit("~~a~~x\n", "x", "www.b.c"), "~~a~~www\\.b.c\n");
    assert_round_trip(&edit("*x*\n", "x", "www.b.c"), "*www\\.b.c*\n");
    assert_round_trip(&edit("**x**\n", "x", "a@b.co"), "**a\\@b.co**\n");
    assert_round_trip(&edit("x*a*\n", "x", "http://b.c"), "http\\://b.c*a*\n");
}

#[test]
fn literals_next_to_other_nodes() {
    // `www` cannot start a literal after a code span, but a scheme can.
    assert_round_trip(&edit("`c`x\n", "x", "www.a.b"), "`c`www.a.b\n");
    assert_round_trip(&edit("`c`x\n", "x", "http://a.b"), "`c`http\\://a.b\n");
    assert_round_trip(&edit("[a](b)x\n", "x", "c@d.ef"), "[a](b)c\\@d.ef\n");
}

#[test]
fn literal_before_a_code_span_keeps_the_code_span() {
    // A literal runs to whitespace or `<`, so it would take the backtick and leave the code as
    // raw HTML (#97).
    let document = edit("Use `<script>` here.\n", "Use ", "Use https://a.b");
    assert_round_trip(&document, "Use https\\://a.b`<script>` here.\n");
    let document = edit("Use `<script>` here.\n", "Use ", "Use www.a.b");
    assert_round_trip(&document, "Use www\\.a.b`<script>` here.\n");
}

#[test]
fn literals_cannot_form_in_link_text() {
    assert_round_trip(&edit("[x](y)\n", "x", "www.a.b"), "[www.a.b](y)\n");
    assert_round_trip(&edit("[x](y)\n", "x", "https://a.b"), "[https://a.b](y)\n");
    assert_round_trip(&edit("[x](y)\n", "x", "a@b.co"), "[a@b.co](y)\n");
    assert_round_trip(&edit("[*x*](y)\n", "x", "a@b.co"), "[*a@b.co*](y)\n");
}

#[test]
fn literals_in_headings_and_table_cells() {
    assert_round_trip(&edit("# x\n", "x", "www.a.b"), "# www\\.a.b\n");
    assert_round_trip(&edit("x\n=\n", "x", "a@b.co"), "# a\\@b.co\n");
    assert_round_trip(
        &edit("| x |\n| - |\n| y |\n", "y", "www.a.b"),
        "| x |\n| --- |\n| www\\.a.b |\n",
    );
    assert_round_trip(
        &edit("| x |\n| - |\n| y |\n", "x", "see http://a.b"),
        "| see http\\://a.b |\n| --- |\n| y |\n",
    );
}

#[test]
fn literals_after_trimmed_leading_whitespace() {
    // Normalizing trims the space before `www`, which leaves it at the start of the paragraph.
    let mut document = parse("&#x20;www.a.b\n");
    document.normalize();
    let markdown = to_markdown(&document).unwrap();
    let mut reparsed = parse(&markdown);
    reparsed.normalize();
    assert_eq!(canonical(&reparsed), canonical(&document), "{markdown:?}");
}

#[test]
fn parsed_literals_are_links_and_are_written_as_they_were() {
    for source in [
        "Visit www.google.com for more details.\n",
        "a www.b.c and d@e.fg and https://h.i/j\n",
        "`c`www.a.b and !www.c.d\n",
        "mailto:a@b.co and xmpp:c@d.ef/g\n",
        "| a |\n| --- |\n| www.b.c |\n",
        "# see http://a.b\n",
        // Image text is written as the text of its nodes, so a literal there stays one.
        "![www.a.b and c@d.ef](g)\n",
    ] {
        let mut document = parse(source);
        document.normalize();
        assert_eq!(to_markdown(&document).unwrap(), source);
    }
}

#[test]
fn literal_scans_stay_linear() {
    // Doubling the input roughly doubles the time, whether the text is one node or, without
    // normalizing, many nodes split at escapes.
    let time = |unit: &str, count: usize, normalize: bool| {
        let source = unit.repeat(count) + "\n";
        let mut document = Document::parse(source.as_str(), &ParseOptions::gfm()).unwrap();
        if normalize {
            document.normalize();
        }
        let started = std::time::Instant::now();
        to_markdown(&document).unwrap();
        started.elapsed()
    };
    for (unit, normalize) in [
        ("www.a_", true),
        ("(www\\.a", true),
        ("\\_\\@", true),
        ("_www.", true),
        ("<1@b.c>", true),
        ("www\\.a ", false),
        ("x\\@y.z", false),
        ("a&#97;", false),
    ] {
        time(unit, 1 << 9, normalize);
        let small = time(unit, 1 << 11, normalize);
        let large = time(unit, 1 << 13, normalize);
        assert!(
            large < small * 16,
            "{:?}: 4x input took {:?} against {:?}",
            unit,
            large,
            small
        );
    }
}
