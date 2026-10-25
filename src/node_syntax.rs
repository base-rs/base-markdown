// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Optional authored syntax, kept outside the semantic event and node layouts.
use crate::{
    stream::{EventKind, Tag, Text},
    CodeFence, ReferenceKind,
};

/// Source formatting that is useful for Markdown serialization but unnecessary for HTML.
///
/// Parsed documents derive this lazily from node provenance. Explicit overrides allocate only when a caller edits this metadata. Semantic values remain in the node's event kind.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeSyntax<'a> {
    /// The authored reference label and whether it was full, collapsed, or shortcut.
    Reference {
        kind: ReferenceKind,
        label: Text<'a>,
    },
    /// The authored spelling of a normalized footnote identifier.
    Footnote { label: Text<'a> },
    /// The decoded complete code info string, or the metadata after a math fence.
    Code { info: Text<'a> },
    /// Frontmatter's raw info and original opening and closing fences.
    Frontmatter {
        info: Text<'a>,
        fence: CodeFence,
        closing_fence_length: usize,
    },
    /// Whether a list item contains blank lines between its own blocks.
    ListItem { spread: bool },
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
            Self::Code { info } => NodeSyntax::Code {
                info: info.borrowed(),
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
        }
    }

    // References cross documents as resolved resources: the destination buffer may have a conflicting or missing definition with the same label.
    pub(crate) fn copied(self, source: &str) -> Option<NodeSyntax<'static>> {
        let own = |text: Text<'_>| text.materialized(source);
        Some(match self {
            Self::Reference { .. } => return None,
            Self::Footnote { label } => NodeSyntax::Footnote { label: own(label) },
            Self::Code { info } => NodeSyntax::Code { info: own(info) },
            Self::Frontmatter {
                info,
                fence,
                closing_fence_length,
            } => NodeSyntax::Frontmatter {
                info: own(info),
                fence,
                closing_fence_length,
            },
            Self::ListItem { spread } => NodeSyntax::ListItem { spread },
        })
    }

    pub(crate) fn valid_for(&self, kind: &EventKind<'_>, source: &str) -> bool {
        match (self, kind) {
            (
                Self::Reference { label, .. },
                EventKind::Start(Tag::Link { .. } | Tag::Image { .. }),
            )
            | (
                Self::Footnote { label },
                EventKind::Start(Tag::FootnoteDefinition(_)) | EventKind::FootnoteReference(_),
            ) => label.resolve(source).is_ok(),
            (Self::Code { info }, EventKind::CodeBlock { .. }) => info.resolve(source).is_ok(),
            (
                Self::Frontmatter {
                    info,
                    fence,
                    closing_fence_length,
                },
                EventKind::Opaque { block: true, .. },
            ) => {
                info.resolve(source).is_ok()
                    && matches!(fence.character, '`' | '~' | '-' | '+')
                    && fence.length >= 3
                    && *closing_fence_length >= fence.length
            }
            (Self::ListItem { .. }, EventKind::Start(Tag::Item { .. })) => true,
            _ => false,
        }
    }
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
        EventKind::Start(Tag::Link { .. } | Tag::Image { .. }) => {
            let prefix = if raw.starts_with("![") {
                2
            } else if raw.starts_with('[') {
                1
            } else {
                return None;
            };
            if !raw.ends_with(']') {
                return None;
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
            Some(NodeSyntax::Reference {
                kind,
                label: Text::Source(label),
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
                label: Text::Source(span.start + start..span.start + end),
            })
        }
        EventKind::CodeBlock {
            fence: Some(fence), ..
        } => {
            let first = &raw[..raw.find(['\r', '\n']).unwrap_or(raw.len())];
            let sequence = &first[first.find(fence.character)?..];
            let info = sequence.get(fence.length..)?;
            if !sequence[..fence.length]
                .chars()
                .all(|character| character == fence.character)
            {
                return None;
            }
            let info = info.trim_matches([' ', '\t']);
            let start = span.start + (info.as_ptr() as usize - first.as_ptr() as usize);
            let text = crate::value_builder::decode_string(source, start..start + info.len());
            Some(NodeSyntax::Code { info: text })
        }
        EventKind::Opaque { block: true, .. } => frontmatter(raw).map(
            |(info, fence, closing_fence_length, _)| NodeSyntax::Frontmatter {
                info: Text::Source(span.start + info.start..span.start + info.end),
                fence,
                closing_fence_length,
            },
        ),
        _ => None,
    }
}

/// Return info and body ranges relative to a full frontmatter source extent.
pub(crate) fn frontmatter(
    raw: &str,
) -> Option<(
    core::ops::Range<usize>,
    CodeFence,
    usize,
    core::ops::Range<usize>,
)> {
    let bytes = raw.as_bytes();
    let fence = CodeFence::frontmatter_open(bytes)?;
    let first_end = bytes.iter().position(|b| matches!(b, b'\r' | b'\n'))?;
    let mut body_start = first_end + 1;
    if bytes[first_end] == b'\r' && bytes.get(body_start) == Some(&b'\n') {
        body_start += 1;
    }
    let raw_end = raw.trim_end_matches(['\r', '\n']).len();
    let closing_start = raw[..raw_end].rfind(['\r', '\n']).map_or(0, |i| i + 1);
    if closing_start < body_start {
        return None;
    }
    let close = raw[closing_start..raw_end].trim_matches([' ', '\t']);
    let closing_fence_length = fence.frontmatter_close(close.as_bytes())?;
    let info = raw[fence.length..first_end].trim_matches([' ', '\t']);
    if !fence.valid_frontmatter_info(info.as_bytes()) {
        return None;
    }
    let start = info.as_ptr() as usize - raw.as_ptr() as usize;
    Some((
        start..start + info.len(),
        fence,
        closing_fence_length,
        body_start..closing_start,
    ))
}
