// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Autolink selection adapted from `mdast-util-to-markdown`.
use crate::{
    inline_parser::literal_autolink_prefix,
    markdown::{construct_name::ConstructName, state::State},
    stream::{EventKind, Tag},
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

/// Write a link in its stored form, unless that form cannot hold the link at all. A link without a
/// stored form is written explicitly.
///
/// A bare link is written bare only between whitespace or line boundaries, where nothing next to
/// it can change where it ends. `after_boundary` says whether the output before the link ends at
/// one. Anywhere else it is written as `<…>` when that reads back as the same link, and
/// explicitly otherwise.
pub fn written_link<'a, S: AsRef<str>>(
    document: &'a Document<S>,
    id: NodeId,
    destination: &str,
    has_title: bool,
    after_boundary: bool,
    state: &State,
) -> WrittenLink<'a> {
    match document.syntax(id) {
        Some(NodeSyntax::Link {
            form: LinkForm::Bare,
        }) if !state.options.resource_link && !has_title => match link_text(document, id) {
            Some(text)
                if reads_back_bare(text, destination)
                    && after_boundary
                    && boundary_after(document, id)
                    && !(text.contains('|') && state.stack.contains(&ConstructName::TableCell)) =>
            {
                WrittenLink::Bare(text)
            }
            _ => auto_link_text(document, id, destination, has_title, state)
                .map_or(WrittenLink::Explicit, WrittenLink::Angle),
        },
        Some(NodeSyntax::Link {
            form: LinkForm::Angle,
        }) => auto_link_text(document, id, destination, has_title, state)
            .map_or(WrittenLink::Explicit, WrittenLink::Angle),
        _ => WrittenLink::Explicit,
    }
}

/// Whether the parser reads `text`, written alone, back as one literal autolink to `destination`.
/// Edited text that no longer forms that link, or forms another, is not written bare.
fn reads_back_bare(text: &str, destination: &str) -> bool {
    literal_autolink_prefix(text)
        .is_some_and(|prefix| destination.strip_prefix(prefix) == Some(text))
}

/// Whether a bare link may touch `character`: a space, a tab or a line ending.
pub fn boundary(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

/// Whether `node`, written before a link, ends at a boundary, judged from the tree alone.
pub fn ends_at_boundary<S: AsRef<str>>(document: &Document<S>, node: NodeId) -> bool {
    match &document.node(node).expect("phrasing node exists").kind {
        EventKind::Text(value) => value
            .resolve(document.source())
            .unwrap()
            .chars()
            .next_back()
            .is_some_and(boundary),
        EventKind::SoftBreak(_) | EventKind::HardBreak => true,
        _ => false,
    }
}

/// Whether what follows the link starts at a boundary: whitespace before more content, a line
/// ending, or the end of a paragraph, heading or table cell. Spaces and tabs before a line ending
/// or the end of the block are written as references, which a link would take, so they do not
/// count, and neither does a node whose first character is only known once written.
fn boundary_after<S: AsRef<str>>(document: &Document<S>, id: NodeId) -> bool {
    let node = document.node(id).expect("link exists");
    // Whether spaces or tabs come first, which the parser may have split over several texts.
    let mut spaces = false;
    for sibling in written_siblings(document, node.next_sibling()) {
        match &document.node(sibling).unwrap().kind {
            EventKind::Text(value) => {
                let value = value.resolve(document.source()).unwrap();
                let rest = value.trim_start_matches([' ', '\t']);
                spaces |= rest.len() < value.len();
                match rest.chars().next() {
                    Some('\n' | '\r') => return !spaces,
                    Some(_) => return spaces,
                    None => {}
                }
            }
            EventKind::SoftBreak(_) => return !spaces,
            EventKind::HardBreak => return false,
            _ => return spaces,
        }
    }
    !spaces
        && node.parent().is_some_and(|parent| {
            matches!(
                document.node(parent).unwrap().kind,
                EventKind::Start(Tag::Paragraph { .. } | Tag::Heading(_) | Tag::TableCell { .. })
            )
        })
}

/// `first` and the siblings after it, skipping empty text, which writes nothing.
fn written_siblings<S: AsRef<str>>(
    document: &Document<S>,
    first: Option<NodeId>,
) -> impl Iterator<Item = NodeId> + '_ {
    core::iter::successors(first, move |&sibling| {
        document.node(sibling).unwrap().next_sibling()
    })
    .filter(move |&sibling| {
        !matches!(
            &document.node(sibling).unwrap().kind,
            EventKind::Text(value) if value.resolve(document.source()).unwrap().is_empty()
        )
    })
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
