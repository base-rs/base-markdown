// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The shared pull-based semantic event contract.
use crate::{CodeFence, LineEnding};
use alloc::{borrow::Cow, string::String};
use core::{fmt, ops::Range};

/// A semantic value, independent of the lifetime of parser scratch storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Text<'a> {
    /// Unchanged UTF-8 bytes in the document's immutable source buffer.
    Source(Range<usize>),
    /// A decoded constant requiring no owned allocation.
    Literal(&'static str),
    /// A constructed or edited value.
    Value(Cow<'a, str>),
}

#[cfg(feature = "serde")]
impl serde::Serialize for Text<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Source(range) => serializer.serialize_newtype_variant("Text", 0, "Source", range),
            Self::Literal(value) => serializer.serialize_newtype_variant("Text", 1, "Value", value),
            Self::Value(value) => serializer.serialize_newtype_variant("Text", 1, "Value", value),
        }
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Text<'_> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        enum Stored {
            Source(Range<usize>),
            Value(String),
            Literal(String),
        }
        Ok(match Stored::deserialize(deserializer)? {
            Stored::Source(range) => Self::Source(range),
            Stored::Value(value) | Stored::Literal(value) => Self::Value(Cow::Owned(value)),
        })
    }
}

impl Text<'_> {
    /// Resolve this value against its source document.
    ///
    /// # Errors
    /// Returns [`StreamError::InvalidRange`] for invalid UTF-8 boundaries or out-of-bounds ranges.
    pub fn resolve<'a>(&'a self, source: &'a str) -> Result<&'a str, StreamError> {
        match self {
            Self::Source(range) => source.get(range.clone()).ok_or(StreamError::InvalidRange),
            Self::Literal(value) => Ok(value),
            Self::Value(value) => Ok(value),
        }
    }

    pub(crate) fn borrowed(&self) -> Text<'_> {
        match self {
            Self::Source(range) => Text::Source(range.clone()),
            Self::Literal(value) => Text::Literal(value),
            Self::Value(value) => Text::Value(Cow::Borrowed(value)),
        }
    }

    pub(crate) fn to_mut(&mut self, source: &str) -> Result<&mut String, StreamError> {
        if !matches!(self, Self::Value(_)) {
            *self = Self::Value(Cow::Owned(self.resolve(source)?.into()));
        }
        match self {
            Self::Value(value) => Ok(value.to_mut()),
            _ => unreachable!("source and literal text were materialized above"),
        }
    }

    pub(crate) fn materialized(self, source: &str) -> Text<'static> {
        match self {
            Self::Source(range) => Text::from(String::from(
                source.get(range).expect("document ranges are validated"),
            )),
            Self::Literal(value) => Text::Literal(value),
            Self::Value(value) => Text::Value(Cow::Owned(value.into_owned())),
        }
    }

    /// Write the source line endings of a source slice as LF. Other values are kept: a CR in a
    /// decoded value comes from a character reference, and a constructed one is the caller's.
    pub(crate) fn normalized(self, source: &str) -> Self {
        match self {
            Self::Source(range)
                if memchr::memchr(b'\r', &source.as_bytes()[range.clone()]).is_some() =>
            {
                let mut value = String::with_capacity(range.len());
                crate::util::line_ending::push_normalized(&mut value, &source[range]);
                Self::Value(Cow::Owned(value))
            }
            text => text,
        }
    }

    pub(crate) fn owned(self) -> Text<'static> {
        match self {
            Self::Source(range) => Text::Source(range),
            Self::Literal(value) => Text::Literal(value),
            Self::Value(value) => Text::Value(Cow::Owned(value.into_owned())),
        }
    }
}

impl From<String> for Text<'static> {
    fn from(value: String) -> Self {
        Self::Value(Cow::Owned(value))
    }
}

/// The identity of a semantic container, used to balance start/end events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TagKind {
    /// The document root.
    Document,
    /// A paragraph.
    Paragraph,
    /// A heading.
    Heading,
    /// A block quote.
    BlockQuote,
    /// An ordered or unordered list.
    List,
    /// A list item.
    Item,
    /// Emphasized inline content.
    Emphasis,
    /// Strong inline content.
    Strong,
    /// Deleted inline content.
    Delete,
    /// A link with a resolved destination.
    Link,
    /// An image with inline alternative text.
    Image,
    /// A table.
    Table,
    /// A table head.
    TableHead,
    /// A table body.
    TableBody,
    /// A table row.
    TableRow,
    /// A table cell.
    TableCell,
    /// A footnote definition.
    FootnoteDefinition,
}

impl TagKind {
    // Capture containers cannot occur inside heading capture or image alt text.
    // Ordinary inline nesting remains unrestricted by the rendering contract.
    pub(crate) fn allows_descendant(self, child: Self) -> bool {
        !matches!(self, Self::Heading | Self::Image)
            || !matches!(child, Self::Heading | Self::FootnoteDefinition)
    }
}

/// A container whose metadata is resolved before its start is yielded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tag<'a> {
    /// The preferred output line ending, inferred from parsed block structure.
    Document(Option<LineEnding>),
    /// Whether the containing list suppresses paragraph tags.
    Paragraph { tight: bool },
    /// A heading level from one through six.
    Heading(u8),
    /// A block quote.
    BlockQuote,
    /// A list, its starting number (if ordered), and tightness.
    List { start: Option<u32>, tight: bool },
    /// A list item; whether its closing tag needs a preceding line break.
    Item { block_end: bool },
    /// Emphasized inline content.
    Emphasis,
    /// Strong inline content.
    Strong,
    /// Deleted inline content.
    Delete,
    /// Resolved link destination and optional title.
    Link {
        destination: Text<'a>,
        title: Option<Text<'a>>,
    },
    /// Resolved image destination and optional title.
    Image {
        destination: Text<'a>,
        title: Option<Text<'a>>,
    },
    /// A table.
    Table,
    /// A table head.
    TableHead,
    /// A table body.
    TableBody,
    /// A table row.
    TableRow,
    /// A cell with its own resolved alignment and header status.
    TableCell {
        alignment: crate::AlignKind,
        header: bool,
    },
    /// A footnote definition, identified by its normalized label.
    FootnoteDefinition(Text<'a>),
}

impl<'s> Tag<'s> {
    /// Identify the matching end event.
    pub fn kind(&self) -> TagKind {
        match self {
            Self::Document(_) => TagKind::Document,
            Self::Paragraph { .. } => TagKind::Paragraph,
            Self::Heading(_) => TagKind::Heading,
            Self::BlockQuote => TagKind::BlockQuote,
            Self::List { .. } => TagKind::List,
            Self::Item { .. } => TagKind::Item,
            Self::Emphasis => TagKind::Emphasis,
            Self::Strong => TagKind::Strong,
            Self::Delete => TagKind::Delete,
            Self::Link { .. } => TagKind::Link,
            Self::Image { .. } => TagKind::Image,
            Self::Table => TagKind::Table,
            Self::TableHead => TagKind::TableHead,
            Self::TableBody => TagKind::TableBody,
            Self::TableRow => TagKind::TableRow,
            Self::TableCell { .. } => TagKind::TableCell,
            Self::FootnoteDefinition(_) => TagKind::FootnoteDefinition,
        }
    }

    pub(crate) fn map_text<'a>(self, mut map: impl FnMut(Text<'s>) -> Text<'a>) -> Tag<'a> {
        match self {
            Self::Document(value) => Tag::Document(value),
            Self::Paragraph { tight } => Tag::Paragraph { tight },
            Self::Heading(value) => Tag::Heading(value),
            Self::BlockQuote => Tag::BlockQuote,
            Self::List { start, tight } => Tag::List { start, tight },
            Self::Item { block_end } => Tag::Item { block_end },
            Self::Emphasis => Tag::Emphasis,
            Self::Strong => Tag::Strong,
            Self::Delete => Tag::Delete,
            Self::Link { destination, title } => Tag::Link {
                destination: map(destination),
                title: title.map(&mut map),
            },
            Self::Image { destination, title } => Tag::Image {
                destination: map(destination),
                title: title.map(&mut map),
            },
            Self::Table => Tag::Table,
            Self::TableHead => Tag::TableHead,
            Self::TableBody => Tag::TableBody,
            Self::TableRow => Tag::TableRow,
            Self::TableCell { alignment, header } => Tag::TableCell { alignment, header },
            Self::FootnoteDefinition(value) => Tag::FootnoteDefinition(map(value)),
        }
    }
}

/// A resolved leaf, or a start/end of a semantic container.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventKind<'a> {
    /// Open a container.
    Start(Tag<'a>),
    /// Close the matching container.
    End(TagKind),
    /// Ordinary decoded text.
    Text(Text<'a>),
    /// A soft source line break within inline content.
    SoftBreak(Text<'a>),
    /// A block separator; conditional separators only terminate nonempty output.
    Separator { value: Text<'a>, conditional: bool },
    /// An explicit hard break.
    HardBreak,
    /// A thematic break.
    Rule,
    /// An inline code span; optional language metadata is an extension.
    Code {
        value: Text<'a>,
        language: Option<Text<'a>>,
    },
    /// A code/math block, including source fence provenance where available.
    CodeBlock {
        value: Text<'a>,
        language: Option<Text<'a>>,
        fence: Option<CodeFence>,
        math: bool,
        unclosed: bool,
    },
    /// Inline mathematics.
    Math(Text<'a>),
    /// Raw HTML.
    Html { value: Text<'a>, block: bool },
    /// A task-list marker.
    TaskMarker(bool),
    /// A resolved definition, retained in the AST but omitted from HTML.
    Definition {
        label: Text<'a>,
        destination: Text<'a>,
        title: Option<Text<'a>>,
    },
    /// Frontmatter retained verbatim and omitted from HTML.
    Opaque { value: Text<'a>, block: bool },
    /// A call to a footnote definition.
    FootnoteReference(Text<'a>),
}

impl<'s> EventKind<'s> {
    pub(crate) fn validate(&self, source: Option<&str>) -> Result<(), StreamError> {
        let text = |value: &Text<'_>| {
            if let Some(source) = source {
                value.resolve(source)?;
            }
            Ok(())
        };
        let optional = |value: &Option<Text<'_>>| {
            if let Some(value) = value {
                text(value)?;
            }
            Ok(())
        };
        match self {
            Self::Start(Tag::Heading(level)) if !(1..=6).contains(level) => {
                Err(StreamError::Unbalanced)
            }
            Self::Start(Tag::Link { destination, title } | Tag::Image { destination, title }) => {
                text(destination)?;
                optional(title)
            }
            Self::Start(Tag::FootnoteDefinition(value))
            | Self::Text(value)
            | Self::SoftBreak(value)
            | Self::Math(value)
            | Self::Separator { value, .. }
            | Self::Html { value, .. }
            | Self::Opaque { value, .. }
            | Self::FootnoteReference(value) => text(value),
            Self::Code { value, language }
            | Self::CodeBlock {
                value, language, ..
            } => {
                text(value)?;
                optional(language)
            }
            Self::Definition {
                label,
                destination,
                title,
            } => {
                text(label)?;
                text(destination)?;
                optional(title)
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn map_text<'a>(self, mut map: impl FnMut(Text<'s>) -> Text<'a>) -> EventKind<'a> {
        match self {
            Self::Start(tag) => EventKind::Start(tag.map_text(map)),
            Self::End(kind) => EventKind::End(kind),
            Self::Text(text) => EventKind::Text(map(text)),
            Self::SoftBreak(text) => EventKind::SoftBreak(map(text)),
            Self::Separator { value, conditional } => EventKind::Separator {
                value: map(value),
                conditional,
            },
            Self::HardBreak => EventKind::HardBreak,
            Self::Rule => EventKind::Rule,
            Self::Code { value, language } => EventKind::Code {
                value: map(value),
                language: language.map(map),
            },
            Self::CodeBlock {
                value,
                language,
                fence,
                math,
                unclosed,
            } => EventKind::CodeBlock {
                value: map(value),
                language: language.map(map),
                fence,
                math,
                unclosed,
            },
            Self::Math(value) => EventKind::Math(map(value)),
            Self::Html { value, block } => EventKind::Html {
                value: map(value),
                block,
            },
            Self::TaskMarker(value) => EventKind::TaskMarker(value),
            Self::Definition {
                label,
                destination,
                title,
            } => EventKind::Definition {
                label: map(label),
                destination: map(destination),
                title: title.map(map),
            },
            Self::Opaque { value, block } => EventKind::Opaque {
                value: map(value),
                block,
            },
            Self::FootnoteReference(value) => EventKind::FootnoteReference(map(value)),
        }
    }
}

/// A semantic event and its original source extent.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event<'a> {
    /// Resolved semantic meaning.
    pub kind: EventKind<'a>,
    /// Original byte extent, independent of transformed or edited text.
    pub span: Range<usize>,
}

impl Event<'_> {
    pub(crate) fn validate(&self, source: Option<&str>) -> Result<(), StreamError> {
        if self.span.start > self.span.end
            || (matches!(self.kind, EventKind::End(_)) && !self.span.is_empty())
            || source.is_some_and(|source| source.get(self.span.clone()).is_none())
        {
            return Err(StreamError::InvalidRange);
        }
        self.kind.validate(source)
    }
}

/// An error while parsing, consuming, or writing a semantic stream.
#[derive(Debug)]
pub enum StreamError {
    /// A custom event producer reported a diagnostic.
    Parse(crate::message::Message),
    /// A source range is out of bounds or splits a UTF-8 character.
    InvalidRange,
    /// Start and end events do not form a balanced document.
    Unbalanced,
    /// The caller's output writer failed.
    Write(fmt::Error),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            Self::InvalidRange => f.write_str("invalid source range"),
            Self::Unbalanced => f.write_str("unbalanced semantic event stream"),
            Self::Write(error) => write!(f, "{error}"),
        }
    }
}

impl From<fmt::Error> for StreamError {
    fn from(error: fmt::Error) -> Self {
        Self::Write(error)
    }
}

impl From<crate::message::Message> for StreamError {
    fn from(error: crate::message::Message) -> Self {
        Self::Parse(error)
    }
}
