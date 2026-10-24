// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink selection adapted from `mdast-util-to-markdown`.
use crate::{markdown::state::State, stream::EventKind, Document, NodeId};

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
    let bytes = destination.as_bytes();
    let scheme_end = bytes.iter().position(|&byte| byte == b':');
    let protocol = scheme_end.is_some_and(|end| {
        end >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1..end]
                .iter()
                .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'+' | b'.' | b'-'))
    });
    ((text == destination || destination.strip_prefix("mailto:") == Some(text))
        && protocol
        && !destination.chars().any(|character| {
            character.is_control() || character.is_whitespace() || matches!(character, '<' | '>')
        }))
    .then_some(text)
}
