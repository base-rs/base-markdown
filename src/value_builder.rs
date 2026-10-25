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
                // Empty fragments retain their origin only for an entirely empty value.
                if range.start == range.end {
                    return;
                }
                if previous.start == previous.end {
                    *previous = range.clone();
                    return;
                }
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
    use super::{decode_string_with_options, decode_unpadded, ValueBuilder};
    use crate::{stream::Text, ParseOptions};
    use alloc::{borrow::Cow, string::String};

    #[test]
    fn empty_fragments_preserve_contiguous_source_values() {
        let source = "abcdef";
        for (ranges, expected) in [
            ([0..0, 1..6, 6..6], 1..6),
            ([1..3, 0..0, 3..6], 1..6),
            ([1..3, 3..6, 0..0], 1..6),
            ([0..2, 2..4, 4..6], 0..6),
            ([0..0, 5..5, 2..2], 0..0),
            ([3..3, 5..5, 5..5], 3..3),
        ] {
            let mut value = ValueBuilder::new(source);
            for range in ranges {
                value.append(Text::Source(range));
            }
            assert_eq!(value.finish(), Text::Source(expected));
        }
        let mut value = ValueBuilder::new(source);
        value.append(Text::Source(3..3));
        assert_eq!(value.finish(), Text::Source(3..3));
        assert_eq!(ValueBuilder::new(source).finish(), Text::Literal(""));
    }

    #[test]
    fn discontinuous_fragments_still_materialize_their_value() {
        let source = "abcdef";
        let mut value = ValueBuilder::new(source);
        for range in [4..4, 0..1, 3..6, 0..0] {
            value.append(Text::Source(range));
        }
        assert_eq!(
            value.finish(),
            Text::Value(Cow::Owned(String::from("adef")))
        );
    }

    #[test]
    fn literal_and_owned_values_preserve_contents_across_empty_fragments() {
        let source = "abc";
        for text in [
            Text::Literal("&"),
            Text::Value(Cow::Borrowed("&")),
            Text::Value(Cow::Owned(String::from("&"))),
        ] {
            let mut value = ValueBuilder::new(source);
            value.append(Text::Source(1..1));
            value.append(text);
            value.append(Text::Source(3..3));
            value.append(Text::Source(0..3));
            assert_eq!(
                value.finish(),
                Text::Value(Cow::Owned(String::from("&abc")))
            );
        }
        let mut value = ValueBuilder::new(source);
        value.append(Text::Value(Cow::Owned(String::new())));
        value.append(Text::Source(1..1));
        assert_eq!(value.finish(), Text::Value(Cow::Owned(String::new())));
    }

    #[test]
    fn leading_escaped_punctuation_keeps_a_source_range() {
        let options = ParseOptions::default();
        for source in [r"\*word", r"\*"] {
            assert_eq!(
                decode_string_with_options(source, 0..source.len(), &options),
                Text::Source(1..source.len())
            );
        }
        assert_eq!(
            decode_string_with_options("abc", 2..2, &options),
            Text::Source(2..2)
        );
        let source = r"\*word";
        let mut options = options;
        options.constructs.character_escape = false;
        assert_eq!(
            decode_string_with_options(source, 0..source.len(), &options),
            Text::Source(0..source.len())
        );
    }

    #[test]
    fn decoded_replacements_and_discontinuous_escapes_keep_their_contents() {
        for (source, expected) in [
            (r"\*\!", "*!"),
            (r"word\*", "word*"),
            (r"a\*b", "a*b"),
            ("&amp;", "&"),
            ("&#42;", "*"),
            ("\0", "�"),
        ] {
            assert_eq!(
                decode_string_with_options(source, 0..source.len(), &ParseOptions::default()),
                Text::Value(Cow::Owned(String::from(expected)))
            );
        }
    }

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
