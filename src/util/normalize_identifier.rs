// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Normalize identifiers.
use crate::byte_scan::AsciiSet;
use alloc::string::String;

static WHITESPACE: AsciiSet = AsciiSet::new(b" \t\r\n");

/// Normalize an identifier, as found in [references][label_end] and
/// [definitions][definition], so it can be compared when matching.
///
/// This collapses whitespace found in markdown (`\t`, `\r`, `\n`, and ` `)
/// into one space, trims it (as in, dropping the first and last space), and
/// then performs unicode case folding twice: first by lowercasing uppercase
/// characters, and then uppercasing lowercase characters.
///
/// Some characters are considered “uppercase”, such as U+03F4 (`ϴ`), but if
/// their lowercase counterpart (U+03B8 (`θ`)) is uppercased will result in a
/// different uppercase character (U+0398 (`Θ`)).
/// Hence, to get that form, we perform both lower- and uppercase.
///
/// Performing these steps in that order works, but the inverse does not work.
/// To illustrate, say the source markdown contains two identifiers
/// `SS` (U+0053 U+0053) and `ẞ` (U+1E9E), which would be lowercased to
/// `ss` (U+0073 U+0073) and `ß` (U+00DF), and those in turn would both
/// uppercase to `SS` (U+0053 U+0053).
/// If we’d inverse the steps, for `ẞ`, we’d first uppercase without a
/// change, and then lowercase to `ß`, which would not match `ss`.
///
/// ## Examples
///
/// ```rust ignore
/// core_markdown::util::normalize_identifier::normalize_identifier;
///
/// assert_eq!(normalize_identifier(" a "), "A");
/// assert_eq!(normalize_identifier("a\t\r\nb"), "A B");
/// assert_eq!(normalize_identifier("ПРИВЕТ"), "ПРИВЕТ");
/// assert_eq!(normalize_identifier("Привет"), "ПРИВЕТ");
/// assert_eq!(normalize_identifier("привет"), "ПРИВЕТ");
/// ```
///
/// ## References
///
/// * [`micromark-util-normalize-identifier` in `micromark`](https://github.com/micromark/micromark/tree/main/packages/micromark-util-normalize-identifier)
///
/// [definition]: crate::definitions
/// [label_end]: crate::inline_parser
pub fn normalize_identifier(value: &str) -> String {
    // Note: it’ll grow a bit smaller for consecutive whitespace.
    let mut result = String::with_capacity(value.len());
    normalize_identifier_into(value, &mut result);
    result
}

/// Normalize into reusable storage. ASCII labels need no allocation after capacity is established.
pub(crate) fn normalize_identifier_into(value: &str, result: &mut String) {
    collapse_whitespace_into(value, result);
    if result.is_ascii() {
        result.make_ascii_uppercase();
    } else {
        *result = result.to_lowercase().to_uppercase();
    }
}

/// Reference matching uses lowercase ASCII so canonical source labels can be borrowed.
/// Footnote identifiers retain the original uppercase form because it is visible in HTML.
pub(crate) fn normalize_reference_identifier_into(value: &str, result: &mut String) {
    collapse_whitespace_into(value, result);
    if result.is_ascii() {
        result.make_ascii_lowercase();
    } else {
        *result = result.to_lowercase().to_uppercase();
        result.make_ascii_lowercase();
    }
}

fn collapse_whitespace_into(value: &str, result: &mut String) {
    result.clear();
    let mut remaining = value;
    while !remaining.is_empty() {
        let end = WHITESPACE.find(remaining.as_bytes());
        if end > 0 {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str(&remaining[..end]);
        }
        remaining = &remaining[end..];
        let padding = remaining
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
            .count();
        remaining = &remaining[padding..];
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_preserve_collapsed_spaces_and_unicode_folding() {
        assert_eq!(super::normalize_identifier(" a\tb\r\nc "), "A B C");
        assert_eq!(super::normalize_identifier(""), "");
        assert_eq!(super::normalize_identifier("a  b"), "A B");
        assert_ne!(
            super::normalize_identifier("a b"),
            super::normalize_identifier("ab")
        );
        assert_eq!(super::normalize_identifier("ẞ ϴ"), "SS Θ");
    }

    #[test]
    fn labels_collapse_only_markdown_whitespace() {
        assert_eq!(super::normalize_identifier(" \t\r\n "), "");
        assert_eq!(
            super::normalize_identifier("a\u{a0}b\u{2003}c"),
            "A\u{a0}B\u{2003}C"
        );
        assert_eq!(super::normalize_identifier("İ\tẞ\nϴ"), "I\u{307} SS Θ");
    }

    #[test]
    fn labels_scan_whole_runs_across_chunk_boundaries() {
        for length in [15, 16, 17, 31, 32, 33, 63, 64, 65] {
            let word = "a".repeat(length);
            let value = alloc::format!("\t{word}\r\n  \t{word}\n");
            let expected = alloc::format!("{} {}", word.to_uppercase(), word.to_uppercase());
            let mut result = alloc::string::String::from("old value");
            super::normalize_identifier_into(&value, &mut result);
            assert_eq!(result, expected);
        }
    }
}
