// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js
use alloc::string::String;

pub struct EscapeInfos {
    pub after: bool,
    pub before: bool,
    /// Apply only if the escaped `<` would otherwise start an email autolink in the written output.
    pub email: bool,
}

/// JS: <https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/lib/util/safe.js#L148>.
///
/// With `overlapping`, every backslash followed by ASCII punctuation is doubled, including one
/// followed by another backslash. Otherwise a match consumes the following byte, as the JS regex
/// does, so a backslash right after a doubled one is not checked.
pub fn write_backslashes(value: &str, after: &str, overlapping: bool, results: &mut String) {
    let mut start = 0;
    let bytes = value.as_bytes();
    let mut at = 0;
    while let Some(offset) = memchr::memchr(b'\\', &bytes[at..]) {
        let position = at + offset;
        let next = bytes.get(position + 1).or_else(|| after.as_bytes().first());
        if next.is_some_and(u8::is_ascii_punctuation) {
            results.push_str(&value[start..position]);
            results.push('\\');
            start = position;
            if !overlapping {
                at = (position + 2).min(bytes.len());
                continue;
            }
        }
        at = position + 1;
    }
    results.push_str(&value[start..]);
}

#[cfg(test)]
pub fn escape_backslashes(value: &str, after: &str, overlapping: bool) -> String {
    let mut output = String::new();
    write_backslashes(value, after, overlapping, &mut output);
    output
}
