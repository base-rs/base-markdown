// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Setext heading selection adapted from `mdast-util-to-markdown`.
use crate::{
    markdown::state::State,
    stream::{EventKind, Tag},
    Document, NodeId,
};

pub fn format_heading_as_setext<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
    state: &State,
) -> bool {
    if !matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Heading(1 | 2))
    ) {
        return false;
    }
    let literal_break =
        document
            .descendants(id)
            .any(|child| match &document.node(child).unwrap().kind {
                EventKind::HardBreak | EventKind::SoftBreak(_) => true,
                EventKind::Code { value, .. }
                | EventKind::Html { value, .. }
                | EventKind::Math(value)
                | EventKind::Text(value) => value
                    .resolve(document.source())
                    .unwrap()
                    .contains(['\r', '\n']),
                _ => false,
            });
    !document.text_content(id).is_empty() && (state.options.setext || literal_break)
}
