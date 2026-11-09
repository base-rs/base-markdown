// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! The shared pull-based semantic event contract.
use crate::{CodeFence, LineEnding};
use alloc::{borrow::Cow, boxed::Box, string::String};
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

    /// Give a parsed value the form the parser stores: a source slice that holds a CR or NUL is
    /// copied with its line endings as LF and each NUL as U+FFFD, and a decoded value with each NUL
    /// as U+FFFD. A CR in a decoded value comes from a character reference and stays. So a value
    /// points into the source exactly when the source there holds neither.
    pub(crate) fn normalized(self, source: &str) -> Self {
        match self {
            Self::Source(range)
                if crate::util::line_ending::needs_normalizing(&source[range.clone()]) =>
            {
                let mut value = String::with_capacity(range.len() + 2);
                crate::util::line_ending::push_normalized(&mut value, &source[range]);
                Self::Value(Cow::Owned(value))
            }
            Self::Value(value) if memchr::memchr(0, value.as_bytes()).is_some() => {
                Self::Value(Cow::Owned(value.replace('\0', "\u{FFFD}")))
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

/// The content class of a node, which decides the containers it can be a child of. See
/// [`TagKind::holds`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    /// The document root, which is never a child.
    Root,
    /// A block: a paragraph, heading, block quote, list, table, footnote definition, code block,
    /// HTML block, thematic break or definition.
    Flow,
    /// Inline content: attention, links, images, text, breaks, inline code, inline HTML and
    /// footnote references.
    Phrasing,
    /// A separator between blocks or table rows.
    Separator,
    /// A list item.
    Item,
    /// A table head or body.
    Section,
    /// A table row.
    Row,
    /// A table cell.
    Cell,
}

impl TagKind {
    pub(crate) fn class(self) -> Class {
        match self {
            Self::Document => Class::Root,
            Self::Paragraph
            | Self::Heading
            | Self::BlockQuote
            | Self::List
            | Self::Table
            | Self::FootnoteDefinition => Class::Flow,
            Self::Item => Class::Item,
            Self::Emphasis | Self::Strong | Self::Delete | Self::Link | Self::Image => {
                Class::Phrasing
            }
            Self::TableHead | Self::TableBody => Class::Section,
            Self::TableRow => Class::Row,
            Self::TableCell => Class::Cell,
        }
    }

    /// The one content model: whether a container of this kind can hold a child of class `child`,
    /// when its first child, if it has one, is of class `first`. The root holds blocks and
    /// separators, or only phrasing, for an inline document; which one is decided by its first
    /// child, so phrasing and flow never share a parent. Separators sit where the parser writes
    /// them: between blocks in a container of blocks, and between the parts and rows of a table.
    ///
    /// Some containers also restrict what can be below them at any depth, which [`Reach`]
    /// decides. That rule is on ancestors rather than the parent, so each path that adds nodes
    /// checks it alongside this one.
    pub(crate) fn holds(self, child: Class, first: Option<Class>) -> bool {
        match self.content() {
            Content::Root => match first {
                Some(Class::Phrasing) => child == Class::Phrasing,
                None => matches!(child, Class::Flow | Class::Separator | Class::Phrasing),
                Some(_) => matches!(child, Class::Flow | Class::Separator),
            },
            Content::Blocks => matches!(child, Class::Flow | Class::Separator),
            Content::Phrasing => child == Class::Phrasing,
            Content::Items => child == Class::Item,
            Content::Sections => matches!(child, Class::Section | Class::Separator),
            Content::Rows => matches!(child, Class::Row | Class::Separator),
            Content::Cells => child == Class::Cell,
        }
    }

    /// What a container of this kind holds; two kinds with the same content hold the same
    /// children.
    pub(crate) fn content(self) -> Content {
        match self {
            Self::Document => Content::Root,
            Self::BlockQuote | Self::Item | Self::FootnoteDefinition => Content::Blocks,
            Self::Paragraph
            | Self::Heading
            | Self::TableCell
            | Self::Emphasis
            | Self::Strong
            | Self::Delete
            | Self::Link
            | Self::Image => Content::Phrasing,
            Self::List => Content::Items,
            Self::Table => Content::Sections,
            Self::TableHead | Self::TableBody => Content::Rows,
            Self::TableRow => Content::Cells,
        }
    }
}

/// The containers above a node that restrict what can be below them at any depth, besides the
/// rule for their children that [`TagKind::holds`] decides: a link holds no footnote reference,
/// as a link's text cannot hold one, and a table cell holds no soft or hard break, as it is one
/// line of its row. The parser produces neither. They are counted so that checking a node takes
/// constant time where looking through its ancestors would make a deep nest quadratic.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Reach {
    links: usize,
    cells: usize,
}

impl Reach {
    /// Count a container as above the nodes that follow, until it is left.
    pub(crate) fn enter(&mut self, kind: TagKind) {
        match kind {
            TagKind::Link => self.links += 1,
            TagKind::TableCell => self.cells += 1,
            _ => {}
        }
    }

    /// Stop counting a container entered before.
    pub(crate) fn leave(&mut self, kind: TagKind) {
        match kind {
            TagKind::Link => self.links -= 1,
            TagKind::TableCell => self.cells -= 1,
            _ => {}
        }
    }

    /// Whether none of the containers counted restricts anything.
    pub(crate) fn is_empty(self) -> bool {
        self.links == 0 && self.cells == 0
    }

    /// Whether a link is counted.
    pub(crate) fn link(self) -> bool {
        self.links != 0
    }

    /// Whether a table cell is counted.
    pub(crate) fn cell(self) -> bool {
        self.cells != 0
    }

    /// Whether some container cannot have `kind` below it, so that adding it checks the
    /// containers above it.
    pub(crate) fn restricted(kind: &EventKind<'_>) -> bool {
        !Self { links: 1, cells: 1 }.allows(kind)
    }

    /// Whether every container counted can have `kind` below it.
    pub(crate) fn allows(self, kind: &EventKind<'_>) -> bool {
        (self.links == 0 || !matches!(kind, EventKind::FootnoteReference { .. }))
            && (self.cells == 0 || !matches!(kind, EventKind::SoftBreak(_) | EventKind::HardBreak))
    }
}

/// What a container holds. See [`TagKind::holds`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Content {
    /// Blocks and separators, or only phrasing.
    Root,
    /// Blocks and separators.
    Blocks,
    /// Phrasing.
    Phrasing,
    /// List items.
    Items,
    /// Table parts and separators.
    Sections,
    /// Table rows and separators.
    Rows,
    /// Table cells.
    Cells,
}

/// A container whose metadata is resolved before its start is yielded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tag<'a> {
    /// The document root.
    Document {
        /// The output line ending: LF when the source has a line ending, since parsed values
        /// store every line ending as LF, or `None` when it has none.
        line_ending: Option<LineEnding>,
        /// The body of the document's frontmatter, the lines between its fences, each with its
        /// line ending, or `None` when it has none, so a body is empty or ends in LF, and, being
        /// raw, holds no CR; any other body is rejected with [`StreamError::InvalidValue`]. A root
        /// with frontmatter holds blocks, so an inline root has none. It is omitted from HTML. See [`Constructs::frontmatter`][crate::Constructs::frontmatter].
        frontmatter: Option<Text<'a>>,
    },
    /// Whether the containing list suppresses paragraph tags.
    Paragraph { tight: bool },
    /// A heading level from one through six. Any other level is rejected with
    /// [`StreamError::InvalidValue`] wherever an event enters a document or renderer.
    Heading(u8),
    /// A block quote.
    BlockQuote,
    /// A list, its starting number (if ordered), and tightness.
    List { start: Option<u32>, tight: bool },
    /// A list item.
    Item {
        /// Whether its closing tag needs a preceding line break.
        block_end: bool,
        /// Whether it is a task item, checked or not, or `None` for an ordinary item. The checkbox
        /// is written before the item's first paragraph, which must be its first child other than
        /// definitions and separators; [`Document::normalize`][crate::Document::normalize] clears
        /// it on an item without one.
        checked: Option<bool>,
    },
    /// Emphasized inline content.
    Emphasis,
    /// Strong inline content.
    Strong,
    /// Deleted inline content.
    Delete,
    /// A link, and where it leads.
    Link(Target<'a>),
    /// An image, and where its source is.
    Image(Target<'a>),
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
    /// A footnote definition, identified by its normalized label. See
    /// [`EventKind::FootnoteReference`] for its `label`.
    FootnoteDefinition {
        identifier: Text<'a>,
        label: Option<Text<'a>>,
    },
}

/// Where a link or image leads: a destination of its own, or the definition it names.
///
/// A reference is a name, and a definition a binding of it: a reference leads where the first
/// definition of its label in document order does. A [`Document`][crate::Document] stores a
/// reference only as [`Target::Reference`], its name, and finds the binding when asked, with
/// [`Document::destination`][crate::Document::destination], so the reference follows edits to the
/// definitions. An event stream carries resolved targets only: a parser's, and a document's
/// events, give a bound reference as [`Target::Resolved`], the name with the destination and
/// title it is bound to, and an unbound one as the text it reads back as.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Target<'a> {
    /// A destination and optional title of its own, written as an inline resource, such as
    /// `[a](/u "t")`, or an autolink.
    Resource {
        destination: Text<'a>,
        title: Option<Text<'a>>,
    },
    /// A reference to a definition by its label, with no destination of its own. This is how a
    /// document stores a reference. One whose label no definition in the document has is
    /// unbound: it is valid, but not normalized, as it reads back as text, which
    /// [`Document::normalize`][crate::Document::normalize] makes it. A document's events give a
    /// bound one as [`Target::Resolved`] and an unbound one as the text it reads back as, so no
    /// stream carries this form, and [`crate::render_events`] rejects it with
    /// [`StreamError::InvalidValue`], as it leads nowhere without the definitions of a document.
    Reference(Box<Reference<'a>>),
    /// A reference with the destination and title of the definition it is bound to, as an event
    /// stream carries it. A document does not store this form, as the destination would not
    /// follow its definitions: [`AstBuilder`][crate::AstBuilder] stores the reference by name
    /// when the document's definitions bind its label to that destination and title, and
    /// otherwise a [`Target::Resource`] to them, and
    /// [`Document::set_kind`][crate::Document::set_kind] and
    /// [`Document::append_child`][crate::Document::append_child] store it as a resource to them.
    Resolved(Box<Resolved<'a>>),
}

/// A reference and the destination and title of the definition it is bound to.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Resolved<'a> {
    /// The reference as written.
    pub reference: Reference<'a>,
    /// The destination of its definition.
    pub destination: Text<'a>,
    /// The title of its definition, if any.
    pub title: Option<Text<'a>>,
}

impl<'s> Target<'s> {
    /// The reference this target was written as, if any.
    pub fn reference(&self) -> Option<&Reference<'s>> {
        match self {
            Self::Resource { .. } => None,
            Self::Reference(reference) => Some(reference),
            Self::Resolved(resolved) => Some(&resolved.reference),
        }
    }

    /// The destination and title this target carries: its own for a resource, its definition's
    /// once resolved, and none for a reference by name.
    pub fn destination(&self) -> Option<(&Text<'s>, Option<&Text<'s>>)> {
        match self {
            Self::Resource { destination, title } => Some((destination, title.as_ref())),
            Self::Reference(_) => None,
            Self::Resolved(resolved) => Some((&resolved.destination, resolved.title.as_ref())),
        }
    }

    /// This target with a resolved reference as a resource to where it leads, which keeps its
    /// meaning whatever the definitions of the document it is put in.
    pub(crate) fn resource(self) -> Self {
        match self {
            Self::Resolved(resolved) => Self::Resource {
                destination: resolved.destination,
                title: resolved.title,
            },
            target => target,
        }
    }

    /// Borrow the values of this target, which a document stores.
    pub(crate) fn borrowed(&self) -> Target<'_> {
        if let Self::Resource { destination, title } = self {
            return Target::Resource {
                destination: destination.borrowed(),
                title: title.as_ref().map(Text::borrowed),
            };
        }
        let Self::Reference(reference) = self else {
            unreachable!("a document stores no resolved reference")
        };
        Target::Reference(Box::new(reference.borrowed()))
    }

    pub(crate) fn map_text<'a>(self, mut map: impl FnMut(Text<'s>) -> Text<'a>) -> Target<'a> {
        match self {
            Self::Resource { destination, title } => Target::Resource {
                destination: map(destination),
                title: title.map(&mut map),
            },
            Self::Reference(reference) => Target::Reference(Box::new(reference.map_text(map))),
            Self::Resolved(resolved) => {
                let Resolved {
                    reference,
                    destination,
                    title,
                } = *resolved;
                Target::Resolved(Box::new(Resolved {
                    reference: reference.map_text(&mut map),
                    destination: map(destination),
                    title: title.map(&mut map),
                }))
            }
        }
    }
}

/// How a link or image was written as a reference to a definition: its form and the label the
/// parser read.
///
/// The label is the raw text between its brackets, as authored, with escapes, but without the
/// prefixes of the lines it continues on, such as a block quote's `>`, which the parser does not
/// read as part of it, and with LF line endings like every value. For a shortcut or collapsed
/// reference it is the text of the link. The Markdown serializer writes a reference as a
/// reference.
///
/// The serializer writes a label as it is, so it must be one the parser can read with some
/// options: it holds something other than whitespace, no carriage return, no blank line, no
/// bracket without a backslash before it, and does not end in a backslash that would escape the
/// closing bracket. A line after the first may look like the start of a block, such as `# b`, as
/// a parser with headings turned off reads it as part of the label; `Document::normalize` indents
/// such a line so the default options read it back too. A full
/// reference's label is also at most 999 characters, counting a tab as the columns it spans, but
/// not line endings and the spaces and tabs after them; the parser reads a shortcut or collapsed
/// one at any size. A label that is not is rejected with [`StreamError::InvalidValue`] wherever
/// an event enters a document or renderer. A shortcut or collapsed reference whose text no
/// longer matches its label is written as a full reference, `[text][label]`, normalized or not,
/// which reads back as a full reference rather than the kind it was, or as a resource when its
/// label is longer than a full reference's can be.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Reference<'a> {
    /// Whether the reference was full, collapsed or a shortcut.
    pub kind: crate::ReferenceKind,
    /// The label as authored.
    pub label: Text<'a>,
}

impl<'s> Reference<'s> {
    pub(crate) fn map_text<'a>(self, map: impl FnOnce(Text<'s>) -> Text<'a>) -> Reference<'a> {
        Reference {
            kind: self.kind,
            label: map(self.label),
        }
    }

    pub(crate) fn borrowed(&self) -> Reference<'_> {
        Reference {
            kind: self.kind,
            label: self.label.borrowed(),
        }
    }
}

impl<'s> Tag<'s> {
    /// The class that decides what this container holds besides its first child, `first`: a root
    /// with frontmatter holds blocks, as frontmatter is a block before its first child, so it
    /// counts as a first child of blocks. See [`TagKind::holds`].
    pub(crate) fn first_class(&self, first: Option<Class>) -> Option<Class> {
        match self {
            Self::Document {
                frontmatter: Some(_),
                ..
            } => Some(Class::Flow),
            _ => first,
        }
    }
    /// Identify the matching end event.
    pub fn kind(&self) -> TagKind {
        match self {
            Self::Document { .. } => TagKind::Document,
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
            Self::FootnoteDefinition { .. } => TagKind::FootnoteDefinition,
        }
    }

    pub(crate) fn map_text<'a>(self, mut map: impl FnMut(Text<'s>) -> Text<'a>) -> Tag<'a> {
        match self {
            Self::Document {
                line_ending,
                frontmatter,
            } => Tag::Document {
                line_ending,
                frontmatter: frontmatter.map(&mut map),
            },
            Self::Paragraph { tight } => Tag::Paragraph { tight },
            Self::Heading(value) => Tag::Heading(value),
            Self::BlockQuote => Tag::BlockQuote,
            Self::List { start, tight } => Tag::List { start, tight },
            Self::Item { block_end, checked } => Tag::Item { block_end, checked },
            Self::Emphasis => Tag::Emphasis,
            Self::Strong => Tag::Strong,
            Self::Delete => Tag::Delete,
            Self::Link(target) => Tag::Link(target.map_text(map)),
            Self::Image(target) => Tag::Image(target.map_text(map)),
            Self::Table => Tag::Table,
            Self::TableHead => Tag::TableHead,
            Self::TableBody => Tag::TableBody,
            Self::TableRow => Tag::TableRow,
            Self::TableCell { alignment, header } => Tag::TableCell { alignment, header },
            Self::FootnoteDefinition { identifier, label } => Tag::FootnoteDefinition {
                identifier: map(identifier),
                label: label.map(&mut map),
            },
        }
    }
}

/// A resolved leaf, or a start/end of a semantic container.
///
/// Each value has the form the parser stores, and only that form is valid, so that normalizing
/// never rewrites a character and the writer only encodes ([the contract](crate::contract),
/// clause 1.2). No value holds NUL, which the parser stores as U+FFFD. A decoded value, text, a
/// destination, a title or an info string, holds any other character, CR included, which the
/// writer encodes as `&#xD;`; a raw value, which is written as it is, holds no CR, which the
/// parser stores as LF. A value that breaks a rule is rejected with
/// [`StreamError::InvalidValue`] wherever an event enters a document or renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EventKind<'a> {
    /// Open a container.
    Start(Tag<'a>),
    /// Close the matching container.
    End(TagKind),
    /// Ordinary decoded text, which holds anything but NUL.
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
    ///
    /// The value is raw, so it holds no CR, and no LF either, as the parser reads each line ending
    /// in a code span as a space. It may be empty, which is valid but not normalized, as a run of
    /// backticks with nothing between them is not code: normalizing removes it. Any other value can
    /// be written, as the writer pads one whose edge spaces the parser would strip.
    ///
    /// The language is what [`ParseOptions::inline_code_info`][crate::ParseOptions::inline_code_info]
    /// reads before the span: a nonempty ASCII token whose first character is a letter or digit
    /// and whose other characters are letters, digits, `_`, `-`, `+`, `#` or `.`. Any other
    /// language, including an empty one, cannot be written and is rejected with
    /// [`StreamError::InvalidValue`] wherever an event enters a document or renderer.
    Code {
        value: Text<'a>,
        language: Option<Text<'a>>,
    },
    /// A code block, including source fence provenance where available.
    ///
    /// The value is raw, so it holds no CR. One that is not empty ends in a line ending, as the
    /// parser stores each line with its own, except in a block that ends the document, which the
    /// parser gives without one for an unclosed fence or indented code at the end of the source;
    /// normalizing adds one to a value without it anywhere else.
    ///
    /// `info` is the whole info string of a fenced block, as the parser decoded it: trimmed of the
    /// spaces and tabs at its edges, with backslash escapes and character references decoded when
    /// [`Constructs::character_escape`][crate::Constructs::character_escape] and
    /// [`Constructs::character_reference`][crate::Constructs::character_reference] are on. A fence
    /// without an info string, and indented code, have none. The language is derived from it with
    /// [`code_language`]: its first word.
    ///
    /// Any info string can be written, since the serializer encodes the spaces and tabs at its
    /// edges, line endings, and the characters that would decode or close the fence, so it reads
    /// back exactly. An empty info string is code without one, which is `None`, so `Some` empty
    /// info is rejected with [`StreamError::InvalidValue`] wherever an event enters a document or
    /// renderer.
    CodeBlock {
        value: Text<'a>,
        info: Option<Text<'a>>,
        fence: Option<CodeFence>,
        unclosed: bool,
    },
    /// Raw HTML, an HTML block or inline HTML.
    ///
    /// Inline HTML, with `block` false, is written as it is, so its value is one inline HTML
    /// construct as the parser reads it: an open tag, a closing tag, a comment, a processing
    /// instruction, a declaration or CDATA, with nothing before or after it, as `<b>`,
    /// `<a\nhref="x">` or `<!-- c -->`. Tag names may have namespace prefixes, as
    /// [`ParseOptions::html_tag_namespaces`][crate::ParseOptions::html_tag_namespaces] reads them,
    /// and it holds no blank line, which would end its paragraph. An empty value is valid and
    /// writes nothing; normalizing removes it. Any other value, such as text, `<b>x</b>`, `<b> `,
    /// `<a\n\nb>`, or `<a\n  b>`, whose continuation line a paragraph would strip, is rejected
    /// with [`StreamError::InvalidValue`] wherever an event enters a document or renderer.
    ///
    /// An HTML block, with `block` true, is one block as the block parser reads it, by its start
    /// and end conditions: the indentation the parser keeps before a block, at most three spaces,
    /// or one to three spaces then a tab and spaces where the tab is at least one column wide,
    /// then HTML that starts a block, and the lines the block takes, without the final line
    /// ending, as `<div>`, `<div>\na` or `<!-- a\n\nb -->`. A block of kinds 1–5 holds no line
    /// after the one that holds its end condition; one that no line closes may end in the blank
    /// lines it takes, as `<!-- a\n\n` holds `<!-- a\n`. One of kind 6 or 7 holds no blank line,
    /// which ends it; but blank lines before a line indented at least as far as the least indented
    /// line before it, and a final line ending, are valid in one of kind 6 or 7 too, as
    /// [`ParseOptions::html_block_blank_lines`][crate::ParseOptions::html_block_blank_lines]
    /// keeps them. So `abc`, `<b> x`, `    <div>`, `<div>\n\nb` and `<div>\n    a\n\n  b` are
    /// rejected, as is an empty value, which no block is.
    ///
    /// Both are raw, so they hold no CR.
    Html { value: Text<'a>, block: bool },
    /// A resolved definition, retained in the AST but omitted from HTML.
    ///
    /// Its label follows the rules of a full [`Reference`]'s, and one that does not is rejected
    /// with [`StreamError::InvalidValue`] wherever an event enters a document or renderer. Its
    /// destination and title are decoded, so they hold anything but NUL.
    Definition {
        label: Text<'a>,
        destination: Text<'a>,
        title: Option<Text<'a>>,
    },
    /// A call to a footnote definition, by its normalized label, `identifier`, and the label as
    /// authored, which the serializer writes. The parser always reads a label; one built without
    /// it is written as its identifier.
    ///
    /// A footnote label is written as it is, so it follows the rules of a full [`Reference`]'s,
    /// and a definition's label also holds no whitespace. A label, or an identifier without one,
    /// that does not is rejected with [`StreamError::InvalidValue`], as is an identifier that is
    /// not the normalized form of its label, or of itself without one, wherever an event enters a
    /// document or renderer.
    FootnoteReference {
        identifier: Text<'a>,
        label: Option<Text<'a>>,
    },
}

impl<'s> EventKind<'s> {
    pub(crate) fn validate(&self, source: Option<&str>) -> Result<(), StreamError> {
        // A raw value, which the writer writes as it is, holds no CR, which the parser reads as a
        // line ending and stores as LF.
        fn raw(value: &str) -> bool {
            memchr::memchr(b'\r', value.as_bytes()).is_none()
        }
        // A value, unless it is source-backed and its source is not known yet, though an empty
        // range is empty in any source.
        fn resolved<'v>(
            value: &'v Text<'_>,
            source: Option<&'v str>,
        ) -> Result<Option<&'v str>, StreamError> {
            match (value, source) {
                (Text::Source(range), None) if range.is_empty() => Ok(Some("")),
                (Text::Source(_), None) => Ok(None),
                (value, source) => value.resolve(source.unwrap_or("")).map(Some),
            }
        }
        // A value is valid only in the form the parser stores, so no value holds NUL, which it
        // stores as U+FFFD, and each kind has its own rule beyond that. A source-backed value is
        // checked once its source is known.
        let check = |value: &Text<'_>, valid: &dyn Fn(&str) -> bool| match resolved(value, source)?
        {
            Some(value) if memchr::memchr(0, value.as_bytes()).is_some() || !valid(value) => {
                Err(StreamError::InvalidValue)
            }
            _ => Ok(()),
        };
        // A decoded value, which the writer encodes, holds any other character, CR included, as
        // the parser decodes `&#xD;` to one.
        let text = |value: &Text<'_>| check(value, &|_| true);
        let optional = |value: &Option<Text<'_>>| match value {
            Some(value) => text(value),
            None => Ok(()),
        };
        // A footnote's label, or its identifier without one, is written, and its identifier is
        // what the label normalizes to, as the parser gives it.
        let footnote =
            |identifier: &Text<'s>, label: Option<&Text<'s>>, valid: &dyn Fn(&str) -> bool| {
                let written = label.unwrap_or(identifier);
                check(written, valid)?;
                match (resolved(identifier, source)?, resolved(written, source)?) {
                    (Some(identifier), Some(written))
                        if crate::util::normalize_identifier::normalize_identifier(written)
                            != identifier =>
                    {
                        Err(StreamError::InvalidValue)
                    }
                    _ => Ok(()),
                }
            };
        match self {
            Self::Start(Tag::Heading(level)) if !(1..=6).contains(level) => {
                Err(StreamError::InvalidValue)
            }
            Self::Start(Tag::Link(target) | Tag::Image(target)) => {
                if let Some((destination, title)) = target.destination() {
                    text(destination)?;
                    if let Some(title) = title {
                        text(title)?;
                    }
                }
                match target.reference() {
                    Some(reference) => check(&reference.label, &|label| {
                        crate::node_syntax::valid_label(
                            label,
                            reference.kind == crate::ReferenceKind::Full,
                        )
                    }),
                    None => Ok(()),
                }
            }
            Self::Start(Tag::FootnoteDefinition { identifier, label }) => footnote(
                identifier,
                label.as_ref(),
                &crate::node_syntax::valid_footnote_definition_label,
            ),
            Self::FootnoteReference { identifier, label } => {
                footnote(identifier, label.as_ref(), &|label| {
                    crate::node_syntax::valid_label(label, true)
                })
            }
            // The parser reads each line ending in inline code as a space.
            Self::Code { value, language } => {
                check(value, &|value| {
                    raw(value) && memchr::memchr(b'\n', value.as_bytes()).is_none()
                })?;
                match language {
                    Some(language) => check(language, &crate::util::inline_code_info::is_valid),
                    None => Ok(()),
                }
            }
            // An empty info string is code without one, which is `None`.
            Self::CodeBlock { value, info, .. } => {
                check(value, &raw)?;
                match info {
                    Some(info) => check(info, &|info| !info.is_empty()),
                    None => Ok(()),
                }
            }
            // HTML is written as it is, so it reads back as itself only when it is one construct
            // the parser reads: inline HTML is one the inline parser reads, or empty, which writes
            // nothing and which normalizing removes, and an HTML block one block the block parser
            // reads.
            Self::Html { value, block } => check(value, &|value| {
                raw(value)
                    && if *block {
                        crate::block_parser::html_block_value(value)
                    } else {
                        crate::util::html_text::HtmlText::is_inline(value)
                    }
            }),
            // The parser stores each line of frontmatter with its line ending.
            Self::Start(Tag::Document {
                frontmatter: Some(value),
                ..
            }) => check(value, &|value| {
                raw(value) && (value.is_empty() || value.ends_with('\n'))
            }),
            Self::Text(value) | Self::SoftBreak(value) | Self::Separator { value, .. } => {
                text(value)
            }
            Self::Definition {
                label,
                destination,
                title,
            } => {
                text(destination)?;
                optional(title)?;
                check(label, &|label| crate::node_syntax::valid_label(label, true))
            }
            _ => Ok(()),
        }
    }

    /// The content class of this node, which is not an end event. See [`TagKind::holds`].
    // The wildcard is the end event, which no caller passes.
    #[allow(clippy::match_wildcard_for_single_variants)]
    pub(crate) fn class(&self) -> Class {
        match self {
            Self::Start(tag) => tag.kind().class(),
            Self::Separator { .. } => Class::Separator,
            Self::Rule
            | Self::CodeBlock { .. }
            | Self::Definition { .. }
            | Self::Html { block: true, .. } => Class::Flow,
            Self::Text(_)
            | Self::SoftBreak(_)
            | Self::HardBreak
            | Self::Code { .. }
            | Self::Html { block: false, .. }
            | Self::FootnoteReference { .. } => Class::Phrasing,
            // An end event is not a node.
            _ => unreachable!("an end event is not a node"),
        }
    }

    /// Whether this node can be a child of `parent` whose first child, if it has one, is `first`.
    /// See [`TagKind::holds`] and [`Tag::first_class`].
    pub(crate) fn nests_in(&self, parent: &Tag<'_>, first: Option<&EventKind<'_>>) -> bool {
        parent.kind().holds(
            self.class(),
            parent.first_class(first.map(EventKind::class)),
        )
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
                info,
                fence,
                unclosed,
            } => EventKind::CodeBlock {
                value: map(value),
                info: info.map(map),
                fence,
                unclosed,
            },
            Self::Html { value, block } => EventKind::Html {
                value: map(value),
                block,
            },
            Self::Definition {
                label,
                destination,
                title,
            } => EventKind::Definition {
                label: map(label),
                destination: map(destination),
                title: title.map(map),
            },
            Self::FootnoteReference { identifier, label } => EventKind::FootnoteReference {
                identifier: map(identifier),
                label: label.map(&mut map),
            },
        }
    }
}

/// The language of a code block whose info string is `info`: its first word, up to the first
/// space, tab or line ending, or `None` when that word is empty.
///
/// The info string is decoded, so a space written as a character reference separates words like
/// a raw one, as in cmark. The parser trims the spaces and tabs at the edges of the info string,
/// so the language of a parsed block is empty only when its info string starts with an encoded
/// space, tab or line ending, such as `&#x20;x`.
///
/// ```
/// use base_markdown::stream::code_language;
///
/// assert_eq!(code_language("rust ignore"), Some("rust"));
/// assert_eq!(code_language("a\tb"), Some("a"));
/// assert_eq!(code_language(" x"), None);
/// ```
pub fn code_language(info: &str) -> Option<&str> {
    let end = info.find([' ', '\t', '\r', '\n']).unwrap_or(info.len());
    (end != 0).then(|| &info[..end])
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
    /// Start and end events do not form a balanced document, or an operation needs a node that
    /// is not where it expects, such as a container to append to.
    Unbalanced,
    /// A node cannot be where it was put: a child that its parent's content model does not hold,
    /// such as a block inside a paragraph, an item outside a list or a row directly inside a
    /// table, phrasing and blocks under one parent, or a footnote reference inside a link.
    InvalidNesting,
    /// A value has a form its event or syntax cannot hold, such as a heading level outside one
    /// through six, an inline code language that
    /// [`ParseOptions::inline_code_info`][crate::ParseOptions::inline_code_info] would not read
    /// back, or an empty code block info string.
    InvalidValue,
    /// The caller's output writer failed.
    Write(fmt::Error),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            Self::InvalidRange => f.write_str("invalid source range"),
            Self::Unbalanced => f.write_str("unbalanced semantic event stream"),
            Self::InvalidNesting => f.write_str("invalid nesting"),
            Self::InvalidValue => f.write_str("invalid event value"),
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
