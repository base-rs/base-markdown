// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The round trip that the fuzz and the contract tests check: normalize, serialize, parse, normalize, compare.

use super::fixture::canonical;
use base_markdown::{to_markdown_with_options, CompileOptions, Document, MarkdownOptions};

pub struct Failure {
    pub kind: &'static str,
    pub output: String,
    pub before: String,
    pub after: String,
}

pub enum Outcome {
    Pass,
    Rejected,
    /// A parsed document that the writer returns `unwritable-node` for, as some shape in it has no
    /// spelling that reads back under its options (contract 3.3).
    Unwritable(base_markdown::message::Message),
    Fail(Failure),
}

/// The HTML of `document`, without what depends only on where line endings were in the source: the line ending at the end of the document, which a hand-built tree has no source for, and, in a parsed document, the one after a hard break, which is part of the break when parsed. A hand-built tree has no line ending there, so a line ending after its hard break is text and stays.
pub fn html<S: AsRef<str>>(document: &Document<S>, built: bool) -> String {
    let mut output = String::new();
    document
        .render_html(&CompileOptions::default(), &mut output)
        .unwrap();
    let output = output.trim_end();
    if built {
        output.into()
    } else {
        output.replace("<br />\n", "<br />")
    }
}

pub fn fail(kind: &'static str, output: String, before: String, after: String) -> Outcome {
    Outcome::Fail(Failure {
        kind,
        output,
        before,
        after,
    })
}

/// Normalize, serialize, parse under the document's own options and normalize `document`, and compare. A hand-built tree, which a serializer error rejects rather than fails, is `built`.
pub fn round_trip<S: AsRef<str>>(
    mut document: Document<S>,
    options: &MarkdownOptions,
    built: bool,
) -> Outcome {
    document.normalize();
    let before = html(&document, built);
    if !document.is_normalized() {
        return fail("normalized", String::new(), before, String::new());
    }
    let output = match to_markdown_with_options(&document, options) {
        Ok(output) => output,
        Err(_) if built => return Outcome::Rejected,
        Err(error) if error.rule_id == "unwritable-node" => return Outcome::Unwritable(error),
        Err(error) => return fail("error", error.to_string(), before, String::new()),
    };
    // The output reads back under the document's own options (contract 3.3).
    let mut reparsed = Document::parse(output.as_str(), document.parse_options()).unwrap();
    reparsed.normalize();
    let after = html(&reparsed, false);
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
