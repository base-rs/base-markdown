// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Helpers for character references.
use crate::util::constant::{
    CHARACTER_REFERENCES, CHARACTER_REFERENCE_DECIMAL_SIZE_MAX,
    CHARACTER_REFERENCE_HEXADECIMAL_SIZE_MAX, CHARACTER_REFERENCE_NAMED_SIZE_MAX,
};
use alloc::string::String;
use core::str;

fn decode_numeric_char(value: &str, radix: u32) -> char {
    if let Some(char) = char::from_u32(u32::from_str_radix(value, radix).unwrap()) {
        if !matches!(char,
            // C0 except for HT, LF, FF, CR, space.
            '\0'..='\u{08}' | '\u{0B}' | '\u{0E}'..='\u{1F}' |
            // Control character (DEL) of C0, and C1 controls.
            '\u{7F}'..='\u{9F}'
            // Lone surrogates, noncharacters, and out of range are handled by
            // Rust.
        ) {
            return char;
        }
    }

    char::REPLACEMENT_CHARACTER
}

/// A resolved reference that requires no owned string until its text is consumed.
pub(crate) enum Reference {
    Named(usize),
    Numeric(char),
}

/// Scan an HTML 5 character reference beginning at `start`, without reading past `end`.
/// Both bounds refer to the complete source; callers check the opening ampersand.
pub(crate) fn scan(source: &str, start: usize, end: usize) -> Option<(usize, Reference)> {
    let bytes = &source.as_bytes()[..end];
    let mut begin = start + 1;
    let mut marker = b'&';
    if bytes.get(begin) == Some(&b'#') {
        marker = b'#';
        begin += 1;
        if matches!(bytes.get(begin), Some(b'x' | b'X')) {
            marker = b'x';
            begin += 1;
        }
    }
    let mut at = begin;
    while at < end && at - begin < value_max(marker) && value_test(marker)(&bytes[at]) {
        at += 1;
    }
    if at == begin || at == end || bytes[at] != b';' {
        return None;
    }
    let value = if marker == b'&' {
        let index = CHARACTER_REFERENCES
            .binary_search_by_key(&&source[begin..at], |entry| entry.0)
            .ok()?;
        Reference::Named(index)
    } else {
        Reference::Numeric(decode_numeric_char(
            &source[begin..at],
            if marker == b'#' { 10 } else { 16 },
        ))
    };
    Some((at + 1, value))
}

/// Get the maximum size of a value for different kinds of references.
///
/// The value is the stuff after the markers, before the `;`.
///
/// ## Panics
///
/// Panics if `marker` is not `b'&'`, `b'x'`, or `b'#'`.
pub fn value_max(marker: u8) -> usize {
    match marker {
        b'&' => CHARACTER_REFERENCE_NAMED_SIZE_MAX,
        b'x' => CHARACTER_REFERENCE_HEXADECIMAL_SIZE_MAX,
        b'#' => CHARACTER_REFERENCE_DECIMAL_SIZE_MAX,
        _ => unreachable!("Unexpected marker `{}`", marker),
    }
}

/// Get a test to check if a byte is allowed as a value for different kinds of
/// references.
///
/// The value is the stuff after the markers, before the `;`.
///
/// ## Panics
///
/// Panics if `marker` is not `b'&'`, `b'x'`, or `b'#'`.
pub fn value_test(marker: u8) -> fn(&u8) -> bool {
    match marker {
        b'&' => u8::is_ascii_alphanumeric,
        b'x' => u8::is_ascii_hexdigit,
        b'#' => u8::is_ascii_digit,
        _ => unreachable!("Unexpected marker `{}`", marker),
    }
}

#[cfg(test)]
mod tests {
    use super::CHARACTER_REFERENCES;
    #[test]
    fn named_reference_table_remains_sorted_and_complete() {
        assert!(CHARACTER_REFERENCES
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0));
        for (expected, &(name, _)) in CHARACTER_REFERENCES.iter().enumerate() {
            let source = alloc::format!("&{name};");
            let (end, reference) = super::scan(&source, 0, source.len()).unwrap();
            assert_eq!(end, source.len());
            assert!(matches!(reference, super::Reference::Named(index) if index == expected));
        }
    }

    #[test]
    fn bounded_scanning_requires_the_semicolon_inside_the_region() {
        for (source, expected) in [
            ("&amp;", "&"),
            ("&NotEqualTilde;", "≂̸"),
            ("&#65;", "A"),
            ("&#x41;", "A"),
            ("&#X1F600;", "😀"),
            ("&#0;", "�"),
            ("&#55296;", "�"),
            ("&#x110000;", "�"),
        ] {
            let input = alloc::format!("é{source}tail");
            let start = "é".len();
            let finish = start + source.len();
            for end in start..finish {
                assert!(
                    super::scan(&input, start, end).is_none(),
                    "{:?}, end={}",
                    input,
                    end
                );
            }
            let (end, reference) = super::scan(&input, start, finish).unwrap();
            assert_eq!(end, finish);
            let actual: alloc::string::String = match reference {
                super::Reference::Named(index) => CHARACTER_REFERENCES[index].1.into(),
                super::Reference::Numeric(character) => character.into(),
            };
            assert_eq!(actual, expected);
        }
        for source in [
            "&",
            "&#",
            "&#x",
            "&;",
            "&#;",
            "&#x;",
            "&amp",
            "&unknown;",
            "&amp ;",
            "&amp!;",
            "&#xg;",
            "&#12345678;",
            "&#x1234567;",
            "&ThisNameIsLongerThanAnyValidNamedReference;",
        ] {
            assert!(
                super::scan(source, 0, source.len()).is_none(),
                "{:?}",
                source
            );
        }
    }

    use super::{decode_numeric_char, value_max, value_test};

    #[test]
    fn replaces_disallowed_numeric_codepoints_but_keeps_ascii_whitespace() {
        assert_eq!(decode_numeric_char("128", 10), '�');
        assert_eq!(decode_numeric_char("55296", 10), '�');
        assert_eq!(decode_numeric_char("9", 10), '\t');
    }

    #[test]
    #[should_panic(expected = "Unexpected marker")]
    fn value_max_rejects_unknown_markers() {
        value_max(b'?');
    }

    #[test]
    #[should_panic(expected = "Unexpected marker")]
    fn value_test_rejects_unknown_markers() {
        value_test(b'?');
    }
}
