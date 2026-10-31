// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink selection adapted from `mdast-util-to-markdown`.
use crate::{
    markdown::{construct_name::ConstructName, state::State},
    stream::EventKind,
    util::autolink::recognize,
    Document, LinkForm, NodeId, NodeSyntax,
};

/// How the serializer writes a link that is not a reference.
pub enum WrittenLink<'a> {
    /// The link's text, as a GFM literal autolink.
    Bare(&'a str),
    /// The text between angle brackets.
    Angle(&'a str),
    /// `[text](destination)`.
    Explicit,
}

/// Write a link in its stored form, trusting it, unless that form cannot hold the link at all.
/// A link without a stored form is written explicitly.
pub fn written_link<'a, S: AsRef<str>>(
    document: &'a Document<S>,
    id: NodeId,
    destination: &str,
    has_title: bool,
    state: &State,
) -> WrittenLink<'a> {
    match document.syntax(id) {
        Some(NodeSyntax::Link {
            form: LinkForm::Bare,
        }) if !state.options.resource_link && !has_title => match link_text(document, id) {
            Some(text) if !text.is_empty() => WrittenLink::Bare(text),
            _ => WrittenLink::Explicit,
        },
        Some(NodeSyntax::Link {
            form: LinkForm::Angle,
        }) => auto_link_text(document, id, destination, has_title, state)
            .map_or(WrittenLink::Explicit, WrittenLink::Angle),
        _ => WrittenLink::Explicit,
    }
}

/// The text of a link whose only child is text.
fn link_text<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> Option<&str> {
    let mut children = document.children(id);
    let child = children.next()?;
    if children.next().is_some() {
        return None;
    }
    let EventKind::Text(text) = &document.node(child).expect("link child exists").kind else {
        return None;
    };
    Some(text.resolve(document.source()).unwrap())
}

/// The text to write between angle brackets, when an autolink can hold the link.
fn auto_link_text<'a, S: AsRef<str>>(
    document: &'a Document<S>,
    id: NodeId,
    destination: &str,
    has_title: bool,
    state: &State,
) -> Option<&'a str> {
    if state.options.resource_link || destination.is_empty() || has_title {
        return None;
    }
    let text = link_text(document, id)?;
    // GFM splits table cells at `|` before parsing autolinks, so a pipe in a cell keeps the resource form.
    (angle_reads_back(text, destination)
        && !(text.contains('|') && state.stack.contains(&ConstructName::TableCell)))
    .then_some(text)
}

/// Whether the parser reads `<text>`, written verbatim, back as an autolink to `destination`.
///
/// This uses the parser's own recognizer, so it accepts only `http://`, `https://` and `mailto:`
/// with a target, or an email address.
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
