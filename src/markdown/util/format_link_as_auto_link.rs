// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink selection adapted from `mdast-util-to-markdown`.
use crate::{
    markdown::{construct_name::ConstructName, state::State},
    stream::EventKind,
    util::autolink::recognize,
    Document, NodeId,
};

pub fn auto_link_text<'a, S: AsRef<str>>(
    document: &'a Document<S>,
    id: NodeId,
    destination: &str,
    has_title: bool,
    state: &State,
) -> Option<&'a str> {
    if state.options.resource_link || destination.is_empty() || has_title {
        return None;
    }
    let mut children = document.children(id);
    let child = children.next()?;
    if children.next().is_some() {
        return None;
    }
    let EventKind::Text(text) = &document.node(child).expect("link child exists").kind else {
        return None;
    };
    let text = text.resolve(document.source()).unwrap();
    // GFM splits table cells at `|` before parsing autolinks, so a pipe in a cell keeps the resource form.
    (angle_reads_back(text, destination)
        && !(text.contains('|') && state.stack.contains(&ConstructName::TableCell)))
    .then_some(text)
}

/// Whether the parser reads `<text>`, written verbatim, back as an autolink to `destination`.
///
/// This uses the parser's own recognizer, so it accepts only the schemes in the link-scheme
/// table, each followed by what its kind requires, or an email address.
pub fn angle_reads_back(text: &str, destination: &str) -> bool {
    let Some((length, email)) = recognize(text.bytes().chain(Some(b'>'))) else {
        return false;
    };
    let target = if email {
        destination.strip_prefix("mailto:")
    } else {
        Some(destination)
    };
    length == text.len() + 1
        && target == Some(text)
        && !text
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}
