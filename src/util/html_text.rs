// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Byte recognition for raw inline HTML and complete HTML block tags.
//! Input starts immediately after `<`. Grammar callers supply normalized bytes;
//! compact callers stop at physical line boundaries and retain source ranges.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum HtmlText {
    #[default]
    Open,
    DeclarationOpen,
    CommentOpen,
    Comment,
    CommentDash,
    CommentEnd,
    CdataOpen(u8),
    Cdata,
    CdataBracket,
    CdataEnd,
    Declaration,
    Instruction,
    InstructionQuestion,
    CloseStart,
    CloseName,
    CloseSpace,
    Name,
    Between,
    Attribute,
    AfterAttribute,
    BeforeValue,
    Quoted(u8),
    AfterQuoted,
    Unquoted,
    End,
}

impl HtmlText {
    /// The length of the inline HTML construct at the start of `bytes`, which start immediately
    /// after `<`, or `None` when they do not start with one. A carriage return reads as a line
    /// feed, so a `\r\n` is a line ending as the compact parser sees it.
    #[inline]
    pub fn scan(bytes: &[u8], namespaces: bool) -> Option<usize> {
        let mut html = Self::default();
        for (offset, &byte) in bytes.iter().enumerate() {
            let byte = if byte == b'\r' { b'\n' } else { byte };
            if html.advance(byte, namespaces)? {
                return Some(offset + 1);
            }
        }
        None
    }

    /// Whether `value`, which holds no CR, is a value of inline HTML that reads back as itself:
    /// empty, which writes nothing, or exactly one inline HTML construct as the inline parser
    /// reads it, an open tag, a closing tag, a comment, a processing instruction, a declaration
    /// or CDATA, with nothing after it, in the form the parser stores. Namespace prefixes in tag
    /// names are allowed when `namespaces`, as `ParseOptions::html_tag_namespaces` reads them.
    pub fn is_inline(value: &str, namespaces: bool) -> bool {
        let bytes = value.as_bytes();
        match bytes.split_first() {
            None => true,
            Some((b'<', rest)) => {
                Self::scan(rest, namespaces) == Some(rest.len()) && continuation_lines_start(bytes)
            }
            _ => false,
        }
    }

    /// Consume one byte: `Some(true)` completes HTML, `None` rejects it.
    pub fn advance(&mut self, byte: u8, namespaces: bool) -> Option<bool> {
        use HtmlText::{
            AfterAttribute, AfterQuoted, Attribute, BeforeValue, Between, Cdata, CdataBracket,
            CdataEnd, CdataOpen, CloseName, CloseSpace, CloseStart, Comment, CommentDash,
            CommentEnd, CommentOpen, Declaration, DeclarationOpen, End, Instruction,
            InstructionQuestion, Name, Open, Quoted, Unquoted,
        };
        loop {
            *self = match *self {
                Open => match byte {
                    b'!' => DeclarationOpen,
                    b'/' => CloseStart,
                    b'?' => Instruction,
                    b'A'..=b'Z' | b'a'..=b'z' => Name,
                    _ => return None,
                },
                DeclarationOpen => match byte {
                    b'-' => CommentOpen,
                    b'[' => CdataOpen(0),
                    b'A'..=b'Z' | b'a'..=b'z' => Declaration,
                    _ => return None,
                },
                CommentOpen => {
                    if byte != b'-' {
                        return None;
                    }
                    CommentEnd
                }
                Comment | CommentDash | CommentEnd => match byte {
                    b'>' if matches!(self, CommentEnd) => return Some(true),
                    b'-' if matches!(self, CommentDash | CommentEnd) => CommentEnd,
                    b'-' => CommentDash,
                    _ => Comment,
                },
                CdataOpen(index) => {
                    if byte != b"CDATA["[usize::from(index)] {
                        return None;
                    }
                    if index == 5 {
                        Cdata
                    } else {
                        CdataOpen(index + 1)
                    }
                }
                Cdata | CdataBracket | CdataEnd => match byte {
                    b'>' if matches!(self, CdataEnd) => return Some(true),
                    b']' if matches!(self, CdataBracket | CdataEnd) => CdataEnd,
                    b']' => CdataBracket,
                    _ => Cdata,
                },
                Declaration => {
                    if byte == b'>' {
                        return Some(true);
                    }
                    Declaration
                }
                Instruction | InstructionQuestion => match byte {
                    b'>' if matches!(self, InstructionQuestion) => return Some(true),
                    b'?' => InstructionQuestion,
                    _ => Instruction,
                },
                CloseStart => {
                    if !byte.is_ascii_alphabetic() {
                        return None;
                    }
                    CloseName
                }
                Name | CloseName
                    if byte.is_ascii_alphanumeric()
                        || byte == b'-'
                        || (namespaces && byte == b':') =>
                {
                    *self
                }
                CloseName => {
                    *self = CloseSpace;
                    continue;
                }
                CloseSpace => match byte {
                    b' ' | b'\t' | b'\n' => CloseSpace,
                    b'>' => return Some(true),
                    _ => return None,
                },
                Name => {
                    if !matches!(byte, b' ' | b'\t' | b'\n' | b'/' | b'>') {
                        return None;
                    }
                    *self = Between;
                    continue;
                }
                Between => match byte {
                    b' ' | b'\t' | b'\n' => Between,
                    b'/' => End,
                    b':' | b'_' | b'A'..=b'Z' | b'a'..=b'z' => Attribute,
                    b'>' => return Some(true),
                    _ => return None,
                },
                Attribute
                    if byte.is_ascii_alphanumeric()
                        || matches!(byte, b'-' | b'.' | b':' | b'_') =>
                {
                    Attribute
                }
                Attribute => {
                    *self = AfterAttribute;
                    continue;
                }
                AfterAttribute => match byte {
                    b' ' | b'\t' | b'\n' => AfterAttribute,
                    b'=' => BeforeValue,
                    _ => {
                        *self = Between;
                        continue;
                    }
                },
                BeforeValue => match byte {
                    b'<' | b'=' | b'>' | b'`' => return None,
                    b' ' | b'\t' | b'\n' => BeforeValue,
                    b'"' | b'\'' => Quoted(byte),
                    _ => Unquoted,
                },
                Quoted(quote) => {
                    if byte == quote {
                        AfterQuoted
                    } else {
                        *self
                    }
                }
                AfterQuoted => {
                    if !matches!(byte, b' ' | b'\t' | b'\n' | b'/' | b'>') {
                        return None;
                    }
                    *self = Between;
                    continue;
                }
                Unquoted => match byte {
                    b'"' | b'\'' | b'<' | b'=' | b'`' => return None,
                    b' ' | b'\t' | b'\n' | b'/' | b'>' => {
                        *self = Between;
                        continue;
                    }
                    _ => Unquoted,
                },
                End => {
                    if byte == b'>' {
                        return Some(true);
                    }
                    return None;
                }
            };
            return Some(false);
        }
    }
}

// Whether each line ending in `bytes` is followed by something other than a space, a tab or
// another line ending. The parser strips the spaces and tabs at the start of each continuation line
// of inline HTML, as of its paragraph (#256), and a blank line would end the paragraph.
fn continuation_lines_start(bytes: &[u8]) -> bool {
    !bytes
        .windows(2)
        .any(|pair| pair[0] == b'\n' && matches!(pair[1], b'\n' | b' ' | b'\t'))
}
