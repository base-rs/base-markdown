// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Small scanners for Markdown formatting decisions.
use alloc::vec::Vec;
use core::ops::Range;

pub fn line_endings(value: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let bytes = value.as_bytes();
    let mut at = 0;
    core::iter::from_fn(move || {
        let start = at + memchr::memchr2(b'\r', b'\n', &bytes[at..])?;
        at = start + 1;
        if bytes[start] == b'\r' && bytes.get(at) == Some(&b'\n') {
            at += 1;
        }
        Some(start..at)
    })
}

// A delimiter must have a length absent from the content's complete marker runs.
pub fn delimiter_size(value: &str, marker: u8, minimum: usize) -> usize {
    let bytes = value.as_bytes();
    let mut lengths = Vec::new();
    let mut at = 0;
    while let Some(offset) = memchr::memchr(marker, &bytes[at..]) {
        let start = at + offset;
        at = start + 1;
        while bytes.get(at) == Some(&marker) {
            at += 1;
        }
        lengths.push(at - start);
    }
    lengths.sort_unstable();
    let mut size = minimum;
    for length in lengths {
        if length == size {
            size += 1;
        }
    }
    size
}

#[cfg(test)]
mod tests {
    use super::{delimiter_size, line_endings};
    use alloc::vec::Vec;

    #[test]
    fn preserves_physical_line_endings_and_marker_runs() {
        let value = "é\r\nα\rb\n";
        assert_eq!(
            line_endings(value)
                .map(|range| &value[range])
                .collect::<Vec<_>>(),
            ["\r\n", "\r", "\n"]
        );
        assert_eq!(delimiter_size("ordinary", b'`', 1), 1);
        assert_eq!(delimiter_size("`` é ` ```` `", b'`', 1), 3);
        assert_eq!(delimiter_size("$$ x $$$", b'$', 2), 4);
        assert_eq!(delimiter_size("$$$ $$$", b'$', 2), 2);
    }

    #[test]
    fn backslash_scanning_matches_punctuation_rules_across_context_boundaries() {
        use super::super::safe::escape_backslashes;
        use alloc::{format, string::String};
        let expression = regex::Regex::new(r"\\[!-/:-@\[-`{-~]").unwrap();
        for value in ["", "\\", "\\\\", "\\\\\\", "é\\!\\α", "\\!\\# \\"] {
            for after in (0..=127).map(char::from) {
                let after = String::from(after);
                let combined = format!("{value}{after}");
                let mut expected = String::new();
                let mut start = 0;
                for found in expression.find_iter(&combined) {
                    expected.push_str(&value[start..found.start()]);
                    expected.push('\\');
                    start = found.start();
                }
                expected.push_str(&value[start..]);
                assert_eq!(
                    escape_backslashes(value, &after),
                    expected,
                    "{value:?}, {after:?}"
                );
            }
        }
    }
}
