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

/// The columns `label` spans as the parser counts a label's size: each byte but a line ending
/// and the spaces and tabs after one, and a tab as the columns up to the next multiple of four.
/// How many a tab spans depends on the column where its line of the label starts, which depends
/// on where the label is written, so this is the least size, at the column of each line where
/// that line is smallest, and the most, with every tab four columns wide.
fn label_size(label: &str) -> (usize, usize) {
    let bytes = label.as_bytes();
    // The columns that the current line of the label spans, for each column modulo four where it
    // could start, and the least that each line before it spans.
    let mut line = [0; 4];
    let mut least = 0;
    let mut most = 0;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\n' => {
                at += 1;
                while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n') {
                    at += 1;
                }
                least += line.iter().min().expect("four columns");
                line = [0; 4];
                continue;
            }
            b'\t' => {
                for (start, columns) in line.iter_mut().enumerate() {
                    *columns += 4 - (start + *columns) % 4;
                }
                most += 4;
            }
            _ => {
                for columns in &mut line {
                    *columns += 1;
                }
                most += 1;
            }
        }
        at += 1;
    }
    (least + line.iter().min().expect("four columns"), most)
}

/// Whether `label`, which reads back as [`valid_label`] checks, is no longer than the parser
/// reads one, 999 bytes as [`label_size`] counts them, at some column of each of its lines: then
/// the parser can read it, and every label the parser read fits, at the columns where it was
/// read. A label that fits at no column, such as 300 tabs and a letter, can never be read back.
pub(crate) fn label_fits(label: &str) -> bool {
    label_size(label).0 <= crate::util::constant::LINK_REFERENCE_SIZE_MAX
}

/// Whether `label` fits as [`label_fits`] checks wherever it is written, with each tab as wide as
/// it can be.
pub(crate) fn label_fits_anywhere(label: &str) -> bool {
    label_size(label).1 <= crate::util::constant::LINK_REFERENCE_SIZE_MAX
}

/// Whether `label` is a label the parser can read, with any options, as the label of a definition,
/// a reference, or a footnote call or definition: it holds something other than spaces, tabs and
/// line endings, no carriage return, as the parser stores each line ending in a label as LF, no
/// blank line, no bracket without a backslash before it, and does not end in a backslash that
/// would escape its closing bracket. When `limited`, it also fits as [`label_fits`] checks: the
/// parser reads a shortcut or collapsed reference's label, which is its link text, at any size,
/// and every other one only up to that size. Whether a line of it starts a block depends on the
/// options it is read with, as each such block can be turned off, so it is not checked here:
/// [`normalized_label`] gives a label the shape that the Markdown serializer writes so that it
/// reads back with the default options.
pub(crate) fn valid_label(label: &str, limited: bool) -> bool {
    let bytes = label.as_bytes();
    let mut content = false;
    let mut escaped = false;
    let mut at = 0;
    loop {
        let end = at
            + bytes[at..]
                .iter()
                .position(|&byte| byte == b'\n')
                .unwrap_or(bytes.len() - at);
        let line = &bytes[at..end];
        if at > 0 && end < bytes.len() && leading_whitespace(line) == line.len() {
            return false;
        }
        for &byte in line {
            match byte {
                b'\r' => return false,
                b'[' | b']' if !escaped => return false,
                b' ' | b'\t' => {}
                _ => content = true,
            }
            escaped = byte == b'\\' && !escaped;
        }
        if end == bytes.len() {
            break;
        }
        escaped = false;
        at = end + 1;
    }
    content && !escaped && (!limited || label_fits(label))
}

// Whether `line`, a line of a label after its first and before its last, could be a table's
// delimiter row, as the parser reads one with the default options, under whatever line is written
// before it: what comes before a label's first line depends on where it is written, so the line
// before is not checked. Indented four columns, it is none, as the parser reads such a line as
// indented code, which cannot interrupt a paragraph, in any container.
fn delimiter_row(line: &[u8]) -> bool {
    let indent = leading_whitespace(line);
    indentation(&line[..indent]).0 < 4 && crate::block_parser::delimiter_row(&line[indent..])
}

// Whether `line`, a line of a label after its first, without its line ending, continues the
// label's paragraph, or its `definition`, at every column where it could be written, as the
// parser reads it with the default options, `last` when the closing bracket follows it.
fn continuation_writable(line: &[u8], last: bool, definition: bool) -> bool {
    let indent = leading_whitespace(line);
    if indentation(&line[..indent]).0 >= 4 {
        return true;
    }
    let mut rest = alloc::vec::Vec::with_capacity(line.len() - indent + 1);
    rest.extend_from_slice(&line[indent..]);
    rest.push(if last { b']' } else { b'\n' });
    !crate::block_parser::continuation_starts_block(&rest, definition)
}

// How many spaces and tabs `line` starts with.
fn leading_whitespace(line: &[u8]) -> usize {
    line.iter()
        .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
        .count()
}

// The least and most columns that `whitespace`, of spaces and tabs, spans, at the columns where
// it could start.
fn indentation(whitespace: &[u8]) -> (usize, usize) {
    let mut columns = [0; 4];
    for &byte in whitespace {
        for (start, columns) in columns.iter_mut().enumerate() {
            *columns += if byte == b'\t' {
                4 - (start + *columns) % 4
            } else {
                1
            };
        }
    }
    (
        *columns.iter().min().expect("four columns"),
        *columns.iter().max().expect("four columns"),
    )
}

/// Whether `label`, written as it is as the label of a footnote definition, reads back as that
/// label, as `footnote_prefix` in the block parser reads one: it reads back as
/// [`valid_label`] checks, limited, and holds no space, tab or line ending, so it is at most
/// 999 bytes long.
pub(crate) fn valid_footnote_definition_label(label: &str) -> bool {
    !label
        .bytes()
        .any(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        && valid_label(label, true)
}

/// The label that [`crate::Document::normalize`] gives a reference, a definition, `definition`, or
/// a footnote call whose label is `label`, which reads back as [`valid_label`] checks, if it
/// changes. Where it is written on one line, `one_line`, as in an ATX heading or a table cell,
/// each run of spaces, tabs and line endings in a label that holds a line ending becomes one
/// space. Otherwise, the spaces and tabs that a line after the first starts with become four
/// spaces when that line, read with the default options, would start a block at some column
/// where it could start, such as `>` or `` ``` ``, as
/// [`crate::block_parser::continuation_starts_block`] decides with the bracket that closes the
/// label after the last, or could be a table's delimiter row under whatever line is written before
/// it: indented four columns, it continues the label at every column. When `tabs` and the label
/// does not fit as [`label_fits_anywhere`] checks, each other tab becomes a space. None of these
/// changes what the label matches, as a run of whitespace in a label matches any other, and the
/// label then reads back as it is wherever it is written, unless it is too long once its line
/// endings are spaces.
pub(crate) fn normalized_label(
    label: &str,
    one_line: bool,
    tabs: bool,
    definition: bool,
) -> Option<alloc::string::String> {
    let bytes = label.as_bytes();
    let mut value = alloc::string::String::new();
    if one_line && bytes.contains(&b'\n') {
        value.reserve(label.len());
        let mut space = false;
        for character in label.chars() {
            let whitespace = matches!(character, ' ' | '\t' | '\n');
            if !whitespace {
                value.push(character);
            } else if !space {
                value.push(' ');
            }
            space = whitespace;
        }
        return Some(value);
    }
    let tabs = tabs && !label_fits_anywhere(label);
    // The end of what `value` holds a copy of, once anything changes.
    let mut cursor = 0;
    let mut start = 0;
    loop {
        let end = start
            + bytes[start..]
                .iter()
                .position(|&byte| byte == b'\n')
                .unwrap_or(bytes.len() - start);
        let line = &bytes[start..end];
        let last = end == bytes.len();
        let mut indent = 0;
        if start > 0 {
            indent = leading_whitespace(line);
            if !continuation_writable(line, last, definition) || !last && delimiter_row(line) {
                value.push_str(&label[cursor..start]);
                value.push_str("    ");
                cursor = start + indent;
            }
        }
        if tabs {
            for (offset, &byte) in line[indent..].iter().enumerate() {
                if byte == b'\t' {
                    let at = start + indent + offset;
                    value.push_str(&label[cursor..at]);
                    value.push(' ');
                    cursor = at + 1;
                }
            }
        }
        if last {
            break;
        }
        start = end + 1;
    }
    (cursor > 0).then(|| {
        value.push_str(&label[cursor..]);
        value
    })
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
        EventKind::Start(Tag::Link(crate::stream::Target::Resource { destination, .. })) => {
            link_form(source, raw, destination)
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
