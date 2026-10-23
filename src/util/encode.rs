// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Encode HTML.
use alloc::string::String;

/// Encode dangerous html characters.
///
/// This ensures that certain characters which have special meaning in HTML are
/// dealt with.
/// Technically, we can skip `>` and `"` in many cases, but `CommonMark`
/// includes them.
///
/// This behavior is not explained in prose in `CommonMark` but can be inferred
/// from the input/output test cases.
///
/// ## Examples
///
/// ```rust ignore
/// use core_markdown::util::encode;
///
/// assert_eq!(encode("I <3 🦀"), "I &lt;3 🦀");
/// ```
///
/// ## References
///
/// * [`micromark-util-encode` in `micromark`](https://github.com/micromark/micromark/tree/main/packages/micromark-util-encode)
pub fn encode(value: &str, encode_html: bool) -> String {
    // It’ll grow a bit bigger for each dangerous character.
    let mut result = String::with_capacity(value.len());
    encode_into(value, encode_html, &mut result);
    result
}

/// Encode directly into the caller's output buffer.
pub fn encode_into(value: &str, encode_html: bool, result: &mut String) {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut start = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if matches!(byte, b'\0') || (encode_html && matches!(byte, b'&' | b'"' | b'<' | b'>')) {
            result.push_str(&value[start..index]);
            result.push_str(match byte {
                b'\0' => "�",
                b'&' => "&amp;",
                b'"' => "&quot;",
                b'<' => "&lt;",
                // `b'>'`
                _ => "&gt;",
            });

            start = index + 1;
        }

        index += 1;
    }

    result.push_str(&value[start..]);
}

#[cfg(test)]
mod tests {
    #[test]
    fn raw_html_still_replaces_null_bytes() {
        assert_eq!(super::encode("<&>\"\0", false), "<&>\"�");
        assert_eq!(super::encode("<&>\"\0", true), "&lt;&amp;&gt;&quot;�");
    }
}
