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
    /// The decoded complete code info string, or the metadata after a math fence.
    ///
    /// The info string of code starts with its language, as the parser reads it: it is empty for
    /// code without a language, and otherwise it is the language, alone or followed by a space or
    /// tab and the metadata. [`Document::set_syntax`][crate::Document::set_syntax] rejects any
    /// other info string with [`StreamError::InvalidValue`]. The spaces and tabs between the
    /// language and the metadata are written as one space, and whitespace at the end of the
    /// metadata, or metadata of only whitespace after the first space or tab, is encoded so it
    /// reads back.
    Code { info: Text<'a> },
    /// Frontmatter's raw info and original opening and closing fences.
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
            Self::Link { form } => NodeSyntax::Link { form: *form },
        }
    }

    // References cross documents as resolved resources: the destination buffer may have a conflicting or missing definition with the same label. A link's form describes only the link's own text and destination, which are copied with it, so it survives.
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
            Self::Link { form } => NodeSyntax::Link { form },
        })
    }

    /// Whether this syntax can belong to a node of `kind`: an error for syntax of another kind of
    /// node or an invalid source range, as [`StreamError::InvalidRange`], and for a code info
    /// string that does not start with the code's language, as [`StreamError::InvalidValue`].
    pub(crate) fn validate(&self, kind: &EventKind<'_>, source: &str) -> Result<(), StreamError> {
        if !self.valid_for(kind, source) {
            return Err(StreamError::InvalidRange);
        }
        if let Self::Reference { label, .. } | Self::Footnote { label } = self {
            if !label_fits(label.resolve(source).expect("checked by `valid_for`")) {
                return Err(StreamError::InvalidValue);
            }
        }
        if let (
            Self::Code { info },
            EventKind::CodeBlock {
                language,
                math: false,
                ..
            },
        ) = (self, kind)
        {
            // The info string is checked above, and the language when the kind was assigned.
            let info = info.resolve(source).expect("checked by `valid_for`");
            let language = language
                .as_ref()
                .map(|value| value.resolve(source).expect("document text is validated"));
            if !code_info_starts_with(info, language) {
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

/// Whether `info` is a code info string whose first word is `language`, as the parser reads it:
/// empty without a language, and otherwise the language, alone or followed by a space or tab and
/// the metadata. The parser splits the language off at the first raw space or tab, so a language
/// that does not end where the info string does or before a space or tab would read back
/// different, and code without a language has no info string.
fn code_info_starts_with(info: &str, language: Option<&str>) -> bool {
    match language {
        None => info.is_empty(),
        Some(language) => info
            .strip_prefix(language)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t'])),
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
        EventKind::CodeBlock {
            fence: Some(fence),
            language,
            math,
            ..
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
            let range = start..start + info.len();
            // The document does not keep the options it was parsed with, which decide whether
            // escapes and references in the info string were decoded, so keep the first decoding
            // that starts with the language as the parser split it off: the info string the
            // parser read. Code from caller events may have a language its source does not
            // start with, and then has no inferred info string. Math has no language, so any
            // metadata is valid, decoded as by default.
            let language = language
                .as_ref()
                .map(|value| value.resolve(source).expect("document text is validated"));
            [(true, true), (false, false), (true, false), (false, true)]
                .iter()
                .map(|&(escapes, references)| {
                    crate::value_builder::decode_info(source, range.clone(), escapes, references)
                })
                .find(|info| {
                    *math
                        || code_info_starts_with(
                            info.resolve(source).expect("decoded from the source"),
                            language,
                        )
                })
                .map(|info| NodeSyntax::Code { info })
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
    let start = info.as_ptr() as usize - raw.as_ptr() as usize;
    Some((
        start..start + info.len(),
        fence,
        closing_fence_length,
        body_start..closing_start,
    ))
}
