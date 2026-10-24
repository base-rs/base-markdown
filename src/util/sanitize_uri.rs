// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Make urls safe.
use alloc::string::String;

const URI_SAFE: [bool; 256] = {
    let mut safe = [false; 256];
    let mut byte = 0_u8;
    while byte < 128 {
        safe[byte as usize] = matches!(byte, b'!' | b'#' | b'$' | b'\''..=b';' | b'=' | b'?'..=b'Z' | b'_' | b'a'..=b'z' | b'~');
        byte += 1;
    }
    safe
};

/// Make a value safe for injection as a URL.
///
/// This encodes unsafe characters with percent-encoding and skips already
/// encoded sequences (see `normalize` below).
/// Further unsafe characters are encoded as character references (see
/// `encode`).
///
/// ## Examples
///
/// ```rust ignore
/// use core_markdown::util::sanitize_uri::sanitize;
///
/// assert_eq!(sanitize("javascript:alert(1)"), "javascript:alert(1)");
/// assert_eq!(sanitize("https://a👍b.c/%20/%"), "https://a%F0%9F%91%8Db.c/%20/%25");
/// ```
///
/// ## References
///
/// * [`micromark-util-sanitize-uri` in `micromark`](https://github.com/micromark/micromark/tree/main/packages/micromark-util-sanitize-uri)
#[must_use]
pub fn sanitize(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    write_sanitized(value, None, |value| {
        output.push_str(value);
        Ok::<_, core::convert::Infallible>(())
    })
    .unwrap();
    output
}

/// Make a value safe for injection as a URL, and check protocols.
///
/// This first uses [`sanitize`][].
/// Then, a vec of (lowercase) allowed protocols can be given, in which case
/// the URL is ignored or kept.
///
/// For example, `&["http", "https", "irc", "ircs", "mailto", "xmpp"]`
/// can be used for `a[href]`, or `&["http", "https"]` for `img[src]`.
/// If the URL includes an unknown protocol (one not matched by `protocol`, such
/// as a dangerous example, `javascript:`), the value is ignored.
///
/// ## Examples
///
/// ```rust ignore
/// use core_markdown::util::sanitize_uri::sanitize_with_protocols;
///
/// assert_eq!(sanitize_with_protocols("javascript:alert(1)", &["http", "https"]), "");
/// assert_eq!(sanitize_with_protocols("https://example.com", &["http", "https"]), "https://example.com");
/// assert_eq!(sanitize_with_protocols("https://a👍b.c/%20/%", &["http", "https"]), "https://a%F0%9F%91%8Db.c/%20/%25");
/// ```
///
/// ## References
///
/// * [`micromark-util-sanitize-uri` in `micromark`](https://github.com/micromark/micromark/tree/main/packages/micromark-util-sanitize-uri)
#[cfg(test)]
fn sanitize_with_protocols(value: &str, protocols: &[&str]) -> String {
    let mut output = String::with_capacity(value.len());
    write_sanitized(value, Some(protocols), |value| {
        output.push_str(value);
        Ok::<_, core::convert::Infallible>(())
    })
    .unwrap();
    output
}

/// Write normalized, HTML-safe URL byte runs without temporary strings.
pub(crate) fn write_sanitized<E>(
    value: &str,
    protocols: Option<&[&str]>,
    mut write: impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if let Some(protocols) = protocols {
        if let Some(end) = value.find([':', '?', '#', '/']) {
            if value.as_bytes()[end] == b':'
                && !protocols
                    .iter()
                    .any(|protocol| value[..end].eq_ignore_ascii_case(protocol))
            {
                return Ok(());
            }
        }
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;
    while index < bytes.len() {
        index += bytes[index..]
            .iter()
            .position(|byte| !URI_SAFE[usize::from(*byte)])
            .unwrap_or(bytes.len() - index);
        if index == bytes.len() {
            break;
        }
        let byte = bytes[index];
        // Preserve the historical acceptance of two alphanumeric bytes, not just hex digits.
        if byte == b'%'
            && bytes.get(index + 1).is_some_and(u8::is_ascii_alphanumeric)
            && bytes.get(index + 2).is_some_and(u8::is_ascii_alphanumeric)
        {
            index += 3;
            continue;
        }
        if start < index {
            write(&value[start..index])?;
        }
        if byte == b'&' {
            write("&amp;")?;
        } else {
            let encoded = [
                b'%',
                HEX[usize::from(byte >> 4)],
                HEX[usize::from(byte & 15)],
            ];
            // Every byte is an ASCII percent sign or hex digit.
            write(core::str::from_utf8(&encoded).unwrap())?;
        }
        index += 1;
        start = index;
    }
    if start < value.len() || value.is_empty() {
        write(&value[start..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{sanitize, sanitize_with_protocols, write_sanitized};
    use alloc::{string::String, vec::Vec};

    #[test]
    fn writes_utf8_percent_sequences_protocols_and_nulls_without_intermediates() {
        assert_eq!(
            sanitize("https://a👍b.c/%20/%GG/%/\0?x=é&y=\"<>"),
            "https://a%F0%9F%91%8Db.c/%20/%GG/%25/%00?x=%C3%A9&amp;y=%22%3C%3E"
        );
        assert_eq!(
            sanitize_with_protocols("HtTpS://example.com?q=x&y=z", &["https"]),
            "HtTpS://example.com?q=x&amp;y=z"
        );
        assert_eq!(
            sanitize_with_protocols("relative/path:ok", &[]),
            "relative/path:ok"
        );
        assert_eq!(sanitize_with_protocols("javascript:x", &["https"]), "");
        for (input, expected) in [
            ("%A", "%25A"),
            ("%AA", "%AA"),
            ("%GG", "%GG"),
            ("%a9", "%a9"),
            ("%A_", "%25A_"),
            ("%é", "%25%C3%A9"),
            ("%%41", "%25%41"),
            ("%41%", "%41%25"),
            ("left&&right", "left&amp;&amp;right"),
        ] {
            assert_eq!(sanitize(input), expected);
        }
        let mut output = String::new();
        write_sanitized("é\0%é", None, |run| {
            output.push_str(run);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(output, "%C3%A9%00%25%C3%A9");
        for input in ["ordinary", "&", "é", "\0"] {
            assert!(write_sanitized(input, None, |_| Err(())).is_err());
        }
        // Every ASCII byte and multibyte scalar can occur at a run boundary.
        let bytes: Vec<_> = (0..128).collect();
        let input = core::str::from_utf8(&bytes).unwrap();
        let output = sanitize(input);
        assert_eq!(
            output,
            concat!(
                "%00%01%02%03%04%05%06%07%08%09%0A%0B%0C%0D%0E%0F",
                "%10%11%12%13%14%15%16%17%18%19%1A%1B%1C%1D%1E%1F",
                "%20!%22#$%25&amp;'()*+,-./0123456789:;%3C=%3E?@ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "%5B%5C%5D%5E_%60abcdefghijklmnopqrstuvwxyz%7B%7C%7D~%7F"
            )
        );
    }
}
