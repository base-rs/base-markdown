// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink selection adapted from `mdast-util-to-markdown`.
use crate::{
    markdown::state::State, stream::EventKind, util::autolink::recognize, Document, NodeId,
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
    // Write `<text>` verbatim only when the parser reads it back as this link, which also limits schemes to `http`, `https` and `mailto`. GFM splits table cells at `|` before parsing autolinks, so a pipe keeps the resource form.
    let (length, email) = recognize(text.bytes().chain(Some(b'>')))?;
    let target = if email {
        destination.strip_prefix("mailto:")
    } else {
        Some(destination)
    };
    (length == text.len() + 1
        && target == Some(text)
        && !text.chars().any(|character| {
            character.is_control() || character.is_whitespace() || character == '|'
        }))
    .then_some(text)
}
