// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Allocation-free recognition of HTML blocks with explicit closing conditions.
use crate::util::constant::{HTML_BLOCK_NAMES, HTML_RAW_NAMES};

#[derive(Clone, Copy)]
pub(super) enum Html {
    Raw,
    Comment,
    Instruction,
    Declaration,
    Cdata,
    Basic,
    Complete,
}

impl Html {
    /// Types 1–6 interrupt paragraphs; complete custom tags only start fresh blocks.
    pub fn start(bytes: &[u8], namespaces: bool, complete: bool) -> Option<Self> {
        if bytes.first() != Some(&b'<') {
            return None;
        }
        if bytes.starts_with(b"<!--") {
            return Some(Self::Comment);
        }
        if bytes.starts_with(b"<?") {
            return Some(Self::Instruction);
        }
        if bytes.starts_with(b"<![CDATA[") {
            return Some(Self::Cdata);
        }
        if bytes.starts_with(b"<!") && bytes.get(2).is_some_and(u8::is_ascii_alphabetic) {
            return Some(Self::Declaration);
        }
        let closing = bytes.get(1) == Some(&b'/');
        let start = 1 + usize::from(closing);
        let mut end = start;
        while bytes.get(end).is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || *byte == b'-' || namespaces && *byte == b':'
        }) {
            end += 1;
        }
        let name = &bytes[start..end];
        let boundary = matches!(
            bytes.get(end),
            None | Some(b' ' | b'\t' | b'\r' | b'\n' | b'>')
        );
        if !closing
            && boundary
            && HTML_RAW_NAMES
                .iter()
                .any(|tag| name.eq_ignore_ascii_case(tag.as_bytes()))
        {
            Some(Self::Raw)
        } else if (boundary || bytes[end..].starts_with(b"/>"))
            && HTML_BLOCK_NAMES
                .iter()
                .any(|tag| name.eq_ignore_ascii_case(tag.as_bytes()))
        {
            Some(Self::Basic)
        } else if complete && name.first().is_some_and(u8::is_ascii_alphabetic) {
            let mut scanner = crate::util::html_text::HtmlText::default();
            for (index, &byte) in bytes[1..].iter().enumerate() {
                if matches!(byte, b'\r' | b'\n') {
                    return None;
                }
                match scanner.advance(byte, namespaces) {
                    Some(true) => {
                        return bytes[index + 2..]
                            .iter()
                            .take_while(|&&byte| !matches!(byte, b'\r' | b'\n'))
                            .all(|byte| matches!(byte, b' ' | b'\t'))
                            .then_some(Self::Complete)
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            None
        } else {
            None
        }
    }

    pub fn blank_terminated(self) -> bool {
        matches!(self, Self::Basic | Self::Complete)
    }

    pub fn closed(self, bytes: &[u8]) -> bool {
        match self {
            Self::Raw => {
                let mut at = 0;
                while let Some(offset) = memchr::memchr(b'<', &bytes[at..]) {
                    at += offset + 1;
                    if bytes.get(at) == Some(&b'/') {
                        let name = &bytes[at + 1..];
                        if HTML_RAW_NAMES.iter().any(|tag| {
                            name.get(..tag.len())
                                .is_some_and(|name| name.eq_ignore_ascii_case(tag.as_bytes()))
                                && name.get(tag.len()) == Some(&b'>')
                        }) {
                            return true;
                        }
                    }
                }
                false
            }
            Self::Comment => memchr::memmem::find(bytes, b"-->").is_some(),
            Self::Instruction => memchr::memmem::find(bytes, b"?>").is_some(),
            Self::Declaration => memchr::memchr(b'>', bytes).is_some(),
            Self::Cdata => memchr::memmem::find(bytes, b"]]>").is_some(),
            Self::Basic | Self::Complete => false,
        }
    }
}
