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
}

impl Html {
    /// Complete custom tags retain grammar fallback; types 1–6 can interrupt paragraphs.
    pub fn start(bytes: &[u8]) -> Option<Self> {
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
        while bytes.get(end).is_some_and(u8::is_ascii_alphanumeric) {
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
        } else {
            None
        }
    }

    pub fn blank_terminated(self) -> bool {
        matches!(self, Self::Basic)
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
            Self::Basic => false,
        }
    }
}
