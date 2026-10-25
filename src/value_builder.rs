// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Source-backed text construction and syntax-aware string decoding.
use crate::{stream::Text, util::constant::CHARACTER_REFERENCES};
use alloc::{
    borrow::Cow,
    string::{String, ToString},
};
use core::ops::Range;

static STRING_MARKERS: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(b"\\&\0");
static UNPADDED_MARKERS: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(b"\\&\0\r\n");

/// Keep a contiguous source range until normalization actually requires an owned string.
pub(crate) struct ValueBuilder<'a> {
    source: &'a str,
    range: Option<Range<usize>>,
    owned: Option<String>,
}
impl<'a> ValueBuilder<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            range: None,
            owned: None,
        }
    }
    pub(crate) fn append(&mut self, text: Text<'_>) {
        if self.range.is_none() && self.owned.is_none() {
            if let Text::Value(alloc::borrow::Cow::Owned(value)) = text {
                self.owned = Some(value);
                return;
            }
        }
        if let Some(owned) = &mut self.owned {
            owned.push_str(text.resolve(self.source).unwrap());
            return;
        }
        if let Text::Source(range) = &text {
            if let Some(previous) = &mut self.range {
                if previous.end == range.start {
                    previous.end = range.end;
                    return;
                }
            } else {
                self.range = Some(range.clone());
                return;
            }
        }
        let mut owned = self
            .range
            .take()
            .map_or_else(String::new, |range| self.source[range].to_string());
        owned.push_str(text.resolve(self.source).unwrap());
        self.owned = Some(owned);
    }
    pub(crate) fn finish(self) -> Text<'static> {
        match self.owned {
            Some(owned) => Text::from(owned),
            None => self.range.map_or(Text::Literal(""), Text::Source),
        }
    }
}

pub(crate) fn decode_string(source: &str, range: Range<usize>) -> Text<'static> {
    decode::<false>(source, range, true, true, false)
}

pub(crate) fn decode_string_with_options(
    source: &str,
    range: Range<usize>,
    options: &crate::ParseOptions,
) -> Text<'static> {
    decode::<false>(
        source,
        range,
        options.constructs.character_escape,
        options.constructs.character_reference,
        true,
    )
}

/// Inline HTML and titles omit physical continuation indentation before decoding.
/// Decoded line endings (such as `&NewLine;`) do not strip following whitespace.
pub(crate) fn decode_unpadded(
    source: &str,
    range: Range<usize>,
    options: Option<&crate::ParseOptions>,
) -> Text<'static> {
    decode::<true>(
        source,
        range,
        options.is_some_and(|options| options.constructs.character_escape),
        options.is_some_and(|options| options.constructs.character_reference),
        true,
    )
}

fn decode<const UNPAD: bool>(
    source: &str,
    range: Range<usize>,
    escapes: bool,
    references: bool,
    replace_nulls: bool,
) -> Text<'static> {
    use crate::util::character_reference::{scan, Reference};
    let bytes = source.as_bytes();
    let mut value = ValueBuilder::new(source);
    let mut start = range.start;
    let mut index = start;
    while index < range.end {
        index += if UNPAD {
            UNPADDED_MARKERS.find(&bytes[index..range.end])
        } else {
            STRING_MARKERS.find(&bytes[index..range.end])
        };
        if index == range.end {
            break;
        }
        if UNPAD && matches!(bytes[index], b'\r' | b'\n') {
            index += 1 + usize::from(bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n'));
            let end = index;
            while index < range.end && matches!(bytes[index], b' ' | b'\t') {
                index += 1;
            }
            if end != index {
                value.append(Text::Source(start..end));
                start = index;
            }
            continue;
        }
        if escapes
            && bytes[index] == b'\\'
            && index + 1 < range.end
            && bytes[index + 1].is_ascii_punctuation()
        {
            value.append(Text::Source(start..index));
            index += 1;
            start = index;
            index += 1;
            continue;
        }
        if replace_nulls && bytes[index] == 0 {
            value.append(Text::Source(start..index));
            value.append(Text::Literal("�"));
            index += 1;
            start = index;
            continue;
        }
        if references && bytes[index] == b'&' {
            if let Some((end, reference)) = scan(source, index, range.end) {
                let mut buffer = [0; 4];
                let decoded = match reference {
                    Reference::Named(index) => Text::Literal(CHARACTER_REFERENCES[index].1),
                    Reference::Numeric(character) => {
                        Text::Value(Cow::Borrowed(character.encode_utf8(&mut buffer)))
                    }
                };
                value.append(Text::Source(start..index));
                value.append(decoded);
                index = end;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    value.append(Text::Source(start..range.end));
    value.finish()
}

#[cfg(test)]
mod tests {
    use super::decode_unpadded;
    use crate::{stream::Text, ParseOptions};

    #[test]
    fn continuation_padding_is_removed_before_entity_and_escape_decoding() {
        let source = "a&NewLine;  b\r\n \t&amp;\\* &#x1F980;\0";
        for (escapes, references, expected) in [
            (true, true, "a\n  b\r\n&* 🦀�"),
            (false, true, "a\n  b\r\n&\\* 🦀�"),
            (true, false, "a&NewLine;  b\r\n&amp;* &#x1F980;�"),
            (false, false, "a&NewLine;  b\r\n&amp;\\* &#x1F980;�"),
        ] {
            let mut options = ParseOptions::default();
            options.constructs.character_escape = escapes;
            options.constructs.character_reference = references;
            assert_eq!(
                decode_unpadded(source, 0..source.len(), Some(&options))
                    .resolve(source)
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            decode_unpadded(source, 0..source.len(), None)
                .resolve(source)
                .unwrap(),
            "a&NewLine;  b\r\n&amp;\\* &#x1F980;�"
        );
        for source in ["", "plain é", "a\nb", "a\rb", "a\r\nb", "&invalid; \\q"] {
            assert_eq!(
                decode_unpadded(source, 0..source.len(), Some(&ParseOptions::default())),
                Text::Source(0..source.len())
            );
        }
    }
}
