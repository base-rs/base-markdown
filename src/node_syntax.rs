// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Optional authored syntax, kept outside the semantic event and node layouts.
use crate::{
    stream::{EventKind, StreamError, Tag, Text},
    CodeFence,
};

/// Source formatting that is useful for Markdown serialization but unnecessary for HTML.
///
/// Parsed documents derive this lazily from node provenance. Explicit overrides allocate only when a caller edits this metadata. Semantic values remain in the node's event kind.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeSyntax<'a> {
    /// The raw info and original opening and closing fences of the frontmatter of the document
    /// root that it belongs to.
    Frontmatter {
        info: Text<'a>,
        fence: CodeFence,
        closing_fence_length: usize,
    },
    /// Whether a list item contains blank lines between its own blocks.
    ListItem { spread: bool },
    /// How a link that is not a reference was written.
    Link { form: LinkForm },
}

/// The authored form of a link that is not a reference.
///
/// The serializer writes the stored form as it is, and a link without one explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum LinkForm {
    /// A GFM literal autolink, such as `www.example.com`, `https://example.com` or
    /// `user@example.com`, written as the link's text.
    Bare,
    /// An autolink in angle brackets, such as `<https://example.com>` or `<user@example.com>`.
    Angle,
    /// A link with its text and destination, such as `[text](https://example.com)`.
    Explicit,
}

impl NodeSyntax<'_> {
    pub(crate) fn borrowed(&self) -> NodeSyntax<'_> {
        match self {
            Self::Frontmatter {
                info,
                fence,
                closing_fence_length,
            } => NodeSyntax::Frontmatter {
                info: info.borrowed(),
                fence: *fence,
                closing_fence_length: *closing_fence_length,
            },
            Self::ListItem { spread } => NodeSyntax::ListItem { spread: *spread },
            Self::Link { form } => NodeSyntax::Link { form: *form },
        }
    }

    /// Whether this syntax can belong to a node of `kind`: an error for syntax of another kind of
    /// node or an invalid source range, as [`StreamError::InvalidRange`].
    pub(crate) fn validate(&self, kind: &EventKind<'_>, source: &str) -> Result<(), StreamError> {
        if self.valid_for(kind, source) {
            Ok(())
        } else {
            Err(StreamError::InvalidRange)
        }
    }

    fn valid_for(&self, kind: &EventKind<'_>, source: &str) -> bool {
        match (self, kind) {
            (
                Self::Frontmatter {
                    info,
                    fence,
                    closing_fence_length,
                },
                EventKind::Start(Tag::Document { .. }),
            ) => {
                info.resolve(source).is_ok()
                    && matches!(fence.character, '`' | '~' | '-' | '+')
                    && fence.length >= 3
                    && *closing_fence_length >= fence.length
            }
            (Self::ListItem { .. }, EventKind::Start(Tag::Item { .. }))
            | (Self::Link { .. }, EventKind::Start(Tag::Link { .. })) => true,
            _ => false,
        }
    }
}

/// Whether `label`, written as the label of a reference or definition, is no longer than the
/// parser reads one: 999 bytes, counting the escape the serializer writes before a bracket that
/// has none, or after a final backslash, but not a line ending or the spaces and tabs after one.
///
/// The parser counts a tab as the columns it spans, up to the next multiple of four, so how many
/// a tab counts depends on the column where its line of the label starts, which depends on where
/// the label is written. A label fits if it fits at some column of each of its lines: then the
/// parser can read it, and every label the parser read fits, at the columns where it was read. A
/// label that fits at no column, such as 300 tabs and a letter, can never be read back.
pub(crate) fn label_fits(label: &str) -> bool {
    let bytes = label.as_bytes();
    // The columns that the current line of the label spans, for each column modulo four where it
    // could start, and the least that each line before it spans.
    let mut line = [0; 4];
    let mut size = 0;
    let mut escaped = false;
    let mut at = 0;
    while at < bytes.len() {
        let width = match bytes[at] {
            b'\r' | b'\n' => {
                at += 1;
                while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
                    at += 1;
                }
                size += line.iter().min().expect("four columns");
                line = [0; 4];
                escaped = false;
                continue;
            }
            b'\t' => {
                for (start, columns) in line.iter_mut().enumerate() {
                    *columns += 4 - (start + *columns) % 4;
                }
                escaped = false;
                at += 1;
                continue;
            }
            b'[' | b']' if !escaped => 2,
            b'\\' => {
                escaped = !escaped;
                at += 1;
                for columns in &mut line {
                    *columns += 1;
                }
                continue;
            }
            _ => 1,
        };
        for columns in &mut line {
            *columns += width;
        }
        escaped = false;
        at += 1;
    }
    size + line.iter().min().expect("four columns") + usize::from(escaped)
        <= crate::util::constant::LINK_REFERENCE_SIZE_MAX
}

/// Whether `label`, written as it is as the label of a footnote definition, reads back as that
/// label, as `footnote_prefix` in the block parser reads one: it is not empty, holds no space, tab
/// or line ending, and no bracket without a backslash before it, does not end in an odd run of
/// backslashes, which would escape its closing bracket, and is at most 999 bytes long.
pub(crate) fn footnote_definition_label_reads_back(label: &str) -> bool {
    !label
        .bytes()
        .any(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        && footnote_call_label_reads_back(label)
        && label.len() <= crate::util::constant::LINK_REFERENCE_SIZE_MAX
}

/// Whether `label`, written as it is as the label of a footnote call, reads back as that label, as
/// the inline parser reads one: it holds something other than whitespace, no bracket without a
/// backslash before it and no blank line, does not end in an odd run of backslashes, which would
/// escape its closing bracket, and fits as [`label_fits`] counts.
pub(crate) fn footnote_call_label_reads_back(label: &str) -> bool {
    let bytes = label.as_bytes();
    let mut content = false;
    let mut blank = false;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'[' | b']' => return false,
            b'\r' | b'\n' if blank => return false,
            b'\r' if bytes.get(at + 1) == Some(&b'\n') => {
                blank = true;
                at += 1;
            }
            b'\r' | b'\n' => blank = true,
            b' ' | b'\t' => {}
            b'\\' if at + 1 == bytes.len() => return false,
            byte => {
                if byte == b'\\' && matches!(bytes[at + 1], b'[' | b']' | b'\\') {
                    at += 1;
                }
                content = true;
                blank = false;
            }
        }
        at += 1;
    }
    content && label_fits(label)
}

pub(crate) fn infer(
    source: &str,
    span: core::ops::Range<usize>,
    kind: &EventKind<'_>,
) -> Option<NodeSyntax<'static>> {
    let raw = source
        .get(span.clone())
        .expect("document source extents are validated before syntax inference");
    match kind {
        EventKind::Start(Tag::Link {
            destination,
            reference: None,
            ..
        }) => link_form(source, raw, destination),
        // The root's span is the whole source, which starts with its frontmatter.
        EventKind::Start(Tag::Document {
            frontmatter: Some(_),
            ..
        }) if span.start == 0 => {
            let extent = crate::block_parser::frontmatter_extent(raw)?;
            let (info, fence, closing_fence_length, _) = frontmatter(&raw[extent.clone()]);
            Some(NodeSyntax::Frontmatter {
                info: Text::Source(extent.start + info.start..extent.start + info.end),
                fence,
                closing_fence_length,
            })
        }
        _ => None,
    }
}

/// The form of a link that is not a reference, when its raw source has the shape of one: a
/// resource, or an autolink whose text the destination derives from as a reader derives it.
fn link_form(source: &str, raw: &str, destination: &Text<'_>) -> Option<NodeSyntax<'static>> {
    let destination = destination
        .resolve(source)
        .expect("document text ranges are validated");
    let derives = |text: &str| {
        destination == text
            || ["mailto:", "https://"]
                .iter()
                .any(|prefix| destination.strip_prefix(prefix) == Some(text))
    };
    let form = if raw.starts_with('[') && raw.ends_with(')') && raw.contains("](") {
        LinkForm::Explicit
    } else if let Some(text) = raw.strip_prefix('<').and_then(|raw| raw.strip_suffix('>')) {
        if !derives(text) {
            return None;
        }
        LinkForm::Angle
    } else if !raw.is_empty() && derives(raw) {
        LinkForm::Bare
    } else {
        return None;
    };
    Some(NodeSyntax::Link { form })
}

/// Return info and body ranges relative to a full frontmatter source extent, which the block
/// parser scanned as closed frontmatter.
pub(crate) fn frontmatter(
    raw: &str,
) -> (
    core::ops::Range<usize>,
    CodeFence,
    usize,
    core::ops::Range<usize>,
) {
    const SCANNED: &str = "the block parser scanned closed frontmatter";
    let bytes = raw.as_bytes();
    let fence = CodeFence::frontmatter_open(bytes).expect(SCANNED);
    let first_end = bytes
        .iter()
        .position(|b| matches!(b, b'\r' | b'\n'))
        .expect(SCANNED);
    let mut body_start = first_end + 1;
    if bytes[first_end] == b'\r' && bytes.get(body_start) == Some(&b'\n') {
        body_start += 1;
    }
    let raw_end = raw.trim_end_matches(['\r', '\n']).len();
    let closing_start = raw[..raw_end].rfind(['\r', '\n']).map_or(0, |i| i + 1);
    let close = raw[closing_start..raw_end].trim_matches([' ', '\t']);
    let closing_fence_length = fence.frontmatter_close(close.as_bytes()).expect(SCANNED);
    let info = raw[fence.length..first_end].trim_matches([' ', '\t']);
    let start = info.as_ptr() as usize - raw.as_ptr() as usize;
    (
        start..start + info.len(),
        fence,
        closing_fence_length,
        body_start..closing_start,
    )
}
