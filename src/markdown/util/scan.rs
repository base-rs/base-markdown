// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Small scanners for Markdown formatting decisions.
use alloc::vec::Vec;
use core::ops::Range;

/// Complete runs of an ASCII syntax marker, skipping ordinary bytes together.
pub fn marker_runs(bytes: &[u8], marker: u8) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut at = 0;
    core::iter::from_fn(move || {
        let offset = memchr::memchr(marker, &bytes[at..])?;
        let start = at + offset;
        at = marker_run_end(bytes, start + 1, marker);
        Some(start..at)
    })
}

// The first matching byte is already known. Continue from the following byte.
pub fn marker_run_end(bytes: &[u8], mut at: usize, marker: u8) -> usize {
    while bytes.get(at) == Some(&marker) {
        at += 1;
    }
    at
}

pub fn longest_run(value: &str, marker: u8) -> usize {
    marker_runs(value.as_bytes(), marker)
        .map(|run| run.len())
        .max()
        .unwrap_or(0)
}

// A delimiter must have a length absent from the content's complete marker runs.
pub fn delimiter_size(value: &str, marker: u8, minimum: usize) -> usize {
    let mut used = 0_usize;
    let mut lengths = Vec::new();
    for run in marker_runs(value.as_bytes(), marker) {
        if let Some(offset) = run.len().checked_sub(minimum) {
            if offset < usize::BITS as usize {
                used |= 1 << offset;
            } else {
                lengths.push(run.len());
            }
        }
    }
    if used != usize::MAX {
        return minimum + used.trailing_ones() as usize;
    }
    // Only long runs need storage. Sorting them when the word is full preserves bounded work even for dense contents.
    let mut size = minimum + usize::BITS as usize;
    lengths.sort_unstable();
    for length in lengths {
        if length == size {
            size += 1;
        }
    }
    size
}

#[cfg(test)]
mod tests {
    use super::{delimiter_size, longest_run, marker_runs};
    use alloc::vec::Vec;

    #[test]
    fn preserves_marker_runs() {
        assert_eq!(delimiter_size("ordinary", b'`', 1), 1);
        assert_eq!(delimiter_size("`` é ` ```` `", b'`', 1), 3);
        assert_eq!(delimiter_size("$$ x $$$", b'$', 2), 4);
        assert_eq!(delimiter_size("$$$ $$$", b'$', 2), 2);
        assert_eq!(longest_run("", b'`'), 0);
        assert_eq!(longest_run("plain é α", b'`'), 0);
        assert_eq!(longest_run("`é``α````", b'`'), 4);
        assert_eq!(longest_run("~~~ é ~~", b'~'), 3);
        assert_eq!(longest_run("$$ α $$$$", b'$'), 4);
        assert_eq!(
            marker_runs(b"`a``b```", b'`').collect::<Vec<_>>(),
            [0..1, 2..4, 5..8]
        );
    }

    #[test]
    fn delimiter_selection_preserves_gaps_across_bitset_windows() {
        use alloc::string::String;
        for marker in *b"`$" {
            let mut value = String::new();
            for length in 1..=2 * usize::BITS as usize + 1 {
                value.extend(core::iter::repeat(char::from(marker)).take(length));
                value.push(' ');
                assert_eq!(delimiter_size(&value, marker, 1), length + 1);
                assert_eq!(delimiter_size(&value, marker, 2), length + 1);
            }
            for gap in [1, 2, usize::BITS as usize, usize::BITS as usize + 1] {
                let mut value = String::new();
                for length in (1..=2 * usize::BITS as usize + 1).rev() {
                    if length != gap {
                        value.extend(core::iter::repeat(char::from(marker)).take(length));
                        value.push(' ');
                    }
                }
                assert_eq!(delimiter_size(&value, marker, 1), gap);
            }
        }
    }

    #[test]
    fn backslash_scanning_matches_punctuation_rules_across_context_boundaries() {
        use super::super::safe::escape_backslashes;
        use alloc::{format, string::String};
        let expression = regex::Regex::new(r"\\[!-/:-@\[-`{-~]").unwrap();
        let punctuation = regex::Regex::new(r"^[!-/:-@\[-`{-~]").unwrap();
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
                    escape_backslashes(value, &after, false),
                    expected,
                    "{value:?}, {after:?}"
                );
                let mut expected = String::new();
                for (index, character) in value.char_indices() {
                    if character == '\\' && punctuation.is_match(&combined[index + 1..]) {
                        expected.push('\\');
                    }
                    expected.push(character);
                }
                assert_eq!(
                    escape_backslashes(value, &after, true),
                    expected,
                    "{value:?}, {after:?}"
                );
            }
        }
    }
}
