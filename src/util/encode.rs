// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Encode HTML.
use core::fmt::{self, Write};

// Raw HTML is written as it is, as no value holds NUL. Encoded text replaces NUL too, as some of
// it can still hold one: the compile options' strings, and text edited through
// `Document::text_mut`, which cannot check its `String`.
#[inline]
pub(crate) fn find_escape(bytes: &[u8]) -> usize {
    static ESCAPES: crate::byte_scan::AsciiSet = crate::byte_scan::AsciiSet::new(b"\0&\"<>");
    ESCAPES.find(bytes)
}

// `index` must be the first candidate returned by `find_escape`, strictly before the end of `value`. Keep the last successfully written byte even when a caller's writer fails partway through an escape.
pub(crate) fn write_escaped(
    output: &mut impl Write,
    last: &mut Option<u8>,
    value: &str,
    mut index: usize,
) -> fmt::Result {
    let mut start = 0;
    loop {
        let replacement = match value.as_bytes()[index] {
            0 => "�",
            b'&' => "&amp;",
            b'"' => "&quot;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            _ => unreachable!("the candidate scanner returns only HTML escapes"),
        };
        if start != index {
            output.write_str(&value[start..index])?;
            *last = Some(value.as_bytes()[index - 1]);
        }
        output.write_str(replacement)?;
        *last = replacement.as_bytes().last().copied();
        start = index + 1;
        index = start + find_escape(&value.as_bytes()[start..]);
        if index == value.len() {
            break;
        }
    }
    if start < value.len() {
        output.write_str(&value[start..])?;
        *last = value.as_bytes().last().copied();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn encoding_appends_runs_without_reencoding_the_existing_output() {
        for length in [0, 1, 15, 16, 17, 31, 32, 33, 64] {
            let prefix = "a".repeat(length);
            let value = alloc::format!("{prefix}é😀<&\"\0>tail\0");
            let mut output = alloc::string::String::from("existing<&");
            assert_eq!(super::find_escape(b""), 0);
            let index = super::find_escape(value.as_bytes());
            super::write_escaped(&mut output, &mut None, &value, index).unwrap();
            assert_eq!(
                output,
                alloc::format!("existing<&{prefix}é😀&lt;&amp;&quot;�&gt;tail�")
            );
        }
    }
}
