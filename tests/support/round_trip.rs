// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The round trip that the fuzz and the contract tests check: normalize, serialize, parse, normalize, compare.

use super::fixture::canonical;
use base_markdown::stream::{EventKind, Text};
use base_markdown::{
    render_events, to_markdown_with_options, CompileOptions, Document, LineWrapping,
    MarkdownOptions,
};
use regex::Regex;

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

/// The HTML of `document`, without what depends only on where line endings were in the source: the line ending at the end of the document, which a hand-built tree has no source for, and the soft break directly after a hard break, which is the line ending the parser keeps as part of the break (contract 3.3). A hand-built tree may have that soft break or not, and both are written as one break, so it is left out of either rendering, as `canonical` leaves it out of the tree. A text that starts with a line ending after a hard break is no soft break and stays.
pub fn html<S: AsRef<str>>(document: &Document<S>) -> String {
    let mut after_hard_break = false;
    let events = document.events().filter(|event| {
        let Ok(event) = event else { return true };
        match event.kind {
            EventKind::Separator { .. } => true,
            EventKind::SoftBreak(_) if after_hard_break => {
                after_hard_break = false;
                false
            }
            _ => {
                after_hard_break = matches!(event.kind, EventKind::HardBreak);
                true
            }
        }
    });
    let mut output = String::new();
    render_events(
        document.source(),
        events,
        &CompileOptions::default(),
        &mut output,
    )
    .unwrap();
    output.trim_end().into()
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
pub fn round_trip<S: AsRef<str> + Clone>(
    mut document: Document<S>,
    options: &MarkdownOptions,
    built: bool,
) -> Outcome {
    document.normalize();
    let before = html(&document);
    if !document.is_normalized() {
        return fail("normalized", String::new(), before, String::new());
    }
    // Unwrapped, the output reads back as the tree with its line endings as spaces.
    let expected = (options.line_wrapping == LineWrapping::Unwrap).then(|| unwrapped(&document));
    let (before, canonical_before) = match &expected {
        Some(expected) => (html(expected), canonical(expected)),
        None => (before, canonical(&document)),
    };
    let output = match to_markdown_with_options(&document, options) {
        Ok(output) => output,
        Err(_) if built => return Outcome::Rejected,
        Err(error) if error.rule_id == "unwritable-node" => return Outcome::Unwritable(error),
        Err(error) => return fail("error", error.to_string(), before, String::new()),
    };
    // The output reads back under the document's own options (contract 3.3).
    let mut reparsed = Document::parse(output.as_str(), document.parse_options()).unwrap();
    reparsed.normalize();
    let after = html(&reparsed);
    if !reparsed.is_normalized() {
        return fail("normalized", output, before, after);
    }
    if after != before {
        return fail("html", output, before, after);
    }
    // The HTML ignores line endings after hard breaks and at the end; the canonical trees do not, so dropping one is not a fix.
    let canonical_after = canonical(&reparsed);
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

/// `document` with each soft break but the line ending of a hard break, and each run of line endings, spaces and tabs in a text value that holds a line ending, one space, as `LineWrapping::Unwrap` writes them (3.3).
pub fn unwrapped<S: AsRef<str> + Clone>(document: &Document<S>) -> Document<S> {
    let line_endings = Regex::new(r"[ \t]*\n[ \t\n]*").unwrap();
    let mut unwrapped = document.clone();
    let mut after_hard_break = false;
    for id in document.descendants(document.root()) {
        let kind = &document.node(id).unwrap().kind;
        let after = core::mem::replace(&mut after_hard_break, matches!(kind, EventKind::HardBreak));
        match kind {
            // The line ending of a hard break is part of it.
            EventKind::SoftBreak(_) if after => {}
            EventKind::SoftBreak(_) => {
                let parent = document.node(id).unwrap().parent().unwrap();
                let space = unwrapped
                    .append_child(parent, EventKind::Text(Text::Literal(" ")))
                    .unwrap();
                unwrapped.attach(parent, space, Some(id)).unwrap();
                unwrapped.detach(id).unwrap();
            }
            EventKind::Text(value) => {
                let value = value.resolve(document.source()).unwrap();
                if value.contains('\n') {
                    *unwrapped.text_mut(id).unwrap() =
                        line_endings.replace_all(value, " ").into_owned();
                }
            }
            _ => {}
        }
    }
    unwrapped.normalize();
    unwrapped
}
