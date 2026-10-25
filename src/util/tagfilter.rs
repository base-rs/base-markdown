// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Make dangerous HTML a tiny bit safer.
use crate::util::constant::{HTML_TAGFILTER_NAMES, HTML_TAGFILTER_SIZE_MAX};
#[cfg(test)]
use alloc::string::String;

/// Make dangerous HTML a tiny bit safer.
///
/// The tagfilter is kinda weird and kinda useless.
/// The tag filter is a naïve attempt at XSS protection.
/// You should use a proper HTML sanitizing algorithm.
///
/// ## Examples
///
/// ```rust ignore
/// use base_markdown::util::tagfilter::tagfilter;
///
/// assert_eq!(tagfilter("<iframe>"), "&lt;iframe>");
/// ```
///
/// ## References
///
/// * [*§ 6.1 Disallowed Raw HTML (extension)* in GFM](https://github.github.com/gfm/#disallowed-raw-html-extension-)
/// * [`cmark-gfm#extensions/tagfilter.c`](https://github.com/github/cmark-gfm/blob/master/extensions/tagfilter.c)
#[cfg(test)]
fn tagfilter(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    write_filtered(value, |value| {
        result.push_str(value);
        Ok::<_, core::convert::Infallible>(())
    })
    .unwrap();
    result
}

/// Emit source runs and escaped tag openers without constructing filtered HTML.
/// NUL replacement and any further escaping remain the caller's responsibility.
pub(crate) fn write_filtered<E>(
    value: &str,
    mut write: impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;
    while let Some(offset) = memchr::memchr(b'<', &bytes[index..]) {
        index += offset;
        let name_start = index + 1 + usize::from(bytes.get(index + 1) == Some(&b'/'));
        let mut name_end = name_start;
        while name_end < bytes.len()
            && name_end - name_start < HTML_TAGFILTER_SIZE_MAX
            && bytes[name_end].is_ascii_alphabetic()
        {
            name_end += 1;
        }
        if (name_end == bytes.len()
            || matches!(
                bytes[name_end],
                b'\t' | b'\n' | 12 | b'\r' | b' ' | b'/' | b'>'
            ))
            && HTML_TAGFILTER_NAMES
                .iter()
                .any(|name| bytes[name_start..name_end].eq_ignore_ascii_case(name.as_bytes()))
        {
            if start < index {
                write(&value[start..index])?;
            }
            write("&lt;")?;
            start = index + 1;
        }
        // Tag names contain no '<', including names too long to be filtered.
        index = name_end;
    }
    write(&value[start..])
}

#[cfg(test)]
mod tests {
    use super::{tagfilter, write_filtered};

    #[test]
    fn streams_tag_matches_without_changing_other_bytes() {
        for length in [0, 1, 15, 16, 17, 31, 32, 33, 64] {
            let prefix = "a".repeat(length);
            for name in crate::util::constant::HTML_TAGFILTER_NAMES {
                let upper = name.to_ascii_uppercase();
                let source = alloc::format!("{prefix}é😀<{upper}>body</{name}>");
                assert_eq!(
                    tagfilter(&source),
                    alloc::format!("{prefix}é😀&lt;{upper}>body&lt;/{name}>")
                );
            }
        }
        for source in [
            "",
            "é😀",
            "<",
            "</",
            "<>",
            "</>",
            "<é>",
            "<scriptx>",
            "<script=source>",
            "<script\0>",
            "<plaintextlong>",
        ] {
            assert_eq!(tagfilter(source), source);
        }
        assert_eq!(
            tagfilter("<<sCrIpT>\0</ScRiPt>"),
            "<&lt;sCrIpT>\0&lt;/ScRiPt>"
        );
    }

    #[test]
    fn propagates_failure_from_every_emitted_run() {
        let source = "é<script>\0</script>😀";
        let mut calls = 0;
        write_filtered(source, |_| {
            calls += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
        for remaining in 0..calls {
            let mut successful = 0;
            assert_eq!(
                write_filtered(source, |_| {
                    if successful == remaining {
                        return Err("write failed");
                    }
                    successful += 1;
                    Ok(())
                }),
                Err("write failed")
            );
            assert_eq!(successful, remaining);
        }
    }

    #[test]
    fn filters_disallowed_tags_with_html_separators() {
        for separator in ['\t', '\n', '\u{000c}', '\r', ' ', '/', '>'] {
            let input = alloc::format!("<script{separator}");
            assert_eq!(tagfilter(&input), input.replacen('<', "&lt;", 1));
        }

        assert_eq!(tagfilter("<script"), "&lt;script");
        assert_eq!(tagfilter("<script=source>"), "<script=source>");
        assert_eq!(tagfilter("<scriptx>"), "<scriptx>");
        assert_eq!(tagfilter("</script>"), "&lt;/script>");
    }
}
