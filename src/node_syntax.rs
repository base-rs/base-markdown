// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Optional authored syntax, kept outside the semantic event and node layouts.
use crate::{
    stream::{EventKind, StreamError, Tag, Text},
    CodeFence, ReferenceKind,
};

/// Source formatting that is useful for Markdown serialization but unnecessary for HTML.
///
/// Parsed documents derive this lazily from node provenance. Explicit overrides allocate only when a caller edits this metadata. Semantic values remain in the node's event kind.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeSyntax<'a> {
    /// The authored reference label and whether it was full, collapsed, or shortcut.
    ///
    /// [`Document::set_syntax`][crate::Document::set_syntax] rejects a label longer than the
    /// parser reads, as for a [`Definition`][crate::stream::EventKind::Definition]'s, with
    /// [`StreamError::InvalidValue`], since it would be written as text. A footnote label is
    /// checked the same way.
    Reference {
        kind: ReferenceKind,
        label: Text<'a>,
    },
    /// The authored spelling of a normalized footnote identifier.
    Footnote { label: Text<'a> },
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
            Self::Reference { kind, label } => NodeSyntax::Reference {
                kind: *kind,
                label: label.borrowed(),
            },
            Self::Footnote { label } => NodeSyntax::Footnote {
                label: label.borrowed(),
            },
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

    // References cross documents as resolved resources: the destination buffer may have a conflicting or missing definition with the same label. A link's form describes only the link's own text and destination, which are copied with it, so it survives.
    pub(crate) fn copied(self, source: &str) -> Option<NodeSyntax<'static>> {
        let own = |text: Text<'_>| text.materialized(source);
        Some(match self {
            Self::Footnote { label } => NodeSyntax::Footnote { label: own(label) },
            Self::ListItem { spread } => NodeSyntax::ListItem { spread },
            Self::Link { form } => NodeSyntax::Link { form },
            // A reference, and frontmatter, which is on the root and never copied.
            _ => return None,
        })
    }

    /// Whether this syntax can belong to a node of `kind`: an error for syntax of another kind of
    /// node or an invalid source range, as [`StreamError::InvalidRange`], and for a label longer
    /// than the parser reads, as [`StreamError::InvalidValue`].
    pub(crate) fn validate(&self, kind: &EventKind<'_>, source: &str) -> Result<(), StreamError> {
        if !self.valid_for(kind, source) {
            return Err(StreamError::InvalidRange);
        }
        if let Self::Reference { label, .. } | Self::Footnote { label } = self {
            if !label_fits(label.resolve(source).expect("checked by `valid_for`")) {
                return Err(StreamError::InvalidValue);
            }
        }
        Ok(())
    }

    fn valid_for(&self, kind: &EventKind<'_>, source: &str) -> bool {
        match (self, kind) {
            (
                Self::Reference { label, .. },
                EventKind::Start(Tag::Link { .. } | Tag::Image { .. }),
            )
            | (
                Self::Footnote { label },
                EventKind::Start(Tag::FootnoteDefinition(_)) | EventKind::FootnoteReference(_),
            ) => label.resolve(source).is_ok(),
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

impl NodeSyntax<'static> {
    /// This reference or footnote syntax with `label` in place of its label.
    pub(crate) fn with_label(self, label: alloc::string::String) -> Self {
        match self {
            Self::Reference { kind, .. } => Self::Reference {
                kind,
                label: Text::from(label),
            },
            Self::Footnote { .. } => Self::Footnote {
                label: Text::from(label),
            },
            _ => unreachable!("only references and footnote calls have read labels"),
        }
    }
}

/// Whether `label`, written as the label of a reference, definition or footnote, is no longer
/// than the parser reads one: 999 bytes, counting the escape the serializer writes before a
/// bracket that has none, or after a final backslash, but not a line ending or the spaces and
/// tabs after one. A tab counts as one byte, as a parsed label's tabs are at least that wide.
pub(crate) fn label_fits(label: &str) -> bool {
    let bytes = label.as_bytes();
    let mut size = 0;
    let mut escaped = false;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\r' | b'\n' => {
                at += 1;
                while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\r' | b'\n') {
                    at += 1;
                }
                escaped = false;
                continue;
            }
            b'[' | b']' if !escaped => size += 2,
            b'\\' => {
                size += 1;
                escaped = !escaped;
                at += 1;
                continue;
            }
            _ => size += 1,
        }
        escaped = false;
        at += 1;
    }
    size + usize::from(escaped) <= crate::util::constant::LINK_REFERENCE_SIZE_MAX
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
        EventKind::Start(tag @ (Tag::Link { .. } | Tag::Image { .. })) => {
            let prefix = if raw.starts_with("![") {
                2
            } else if raw.starts_with('[') {
                1
            } else {
                return link_form(source, raw, tag);
            };
            if !raw.ends_with(']') {
                return link_form(source, raw, tag);
            }
            // Match the final bracket pair, respecting nested and backslash-escaped brackets. Code inside the first label is irrelevant to matching the final reference suffix.
            let bytes = raw.as_bytes();
            let mut depth = 0;
            let mut start = None;
            // Only brackets affect nesting. Each preceding escape run is examined once.
            for index in memchr::memrchr2_iter(b'[', b']', bytes) {
                let mut back = index;
                while back > 0 && bytes[back - 1] == b'\\' {
                    back -= 1;
                }
                if (index - back) % 2 == 1 {
                    continue;
                }
                if bytes[index] == b']' {
                    depth += 1;
                } else {
                    depth -= 1;
                    if depth == 0 {
                        start = Some(index);
                        break;
                    }
                }
            }
            let start = start?;
            let (kind, label) = if start > prefix && bytes[start - 1] == b']' {
                if start + 1 == bytes.len() - 1 {
                    (
                        ReferenceKind::Collapsed,
                        span.start + prefix..span.start + start - 1,
                    )
                } else {
                    (ReferenceKind::Full, span.start + start + 1..span.end - 1)
                }
            } else {
                (ReferenceKind::Shortcut, span.start + prefix..span.end - 1)
            };
            // A label is written as authored, but its line endings are LF like every value's.
            Some(NodeSyntax::Reference {
                kind,
                label: Text::Source(label).normalized(source),
            })
        }
        EventKind::Start(Tag::FootnoteDefinition(_)) | EventKind::FootnoteReference(_) => {
            let start = raw.find("[^")? + 2;
            let mut end = start;
            while end < raw.len() {
                match raw.as_bytes()[end] {
                    b'\\' => end += 2,
                    b']' => break,
                    _ => end += 1,
                }
            }
            if end >= raw.len() {
                return None;
            }
            Some(NodeSyntax::Footnote {
                label: Text::Source(span.start + start..span.start + end).normalized(source),
            })
        }
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
fn link_form(source: &str, raw: &str, tag: &Tag<'_>) -> Option<NodeSyntax<'static>> {
    let Tag::Link { destination, .. } = tag else {
        return None;
    };
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
